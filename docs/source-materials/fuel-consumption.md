# Fuel Consumption (Rules §49) — full Logistics Game

Digest of the fuel rules (transcribed 2026-07-14).

## Model
- **Fuel Point** ≈ 250 lb / 35 gal (§49.11).
- Fuel is consumed by **vehicles** (§49.13): every TOE Strength Point of a vehicle weapon
  system has a **fuel consumption rate**. Leg infantry (Organic units, no vehicles) burn no
  fuel; motor-cycles and towing are exempt. Fuel users include HQs with non-parenthesized TOE
  and all gun-class units.
- **Consumption (§49.13):** fuel = rate × ⌈CP/5⌉ per TOE Strength Point, for CP expended on
  **movement only** (not combat). Example: 1 TOE of tanks (rate 4) moving 12 CP burns 4 × 3 = 12.
- **Capacity (§49.14):** `fuel capacity = CPA × ⅕ × rate` per TOE SP — exactly enough to spend a
  full CPA on movement. A unit enters play with full tanks.
- **Presence (§49.15/16):** fuel must be in the same hex to be used. A unit draws its move's
  fuel, at the start of the move, from its own tanks and any Supply Dump in that hex (not from
  2nd/3rd-line convoy trucks without off-loading). No CP cost to "gas up".
- **Siphoning (§49.17, implemented):** `siphon_fuel(donor, receiver, amount)` transfers Fuel between co-located units up to the receiver's capacity, costing 3 CP to both; fuel is non-denominational (§49.19) so an enemy abandoned/broken-down vehicle may be the donor.
- **Non-denominational (§49.19):** either side can burn captured fuel.
- **Evaporation/spillage (§49.3):** during the Stores Expenditure Stage, all on-map fuel −6%
  (Commonwealth −9% Sept 1940–Aug 1941), +5% in hot weather (deferred — needs the Stores Stage).

## Implementation
Fuel rate is `WeaponsSystem.fuel_rate` (§4.47–49 charts), **already populated** in
`data/reference/weapons/*.ron`. `GameState`:
- `unit_fuel_rate_total(id)` — Σ over weapon groups of count × fuel_rate (0 for leg infantry).
- `unit_fuel_capacity(id)` = CPA × ⅕ × total rate; `refuel_to_capacity(id)` (units spawn full).
- `unit_move_fuel_cost(id, cp_tenths)` = total rate × ⌈CP/5⌉ (5 CP = 50 tenths).
- `consume_move_fuel(unit, from, cp)` — each mover/rider pays from its tank, then friendly
  Supply Dumps in the start hex; `move_counter` rejects a move (`CommandError::InsufficientFuel`)
  when the collective demand exceeds tanks + start-hex dump Fuel.

Implemented (§49.18): a unit's move fuel may be drawn from the fuel its own Truck Points carry as cargo (`truck_cargo.fuel`), after its tank and before start-hex dumps.

Deferred:
consuming its own cargo (§49.18, with Trucks), and explicit "gas up to full" as an Org action.
