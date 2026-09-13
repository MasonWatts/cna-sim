//! Tier-2 unit state: a `Unit` instance on/in the game, distinct from the reference
//! `UnitCharacteristics`. See design.md §3.4.
//!
//! Units are stored flat (keyed by `UnitId` in `GameState`, added later); the
//! organization hierarchy is expressed by the two independent parent links
//! (`assigned_parent`, `attached_parent`, §19.1). Combat strengths are DERIVED from the
//! reference charts on demand (store primitives, compute the rest — §3.2).

use serde::{Deserialize, Serialize};

use crate::reference::terrain::Fixed;
use crate::reference::units::{Nationality, UnitCharacteristics, WeaponsSystem};
use crate::reference::ReferenceData;

/// Where a unit sits: on a map hex, or in an off-map box (§8.8 Tripoli/Tunisia complex).
/// Room to grow for other off-board states (holding boxes, training, withdrawn).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnitLocation {
    OnMap(HexCoord),
    /// An off-map node in the §8.8 complex. `transit` = Operations Stages already spent on
    /// the current leg (0 = in the box); `toward_map` = the leg's direction (true = toward
    /// Nofilia, false = inland). §8.84 In-Transit stepping / reversal.
    OffMap { node: String, transit: u8, toward_map: bool },
}

impl UnitLocation {
    /// The on-map hex, if this location is on the board (`None` if off-map).
    pub fn hex(&self) -> Option<HexCoord> {
        match self {
            UnitLocation::OnMap(h) => Some(*h),
            UnitLocation::OffMap { .. } => None,
        }
    }
    /// The off-map node name, if off the board.
    pub fn off_map_node(&self) -> Option<&str> {
        match self {
            UnitLocation::OffMap { node, .. } => Some(node),
            UnitLocation::OnMap(_) => None,
        }
    }
}

impl From<HexCoord> for UnitLocation {
    fn from(h: HexCoord) -> Self {
        UnitLocation::OnMap(h)
    }
}
use crate::state::map::HexCoord;

/// Stable identifier for a unit instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct UnitId(pub u32);

/// A group of identical weapons-system TOE points within a unit (e.g. 6× "25-pounder Gun").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaponGroup {
    pub system: String,
    pub count: u32,
}

fn neg_one() -> i32 {
    -1
}

/// First-line Truck Points attached to a unit (§53.11 / §54.2), by type. Used to motorize the
/// unit's TOE or to carry supplies.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TruckPoints {
    #[serde(default)]
    pub light: u32,
    #[serde(default)]
    pub medium: u32,
    #[serde(default)]
    pub heavy: u32,
}

/// How a unit's current strength is composed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Composition {
    /// Ratings come from the unit's own `UnitCharacteristics` (infantry / recce codes).
    Organic { current_toe: u32 },
    /// Weapons-system TOE points (tank / artillery / anti-tank / anti-air holders);
    /// ratings come from `WeaponsSystem`. Empty groups = a bare HQ.
    Weapons { groups: Vec<WeaponGroup> },
}

/// Gun position for combat (§12.1). Meaningful only for gun-class units in a Combat Segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GunPosition {
    Forward,
    Back,
}

/// Reserve status (§18.0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReserveStatus {
    None,
    ReserveI,
    ReserveII,
}

/// Transient/marker status flags carried on the unit (design.md: markers that are unit
/// state, not placed map objects).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct UnitStatus {
    pub pinned: bool,
    pub engaged: bool,
    pub in_contact: bool,
    pub in_training: bool,
    pub short_of_stores: bool,
    pub depleted: bool,
    /// Set when the unit Surrenders on the Morale Modifier Table (§17.4).
    pub surrendered: bool,
    /// Axis unit stacked with General Rommel (+1 Adjusted Morale in assault, §17.28).
    pub with_rommel: bool,
}

/// The four consumable supply types (§3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Supplies {
    pub ammo: u32,
    pub fuel: u32,
    pub stores: u32,
    pub water: u32,
}

/// A combat rating category for the derived strength calculator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RatingKind {
    Barrage,
    AntiArmor,
    CloseAssaultOff,
    CloseAssaultDef,
    AntiAir,
}

/// Organizational equivalent, for stacking and unit-size comparisons (§9.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum OrgSize {
    Division,
    SuperBrigade,
    StandardBrigade,
    BattleGroup,
    #[default]
    Battalion,
    Company,
}

