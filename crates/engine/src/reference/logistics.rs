//! Full Logistics Game reference tables (§49–52). See `data/reference/*.ron` and the
//! `docs/source-materials/*-consumption.md` companions.

use serde::{Deserialize, Serialize};

/// Ammunition Consumption Rates (§50.2, full-game column): Ammo Points expended per TOE
/// Strength Point performing each combat action (§50.14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AmmoConsumptionRates {
    pub barrage: u32,
    pub anti_armor: u32,
    /// Applies to Armor-class, Gun-class, MG-inf and HvyWpn-inf (not plain rifle infantry).
    pub close_assault: u32,
    pub anti_air: u32,
}

/// A depletable well source (§52.7). Major Cities and Oases are unlimited and are not listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WaterSource {
    /// A village well ("Town" row).
    Town,
    /// A desert well ("Bir" row).
    Bir,
}

/// Water Availability Table (§52.7): Water Points drawn from a Town/Bir well by a d6, and the
/// die results that trigger a secret depletion check (roll again; a '1' depletes the well).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct WaterAvailabilityTable {
    pub town: Vec<u32>,
    pub bir: Vec<u32>,
    #[serde(default)]
    pub town_deplete_check: Vec<u32>,
    #[serde(default)]
    pub bir_deplete_check: Vec<u32>,
}

impl WaterAvailabilityTable {
    /// Water Points drawn (§52.7) and whether a depletion check applies, for a d6 roll (1–6).
    pub fn draw(&self, source: WaterSource, die: u32) -> (u32, bool) {
        let idx = (die.clamp(1, 6) - 1) as usize;
        match source {
            WaterSource::Town => (self.town.get(idx).copied().unwrap_or(0), self.town_deplete_check.contains(&die)),
            WaterSource::Bir => (self.bir.get(idx).copied().unwrap_or(0), self.bir_deplete_check.contains(&die)),
        }
    }
}

/// The three Truck Point types (§53/§54.2). One Truck Point ≈ ten actual trucks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TruckKind {
    Light,
    Medium,
    Heavy,
}

/// One row of the §54.2 Truck Characteristics Chart. CPA fields are the Truck Point's CPA when
/// transporting that item (`None` = "na"). TOE capacities are Max TOE Strength Points carried
/// per Truck Point, in **tenths** (Light Infantry ½ = 5). Cargo fields are Supply Points per
/// Truck Point of each commodity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TruckCharacteristics {
    #[serde(default)]
    pub cpa_inf: Option<i32>,
    #[serde(default)]
    pub cpa_guns: Option<i32>,
    pub cpa_supplies: i32,
    #[serde(default)]
    pub toe_inf_tenths: Option<u32>,
    #[serde(default)]
    pub toe_arty_tenths: Option<u32>,
    #[serde(default)]
    pub toe_aa_tenths: Option<u32>,
    pub cargo_ammo: u32,
    pub cargo_fuel: u32,
    pub cargo_stores: u32,
    pub cargo_water: u32,
    pub fuel_capacity: u32,
    #[serde(default)]
    pub fuel_consumption_factor: u32,
    /// Breakdown Adjustment Rating: column shifts (negative = left, per the chart's "2L").
    #[serde(default)]
    pub bar: i32,
}

/// The §54.2 Truck Characteristics Chart (three rows).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TruckChart {
    pub light: TruckCharacteristics,
    pub medium: TruckCharacteristics,
    pub heavy: TruckCharacteristics,
}

impl TruckChart {
    pub fn get(&self, kind: TruckKind) -> &TruckCharacteristics {
        match kind {
            TruckKind::Light => &self.light,
            TruckKind::Medium => &self.medium,
            TruckKind::Heavy => &self.heavy,
        }
    }

    /// Truck Points of `kind` needed to motorize `toe` Infantry Strength Points (§54.2 TOE-Inf
    /// capacity; Light needs two points per SP). `None` if that type cannot carry infantry.
    pub fn inf_points_needed(&self, kind: TruckKind, toe: u32) -> Option<u32> {
        let cap = self.get(kind).toe_inf_tenths?;
        if cap == 0 {
            return None;
        }
        Some((toe * 10).div_ceil(cap)) // ⌈toe / (cap/10)⌉
    }
}

