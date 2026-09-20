//! Squadron / SGSU characteristics (§35) — the base-unit model for the Air Game.
//!
//! Every plane is attached to a squadron, represented on the map by a Squadron Ground Support Unit
//! (SGSU) counter: the mechanics, trucks and equipment that maintain and supply that squadron
//! (§35.0). SGSUs are vehicles (medium trucks) with a Capability Point Allowance and no combat
//! value (§35.12). This module holds the *reference* characteristics — class rules and capacities;
//! the placed, plane-holding SGSU lives in the state layer.

use super::aircraft::AircraftClass;
use super::units::Nationality;
use serde::{Deserialize, Serialize};

/// The class of aircraft a squadron/SGSU handles (§35.21). Each squadron holds exactly ONE class:
/// fighter, bomber, or transport. (Recon planes may go anywhere; fighter-bombers into either a
/// fighter or bomber squadron — see [`SquadronClass::accepts`].)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SquadronClass {
    Fighter,
    Bomber,
    Transport,
}

impl SquadronClass {
    /// May a plane of `class` be assigned to a squadron of this class (§35.21)?
    ///
    /// - Reconnaissance planes may be assigned anywhere.
    /// - Fighter-bombers may go into either a fighter or a bomber squadron.
    /// - Dive-bombers are treated the same as regular bombers (§34.34), so they take a bomber
    ///   squadron; flying-boats bomb and are not fighters/transports, so they too take a bomber
    ///   squadron. (§35.21 does not explicitly place flying-boats; this is a documented inference,
    ///   not a transcribed rule value — note their basing differs, §34.4.)
    /// - Otherwise the plane's class must match the squadron's class.
    pub fn accepts(self, class: AircraftClass) -> bool {
        match class {
            AircraftClass::Reconnaissance => true,
            AircraftClass::FighterBomber => {
                matches!(self, SquadronClass::Fighter | SquadronClass::Bomber)
            }
            AircraftClass::Fighter => self == SquadronClass::Fighter,
            AircraftClass::Bomber | AircraftClass::DiveBomber | AircraftClass::FlyingBoat => {
                self == SquadronClass::Bomber
            }
            AircraftClass::Transport => self == SquadronClass::Transport,
        }
    }

    /// The squadron class to *display* a plane of `class` under (§35.21) — Malta groups loose planes
    /// as if squadrons, so a plane type maps to one representative class for the counter glyph.
    /// Fighter/fighter-bomber → Fighter; transport → Transport; bomber/dive-bomber/flying-boat and
    /// (for glyph purposes) reconnaissance → Bomber.
    pub fn for_aircraft(class: AircraftClass) -> SquadronClass {
        match class {
            AircraftClass::Fighter | AircraftClass::FighterBomber => SquadronClass::Fighter,
            AircraftClass::Transport => SquadronClass::Transport,
            AircraftClass::Bomber
            | AircraftClass::DiveBomber
            | AircraftClass::FlyingBoat
            | AircraftClass::Reconnaissance => SquadronClass::Bomber,
        }
    }
}

/// Ready/reserve plane capacity of a squadron (§35.23). `ready` planes may fly; the squadron may
/// keep `reserve` more (readied or not) which cannot fly while the ready capacity is flying
/// (§35.26). Reserve is always one-third of ready in the published table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SquadronCapacity {
    pub ready: u32,
    pub reserve: u32,
}

impl SquadronCapacity {
    /// Maximum planes the squadron may hold (ready + reserve).
    pub fn total(self) -> u32 {
        self.ready + self.reserve
    }
}

/// GT 41 = first week of July 1941 (GT1 = Sep 1940, four one-week turns per month — the same
/// clock as `CW_HIGH_EVAP_UNTIL_GT`). Commonwealth squadrons increase capacity starting July 1941
/// (§35.23).
const CW_CAPACITY_INCREASE_GT: u32 = 41;

/// Ready/reserve capacity of a squadron by nationality and Game-Turn (§35.23 table). Italian
/// Squadriglia 9/3, German Staffel 12/4, Commonwealth Squadron 15/5 (1940–Jun '41) rising to
/// 18/6 (Jul '41 onward).
pub fn squadron_capacity(nationality: Nationality, game_turn: u32) -> SquadronCapacity {
    match nationality {
        Nationality::Italian => SquadronCapacity { ready: 9, reserve: 3 },
        Nationality::German => SquadronCapacity { ready: 12, reserve: 4 },
        Nationality::Commonwealth => {
            if game_turn < CW_CAPACITY_INCREASE_GT {
                SquadronCapacity { ready: 15, reserve: 5 }
            } else {
                SquadronCapacity { ready: 18, reserve: 6 }
            }
        }
    }
}