impl OrgSize {
    /// Coarse size rank (§8.54 / §9.2): division 3, brigade-equivalent 2, battalion 1,
    /// company 0. Used for pinning and unit differentiation.
    pub fn size_rank(self) -> i32 {
        match self {
            OrgSize::Division => 3,
            OrgSize::SuperBrigade | OrgSize::StandardBrigade | OrgSize::BattleGroup => 2,
            OrgSize::Battalion => 1,
            OrgSize::Company => 0,
        }
    }

    /// Basic Stacking Point Value (§9.4).
    pub fn stacking_points(self) -> u32 {
        match self {
            OrgSize::Division => 5,
            OrgSize::SuperBrigade => 3,
            OrgSize::StandardBrigade | OrgSize::BattleGroup => 2,
            OrgSize::Battalion => 1,
            OrgSize::Company => 0,
        }
    }

    /// The equivalent one level down, used when a unit is a shell (§9.28).
    pub fn shell(self) -> OrgSize {
        match self {
            OrgSize::Division => OrgSize::SuperBrigade,
            OrgSize::SuperBrigade | OrgSize::StandardBrigade | OrgSize::BattleGroup => {
                OrgSize::Battalion
            }
            OrgSize::Battalion | OrgSize::Company => OrgSize::Company,
        }
    }
}

/// §19.5: a named or special-arm formation role that overrides the generic Maximum-Attachment
/// row. Derived from the OB at scenario setup (by formation name / organic arm), never from
/// runtime composition. `Normal` = governed by the generic org-size row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FormationRole {
    #[default]
    Normal,
    /// German Artillery Brigade HQ (§19.5): its 1 attachment must be an Artillery (gun-class) unit.
    ArtilleryBrigadeHq,
    /// The Mersa Matruh Garrison / Selby Force (§19.5), a Game-Turn-dependent Allied special:
    /// GT 1–12 (Matruh) — 6 units, no tank, ≤1 gun class; GT 13+ (Selby) — 5 units, no tank,
    /// no recce, ≤3 infantry.
    MatruhSelby,
}

