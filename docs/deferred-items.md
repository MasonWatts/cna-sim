# Deferred Items — Implementation Tracker

Master task list for closing every documented deferral / simplification in the engine.
Status legend: `[ ]` todo · `[~]` in progress · `[x]` done · `[-]` won't do (by design).
Each item cites the rule(s), the current site, and the acceptance check (a test).

Keep this file in sync as items land. When an item is done, update the matching prose in
`docs/design.md §8` and any stale code comment, and check the box here.

---

## A. Land combat (§8.4, `state/combat.rs`, `state/command.rs`)

- [x] **A1 — Anti-armor phasing-player −1-row modifier** (§14.6). `AntiArmorCrt::phasing_row_dice`
  + resolver applies it when the firer is the active (phasing) player; `AntiArmorOutcome.phasing_penalty`.
- [x] **A2 — Mutual / non-phasing defensive anti-armor fire** (§14.12). Already supported: `AntiArmor`
  isn't gated to the phasing player, so the non-phasing side fires it at the assaulting armor's hex;
  two calls (one per side) give a mutual exchange. Test `non_phasing_defensive_anti_armor_14_12` added.
- [x] **A3 — §14.21 anti-armor withholding.** `AntiArmor` gains a `withheld: Vec<UnitId>` (defender
  choice, like Patrol reveals); `resolve_anti_armor_core`/`apply_armor_damage` exclude those units.
  Remaining finer detail: the §15.16 per-TOE-point close-assault commitment split (not the AA immunity).
- [x] **A4 — Self-propelled-gun absorption (§14.47).** SP guns (Artillery/AntiTank weapon with an
  armor-protection rating) absorb at AP + Vulnerability in `apply_armor_damage`.
  - [x] **A4b — §14.48 halftrack absorption.** `apply_armor_damage` now includes truck-transported
    Organic units whose §4.44 characteristics carry an Armor-Protection rating (German "i" Schützen):
    they absorb at that rating, doubled when the hit side's only affectable points are halftracks;
    a destroyed halftrack TOE point also drops its transporting Truck Points. Dismounted (no trucks)
    the unit is ordinary infantry, unaffected. Test `halftrack_absorption_14_48`.
- [x] **A5 — Destroyed-tank markers & capture** (§14.46/§14.5). `apply_armor_damage` records destroyed
  *tanks* (Tank-kind only, §14.51) as `CapturedEquipment{destroyed_tank:true}` owned by their side;
  `resolve_destroyed_tank_capture` (§14.52, per Operations Stage) flips ownership to the enemy when
  they control the hex; the §28.3 repair pipeline returns them to play (§14.54/§22.5).
- [x] **A6 — Close-assault two-terrain-hexside exception** (§15.35). Already correct: `hexsides_on`
  returns every feature on an edge and `close_assault_terrain_shift` sums them before picking the
  single best edge. Stale deferral note removed; regression test added.
- [x] **A7 — Multi-hex defenders in close assault** (§15). N/A: `cmd_close_assault` always targets
  one hex's whole stack (§15.23), so defenders are always co-located. Doc corrected.
- [x] **A8 — Reaction §8.56 detach-to-react** (`cmd_react`): an attached unit reacts by detaching
  first (1 CP), with a guard against detach-reacting a unit that has its own attachments. §8.53a
  Truck-Convoy exception is N/A (truck convoys use `MoveTruckConvoy`); the motorized gate is the
  faithful §8.53a for regular units.

## B. Air game (§8.5, `rules/air.rs`, `state/air.rs`, `state/game.rs`)

- [x] **B1 — Box-based flight execution.** Already implemented: `flight_distance` routes a Box
  squadron's missions via `box_flight_distance` (§37.4), and bombing/CAP/recon commands accept a
  Box-based squadron (`squadron_is_based` covers `med_base`). Tested by `box_based_flight_37_4`.
  (The "Still open" design note was stale — contradicted later in the same §8.5.)
- [x] **B2 — §43.23 Crete→Suez timing.** Enforced in the bombing command via `crete_suez_window`;
  tested by `crete_suez_commitment_43_23`.
- [x] **B3 — §43.24/§43.25 Box-bomber escort coordination.** Enforced (a Crete bomber may not take an
  on-map escort; African fighters may escort Italy/Sicily bombers); covered by the med-base escort tests.
