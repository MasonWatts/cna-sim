# Trucks (Rules §53, §54.2 chart) — full Logistics Game

One Truck Point ≈ ten actual trucks (Heavy ≈ 3-ton+, Medium ≈ 30 cwt, Light lighter). Trucks
carry supplies or motorize infantry-type units; they consume fuel + water and suffer breakdown.
Transcribed 2026-07-15.

## §54.2 Truck Characteristics Chart
| Type | CPA Inf | CPA Guns | CPA Supplies\* | TOE Inf | TOE Arty | TOE AA | Ammo | Fuel | Stores | Water | Fuel Cap | FCF | BAR |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Light | 25 | na | 40 | ½† | na | 1 | 2 | 50 | 6 | 40 | 8 | 1 | 2L‡ |
| Medium | 20 | 15 | 30 | 1 | 1 | 2 | 4 | 120 | 15 | 100 | 6 | 1 | 2L |
| Heavy | 20 | 15 | 30 | 2 | 1 | 4 | 8 | 250 | 30 | 200 | 6 | 1 | 2L |

- **CPA** = the Truck Point's CPA when transporting that item (Supplies\* only for convoys — matches
  the §53.22 convoy CPAs: Light 40, Medium/Heavy 30). `na` = not allowed.
- **TOE Strength Points** = max TOE SP of that type one Truck Point carries. †Two Light Truck Points
  are needed per Infantry TOE SP (½ each). Light cannot carry Artillery TOE or guns.
- **Supply Points** = Points of each commodity one Truck Point carries.
- **Fuel Capacity** = the truck's own tank; **FCF** = fuel consumption factor (1). **BAR** =
  Breakdown Adjustment Rating, column shifts left ("2L" → −2). ‡Light trucks off-road gain +1
  breakdown point beyond the terrain's.

## §53 formations & rules (summary)
- **First-line** trucks: attached to a parent combat unit, no counter; carry that unit's men/supplies;
  attach/detach only in the Organization Phase (§53.11, §8.97). Use basic CPA (20/25), may exceed like the parent.
- **Second/Third-line** trucks: convoy counters (units↔dumps, ports↔dumps); move only in the Truck
  Convoy Movement Segment at extended CPA (Heavy/Medium 30, Light 40) which they may **never** exceed —
  doing so = breakdown (§53.22). §53.14 recommends a three-tier haul (port→dump→unit).
- **§6.17/§8.9 motorization:** leg infantry (basic CPA ≤ 10) or 0+ AA carried by trucks assumes the
  truck's CPA. `+` on the Unit Chart = inherently motorized (§8.93).
- Load/offload costs CP (§53.24 / §6.3); trucks consume fuel (§49.18) and water (§52.42) moving.

## Implementation
- **6a (done):** `reference::logistics::{TruckKind, TruckCharacteristics, TruckChart}` +
  `data/reference/truck-characteristics.ron` + loader; `Unit.trucks: TruckPoints { light, medium,
  heavy }` (first-line). `TruckChart::inf_points_needed(kind, toe)`;
  `GameState::unit_truck_inf_capacity(id)` and `unit_motorized_cpa(id)` (§6.17 — leg infantry ≤ CPA 10
  fully carried assumes the lowest present truck Inf-CPA).
- **6b (done):** attached first-line Truck Points now consume fuel when the unit moves
  (§49.12/18) — `unit_fuel_rate_total` adds each truck point's fuel_consumption_factor, so a
  motorized infantry unit (no weapon systems) burns fuel via its trucks.
- **Cargo (done):** `Unit.truck_cargo: Commodities`; `load_trucks(id, commodity, amount)` draws
  from friendly Supply Dumps in the hex up to the trucks' cargo capacity, and
  `unload_trucks(id, commodity, amount)` deposits into (or creates) a dump in the hex — closing
  the haul loop (load rear → move → unload front). Simplification: each commodity is capped
  independently (per-Truck-Point one-commodity assignment not enforced); §53.24 CP cost deferred.
- **Deferred:** loading truck points from scenarios [done separately]; second/third-line
  second/third-line convoy movement (§53.22, Truck Convoy phase) + truck fuel/water consumption;
  breakdown (§21, BAR); inherently-motorized `+` units; gun/AA motorization CPA.
