# Visualization Architecture

A website where viewers watch CNA-Sim games play out — current board state, a running log of player
actions, and the rule behind each action. Recorded (scrub a finished game) first; live spectating
(watch as it plays) as a follow-on. Status: **recorded viewer built and deployed** (React + SVG,
`web/`); WASM client-side replay and live spectating remain future work.

## Guiding principle: exploit determinism

The engine is a pure, seeded-deterministic, serde-serializable core with a clean
`Command → CommandOutcome → apply` boundary (see `design.md`). Therefore:

> **A whole game *is* `(initial scenario + seed + ordered command log)`.** Any state can be
> reconstructed by replaying that log.

Consequences that shape everything below:
- The browser never has to reimplement game logic — it can replay the *same* engine code.
- We don't have to stream heavy state; a compact event log is enough.
- Recorded games are static assets (JSON) — no backend needed to watch them.
- The agents are deterministic functions of `(state, seed)` (they seed their own PRNG and reseed
  simulation clones), so a recorded game is exactly reproducible. The only cross-process drift is
  the engine's `HashMap`-ordered iteration (see `design.md` §8.10); irrelevant to a *recorded*
  replay, which fixes the log.

## Model: record-then-replay (not "push pixels")

**Tier 1 — Recorder** (new Rust crate, e.g. `crates/replay`, depending on `cna-engine`). Runs a
game via the existing `self_play` / tournament driver and emits a structured **event log**: the
initial scenario + seed, then per step the `Command`, its `CommandOutcome`, active phase/turn/stage,
the side, the relevant `§` rule citations, and a state snapshot (or delta). serde serializes it to
JSON.

**Tier 2 — Web frontend** consumes the event log and renders: a scrubber timeline, the hex map, an
action log, and a rules panel.

## Where the engine runs for the frontend

### Target: WASM client-side replay
Compile the engine to WASM (`wasm-bindgen` / `wasm-pack`). The browser holds
`(initial state + command log)` and calls the *same Rust code* to produce the state at any step.
- **Zero logic duplication** — the web view is byte-identical to the engine; no JS reimplementation
  of stacking/ZOC/combat to drift out of sync.
- **Free scrubbing / step-back / pause** — "jump to step N" is just replay-to-N. The O(1) clone
  makes checkpoint caching cheap.
- **Static hosting** — replays are JSON files on S3/CloudFront; no live server. Scales trivially.

### Current: delta event log, dumb client (no WASM) — **built**
The recorder emits a **delta-encoded JSONL** event log (see "Snapshots vs deltas" below); the
frontend is a pure renderer with no engine — it folds the per-step deltas into full frames and
draws them. This is what ships today. The same event-log schema is the WASM-replay stepping stone,
so client-side replay can be added later without changing it.

## Live vs recorded
- **Recorded** (scrub a finished AI-vs-AI game): static JSON + WASM replay. Covers ~90% of the use
  case, needs no backend.
- **Live** (watch as it's played): a small Rust service runs the game and pushes the same events
  over **WebSocket** (or SSE for one-way). The frontend doesn't care whether events come from a file
  or a socket — same schema.

## Frontend stack
- **React + TypeScript** for the shell (timeline scrubber, action log, rules panel, agent name
  plates). Svelte is an acceptable leaner alternative.
- **Hex map: SVG** at CNA scale (~150 units) — crisp, every hex/counter is a hoverable node.
  Hovering a stacked hex (or an off-map counter) opens a **master-detail info panel** — a scrollable
  list of the stack's occupants and, for the selected one, its rules blurb + dynamic state (a unit's
  TOE / combat / movement / supply / formation, a depot's contents, an airfield's occupancy, …).
  Place / terrain / unit-type hovers add historical lore. Move to **Canvas / WebGL (PixiJS or
  deck.gl)** only if counts or animation demand it later.
- **`ts-rs` or `typeshare`** to auto-generate the TypeScript types for the event-log / `Command` /
  state structs *from the Rust definitions*, so frontend types cannot drift from the engine. Adopt
  from day one.

## The "relevant rules" feature — **built**
`Command::rule_refs()` (engine, co-located with the `Command` variants) maps every command to its
governing `§` section(s); the recorder carries them in `Event.rules` and the viewer's **Relevant
Rules** panel shows them. Separately, each event gets a friendly plain-language `Event.summary`
(from `describe_command`, resolving unit/ship/squadron ids to names and hexes) shown in **Current
Action** — the summary carries no `§`, so the two panels don't duplicate. Per-*outcome* refs (which
CRT/shift a resolution actually used) would live on `CommandOutcome` — a later enhancement.

## Relationship to the MCP goal
The event log's `Command` schema is the *same* action vocabulary an MCP-driven model would use to
play. Define the `Command` (de)serialization once (serde + `ts-rs`) as a first-class, **versioned**
schema serving three consumers: the recorder/visualizer, the future MCP action interface, and
external tooling.

