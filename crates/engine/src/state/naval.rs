//! Tier-2 naval state: placed Ports and Axis Naval Convoys (§55–56). The charted characteristics
//! (capacity, max efficiency, lane distances, tonnage levels) live in the reference layer; this
//! module holds the mutable, in-play instances.

use serde::{Deserialize, Serialize};

use crate::reference::ports::PortSpec;
use crate::state::map::HexCoord;

/// A port in play (§55). Its current Efficiency Level rises and falls with damage/repair while its
/// maximum and base capacity come from the §55.3 chart (`PortSpec`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Port {
    pub name: String,
    /// Maximum (assigned) Efficiency Level (§55.3).
    pub max_efficiency: u32,
    /// Current Efficiency Level (0..=max). 0 = the port is inoperable.
    pub efficiency: u32,
    /// Base supply tonnage capacity per Operations Stage at full efficiency (§55.3).
    pub max_tonnage: u32,
    /// Air-laid mines in the harbour (§55.27); each holds one Efficiency Level down until removed by
    /// an engineer. Tracked separately because their removal differs from bomb-damage regen.
    #[serde(default)]
    pub mines: u32,
    /// Efficiency Levels held down by a scuttled wreck blocking the harbour (§55.25 San Giorgio at
    /// Tobruk). Cleared when the wreck is finally removed by engineers (§30.17).
    #[serde(default)]
    pub wreck_block: u32,
    /// §55.11/§55.13: personnel (TOE Stacking-Point) capacity that may be received per Operations
    /// Stage. `Some(0)` = a minor port that may not receive men (§55.11); `None` = uncharted.
    #[serde(default)]
    pub stacking_in: Option<u32>,
    /// §55.13: personnel (TOE Stacking-Point) capacity that may be shipped out per Operations Stage.
    #[serde(default)]
    pub stacking_out: Option<u32>,
}

impl Port {
    /// Create a port from its §55.3 spec, starting at its (possibly campaign-blocked) efficiency
    /// (§55.25 San Giorgio reduces Tobruk's start by three).
    pub fn from_spec(name: impl Into<String>, spec: &PortSpec) -> Self {
        Port {
            name: name.into(),
            max_efficiency: spec.efficiency_level,
            efficiency: spec.starting_efficiency(),
            max_tonnage: spec.max_tonnage,
            mines: 0,
            wreck_block: spec.starts_blocked,
            stacking_in: spec.stacking_in,
            stacking_out: spec.stacking_out,
        }
    }

    /// The port is destroyed/inoperable when at zero Efficiency Level.
    pub fn is_operable(&self) -> bool {
        self.efficiency > 0
    }

    /// §30.56/§30.58: whether the port may receive or send *personnel*. A port incapacitated by
    /// bombing (Efficiency 0) never can; blocking/damage otherwise permits personnel until the port
    /// falls below two-thirds (rounded up) of its maximum Efficiency Level.
    pub fn personnel_operable(&self) -> bool {
        self.efficiency > 0 && self.efficiency * 3 >= self.max_efficiency * 2
    }

    /// Reduce the Efficiency Level by `levels` (bombing/scuttling/mines), clamped at zero (§55.14).
    pub fn reduce_efficiency(&mut self, levels: u32) {
        self.efficiency = self.efficiency.saturating_sub(levels);
    }

    /// §55.18: regain one Efficiency Level (a stage with no bomb loss), never above the maximum less
    /// any Efficiency Levels still held down by unremoved mines (§55.27).
    pub fn regain_efficiency(&mut self) {
        let ceiling = self.max_efficiency.saturating_sub(self.mines + self.wreck_block);
        if self.efficiency < ceiling {
            self.efficiency += crate::rules::naval::PORT_EFFICIENCY_REGEN_PER_STAGE;
        }
    }

    /// Current effective supply capacity this Operations Stage (§55.14): base × current/max, round
    /// up.
    pub fn effective_capacity(&self) -> u32 {
        crate::rules::naval::effective_port_capacity(
            self.max_tonnage,
            self.efficiency,
            self.max_efficiency,
        )
    }

    /// §55.27: an air-laid mine lowers the Efficiency Level by one and is tracked until removed.
    pub fn lay_mine(&mut self) {
        self.mines += 1;
        self.reduce_efficiency(1);
    }

    /// §55.27: an engineer attempts to remove one mine (die 1–3 succeeds). Returns whether a mine
    /// was removed. The freed Efficiency Level is recovered subsequently via `regain_efficiency`.
    pub fn remove_mine(&mut self, die: u32) -> bool {
        if self.mines > 0 && crate::rules::naval::harbor_mine_removed(die) {
            self.mines -= 1;
            true
        } else {
            false
        }
    }
}

