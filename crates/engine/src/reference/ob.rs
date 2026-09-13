//! Order of Battle reference data (§4.44a–c "Organization at Arrival" charts).
//!
//! One `OrderOfBattle` per nationality: the full campaign roster of formations, each a flat
//! list of units carrying their id_code (→ Unit Characteristics), composition (TOE &
//! weapons), arrival turn, and parent (by counter abbreviation) so the division→regiment→
//! battalion hierarchy can be rebuilt. Scenarios select formations from here and place them
//! (see `state::scenario`); the OB itself is scenario-independent reference data (D12).

use serde::{Deserialize, Serialize};

use crate::state::unit::OrgSize;

/// One nationality's complete order of battle.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OrderOfBattle {
    pub formations: Vec<Formation>,
}

impl OrderOfBattle {
    pub fn formation(&self, name: &str) -> Option<&Formation> {
        self.formations.iter().find(|f| f.name == name)
    }
}

/// A formation sheet (a Division/Brigade/Group + its sub-units).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Formation {
    pub name: String,
    #[serde(default)]
    pub org_size: OrgSize,
    /// "Basic Morale" — the trained/reference rating (§4.44 legend).
    #[serde(default)]
    pub basic_morale: i32,
    /// Some(x) = the (lower) morale the units arrive/deploy with and train up from
    /// (Commonwealth greens); None = arrives already at `basic_morale`.
    #[serde(default)]
    pub arrival_morale: Option<i32>,
    /// A dynamic pool of empty HQ counters formed by attachment (Kampfgruppen, Selby Force,
    /// 22nd Guards Brigade) rather than a fixed composition.
    #[serde(default)]
    pub dynamic: bool,
    /// Deployed in a scenario setup but not individually sheeted in §4.44 (e.g. 62
    /// Marmarica / 63 Cirene) — modeled on a standard template.
    #[serde(default)]
    pub abstracted: bool,
    /// §9.16a: for a garrison formation, the hex label of the city/town it garrisons — every unit
    /// deploys with `garrison_location` set to it (0 Stacking Points there). A per-unit `garrison`
    /// override wins for scattered garrisons. `None` for non-garrison formations.
    #[serde(default)]
    pub garrison_hex: Option<String>,
    #[serde(default)]
    pub units: Vec<ObUnit>,
}

/// One counter within a formation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ObUnit {
    pub name: String,
    /// Counter abbreviation (used as the parent key for nesting).
    #[serde(default)]
    pub abbr: String,
    /// Unit Characteristics ID code (§4.46 key: 'a'..'uu').
    pub id: String,
    pub comp: ObComposition,
    #[serde(default)]
    pub org_size: OrgSize,
    #[serde(default)]
    pub arrival: Arrival,
    /// Counter abbreviation of the parent unit (chart indentation); "" = top of the sheet.
    #[serde(default)]
    pub parent: String,
    /// §19.1: attached (not assigned) — the chart's four-space indent / "attached, not
    /// assigned" notes.
    #[serde(default)]
    pub attached: bool,
    /// §42.41: the OA sheet marks this unit as airdrop-eligible (the Folgore / Ramcke airborne
    /// units). Set when the airborne formations are promoted into the loaded OB.
    #[serde(default)]
    pub airdroppable: bool,
    /// §20.8: `(Game-Turn, Operations Stage)` this unit must be withdrawn by (the [4.43a] schedule).
    /// Stamped onto the spawned unit's `mandatory_withdrawal` at deploy. `None` = not scheduled.
    #[serde(default)]
    pub withdrawal: Option<(u32, u32)>,
    /// §9.16a: hex label of the city/town this garrison unit is assigned to (overrides the
    /// formation's `garrison_hex`, for scattered minor garrisons). `None` = use the formation's.
    #[serde(default)]
    pub garrison: Option<String>,
    #[serde(default)]
    pub notes: String,
}

/// TOE composition from the chart's "TOE & Weapon System(s)" column.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ObComposition {
    /// Non-tank/-gun unit: the chart's `N` (full TOE), `U@x` (understrength), `O@x`
    /// (overstrength). Resolved to actual TOE against the id-code's max at load.
    Organic(Toe),
    /// Explicit weapon systems: list of (weapon name, count).
    Weapons(Vec<(String, u32)>),
}

impl Default for ObComposition {
    fn default() -> Self {
        ObComposition::Organic(Toe::Normal)
    }
}

/// A non-tank/-gun unit's strength code (§4.44 legend).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Toe {
    /// `N` — arrives at the maximum TOE for its ID code.
    Normal,
    /// `U@x` — understrength at `x` TOE.
    Under(u32),
    /// `O@x` — overstrength at `x` TOE.
    Over(u32),
}

/// The chart's "Arrives" column.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum Arrival {
    /// `D` — on the map at the start of Game-Turn 1.
    #[default]
    Deployed,
    /// `stage/turn` — arrives on that Operations Stage of that Game-Turn.
    Turn { game_turn: u16, stage: u8 },
}
