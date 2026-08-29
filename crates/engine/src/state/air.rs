//! Tier-2 Air Game state: the placed Squadron Ground Support Unit (SGSU) and its plane roster
//! (the Squadron Composition Sheet, §35.2). Distinct from a land `Unit`: an SGSU has no combat
//! value, is a vehicle (medium truck) that moves in the Truck Convoy Phase (§35.12), and serves as
//! the base that maintains and supplies the planes attached to it (§35.0).
//!
//! Reference characteristics (class rules, per-nationality capacities) live in
//! [`crate::reference::squadron`]; this module holds the mutable, placed instance.

use serde::{Deserialize, Serialize};

use crate::reference::aircraft::AircraftClass;
use crate::reference::squadron::{squadron_capacity, SquadronCapacity, SquadronClass};
use crate::reference::units::Nationality;
use crate::state::map::HexCoord;

/// Stable identifier for a squadron / its SGSU counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SquadronId(pub u32);

/// A group of identical planes attached to a squadron: a key into the nationality's aircraft chart
/// plus a count. A squadron may hold several types so long as they share the squadron's class
/// (§35.21). Readiness (§38) is tracked as nested counts: `refitted` planes have been repaired
/// (§38.3) and may fly once fueled; `ready` planes are refitted + fueled and able to fly a mission
/// (§38.1); `armed` is the subset of ready planes carrying ammunition/bombs (§38.4). Invariant:
/// `armed <= ready <= refitted <= count`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaneGroup {
    /// Key into the nationality's [`crate::reference::aircraft::AircraftChart`].
    pub aircraft_type: String,
    pub count: u32,
    #[serde(default)]
    pub refitted: u32,
    #[serde(default)]
    pub ready: u32,
    #[serde(default)]
    pub armed: u32,
}

/// A pilot (§40.1). Pilots are not counters — they are numbers (their Rating) assigned to a
/// squadron; the Rating adds to a plane's TacAir in air-to-air combat (§40.15). A plane flown with
/// no assigned pilot uses a zero-rated pilot (§40.16, unlimited supply, not tracked here).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pilot {
    /// 0 (untrained/default) or one of 1, 2, 3, 4, 6 (§40.15).
    pub rating: u32,
    /// The squadron the pilot is assigned to (§40.12), if any.
    #[serde(default)]
    pub squadron: Option<SquadronId>,
    /// Operations Stages remaining before this pilot may fly: reassignment lockout (§40.13, 1
    /// stage), retraining onto a new plane type (§40.14, 3 stages), or the post-recovery delay
    /// (§40.17, 3 stages). 0 = may fly.
    #[serde(default)]
    pub grounded_stages: u32,
    /// Air-to-air kills, for the optional ace rule (§40.18).
    #[serde(default)]
    pub kills: u32,
}

impl Pilot {
    /// A pilot of the given Rating, assigned to `squadron`, ready to fly.
    pub fn new(rating: u32, squadron: Option<SquadronId>) -> Self {
        Pilot { rating, squadron, grounded_stages: 0, kills: 0 }
    }

    /// May this pilot fly this stage (§40.13/§40.14/§40.17)?
    pub fn may_fly(&self) -> bool {
        self.grounded_stages == 0
    }

    /// Advance one Operations Stage: decrement any training/lockout/recovery countdown.
    pub fn advance_stage(&mut self) {
        self.grounded_stages = self.grounded_stages.saturating_sub(1);
    }

    /// Reassign to `squadron`; if `retrain` (a different plane type, §40.14) the pilot is grounded
    /// for three stages, otherwise one (§40.13).
    pub fn reassign(&mut self, squadron: Option<SquadronId>, retrain: bool) {
        self.squadron = squadron;
        self.grounded_stages = if retrain { 3 } else { 1 };
    }
}

