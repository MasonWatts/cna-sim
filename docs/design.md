# Campaign for North Africa — Digital Simulation
## Project Design Document

**Status:** Draft 3 — Phase 1 land engine complete; air, naval, and full-logistics
systems substantially built (rules + reference tiers essentially complete, command/
resolution layer partial). See §8 for the detailed status.
**Date:** 2026-08-29
**Scope of this document:** Project goals, guiding principles, phased roadmap, key
decisions, and technology direction. Detailed data schemas/types are deliberately
*out of scope here* and will follow as a separate document once this is settled.

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
anticipating it is cheap.

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
| D14 | OB auto-extraction fidelity | **Low-confidence; revisit for faithfulness.** Ship the OCR→OB pipeline with conservative, chart-validated ID inference (sibling-in-formation + dominant type-mode), everything else flagged (`// TODO id?` / `// id inferred`). | The §4.46 single-letter ID glyphs defeat OCR at any DPI, so ~half are inferred; spot-checks found real errors (case, Cyrillic garbage, cross-type leaks — since fixed). Inferred/flagged values are provisional and must be verified against the physical charts in a later faithfulness pass. A Rust test asserts every OB id is a valid characteristics key, that parents resolve within the formation, and that no unit outranks its parent HQ in org size. **`org_size` is data-driven, not transcribed:** for combat units it derives from the id's characteristics `unit_type` suffix (`Bn/Coy/Bde-Equivalent`); HQs (detected by name, since some HQ counters carry a combat id) size by hierarchy (top = formation echelon, sub-HQ = StandardBrigade). The ultimate source is the counter's §9.2 size symbol, still unverified — so wrong inferred ids can still yield wrong sizes (part of this revisit). |
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
- Detailed naval fleet/coastal shipping/chariot raids (§30), desert raiders &
  commandos (§27), Rommel special rules (§31).
- Full construction/engineers/minefields/fortifications (§§23–26) — partial first.
- Detailed prisoners (§28), repair (§22), breakdown (§21) — simplified first.
- Fog-of-war enforcement (§3.5).

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

