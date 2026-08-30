# Reinforcements & Replacements (Rules §20)

Reinforcements are whole new units (handled by the scenario/OB arrival driver). Replacement
Points rebuild existing units' losses. Transcribed 2026-07-15.

## Replacement Points (§20.2)
- Four classes: **Infantry, Gun, Tank, Recce**. Enter via the Axis Replacement Pool (§20.6),
  the Commonwealth Production System (§20.7), or (rarely) the Reinforcement Track.
- Arrive in the **Naval Convoy Arrival Phase**; the Axis plans 2 Game-Turns ahead, the
  Commonwealth 4 (§20.21). 5 Replacement Points = one battalion (§20.45).
- **Restrictions (§20.42):** may not voluntarily enter an enemy ZOC (unless a friendly unit is
  there); may only defend against close assault; Basic Morale −3 unabsorbed; never exceed CPA
  (else Cohesion −26).

## Training (§20.43)
| Class | OpStages of Training |
|---|---|
| Gun | 1 |
| Infantry | 3 |
| Tank / AC / Recce | 6 |

Axis points train in Tripoli or any Major City; Commonwealth per §17.32.

## Use (§20.4)
- Move toward the front as **Truck Convoys** (§20.46); require Stores + Fuel like units (§20.45).
- Assigned to a specific unit on arrival (§20.41); **absorbed only in the Reorganization Segment**
  (§20.49), in the same hex, taking on the unit's cohesion/breakdown levels — restoring its TOE.
- §20.5: upgrading armored-car/recce points. §20.3 Replacement Point Conversion Chart (chart).

## Implementation
- **Done (core):** `ReplacementClass`, `ReplacementPoints { class, count, location,
  training_remaining, assigned_to }` on `GameState.replacement_points`;
  `replacement_training_stages(class)` (§20.43); `add_replacement_points(...)`;
  training ticks each Operations Stage (`begin_operations_stage`); `absorb_replacement(index,
  unit)` restores TOE (capped at `max_toe` for Organic units) when trained, co-located and
  assigned.
- **Done (§20.66a):** `reference::logistics::AxisTruckProduction` + `axis-truck-production.ron`:
  per-type lifetime total, convoy tonnage/point, and phased per-Game-Turn maxima (Light 4→15 from
  GT4/13; Medium 20→50 from GT6/13; Heavy 3→8 from GT5/13). `row(kind).max_per_turn(gt)`.
- **Done (§20.66 German):** `reference::logistics::{ProductionPeriod, ProductionItem,
  ProductionChart}` + `german-production.ron` (23 items: Infantry, Armed Recce, AA, guns, Pak,
  Marder, PzII/III/IV — each total / per-period max / first Game-Turn / convoy tonnage);
  `production_charts: HashMap<Nationality, ProductionChart>`; `chart.available(name, gt)`.
  Per-period notes captured (GameTurn / TwoWeeks `*` / Month `†`).
- **Done (§20.66 Italian):** `italian-production.ron` (20 items) using phased `tiers`
  (per-date-range maxima, e.g. Infantry 5→10→25 over GT5-8/9-24/25+, total 1200) plus flat items;
  `ProductionItem` gained `tiers: Vec<ProductionTierWindow { max, from_gt, to_gt, period }>`.
  Notes recorded: * Autobelinda-41 recce (ID "vv"), ‡ Tunis-box French guns (GT39, no delay),
  † per-month.
- **Done (§20.78A):** `reference::logistics::CommonwealthTruckProduction` +
  `cw-truck-production.ron` — a d6-per-type roll table with two eras (GT1–30, GT31+ Lend-Lease);
  `produce(kind, gt, die)`. Trucks arrive four Game-Turns later at Alexandria (≤25%/GT).
- **Done (§20.78B):** `reference::logistics::CwInfantryProduction` + `cw-infantry-production.ron`
  — a 2d6 (2–12) roll table across four Game-Turn eras (3–30, 31–46, 47–102, 103–107);
  `produce(gt, roll)`. Arrives four Game-Turns later, all in one city.
- **Done (§20.78C):** the Commonwealth Production Chart — 24 items (Armored Recce, AA, the gun/
  howitzer/anti-tank lines, and the Cruiser/Crusader/Matilda/Valentine/Stuart/Grant/Sherman/
  Churchill tank series) into `production_charts[Commonwealth]` via `cw-production.ron`, with
  first/last date ranges (`last_gt`) and no convoy tonnage. **Its symbol legend is REVERSED vs
  §20.66**: here `*` = per-Month, `†` = per-Two-Weeks (encoded accordingly). The "Tiger" Convoy
  (GT32) is separate.
- **Done (§20.3):** `reference::logistics::{ConversionReq, ConversionEntry, ReplacementConversion}`
  + `replacement-conversion.ron` (20 unit types → the Replacement Points to rebuild one TOE SP,
  with "or" alternatives, "&" combinations, and a Supplies flag). `conv.cost(unit_type)`.
- **Planning → arrival (done, §20.21):** `plan_replacement(class, count, gt, stage, location,
  assigned_to)` queues a pool; `process_replacement_arrivals()` (run at the Naval Convoy Arrival
  Phase, beside reinforcements) brings due plans into the Replacement Point pool, starting
  Training. Full lifecycle now runs: **plan → arrive → train → absorb** (restore TOE).
- **Production planning wired (§20.66/§20.78C):** the `PlanProduction { nationality, item, count }`
  command (Organization Phase) validates against the chart's lifetime `total` and per-period max
  (rolling Game-Turn window keyed to each item's GameTurn/Two-Weeks/Month period via
  `ProductionItem::period_at` + `ProductionPeriod::span_game_turns`), records the required Axis
  convoy tonnage on a `ProductionOrder`, maps the item to a Replacement class
  (`replacement_class_for_item`), and schedules the points to arrive after the §20.21 delay
  (Axis 2 / CW 4 Game-Turns). Full lifecycle: **plan → produce → arrive → train → absorb.**
- **§64.74/§64.75 victory points are implemented** (`replacement_vp` reads the chart totals minus
  `replacements_used`; withdrawal VP via §64.75).
- **§20.3 Conversion Chart applied on absorption:** `absorb_replacement` now rebuilds TOE Strength
  Points at the §20.3 cost — each SP consumes an affordable `options` set (plus a Stores Point when
  the entry is `supplies`) drawn from co-located, trained, assignable pools, collapsing the
  requirement kinds onto the four pool classes (Inf→Infantry; Tank/Lt Tank→Tank; ArmR→Recce; all
  gun/AA/AT kinds→Gun). A unit's conversion row comes from a **provisional classifier**
  (`conversion_row_for_unit`, flagged cf. D14): the special-infantry rows (Machinegun / Heavy
  Weapons) and Armored-Reconnaissance vs Armored-Car vs Motorcycle/Camel distinctions are not
  derivable from the OB data and default to the generic row (Commando / Bersaglieri / Paratroop are
  caught by unit name). With no chart loaded, absorption falls back to the legacy 1:1.
- **Still deferred:** the production item→class map flags SP-gun/tankette calls as *inferred*
  (verify vs the physical §20.66 chart, cf. D14); the recorded convoy tonnage does not yet consume
  Axis convoy capacity; the §20.78A truck / §20.78B CW-infantry roll tables remain reference-only;
  and the §20.42 restrictions/morale and §20.5 upgrades are not modelled.
