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

---

## Audit 2026-09-12 — untracked rule-coverage gaps (full §1–§65 re-read)

Found by a fresh rulebook audit; none were in the tracker above. Ordered by impact.
Scope items excluded from "address them all" are marked `[-]` (by design).

### Systemic — the CP / cohesion economy
- [x] **AU1 — §11.2 combat expends Capability Points** (5 phasing / 3 non-phasing / 2 probe / 1 at
  −4 diff). `cp_expended` is written only by movement/air-transport; close-assault/anti-armor/barrage
  never charge CP, so fighting is free against CPA and the §6.21 over-CPA disorganization never fires
  for combat.
- [x] **AU2 — §6.23/§6.24 cohesion recovery (Reorganization Points).** +5 for an idle Operations
  Stage, +3 for a won close assault, capped at +10; today every cohesion write is a decrement — units
  never heal.
- [x] **AU3 — §15.87 (+§6.21) close-assault Disorganization.** A unit taking ≥30% TOE loss in one
  close assault gains 3 Disorganization Points (all involved units). Losses are computed but no DP.

### ZOC / reserve / status gates
- [ ] **AU4 — §10.3/§10.31–36 mandatory attack of enemy ZOC-exerting hexes** + Holding-Off Barrage +
  §10.36 forced 3-hex retreat/3 DP fallback. ZOC projection/Contact/Engaged computed but never obligates combat.
- [ ] **AU5 — §18.2 Reserve I/II effects** (Reserve I ≤1 hex, Reserve II no move; CP release caps;
  §18.24 auto-DP; two-hex proximity exception). `ReserveStatus` stored but movement/reaction/release ignore it.
- [x] **AU6 — §12.44 Pinned** blocks Move/AntiArmor/CloseAssault (currently only RBA/Reaction check it).
- [x] **AU7 — §6.26 cohesion ≤ −26** blocks Move/CloseAssault/AntiArmor + adjacency auto-surrender
  (currently only RBA/Reaction check the −26 gate; no auto-surrender).

### Combat detail (§12–§15, §23, §26)
- [ ] **AU8 — §15.9 Probe combat mode** (Engaged ignored, not-In-Contact, §15.95 all-recce −10%,
  §15.96 defender spends no CP at diff < −3).
- [ ] **AU9 — §15.29 defender allocates no units → auto-retreat 3 hexes + 3 DP** (decline-to-defend).
- [x] **AU10 — §15.23/§14.22 once-per-Combat-Segment per-hex guard.** close_assaulted_hexes_this_stage / anti_armor_hexes_this_stage (reset per stage) refuse a repeat close assault / anti-armor on the same hex. Test hex_once_per_combat_segment_15_23.
- [ ] **AU11 — §13.28 a Retreat-Before-Assault unit still eats anti-armor fire (can't return it) and
  its TOE counts for % losses.**
- [x] **AU12 — §15.88/§15.15 assaulted out-of-ammo / ≤−17-cohesion defenders auto-surrender.** cmd_close_assault surrenders such defenders before resolution (en masse if all); the rest defend. Test assaulted_defender_auto_surrenders_15_88.
- [ ] **AU13 — §12.1/§12.11–16 Gun Forward/Back declaration** command + its barrage/coordination
  consequences (enum exists, feeds §15.84 vulnerability, but defaults Back with no setter).
- [ ] **AU14 — §12.46 barrage rolls the Trucks column** (truck casualties; a hit motorized unit loses
  transporting trucks).
- [ ] **AU15 — §12.5/§12.52–55 artillery barrage vs Facilities** (cities/forts/roads/rails/dumps/
  airfields) — land barrage targets a UnitId only.
- [x] **AU16 — §23.24 engineer/HQ in a close assault vs a fortified (non-major-city) hex → +1 Assault
  Differential for the attacker.**
- [~] **AU17 — §26.26 minefield combat shift.** Close-assault +1 defender shift done (close_assault_env_shift); the anti-armor-fire column shift is still pending (the AA CRT is points-bracket, not a shiftable column).
- [x] **AU18 — §23.21 reduced enemy-minefield entry cost extends to units stacked with an engineer.** move_step_plan now grants the engineer reduction to a mover co-located with an unpinned engineer-capable unit.

### Morale / organization / reinforcements (§17–§22)
- [ ] **AU19 — §17.5/§17.51–56 voluntary surrender of a stack** (enemy pays 2 CP; optional pre-capture
  destruction of tanks/guns/trucks).
- [ ] **AU20 — §19.8/§19.81–87 ad-hoc Axis AT batteries** (augment a Brigade HQ with AT Replacement
  Points; 67% gate, single gun type).
- [ ] **AU21 — §19.9/§19.91–98 CW battalion AT augmentation** from GT-75 / Apr 1942.
- [ ] **AU22 — §19.72/§19.73 battle-group forming composition caps** (German ≤4 bn/≤1 tank/≤2 inf;
  Italian ≤3 bn/≤2 inf/≤1 armor, ≤2 in play) — pool exists, limits unenforced.
- [ ] **AU23 — §20.5/§20.51–55 upgrade Armored-Car/Recce to newer vehicles** (Stuarts etc.; TOE change;
  not before 1/39).
- [ ] **AU24 — §20.8/§20.81–85 scheduled CW Mandatory Withdrawals** (reach Cairo/Alexandria by a listed
  GT; §20.85 penalty) — distinct from the §64.75 voluntary path.
- [ ] **AU25 — §21.5/§21.51–55 capture enemy broken-down (not destroyed) vehicles during movement.**
- [ ] **AU26 — §21.6/§21.61–67 tow broken-down vehicles to a repair facility.**
- [ ] **AU27 — §22.6/§22.61–66 CW Tank Delivery Squadrons** (deliver tank Replacement Points).
- [ ] **AU28 — §22.7 German Mobile Tank Repair unit.**

### Movement / stacking / terrain (§8–§9)
- [ ] **AU29 — §9.33/§9.34 road/track 5-Stacking-Point transit limit** for motorized units passing
  through friendlies (no break-up to evade).
- [-] **AU30 — §8.17 150% CPA hard ceiling.** Deferred: a hard ceiling blocks a single expensive
  step (escarpment/sandstorm) that current cost-mechanics tests rely on, and likely needs a §8.16
  minimum-move exception; low value vs. the §6.21 over-CPA Disorganization already modeled.
- [ ] **AU31 — §9.25 ≤ five 0-Stacking-Point companies/batteries per hex** (unlimited only in a Major City).
- [ ] **AU32 — §9.16 garrison "G" units stack free in their assigned city; AA/Flak free in Major
  Cities and on airfields.**
- [ ] **AU33 — §8.44/§8.45 Salt Marsh / Desert unit-subtype entry restrictions** (light-vehicle-only
  off-road; no AFV close-assault into Salt Marsh).
- [-] **AU34 — §8.19 no CW west of Marble Arch (A2109 = col 9).** Deferred: a blanket col-9 gate
  breaks the many synthetic tests that place CW units at low columns; it's a map-convenience
  boundary of little gameplay value on the real map.

### Weather (§29) — rainstorm terrain cluster
- [x] **AU35 — §29.55 rainstorm: wadi hexsides uncrossable except by road; no wadi water draw.**
- [ ] **AU36 — §29.56 rainstorm degrades road hexes to track cost/breakdown.**
- [x] **AU37 — §29.57 rainstorm: non-road/rail river hexsides impassable.**
- [x] **AU38 — §29.58 rainstorm bars vehicle/motorized units from delta hexes.**
- [x] **AU39 — §29.42/§29.54 no construction in a sandstorm/rainstorm hex** (BuildRoad/BuildRailroad
  apply no weather check; only field repair is gated).
- [ ] **AU40 — §29.32 construction sites need water rations in hot weather.**

### Air ↔ land, logistics, naval (§35, §49–56)
- [x] **AU41 — §35.12 SGSU overrun by advancing land units.** move_counter now eliminates an enemy
  SGSU a mover ends adjacent to when no friendly combat unit is stacked with it (it can't react
  during the enemy move); location→None (redeployable later, §35.13).
- [~] **AU42 — §35.27 grounded planes.** An overrun SGSU's grounded planes are now lost (planes
  cleared on elimination). The capture-for-enemy-reuse (1 GT later) refinement is still pending —
  needs a cross-nationality captured-plane pool.
