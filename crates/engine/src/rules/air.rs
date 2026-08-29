//! Air Game — Flight (§37). Pure rules/logic: range and transfer distance, the fly-capacity caps
//! (§37.24), and the emergency-flight (§37.33) and crash-landing (§37.23) die-roll outcomes.
//!
//! Flight is operational: a plane is assigned to fly from its base to a target hex along any path
//! whose hex count does not exceed its range (§37.12). Fuel/refit gating (§37.15–16) and the
//! sandstorm/rainstorm path prohibition (§37.22) are applied where flight is executed; this module
//! provides the range/eligibility arithmetic and the random outcomes.

use crate::reference::aircraft::AircraftClass;
use crate::reference::squadron::SquadronCapacity;
use crate::reference::units::Nationality;

/// May a plane whose mission capabilities are `missions` be assigned `mission`, by day or by night
/// (§39.15/§39.42/§39.43)? Transfer is available to any plane (§39.18). For all other missions the
/// matching chart column must permit it:
/// - `No` — never;
/// - `Day` / `DayStrafeArmor` / `StrafeOnly` — day only (no night, §39.42);
/// - `Night` — day or night;
/// - `NightScrambleOnly` — only a night Scramble (§4.44c `*` for Bf.110 / Ju.88C).
///
/// Offensive CAP and Scramble are the two missions a night-capable plane may fly at night without a
/// specifically-assigned night mission (§39.43); a `Night` capability already covers those here.
pub fn can_perform(
    missions: &crate::reference::aircraft::Missions,
    mission: crate::reference::aircraft::Mission,
    night: bool,
) -> bool {
    use crate::reference::aircraft::{Mission, MissionCap};
    if mission == Mission::Transfer {
        return true;
    }
    match missions.cap_for(mission) {
        MissionCap::No => false,
        MissionCap::Night => true,
        MissionCap::Day | MissionCap::DayStrafeArmor | MissionCap::StrafeOnly => !night,
        MissionCap::NightScrambleOnly => night && mission == Mission::Scramble,
    }
}

/// The one-way flight-range limit in hexes (§37.11/§37.21). A normal mission uses the plane's
/// listed (loadout) range; a transfer mission — and an emergency flight, which is a transfer
/// mission (§37.31/§37.34) — doubles it for the one-way trip.
pub fn one_way_range(listed_range: u32, transfer: bool) -> u32 {
    if transfer {
        listed_range * 2
    } else {
        listed_range
    }
}

/// May a plane with `listed_range` reach a target `distance` hexes away (§37.12/§37.21)? Planes
/// may never exceed their range in one direction (double for transfer/emergency).
pub fn can_fly(listed_range: u32, distance: u32, transfer: bool) -> bool {
    distance <= one_way_range(listed_range, transfer)
}

/// §37.24: at a given air facility, no more squadrons may ready and fly than the field's current
/// (functional) capability level, regardless of how many SGSUs are present — the excess stay
/// inactive. Returns how many of `sgsus_present` may operate.
pub fn active_squadrons(sgsus_present: u32, facility_capacity: u32) -> u32 {
    sgsus_present.min(facility_capacity)
}

/// §37.24: a squadron may send no more planes on a mission than its ready capacity (§35.23),
/// regardless of how many ready planes it holds as reserves.
pub fn max_planes_on_mission(ready_planes: u32, capacity: SquadronCapacity) -> u32 {
    ready_planes.min(capacity.ready)
}

/// §37.33 emergency-flight escape roll (one die per plane): a fighter or fighter-bomber escapes on
/// a 1–3; a bomber, recon or transport (and any other class — dive-bombers are bombers §34.34,
/// flying-boats fall here too) escapes on a 1–2. Any higher roll leaves the plane grounded.
pub fn emergency_flight_escapes(class: AircraftClass, die: u8) -> bool {
    let max_escape = match class {
        AircraftClass::Fighter | AircraftClass::FighterBomber => 3,
        _ => 2,
    };
    (1..=max_escape).contains(&die)
}

/// Outcome of a §37.23 crash landing (when a plane cannot return to base and cannot land at any
/// facility).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashLanding {
    /// First die 1–3: plane and pilot survive — recovered next OpStage, ready the following one.
    PlaneAndPilotSurvive,
    /// First die 4–6, second die 1–2: plane destroyed, pilot survives and returns one Game-Turn
    /// later.
    PlaneLostPilotSurvives,
    /// First die 4–6, second die 3–6: plane and pilot both lost.
    PlaneAndPilotLost,
}

/// Resolve a §37.23 crash landing from two die rolls. `second_die` is only consulted when the
/// first is 4–6 (plane does not survive), so a dummy value may be passed when `first_die` ≤ 3.
pub fn crash_landing(first_die: u8, second_die: u8) -> CrashLanding {
    if (1..=3).contains(&first_die) {
        CrashLanding::PlaneAndPilotSurvive
    } else if (1..=2).contains(&second_die) {
        CrashLanding::PlaneLostPilotSurvives
    } else {
        CrashLanding::PlaneAndPilotLost
    }
}

/// §38.35 refit die-roll modifier (used with the §38.37 "By Squadron" table): the refitting
/// nationality contributes +1 (German) or +2 (Italian) — Commonwealth adds nothing — and refitting
/// planes not assigned to the SGSU doing the work adds a further +1. Modifiers are cumulative.
pub fn refit_die_modifier(refitting_nationality: Nationality, wrong_squadron: bool) -> i32 {
    let mut m = match refitting_nationality {
        Nationality::German => 1,
        Nationality::Italian => 2,
        Nationality::Commonwealth => 0,
    };
    if wrong_squadron {
        m += 1;
    }
    m
}

/// §38.37 (rule): after a refit, 20% (rounded up) of the refitted planes sitting in a sandstorm
/// hex must be refitted again before they may fly. Returns that count.
pub fn sandstorm_refit_loss(refitted: u32) -> u32 {
    (refitted * 20).div_ceil(100)
}

/// §38.21: the Fuel Points needed to refuel one plane for a mission is its loadout's Fuel
/// Consumption Rating. (§37.16: flying any distance then consumes all of it.)
pub fn refuel_cost(loadout: &crate::reference::aircraft::Loadout) -> u32 {
    loadout.fuel
}

/// §38.43: a squadron needs one Ammunition Point for its planes to use their TacAir Rating in a
/// stage; without it each plane's TacAir counts as zero and may not fire at Enemy planes (§46.21).
pub const SQUADRON_TACAIR_AMMO: u32 = 1;

/// Ammunition Points to arm one plane's ordnance: one Point covers all bombs loaded regardless of
/// tonnage (§38.44), and loading mines or torpedoes likewise costs one Point (§38.47). The
/// per-squadron TacAir Point (`SQUADRON_TACAIR_AMMO`, §38.43) is separate. A plane's bombload is
/// expended on any mission it flies, used or not (§38.45).
pub fn plane_ordnance_ammo(carries_bombs: bool, carries_mine_or_torpedo: bool) -> u32 {
    u32::from(carries_bombs) + u32::from(carries_mine_or_torpedo)
}

/// §39.35 voluntary abort for a fighter (or fighter-bomber): it may abort if its Maneuver Rating
/// is no more than 10 points below the best Enemy CAP fighter's (`own - enemy >= -10`). With no
/// Enemy CAP present a fighter may always abort (exception §40.26, handled elsewhere).
pub fn fighter_may_abort(own_maneuver: i32, best_enemy_cap_maneuver: Option<i32>) -> bool {
    match best_enemy_cap_maneuver {
        None => true,
        Some(enemy) => own_maneuver - enemy >= -10,
    }
}

/// §39.37 sufficient fighter screen: the count of Friendly escort fighters (planes, not ratings)
/// must be at least 25% of the Enemy fighters flying CAP in the hex. (12 escorts screen 40 CAP but
/// not 50.) Screening fighters may not themselves abort, and fighters cannot screen fighters.
pub fn sufficient_fighter_screen(friendly_fighters: u32, enemy_cap_fighters: u32) -> bool {
    friendly_fighters * 4 >= enemy_cap_fighters
}

