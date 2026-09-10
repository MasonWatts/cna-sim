# CNA-Sim Viewer

Spectator website for CNA-Sim recorded games — see `../docs/visualization-architecture.md`.
This is the **dumb-client v0** (build-order step 3–4): it loads a `game.json` event log produced by
the `cna-replay` recorder and renders the board, a timeline scrubber, and the action log. It does
**not** run the engine (WASM replay is step 5); it just renders the recorded snapshots.

## Run

```bash
# 1. Produce a recording (from the repo root):
cargo run --release -p cna-replay --bin record -- web/public/game.json 4

# 2. Start the viewer:
cd web
npm install
npm run dev        # open the printed localhost URL
```

`npm run build` type-checks (`tsc --noEmit`) and bundles to `dist/`.

## How it fits together
- Types come from the Rust structs via `ts-rs` (`@bindings/*` → `../crates/replay/bindings/*.ts`),
  so the schema is single-sourced in the engine; a struct change reshapes the frontend types.
- Hex geometry (`src/hex.ts`) mirrors the engine's pointy-top / even-r offset layout so on-screen
  adjacency matches the simulation.
- `App.tsx` owns the current step; `HexMap`/`Scrubber`/`ActionLog` are presentational.

## Not yet (see the architecture doc)
- Per-action `§` rule citations (needs a `Command → §` map in the recorder).
- WASM client-side replay (step 5) — today the recorder ships full per-step snapshots.
