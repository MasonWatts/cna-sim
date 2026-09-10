//! Combat Results Tables (CRTs) — DATA (lookup tables) with code that interprets them
//! (design.md §3.8 / D9). Human-readable companions live under `docs/source-materials/`.

use serde::{Deserialize, Serialize};

/// The Anti-Armor Fire Combat Results Table (§14.6).
///
/// 18 rows keyed by the two-dice read (paired 11,12 / 13,14 / … / 65,66), 17 columns of
/// Actual Anti-Armor Points (`0*`, 1..15, `16+`). Cells are Damage Points — the number
/// of Armor Protection Points the target must lose. `0` encodes the chart's "-".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntiArmorCrt {
    /// `rows[row][col]`. See `row_index` / `col_index`.
    pub rows: Vec<Vec<i32>>,
}

impl AntiArmorCrt {
    /// Row index for a two-dice read `dice` = die1*10 + die2 (each die 1..6, so 11..66),
    /// read straight (not reordered) — the table covers all 36 outcomes, paired.
    pub fn row_index(dice: u8) -> usize {
        let tens = (dice / 10) as usize;
        let ones = (dice % 10) as usize;
        (tens - 1) * 3 + (ones - 1) / 2
    }

    /// §14.6 note: the Phasing Player decreases his dice roll by one row (the top row 11,12 is
    /// unaffected). Given a two-dice read, returns a representative read for the row one above it,
    /// suitable for `row_index` / `damage`. Rows 11,12 return themselves.
    pub fn phasing_row_dice(dice: u8) -> u8 {
        let up = Self::row_index(dice).saturating_sub(1);
        let tens = (up / 3) as u8 + 1;
        let ones = (up % 3) as u8 * 2 + 1;
        tens * 10 + ones
    }

    /// Column index from Actual and Raw Anti-Armor Points. The `0*` column (index 0) is
    /// used only when Actual is 0 but Raw is 1..4 (§14.6 footnote); returns `None` when
    /// no column applies (no effective fire).
    pub fn col_index(actual_points: i32, raw_points: i32) -> Option<usize> {
        if actual_points >= 1 {
            Some(actual_points.min(16) as usize)
        } else if (1..=4).contains(&raw_points) {
            Some(0)
        } else {
            None
        }
    }

    /// Damage Points for a resolved anti-armor attack (0 if no column applies).
    pub fn damage(&self, dice: u8, actual_points: i32, raw_points: i32) -> i32 {
        match Self::col_index(actual_points, raw_points) {
            Some(col) => self.rows[Self::row_index(dice)][col],
            None => 0,
        }
    }
}

/// Result of a barrage against a land unit (§12.6). Severity increases left→right.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BarrageResult {
    NoEffect,
    /// May not Retreat Before Assault, may not participate in Assault.
    Pinned,
    /// Infantry/Armor: lose 1 TOE point + Pinned. Guns/Trucks: lose 1 TOE point.
    One,
    /// As `One` but lose 2 TOE points.
    Two,
}

/// The target's class on the Barrage CRT (§12.6). Determines which block applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BarrageTargetClass {
    Infantry,
    Armor,
    Gun,
    Truck,
}

/// One printed cell: a dice range `[min, max]` (straight two-dice read) → `result`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BarrageEntry(pub u8, pub u8, pub BarrageResult);

/// Barrage Against Land Units CRT (§12.6). An *inverted* table: pick the target-class
/// block and the Barrage-Points column, then find which dice range contains the roll.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarrageCrt {
    /// Per target class, 9 columns (`1,2` / `3,4` / … / `17+`); each column is the list
    /// of dice ranges printed in that cell (covering the 11..=66 reads).
    pub classes: Vec<(BarrageTargetClass, Vec<Vec<BarrageEntry>>)>,
}

impl BarrageCrt {
    /// Column index for Actual Barrage Points: `1,2`→0, `3,4`→1, …, `17+`→8. `None` when
    /// there are no barrage points (no effect / no roll).
    pub fn column(actual_points: i32) -> Option<usize> {
        if actual_points < 1 {
            None
        } else {
            Some((((actual_points - 1) / 2).min(8)) as usize)
        }
    }

