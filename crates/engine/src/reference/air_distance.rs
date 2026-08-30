//! §37.4 Air Distance Chart: the point-to-point air distances (in hexes) between the off-map areas
//! and the named strategic targets, used for flight range from off-map bases (§36.53/§37.12).
//!
//! Part A (Axis off-map N. African areas: Tripolitania/Tripoli/Gabes/Tunis) and Part B (N.
//! Mediterranean off-map areas: Crete/Malta/Sicily/Italy) are stored as one flat `(from, to)`
//! table. `Prohibited` = the "P" cells (no direct air route between the two points).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One air-distance cell (§37.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AirDist {
    /// A direct air route of this many hexes.
    Hexes(u32),
    /// "P": no direct air distance between the two points.
    Prohibited,
}

/// The §37.4 Air Distance Chart: `(from, to)` → distance. Symmetric in intent, but stored exactly
/// as printed (look-ups try both orderings).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AirDistanceChart {
    /// Keyed by `(from, to)` place names as printed on the chart.
    pub cells: HashMap<(String, String), AirDist>,
}

impl AirDistanceChart {
    /// The air distance from `from` to `to` (§37.4), trying both key orderings; `None` if the pair
    /// is not on the chart.
    pub fn distance(&self, from: &str, to: &str) -> Option<AirDist> {
        self.cells
            .get(&(from.to_string(), to.to_string()))
            .or_else(|| self.cells.get(&(to.to_string(), from.to_string())))
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn air_distance_37_4() {
        let c: AirDistanceChart =
            ron::from_str(include_str!("../../../../data/reference/air-distance.ron")).unwrap();
        // Part B (N. Mediterranean): the Malta column and a few Crete rows.
        assert_eq!(c.distance("Sicily", "Malta"), Some(AirDist::Hexes(24)));
        assert_eq!(c.distance("Italy", "Malta"), Some(AirDist::Hexes(72)));
        assert_eq!(c.distance("Crete", "Derna"), Some(AirDist::Hexes(27)));
        assert_eq!(c.distance("Crete", "Alexandria"), Some(AirDist::Hexes(84)));
        // Prohibited cells, and symmetric lookup.
        assert_eq!(c.distance("Sicily", "Alexandria"), Some(AirDist::Prohibited));
        assert_eq!(c.distance("Malta", "Sicily"), Some(AirDist::Hexes(24))); // reversed key
        // Part A (N. African areas).
        assert_eq!(c.distance("Gabes", "Tripoli"), Some(AirDist::Hexes(40)));
        assert_eq!(c.distance("Benghazi", "Tunis"), Some(AirDist::Hexes(136)));
        assert_eq!(c.distance("Crete", "Nowhere"), None);
    }
}
