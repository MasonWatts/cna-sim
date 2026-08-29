# Construction (§24)

Fortifications, minefields, air/repair facilities, roads, railroads, and supply dumps come into
being through construction (§24.0). It happens in the Construction Segment of the Organization
Phase; a project takes N Operations Stages (Construction Chart §24.17), completing at the start of
a later stage's Construction Segment. Constructing units may not expend CP that stage or it halts
(§24.12); required supplies must be present in the hex and are expended (§24.13); one project per
unit (§24.15); a unit may cease anytime (§24.16).

## Implemented
- **Roads (§24.5):** `build_road(builder, a, b)` — an Infantry Battalion of 3+ TOE (§24.53a) or an
  Engineer/HQ (§24.53b) in `a`/`b` converts an `UnfinishedRoad` hexside to `Road`, expending
  **2 Stores** from a dump in the builder's hex (§24.54). Modeled as immediate ("one road hex per
  Construction Stage"). `Map::remove_hexside` supports the unfinished→built conversion.

- **Fortifications (§24.4, implemented):** `begin_fortification(builders, hex, level)` — an
  Engineer/HQ **and** an Infantry Bn (3+ TOE) in the hex, 30 Stores expended, legal terrain
  (§24.44) and not in an enemy ZOC; completes after **3 clean Construction Segments**
  (`advance_construction`, called each Operations Stage from `begin_operations_stage`). §24.12: a
  builder expending CP halts the stage. Introduced the multi-stage `ConstructionProject` machine.

## Deferred
- **Railroads (§24.6, implemented):** `begin_railroad(builders, a, b)` — rail-construction units
  (NZRRC; modeled as "Construction (Road/RR)" units — the specific 10th/13th-only restriction is an
  OB-counter nuance) in the construction hex build an UnfinishedRailroad edge into Railroad. §24.67:
  must extend the railhead (an endpoint already touches built track — enforces no-skipping). §24.62:
  two companies complete in one stage, one takes two. §24.64: 1 Store expended. Destroyed track (§24.66, `HexsideKind::DestroyedRailroad`, inert) is rebuilt by `rebuild_railroad`
  — ANY Engineer/HQ/construction unit (§24.6), 1 Store, immediate (§24.63), allowed in an enemy ZOC
  (§24.45). `destroy_railroad` converts Railroad→DestroyedRailroad (the low-level effect, for air/artillery
  §24.66 & pipeline §52.25). Player **ground demolition** (§24.66) is `begin_railroad_demolition`:
  an Engineer/HQ/Infantry-3+TOE unit spends one clean Operations Stage in the hex (no CP, §24.12).
- **Minefields (§24.3, implemented):** `begin_minefield(builder, hex, dummy)` — an Engineer/HQ in
  the hex, Clear/Sand-Gravel/Rough terrain (§24.35), not in an enemy ZOC (§24.36), expending 15
  Stores + 15 Ammo (real, §24.33) or 3 Stores (dummy, §24.34); completes after one full Operations
  Stage (`Minefield` placed). `begin_minefield_removal` (§26.13/§23.22) clears it over one clean
  stage. Entering an **enemy** minefield costs the mover's full CPA on top of terrain (§26.21/22), or only +4 CP if it is an Engineer/HQ (§26.24); a friendly minefield is free (§26.12). Dummy minefields are revealed when an enemy enters (§26.15/§26.23), staying present (same-phase movers still pay) until removed at the next Operations Stage (`revealed_minefields`). A non-engineer vehicle entering an enemy minefield also risks a casualty (§26.25): a d6 of 5-6 destroys one TOE/Truck point (`resolve_mine_casualty`).
- **Repair facilities (§24.83, implemented):** `begin_temporary_facility(builder, hex)` — an
  Engineer/HQ in a Major City or Village hex (§24.85, max 2 temporary), 250 Stores + 150 Fuel
  expended, 3 Construction Segments → a Temporary Repair Facility. Only temporary facilities are
  constructible (§24.81); major ones pre-exist. Deferred: dismantle (§24.86), rebuild in a reduced
  city (§24.84).
- **Air facilities (§24.7)** — Air Game only (deferred with the Air Game). **Supply-dump
  construction** deferred.
- The general **multi-stage project state machine** (progress across OpStages, the §24.12 CP-halt
  rule, the §24.17 Construction Chart durations) — roads sidestep it by being single-stage.