    /// Result for a barrage: target `class`, Actual Barrage Points, and dice read (11..=66).
    pub fn result(&self, class: BarrageTargetClass, actual_points: i32, dice: u8) -> BarrageResult {
        self.result_shifted(class, actual_points, dice, 0)
    }

    /// As `result`, but with a terrain column shift (§12.33): `column_shift` is negative to
    /// benefit the defender. A shift below the `1,2` column (§12.34) = No effect.
    pub fn result_shifted(
        &self,
        class: BarrageTargetClass,
        actual_points: i32,
        dice: u8,
        column_shift: i32,
    ) -> BarrageResult {
        let Some(base) = Self::column(actual_points) else {
            return BarrageResult::NoEffect;
        };
        let shifted = base as i32 + column_shift;
        if shifted < 0 {
            return BarrageResult::NoEffect;
        }
        let col = (shifted as usize).min(8);
        for (c, cols) in &self.classes {
            if *c == class {
                if let Some(entries) = cols.get(col) {
                    for e in entries {
                        if dice >= e.0 && dice <= e.1 {
                            return e.2;
                        }
                    }
                }
            }
        }
        BarrageResult::NoEffect
    }
}

/// An outcome on the Morale Modifier Table (§17.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoraleModifier {
    /// Add this number to the unit's Basic Morale for that combat (`NO` = `Delta(0)`).
    Delta(i32),
    /// All of the unit's Strength Points Surrender (exception: §17.26).
    Surrender,
}

/// One printed cell: a dice range `[min, max]` (straight two-dice read) → `outcome`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MoraleEntry(pub u8, pub u8, pub MoraleModifier);

/// Morale Modifier Table (§17.4). An *inverted* table keyed by Disorganization Level:
/// find the level's row, then which dice range contains the roll → the morale modifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoraleModifierTable {
    /// 26 rows, high→low Disorganization Level: index 0 = `+8+`, …, index 25 = `−17−`.
    pub rows: Vec<Vec<MoraleEntry>>,
}

impl MoraleModifierTable {
    /// Row index for a Disorganization Level, clamped to the `+8+ … −17−` range.
    pub fn level_index(level: i32) -> usize {
        (8 - level.clamp(-17, 8)) as usize
    }

    /// The morale modifier for a Disorganization Level and dice read (11..=66).
    pub fn modifier(&self, disorganization_level: i32, dice: u8) -> MoraleModifier {
        let row = &self.rows[Self::level_index(disorganization_level)];
        for e in row {
            if dice >= e.0 && dice <= e.1 {
                return e.2;
            }
        }
        MoraleModifier::Delta(0)
    }
}

/// Which side's block of the Close Assault CRT (§15.79) to consult.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloseAssaultSide {
    Attacker,
    Defender,
}

/// A secondary (dice-sum) result on the Close Assault CRT. `Capt` → Prisoners Captured
/// table (§15.7); `Engaged` (attacker block); `Retreat(n)` = retreat at least n hexes
/// (defender block).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloseAssaultSecondary {
    Capt,
    Engaged,
    Retreat(u8),
}

/// One loss cell: `(column, read_min, read_max)` — the two-dice read (11..66) range in
/// that differential column that yields the row's loss percentage.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LossCell(pub usize, pub u8, pub u8);

/// A loss row: a casualty `percent` and the columns (with read ranges) that produce it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LossRow {
    pub percent: u8,
    pub cells: Vec<LossCell>,
}

/// One secondary cell: `(column, dice-sum ranges)` — possibly non-contiguous sum sets
/// (e.g. `9-10,12` → `[(9,10),(12,12)]`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecondaryCell(pub usize, pub Vec<(u8, u8)>);

/// A secondary row: a `result` and the columns (with dice-sum ranges) that produce it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecondaryRow {
    pub result: CloseAssaultSecondary,
    pub cells: Vec<SecondaryCell>,
}

/// One side's block of the Close Assault CRT: loss rows (keyed by the two-dice read) and
/// secondary rows (keyed by the dice sum).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloseAssaultBlock {
    pub loss: Vec<LossRow>,
    pub secondary: Vec<SecondaryRow>,
}