/// A unit instance: identity + reference link + organization + composition + dynamic state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unit {
    pub id: UnitId,
    pub nationality: Nationality,
    /// Key into the nationality's Unit Characteristics chart.
    pub id_code: String,
    /// Historical designation (e.g. "8th Panzer Regiment").
    pub name: String,
    /// Organizational equivalent (drives stacking points, §9.4).
    #[serde(default)]
    pub org_size: OrgSize,
    /// Whether this counter is an HQ with a *parenthesized* Stacking Point value (§9.12):
    /// it represents its formation (its org SP) while combat units are attached, but counts
    /// as 0 SP — and may not voluntarily attack (§3.33) — when it has none attached.
    #[serde(default)]
    pub is_hq: bool,
    /// §42.41: this unit is airborne / airdrop-eligible (set from its OA sheet).
    #[serde(default)]
    pub airdroppable: bool,

    // Organization: two independent, nullable links (§19.1).
    pub assigned_parent: Option<UnitId>,
    pub attached_parent: Option<UnitId>,
    /// §19.3: the formation's historical assignment capacity — the number of units assigned to it at
    /// scenario start (its OA-sheet organization). A reassignment (§19.21) may not push its assigned
    /// count above this. 0 = not a parent / no limit recorded.
    #[serde(default)]
    pub assigned_capacity: u32,
    /// §19.5: whether this formation is an **armored** formation (an armor division / tank brigade /
    /// tank group) — derived from its OB organization (it has tank units assigned) at scenario setup.
    /// Selects the §19.5 Maximum-Attachment row's type constraints ("no Tank", etc.).
    #[serde(default)]
    pub is_armor: bool,
    /// §19.5: a named/special-arm formation role that overrides the generic Maximum-Attachment
    /// row (German Artillery Brigade HQ; the Matruh Garrison / Selby Force). Derived from the OB
    /// at scenario setup. `Normal` for everything else.
    #[serde(default)]
    pub formation_role: FormationRole,

    pub composition: Composition,

    // Dynamic state (path-dependent primitives).
    pub cohesion: i32,
    pub cp_expended: Fixed,
    /// §11.2/§11.24: Capability Points already charged to this unit **for combat** this Operations
    /// Stage, so repeated combats respect the per-segment cap (5 phasing / 3 non-phasing). Part of
    /// `cp_expended`; reset each stage.
    pub combat_cp: Fixed,
    /// §8.14: set when the unit moved into an enemy ZOC this Operations Stage — it must
    /// then stop (no further movement until the next stage). Reset each stage.
    #[serde(default)]
    pub moved_into_zoc: bool,
    /// Current onboard Fuel Points in the unit's "gas tanks" (§49.14–15). Bounded by the fuel
    /// capacity of its type; units whose type has no fuel rate ignore this.
    #[serde(default)]
    pub fuel: u32,
    /// Current onboard Ammunition Points (§50.17: a combat unit carries enough to fire once).
    #[serde(default)]
    pub ammo: u32,
    /// Consecutive Game-Turns this unit has gone without its required Stores (§51.22 attrition
    /// is applied every two such turns); reset to 0 when fully supplied.
    #[serde(default)]
    pub turns_without_stores: u32,
    /// Current onboard Water Points (§52), drawn from wells; consumed each Operations Stage.
    #[serde(default)]
    pub water: u32,
    /// Consecutive Operations Stages this unit has gone without its required Water (§52.53
    /// attrition after the first); reset to 0 when supplied.
    #[serde(default)]
    pub stages_without_water: u32,
    /// First-line Truck Points attached to this unit (§53.11), for motorization / cargo.
    #[serde(default)]
    pub trucks: TruckPoints,
    /// Supply currently loaded on this unit's Truck Points (§53.24) — hauled between dumps/units.
    #[serde(default)]
    pub truck_cargo: crate::state::game::Commodities,
    /// Broken-down first-line Truck Points (§21) awaiting field repair (§22.23) — not usable
    /// until repaired back into `trucks`.
    #[serde(default)]
    pub trucks_broken: TruckPoints,
    /// Broken-down weapon-system TOE (§21, tanks/SP/recce) awaiting repair (§22.8) — held out
    /// of the active composition until repaired.
    #[serde(default)]
    pub weapons_broken: Vec<WeaponGroup>,
    /// Broken-down Organic TOE Strength Points (§21) for a self-rated vehicle unit (Recce /
    /// Armored Car) — held out of `current_toe` until field-repaired (§22.24).
    #[serde(default)]
    pub toe_broken: u32,
    /// Breakdown Points accumulated this Operations Stage (§21.21), in tenths (matching the
    /// terrain `breakdown` Fixed values). Reset each stage.
    #[serde(default)]
    pub breakdown_points: i32,
    /// Highest Breakdown-Table column index already checked this stage (§21.26 — recheck only on
    /// entering a higher column). `-1` = not yet checked. Reset each stage.
    #[serde(default = "neg_one")]
    pub breakdown_checked_col: i32,
    /// §15.82: hexes of Retreat still owed from a Close Assault result (0 = none).
    #[serde(default)]
    pub pending_retreat: u8,
    /// The hex the pending Retreat is measured away from (the assaulted hex).
    #[serde(default)]
    pub retreat_origin: Option<HexCoord>,
    /// Current (trainable) Basic Morale; CW units train up toward the reference value.
    pub basic_morale: i32,
    /// The unit's target/trained Basic Morale (§4.44). Green units start below this
    /// (`basic_morale` = arrival value) and train up to it; equal for non-green units.
    #[serde(default)]
    pub basic_morale_reference: i32,
    /// §17.34: consecutive Operations Stages this unit has spent Training toward the next +1 Basic
    /// Morale step (0–5). Every sixth raises `basic_morale` and resets; any CP expenditure (§17.33)
    /// resets it to 0.
    #[serde(default)]
    pub training_progress: u32,
    pub status: UnitStatus,
    pub gun_position: GunPosition,
    pub reserve: ReserveStatus,
    /// Where the unit is: `Some(OnMap)` when an independent counter on the board,
    /// `Some(OffMap)` when in an off-map box (§8.8) or other off-board state, `None` when
    /// attached (represented by its parent's counter).
    pub location: Option<UnitLocation>,
    pub supplies: Supplies,
}