/// §39.36 voluntary abort for a non-fighter: allowed only with a sufficient fighter screen
/// (§39.37), or — unlikely — if its own Maneuver exceeds the best Enemy fighter's.
pub fn nonfighter_may_abort(
    sufficient_screen: bool,
    own_maneuver: i32,
    best_enemy_fighter_maneuver: Option<i32>,
) -> bool {
    if sufficient_screen {
        return true;
    }
    match best_enemy_fighter_maneuver {
        None => true,
        Some(enemy) => own_maneuver > enemy,
    }
}

/// §39.44: planes flying a night mission may not be intercepted and may not engage in air-to-air
/// combat, and non-fighters gain no Formation-Flying benefit at night. This reports whether the
/// two air-to-air interactions are permitted against/for a night-mission plane.
pub fn night_mission_air_to_air_allowed() -> bool {
    false
}

/// §39.44: night-mission planes may not be intercepted.
pub fn night_mission_may_be_intercepted() -> bool {
    false
}

// ---- Pilots (§40.1) ----

/// §40.15: the valid non-zero Pilot Ratings. There is deliberately no 5; the 6 represents the ace
/// Hans-Joachim Marseille. A plane flown without an assigned pilot uses a zero-rated pilot (§40.16).
pub const PILOT_RATINGS: [u32; 5] = [1, 2, 3, 4, 6];

/// §40.14: training a pilot onto a new plane type takes one full Game-Turn — three Operations
/// Stages — before he may fly.
pub const PILOT_TRAINING_STAGES: u32 = 3;

/// §40.17: a recovered (bailed-out) pilot may fly again three Operations Stages after recovery.
pub const PILOT_RECOVERY_STAGES: u32 = 3;

/// §40.15: a plane's air-to-air TacAir Rating is its base TacAir plus the assigned pilot's Rating
/// (zero if no pilot is assigned, §40.16).
pub fn air_to_air_tacair(plane_tacair: i32, pilot_rating: u32) -> i32 {
    plane_tacair + pilot_rating as i32
}

/// §40.18 optional ace rule: with all pilots starting at Rating 0, a pilot gains one Rating point
/// per five air-to-air kills (no cap). Returns the Rating implied by `kills`.
pub fn ace_rating_from_kills(kills: u32) -> u32 {
    kills / 5
}

// ---- Combat Air Patrol (§40.2) ----

/// §40.21: only planes with F (fighter) capability may fly CAP.
pub fn can_fly_cap(missions: &crate::reference::aircraft::Missions) -> bool {
    missions.cap != crate::reference::aircraft::MissionCap::No
}

/// §40.23: offensive CAP with three or more fighters in the hex projects an air Zone of Control —
/// the hex plus its six neighbours. Defensive CAP never has an air ZOC (§40.26).
pub fn cap_has_air_zoc(cap: crate::reference::aircraft::CapType, fighter_count: u32) -> bool {
    matches!(cap, crate::reference::aircraft::CapType::Offensive) && fighter_count >= 3
}

/// §40.26: only offensive CAP may initiate air-to-air combat.
pub fn cap_may_initiate(cap: crate::reference::aircraft::CapType) -> bool {
    matches!(cap, crate::reference::aircraft::CapType::Offensive)
}

/// §40.26: offensive CAP *must* engage enemy planes entering its hex/ZOC to let them perform a
/// mission there (only one such hex need be attacked); defensive CAP may decline.
pub fn cap_must_engage(cap: crate::reference::aircraft::CapType) -> bool {
    matches!(cap, crate::reference::aircraft::CapType::Offensive)
}

/// §40.26: defensive CAP may decline combat and abort; offensive CAP may not abort.
pub fn cap_may_abort(cap: crate::reference::aircraft::CapType) -> bool {
    matches!(cap, crate::reference::aircraft::CapType::Defensive)
}

/// The seven hexes of an offensive-CAP air Zone of Control (§40.23): the patrolled `center` hex and
/// its six neighbours. (Only meaningful when [`cap_has_air_zoc`] holds.)
pub fn air_zoc_hexes(center: crate::state::map::HexCoord) -> Vec<crate::state::map::HexCoord> {
    let mut hexes = vec![center];
    hexes.extend(center.neighbors());
    hexes
}

/// §40.27: fighters on offensive CAP may intercept only one mission per Operations Stage.
pub const OFFENSIVE_CAP_INTERCEPTS_PER_STAGE: u32 = 1;

// ---- Scramble (§40.3) ----

/// §40.31: a plane may be assigned a scramble mission only if it has scramble capability. Reuses
/// the day/night mission eligibility (a `NightScrambleOnly` plane may scramble only at night).
pub fn can_scramble(missions: &crate::reference::aircraft::Missions, night: bool) -> bool {
    can_perform(missions, crate::reference::aircraft::Mission::Scramble, night)
}

/// §40.33: a scramble path may not pass through any hex — other than the target hex — that contains
/// other Enemy fighters. `path` excludes the base hex and includes the target as its last element;
/// `enemy_fighter_hex` reports whether a given hex holds enemy fighters.
pub fn scramble_path_clear(
    path: &[crate::state::map::HexCoord],
    target: crate::state::map::HexCoord,
    enemy_fighter_hex: impl Fn(crate::state::map::HexCoord) -> bool,
) -> bool {
    path.iter().all(|&h| h == target || !enemy_fighter_hex(h))
}

// ---- Strafing (§40.5–40.8) ----

/// The target types that may be strafed (§40.54).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrafeTarget {
    /// Infantry-type units (battalions/companies).
    Infantry,
    /// Trucks in a Convoy.
    TrucksInConvoy,
    /// First-line (attached) trucks of a parent unit.
    FirstLineTrucks,
    SupplyDump,
    GroundedAircraft,
    /// Tanks — only Hurricane IID's may strafe them (§40.65).
    Tanks,
    Port,
    WaterPipeline,
}

/// Encode a two-dice roll as `10*d1 + d2` (each die 1–6), the monotonic form the Strafing Table's
/// 11..66 entries use.
pub fn encode_2d6(d1: u32, d2: u32) -> u32 {
    d1 * 10 + d2
}

/// §40.8 column shift for a strafing attack: +1 column right for Trucks-in-Convoy or a Supply Dump;
/// Infantry shifts left 1 in a Fortification Level 0/1 hex and 2 in a Level 2 hex; Armor shifts left
/// 1 in a Level 2 hex. (§40.62 leaves the shift for first-line attached trucks unstated, so none is
/// applied here.) Positive = right/stronger, negative = left/weaker.
pub fn strafe_column_shift(target: StrafeTarget, fort_level: u32) -> i32 {
    match target {
        StrafeTarget::TrucksInConvoy | StrafeTarget::SupplyDump => 1,
        StrafeTarget::Infantry => {
            if fort_level >= 2 {
                -2
            } else {
                -1
            }
        }
        StrafeTarget::Tanks if fort_level >= 2 => -1,
        _ => 0,
    }
}

/// §40.67: a Strafing Table result of 2 or 3 destroys a water pipeline.
pub fn pipeline_destroyed_by_strafe(result: u32) -> bool {
    result >= 2
}

/// §40.64: a Strafing Table result against a supply dump destroys `result * 10` percent of all
/// supplies in the dump.
pub fn supply_dump_percent_destroyed(result: u32) -> u32 {
    result * 10
}

/// §40.66: strafing a port is folded into a bombing raid — half the strafing TacAir Points (round
/// down) are added to the bombload on the Air Bombardment Table (§41.5). Returns that bonus.
pub fn port_strafe_bombload_bonus(strafe_tacair: u32) -> u32 {
    strafe_tacair / 2
}