/// A planned Axis naval convoy on a shipping lane (§56). Convoys are planned one Game-Turn ahead
/// (§56.15) and carry supply tonnage and/or replacement tonnage to a destination port.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Convoy {
    /// Shipping-lane number 1–6 (§56.18).
    pub lane: u8,
    /// Destination port (the lane's destination, §56.11).
    pub destination: String,
    /// The Game-Turn the convoy sails/arrives (planned the previous turn, §56.15).
    pub arrival_gt: u32,
    pub fuel_tons: u32,
    pub ammo_tons: u32,
    pub stores_tons: u32,
    /// Replacement-point tonnage carried (§56.24).
    pub replacement_tons: u32,
    /// §56.15: set when the destination port is captured by the Commonwealth — the convoy never
    /// sails.
    #[serde(default)]
    pub cancelled: bool,
}

impl Convoy {
    /// Total tonnage assigned to the convoy (§56.12) — supplies plus replacements.
    pub fn total_tonnage(&self) -> u32 {
        self.fuel_tons + self.ammo_tons + self.stores_tons + self.replacement_tons
    }

    /// §56.16: Flak/AA Points the convoy fires when bombarded (total tonnage / 1000).
    pub fn flak_points(&self) -> u32 {
        crate::rules::naval::convoy_flak_points(self.total_tonnage())
    }
}

/// Where a Commonwealth warship is (§30.15/§30.18). Ships don't move; they are placed in their
/// assignment hex each Operations Stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShipLocation {
    /// The home port (repair + recovery, §30.37).
    Alexandria,
    /// Malta's port (max five ships, §30.18).
    Valetta,
    /// Assigned to a sea/coastal hex for bombardment/AA.
    AtSea(HexCoord),
}

/// An (abstracted) Commonwealth warship (§30.1). Its Gun Rating is both its barrage strength and
/// its damage capacity; the AA Rating scales with remaining Gun Rating (§30.13).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ship {
    pub name: String,
    /// Face-value Gun Rating.
    pub max_gun: u32,
    /// Current Gun Rating (0 = sunk).
    pub gun_rating: u32,
    /// AA Rating at full Gun Rating (§30.12).
    pub max_aa: u32,
    pub location: ShipLocation,
    /// Consecutive Operations Stages at sea (§30.16: no more than three).
    #[serde(default)]
    pub at_sea_stages: u32,
    /// Consecutive Operations Stages spent in Alexandria toward the next repaired Gun level
    /// (§30.37: six per level).
    #[serde(default)]
    pub repair_stages: u32,
    /// §30.24: whether this ship has already used its Barrage Strength this Operations Stage.
    #[serde(default)]
    pub fired_this_stage: bool,
    /// §30.16/§30.25: Operations Stages the ship still owes in port before it may put to sea
    /// again (two per stage at sea; at least two in Alexandria after an off-shore bombardment).
    #[serde(default)]
    pub rest_owed: u32,
}

impl Ship {
    /// Operations Stages a ship must spend in Alexandria to repair one Gun Rating level (§30.37).
    pub const STAGES_PER_REPAIR: u32 = 6;
    /// Consecutive Operations Stages a ship may stay at sea (§30.16).
    pub const MAX_SEA_STAGES: u32 = 3;

    /// A fully-repaired ship in Alexandria.
    pub fn new(name: impl Into<String>, gun: u32, aa: u32) -> Self {
        Ship { name: name.into(), max_gun: gun, gun_rating: gun, max_aa: aa, location: ShipLocation::Alexandria, at_sea_stages: 0, repair_stages: 0, fired_this_stage: false, rest_owed: 0 }
    }

    /// §30.13: current AA Rating, reduced in proportion to Gun Rating loss.
    pub fn current_aa(&self) -> u32 {
        (self.max_aa * self.gun_rating).checked_div(self.max_gun).unwrap_or(0)
    }

    /// §30.36: a ship is sunk when its entire Gun Rating is eliminated (unrepairable).
    pub fn is_sunk(&self) -> bool {
        self.gun_rating == 0
    }

    /// §30.36: a ship below 50% of its face Gun Rating may not leave port until repaired above it.
    pub fn seaworthy(&self) -> bool {
        self.gun_rating * 2 >= self.max_gun && self.gun_rating > 0
    }

    /// §30.21: battleships and heavy cruisers (Gun Rating 4+) may bombard a hex adjacent to their
    /// coastal hex (at half strength, §30.22).
    pub fn is_heavy(&self) -> bool {
        self.max_gun >= 4
    }