/// One roll band in a Breakdown Table column (§21.38): the highest sequential-read roll (§12.42,
/// larger×10+smaller) that yields `percent` breakdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BreakdownBand {
    pub max_roll: u32,
    pub percent: u32,
}

/// One Accumulated-Breakdown-Points column of the §21.38 table: applies when accumulated BP is
/// ≤ `bp_max` (and above the previous column), with roll bands ascending by `max_roll`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BreakdownColumn {
    pub bp_max: u32,
    pub bands: Vec<BreakdownBand>,
}

/// The §21.38 Breakdown Table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BreakdownTable {
    pub columns: Vec<BreakdownColumn>,
}

impl BreakdownTable {
    /// The column index whose Accumulated-BP range contains `bp` (before any shift).
    pub fn column_index(&self, bp: u32) -> usize {
        self.columns
            .iter()
            .position(|c| bp <= c.bp_max)
            .unwrap_or(self.columns.len().saturating_sub(1))
    }

    /// Percentage of a vehicle type that breaks down (§21.38), given accumulated Breakdown
    /// Points, a column `shift` (BAR — negative = Left/better — plus +1 per Hot weather and per
    /// Sandstorm, cumulative), and a sequential-read d6×d6 `roll` (larger×10+smaller).
    pub fn percent(&self, bp: u32, shift: i32, roll: u32) -> u32 {
        if self.columns.is_empty() {
            return 0;
        }
        let base = self.column_index(bp);
        let idx = (base as i32 + shift).clamp(0, self.columns.len() as i32 - 1) as usize;
        self.columns[idx]
            .bands
            .iter()
            .find(|b| roll <= b.max_roll)
            .map(|b| b.percent)
            .unwrap_or(0)
    }
}

/// The §22.8 Broken-Down Vehicle Repair Table, indexed by modified die (0–8). Field columns give
/// Truck Points / AC-Recce TOE repaired and the Tank/SPA/TD repair percentage; facility columns
/// give the repair percentage at a Temporary / Major facility. (`na` entries stored as 0.)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RepairTable {
    pub truck_field: Vec<u32>,
    pub acr_field: Vec<u32>,
    pub tank_field_pct: Vec<u32>,
    pub temp_facility_pct: Vec<u32>,
    pub major_facility_pct: Vec<u32>,
}

impl RepairTable {
    fn at(v: &[u32], die: u32) -> u32 {
        v.get((die.min(8)) as usize).copied().unwrap_or(0)
    }
    /// Tank/SPA/TD field-repair percentage for a modified die roll (§22.25).
    pub fn tank_field_pct(&self, die: u32) -> u32 {
        Self::at(&self.tank_field_pct, die)
    }
    /// Temporary-facility repair percentage (§22.3).
    pub fn temp_facility_pct(&self, die: u32) -> u32 {
        Self::at(&self.temp_facility_pct, die)
    }
    /// Major-facility repair percentage (§22.3).
    pub fn major_facility_pct(&self, die: u32) -> u32 {
        Self::at(&self.major_facility_pct, die)
    }
}

/// One tier of a phased production schedule (§20.66a): from Game-Turn `from_gt`, up to
/// `max_per_turn` points may be planned per Game-Turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductionTier {
    pub from_gt: u32,
    pub max_per_turn: u32,
}

/// One row of the §20.66a Axis Truck Production Chart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TruckProductionRow {
    /// Lifetime total Truck Points that may be brought in.
    pub total: u32,
    /// Axis Naval Convoy tons required to ship one Truck Point.
    pub tonnage: u32,
    pub schedule: Vec<ProductionTier>,
}

impl TruckProductionRow {
    /// Max Truck Points planable this Game-Turn (§20.66a) — the highest tier reached.
    pub fn max_per_turn(&self, gt: u32) -> u32 {
        self.schedule
            .iter()
            .filter(|t| t.from_gt <= gt)
            .map(|t| t.max_per_turn)
            .max()
            .unwrap_or(0)
    }
}