- [x] **B4 — §43.21 Box auto-supply** made explicit: `consume_sgsu_upkeep` now sets `supplied = true`
  for a Box-based (off-map, `med_base`) squadron and draws no upkeep, with a §43.21 comment.
- [x] **B5 — §25.12 major-city Fortification Level damage tracker.** New `city_fort_damage` map;
  `fort_level` subtracts it from the city's inherent level; §41.37 bombing knocks down the effective
  level (placed fort else static city level). Test extended.
- [-] **B6 — Defensive-CAP engagement choice** (§45.14). Owner-ruled operational abstraction
  (design §8.5, 2026-08-29: "air combat resolved on an operational level, much abstracted").
- [-] **B7 — Per-plane cargo-tonnage cap for air transport** (§42). Owner-ruled operational
  abstraction — design §8.5 names it alongside B6 as the two 2026-08-29 accepted air abstractions.
- [-] **B8 — §45.23/§45.33 match-up assignment choice.** Owner-ruled operational abstraction
  (best-first/round-robin default; same 2026-08-29 ruling as B6).
- [x] **B9 — Land-unit air recon** (§42.27). New `ReconHex` command flies a recon-capable squadron
  (§42.21) to a hex, validates range/weather, and reveals N enemy battalion-equivalents (§42.27 table,
  §42.24a order) into `recon_revealed`; `observe_units` shows their strength to the flier that stage.

## C. Logistics / production (§8.7, `state/game.rs`, `reference/*`)

- [x] **C1 — §20.3 absorb classifier residuals.** `conversion_row_for_unit`: recce row now derived
  from composition (`unit_has_armored_points`) — armored-recce vs light Motorcycle/Camel/Reconnaissance
  — instead of "assume armored". MG/HW rows already verified by §4.46 names.
  - [x] **C1b — Armored-Car vs Armored-Reconnaissance split.** Resolved from the §4.46 counter
    symbols (nationality legends + CW counter sheets PDF p184–185, read 2026-09-12). Key correction:
    real recce units are **all `Organic`**, so `conversion_row_for_unit` now classifies by the
    §4.44 **Armor-Protection rating** (armored vs light) + nationality/counter, not composition
    (the old `unit_has_armored_points` gate and its "Organic recce = light" test premise were wrong
    and were removed):
    - **German** armored recce (Aufklärungs `ee`/`ff`) → Armored Reconnaissance (legend has no
      armored-car glyph).
    - **Italian** armored recce (Autoblinda `vv`/`ww`) → Armored Car (legend has no armored-recce
      glyph); unarmored (camel/desert-patrol/Bersaglieri-MG `xx`/`yy`/`zz`/`aaa`) → light.
    - **Commonwealth** (all recce armored, mixed car/recce — same id spans both, e.g. CIH vs 13 DCO
      Lancers both `ll`): read per-counter. **Armored Car** = Derbyshire Yeomanry, Royal Dragoons,
      13th DCO Lancers, 10th QVOC Guides, 1 Free Fr Flying Column, the SA "Armored Car" regts.
      **Armored Reconnaissance** = CIH, 44/50/51 Recce Regt, 6/7/9 Aus Div Cav, 1 DoY Cav, 2 NZ Cav,
      7 SA Recce, 1 Polish Cav. Only **11th Hussars** wasn't locatable on the sheets — it defaults
      to Armored Reconnaissance (historically an armored-car regt; revisit if its counter surfaces).
      Tests `recce_armored_car_vs_recce_rows_20_3` + updated `replacement_conversion_absorb_20_3`.

## D. Reorganization (§19, §8.12, `state/game.rs`)

The §19 reorg cluster all keys off the **§19.3 Formation Organization Chart**, which is *not* in the
digitized Charts & Tables (its own note defers "singular" formations to their OA-sheet structure).
The engine's accepted workaround is each formation's historical `assigned_capacity` captured from the
loaded OB. Implementing the below with guessed maxima would be *less* faithful than that, so they are
blocked on either that chart or hierarchical-OB modeling — not on engine logic.
- [-] **D1 — Hierarchical OB non-shell children vs OA maxima** (§9.26/§9.27). Blocked: needs
  hierarchical OB modeling (units currently flat with parent pointers).
- [-] **D2 — §19.3 real assignment maxima.** Resolved as approximated-by-design (2026-09-09): there
  is **no dedicated §19.3 chart** (confirmed with the rulebook owner) — §19.3 defers each formation's
  organization to its OA-sheet structure, which the engine already captures as `assigned_capacity`
  (the historical assigned count) and enforces in `ReassignUnit`. This is the faithful representation,
  not a stand-in. (Distinct from the §19.5 Maximum Attachment Chart, which *is* digitized and enforced.)
