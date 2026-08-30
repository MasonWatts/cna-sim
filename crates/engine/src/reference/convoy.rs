//! Axis Naval Convoy Level Chart (§56.4) + Capacity Table (§56.5) — DATA layer. See
//! `data/reference/convoy-levels.ron`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A convoy Level's tonnage formula (§56.5): `Fixed + Variable × die`, rounded up to 1,000 t.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConvoyLevel {
    pub fixed: u32,
    pub variable: u32,
}

impl ConvoyLevel {
    /// Tonnage shippable at this Level for a `die` roll (§56.5): Fixed + Variable × die, rounded up
    /// to the nearest 1,000 tons.
    pub fn tonnage(&self, die: u32) -> u32 {
        (self.fixed + self.variable * die).div_ceil(1000) * 1000
    }
}

/// The Axis Naval Convoy Level Chart (§56.4) + Capacity Table (§56.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConvoyLevelChart {
    /// Level letter (A–G) → its tonnage formula.
    pub capacity: HashMap<String, ConvoyLevel>,
    /// Year → the twelve monthly Level letters (Jan..Dec); `None` = no value (before campaign start
    /// or not yet transcribed).
    pub schedule: HashMap<u32, Vec<Option<String>>>,
}

impl ConvoyLevelChart {
    /// The convoy Level letter for `year`/`month` (1 = January), if scheduled.
    pub fn level_for(&self, year: u32, month: u32) -> Option<&str> {
        let row = self.schedule.get(&year)?;
        let idx = (month.checked_sub(1)?) as usize;
        row.get(idx)?.as_deref()
    }

    /// The tonnage the Axis may ship in `year`/`month` on a `die` roll (§56.4 → §56.5). `None` if
    /// that month has no scheduled Level or the Level is unknown.
    pub fn tonnage_for(&self, year: u32, month: u32, die: u32) -> Option<u32> {
        let level = self.level_for(year, month)?;
        self.capacity.get(level).map(|c| c.tonnage(die))
    }

    /// The best-case Shipping Tonnage the Axis could ship in `year`/`month` — the §56.5 formula is
    /// monotonic in the die, so this is the die-6 tonnage. Used as the §20.62 ceiling against which
    /// planned Axis Replacement-Point tonnage for that Game-Turn is counted.
    pub fn max_tonnage_for(&self, year: u32, month: u32) -> Option<u32> {
        self.tonnage_for(year, month, 6)
    }
}

/// The Air Reconnaissance of Axis Naval Convoys Table (§42.53): the number of Commonwealth recon
/// planes on a lane gives the maximum 2d6 roll that forces the Axis to reveal the lane's convoy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavalReconTable {
    /// `respond[min(planes,8)-1]`.
    pub respond: Vec<u32>,
}

impl NavalReconTable {
    /// The maximum successful 2d6 roll (encoded `10*d1+d2`) for `planes` recon planes.
    pub fn respond_max(&self, planes: u32) -> u32 {
        let idx = planes.clamp(1, 8) as usize - 1;
        self.respond.get(idx).copied().unwrap_or(0)
    }

    /// Does a `planes`-plane recon force the Axis to reveal the convoy on `roll` (§42.53: roll ≤ the
    /// Respond value)?
    pub fn reveals(&self, planes: u32, roll: u32) -> bool {
        planes >= 1 && roll <= self.respond_max(planes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chart() -> ConvoyLevelChart {
        ron::from_str(include_str!("../../../../data/reference/convoy-levels.ron")).unwrap()
    }

    #[test]
    fn capacity_table_56_5() {
        let c = chart();
        // §56.21 example: Level E, die 4 -> 21,000 t.
        assert_eq!(c.capacity["E"].tonnage(4), 21000);
        // Round up to the nearest 1,000: B die 1 = 7000 + 1500 = 8500 -> 9000.
        assert_eq!(c.capacity["B"].tonnage(1), 9000);
        assert_eq!(c.capacity["A"].tonnage(6), 12000); // 6000 + 6000
        assert_eq!(c.capacity["G"].tonnage(6), 50000); // 32000 + 18000
    }

    #[test]
    fn level_schedule_56_4() {
        let c = chart();
        // 1941 row: B E F E D G C E C D E A.
        assert_eq!(c.level_for(1941, 1), Some("B")); // Jan
        assert_eq!(c.level_for(1941, 11), Some("E")); // Nov (matches the 56.21 example)
        assert_eq!(c.level_for(1941, 12), Some("A")); // Dec
        // Full resolution: Nov 1941, die 4 -> 21,000 t.
        assert_eq!(c.tonnage_for(1941, 11, 4), Some(21000));
        // 1940 begins in September (campaign start); Sep-Dec = B.
        assert_eq!(c.level_for(1940, 8), None); // August: before campaign
        assert_eq!(c.level_for(1940, 9), Some("B"));
        assert_eq!(c.level_for(1940, 12), Some("B"));
        // 1942 row: C B B G F A F B D A G C.
        assert_eq!(c.level_for(1942, 1), Some("C"));
        assert_eq!(c.level_for(1942, 4), Some("G"));
        assert_eq!(c.level_for(1942, 12), Some("C"));
    }

    #[test]
    fn naval_recon_table_42_53() {
        let t: NavalReconTable =
            ron::from_str(include_str!("../../../../data/reference/naval-recon.ron")).unwrap();
        assert_eq!(t.respond_max(1), 12);
        assert_eq!(t.respond_max(4), 32);
        assert_eq!(t.respond_max(8), 66);
        assert_eq!(t.respond_max(12), 66); // 8+
        assert!(t.reveals(1, 12));
        assert!(!t.reveals(1, 13));
        assert!(t.reveals(5, 43));
        assert!(!t.reveals(5, 44));
    }
}
