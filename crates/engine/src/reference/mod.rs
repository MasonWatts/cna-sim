//! Tier 1: immutable rulebook DATA (parameters), loaded at startup.
//!
//! Only *parameters* live here (numbers/coefficients/lookup tables). The *rules*
//! that consume them live in `crate::rules`. See `docs/design.md` §3.8 / D9.
//!
//! `ReferenceData` is assembled from multiple RON files (one per chart / nationality)
//! rather than a single blob, so each source chart maps to its own data file.

pub mod crt;
pub mod initiative;
pub mod ob;
pub mod off_map;
pub mod ports;
pub mod terrain;
pub mod units;
pub mod victory;
pub mod weather;
pub mod logistics;
pub mod aircraft;
pub mod air_bombardment;
pub mod anti_aircraft;
pub mod convoy;
pub mod equivalent_weights;
pub mod squadron;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crt::{AntiArmorCrt, BarrageCrt, CloseAssaultCrt, MoraleModifierTable, PrisonersTable};
use initiative::InitiativeRatings;
use ob::OrderOfBattle;
use off_map::OffMapNetwork;
use victory::CampaignVictory;
use weather::WeatherTable;
use terrain::{HexTerrain, HexsideParams, HexsideTerrain, TerrainParams};
use units::{Nationality, UnitCharacteristics, WeaponsSystem};

/// Shape of the terrain reference file (`data/reference/terrain.ron`).
#[derive(Debug, Clone, Deserialize)]
struct TerrainFile {
    terrain: HashMap<HexTerrain, TerrainParams>,
    hexsides: HashMap<HexsideTerrain, HexsideParams>,
}