/// A Squadron Ground Support Unit on (or off) the map, with the planes attached to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sgsu {
    pub id: SquadronId,
    pub nationality: Nationality,
    /// The single aircraft class this squadron maintains (§35.21).
    pub class: SquadronClass,
    /// Historical squadron designation (§35.13); not used historically in play.
    pub designation: String,
    /// Capability Point Allowance printed on the counter (§35.12); it is a medium-truck vehicle.
    pub cpa: u32,
    /// Where the SGSU sits. `None` = eliminated / awaiting return to play (§35.13) or in a pool.
    pub location: Option<HexCoord>,
    /// The planes attached to this squadron (its Composition Sheet, §35.2).
    #[serde(default)]
    pub planes: Vec<PlaneGroup>,
    /// §35.14: whether the SGSU has its own upkeep this stage (1 Stores/Game-Turn, 1 Fuel + 1 Water
    /// /Operations Stage). An unsupplied SGSU may not repair (refit) its planes. Defaults true.
    #[serde(default = "default_true")]
    pub supplied: bool,
}

fn default_true() -> bool {
    true
}

impl Sgsu {
    /// An SGSU expends one Stores Point per Game-Turn for itself (§35.14).
    pub const STORES_UPKEEP_PER_TURN: u32 = 1;
    /// …plus one Fuel and one Water Point per Operations Stage (§35.14).
    pub const FUEL_UPKEEP_PER_STAGE: u32 = 1;
    pub const WATER_UPKEEP_PER_STAGE: u32 = 1;

    /// A new, empty SGSU placed at `location`.
    pub fn new(
        id: SquadronId,
        nationality: Nationality,
        class: SquadronClass,
        designation: impl Into<String>,
        cpa: u32,
        location: Option<HexCoord>,
    ) -> Self {
        Sgsu {
            id,
            nationality,
            class,
            designation: designation.into(),
            cpa,
            location,
            planes: Vec::new(),
            supplied: true,
        }
    }

    /// Ready/reserve capacity for this squadron at `game_turn` (§35.23).
    pub fn capacity(&self, game_turn: u32) -> SquadronCapacity {
        squadron_capacity(self.nationality, game_turn)
    }

    /// §38.23/§38.33: the maximum number of planes this SGSU may refuel, rearm or refit in one
    /// Operations Stage / Strategic Maintenance Phase — its full capacity (ready + reserve),
    /// regardless of which squadron the planes belong to.
    pub fn handling_limit(&self, game_turn: u32) -> u32 {
        self.capacity(game_turn).total()
    }

    /// Total planes currently attached (ready + reserve).
    pub fn plane_count(&self) -> u32 {
        self.planes.iter().map(|g| g.count).sum()
    }

    /// May `count` planes of `class` be added without exceeding the class rule (§35.21) or the
    /// total (ready + reserve) capacity (§35.23) at `game_turn`?
    pub fn can_add(&self, class: AircraftClass, count: u32, game_turn: u32) -> bool {
        self.class.accepts(class)
            && self.plane_count() + count <= self.capacity(game_turn).total()
    }

    /// Attach `count` planes of `aircraft_type`/`class`, merging into an existing group of the same
    /// type. Returns `false` (and changes nothing) if the class rule or capacity would be violated.
    pub fn add_planes(
        &mut self,
        aircraft_type: &str,
        class: AircraftClass,
        count: u32,
        game_turn: u32,
    ) -> bool {
        if !self.can_add(class, count, game_turn) {
            return false;
        }
        if let Some(g) = self.planes.iter_mut().find(|g| g.aircraft_type == aircraft_type) {
            g.count += count;
        } else {
            self.planes.push(PlaneGroup {
                aircraft_type: aircraft_type.to_string(),
                count,
                refitted: 0,
                ready: 0,
                armed: 0,
            });
        }
        true
    }

    /// Total refitted planes (§38.3).
    pub fn refitted_planes(&self) -> u32 {
        self.planes.iter().map(|g| g.refitted).sum()
    }

    /// Total planes ready to fly a mission (refitted + fueled, §38.1).
    pub fn ready_planes(&self) -> u32 {
        self.planes.iter().map(|g| g.ready).sum()
    }