- [x] **D3 — CW battalion-shuffle** (§19.26). Already supported: `ReassignUnit`/`DetachUnit`/`AttachUnit`
  let CW freely shuffle battalions within a brigade's `assigned_capacity` — the rule "falls out" of
  the general reorg commands.
- [-] **D4 — Historical (time-phased) assignment shifts** (§19.24/§19.25). Approximated-by-design
  (2026-09-09): with no §19.3 chart (see D2), there are no per-slot time-phased assignment maxima to
  model; `assigned_capacity` is the OA-sheet organization. The §19.5 *attachment* caps that ARE
  time-phased (CW armor division / Tank Brigade GT-1–67 vs 68+, Matruh/Selby GT windows) are enforced.
- [-] **D5 — Substitution** (§19.45). Approximated-by-design (2026-09-09): needs §19.3 per-slot maxima
  which don't exist as a chart (see D2); the general reassign/attach commands already cover the
  observable behavior within `assigned_capacity`.

## E. Prisoners & morale training (§8.13, §8.4, `state/game.rs`)

- [x] **E1 — §28.24 far-escapee autonomous movement / water-attrition.** `resolve_prisoner_escapes`
  now steps each far escapee one hex toward the nearest friendly combat unit over passable,
  non-enemy-occupied desert, and applies 10%-per-Operations-Stage thirst loss (ceil) when the hex
  has no water source; a stack reduced to 0 dies out.
- [x] **E2 — §51.17 Guard-Point Stores upkeep.** New `feed_guards` draws 2 Stores per Guard Point per
  Game-Turn from the nearest dump (§51.12 prisoner Stores were already modeled via §28.15/`feed_prisoners`).
