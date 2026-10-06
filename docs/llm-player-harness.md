# LLM Player Harness — Plan

Status: **M1 built** (2026-10-03) — `crates/harness`, tests in `crates/harness/tests/mcp_e2e.rs`. Goal: let AI models play CNA-Sim as either side through an
MCP tool interface, while keeping the existing deterministic scripted agents (`RandomAgent`,
`GreedyAgent`, `LookaheadAgent`, …) as the fast path for engine testing.

## 1. Architecture

```
                       ┌──────────────────────── cna-harness (one process) ───────────────────────┐
                       │                                                                           │
  claude -p ... ◄──────┤  launcher: spawns one model session per *decision window*                 │
  (or any MCP client)  │                                                                           │
        │  MCP (HTTP)  │  ┌───────────────┐   channel   ┌──────────────────────────────────────┐  │
        └─────────────►│  │  MCP server   │ ◄─────────► │  game loop thread                     │  │
                       │  │  (rmcp,tokio) │  requests / │  record_game_streaming(...)           │  │
                       │  │  /mcp/cw      │  replies    │    ├─ McpAgent  (LLM side)            │  │
                       │  │  /mcp/axis    │             │    └─ GreedyAgent / any Agent (other) │  │
                       │  └───────────────┘             │  GameState (only owner)                │  │
                       │                                └──────────────────────────────────────┘  │
                       │   outputs: game.jsonl (viewer) · commands.jsonl (replay) · transcript/    │
                       └───────────────────────────────────────────────────────────────────────────┘
```

Principles:

1. **The harness owns the loop.** Players never drive the engine; they answer when asked.
2. **`McpAgent` is just another `Agent`.** The existing recorder loop (`record_game_streaming`) runs
   unchanged, so LLM games are viewer-replayable, and any side can be scripted (LLM vs `GreedyAgent`
   is the default eval).
3. **The MCP boundary *is* the fog of war.** The MCP server thread never touches `GameState`. Every
   tool call becomes a request to the game thread, answered from the side's *observation*
   (`observe_units`, `observe_supply_dumps`, …). Enemy TOE, dummy dumps, hidden raiders, etc. are
   never exposed. (This closes the deferred "act only on observed info" item for LLM players.)
4. **Determinism lives in the log.** Dice are rolled inside the engine boundary (seeded RNG), so an
   LLM game replays exactly from `seed + commands.jsonl`, regardless of model nondeterminism.
5. **Per-side isolation.** One endpoint per side (`/mcp/commonwealth`, `/mcp/axis`) with a random
   per-run bearer token; a player can only query/act for its own side.

## 2. Engine changes (`cna-engine`)

- `Agent` gains a **free-form** mode so a player may submit commands that are not in the
  precomputed `legal_commands` snapshot (parameterized commands, macro expansions):
  - `fn free_form(&self) -> bool { false }` — default keeps index-based agents unchanged.
  - `fn on_result(&mut self, cmd: &Command, result: &Result<CommandOutcome, CommandError>) {}`.
- `drain_phase` and the recorder loop: for free-form agents call `choose` → `apply` → `on_result`
  until `choose` returns `None` (with the same per-phase action cap as a safety net).
- No new dependencies in the engine crate.

## 3. New crate `crates/harness` (`cna-harness`)

Depends on `cna-engine`, `cna-replay`, `rmcp` (server + streamable-HTTP), `tokio`, `axum`,
`serde_json`, `schemars`.

### 3.1 `McpAgent` (game thread)

`choose(&GameState, side)` blocks on a channel of requests from the MCP server:

| Request | Handling on the game thread |
|---|---|
| `Status` | turn/stage/phase, acting side, VP, whether a window is open |
| `Observe { filter }` | side-filtered observation JSON (own units full, enemy per `observe_units`) |
| `LegalActions { unit, kind, offset, limit }` | paginated `legal_commands(side)` with stable ids + friendly text |
| `SubmitPlan(items)` | queue items; return them one by one from `choose`; reply after the queue drains with per-item results |
| `EndPhase` | `choose` returns `None` → the loop advances |