    /// Total armed planes (§38.4).
    pub fn armed_planes(&self) -> u32 {
        self.planes.iter().map(|g| g.armed).sum()
    }

    /// §38.31: mark all the squadron's planes refitted (scenario start, "all planes are refitted").
    pub fn refit_all(&mut self) {
        for g in self.planes.iter_mut() {
            g.refitted = g.count;
        }
    }

    /// §38.34/§38.37: attempt to refit the squadron's un-refitted planes with one die (`base_die`)
    /// and the §38.35 `modifier`, using the Aircraft Refit Table. One roll covers the whole
    /// squadron; the resulting percentage (rounded up) of the planes undergoing refit are refitted,
    /// distributed across the plane groups. Returns the number newly refitted. (§38.36's Stores cost
    /// is handled by the caller.)
    pub fn refit(
        &mut self,
        table: &crate::reference::squadron::AircraftRefitTable,
        base_die: u32,
        modifier: i32,
    ) -> u32 {
        let undergoing: u32 = self.planes.iter().map(|g| g.count - g.refitted).sum();
        let mut budget = table.planes_refitted(undergoing, base_die, modifier);
        let mut done = 0;
        for g in self.planes.iter_mut() {
            if budget == 0 {
                break;
            }
            let n = (g.count - g.refitted).min(budget);
            g.refitted += n;
            budget -= n;
            done += n;
        }
        done
    }

    /// §38.2: refuel up to `want` refitted-but-unfueled planes to ready, bounded by the squadron's
    /// ready capacity (§35.23/§37.24). Returns the number newly readied.
    pub fn refuel_to_ready(&mut self, want: u32, game_turn: u32) -> u32 {
        let cap = self.capacity(game_turn).ready;
        let room = cap.saturating_sub(self.ready_planes());
        let mut budget = want.min(room);
        let mut done = 0;
        for g in self.planes.iter_mut() {
            if budget == 0 {
                break;
            }
            let n = (g.refitted - g.ready).min(budget);
            g.ready += n;
            budget -= n;
            done += n;
        }
        done
    }

    /// §38.4: arm up to `want` ready-but-unarmed planes. Returns the number newly armed.
    pub fn arm_up_to(&mut self, want: u32) -> u32 {
        let mut budget = want;
        let mut done = 0;
        for g in self.planes.iter_mut() {
            if budget == 0 {
                break;
            }
            let n = (g.ready - g.armed).min(budget);
            g.armed += n;
            budget -= n;
            done += n;
        }
        done
    }

    /// §37.16/§38.31: `want` ready planes fly a (non-transfer) mission — they burn their fuel and
    /// must be refitted again, so they drop out of both `ready` and `refitted`. Returns the number
    /// that flew.
    pub fn fly(&mut self, want: u32) -> u32 {
        let mut budget = want;
        let mut flown = 0;
        for g in self.planes.iter_mut() {
            if budget == 0 {
                break;
            }
            let n = g.ready.min(budget);
            g.ready -= n;
            g.refitted -= n;
            g.armed = g.armed.min(g.ready);
            budget -= n;
            flown += n;
        }
        flown
    }