/// §40.51: a plane may strafe if it has strafing capability — either the F (CAP-or-strafe) column
/// or the D (strafe-and/or-bomb) column is present.
pub fn can_strafe(missions: &crate::reference::aircraft::Missions) -> bool {
    use crate::reference::aircraft::MissionCap;
    missions.cap != MissionCap::No || missions.strike != MissionCap::No
}

// ---- Fighter flak suppression (§40.7) & night fighters (§40.9) ----

/// The kinds of anti-aircraft points, for §40.74 suppression eligibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AaType {
    Light,
    Heavy,
    Ship,
}

/// §40.74: for every three fighters (individual planes) flying flak suppression, one point of
/// AA/flak fire is neutralised for the Operations Stage.
pub fn flak_points_suppressed(fighter_count: u32) -> u32 {
    fighter_count / 3
}

/// §40.74: only Light and Ship AA points may be suppressed by fighters; Heavy AA may not.
pub fn aa_suppressible_by_fighters(aa: AaType) -> bool {
    !matches!(aa, AaType::Heavy)
}

/// §40.91: a night fighter scrambling against a night bombing mission adds two to its scramble die
/// roll (making success harder on the §40.4 table).
pub const NIGHT_FIGHTER_SCRAMBLE_DIE_MODIFIER: u32 = 2;

/// §40.92: these night fighters have their Maneuver Rating lowered by 4 points when used by day.
pub const NIGHT_FIGHTERS_WITH_DAY_PENALTY: [&str; 3] = ["Blenheim Mk.IVF", "Bf.110", "Ju.88c"];

/// §40.92 day Maneuver penalty for the listed night fighters (0 for any other aircraft).
pub fn night_fighter_day_maneuver_penalty(aircraft_type: &str) -> i32 {
    if NIGHT_FIGHTERS_WITH_DAY_PENALTY.contains(&aircraft_type) {
        -4
    } else {
        0
    }
}

/// §40.93: planes on offensive CAP have no air Zone of Control at night (overriding §40.23).
pub fn offensive_cap_has_zoc_at_night() -> bool {
    false
}

// ---- Bombing missions (§41) ----

/// §41.0: any plane with a Bombload Capacity greater than zero may bomb.
pub fn can_bomb(loadout: &crate::reference::aircraft::Loadout) -> bool {
    loadout.bombload > 0
}

/// §41.11: bombers may not scramble and have no air Zone of Control.
pub fn bombers_may_scramble() -> bool {
    false
}
pub fn bombers_have_air_zoc() -> bool {
    false
}

/// §41.17: torpedoes may be used only against ships and ports (§41.75 restricts them to
/// "T"-capability planes; §41.74 notes torpedoes vs ports count as normal bombs).
pub fn torpedo_valid_target(mission: crate::reference::aircraft::BombingMission) -> bool {
    use crate::reference::aircraft::BombingMission;
    matches!(mission, BombingMission::Ships | BombingMission::Port)
}

/// §41.45: night bombing halves the delivered bombload tonnage, rounded down.
pub fn night_bombload(tons: u32) -> u32 {
    tons / 2
}

/// §41.45: torpedoes may not be used at night.
pub fn torpedoes_allowed_at_night() -> bool {
    false
}

/// §41.48: flak/AA fire against a night mission is adjusted one column to the left (and is
/// ineffective if already in the lowest column). Returns the column shift.
pub fn night_flak_column_shift() -> i32 {
    -1
}

/// §41.47 night search: a CAP/scrambling squadron finds the bombers on a die roll of 1; night
/// fighters subtract 2 from the roll. Returns whether the bombers are found.
pub fn night_search_finds(die: u32, night_fighter: bool) -> bool {
    die.saturating_sub(if night_fighter { 2 } else { 0 }) <= 1
}

/// §41.73: when Commonwealth planes bomb an Axis convoy and at least half carry torpedoes, the
/// effective bombing tonnage is increased by 25% (rounded up).
pub fn cw_convoy_torpedo_tonnage(tons: u32, torpedo_planes: u32, total_planes: u32) -> u32 {
    if total_planes > 0 && torpedo_planes * 2 >= total_planes {
        (tons * 125).div_ceil(100)
    } else {
        tons
    }
}

/// §41.31: for anti-personnel bombing pins, each tank or gun (artillery/anti-tank) unit counts as
/// two battalion-equivalent units; other units count as one.
pub fn pin_unit_weight(is_tank_or_gun: bool) -> u32 {
    if is_tank_or_gun {
        2
    } else {
        1
    }
}

/// §41.91: a Pinned ("A" Player) unit may voluntarily use only half its CPA (rounded down) for the
/// Operations Stage (it also may not voluntarily enter/leave an enemy ZOC or initiate combat).
pub fn pinned_cpa(cpa: u32) -> u32 {
    cpa / 2
}

/// The Air Bombardment CRT target row-group a bombing mission is resolved on (§41.3). Mining
/// harbors is not on this table (§41.39A uses the Mining Harbors Table) and returns `None`.
pub fn bombing_target_group(
    mission: crate::reference::aircraft::BombingMission,
) -> Option<crate::reference::air_bombardment::TargetGroup> {
    use crate::reference::aircraft::BombingMission as M;
    use crate::reference::air_bombardment::TargetGroup as G;
    Some(match mission {
        M::Personnel | M::TruckConvoy | M::FlakDestruction | M::Ships => G::Units,
        M::SupplyDump => G::SupplyDump,
        M::AirFacility | M::Port => G::AirfieldPortStrip,
        M::CityFortification => G::Fortification,
        M::Road => G::Road,
        M::Railroad => G::Railroad,
        M::MiningHarbor => return None,
    })
}

/// §41.5 key modifier: subtract one (floored at zero) from a Land Support Bombing result when the
/// target is armor units.
pub fn bombing_result_vs_armor(result: i32, target_is_armor: bool) -> i32 {
    if target_is_armor {
        (result - 1).max(0)
    } else {
        result
    }
}

/// §41.5 key: an Air Landing Strip receiving a result of 1 or greater is eliminated.
pub fn air_landing_strip_eliminated(result: i32) -> bool {
    result >= 1
}

/// §41.36: bombing an air facility removes 10% of the grounded planes per capacity level destroyed
/// (rounded down). Returns the number of planes lost.
pub fn air_facility_planes_lost(levels_reduced: i32, planes_on_ground: u32) -> u32 {
    if levels_reduced <= 0 {
        return 0;
    }
    planes_on_ground * (levels_reduced as u32) * 10 / 100
}

/// §41.35: bombing a supply dump also costs one Truck Point per 10% of supplies destroyed (for any
/// unattached trucks in the hex). Returns that truck loss for a `percent_destroyed` result.
pub fn supply_dump_truck_loss(percent_destroyed: i32) -> u32 {
    (percent_destroyed.max(0) as u32) / 10
}

/// §41.39A: harbor-mining is rolled once for each group of six bombers attempting to lay mines.
/// Returns the number of mine-laying die rolls a force of `bombers` planes gets.
pub fn mining_attempts(bombers: u32) -> u32 {
    bombers / 6
}

// ---- Air-to-air combat (§45) ----

/// §45.36 / §45.0(b): a bomber's TacAir gains a formation-flying bonus — +1 with 6–17 bombers on
/// the mission, +2 with 18 or more. Dive-bombers are not eligible.
pub fn formation_tacair_bonus(bomber_count: u32, is_dive_bomber: bool) -> i32 {
    if is_dive_bomber {
        0
    } else if bomber_count >= 18 {
        2
    } else if bomber_count >= 6 {
        1
    } else {
        0
    }
}

/// §45.0 / §45.12: only a plane with a non-parenthesized TacAir (i.e. not `no_air_to_air`) may
/// *initiate* air-to-air combat. (Offensive CAP additionally *must* initiate, §45.14 / §40.26.)
pub fn may_initiate_air_to_air(no_air_to_air: bool) -> bool {
    !no_air_to_air
}

/// The side that is the Attacker in an air-to-air engagement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attacker {
    A,
    B,
}

