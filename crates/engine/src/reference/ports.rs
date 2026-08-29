//! Port Capacity & Efficiency Level Chart (§55.3) — DATA layer. See `data/reference/port-capacity.ron`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A port's charted characteristics (§55.3). Stacking in/out are `None` when "na" (not allowed);
/// `Some(0)` means only a single zero-stacking-point unit may be shipped in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortSpec {
    /// Maximum (assigned) Efficiency Level.
    pub efficiency_level: u32,
    pub stacking_in: Option<u32>,
    pub stacking_out: Option<u32>,
    /// Maximum supply tonnage in/out per Operations Stage.
    pub max_tonnage: u32,
    /// Game-Turn from which the port may first be used (Bizerta, §55.17); `None` = always.
    #[serde(default)]
    pub available_from_gt: Option<u32>,
    /// Efficiency Levels blocked at campaign start (Tobruk's San Giorgio, §55.25).
    #[serde(default)]
    pub starts_blocked: u32,
}

impl PortSpec {
    /// A major port may receive both men and supplies; a minor port only supplies (§55.11). Modeled
    /// here as: a port that can ship stacking points IN is a major port. (Refined if the OB marks
    /// port type explicitly.)
    pub fn is_major(&self) -> bool {
        self.stacking_in.is_some()
    }

    /// Starting Efficiency Level (max minus any campaign-start blocking, §55.25).
    pub fn starting_efficiency(&self) -> u32 {
        self.efficiency_level.saturating_sub(self.starts_blocked)
    }
}

/// The Port Capacity & Efficiency Level Chart (§55.3): named ports plus a default for "all others".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortsChart {
    pub ports: HashMap<String, PortSpec>,
    pub default: PortSpec,
}

impl PortsChart {
    /// The spec for `port` (falling back to the "all others" default).
    pub fn spec(&self, port: &str) -> PortSpec {
        self.ports.get(port).copied().unwrap_or(self.default)
    }
}

/// A rally point from which the distance to a shipping lane is measured (§56.18). Sicily/Italy/Crete
/// are off-map boxes and Malta is any Malta hex; Benghazi/Derna/Tobruk are North African cities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RallyPoint {
    Sicily,
    Italy,
    Crete,
    Malta,
    Benghazi,
    Derna,
    Tobruk,
}

/// One of the six Axis shipping lanes (§56.18) with its origin/destination ports and the distance in
/// hexes from each rally point (`None` = no distance listed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShippingLane {
    pub nr: u8,
    pub origin: String,
    pub destination: String,
    pub sicily: Option<u32>,
    pub italy: Option<u32>,
    pub crete: Option<u32>,
    pub malta: Option<u32>,
    pub benghazi: Option<u32>,
    pub derna: Option<u32>,
    pub tobruk: Option<u32>,
}

impl ShippingLane {
    /// Distance in hexes from `rally` to this lane (`None` if not listed).
    pub fn distance_from(&self, rally: RallyPoint) -> Option<u32> {
        match rally {
            RallyPoint::Sicily => self.sicily,
            RallyPoint::Italy => self.italy,
            RallyPoint::Crete => self.crete,
            RallyPoint::Malta => self.malta,
            RallyPoint::Benghazi => self.benghazi,
            RallyPoint::Derna => self.derna,
            RallyPoint::Tobruk => self.tobruk,
        }
    }
}

/// The Axis Naval Convoy Air Distance Chart (§56.18): the six shipping lanes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShippingLanesChart {
    pub lanes: Vec<ShippingLane>,
}

impl ShippingLanesChart {
    /// The lane with description number `nr` (1–6).
    pub fn lane(&self, nr: u8) -> Option<&ShippingLane> {
        self.lanes.iter().find(|l| l.nr == nr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chart() -> PortsChart {
        ron::from_str(include_str!("../../../../data/reference/port-capacity.ron")).unwrap()
    }

    #[test]
    fn port_specs_55_3() {
        let c = chart();
        let tripoli = c.spec("Tripoli");
        assert_eq!(tripoli.efficiency_level, 10);
        assert_eq!(tripoli.max_tonnage, 15000);
        assert!(tripoli.is_major());
        // Tobruk starts 3 levels down (San Giorgio, §55.25): 5 - 3 = 2.
        let tobruk = c.spec("Tobruk");
        assert_eq!(tobruk.efficiency_level, 5);
        assert_eq!(tobruk.starting_efficiency(), 2);
        // Bizerta gated to GT 35.
        assert_eq!(c.spec("Bizerta").available_from_gt, Some(35));
        // Derna cannot ship in (minor), only out.
        let derna = c.spec("Derna");
        assert_eq!(derna.stacking_in, None);
        assert!(!derna.is_major());
        // Unknown port -> default "all others".
        let other = c.spec("Nofilia");
        assert_eq!(other.efficiency_level, 1);
        assert_eq!(other.max_tonnage, 100);
        assert_eq!(other.stacking_out, None);
    }

    #[test]
    fn shipping_lanes_56_18() {
        let c: ShippingLanesChart =
            ron::from_str(include_str!("../../../../data/reference/shipping-lanes.ron")).unwrap();
        assert_eq!(c.lanes.len(), 6);
        let l1 = c.lane(1).unwrap();
        assert_eq!(l1.origin, "West Sicily");
        assert_eq!(l1.destination, "Bizerta");
        assert_eq!(l1.distance_from(RallyPoint::Sicily), Some(20));
        assert_eq!(l1.distance_from(RallyPoint::Crete), None); // "–"
        assert_eq!(l1.distance_from(RallyPoint::Tobruk), Some(165));
        // Greece lanes 4 & 5.
        assert_eq!(c.lane(4).unwrap().origin, "Greece");
        assert_eq!(c.lane(5).unwrap().destination, "Tobruk");
        assert_eq!(c.lane(5).unwrap().distance_from(RallyPoint::Derna), Some(18));
    }
}
