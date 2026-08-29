# The Axis Naval Convoy System (§55–56) — build plan

Large subsystem. Built incrementally; parameters -> RON, logic -> code (D9).

## Done
- **rules::naval** (chart-independent, §55–56):
  - §55.14 `effective_port_capacity` (base × current/max level, round up).
  - §55.18 `PORT_EFFICIENCY_REGEN_PER_STAGE`=1; §55.24 `MAX_PORT_LEVEL_REDUCTION_PER_STAGE`=1.
  - §55.25 `SAN_GIORGIO_TOBRUK_BLOCK_LEVELS`=3.
  - §55.21 `UNBLOCKABLE_PORTS` + `harbor_blockable`; §55.22/23 `harbor_block_cost` (Tobruk 50/25, else 25/10); §55.27 `harbor_mine_removed` (1–3).
  - §55.17 Bizerta `BIZERTA_AVAILABLE_FROM_GT`=37 + `bizerta_opens` (2d6 == 12).
  - §56.11/12 `NUM_SHIPPING_LANES`/`MAX_CONVOYS_PER_TURN`=6; §56.14 `is_greece_lane` (4,5) + `GREECE_LANES_AVAILABLE_FROM_GT`=34 + `lane_available`; §56.16 `convoy_flak_points` (tonnage/1000).

- **Port Capacity chart (§55.3)**: `data/reference/port-capacity.ron` + `reference::ports` (PortSpec, PortsChart, `is_major`, `starting_efficiency`, `spec` w/ "all others" default). Tobruk starts eff 2 (5-3 San Giorgio). Bizerta available_from_gt 35.
- **Calendar reconciled** via the Turn Record Track (`date_of`, docs/source-materials/turn-record-track.md): Bizerta GT35=Jun I 1941 (chart+prose agree), Greece lanes GT32=May II 1941 (was 34), CW high-evap through Aug 1941 = GT46 (was 48, fixed to use date_of).

- **Equivalent Weights chart (§54.5)**: `data/reference/equivalent-weights.ron` + `reference::equivalent_weights` (Rate num/den, tonnage/points; Ammo 4, Fuel 1/8, Stores 1, Water 1/6; air-replacement 2t infantry-only; interport-truck 50t; SP equivalents truck 1/10, replacement 1/5, SGSU 1/2, unit 1/2). Verified consistent with the existing rail 1500t->1500/375/12000 figures. (Rail code at game.rs:1662 still hardcodes these — could be refactored onto this chart later.)

- **Shipping Lanes chart (§56.18)**: `data/reference/shipping-lanes.ron` + `reference::ports` (ShippingLane, RallyPoint, ShippingLanesChart, distance_from, lane). 6 lanes: 1 W.Sicily->Bizerta, 2 W.Sicily->Tripoli, 3 Italy->Benghazi, 4 Greece->Benghazi, 5 Greece->Tobruk, 6 Italy->Tobruk (confirms 4,5=Greece). `rules::naval::convoy_lane_reachable` (§42.52 facility->rally + rally->lane).

- **Convoy Level Chart (§56.4) + Capacity Table (§56.5)**: `data/reference/convoy-levels.ron` + `reference::convoy` (ConvoyLevel, ConvoyLevelChart, tonnage/level_for/tonnage_for). Capacity = Fixed + Variable*die, round up to 1000. Levels A:6000/1000, B:7000/1500, C:10000/1500, D:11000/2000, E:11000/2500, F:15000/2000, G:32000/3000. Validated: E die4 = 21000 (56.21 example). 1941 schedule row: B E F E D G C E C D E A (Nov=E confirms example).
- §56.4 schedule COMPLETE: 1940 Sep-Dec=B, 1941 B E F E D G C E C D E A, 1942 C B B G F A F B D A G C (1940/1942 read from the physical chart).

## CHARTS NEEDED (OCR too garbled — need clean captures)

## State layer (done)
- **Port state** (`state::naval::Port`): efficiency/max/capacity/mines, from_spec, reduce/regain, effective_capacity. `GameState.ports`.
- **Convoy state** (`state::naval::Convoy`): lane/dest/arrival/tonnages/cancelled, total_tonnage, flak_points. `GameState.convoys` + `plan_convoy` (§56.14/56.12/56.2) + `cancel_convoys_to` (§56.15).
- **§41.6 strategic convoy bombing** (`GameState.resolve_convoy_bombing`): torpedo +25% (§41.73) -> Air Bombardment CRT Axis-Naval-Convoy row -> % lost -> applied evenly per cargo type rounding up (§41.67). `rules::naval::convoy_cargo_loss`.
- **§56.16 convoy delivery** (`GameState.deliver_convoy`): surviving tonnage -> supply points via §54.5 rates (Fuel x8, Ammo /4, Stores x1), convoy removed. Replacement tonnage handled by the replacement pool. Port receiving-capacity (§55.13) enforced at planning.

## Deferred (needs the above + state)
- ~~Convoy state + planning~~ DONE. Tonnage auto-derivation DONE: `GameState.allowable_convoy_tonnage(arrival_gt, die)` via `date_of` -> Convoy Level Chart -> Capacity Table (validated: GT55=Nov I 1941=E, die4 -> 21000, the §56.21 example).
- ~~Port state + regen/reduction~~ DONE. §55.2 harbour blocking/mining DONE: `Port::lay_mine`/`remove_mine` (§55.27), `GameState::block_harbor` (§55.21/22/24) + `unblock_harbor` (§55.26, Benghazi needs 2 engineers); costs via `rules::naval::harbor_block_cost`/`harbor_unblock_cost`/`harbor_unblock_engineers`.
- §56.2 tonnage determination + §56.22–24 loadout; §41.6 strategic convoy bombing resolution (uses Air Bombardment CRT Axis-Naval-Convoy rows already transcribed); §42.5 naval convoy recon (needs Air Recon of Axis Naval Convoys table).
- §43–44 Axis-Med basing / Malta (depend on Med geography + convoy).