/// The Capability Point Allowance printed on a nationality's SGSU counter (§35.12): Italian 20,
/// German 25, Commonwealth 25. (USAAF-designation counters carry a CPA of 30 per §34.82A, but they
/// are a distinct counter type available only from Aug 1942 and are not modeled here.) Values are
/// read from the physical counters — they appear on no chart in the text rules.
pub fn sgsu_cpa(nationality: Nationality) -> u32 {
    match nationality {
        Nationality::Italian => 20,
        Nationality::German => 25,
        Nationality::Commonwealth => 25,
    }
}

/// §35.29: the aircraft role a German SGSU handles, inferred from its Geschwader designation prefix.
/// German SGSUs are type-specific (unlike the generic Commonwealth/Italian counters): `JG` = Fighter,
/// `St.G` = Dive Bomber, `KG`/`KLG`/`LG` = Bomber, `ZG` = Fighter-Bomber, `(H)`/`(F)` = Recon; `NJG`
/// (night fighter) is treated as a Fighter. Returns `None` for a designation that matches no known
/// prefix (e.g. the uncertain `SG` counter — see the roster transcription). Per §35.29 following this
/// is optional, so the result is advisory, not a placement rule.
pub fn german_sgsu_role(designation: &str) -> Option<AircraftClass> {
    if designation.contains("NJG") {
        Some(AircraftClass::Fighter)
    } else if designation.contains("St.G") || designation.contains("StG") {
        Some(AircraftClass::DiveBomber)
    } else if designation.contains("ZG") {
        Some(AircraftClass::FighterBomber)
    } else if designation.contains("(H)") || designation.contains("(F)") {
        Some(AircraftClass::Reconnaissance)
    } else if designation.contains("KG") || designation.contains("KLG") || designation.contains("LG") {
        Some(AircraftClass::Bomber)
    } else if designation.contains("JG") {
        Some(AircraftClass::Fighter)
    } else {
        None
    }
}

/// The per-nationality maximum 2d6 total that still refits a single plane (§38.37 chart A / §38.39).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByPlaneMax {
    pub commonwealth: u32,
    pub german: u32,
    pub italian: u32,
}

impl ByPlaneMax {
    pub fn for_nationality(self, n: Nationality) -> u32 {
        match n {
            Nationality::Commonwealth => self.commonwealth,
            Nationality::German => self.german,
            Nationality::Italian => self.italian,
        }
    }
}

/// The Aircraft Refit Table (§38.37). `by_squadron[r-1]` is the percentage of a squadron's planes
/// refitted on a modified roll of `r` (§38.34); `by_plane_max` holds the optional per-plane 2d6
/// thresholds (§38.39).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AircraftRefitTable {
    pub by_squadron: Vec<u32>,
    pub by_plane_max: ByPlaneMax,
}

impl AircraftRefitTable {
    /// Percentage of a squadron's planes refitted for a `base_die` roll adjusted by `modifier`
    /// (§38.34/§38.35). The modified roll is clamped into the table's range.
    pub fn squadron_percent(&self, base_die: u32, modifier: i32) -> u32 {
        let hi = self.by_squadron.len() as i32;
        let roll = (base_die as i32 + modifier).clamp(1, hi);
        self.by_squadron[(roll - 1) as usize]
    }

    /// Number of `num_planes` successfully refitted at the given roll (§38.34, fractions rounded up).
    pub fn planes_refitted(&self, num_planes: u32, base_die: u32, modifier: i32) -> u32 {
        let pct = self.squadron_percent(base_die, modifier);
        (num_planes * pct).div_ceil(100)
    }

    /// Optional per-plane method (§38.39): does a single `nationality` plane refit on a 2d6
    /// `two_d6_total`? +2 to the roll if the plane is not assigned to the refitting SGSU. A plane
    /// refits when the (modified) total is within 2..=max for its nationality.
    pub fn plane_refits(&self, nationality: Nationality, two_d6_total: u32, wrong_squadron: bool) -> bool {
        let total = two_d6_total + if wrong_squadron { 2 } else { 0 };
        total <= self.by_plane_max.for_nationality(nationality)
    }
}

/// One row of the §40.4 Scramble Table: any target distance up to `max_distance` hexes scrambles a
/// squadron on a d6 roll ≤ `success_die_max`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScrambleThreshold {
    pub max_distance: u32,
    pub success_die_max: u32,
}

/// The Scramble Table (§40.4). Thresholds are ordered by increasing distance band.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScrambleTable {
    pub thresholds: Vec<ScrambleThreshold>,
}

impl ScrambleTable {
    /// The maximum successful d6 roll for a scramble to a target `distance` hexes away, or `None`
    /// if the distance is beyond the table (the squadron cannot scramble that far, §40.33).
    pub fn success_die_max(&self, distance: u32) -> Option<u32> {
        self.thresholds
            .iter()
            .find(|t| distance <= t.max_distance)
            .map(|t| t.success_die_max)
    }

