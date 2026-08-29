# Water (Rules §52) — full Logistics Game

Water comes only from wells (in Major Cities, villages, birs) and oases; most are near the
coast. Wells can be depleted (by use) or poisoned, and replenished by rain or sweetened.
Transcribed 2026-07-14.

## Sources (§52.1, §52.3)
- Wells exist automatically in **Major Cities, villages, birs, and oases** (§52.11); the
  Tripoli/Tunisia boxes have unlimited wells. No other hex has water.
- **Major Cities and Oases: unlimited, never deplete/poison** (§52.13). Oases are also natural
  Stores dumps and keep occupying units fully supplied all game (§52.3).
- **Villages (Town) and Birs**: draw by die roll on the §52.7 table; may deplete/poison.

## §52.7 Water Availability Table
| Die | 1 | 2 | 3 | 4 | 5 | 6 |
|---|---|---|---|---|---|---|
| **Town** | 100 | 150 | 200 | 300 | 350* | 500* |
| **Bir** | 50 | 100 | 150 | 200* | 300* | 400* |

Number = Water Points drawn. `*` → roll one die again secretly; a '1' Depletes that Town/Bir.
Drawing costs 1 CP (§52.13); Cities/Oases need no roll. Depleted wells are replenished by a
rainstorm on that map section (§52.15). Villages/birs (not cities/oases) may be poisoned
(1 CP, die '1' succeeds) and sweetened (5 CP, die 1–3) — §52.16/17.

## Usage rates (§52.4)
- **§52.41** Each infantry battalion/company needs **1 Water Point per OpStage**, regardless of TOE.
- **§52.42** Each Vehicle/Truck TOE Strength Point needs **1 Water Point per OpStage — only if it
  used any CPA** that stage.
- **§52.43** Hot weather (§29.3) adds water usage. **§52.44** stored water evaporates like fuel (§49.3).

## Effects of lack (§52.5)
- **§52.51** Vehicles without water may not move or assault offensively; defend at half raw strength.
- **§52.52** Infantry without water may not exceed CPA or assault offensively; defend at half.
- **§52.53** Each consecutive OpStage after the first without water, an infantry unit loses 1 TOE Point.

## Implementation status
- **Implemented (5a):** `reference::logistics::WaterAvailabilityTable` (§52.7) + data + loader;
  `WaterSource { Town, Bir }`; `table.draw(source, die) -> (points, depletion_check)`.
  `GameState::unit_water_requirement(id, acted)` per §52.41/42.
- **Deferred:** per-OpStage consumption wiring; wells/birs/oases/pipelines as map features and
  the draw/depletion/poisoning/sweetening mechanics (§52.1–3); §52.5 lack-of-water effects and
  §52.53 attrition;  water evaporation (§52.44); the
  Italian Pasta Rule (§52.6).

## §52.2 Water Pipelines (implemented)
- §52.21: pipelines extend only from a well in a **major city** (not oases, §52.3).
- §52.22: any length; the CW may treat any operating Railroad hex as a pipeline (Axis may not use
  the defunct Barce–Benghazi RR). **Implemented:** `hex_is_railroad(hex)` (touches a built
  `Railroad` hexside; `UnfinishedRailroad` excluded per §24.67) + `hex_water_source_for(hex, side)`
  returns `Unlimited` for the Commonwealth on a railroad hex (§52.23/§54.33). The CW may also build
  pipelines outward FROM a railroad hex: `build_pipeline` accepts a rail-hex origin for the CW, and
  `pipeline_connected_via(hex, allow_rail=true)` anchors CW pipelines on rail — rail-anchored
  pipelines are CW-only water sources.
- §52.23: a connected pipeline hex is a water source **like a major city** — unlimited, cannot be
  poisoned or depleted; it can be destroyed.
- §52.24: built by an Engineer battalion or (CW) HQ; 1 Construction Phase + **10 Stores** per hex;
  only **one hex per Operations Stage per side** (`pipeline_built_this_stage`, cleared in `begin_operations_stage`).
- §52.25: destroyed by desert raid / enemy occupation / strafing (RR pipelines die with the RR).
  When severed, water is drawn only from the farthest still-connected hex.

**Engine:** `GameState.pipelines: HashSet<HexCoord>`; `hex_water_source` returns `Unlimited` for a
pipeline hex when `pipeline_connected` (BFS over contiguous pipeline hexes back to a major-city
neighbor) holds; `build_pipeline(builder, hex)` (engineer/HQ in-hex, adjoins city or existing
pipeline, expends 10 Stores from a dump in the hex); `destroy_pipeline(hex)`.


## §49.3 / §52.44 / §29.34 Evaporation & Spillage (implemented)
Stored Fuel and Water in dumps evaporate (Ammo/Stores and well/pipeline water are exempt):
- **Base 6%/Game-Turn** at the Stores Expenditure Stage (`evaporate_base_supplies`).
- **Commonwealth 9%/GT** from Sept 1940 through the **last GT of August 1941 = GT 48** (§49.3,
  poorly-built British containers before the copied "jerry can"), reverting to 6% at GT 49
  (`base_evaporation_rate`). The GT-48 boundary follows the fixed campaign calendar of 4 one-week
  turns per month: Sept 1940 = GT 1–4 … Aug 1941 = GT 45–48.
- **Extra 5%** in hot weather, flat for both sides, taken at Weather Determination (§29.34,
  `evaporate_supplies(5)`).

## §52.6 The Italian Pasta Rule (implemented)
Each Italian battalion must receive an extra **1 Water Point** ("Pasta Point") when Stores are
distributed (pasta needs water to cook). Modeled by `distribute_pasta()` (called from
`expend_stores`): it consumes 1 Water from the battalion's own load or a dump in its hex.
- A battalion that does **not** get its Point is flagged in `no_pasta` and may not voluntarily
  exceed its CPA that turn (move blocked via `CommandError::NoPasta`).
- If such a battalion's cohesion is **≤ −10** it immediately becomes Disorganized (cohesion set to
  −26, the disorganized threshold), its original cohesion saved and restored the moment it next
  receives its Pasta Point.