    /// Permanently remove `want` planes from the squadron (combat losses, §45/§46), reducing the
    /// total count and clamping the readiness counts. Empty groups are dropped. Returns the number
    /// removed.
    pub fn lose_planes(&mut self, want: u32) -> u32 {
        let mut budget = want;
        let mut lost = 0;
        for g in self.planes.iter_mut() {
            if budget == 0 {
                break;
            }
            let n = g.count.min(budget);
            g.count -= n;
            g.refitted = g.refitted.min(g.count);
            g.ready = g.ready.min(g.refitted);
            g.armed = g.armed.min(g.ready);
            budget -= n;
            lost += n;
        }
        self.planes.retain(|g| g.count > 0);
        lost
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex() -> HexCoord {
        HexCoord::new(10, 10)
    }

    #[test]
    fn respects_class_rule_and_capacity() {
        // German Staffel: fighter squadron, capacity 12 ready + 4 reserve = 16 (§35.23).
        let mut sq = Sgsu::new(
            SquadronId(1),
            Nationality::German,
            SquadronClass::Fighter,
            "JG 27",
            5,
            Some(hex()),
        );
        assert_eq!(sq.capacity(1).total(), 16);

        // A bomber may not join a fighter squadron (§35.21).
        assert!(!sq.add_planes("Ju.87D", AircraftClass::DiveBomber, 1, 1));
        assert_eq!(sq.plane_count(), 0);

        // Fighters fit; fill to the 16-plane total.
        assert!(sq.add_planes("Bf.109F", AircraftClass::Fighter, 12, 1));
        assert!(sq.add_planes("Bf.109G", AircraftClass::Fighter, 4, 1));
        assert_eq!(sq.plane_count(), 16);
        assert_eq!(sq.planes.len(), 2);

        // One more would exceed capacity.
        assert!(!sq.add_planes("Bf.109F", AircraftClass::Fighter, 1, 1));
        assert_eq!(sq.plane_count(), 16);
    }

    #[test]
    fn recon_fits_any_class_and_merges() {
        let mut sq = Sgsu::new(
            SquadronId(2),
            Nationality::Commonwealth,
            SquadronClass::Bomber,
            "39 Sqn",
            5,
            Some(hex()),
        );
        // Recon may be assigned anywhere (§35.21); same type merges into one group.
        assert!(sq.add_planes("Maryland", AircraftClass::Reconnaissance, 3, 1));
        assert!(sq.add_planes("Maryland", AircraftClass::Reconnaissance, 2, 1));
        assert_eq!(sq.planes.len(), 1);
        assert_eq!(sq.plane_count(), 5);
    }

    #[test]
    fn upkeep_constants_match_35_14() {
        assert_eq!(Sgsu::STORES_UPKEEP_PER_TURN, 1);
        assert_eq!(Sgsu::FUEL_UPKEEP_PER_STAGE, 1);
        assert_eq!(Sgsu::WATER_UPKEEP_PER_STAGE, 1);
    }

    #[test]
    fn facility_capacities_and_service_36() {
        use AirFacilityKind::*;
        assert_eq!(Airfield.max_capacity(), 6);
        assert_eq!(AirLandingStrip.max_capacity(), 1);
        assert_eq!(FlyingBoatBasin.max_capacity(), 3);
        assert_eq!(FlyingBoatAlightingArea.max_capacity(), 1);
        // Normal aircraft vs flying-boats (§34.4/§36.3).
        assert!(Airfield.serves_normal_aircraft() && !Airfield.serves_flying_boats());
        assert!(AirLandingStrip.serves_normal_aircraft());
        assert!(FlyingBoatBasin.serves_flying_boats() && !FlyingBoatBasin.serves_normal_aircraft());
        assert!(FlyingBoatAlightingArea.serves_flying_boats());
        // Intrinsic AA = 1 for all (§36.18).
        for k in [Airfield, AirLandingStrip, FlyingBoatBasin, FlyingBoatAlightingArea] {
            assert_eq!(k.intrinsic_aa(), 1);
        }
        // Only the alighting area is artillery-barrage immune (§36.4).
        assert!(FlyingBoatAlightingArea.immune_to_artillery_barrage());
        assert!(!FlyingBoatBasin.immune_to_artillery_barrage());
    }

    #[test]
    fn airfield_holds_six_sgsus_when_damaged_36_14() {        let mut af = AirFacility::new(AirFacilityKind::Airfield, hex());
        assert_eq!(af.capacity_level, 6);
        assert_eq!(af.max_sgsus(), 6);
        af.reduce_capacity(3);
        assert_eq!(af.functional_capacity(), 3);
        assert!(!af.is_destroyed());
        assert_eq!(af.max_sgsus(), 6, "airfield keeps 6-SGSU room even at reduced capacity");
        af.reduce_capacity(10);
        assert_eq!(af.capacity_level, 0);
        assert!(af.is_destroyed());
        af.restore_capacity(2);
        assert_eq!(af.capacity_level, 2);
    }

    #[test]
    fn plane_readiness_lifecycle_38() {
        use crate::reference::squadron::AircraftRefitTable;
        let table: AircraftRefitTable =
            ron::from_str(include_str!("../../../../data/reference/aircraft-refit.ron")).unwrap();
        // German Staffel: ready capacity 12 (§35.23).
        let mut sq = Sgsu::new(
            SquadronId(9),
            Nationality::German,
            SquadronClass::Fighter,
            "JG 27",
            5,
            Some(hex()),
        );
        sq.add_planes("Bf.109F", AircraftClass::Fighter, 16, 1); // 12 ready + 4 reserve
        assert_eq!(sq.refitted_planes(), 0);
        // Refit with roll 1 (modifier 0) -> 100% of the 16 undergoing refit.
        assert_eq!(sq.refit(&table, 1, 0), 16);
        assert_eq!(sq.refitted_planes(), 16);
        // Refuel to ready, capped at the 12 ready capacity (§37.24).
        assert_eq!(sq.refuel_to_ready(20, 1), 12);
        assert_eq!(sq.ready_planes(), 12);
        // Arm 8, then fly 10 -> they drop from ready AND refitted (need refit again).
        assert_eq!(sq.arm_up_to(8), 8);
        assert_eq!(sq.fly(10), 10);
        assert_eq!(sq.ready_planes(), 2);
        assert_eq!(sq.refitted_planes(), 6); // 16 refitted - 10 that flew
        assert_eq!(sq.armed_planes(), 2);
        // The 4 still-refitted (unfueled) planes can be refueled to ready without re-refitting.
        assert_eq!(sq.refuel_to_ready(20, 1), 4);
        assert_eq!(sq.ready_planes(), 6);
    }

    #[test]
    fn pilot_training_and_flight_40_1() {
        let mut p = Pilot::new(4, Some(SquadronId(1)));
        assert!(p.may_fly());
        // Reassign within the same plane type: grounded one stage (§40.13).
        p.reassign(Some(SquadronId(2)), false);
        assert!(!p.may_fly());
        p.advance_stage();
        assert!(p.may_fly());
        // Reassign onto a new plane type: grounded three stages (§40.14).
        p.reassign(Some(SquadronId(3)), true);
        assert_eq!(p.grounded_stages, 3);
        p.advance_stage();
        p.advance_stage();
        assert!(!p.may_fly());
        p.advance_stage();
        assert!(p.may_fly());
    }
}

/// The four air facility types (§36). Each handles a fixed maximum number of squadrons and either
/// normal aircraft (airfields, landing strips) or flying-boats (basins, alighting areas).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AirFacilityKind {
    /// §36.1 — fully-equipped field; max 6 squadrons; a supply dump (§36.17).
    Airfield,
    /// §36.2 — cleared strip, minimal maintenance; max 1 squadron; removed from the map when its
    /// capacity is destroyed.
    AirLandingStrip,
    /// §36.3 — "airfield" for flying-boats in a coastal hex; max 3 squadrons; seaplanes only.
    FlyingBoatBasin,
    /// §36.4 — like a basin but max 1 squadron and immune to artillery barrage (still bombable).
    FlyingBoatAlightingArea,
}