impl Unit {
    /// The unit's on-map hex, if it is an independent on-board counter (`None` if attached
    /// or off-map). The common read for movement/combat/ZOC.
    pub fn hex(&self) -> Option<HexCoord> {
        self.location.as_ref().and_then(|l| l.hex())
    }
    /// Create a unit with default dynamic state (cohesion 0, unattached, back, no reserve).
    pub fn new(
        id: UnitId,
        nationality: Nationality,
        id_code: impl Into<String>,
        name: impl Into<String>,
        composition: Composition,
    ) -> Self {
        Self {
            id,
            nationality,
            id_code: id_code.into(),
            name: name.into(),
            org_size: OrgSize::Battalion,
            is_hq: false,
            airdroppable: false,
            assigned_parent: None,
            attached_parent: None,
            assigned_capacity: 0,
            is_armor: false,
            formation_role: FormationRole::Normal,
            composition,
            cohesion: 0,
            cp_expended: Fixed(0),
            combat_cp: Fixed(0),
            moved_into_zoc: false,
            fuel: 0,
            ammo: 0,
            turns_without_stores: 0,
            water: 0,
            stages_without_water: 0,
            trucks: TruckPoints::default(),
            truck_cargo: crate::state::game::Commodities::default(),
            trucks_broken: TruckPoints::default(),
            weapons_broken: Vec::new(),
            toe_broken: 0,
            breakdown_points: 0,
            breakdown_checked_col: -1,
            pending_retreat: 0,
            retreat_origin: None,
            basic_morale: 0,
            basic_morale_reference: 0,
            training_progress: 0,
            status: UnitStatus::default(),
            gun_position: GunPosition::Back,
            reserve: ReserveStatus::None,
            location: None,
            supplies: Supplies::default(),
        }
    }

    /// Total current TOE Strength Points across the composition.
    pub fn total_toe(&self) -> u32 {
        match &self.composition {
            Composition::Organic { current_toe } => *current_toe,
            Composition::Weapons { groups } => groups.iter().map(|g| g.count).sum(),
        }
    }

    /// Basic Stacking Point Value from this unit's organizational size (§9.4).
    /// (Shell reduction, §9.28, is applied by the caller once shell status is known.)
    pub fn stacking_points(&self) -> u32 {
        self.org_size.stacking_points()
    }

    /// Raw points for a rating kind: Σ (rating × count) across the composition (§11.34).
    pub fn raw_strength(&self, refs: &ReferenceData, kind: RatingKind) -> i32 {
        match &self.composition {
            Composition::Organic { current_toe } => {
                let rating = refs
                    .unit_characteristics
                    .get(&self.nationality)
                    .and_then(|m| m.get(&self.id_code))
                    .and_then(|uc| organic_rating(uc, kind))
                    .unwrap_or(0);
                rating * (*current_toe as i32)
            }
            Composition::Weapons { groups } => {
                let table = refs.weapons_systems.get(&self.nationality);
                groups
                    .iter()
                    .map(|g| {
                        let rating = table
                            .and_then(|m| m.get(&g.system))
                            .and_then(|ws| weapon_rating(ws, kind))
                            .unwrap_or(0);
                        rating * (g.count as i32)
                    })
                    .sum()
            }
        }
    }

    /// Actual Strength Points (§11.3): Raw ÷ 10, rounded to nearest (§11.32) — except
    /// Anti-Air, which uses raw points directly (no ÷10, §11.37).
    ///
    /// Note: this uses each rating's face value; the parenthesized-rating conditional
    /// (§3.4) and the "both sides < 10 raw" close-assault special case (§11.33) are
    /// combat-resolution concerns handled later.
    pub fn actual_strength(&self, refs: &ReferenceData, kind: RatingKind) -> i32 {
        let raw = self.raw_strength(refs, kind);
        if matches!(kind, RatingKind::AntiAir) {
            raw
        } else {
            // Round half up; raw ≤ 4 → 0 (< 0.5 actual, §11.33).
            (raw + 5) / 10
        }
    }
}

fn organic_rating(uc: &UnitCharacteristics, kind: RatingKind) -> Option<i32> {
    match kind {
        RatingKind::Barrage => uc.barrage.map(|r| r.value),
        RatingKind::AntiArmor => uc.anti_armor.map(|r| r.value),
        RatingKind::CloseAssaultOff => uc.close_assault.map(|c| c.off.value),
        RatingKind::CloseAssaultDef => uc.close_assault.map(|c| c.def.value),
        RatingKind::AntiAir => uc.anti_air.map(|r| r.value),
    }
}

fn weapon_rating(ws: &WeaponsSystem, kind: RatingKind) -> Option<i32> {
    match kind {
        RatingKind::Barrage => ws.barrage.map(|r| r.value),
        RatingKind::AntiArmor => ws.anti_armor.map(|r| r.value),
        RatingKind::CloseAssaultOff => ws.close_assault.map(|c| c.off.value),
        RatingKind::CloseAssaultDef => ws.close_assault.map(|c| c.def.value),
        RatingKind::AntiAir => ws.anti_air.map(|r| r.value),
    }
}
