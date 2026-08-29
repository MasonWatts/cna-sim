# The Commonwealth Fleet & Mediterranean Naval Gunnery (§30)

The Mediterranean naval war in CNA is abstracted: the Commonwealth has a small fleet of "type"
warships (§30.1) that bombard the coast, take air/gunfire damage, and shuttle between Alexandria,
Valetta, and sea-hexes; the Axis has only the San Giorgio, a scuttled cruiser blocking Tobruk
(§30.17). This subsystem interacts in every direction — air→ship, ship→land, land→ship, and
ship→ship. Parameters are held in state; the rules/logic live in `state::naval` + `state::command`.

## Implemented

- **Ship state (§30.1, §30.12-13, §30.36-37)**: `state::naval::Ship` + `ShipLocation`
  (`Alexandria` / `Valetta` / `AtSea(hex)`) + `GameState.fleet`. A ship's **Gun Rating is both its
  barrage strength and its damage capacity** (§30.13); the AA Rating scales *proportionally* with
  remaining Gun (`current_aa`, §30.13). Methods: `is_sunk` (Gun 0, §30.36), `seaworthy` (≥50% face
  Gun, §30.36), `is_heavy` (Gun 4+, §30.21), `bombardment_strength` (Gun, halved at two hexes,
  §30.22), `damage` (Gun-Rating Damage Points, §30.34), `repair_tick` (one Gun level per six
  consecutive Alexandria stages, §30.37).
- **Firing / rest / at-sea cycle (§30.16, §30.24, §30.25)**: `fired_this_stage` (§30.24 — a ship
  fires its Barrage Strength once per Operations Stage), `at_sea_stages` + `rest_owed` with
  `can_put_to_sea` (§30.16 — at most three consecutive stages at sea, two port stages owed per sea
  stage) and `begin_stage` (clears the fire flag, pays down owed rest while in port). Wired into
  `GameState::begin_operations_stage`.
- **Commonwealth Fleet Phase commands (§IV.E)**: `AssignShip` (place a warship, enforcing §30.36
  seaworthiness, §30.18 Valetta five-ship cap, and the rest/sea-cycle limits → `ShipMustRest`) and
  `RepairShip` (§30.37, Alexandria-only). Outcomes `ShipAssigned` / `ShipRepaired`.
- **Off-shore bombardment (§30.2)**: `OffShoreBombard` (Movement & Combat / Barrage Step). The ship
  fires its Gun Rating **directly as Actual Barrage Points** (§30.22) at an enemy unit of a chosen
  class — same coastal hex (any ship) or an adjacent hex (heavy ships only, half strength, §30.21).
  No ammunition (§30.22). Shares the barrage core via `GameState::resolve_barrage_points`
  (refactored out of `resolve_barrage_with_dice`). Sets the once-per-stage fire and a two-stage
  Alexandria rest (§30.24/30.25).
- **Air attacks on ships (§30.34)**: `BombShip` (Land Support Air Phase) — a full bombing mission
  (intercept → flak → bomb) resolved on the Air Bombardment "units" CRT group, the result applied
  as Gun-Rating damage. Never at night (§41.34).
- **Surface gunfire vs ships (§30.35 / §30.39)**: shared `GameState::resolve_gun_hit_on_ship`
  (points → Air Bombardment "units" group → Gun-Rating damage). `ArtilleryBombardShip` (§30.35 —
  Axis artillery/emplaced guns in the ship's hex; raw Barrage Points halved for mobile guns, whole
  for emplaced/zero-CPA guns; Ammo expended; side-checked so only the fleet's enemy fires) and
  `SanGiorgioAttackShip` (§30.39 — the San Giorgio's full Gun Rating at a ship in an adjacent sea
  hex). Outcome `ShipDamaged`.
- **San Giorgio (§30.17, corrected by §60/§61 note c)**: `state::naval::SanGiorgio` (hex, blocked
  port, `gun_rating`, `aa_rating`) + `GameState.san_giorgio`; `place_san_giorgio` /
  `demolish_san_giorgio`. Its harbour block is a **persistent Efficiency hold-down** — `Port.wreck_block`
  (from `PortSpec.starts_blocked`) folded into the regain ceiling, so Tobruk caps at 2 while the
  wreck stands and repairs to 5 only after removal (this **fixed a latent bug** where regain ignored
  the block). Fires only at **ships** (`SanGiorgioAttackShip`) and **aircraft** (`aa_points`,
  zero once removed) — **never land targets** (see Discrepancies).

## Source discrepancies (flagged, not silently resolved)

- **San Giorgio land fire — §30.17 vs §60/§61 note (c).** §30.17 reads "It may fire into adjacent
  hexes," which an earlier commit implemented as a land barrage (`SanGiorgioBombard`). Setup note
  (c) is explicit and more specific: the San Giorgio **may not barrage land targets nor ever be
  attacked by land units**; it fires only at warships (barrage rating) and aircraft (AA rating).
  **Resolution: note (c) wins.** `SanGiorgioBombard` was removed; `SanGiorgioAttackShip` + `aa_points`
  remain (commit `bfed3cd`).

## Data still needed (counter/scenario-card values not in the machine-readable rules)

- **Warship Gun/AA ratings by type.** Only the Battleship (6/12, §30.12) is stated in the text.
  Cruiser, AA Cruiser, and Destroyer ratings are on the counters — **unknown**. Needed before the
  fleet roster can be pre-placed (see `scenario-setup.md`).
- **San Giorgio's barrage + AA counter values** — placeholders used in tests; real values unknown.

## Deferred (mechanic works; refinement pending)

- Coastal-hex terrain requirement for off-shore bombardment (§30.2) — not enforced (any hex allowed).
- §30.35 Vulnerability-Rating ≥ 6 eligibility gate — not enforced (Vulnerability is not a
  `RatingKind`, so it is not cleanly readable).
- San Giorgio sea-hexside line-of-fire (§30.17) — adjacency is checked, hexside legality is not.
- `SanGiorgio::aa_points()` is exposed but **not yet wired into the flak** of Commonwealth air
  missions bombing/over Tobruk.
- Off-shore bombardment feeds the §12 barrage CRT directly; the §30.23 "which Barrage Step" timing
  is caller-driven, not modeled.

## Tests

`ship_damage_aa_and_repair_30`, `ship_rest_cycle_30_16` (state, `state/naval.rs`);
`commonwealth_fleet_commands_30`, `off_shore_bombardment_30_2`, `bomb_ship_applies_gun_damage_30_34`,
`artillery_and_san_giorgio_vs_ship_30_35_39`, `san_giorgio_blocks_aa_and_is_removed_30_17`
(commands, `lib.rs`).