impl AirFacilityKind {
    /// Maximum squadron capacity level (§36.12/§36.2/§36.3/§36.4): airfield 6, strip 1, basin 3,
    /// alighting area 1.
    pub fn max_capacity(self) -> u32 {
        match self {
            AirFacilityKind::Airfield => 6,
            AirFacilityKind::FlyingBoatBasin => 3,
            AirFacilityKind::AirLandingStrip | AirFacilityKind::FlyingBoatAlightingArea => 1,
        }
    }

    /// Flying-boats must use a basin or alighting area (§34.4/§36.3); they may not use the others.
    pub fn serves_flying_boats(self) -> bool {
        matches!(
            self,
            AirFacilityKind::FlyingBoatBasin | AirFacilityKind::FlyingBoatAlightingArea
        )
    }

    /// Normal (non-flying-boat) aircraft use airfields and landing strips; basins may not be used
    /// for normal aircraft (§36.3).
    pub fn serves_normal_aircraft(self) -> bool {
        !self.serves_flying_boats()
    }

    /// Intrinsic anti-aircraft Strength Point (§36.18): one for every facility type (§36.2 and
    /// §36.4 inherit it from airfields/basins). Usable only vs strafing/dive-bombing missions.
    pub fn intrinsic_aa(self) -> u32 {
        1
    }

