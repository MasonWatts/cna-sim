//! Anti-Aircraft Combat Results Table (§46.3) + Flak Adjustment Chart (§46.4) — DATA layer. Total
//! flak points pick a column (shifted by the §46.4 plane-density adjustment and any night
//! adjustment, §41.48); a 2d6 roll read on the section's rows gives planes destroyed/aborted. See
//! `data/reference/anti-aircraft.ron`.

use serde::{Deserialize, Serialize};

/// Which section of the AA CRT to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AaSection {
    /// Planes on other (non-fighter) missions — number destroyed.
    OtherDestroyed,
    /// Planes on other missions — number aborted (rolled separately, same column).
    OtherAborted,
    /// Planes on fighter-type missions — number destroyed.
    FighterDestroyed,
}

/// One §46.4 density band: up to `max_planes` non-fighter planes shifts the flak column `columns`
/// places in favor of the firing player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DensityBand {
    pub max_planes: u32,
    pub columns: i32,
}

/// One result row: the `result` (planes destroyed/aborted) and per-column 2d6 range (encoded
/// `10*d1+d2`) that yields it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AaResultRow {
    pub result: i32,
    pub ranges: Vec<Option<(u32, u32)>>,
}

/// The Anti-Aircraft Combat Results Table (§46.3) plus the Flak Adjustment Chart (§46.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AntiAircraftTable {
    pub flak_bands: Vec<u32>,
    pub density_adjustment: Vec<DensityBand>,
    pub other_destroyed: Vec<AaResultRow>,
    pub other_aborted: Vec<AaResultRow>,
    pub fighter_destroyed: Vec<AaResultRow>,
}

impl AntiAircraftTable {
    fn rows(&self, section: AaSection) -> &[AaResultRow] {
        match section {
            AaSection::OtherDestroyed => &self.other_destroyed,
            AaSection::OtherAborted => &self.other_aborted,
            AaSection::FighterDestroyed => &self.fighter_destroyed,
        }
    }

    /// §46.4: the column shift (in favor of the firing player) for `non_fighter_planes` overhead.
    pub fn density_adjustment(&self, non_fighter_planes: u32) -> i32 {
        self.density_adjustment
            .iter()
            .find(|b| non_fighter_planes <= b.max_planes)
            .map(|b| b.columns)
            .unwrap_or(0)
    }

    /// The base column index for `flak_points` (first band whose bound ≥ the points).
    pub fn column_index(&self, flak_points: u32) -> usize {
        self.flak_bands
            .iter()
            .position(|&b| flak_points <= b)
            .unwrap_or(self.flak_bands.len().saturating_sub(1))
    }

    /// The result for `flak_points` flak with a signed column `shift` (positive = right/stronger,
    /// e.g. the §46.4 density adjustment; negative = left, e.g. the §41.48 night −1) and a 2d6
    /// `roll`. The shift is clamped to valid columns.
    pub fn result(&self, section: AaSection, flak_points: u32, shift: i32, roll: u32) -> i32 {
        let last = self.flak_bands.len() as i32 - 1;
        let col = (self.column_index(flak_points) as i32 + shift).clamp(0, last) as usize;
        for row in self.rows(section) {
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

    fn table() -> AntiAircraftTable {
        ron::from_str(include_str!("../../../../data/reference/anti-aircraft.ron")).unwrap()
    }

    #[test]
    fn density_and_columns_46_4() {
        let t = table();
        assert_eq!(t.density_adjustment(23), 0);
        assert_eq!(t.density_adjustment(24), 1);
        assert_eq!(t.density_adjustment(47), 1);
        assert_eq!(t.density_adjustment(48), 2);
        assert_eq!(t.density_adjustment(72), 3);
        assert_eq!(t.column_index(4), 0);
        assert_eq!(t.column_index(5), 1);
        assert_eq!(t.column_index(40), 9);
    }

    #[test]
    fn aa_crt_results_46_3() {
        let t = table();
        // Other-missions destroyed, col0 (1-4 flak): 11-55 -> 0, 56-66 -> 1.
        assert_eq!(t.result(AaSection::OtherDestroyed, 3, 0, 55), 0);
        assert_eq!(t.result(AaSection::OtherDestroyed, 3, 0, 56), 1);
        // Other-missions aborted, col0: 11-53 -> 0, 54-66 -> 1.
        assert_eq!(t.result(AaSection::OtherAborted, 3, 0, 53), 0);
        assert_eq!(t.result(AaSection::OtherAborted, 3, 0, 54), 1);
        // Fighter destroyed, col5 (21-24 flak): 11-16 -> 0, 21-62 -> 1, 63-66 -> 2.
        assert_eq!(t.result(AaSection::FighterDestroyed, 22, 0, 16), 0);
        assert_eq!(t.result(AaSection::FighterDestroyed, 22, 0, 62), 1);
        assert_eq!(t.result(AaSection::FighterDestroyed, 22, 0, 63), 2);
        // Column shift: +1 density on 1-4 flak reads column 1 (5-8).
        assert_eq!(
            t.result(AaSection::OtherDestroyed, 3, 1, 66),
            t.result(AaSection::OtherDestroyed, 6, 0, 66)
        );
        // High flak, top rolls reach the max results.
        assert_eq!(t.result(AaSection::OtherAborted, 40, 0, 66), 7);
        assert_eq!(t.result(AaSection::OtherDestroyed, 40, 0, 66), 5);
    }
}
