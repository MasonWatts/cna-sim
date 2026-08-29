# Abstract Logistics — Supply (Rules §32)

> **SUPERSEDED (D15).** The §32 abstract supply mode has been dropped in favour of the full
> Logistics Game (Fuel §49 / Ammo §50 / Stores §51 / Water §52; see design.md §8.7). This page is
> kept as a historical transcription of the §32 rules; the "Implementation status" section below
> describes code that has since been removed or replaced — in particular the §32.16
> `unit_in_supply` trace no longer exists (replaced by the §64.73 `unit_supply_adequate` test).

Human-readable digest of the Land-Game supply rules (played without the full Air/Logistics
Games). Transcribed from the rulebook (2026-07-14).

## §32.1 Supply Units
- A **Supply Unit** = a Supply Dump counter. No combat value; captured (supplies used) if an
  enemy combat unit enters its hex (§32.13).
- Stacking value 0, but **limits (§32.14):** ≤5 in a Major City, ≤3 in any other hex; the
  Tripoli/Tunisia boxes hold unlimited Supply Units.
- Each holds **≤40 Ammo + ≤60 Fuel** (§32.15); removed when both hit 0. Supplies may be
  rearranged among Supply Units in the same hex during the Organization Phase.
- **Dummies (§32.18):** one per three real Supply Units; removed on discovery.

## §32.16 In supply (the supply-line trace)
A combat unit draws supply if a friendly Supply Unit is within **½ its CPA**, measured as
**medium-truck movement** — *unless* the unit is **non-motorized infantry**, traced as
infantry movement. The line may **not** pass through impassable terrain or an enemy ZOC that
is unoccupied by a friendly unit. (Examples: non-mot infantry ≤5 CP; truck-borne heavy weapons
≤10 CP; recce ≤~23 CP.)

## §32.2 Expenditures
- **Ammo (§32.21–22):** per battalion-equivalent, at the instant of combat. A non-phasing
  Bn-eq spends 1 Ammo to Assault, 2 to Barrage; a **phasing** unit spends double. (A 5-SP
  division spends per its component counters, not as one.)
- **Fuel (§32.23–24):** per Operations Stage of land movement. First land move of the stage
  spends the unit's fuel; exceeding CPA spends more. **Tank Bn-eq = 2**, tank Coy = 1,
  Gun-class & Recce Bn-eq (not camel) = 1, Motorization Points transporting infantry/guns/a
  Supply Unit = 1. Company-eqs and other motorization uses cost no fuel.
- **Patrolling (§32.25)** and the Commonwealth Fleet cost no supply.

## Implementation status
- **Implemented (Increment 1):** Supply Units as state (`SupplyDump`: location, side, ammo,
  fuel, dummy) and the **§32.16 in-supply trace** (`GameState::unit_in_supply`) — a cost-
  bounded Dijkstra to ½ CPA by truck / infantry, blocked by impassable terrain and unoccupied
  enemy ZOC. Off-map (Tripoli-box) units are always in supply (§8.88).
- **Implemented (Increment 2):** §32.23 **Fuel consumption** — a moving counter and its
  attached members each draw their Fuel (unit_fuel_cost: tank Bn 2 / tank Coy 1 / other Bn 1 /
  other Coy 0) once per Operations Stage from the nearest in-range Supply Unit (via the §32.16
  trace); a failed draw doesn't block movement (§32.11).
- **Implemented (Increment 3):** §32.21/22 **Ammo consumption** — at the instant of combat,
  each participating Bn-eq draws Ammo (unit_ammo_cost: 1 Assault / 2 Barrage, doubled if
  phasing) from the nearest in-range Supply Unit via draw_ammo. Barrage charges the barragers;
  Close Assault charges attackers (phasing → double) and every defender (non-phasing). Phasing
  = the unit's side is the active player. A failed draw does not prevent combat (§32.11).
- **Implemented (Increment 4):** **initial dump placement** — a scenario `supply_dumps` list
  (SupplyDumpSpec: side, at [hex or off-map node], ammo, fuel, count, dummy) resolved via
  `resolve_entry`. Scenario 1 places all 46 §60.92 Supply Units. See
  `scenario1-initial-supply.md`.
- **Next:** the strategic §64.71/§64.72 supply-line trace (dump → convoy → Tobruk/Tripoli,
  90/60 MP) to close the campaign auto-wins and require *supplied* occupation (§64.73).