    /// §30.22: off-shore bombardment strength — the Gun Rating, halved when firing at a hex two
    /// away (adjacent to the ship's coastal hex).
    pub fn bombardment_strength(&self, two_hexes: bool) -> u32 {
        if two_hexes {
            self.gun_rating / 2
        } else {
            self.gun_rating
        }
    }

    /// §30.34: take `points` of air-bombardment damage in Gun Rating Points.
    pub fn damage(&mut self, points: u32) {
        self.gun_rating = self.gun_rating.saturating_sub(points);
        self.repair_stages = 0;
    }

    /// §30.37: spend one Operations Stage in Alexandria repairing; every sixth consecutive stage
    /// restores one Gun Rating level (never above the face value). No-op if sunk or already full.
    pub fn repair_tick(&mut self) {
        if self.is_sunk() || self.gun_rating >= self.max_gun {
            return;
        }
        self.repair_stages += 1;
        if self.repair_stages >= Self::STAGES_PER_REPAIR {
            self.gun_rating += 1;
            self.repair_stages = 0;
        }
    }

    /// Per-Operations-Stage upkeep for a ship: clear the once-per-stage fire flag (§30.24) and,
    /// while in port, pay down one owed rest stage (§30.16/§30.25).
    pub fn begin_stage(&mut self) {
        self.fired_this_stage = false;
        if !matches!(self.location, ShipLocation::AtSea(_)) {
            self.rest_owed = self.rest_owed.saturating_sub(1);
        }
    }

    /// §30.16/§30.25/§30.36: whether the ship may be assigned to sea this stage — it must be
    /// seaworthy, owe no rest, and not already have spent three consecutive stages at sea.
    pub fn can_put_to_sea(&self) -> bool {
        self.seaworthy() && self.rest_owed == 0 && self.at_sea_stages < Self::MAX_SEA_STAGES
    }
}

/// The San Giorgio (§30.17): an Italian warship partially scuttled to block Tobruk harbour. It may
/// not move and, per the setup note (§60/§61 note c), it may **not** barrage land targets and may
/// never be attacked by land units; it may freely fire at Commonwealth warships (it is "forward")
/// and at aircraft, using its barrage and anti-air ratings respectively. It cannot be refloated but
/// may be destroyed (by Commonwealth engineers finishing the sinking), at which point the harbour
/// clears.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SanGiorgio {
    /// The Tobruk hex it sits in.
    pub hex: HexCoord,
    /// The name of the harbour it blocks (§55.2) while present.
    pub blocks_port: String,
    /// Current Gun Rating (0 = completely sunk / removed) — its Barrage Points vs ships.
    pub gun_rating: u32,
    /// Face-value Gun Rating.
    pub max_gun: u32,
    /// Anti-air rating (§60 note c): AA Points it fires at Commonwealth aircraft.
    #[serde(default)]
    pub aa_rating: u32,
}

impl SanGiorgio {
    pub fn new(hex: HexCoord, blocks_port: impl Into<String>, gun: u32, aa: u32) -> Self {
        SanGiorgio { hex, blocks_port: blocks_port.into(), gun_rating: gun, max_gun: gun, aa_rating: aa }
    }

    /// §30.17: completely sunk (removed); the blocked harbour then clears.
    pub fn is_removed(&self) -> bool {
        self.gun_rating == 0
    }

    /// §30.17/§30.35: emplaced guns fire their full (un-halved) Gun Rating as Bombardment Points
    /// against Commonwealth warships.
    pub fn bombardment_points(&self) -> u32 {
        self.gun_rating
    }

    /// §60 note c: AA Points fired at Commonwealth aircraft over/adjacent to Tobruk (zero once the
    /// wreck is destroyed).
    pub fn aa_points(&self) -> u32 {
        if self.is_removed() {
            0
        } else {
            self.aa_rating
        }
    }

