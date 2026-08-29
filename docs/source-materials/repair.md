# Repair (Rules §22, §22.8 table)

Broken-down vehicles and destroyed tanks may be repaired — in the field or at Temporary/Major
Repair Facilities — in the Repair Phase of an Operations Stage (Phasing Player only). Transcribed
2026-07-15.

## Who / where (§22.1)
- Any Broken-down vehicle, and tanks destroyed in combat, that the Player controls (§22.11).
- No repair in an enemy-controlled hex (except a Major Facility), with no supplies, if towed this
  phase, or in Rainstorm/Sandstorm (except a Major Facility) — §22.13.
- Axis repairs German and Italian (and captured) vehicles separately (§22.14).

## §22.8 Broken-Down Vehicle Repair Table (by modified die 0–8)
| Die | Truck (field) | AC/R (field) | Tank/SPA/TD (field) | Temporary | Major |
|---|---|---|---|---|---|
| 0,1 | 2 | 1 | 25% | 50% | 75% |
| 2 | 1 | 0 | 10%* | 33% | 50% |
| 3 | 0 | 0 | 10%* | 25% | 50% |
| 4 | 0 | 0 | 10%* | 25% | 50% |
| 5 | 0 | 0 | 0% | 10%* | 33% |
| 6 | 0 | 0 | 0% | 10%* | 33% |
| 7 | na | na | na | 10%* | 25% |
| 8 | na | na | na | 10%* | 10% |

Truck: 1 = one Light/Medium point; 2 = one Heavy (or two Light/Medium). AC/R = 1 TOE SP of
armored car/recce. %: that percentage of the type repaired, rounding fractions **up**; `10%*` on
a single TOE Strength Point is treated as **0%**. DRMs: −1 with a Tank Delivery Squadron / Mobile
Tank Repair Squad; +1 facility in a Major City at Fort Level 2; +2 facility in a Major City at
Fort Level 0–1.

## §22.2 Field / §22.3 Facility
- Field (§22.2): any hex with a Broken-down vehicle stacked with a friendly unit. Trucks (§22.23)
  and AC/recce (§22.24) cost no supply; tanks/SP (§22.25) cost **1 Fuel per TOE attempted** (§22.26)
  and only those fueled may be repaired. Destroyed tanks cannot be field-repaired (tow to a facility).
- Facility (§22.3): Temporary (constructible, §24.8) or Major, using the facility columns.

## Implementation
- **Done:** `reference::logistics::RepairTable` (§22.8, all five columns) + `repair-table.ron` +
  loader. `field_repair_trucks(id, roll)` (§22.23) and `field_repair_tanks(id, roll)` (§22.25 —
  ⌈%⌉ of broken Tank-class TOE, 10%* single-TOE → 0), moving vehicles from `trucks_broken` /
  `weapons_broken` back into service.
- **Facility repair (§22.3, done):** `GameState.repair_facilities: HashMap<HexCoord,
  FacilityKind>` (Temporary/Major); `facility_repair(id, roll)` applies the §22.8 facility % (Temp
  or Major column, rounded up) to every broken Truck type and weapon-system group in the unit's
  facility hex.
- **Deferred:** §22.26 Fuel cost for tank field repair; AC/recce field repair (§22.24 — no
  Reconnaissance weapon-system data yet); loading facilities from scenarios (§60.31/§60.44 name
  them) + constructing Temporary facilities (§24.8); destroyed-tank repair (§20.2 replacement
  points); the §22.8 die-roll modifiers.
- **Implemented (§22.27 → §22.13d):** field repairs (trucks and tanks) are blocked when the unit's hex weather is Rainstorm or Sandstorm (`field_repair_blocked_by_weather`); Major Repair Facilities are exempt.