/// The full set of reference data for a game, assembled from multiple RON files.
/// Grows as we add CRTs, schedules, organization limits, etc.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReferenceData {
    pub terrain: HashMap<HexTerrain, TerrainParams>,
    pub hexsides: HashMap<HexsideTerrain, HexsideParams>,
    /// Unit Characteristics (§4.46), keyed by nationality then ID code.
    pub unit_characteristics: HashMap<Nationality, HashMap<String, UnitCharacteristics>>,
    /// Tank & Gun weapons systems (§4.47–4.49), keyed by nationality then weapon name.
    pub weapons_systems: HashMap<Nationality, HashMap<String, WeaponsSystem>>,
    /// §4.44A Aircraft Characteristics Charts by nationality (Air Game).
    #[serde(default)]
    pub aircraft_charts: HashMap<Nationality, aircraft::AircraftChart>,
    /// §38.37 Aircraft Refit Table (Air Game maintenance).
    #[serde(default)]
    pub aircraft_refit: Option<squadron::AircraftRefitTable>,
    /// §40.4 Scramble Table (Air Game fighter combat).
    #[serde(default)]
    pub scramble_table: Option<squadron::ScrambleTable>,
    /// §40.8 Strafing Table (Air Game fighter combat).
    #[serde(default)]
    pub strafing_table: Option<squadron::StrafingTable>,
    /// §41.5 Air Bombardment & Secondary Barrage Targets Table.
    #[serde(default)]
    pub air_bombardment: Option<air_bombardment::AirBombardmentTable>,
    /// §41.39A Mining Harbor by Plane Table.
    #[serde(default)]
    pub mining_harbor: Option<squadron::MiningHarborTable>,
    /// §45.4/§45.5 Air-to-Air Combat tables (Maneuver Adjustment + TacAir Kill).
    #[serde(default)]
    pub air_to_air: Option<squadron::AirToAirTables>,
    /// §46.3/§46.4 Anti-Aircraft Combat Results Table + Flak Adjustment Chart.
    #[serde(default)]
    pub anti_aircraft: Option<anti_aircraft::AntiAircraftTable>,
    /// §42.27 Air Reconnaissance of Land Units Table.
    #[serde(default)]
    pub air_recon: Option<squadron::AirReconTable>,
    /// §55.3 Port Capacity & Efficiency Level Chart.
    #[serde(default)]
    pub ports_chart: Option<ports::PortsChart>,
    /// §44.42 Axis Malta Availability Table.
    #[serde(default)]
    pub malta_availability: Option<squadron::MaltaAvailabilityTable>,
    /// §44.5 Maltese Air Facility Construction Table.
    #[serde(default)]
    pub malta_construction: Option<squadron::MaltaConstructionTable>,
    /// §44.41 Axis Strategic Airforce Commitment Chart.
    #[serde(default)]
    pub malta_commitment: Option<squadron::MaltaCommitmentChart>,
    /// §56.18 Axis Naval Convoy Air Distance Chart (the six shipping lanes).
    #[serde(default)]
    pub shipping_lanes: Option<ports::ShippingLanesChart>,
    /// §56.4/§56.5 Axis Naval Convoy Level Chart + Capacity Table.
    #[serde(default)]
    pub convoy_levels: Option<convoy::ConvoyLevelChart>,
    /// §42.53 Air Reconnaissance of Axis Naval Convoys Table.
    #[serde(default)]
    pub naval_recon: Option<convoy::NavalReconTable>,
    /// §54.5 Equivalent Weights Chart.
    #[serde(default)]
    pub equivalent_weights: Option<equivalent_weights::EquivalentWeights>,
    /// Anti-Armor Fire Combat Results Table (§14.6).
    #[serde(default)]
    pub anti_armor_crt: Option<AntiArmorCrt>,
    /// Barrage Against Land Units Combat Results Table (§12.6).
    #[serde(default)]
    pub barrage_crt: Option<BarrageCrt>,
    /// Morale Modifier Table (§17.4).
    #[serde(default)]
    pub morale_modifier_table: Option<MoraleModifierTable>,
    /// Close Assault Combat Results Table (§15.79).
    #[serde(default)]
    pub close_assault_crt: Option<CloseAssaultCrt>,
    /// Prisoners Captured Results Table (§15.89).
    #[serde(default)]
    pub prisoners_table: Option<PrisonersTable>,
    /// Initiative Ratings Chart (§7.2).
    #[serde(default)]
    pub initiative_ratings: Option<InitiativeRatings>,
    /// Order of Battle (§4.44a–c), keyed by nationality.
    #[serde(default)]
    pub orders_of_battle: HashMap<Nationality, OrderOfBattle>,
    /// Reinforcement debarkation hex-id by nationality (§20.14–20.15): where a whole-
    /// formation reinforcement enters (Commonwealth → Cairo). Off-map entries (Axis →
    /// Tripoli) await the off-map-node system.
    #[serde(default)]
    pub reinforcement_entry: HashMap<Nationality, String>,
    /// Off-map Tripoli/Tunisia land-movement complex (§8.8).
    #[serde(default)]
    pub off_map: Option<OffMapNetwork>,
    /// Weather Table (§29.61) + Foul Weather Location Table (§29.7).
    #[serde(default)]
    pub weather_table: Option<WeatherTable>,
    /// Campaign Victory Conditions (§64.7).
    #[serde(default)]
    pub campaign_victory: Option<CampaignVictory>,
    /// Ammunition Consumption Rates (§50.2).
    #[serde(default)]
    pub ammo_rates: Option<logistics::AmmoConsumptionRates>,
    /// Water Availability Table (§52.7).
    #[serde(default)]
    pub water_table: Option<logistics::WaterAvailabilityTable>,
    /// Truck Characteristics Chart (§54.2).
    #[serde(default)]
    pub truck_chart: Option<logistics::TruckChart>,
    /// Breakdown Table (§21.38).
    #[serde(default)]
    pub breakdown_table: Option<logistics::BreakdownTable>,
    /// Broken-Down Vehicle Repair Table (§22.8).
    #[serde(default)]
    pub repair_table: Option<logistics::RepairTable>,
    /// Axis Truck Production Chart (§20.66a).
    #[serde(default)]
    pub axis_truck_production: Option<logistics::AxisTruckProduction>,
    /// Replacement-Point Production Charts by nationality (§20.66 German / §20.7 Commonwealth).
    #[serde(default)]
    pub production_charts: HashMap<Nationality, logistics::ProductionChart>,
    /// Commonwealth Truck Production Table (§20.78A).
    #[serde(default)]
    pub cw_truck_production: Option<logistics::CommonwealthTruckProduction>,
    /// Commonwealth Infantry Production Table (§20.78B).
    #[serde(default)]
    pub cw_infantry_production: Option<logistics::CwInfantryProduction>,
    /// Replacement Point Conversion Chart (§20.3).
    #[serde(default)]
    pub replacement_conversion: Option<logistics::ReplacementConversion>,
}

