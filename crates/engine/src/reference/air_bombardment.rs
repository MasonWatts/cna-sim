//! Air Bombardment & Secondary Barrage Targets Table (§41.5) — DATA layer. Total weapon points pick
//! a column (Bomb Points normally; Torpedo Points only vs the Commonwealth Fleet, §41.7 note a) and
//! a 2d6 roll read on the target group's rows gives the result. See `data/reference/air-bombardment.ron`.

use serde::{Deserialize, Serialize};

/// Which weapon-point scale selects the column (§41.5 header). Torpedo Points are used only when
/// attacking Commonwealth Fleet ships; torpedoes vs ports or convoys use the Bomb Points scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponPoints {
    Bomb,
    Torpedo,
}

/// The target row-groups of the CRT. Airfields, air landing strips and ports share one group;
/// trucks, flak suppression, combat units and the Commonwealth Fleet share another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetGroup {
    AirfieldPortStrip,
    SupplyDump,
    Fortification,
    Railroad,
    Road,
    Units,
    AxisNavalConvoy,
}

/// One result row: `result` (level count / percent / 0-or-1 categorical, per the group) and the
/// per-column 2d6 range (encoded `10*d1+d2`) that yields it (`None` = unreachable in that column).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BombResultRow {
    pub result: i32,
    pub ranges: Vec<Option<(u32, u32)>>,
}

/// The Air Bombardment & Secondary Barrage Targets Table (§41.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AirBombardmentTable {
    pub bomb_bands: Vec<u32>,
    pub torpedo_bands: Vec<u32>,
    pub airfield_port_strip: Vec<BombResultRow>,
    pub supply_dump: Vec<BombResultRow>,
    pub fortification: Vec<BombResultRow>,
    pub railroad: Vec<BombResultRow>,
    pub road: Vec<BombResultRow>,
    pub units: Vec<BombResultRow>,
    pub axis_naval_convoy: Vec<BombResultRow>,
}

impl AirBombardmentTable {
    fn rows(&self, group: TargetGroup) -> &[BombResultRow] {
        match group {
            TargetGroup::AirfieldPortStrip => &self.airfield_port_strip,
            TargetGroup::SupplyDump => &self.supply_dump,
            TargetGroup::Fortification => &self.fortification,
            TargetGroup::Railroad => &self.railroad,
            TargetGroup::Road => &self.road,
            TargetGroup::Units => &self.units,
            TargetGroup::AxisNavalConvoy => &self.axis_naval_convoy,
        }
    }

    /// The column index for `points` on the given `weapon` scale (first band whose bound ≥ points).
    pub fn column_index(&self, weapon: WeaponPoints, points: u32) -> usize {
        let bands = match weapon {
            WeaponPoints::Bomb => &self.bomb_bands,
            WeaponPoints::Torpedo => &self.torpedo_bands,
        };
        bands
            .iter()
            .position(|&b| points <= b)
            .unwrap_or(bands.len().saturating_sub(1))
    }

    /// The raw result for a `group` target hit by `points` weapon points and a 2d6 `roll` (encoded
    /// `10*d1+d2`). Returns the matching row's result, or 0 if no row covers the roll (e.g. a roll
    /// that only ever yields 0 in an all-`None` column).
    pub fn result(&self, group: TargetGroup, weapon: WeaponPoints, points: u32, roll: u32) -> i32 {
        let col = self.column_index(weapon, points);
        for row in self.rows(group) {
            if let Some(&Some((lo, hi))) = row.ranges.get(col) {
                if (lo..=hi).contains(&roll) {
                    return row.result;
                }
            }
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> AirBombardmentTable {
        ron::from_str(include_str!("../../../../data/reference/air-bombardment.ron")).unwrap()
    }

    #[test]
    fn column_selection_41_5() {
        let t = table();
        assert_eq!(t.column_index(WeaponPoints::Bomb, 20), 0);
        assert_eq!(t.column_index(WeaponPoints::Bomb, 21), 1);
        assert_eq!(t.column_index(WeaponPoints::Bomb, 200), 5);
        assert_eq!(t.column_index(WeaponPoints::Bomb, 471), 10);
        assert_eq!(t.column_index(WeaponPoints::Bomb, 99999), 10);
        // Torpedo scale differs.
        assert_eq!(t.column_index(WeaponPoints::Torpedo, 5), 0);
        assert_eq!(t.column_index(WeaponPoints::Torpedo, 50), 3);
        assert_eq!(t.column_index(WeaponPoints::Torpedo, 341), 10);
    }

    #[test]
    fn crt_results_41_5() {
        let t = table();
        // Airfield/Port/Strip, low bombs (col0 1-20): 11-62 -> 0, 63-66 -> 1.
        assert_eq!(t.result(TargetGroup::AirfieldPortStrip, WeaponPoints::Bomb, 10, 62), 0);
        assert_eq!(t.result(TargetGroup::AirfieldPortStrip, WeaponPoints::Bomb, 10, 63), 1);
        // Airfield with heavy bombs (col10 471+): 62-66 -> 4.
        assert_eq!(t.result(TargetGroup::AirfieldPortStrip, WeaponPoints::Bomb, 500, 66), 4);
        // Units at col3 (81-120): 11-12 -> 0, 13-42 -> 1, 43-66 -> 2.
        assert_eq!(t.result(TargetGroup::Units, WeaponPoints::Bomb, 100, 12), 0);
        assert_eq!(t.result(TargetGroup::Units, WeaponPoints::Bomb, 100, 42), 1);
        assert_eq!(t.result(TargetGroup::Units, WeaponPoints::Bomb, 100, 43), 2);
        // Supply dump percentages: col0 63-66 -> 10%.
        assert_eq!(t.result(TargetGroup::SupplyDump, WeaponPoints::Bomb, 10, 63), 10);
        // Fortification: col0 66 -> Reduced (1), 65 -> No Effect (0).
        assert_eq!(t.result(TargetGroup::Fortification, WeaponPoints::Bomb, 10, 66), 1);
        assert_eq!(t.result(TargetGroup::Fortification, WeaponPoints::Bomb, 10, 65), 0);
        // Axis naval convoy: col0 always 0%; col1 65-66 -> 10%.
        assert_eq!(t.result(TargetGroup::AxisNavalConvoy, WeaponPoints::Bomb, 20, 66), 0);
        assert_eq!(t.result(TargetGroup::AxisNavalConvoy, WeaponPoints::Bomb, 30, 66), 10);
    }
}
