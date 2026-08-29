# Breakdown (Rules §21, §21.38 table)

Vehicles accumulate Breakdown Points (BP) from the terrain/hexsides they move through; on
stopping, a die roll on the §21.38 table gives the % of that vehicle type that breaks down.
Transcribed 2026-07-15.

## Which & when
- **Subject (§21.11):** Truck Points, tank, armored-recce/car, and SP-gun TOE Strength Points.
  Motorcycles and inherent-transport artillery never break down.
- **BP accumulation (§21.21–23):** each hex entered / hexside crossed adds its Terrain-Effects-Chart
  Breakdown Point Value (already in `terrain.ron` as `breakdown`); all movement counts (incl.
  retreat/reaction); combat does not. Cumulative within an Operations Stage, across both players'
  portions; modified by weather.
- **Check (§21.24–27):** on ceasing movement, if BP > 3 and it has entered a *higher* BP column
  than at its last check, roll. BP keep accumulating within the stage.

## §21.38 Breakdown Table
Columns = Accumulated BP (0–3, 4–10, 11–20, 21–30, 31–40, 41–50, 51–60, 61–70, 71+). Rows =
% Breakdown (0/10/25/33/50/75). Cell = the sequential-read roll (§12.42, larger×10+smaller,
11–66) landing in that %. **BAR** shifts the column (negative = Left/fewer breakdowns; All Trucks
2L, Armored Car/ArmRecce 1L, SP Guns / Tanks per the Tank & Gun chart). **Hot weather** +1 column
Right; **Sandstorm** +1 Right if ≥50% of the BP were acquired in sandstorm sections; BAR + weather
are cumulative.

## Implementation
- **6c (done):** `reference::logistics::{BreakdownBand, BreakdownColumn, BreakdownTable}` +
  `data/reference/breakdown-table.ron` + loader. `BreakdownTable::percent(bp, shift, roll)` picks
  the BP column, applies `shift` (BAR + hot + sandstorm), and reads the roll band → % breakdown.
- **6d (done):** `Unit.breakdown_points` (tenths) accumulate each entered hex's terrain
  `breakdown` value during movement (mover + riders), reset per Operations Stage;
  `unit_bar(id)` (tank/SP/recce `breakdown_adjustment`, or −2 for truck-motorized infantry);
  `resolve_breakdown(id)` rolls the §21.38 table (with BAR + Hot/Sandstorm shifts) on ceasing
  movement, honoring §21.27 (>3 BP) and §21.26 (recheck only on a higher column via
  `breakdown_checked_col`), and breaks down that % of the unit's weapon TOE (or Truck Points).
  Wired into `move_counter`; no-op unless the §21.38 table is loaded.
- **Repair (§22.23, done for trucks):** broken-down **Truck Points** are now recoverable —
  breakdown moves them to `Unit.trucks_broken` instead of deleting them; `field_repair_trucks(id,
  roll)` restores them per §22.23 (a d6: 2 → one Light/Medium, 1 → one Heavy or two Light/Medium,
  no supply cost). Weapon-system (tank/SP/recce) breakdown is also held recoverable now (moved to
  `Unit.weapons_broken` rather than deleted); repairing it needs the §22.8 Broken-Down Vehicle
  Repair Table (chart) + §22.26 fuel, deferred.
- **Deferred:** hexside Breakdown Point Values; the Light-truck off-road +1 (§54.2‡); recoverable
  tank/SP breakdown + the §22.8 repair table + facility repairs (§22.3); armored-car/recce field
  repair (§22.24); per-vehicle-type checks within a mixed-composition unit.