## Snapshots vs deltas — resolved (deltas), streamed as JSONL, gzipped
Full snapshots were fine for the toy skirmish but blew up on the real map: Scenario 1 (7,011 hexes,
80 units) was **15.5 MB for one turn**. So the event log is **delta-encoded** — each `Event.units`
lists only the units that changed since the previous step (step 0 lists all), with `Event.removed`
for eliminations; every other dynamic layer (weather, dumps, convoys, raiders, air, naval, off-map,
overlays, prisoners, …) is likewise a `Some`-on-change delta. The frontend folds them into full
per-step frames on load. The static map is serialized once.

The recorder **streams** the log as **JSONL** — a header line (scenario/seed/agents + map) then one
JSON `Event` per line — so `record` writes it incrementally and the file need never be held whole in
memory. For the web asset it is **gzipped** to `game.jsonl.gz`: the JSONL is ~97% redundant, so a
full campaign compresses tens of MB → under 1 MB, staying beneath Cloudflare Pages' 25 MB/file limit
and cutting transfer. The browser inflates it with the native `DecompressionStream`. This is also
the WASM-replay stepping stone (a command log is the eventual end state).

## Build order
1. ✅ **Done** — recorder crate (`crates/replay`) streams an AI-vs-AI game to a `.jsonl` log.
   `cna_replay::record_game_streaming` mirrors `self_play` and emits one event per accepted command;
   the `record` bin writes it (`cargo run --release -p cna-replay --bin record -- <out.jsonl>
   [turns] [scenario]`). The header line carries scenario/seed/agents + the static map; each event
   carries phase/turn/stage/side, the raw action + outcome (`Debug` strings), a friendly `summary`,
   `rules`, an `activity` jump target, and the post-action delta of dynamic state.
2. ✅ **Done** — `ts-rs` emits the TypeScript types from the record structs into
   `crates/replay/bindings/*.ts` when `cargo test` runs (committed; regenerated by the auto-added
   export tests, so a struct change that isn't reflected shows up as a git diff). `u64` maps to
   `bigint` (seed), `Option<[i16;2]>` to `[number, number] | null`; doc-comments carry over as JSDoc.
3. ✅ **Done** — React + SVG renderer (`web/`, Vite + TS). `HexMap` draws the terrain grid + unit
   counters; hex geometry (`web/src/hex.ts`) mirrors the engine's pointy-top / even-r layout so
   adjacency matches. Consumes the ts-rs bindings via a `@bindings/*` alias. `npm run dev` after
   recording + gzipping to `web/public/game.jsonl.gz`.
4. ✅ **Done** — `Scrubber` (play/pause, step, slider) + `ActionLog` (click-to-jump, current
   highlighted) + a side panel showing the **friendly action description** and its **governing rule
   sections**. The recorder populates `event.rules` from `Command::rule_refs()` (engine method,
   co-located with the `Command` variants) — now **exhaustive**, so every command has a citation —
   and `event.summary` from `describe_command` (a plain-language sentence per command, ids resolved
   to names/hexes). Each event also carries an `activity` jump target (a hex, or a named off-map box)
   so a "jump to activity" control can center the map on the action. Per-*outcome* refs (which
   CRT/shift a resolution used) would live on `CommandOutcome` — a later enhancement.
5. **Future** — WASM-compile the engine; swap the dumb client's folded delta log for a replayable
   command log and `engine.replay_to(N)`.
6. **Future** — *(Optional)* WebSocket service for live games.

## Recorder notes / follow-ups
- **Event granularity:** one event per accepted command → fine-grained action tracking.
- **Scenarios:** the `record` bin takes `record <out.jsonl> [turns] [scenario]` — scenario is
  `skirmish` (synthetic), `scenario1` (§60 Italian Offensive on the real five-section map + historical
  OB, 7,011 hexes / 80 units), or `campaign` (§64, the default demo). The committed web asset
  (`web/public/game.jsonl.gz`) is a gzipped 5-turn campaign run.
- **Rules citations are done.** `Command::rule_refs()` is exhaustive and the recorder emits them per
  event; the viewer's Relevant Rules panel is always populated. The friendly `summary` deliberately
  carries no `§` so it doesn't duplicate that panel.
- **Actions/outcomes are still `Debug` strings** in the event log (alongside the friendly `summary`).
  A round-trippable `Command` schema exists (`Command`/`CommandOutcome` derive `Serialize`/
  `Deserialize`, round-trip tested), i.e. the shared action vocabulary for **WASM replay** and the
  **MCP interface**. Remaining for WASM replay (step 5): emit a structured command log the browser
  replays, compile the engine to WASM, and expose `replay_to(N)`.