/// §45.16: the Player with the most fighters (raw plane count) is the Attacker; on a tie the Player
/// with initiative this Game-Turn attacks. `a_has_initiative` breaks the tie.
pub fn air_combat_attacker(fighters_a: u32, fighters_b: u32, a_has_initiative: bool) -> Attacker {
    if fighters_a > fighters_b {
        Attacker::A
    } else if fighters_b > fighters_a {
        Attacker::B
    } else if a_has_initiative {
        Attacker::A
    } else {
        Attacker::B
    }
}

/// §45.15: a plane on a mission other than CAP is screened when it has friendly defensive-CAP
/// escort; a non-CAP plane with no escort is not screened, and CAP planes are never "screened".
pub fn is_screened(mission_is_cap: bool, has_escort: bool) -> bool {
    !mission_is_cap && has_escort
}

/// §45.0 fighter-vs-fighter fire order: the plane with the "minus" (lower) TacAir differential
/// fires first. On a 0 differential the higher-rated pilot fires first, ties going to the Attacker.
/// Returns whether *own* plane fires first. (Exceptions handled by the caller: a lone defender
/// ganged by several attackers fires first, §45.0; and in fighter-vs-non-fighter the non-fighter
/// always fires first — see [`nonfighter_fires_first`].)
pub fn fighter_fires_first(
    own_diff: i32,
    own_pilot: u32,
    opp_pilot: u32,
    own_is_attacker: bool,
) -> bool {
    if own_diff != 0 {
        own_diff < 0
    } else if own_pilot != opp_pilot {
        own_pilot > opp_pilot
    } else {
        own_is_attacker
    }
}

/// §45.0 / §45.34: in fighter-vs-non-fighter combat the non-fighter (defending/screened) plane's
/// attack is always resolved first.
pub fn nonfighter_fires_first() -> bool {
    true
}

/// Outcome of the §45.63 recovery roll for a shot-down *fighter* (one die).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FighterLossRecovery {
    /// Roll 1: it wasn't really shot down — plane and pilot limp home.
    PlaneAndPilotReturn,
    /// Roll 2: the pilot makes it back but the plane is lost.
    PilotOnly,
    /// Roll 3–6: plane lost; pilot survival is a separate §45.66 bail-out roll.
    PlaneLost,
}

/// §45.63: resolve a shot-down fighter's recovery from one die.
pub fn fighter_loss_recovery(die: u32) -> FighterLossRecovery {
    match die {
        1 => FighterLossRecovery::PlaneAndPilotReturn,
        2 => FighterLossRecovery::PilotOnly,
        _ => FighterLossRecovery::PlaneLost,
    }
}

/// §45.62: a plane strafed on the ground is repaired on a die roll of 1 or 2 (supplies — 1 Stores +
/// 1 Fuel — are spent only on success); it may fly one full Game-Turn later.
pub fn strafed_plane_repaired(die: u32) -> bool {
    die <= 2
}

/// §45.66: a shot-down plane's pilot bails out and is recovered on a 1 or 2 (ready three Operations
/// Stages later, `PILOT_RECOVERY_STAGES`); otherwise the pilot is killed.
pub fn pilot_bails_out(die: u32) -> bool {
    die <= 2
}

// ---- Anti-aircraft fire (§46) ----

/// §46.0/§46.11: a unit's anti-aircraft points = its TOE Strength × its AA rating (there is no
/// dividing by ten as with other strengths).
pub fn aa_points(toe_strength: u32, aa_rating: u32) -> u32 {
    toe_strength * aa_rating
}

/// §46.26: sort a plane into its anti-aircraft target group. Strafing/dive-bombing planes form
/// group (e); other fighter-type missions (CAP/scramble/interception) form (c) by day, (d) by
/// night; non-fighter missions form (a) by day, (b) by night.
pub fn aa_target_group(
    is_fighter_mission: bool,
    is_strafe_or_divebomb: bool,
    night: bool,
) -> crate::reference::aircraft::AaTargetGroup {
    use crate::reference::aircraft::AaTargetGroup as G;
    if is_strafe_or_divebomb {
        G::StrafeDiveBomb
    } else if is_fighter_mission {
        if night {
            G::NightFighter
        } else {
            G::DayFighter
        }
    } else if night {
        G::NightOtherMission
    } else {
        G::DayOtherMission
    }
}

/// §46.16/§46.22: infantry, recce and tank units may fire AA only at the strafing/dive-bombing
/// target group (e). Pure flak units and air-facility AA may fire at any group.
pub fn ground_combat_aa_allowed(target: crate::reference::aircraft::AaTargetGroup) -> bool {
    matches!(target, crate::reference::aircraft::AaTargetGroup::StrafeDiveBomb)
}

/// §46.24: planes flying flak-suppression missions and those flying naval-convoy reconnaissance may
/// not be attacked by anti-aircraft fire. Returns whether a plane is a valid AA target.
pub fn aa_targetable(is_flak_suppression: bool, is_convoy_recon: bool) -> bool {
    !is_flak_suppression && !is_convoy_recon
}

/// §46.17: pure AA/flak units stack freely (do not count against stacking limits) in major cities;
/// one may stack free in an air landing strip hex and three in an airfield hex.
pub const PURE_AA_FREE_STACK_AIRFIELD: u32 = 3;
pub const PURE_AA_FREE_STACK_LANDING_STRIP: u32 = 1;

// ---- Non-combat missions (§42) ----

/// §42.14: a plane need not be refitted to fly a transfer, but a plane that has *completed* a
/// transfer must be refitted before flying any other mission next Stage/Turn.
pub fn transfer_requires_refit_afterward() -> bool {
    true
}

/// §42.21: a Recon mission may be flown by any plane with Recon (R) capability.
pub fn can_recon(missions: &crate::reference::aircraft::Missions) -> bool {
    missions.recon != crate::reference::aircraft::MissionCap::No
}

/// §42.24b: when revealing a reconnoitered unit's TOE strength the owner may misstate it by up to
/// this deviation in either direction.
pub const RECON_TOE_LIE_DEVIATION: u32 = 2;

/// §42.24a: a dummy tank unit counts as a TOE Strength of three for recon-reveal purposes.
pub const DUMMY_TANK_RECON_TOE: u32 = 3;

/// §42.24a: the order in which unit types must be revealed to a successful recon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconRevealOrder {
    Tanks,
    Infantry,
    Artillery,
}

/// §42.25: no Recon mission may be flown over a hex that is undergoing air bombardment.
pub fn recon_allowed(hex_under_air_bombardment: bool) -> bool {
    !hex_under_air_bombardment
}

/// §42.37: the unit categories that may be carried on a Transport mission — infantry without
/// trucks, motorcycle infantry/recce, and airborne units (nothing else).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirCargo {
    InfantryNoTrucks,
    MotorcycleInfantryOrRecce,
    Airborne,
    Other,
}

/// §42.37: may this cargo category be air-transported?
pub fn can_air_transport(cargo: AirCargo) -> bool {
    !matches!(cargo, AirCargo::Other)
}

/// §42.34: personnel delivered by a Transport mission may not voluntarily move (they have spent
/// their CPA) in the Operations Stage they are transported in.
pub fn transported_personnel_may_move() -> bool {
    false
}

/// §42.35: air-transported supplies must be assigned to a dump or units in the Organization Phase
/// before use — they cannot be used directly off the plane (contrast airdropped supplies, §42.46).
pub fn air_transported_supply_usable_immediately() -> bool {
    false
}

/// §42.43: airdrops may occur only into clear, gravel (sand/gravel) or desert hexes.
pub fn airdrop_terrain_ok(terrain: crate::reference::terrain::HexTerrain) -> bool {
    use crate::reference::terrain::HexTerrain as T;
    matches!(terrain, T::Clear | T::SandGravel | T::Desert)
}

/// §42.43/§42.44: a drop-zone hex may be no more than two hexes from a recognizable land feature
/// (major city, road, railroad, village, air facility, or oasis).
pub const AIRDROP_MAX_FEATURE_DISTANCE: u32 = 2;