    /// Does a scramble to `distance` succeed on `die` (§40.33: roll ≤ the listed number)? Returns
    /// `false` when the distance is beyond the table.
    pub fn scramble_succeeds(&self, distance: u32, die: u32) -> bool {
        self.success_die_max(distance).is_some_and(|m| die <= m)
    }
}

/// One TacAir-Points column of the §40.8 Strafing Table. `r1`/`r2`/`r3` are the smallest 2d6 rolls
/// (encoded `10*d1+d2`) that yield results 1/2/3; `None` = that result is unreachable in this
/// column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrafeColumn {
    pub tacair_max: u32,
    pub r1: Option<u32>,
    pub r2: Option<u32>,
    pub r3: Option<u32>,
}

/// The Strafing Table (§40.8): columns by TacAir-Points band, each giving the result (0–3) for a
/// 2d6 roll.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrafingTable {
    pub columns: Vec<StrafeColumn>,
}

impl StrafingTable {
    /// The base column index for `tacair` points (first column whose `tacair_max` ≥ `tacair`).
    pub fn column_index(&self, tacair: u32) -> usize {
        self.columns
            .iter()
            .position(|c| tacair <= c.tacair_max)
            .unwrap_or(self.columns.len().saturating_sub(1))
    }

    /// The result (0–3) for a 2d6 `roll` (encoded `10*d1+d2`) read on column index `col`.
    pub fn result_at(&self, col: usize, roll: u32) -> u32 {
        let c = &self.columns[col];
        if c.r3.is_some_and(|m| roll >= m) {
            3
        } else if c.r2.is_some_and(|m| roll >= m) {
            2
        } else if c.r1.is_some_and(|m| roll >= m) {
            1
        } else {
            0
        }
    }

    /// Result for `tacair` points and a 2d6 `roll`, applying a signed column `shift` (positive =
    /// right/stronger per the §40.8 notes, negative = left/weaker), clamped to valid columns.
    pub fn result(&self, tacair: u32, shift: i32, roll: u32) -> u32 {
        let base = self.column_index(tacair) as i32;
        let last = self.columns.len() as i32 - 1;
        let col = (base + shift).clamp(0, last) as usize;
        self.result_at(col, roll)
    }
}

/// The Mining Harbor by Plane Table (§41.39A / §41.52). Indexed by the average bombload of the six
/// mining planes: `bands[i]` is the band's upper bound and `die_max[i]` the maximum d6 that mines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MiningHarborTable {
    pub bands: Vec<u32>,
    pub die_max: Vec<u32>,
}

impl MiningHarborTable {
    /// The maximum successful d6 for an average bombload of `avg_bombload` (0 = can never mine).
    pub fn die_max_for(&self, avg_bombload: u32) -> u32 {
        self.bands
            .iter()
            .position(|&b| avg_bombload <= b)
            .map(|i| self.die_max[i])
            .unwrap_or(0)
    }

    /// Is the harbor mined on `die` for the given average bombload (§41.39A: roll ≤ the listed
    /// number)?
    pub fn harbor_mined(&self, avg_bombload: u32, die: u32) -> bool {
        die <= self.die_max_for(avg_bombload)
    }
}

/// One band of the §45.4 Maneuver Adjustment Chart: an absolute maneuver difference up to `max_diff`
/// gives this TacAir `adjustment`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManeuverAdjustmentRow {
    pub max_diff: i32,
    pub adjustment: i32,
}

/// One row of the §45.5 TacAir Kill Table: an adjusted TacAir differential up to `max_diff` kills
/// the defender on a 2d6 roll ≤ `kill` (`None` = "na", never eliminated).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TacAirKillRow {
    pub max_diff: i32,
    pub kill: Option<u32>,
}

/// The Air-to-Air Combat tables (§45.4 Maneuver Adjustment + §45.5 TacAir Kill).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AirToAirTables {
    pub maneuver_adjustment: Vec<ManeuverAdjustmentRow>,
    pub kill_table: Vec<TacAirKillRow>,
}

impl AirToAirTables {
    /// §45.4: the TacAir adjustment for an absolute maneuver-rating difference.
    pub fn maneuver_adjustment(&self, maneuver_diff_abs: u32) -> i32 {
        let d = maneuver_diff_abs as i32;
        self.maneuver_adjustment
            .iter()
            .find(|r| d <= r.max_diff)
            .map(|r| r.adjustment)
            .unwrap_or(0)
    }

