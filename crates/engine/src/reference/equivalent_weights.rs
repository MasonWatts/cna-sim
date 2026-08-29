//! Equivalent Weights Chart (§54.5) — DATA layer. Tons-per-point conversions for shipping supplies
//! and points by naval convoy, interport, rail or air. See `data/reference/equivalent-weights.ron`.

use serde::{Deserialize, Serialize};

/// An exact tons-per-point rate as a fraction `num/den` (e.g. Fuel = 1/8 ton per point).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rate {
    pub num: u32,
    pub den: u32,
}

impl Rate {
    /// Tonnage of `points` at this rate (`points × num / den`).
    pub fn tonnage(self, points: u32) -> u32 {
        points * self.num / self.den
    }

    /// Points that fit in `tons` at this rate (`tons × den / num`).
    pub fn points(self, tons: u32) -> u32 {
        tons * self.den / self.num
    }
}

/// The Equivalent Weights Chart (§54.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquivalentWeights {
    pub ammo: Rate,
    pub fuel: Rate,
    pub stores: Rate,
    pub water: Rate,
    /// Tons per replacement point transported by air (infantry-class only, §54.5 key *).
    pub air_replacement_tons: u32,
    /// Tons per truck/motorization point by interport (§54.5 key †).
    pub interport_truck_tons: u32,
    // Stacking-point equivalents for rail/interport.
    pub sp_truck: Rate,
    pub sp_replacement: Rate,
    pub sp_sgsu: Rate,
    pub sp_unit: Rate,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chart() -> EquivalentWeights {
        ron::from_str(include_str!("../../../../data/reference/equivalent-weights.ron")).unwrap()
    }

    #[test]
    fn supply_tonnage_54_5() {
        let e = chart();
        // Ammo 4 t/pt, Stores 1, Fuel 1/8, Water 1/6.
        assert_eq!(e.ammo.tonnage(10), 40);
        assert_eq!(e.stores.tonnage(1500), 1500);
        assert_eq!(e.fuel.tonnage(8), 1); // 8 fuel points = 1 ton
        assert_eq!(e.water.tonnage(6), 1);
        // Points that fit in a tonnage budget (matches the rail 1500 t figures).
        assert_eq!(e.stores.points(1500), 1500);
        assert_eq!(e.ammo.points(1500), 375); // 1500 / 4
        assert_eq!(e.fuel.points(1500), 12000); // 1500 * 8
        // §54.5 replacement / truck.
        assert_eq!(e.air_replacement_tons, 2);
        assert_eq!(e.interport_truck_tons, 50);
        assert_eq!(e.sp_truck, Rate { num: 1, den: 10 });
    }
}
