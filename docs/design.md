# Campaign for North Africa — Digital Simulation
## Project Design Document

**Status:** Draft 3 — Phase 1 land engine complete; air, naval, and full-logistics
systems substantially built (rules + reference tiers essentially complete, command/
resolution layer partial). See §8 for the detailed status.
**Date:** 2026-08-29
**Scope of this document:** Project goals, guiding principles, phased roadmap, key
decisions, and technology direction. Detailed data schemas/types are deliberately
*out of scope here* and will follow as a separate document once this is settled.
**Related docs:** `deferred-items.md` (rules-fidelity backlog); `visualization-architecture.md`
(the spectator website — record-then-replay, WASM client-side replay, event-log schema).

---

## 1. Vision

Build a digital simulation of SPI's *The Campaign for North Africa: The Desert War
1940–43* (CNA) — a notoriously detailed, logistics-heavy operational wargame. The
physical game is essentially a large manual state machine: most of its legendary
playing time is humans performing bookkeeping and rules arbitration that software
can absorb.

The system has three major components, to be built roughly in this order:

1. **Core engine** — an authoritative data model plus rules enforcement. Encapsulates
   game state and legal-move validation. No UI, no I/O in the core.
2. **AI agents** — programmatic players that observe game state and issue legal actions.
3. **Visualization** — a way to see game state; end goal is a web-hosted view of live
   games running on a server, with local development supported throughout.

---

## 2. Goals and Non-Goals

### Goals
- A correct, testable rules engine that is the single source of truth for legal play.
- Faithful eventual reproduction of the *full* published rules (approached in phases).
- Portability: the engine runs natively (local dev, AI, server) and in the browser
  (WASM) for live web visualization, from one codebase.
- Clean separation between the engine and any front-end, via a serializable state
  model and an explicit command/query API.
- Deterministic, reproducible games (seeded RNG) to support testing, replay, and AI
  rollouts.

### Non-Goals (for now)
- A polished/production UI. Early visualization is a debugging aid.
- Networked multiplayer infrastructure, accounts, matchmaking, etc.
- Reproducing the Air Game and full Logistics Game in the first phase (see §5).
- Fog-of-war *enforcement* in the first phase (but we design for it — see §4.5).

---

## 3. Guiding Architectural Principles

These are the load-bearing decisions the rest of the project depends on.

### 3.1 Two-tier state model: Reference data vs. Game state
- **Reference / rulebook data** is immutable, loaded at startup: terrain effects,
  Combat Results Tables, unit characteristics charts, weapons-system charts, OA
  charts, reinforcement schedules, organization/attachment limits, scenario setup.
- **Game state** is the mutable save-game: unit strengths, positions, supplies,
  cohesion, placed map objects, and whose turn/phase it is.

This split enables clean save/load, deterministic replay, and — critically for the AI
phase — snapshotting state to run "what-if" rollouts without touching the rulebook.

### 3.2 Store primitives; derive everything derivable
Persist only path-dependent primitives (current TOE strength points, cohesion, CPs
expended this stage, supplies on hand, position, status flags, training progress).
**Compute** everything else on demand: actual combat strengths (`Rating × TOE ÷ 10`),
a parent formation's CPA (= slowest child), stacking totals, shell status, Zones of
Control. This prevents stale/contradictory state and is a hard project rule.

### 3.3 Pure engine core, decoupled from I/O and presentation
The core performs no I/O. It exposes:
- **Commands** (attempt an action → validated, applied or rejected with reason).
- **Queries** (read derived views of state).
- **Serialization** of the full game-state tier.

This boundary is what makes the two deployment models below both viable without a
rewrite:
- **Server-authoritative:** engine runs native on a server; browser clients are thin
  views over HTTP/WebSocket. Good for live, AI-driven, or multiplayer games.
- **Client-side WASM:** the same engine compiled to WebAssembly runs in the browser.
  Good for solitaire play, replays, and offline/local use.

### 3.4 Faithful modeling of the two hard structural realities
- **Assignment ≠ Attachment.** A unit may be *assigned* to one parent (organizational
  membership, need not be co-located) and simultaneously *attached* to a different
  parent (physically stacked, represented by that parent's counter). These are two
  independent, nullable relationships. Getting this wrong breaks stacking, CPA
  inheritance, and reorganization.
- **TOE Strength Points are the atomic unit of composition and loss.** A unit is a
  multiset of weapons-system point-groups (e.g., "6× 25-pdr + 2× 18-pdr"), each type
  with its own ratings. Combat losses remove specific points, not an abstract scalar.

### 3.5 Design for hidden information; implement it later
CNA leans hard on secrecy (blind barrage, class-only target info, patrols that reveal
intel, deceptive dummy tank formations). For AI players this matters: an agent should
act on what it *knows*, not ground truth. We keep the core state model clean now and
add a thin per-player "knowledge/observation" layer later, rather than baking
omniscience into the core. Retrofitting fog-of-war into an omniscient model is painful;
anticipating it is cheap. *(A first cut of that layer now exists — see §8.10.)*

### 3.6 Explicit Sequence-of-Play state machine
The turn structure (Game-Turn → 3 Operations Stages → Phases → Segments → Steps) is
modeled explicitly so the engine can enforce "this action is not legal in this
phase." The sequence is itself part of game state.

### 3.7 Determinism
All randomness flows through a single seeded RNG owned by game state. Given a seed and
a sequence of commands, a game is fully reproducible.

### 3.8 Data holds parameters; code holds rules
The reference tier is split further. *Parameters* — scalars, ratings, per-type
coefficients, rates, and pure lookup tables (number-in → number-out) — live in
external **data files** and are editable without recompiling. The *rules* that consume
them — conditional logic, formulas, sequencing, validation, and how parameters combine
— live in **code**. Example: a terrain type's base movement cost and stacking limit
are data; the logic that "roads negate wadis, tracks halve the cost, and vehicles may
never cross an escarpment uphill" is code. Type *identities* (the fixed set of terrain
types, unit types, etc.) remain closed enums in code so exhaustive matching flags every
unhandled case as we grow fidelity; their *parameters* load from data keyed by those
enums. Gray-area calls: Combat Results Tables are data (tabulated results), but
interpreting a result (what "Engaged" does, how losses apply) is code; capability-point
costs are data, but *when* a cost applies is code.

---

## 4. Key Decisions (Decision Log)

