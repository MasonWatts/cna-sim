# Visualization Architecture

Plan for a website where viewers watch CNA-Sim games play out — current board state, a running
log of player actions, and the rule behind each action. Recorded (scrub a finished game) first;
live spectating (watch as it plays) as a follow-on. Status: **design agreed, not yet built.**

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

### v0 quick cut: server-side snapshots, dumb client (no WASM)
Recorder emits a full state snapshot per step as JSON (state is small, ~150 units); the frontend is
a pure renderer with no engine. Fastest to stand up. Same event-log schema, so WASM replay can be
added later without changing it.

## Live vs recorded
- **Recorded** (scrub a finished AI-vs-AI game): static JSON + WASM replay. Covers ~90% of the use
  case, needs no backend.
- **Live** (watch as it's played): a small Rust service runs the game and pushes the same events
  over **WebSocket** (or SSE for one-way). The frontend doesn't care whether events come from a file
  or a socket — same schema.

## Frontend stack
- **React + TypeScript** for the shell (timeline scrubber, action log, rules panel, agent name
  plates). Svelte is an acceptable leaner alternative.
- **Hex map: SVG** at CNA scale (~150 units) — crisp, every hex/counter is a clickable node with
  native tooltips (click a unit → its TOE / supply / the rule governing its last action). Move to
  **Canvas / WebGL (PixiJS or deck.gl)** only if counts or animation demand it later.
- **`ts-rs` or `typeshare`** to auto-generate the TypeScript types for the event-log / `Command` /
  state structs *from the Rust definitions*, so frontend types cannot drift from the engine. Adopt
  from day one.

## The "relevant rules" feature
`Command` / `CommandOutcome` and the engine are densely annotated with `§` citations. Carry those
refs in each event-log entry; the rules panel then shows, e.g., "this Close Assault resolved on the
§15.7 CRT with a §15.4 combined-arms shift," optionally linking to a glossary built from
`design.md` / the rulebook. Surfacing the *why* per action is a high-value differentiator and nearly
free given the existing annotations.

## Relationship to the MCP goal
The event log's `Command` schema is the *same* action vocabulary an MCP-driven model would use to
play. Define the `Command` (de)serialization once (serde + `ts-rs`) as a first-class, **versioned**
schema serving three consumers: the recorder/visualizer, the future MCP action interface, and
external tooling.

## Open decision: snapshots vs deltas
Start with **full state snapshots** per event (simple; state is small). Add deltas only if bandwidth
ever bites — the schema can carry either.

## Build order
1. ✅ **Done** — recorder crate (`crates/replay`) dumps an AI-vs-AI game to `game.json`.
   `cna_replay::record_game` mirrors `self_play` and emits one event per accepted command; the
   `record` bin writes a Greedy-vs-Random skirmish (`cargo run --release -p cna-replay --bin record
   -- game.json [turns]`). Header carries scenario/seed/agents + the static map; each event carries
   phase/turn/stage/side, the action + outcome (v0: `Debug` strings), and post-action unit views.
2. ✅ **Done** — `ts-rs` emits the TypeScript types from the record structs into
   `crates/replay/bindings/*.ts` when `cargo test` runs (committed; regenerated by the auto-added
   export tests, so a struct change that isn't reflected shows up as a git diff). `u64` maps to
   `bigint` (seed), `Option<[i16;2]>` to `[number, number] | null`; doc-comments carry over as JSDoc.
3. ✅ **Done** — React + SVG renderer (`web/`, Vite + TS). `HexMap` draws the terrain grid + unit
   counters; hex geometry (`web/src/hex.ts`) mirrors the engine's pointy-top / even-r layout so
   adjacency matches. Consumes the ts-rs bindings via a `@bindings/*` alias. `npm run dev` after
   recording to `web/public/game.json`.
4. ✅ **Mostly done** — `Scrubber` (play/pause, step, slider) + `ActionLog` (click-to-jump, current
   highlighted) + a side panel showing the current action/outcome. **Rules panel is stubbed** —
   `event.rules` is empty until the `Command → §` map (needs the recorder to populate citations).
5. WASM-compile the engine; swap the dumb client's "state at N" for `engine.replay_to(N)`.
6. *(Optional)* WebSocket service for live games.

## v0 recorder notes / follow-ups
- **Event granularity:** one event per accepted command → fine-grained action tracking (a 5-turn
  6-unit skirmish records ~1200 events, ~2.8 MB pretty JSON). Fine for a demo; the snapshots→deltas
  optimization (above) is the lever if it grows.
- **`rules` is empty for now.** The `§` citations live in engine doc-comments, not runtime data, so
  the per-action rules panel needs a `Command → §` mapping built later (step 4).
- **Actions/outcomes are `Debug` strings** in v0. The round-trippable `Command` schema (serde derive
  on `Command`/`CommandOutcome`, shared with MCP) is a follow-on and unblocks WASM replay (step 5).
