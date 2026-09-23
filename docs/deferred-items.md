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

The §19 reorg cluster all keys off the **§19.3 Formation Organization Chart**. **Correction (2026-09-13):**
that chart *does* exist and is in the source materials — three nationality panels: **§19.31 Allied (PDF
p135), §19.32 Italian (p174), §19.33 German (p175)**. The earlier note here ("no dedicated §19.3 chart,
confirmed with the rulebook owner") was **wrong** and is retracted. The chart lists, per formation type,
the maximum assigned composition — but slots are drawn at their **echelon** (e.g. a Panzergrenadier
division's infantry slots are *regiments*, not battalions; cf. 164 Leichte's OB = 3 regiments / 9 bns)
and use symbol-level distinctions (tank vs tank-destroyer vs anti-tank; the `(2)[⊡]` "artillery HQ +
all its units, or any three 1-SP units" group vs the `1[⊡]` single 1-SP unit; heavy vs light AA). The
engine already bounds each formation by its historical `assigned_capacity` (total assigned count,
enforced in `ReassignUnit`) and by the §19.5 Maximum-Attachment type/count caps.
- [x] **D1 — Non-shell children vs OA maxima** (§9.26/§9.27). Unblocked by the §19.3 chart data:
  `is_shell` now implements the division and brigade/regiment clauses. A **division** is a shell when
  ≤50% of the brigade-equivalents its chart row allows (`formation_org_max_subunits(hq, Bde)`) are
  attached and not themselves shells; a **brigade/regiment** when <2/3 of its allowed battalions are
  attached and non-shell (with the §9.26 two-battalion-brigade exception). §9.27 (shell children don't
  count toward the prerequisite) falls out of the recursive `count_attached_nonshell`. Formations
  without a chart row (pool/dynamic HQs) are never flagged. Test: `formation_org_division_shell_9_26`.
- [x] **D2 — §19.3 real assignment maxima.** Chart located (p135/174/175), transcribed to
  `docs/formation-org-chart-transcription.md`, and encoded in `reference::formation_org` as
  per-formation slots `(family, echelon, max)` — echelon-aware per §9.26 (divisions cap brigade/
  regiment sub-formations, brigades cap battalions). **Enforced for all three nationalities** in
  `ReassignUnit` via `formation_org_assign_ok`, dispatching the family classifier by nationality
  (`org_family_of`: German/Italian by name; Commonwealth by §4.46 `unit_type` + a tank flag, since CW
  battalions are named after their parent regiment). Each nationality is guarded by an OB-fill
  invariant test (`formation_org_{german,italian,commonwealth}_ob_within_maxima` — every formation's
  historical fill ≤ its chart maxima). Plus the German text rules: "No Battle Group assigned to a
  division" and the 15th-Inf-Bde ≤6-infantry cap. Reconciliation calls: German 15 Pz infantry = 3
  regiments (the 15th Schützen Bde grouping); Italian/CW divisions modeled at full historical org
  (Maletti +3rd regiment; Libyan Tank Command = the GT 1–26 "Tank Group"). Remaining refinements: CW
  time-phasing (see D4) and counting division regiments transitively through a brigade container
  (currently direct sub-formations of the HQ).
- [x] **D3 — CW battalion-shuffle** (§19.26). Already supported: `ReassignUnit`/`DetachUnit`/`AttachUnit`
  let CW freely shuffle battalions within a brigade's `assigned_capacity` — the rule "falls out" of
  the general reorg commands.
- [x] **D4 — Historical (time-phased) assignment shifts** (§19.24/§19.25). Time-phased §19.3 rows
  (`FormationOrgRow.gt`) are enforced and audited per Game-Turn window: German 5 Le (GT 1–57) vs 21
  Panzer; Italian Libyan Tank Command "Tank Group" (GT 1–26); and the **Commonwealth armored brigade
  Type I/II/III** (GT 1–18 = 3 tank; 19–70/92+ = +1 infantry; 71–91 = +1 artillery). The CW audit is
  window-aware (counts only units present in each window via OB arrival/withdrawal). The 5th Light →
  21st Panzer transformation is modeled in the OB (one formation, both HQ counters, arrivals 5 Le
  GT21/3 → 21 GT58/2). Test: `formation_org_commonwealth_armored_brigade_time_phased_d4`. *Note:* the
  CW armored-division and Support-Group Type windows remain modeled by their union composition (max
  across periods) — a faithful upper bound; per-window gating for those would need the same
  withdrawal-timing reconciliation and is a minor future refinement. The §19.5 *attachment* caps that
  are time-phased (CW armor division / Tank Brigade GT 1–67 vs 68+, Matruh/Selby) are enforced.
- [x] **D5 — Substitution** (§19.45 / §19.33). The §19.33 substitution — "The 28th Sonderverband and
  361st Afrika regiments may be freely substituted for any 'normal' infantry regiment" — is *permissive*
  and already falls out of the general reassign/attach commands (no per-slot type gate blocks it). The
  paired *restriction* "No Battle Group may be assigned to a division" is now enforced in `ReassignUnit`.

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
- [x] **H4 — Ground-unit equipment refit subsystem — DONE (2026-09-22).** §4.44a scheduled, in-place
  equipment refit: `ObUnit.refit: Option<ObRefit>` (data) is stamped onto `Unit.pending_refit` at
  deploy; `GameState::process_refits` (called each Operations Stage beside `process_returns`) swaps the
  unit's `from` weapon system to `to` (TOE counts unchanged) and re-types the counter when its
  `(Game-Turn, Op-Stage)` is reached, queuing a `SystemLogEntry` so the refit shows in the viewer's
  **Action Log** (recorder drains `game.system_log` after each phase advance → a "Refit" event with
  `summary`, `§4.44a`, and a jump-to-activity hex). **11 RHA** is now one counter (25-pdr, turn 59)
  that refits to the **Bishop SP 25-pdr** at turn 92 — the old duplicate pool entry was removed. Test
  `scheduled_equipment_refit_4_44a`. **The two other CW refits — which are NOT schedule-driven — were
  implemented 2026-09-22 with their own mechanisms (not `ObRefit`):**
  - **Deacon → 102nd Anti-tank Regt — player-triggered & repeatable command — DONE.** §4.44a footnote b
    (the `102` A/T Regt, id `aa`, *not* the `102(NH)` LAA/AT unit). `Command::RefitDeacons { unit }`
    (Organization Phase, Commonwealth): gated by `Unit.can_refit_deacons` (stamped from the OB
    `deacon_refit: true` flag), the unit at Cairo (E1430) for two continuous Game-Turns
    (`Unit.cairo_dwell >= 2`, bumped once per Game-Turn in `begin_operations_stage` via
    `update_deacon_dwell`), and ≥1 trained SP-6-pdr AT (`Gun`/`LtAt`) Replacement Point co-located at
    Cairo. `GameState::refit_deacons` converts `min(stock, tradeable AT TOE)` guns: consumes that many
    Deacon RP, swaps that many of the unit's AT-gun TOE to `SP 6-pounder`, and returns the traded-in
    guns as *trained* `Gun` RP at Cairo. Offered via `legal_organization_commands` (dry-run gated to the
    eligible 102nd); friendly Action-Log summary + jump-to-hex via the replay `describe_command` /
    `activity_target` arms. Test `deacon_refit_player_command_4_44a_b`. *Modeling note:* the pool's own
    "two continuous Game-Turns in Cairo" is approximated by requiring it to be trained (`training_remaining
    == 0`) and co-located — the §20.43 stage-counted Training already encodes the pool's wait; only the
    unit's dwell is tracked explicitly. The rule's optional clause (Deacons to *other* AT units up to
    their TOE-limit gap) is not modeled — the historical use is the 102nd.
  - **Scorpion → 1st Army Tank Bde (42nd & 44th RTR), via §23.15 — refit-on-return — DONE.** Data-only,
    reusing the H4 subsystem: 42/44 RTR carry `refit: (at:(99,1), from:"Matilda Mk.II", to:"Scorpion",
    unit_type: Some(Engineer))`. They withdraw at (93,2) and return at (99,1); `process_returns` runs
    before `process_refits` in the same stage, so they re-enter and are immediately re-equipped as
    Scorpion anti-mine engineer units, matching §23.15. (8 RTR is unaffected.)
- [x] **H5 — CNA Addenda (Sep 1979) rules-logic corrections — DONE (2026-09-22).** OB/chart *data*
  corrections were applied 2026-09-16 (see `docs/source-materials/CNA Addenda Sep 79.pdf`); the
  rules-logic items were closed 2026-09-22:
  - **§14.48 halftrack loss cap.** `apply_armor_damage` (combat.rs) caps halftrack-motorized TOE loss at
    2 per Anti-Armor Fire segment (`HALFTRACK_LOSS_CAP`); test `halftrack_absorption_14_48` (5 → 2).
  - **§49.4 no-fuel assault.** `cmd_close_assault` bars an offensive close assault by a fuel-using unit
    (motorized / mechanized / tank-AC-recce) with no accessible Fuel; foot infantry unaffected. The
    §49.4 *defensive* two-column attacker shift when a defending tank-type unit has no fuel is applied
    via `close_assault_fuel_shift`. Test `no_fuel_bars_close_assault_49_4`.
  - **§14.47 SP-gun "Back" (erratum).** SP guns absorb anti-armor at their Armor Protection Rating only
    (dropped the old `+ Vulnerability` bonus); an SP gun barraging "Back" is excluded entirely (neither
    absorbs nor is affected). `apply_armor_damage`.
  - **§50.12 no-ammo surrender.** `cmd_anti_armor` now surrenders an out-of-ammo unit fired upon by
    anti-armor (mirroring the close-assault path); ZOC/barrage still don't cause surrender.
  - **§50.2 infantry ammo (erratum).** Organic (rifle) infantry consume 1 Ammo/TOE for assault
    (`unit_combat_ammo_cost` + `expend_defensive_assault_ammo`); the chart's rate-2 stays for weapon
    units. Test `ammo_rates_and_consumption` updated.
  - **§8.37 track/road cost (erratum).** `movement::step_cost` reworked to the chart footnotes: a Road
    negates the hex terrain + hexside features (flat road rate); a Track halves the entered hex's
    terrain cost and hexside crossings (except a down-escarpment). Rainstorm road-as-track (§29.56)
    routes through the same path. Test `track_halves_road_negates_step_cost_8_37`.
  - **§54.17 — retracted (mis-citation).** The deferred note said "Axis-railroad reliability table",
    but §54.17 is the *Supply Dump Demolition Table* and §54.4 (Axis rail) is deterministic (matches
    the rules — no reliability roll). The demolition-table mechanic itself is not modeled (dump
    destruction uses the flat-% §27.55a / §40.64 paths); implementing the §54.17 table is a separate
    unbuilt feature, not an addenda fix.
  - **§62.33 — verified no-op.** "142 Blenheim IV F's should be Blenheim IV's" corrects the *summary*
    force chart only. `air-reinforcements.ron` already classifies Blenheims correctly (≈606 Mk.IV
    bombers vs 64 Mk.IVF fighters — no mislabeled block of 142 fighters), so no data change is needed.
- [x] **H6 — Replacement-Point movement (§20.42/§20.45/§20.46).** `MoveReplacement` (Truck Convoy Phase)
  moves an on-map RP pool one adjacent hex, enforcing: §20.42a no voluntary enemy-ZOC entry unless a
  Friendly unit holds the hex; §20.42d a per-pool CPA budget (`ReplacementPoints.cpa_used`, in tenths,
  reset each Operations Stage) capped at the **assigned unit's** `effective_cpa` — movement requires
  `assigned_to`. **§20.44/§20.46 truck haulage (full coupling):** an **Infantry** pool moves only if a
  co-located Friendly truck convoy has enough §54.2 infantry lift (Σ trucks × per-point capacity ≥ the
  pool's TOE-SP, 1 RP ≈ 1 SP) and movement allowance (`truck_convoy_cpa`) for the step; that convoy
  **travels with the pool** and its Truck Points burn the Fuel. **Tank/Gun/Recce** pools may move on
  their own, costing the Fuel of the minimum Truck Points that would lift them. **§20.45/§49.13 Fuel:**
  `Truck Points × FCF(1) × ⌈CP/5⌉`, drawn at the departure hex from a co-located Friendly Supply Dump
  (§49.15); no per-hex Stores (Stores aren't a movement cost). Added 2026-09-16; test
  `replacement_pool_moves_20_46`. §20 class-matching on absorption was **already** enforced
  (`available_replacement_class`/`draw_replacement_class` filter by `p.class`).
- [x] **H7 — Truck system §53/§54 completion (2026-09-17).** Closed the truck-audit gaps: **§53.24**
  load/off-load costs 2 CP (§21.45) + one-commodity-per-Truck-Point packing (fleet commodity shares
  sum ≤ 1); **§53.14** free-convoy load/unload at dumps (`LoadConvoy`/`UnloadConvoy` + `convoy_cargo_capacity`);
  **§53.22/§21** free-convoy breakdown on move (terrain BP + truck BAR 2L via `resolve_convoy_breakdown`,
  losing Truck Points); **§52.42** Truck-Point water when moved; **§8.93** inherently-motorized "+"
  units (`Cpa.motorized_capable`) motorized without trucks; **§6.17** gun/AA motorization CPA (guns/AA
  lifted by trucks assume the truck gun-CPA). Test `truck_system_additions`.
- [x] **H8 — Truck system final deferred items (2026-09-17).** **§29.35** hot weather doubles a unit's
  water requirement; **§22.13** free-convoy breakdown now sends Truck Points to `TruckConvoy.trucks_broken`
  (recovered by the new `RepairConvoyTrucks` command / `repair_convoy_trucks`) instead of being lost;
  **§53.25** no-leapfrogging — supplies off-loaded by a convoy during a Truck Convoy phase are locked
  (`game.leapfrog_locked`, reset per stage) against re-loading by another convoy that phase. Test
  `truck_deferred_items`. Remaining (minor): §21.6 towing of broken convoy trucks by another unit (field
  repair covers recovery); §53.25 uses a same-phase drop lock rather than literal CP-inheritance accounting.

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
- [-] **AU4 — §10.3/§10.31–36 mandatory attack of ZOC-exerting hexes.** Deferred: a mandatory-combat
  *obligation* (Holding-Off Barrage + §10.36 forced 3-hex retreat/3-DP fallback) reshapes the turn
  structure and the self-play agents' action enumeration — all combat is currently modeled as
  voluntary. High-regression, cross-cutting subsystem better done as a dedicated effort; ZOC/Contact/
  Engaged are computed, only the obligation is missing.
- [x] **AU5 — §18.2 Reserve I/II effects.** §18.22: Reserve II never moves; Reserve I moves one hex/
  stage (`reserve_moved_this_stage`). §18.23/§18.24 release restrictions: releasing a unit from Reserve
  (DesignateReserve→None) sets `Unit.released_from_reserve`, which imposes a hard CPA cap on movement —
  full for Reserve I, half (rounded down) for Reserve II (§18.23.1/§18.24.1) — allows only one offensive
  Close Assault (`made_offensive_assault`, §18.23.2/§18.24.2), and levies the §18.24.3 auto-DP on a
  Reserve-II-released unit's first offensive Close Assault. Tests reserve_ii_may_not_move_18_22,
  reserve_release_cpa_cap_18_24, reserve_release_one_offensive_assault_18_23. (Minor simplifications:
  the §18.24.3 DP fires on the Close-Assault path, not AA/Barrage-only participation; §18.25's "move
  without being within two hexes of an enemy" is a relaxation of the unmodeled §8.23 base rule, moot.)
- [x] **AU6 — §12.44 Pinned** blocks Move/AntiArmor/CloseAssault (currently only RBA/Reaction check it).
- [x] **AU7 — §6.26 cohesion ≤ −26** blocks Move/CloseAssault/AntiArmor + adjacency auto-surrender
  (currently only RBA/Reaction check the −26 gate; no auto-surrender).

### Combat detail (§12–§15, §23, §26)
- [-] **AU8 — §15.9 Probe combat mode.** Deferred: a distinct combat *mode* (new command + agent
  wiring + the §15.95 all-recce −10% / §15.96 no-defender-CP modifiers) that the operational
  abstraction never invokes; a discrete opt-in feature, best as its own effort rather than bolted
  onto the atomic close-assault resolver.
- [-] **AU9 — §15.29 decline-to-defend.** Deferred: tied to the defender-allocation choice model (the
  remaining A3 detail, §15.16 per-TOE defensive commitment). `cmd_close_assault` resolves atomically
  with the *whole* enemy stack defending, so there is no "allocate zero units" trigger to hang the
  auto-retreat-3/3-DP on until defender allocation is modeled.
- [x] **AU10 — §15.23/§14.22 once-per-Combat-Segment per-hex guard.** close_assaulted_hexes_this_stage / anti_armor_hexes_this_stage (reset per stage) refuse a repeat close assault / anti-armor on the same hex. Test hex_once_per_combat_segment_15_23.
- [x] **AU11 — §13.28 a Retreat-Before-Assault unit still eats anti-armor fire (can't return it) and
  its TOE counts for % losses.**
- [x] **AU12 — §15.88/§15.15 assaulted out-of-ammo / ≤−17-cohesion defenders auto-surrender.** cmd_close_assault surrenders such defenders before resolution (en masse if all); the rest defend. Test assaulted_defender_auto_surrenders_15_88.
- [x] **AU13 — §12.1/§12.11–16 Gun Forward/Back declaration** command + its barrage/coordination
  consequences (enum exists, feeds §15.84 vulnerability, but defaults Back with no setter).
- [x] **AU14 — §12.46 barrage rolls the Trucks column** (truck casualties; a hit motorized unit loses
  transporting trucks).
- [x] **AU15 — §12.5/§12.52–55 artillery barrage vs Facilities.** New Command::BarrageFacility{
  barragers,target,facility}. The §41.5 Artillery Barrage Points column was transcribed from PDF
  p107 into air-bombardment.ron (`barrage_bands` = 1-2/3-4/5-6/7-10/11-12/13-14/15-16/17-18/19-20/
  21+) with a new `WeaponPoints::Barrage`. §12.53 structures (Fortification/Major City, Road,
  Railroad) read that column with **Actual** Barrage Points; §12.54 Supply Dumps and Air Facilities
  read the Bomb column with **Raw** Barrage Points. Effects reuse `apply_land_bombing_effect`
  (forts/roads/rails/dumps) and `damage_air_facility` (airfields/strips/ports). Tests
  barrage_against_facility_12_5 + column_selection_41_5 (Barrage scale).
- [x] **AU16 — §23.24 engineer/HQ in a close assault vs a fortified (non-major-city) hex → +1 Assault
  Differential for the attacker.**
- [x] **AU17 — §26.26 minefield combat shift.** Close-assault +1 defender shift (close_assault_env_shift)
  AND the anti-armor column shift: when the firing units sit in an enemy minefield (or fire at a unit
  in one), `resolve_anti_armor_core` shifts the AA column one in the defender's favour (dummy minefields
  excluded; not applied to artillery bombardment). Test anti_armor_minefield_shift_26_26.
- [x] **AU18 — §23.21 reduced enemy-minefield entry cost extends to units stacked with an engineer.** move_step_plan now grants the engineer reduction to a mover co-located with an unpinned engineer-capable unit.

### Morale / organization / reinforcements (§17–§22)
- [x] **AU19 — §17.5/§17.51–56 voluntary surrender of a stack.** New Command::VoluntarySurrender{hex,
  captor,destroy_material}: the whole enemy-of-captor stack surrenders to an adjacent enemy combat
  unit that pays 2 CP (§17.54; CaptorCannotAccept if it can't); optional §17.55/§17.56 pre-capture
  destruction of tanks/guns ((die−1)×10%) and trucks ((die−1)×20%), then surrender_units (§17.51).
  Test voluntary_surrender_17_5.
- [x] **AU20 — §19.8/§19.81–87 ad-hoc Axis AT batteries.** New Command::AugmentAntiTank{hq,gun_system,
  count}: augments a Brigade-level infantry-type Axis HQ with a single AT gun type drawn from
  co-located trained Gun Replacement pools — §19.83 initial ≥3, §19.81 ≤6, §19.86 one type,
  §19.82 all-other-AT-units-≥67% gate (AntiTankReadinessGate); §19.85 the HQ derives the gun's CPA
  in unit_cpa. Fully rules-specified (no OB data needed). Test ad_hoc_axis_anti_tank_battery_19_8.
- [x] **AU21 — §19.9/§19.91–98 CW battalion AT augmentation.** New Command::AugmentBattalionAntiTank
  {battalion,gun_system,count}: from GT 75 a CW infantry battalion with CPA ≥10 and Off/Def Close
  Assault 1/2 or 2/2 (§19.92) may take AT gun TOE — 2 points if the chart `+` motorized-capable flag
  is set, else 1 (§19.94) — drawn from co-located trained Gun Replacement pools, gated by §19.96
  (parent AT regiments at max). Modeled via a new `Unit.anti_tank_augment` field folded into
  raw_strength (a "second weapons system", §19.93) that leaves TOE/shell (§19.98) and CPA (§19.97)
  unchanged. Fully rules-specified. Test commonwealth_battalion_anti_tank_19_9.
- [x] **AU22 — §19.72/§19.73 battle-group forming composition caps** (German ≤4 bn/≤1 tank/≤2 inf;
  Italian ≤3 bn/≤2 inf/≤1 armor, ≤2 in play) — pool exists, limits unenforced.
- [x] **AU23 — §20.5/§20.51–55 upgrade Armored-Car/Recce to newer vehicles.** New Command::UpgradeRecce
  {unit}: an armored Recce/AC unit with Anti-Armor 0 (not a CW recce in an infantry division, §20.51)
  consumes one Tank (Italy: or Recce) Replacement Point from a co-located trained pool (§20.53), adds
  a TOE point if below max (§20.52), and gains per-TOE Anti-Armor 1 / Off Close Assault 3 (§20.54) via
  a new `Unit.recce_upgraded` flag folded into raw_strength. The §20.55 1/39 gate predates the
  campaign (always satisfied). Fully rules-specified. Test upgrade_armored_car_effectiveness_20_5.
- [x] **AU24 — §20.8/§20.81–85 scheduled CW Mandatory Withdrawals.** Schedule transcribed verbatim
  from [4.43a] (PDF p114–115, 2026-09-13) into `data/reference/cw-withdrawal-schedule.ron` (31 WD
  entries: GT/stage, units, Tpt) and loaded into `ReferenceData.cw_withdrawal_schedule`. Mechanics
  done: `Unit.mandatory_withdrawal` deadline + `enforce_mandatory_withdrawals` (§20.82–85 — at
  Cairo/Alexandria ≥75% max TOE → removed from play `status.withdrawn`, §20.84; else eliminated,
  §20.83). Test mandatory_withdrawal_schedule_20_8. **OB binding done:** each ObUnit carries an optional
  `withdrawal: (GT, stage)` stamped onto `Unit.mandatory_withdrawal` at deploy; all 22 *first-time*
  schedule entries are bound across 13 formations in `data/ob/commonwealth.ron`, resolving the
  `[Less …]` division splits and brigade-only withdrawals by exact OB unit (tests
  cw_withdrawal_schedule_bound_to_ob_20_8 + deploy_formation_builds_hierarchy). **Return ("Rtn")
  subsystem done:** new `Unit`/`ObUnit` `return_at`, `return_toe`, `withdrawal_after_return`;
  `process_returns` (wired into begin_operations_stage before the withdrawal sweep) brings a withdrawn
  unit back at Cairo — restored to `return_toe` (else max), un-`withdrawn`, carrying its "[again]"
  re-withdrawal deadline. The 7th Indian Brigade full cycle (WD 14/3 → Rtn 30/2 → WD-again 75/1) and
  11th Indian Brigade (Rtn 30/1) are bound as proof; tests mandatory_withdrawal_return_cycle_20_8 +
  cw_withdrawal_schedule_bound_to_ob_20_8. **All single-cycle returns now bound** across 6 formations
  (4th Indian Div HQ+arty+CIH Rtn28/3, 5th Bde Rtn28/3→WD-again36/1, 7th Bde Rtn30/2→WD-again75/1,
  11th Bde Rtn30/1; 5th Indian Div 9th Bde+HQ+144-Fld Rtn75/3 & 9th Bde WD-again100/2, 10th Bde+28-Fld
  Rtn73/1; 70th Inf Div 16th Bde Rtn50/2→WD-again73/1 and 14th Bde WD40/1→Rtn54/1 and 23rd Bde's
  1 Essex+1 Durham LI WD38/2→Rtn51/3; 50th Inf Div 150th Bde Rtn58/3; 9th Aus Div all Rtn86/3→
  WD-again108/1; 1st Army Tank 42/44 RTR Rtn99/1). Also picked up two withdrawal entries first missed
  in the dense columns (GT40/1 14th Bde, GT38/2 Essex/Durham). **Double-cycle done:** a `return_at_2`
  slot (Unit/ObUnit) lets `process_returns` arm a second return, so the 2nd NZ Division's full cycle
  is now bound — 4th NZ Bde WD25/1→Rtn35/1→WD72/2→Rtn85/2, 6th NZ Bde WD25/2→Rtn31/1 (bns at 2 TOE),
  HQ+support WD25/2→Rtn35/1. **By-type player choices done:** GT23/3 "1st Armored Bde [2]" (the 2nd
  Armored Division is absent from the OB) and GT97/2's "any three infantry battalions" are modeled as
  `WithdrawalChoice`s (campaign.ron) satisfied by `Command::SatisfyWithdrawalChoice` (player picks
  units of the right type at Cairo/Alexandria ≥75% TOE; else flagged overdue at the deadline, §20.83).
  1st Armored is treated as leave-forever + a fresh 32nd Army Tank Bde would arrive (moot — neither is
  in the OB, §20.8's "returns arrive at max, no state carryover" makes this faithful). Tests
  mandatory_withdrawal_return_cycle_20_8, withdrawal_by_type_player_choice_20_81.
- [x] **AU25 — §21.5 capture broken-down vehicles.** New `BrokenVehicle` map marker: a unit's broken
  vehicles (weapons_broken/toe_broken/trucks_broken) are shed as a marker when it is eliminated
  (`shed_broken_vehicles` in surrender_units). §21.52 `capture_broken_vehicles` — a moving
  battalion-or-larger combat unit flips an adjacent unprotected enemy marker to its side; §21.53
  protection (guarded by an owner combat/HQ/Engineer, or in an owner-controlled hex) via
  `broken_vehicle_protected`. Test broken_vehicle_capture_and_tow_21_5.
- [x] **AU26 — §21.6 tow broken-down vehicles.** New Command::TowBrokenVehicle (Repair Phase):
  `tow_broken_vehicle` moves a marker one hex as a Medium Truck within a 10-CP budget (§21.63; 20 CP
  with a co-located Tank Delivery Squadron, §22.65), never into an enemy-controlled hex (§21.64);
  budget reset each stage. Test broken_vehicle_capture_and_tow_21_5.
- [x] **AU27 — §22.6/§22.61–66 CW Tank Delivery Squadrons.** New `Unit.is_tds` unit type. Implemented:
  §22.62 field-repair −1 for a co-located TDS (`hex_has_friendly_tds` → `field_repair_tanks`); §22.63
  CPA 25 (`unit_cpa`), 0 Stacking Points (`effective_stacking_points`), no combat ratings (inherent),
  ZOC-elimination of a lone TDS by an advancing enemy combat unit (`move_counter`) + eight-Game-Turn
  return at Cairo (`process_tds_returns`); §22.64 ≤3-TOE tank reserves usable as co-located Tank
  Replacement Points (`available_/draw_replacement_class`); §22.66 CPA-exceeded repair lockout
  (`tds_repair_locked`, set at stage rollover, suppresses the §22.62 assist). Test
  tank_delivery_squadron_22_6. §22.65 TDS 20-CP towing works via the AU25/AU26 broken-vehicle marker
  model. §22.62 destroyed-tank repair-on-a-1 **done**: `cmd_repair_captured_equipment` now gates a
  Destroyed Tank marker on a Repair Facility (§14.54) or a co-located same-nationality TDS rolling a 1
  (§22.62 — CommandError::RepairFailed otherwise); surrendered §28.3 equipment is unchanged. Test
  destroyed_tank_repair_gate_22_62. **Destroyed-tank towing done:** `CapturedEquipment.tow_cp_used` +
  `tow_destroyed_tank` + Command::TowDestroyedTank move a destroyed-tank marker one hex/Medium-Truck
  within the 10-CP (20-with-TDS) budget to a facility (§14.54/§21.6). **OB placement done:** new
  `ObUnit.is_tds` (stamped at deploy); the three Desert Tank Delivery Squadrons (A/B/C) are in the CW
  OB arriving GT29/2 ([4.43a]). AU27 fully implemented.
- [x] **AU28 — §22.7 German Mobile Tank Repair Squad.** Treated exactly as a TDS (reuses `is_tds` —
  CPA 25, 0 stacking, ZOC-elimination/return, reserves, lockout), but the §22.62 assist and §22.64
  reserves are now matched by **nationality** (not just side), so a German squad repairs only German
  tanks — never Italian or captured Commonwealth (§22.7). Test german_mobile_tank_repair_squad_22_7.
  (Same §22.65 tow / §22.62 destroyed-tank-on-1 sub-rules deferred as for AU27; the single counter's
  OB placement is a scenario follow-on.)

### Movement / stacking / terrain (§8–§9)
- [-] **AU29 — §9.33/§9.34 road/track 5-SP transit limit.** Deferred: presupposes the §9.29 base
  5-point Road/Track Stacking Limit and an explicit "on-the-road / off-road-marker" per-hex state,
  neither of which the current cost-only movement model represents (a road is just a cheap hexside
  crossing; there is no road-bonus/negate-terrain state to lose by "going around"). Implementing the
  transit refinement alone would fabricate a mechanic on top of an absent base rule; needs the road
  stacking + on-road-state model first.
- [-] **AU30 — §8.17 150% CPA hard ceiling.** Deferred: a hard ceiling blocks a single expensive
  step (escarpment/sandstorm) that current cost-mechanics tests rely on, and likely needs a §8.16
  minimum-move exception; low value vs. the §6.21 over-CPA Disorganization already modeled.
- [x] **AU31 — §9.25 ≤ five independent 0-SP companies/batteries per hex** (unlimited in a Major City; attached units ride their parent). Enforced in move_step_plan. Test zero_sp_company_stacking_cap_9_25.
- [x] **AU32 — §9.16 free-stacking exceptions.** Mechanics done in `effective_stacking_points`:
  §9.16a a garrison unit occupies 0 SP while in its assigned hex (new `Unit.garrison_location`);
  §9.16b a *pure* AA/Flak unit (`unit_is_pure_anti_air`) occupies 0 SP in a Major City. Test
  free_stacking_garrison_and_flak_9_16. §9.16b air-facility allowance applied in `hex_stacking_points`
  (up to 3 AA/Flak free at an airfield, 1 at a landing strip). §9.16a garrison binding **done**: the
  rulebook confirms all Italian garrison-formation units carry the "G" and gives every location, so
  the OB now carries a formation `garrison_hex` (Benghazi A4827 / Tobruk C4807 / Bardia C4321 / Derna
  B5925 / Giariabub C1014) plus per-unit `garrison` overrides for the scattered minor garrisons
  (Sceferzen C3419, Fort Maddalenna C3019, EGUG C1715, Benina A4829, Mechili B4921, Soluch A4130),
  stamped onto `garrison_location` at deploy. Test italian_garrison_free_stacking_9_16.
- [x] **AU33 — §8.44/§8.45 Salt Marsh / Desert restrictions.** Fully done — the movement subtypes turned
  out to be derivable from existing data: `unit_is_motorcycle` (the game's only one, German
  Kradschützen, by name), `unit_is_camel` (Meharisti/Sahariano, by name), `unit_is_light_truck_borne`
  (light-only TruckPoints), `is_recce_unit`, `unit_is_afv`; there are no "motorcycle Recce" units
  (light recce are all camel/desert-patrol/Bersaglieri-MG, per C1b). §8.44 assault ban
  (SaltMarshAssault); §8.44 plan_move_step off-track gate (a prohibited vehicle — motorized/AFV not a
  Light-Truck/Recce/motorcycle/camel — needs a Road/Track hexside to enter/leave a Salt Marsh); §8.45
  Desert entry barred to Light Trucks and motorcycle units. Tests salt_marsh_no_vehicle_assault_8_44,
  salt_marsh_and_desert_movement_subtypes_8_44_8_45.
- [-] **AU34 — §8.19 no CW west of Marble Arch (A2109 = col 9).** Deferred: a blanket col-9 gate
  breaks the many synthetic tests that place CW units at low columns; it's a map-convenience
  boundary of little gameplay value on the real map.

### Weather (§29) — rainstorm terrain cluster
- [x] **AU35 — §29.55 rainstorm: wadi hexsides uncrossable except by road; no wadi water draw.**
- [x] **AU36 — §29.56 rainstorm degrades road hexes to track cost/breakdown.**
- [x] **AU37 — §29.57 rainstorm: non-road/rail river hexsides impassable.**
- [x] **AU38 — §29.58 rainstorm bars vehicle/motorized units from delta hexes.**
- [x] **AU39 — §29.42/§29.54 no construction in a sandstorm/rainstorm hex** (BuildRoad/BuildRailroad
  apply no weather check; only field repair is gated).
- [x] **AU40 — §29.32 construction sites need water rations in hot weather.**

### Air ↔ land, logistics, naval (§35, §49–56)
- [x] **AU41 — §35.12 SGSU overrun by advancing land units.** move_counter now eliminates an enemy
  SGSU a mover ends adjacent to when no friendly combat unit is stacked with it (it can't react
  during the enemy move); location→None (redeployable later, §35.13).
- [x] **AU42 — §35.27 grounded planes captured.** An overrun SGSU's grounded planes are now captured
  by the overrunning side into `GameState.captured_planes` (not merely destroyed) and become usable
  one Game-Turn later: `available_captured_planes(side)` reports the ready pool and
  `claim_captured_planes(side, squadron)` moves them into one of the captor's squadrons (cross-
  nationality, per §35.27 overriding §35.28). Test extended in sgsu_overrun_by_land_35_12.
- [x] **AU43 — §49.19/§50.16/§51.16 captured supply dumps** change ownership on overrun + captured-usable
  fraction (fuel non-denominational; ⅓ ammo, ½ stores) — dumps currently never change hands.
- [-] **AU44 — §56.16 convoy flak fires back.** The flak-point formula exists (rules::naval::convoy_flak_points); applying the losses to the attacking bombers needs a squadron ref, but the convoy-bombing command is numeric/abstracted — within the owner-ruled air-combat abstraction (B6–B8).
- [-] **AU45 — §55.17 Bizerta annual 2d6=12 roll.** Deferred: the port's `available_from_gt` isn't
  wired into runtime `Port::is_operable` (which is just efficiency>0), so both the GT-35 gate and the
  annual roll need a Port open-flag set at turn start; low impact (Tunis-box late-war edge port).
- [-] **AU46 — §41.81/§41.84/§41.86 CW bombing of the Axis Tripoli/Tunisia off-map boxes.** Deferred:
  the §8.8 off-map complex is modeled as transit *nodes* (name/transit/direction), not facilities with
  a port level or grounded-plane state to reduce. Needs an off-map-box facility model (port level +
  based-plane state) before the bombing effect (incl. night-vs-Tripoli) has anything to act on.
- [-] **AU47 — §30.5/§30.51–59 CW port-to-port naval troop transport.** Deferred (modeling gap, NOT
  missing data): the Port Personnel Capacity (§30.54/§30.59) is already digitized as
  `PortSpec.stacking_in`/`stacking_out` in `port-capacity.ron`. The blocker is that the engine keys
  ports by **name** (`ports: HashMap<String, Port>`) with no general port-name↔hex association (the
  map only marks a nameless `Feature::Port`; only San Giorgio carries an ad-hoc hex↔port link), so a
  unit transfer between port *hexes* can't resolve which port (and capacity) a hex is. Needs a
  port↔hex mapping (+ scenario port-hex data) before §30.51–58 can be located; the capacity math is
  then trivial.
- [x] **AU48 — §30.4/§30.41–46 Italian Chariot raid on Alexandria.** New Command::PlanChariotRaid +
  ExecuteChariotRaid (Axis). §30.41 not before July 1941 (`date_of`); §30.42 executes on the 7th
  Operations Stage after planning (`global_ops_stage`); §30.45 once per game; §30.46 Chariot Raid
  Table (from the Desktop capture: die 1→3 dice, 2–3→2, 4–5→1, 6→no effect) → the dice total is
  Damage Points applied to Commonwealth ships at Alexandria (§30.34 `Ship::damage`, concentrated on
  the strongest). Test chariot_raid_30_4.
- [-] **AU49 — §35.22 in-play plane transfer between squadrons.** Deferred: a faithful transfer needs
  the §35.21 same-class rule, the §35.23 capacity cap, and the §35.22 "transfer mission" (fly to the
  new base unless co-located) with its in-transit grounding — a real air-management subsystem for a
  niche action the operational abstraction never issues. Better as a dedicated effort.
- [-] **AU50 — §35.28 Italian squadrons all-German planes.** Deferred: plane groups key by aircraft-type string, not per-plane nationality, so enforcing it needs a cross-chart nationality lookup for a minor historical edge.
- [-] **AU51 — §53.31/§53.32 trucks/guns off-track in salt-marsh auto-abandoned.** Deferred: hinges on
  the Salt Marsh terrain subtype + "on a road/track" per-unit state that AU33 (§8.44/§8.45, INPUT
  NEEDED) has to establish first; a Qattara corner case with nothing to trigger it until then.
- [-] **AU52 — §53.25 leapfrogging prohibition.** Deferred: requires per-cargo CP *provenance* (the
  supplies/men remember the CP their first truck expended and cap at that truck's CPA), which the
  abstracted truck-convoy movement doesn't track; needs a cargo-provenance model.
- [-] **AU53 — §28.18 prisoners by truck.** Deferred: the rule is underspecified ("if you are feeling profligate with your transport" — no capacity/CP mechanics), and the foot path (§28.13) is modeled; low value.

### Scope — won't-do / out of scope (excluded from "address them all")
- [-] **§32 Abstract Logistics** (+ §47/§58 abstract air/naval) — a whole *alternate* simplified
  ruleset; the engine implements the full §34–56 games instead. Deliberate either/or.
- [-] **Scenario-group victory conditions** §60.82 / §61.8 / §62.8 / §63.91 / §63.92 — absent because
  those scenarios aren't built as playable setups (only §60.81 Scenario 1 and §64.7 campaign exist).
  Land the scenarios first; the victory evaluators come with them.
- [-] **§64.53/§64.54 "Desert Fox" shorter-campaign reduced pools; §59.36 first-OpStage maintenance ban;
  §63.5 El Alamein special-rule toggles** — tied to unbuilt scenario setups.