/// A fully-resolved Close Assault lookup for one side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloseAssaultResult {
    /// Casualty percentage (0..=50) from the two-dice read.
    pub loss_percent: u8,
    /// `Capt` — proceed to the Prisoners Captured table (§15.7).
    pub captured: bool,
    /// `Eng` — the defender is Engaged (attacker block only).
    pub engaged: bool,
    /// Retreat requirement in hexes (defender block only; 0 = none).
    pub retreat_hexes: u8,
    /// The dice sum matched no secondary result → In Contact.
    pub in_contact: bool,
}

/// The Close Assault Combat Results Table (§15.79). Keyed by the Final Close Assault
/// Differential (attacker CA − defender CA) → column; each side's dice roll is read both
/// as a two-dice value (11..66 → loss %) and as a sum (2..12 → Capt/Eng/Retreat).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloseAssaultCrt {
    pub attacker: CloseAssaultBlock,
    pub defender: CloseAssaultBlock,
}

impl CloseAssaultCrt {
    /// Column index (0..=17) for the Final Close Assault Differential.
    pub fn column_for_differential(differential: i32) -> usize {
        match differential {
            d if d <= -11 => 0,
            -10..=-8 => 1,
            -7..=-6 => 2,
            -5..=-4 => 3,
            -3 => 4,
            -2 => 5,
            -1 => 6,
            0 => 7,
            1 => 8,
            2 => 9,
            3 => 10,
            4 => 11,
            5..=6 => 12,
            7..=8 => 13,
            9..=10 => 14,
            11..=13 => 15,
            14..=16 => 16,
            _ => 17, // >= 17
        }
    }

    /// §15.77: the high differential columns (15..=17, i.e. differential +11 and up) form
    /// the Overrun section of the CRT.
    pub fn is_overrun_column(col: usize) -> bool {
        col >= 15
    }

    fn block(&self, side: CloseAssaultSide) -> &CloseAssaultBlock {
        match side {
            CloseAssaultSide::Attacker => &self.attacker,
            CloseAssaultSide::Defender => &self.defender,
        }
    }

    /// Resolve a close assault for one `side`: `read` is the two-dice value (11..66) and
    /// `sum` is the dice sum (2..12) of that same roll.
    pub fn resolve(
        &self,
        side: CloseAssaultSide,
        differential: i32,
        read: u8,
        sum: u8,
    ) -> CloseAssaultResult {
        self.resolve_at_column(side, Self::column_for_differential(differential), read, sum)
    }

    /// Resolve a close assault at an explicit (already column-shifted) column index
    /// (§15.27: terrain/size/morale adjust the *column*, not the differential number).
    pub fn resolve_at_column(
        &self,
        side: CloseAssaultSide,
        col: usize,
        read: u8,
        sum: u8,
    ) -> CloseAssaultResult {
        let block = self.block(side);

        let mut loss_percent = 0u8;
        for row in &block.loss {
            if row
                .cells
                .iter()
                .any(|c| c.0 == col && read >= c.1 && read <= c.2)
            {
                loss_percent = row.percent;
                break;
            }
        }

        let mut captured = false;
        let mut engaged = false;
        let mut retreat_hexes = 0u8;
        for row in &block.secondary {
            let hit = row.cells.iter().any(|c| {
                c.0 == col && c.1.iter().any(|(lo, hi)| sum >= *lo && sum <= *hi)
            });
            if hit {
                match row.result {
                    CloseAssaultSecondary::Capt => captured = true,
                    CloseAssaultSecondary::Engaged => engaged = true,
                    CloseAssaultSecondary::Retreat(n) => retreat_hexes = retreat_hexes.max(n),
                }
            }
        }
        let in_contact = !captured && !engaged && retreat_hexes == 0;

        CloseAssaultResult {
            loss_percent,
            captured,
            engaged,
            retreat_hexes,
            in_contact,
        }
    }
}

/// Prisoners Captured Results Table (§15.89). A single d6 → the percentage of Infantry,
/// Armor, and Gun casualties that the opposing Player takes as Prisoners.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrisonersTable {
    /// `percent[die - 1]` for die 1..=6.
    pub percent: Vec<u8>,
}

impl PrisonersTable {
    /// Percentage of casualties taken as prisoners for a d6 roll (1..=6).
    pub fn percent(&self, die: u8) -> u8 {
        self.percent[(die.clamp(1, 6) - 1) as usize]
    }
}