- [x] **E3 — §17.32/§20.43 training areas (data-driven, side-locked).** Superseded the earlier
  hard-coded `is_cw_training_area` list: training areas are now `Feature::CwTrainingArea` /
  `Feature::AxisTrainingArea` on the map (§20.43 — the two lists are not interchangeable), placed on
  the section RON (CW: Cairo/Helwan/Alexandria/Amiriya/Aboukir/Dekheila; Axis: Benghazi/Tobruk/
  Bardia). `is_training_area(hex, side)` reads the side's feature. §20.43 Replacement-Point training
  is also location-gated on these (the countdown runs only at a training area for the pool's side).

## F. Desert raiders & patrols (§8.9, §8.11, `state/command.rs`, `state/game.rs`)

- [x] **F1 — §27.12 trained-to-max-morale precondition.** Already enforced in `raider_source_ok`
  (`basic_morale >= basic_morale_reference`); stale design note corrected.
- [x] **F2 — §27.38 per-hex fuel.** No change needed: §27.38 says raiders consume *no* per-hex fuel
  (only the §27.35 flat allotment), which the engine already models. The "per-hex" deferral was a
  misnomer; design note corrected.
- [x] **F3 — §27.58 all-replacement convoy auto-elimination.** `resolve_truck_convoy_raid` now
  auto-eliminates the raider (no truck loss) when the convoy hauls Replacement Points and no
  commodity cargo (`carries_replacements && cargo.is_empty()`), alongside the existing "any" case.
- [x] **F4 — §27.53/§27.54 side-tagging.** Plane raids already target only enemy SGSUs; air facilities
  are non-denominational by §36.15 (no owner to tag), so the airfield-raid gate is correct as-is.
- [x] **F5 — §12.23 specific-unit barrage designation.** `Barrage` gains `target_unit: Option<UnitId>`;
  a designated unit must be an eligible target of the class AND known to the firer (In-Contact/Engaged
  = "previous combat"). New `CommandError::TargetNotKnown`. (Patrol-knowledge path isn't persisted, §8.11.)

## G. Range-proxy simplifications → real CP-based ranges

New shared helper `within_cp_budget` (cost-limited Dijkstra over `move_step_cost`) and
`patrol_path_exists` (bounded BFS) back these.
- [x] **G1 — §16.22 patrol path.** `patrol_path_exists`: a legal ≤5-hex path must exist (passable
  terrain, not through enemy-occupied hexes), replacing the straight-line distance check. §16.22's
  "5 hexes" is the actual rule (not a proxy); the path validity was the gap.
- [x] **G2 — §27.59 raider convoy-intercept range.** Now "within 4 Movement CPs" via `within_cp_budget`
  (motorized), paying real terrain costs, not hex ≤4.
- [x] **G3 — §28.13 prisoner move CPA.** Entering each hex now costs its real foot (non-motorized)
  terrain cost against the CPA-8 budget, not a flat 1 CP.
- [x] **G4 — §28.24 escapee return range.** `friendly_unit_within` now checks "within 8 CP" of foot
  movement (`within_cp_budget`, non-motorized), not hex-distance ≤8.

## H. Scenario & OB fidelity

- [x] **H1 — Real TOE compositions at scenario setup.** Already implemented: `scenario.rs` resolves
  each Organic unit's starting TOE from its §4.44 code (`N`→id Max-TOE via `unit_max_toe`, `U@x`/`O@x`→x)
  and Weapons units carry the chart's real counts. The `current_toe: 0` is a transient placeholder
  overwritten before placement. (Stale module/design comment corrected.)
- [x] **H2 — §9.2 unit-size verification (2026-09-09).** Resolved with the rulebook (PDF read via
  `tools/pdf-page.sh`). **The premise was a mislabel:** §9.2 is *"Unit Equivalents"*, not counter
  size-symbols, and §9.22 states the game org level "is guided **NOT by the unit designation on the
  counter** or the unit's name **but rather through the printed Stacking Point value**" (cross-ref
  §9.4). So the XX/X/III/II/I symbols are the *historical* echelon (§3.2), explicitly non-authoritative
  for game size — a III "Regiment" is routinely a battalion-equivalent (Italian regiments, British
  "brigades" that are really battalions). The engine's `org_size` derives from the §4.44
  Bn-/Bde-/Coy-Equivalent groupings — i.e. §9.2 itself — so it is authoritative by construction; the
  61 org_size-vs-symbol differences are the rulebook's own documented "maze", not errors.
  **Verified the one absolute rule, §9.24** ("every artillery unit … has a Stacking Point value of 1"
  = battalion-equivalent, batteries excepted): found and fixed **4 Italian artillery regiments** wrongly
  set to `StandardBrigade` (203rd/44th/45th border-division + 185th Folgore Artigliera Regt, all id `kk`
  "Artillery Bn-Equivalent") → `Battalion`. Added a §9.24 regression assertion to
  `ob_ids_are_valid_characteristics_keys`. Coastal-defense arty *groups* (Battalion) and *batteries*
  (Company) are §9.24-consistent; garrison/oasis "Bn"s follow their designation per §9.23.
- [x] **H3 — OB `Deployed`-arrival verification pass (2026-09-08).** Every `Deployed`-default unit in
  `data/ob/commonwealth.ron` and `data/ob/italian.ron` was cross-checked, formation-scoped, against
  the §4.44a/§4.44b OA-sheet "Arrives" columns in `docs/source-materials/*-ob-4.44*.transcribed.md`.
  All faithful — the Italian 10th Army was entirely in Libya Sept 1940 (every garrison/division/pool
  correctly "D"); 6th Australian and 2nd New Zealand carry "D" HQs on their own sheets (footnoted
  "arrived early, in sections, not committed to battle for months"); 70th Infantry matches exactly
  (HQ 2/21, 16th Bde `Deployed` on loan to 4th Indian, 23rd Bde 3/51). **No OB corrections were
  required** (contrast the earlier German Ramcke/II-155 fixes, which *were* real gaps). 70th Inf /
  6th Aus / 2nd NZ stay absent from `campaign.ron` by design: the on-map start force is the §60.41
  chart, and §4.44 "D" is a master-OB availability marker gated per-scenario (the loader only
  schedules `Arrival::Turn` units for reinforcement-only formations). Supersedes the "arrival dates
  not all transcribed" gap noted in design.md §8.8 / campaign.ron header.

## Won't do — by design (documented deliberate decisions)

- [-] Hex-placed convoy / sea-movement model — violates §56.11 faithful lane model.
- [-] Command-level enforcement of "act only on observed info" — Phase-2 agent concern (§8.10).
- [-] Air recon "operational abstraction" ceiling already ruled acceptable (2026-08-29) beyond B6/B7.

Note: the earlier "only §64.7 campaign victory is modeled" entry is retired — §60.81 Scenario-1
("Graziani's Offensive") victory is now implemented (`GameState::scenario1_victory`, `view.rs`).