| # | Decision | Choice | Rationale |
|---|----------|--------|-----------|
| D1 | Initial rules scope | **Simplified Land Game**, extensible to full rules | Smallest real, winnable slice; the model is built so Air/Logistics bolt on later. |
| D2 | Fidelity target | **Eventually 100% faithful**, phased | Approximate the heavily-abstracted bits first; tighten over time. |
| D3 | Target scenario | **Scenario 1, "The Italian Offensive" (60.0)** | The rules nominate it as the smallest teaching scenario. |
| D4 | Portability / deployment | Portable core; **native + WASM**; end goal web-hosted live viz | Serves local dev now and web viz later from one engine. |
| D5 | Hidden information | **Design-for-now, implement-later** | See §3.5. |
| D6 | Derived vs stored values | **Compute everything derivable** | Correctness; avoids stale state (§3.2). |
| D7 | Engine language | **Rust (confirmed)** | Sum types + exhaustive matching, `Option`, `serde`, first-class WASM, native perf. Engine/viz boundary keeps it decoupled if ever revisited. |
| D8 | Front-end language (later) | Likely **TypeScript** | Web-native; talks to engine over the §3.3 boundary regardless of engine language. |
| D9 | Reference data vs. rules | **Parameters → data files; rules/formulas → code** | Coefficients (terrain & unit params, rates, CRT tables) are editable data; conditional logic and calculations that consume them are code. Type *identities* (e.g. the set of terrain types) stay closed enums in code for exhaustive matching; their *parameters* load from data. |
| D10 | Reference data format | **RON initially**, swappable via `serde` | Rust-native, maps cleanly to enums, and supports comments (to cite rule sections / flag placeholder values). Engine-internal; the front-end reads derived state via the API, not raw reference files. |
| D11 | Source-material transcription | **Save human-readable transcriptions of all digitized game items** (markdown under `docs/source-materials/`, alongside the authoritative RON) | Future agents/humans can reference charts/counters/OA sheets without the physical materials or re-OCR. See §9. |
| D12 | End scope | **The full Campaign Game** (111 Game-Turns of one week each, from September 1940) — not just a scenario slice. Scenario 1 remains the first playable milestone (D3). | Both Scenario 1 and the Campaign Game start Sept 1940; building for the whole campaign now shapes the data (full OB with arrival turns, all map sections, holding boxes, reinforcement scheduling, and eventually supply/air). We bank the complete OB (Italian/German/Commonwealth §4.44a–c) rather than a Scenario-1 subset. |
| D13 | 62 Marmarica / 63 Cirene | **Model on the standard semi-motorized division template** (Catanzaro's structure) + their Libyan-tank attachments (LXII(L)/LXIII(L)); flag as `abstracted_in_444b`. | Deployed in the Sept-1940 setup (§60.31) but deliberately abstracted out of the §4.44b OB (scarce records / "melded"), so they have no OA sheet. They were structurally standard binary divisions. See `docs/source-materials/italian-cirene-marmarica-template.md`. |
| D14 | OB auto-extraction fidelity | **Low-confidence; revisit for faithfulness.** Ship the OCR→OB pipeline with conservative, chart-validated ID inference (sibling-in-formation + dominant type-mode), everything else flagged (`// TODO id?` / `// id inferred`). | The §4.46 single-letter ID glyphs defeat OCR at any DPI, so ~half are inferred; spot-checks found real errors (case, Cyrillic garbage, cross-type leaks — since fixed). Inferred/flagged values are provisional and must be verified against the physical charts in a later faithfulness pass. A Rust test asserts every OB id is a valid characteristics key, that parents resolve within the formation, and that no unit outranks its parent HQ in org size. **`org_size` is data-driven, not transcribed:** for combat units it derives from the id's characteristics `unit_type` suffix (`Bn/Coy/Bde-Equivalent`); HQs (detected by name, since some HQ counters carry a combat id) size by hierarchy (top = formation echelon, sub-HQ = StandardBrigade). **Progress (2026-08-31):** the §4.46 counter *type* glyphs are now transcribed from the physical counter sheets (PDF p182 Italian, p183 German, p184–185 Commonwealth) into `docs/source-materials/{italian,german,commonwealth}-counters-4.46.md`, cross-checked against the OBs — validating the MG/Heavy-Weapons/recce §20.3 classifier rows and fixing the German "Machinegewehr" MG gap. **§9.2 resolved (2026-09-09):** §9.2 is *"Unit Equivalents"*, not size symbols — §9.22 says game org level follows the *printed Stacking Point value* (§9.4), explicitly **not** the counter's size symbol or designation (a III "Regiment" is routinely a battalion-equivalent). So `org_size` deriving from the §4.44 Bn-/Bde-/Coy-Equivalent groupings *is* the authoritative §9.2 source; counter symbols are historical echelon only. The one absolute rule §9.24 (all artillery = Stacking Point 1 = battalion-equivalent, batteries excepted) is now verified and enforced by a regression assertion — it caught 4 Italian artillery regiments wrongly set to `StandardBrigade` (now `Battalion`). See deferred-items H2. |
| D15 | Full Logistics only (drop the §32 abstract game) | **Implement the full Logistics Game (Fuel §49 / Ammo §50 / Stores §51 / Water §52, real Trucks, convoys); do NOT support the §32 abstract mode.** The abstract §32 work (Supply Units capped 40/60, Motorization Points, ½-CPA supply, flat consumption rates) is superseded and removed as full logistics is built. | User directive: the end goal is the full game "in all respects." Running both variants doubles the surface for little value, and the primitives differ fundamentally (Supply Units vs 4-commodity dumps; Motorization Points vs Light/Medium/Heavy Trucks). Sequence: (1) commodities foundation, (2) Fuel §49, (3) Ammo §50, (4) Stores §51 + Stores Expenditure Stage, (5) Water §52 + hot-weather §29.3, (6) Trucks, (7) supply transport/dumps/depots, (8) convoys §44/§30.5 (partly deferred pending the Air game), (9) production §20 (also closes §64.74/§64.75 VP). Increment 1: `SupplyDump` now holds `Commodities { fuel, ammo, stores, water }`; abstract consumption (`unit_fuel_cost`/`unit_ammo_cost`/`draw_fuel_for_move`/`draw_ammo`, §32.16 ½-CPA, `fuel_drawn_this_stage`) removed; the mode-agnostic truck tracing + victory hooks retained. (Update: the interim `unit_in_supply` dump-reachability proxy has since been replaced by the real §64.73 supply-adequacy test `unit_supply_adequate`; see §8.7.) |

---

## 5. Phased Roadmap

Two axes phase independently: **component** (engine → AI → viz) and **rules fidelity**
(simplified Land → full Land → +Air → +Logistics).

### Phase 0 — Foundations (this stage)
- This design document.
- Data schemas/types for the two-tier model (next deliverable).
- Project skeleton, build, test harness, seeded RNG.

### Phase 1 — Simplified Land engine (Scenario 1)
Target: a legal, playable, machine-refereed simplified Land Game.

**In scope:**
- Map: hexes, directional hexsides, terrain, core features (cities, towns, ports,
  oases, tracks/roads/rail, escarpments/slopes/ridges/wadis). Off-map Tripoli/Tunisia
  nodes as graph nodes.
- Units: infantry / tank / recce / artillery / anti-tank / anti-air / HQ / engineer,
  TOE-point composition, org hierarchy, assignment & attachment, stacking, ZOC.
- Movement: CPA, terrain costs, continual movement, reaction, retreat-before-assault,
  breaking off; breakdown present but may be simplified.
- Combat: barrage, anti-armor, close assault, probes; the associated CRTs; morale &
  cohesion; forward/back gun positioning.
- Sequence-of-Play state machine; initiative; weather.
- ~~**Abstract logistics** (rules §32)~~ — **superseded by D15.** The §32 abstract mode
  was dropped; the **full** Logistics Game (Fuel §49 / Ammo §50 / Stores §51 / Water §52,
  real Trucks, convoys) is being built instead and is now largely implemented (§8.7).
- Basic reinforcement/replacement arrival.

**Deferred to later phases:**
- ~~Full Logistics Game (§§48–58), Air Game (§§33–47)~~ — **now substantially built ahead
  of the original phasing** (D15; air, naval, and full-logistics status in §8.5–8.7).
- Detailed naval fleet/coastal shipping/chariot raids (§30). (~~Rommel special rules (§31)~~,
  ~~desert raiders & commandos (§27)~~ — **now fully implemented**. See §8.2/§8.9.)
- Full construction/engineers/minefields/fortifications (§§23–26) — partial first.
- Detailed prisoners (§28), repair (§22), breakdown (§21) — simplified first.
- ~~Fog-of-war enforcement (§3.5)~~ — **observation layer implemented** (units/dumps/raiders/
  minefields/wells, §8.10); command-side *enforcement* is a Phase-2 agent concern.

*(This Phase-1 cut is a starting proposal to refine, not a frozen boundary.)*

### Phase 2 — AI agents
Agents consuming the query API and issuing commands; seeded rollouts for evaluation.

### Phase 3 — Visualization
Web view of live/replayed games over the §3.3 boundary; server-authoritative and/or
WASM client.

### Phase 4+ — Rules fidelity toward 100%
Incrementally lift deferred systems, guided by exhaustive matching flagging unhandled
cases.

---

## 6. Technology Direction

- **Engine:** Rust. Value-oriented domain data + logic (its sweet spot), algebraic
  data types for the pervasive sum types, exhaustive matching to safely grow rules,
  `serde` for the save-game tier, native builds for AI/server, WASM for the browser.
- **RNG:** a single seeded, serializable PRNG owned by game state (determinism).
- **Testing:** engine-level unit/property tests are first-class; the rules engine is
  the highest-risk correctness surface. Worked examples from the rulebook (e.g., the
  extended combat example in §15) become regression tests.
- **Front-end (later):** TypeScript + a canvas/WebGL rendering approach, decoupled via
  the command/query + serialization boundary.
- **Reconsideration point:** D7 (engine language) is revisitable; C++ remains viable
  if preferred. The engine/front-end boundary makes this low-cost to change early.

---

## 7. Data Model Summary (pointer)

The detailed schemas are the next deliverable. At a high level the model comprises:

- **Reference tier:** terrain definitions, CRTs, capability-point costs, initiative
  ratings, unit characteristics charts, weapons-system charts, OA charts, reinforcement
  & replacement schedules, organization/attachment limits, scenario setup.
- **State tier:** the map's placed objects; units (identity, org links, TOE-point
  composition, morale, cohesion, CPs expended, status flags, gun position, reserve
  status, location, attached transport, supplies on hand); supplies & trucks (by line
  and weight, with cargo); and the sequence/turn state (game turn, ops stage,
  phase/segment, initiative, A/B assignment, weather).
- **Markers are partitioned:** unit-status flags (pinned, engaged, in-contact,
  reserve, in-training, short-of-stores, depleted) live *on the unit*; independent
  placed objects (supply dumps, minefields, fortifications, destroyed/broken-down
  vehicles, impact markers, construction) live *on the map*.

---

## 8. Implementation Status

Settled foundations (no longer open):
- ~~Confirm engine language~~ → **Rust** (D7).
- ~~Reference data source/format~~ → external **RON** files; parameters-vs-rules
  boundary (D9, D10, §3.8).
- ~~Phase-1 rules cut~~ → the simplified Land Game is complete and implementation has
  advanced well past the original Phase-1 boundary: the air, naval, and **full**
  Logistics systems (D15, superseding the §32 abstract mode named in §5) are now
  substantially built. §5's phasing is the original plan; this section is ground truth.

The engine **builds clean and 366 tests pass** across the workspace (unit tests in each
module plus integration tests in `lib.rs`). `cargo run --bin demo` narrates the three
land-combat systems end to end. The layering is consistent throughout: **reference**
(`reference/*.rs`, pure chart data from RON) → **rules** (`rules/*.rs`, pure stateless
arithmetic/eligibility, no game state) → **state** (`state/*.rs`, mutable instances) →
**command boundary** (`GameState::apply`, phase-gated `Command`/`CommandOutcome`/
`CommandError`). Resolution wiring lives in `state/game.rs`.

### 8.1 Reference tier — complete (land, air, naval, logistics)
All chart data is transcribed, RON-backed, and load-tested (human-readable companions in
`docs/source-materials/`; see §9). Beyond the original land charts (Terrain Effects; Unit
Characteristics & Weapons Systems CW/It/Ge; Stacking Point Values §9.4; the combat CRTs —
Anti-Armor §14.6, Barrage §12.6, Close Assault §15.79, Morale Modifier §17.4, Prisoners
§15.89; Initiative Ratings §7.2), the reference tier now also carries the **air** charts
(Aircraft Characteristics §4.44A; Refit §38.37; Scramble §40.4; Strafing §40.8; Air
Bombardment §41.5; Mining-Harbor §41.39A; Air-to-Air §45.4/45.5; Anti-Aircraft §46.3/46.4;
Air Recon §42.27; Malta availability/construction/commitment §44), the **naval** charts
(Port Capacity §55.3; Shipping Lanes §56.18; Convoy Level & Capacity §56.4/56.5; Naval Recon
§42.53), the **logistics** tables (ammo/fuel/water/stores rates, Truck Characteristics,
Breakdown §21.38, Repair §22, Equivalent Weights §54.5), and the **production** charts (§20:
German/Italian/CW production, Axis/CW truck production, CW infantry production, Replacement
Conversion §20.3). All are deserialized and unit-tested; the Replacement-Point production charts
are consulted by the `PlanProduction` command and the §20.3 Conversion chart by `absorb_replacement`
(§8.7); the truck/CW-infantry production tables are now driven by the `ProduceCommonwealthTrucks`
(§20.78A), `ProduceCommonwealthInfantry` (§20.78B), and `ProduceAxisTrucks` (§20.66a) commands.
**`ReferenceData::load_campaign()`** is the production bootstrap: it `include_str!`s and loads the
entire reference layer (every chart/table/CRT + the per-nationality unit/weapon/aircraft/production
datasets and all three full, chart-verified Orders of Battle) in one call, so
`GameState::new(ReferenceData::load_campaign(), seed)` yields an engine where every command has its
data — rather than each table being loaded individually (as the unit tests still do). An integration
driver test (`campaign_driver_advances_five_turns`) builds the engine this way, deploys the
Italian-offensive scenario onto the full map, and walks five complete Game-Turns through the Sequence
of Play — the first end-to-end exercise of the assembled engine, not just its pieces; a companion
test confirms previously-`NoTable` commands (production) now resolve from the bootstrap.

