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
- **First-line cargo (done):** `Unit.truck_cargo: Commodities`; `load_trucks`/`unload_trucks` draw
  from / deposit into a co-located Supply Dump, closing the rear→front haul. The
  **one-commodity-per-Truck-Point** rule is enforced — the fleet's per-commodity shares (loaded ÷
  all-truck capacity) must sum to ≤ 1.
- **§24.9 dumps: loose vs constructed (corrected 2026-09-18):** `SupplyDump.constructed: bool`. Off-loading
  (`unload_trucks`/`unload_convoy`) drops **loose supplies** (`constructed: false`) — usable/consumable in
  the hex but **a truck "in convoy" may not re-load them** (`load_convoy` filters `constructed`). A real,
  loadable dump must be **constructed** by a co-located TOE Strength Point via the new
  `ConstructSupplyDump { unit, hex, dummy }` command (Organization Phase): **3 CP + 20 Store Points** real,
  **3 CP** dummy; it promotes the hex's loose piles to `constructed`. Enumerated in `legal_reorg_commands`.
- **§6.3/§53.24 load cost (corrected 2026-09-18):** loading/off-loading during the Organization Phase is
  **free** (0 CP) — the earlier 2-CP charge on `LoadTrucks`/`UnloadTrucks` was wrong and was removed.
  Tests: `supply_dump_construction_24_9`, updated `truck_system_additions`/`truck_deferred_items`.
- **Free (2nd/3rd-line) convoys (done):** `FormTruckConvoy` (detach + carry cargo), `MoveTruckConvoy`
  (Truck Convoy phase, supply-CPA cap, §10.29 ZOC), and **`LoadConvoy`/`UnloadConvoy` (done
  2026-09-17, §53.14)** load/off-load a free convoy's `cargo` at a co-located dump — completing the
  §53.14 port→dump→unit haul. Convoys also haul Replacement-Point pools (§20.44/46).
- **Fuel & water (done):** first-line trucks burn fuel on the parent's move (§49.18); **§52.42 (done
  2026-09-17):** each Truck Point that moved owes 1 Water Point (`unit_water_requirement`).
- **§49.18 convoy self-fuel (done 2026-09-17):** a free convoy hauling an RP burns its **own** carried
  fuel first, drawing only the shortfall from the origin-hex dump (`cmd_move_replacement`:
  `from_convoy = min(cargo.fuel, cost)`, remainder `from_dump`); `TruckConvoy.cpa_used`/`hex` advance
  with the pool it carries.
- **§20.44 RP↔convoy binding (done 2026-09-17):** the stored `TruckConvoy.carries_replacements` bool was
  removed in favour of `ReplacementPoints.carried_by: Option<usize>` (a stable convoy index — convoys are
  only ever pushed, never removed). §27.58 raid effects are **derived** at raid time via
  `convoy_carries_replacements()`, so they can never go stale. New commands `LoadReplacement{convoy,pool}` /
  `UnloadReplacement{pool}` (§20.44) pick up / drop a pool; `cmd_move_replacement` honours an existing
  binding, else auto-loads a co-located convoy. An Infantry pool is stranded until loaded; delivery/absorb
  removes the pool and thus the binding.
- **Viewer as counters (done 2026-09-17):** free convoys and RP pools render as **tiles in the per-hex
  stack** (like combat units), not corner glyphs — `HexMap.tsx` groups `Tile = unit|convoy|rp` by hex
  (rank unit > convoy > rp), drawing the top counter + `+N` badge + aggregated tooltip. A hauled Infantry
  pool co-locates with its convoy so it shows as a subordinate; a self-moving or unloaded pool stands alone.
  `TruckConvoyView` gained `id`, `ReplacementView` gained `carried_by` (ts-rs). Demo (`record_rp_demo`):
  depot dump at (0,1) fuel 30; the pool trains unbound (steps 1-4), is loaded at step 5 (`LoadReplacement`
  + `LoadConvoy`), then hauled forward (6-9) with the convoy's fuel draining 30→25→20→15→10.
- **Breakdown (done 2026-09-17):** a moving free convoy accrues terrain Breakdown Points and checks
  on the §21.38 table with the truck **BAR 2L** (−2 cols, +1 hot/+1 sandstorm), losing that % of its
  Truck Points (`resolve_convoy_breakdown`); §53.22 over-CPA is prevented by the hard CPA cap.
- **Motorization (done):** §6.17 leg-infantry-on-trucks; **§8.93 (done 2026-09-17)** inherently-
  motorized "+" units (`Cpa.motorized_capable`) are motorized with no assigned trucks; **§6.17 gun/AA
  (done 2026-09-17)** gun/anti-tank & AA units whose trucks can lift them assume the truck gun-CPA
  (`unit_motorized_cpa`).
- **Finishing pass (done 2026-09-17):** **§29.35** hot weather doubles water (`unit_water_requirement`);
  **§22.13** free-convoy Truck Points break down into `TruckConvoy.trucks_broken` and are recovered by
  `RepairConvoyTrucks` (`repair_convoy_trucks`) rather than lost; **§53.25** no leapfrogging — supplies a
  convoy off-loads during a Truck Convoy phase are locked (`game.leapfrog_locked`) against re-loading by
  another convoy that phase. Tests `truck_deferred_items`. Minor still-open: §21.6 *towing* of broken
  convoy trucks by another unit (field repair covers recovery); §53.25 is modeled as a same-phase drop
  lock rather than the literal "supplies inherit the trucks' spent CP" accounting.