    /// Engineer demolition / battle damage reducing the wreck's Gun Rating (§30.17).
    pub fn damage(&mut self, points: u32) {
        self.gun_rating = self.gun_rating.saturating_sub(points);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::ports::PortsChart;
    fn chart() -> PortsChart {
        ron::from_str(include_str!("../../../../data/reference/port-capacity.ron")).unwrap()
    }

    #[test]
    fn port_from_spec_and_capacity() {
        let c = chart();
        // Benghazi: max eff 3, capacity 2500.
        let mut benghazi = Port::from_spec("Benghazi", &c.spec("Benghazi"));
        assert_eq!(benghazi.efficiency, 3);
        assert_eq!(benghazi.effective_capacity(), 2500);
        // Reduce to level 1 -> 834 (round up, §55.14).
        benghazi.reduce_efficiency(2);
        assert_eq!(benghazi.efficiency, 1);
        assert_eq!(benghazi.effective_capacity(), 834);
        // Regain a level (§55.18).
        benghazi.regain_efficiency();
        assert_eq!(benghazi.efficiency, 2);

        // Tobruk starts at 2 (5 - 3 San Giorgio).
        let tobruk = Port::from_spec("Tobruk", &c.spec("Tobruk"));
        assert_eq!(tobruk.max_efficiency, 5);
        assert_eq!(tobruk.efficiency, 2);

        // A mine holds the regen ceiling down (§55.27).
        let mut derna = Port::from_spec("Derna", &c.spec("Derna"));
        derna.reduce_efficiency(1); // now 0
        derna.mines = 1;
        derna.regain_efficiency();
        assert_eq!(derna.efficiency, 0, "mine holds efficiency at 0 (ceiling = max 1 - 1 mine)");
    }

    #[test]
    fn mines_lay_and_remove_55_27() {        let c = chart();
        let mut tobruk = Port::from_spec("Tobruk", &c.spec("Tobruk")); // eff 2, max 5
        tobruk.lay_mine();
        assert_eq!(tobruk.mines, 1);
        assert_eq!(tobruk.efficiency, 1); // -1 per mine
        // Engineer removes on 1-3.
        assert!(!tobruk.remove_mine(4));
        assert_eq!(tobruk.mines, 1);
        assert!(tobruk.remove_mine(2));
        assert_eq!(tobruk.mines, 0);
        // Freed level recovered via regen (ceiling back up to max).
        tobruk.regain_efficiency();
        assert_eq!(tobruk.efficiency, 2);
    }

    #[test]
    fn ship_rest_cycle_30_16() {
        // §30.16: no more than three consecutive Operations Stages at sea, and two port stages
        // owed per stage at sea.
        let mut s = Ship::new("Warspite", 6, 12);
        assert!(s.can_put_to_sea());
        // Three consecutive sea stages allowed.
        for n in 1..=3 {
            s.at_sea_stages = n - 1;
            assert!(s.can_put_to_sea(), "stage {n} at sea should be allowed");
        }
        // A fourth consecutive sea stage is not.
        s.at_sea_stages = 3;
        assert!(!s.can_put_to_sea());
        // Owed rest also blocks sea and is paid down one stage at a time while in port.
        s.at_sea_stages = 0;
        s.rest_owed = 2;
        assert!(!s.can_put_to_sea());
        s.location = ShipLocation::Alexandria;
        s.begin_stage();
        assert_eq!(s.rest_owed, 1);
        s.begin_stage();
        assert_eq!(s.rest_owed, 0);
        assert!(s.can_put_to_sea());
        // At sea, rest is not paid down (and the fire flag resets each stage).
        s.location = ShipLocation::AtSea(HexCoord::new(2, 2));
        s.rest_owed = 2;
        s.fired_this_stage = true;
        s.begin_stage();
        assert_eq!(s.rest_owed, 2);
        assert!(!s.fired_this_stage);
    }

    #[test]
    fn ship_damage_aa_and_repair_30() {
        // Ajax, a heavy cruiser: Gun 4, AA 8.
        let mut ajax = Ship::new("Ajax", 4, 8);
        assert!(ajax.is_heavy());
        assert_eq!(ajax.current_aa(), 8);
        assert!(ajax.seaworthy());
        // §30.34: two damage -> Gun 2, AA halved to 4 (proportional, §30.13).
        ajax.damage(2);
        assert_eq!(ajax.gun_rating, 2);
        assert_eq!(ajax.current_aa(), 4);
        // §30.22: off-shore bombardment strength (halved at two hexes).
        assert_eq!(ajax.bombardment_strength(false), 2);
        assert_eq!(ajax.bombardment_strength(true), 1);
        // §30.36: at exactly 50% still seaworthy; below is not.
        assert!(ajax.seaworthy()); // 2 of 4 = 50%
        ajax.damage(1); // Gun 1 (< 50%)
        assert!(!ajax.seaworthy());
        // §30.37: six consecutive stages in Alexandria repair one Gun level.
        for _ in 0..5 { ajax.repair_tick(); }
        assert_eq!(ajax.gun_rating, 1);
        ajax.repair_tick(); // sixth
        assert_eq!(ajax.gun_rating, 2);
        // §30.36: all Gun eliminated -> sunk (unrepairable).
        ajax.damage(2);
        assert!(ajax.is_sunk());
        ajax.repair_tick();
        assert_eq!(ajax.gun_rating, 0);
    }
}