    /// Alighting areas are immune to artillery barrage (§36.4); all others may be barraged.
    pub fn immune_to_artillery_barrage(self) -> bool {
        matches!(self, AirFacilityKind::FlyingBoatAlightingArea)
    }

    /// A landing strip is eliminated and removed from the map at zero capacity (§36.2). An airfield
    /// at zero capacity is "destroyed for all purposes" but stays in play to be rebuilt (§36.14);
    /// §36 does not state removal for basins/alighting areas, so we do not fabricate one.
    pub fn removed_when_zero(self) -> bool {
        matches!(self, AirFacilityKind::AirLandingStrip)
    }
}

/// A placed air facility (§36) with its current, damageable capacity level. Air facilities are
/// non-denominational — usable by either side (§36.15).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AirFacility {
    pub kind: AirFacilityKind,
    pub hex: HexCoord,
    /// Current capacity level in squadrons (0..=max). Reduced by enemy bombardment (§36.14);
    /// 0 = destroyed for all purposes.
    pub capacity_level: u32,
    /// §36.5: an off-map facility (Tunisia boxes, Deversoir, etc.) — unlimited maintenance supply,
    /// may exceed the on-map SGSU limit up to `off_map_sgsu_limit`, and can never be destroyed by
    /// bombing (only reduced to zero, then rebuilt).
    #[serde(default)]
    pub off_map: bool,
    /// The SGSU limit printed on an off-map field (§36.5b). `None` on-map (the 6-SGSU airfield cap
    /// applies instead).
    #[serde(default)]
    pub off_map_sgsu_limit: Option<u32>,
}

impl AirFacility {
    /// A fully-built facility of `kind` at `hex` (capacity at its maximum).
    pub fn new(kind: AirFacilityKind, hex: HexCoord) -> Self {
        AirFacility { kind, hex, capacity_level: kind.max_capacity(), off_map: false, off_map_sgsu_limit: None }
    }

    /// Zero capacity = destroyed for all purposes (§36.14).
    pub fn is_destroyed(&self) -> bool {
        self.capacity_level == 0
    }

    /// The number of squadrons that may currently land at / be readied by this field (§36.13):
    /// its current capacity level.
    pub fn functional_capacity(&self) -> u32 {
        self.capacity_level
    }

    /// Maximum SGSU counters that may sit in the facility hex. An on-map airfield may keep 6 SGSUs
    /// regardless of its (possibly reduced) capacity level (§36.12/§36.14). Off-map fields use
    /// their listed limit (§36.5b). For strips/basins/alighting areas §36 gives no separate SGSU
    /// cap, so we use the facility's maximum capacity (a documented simplification).
    pub fn max_sgsus(&self) -> u32 {
        if self.off_map {
            self.off_map_sgsu_limit.unwrap_or_else(|| self.kind.max_capacity())
        } else {
            match self.kind {
                AirFacilityKind::Airfield => 6,
                _ => self.kind.max_capacity(),
            }
        }
    }

    /// Reduce capacity by `levels` from bombardment (§36.14), clamped at zero. Off-map fields are
    /// never destroyed by bombing but may still be reduced to zero (§36.5-3).
    pub fn reduce_capacity(&mut self, levels: u32) {
        self.capacity_level = self.capacity_level.saturating_sub(levels);
    }

