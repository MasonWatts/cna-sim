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

The engine **builds clean and 326 tests pass** across the workspace (unit tests in each
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
including the one-full-turn objective-hold check. (The scenario-level §60.8 victory is not
separately modeled.)

### 8.3 Command boundary — ~90 commands, phase-gated
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
(as repeated single-hex steps), ZOC-to-ZOC prohibition and leave-ZOC surcharge (§8.14/§8.15/
§10.24), **reaction** (§8.5 with §8.54 pinning), **retreat** (§15.82), **breakdown** (§21), and
combat ammo/fuel/water gating. **Fortifications & minefields are modeled:** a per-hex Fortification
Level (`fort_level` — placed forts plus the §25.12 static major-city/capital levels) applies its
column shift in anti-armor (§14.33 best-of), barrage (§12.34 best-of) and close assault (§15.35
cumulative), is raised by `BuildFortification` (§24.4) and reduced by fortification bombing (§41.37);
minefields (`LayMinefield`/`ClearMinefield`, §24.3) sit on the map with real/dummy status, dummy
reveal, and §26.2 entry casualty checks. §50.14 **defensive**-assault Ammunition is now consumed
by the melee defenders too (not just the attacker), and §9.35 retreat enforces the destination
Stacking Limit (a stack splits into different hexes by retreating each unit on its own path).
Combat deferrals: §15.35 two-terrain-hexside exception and multi-hex defenders.

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
rebuildable §24.53; a Railroad hex's edges become DestroyedRailroad, rebuildable §24.63). Only the
static §25.12 major-city Fortification Level (no damage tracker) remains unmodelled.
The `Strafe` command (§40.5–40.8) flies a map-validated strafing mission and resolves it on the
Strafing Table with the §40.8 column shift, applying the effect for **supply dumps** (§40.64, r×10%
of each commodity destroyed), **water pipelines** (§40.67, destroyed on r≥2), **infantry** (§40.6,
r TOE Strength Points eliminated from an enemy infantry unit in the hex), and **grounded aircraft**
(§40.6, the result is the number of d6 rolled — their sum is planes destroyed from an enemy SGSU in
the hex), and **first-line trucks** (§40.62, r Truck Points eliminated from an enemy unit's attached
trucks with the carried cargo lost in proportion), and **tanks** (§40.65 — armour-capable planes
only, i.e. the Hurricane Mk.IID `DayStrafeArmor`; the result is the number of d6 rolled and their
sum is the minimum Armour-Protection Points removed, taken as TOE weapon points from an enemy
armoured unit). The remaining strafe targets — trucks in convoy (abstract lane tonnages, not
hex-placed) and ports (§40.66 folds into a bombing raid) — are validated-but-rejected pending those
models. **Air transport & airdrop (§42.3/§42.4):** `AirdropSupplies` drops Ammo or Stores from a
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
or consume the CAP (partial abort, §39.34, is supported — see below). Land-unit air
recon waits on fog-of-war. **Malta strategic-air commitment (§43/§44.41) is modeled:** each Axis
Malta Availability Level (I–IV) is limited to the scenario's Game-Turn count (the §44.41 chart),
tracked in `malta_level_used` and gated at `DetermineMaltaAvailability` (Level I unlimited, §44.25);
SGSUs carry a §43.0 Mediterranean base region (`med_base`: Italy/Sicily or Crete, else on-map
Africa), and a raid's **absolute plane count** is computed from the in-play Italy/Sicily bombers
(§44.25 map-% + strategic-%). `TransferSquadron` (§43.14) moves a squadron between the Boxes and
on-map Africa with one Operations Stage of transit (`transfer_remaining`; a squadron in transit may
not fly), refusing any move that would drop German strategic bombers below the §43.11 75%-Med
requirement — date-aware: before Game-Turn 35 it enforces §43.12 (≥75% of all German bombers in
Italy/Sicily), and from GT35 §43.13 (≥50% of strategic bombers in Crete). Still open — all governing
flight *from* the Boxes, which awaits Box-based flight execution: the §43.23 Crete→Suez timing and
§43.24/§43.25 Box-bomber CAP coordination; §43.21 auto-supply is only implicit (a Box-based SGSU has
no dump, so upkeep is skipped). (Both airborne formations are in the loaded OBs with their
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
with requirement kinds collapsed onto the four pool classes — using a **provisional** unit→row
classifier (`conversion_row_for_unit`, flagged cf. D14; special-infantry and armored-recce/car
distinctions default to the generic row, with Commando/Bersaglieri/Paratroop caught by name). With
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
SP-gun/tankette Tank calls were owner-confirmed 2026-08-29; the §20.3 absorb classifier's
special-infantry/armored-car distinctions remain provisional.)

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
from the enemy's observation). Flagged simplifications (D): §27.12 "trained to
max Morale" precondition; per-hex fuel accounting (§27.38); all-replacement convoy auto-elimination
(§27.58); facility/plane side-tagging (§27.53/54); and §27.59 range as hex distance ≤4 (proxy for
"4 Movement CPs"). The §27.93 SAS Raid Table (0/10/25/25/33/50 by d6) is owner-confirmed (2026-08-30).

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
defender's narration / the deferred §28 POW logistics.

### 8.12 Reorganization (§19)
Units carry both an `assigned_parent` (organizational membership, occupies TOE space even when not
co-located) and an `attached_parent` (physically combined onto the parent's counter, same hex),
set at scenario setup (§19.1). The reorganization commands act on them: **`AttachUnit`** (§19.41/42
— 1 CP each for an assigned unit any time, 2 CP each for a non-assigned unit in the Organization
Phase within the §19.5 limit), **`DetachUnit`** (§19.43/44 — 1 CP each, Organization/Movement),
**`ReassignUnit`** (§19.21/28 — a currently-attached, not-assigned-elsewhere unit, Organization
Phase, no CP). The §19.5 Maximum Attachment Chart is transcribed (`maximum-attachment-19.5.md`) and
its numeric caps enforced by `nonassigned_attachment_limit` (keyed by parent org-size/nationality/
Game-Turn). Documented simplifications: the engine does not track a formation's arm, so §19.5's
type-split columns use the more permissive value, and the type sub-constraints, German OR-alternatives,
Matruh/Selby specials, and the finer §19.3 assignment maxima (Formation Organization Chart, not
located) are not yet enforced; CW battalion-shuffle (§19.26), historical assignment shifts
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
(§28.23–25): an unguarded stack escapes; escapees within eight hexes of a friendly unit rejoin and
return as Infantry Replacement Points a month later (§28.24), and are recaptured if an enemy enters
their hex (§28.25); the far-escapee autonomous movement/water-attrition is a documented simplification
(owner decision 2026-08-31). Captured equipment is repaired via `RepairCapturedEquipment` (§28.3).
Documented simplifications: the §28.17 moving-guard ratio is read as 1-per-5 (the 5-based POW system;
the printed digit was ambiguous); ranges use hex distance; the §28.12 free 3-hex relocation and the
40-per-hex camp cap fold into ordinary `MovePrisoners`; instantiating repaired captured equipment as a
live unit is a follow-on.

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