    /// A plane's adjusted TacAir differential (§45.0): its modified TacAir minus the enemy's, plus
    /// the §45.4 maneuver adjustment added for the higher-maneuver plane and subtracted for the
    /// lower (zero when maneuver ratings are equal).
    pub fn differential(
        &self,
        own_tacair: i32,
        enemy_tacair: i32,
        own_maneuver: i32,
        enemy_maneuver: i32,
    ) -> i32 {
        let base = own_tacair - enemy_tacair;
        let adj = self.maneuver_adjustment((own_maneuver - enemy_maneuver).unsigned_abs());
        let signed = match own_maneuver.cmp(&enemy_maneuver) {
            std::cmp::Ordering::Greater => adj,
            std::cmp::Ordering::Less => -adj,
            std::cmp::Ordering::Equal => 0,
        };
        base + signed
    }

    /// §45.5: the maximum killing 2d6 roll for an adjusted TacAir differential (`None` = never).
    pub fn kill_max(&self, adj_tacair_diff: i32) -> Option<u32> {
        self.kill_table
            .iter()
            .find(|r| adj_tacair_diff <= r.max_diff)
            .and_then(|r| r.kill)
    }

    /// §45.5: is the defender killed by a 2d6 `roll` (encoded `10*d1+d2`) at this differential?
    pub fn is_kill(&self, adj_tacair_diff: i32, roll: u32) -> bool {
        self.kill_max(adj_tacair_diff).is_some_and(|k| roll <= k)
    }
}

/// One row of the §42.27 Air Reconnaissance of Land Units Table: a modified die up to `max_die`
/// reveals `reveal` battalion-equivalents (`None` = "All", reveal every unit in the hex).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AirReconRow {
    pub max_die: i32,
    pub reveal: Option<u32>,
}

/// The Air Reconnaissance of Land Units Table (§42.27).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AirReconTable {
    pub rows: Vec<AirReconRow>,
}

impl AirReconTable {
    /// §42.27: +1 to the die for every four planes completing the mission (fractions ignored).
    pub fn plane_bonus(planes: u32) -> u32 {
        planes / 4
    }

    /// §42.27: the number of battalion-equivalents revealed for a `base_die` (1d6) with `planes` on
    /// the mission. `None` = "All" (reveal every unit in the hex).
    pub fn reveal(&self, base_die: u32, planes: u32) -> Option<u32> {
        let modified = (base_die + Self::plane_bonus(planes)) as i32;
        self.rows
            .iter()
            .find(|r| modified <= r.max_die)
            .map_or(Some(0), |r| r.reveal)
    }
}

/// The Axis Malta Availability Table (§44.42): a 2d6 roll (2–12) and Availability Level (I–IV) give
/// `(map_percent, strategic_percent)` — the percentage of in-play Italy/Sicily planes usable and the
/// percentage giving the not-in-play strategic planes addable — or `None` ("na", no forces).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaltaAvailabilityTable {
    /// `rows[roll-2][level-1]`.
    pub rows: Vec<Vec<Option<(u32, u32)>>>,
}

impl MaltaAvailabilityTable {
    /// The `(map_percent, strategic_percent)` for a 2d6 `roll` (2–12) at Availability `level` (1–4),
    /// or `None` for "na" / out-of-range input.
    pub fn cell(&self, roll: u32, level: u32) -> Option<(u32, u32)> {
        if !(2..=12).contains(&roll) || !(1..=4).contains(&level) {
            return None;
        }
        self.rows.get((roll - 2) as usize)?.get((level - 1) as usize).copied().flatten()
    }
}

/// The Maltese Air Facility Construction Table (§44.5): a d6 roll gives the Air Facility levels the
/// Commonwealth may build on one field this Game-Turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaltaConstructionTable {
    /// `levels[die-1]` for die 1–6.
    pub levels: Vec<u32>,
}

impl MaltaConstructionTable {
    /// Air Facility levels built on a `die` roll (0 for a roll of 1 or out of range).
    pub fn levels_built(&self, die: u32) -> u32 {
        if (1..=6).contains(&die) {
            self.levels.get((die - 1) as usize).copied().unwrap_or(0)
        } else {
            0
        }
    }
}

/// A cell of the §44.41 Axis Strategic Airforce Commitment Chart: how many Game-Turns an
/// Availability Level may be used in a scenario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaltaCommitment {
    /// May be used any number of Game-Turns (§44.25: Level I is always unlimited).
    Unlimited,
    /// May be used at most this many Game-Turns.
    Turns(u32),
    /// "na" — not applicable / not allowed.
    NotAllowed,
}

/// The Axis Strategic Airforce Commitment Chart (§44.41): per scenario, the four Availability
/// Levels' [I, II, III, IV] usage limits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaltaCommitmentChart {
    pub scenarios: std::collections::HashMap<String, Vec<MaltaCommitment>>,
}

impl MaltaCommitmentChart {
    /// The commitment for `scenario` at Availability `level` (1–4), if listed.
    pub fn commitment(&self, scenario: &str, level: u32) -> Option<MaltaCommitment> {
        if !(1..=4).contains(&level) {
            return None;
        }
        self.scenarios.get(scenario)?.get((level - 1) as usize).copied()
    }
}