The engine **builds clean and 269 tests pass** across the workspace (unit tests in each
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
are now consulted by the `PlanProduction` command (§8.7), though the §20.3 Conversion chart and
the truck/CW-infantry roll tables are not yet driven by game logic.

### 8.2 Turn driver / Sequence of Play — implemented
`GameState`'s turn driver (`advance_phase`) walks the full per-Game-Turn sequence, not just
the shared setup + A/B blocks:
- **Initiative Determination** (§7): d6 + Initiative Rating per side, ties re-roll, GT1
  optionally predetermined; the Axis rating is state-derived (Rommel-on-map / any German).
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
including the one-full-turn objective-hold check. (The scenario-level §60.8 victory is not
separately modeled.)

### 8.3 Command boundary — ~47 commands, phase-gated
`GameState::apply` covers land movement/combat, air, naval, logistics, and construction. Land:
`MoveStep`/`FormationMove`, `Barrage`/`AntiArmor`/`CloseAssault`/`Retreat`, `React`,
`MoveOffMap`, `DesignateReserve`, `RailMove`/`RailMoveSupply`, `PoisonWell`/`SweetenWell`.
Logistics/construction (Organization phase): `LoadTrucks`/`UnloadTrucks`, `SiphonFuel`/
`RefuelUnit`/`RearmUnit`, `BuildPipeline`/`BuildFortification`/`BuildRailroad`/`DemolishRailroad`/
`BuildRoad`, `LayMinefield`/`ClearMinefield`, `WithdrawUnit`/`ReturnUnit`/`AbsorbReplacement`,
`PlanProduction` (§20.66), `BlockHarbor`/`UnblockHarbor`, `Repair`. Naval: `PlanConvoy`/`BombConvoy`/
`DeliverConvoy`/`ReconConvoyLane`, `AssignShip`/`RepairShip`, `ArtilleryBombardShip`/
`SanGiorgioAttackShip`/`BombShip`/`OffShoreBombard`, `LayHarborMine`/`RemoveHarborMine`. Air:
`AssignSquadronPlanes`, `ReadySquadron`/`ArmSquadron`/`RefitSquadron`, `FlySquadronMission`/
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
(as repeated single-hex steps), ZOC-to-ZOC prohibition and leave-ZOC surcharge (§8.14/§8.15/
§10.24), **reaction** (§8.5 with §8.54 pinning), **retreat** (§15.82), **breakdown** (§21),
minefield entry (§26.2), and combat ammo/fuel/water gating. Combat deferrals: fortification/
minefield *terrain shifts* (those features are not yet map-modeled), §15.35 two-terrain-hexside
exception, multi-hex defenders, detailed anti-armor line-of-fire, and defensive-assault ammo
(§50.14).

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
or target (§37.22), and deriving the target's **flak** (§46) from the enemy AA units in the target
hex (`hex_flak_points`) rather than a caller-supplied number. **Still not computed from the map:**
CAP/scramble **interception** (the interceptor flight is still an explicit input, pending CAP
designation state, §40.2), multi-hex flight **path/ZOC routing**, and fuel gating at execution;
plus **scramble**, **strafing**, land-unit air recon, transport/airdrop (§42.3/42.4), a real pilot
roster with ace progression, and Med-basing/Malta-commitment limit enforcement (§43/§44.41).

### 8.6 Naval / convoy — abstract model implemented
Ports (§55: efficiency, blocking §55.22, mining §55.27, San Giorgio §30.17), the Commonwealth
Fleet (§30: ships as gun-rating strength/damage, seaworthiness, repair cycle, at-sea/rest,
bombardment), and the **convoy pipeline** (plan → bomb → deliver, one/lane/turn §56.12, tonnage
from the Convoy Level Chart, survivors converted to supply points) all work end-to-end and are
where reinforcements enter (Naval Convoy Arrival Phase). Consistent with the game's abstraction,
there is **no sea-movement/interdiction/submarine model** — convoys are planned tonnages
attacked only by air; the §42.53 size-class reveal is a rule helper not yet used by recon.

### 8.7 Logistics (full game, D15) — increments 1–8 built; production (§20) is the gap
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
repair §22); **(7)** supply transport/dumps + Commonwealth rail supply §54.3 and the two-leg
strategic-supply trace for victory §64.71/72; **(8)** naval convoys §56 (see §8.6). **(9)
Production §20 — Replacement-Point planning now wired:** the `PlanProduction` command validates a
requested item/count against the nationality's §20.66/§20.78C chart (lifetime `total` and the
per-period maximum, via a rolling Game-Turn window keyed to each item's GameTurn/Two-Weeks/Month
period), records the required Axis convoy tonnage, and — mapping the item to a Replacement class —
schedules the points to arrive after the §20.21 delay (Axis 2 / CW 4 Game-Turns) and begin
Training, feeding the existing plan→arrive→train→absorb lifecycle. **Still open in production:**
the item→class map flags the SP-gun/tankette calls as *inferred* (cf. D14) pending physical-chart
verification; the §20.3 Replacement Conversion cost chart is not yet applied on absorption; the
recorded convoy tonnage does not yet consume Axis convoy capacity (D15 step 8); and the truck
(§20.78A) / CW-infantry (§20.78B) roll tables remain reference-only.

### 8.8 Scenario, map & OB
The Scenario-1 order of battle and the full map are in: all five map sections (A–E) digitized,
load-tested, and loaded together (~7,100 real hexes). OB formations deploy from the §4.44 charts
via scenario `placements`, including **dispersed placement** (per-unit hex overrides, e.g. 7th
Armoured's §60.41 covering screen with the main body at D3612). Both reinforcement paths work:
**formation-internal arrivals** (later-arriving units of an on-map formation attach to their
on-map parent at their scheduled game_turn/stage) and **whole-formation entry** (a formation not
on-map at start enters at a resolved entry hex or off-map node), both resolving at the Naval
Convoy Arrival Phase. Loaded OB: Italian Catanzaro/Marmarica/Cirene/Ariete, Commonwealth 4th
Indian/7th Armoured (ids/weapons/parents/org-size all test-validated). Remaining unit-data
promotion (Italian border force, CW rear) is tracked separately (§4.44 OB pipeline, D14).

## 9. Source-Material Transcription Practice

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