impl ReferenceData {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load terrain + hexside parameters from a `terrain.ron` string.
    pub fn load_terrain(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        let f: TerrainFile = ron::from_str(ron_str)?;
        self.terrain = f.terrain;
        self.hexsides = f.hexsides;
        Ok(())
    }

    /// Load one nationality's §4.44A Aircraft Characteristics chart (name → aircraft).
    pub fn load_aircraft_chart(
        &mut self,
        nationality: Nationality,
        ron_str: &str,
    ) -> Result<(), ron::error::SpannedError> {
        let chart: aircraft::AircraftChart = ron::from_str(ron_str)?;
        self.aircraft_charts.insert(nationality, chart);
        Ok(())
    }

    /// Load the §38.37 Aircraft Refit Table.
    pub fn load_aircraft_refit(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.aircraft_refit = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the §40.4 Scramble Table.
    pub fn load_scramble_table(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.scramble_table = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the §40.8 Strafing Table.
    pub fn load_strafing_table(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.strafing_table = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the §41.5 Air Bombardment & Secondary Barrage Targets Table.
    pub fn load_air_bombardment(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.air_bombardment = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the §41.39A Mining Harbor by Plane Table.
    pub fn load_mining_harbor(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.mining_harbor = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the §45.4/§45.5 Air-to-Air Combat tables.
    pub fn load_air_to_air(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.air_to_air = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the §46.3/§46.4 Anti-Aircraft Combat Results Table + Flak Adjustment Chart.
    pub fn load_anti_aircraft(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.anti_aircraft = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the §42.27 Air Reconnaissance of Land Units Table.
    pub fn load_air_recon(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.air_recon = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the §55.3 Port Capacity & Efficiency Level Chart.
    pub fn load_ports_chart(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.ports_chart = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the §44.42 Axis Malta Availability Table.
    pub fn load_malta_availability(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.malta_availability = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the §44.5 Maltese Air Facility Construction Table.
    pub fn load_malta_construction(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.malta_construction = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the §44.41 Axis Strategic Airforce Commitment Chart.
    pub fn load_malta_commitment(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.malta_commitment = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the §56.18 Axis Naval Convoy Air Distance Chart (the six shipping lanes).
    pub fn load_shipping_lanes(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.shipping_lanes = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the §56.4/§56.5 Axis Naval Convoy Level Chart + Capacity Table.
    pub fn load_convoy_levels(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.convoy_levels = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the §42.53 Air Reconnaissance of Axis Naval Convoys Table.
    pub fn load_naval_recon(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.naval_recon = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the §54.5 Equivalent Weights Chart.
    pub fn load_equivalent_weights(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.equivalent_weights = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load one nationality's Unit Characteristics chart (map keyed by ID code).
    pub fn load_unit_characteristics(
        &mut self,
        nationality: Nationality,
        ron_str: &str,
    ) -> Result<(), ron::error::SpannedError> {
        let map: HashMap<String, UnitCharacteristics> = ron::from_str(ron_str)?;
        self.unit_characteristics.insert(nationality, map);
        Ok(())
    }

    /// Load one nationality's Tank & Gun chart (map keyed by weapon name).
    pub fn load_weapons_systems(
        &mut self,
        nationality: Nationality,
        ron_str: &str,
    ) -> Result<(), ron::error::SpannedError> {
        let map: HashMap<String, WeaponsSystem> = ron::from_str(ron_str)?;
        self.weapons_systems.insert(nationality, map);
        Ok(())
    }

    /// Load one nationality's Order of Battle (§4.44a–c).
    pub fn load_order_of_battle(
        &mut self,
        nationality: Nationality,
        ron_str: &str,
    ) -> Result<(), ron::error::SpannedError> {
        let ob: OrderOfBattle = ron::from_str(ron_str)?;
        self.orders_of_battle.insert(nationality, ob);
        Ok(())
    }

    /// Load the Anti-Armor Fire Combat Results Table (§14.6).
    pub fn load_anti_armor_crt(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.anti_armor_crt = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the Barrage Against Land Units Combat Results Table (§12.6).
    pub fn load_barrage_crt(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.barrage_crt = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the Morale Modifier Table (§17.4).
    pub fn load_morale_modifier_table(
        &mut self,
        ron_str: &str,
    ) -> Result<(), ron::error::SpannedError> {
        self.morale_modifier_table = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the Close Assault Combat Results Table (§15.79).
    pub fn load_close_assault_crt(
        &mut self,
        ron_str: &str,
    ) -> Result<(), ron::error::SpannedError> {
        self.close_assault_crt = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the Prisoners Captured Results Table (§15.89).
    pub fn load_prisoners_table(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.prisoners_table = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the Initiative Ratings Chart (§7.2).
    pub fn load_initiative_ratings(
        &mut self,
        ron_str: &str,
    ) -> Result<(), ron::error::SpannedError> {
        self.initiative_ratings = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the reinforcement entry-hex table (§20.14–20.15), keyed by nationality.
    pub fn load_reinforcement_entry(
        &mut self,
        ron_str: &str,
    ) -> Result<(), ron::error::SpannedError> {
        self.reinforcement_entry = ron::from_str(ron_str)?;
        Ok(())
    }

    /// The reinforcement debarkation hex-id for a nationality (§20.14–20.15), if set.
    pub fn reinforcement_entry_hex(&self, nationality: Nationality) -> Option<&str> {
        self.reinforcement_entry.get(&nationality).map(|s| s.as_str())
    }

    /// Load the off-map Tripoli/Tunisia land-movement complex (§8.8).
    pub fn load_off_map_nodes(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.off_map = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the Weather Table (§29.61) + Foul Weather Location Table (§29.7).
    pub fn load_weather_table(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.weather_table = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the Campaign Victory Conditions (§64.7).
    pub fn load_campaign_victory(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.campaign_victory = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the Ammunition Consumption Rates (§50.2).
    pub fn load_ammo_consumption(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.ammo_rates = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the Water Availability Table (§52.7).
    pub fn load_water_availability(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.water_table = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the Truck Characteristics Chart (§54.2).
    pub fn load_truck_chart(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.truck_chart = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the Breakdown Table (§21.38).
    pub fn load_breakdown_table(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.breakdown_table = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the Broken-Down Vehicle Repair Table (§22.8).
    pub fn load_repair_table(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.repair_table = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the Axis Truck Production Chart (§20.66a).
    pub fn load_axis_truck_production(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.axis_truck_production = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load a nationality's Replacement-Point Production Chart (§20.66 / §20.7).
    pub fn load_production_chart(
        &mut self,
        nationality: Nationality,
        ron_str: &str,
    ) -> Result<(), ron::error::SpannedError> {
        self.production_charts.insert(nationality, ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the Commonwealth Truck Production Table (§20.78A).
    pub fn load_cw_truck_production(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.cw_truck_production = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the Commonwealth Infantry Production Table (§20.78B).
    pub fn load_cw_infantry_production(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.cw_infantry_production = Some(ron::from_str(ron_str)?);
        Ok(())
    }

    /// Load the Replacement Point Conversion Chart (§20.3).
    pub fn load_replacement_conversion(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.replacement_conversion = Some(ron::from_str(ron_str)?);
        Ok(())
    }
}