/// The capability class of a squadron withdrawal (§34.84 `WD` column): the player removes a
/// squadron able to perform that role. `F` = a ≥12-plane CAP-capable fighter squadron, `B` = a
/// ≥12-plane bombing-capable squadron, `R` = a ≥12-plane recon-capable squadron.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WdClass {
    Fighter,
    Bomber,
    Recon,
}

/// One row of the §36.53 Allied Off-Map Air Facilities Chart: a Commonwealth off-map air facility
/// reached through an on-map entry/exit hex.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OffMapAirFacility {
    /// The on-map hex through which planes enter/exit this off-map facility.
    pub entry_hex: String,
    pub name: String,
    /// Distance (hexes) from the facility to its entry/exit hex (§37.4 flight cost).
    pub distance: u32,
    /// Printed SGSU capacity (§36.5b). `None` = unlimited (Ethiopia).
    pub max_squadrons: Option<u32>,
    /// A Flying-Boat Basin (§36.3 — flying boats only). Port Said.
    #[serde(default)]
    pub flying_boat: bool,
    /// Game-Turn before which no planes may be based here (§36.53 note — Ethiopia = 35). `None` =
    /// always available.
    #[serde(default)]
    pub available_from_gt: Option<u32>,
    /// Whether this counts as a "North African" off-map facility for the §34.82C creation test
    /// (the six Canal-zone boxes; Ethiopia is East African and excluded).
    pub north_african: bool,
}

/// The §36.53 Allied Off-Map Air Facilities Chart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OffMapAirFacilitiesChart {
    pub facilities: Vec<OffMapAirFacility>,
}

impl OffMapAirFacilitiesChart {
    /// §34.82C: the total printed squadron capacity of the Commonwealth North African off-map
    /// facilities (the sum used to compute the "maximum planes possibly based" threshold).
    pub fn north_african_squadron_capacity(&self) -> u32 {
        self.facilities
            .iter()
            .filter(|f| f.north_african)
            .filter_map(|f| f.max_squadrons)
            .sum()
    }
}

/// §34.84 / §34.85 Airplane Reinforcement schedule: the planes each nationality receives into its
/// pool over the campaign. Each period is a monthly total (over its Game-Turn range); the total is
/// divided as evenly as possible across the period's Game-Turns, remainder front-loaded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirReinforcementSchedule {
    pub periods: Vec<AirReinforcementPeriod>,
}

/// One month of the reinforcement schedule for one nationality.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirReinforcementPeriod {
    pub gt_start: u32,
    pub gt_end: u32,
    pub nationality: Nationality,
    /// `(aircraft-type key, monthly total)` — the key indexes the nationality's `AircraftChart`.
    pub planes: Vec<(String, u32)>,
    /// §34.84 `WD` column: `(capability class, count)` squadrons the player must withdraw at the
    /// *start* of this month (its `gt_start`). Only the base counts are modeled; the chart's
    /// `*`/`**`/`‡` qualifiers (multi-squadron / bomb-load conditions) are not yet interpreted.
    #[serde(default)]
    pub withdrawals: Vec<(WdClass, u32)>,
}

impl AirReinforcementSchedule {
    /// Planes arriving in Game-Turn `gt` — each period's monthly total split evenly across its
    /// Game-Turns (remainder front-loaded, §34.84 note). Returns `(nationality, aircraft-type, count)`.
    pub fn arrivals_for(&self, gt: u32) -> Vec<(Nationality, String, u32)> {
        let mut out = Vec::new();
        for p in &self.periods {
            if gt < p.gt_start || gt > p.gt_end {
                continue;
            }
            let n = p.gt_end - p.gt_start + 1;
            let k = gt - p.gt_start; // 0-based Game-Turn index within the month
            for (ac, total) in &p.planes {
                let cnt = total / n + if k < total % n { 1 } else { 0 };
                if cnt > 0 {
                    out.push((p.nationality, ac.clone(), cnt));
                }
            }
        }
        out
    }

