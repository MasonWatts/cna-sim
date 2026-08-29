//! Unit-characteristics and weapons-system reference types.
//!
//! Two interlocking charts (design.md §7):
//! - **Unit Characteristics** (§4.46, per nationality): keyed by ID code. Infantry/recce
//!   codes carry their combat ratings directly; HQ/tank/artillery/anti-tank/AA codes
//!   carry CPA + Max-TOE + assignment limits, and derive their combat power from the
//!   weapons systems they hold.
//! - **Tank & Gun Characteristics** (§4.47–4.49, per nationality): keyed by weapon name.
//!
//! See the human-readable transcriptions under `docs/source-materials/`.

use serde::{Deserialize, Serialize};

/// Which army a unit/weapon belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Nationality {
    Commonwealth,
    German,
    Italian,
}

/// The two opposing sides. Axis = German + Italian.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    Axis,
    Commonwealth,
}

impl Nationality {
    pub fn side(self) -> Side {
        match self {
            Nationality::Commonwealth => Side::Commonwealth,
            Nationality::German | Nationality::Italian => Side::Axis,
        }
    }
}

impl Side {
    pub fn opponent(self) -> Side {
        match self {
            Side::Axis => Side::Commonwealth,
            Side::Commonwealth => Side::Axis,
        }
    }
}

/// A combat rating that may be "parenthesized" — a conditional rating usable only if no
/// unit in the hex has Ammo plus a non-parenthesized rating (Case 3.4; chart "( )").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rating {
    pub value: i32,
    #[serde(default)]
    pub parenthesized: bool,
}

/// Capability Point Allowance for a unit type, with chart modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cpa {
    pub value: i32,
    /// Chart `*` — the unit retains this CPA regardless of the CPA of assigned TOE points.
    #[serde(default)]
    pub retains_regardless: bool,
    /// Chart `+` — historically supplied with trucks to fully motorize (CW bn/coy-eq only);
    /// only CPA changes while motorized.
    #[serde(default)]
    pub motorized_capable: bool,
}

/// Offensive / Defensive Close Assault ratings (chart "Off/Def").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloseAssault {
    pub off: Rating,
    pub def: Rating,
}

/// Maximum TOE strength a unit may contain, with chart modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaxToe {
    pub value: u32,
    #[serde(default)]
    pub parenthesized: bool,
    /// Chart `**` — if the unit is all US-tank TOE points, the maximum is reduced
    /// (to 9 for the Commonwealth).
    #[serde(default)]
    pub us_tank_special: bool,
}

/// The kinds of weapon TOE points a parent/HQ unit may be assigned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeaponKind {
    Tank,
    Artillery,
    AntiTank,
    AntiAir,
    LightAntiAir,
    HeavyAntiAir,
    Reconnaissance,
}

/// A limit on how many TOE points of a given weapon kind a unit may be assigned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssignmentLimit {
    pub weapon_kind: WeaponKind,
    pub max_count: u32,
}

/// A row of a Unit Characteristics Chart (§4.46), keyed elsewhere by ID code.
///
/// Direct combat ratings are `Some` for infantry/recce codes and `None` where the unit's
/// power derives from its weapons systems (`-` on the chart also maps to `None`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitCharacteristics {
    /// Group label as printed (e.g. "Infantry Bn-Equivalents", "Reconnaissance Bn-Eq").
    pub unit_type: String,
    pub cpa: Cpa,
    #[serde(default)]
    pub anti_air: Option<Rating>,
    #[serde(default)]
    pub barrage: Option<Rating>,
    #[serde(default)]
    pub anti_armor: Option<Rating>,
    #[serde(default)]
    pub vulnerability: Option<Rating>,
    #[serde(default)]
    pub armor_protection: Option<Rating>,
    #[serde(default)]
    pub close_assault: Option<CloseAssault>,
    #[serde(default)]
    pub max_toe: Option<MaxToe>,
    /// Assignment limits for weapon-holding units (HQ/tank/artillery/anti-tank/AA).
    #[serde(default)]
    pub assignments: Vec<AssignmentLimit>,
    #[serde(default)]
    pub notes: Vec<String>,
}

/// A weapons-system row from a Tank & Gun Characteristics Chart (§4.47–4.49), keyed
/// elsewhere by weapon name. Its ratings are what a unit holding this TOE point uses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponsSystem {
    /// The weapon's category (§4.47–4.49), used for combined arms (§15.4) and barrage
    /// target class (§12.6).
    pub kind: WeaponKind,
    /// `None` where the chart shows "-" (the weapon uses its parent unit's CPA).
    #[serde(default)]
    pub cpa: Option<i32>,
    #[serde(default)]
    pub anti_air: Option<Rating>,
    #[serde(default)]
    pub barrage: Option<Rating>,
    #[serde(default)]
    pub anti_armor: Option<Rating>,
    #[serde(default)]
    pub vulnerability: Option<Rating>,
    #[serde(default)]
    pub armor_protection: Option<Rating>,
    #[serde(default)]
    pub close_assault: Option<CloseAssault>,
    #[serde(default)]
    pub fuel_rate: Option<i32>,
    /// Breakdown Adjustment Rating (chart "BAR"): number of column shifts on the
    /// breakdown table — positive = Right, negative = Left, `Some(0)` = none, `None` =
    /// not applicable (towed guns are not subject to breakdown).
    #[serde(default)]
    pub breakdown_adjustment: Option<i32>,
    /// Chart footnotes (e.g. "US tank", "self-propelled", "mineclearing").
    #[serde(default)]
    pub notes: Vec<String>,
}
