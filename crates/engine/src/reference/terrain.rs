//! Terrain reference types.
//!
//! Identity of each terrain type is a closed enum (code) — so exhaustive `match` flags
//! any unhandled terrain as we grow fidelity — while the *parameters* for each type are
//! DATA loaded from `data/reference/terrain.ron`, keyed by the enum (design.md §3.8).
//!
//! Structure mirrors the Terrain Effects Chart (§8.37); see the human-readable
//! transcription at `docs/source-materials/terrain-effects-chart.md`.

use serde::de::{self, Visitor};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Fixed-point number in **tenths** of a point, for exact/deterministic Capability
/// Point and Breakdown Point values (the rules use halves, e.g. ½ CP on roads).
/// Data files write ordinary decimals (`2`, `0.5`); we store tenths internally so all
/// arithmetic stays integer and reproducible (design.md §3.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fixed(pub i32); // tenths

impl Fixed {
    pub const fn from_tenths(t: i32) -> Self {
        Fixed(t)
    }
    pub fn tenths(self) -> i32 {
        self.0
    }
}

impl Serialize for Fixed {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_f64(self.0 as f64 / 10.0)
    }
}

struct FixedVisitor;
impl<'de> Visitor<'de> for FixedVisitor {
    type Value = Fixed;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a number of capability or breakdown points")
    }
    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Fixed, E> {
        Ok(Fixed((v * 10.0).round() as i32))
    }
    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Fixed, E> {
        Ok(Fixed((v as i32) * 10))
    }
    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Fixed, E> {
        Ok(Fixed((v as i32) * 10))
    }
}
impl<'de> Deserialize<'de> for Fixed {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Fixed, D::Error> {
        d.deserialize_any(FixedVisitor)
    }
}

/// Movement cost for one movement mode (a single cell of the CP-cost columns).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveCost {
    /// A numeric cost in Capability Points (additive for hexsides).
    Cost(Fixed),
    /// Entry/crossing is prohibited to this mode ("P" on the chart).
    Prohibited,
    /// Costs the unit's entire Capability Point Allowance (enemy minefield, "+CPA").
    EntireCpa,
}

/// A pair of movement costs: non-motorized and motorized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MovePair {
    pub non_mot: MoveCost,
    pub mot: MoveCost,
}

/// A combat column shift (Barrage / Anti-Armor / Close Assault columns).
///
/// Convention: `Shift(-n)` = n columns LEFT (favors defender, printed "L#");
/// `Shift(n)` = n columns RIGHT (favors attacker, printed "R#"); `NoShift` = blank;
/// `Prohibited` = that type of fire may not occur across the hexside (e.g. Anti-Armor
/// up an escarpment).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CombatMod {
    NoShift,
    Shift(i32),
    Prohibited,
}

/// The fixed set of hex (area) terrain types.
///
/// Village/Bir/Oasis and Railroads ("same as terrain in hex") and Fortifications /
/// Minefields are modeled separately as map *features*, not base terrain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HexTerrain {
    Clear,
    SandGravel,
    SaltMarsh,
    HeavyVegetation,
    Rough,
    Mountain,
    Delta,
    Desert,
    MajorCity,
    Swamp,
    Sea,
}

/// The fixed set of hexside terrain types.
///
/// Slopes and escarpments are split by direction because their parameters differ
/// up vs. down; the map records the orientation and code selects the matching variant
/// when a unit crosses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HexsideTerrain {
    Wadi,
    Ridge,
    UpSlope,
    DownSlope,
    UpEscarpment,
    DownEscarpment,
    MajorRiver,
    MinorRiver,
    Road,
    Track,
    Railroad,
    /// Unbuilt track (§24.67): fully inert until built.
    UnfinishedRailroad,
    Lake,
}

/// Parameters for a hex (area) terrain type. DATA — Terrain Effects Chart (§8.37).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainParams {
    /// Cost to enter a hex of this terrain, by movement mode.
    pub enter: MovePair,
    /// Breakdown points incurred by a vehicle entering this hex.
    pub breakdown: Fixed,
    pub barrage: CombatMod,
    pub anti_armor: CombatMod,
    pub close_assault: CombatMod,
    /// Max stacking points that may end a Movement Segment here. `None` = unlimited or
    /// not-yet-confirmed (see the transcription's flags).
    pub stacking_limit: Option<u32>,
    /// Chart footnote numbers whose rules are enforced in code.
    pub notes: Vec<u8>,
}

/// Parameters for a hexside terrain type. DATA — Terrain Effects Chart (§8.37).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HexsideParams {
    /// Cost to cross this hexside, by movement mode (additive; Road/Track are a
    /// movement-along overlay handled specially in code).
    pub cross: MovePair,
    /// Breakdown points incurred by a vehicle crossing this hexside.
    pub breakdown: Fixed,
    pub anti_armor: CombatMod,
    pub close_assault: CombatMod,
    /// Whether this hexside blocks Zones of Control (§10.21).
    pub blocks_zoc: bool,
    /// Chart footnote numbers whose rules are enforced in code.
    pub notes: Vec<u8>,
}