### 8.2 Turn driver / Sequence of Play — implemented
`GameState`'s turn driver (`advance_phase`) walks the full per-Game-Turn sequence, not just
the shared setup + A/B blocks:
- **Initiative Determination** (§7): d6 + Initiative Rating per side, ties re-roll, GT1
  optionally predetermined; the Axis rating is state-derived (Rommel-on-map / any German).
- **General Rommel (§31)** is modeled as a full moving counter (`RommelState`): a non-combat vehicle
  with a 60-CP Movement Allowance (`MoveRommel`, motorized medium-truck cost, blocked by enemy ZOC)
  that may `RetreatRommel` through an enemy ZOC with impunity (§31.2). Scenario-placed (hex + arrival
  Game-Turn); when present-on-map he sets the Axis Initiative Rating to 6 (§7.2), gives his stack +1
  combat Morale (§31.3, via co-location), and grants +5 CPA to one `DesignateRommelUnit`-nominated
  co-located unit that he stays with the whole stage (§31.4, owner ruling: a single unit). At each
  Game-Turn start a 2d6 recall roll (=12) sends him to Germany for that turn — suspending **all** the
  above (owner ruling), dropping the Axis rating to the German value (3).
- **Pre-turn sequence** (once/turn): Strategic Air Designation → Malta Availability → Strategic
  Mission Assignment → Malta Raid → Naval Convoy Schedule → Convoy Resolution.
- **Three Operations Stages**, each: Weather → Naval Convoy Arrival → Organization →
  Commonwealth Fleet → Land Support Air → then per-player {Reserve Designation, Movement &
  Combat, Truck Convoy, Commonwealth Rail, Repair, Patrol}, with per-stage CP reset.
- **Post-turn sequence**: Strategic Air Return → Aircraft Maintenance, then `game_turn += 1`.

**Weather (§29)** is rolled per section (seasonal Weather Table + Foul Weather Location Table),
with hot-weather fuel/water evaporation, sandstorm movement-cost doubling, and rainstorm well
replenishment. The **declare-A-or-B choice** is a real command (`DeclareInitiativeOrder`),
defaulting to initiative-player-first. **Victory** is implemented as the **§64.7 campaign
victory** (geographical occupation points §64.73 requiring an in-supply combat unit; §64.74
unused-replacement VP; §64.75 CW withdrawal VP; §64.76 band), with §64.71/§64.72 auto-win
including the one-full-turn objective-hold check. The **§60.81 Scenario-1 victory** ("Graziani's
Offensive") is also modeled: `scenario1_victory(side)` returns the highest level (Tactical / Decisive
/ Strategic) a side holds, each level requiring a supplied combat unit on its set of named objective
hexes (Tobruk, Sollum, Halfaya, Siwa, Mersa Matruh, Cairo/Alexandria, …). The specific §60.81 supply
trace is approximated by the general §64.73 adequacy test.

### 8.3 Command boundary — ~100 commands, phase-gated
`GameState::apply` covers land movement/combat, air, naval, logistics, and construction. Land:
`MoveStep`/`FormationMove`, `Barrage`/`AntiArmor`/`CloseAssault`/`Retreat`, `React`,
`MoveOffMap`, `DesignateReserve`, `RailMove`/`RailMoveSupply` (both sides — Axis rails under
§54.4), `PoisonWell`/`SweetenWell`.
Logistics/construction (Organization phase): `LoadTrucks`/`UnloadTrucks`, `AssignTrucks`/`ReleaseTrucks` (§6.17/§53.11), `SiphonFuel`/
`RefuelUnit`/`RearmUnit`, `BuildPipeline`/`BuildFortification`/`BuildRailroad`/`DemolishRailroad`/
`BuildRoad`, `LayMinefield`/`ClearMinefield`, `WithdrawUnit`/`ReturnUnit`/`AbsorbReplacement`,
`PlanProduction` (§20.66), `ProduceCommonwealthTrucks`/`ProduceCommonwealthInfantry`/
`ProduceAxisTrucks` (§20.78A/B, §20.66a), `ImportRollingStock` (§54.43 Axis rail),
`BlockHarbor`/`UnblockHarbor`, `Repair`. Naval: `PlanConvoy`/`BombConvoy`/
`DeliverConvoy`/`ReconConvoyLane`, `AssignShip`/`RepairShip`, `ArtilleryBombardShip`/
`SanGiorgioAttackShip`/`BombShip`/`OffShoreBombard`, `LayHarborMine`/`RemoveHarborMine`. Air:
`AssignSquadronPlanes`, `AssignPilot` (§40.12), `TransferSquadron` (§43.14), `ReadySquadron`/
`ArmSquadron`/`RefitSquadron`,
`DesignateCap` (§40.2), `Strafe` (§40.5), `AirdropSupplies` (§42.4), `TransportUnit` (§42.3),
`AirdropUnit` (§42.41), `FlySquadronMission`/
`FlyBombingMission`/`FlyBombingMissionAt` (map-validated, §8.5)/`BombAirFacility`/`BombMaltaBase`,
`DetermineMaltaAvailability`/`ConstructMaltaFacility`. Every command is gated to its
Sequence-of-Play phase.

### 8.4 Land combat & movement — complete for Phase 1
Resolvers for **Anti-Armor** (§14), **Barrage** (§12.6), and **Close Assault** (§15). Close
Assault models the differential (§15.26/§15.28) plus all four modifier systems — combined arms
(§15.4), unit size (§15.5, incl. §15.54 zero-def loner), morale (§15.6/§17.2 with §6.27/§15.64c
averaging, §17.23 clamp, §17.25/§17.26 Surrender, §17.28 Rommel), and terrain (§15.3) — then
losses, prisoners (§15.89), Engaged/In-Contact status, overrun (§15.77), and gun vulnerability
(§15.84). Barrage/Anti-Armor enforce adjacency (§12.31/§12.35 no LOS); Barrage applies the
§12.33/§12.34 terrain column shift. **Movement**: CPA over-expenditure/Disorganization (§6.21),
terrain & multi-feature hexside costs (§8.37) with escarpment rules (§8.42), continual movement
(as repeated single-hex steps), ZOC-to-ZOC prohibition and the §8.6 **break-off** surcharge — In
Contact (§8.62, in an enemy ZOC at segment start) costs +2 CP, Engaged (§8.63, a Close-Assault
result) +4 CP and applies even outside a ZOC (§8.66). Contact/Engaged status has a full **lifecycle**
(`refresh_contact_status`): In-Contact is re-derived from ZOCs each Movement Segment (§8.62), and an
Engagement clears once no opposing combat unit remains adjacent — i.e. when the last unit breaks off
(§8.67). Also **reaction** (§8.5 with §8.54 pinning), **retreat** (§15.82), **breakdown** (§21), and
combat ammo/fuel/water gating. **Fortifications & minefields are modeled:** a per-hex Fortification
Level (`fort_level` — placed forts plus the §25.12 static major-city/capital levels) applies its
column shift in anti-armor (§14.33 best-of), barrage (§12.34 best-of) and close assault (§15.35
cumulative), is raised by `BuildFortification` (§24.4) and reduced by fortification bombing (§41.37);
minefields (`LayMinefield`/`ClearMinefield`, §24.3) sit on the map with real/dummy status, dummy
reveal, and §26.2 entry casualty checks. §50.14 **defensive**-assault Ammunition is now consumed
by the melee defenders too (not just the attacker), and §9.35 retreat enforces the destination
Stacking Limit (a stack splits into different hexes by retreating each unit on its own path).
**Morale training (§17.3):** green Commonwealth units load at their OA-sheet untrained morale
(`basic_morale`) below their trained maximum (`basic_morale_reference`, the §17.31 parenthesized
pair). `TrainUnit` designates a unit to train in a §17.32 area (a Commonwealth capital hex —
Cairo/Helwan, Alexandria; the minor Amiriya/Abougir/Deghelia areas are a data gap) with a
co-located §17.35 instruction battalion (fully trained, ≥¼ TOE, matching arm); six uninterrupted
CP-free stages (§17.33/§17.34, tracked in `training_progress`) raise Basic Morale one step toward
the reference. This closes the §27.12 "trained to maximum Morale" gate on desert-raider formation.
The §15.35 hexside rules are modeled in full: the defender benefits from only one *type* of
hexside except when a single crossed edge carries two terrain features (the rare wadi-along-slope
case), where both apply. Close Assault always targets one hex's whole stack (§15.23), so there are
no multi-hex defenders.