/// The §20.66a Axis Truck Production Chart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AxisTruckProduction {
    pub light: TruckProductionRow,
    pub medium: TruckProductionRow,
    pub heavy: TruckProductionRow,
}

impl AxisTruckProduction {
    pub fn row(&self, kind: TruckKind) -> &TruckProductionRow {
        match kind {
            TruckKind::Light => &self.light,
            TruckKind::Medium => &self.medium,
            TruckKind::Heavy => &self.heavy,
        }
    }
}

/// The period over which a Production item's `max` applies (§20.66 notes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ProductionPeriod {
    #[default]
    GameTurn,
    /// The `*` note: max points every two weeks (two Game-Turns).
    TwoWeeks,
    /// The `†` note: max points per month.
    Month,
}

impl ProductionPeriod {
    /// The length of one production period in Game-Turns (a Game-Turn is one week): a plain
    /// Game-Turn = 1, `*`/`†` Two-Weeks = 2, `†`/`*` Month ≈ 4 (§20.66 notes).
    pub fn span_game_turns(self) -> u32 {
        match self {
            ProductionPeriod::GameTurn => 1,
            ProductionPeriod::TwoWeeks => 2,
            ProductionPeriod::Month => 4,
        }
    }
}

/// One Replacement-Point line of a Production Chart (§20.66): total available, per-period max,
/// first plannable Game-Turn (arrives two Game-Turns later), and convoy tons per point. Items
/// with a phased schedule (per-date-range maxima) use `tiers` instead of the flat `max`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductionItem {
    pub item: String,
    pub total: u32,
    /// Axis Naval Convoy tons per point (0 for Commonwealth production, which lands at Alexandria).
    #[serde(default)]
    pub tonnage: u32,
    #[serde(default)]
    pub max: u32,
    #[serde(default)]
    pub period: ProductionPeriod,
    #[serde(default)]
    pub first_gt: u32,
    /// Last Game-Turn the item may be planned (§20.78C "first/last" dates); `None` = open-ended.
    #[serde(default)]
    pub last_gt: Option<u32>,
    /// Phased maxima by Game-Turn window (§20.66 "Dates" ranges); empty = use `max`/`first_gt`.
    #[serde(default)]
    pub tiers: Vec<ProductionTierWindow>,
}

impl ProductionItem {
    /// The period the per-turn maximum applies over on Game-Turn `gt` — the matching tier's
    /// period when the item is phased, else the item's flat `period` (§20.66 notes).
    pub fn period_at(&self, gt: u32) -> ProductionPeriod {
        if !self.tiers.is_empty() {
            self.tiers
                .iter()
                .find(|t| gt >= t.from_gt && t.to_gt.map(|e| gt <= e).unwrap_or(true))
                .map(|t| t.period)
                .unwrap_or(self.period)
        } else {
            self.period
        }
    }
}

/// A per-date-range maximum for a phased production item (§20.66 Italian "Dates" like "9…24").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductionTierWindow {
    pub max: u32,
    pub from_gt: u32,
    /// Last Game-Turn of the window; `None` = open-ended.
    #[serde(default)]
    pub to_gt: Option<u32>,
    #[serde(default)]
    pub period: ProductionPeriod,
}

/// A nationality's Replacement-Point Production Chart (§20.66) — a list of item lines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProductionChart {
    pub items: Vec<ProductionItem>,
}

impl ProductionChart {
    pub fn item(&self, name: &str) -> Option<&ProductionItem> {
        self.items.iter().find(|i| i.item == name)
    }
    /// Max Replacement Points of `name` plannable on Game-Turn `gt` (0 before its first date).
    /// Uses the phased `tiers` when present, else the flat `max`/`first_gt`. (The per-period
    /// bucket accounting of §20.66 is applied by the planning layer.)
    pub fn available(&self, name: &str, gt: u32) -> u32 {
        let Some(i) = self.item(name) else {
            return 0;
        };
        if !i.tiers.is_empty() {
            i.tiers
                .iter()
                .find(|t| gt >= t.from_gt && t.to_gt.map(|e| gt <= e).unwrap_or(true))
                .map(|t| t.max)
                .unwrap_or(0)
        } else if gt >= i.first_gt && i.last_gt.map(|l| gt <= l).unwrap_or(true) {
            i.max
        } else {
            0
        }
    }
}