/// §42.45: airdrops may not occur in a sandstorm or rainstorm hex (there is no drift and no
/// casualties when they do occur).
pub fn airdrop_weather_ok(sandstorm: bool, rainstorm: bool) -> bool {
    !sandstorm && !rainstorm
}

/// §42.48: a unit may not be airdropped more than once a month — every twelve Operations Stages.
pub const AIRDROP_COOLDOWN_STAGES: u32 = 12;

/// §42.46: only ammunition and stores may be airdropped (not fuel or water).
pub fn supply_airdroppable(commodity: crate::state::game::Commodity) -> bool {
    use crate::state::game::Commodity as C;
    matches!(commodity, C::Ammo | C::Stores)
}

/// §42.46: airdropped supplies may be used as soon as they arrive (unlike air-*transported*
/// supplies, §42.35).
pub fn airdropped_supply_usable_immediately() -> bool {
    true
}

// ---- Axis Mediterranean air bases (§43) & Malta (§44) ----

/// §43.11: at least this percentage of the German heavy bombers (He 111, Ju 88D, Fw 200) must be
/// based in Mediterranean bases (Italy/Sicily/Crete).
pub const AXIS_MED_HEAVY_BOMBER_MIN_PERCENT: u32 = 75;

/// §43.12/§43.13: the Mediterranean basing transition is GT 35 (June week I 1941). Before GT 35,
/// 75% of all German bombers must be in Italy/Sicily; from GT 35 on, at least 50% of the He 111 /
/// Ju 88 / Fw 200 force must be in Crete (the other 25% in Sicily/Italy or Crete).
pub const AXIS_MED_BASING_TRANSITION_GT: u32 = 35;

/// §43.13: is the "≥50% of heavy bombers in Crete" requirement in force this Game-Turn?
pub fn crete_basing_required(game_turn: u32) -> bool {
    game_turn >= AXIS_MED_BASING_TRANSITION_GT
}

/// §43.14: transferring between the Italy, Sicily and Crete boxes takes one Operations Stage.
pub const AXIS_MED_TRANSFER_STAGES: u32 = 1;

/// §43.21: German bombers based in the Mediterranean boxes need no SGSU — the box supplies their
/// fuel and ammunition automatically — but they must still be refitted (§38.3).
pub fn med_bomber_needs_sgsu() -> bool {
    false
}
pub fn med_bomber_auto_supplied() -> bool {
    true
}
pub fn med_bomber_needs_refit() -> bool {
    true
}

/// §43.24: bombers flying from Crete receive no protective CAP from planes based in Africa.
pub fn crete_bombers_get_african_cap() -> bool {
    false
}

/// §43.25: bombers based in Italy/Sicily may coordinate with African-based fighters for CAP and may
/// join raids on Malta (§44).
pub fn italy_sicily_bombers_can_coordinate_cap() -> bool {
    true
}

/// §44.14: each level of a Maltese air facility handles up to 18 planes of any type (no SGSU
/// needed, §44.14); a full six-level field thus handles 108.
pub const MALTA_PLANES_PER_FACILITY_LEVEL: u32 = 18;

/// §44.14: the number of planes a Maltese air facility of `levels` may handle.
pub fn malta_facility_plane_capacity(levels: u32) -> u32 {
    levels * MALTA_PLANES_PER_FACILITY_LEVEL
}

/// §44.16: planes based on Malta are automatically refuelled and rearmed each Stage (no fuel/ammo
/// expended), but must be refitted like any other plane.
pub fn malta_planes_auto_supplied() -> bool {
    true
}
pub fn malta_planes_need_refit() -> bool {
    true
}

/// §44.12/§44.0: Maltese air facilities are permanent — they may be reduced to zero effectiveness
/// by Axis bombing but never destroyed or built anew, and the Axis may not invade Malta.
pub fn malta_facilities_destructible() -> bool {
    false
}
pub fn malta_invadable() -> bool {
    false
}

/// §44.25/§44.27: the number of a given plane type's IN-PLAY Italy/Sicily aircraft that may join a
/// Malta raid — `map_percent` of `in_play_italy_sicily` (rounded down).
pub fn malta_map_usable(map_percent: u32, in_play_italy_sicily: u32) -> u32 {
    in_play_italy_sicily * map_percent / 100
}

/// §44.25/§44.42 key: the number of NOT-in-play strategic aircraft of a type added to a Malta raid —
/// `strategic_percent` of the in-play Italy/Sicily count of that type (rounded down; 200 = double).
pub fn malta_strategic_addable(strategic_percent: u32, in_play_italy_sicily: u32) -> u32 {
    in_play_italy_sicily * strategic_percent / 100
}

/// §44.28: losses fall only on aircraft actually in play. Given `total_losses` across the raid,
/// `in_play_planes` used (Africa-based + the map-percent Italy/Sicily planes) and `total_planes`
/// used (including the not-in-play strategic planes), returns the losses applied to in-play planes,
/// rounded up. (33% of 8 → 3 in the rulebook example.)
pub fn malta_actual_loss(total_losses: u32, in_play_planes: u32, total_planes: u32) -> u32 {
    if total_planes == 0 {
        return 0;
    }
    (total_losses * in_play_planes).div_ceil(total_planes)
}