Plan items: `{"id": n}` (from the latest `legal_actions` listing), `{"command": <Command JSON>}`
(free-form, validated by `apply`), or the macro `{"move_to": {"unit": u, "hex": [c, r], "mode": …}}`
(expands step-by-step into legal `MoveStep`s that reduce distance to the target, stopping on arrival,
no progress, or an illegal step).

A model-controlled side plays **every** decision phase itself (no hybrid/scripted delegation).
Windows with **no enumerated legal actions are auto-passed** (no model call; `--no-auto-pass`
consults the model anyway, for free-form commands the enumerator doesn't list). A window also
closes on launcher exit or timeout (→ `EndPhase`).

### 3.2 MCP server (tokio thread)

Tools (all scoped to the endpoint's side): `get_status`, `get_observation`, `list_legal_actions`,
`get_rule` (looks up a § section in `docs/source-materials/rules.md`), `submit_plan`, `end_phase`,
`write_notes` / `read_notes` / `get_journal` / `get_briefing` (memory, §3.3a), and `wait_for_turn`
(blocks until this side has an open window — for long-running external clients).

### 3.3 Launcher

Per decision window, run a configurable command template (default: headless Claude Code —
`claude -p --mcp-config <generated.json> --allowedTools "mcp__cna" <briefing>`), with a per-window
timeout. `--player external` skips the launcher: an external MCP client connects and uses
`wait_for_turn`. The briefing states the side, the window (turn/phase), the objective, and the tool
workflow (status → observe → legal actions → submit_plan → end_phase).

### 3.3a Context across window sessions

Each window is a **fresh** model session (bounded context and cost); continuity lives in the harness
(`briefing.rs`, `memory.rs`):

- **System prompt** (`--system-prompt`, replacing the client default): commander persona, how play
  and decision windows work, the mechanics that matter (CP/CPA, cohesion, supply, ZOC and §10.3
  Holding Off, combat types and probes, fog of war), the tool contract, and the memory contract.
- **Window prompt**: scenario objectives (§60.81 / §64.7 / skirmish, per side), the situation
  (date, turn, stage, phase, weather), the **journal**, and the **notebook**.
- **Journal** (harness-written, fog-safe): per window, a diff of the side's own observation since
  its previous window — own losses, cohesion and position changes (incl. forced retreats and staff
  actions), enemy contacts appearing / moving / lost — plus every action it took with its result.
  The last 4 windows in detail, older ones as one-line ledger entries; `get_journal` returns all.
- **Notebook**: free text the model rewrites each window with `write_notes` (replace semantics,
  capped at 6,000 characters) — strategy, formation tasks, enemy intel, lessons.
- Snapshots are written to `<out>.transcript/notes-<side>.md` and `journal-<side>.md`, and each
  window's exact prompts to `w####-<side>-<phase>.prompt.md`. External clients get the same context
  via `get_briefing`.

### 3.3b Scale, robustness and cost (M1.2)

- **Observation at scale:** `get_overview` (≈8 KB for the 210-unit Axis campaign vs 115 KB for the
  full unit list): own stacks by hex (contact first), formations, supply problems, enemy contacts by
  hex, largest dumps. `get_observation` is compact by default (one line per unit); `detail` or
  `units` gives full records with rule-cited blockers. Journal diffs are ordered by importance.
- **Macros:** `move_stack` (every counter in a hex), `resupply_hex` (refuel + rearm a hex from its
  dumps), `assault` (every adjacent combat unit by default).
- **Rationale (for analysis/commentary):** `submit_plan` takes a required `rationale` (1–3 sentences of
  intent) and `end_phase` an optional closing one. Each window appends one line to
  `<out>.transcript/rationale.jsonl` — `{window, key, side, pregame, at {game_turn, stage, phase, date},
  cost_usd, actions_ok/failed, rationales: [{kind: plan|end, text, results?}]}` (each plan's item results
  attached; join with `commands.jsonl` on `key`) — and the journal shows it back to the player.
  (Model "thinking" isn't recoverable: the client writes thinking blocks empty.)
- **Explained rejections:** a failed plan item carries the engine error plus a `why` (blocking rule,
  numbers, way out — e.g. "D3714 already holds 5 of its 6 Stacking Points … §9.31 … detach brigades");
  a `move_to` that can't get closer lists each closer neighbour's actual blocker in `blocked_steps`.
  `examples/probe_move.rs` and `examples/stacking_audit.rs` diagnose a recorded game offline.
- **Retry:** a session that crashes or never engages is relaunched once (`-tryN` transcript).
- **Checkpoint/resume:** the command log is flushed per attempt; each side's notebook/journal is
  saved as `memory-<side>.json` per window. `--resume <commands.jsonl>` replays the log exactly,
  restores memory, and continues live.
- **Pre-game planning (M1.3):** one read-only window per side before its first decision, with its
  own allowance (`--pregame-model/-budget-usd/-timeout`, `--no-pregame`): it studies the rulebook
  (`rules_toc` / `read_rules` / `search_rules`, available in every window) and the situation and
  writes a **doctrine** (`write_doctrine`, ≤15k chars) shown in every later window. The system prompt
  is lean (role, windows, tools, engine-only facts); `--coaching` restores the harness's mechanics
  summary for A/B tests.
- **Cost:** one model for every phase by default; `--routine-model` optionally overrides non-Movement phases, `--budget-usd` per window,
  `--max-cost-usd` per game (further windows pass once reached); per-window and running spend is
  logged. Sessions get a grace period after `end_phase`, then their whole process group is killed.
- **Result:** `<out>.result.json` and a RESULT line — campaign §64.71/§64.72 automatic win and
  §64.73–§64.76 VP/level, §60.81 level for Scenario 1, surviving strength for all.
- Measured (campaign, 1 Game-Turn, Claude vs Claude, Opus for Movement + Sonnet routine):
  18 windows, ≈ $0.9 total.

### 3.4 CLI

```
cna-harness --scenario campaign --turns 1 --seed 7 \
            --commonwealth llm --axis llm \
            --out /tmp/game.jsonl [--player-cmd '…'] [--window-timeout 300] [--port 0] [--no-auto-pass]
cna-harness --replay /tmp/game.commands.jsonl   # re-applies the log, verifies it reproduces
```

### 3.5 Outputs

- `game.jsonl` — the existing viewer event log (header records the agent names / model).
- `<out>.commands.jsonl` — every **accepted** command as `{"step", "side", "command"}` (serde JSON),
  enough to replay the game deterministically.
- `<out>.transcript/` — per-window launcher stdout/stderr (the model's reasoning / tool trace).

## 4. Testing

- Engine: free-form agents through `drain_phase` (accept, reject + `on_result`, cap).
- Harness unit: observation never contains enemy `strength` unless revealed; dummy dumps hidden;
  legal-action pagination/ids; plan results (ok / engine error); `move_to` expansion.
- Harness integration: an in-test **rmcp client** plays a 1-turn game as one side against
  `GreedyAgent` through the real HTTP server (no model needed); the run records `game.jsonl`, and
  `--replay` of its `commands.jsonl` reproduces the final state.
- Manual: one real `claude -p` window to validate the launcher end-to-end.

## 5. Milestones

- **M1 (this build):** everything above.
- **M1.1 (done):** custom system prompt, scenario objectives, journal + notebook continuity.
- **M2:** strategy prompt tuning, cost/latency controls (window budgets, batching hints), richer macros (formation moves, supply
  ops), LLM-vs-LLM and LLM-vs-scripted tournaments, surfacing the model's per-window rationale in
  the viewer's Action Log.

## 6. Known limits (M1)

- `legal_commands` does not enumerate every parameterized command; free-form `command` items cover
  the rest (validated by `apply`).
- Legality checks can reveal *some* information (e.g. an assault target exists) — positions are
  public anyway, strengths/dummies are not exposed.
- Cross-window memory is the journal + notebook only (no raw transcript carry-over), by design.
