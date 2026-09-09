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
Battle are loaded and chart-verified. The engine builds clean and **366 tests pass**. See
[`docs/design.md` §8](docs/design.md) for the detailed, current implementation status.

## Architecture

The system has three major components, built roughly in order:

1. **Core engine** (this repo, in progress) — an authoritative data model plus rules
   enforcement. No UI, no I/O in the core.
2. **AI agents** (future) — programmatic players that observe state and issue legal actions.
3. **Visualization** (future) — a web-hosted view of live/replayed games.

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
data/                 RON reference data, maps, orders of battle, scenarios
docs/design.md        Project design document (vision, principles, roadmap, status)
docs/source-materials/ Human-readable transcriptions of digitized game charts (provenance)
tools/                OCR → order-of-battle pipeline and map utilities
```

## Build, test, run

```bash
cargo build              # build the workspace
cargo test --workspace   # run the full test suite (366 tests)
cargo run --bin demo     # narrate barrage, anti-armor, and close assault in sequence
```