/// Resolve one §45 fighter-vs-fighter duel. `attacker_diff` is the attacker's adjusted TacAir
/// differential (the defender's is its negation); `roll1`/`roll2` are 2d6 rolls (encoded
/// `10*d1+d2`). The plane with the "minus" differential fires first (0 → higher pilot, tie →
/// attacker, §45.0); a kill ends the duel before the other fires. Returns
/// `(attacker_killed, defender_killed)`.
pub fn air_to_air_duel(
    tables: &crate::reference::squadron::AirToAirTables,
    attacker_diff: i32,
    attacker_pilot: u32,
    defender_pilot: u32,
    roll1: u32,
    roll2: u32,
) -> (bool, bool) {
    let defender_diff = -attacker_diff;
    if fighter_fires_first(attacker_diff, attacker_pilot, defender_pilot, true) {
        if tables.is_kill(attacker_diff, roll1) {
            return (false, true);
        }
        if tables.is_kill(defender_diff, roll2) {
            return (true, false);
        }
    } else {
        if tables.is_kill(defender_diff, roll1) {
            return (true, false);
        }
        if tables.is_kill(attacker_diff, roll2) {
            return (false, true);
        }
    }
    (false, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::squadron::squadron_capacity;
    use crate::reference::units::Nationality;

    #[test]
    fn range_and_transfer_37_11_21() {
        // Normal mission: within listed range only.
        assert!(can_fly(40, 40, false));
        assert!(!can_fly(40, 41, false));
        // Transfer / emergency doubles the one-way range.
        assert_eq!(one_way_range(40, true), 80);
        assert!(can_fly(40, 80, true));
        assert!(!can_fly(40, 81, true));
    }

    #[test]
    fn fly_caps_37_24() {
        // 5 SGSUs present, field capacity reduced to 2 -> only 2 may operate.
        assert_eq!(active_squadrons(5, 2), 2);
        assert_eq!(active_squadrons(2, 6), 2);
        // Italian squadron caps a mission at its ready capacity (9), not its reserves.
        let it = squadron_capacity(Nationality::Italian, 1);
        assert_eq!(max_planes_on_mission(12, it), 9);
        assert_eq!(max_planes_on_mission(7, it), 7);
    }

    #[test]
    fn emergency_escape_37_33() {
        // Fighters/fighter-bombers escape on 1-3.
        for d in 1..=3 {
            assert!(emergency_flight_escapes(AircraftClass::Fighter, d));
            assert!(emergency_flight_escapes(AircraftClass::FighterBomber, d));
        }
        assert!(!emergency_flight_escapes(AircraftClass::Fighter, 4));
        // Bombers/recon/transport (and dive-bombers §34.34) escape on 1-2 only.
        assert!(emergency_flight_escapes(AircraftClass::Bomber, 2));
        assert!(!emergency_flight_escapes(AircraftClass::Bomber, 3));
        assert!(!emergency_flight_escapes(AircraftClass::DiveBomber, 3));
        assert!(emergency_flight_escapes(AircraftClass::Transport, 1));
    }

    #[test]
    fn crash_landing_37_23() {
        assert_eq!(crash_landing(1, 6), CrashLanding::PlaneAndPilotSurvive);
        assert_eq!(crash_landing(3, 6), CrashLanding::PlaneAndPilotSurvive);
        assert_eq!(crash_landing(4, 1), CrashLanding::PlaneLostPilotSurvives);
        assert_eq!(crash_landing(6, 2), CrashLanding::PlaneLostPilotSurvives);
        assert_eq!(crash_landing(5, 3), CrashLanding::PlaneAndPilotLost);
        assert_eq!(crash_landing(6, 6), CrashLanding::PlaneAndPilotLost);
    }

    #[test]
    fn refit_modifiers_38_35() {
        assert_eq!(refit_die_modifier(Nationality::Commonwealth, false), 0);
        assert_eq!(refit_die_modifier(Nationality::German, false), 1);
        assert_eq!(refit_die_modifier(Nationality::Italian, false), 2);
        // Cumulative with the wrong-squadron +1.
        assert_eq!(refit_die_modifier(Nationality::Italian, true), 3);
        assert_eq!(refit_die_modifier(Nationality::Commonwealth, true), 1);
    }

    #[test]
    fn sandstorm_refit_loss_38_37() {
        assert_eq!(sandstorm_refit_loss(0), 0);
        assert_eq!(sandstorm_refit_loss(10), 2);
        assert_eq!(sandstorm_refit_loss(9), 2); // ceil(1.8)
        assert_eq!(sandstorm_refit_loss(5), 1); // ceil(1.0)
        assert_eq!(sandstorm_refit_loss(1), 1); // ceil(0.2)
    }

    #[test]
    fn arming_and_refuel_38_2_38_4() {        // Refuel cost = the loadout's fuel rating (§38.21).
        let ld = crate::reference::aircraft::Loadout {
            range: 40,
            drop_tanks: false,
            tac_air: 6,
            no_air_to_air: false,
            maneuver: 34,
            bombload: 0,
            torpedo: 0,
            fuel: 2,
        };
        assert_eq!(refuel_cost(&ld), 2);
        // Ordnance ammo: 1 for bombs (§38.44), 1 for mines/torpedoes (§38.47).
        assert_eq!(plane_ordnance_ammo(false, false), 0);
        assert_eq!(plane_ordnance_ammo(true, false), 1);
        assert_eq!(plane_ordnance_ammo(false, true), 1);
        assert_eq!(plane_ordnance_ammo(true, true), 2);
        // TacAir arming is one point per squadron (§38.43).
        assert_eq!(SQUADRON_TACAIR_AMMO, 1);
    }

    #[test]
    fn mission_eligibility_39_15_42_43() {
        use crate::reference::aircraft::{Mission, MissionCap, Missions};
        // A day fighter: CAP by day, not by night; Scramble day-only; no bombing.
        let day_fighter = Missions {
            cap: MissionCap::Day,
            scramble: MissionCap::Day,
            strike: MissionCap::Day,
            ..Default::default()
        };
        assert!(can_perform(&day_fighter, Mission::Cap, false));
        assert!(!can_perform(&day_fighter, Mission::Cap, true));
        assert!(!can_perform(&day_fighter, Mission::ConvoyLandBombing, false));
        // Transfer is open to any plane, day or night (§39.18).
        assert!(can_perform(&day_fighter, Mission::Transfer, true));
        // A night bomber: strike (bombing) by night.
        let night_bomber = Missions { strike: MissionCap::Night, ..Default::default() };
        assert!(can_perform(&night_bomber, Mission::Strike, true));
        assert!(can_perform(&night_bomber, Mission::Strike, false));
        // NightScrambleOnly (Bf.110/Ju.88C): only a night Scramble.
        let ns = Missions { scramble: MissionCap::NightScrambleOnly, ..Default::default() };
        assert!(can_perform(&ns, Mission::Scramble, true));
        assert!(!can_perform(&ns, Mission::Scramble, false));
        // §39.2 combined strike+bomb requires the D column.
        assert!(day_fighter.can_combine_strike_and_bomb());
        assert!(!Missions::default().can_combine_strike_and_bomb());
    }

    #[test]
    fn abort_rules_39_35_36_37() {
        // §39.35 fighter abort: own within 10 below the best enemy CAP maneuver.
        assert!(fighter_may_abort(30, Some(40))); // -10 exactly
        assert!(!fighter_may_abort(29, Some(40))); // -11
        assert!(fighter_may_abort(50, None)); // no enemy CAP
        // §39.37 screen: 12 escorts screen 40 CAP but not 50.
        assert!(sufficient_fighter_screen(12, 40));
        assert!(!sufficient_fighter_screen(12, 50));
        // §39.36 non-fighter: needs the screen, or a maneuver edge.
        assert!(nonfighter_may_abort(true, 10, Some(35)));
        assert!(!nonfighter_may_abort(false, 10, Some(35)));
        assert!(nonfighter_may_abort(false, 40, Some(35)));
        // §39.44 night interactions.
        assert!(!night_mission_air_to_air_allowed());
        assert!(!night_mission_may_be_intercepted());
    }

    #[test]
    fn pilots_40_1() {
        // §40.15: rating adds to TacAir. Bf.109F (6) + Marseille (6) in air-to-air.
        assert_eq!(air_to_air_tacair(6, 6), 12);
        assert_eq!(air_to_air_tacair(8, 0), 8); // zero-rated pilot (§40.16)
        // Valid non-zero ratings: 1,2,3,4,6 (no 5).
        assert!(!PILOT_RATINGS.contains(&5));
        assert!(PILOT_RATINGS.contains(&6));
        // Training / recovery are three Operations Stages (§40.14/§40.17).
        assert_eq!(PILOT_TRAINING_STAGES, 3);
        assert_eq!(PILOT_RECOVERY_STAGES, 3);
        // §40.18 ace rule: one rating point per five kills.
        assert_eq!(ace_rating_from_kills(4), 0);
        assert_eq!(ace_rating_from_kills(5), 1);
        assert_eq!(ace_rating_from_kills(23), 4);
    }

    #[test]
    fn cap_behavior_40_2() {
        use crate::reference::aircraft::{CapType, MissionCap, Missions};
        use crate::state::map::HexCoord;
        // §40.21: needs F capability.
        assert!(can_fly_cap(&Missions { cap: MissionCap::Day, ..Default::default() }));
        assert!(!can_fly_cap(&Missions::default()));
        // §40.23: offensive with 3+ has an air ZOC; 2 does not; defensive never does.
        assert!(cap_has_air_zoc(CapType::Offensive, 3));
        assert!(!cap_has_air_zoc(CapType::Offensive, 2));
        assert!(!cap_has_air_zoc(CapType::Defensive, 10));
        // §40.26 offensive vs defensive behaviour.
        assert!(cap_may_initiate(CapType::Offensive));
        assert!(!cap_may_initiate(CapType::Defensive));
        assert!(cap_must_engage(CapType::Offensive));
        assert!(cap_may_abort(CapType::Defensive));
        assert!(!cap_may_abort(CapType::Offensive));
        // §40.23 ZOC = center + 6 neighbours = 7 distinct hexes.
        let z = air_zoc_hexes(HexCoord::new(10, 10));
        assert_eq!(z.len(), 7);
        assert!(z.contains(&HexCoord::new(10, 10)));
        // §40.27.
        assert_eq!(OFFENSIVE_CAP_INTERCEPTS_PER_STAGE, 1);
    }

    #[test]
    fn scramble_40_3() {
        use crate::reference::aircraft::{MissionCap, Missions};
        use crate::state::map::HexCoord;
        // §40.31: needs scramble capability; NightScrambleOnly scrambles only at night.
        let day = Missions { scramble: MissionCap::Day, ..Default::default() };
        assert!(can_scramble(&day, false));
        assert!(!can_scramble(&day, true));
        let ns = Missions { scramble: MissionCap::NightScrambleOnly, ..Default::default() };
        assert!(can_scramble(&ns, true));
        assert!(!can_scramble(&ns, false));
        assert!(!can_scramble(&Missions::default(), false));
        // §40.33: path clear unless a non-target hex holds enemy fighters.
        let target = HexCoord::new(5, 0);
        let mid = HexCoord::new(3, 0);
        let path = [mid, target];
        assert!(scramble_path_clear(&path, target, |_| false));
        // Enemy fighters in the target hex are fine; in a mid hex they block.
        assert!(scramble_path_clear(&path, target, |h| h == target));
        assert!(!scramble_path_clear(&path, target, |h| h == mid));
    }

    #[test]
    fn strafing_40_5_40_8() {
        // §40.8 encoding: dice (3,5) -> 35.
        assert_eq!(encode_2d6(3, 5), 35);
        assert_eq!(encode_2d6(6, 6), 66);
        // Column shifts (§40.8 notes).
        assert_eq!(strafe_column_shift(StrafeTarget::TrucksInConvoy, 0), 1);
        assert_eq!(strafe_column_shift(StrafeTarget::SupplyDump, 0), 1);
        assert_eq!(strafe_column_shift(StrafeTarget::Infantry, 0), -1);
        assert_eq!(strafe_column_shift(StrafeTarget::Infantry, 1), -1);
        assert_eq!(strafe_column_shift(StrafeTarget::Infantry, 2), -2);
        assert_eq!(strafe_column_shift(StrafeTarget::Tanks, 1), 0);
        assert_eq!(strafe_column_shift(StrafeTarget::Tanks, 2), -1);
        assert_eq!(strafe_column_shift(StrafeTarget::FirstLineTrucks, 0), 0);
        // Result interpreters (§40.64/66/67).
        assert!(!pipeline_destroyed_by_strafe(1));
        assert!(pipeline_destroyed_by_strafe(2));
        assert_eq!(supply_dump_percent_destroyed(3), 30);
        assert_eq!(port_strafe_bombload_bonus(9), 4); // half, round down
    }

    #[test]
    fn flak_suppression_and_night_fighters_40_7_40_9() {
        use crate::reference::aircraft::{MissionCap, Missions};
        // §40.51 strafe capability: F (cap) or D (strike) column.
        assert!(can_strafe(&Missions { cap: MissionCap::Day, ..Default::default() }));
        assert!(can_strafe(&Missions { strike: MissionCap::Day, ..Default::default() }));
        assert!(!can_strafe(&Missions { recon: MissionCap::Day, ..Default::default() }));
        // §40.74: 3 fighters -> 1 flak point suppressed; light/ship yes, heavy no.
        assert_eq!(flak_points_suppressed(7), 2);
        assert!(aa_suppressible_by_fighters(AaType::Light));
        assert!(aa_suppressible_by_fighters(AaType::Ship));
        assert!(!aa_suppressible_by_fighters(AaType::Heavy));
        // §40.9 night fighters.
        assert_eq!(NIGHT_FIGHTER_SCRAMBLE_DIE_MODIFIER, 2);
        assert_eq!(night_fighter_day_maneuver_penalty("Bf.110"), -4);
        assert_eq!(night_fighter_day_maneuver_penalty("Blenheim Mk.IVF"), -4);
        assert_eq!(night_fighter_day_maneuver_penalty("Hurricane IIC"), 0);
        assert!(!offensive_cap_has_zoc_at_night());
    }

    #[test]
    fn bombing_restrictions_night_torpedo_pin_41() {
        use crate::reference::aircraft::{BombingMission, Loadout};
        let bomber = Loadout {
            range: 100, drop_tanks: false, tac_air: 6, no_air_to_air: true,
            maneuver: 20, bombload: 27, torpedo: 0, fuel: 3,
        };
        let fighter = Loadout {
            range: 40, drop_tanks: false, tac_air: 6, no_air_to_air: false,
            maneuver: 34, bombload: 0, torpedo: 0, fuel: 1,
        };
        // §41.0 / §41.11.
        assert!(can_bomb(&bomber));
        assert!(!can_bomb(&fighter));
        assert!(!bombers_may_scramble());
        assert!(!bombers_have_air_zoc());
        // §41.17 torpedo targets.
        assert!(torpedo_valid_target(BombingMission::Ships));
        assert!(torpedo_valid_target(BombingMission::Port));
        assert!(!torpedo_valid_target(BombingMission::TruckConvoy));
        // §41.45 night.
        assert_eq!(night_bombload(27), 13);
        assert!(!torpedoes_allowed_at_night());
        assert_eq!(night_flak_column_shift(), -1);
        // §41.47 night search: found on 1; NF subtracts 2 (so 1-3 find).
        assert!(night_search_finds(1, false));
        assert!(!night_search_finds(2, false));
        assert!(night_search_finds(3, true));
        assert!(!night_search_finds(4, true));
        // §41.73 CW convoy torpedo bonus (>=50% torpedo planes -> +25% round up).
        assert_eq!(cw_convoy_torpedo_tonnage(160, 17, 30), 200);
        assert_eq!(cw_convoy_torpedo_tonnage(160, 10, 30), 160); // <50%
        // §41.31 / §41.91.
        assert_eq!(pin_unit_weight(true), 2);
        assert_eq!(pin_unit_weight(false), 1);
        assert_eq!(pinned_cpa(7), 3);
    }

    #[test]
    fn bombing_result_mapping_41_5() {
        use crate::reference::aircraft::BombingMission as M;
        use crate::reference::air_bombardment::TargetGroup as G;
        // Mission -> CRT group (§41.3).
        assert_eq!(bombing_target_group(M::Personnel), Some(G::Units));
        assert_eq!(bombing_target_group(M::Ships), Some(G::Units));
        assert_eq!(bombing_target_group(M::SupplyDump), Some(G::SupplyDump));
        assert_eq!(bombing_target_group(M::Port), Some(G::AirfieldPortStrip));
        assert_eq!(bombing_target_group(M::AirFacility), Some(G::AirfieldPortStrip));
        assert_eq!(bombing_target_group(M::CityFortification), Some(G::Fortification));
        assert_eq!(bombing_target_group(M::MiningHarbor), None); // separate table
        // §41.5 key: armor -1 (floored), strip elimination, facility plane loss, dump truck loss.
        assert_eq!(bombing_result_vs_armor(3, true), 2);
        assert_eq!(bombing_result_vs_armor(0, true), 0);
        assert_eq!(bombing_result_vs_armor(3, false), 3);
        assert!(air_landing_strip_eliminated(1));
        assert!(!air_landing_strip_eliminated(0));
        assert_eq!(air_facility_planes_lost(2, 45), 9); // 20% of 45 = 9
        assert_eq!(air_facility_planes_lost(0, 45), 0);
        assert_eq!(supply_dump_truck_loss(30), 3);
        // §41.39A: one mining roll per six bombers.
        assert_eq!(mining_attempts(13), 2);
        assert_eq!(mining_attempts(5), 0);
    }

    #[test]
    fn air_to_air_45() {
        // §45.36 formation bonus: 6-17 -> +1, 18+ -> +2, dive-bombers 0.
        assert_eq!(formation_tacair_bonus(5, false), 0);
        assert_eq!(formation_tacair_bonus(12, false), 1);
        assert_eq!(formation_tacair_bonus(18, false), 2);
        assert_eq!(formation_tacair_bonus(20, true), 0);
        // §45.0/45.12 initiate: parenthesized TacAir may not initiate.
        assert!(may_initiate_air_to_air(false));
        assert!(!may_initiate_air_to_air(true));
        // §45.16 attacker: most fighters, tie -> initiative.
        assert_eq!(air_combat_attacker(10, 6, false), Attacker::A);
        assert_eq!(air_combat_attacker(6, 10, true), Attacker::B);
        assert_eq!(air_combat_attacker(8, 8, true), Attacker::A);
        assert_eq!(air_combat_attacker(8, 8, false), Attacker::B);
        // §45.15 screening.
        assert!(is_screened(false, true));
        assert!(!is_screened(false, false));
        assert!(!is_screened(true, true));
        // §45.0 fire order: minus differential first; 0 -> higher pilot, tie -> attacker.
        assert!(fighter_fires_first(-2, 0, 0, false)); // minus fires first
        assert!(!fighter_fires_first(2, 0, 0, false));
        assert!(fighter_fires_first(0, 3, 2, false)); // higher pilot
        assert!(fighter_fires_first(0, 2, 2, true)); // tie -> attacker
        assert!(!fighter_fires_first(0, 2, 2, false));
        assert!(nonfighter_fires_first());
        // §45.63/62/66 recovery.
        assert_eq!(fighter_loss_recovery(1), FighterLossRecovery::PlaneAndPilotReturn);
        assert_eq!(fighter_loss_recovery(2), FighterLossRecovery::PilotOnly);
        assert_eq!(fighter_loss_recovery(4), FighterLossRecovery::PlaneLost);
        assert!(strafed_plane_repaired(2));
        assert!(!strafed_plane_repaired(3));
        assert!(pilot_bails_out(2));
        assert!(!pilot_bails_out(3));
    }

    #[test]
    fn anti_aircraft_46() {
        use crate::reference::aircraft::AaTargetGroup as G;
        // §46.11: AA points = TOE * rating (no /10).
        assert_eq!(aa_points(5, 3), 15);
        // §46.26 target-group classification.
        assert_eq!(aa_target_group(false, false, false), G::DayOtherMission);
        assert_eq!(aa_target_group(false, false, true), G::NightOtherMission);
        assert_eq!(aa_target_group(true, false, false), G::DayFighter);
        assert_eq!(aa_target_group(true, false, true), G::NightFighter);
        assert_eq!(aa_target_group(true, true, false), G::StrafeDiveBomb);
        // §46.16/46.22: ground combat units fire AA only at group (e).
        assert!(ground_combat_aa_allowed(G::StrafeDiveBomb));
        assert!(!ground_combat_aa_allowed(G::DayFighter));
        assert!(!ground_combat_aa_allowed(G::DayOtherMission));
        // §46.24: flak-suppression / convoy-recon planes not targetable.
        assert!(aa_targetable(false, false));
        assert!(!aa_targetable(true, false));
        assert!(!aa_targetable(false, true));
        // §46.17 pure-AA free stacking.
        assert_eq!(PURE_AA_FREE_STACK_AIRFIELD, 3);
        assert_eq!(PURE_AA_FREE_STACK_LANDING_STRIP, 1);
    }

    #[test]
    fn non_combat_missions_42() {
        use crate::reference::aircraft::{MissionCap, Missions};
        use crate::reference::terrain::HexTerrain;
        use crate::state::game::Commodity;
        // §42.14 transfer.
        assert!(transfer_requires_refit_afterward());
        // §42.21 recon capability.
        assert!(can_recon(&Missions { recon: MissionCap::Day, ..Default::default() }));
        assert!(!can_recon(&Missions::default()));
        assert_eq!(RECON_TOE_LIE_DEVIATION, 2);
        assert_eq!(DUMMY_TANK_RECON_TOE, 3);
        // §42.25 no recon under bombardment.
        assert!(recon_allowed(false));
        assert!(!recon_allowed(true));
        // §42.37 transportable cargo.
        assert!(can_air_transport(AirCargo::InfantryNoTrucks));
        assert!(can_air_transport(AirCargo::Airborne));
        assert!(!can_air_transport(AirCargo::Other));
        assert!(!transported_personnel_may_move());
        assert!(!air_transported_supply_usable_immediately());
        // §42.43/45 airdrop terrain & weather.
        assert!(airdrop_terrain_ok(HexTerrain::Clear));
        assert!(airdrop_terrain_ok(HexTerrain::Desert));
        assert!(!airdrop_terrain_ok(HexTerrain::MajorCity));
        assert!(!airdrop_terrain_ok(HexTerrain::Mountain));
        assert!(airdrop_weather_ok(false, false));
        assert!(!airdrop_weather_ok(true, false));
        assert_eq!(AIRDROP_MAX_FEATURE_DISTANCE, 2);
        assert_eq!(AIRDROP_COOLDOWN_STAGES, 12);
        // §42.46 airdroppable supplies (ammo/stores only) and immediate use.
        assert!(supply_airdroppable(Commodity::Ammo));
        assert!(supply_airdroppable(Commodity::Stores));
        assert!(!supply_airdroppable(Commodity::Fuel));
        assert!(!supply_airdroppable(Commodity::Water));
        assert!(airdropped_supply_usable_immediately());
    }

    #[test]
    fn med_air_and_malta_43_44() {
        // §43.11/13 basing.
        assert_eq!(AXIS_MED_HEAVY_BOMBER_MIN_PERCENT, 75);
        assert!(!crete_basing_required(34)); // before Jun I 1941
        assert!(crete_basing_required(35)); // GT 35 = Jun I 1941
        assert_eq!(AXIS_MED_TRANSFER_STAGES, 1);
        // §43.21 Med bombers: no SGSU, auto-supplied, still refit.
        assert!(!med_bomber_needs_sgsu());
        assert!(med_bomber_auto_supplied());
        assert!(med_bomber_needs_refit());
        // §43.24/25.
        assert!(!crete_bombers_get_african_cap());
        assert!(italy_sicily_bombers_can_coordinate_cap());
        // §44.14 Malta capacity: 18/level, 6 levels -> 108.
        assert_eq!(malta_facility_plane_capacity(1), 18);
        assert_eq!(malta_facility_plane_capacity(6), 108);
        // §44.16 / §44.12.
        assert!(malta_planes_auto_supplied());
        assert!(malta_planes_need_refit());
        assert!(!malta_facilities_destructible());
        assert!(!malta_invadable());
    }

    #[test]
    fn malta_raid_computation_44() {
        // §44.25/27: map-usable and strategic-addable from Italy/Sicily counts.
        assert_eq!(malta_map_usable(50, 12), 6); // 50% of 12
        assert_eq!(malta_strategic_addable(200, 12), 24); // double
        assert_eq!(malta_map_usable(25, 10), 2); // round down (2.5 -> 2)
        // §44.28 loss example: 8 losses, 12 in-play of 36 total (33%) -> 3, round up.
        assert_eq!(malta_actual_loss(8, 12, 36), 3);
        assert_eq!(malta_actual_loss(0, 12, 36), 0);
        assert_eq!(malta_actual_loss(8, 0, 36), 0); // no in-play planes -> no in-play losses
    }

    #[test]
    fn air_to_air_duel_45() {
        use crate::reference::squadron::AirToAirTables;
        let t: AirToAirTables =
            ron::from_str(include_str!("../../../../data/reference/air-to-air.ron")).unwrap();
        // Rulebook example: Hurricane +5 vs Ju88 -5. Minus-differential (Ju88) fires first.
        // Ju88 needs <=11; Hurricane needs <=26.
        // Ju88 rolls 11 -> attacker (Hurricane) killed.
        assert_eq!(air_to_air_duel(&t, 5, 0, 0, 11, 26), (true, false));
        // Ju88 misses (12), Hurricane hits (26) -> defender (Ju88) killed.
        assert_eq!(air_to_air_duel(&t, 5, 0, 0, 12, 26), (false, true));
        // Both miss.
        assert_eq!(air_to_air_duel(&t, 5, 0, 0, 12, 32), (false, false));
    }
}