- [x] **AU43 — §49.19/§50.16/§51.16 captured supply dumps** change ownership on overrun + captured-usable
  fraction (fuel non-denominational; ⅓ ammo, ½ stores) — dumps currently never change hands.
- [ ] **AU44 — §56.16 bombed convoy's own flak** (= lane tonnage ÷ 1000) fires back at attacking CW bombers.
- [-] **AU45 — §55.17 Bizerta annual 2d6=12 roll.** Deferred: the port's `available_from_gt` isn't
  wired into runtime `Port::is_operable` (which is just efficiency>0), so both the GT-35 gate and the
  annual roll need a Port open-flag set at turn start; low impact (Tunis-box late-war edge port).
- [ ] **AU46 — §41.81/§41.84/§41.86 CW bombing of the Axis Tripoli/Tunisia off-map boxes** (port level,
  grounded planes; night vs Tripoli).
- [ ] **AU47 — §30.5/§30.51–59 CW port-to-port naval troop transport** (Port Personnel Capacity; one
  transfer/stage/port).
- [ ] **AU48 — §30.4/§30.41–46 Italian 10th Light Flotilla frogman/Chariot raid** on the fleet at Alexandria.
- [ ] **AU49 — §35.22 in-play plane transfer between squadrons** on SGSU loss / <50% ready capacity (low
  impact while AU41 unmodeled).
- [-] **AU50 — §35.28 Italian squadrons all-German planes.** Deferred: plane groups key by aircraft-type string, not per-plane nationality, so enforcing it needs a cross-chart nationality lookup for a minor historical edge.
- [ ] **AU51 — §53.31/§53.32 trucks/guns off-track in salt-marsh auto-abandoned** (Qattara corner case).
- [ ] **AU52 — §53.25 leapfrogging prohibition** (cargo inherits first truck's expended CP).
- [-] **AU53 — §28.18 prisoners by truck.** Deferred: the rule is underspecified ("if you are feeling profligate with your transport" — no capacity/CP mechanics), and the foot path (§28.13) is modeled; low value.

### Scope — won't-do / out of scope (excluded from "address them all")
- [-] **§32 Abstract Logistics** (+ §47/§58 abstract air/naval) — a whole *alternate* simplified
  ruleset; the engine implements the full §34–56 games instead. Deliberate either/or.
- [-] **Scenario-group victory conditions** §60.82 / §61.8 / §62.8 / §63.91 / §63.92 — absent because
  those scenarios aren't built as playable setups (only §60.81 Scenario 1 and §64.7 campaign exist).
  Land the scenarios first; the victory evaluators come with them.
- [-] **§64.53/§64.54 "Desert Fox" shorter-campaign reduced pools; §59.36 first-OpStage maintenance ban;
  §63.5 El Alamein special-rule toggles** — tied to unbuilt scenario setups.
