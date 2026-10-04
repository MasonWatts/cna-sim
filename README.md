# CNA-Sim

A digital simulation of SPI's *The Campaign for North Africa: The Desert War 1940–43*
(CNA) — a famously detailed, logistics-heavy operational wargame. The physical game is
essentially a large manual state machine; most of its legendary playing time is humans
performing bookkeeping and rules arbitration that software can absorb. This project makes
the engine the authoritative referee of legal play.

The end goal is the **full Campaign Game**, approached in phases. **Scenario 1, "The
Italian Offensive"** is the first playable milestone.

## Status

The Land Game, the air and naval games, and the **full** Logistics system (§20 production,
convoys, and the §54.4 Axis railroad included) are built, and all three faction Orders of
Battle are loaded and chart-verified. The engine builds clean and the workspace passes
**492 tests** (450 engine + 42 replay). See [`docs/design.md` §8](docs/design.md) for the
detailed, current implementation status.

## Architecture

The system has three major components, built roughly in order:

1. **Core engine** (this repo) — an authoritative data model plus rules enforcement. No UI,
   no I/O in the core.
2. **AI agents** (built) — programmatic players (e.g. Greedy, Random) that observe state and
   issue legal actions; used by the self-play/tournament drivers and the replay recorder.
3. **Visualization** (built) — a React + SVG spectator viewer (`web/`) that replays a recorded
   game: hex map, timeline scrubber, friendly action log, and per-action rule citations. Records
   are produced by the `cna-replay` recorder. WASM client-side replay and live spectating remain
   future work (see `docs/visualization-architecture.md`).

Key design principles (full rationale in `docs/design.md`):

- **Two-tier state model** — immutable *reference/rulebook data* (RON files under `data/`)
  vs. the mutable *game-state* save. Parameters live in data; rules/formulas live in code.
- **Pure engine core** — no I/O; exposes commands (`GameState::apply`), queries, and
  serializable state. Designed for native + WASM from one codebase.
- **Store primitives, derive everything derivable** (combat strengths, CPA, ZOC, stacking).
- **Deterministic** — all randomness flows through a single seeded, serializable RNG, so a
  seed + a command sequence fully reproduce a game.

The engine is layered: **reference** (`crates/engine/src/reference/`, pure chart data) →
**rules** (`crates/engine/src/rules/`, pure stateless arithmetic/eligibility) → **state**
(`crates/engine/src/state/`, mutable instances) → **command boundary** (`GameState::apply`,
phase-gated `Command`/`CommandOutcome`/`CommandError`).

## Layout

```
crates/engine/        Rust rules engine (the core)
  src/reference/       Tier 1: immutable rulebook parameters (loaded from RON)
  src/rules/           Pure rule logic (movement, combat, air, naval)
  src/state/           Mutable game state (map, units, combat, turn driver, commands)
  src/bin/demo.rs      Scripted narration of the three land-combat systems
crates/replay/        Records a game to a JSONL event log (+ ts-rs TS bindings) for the viewer
web/                  React + SVG spectator viewer (Vite + TypeScript)
data/                 RON reference data, maps, orders of battle, scenarios
docs/design.md        Project design document (vision, principles, roadmap, status)
docs/source-materials/ Human-readable transcriptions of digitized game charts (provenance)
tools/                OCR → order-of-battle pipeline and map utilities
```

## Build, test, run

```bash
cargo build              # build the workspace
cargo test --workspace   # run the full test suite
cargo run --bin demo     # narrate barrage, anti-armor, and close assault in sequence
```

## Spectator viewer

```bash
# Record a game to a JSONL event log, gzip it as the web asset, then serve the viewer:
cargo run --release -p cna-replay --bin record -- /tmp/game.jsonl 5 campaign
gzip -9 -c /tmp/game.jsonl > web/public/game.jsonl.gz
cd web && npm install && npm run dev   # open the printed localhost URL
```

See [`docs/visualization-architecture.md`](docs/visualization-architecture.md) and
[`web/README.md`](web/README.md).

## AI-model players (MCP harness)

`cna-harness` runs a game in which either side is played by an AI model through an MCP server
(fog-of-war-safe tools: observe, list legal actions, submit a plan, end phase). The scripted agents
remain available for fast engine testing.

```bash
# Claude (headless Claude Code) plays the Commonwealth — every phase — vs Claude as the Axis:
cargo run -p cna-harness -- --scenario skirmish --turns 2 --commonwealth llm --axis llm \
    --budget-usd 0.50 --out /tmp/game.jsonl
# Bring your own MCP client instead (prints endpoint + token; use wait_for_turn):
cargo run -p cna-harness -- --commonwealth llm --external --out /tmp/game.jsonl
# The campaign, limited to a few turns, with cost controls (one model for every phase; each side
# first gets a pre-game planning window to study the rulebook and write its doctrine):
cargo run --release -p cna-harness -- --scenario campaign --turns 2 --commonwealth llm --axis llm \
    --budget-usd 1.50 --max-cost-usd 25 --out /tmp/camp.jsonl
# View a run in the spectator viewer: copy it under web/public/runs/ and open ?game=runs/<file>
gzip -9 -c /tmp/camp.jsonl > web/public/runs/camp.jsonl.gz   # then http://localhost:5173/?game=runs/camp.jsonl.gz
# Continue a crashed or budget-stopped game from its command log:
cargo run --release -p cna-harness -- --resume /tmp/camp.commands.jsonl --commonwealth llm --axis llm
# Reproduce any recorded game exactly from its command log:
cargo run -p cna-harness -- --replay /tmp/game.commands.jsonl
```

The `.jsonl` output is the normal viewer event log; per-window model transcripts land in
`<out>.transcript/`. See [`docs/llm-player-harness.md`](docs/llm-player-harness.md).