    /// Squadron withdrawals due in Game-Turn `gt` (§34.84 `WD`) — applied once, at each period's
    /// `gt_start`. Returns `(nationality, capability class, count)`.
    pub fn withdrawals_for(&self, gt: u32) -> Vec<(Nationality, WdClass, u32)> {
        let mut out = Vec::new();
        for p in &self.periods {
            if gt != p.gt_start {
                continue;
            }
            for &(class, count) in &p.withdrawals {
                if count > 0 {
                    out.push((p.nationality, class, count));
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacities_match_35_23() {
        // Italian / German are date-independent.
        assert_eq!(
            squadron_capacity(Nationality::Italian, 1),
            SquadronCapacity { ready: 9, reserve: 3 }
        );
        assert_eq!(
            squadron_capacity(Nationality::German, 60),
            SquadronCapacity { ready: 12, reserve: 4 }
        );
        // Commonwealth increases at July 1941 (GT 41): 15/5 before, 18/6 from GT 41.
        assert_eq!(
            squadron_capacity(Nationality::Commonwealth, 40),
            SquadronCapacity { ready: 15, reserve: 5 }
        );
        assert_eq!(
            squadron_capacity(Nationality::Commonwealth, 41),
            SquadronCapacity { ready: 18, reserve: 6 }
        );
        assert_eq!(squadron_capacity(Nationality::German, 1).total(), 16);
    }

    #[test]
    fn class_acceptance_35_21() {        // Recon fits anywhere.
        for sq in [SquadronClass::Fighter, SquadronClass::Bomber, SquadronClass::Transport] {
            assert!(sq.accepts(AircraftClass::Reconnaissance));
        }
        // Fighter-bomber into fighter or bomber, not transport.
        assert!(SquadronClass::Fighter.accepts(AircraftClass::FighterBomber));
        assert!(SquadronClass::Bomber.accepts(AircraftClass::FighterBomber));
        assert!(!SquadronClass::Transport.accepts(AircraftClass::FighterBomber));
        // Pure fighter only in a fighter squadron.
        assert!(SquadronClass::Fighter.accepts(AircraftClass::Fighter));
        assert!(!SquadronClass::Bomber.accepts(AircraftClass::Fighter));
        // Dive-bombers (§34.34) and flying-boats take a bomber squadron.
        assert!(SquadronClass::Bomber.accepts(AircraftClass::DiveBomber));
        assert!(SquadronClass::Bomber.accepts(AircraftClass::FlyingBoat));
        assert!(!SquadronClass::Fighter.accepts(AircraftClass::DiveBomber));
        // Transport only in a transport squadron.
        assert!(SquadronClass::Transport.accepts(AircraftClass::Transport));
        assert!(!SquadronClass::Bomber.accepts(AircraftClass::Transport));
    }

    #[test]
    fn refit_table_38_37() {
        let t: AircraftRefitTable =
            ron::from_str(include_str!("../../../../data/reference/aircraft-refit.ron")).unwrap();
        // By-squadron percentages (§38.34), roll clamped.
        assert_eq!(t.squadron_percent(1, 0), 100);
        assert_eq!(t.squadron_percent(6, 0), 40);
        assert_eq!(t.squadron_percent(6, 1), 40); // roll 7 -> 40
        assert_eq!(t.squadron_percent(6, 2), 33); // roll 8 -> 33
        assert_eq!(t.squadron_percent(6, 5), 33); // clamp at 9
        // Round up (§38.34): 12 planes at 33% -> ceil(3.96) = 4.
        assert_eq!(t.planes_refitted(12, 8, 0), 4);
        assert_eq!(t.planes_refitted(10, 1, 0), 10); // 100%
        // By-plane thresholds (§38.39): CW 2-8, German 2-7, Italian 2-6.
        assert!(t.plane_refits(Nationality::Commonwealth, 8, false));
        assert!(!t.plane_refits(Nationality::Commonwealth, 9, false));
        assert!(!t.plane_refits(Nationality::Italian, 7, false));
        assert!(t.plane_refits(Nationality::German, 7, false));
        // Wrong-squadron +2 shrinks the range: German 5+2=7 ok, 6+2=8 fails.
        assert!(t.plane_refits(Nationality::German, 5, true));
        assert!(!t.plane_refits(Nationality::German, 6, true));
    }

    #[test]
    fn scramble_table_40_4() {
        let t: ScrambleTable =
            ron::from_str(include_str!("../../../../data/reference/scramble-table.ron")).unwrap();
        // Distance bands -> max successful d6.
        assert_eq!(t.success_die_max(0), Some(6));
        assert_eq!(t.success_die_max(2), Some(6));
        assert_eq!(t.success_die_max(3), Some(5));
        assert_eq!(t.success_die_max(6), Some(4));
        assert_eq!(t.success_die_max(8), Some(3));
        assert_eq!(t.success_die_max(10), Some(2));
        assert_eq!(t.success_die_max(15), Some(1));
        assert_eq!(t.success_die_max(16), None); // beyond the table
        // Success is roll <= threshold (§40.33).
        assert!(t.scramble_succeeds(2, 6));
        assert!(t.scramble_succeeds(15, 1));
        assert!(!t.scramble_succeeds(15, 2));
        assert!(!t.scramble_succeeds(16, 1));
    }

    #[test]
    fn strafing_table_40_8() {
        let t: StrafingTable =
            ron::from_str(include_str!("../../../../data/reference/strafing-table.ron")).unwrap();
        // Column selection by TacAir points.
        assert_eq!(t.column_index(3), 0); // 1-5
        assert_eq!(t.column_index(15), 2); // 11-15
        assert_eq!(t.column_index(100), 9); // 46+
        // Column 1-5 (idx 0): 11-63 -> 0, 64-66 -> 1, no 2/3.
        assert_eq!(t.result_at(0, 63), 0);
        assert_eq!(t.result_at(0, 64), 1);
        assert_eq!(t.result_at(0, 66), 1);
        // Column 46+ (idx 9): 11-46 -> 1, 51-64 -> 2, 65-66 -> 3.
        assert_eq!(t.result_at(9, 11), 1);
        assert_eq!(t.result_at(9, 46), 1);
        assert_eq!(t.result_at(9, 51), 2);
        assert_eq!(t.result_at(9, 64), 2);
        assert_eq!(t.result_at(9, 65), 3);
        // Column 36-40 (idx 7): 11-16 -> 0, 21-56 -> 1, 61-66 -> 2.
        assert_eq!(t.result_at(7, 16), 0);
        assert_eq!(t.result_at(7, 21), 1);
        assert_eq!(t.result_at(7, 61), 2);
        // Shift: +1 right moves 15-TacAir (idx 2) to idx 3.
        assert_eq!(t.result(15, 1, 66), t.result_at(3, 66));
    }

    #[test]
    fn mining_harbor_table_41_39() {
        let t: MiningHarborTable =
            ron::from_str(include_str!("../../../../data/reference/mining-harbor.ron")).unwrap();
        // Average bombload band -> max successful die.
        assert_eq!(t.die_max_for(3), 0); // 0-5: never
        assert_eq!(t.die_max_for(8), 1); // 6-10
        assert_eq!(t.die_max_for(30), 5); // 26-30
        assert_eq!(t.die_max_for(40), 6); // 31+: always
        // Mined if die <= threshold.
        assert!(!t.harbor_mined(3, 1)); // threshold 0 -> never
        assert!(t.harbor_mined(8, 1));
        assert!(!t.harbor_mined(8, 2));
        assert!(t.harbor_mined(40, 6));
    }

    #[test]
    fn air_to_air_tables_45_4_45_5() {
        let t: AirToAirTables =
            ron::from_str(include_str!("../../../../data/reference/air-to-air.ron")).unwrap();
        // §45.4 maneuver adjustment bands.
        assert_eq!(t.maneuver_adjustment(0), 0);
        assert_eq!(t.maneuver_adjustment(3), 1);
        assert_eq!(t.maneuver_adjustment(4), 2);
        assert_eq!(t.maneuver_adjustment(11), 3);
        assert_eq!(t.maneuver_adjustment(16), 4);
        assert_eq!(t.maneuver_adjustment(25), 5);
        // §45.5 kill maxima.
        assert_eq!(t.kill_max(-13), None); // na
        assert_eq!(t.kill_max(-14), None);
        assert_eq!(t.kill_max(-5), Some(11));
        assert_eq!(t.kill_max(-2), Some(12));
        assert_eq!(t.kill_max(0), Some(14));
        assert_eq!(t.kill_max(5), Some(26));
        assert_eq!(t.kill_max(12), Some(56));
        assert_eq!(t.kill_max(20), Some(56));
        // Rulebook example (b): Hurricane 8 TacAir, +11 maneuver vs Ju88 6 TacAir.
        // Hurricane differential +5, Ju88 -5.
        assert_eq!(t.differential(8, 6, 39, 28), 5);
        assert_eq!(t.differential(6, 8, 28, 39), -5);
        assert!(t.is_kill(5, 26)); // Hurricane needs 11-26
        assert!(!t.is_kill(5, 32));
        assert!(t.is_kill(-5, 11)); // Ju88 needs 11
        assert!(!t.is_kill(-5, 12));
        // Example (a): equal TacAir 6, maneuver diff 4 -> +2 / -2.
        assert_eq!(t.differential(6, 6, 32, 28), 2);
        assert_eq!(t.differential(6, 6, 28, 32), -2);
        assert!(t.is_kill(-2, 12)); // CR42 needs 11-12
        assert!(!t.is_kill(-2, 13));
    }

    #[test]
    fn air_recon_table_42_27() {
        let t: AirReconTable =
            ron::from_str(include_str!("../../../../data/reference/air-recon.ron")).unwrap();
        // Base die (no plane bonus).
        assert_eq!(t.reveal(2, 0), Some(0));
        assert_eq!(t.reveal(3, 0), Some(1));
        assert_eq!(t.reveal(6, 0), Some(2));
        // +1 per four planes: die 6 with 8 planes -> modified 8 -> reveal 4.
        assert_eq!(AirReconTable::plane_bonus(8), 2);
        assert_eq!(t.reveal(6, 8), Some(4));
        // 13+ -> All (None).
        assert_eq!(t.reveal(6, 28), None); // 6 + 7 = 13
        assert_eq!(t.reveal(6, 40), None);
    }

    #[test]
    fn malta_availability_44_42() {
        let t: MaltaAvailabilityTable =
            ron::from_str(include_str!("../../../../data/reference/malta-availability.ron")).unwrap();
        // §44.27 example: Level III, roll 6 -> 50/100.
        assert_eq!(t.cell(6, 3), Some((50, 100)));
        // "na" cells.
        assert_eq!(t.cell(4, 1), None); // roll 4, Level I = na
        assert_eq!(t.cell(2, 3), None); // roll 2, Level III = na
        // Corners.
        assert_eq!(t.cell(2, 1), Some((10, 30)));
        assert_eq!(t.cell(12, 4), Some((25, 75)));
        // Out of range.
        assert_eq!(t.cell(1, 1), None);
        assert_eq!(t.cell(6, 5), None);
    }

    #[test]
    fn malta_construction_44_5() {
        let t: MaltaConstructionTable =
            ron::from_str(include_str!("../../../../data/reference/malta-construction.ron")).unwrap();
        assert_eq!(t.levels_built(1), 0);
        assert_eq!(t.levels_built(2), 1);
        assert_eq!(t.levels_built(5), 1);
        assert_eq!(t.levels_built(6), 2);
    }

    #[test]
    fn german_sgsu_role_35_29() {
        // The §35.29 Geschwader-prefix convention, tested against the transcribed roster.
        assert_eq!(german_sgsu_role("8/III JG 27"), Some(AircraftClass::Fighter));
        assert_eq!(german_sgsu_role("Stab St.G.3"), Some(AircraftClass::DiveBomber));
        assert_eq!(german_sgsu_role("1/I KG 54"), Some(AircraftClass::Bomber));
        assert_eq!(german_sgsu_role("5/II KLG 1"), Some(AircraftClass::Bomber));
        assert_eq!(german_sgsu_role("8/III ZG 26"), Some(AircraftClass::FighterBomber));
        assert_eq!(german_sgsu_role("2.(H) 14"), Some(AircraftClass::Reconnaissance));
        assert_eq!(german_sgsu_role("1.(F) 121"), Some(AircraftClass::Reconnaissance));
        // NJG (night fighter) is treated as a fighter — and must not be caught by the JG branch first.
        assert_eq!(german_sgsu_role("1/I NJG 2"), Some(AircraftClass::Fighter));
        // The uncertain SG counter matches no known prefix.
        assert_eq!(german_sgsu_role("1/I SG 2"), None);
    }

    #[test]
    fn off_map_air_facilities_36_53() {
        let c: OffMapAirFacilitiesChart =
            ron::from_str(include_str!("../../../../data/reference/off-map-air-facilities.ron")).unwrap();
        assert_eq!(c.facilities.len(), 7);
        // Port Said is a flying-boat basin with 3 squadrons.
        let ps = c.facilities.iter().find(|f| f.name == "Port Said").unwrap();
        assert!(ps.flying_boat);
        assert_eq!(ps.max_squadrons, Some(3));
        // Ethiopia: unlimited, East African (not counted), time-gated to GT35.
        let eth = c.facilities.iter().find(|f| f.name == "Ethiopia").unwrap();
        assert_eq!(eth.max_squadrons, None);
        assert!(!eth.north_african);
        assert_eq!(eth.available_from_gt, Some(35));
        // §34.82C: the six Canal-zone boxes total 48 printed squadrons.
        assert_eq!(c.north_african_squadron_capacity(), 48);
    }

    #[test]
    fn malta_commitment_44_41() {
        let c: MaltaCommitmentChart =
            ron::from_str(include_str!("../../../../data/reference/malta-commitment.ron")).unwrap();
        // Campaign Game row (primary): U / 25 / 12 / 12.
        assert_eq!(c.commitment("Campaign Game", 1), Some(MaltaCommitment::Unlimited));
        assert_eq!(c.commitment("Campaign Game", 2), Some(MaltaCommitment::Turns(25)));
        assert_eq!(c.commitment("Campaign Game", 3), Some(MaltaCommitment::Turns(12)));
        assert_eq!(c.commitment("Campaign Game", 4), Some(MaltaCommitment::Turns(12)));
        // Last Chance: Malta not in scenario -> all na.
        assert_eq!(c.commitment("Last Chance", 1), Some(MaltaCommitment::NotAllowed));
        // Race for Tobruk IV (blank -> na); Long Retreat I (unlimited per §44.25).
        assert_eq!(c.commitment("Race for Tobruk", 4), Some(MaltaCommitment::NotAllowed));
        assert_eq!(c.commitment("Long Retreat", 1), Some(MaltaCommitment::Unlimited));
    }
}
