# CNA-Sim Viewer

Spectator website for CNA-Sim recorded games — see `../docs/visualization-architecture.md`.
It loads a **delta-encoded JSONL** event log produced by the `cna-replay` recorder (gzipped to
`public/game.jsonl.gz`) and renders the board, a timeline scrubber, a friendly action log, and the
rule behind each action. It does **not** run the engine (WASM client-side replay is a future step);
it folds the per-step deltas into full frames and renders them.

## Run

```bash
# 1. Produce a recording (from the repo root) and gzip it as the web asset:
cargo run --release -p cna-replay --bin record -- /tmp/game.jsonl 5 campaign
gzip -9 -c /tmp/game.jsonl > web/public/game.jsonl.gz

# 2. Start the viewer:
cd web
npm install
npm run dev        # open the printed localhost URL
```

`npm run build` type-checks (`tsc --noEmit`) and bundles to `dist/` (copying `public/` assets).

The log is gzipped because a full campaign run is tens of MB uncompressed (≈97% redundant) — past
Cloudflare Pages' 25 MB/file limit. The loader fetches `game.jsonl.gz`, inflates it in the browser
via `DecompressionStream`, and falls back to a raw `game.jsonl` then a single `game.json`.

## How it fits together
- Types come from the Rust structs via `ts-rs` (`@bindings/*` → `../crates/replay/bindings/*.ts`),
  so the schema is single-sourced in the engine; a struct change reshapes the frontend types.
- Hex geometry (`src/hex.ts`) mirrors the engine's pointy-top / even-r offset layout so on-screen
  adjacency matches the simulation.
- `App.tsx` owns the current step and folds the event deltas into per-step frames; `HexMap`,
  `Scrubber`, `ActionLog`, `InfoPanel`, `TerrainKey`, `UnitKey`, and `CompassRose` are presentational.
- **Current Action** shows a friendly, plain-language description of each command (from the recorder's
  `Event.summary`), with a "jump to activity" control that centers the map on the action; **Relevant
  Rules** shows the `§` citations (`Event.rules`). Hovering a stacked hex or off-map counter opens a
  master-detail info panel; place/terrain/unit-type hovers show rules + historical lore
  (`public/lore/*.json`).

## Not yet (see the architecture doc)
- WASM client-side replay — today the recorder ships a folded delta log, not a replayable command log.
- Live spectating over WebSocket.