### 8.5 Air game — rules & reference complete; resolution partial
The §33–46 ruleset is present as a large, thoroughly-cited pure-rules layer (`rules/air.rs`)
with matching placed state (`state/air.rs`: SGSU plane-readiness lifecycle, pilots, air
facilities, Malta base). Working end-to-end through the command boundary: squadron maintenance
(refit/ready/arm/fly with supply/stores gating), **bombing** of land/ship/facility/Malta/convoy
targets resolved through CAP interception → flak → Air Bombardment CRT, **air-to-air** duel
resolution (§45.4/45.5), and Malta availability/construction. **Map-based flight (partial):** the
`FlyBombingMissionAt` command flies a land-support bombing mission from the squadron's base hex to
a target hex, validating flight **range/distance** (§37.12) against the aircraft chart, the
day/night **mission capability** (§39.42), and the **sandstorm/rainstorm grounding** of the base
or target (§37.22); deriving the target's **flak** (§46) from the enemy AA units in the target hex
(`hex_flak_points`); and resolving **interception from map state** — `DesignateCap` puts a squadron
on Combat Air Patrol (§40.2) over a hex (validating CAP capability §40.21, range, and weather), an
offensive patrol of ≥3 fighters projects an air ZOC (§40.23, centre + six neighbours), and a
bombing mission entering an enemy offensive patrol's coverage is intercepted, spending that
patrol's one interception per stage (§40.26/§40.27). Patrols reset each Operations Stage. The
resolved mission then **applies its §41.3x land effect** (`apply_land_bombing_effect`): **Personnel**
pins battalion-equivalents in the hex (tank/gun units cost two, §41.31), **Supply Dump** destroys
`result`% of each commodity in the target's enemy dumps (§41.35), **Flak Destruction** removes
`result` AA-rated TOE Strength Points from an enemy AA unit in the hex (§41.33), and **Truck Convoy**
destroys `result` first-line attached Truck Points of an enemy unit (§41.32, D-capability planes
only; hex-placed convoys aren't modeled), and **City/Fortification** knocks a *placed* fortification
down one level (§41.37; units in a major city are shielded from bombing until its level ≤1, §41.31);
the ship (§41.34) and air-facility (§41.36) effects have dedicated commands, and **Road/Railroad**
(§41.38) destroys the feature on a positive result (a Road degrades to a track/UnfinishedRoad,
rebuildable §24.53; a Railroad hex's edges become DestroyedRailroad, rebuildable §24.63). The static
§25.12 major-city Fortification Level is now damageable too: §41.37 bombing knocks down whichever
level is effective (a placed fort, else the city's inherent level via `city_fort_damage`), so a
bombed-down city eventually drops to ≤1 and stops shielding its units (§41.31).
The `Strafe` command (§40.5–40.8) flies a map-validated strafing mission and resolves it on the
Strafing Table with the §40.8 column shift, applying the effect for **supply dumps** (§40.64, r×10%
of each commodity destroyed), **water pipelines** (§40.67, destroyed on r≥2), **infantry** (§40.6,
r TOE Strength Points eliminated from an enemy infantry unit in the hex), and **grounded aircraft**
(§40.6, the result is the number of d6 rolled — their sum is planes destroyed from an enemy SGSU in
the hex), and **first-line trucks** (§40.62, r Truck Points eliminated from an enemy unit's attached
trucks with the carried cargo lost in proportion), and **tanks** (§40.65 — armour-capable planes
only, i.e. the Hurricane Mk.IID `DayStrafeArmor`; the result is the number of d6 rolled and their
sum is the minimum Armour-Protection Points removed, taken as TOE weapon points from an enemy
armoured unit), and **trucks in convoy** (§40.62 — r Truck Points removed from an enemy on-map free
Truck Convoy, cargo lost in proportion). §40.55 also blocks strafing in a major-city hex. The only
§40.54 target not resolved standalone is **ports** — per §40.66 a port is strafed only as part of a
bombing raid (its half-value TacAir folds into the §41.5 bombardment), so a standalone Port strafe is
rejected. **Air transport & airdrop (§42.3/§42.4):** `AirdropSupplies` drops Ammo or Stores from a
transport squadron's base dumps into a target hex, validating transport capability, range, weather
(§42.45), and the §42.43/§42.44 drop-zone rules (clear/gravel/desert, not a major city, not
enemy-occupied or in an enemy ZOC and within two hexes of a recognizable land feature — waived for a
Friendly-occupied hex); delivered supplies are usable immediately. `TransportUnit` (§42.3) air-lifts
an eligible co-located unit (infantry-without-trucks or motorcycle/recce, §42.37) to a friendly air
facility, landing it and exhausting its CPA (§42.34), one plane per TOE point (§42.31). `AirdropUnit`
(§42.41/§42.42) drops an **airdrop-eligible** unit (the OB carries an `airdroppable` flag, set from
the OA sheet — the Folgore/Ramcke airborne units) into a legal §42.43/44 drop zone (no air facility
needed at the target), honoring stacking and the §42.48 once-per-12-stages limit. **Scramble (§40.3):** when no CAP covers a bombing target, an enemy fighter base
within Scramble-Table range (§40.4) may scramble to intercept — the first eligible squadron rolls a
d6 and, on success, its ready fighters fly and intercept (`scramble_interception`) — and the §40.33
**clear-path** rule is enforced: a squadron may scramble only if a fighter-free flight path to the
target exists (a bounded search that allows the §37.12 "any path" detour). Flight *distance* is
hex-count by rule (§37.12), so there is no routed path to model, and fuel is consumed per plane at
readying (§37.15, `ReadySquadron`); §42.47 forbids an airdropped unit from exceeding its CPA in its
drop stage. **Remaining operational abstractions** (project-owner ruling 2026-08-29, matching the
rulebook's "air combat is resolved on an operational level, with much abstracted"): the
defensive-CAP engagement *choice* (§45.14) and a precise per-plane cargo-tonnage cap for air
transport. **Box-based
flight (§37.4):** a squadron with no on-map base flies from its off-map Box using the transcribed
§37.4 Air Distance Chart — `box_flight_distance` looks up the Box→named-point distances and routes an
arbitrary target through the nearest on-map reference point (chart distance + hex distance),
respecting the chart's Prohibited routes; the bombing/CAP/strafe commands use `flight_distance` for
range (unchanged for on-map bases). Italy/Sicily takes the more favourable of the two §37.4 columns
(the region is modeled as one, §43.12). **§43.23** is enforced (a Crete bomber may not fly a bombing
mission in the first four Operations Stages of each month — the Suez commitment). **Escort/screening
(§45/§39.35–37):** a bombing mission may carry a fighter `escort` (`FlyBombingMissionAt`); intercepting
CAP/scramble fights the escort first (§45.2), and only the CAP's **excess** over the surviving escort
screen reaches the bombers (§45.32) — a matching or larger escort fully screens them, and a screened
bomber returns fire at each attacker (§45.34). Escort losses are applied to the escort squadron, so
§43.25's African-fighter escort of Italy/Sicily bombers works and
§43.24 is enforced (a Crete-based bomber may not take an on-map/African escort). **Voluntary abort
(§39.3):** a bombing mission may set `abort` — before any combat the bombers turn back (planes fly,
bombload dumped, no interception/flak/effect) if they qualify (§39.36/§39.37 a sufficient escort
screen — ≥25% of the covering CAP — or §39.35 superior Maneuver); an aborted mission does not engage
or consume the CAP (partial abort, §39.34, is supported — see below). **Land-unit air recon
(§42.27)** is modeled: the `ReconHex` command flies a recon-capable squadron (§42.21) to a target
hex, validates range/weather, rolls the §42.27 table (+1 per four planes) for how many enemy
battalion-equivalents are revealed — in the §42.24a order Tanks→Infantry→Artillery — and records the
reveal in `recon_revealed` so `observe_units` shows those enemy units' TOE Strength to the flier for
that Operations Stage (§8.10 fog-of-war). **Malta strategic-air commitment (§43/§44.41) is modeled:** each Axis
Malta Availability Level (I–IV) is limited to the scenario's Game-Turn count (the §44.41 chart),
tracked in `malta_level_used` and gated at `DetermineMaltaAvailability` (Level I unlimited, §44.25);
SGSUs carry a §43.0 Mediterranean base region (`med_base`: Italy/Sicily or Crete, else on-map
Africa), and a raid's **absolute plane count** is computed from the in-play Italy/Sicily bombers
(§44.25 map-% + strategic-%). `TransferSquadron` (§43.14) moves a squadron between the Boxes and
on-map Africa with one Operations Stage of transit (`transfer_remaining`; a squadron in transit may
not fly), refusing any move that would drop German strategic bombers below the §43.11 75%-Med
requirement — date-aware: before Game-Turn 35 it enforces §43.12 (≥75% of all German bombers in
Italy/Sicily), and from GT35 §43.13 (≥50% of strategic bombers in Crete). Flight *from* the Boxes is
executed: `flight_distance` routes a Box-based squadron's missions via the §37.4 chart
(`box_flight_distance`), the bombing/CAP/recon commands accept a Box-based squadron (`squadron_is_based`
covers `med_base`), §43.23 Crete→Suez timing (`crete_suez_window`) and §43.24/§43.25 Box-bomber escort
coordination are enforced, and §43.21 Box auto-supply is explicit (`consume_sgsu_upkeep` marks a
Box squadron supplied and draws no upkeep). Cargo/unit missions (transport, airdrop) still require an
on-map base, since their cargo is drawn from the base's dumps. (Both airborne formations are in the loaded OBs with their
battalions flagged `airdroppable: true` per the §42.41 OA sheets — the Italian **185 Folgore**
(`data/ob/italian.ron`) and the German **Fallschirmjäger-Brigade Ramcke** (`data/ob/german.ron`) —
so `AirdropUnit` is exercisable against real formations on both sides.) A **pilot
roster** (`AssignPilot`, §40.12) is modeled, and **air-to-air is resolved as individual
plane-vs-plane match-ups (§45.24)**: each flyable pilot's Rating (§40.15/§40.16) crews one plane
(best-first) and adds to that plane's TacAir; the larger side piles its extra fighters onto the
enemy (§45.23); reassignment/retraining/recovery lockouts (§40.13/14/17) tick each Operations Stage;
and **ace progression** (§40.18) is in — a
bombing interception credits the intercepting squadron's pilot with the bombers it downs, and a
pilot's effective Rating is the greater of his assigned Rating and his earned ace Rating (1 per 5
kills). All three bombing paths now credit the intercepting squadron's pilot: `FlyBombingMissionAt`
selects the CAP/scramble squadron itself, and the caller-supplied `FlyBombingMission`/`BombShip`
commands carry an `interceptor_squadron` so their interceptions credit ace kills too. **Voluntary
abort (§39.3) supports partial abort (§39.34):** `abort_planes` on `FlyBombingMissionAt` may be a
subset of the bombers — those turn back and dump (consuming fuel, §39.31/33) while the remainder
press on and resolve normally. (Per-plane pilots and the §45.23/§45.32 pile-on are modeled; the
§45.23/§45.33 match-up *assignment* choice uses a best-first/round-robin default.)

### 8.6 Naval / convoy — abstract model implemented
Ports (§55: efficiency, blocking §55.22, mining §55.27, San Giorgio §30.17), the Commonwealth
Fleet (§30: ships as gun-rating strength/damage, seaworthiness, repair cycle, at-sea/rest,
bombardment), and the **convoy pipeline** (plan → bomb → deliver, one/lane/turn §56.12, tonnage
from the Convoy Level Chart, survivors converted to supply points) all work end-to-end and are
where reinforcements enter (Naval Convoy Arrival Phase). **This abstraction is the faithful model,
not a shortcut** (rulebook §56.11: "the Shipping Lanes are not printed on the game map … identified
by their ports of origin and destination"; §56.12: "a convoy is the total tonnage assigned to a
Shipping Lane"). So there is deliberately **no hex-placed convoy / sea-movement model** — convoys
are lane tonnages attacked only by air (§41.6), and on-map convoy strafing does not apply.
**§42.53 naval reconnaissance** reveals only a convoy's *size class* (Small/Medium/Large), never its
exact tonnage: `ReconConvoyLane` returns the class the Axis reports, chosen adversarially among the
deliberately-overlapping §42.53 ranges (the most open-ended legal class, which conceals the true
tonnage best).

### 8.7 Logistics (full game, D15) — increments 1–9 built (production §20 included)
The §32 abstract supply mode has been fully removed as D15 directed; the interim coarse
`unit_in_supply` dump-reachability proxy has now been replaced by the real **§64.73 supply-adequacy
test** (`unit_supply_adequate`): a combat unit occupies for victory only if, at its hex, it has
Stores and Water for a week and Fuel/Ammo to move 20 CP and fire 3× — counting its on-hand supply
plus co-located friendly dumps (and a Water source for Water). §64.71's auto-win uses the separate
≤90-MP strategic convoy line, not this test. No §32 supply logic remains.
Implemented and tested: **(1)** commodities foundation (`Commodities{fuel,ammo,stores,water}`,
`SupplyDump`, setup dump placement); **(2)** Fuel §49 (rates/capacity, movement draw from
tanks→truck-cargo→dumps, siphon §49.17, evaporation §49.3/§29.34); **(3)** Ammo §50 (per-combat
consumption gating all three land-combat commands, one-fire reserve rearm); **(4)** Stores §51 +
the Stores Expenditure Stage (requirement draw, starvation attrition, half-rations, construction
& air-refit consumption); **(5)** Water §52 + hot-weather §29.35 (wells with secret depletion,
pipelines §52.2, the §52.6 Italian Pasta Rule, deprivation restrictions on move/combat);
**(6)** Trucks (Light/Medium/Heavy chart, motorization §6.17, breakdown §21.38, field/facility
repair §22); **(7)** supply transport/dumps + Commonwealth rail supply §54.3, **Axis use of the Commonwealth
railroad §54.4** (rail-hex control tracking §54.41, `ImportRollingStock` activating haulage from
250 Stores + 100 Fuel §54.43, Axis unit/supply rail movement bounded by the five-contiguous-hex
rule and the one-direction Rolling-Stock budget §54.44/46, Rolling Stock destroyed on losing control
§54.45, and repair restricted to controlled hexes with no new line §54.42), and the two-leg
strategic-supply trace for victory §64.71/72; **(8)** naval convoys §56 (see §8.6). **(9)
Production §20 — Replacement-Point planning now wired:** the `PlanProduction` command validates a
requested item/count against the nationality's §20.66/§20.78C chart (lifetime `total` and the
per-period maximum, via a rolling Game-Turn window keyed to each item's GameTurn/Two-Weeks/Month
period), records the required Axis convoy tonnage, and — mapping the item to a Replacement class —
schedules the points to arrive after the §20.21 delay (Axis 2 / CW 4 Game-Turns) and begin
Training, feeding the existing plan→arrive→train→absorb lifecycle. **Absorption applies the §20.3
Conversion cost:** `absorb_replacement` rebuilds each TOE Strength Point at its chart cost — an
affordable `options` set (plus a Stores Point when flagged) drawn from co-located, trained pools,
with requirement kinds collapsed onto the four pool classes — using a unit→row classifier
(`conversion_row_for_unit`). Its Machinegun / Heavy-Weapons / recce / Commando / Bersaglieri /
Paratroop detection was **verified against the §4.46 counter glyphs** transcribed from PDF p182–185
(`docs/source-materials/{italian,german,commonwealth}-counters-4.46.md`); the transcription confirmed
every MG/HW unit in the three OBs is named accordingly and fixed the German "Machinegewehr" gap. The
recce row is now derived from composition: a recce unit carrying armor-protected points is Armored
Reconnaissance, while an Organic/unarmored recce unit is the light Motorcycle/Camel/Reconnaissance
row (no longer a blanket "assume armored" default). With
no §20.3 chart loaded it falls back to legacy 1:1. **Axis Replacement-Point and Truck-Point tonnage
consume convoy capacity (§20.62):** at planning time (two turns out, before the tonnage die is
rolled, §20.63/§56.21) `PlanProduction`/`ProduceAxisTrucks` reject an order whose tonnage — added to
everything already committed to its arrival Game-Turn (via `axis_shipping_committed`) — would exceed
the die-6 *upper bound* for the month; the actual binding happens later against the **rolled**
allowance. The turn's roll is stored (`axis_shipping_rolled`, fixed by the first convoy plan, §56.21)
so supply and production bind to the same figure: a supply convoy may claim only the tonnage
production has left (§20.64 — Replacement Points have shipping priority over supplies), and at the
Naval Convoy Arrival Phase `defer_over_shipped_axis_replacements` caps arriving Axis production at the
roll, deferring the excess to the next Game-Turn (kept, not lost). Commonwealth production
(strategic-supply line, Alexandria) is exempt; with no Convoy Level Chart loaded the gate is skipped.
**Truck & Commonwealth-infantry production are now
driven:** `ProduceCommonwealthTrucks` (§20.78A: one d6 per truck type → Truck Points arriving +4
Game-Turns at Alexandria) and `ProduceCommonwealthInfantry` (§20.78B: 2d6 → Infantry Replacement
Points arriving +4 Game-Turns, fed through the standard replacement lifecycle) roll their tables;
`ProduceAxisTrucks` (§20.66a) plans against the row's lifetime total and per-Game-Turn maximum,
delivering +2 Game-Turns. Produced Truck Points land in a per-side free-truck pool (`truck_pools`)
at the Naval Convoy Arrival Phase; **`AssignTrucks`/`ReleaseTrucks`** (§6.17/§53.11) move Truck
Points between that pool and a unit's first-line trucks, closing the loop — assigned trucks feed the
existing §6.17 motorization (`unit_motorized_cpa`/`effective_cpa`) and §53.24 cargo capacity. The §20.78A
**"≤25% of the total trucks produced may arrive per Game-Turn"** cap is enforced (owner ruling
2026-08-30: scoped to the roll's own batch) — each Commonwealth truck type is divided as evenly as
possible across four Game-Turns (GT+4…GT+7). **§20.74 OpStage division** is also modeled for
Commonwealth production: within each arrival Game-Turn the arriving Truck Points / Infantry
Replacement Points are divided among the three OpStages as evenly as possible (Axis production is
exempt — §20.62 lets it arrive in any one OpStage). (The production item→class map's
SP-gun/tankette Tank calls were owner-confirmed 2026-08-29. The §20.3 absorb classifier resolves
special-infantry rows by the verified §4.46 names and recce rows by composition; an "Armored Car"
still needs an explicit name, absent an armored-car weapon kind in the data.)

### 8.8 Scenario, map & OB
The Scenario-1 order of battle and the full map are in: all five map sections (A–E) digitized,
load-tested, and loaded together (~7,100 real hexes). OB formations deploy from the §4.44 charts
via scenario `placements`, including **dispersed placement** (per-unit hex overrides, e.g. 7th
Armoured's §60.41 covering screen with the main body at D3612). Both reinforcement paths work:
**formation-internal arrivals** (later-arriving units of an on-map formation attach to their
on-map parent at their scheduled game_turn/stage) and **whole-formation entry** (a formation not
on-map at start enters at a resolved entry hex or off-map node), both resolving at the Naval
Convoy Arrival Phase. **All three faction Orders of Battle are now fully loaded and chart-verified**
against the §4.44a/b/c OA sheets (ids/weapons/parents/org-size test-validated), the DRAFT/D14
provisional stage retired: **Commonwealth** 36 formations (`data/ob/commonwealth.ron`, §4.44a
PDF p118–132), **Italian** 38 formations (`data/ob/italian.ron`, §4.44b p147–161; incl. the
D13-abstracted Marmarica/Cirene template divisions), and **German** 11 formations
(`data/ob/german.ron`, §4.44c p162–166 — the full DAK plus the Oasis Battalion, Kampfgruppen HQ's,
non-divisional artillery, and unassigned anti-air). Each faction's characteristics (§4.46) and
tank/gun/weapon charts (§4.47–4.49) were verified against the OA sheets, correcting the earlier
category-guessed ids. Both airborne formations (Italian **185 Folgore**, German
**Fallschirmjäger-Brigade Ramcke**) flag their battalions `airdroppable: true` (§42.41), so each
side has an `AirdropUnit` formation. The verbatim transcriptions live in
`docs/source-materials/*-ob-4.44*.transcribed.md`.

**Two scenarios are wired end-to-end.** `data/scenarios/italian-offensive.ron` is Scenario 1 (§60);
`data/scenarios/campaign.ron` is the **full Campaign Game (§64)** — per §64.3 it uses the §60.0
Sept-1940 initial deployment and then runs on (GT1→GT111, §64.2), with General Rommel arriving
Game-Turn 26 and the full reinforcement roster: every OB formation whose units carry explicit `Turn`
arrivals (the Deutsches Afrika Korps and the later Italian/Commonwealth formations) is listed as a
`reinforcement` and enters at its §20.14/§20.15 debarkation point on its OB schedule. The
`campaign_game_deploys_and_reaches_rommel` integration test builds the whole engine, confirms the
start force is Italian + Commonwealth only, and advances past Rommel's arrival with the DAK entering
on schedule. The German roster is now complete: two stray `Deployed` units — Ramcke's I Bn (Kroh) and
the II/155 reassignment counter — were dated from the §4.44c OA sheet (1/91 and the Nov-1941 2/58
reassignment), so **Ramcke and the Non-Divisional Artillery pool now enter on schedule** rather than
wrongly at Sept-1940; the **Kampfgruppen HQ's** stay omitted as a dynamic form-at-will pool.
**Scenario setup now covers the air dimension too:** the scenario RON accepts `squadrons` (SGSUs
with their plane groups, based on-map or in an off-map Med Box, planes deployed ready+armed) and
`air_facilities` (§36 airfields/strips/basins) — the loader builds `sgsus` + `air_facilities`
(`load_scenario_deploys_air_forces` test). The air *engine* was already complete; this wires air
into scenario deployment. The CNA air model decouples planes from squadrons: aircraft arrive as a
per-turn **plane pool** (§34.84 Commonwealth / §34.85 Axis reinforcement charts — now transcribed to
`docs/source-materials/air-reinforcements-34.84-34.85.transcribed.md`, GT2→GT110, all three
nationalities, names mapped to the `data/reference/aircraft/*.ron` keys) and the player assigns them
to SGSU counters (§35.2). The **plane-pool + reinforcement mechanism is now built**: the schedule is
loaded into `ReferenceData::air_reinforcements` from `data/reference/air-reinforcements.ron`
(generated from the transcription, 78 monthly periods), `GameState.plane_pool` holds each side's
received-but-unassigned planes by aircraft type, and `accrue_air_reinforcements` (called on every
Game-Turn rollover) adds that turn's arrivals — each month's total split evenly across its
Game-Turns (`AirReinforcementSchedule::arrivals_for`). The **§60 starting plane pools are seeded**:
per §60.32 (Italian) and §60.42 (Commonwealth) the scenario `initial_planes` field loads each side's
GT1 aircraft into `plane_pool` (no German air until the DAK, GT26+) — `campaign_seeds_initial_air_pool`
verifies 133 SM79 / 70 CR32 (It) and 36 Gladiator / 6 Hurricane I (CW). Per §60, planes are assigned
to squadron counters and based at airfields **by the player** within class/capacity limits, so the
pool is the authoritative starting state. **Remaining air work:** the **SGSU counter roster** (how the
39 Italian / 14 CW squadrons split by class) + a default opening deployment / `AssignSquadronPlanes`
drawing from the pool; pilots (§60.32/§60.42 give the ratings); and squadron **withdrawals** (`WD`).
The §60.32 "2801 9/9" line (an OCR garble in the text rules) is omitted pending verification. Naval fleet deployment is the analogous
still-missing dimension (a `fleet` scenario field + a ship OB).
**D14 verification pass (2026-09-08) — closed.** Every `Deployed`-default unit in the Commonwealth
and Italian OB was cross-checked, formation-scoped, against the §4.44a/§4.44b OA-sheet "Arrives"
columns transcribed in `docs/source-materials/*-ob-4.44*.transcribed.md`. All are faithful: the
Italian 10th Army was entirely in Libya in Sept 1940 (every garrison/division/pool correctly "D");
6th Australian and 2nd New Zealand carry "D" HQs on their own sheets (footnoted "arrived early, in
sections, not committed to battle for months"); 70th Infantry matches its sheet exactly (HQ 2/21,
16th Bde `Deployed` on loan to 4th Indian, 23rd Bde 3/51). **Zero OB corrections were required** —
unlike the German fixes, these were not transcription gaps. Note that 70th Infantry, 6th Australian,
and 2nd New Zealand are intentionally *absent* from `campaign.ron`: the on-map start force is the
§60.41 CW deployment chart (7th Armoured + 4th Indian + the border battalions), and the §4.44 "D" is
a master-OB availability marker gated by each scenario's own deployment chart — it does not by itself
place a formation. The loader enforces this: `register_pending_arrivals` only schedules `Arrival::Turn`
units, so a reinforcement-only formation's `Deployed` units are never spuriously placed.

### 8.9 Desert raiders & commandos (§27) — implemented
Desert raiders are a bespoke counter (`Raider`, LRDG ≤2 Commonwealth / DSA ≤1 German): a non-combat
Recce vehicle with a 50-CP allowance, no ZOC/stacking, formed from a Recce/AC TOE point (§27.13,
`FormRaider`) and re-forming four Game-Turns after elimination (§27.16). They move ignoring enemy ZOC
(`MoveRaider`, §27.34), risk spotting on entering an enemy-controlled hex (§27.43) and via enemy
`SpotRaider` (§27.44), draw special supply (`ResupplyRaider`, §27.35: 2 Fuel + 1 Ammo from a dump,
Water from an oasis/bir, water-or-die each Game-Turn, 2 Fuel to move), and perform all five §27.5
raids — `RaidPipeline`/`RaidAirfield`/`RaidPlanes`/`RaidSupplyDump` (guarded via §27.55b) /
`RaidTruckConvoy` — plus the LRDG's §27.6 `RaidRommel` (the §27.64 table). Truck-convoy raids target
faithful **free truck convoys** (§8.98: unattached Truck Points hauling supply, `FormTruckConvoy`/
`MoveTruckConvoy`, moving in the Truck Convoy Phase under the §10.29 ZOC rule). The **Layforce**
commando (§27.7) lands amphibiously from Alexandria into any coastal hex after a full Game-Turn there
(`LandLayforce`, §27.72/73), is auto-supplied for three Operations Stages (§27.74), and re-embarks to
Alexandria (`EmbarkLayforce`, §27.77). The **SAS Brigade** (§27.8) lands by sea (`LandSas`, §27.83)
with the §27.84 three-stage supply window and raids grounded Axis planes (`RaidAirfieldSas`,
§27.85/86 via the §27.93 table, §27.87 ≥6-stage frequency), gaining CPA 30 when escorted by an LRDG
(§27.88). §27.59 convoy interception (`InterceptTruckConvoy`) lets a raider within four hexes of a
passing convoy attack it in the Truck Convoy Phase. Positions are tracked for real; the §27.32
hidden-movement concealment is now enforced at the *view* layer (§8.10 — a hidden raider is absent
from the enemy's observation). Faithful details now closed: §27.12 "trained to max Morale" is
enforced at `raider_source_ok`; §27.38 raiders consume no per-hex fuel (only the §27.35 flat
allotment), matching the rule; §27.58 replacement-cargo now resolves both the "any truck" case (+1
to the roll, a 6 eliminates the raider) and the "all trucks" case (raider auto-eliminated, no truck
losses); §27.54 plane raids are side-tagged to enemy SGSUs and §27.53 airfields are non-denominational
by §36.15 (no owner to tag — the raid gate is airfield-present / no enemy combat units / not a major
city). Remaining simplification: §27.59 range as hex distance ≤4 (proxy for "4 Movement CPs"). The
§27.93 SAS Raid Table (0/10/25/25/33/50 by d6) is owner-confirmed (2026-08-30).

### 8.10 Fog of war (§3.5 / D5) — per-side observation layer implemented
The core `GameState` holds ground truth; `crate::state::view` derives a **thin per-side view** of
what a player is entitled to know (§3.5), so an AI agent or UI acts on knowledge, not omniscience.
Non-mutating query methods: `observe_units(side)` shows every on-map counter but hides an enemy
unit's current **TOE Strength** (`strength: None`); `observe_supply_dumps(side)` shows a friendly
dump's contents/dummy flag but reveals only a marker for an enemy dump (§32.18); `observe_raiders(side)`
shows own raiders always and enemy raiders only once **revealed** (a new `Raider.revealed` flag set
when a raider raids or is spotted, §27.32/§27.33, and cleared at the next Operations Stage so a
revealed raider re-hides). `observe_minefields(side)` shows each marker + owner but hides real/dummy
status until the field is the viewer's own or has been revealed (§26.15/§26.23). This closes the
§27.32 hidden-movement caveat for the enemy's *view*. **Well secrecy (§52.14/§52.16)** is modeled
with a "discovered" scheme: `well_known_by` records the side that drew a well dry or poisoned it (it
alone knows), and an opposing draw in `expend_water` flips the well into the public `revealed_wells`
set (§52.14's placed marker); `observe_well_depleted(side, hex)` / `observe_well_poisoned` return
`Some(status)` only to a knower or once revealed, else `None`. Rainstorm replenish (§52.15) and
sweetening (§52.17) clear that bookkeeping. (Exact convoy tonnage is already class-only via §42.53
recon.) The view layer now covers every major hidden-info category — units, dumps, raiders,
minefields, wells. No command *enforces* acting only on observed info (rules resolution legitimately
uses ground truth); consuming the view is a Phase-2 agent concern.

**First Phase-2 primitives (agent support).** `dry_run(&cmd)` tests a command against the command
boundary without mutating state (clone-and-apply), the reusable "is this legal / what would it do?"
oracle. `legal_move_steps(side)` and `legal_commands(side)` enumerate a side's legal Movement &
Combat actions (every single-hex move, plus a CloseAssault/AntiArmor against each adjacent
enemy-held hex), each candidate validated through `dry_run` so the engine — not a re-implemented
gate — is the source of truth. A seeded campaign **fuzzer** (`campaign_fuzz_survives_random_play`)
drives tens of thousands of random commands through several Game-Turns to confirm the command
boundary never panics on illegal input. To make `dry_run`/rollouts cheap, `Map` and `ReferenceData`
are held behind `Rc` with copy-on-write in their mutators, so cloning a whole `GameState` is O(1)
(~12µs) rather than a deep copy of the chart/OB layer and the 7,100-hex map — a probing clone shares
both until something writes to the map. The residual per-move cost is `apply` itself (movement/ZOC
resolution), not the clone. Remaining enumerator extensions (Barrage, reorg/logistics/air categories)
are future work.

**Agents (Phase 2 kickoff), `crate::agent`.** An `Agent` trait (`choose(game, side) -> Option<Command>`)
picks the next command to issue, always from `legal_commands` so an agent can only attempt legal
actions. Two reference players ship: `RandomAgent` (uniform over the legal set) and `GreedyAgent`
(assault an adjacent enemy if able, else advance the piece that gets closest to the nearest enemy);
both are seed-deterministic with their own PRNG separate from the game RNG. `self_play` drives two
agents against each other through the Sequence of Play — during each Movement & Combat phase the
active side's agent issues commands (validated by `apply`) up to a per-phase cap, and every other
phase is driven by `advance_phase`. A third player, `LookaheadAgent`, does a one-ply search: for
each legal command it clones, applies, and scores the result with `GameState::heuristic_value(side)`
(material advantage — own minus enemy TOE, weighted — minus a closeness term that presses toward
contact), choosing the command that most improves its position and yielding when nothing strictly
does. The O(1) clone makes evaluating the whole action set per decision cheap. `legal_commands` now
also enumerates Barrage (one candidate per target class in a hex). Two known limits point to the next
step: the eval scores a single *sampled* combat/breakdown outcome (averaging rollouts would de-noise
it — e.g. it correctly declines to advance an unreliable tank that broke down on the probe roll), and
goals/supply/air/logistics actions and deeper search are still to come.

`RolloutAgent` is the flat Monte-Carlo answer to that noise: for each legal command it runs N
rollouts — clone, **`reseed_rng`** the clone (independent dice per sample; a bare clone would replay
identically), apply the command, play up to `depth` further random plies, then score with
`heuristic_value` — and picks the command with the best *average*, so one unlucky/lucky roll no longer
swings the decision. `depth = 0` is pure immediate-outcome averaging; larger `depth` adds shallow
lookahead. All rollouts run on throwaway clones, so game determinism is untouched.

Building these agents also flushed out a real movement bug (the value of exercising the command
boundary this hard): `MoveStep` did not forbid entering an **enemy-occupied** hex — the rollout agent
gleefully "advanced" onto the enemy to zero its distance term. Fixed in `move_counter` per §8.13 (a
unit attacks an enemy-held hex, it does not enter it); non-combat enemy markers still don't block.

`MctsAgent` is the tree-search step up from flat rollouts: a UCT search that plans a whole Movement &
Combat phase. Since the opponent does not act during your own M&C phase, planning is a single-agent
search over sequences of your commands — tree edges are legal commands plus a **Pass**, a node is
terminal at Pass or the `max_actions` depth cap. Stochastic transitions are handled by generating
every node/rollout state on a **reseeded clone** (the tree samples chance rather than assuming it),
rewards are `heuristic_value` at the rollout leaf, min/max-normalized to `[0,1]` for UCB1, and
`choose` returns the most-visited root child's action (or Pass → yield). Each decision re-plans from
the true state (open-loop), adapting to what the dice actually did. The engine now has five agents —
random, greedy, 1-ply lookahead, flat Monte-Carlo rollout, and UCT — all sharing the `legal_commands`
enumerator and the O(1) clone.

`CrossPhaseMctsAgent` closes the last gap: an **adversarial UCT that spans multiple phases and models
the opponent**. A tree node is a decision point (a Movement & Combat phase) tagged with whose decision
it is; a command edge keeps that side acting, while a **Pass** edge advances the Sequence of Play
(`advance_to_movement_combat`) to the next decision point — often the opponent's. Rewards are stored
from the *root* side's perspective, and UCB exploits high values at root-decision nodes but *low* root
values at opponent nodes (minimax via player-aware exploitation). Rollouts play random self-play across
phases to a turn horizon, then score `heuristic_value(root)`; all states are reseeded clones, so chance
is sampled and the live game is untouched. Bounded by `turn_horizon` / `max_depth` / `rollout_steps`.
The engine now carries six agents from trivial to adversarial-search.

**Richer evaluation.** `heuristic_value(side)` combines five terms: **material** (own − enemy TOE,
weight 100, dominant), **objectives** (net §64.73 Geographical-Occupation VP of the victory cities the
side holds with an in-supply combat unit vs. the enemy's, weight 60 — the actual win condition),
**cohesion** (own − enemy summed Cohesion, weight 1), a **supply** penalty per own out-of-supply
combat unit (§64.73 adequacy), and the **closeness** tie-breaker (advance toward contact). So every
agent now trades off strength, ground, order, and logistics rather than raw TOE alone. Remaining AI
work: progressively widen `legal_commands` to the reorg/logistics/air action categories so search can
plan those too, and tune the term weights via self-play.

**Phase-aware enumerator.** `legal_commands` now dispatches by phase: Movement & Combat yields
moves + close-assault + anti-armor + barrage (as before); the **Organization phase** yields a bounded
logistics/reorg set — refuel and rearm from a co-located dump, detach from a parent, and attach to a
co-located friendly unit — each validated through `dry_run`. The agents' decision-phase gate and the
`self_play` driver were widened to act in both phases, so a value-based agent will, e.g., refuel an
out-of-supply unit in the Organization phase to clear the eval's supply penalty.

The enumerator now covers three more phases on the same `dry_run`-validated pattern: **Land Support
Air** (air reconnaissance §42.27 over enemy hexes, offensive CAP §40.2 over friendly hexes, per ready
squadron), **Truck Convoy** (move each free truck convoy §8.98 and escort each prisoner stack §28.13
one hex), and **Repair** (repair each piece of captured equipment §28.3/§22.5). Agents and the
`self_play` driver act in all five decision phases now, so a game can be driven end-to-end by agents
across the Sequence of Play.

**Enumerator depth** fills those categories in: air now fans out **bombing** (`FlyBombingMissionAt`
over the land-effect §41.3x missions) and **strafing** (`Strafe` over the §40.5x ground targets)
against each enemy hex, on top of recon/CAP; **Repair** adds the die-carrying `Repair` for a unit's
broken-down trucks / destroyed tanks / recce points (a representative roll — legality is roll-
independent) alongside captured-equipment repair; and two naval phases are enumerated — **Commonwealth
Fleet** (`RepairShip`, `AssignShip` to Alexandria/Valetta per ship) and **Naval Convoy Arrival**
(`DeliverConvoy` per pending convoy). Every candidate is still `dry_run`-validated, so only legal,
in-range, capable actions survive. Agents act across all seven decision phases. What's left is mostly
parameter breadth plus tuning the eval weights via self-play.

**Parameter breadth.** Bombing missions now fan out over **escorts** — unescorted plus each available
fighter squadron (§45), with `dry_run` rejecting ineligible escorts (e.g. the §43.24 Crete rule); and
the Fleet enumerator adds **at-sea bombardment stations** — `AssignShip::AtSea` to each sea hex off an
enemy-held coast, alongside the port/repair options. Convoy *planning* (`PlanConvoy`: lane +
destination + four tonnage figures + arrival turn) is deliberately **not** brute-enumerated — it is a
strategic choice better made by a purpose-built policy than by fanning out a huge parameter grid;
`legal_commands` stays the tactical-action enumerator. Off-shore-bombardment target selection and
per-plane bombload are the remaining narrow gaps. Net: agents can now attack from the air with escort,
maneuver the fleet for shore support, and act in all seven decision phases.

**Self-play eval-weight tuning.** The eval's term weights are now an `EvalWeights` struct
(`heuristic_value_with`); `heuristic_value` delegates to the default blend, and `LookaheadAgent`
carries its own weights (`with_weights`). `agent::tune_weights` runs greedy **coordinate ascent**:
from the default weights it tries raising/lowering each of the five terms and keeps any change that
improves `evaluate_weights` — the average `outcome_score` (a fixed, tuning-independent VP-then-material
measure, so the objective doesn't move under it) of a `LookaheadAgent` using the candidate weights
against a fixed default-weight opponent, over several seeds and both side assignments. It's a simple
local search — a foundation for stronger optimisation (population methods, self-play against the
current best, more games per evaluation, deeper agents) — and, thanks to the O(1) clone, evaluations
are cheap: a game builder can hand out `template.clone()` rather than reloading the reference layer.
The tuner is **co-evolutionary steepest ascent**: each round holds the current champion fixed as the
opponent and promotes the ±`step` neighbour that beats it by the largest margin, stopping when none
beats the champion's own self-play baseline (a stronger convergence test than the old "beat the fixed
default", and one that never regresses). The tournament bin's **`tuned` roster** closes the loop: it
tunes on the chosen game (6 seeds = 12 games per evaluation, step 15), then enters a Lookahead using
the tuned weights alongside the default one. Result: even this stronger, variance-reduced search
returns *exactly* the default blend (Tuned vs default is an even 3-0-3) — so on the skirmish the
hand-picked weights are a genuine **self-play local optimum**, not merely un-tuned: no single-coordinate
change makes the eval a stronger opponent. (The likely reason is that the small, short skirmish is
material-dominated, so the minor terms can't separate; whether tuning helps on longer/larger games is
open, gated by the cost of tuning at that scale.)

The remaining **narrow enumerator gaps** are now filled: **off-shore bombardment** (`OffShoreBombard`
— a heavy Commonwealth ship at sea shells an adjacent enemy-held hex, per target class, in Movement &
Combat); **per-plane bombload** (each bombing candidate at a light and a heavy load, `dry_run` dropping
over-capacity ones); and **convoy planning** as a bounded **policy** rather than a brute grid — one
modest balanced supply convoy per Axis shipping lane to its destination, arriving after the §20.21
delay (`legal_convoy_planning_commands`, Naval Convoy Schedule phase; full tonnage/destination
optimisation stays a job for a purpose-built logistics planner). Agents act across eight decision
phases. `legal_commands` is a broad, faithful action surface end to end; the remaining AI work is
stronger optimisation (real self-play tuning runs, tuning the search agents), not more enumeration.

**Tournament harness (agent strength measurement), `crate::agent`.** `round_robin` plays every agent
pair over N seeds × **both side assignments** (cancelling the side/initiative bias), scoring each game
by the final `outcome_score` margin and returning `Standing`s (games/W/D/L, win-rate, average margin)
sorted best-first; `format_standings` renders the table and `Contender` wraps a per-game agent factory.
The `tournament` bin runs it over a compact **skirmish** (default — a few supplied units per side on a
small clear map) or the full **campaign** (opt-in). A `tournament_round_robin_is_consistent_and_deterministic`
test asserts the bookkeeping invariants (equal game counts, wins == losses, margins cancel, sorted, and
byte-identical on replay); an `#[ignore]`d campaign run asserts Random never finishes top-half.
Measured (skirmish, `fast` roster, 6 turns × 4 seeds, 48 games), representative run: **Rollout ≈ 63–71 %,
Lookahead ≈ 58–73 %, Greedy ≈ 56 %, Random ≈ 12–23 %** — the eval/search agents clearly dominate and
Random is decisively worst, validating the whole Phase-2 stack. (The exact percentages vary a little
across process runs — combat resolution iterates units in per-process-randomized `HashMap` order — but
the ordering shape is stable; results reproduce within a process. Deterministic cross-process tournaments
would need ordered iteration in combat/casualty allocation, a separate follow-up.)

**Enumerator performance (the confirmed bottleneck, then fixed).** The tournament first showed even the
cheap agents were slow, and a microbenchmark (`perf_hotpath_microbench`, 81 units) localised it:
`legal_commands` took **≈187 ms** per call, essentially all in `legal_move_steps`, because each of the
~1 000 candidate moves was validated by `dry_run` = clone + full `apply` (the move's execution — fuel/CP
mutation, breakdown, `refresh_contact_status` — all O(units)). The fix splits move handling into an
immutable **`plan_move_step`** (all validation + cost, no mutation) shared by both `move_counter` (which
then executes) and a new **`can_move_step`** the enumerator calls instead of `dry_run` — so enumeration
pays neither the clone nor the execution. Result: `legal_commands` **≈187 ms → ≈5 ms (~36×)**. Two
equivalence tests (`can_move_step_matches_dry_run{,_over_evolving_game}`) assert the fast path accepts
exactly the moves `apply` does, so the shared validation cannot silently diverge. This made
**search-agent tournaments viable**: a full six-agent skirmish round-robin (previously it never
finished) now completes, and **`MctsAgent` tops the table (~80 %)** ahead of Rollout, Lookahead,
CrossMCTS, with Random/Greedy at the bottom — tree search is measurably the strongest agent.

**Where the clone does *not* matter (measured, not assumed).** The obvious next guess was the
`GameState` clone (units are a plain `HashMap<UnitId, Unit>`, deep-copied per clone). Measurement
killed that idea: `MctsAgent::standard.choose` is ~4 s per move on an 81-unit state, of which the
clone (~20 µs × ~700 clones ≈ 14 ms) is **under 1 %**. The cost is `legal_commands` itself, which
`MctsAgent` calls ~700× per move (100 iterations × depth-6 rollouts) — and it is still **O(units²)**
because each `can_move_step` runs O(units) scans (`units_in_hex`, `hex_stacking_points`,
`hex_in_zoc_of`). So the real scaling lever for *campaign*-scale MCTS is a **per-hex occupancy /
ZOC index** (turning those scans O(1), making enumeration O(units)); the clone refactor is not worth
its churn. Skirmish-scale search tournaments already work today.

### 8.11 Patrols, reconnaissance & retreat-before-assault (§13 / §16)
**§13 Retreat Before Assault** (`RetreatBeforeAssault`) — a non-phasing, unpinned unit (Cohesion
> −26) voluntarily withdraws after barrages: ordinary movement (CP + Fuel, breakdown, §8.6
break-off, ZOC rules) capped at 4 CP / one hex unless it began adjacent to an enemy (§13.24).
**§16 Patrol & Reconnaissance** (`Patrol`, Patrol Phase) — the phasing player detaches Patrol
Points (≤2/hex, ≤3 total) from Recce/light-tank/motorized-infantry units (§16.11), spending
1 Ammo + 2 Fuel each (§16.16), to reconnoitre an enemy hex ≤5 away, resolving on the §16.6
Survival / §16.7 Reconnaissance / §16.8 Objective-Loss tables (`reference/patrol.rs`, transcribed
from PDF p96/p98). The defender's disclosures are command inputs — a reported TOE within ±2 of
actual (§16.51), the defender choosing which units within §16.53/54/55 — returned once in a
`PatrolReport` (not persisted; the player remembers externally). §16.21's no-combat/no-barraged-hex
preconditions use per-stage `land_combat_this_stage` / `barraged_hexes_this_stage`. **§16.4 Dummy
Tank Formations** (`PlaceDummyTank`) — a per-side bluff (10 Stores, ≤3/side, ≤1/hex, §16.42/43),
eliminated by close assault or armorless anti-armor (§16.46). Range paths use hex-distance
simplifications; the §16.45 dummy disclosure and §16.34 Prisoner-Point accounting fold into the
defender's narration / the §28 POW logistics (§8.13).

### 8.12 Reorganization (§19)
Units carry both an `assigned_parent` (organizational membership, occupies TOE space even when not
co-located) and an `attached_parent` (physically combined onto the parent's counter, same hex),
set at scenario setup (§19.1). The reorganization commands act on them: **`AttachUnit`** (§19.41/42
— 1 CP each for an assigned unit any time, 2 CP each for a non-assigned unit in the Organization
Phase within the §19.5 limit), **`DetachUnit`** (§19.43/44 — 1 CP each, Organization/Movement),
**`ReassignUnit`** (§19.21/28 — a currently-attached, not-assigned-elsewhere unit, Organization
Phase, no CP). The §19.5 Maximum Attachment Chart is transcribed (`maximum-attachment-19.5.md`) and
its numeric caps enforced by `nonassigned_attachment_limit` (keyed by parent org-size/nationality/
**arm** (`is_armor`)/Game-Turn — the arm split matters: a CW infantry division is capped at 2 even
at GT 68+ where an armor division rises to 3, a CW non-tank brigade at 1 where a Tank Brigade is 2
before GT 68, and a German infantry division at 3 where an armor division is 4). The one un-modeled
§19.5 detail is the chart's *Modification* allowing two non-shell company-equivalents to attach free
of the maximum (3 company-eq = 1 Bn-eq) — every attachment currently counts equally. The **§19.3 assignment maxima** are enforced too: since §19.3's Formation Organization Chart is not
in the shared Charts & Tables (its note defers "singular" formations to their OA-sheet structure
anyway), each formation's historical assignment count from the loaded OB is captured at scenario
setup as its `assigned_capacity`, and `ReassignUnit` refuses an assignment that would push a
parent's assigned count above it (§19.21; eliminated units don't count, §19.23).
§19.46 nesting is enforced: a non-assigned attachment counts against `parent` **and every formation
it is transitively attached to** (`attachment_ancestors` × `transitive_nonassigned_attachments`), so a
nested attach is refused if any ancestor is at its §19.5 limit.
The §19.5 **type constraints** are enforced too: each formation's arm (`is_armor`) is derived from
its OB organization at setup (it has tank units assigned), selecting the §19.5 row's "no Tank" /
per-type-maximum rule, which `attachment_type_ok` checks against the attaching unit's type across
every ancestor (§19.46). The named/special-arm rows are covered too, via a `formation_role` derived
from the OB at setup (formation designation / organic arm): the German **Artillery Brigade HQ**
(its one attachment must be an Artillery/gun unit), the **Italian infantry division** ("1 Infantry
*or* 1 Tank" — combined ≤1), and the **Matruh Garrison / Selby Force** (GT 1–12: 6 units, no tank,
≤1 gun; GT 13+: 5 units, no tank, no recce, ≤3 infantry). Documented simplifications: CW
battalion-shuffle (§19.26), historical assignment shifts
(§19.24/25), substitution (§19.45) and nesting constraints (§19.46) fall out of the general
reassign/attach rules or are left as flavor.

### 8.13 Prisoners & captured equipment (§28)
Full POW logistics. Surrender (§17.25 / morale) routes captured Strength via `capture_on_surrender`:
**infantry (Organic) TOE → Prisoner Points** (§28.11), **weapon (tank/gun) TOE → captured equipment**
(§28.3). Prisoner stacks (`prisoner_groups`) are fed 1 Store per 5 per Operations Stage from the
nearest dump (§28.15, in the Stores Expenditure Stage), guarded by Guard Points formed from infantry
TOE (`FormGuard`/`ReturnGuard`, §28.22), moved one hex at a time in the Truck Convoy Phase
(`MovePrisoners`, CPA 8, §28.13) with escort (1 guard per 5, §28.17) — un-escortable excess escapes
(§28.23) — and removed permanently at a Departure Point (`pow_departure_points`, §28.16). Escapes
(§28.23–25): an unguarded stack escapes; escapees within eight Capability Points (foot) of a friendly
unit rejoin and return as Infantry Replacement Points a month later (§28.24), and are recaptured if an
enemy enters their hex (§28.25). A **far** escapee (beyond 8 CP) now moves autonomously: each
Operations Stage it steps one hex toward the nearest friendly combat unit over passable,
non-enemy-occupied desert and, with no water source in its hex, loses 10% (rounded up) of its strength
to thirst until it reaches safety, is recaptured, or dies out (§28.24). Prisoner Stores (§51.12) and
Guard-Point Stores (§51.17, 2/point/Game-Turn) are drawn from the nearest dump.
Captured equipment is repaired via `RepairCapturedEquipment` (§28.3):
per §22.5 a repaired vehicle is "treated as Replacement Points for that type of unit ... except that
they do not have to undergo Training", so it re-enters play as a **trained** Replacement Point pool of
the matching class (Tank/Recce/Gun, from the weapon system's kind) at the hex, for the captor — usable
immediately against the former owner (§28.3), one point per captured TOE Strength Point.
Documented simplifications: the §28.17 moving-guard ratio is read as 1-per-5 (the 5-based POW system;
the printed digit was ambiguous); ranges use hex distance; the §28.12 free 3-hex relocation and the
40-per-hex camp cap fold into ordinary `MovePrisoners`.

As we digitize physical game components (charts, tables, counters, OA sheets, scenario
setups), we save a faithful, human-readable transcription so future agents and humans
can reference them without the physical materials or repeated OCR.

- Human-readable transcriptions live under `docs/source-materials/` as markdown — they
  can hold footnotes, rule citations, and provenance notes, and are easy to review.
- Once a chart's schema is settled, its authoritative machine values live in the RON
  files under `data/reference/`; the markdown transcription remains as the readable
  companion and provenance record.
- Any cell that could not be read is flagged and left awaiting confirmation.
- OCR tooling: Apple Vision via `tools/ocr.swift` (`swift tools/ocr.swift <image>`) has
  proven accurate for screenshots and returns per-line bounding boxes useful for
  reconstructing table layout. Note: macOS screenshot filenames contain a narrow
  no-break space (U+202F) — match them with a shell glob rather than typing the literal
  path.
- Source PDF & page extraction: the full rulebook PDF lives at
  `docs/source-materials/The Campaign for North Africa.pdf` (gitignored — large binary,
  kept locally). It is the cleanest source. Render a page to a high-DPI PNG with
  `tools/pdf-page.sh <pdf> <page> [dpi]` (wraps `pdftoppm`), then OCR with
  `tools/ocr.swift`. Vector-PDF rendering at 300 DPI is far cleaner than screenshots and
  avoids the dropped-cell problem seen when OCRing screen captures of dense charts.
- Done so far (each: `docs/source-materials/*.md` + `data/reference/**/*.ron`):
  - **Terrain Effects Chart** → `terrain-effects-chart.md` / `terrain.ron`.
  - **Unit Characteristics** (§4.46) — Commonwealth, Italian, German.
  - **Tank & Gun / Weapons Systems** (§4.47–4.49) — Commonwealth, Italian, German.
  - **Stacking Point Values** (§9.4) → `stacking-point-values.md` (encoded in code).
  - **Anti-Armor Fire CRT** (§14.6) → `crt-anti-armor.md` / `crt/anti-armor.ron`.
  - **Barrage Against Land Units CRT** (§12.6) → `crt-barrage.md` / `crt/barrage.ron`.
  - **Close Assault CRT** (§15.79) → `crt-close-assault.md` / `crt/close-assault.ron`.
  - **Morale Modifier Table** (§17.4) → `morale-modifier-table.md` /
    `crt/morale-modifier.ron`.
  - **Prisoners Captured** (§15.89) → `prisoners-captured-table.md` /
    `crt/prisoners.ron`.
  - **Initiative Ratings** (§7.2) → `initiative-ratings-chart.md` /
    `initiative-ratings.ron` (Commonwealth by Game-Turn; Axis situational).
- Dice-read convention: the combat CRTs use a straight two-dice read (11–66, 36
  outcomes) via `Rng::two_dice_raw`; the Close Assault table additionally reads the
  same roll as a 2–12 sum. Chart pages are often printed sideways — rotate (`sips -r`)
  before OCR/crop, and split dense tables into crops (JPEG-compress under the image
  reader's ~4 MB limit).