/// One era row of the §20.78A Commonwealth Truck Production Table: Truck Points produced by a
/// d6 (index 0–5) for each type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CwTruckEra {
    pub light: Vec<u32>,
    pub medium: Vec<u32>,
    pub heavy: Vec<u32>,
}

impl CwTruckEra {
    pub fn produce(&self, kind: TruckKind, die: u32) -> u32 {
        let row = match kind {
            TruckKind::Light => &self.light,
            TruckKind::Medium => &self.medium,
            TruckKind::Heavy => &self.heavy,
        };
        die.checked_sub(1)
            .and_then(|i| row.get(i as usize))
            .copied()
            .unwrap_or(0)
    }
}

/// The §20.78A Commonwealth Truck Production Table (two eras; three d6 rolls per production, one
/// per type; trucks arrive four Game-Turns later at Alexandria).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CommonwealthTruckProduction {
    pub late_from_gt: u32,
    pub early: CwTruckEra,
    pub late: CwTruckEra,
}

impl CommonwealthTruckProduction {
    pub fn era(&self, gt: u32) -> &CwTruckEra {
        if gt < self.late_from_gt {
            &self.early
        } else {
            &self.late
        }
    }
    /// Truck Points produced for `kind` on Game-Turn `gt` with d6 `die` (§20.78A).
    pub fn produce(&self, kind: TruckKind, gt: u32, die: u32) -> u32 {
        self.era(gt).produce(kind, die)
    }
}

/// One Game-Turn era of the §20.78B Commonwealth Infantry Production Table: Infantry Replacement
/// Points by a 2d6 roll (`by_roll[0]` = roll 2 … `by_roll[10]` = roll 12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CwInfantryEra {
    pub from_gt: u32,
    pub to_gt: u32,
    pub by_roll: Vec<u32>,
}

/// The §20.78B Commonwealth Infantry Production Table (four eras; a 2d6 roll; arrives four
/// Game-Turns later, all in one city).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CwInfantryProduction {
    pub eras: Vec<CwInfantryEra>,
}

impl CwInfantryProduction {
    /// Infantry Replacement Points produced on Game-Turn `gt` with 2d6 `roll` (2–12); 0 outside
    /// any era or on a "none" result.
    pub fn produce(&self, gt: u32, roll: u32) -> u32 {
        self.eras
            .iter()
            .find(|e| gt >= e.from_gt && gt <= e.to_gt)
            .and_then(|e| roll.checked_sub(2).and_then(|i| e.by_roll.get(i as usize)).copied())
            .unwrap_or(0)
    }
}

/// One Replacement-Point requirement (§20.3): `count` points of a `kind` (free-form, e.g. "Inf",
/// "Gun", "ArmR", "Lt Tank", "Tank", "Artillery", "Anti-tank", "Anti-air", "Lt AT", "ParaArt",
/// "7.5cm Lt Gun").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversionReq {
    pub kind: String,
    pub count: u32,
}

/// The §20.3 conversion cost to rebuild one TOE Strength Point of a unit type: any one of the
/// `options` (each a set of requirements that must all be met); `supplies` = also expend Supplies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversionEntry {
    pub unit_type: String,
    /// Alternative requirement-sets ("or"); an empty list means no Replacement Points are needed.
    pub options: Vec<Vec<ConversionReq>>,
    #[serde(default)]
    pub supplies: bool,
}

/// The §20.3 Replacement Point Conversion Chart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ReplacementConversion {
    pub entries: Vec<ConversionEntry>,
}

impl ReplacementConversion {
    /// The conversion cost to rebuild a TOE Strength Point of `unit_type` (§20.3).
    pub fn cost(&self, unit_type: &str) -> Option<&ConversionEntry> {
        self.entries.iter().find(|e| e.unit_type == unit_type)
    }
}