    /// Rebuild capacity by `levels` (§36.14), clamped at the facility's maximum.
    pub fn restore_capacity(&mut self, levels: u32) {
        self.capacity_level = (self.capacity_level + levels).min(self.kind.max_capacity());
    }
}

/// A Maltese air facility (§44.1). Unlike mainland facilities, Malta's bases are printed on the map
/// and are permanent: they may be bombed down to zero effectiveness but never destroyed (§44.12),
/// and rebuilt only via the Malta Air Facility Construction Table, once per Game-Turn each, with no
/// supply cost (§44.13). Capacity is measured in "levels"; each level handles 18 planes (§44.14).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaltaAirBase {
    pub name: String,
    /// Standard maximum capacity level (the ceiling for construction, §44.13).
    pub max_levels: u32,
    /// Current effective capacity level (0..=max_levels); 0 = bombed out (§44.12).
    pub levels: u32,
    /// AA/Flak points diverted to this base (§44.18); Malta-wide these total at most 48.
    #[serde(default)]
    pub aa_points: u32,
    /// §44.13: whether the Construction Table has already been used on this base this Game-Turn.
    #[serde(default)]
    pub constructed_this_turn: bool,
}

impl MaltaAirBase {
    /// Planes a single capacity level can handle (§44.14).
    pub const LEVEL_CAPACITY: u32 = 18;
    /// Malta-wide maximum diverted AA points (§44.18).
    pub const MAX_AA_POINTS: u32 = 48;

    pub fn new(name: impl Into<String>, max_levels: u32, levels: u32) -> Self {
        MaltaAirBase { name: name.into(), max_levels, levels: levels.min(max_levels), aa_points: 0, constructed_this_turn: false }
    }

    /// §44.14: planes this base can currently handle (levels × 18).
    pub fn plane_capacity(&self) -> u32 {
        self.levels * Self::LEVEL_CAPACITY
    }

    /// §44.12: Axis bombing reduces capacity levels (never below zero; the base is not destroyed).
    pub fn bomb(&mut self, levels_lost: u32) {
        self.levels = self.levels.saturating_sub(levels_lost);
    }

    /// §44.13: build up to `levels` more capacity levels (once per Game-Turn, capped at the
    /// standard maximum, no supply cost). Returns the number of levels actually added.
    pub fn construct(&mut self, levels: u32) -> u32 {
        if self.constructed_this_turn {
            return 0;
        }
        let added = levels.min(self.max_levels.saturating_sub(self.levels));
        self.levels += added;
        self.constructed_this_turn = true;
        added
    }

    /// Reset the once-per-Game-Turn construction flag (§44.13).
    pub fn begin_turn(&mut self) {
        self.constructed_this_turn = false;
    }
}

#[cfg(test)]
mod malta_tests {
    use super::*;

    #[test]
    fn malta_base_capacity_bomb_and_construct_44() {
        // Luqa: standard max 6 levels, starting at 4 (scenario, §44.13).
        let mut luqa = MaltaAirBase::new("Luqa", 6, 4);
        assert_eq!(luqa.plane_capacity(), 72); // 4 × 18 (§44.14)

        // §44.12: bombing reduces levels but never destroys the base.
        luqa.bomb(2);
        assert_eq!(luqa.levels, 2);
        luqa.bomb(10);
        assert_eq!(luqa.levels, 0);
        assert_eq!(luqa.plane_capacity(), 0);

        // §44.13: construct up to the standard max, once per Game-Turn.
        assert_eq!(luqa.construct(3), 3);
        assert_eq!(luqa.levels, 3);
        // Second construction the same turn does nothing.
        assert_eq!(luqa.construct(2), 0);
        assert_eq!(luqa.levels, 3);
        // Next turn resets the allowance; construction is capped at the max (6).
        luqa.begin_turn();
        assert_eq!(luqa.construct(10), 3);
        assert_eq!(luqa.levels, 6);
    }
}
