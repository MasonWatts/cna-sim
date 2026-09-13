//! The `GameState` aggregate and the sequence-of-play phase machine (design.md §3.6).
//!
//! `GameState` is the whole mutable game plus the loaded reference data and a seeded RNG.
//! The `Phase` enum models the Land Game's Sequence of Play (§5.2) at a coarse
//! granularity we can enforce today; sub-steps are added as the systems that need them
//! (movement, combat) are built.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::reference::terrain::{Fixed, HexTerrain, MoveCost};
use crate::reference::ob::Arrival;
use crate::reference::logistics::WaterSource;
use crate::reference::units::{Nationality, Side, WeaponKind};
use crate::reference::ReferenceData;
use crate::rng::Rng;
use crate::rules::movement::{step_cost, MoveMode};
use crate::state::air::{AirFacility, AirFacilityKind, Sgsu, SquadronId};
use crate::state::map::{Feature, HexCoord, HexId, HexsideKind, Map};
use crate::state::unit::{Composition, OrgSize, Unit, UnitId, UnitLocation};

/// Weather for the Operations Stage (§29.0). Resolved per map section (§29.1) via
/// `weather_by_section` / `determine_weather`; this is the overall/default value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Weather {
    Normal,
    Hot,
    Sandstorm,
    Rainstorm,
}

/// Season for weather determination (§29.1). Seasons change at week III of Mar/Jun/Sep/Dec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Season {
    Spring,
    Summer,
    Fall,
    Winter,
}

/// The §29.61 weather-season for a Game-Turn. The Weather Table's own Game-Turn column is
/// authoritative: seasons cycle in 12-GT blocks over a 48-GT year (1-12 Spring, 13-24 Summer,
/// 25-36 Fall, 37-48 Winter, repeating). NB: these table blocks do **not** line up with the
/// calendar-date seasons in §29.1's prose (GT1 = Sep 1940), but the table is what drives
/// weather determination.
pub fn season_of(game_turn: u32) -> Season {
    match (game_turn.saturating_sub(1) % 48) / 12 {
        0 => Season::Spring,
        1 => Season::Summer,
        2 => Season::Fall,
        _ => Season::Winter,
    }
}

/// A calendar date on the Axis Game-Turn Record Track: `year`, `month` (1–12) and `week` (1–4, the
/// Roman numeral I–IV). See `docs/source-materials/turn-record-track.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameDate {
    pub year: u32,
    pub month: u32,
    pub week: u32,
}

/// Map a Game-Turn to its calendar date on the Turn Record Track. The campaign opens on GT1 =
/// September (week III) 1940 (the Italian invasion of Egypt), so September 1940 has only two turns
/// (weeks III–IV, GT1–2); every month thereafter runs four one-week turns (§1: one week per turn).
/// GT35 = Jun I 1941 (Bizerta opens, §55.17), GT46 = Aug IV 1941 (last CW high-evaporation turn),
/// GT110 = Dec IV 1942 (end of the campaign track).
pub fn date_of(game_turn: u32) -> GameDate {
    match game_turn {
        1 => GameDate { year: 1940, month: 9, week: 3 },
        2 => GameDate { year: 1940, month: 9, week: 4 },
        _ => {
            let n = game_turn - 3; // weeks since October (week I) 1940
            let week = n % 4 + 1;
            // October 1940 as a zero-based absolute month index (January = 0).
            let abs = 1940 * 12 + 9 + n / 4;
            GameDate { year: abs / 12, month: abs % 12 + 1, week }
        }
    }
}

/// The four supply commodities of the full Logistics Game, in Points (§49 Fuel, §50 Ammo,
/// §51 Stores, §52 Water). A Fuel Point ≈ 250 lb / 35 gal (§49.11).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Commodities {
    #[serde(default)]
    pub fuel: u32,
    #[serde(default)]
    pub ammo: u32,
    #[serde(default)]
    pub stores: u32,
    #[serde(default)]
    pub water: u32,
}

impl Commodities {
    /// The points of a given commodity.
    pub fn amount(&self, c: Commodity) -> u32 {
        match c {
            Commodity::Fuel => self.fuel,
            Commodity::Ammo => self.ammo,
            Commodity::Stores => self.stores,
            Commodity::Water => self.water,
        }
    }
    /// Mutable access to a given commodity's points.
    pub fn amount_mut(&mut self, c: Commodity) -> &mut u32 {
        match c {
            Commodity::Fuel => &mut self.fuel,
            Commodity::Ammo => &mut self.ammo,
            Commodity::Stores => &mut self.stores,
            Commodity::Water => &mut self.water,
        }
    }
}

/// The result of a resolved bombing mission (§39.0): planes flown, lost to flak, aborted by flak,
/// the bomb points delivered by the survivors, and the Air Bombardment CRT result.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BombingMissionOutcome {
    pub flown: u32,
    /// Bombers shot down by intercepting CAP before flak (§39.0(c/d), §45).
    pub intercepted: u32,
    pub destroyed: u32,
    pub aborted: u32,
    pub delivered_bomb_points: u32,
    pub result: i32,
    /// §45: escort fighters lost defending the bombers against intercepting CAP.
    pub escort_losses: u32,
}

/// A body of fighters engaging in air-to-air combat (§45): its plane count and the per-plane
/// combat ratings (TacAir, pilot Rating, and Maneuver).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flight {
    pub count: u32,
    pub tacair: i32,
    pub pilot: u32,
    pub maneuver: i32,
}

/// Air-to-air losses (§45): planes lost by each side.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AirToAirOutcome {
    pub attacker_losses: u32,
    pub defender_losses: u32,
}

/// What a resolved strafing mission (§40.5–40.8) did to its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrafeEffect {
    /// No effect (Strafing-Table result 0, or below the threshold for this target type).
    None,
    /// §40.64: this percentage of every commodity in the target's enemy Supply Dumps was destroyed.
    SupplyDumpDamaged { percent: u32 },
    /// §40.67: the target hex's water pipeline was destroyed.
    PipelineDestroyed,
    /// §40.6: TOE Strength Points eliminated from an enemy infantry unit in the target hex.
    InfantryEliminated { points: u32 },
    /// §40.6: planes destroyed on the ground (the Strafing-Table result was the number of d6 to
    /// roll; their sum is the planes lost).
    GroundedAircraftDestroyed { planes: u32 },
    /// §40.62: first-line attached Truck Points eliminated from an enemy unit in the target hex.
    TrucksEliminated { points: u32 },
    /// §40.65: TOE weapon points removed from an enemy armored unit by tank-strafing.
    TankPointsEliminated { points: u32 },
}

/// A squadron flying Combat Air Patrol over `center` this Operations Stage (§40.2). Offensive CAP
/// with three or more fighters projects an air Zone of Control (`center` + its six neighbours,
/// §40.23) and must engage enemy missions entering it (§40.26); it may intercept only one mission
/// per stage (§40.27). Defensive CAP covers only `center` and may decline. `flight` is the
/// §16.4 a Dummy Tank Formation: a battalion-size bluff owned by `side`, "placed" at `hex` with a
/// borrowed `designation` (§16.44). No counter, CPA 0, worthless in combat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DummyTank {
    pub side: Side,
    pub hex: HexCoord,
    pub designation: String,
}

/// §28 a stack of Prisoner Points held at `hex`. `captor` guards/feeds them; `original` is the side
/// they belong to (for §28.24 escape → replacement). `guards` Guard Points watch them (§28.2);
/// `cpa_used` is movement expended this Convoy Phase (§28.13, CPA 8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrisonerGroup {
    pub captor: Side,
    pub original: Side,
    pub hex: HexCoord,
    pub points: u32,
    #[serde(default)]
    pub guards: u32,
    #[serde(default)]
    pub cpa_used: u32,
}

/// §28.3 captured equipment (tank/gun/AA TOE taken when a weapons unit surrenders) — reusable by the
/// captor against its former owner once repaired. `nationality` is the original owner (its weapon
/// system / repair rules apply).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturedEquipment {
    pub captor: Side,
    pub hex: HexCoord,
    pub nationality: Nationality,
    pub system: String,
    pub count: u32,
    /// §28.3: captured equipment may not be used until repaired.
    #[serde(default)]
    pub repaired: bool,
    /// §14.46: this entry is a Destroyed Tank Marker (knocked out by Anti-Armor fire, not
    /// surrendered). It starts owned by the tank's side and may be captured by the enemy under
    /// §14.52 (they control the hex); either owner may repair it (§14.54 / §28.3 pipeline).
    #[serde(default)]
    pub destroyed_tank: bool,
}

/// §28.24 escaped prisoners of `side` at `hex` heading home. In the modelled scope they wait to be
/// rejoined (when a friendly unit is within 8 hexes) or recaptured (an enemy enters the hex, §28.25);
/// a far escapee (beyond 8 CP) moves one hex/Operations Stage toward the nearest friendly unit and
/// takes 10% thirst attrition without water (§28.24, in `resolve_prisoner_escapes`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Escapee {
    pub side: Side,
    pub hex: HexCoord,
    pub points: u32,
}

/// committed fighters' combat strength (for the §45 air-to-air resolution).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapPatrol {
    pub squadron: SquadronId,
    pub side: Side,
    pub cap_type: crate::reference::aircraft::CapType,
    pub center: HexCoord,
    pub flight: Flight,
    /// §40.27: air-to-air interceptions already made this Operations Stage (offensive CAP caps at 1).
    #[serde(default)]
    pub intercepts_used: u32,
}

impl CapPatrol {
    /// The hexes this patrol covers for interception (§40.23/§40.26): an offensive CAP of ≥3
    /// fighters covers its air ZOC (centre + six neighbours), otherwise just the patrolled hex.
    /// Defensive CAP covers only its own hex and (§40.26) is not counted for *automatic*
    /// interception, so this is used for offensive patrols.
    pub fn covers(&self, hex: HexCoord) -> bool {
        if crate::rules::air::cap_has_air_zoc(self.cap_type, self.flight.count) {
            crate::rules::air::air_zoc_hexes(self.center).contains(&hex)
        } else {
            self.center == hex
        }
    }
}

impl Commodities {
    /// Whether every commodity is zero (an emptied dump).
    pub fn is_empty(&self) -> bool {
        self.fuel == 0 && self.ammo == 0 && self.stores == 0 && self.water == 0
    }

    /// Read one commodity's amount.
    pub fn get(&self, c: Commodity) -> u32 {
        match c {
            Commodity::Fuel => self.fuel,
            Commodity::Ammo => self.ammo,
            Commodity::Stores => self.stores,
            Commodity::Water => self.water,
        }
    }

    /// Mutable access to one commodity's amount.
    pub fn get_mut(&mut self, c: Commodity) -> &mut u32 {
        match c {
            Commodity::Fuel => &mut self.fuel,
            Commodity::Ammo => &mut self.ammo,
            Commodity::Stores => &mut self.stores,
            Commodity::Water => &mut self.water,
        }
    }
}

/// A single supply commodity selector (§49–52).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Commodity {
    Fuel,
    Ammo,
    Stores,
    Water,
}

/// A Supply Dump (§24.9 / §32.1) holding the four commodities. No combat value; captured if
/// an enemy combat unit enters its hex (§32.13). Fuel is non-denominational (§49.19).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupplyDump {
    pub location: UnitLocation,
    pub side: Side,
    #[serde(default)]
    pub supply: Commodities,
    /// A dummy Supply Dump (§32.18 / §59.53) — no real supplies; removed on discovery.
    #[serde(default)]
    pub dummy: bool,
}

/// A commodity that may be carried by Commonwealth rail supply (§54.33 — not Water).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RailCargo {
    Fuel,
    Ammo,
    Stores,
}

/// A minefield in a hex (§26.1): whose it is, and whether it is real or a dummy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Minefield {
    pub side: Side,
    pub dummy: bool,
}

/// What a multi-stage construction project (§24) will produce on completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConstructionKind {
    /// A Fortification of the given Level (§24.4).
    Fortification { level: u8 },
    /// A stretch of Railroad on the edge between two hexes (§24.6) — an UnfinishedRailroad built
    /// into real track.
    Railroad { a: HexCoord, b: HexCoord },
    /// Ground demolition of a built Railroad stretch (§24.66) — one clean Operations Stage.
    RailroadDemolition { a: HexCoord, b: HexCoord },
    /// Laying a minefield (§24.3), real or dummy, for the given side.
    Minefield { side: Side, dummy: bool },
    /// Clearing the minefield in the hex (§26.13 / §23.22).
    MinefieldRemoval,
    /// A new Temporary Repair Facility (§24.83). Only temporary facilities are constructible.
    RepairFacility,
}

/// An in-progress construction project (§24): it advances one step per Construction Segment in
/// which its builders stay in the hex and expend no CP (§24.12), completing after
/// `stages_required` such stages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstructionProject {
    pub hex: HexCoord,
    pub kind: ConstructionKind,
    pub builders: Vec<UnitId>,
    pub stages_done: u32,
    pub stages_required: u32,
}

/// A legality constraint on where a player-choice Supply Unit may be placed at setup
/// (§59.51 / §60.92 asterisked dumps). All present conditions must hold.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DumpConstraint {
    /// Must be on this map section (e.g. 'C').
    #[serde(default)]
    pub section: Option<char>,
    /// Must be strictly west of (lower column than) this reference hex label.
    #[serde(default)]
    pub west_of: Option<String>,
    /// Must be strictly east of (higher column than) this reference hex label.
    #[serde(default)]
    pub east_of: Option<String>,
    /// May not be within this many hexes of any enemy unit (§60.92 Dump 1/2: "not within
    /// four hexes of a Commonwealth unit").
    #[serde(default)]
    pub not_within_of_enemy: Option<u32>,
}

/// A player-choice group of Supply Units awaiting placement (§59.51 / §60.92). The location
/// is decided later — by a player, an AI agent, or `auto_place_dump_choices` in tests/demos —
/// and validated against `constraint`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingDumpChoice {
    pub side: Side,
    /// A human-readable label from the scenario (e.g. "Dump 1").
    #[serde(default)]
    pub label: String,
    pub count: u32,
    #[serde(default)]
    pub ammo: u32,
    #[serde(default)]
    pub fuel: u32,
    #[serde(default)]
    pub stores: u32,
    #[serde(default)]
    pub water: u32,
    #[serde(default)]
    pub dummy: bool,
    #[serde(default)]
    pub constraint: DumpConstraint,
}

/// Why a player-choice Supply Unit could not be placed (§59.51 / §60.92).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DumpPlacementError {
    /// No pending dump choice at the given index.
    NoSuchChoice,
    /// The chosen hex violates the placement constraint (section / bound / enemy proximity).
    IllegalHex,
}

/// A combat function that consumes Ammunition (§50.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatAction {
    Barrage,
    AntiArmor,
    CloseAssault,
    AntiAir,
}

/// The kind of water source at a hex (§52.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaterKind {
    /// Major City or Oasis: unlimited, never depletes.
    Unlimited,
    /// A village well (the §52.7 "Town" row).
    Village,
    /// A desert well (the §52.7 "Bir" row).
    Bir,
}

/// A vehicle Repair Facility (§22.3 / §22.31).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FacilityKind {
    Temporary,
    Major,
}

/// The class of a Replacement Point (§20.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReplacementClass {
    Infantry,
    Gun,
    Tank,
    Recce,
}

/// A pool of Replacement Points (§20.2) — restore losses when absorbed into a unit (§20.49).
/// Must complete Training (§20.43) before absorption.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplacementPoints {
    pub class: ReplacementClass,
    pub count: u32,
    /// Owning side — selects which §20.43 training areas count (CW §17.32 list vs Axis Tripoli / any
    /// major city).
    pub side: Side,
    #[serde(default)]
    pub location: Option<UnitLocation>,
    /// Operations Stages of Training remaining (§20.43); 0 = trained and absorbable. Counts down only
    /// while the pool sits at a training area for its side (`is_replacement_training_area`).
    #[serde(default)]
    pub training_remaining: u32,
    /// The unit these points are assigned to for delivery (§20.41).
    #[serde(default)]
    pub assigned_to: Option<UnitId>,
}

/// A Replacement Point pool planned to arrive at a future Operations Stage (§20.21) — the Axis
/// plans two Game-Turns ahead, the Commonwealth four. Arrives at the Naval Convoy Arrival Phase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingReplacement {
    pub class: ReplacementClass,
    pub count: u32,
    /// Owning side (carried onto the pool when it arrives — see `ReplacementPoints::side`).
    pub side: Side,
    pub game_turn: u32,
    #[serde(default)]
    pub stage: u32,
    #[serde(default)]
    pub location: Option<UnitLocation>,
    #[serde(default)]
    pub assigned_to: Option<UnitId>,
    /// §20.62: the Axis Naval Convoy tonnage this Replacement pool needs to ship (0 for Commonwealth
    /// production, which lands at Alexandria off-convoy). Used to defer arrivals that exceed the
    /// arrival turn's actual rolled Shipping Tonnage allowance (§20.64 — Replacement Points have
    /// priority over supplies, but cannot exceed the roll itself).
    #[serde(default)]
    pub tonnage: u32,
}

/// A free (unattached) Truck-Point convoy on the map (§8.98/§8.99): Truck Points not attached to a
/// unit, hauling supply `cargo`, that move in the Truck Convoy Phase (§53) respecting their CPA
/// (§8.98) and the §10.29 ZOC restriction. Detached from a unit's first-line trucks (§8.97). This is
/// the counter desert-raider truck-convoy raids target (§27.57-59).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TruckConvoy {
    pub side: Side,
    pub hex: HexCoord,
    pub trucks: crate::state::unit::TruckPoints,
    #[serde(default)]
    pub cargo: Commodities,
    /// Movement expended this Truck Convoy Phase, tenths of a CP (§8.98 CPA cap).
    #[serde(default)]
    pub cpa_used: u32,
    /// §27.57: whether any of the hauled cargo is Replacement Points (affects the §27.58 raid roll).
    #[serde(default)]
    pub carries_replacements: bool,
}

/// The two kinds of desert-raider counter (§27): the Commonwealth Long Range Desert Group (§27.1,
/// up to two) and the Axis Die Sonderkommando Almasy (§27.2, one, German-formed only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RaiderKind {
    Lrdg,
    Dsa,
}

/// A desert-raider counter (§27): a hidden vehicle (Recce type) with a 50-CP Movement Allowance and
/// no combat ratings, no ZOC, and no Stacking Points (§27.14/§27.34). Formed from a trained Recce/AC
/// TOE point (§27.13); on elimination it may re-form four Game-Turns later (§27.16). Position is
/// tracked for real; concealment (§27.32) awaits the §3.5 fog-of-war layer. Supply is special
/// (§27.35): it carries `fuel`/`ammo`, must draw water each Game-Turn (`drew_water_this_gt`) or be
/// eliminated, and needs 2 Fuel Points to move in a Game-Turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Raider {
    pub kind: RaiderKind,
    pub side: Side,
    /// Current hex while in play; `None` while eliminated and awaiting the §27.16 re-form cooldown.
    #[serde(default)]
    pub hex: Option<HexCoord>,
    /// Movement expended this Operations Stage, tenths of a CP (§27.37 cap: 500 = 50 CP).
    #[serde(default)]
    pub cpa_used: u32,
    #[serde(default)]
    pub fuel: u32,
    #[serde(default)]
    pub ammo: u32,
    /// §27.35: whether the raider has drawn Water this Game-Turn (else eliminated at Game-Turn end).
    #[serde(default)]
    pub drew_water_this_gt: bool,
    /// §27.16: the Game-Turn on which an eliminated raider may re-form (0 = in play).
    #[serde(default)]
    pub reform_game_turn: u32,
    /// §27.32/§27.33: whether the enemy can currently see this raider — hidden until it raids or is
    /// spotted (§3.5 fog-of-war). Friendly players always see their own raiders.
    #[serde(default)]
    pub revealed: bool,
}

/// General Rommel's counter (§31): a non-combat vehicle (treated as a 4-wheel-drive medium truck,
/// §31.0-1) with a 60-CP Movement Allowance and no combat ratings. He enters at `arrival_game_turn`
/// at `start_hex` (scenario-placed), then occupies `hex`. `in_germany` is set for a Game-Turn by the
/// §31 recall roll (2d6 = 12), suspending all his effects that turn. `mcp_left` (tenths of a CP) is
/// his remaining movement this Operations Stage; `stage_start_hex` is where he began it (§31.4 needs
/// him to remain the whole stage); `cpa_unit` is the one unit he grants +5 CPA (§31.4, owner ruling).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RommelState {
    pub arrival_game_turn: u32,
    pub start_hex: Option<HexCoord>,
    #[serde(default)]
    pub hex: Option<HexCoord>,
    #[serde(default)]
    pub in_germany: bool,
    #[serde(default)]
    pub mcp_left: u32,
    #[serde(default)]
    pub stage_start_hex: Option<HexCoord>,
    #[serde(default)]
    pub cpa_unit: Option<UnitId>,
}

/// Truck Points scheduled to arrive (§20.78A / §20.66a): both the Commonwealth rolled Truck
/// Production and Axis planned Truck Production land in the side's free-truck pool at the Naval
/// Convoy Arrival Phase of a future Operations Stage (CW +4 Game-Turns at Alexandria; Axis +2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingTruckDelivery {
    pub side: Side,
    pub kind: crate::reference::logistics::TruckKind,
    pub count: u32,
    pub game_turn: u32,
    #[serde(default)]
    pub stage: u32,
}

/// An accepted §20.66/§20.78C production order: `count` Replacement Points of `item` for
/// `nationality`, planned on Game-Turn `game_turn` and due to arrive on `arrival_game_turn`
/// (§20.21 delay), requiring `tonnage` Axis Naval Convoy tons (0 for Commonwealth production,
/// which lands at Alexandria). Axis tonnage is counted against `arrival_game_turn`'s convoy
/// Shipping Tonnage allowance (§20.62).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductionOrder {
    pub nationality: Nationality,
    pub item: String,
    pub game_turn: u32,
    pub arrival_game_turn: u32,
    pub count: u32,
    pub tonnage: u32,
}

/// Classify a §20.66/§20.78C Production Chart item into one of the four Replacement Point
/// classes (§20.2). Order matters: Recce → Infantry → Tank (incl. tracked SP guns) → Gun
/// (the artillery/AA/anti-tank remainder). Returns `None` only for an unrecognized name.
///
/// The SP-gun / tankette calls (Marder, Semovente, `SP …`, the Italian CV/CA light tanks) are
/// classified as Tank — tracked, gun-armed AFVs take Tank Replacement Points (6 Training stages,
/// §20.43), not Gun. Confirmed by the project owner (2026-08-29) against the production charts.
pub fn replacement_class_for_item(item: &str) -> Option<ReplacementClass> {
    if item.contains("Recce") || item.contains("Reconnaissance") {
        return Some(ReplacementClass::Recce);
    }
    if item == "Infantry" {
        return Some(ReplacementClass::Infantry);
    }
    let is_tank = item.starts_with("Pz")
        || item.starts_with("CA ")
        || item.starts_with("CV ")
        || item.starts_with("SP ") // self-propelled guns take Tank points (§20.3, owner-confirmed)
        || item.contains("Cruiser")
        || item.contains("Crusader")
        || item.contains("Semovente")
        || item.contains("Marder")
        || matches!(
            item,
            "Matilda" | "Valentine" | "Stuart" | "Grant" | "Sherman" | "Churchill" | "Mark VI Light"
        );
    if is_tank {
        return Some(ReplacementClass::Tank);
    }
    // The remainder are guns, howitzers, AA, anti-tank, and paratroop artillery → Gun.
    Some(ReplacementClass::Gun)
}

/// Default scenario identifier (serde) — the full Campaign Game.
fn default_scenario() -> String {
    "Campaign Game".to_string()
}

/// Result of evaluating the campaign victory conditions (§64.7): occupation-point totals,
/// the leading side, and the §64.76 victory level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CampaignResult {
    pub axis_vp: u32,
    pub cw_vp: u32,
    /// The side ahead on points, or `None` for a draw.
    pub winner: Option<Side>,
    /// §64.76 level name (Draw / Marginal / Decisive / Smashing Victory).
    pub level: String,
}

/// Coarse Land-Game phases (§5.2). `InitiativeDetermination` runs once per Game-Turn;
/// the rest occur within each of the three Operations Stages — Weather/Organization once
/// (by the Initiative player), then the F–L phases once for each Player (A then B).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    InitiativeDetermination,
    // Stage II — Strategic Air Planning (once per Game-Turn, §33.0 II).
    StrategicAirDesignation,
    MaltaAvailabilityDetermination,
    StrategicMissionAssignment,
    MaltaRaid,
    // Stage III — Naval Convoy (once per Game-Turn, §33.0 III).
    NavalConvoySchedule,
    ConvoyResolution,
    // Stage IV/V/VI — Operations Stage phases (§33.0 IV).
    Weather,
    /// Naval Convoy Arrival Phase (§20.12): reinforcements/replacements enter here.
    NavalConvoyArrival,
    Organization,
    /// Commonwealth Fleet Phase (§33.0 IV.E): ship assignment/repair.
    CommonwealthFleet,
    /// Land Support Air Phase (§33.0 IV.F): tactical air missions.
    LandSupportAir,
    ReserveDesignation,
    MovementAndCombat,
    TruckConvoy,
    /// Commonwealth Rail Movement Phase (§33.0 IV.K).
    CommonwealthRail,
    Repair,
    Patrol,
    // Stage VII — Strategic Air Recovery (once per Game-Turn, §33.0 VII).
    StrategicAirReturn,
    AircraftMaintenance,
}

/// Who acts in a given stage step: the Initiative player (shared setup), or Player A / B.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StageActor {
    Initiative,
    PlayerA,
    PlayerB,
    Axis,
    Commonwealth,
}

/// Stages II–III, run once per Game-Turn before the Operations Stages (§33.0 II Strategic Air
/// Planning + III Naval Convoy). Malta and convoy scheduling are the Axis Player's; the shared
/// planning/resolution steps are sequenced under the Initiative player.
const PRE_TURN_SEQUENCE: [(Phase, StageActor); 6] = [
    (Phase::StrategicAirDesignation, StageActor::Initiative),
    (Phase::MaltaAvailabilityDetermination, StageActor::Axis),
    (Phase::StrategicMissionAssignment, StageActor::Initiative),
    (Phase::MaltaRaid, StageActor::Axis),
    (Phase::NavalConvoySchedule, StageActor::Axis),
    (Phase::ConvoyResolution, StageActor::Initiative),
];

/// Stage VII — Strategic Air Recovery, run once per Game-Turn after the Operations Stages
/// (§33.0 VII): return strategic-mission planes to base, then ready them.
const POST_TURN_SEQUENCE: [(Phase, StageActor); 2] = [
    (Phase::StrategicAirReturn, StageActor::Initiative),
    (Phase::AircraftMaintenance, StageActor::Initiative),
];

/// The ordered steps within one Operations Stage (§33.0 IV): shared setup by the Initiative
/// player (Weather → Naval Convoy Arrival → Organization → Commonwealth Fleet → Land Support Air),
/// then the per-player phases G–M for Player A, then for Player B. The §IV.A "declare A or B"
/// choice is applied via `declare_initiative_order`; the Commonwealth Fleet and Rail phases are
/// Commonwealth-only in effect (§33.0 IV.E/IV.K).
const STAGE_SEQUENCE: [(Phase, StageActor); 17] = [
    (Phase::Weather, StageActor::Initiative),
    (Phase::NavalConvoyArrival, StageActor::Initiative),
    (Phase::Organization, StageActor::Initiative),
    (Phase::CommonwealthFleet, StageActor::Commonwealth),
    (Phase::LandSupportAir, StageActor::Initiative),
    (Phase::ReserveDesignation, StageActor::PlayerA),
    (Phase::MovementAndCombat, StageActor::PlayerA),
    (Phase::TruckConvoy, StageActor::PlayerA),
    (Phase::CommonwealthRail, StageActor::PlayerA),
    (Phase::Repair, StageActor::PlayerA),
    (Phase::Patrol, StageActor::PlayerA),
    (Phase::ReserveDesignation, StageActor::PlayerB),
    (Phase::MovementAndCombat, StageActor::PlayerB),
    (Phase::TruckConvoy, StageActor::PlayerB),
    (Phase::CommonwealthRail, StageActor::PlayerB),
    (Phase::Repair, StageActor::PlayerB),
    (Phase::Patrol, StageActor::PlayerB),
];

/// The complete mutable game state plus reference data and RNG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    /// Immutable reference/rulebook data. Behind `Rc` so cloning a `GameState` (for `dry_run` /
    /// legal-action enumeration) does not deep-copy the entire chart/OB layer — the dominant clone
    /// cost. Mutate (during setup, to load extra charts) only via `reference_mut`.
    pub reference: std::rc::Rc<ReferenceData>,
    pub map: Map,
    pub units: HashMap<UnitId, Unit>,
    /// Squadron Ground Support Units — the Air Game's base counters (§35).
    #[serde(default)]
    pub sgsus: HashMap<SquadronId, Sgsu>,
    /// §34.84/§34.85: each nationality's pool of received-but-unassigned planes, by aircraft type.
    /// Grows each Game-Turn from the reinforcement schedule; drawn down when planes are assigned to
    /// squadrons (§35.2).
    #[serde(default)]
    pub plane_pool: HashMap<Nationality, HashMap<String, u32>>,
    /// §60.32/§60.42: each nationality's pool of unassigned rated pilots, keyed by Rating (1/2/3/…).
    /// Drawn down when a rated pilot is assigned to a squadron (§40.12); rating-0 pilots are unlimited
    /// (§40.16) and are not tracked here.
    #[serde(default)]
    pub pilot_pool: HashMap<Nationality, HashMap<u32, u32>>,
    /// §60.32/§60.42: each nationality's supply of unplaced SGSU counters (Italian 39, Commonwealth
    /// 14 at campaign start). `DeploySgsu` spends one to put a squadron on the map (§34.82).
    #[serde(default)]
    pub available_sgsu: HashMap<Nationality, u32>,
    /// §34.84: squadron withdrawals already performed this Game-Turn, as `(nationality, class,
    /// count)`, so a mandatory `WD` is satisfied exactly (a `WD: 1F` withdraws one fighter squadron,
    /// not every eligible one). Reset when the Game-Turn changes.
    #[serde(default)]
    pub squadron_wd_done: Vec<(Nationality, crate::reference::squadron::WdClass, u32)>,
    #[serde(default)]
    pub squadron_wd_done_gt: u32,
    /// Placed air facilities — airfields, landing strips, flying-boat basins/alighting areas (§36).
    #[serde(default)]
    pub air_facilities: Vec<AirFacility>,
    /// Ports in play, keyed by name (§55).
    #[serde(default)]
    pub ports: HashMap<String, crate::state::naval::Port>,
    /// Planned/in-transit Axis naval convoys (§56).
    #[serde(default)]
    pub convoys: Vec<crate::state::naval::Convoy>,
    /// Commonwealth warships (§30).
    #[serde(default)]
    pub fleet: Vec<crate::state::naval::Ship>,
    /// The San Giorgio wreck blocking Tobruk (§30.17), while present.
    #[serde(default)]
    pub san_giorgio: Option<crate::state::naval::SanGiorgio>,
    /// The Maltese air facilities (§44.1), printed permanently on the map.
    #[serde(default)]
    pub malta_bases: Vec<crate::state::air::MaltaAirBase>,
    pub rng: Rng,
    next_unit_id: u32,

    pub game_turn: u32,
    pub operations_stage: u8,
    pub phase: Phase,
    /// Index into `STAGE_SEQUENCE` for the current stage step (0 while in Initiative
    /// Determination).
    #[serde(default)]
    stage_step: usize,
    /// Index into `PRE_TURN_SEQUENCE` while in the Strategic Air Planning / Naval Convoy stages
    /// (§33.0 II–III); `None` otherwise.
    #[serde(default)]
    pre_turn_step: Option<usize>,
    /// Index into `POST_TURN_SEQUENCE` while in the Strategic Air Recovery stage (§33.0 VII);
    /// `None` otherwise.
    #[serde(default)]
    post_turn_step: Option<usize>,
    pub initiative: Side,
    /// The side currently acting (the Phasing Player for the current step).
    pub active_player: Side,
    /// Player "A" (moves first) for the current Operations Stage — the Initiative player's
    /// §III.A declaration; defaults to the Initiative player each stage.
    #[serde(default = "default_side")]
    pub player_a: Side,
    /// General Rommel (§31): a non-combat moving counter, present once the scenario grants it.
    /// `None` = the Rommel counter is not in this scenario. Drives the Axis Initiative Rating
    /// (§7.2), the §31.3 stack-morale bonus, and the §31.4 CPA bonus — see `rommel_on_map`.
    #[serde(default)]
    pub rommel: Option<RommelState>,
    /// Free (unattached) Truck-Point convoys on the map (§8.98/§53), moved in the Truck Convoy Phase.
    #[serde(default)]
    pub truck_convoys: Vec<TruckConvoy>,
    /// Desert-raider counters in play or awaiting re-form (§27): up to two LRDG + one DSA.
    #[serde(default)]
    pub raiders: Vec<Raider>,
    /// §27.73: the Game-Turn the Layforce commando battalion entered Alexandria (it must spend one
    /// full Game-Turn there before an amphibious landing); `None` while it is not in Alexandria.
    #[serde(default)]
    pub layforce_alexandria_since: Option<u32>,
    /// §27.74: the (Game-Turn, Operations-Stage) through which the just-landed Layforce battalion is
    /// automatically in supply (its landing stage plus two more); `None` when the window is closed.
    #[serde(default)]
    pub layforce_free_supply_until: Option<(u32, u8)>,
    /// §27.84: the (Game-Turn, Operations-Stage) through which a sea/air-transported SAS Brigade is
    /// automatically in supply (transport stage + two more); `None` when closed.
    #[serde(default)]
    pub sas_free_supply_until: Option<(u32, u8)>,
    /// §27.87: the (Game-Turn, Operations-Stage) of the SAS Brigade's last airfield raid, for the
    /// once-a-month / ≥6-Operations-Stages-between-raids limit.
    #[serde(default)]
    pub sas_last_raid: Option<(u32, u8)>,
    /// §27.64: Raids on Rommel attempted so far (max four per Campaign Game).
    #[serde(default)]
    pub raids_on_rommel: u32,
    /// §27.64 "AI temp 3": the last Game-Turn on which a Raid-on-Rommel result forces the Axis
    /// Initiative Rating to the German value (3); 0 = no active window.
    #[serde(default)]
    pub rommel_raid_ai3_until_gt: u32,
    /// The predetermined Initiative for the first Game-Turn (§7.15), if the scenario sets
    /// it; otherwise the first turn is rolled like any other.
    #[serde(default)]
    pub first_turn_initiative: Option<Side>,
    pub weather: Weather,
    /// Per-map-section weather for the current Operations Stage (§29.1): section letter →
    /// weather. Absent = Normal. Storms hit only some sections; Hot is all sections.
    #[serde(default)]
    pub weather_by_section: HashMap<char, Weather>,
    /// Reinforcements not yet on the map: units of a deployed formation whose scheduled
    /// arrival (game_turn, stage) has not been reached. Registered at deploy time and
    /// brought in by `process_arrivals` (§4.44 Organization at Arrival).
    #[serde(default)]
    pub pending_arrivals: Vec<PendingArrival>,
    /// Supply Units on the map / in off-map boxes (§32.1).
    #[serde(default)]
    pub supply_dumps: Vec<SupplyDump>,
    /// Player-choice Supply Unit groups awaiting placement at setup (§59.51 / §60.92).
    #[serde(default)]
    pub pending_dump_choices: Vec<PendingDumpChoice>,
    /// Game-Turn on which the Axis first (continuously) satisfied the §64.71 objective+supply
    /// condition. The auto-win requires holding it for one full Game-Turn; `None` when the
    /// condition is not currently satisfied.
    #[serde(default)]
    pub axis_objective_held_since: Option<u32>,
    /// Village/Bir wells that have been drawn dry (§52.14); replenished by rainstorm (§52.15).
    #[serde(default)]
    pub depleted_wells: std::collections::HashSet<HexCoord>,
    /// Village/Bir wells that have been poisoned (§52.16); un-poisoned by sweetening (§52.17).
    #[serde(default)]
    pub poisoned_wells: std::collections::HashSet<HexCoord>,
    /// §52.14: the side that secretly knows a well is depleted/poisoned — the one that drew it dry
    /// or poisoned it. Until it is publicly revealed, only this side observes the well's true state.
    #[serde(default)]
    pub well_known_by: std::collections::HashMap<HexCoord, Side>,
    /// §52.14: wells whose depleted/poisoned state has been publicly revealed (a "depleted marker"
    /// placed) — an opposing unit drew there and was told. Known to everyone thereafter.
    #[serde(default)]
    pub revealed_wells: std::collections::HashSet<HexCoord>,
    /// Hexes containing a constructed Water Pipeline (§52.2). A pipeline connected back to a major
    /// city is an unlimited water source.
    #[serde(default)]
    pub pipelines: std::collections::HashSet<HexCoord>,
    /// §52.24: sides that have already built a pipeline hex this Operations Stage (at most one hex
    /// per side per stage). Cleared at the start of each Operations Stage.
    #[serde(default)]
    pub pipeline_built_this_stage: std::collections::HashSet<Side>,
    /// Repair Facilities by hex (§22.3 / §22.31 / §24.8).
    #[serde(default)]
    pub repair_facilities: std::collections::HashMap<HexCoord, FacilityKind>,
    /// Replacement Point pools (§20.2) awaiting training / absorption.
    #[serde(default)]
    pub replacement_points: Vec<ReplacementPoints>,
    /// Replacement Points planned to arrive at a future Naval Convoy Arrival Phase (§20.21).
    #[serde(default)]
    pub pending_replacements: Vec<PendingReplacement>,
    /// Accepted §20.66/§20.78C production orders, for enforcing lifetime totals and per-period
    /// maxima across a game (§20.66 notes). Each records the Game-Turn it was planned.
    #[serde(default)]
    pub production_orders: Vec<ProductionOrder>,
    /// §56.21: the Axis Naval Convoy Shipping Tonnage allowance actually rolled for a Game-Turn
    /// (keyed by arrival Game-Turn). Rolled once per turn (the first convoy plan fixes it), so both
    /// supply convoys and §20.62 production bind against the same figure. Absent ⇒ not yet rolled.
    #[serde(default)]
    pub axis_shipping_rolled: std::collections::HashMap<u32, u32>,
    /// §16.21: sides that engaged in Anti-Armor or Close Assault this Operations Stage — such a
    /// (phasing) side may not Patrol this stage. Reset each stage.
    #[serde(default)]
    pub land_combat_this_stage: std::collections::HashSet<Side>,
    /// §16.21: hexes that underwent Barrage or Air Bombardment this Operations Stage — they may not
    /// be reconnoitred by a Patrol this stage. Reset each stage.
    #[serde(default)]
    pub barraged_hexes_this_stage: std::collections::HashSet<HexCoord>,
    /// §15.23: hexes already Close-Assaulted this Combat Segment — a hex may not be close-assaulted
    /// more than once per segment. Reset each stage.
    #[serde(default)]
    pub close_assaulted_hexes_this_stage: std::collections::HashSet<HexCoord>,
    /// §14.22: hexes already targeted by Anti-Armor fire this Combat Segment (once-per-segment).
    /// Reset each stage.
    #[serde(default)]
    pub anti_armor_hexes_this_stage: std::collections::HashSet<HexCoord>,
    /// §16.4: Dummy Tank Formations in play (≤3 per side, ≤1 per hex). No real counter — a bluff
    /// worth nothing in combat that may be disclosed as a real tank battalion during reconnaissance
    /// (§16.45) and is eliminated by close assault / armorless anti-armor (§16.46).
    #[serde(default)]
    pub dummy_tanks: Vec<DummyTank>,
    /// §28 Prisoner Point stacks held on the map (until they reach a Departure Point, §28.16).
    #[serde(default)]
    pub prisoner_groups: Vec<PrisonerGroup>,
    /// §28.3 captured enemy equipment held on the map, reusable once repaired.
    #[serde(default)]
    pub captured_equipment: Vec<CapturedEquipment>,
    /// §28.16 Prisoner Departure Points by captor (Commonwealth: Alexandria/Cairo; Axis: the Sirte
    /// box). Prisoners entering are removed permanently. Populated at scenario setup.
    #[serde(default)]
    pub pow_departure_points: std::collections::HashMap<Side, std::collections::HashSet<HexCoord>>,
    /// §28.24 escaped prisoners waiting to rejoin their own side or be recaptured (§28.25).
    #[serde(default)]
    pub escapees: Vec<Escapee>,
    /// §42.27: `(viewer, unit)` pairs whose TOE Strength an air-reconnaissance mission has revealed
    /// to `viewer` this Operations Stage — `observe_units` shows the strength of these enemy units.
    /// Cleared each Operations Stage (recon is a per-stage snapshot).
    #[serde(default)]
    pub recon_revealed: std::collections::HashSet<(Side, UnitId)>,
    /// Truck Points produced (§20.78A / §20.66a) awaiting arrival at their delivery city.
    #[serde(default)]
    pub pending_trucks: Vec<PendingTruckDelivery>,
    /// Free Truck Points delivered by production and awaiting assignment to units, by side
    /// (§20.78A trucks arrive at Alexandria; §20.66a Axis trucks at their entry port).
    #[serde(default)]
    pub truck_pools: std::collections::HashMap<Side, crate::state::unit::TruckPoints>,
    /// Lifetime Truck Points planned per side per §20.66a row (Axis planned truck production), for
    /// enforcing the row's lifetime total. Keyed by (side, TruckKind discriminant name).
    #[serde(default)]
    pub axis_truck_orders: Vec<PendingTruckDelivery>,
    /// Squadrons currently flying Combat Air Patrol (§40.2), reset at the start of each Operations
    /// Stage. Enemy missions entering an offensive patrol's coverage are intercepted (§40.26).
    #[serde(default)]
    pub cap_patrols: Vec<CapPatrol>,
    /// The pilot roster (§40.1). Each pilot carries a Rating that adds to a plane's TacAir in
    /// air-to-air combat (§40.15) and is assigned to a squadron (§40.12).
    #[serde(default)]
    pub pilots: Vec<crate::state::air::Pilot>,
    /// §42.48: the absolute Operations-Stage index at which each unit was last airdropped — a unit
    /// may not be airdropped more than once per 12 Operations Stages (a month).
    #[serde(default)]
    pub airdrop_last_stage: std::collections::HashMap<UnitId, u32>,
    /// The scenario identifier, used for per-scenario limits (e.g. the §44.41 Malta Strategic
    /// Airforce Commitment Chart). Defaults to the full Campaign Game.
    #[serde(default = "default_scenario")]
    pub scenario_name: String,
    /// §44.23/§44.41: how many Game-Turns each Axis Malta Availability Level (1–4) has been used —
    /// no level may be used more Game-Turns than the commitment chart allows for the scenario.
    #[serde(default)]
    pub malta_level_used: std::collections::HashMap<u32, u32>,
    /// Replacement Points consumed (absorbed) so far, per side — for the §64.74 unused-
    /// replacement victory points.
    #[serde(default)]
    pub replacements_used: std::collections::HashMap<Side, u32>,
    /// Commonwealth battalions currently voluntarily withdrawn (§64.75) → the Game-Turn withdrawn.
    #[serde(default)]
    pub withdrawn: std::collections::HashMap<UnitId, u32>,
    /// Banked §64.75 Withdrawal Points from completed withdraw/return cycles (may be negative).
    #[serde(default)]
    pub withdrawal_banked: i32,
    /// Units that may not be re-withdrawn until the given Game-Turn (§64.75 B: six months).
    #[serde(default)]
    pub withdrawal_lockout: std::collections::HashMap<UnitId, u32>,
    /// §52.6 Italian Pasta Rule: Italian battalions that did NOT receive their Pasta Point (1
    /// Water) this Game-Turn — they may not voluntarily exceed their CPA.
    #[serde(default)]
    pub no_pasta: std::collections::HashSet<UnitId>,
    /// §52.6: cohesion saved when a pasta-less Italian battalion (cohesion ≤ −10) was force-
    /// Disorganized to −26; restored when it next receives its Pasta Point.
    #[serde(default)]
    pub pasta_saved_cohesion: std::collections::HashMap<UnitId, i32>,
    /// §24.4/§25.1: placed/constructed Fortification Level per hex (1 or 2; scenario-placed forts
    /// too). Static city levels (§25.12) are added on top in `fort_level`.
    #[serde(default)]
    pub fortifications: std::collections::HashMap<HexCoord, u8>,
    /// §41.37: levels knocked off a Major City's *static* §25.12 Fortification Level by bombing.
    /// `fort_level` subtracts this from the city's inherent level (capped at it).
    #[serde(default)]
    pub city_fort_damage: std::collections::HashMap<HexCoord, u8>,
    /// §51.23: units the owning player has placed on half rations (stores requirement 4→2, but
    /// may not voluntarily exceed CPA or enter an enemy ZOC).
    #[serde(default)]
    pub half_rations: std::collections::HashSet<UnitId>,
    /// §24: in-progress multi-stage construction projects (fortifications, etc.).
    #[serde(default)]
    pub construction: Vec<ConstructionProject>,
    /// §26: minefields by hex (one per hex, §24.35).
    #[serde(default)]
    pub minefields: std::collections::HashMap<HexCoord, Minefield>,
    /// §26.23: dummy minefields an enemy unit has entered this Operations Stage — revealed now but
    /// not removed until the end of the Movement Phase (so same-phase movers still pay). Cleared
    /// (and the fields removed) at the start of the next Operations Stage.
    #[serde(default)]
    pub revealed_minefields: std::collections::HashSet<HexCoord>,
    /// §8.74: whether the Commonwealth has used Rail Movement west / east this Operations Stage
    /// (one stack each direction). Reset each stage.
    #[serde(default)]
    pub rail_west_used: bool,
    #[serde(default)]
    pub rail_east_used: bool,
    /// §54.34: the Operations Stage each calendar month the railroad is unavailable (it carries
    /// water forward for its own use). While set, no Rail Movement or rail supply is possible.
    #[serde(default)]
    pub rail_maintenance_this_stage: bool,
    /// §54.41: the last Side to pass a land combat unit through a rail hex "controls" it. A hex
    /// absent from this map is controlled by the Commonwealth (the railroad's owner). Used to gate
    /// Axis use of the coastal railroad (§54.4).
    #[serde(default)]
    pub rail_control: std::collections::HashMap<HexCoord, Side>,
    /// §54.43: the Axis's activated Rolling Stock, as tons of haulage capacity available per
    /// Operations Stage in one direction (each 250 Stores + 100 Fuel imported activates +300 tons).
    /// Persists until the Axis loses the §54.41 five-contiguous-hex control, when it is destroyed
    /// (§54.45).
    #[serde(default)]
    pub axis_rolling_stock_tons: u32,
    /// §54.43/44: tons of Axis rail haulage used so far this Operations Stage (units at 900 t/Stacking
    /// Point, supplies by §54.5 weight). Reset each stage.
    #[serde(default)]
    pub axis_rail_tons_used: u32,
    /// §54.43/44: the direction (Some(true)=west) the Axis committed its rail to this Operations
    /// Stage — haulage is "in any one direction" per stage. `None` until the first Axis rail use.
    #[serde(default)]
    pub axis_rail_dir: Option<bool>,
}

/// A deployed formation's later-arriving units, to be spawned when their turn/stage comes.
/// Reuses the original placement's hex + dispersion so arrivals join the formation.
/// Direction of off-map movement in the §8.8 complex.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OffMapDir {
    /// Toward the map (toward Nofilia / lower node index).
    TowardMap,
    /// Inland, away from the map (toward Tunis / higher node index).
    Inland,
}

/// Outcome of one Operations Stage of off-map movement toward the map (§8.8/§8.84).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OffMapMove {
    /// Still moving along the current leg (in an In-Transit box).
    InTransit,
    /// Completed a leg and reached an adjacent box, named.
    Arrived(String),
    /// Completed the last leg toward the map and entered it at the junction (Nofilia, §8.85).
    EnteredMap(HexCoord),
}

/// Why an off-map move could not be performed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OffMapError {
    /// The unit is not in the off-map complex.
    NotOffMap,
    /// No off-map network is loaded.
    NoNetwork,
    /// The unit's node isn't in the network.
    UnknownNode,
    /// The network's junction hex-id didn't parse.
    BadJunction,
    /// Tried to move inland from the innermost box (no box further inland).
    NoInlandBox,
    /// The unit has no CPA (missing characteristics).
    NoCpa,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PendingArrival {
    pub nationality: Nationality,
    pub formation: String,
    /// Where the formation enters — an on-map hex or an off-map node (§8.8/§20.15).
    pub entry: UnitLocation,
    pub unit_hexes: Vec<(String, HexCoord)>,
    pub game_turn: u32,
    pub stage: u8,
}

fn default_side() -> Side {
    Side::Commonwealth
}

impl GameState {
    pub fn new(reference: ReferenceData, seed: u64) -> Self {
        Self {
            reference: std::rc::Rc::new(reference),
            map: Map::new(),
            units: HashMap::new(),
            sgsus: HashMap::new(),
            plane_pool: HashMap::new(),
            pilot_pool: HashMap::new(),
            available_sgsu: HashMap::new(),
            squadron_wd_done: Vec::new(),
            squadron_wd_done_gt: 0,
            air_facilities: Vec::new(),
            ports: HashMap::new(),
            convoys: Vec::new(),
            fleet: Vec::new(),
            san_giorgio: None,
            malta_bases: Vec::new(),
            rng: Rng::new(seed),
            next_unit_id: 0,
            game_turn: 1,
            operations_stage: 1,
            phase: Phase::InitiativeDetermination,
            stage_step: 0,
            pre_turn_step: None,
            post_turn_step: None,
            initiative: Side::Commonwealth,
            active_player: Side::Commonwealth,
            player_a: Side::Commonwealth,
            rommel: None,
            truck_convoys: Vec::new(),
            raiders: Vec::new(),
            layforce_alexandria_since: None,
            layforce_free_supply_until: None,
            sas_free_supply_until: None,
            sas_last_raid: None,
            raids_on_rommel: 0,
            rommel_raid_ai3_until_gt: 0,
            first_turn_initiative: None,
            weather: Weather::Normal,
            weather_by_section: HashMap::new(),
            pending_arrivals: Vec::new(),
            supply_dumps: Vec::new(),
            pending_dump_choices: Vec::new(),
            axis_objective_held_since: None,
            depleted_wells: std::collections::HashSet::new(),
            poisoned_wells: std::collections::HashSet::new(),
            well_known_by: std::collections::HashMap::new(),
            revealed_wells: std::collections::HashSet::new(),
            pipelines: std::collections::HashSet::new(),
            pipeline_built_this_stage: std::collections::HashSet::new(),
            repair_facilities: std::collections::HashMap::new(),
            replacement_points: Vec::new(),
            pending_replacements: Vec::new(),
            production_orders: Vec::new(),
            axis_shipping_rolled: std::collections::HashMap::new(),
            land_combat_this_stage: std::collections::HashSet::new(),
            barraged_hexes_this_stage: std::collections::HashSet::new(),
            close_assaulted_hexes_this_stage: std::collections::HashSet::new(),
            anti_armor_hexes_this_stage: std::collections::HashSet::new(),
            dummy_tanks: Vec::new(),
            prisoner_groups: Vec::new(),
            captured_equipment: Vec::new(),
            pow_departure_points: std::collections::HashMap::new(),
            escapees: Vec::new(),
            pending_trucks: Vec::new(),
            truck_pools: std::collections::HashMap::new(),
            axis_truck_orders: Vec::new(),
            cap_patrols: Vec::new(),
            pilots: Vec::new(),
            airdrop_last_stage: std::collections::HashMap::new(),
            scenario_name: default_scenario(),
            malta_level_used: std::collections::HashMap::new(),
            replacements_used: std::collections::HashMap::new(),
            withdrawn: std::collections::HashMap::new(),
            withdrawal_banked: 0,
            withdrawal_lockout: std::collections::HashMap::new(),
            no_pasta: std::collections::HashSet::new(),
            pasta_saved_cohesion: std::collections::HashMap::new(),
            fortifications: std::collections::HashMap::new(),
            city_fort_damage: std::collections::HashMap::new(),
            recon_revealed: std::collections::HashSet::new(),
            half_rations: std::collections::HashSet::new(),
            construction: Vec::new(),
            minefields: std::collections::HashMap::new(),
            revealed_minefields: std::collections::HashSet::new(),
            rail_west_used: false,
            rail_east_used: false,
            rail_maintenance_this_stage: false,
            rail_control: std::collections::HashMap::new(),
            axis_rolling_stock_tons: 0,
            axis_rail_tons_used: 0,
            axis_rail_dir: None,
        }
    }

    /// Mutable access to the reference data, for loading extra charts into an already-built engine
    /// during setup. Valid only while the reference is not shared by a clone (`Rc::get_mut`); it
    /// panics otherwise, which is correct — mutating shared rulebook data would be a bug.
    pub fn reference_mut(&mut self) -> &mut ReferenceData {
        std::rc::Rc::get_mut(&mut self.reference).expect("reference data is uniquely owned during setup")
    }

    /// Replace this state's RNG with a fresh one seeded from `seed`. Used by Monte-Carlo rollout
    /// agents: cloned probe states share the parent's RNG, so applying the same command to two
    /// clones yields *identical* stochastic outcomes; reseeding each probe gives the independent
    /// dice draws needed to sample (and average over) combat/breakdown variance. Never call this on
    /// a live game — it would break `(seed, commands)` reproducibility.
    pub fn reseed_rng(&mut self, seed: u64) {
        self.rng = Rng::new(seed);
    }

    /// Advance the Sequence of Play until the next Movement & Combat phase, or until `game_turn`
    /// exceeds `turn_limit`. Returns `true` if a Movement & Combat phase was reached (the next
    /// decision point), `false` if the turn horizon was hit first. If already at a Movement &
    /// Combat phase it advances at least one phase first, so repeated calls make progress. Used by
    /// cross-phase planning agents to jump between decision points across the SoP.
    pub fn advance_to_movement_combat(&mut self, turn_limit: u32) -> bool {
        let mut guard = 0u32;
        self.advance_phase();
        while self.game_turn <= turn_limit {
            if self.phase == Phase::MovementAndCombat {
                return true;
            }
            self.advance_phase();
            guard += 1;
            if guard > 100_000 {
                return false;
            }
        }
        false
    }

    /// Create a unit with default dynamic state and register it, returning its id.
    pub fn spawn_unit(
        &mut self,
        nationality: Nationality,
        id_code: impl Into<String>,
        name: impl Into<String>,
        composition: Composition,
    ) -> UnitId {
        let id = UnitId(self.next_unit_id);
        self.next_unit_id += 1;
        self.units
            .insert(id, Unit::new(id, nationality, id_code, name, composition));
        // §49.14: a unit enters play with full tanks (no-op for fuel-less types).
        self.refuel_to_capacity(id);
        // §50.17: a combat unit enters play with a one-fire Ammo reserve (no-op without rates).
        self.rearm_fire_once(id);
        id
    }

    pub fn unit(&self, id: UnitId) -> Option<&Unit> {
        self.units.get(&id)
    }

    /// Total Basic Stacking Points of the independent on-map counters in `hex` (§9.14).
    /// Attached units are represented by their parent's counter and are not counted here.
    pub fn hex_stacking_points(&self, hex: HexCoord) -> u32 {
        self.units
            .values()
            .filter(|u| u.hex() == Some(hex))
            .map(|u| self.effective_stacking_points(u.id))
            .sum()
    }

    pub fn unit_mut(&mut self, id: UnitId) -> Option<&mut Unit> {
        self.units.get_mut(&id)
    }

    /// A unit's Capability Point Allowance from its Unit Characteristics (§6.11).
    /// (A motorized unit riding trucks assumes the truck's CPA via effective_cpa
    /// is modeled; this returns the unit's own printed CPA.)
    pub fn unit_cpa(&self, id: UnitId) -> Option<i32> {
        let u = self.units.get(&id)?;
        self.reference
            .unit_characteristics
            .get(&u.nationality)?
            .get(&u.id_code)
            .map(|uc| uc.cpa.value)
    }

    /// A unit's total Fuel consumption rate (§49.13): Fuel Points per five Capability Points of
    /// movement. Summed over the unit's vehicle weapon-system TOE points **and** its attached
    /// first-line Truck Points (§49.12/18 — trucks are fuel users). Leg infantry with no trucks
    /// burns nothing.
    pub fn unit_fuel_rate_total(&self, id: UnitId) -> u32 {
        let Some(u) = self.units.get(&id) else {
            return 0;
        };
        // Weapon-system vehicles (§49.13): zero for leg infantry (Organic composition).
        let weapons_rate: u32 = match &u.composition {
            Composition::Weapons { groups } => groups
                .iter()
                .map(|grp| {
                    let rate = self
                        .reference
                        .weapons_systems
                        .get(&u.nationality)
                        .and_then(|m| m.get(&grp.system))
                        .and_then(|w| w.fuel_rate)
                        .unwrap_or(0)
                        .max(0) as u32;
                    grp.count * rate
                })
                .sum(),
            Composition::Organic { .. } => 0,
        };
        // Attached first-line Truck Points also consume fuel when moving (§49.12/18).
        let truck_rate: u32 = match self.reference.truck_chart.as_ref() {
            Some(chart) => {
                let t = &u.trucks;
                t.light * chart.light.fuel_consumption_factor
                    + t.medium * chart.medium.fuel_consumption_factor
                    + t.heavy * chart.heavy.fuel_consumption_factor
            }
            None => 0,
        };
        weapons_rate + truck_rate
    }

    /// A unit's total onboard Fuel capacity (§49.14): CPA × ⅕ × (total fuel rate). Zero for
    /// fuel-less units.
    pub fn unit_fuel_capacity(&self, id: UnitId) -> u32 {
        let total_rate = self.unit_fuel_rate_total(id);
        if total_rate == 0 {
            return 0;
        }
        let cpa = self.unit_cpa(id).unwrap_or(0).max(0) as u32;
        cpa * total_rate / 5
    }

    /// Fuel Points consumed to expend `cp_tenths` of movement (§49.13): total fuel rate ×
    /// ⌈CP/5⌉ (5 CP = 50 tenths). Zero for fuel-less units.
    pub fn unit_move_fuel_cost(&self, id: UnitId, cp_tenths: i32) -> u32 {
        if cp_tenths <= 0 {
            return 0;
        }
        let total_rate = self.unit_fuel_rate_total(id);
        if total_rate == 0 {
            return 0;
        }
        let groups = ((cp_tenths + 49) / 50) as u32; // ⌈CP / 5⌉, with 5 CP = 50 tenths
        total_rate * groups
    }

    /// Fill a unit's tanks to its Fuel capacity (§49.14). No-op for fuel-less units.
    pub fn refuel_to_capacity(&mut self, id: UnitId) {
        let cap = self.unit_fuel_capacity(id);
        if let Some(u) = self.unit_mut(id) {
            u.fuel = cap;
        }
    }

    /// §49.17: "siphon gas" in an emergency — transfer up to `amount` Fuel Points from `donor` to
    /// `receiver` when both occupy the same hex, bounded by the donor's Fuel and the room left in
    /// the receiver's Fuel Capacity. A successful siphon costs **3 CP to both units**. Fuel is
    /// non-denominational (§49.19), so `donor` may be an enemy abandoned/broken-down vehicle.
    /// (Whether it is truly an "emergency" — §49.17 — is left to the caller.) Returns the amount
    /// transferred.
    pub fn siphon_fuel(&mut self, donor: UnitId, receiver: UnitId, amount: u32) -> u32 {
        if donor == receiver {
            return 0;
        }
        let dh = self.units.get(&donor).and_then(|u| u.hex());
        let rh = self.units.get(&receiver).and_then(|u| u.hex());
        if dh.is_none() || dh != rh {
            return 0;
        }
        let room = self
            .unit_fuel_capacity(receiver)
            .saturating_sub(self.units.get(&receiver).map_or(0, |u| u.fuel));
        let avail = self.units.get(&donor).map_or(0, |u| u.fuel);
        let moved = amount.min(avail).min(room);
        if moved == 0 {
            return 0;
        }
        if let Some(u) = self.unit_mut(donor) {
            u.fuel -= moved;
        }
        if let Some(u) = self.unit_mut(receiver) {
            u.fuel += moved;
        }
        let cpa_d = self.unit_cpa(donor).unwrap_or(0);
        let cpa_r = self.unit_cpa(receiver).unwrap_or(0);
        self.expend_cp_against(donor, Fixed(30), cpa_d); // 3 CP
        self.expend_cp_against(receiver, Fixed(30), cpa_r);
        moved
    }

    /// Ammo cost (§50.14) for `id` performing `action`: rate × current TOE. Close-assault ammo
    /// applies only to weapon-holding (armor/gun-class) units; zero when the ammo table is
    /// unloaded or the unit does not pay for the action.
    pub fn unit_combat_ammo_cost(&self, id: UnitId, action: CombatAction) -> u32 {
        let Some(rates) = self.reference.ammo_rates else {
            return 0;
        };
        let rate = match action {
            CombatAction::Barrage => rates.barrage,
            CombatAction::AntiArmor => rates.anti_armor,
            CombatAction::CloseAssault => rates.close_assault,
            CombatAction::AntiAir => rates.anti_air,
        };
        if matches!(action, CombatAction::CloseAssault)
            && !matches!(
                self.units.get(&id).map(|u| &u.composition),
                Some(Composition::Weapons { .. })
            )
        {
            return 0; // plain rifle infantry pays no Ammo for close assault (§50.2)
        }
        let toe = self.units.get(&id).map_or(0, |u| u.total_toe());
        rate * toe
    }

    /// §15.15/§15.88: whether a defender is "out of Ammunition for Close Assault" — a melee weapons
    /// unit (not a gun; rifle infantry pays none) whose close-assault Ammo cost exceeds what it can
    /// draw. Such a defender auto-surrenders when assaulted.
    pub(crate) fn out_of_close_assault_ammo(&self, id: UnitId) -> bool {
        if self.is_gun_unit(id)
            || !matches!(self.units.get(&id).map(|u| &u.composition), Some(Composition::Weapons { .. }))
        {
            return false;
        }
        let cost = self.unit_combat_ammo_cost(id, CombatAction::CloseAssault);
        cost > 0 && self.unit_ammo_available(id) < cost
    }

    /// Ammo available to a firing unit: its onboard reserve plus friendly non-dummy Supply
    /// Dumps in its hex (§50.15).
    fn unit_ammo_available(&self, id: UnitId) -> u32 {
        let Some(u) = self.units.get(&id) else {
            return 0;
        };
        let dumps: u32 = match u.hex() {
            Some(h) => self
                .supply_dumps
                .iter()
                .filter(|d| d.side == u.nationality.side() && !d.dummy && d.location.hex() == Some(h))
                .map(|d| d.supply.ammo)
                .sum(),
            None => 0,
        };
        u.ammo + dumps
    }

    /// Expend combat Ammo for `units` performing `action` (§50.14): each draws its cost from its
    /// onboard reserve, then Supply Dumps in its hex. Returns `false` (consuming nothing) if any
    /// unit cannot pay — the caller then rejects the combat (§50.12). No-op when the ammo table
    /// is not loaded.
    pub fn consume_combat_ammo(&mut self, units: &[UnitId], action: CombatAction) -> bool {
        if self.reference.ammo_rates.is_none() {
            return true;
        }
        for &u in units {
            let cost = self.unit_combat_ammo_cost(u, action);
            if cost > 0 && self.unit_ammo_available(u) < cost {
                return false;
            }
        }
        for &u in units {
            let cost = self.unit_combat_ammo_cost(u, action);
            if cost == 0 {
                continue;
            }
            let onboard = self.units.get(&u).map_or(0, |x| x.ammo);
            let from_onboard = onboard.min(cost);
            if let Some(x) = self.unit_mut(u) {
                x.ammo -= from_onboard;
            }
            let mut short = cost - from_onboard;
            if short > 0 {
                let (hex, side) = {
                    let x = self.units.get(&u).expect("checked");
                    (x.hex(), x.nationality.side())
                };
                if let Some(h) = hex {
                    for d in self.supply_dumps.iter_mut().filter(|d| {
                        d.side == side && !d.dummy && d.location.hex() == Some(h)
                    }) {
                        if short == 0 {
                            break;
                        }
                        let take = short.min(d.supply.ammo);
                        d.supply.ammo -= take;
                        short -= take;
                    }
                }
            }
        }
        true
    }

    /// Rearm a unit's onboard Ammunition to a one-fire reserve (§50.17), the heaviest single
    /// action (barrage rate × TOE). No-op when the ammo table is not loaded.
    pub fn rearm_fire_once(&mut self, id: UnitId) {
        let Some(rates) = self.reference.ammo_rates else {
            return;
        };
        let toe = self.units.get(&id).map_or(0, |u| u.total_toe());
        let cap = rates.barrage * toe;
        if let Some(u) = self.unit_mut(id) {
            u.ammo = cap;
        }
    }

    /// §50.14: expend **defensive** close-assault Ammunition for the units defending a Close Assault.
    /// A defender fires defensively as it is assaulted, so — like the attacker — its melee weapons
    /// units spend the close-assault rate × TOE in Ammo. Guns take no part in the melee (§15.21/§15.84a)
    /// and plain rifle infantry pays no Ammo (§50.2), so neither is charged. Unlike the attacker's
    /// pre-combat check (§50.12, which can forbid the assault), the defender is already committed:
    /// each unit pays what it has (onboard reserve, then co-located dumps), capped at what is
    /// available — a best-effort drain that never rejects the resolved combat. No-op without the
    /// ammo table. Returns the total Ammo Points expended.
    pub fn expend_defensive_assault_ammo(&mut self, defenders: &[UnitId]) -> u32 {
        if self.reference.ammo_rates.is_none() {
            return 0;
        }
        let mut spent = 0;
        for &id in defenders {
            // Only melee weapons units pay: guns don't fight the melee; Organic infantry pays none.
            if self.is_gun_unit(id) {
                continue;
            }
            if !matches!(self.units.get(&id).map(|u| &u.composition), Some(Composition::Weapons { .. })) {
                continue;
            }
            let cost = self.unit_combat_ammo_cost(id, CombatAction::CloseAssault);
            if cost == 0 {
                continue;
            }
            let mut owed = cost.min(self.unit_ammo_available(id));
            let onboard = self.units.get(&id).map_or(0, |u| u.ammo);
            let from_onboard = onboard.min(owed);
            if let Some(u) = self.unit_mut(id) {
                u.ammo -= from_onboard;
            }
            owed -= from_onboard;
            spent += from_onboard;
            if owed > 0 {
                let (hex, side) = {
                    let u = self.units.get(&id).expect("checked");
                    (u.hex(), u.nationality.side())
                };
                if let Some(h) = hex {
                    for d in self.supply_dumps.iter_mut().filter(|d| {
                        d.side == side && !d.dummy && d.location.hex() == Some(h)
                    }) {
                        if owed == 0 {
                            break;
                        }
                        let take = owed.min(d.supply.ammo);
                        d.supply.ammo -= take;
                        owed -= take;
                        spent += take;
                    }
                }
            }
        }
        spent
    }

    /// A unit's Stores requirement per Game-Turn (§51.11/13): 4 per TOE Strength Point, or a
    /// flat 1 for a Headquarters or Engineer unit.
    pub fn unit_stores_requirement(&self, id: UnitId) -> u32 {
        let Some(u) = self.units.get(&id) else {
            return 0;
        };
        let ut = self
            .reference
            .unit_characteristics
            .get(&u.nationality)
            .and_then(|m| m.get(&u.id_code))
            .map(|uc| uc.unit_type.as_str())
            .unwrap_or("");
        if ut.contains("Headquarters") || ut.contains("Engineer") {
            1
        } else if self.half_rations.contains(&id) {
            2 * u.total_toe() // §51.23: half rations cut the requirement from 4 to 2 per TOE.
        } else {
            4 * u.total_toe()
        }
    }

    /// Water Points a unit needs this Operations Stage (§52.41/42): an infantry battalion or
    /// company needs 1 regardless of TOE; a vehicle/truck unit needs 1 per TOE Strength Point,
    /// but only if it used any CPA this stage (`acted`). Hot-weather surcharge (§52.43/§29.3)
    /// is deferred.
    pub fn unit_water_requirement(&self, id: UnitId, acted: bool) -> u32 {
        let Some(u) = self.units.get(&id) else {
            return 0;
        };
        match &u.composition {
            // §52.41: the flat 1 Water Point applies to an *infantry battalion or company*.
            // HQs and Engineers are also `Organic` but are neither, so they owe no water (they
            // are likewise special-cased for Stores in `unit_stores_requirement`, §51.13). A
            // non-infantry Organic unit is thus never "deprived" and never takes §52.53 attrition.
            Composition::Organic { .. } => {
                let ut = self
                    .reference
                    .unit_characteristics
                    .get(&u.nationality)
                    .and_then(|m| m.get(&u.id_code))
                    .map(|uc| uc.unit_type.as_str())
                    .unwrap_or("");
                if ut.contains("Headquarters") || ut.contains("Engineer") {
                    0
                } else {
                    1
                }
            }
            Composition::Weapons { .. } if acted => u.total_toe(),
            _ => 0,
        }
    }

    /// Whether a unit is currently deprived of Water (§52.5) — it went unsupplied on its last
    /// water check this Operations Stage.
    pub fn unit_water_deprived(&self, id: UnitId) -> bool {
        self.units.get(&id).is_some_and(|u| u.stages_without_water > 0)
    }

    /// Infantry TOE Strength Points the unit's attached first-line Truck Points can carry
    /// (§54.2 TOE-Inf capacity, summed over Light/Medium/Heavy).
    pub fn unit_truck_inf_capacity(&self, id: UnitId) -> u32 {
        let (Some(chart), Some(u)) = (self.reference.truck_chart.as_ref(), self.units.get(&id))
        else {
            return 0;
        };
        let t = &u.trucks;
        let tenths = t.light * chart.light.toe_inf_tenths.unwrap_or(0)
            + t.medium * chart.medium.toe_inf_tenths.unwrap_or(0)
            + t.heavy * chart.heavy.toe_inf_tenths.unwrap_or(0);
        tenths / 10
    }

    /// The effective CPA of an infantry unit fully motorized by its attached Truck Points
    /// (§6.17): the lowest Infantry-transport CPA among the truck types present. `None` unless
    /// the unit is leg infantry (Organic, basic CPA ≤ 10) whose full TOE the trucks can carry.
    pub fn unit_motorized_cpa(&self, id: UnitId) -> Option<i32> {
        let chart = self.reference.truck_chart.as_ref()?;
        let u = self.units.get(&id)?;
        if !matches!(u.composition, Composition::Organic { .. }) {
            return None;
        }
        if self.unit_cpa(id)? > 10 {
            return None; // §6.17: only infantry with a basic CPA of 10 or less
        }
        if self.unit_truck_inf_capacity(id) < u.total_toe() {
            return None;
        }
        let t = &u.trucks;
        let mut cpa: Option<i32> = None;
        for (n, ch) in [
            (t.light, &chart.light),
            (t.medium, &chart.medium),
            (t.heavy, &chart.heavy),
        ] {
            if n > 0 {
                if let Some(c) = ch.cpa_inf {
                    cpa = Some(cpa.map_or(c, |x| x.min(c)));
                }
            }
        }
        cpa
    }

    /// The CPA that governs a unit's movement this stage: the §6.17 motorized CPA (the truck's,
    /// when leg infantry is fully carried by attached Truck Points) if it applies, otherwise the
    /// unit's own basic CPA — plus the §31.4 Rommel +5 bonus for his designated unit.
    pub fn effective_cpa(&self, id: UnitId) -> Option<i32> {
        // §27.88: an SAS Brigade escorted by an LRDG moves as an LRDG with a CPA of 30.
        if self.is_sas(id) && self.sas_escorted_by_lrdg(id) {
            return Some(30 + self.rommel_cpa_bonus(id));
        }
        let base = self.unit_motorized_cpa(id).or_else(|| self.unit_cpa(id))?;
        Some(base + self.rommel_cpa_bonus(id))
    }

    /// §31.4: +5 CPA for the one unit Rommel is designated to accompany this Operations Stage —
    /// applies only while Rommel is present (not recalled, §31 recall), co-located with the unit,
    /// and has remained in the hex he began the stage in ("remains with that unit for the entire
    /// Operations Stage"). If Rommel moves away the bonus lapses.
    pub fn rommel_cpa_bonus(&self, id: UnitId) -> i32 {
        let Some(r) = self.rommel.as_ref() else {
            return 0;
        };
        if !self.rommel_on_map() || r.cpa_unit != Some(id) || r.hex != r.stage_start_hex {
            return 0;
        }
        if r.hex.is_some() && self.unit(id).and_then(|u| u.hex()) == r.hex {
            5
        } else {
            0
        }
    }

    /// Maximum Supply Points the unit's attached Truck Points could carry of each commodity, if
    /// every point carried only that one commodity (§54.2 cargo capacities). An upper bound —
    /// a given Truck Point carries only one commodity at a time.
    pub fn unit_truck_cargo_capacity(&self, id: UnitId) -> Commodities {
        let (Some(chart), Some(u)) = (self.reference.truck_chart.as_ref(), self.units.get(&id))
        else {
            return Commodities::default();
        };
        let t = &u.trucks;
        Commodities {
            fuel: t.light * chart.light.cargo_fuel
                + t.medium * chart.medium.cargo_fuel
                + t.heavy * chart.heavy.cargo_fuel,
            ammo: t.light * chart.light.cargo_ammo
                + t.medium * chart.medium.cargo_ammo
                + t.heavy * chart.heavy.cargo_ammo,
            stores: t.light * chart.light.cargo_stores
                + t.medium * chart.medium.cargo_stores
                + t.heavy * chart.heavy.cargo_stores,
            water: t.light * chart.light.cargo_water
                + t.medium * chart.medium.cargo_water
                + t.heavy * chart.heavy.cargo_water,
        }
    }

    /// Load a commodity onto a unit's Truck Points from friendly Supply Dumps in its hex
    /// (§53.24 — CP cost deferred). Bounded by the trucks' remaining cargo capacity for that
    /// commodity and the dump contents. Returns the amount actually loaded.
    pub fn load_trucks(&mut self, id: UnitId, c: Commodity, amount: u32) -> u32 {
        let cap = self.unit_truck_cargo_capacity(id).get(c);
        let (Some(hex), Some(side), cur) = (
            self.units.get(&id).and_then(|u| u.hex()),
            self.units.get(&id).map(|u| u.nationality.side()),
            self.units.get(&id).map_or(0, |u| u.truck_cargo.get(c)),
        ) else {
            return 0;
        };
        let mut remaining = amount.min(cap.saturating_sub(cur));
        let mut loaded = 0;
        for d in self.supply_dumps.iter_mut().filter(|d| {
            d.side == side && !d.dummy && d.location.hex() == Some(hex)
        }) {
            if remaining == 0 {
                break;
            }
            let take = remaining.min(d.supply.get(c));
            *d.supply.get_mut(c) -= take;
            remaining -= take;
            loaded += take;
        }
        if loaded > 0 {
            if let Some(u) = self.unit_mut(id) {
                *u.truck_cargo.get_mut(c) += loaded;
            }
        }
        loaded
    }

    /// Unload a commodity from a unit's Truck Points into a friendly Supply Dump in its hex
    /// (creating one if none is present). Returns the amount unloaded.
    pub fn unload_trucks(&mut self, id: UnitId, c: Commodity, amount: u32) -> u32 {
        let (Some(hex), Some(side), cur) = (
            self.units.get(&id).and_then(|u| u.hex()),
            self.units.get(&id).map(|u| u.nationality.side()),
            self.units.get(&id).map_or(0, |u| u.truck_cargo.get(c)),
        ) else {
            return 0;
        };
        let n = amount.min(cur);
        if n == 0 {
            return 0;
        }
        if let Some(u) = self.unit_mut(id) {
            *u.truck_cargo.get_mut(c) -= n;
        }
        if let Some(d) = self
            .supply_dumps
            .iter_mut()
            .find(|d| d.side == side && !d.dummy && d.location.hex() == Some(hex))
        {
            *d.supply.get_mut(c) += n;
        } else {
            let mut supply = Commodities::default();
            *supply.get_mut(c) = n;
            self.supply_dumps.push(SupplyDump {
                location: UnitLocation::OnMap(hex),
                side,
                supply,
                dummy: false,
            });
        }
        n
    }

    /// A unit's Breakdown Adjustment Rating (§21.13/14): for a weapons unit, the breakdown
    /// adjustment of a vehicle weapon it holds (tanks/SP/armored-recce); for infantry motorized
    /// by trucks, the All-Trucks rating (−2, "2 Left"). `None` if not subject to breakdown
    /// (leg infantry, towed guns).
    pub fn unit_bar(&self, id: UnitId) -> Option<i32> {
        let u = self.units.get(&id)?;
        match &u.composition {
            Composition::Weapons { groups } => groups.iter().find_map(|g| {
                self.reference
                    .weapons_systems
                    .get(&u.nationality)
                    .and_then(|m| m.get(&g.system))
                    .and_then(|w| w.breakdown_adjustment)
            }),
            Composition::Organic { .. } => {
                // §21.14: armored recce / armored car points are 1 Left (−1); all trucks 2 Left.
                if self.is_recce_unit(id) {
                    Some(-1)
                } else {
                    let t = &u.trucks;
                    if t.light + t.medium + t.heavy > 0 {
                        Some(-2)
                    } else {
                        None
                    }
                }
            }
        }
    }

    /// Apply a `pct` breakdown loss to a unit's vehicles (§21): reduce each weapon group's count
    /// (or, for truck-motorized infantry, its Truck Points) by that percentage.
    pub fn apply_breakdown_loss(&mut self, id: UnitId, pct: u32) {
        let recce = self.is_recce_unit(id);
        let Some(u) = self.unit_mut(id) else {
            return;
        };
        let mut broken: Vec<crate::state::unit::WeaponGroup> = Vec::new();
        match &mut u.composition {
            Composition::Weapons { groups } => {
                for g in groups.iter_mut() {
                    let loss = (g.count * pct + 50) / 100;
                    if loss > 0 {
                        g.count -= loss;
                        broken.push(crate::state::unit::WeaponGroup {
                            system: g.system.clone(),
                            count: loss,
                        });
                    }
                }
            }
            Composition::Organic { current_toe } => {
                // §21.11: a recce/armored-car unit's own TOE Points (armored cars) break down,
                // held recoverable for §22.24 field repair.
                if recce {
                    let loss = (*current_toe * pct + 50) / 100;
                    *current_toe -= loss;
                    u.toe_broken += loss;
                }
                let lb = (u.trucks.light * pct + 50) / 100;
                let mb = (u.trucks.medium * pct + 50) / 100;
                let hb = (u.trucks.heavy * pct + 50) / 100;
                u.trucks.light -= lb;
                u.trucks.medium -= mb;
                u.trucks.heavy -= hb;
                // Broken-down trucks are recoverable via field repair (§22.23).
                u.trucks_broken.light += lb;
                u.trucks_broken.medium += mb;
                u.trucks_broken.heavy += hb;
            }
        }
        // Broken-down weapon-system TOE is held recoverable (§22.8 repair), not deleted.
        for b in broken {
            if let Some(e) = u.weapons_broken.iter_mut().find(|e| e.system == b.system) {
                e.count += b.count;
            } else {
                u.weapons_broken.push(b);
            }
        }
    }

    /// §22.13d: whether a unit's hex has Rainstorm or Sandstorm weather, which forbids *field*
    /// repairs (Major Repair Facilities are exempt — handled separately in `facility_repair`).
    pub fn field_repair_blocked_by_weather(&self, id: UnitId) -> bool {
        self.units
            .get(&id)
            .and_then(|u| u.hex())
            .is_some_and(|h| matches!(self.weather_at(h), Weather::Rainstorm | Weather::Sandstorm))
    }

    /// Field-repair broken-down Truck Points in a unit's hex (§22.23): given a d6 `roll`, a 2
    /// repairs one Light or Medium point; a 1 repairs one Heavy, or two Light/Medium; other
    /// rolls repair none. Repaired points move from `trucks_broken` back to `trucks`. No supply
    /// cost. Returns the number of Truck Points repaired.
    pub fn field_repair_trucks(&mut self, id: UnitId, roll: u8) -> u32 {
        // §22.13d: no field repairs in a Rainstorm or Sandstorm hex (Major Facilities exempt).
        if self.field_repair_blocked_by_weather(id) {
            return 0;
        }
        let Some(u) = self.unit_mut(id) else {
            return 0;
        };
        let mut repaired = 0;
        let repair_light_or_medium = |u: &mut crate::state::unit::Unit, r: &mut u32| {
            if u.trucks_broken.medium > 0 {
                u.trucks_broken.medium -= 1;
                u.trucks.medium += 1;
                *r += 1;
            } else if u.trucks_broken.light > 0 {
                u.trucks_broken.light -= 1;
                u.trucks.light += 1;
                *r += 1;
            }
        };
        match roll {
            1 => {
                if u.trucks_broken.heavy > 0 {
                    u.trucks_broken.heavy -= 1;
                    u.trucks.heavy += 1;
                    repaired += 1;
                } else {
                    repair_light_or_medium(u, &mut repaired);
                    repair_light_or_medium(u, &mut repaired);
                }
            }
            2 => repair_light_or_medium(u, &mut repaired),
            _ => {}
        }
        repaired
    }

    /// Field-repair broken-down tank TOE Strength Points (§22.25/§22.8): given a modified die
    /// `roll`, repairs ⌈that %⌉ of each broken Tank-class system (a 10%* result on a single TOE
    /// is treated as 0%), moving them from `weapons_broken` back into the composition. Returns
    /// the TOE repaired. §22.26 Fuel cost is enforced: one Fuel Point per Tank TOE Point attempted
    /// is expended (from the unit's tanks, then a co-located dump) before rolling, capping the attempt.
    pub fn field_repair_tanks(&mut self, id: UnitId, roll: u32) -> u32 {
        // §22.13d: no field repairs in a Rainstorm or Sandstorm hex (Major Facilities exempt).
        if self.field_repair_blocked_by_weather(id) {
            return 0;
        }
        let Some(table) = self.reference.repair_table.clone() else {
            return 0;
        };
        let pct = table.tank_field_pct(roll);
        if pct == 0 {
            return 0;
        }
        let Some(u) = self.units.get(&id) else {
            return 0;
        };
        let nat = u.nationality;
        let hex = u.hex();
        let broken: Vec<(String, u32)> = u
            .weapons_broken
            .iter()
            .filter(|g| {
                self.reference
                    .weapons_systems
                    .get(&nat)
                    .and_then(|m| m.get(&g.system))
                    .map(|w| w.kind)
                    == Some(WeaponKind::Tank)
            })
            .map(|g| (g.system.clone(), g.count))
            .collect();
        let total_broken: u32 = broken.iter().map(|(_, c)| c).sum();
        if total_broken == 0 {
            return 0;
        }
        // §22.26: one Fuel Point must be present and expended (before rolling) for each Tank TOE
        // Point attempted; only those points may be repaired. Fuel comes from the unit's own tanks
        // and any Supply Dump in its hex. The number attempted is capped by available Fuel.
        let unit_fuel = u.fuel;
        let dump_fuel: u32 = self
            .supply_dumps
            .iter()
            .filter(|d| !d.dummy && Some(&d.location) == hex.map(UnitLocation::OnMap).as_ref())
            .map(|d| d.supply.fuel)
            .sum();
        let attempt = total_broken.min(unit_fuel + dump_fuel);
        if attempt == 0 {
            return 0;
        }
        // Expend the attempted Fuel: from the unit first, then dumps in the hex.
        let mut owed = attempt;
        if let Some(u) = self.unit_mut(id) {
            let take = u.fuel.min(owed);
            u.fuel -= take;
            owed -= take;
        }
        if owed > 0 {
            let target = hex.map(UnitLocation::OnMap);
            for d in self
                .supply_dumps
                .iter_mut()
                .filter(|d| !d.dummy && Some(&d.location) == target.as_ref())
            {
                let take = d.supply.fuel.min(owed);
                d.supply.fuel -= take;
                owed -= take;
                if owed == 0 {
                    break;
                }
            }
        }
        // Of the attempted points, ⌈attempt × pct%⌉ are repaired (a lone point needs >10%).
        let mut to_repair = if attempt == 1 && pct == 10 {
            0
        } else {
            (attempt * pct).div_ceil(100).min(attempt)
        };
        let mut total = 0;
        for (sys, cnt) in broken {
            if to_repair == 0 {
                break;
            }
            let repaired = to_repair.min(cnt);
            if let Some(u) = self.unit_mut(id) {
                if let Some(e) = u.weapons_broken.iter_mut().find(|g| g.system == sys) {
                    e.count -= repaired;
                }
                u.weapons_broken.retain(|g| g.count > 0);
                if let Composition::Weapons { groups } = &mut u.composition {
                    if let Some(g) = groups.iter_mut().find(|g| g.system == sys) {
                        g.count += repaired;
                    } else {
                        groups.push(crate::state::unit::WeaponGroup { system: sys, count: repaired });
                    }
                }
            }
            total += repaired;
            to_repair -= repaired;
        }
        total
    }

    /// Whether a unit is an armored-reconnaissance / armored-car unit (§21.11 / §22.24): its
    /// combat power and vehicles are its own Organic TOE (rated on the Unit Characteristics chart),
    /// and those TOE points are subject to Breakdown and field repair.
    pub fn is_recce_unit(&self, id: UnitId) -> bool {
        self.units
            .get(&id)
            .and_then(|u| {
                self.reference
                    .unit_characteristics
                    .get(&u.nationality)
                    .and_then(|m| m.get(&u.id_code))
            })
            .map(|c| c.unit_type.as_str())
            .is_some_and(|ut| ut.contains("Recce") || ut.contains("Reconnaissance"))
    }

    /// Field-repair one broken-down Recce / Armored-Car TOE Strength Point (§22.24): the player
    /// rolls one die for the recce/AC points in the hex and, on a **1**, recovers one broken Point
    /// (moved from `toe_broken` back to `current_toe`). No supply is expended. Blocked in a
    /// Rainstorm/Sandstorm hex (§22.13d). Returns the number of Points repaired (0 or 1).
    pub fn field_repair_recce(&mut self, id: UnitId, roll: u8) -> u32 {
        if self.field_repair_blocked_by_weather(id) {
            return 0;
        }
        if roll != 1 {
            return 0;
        }
        let Some(u) = self.units.get(&id) else {
            return 0;
        };
        if u.toe_broken == 0 {
            return 0;
        }
        if !self.is_recce_unit(id) {
            return 0;
        }
        if let Some(u) = self.unit_mut(id) {
            u.toe_broken -= 1;
            if let Composition::Organic { current_toe } = &mut u.composition {
                *current_toe += 1;
            }
        }
        1
    }

    /// Repair broken-down vehicles at a Repair Facility (§22.3 / §22.8): the unit must be in a
    /// hex with a Temporary or Major facility; a modified die `roll` gives that facility's repair
    /// percentage, applied (rounded up) to every broken Truck Point type and weapon-system group,
    /// returning them to service. Returns the number of Truck Points + TOE repaired.
    pub fn facility_repair(&mut self, id: UnitId, roll: u32) -> u32 {
        let Some(table) = self.reference.repair_table.clone() else {
            return 0;
        };
        let Some(hex) = self.units.get(&id).and_then(|u| u.hex()) else {
            return 0;
        };
        let pct = match self.repair_facilities.get(&hex) {
            Some(FacilityKind::Temporary) => table.temp_facility_pct(roll),
            Some(FacilityKind::Major) => table.major_facility_pct(roll),
            None => return 0,
        };
        if pct == 0 {
            return 0;
        }
        let ceil_pct = |n: u32| (n * pct).div_ceil(100).min(n);
        let mut total = 0;
        if let Some(u) = self.unit_mut(id) {
            for (broken, active) in [
                (u.trucks_broken.light, &mut u.trucks.light),
                (u.trucks_broken.medium, &mut u.trucks.medium),
                (u.trucks_broken.heavy, &mut u.trucks.heavy),
            ] {
                let r = ceil_pct(broken);
                *active += r;
                total += r;
            }
            u.trucks_broken.light -= ceil_pct(u.trucks_broken.light);
            u.trucks_broken.medium -= ceil_pct(u.trucks_broken.medium);
            u.trucks_broken.heavy -= ceil_pct(u.trucks_broken.heavy);

            let broken_groups: Vec<(String, u32)> =
                u.weapons_broken.iter().map(|g| (g.system.clone(), g.count)).collect();
            for (sys, cnt) in broken_groups {
                let r = ceil_pct(cnt);
                if r == 0 {
                    continue;
                }
                if let Some(e) = u.weapons_broken.iter_mut().find(|g| g.system == sys) {
                    e.count -= r;
                }
                if let Composition::Weapons { groups } = &mut u.composition {
                    if let Some(g) = groups.iter_mut().find(|g| g.system == sys) {
                        g.count += r;
                    } else {
                        groups.push(crate::state::unit::WeaponGroup { system: sys, count: r });
                    }
                }
                total += r;
            }
            u.weapons_broken.retain(|g| g.count > 0);
        }
        total
    }

    /// Operations Stages of Training a Replacement Point class requires before absorption
    /// (§20.43): Gun 1, Infantry 3, Tank/Recce 6.
    pub fn replacement_training_stages(class: ReplacementClass) -> u32 {
        match class {
            ReplacementClass::Gun => 1,
            ReplacementClass::Infantry => 3,
            ReplacementClass::Tank | ReplacementClass::Recce => 6,
        }
    }

    /// The Replacement-Point class a weapon `system` (of `nat`) rebuilds: Tank→Tank,
    /// Reconnaissance→Recce, all gun kinds (artillery/anti-tank/anti-air)→Gun. Unknown → Gun.
    pub fn replacement_class_for_system(&self, nat: Nationality, system: &str) -> ReplacementClass {
        use crate::reference::units::WeaponKind;
        match self
            .reference
            .weapons_systems
            .get(&nat)
            .and_then(|m| m.get(system))
            .map(|w| w.kind)
        {
            Some(WeaponKind::Tank) => ReplacementClass::Tank,
            Some(WeaponKind::Reconnaissance) => ReplacementClass::Recce,
            _ => ReplacementClass::Gun,
        }
    }

    /// Add a pool of Replacement Points (§20.2), starting its Training clock (§20.43). Returns
    /// its index in `replacement_points`.
    pub fn add_replacement_points(
        &mut self,
        class: ReplacementClass,
        count: u32,
        side: Side,
        location: Option<UnitLocation>,
        assigned_to: Option<UnitId>,
    ) -> usize {
        let training_remaining = Self::replacement_training_stages(class);
        self.replacement_points.push(ReplacementPoints {
            class,
            count,
            side,
            location,
            training_remaining,
            assigned_to,
        });
        self.replacement_points.len() - 1
    }

    /// Plan a Replacement Point pool to arrive at a future Operations Stage (§20.21). It enters
    /// the pool (starting Training) when that Naval Convoy Arrival Phase is reached. Returns the
    /// index of the queued pending replacement (so a caller may set its §20.62 `tonnage`).
    #[allow(clippy::too_many_arguments)]
    pub fn plan_replacement(
        &mut self,
        class: ReplacementClass,
        count: u32,
        side: Side,
        game_turn: u32,
        stage: u32,
        location: Option<UnitLocation>,
        assigned_to: Option<UnitId>,
    ) -> usize {
        self.pending_replacements.push(PendingReplacement {
            class,
            count,
            side,
            game_turn,
            stage,
            location,
            assigned_to,
            tonnage: 0,
        });
        self.pending_replacements.len() - 1
    }

    /// Bring in any planned Replacement Points now due (§20.21) — called at the Naval Convoy
    /// Arrival Phase. Each due plan enters the Replacement Point pool (beginning Training).
    pub fn process_replacement_arrivals(&mut self) {
        let now = (self.game_turn, self.operations_stage as u32);
        let mut due = Vec::new();
        let mut i = 0;
        while i < self.pending_replacements.len() {
            let p = &self.pending_replacements[i];
            if (p.game_turn, p.stage) <= now {
                due.push(self.pending_replacements.remove(i));
            } else {
                i += 1;
            }
        }
        for p in due {
            self.add_replacement_points(p.class, p.count, p.side, p.location, p.assigned_to);
        }
    }

    /// §20.62/§20.64: before Axis Replacement production arrives this Game-Turn, cap it at the
    /// turn's **actual** rolled Shipping Tonnage allowance (§56.21). Replacement Points have priority
    /// over supplies (§20.64), but they cannot exceed the roll itself — production planned two turns
    /// out (§20.63) against a hoped-for tonnage that the actual roll does not cover is deferred to the
    /// next Game-Turn (kept, not lost). The turn's allowance is the roll fixed when convoys were
    /// planned; if none was (no convoy this turn), one is rolled now. With no Convoy Level Chart
    /// loaded there is no cap (the gate is skipped, as elsewhere). Deferral is greedy in queue order.
    pub fn defer_over_shipped_axis_replacements(&mut self) {
        let now = self.game_turn;
        let allowance = match self.axis_shipping_rolled.get(&now).copied() {
            Some(a) => a,
            None => {
                if self.reference.convoy_levels.is_none() {
                    return; // no chart → no tonnage model (unchanged behaviour)
                }
                let die = self.rng.d6() as u32;
                let a = self.allowable_convoy_tonnage(now, die).unwrap_or(u32::MAX);
                self.axis_shipping_rolled.insert(now, a);
                a
            }
        };
        let mut running = 0u32;
        for p in self.pending_replacements.iter_mut() {
            if p.tonnage == 0 || p.game_turn != now {
                continue; // Commonwealth (off-convoy) or not arriving this turn
            }
            if running + p.tonnage <= allowance {
                running += p.tonnage;
            } else {
                p.game_turn = now + 1; // §20.62: defer the excess to next turn's allowance
            }
        }
    }

    /// Axis Naval Convoy tons to ship `count` Truck Points of `kind` (§20.66a row tonnage per
    /// point); 0 if the Axis Truck Production Chart is not loaded.
    pub fn axis_truck_tonnage(&self, kind: crate::reference::logistics::TruckKind, count: u32) -> u32 {
        self.reference
            .axis_truck_production
            .as_ref()
            .map(|c| c.row(kind).tonnage * count)
            .unwrap_or(0)
    }

    /// §20.62: total Axis convoy Shipping Tonnage already committed to arrive on `arrival_gt` —
    /// replacement-production tonnage (arrives that turn), Axis planned-truck tonnage (planned two
    /// turns earlier, §20.66a), and supply tonnage booked on convoys arriving that turn. Replacement
    /// Points and Truck Points share the same per-Game-Turn allowance, so all three compete.
    pub fn axis_shipping_committed(&self, arrival_gt: u32) -> u32 {
        use crate::reference::units::Side;
        let replacement: u32 = self
            .production_orders
            .iter()
            .filter(|o| o.nationality.side() == Side::Axis && o.arrival_game_turn == arrival_gt)
            .map(|o| o.tonnage)
            .sum();
        let trucks: u32 = self
            .axis_truck_orders
            .iter()
            .filter(|o| o.side == Side::Axis && o.game_turn + 2 == arrival_gt)
            .map(|o| self.axis_truck_tonnage(o.kind, o.count))
            .sum();
        // Supply tonnage (fuel/ammo/stores) on convoys arriving that turn; replacement_tons is
        // excluded — that lift is tracked via the production/truck orders above.
        let supply: u32 = self
            .convoys
            .iter()
            .filter(|c| !c.cancelled && c.arrival_gt == arrival_gt)
            .map(|c| c.fuel_tons + c.ammo_tons + c.stores_tons)
            .sum();
        replacement + trucks + supply
    }

    /// Schedule `count` Truck Points of `kind` for `side` to arrive at the Naval Convoy Arrival
    /// Phase of `game_turn`/`stage` (§20.78A CW +4 GT; §20.66a Axis +2 GT).
    pub fn schedule_truck_delivery(
        &mut self,
        side: Side,
        kind: crate::reference::logistics::TruckKind,
        count: u32,
        game_turn: u32,
        stage: u32,
    ) {
        if count == 0 {
            return;
        }
        self.pending_trucks.push(PendingTruckDelivery { side, kind, count, game_turn, stage });
    }

    /// Bring in any produced Truck Points now due (§20.78A / §20.66a) — called at the Naval Convoy
    /// Arrival Phase. Each due delivery adds to its side's free-truck pool.
    pub fn process_truck_arrivals(&mut self) {
        use crate::reference::logistics::TruckKind;
        let now = (self.game_turn, self.operations_stage as u32);
        let mut i = 0;
        while i < self.pending_trucks.len() {
            if (self.pending_trucks[i].game_turn, self.pending_trucks[i].stage) <= now {
                let d = self.pending_trucks.remove(i);
                let pool = self.truck_pools.entry(d.side).or_default();
                match d.kind {
                    TruckKind::Light => pool.light += d.count,
                    TruckKind::Medium => pool.medium += d.count,
                    TruckKind::Heavy => pool.heavy += d.count,
                }
            } else {
                i += 1;
            }
        }
    }

    /// Add TOE Strength Points to a unit, capped at its `max_toe` (§4.44) for Organic units.
    /// Weapons units add to their first group. Returns the amount actually added.
    fn add_toe(&mut self, id: UnitId, count: u32) -> u32 {
        let cap = self.units.get(&id).and_then(|u| {
            self.reference
                .unit_characteristics
                .get(&u.nationality)
                .and_then(|m| m.get(&u.id_code))
                .and_then(|uc| uc.max_toe.as_ref())
                .map(|m| m.value)
        });
        let Some(u) = self.unit_mut(id) else {
            return 0;
        };
        match &mut u.composition {
            Composition::Organic { current_toe } => {
                let add = match cap {
                    Some(c) => count.min(c.saturating_sub(*current_toe)),
                    None => count,
                };
                *current_toe += add;
                add
            }
            Composition::Weapons { groups } => {
                if let Some(g) = groups.first_mut() {
                    g.count += count;
                    count
                } else {
                    0
                }
            }
        }
    }

    /// Absorb a trained Replacement Point pool (§20.49) into a co-located unit, restoring its
    /// TOE. Fails (returns `None`) if the points are still training, are assigned to a different
    /// unit, or are not in the same hex. Returns the TOE Strength Points added.
    pub fn absorb_replacement(&mut self, index: usize, unit: UnitId) -> Option<u32> {
        let rp = self.replacement_points.get(index)?.clone();
        if rp.training_remaining > 0 {
            return None;
        }
        if matches!(rp.assigned_to, Some(a) if a != unit) {
            return None;
        }
        let unit_hex = self.units.get(&unit)?.hex()?;
        if rp.location.and_then(|l| l.hex()) != Some(unit_hex) {
            return None;
        }
        let side = self.units.get(&unit)?.nationality.side();

        // Legacy 1:1 absorption when no §20.3 Conversion Chart is loaded (one pool point = one TOE).
        let Some(conv) = self.reference.replacement_conversion.clone() else {
            let added = self.add_toe(unit, rp.count);
            *self.replacements_used.entry(side).or_insert(0) += rp.count;
            self.replacement_points.remove(index);
            return Some(added);
        };
        let row = self.conversion_row_for_unit(unit);
        let Some(entry) = conv.cost(row).cloned() else {
            // Unmapped type → fall back to 1:1.
            let added = self.add_toe(unit, rp.count);
            *self.replacements_used.entry(side).or_insert(0) += rp.count;
            self.replacement_points.remove(index);
            return Some(added);
        };

        // §20.3 cost-based absorption: rebuild TOE Strength Points, each costing one affordable
        // `options` set drawn from co-located, trained, assignable Replacement-Point pools (plus a
        // Stores Point when the entry is `supplies`), until the unit is full or nothing is affordable.
        let mut room = self.unit_toe_headroom(unit);
        let mut added = 0u32;
        let mut consumed = 0u32;
        while room > 0 {
            // Choose the first affordable option (empty options list = free rebuild).
            let cost = if entry.options.is_empty() {
                Some(std::collections::HashMap::new())
            } else {
                entry.options.iter().find_map(|opt| {
                    let mut need: std::collections::HashMap<ReplacementClass, u32> = std::collections::HashMap::new();
                    for req in opt {
                        *need.entry(Self::conversion_class_for_kind(&req.kind)).or_insert(0) += req.count;
                    }
                    need.iter()
                        .all(|(&c, &n)| self.available_replacement_class(c, unit, unit_hex) >= n)
                        .then_some(need)
                })
            };
            let Some(cost) = cost else { break };
            if entry.supplies && self.available_commodity_at(unit_hex, side, Commodity::Stores) < 1 {
                break;
            }
            // Commit: draw the points, spend Stores, and rebuild one TOE Strength Point.
            for (&class, &n) in &cost {
                self.draw_replacement_class(class, unit, unit_hex, n);
                consumed += n;
            }
            if entry.supplies {
                self.consume_commodity_at(unit_hex, side, Commodity::Stores, 1);
            }
            if self.add_toe(unit, 1) == 0 {
                break;
            }
            added += 1;
            room -= 1;
        }
        *self.replacements_used.entry(side).or_insert(0) += consumed;
        self.replacement_points.retain(|p| p.count > 0);
        Some(added)
    }

    /// TOE Strength Points a unit may still gain before hitting its §4.44 Max-TOE (`u32::MAX` if the
    /// unit has no listed maximum).
    fn unit_toe_headroom(&self, id: UnitId) -> u32 {
        let Some(u) = self.units.get(&id) else { return 0 };
        let cur = u.total_toe();
        match self
            .reference
            .unit_characteristics
            .get(&u.nationality)
            .and_then(|m| m.get(&u.id_code))
            .and_then(|c| c.max_toe.as_ref())
        {
            Some(m) => m.value.saturating_sub(cur),
            None => u32::MAX,
        }
    }

    /// Replacement Points of `class` available to rebuild `unit` at `hex`: those in co-located,
    /// trained pools assigned to this unit or unassigned (§20.41/§20.43).
    fn available_replacement_class(&self, class: ReplacementClass, unit: UnitId, hex: HexCoord) -> u32 {
        self.replacement_points
            .iter()
            .filter(|p| {
                p.class == class
                    && p.training_remaining == 0
                    && p.location.as_ref().and_then(|l| l.hex()) == Some(hex)
                    && (p.assigned_to.is_none() || p.assigned_to == Some(unit))
            })
            .map(|p| p.count)
            .sum()
    }

    /// Remove `n` Replacement Points of `class` for `unit` at `hex` (pools assigned to this unit
    /// first, then unassigned).
    fn draw_replacement_class(&mut self, class: ReplacementClass, unit: UnitId, hex: HexCoord, n: u32) {
        let mut owed = n;
        for assigned_first in [true, false] {
            for p in self.replacement_points.iter_mut().filter(|p| {
                p.class == class
                    && p.training_remaining == 0
                    && p.location.as_ref().and_then(|l| l.hex()) == Some(hex)
                    && if assigned_first { p.assigned_to == Some(unit) } else { p.assigned_to.is_none() }
            }) {
                if owed == 0 {
                    break;
                }
                let take = owed.min(p.count);
                p.count -= take;
                owed -= take;
            }
        }
    }

    /// §20.3 classifier: map a unit's §4.46 `unit_type` label (and name) to a Replacement-Point
    /// Conversion Chart row. The §20.3 footnotes drive the name-based calls — Commando (Layforce/
    /// SAS), Paratroop (Folgore/Ramcke), Engineer (incl. the Australian Pioneer), Bersaglieri, and
    /// the three recce rows. Machinegun (English "Machine Gun/MG" and German "Machinegewehr") and
    /// Heavy-Weapons ("Heavy Wpns") battalions are detected by name — verified against the §4.46
    /// counter glyphs transcribed from PDF p182–185 (`*-counters-4.46.md`), which confirmed every
    /// MG/HW unit in the three OBs is named accordingly. Residual (cf. D14): a plain (non-armored)
    /// recce battalion still defaults to the Armored-Reconnaissance row, and a few genuinely
    /// out-of-legend counter glyphs (Italian divisional AA/AT support companies, coastal-defence
    /// counters) are recorded with `?` in the transcription docs — none change the current OBs' rows.
    pub fn conversion_row_for_unit(&self, id: UnitId) -> &'static str {
        let Some(u) = self.units.get(&id) else { return "Any other Infantry" };
        let ut = self
            .reference
            .unit_characteristics
            .get(&u.nationality)
            .and_then(|m| m.get(&u.id_code))
            .map(|c| c.unit_type.as_str())
            .unwrap_or("");
        let name = u.name.as_str();
        if ut.contains("Headquarters") {
            "Any Headquarters Unit"
        } else if ut.contains("Construction") {
            "Road/Railroad Construction"
        } else if ut.contains("Engineer") || name.contains("Pioneer") {
            // §20.3 footnote e: the Engineer Battalion row includes the Australian Pioneer unit.
            if ut.contains("Coy") && !ut.contains("Bn") { "Engineer Company" } else { "Engineer Battalion" }
        } else if ut.contains("Tank") {
            "Tank"
        } else if ut.contains("Artillery") {
            "Artillery"
        } else if ut.contains("Anti-tank") {
            "Anti-tank"
        } else if ut.contains("Anti-air") {
            "Anti-air"
        } else if ut.contains("Reconnaissance") || ut.contains("Recce") {
            // §20.3 recce rows, resolved from the §4.46 counter symbols (the nationality legends plus
            // the Commonwealth counter sheets, PDF p184–185) — NOT composition: real recce units are
            // all Organic, so "armored" is the §4.44 Armor-Protection rating on the characteristics,
            // and the Armored-Car vs Armored-Reconnaissance split is the counter glyph:
            //  • Commonwealth — every recce unit is armored; Armored Car (oval-on-wheels) for the
            //    units read as such below, otherwise Armored Reconnaissance (oval+slash).
            //  • Italian — armored recce (Autoblinda `vv`/`ww`) is Armored Car (no armored-recce
            //    glyph in the Italian legend); unarmored (camel/desert-patrol/Bersaglieri-MG) is the
            //    light Motorcycle/Camel/Reconnaissance row.
            //  • German — armored recce (Aufklärungs `ee`/`ff`) is Armored Reconnaissance (no
            //    armored-car glyph in the German legend).
            let armored = self
                .reference
                .unit_characteristics
                .get(&u.nationality)
                .and_then(|m| m.get(&u.id_code))
                .and_then(|c| c.armor_protection)
                .map(|r| r.value)
                .unwrap_or(0)
                > 0;
            // Commonwealth armored-car counters (oval-on-wheels), read from p184–185. 11th Hussars is
            // the one CW recce whose counter wasn't located; it defaults to Armored Reconnaissance.
            let cw_armored_car = name.contains("Armored Car")
                || name.contains("Armoured Car")
                || name.contains("Derbyshire Yeomanry")
                || name.contains("Royal Dragoons")
                || name.contains("DCO Lancers")
                || name.contains("QVOC Guides")
                || name.contains("Flying Column");
            if !armored {
                "Motorcycle/Camel/Reconnaissance"
            } else if cw_armored_car {
                "Armored Car"
            } else if u.nationality == Nationality::Commonwealth {
                "Armored Reconnaissance"
            } else if u.nationality == Nationality::Italian {
                "Armored Car"
            } else {
                "Armored Reconnaissance"
            }
        } else if ut.contains("Infantry") {
            // §20.3 footnotes a/b/c/d: special-infantry rows keyed off the unit's identity. Detected
            // by name where it says so; Machinegun/Heavy-Weapons units not named as such fall to the
            // generic row (needs the §4.46 counter symbols to resolve fully).
            if name.contains("Bersaglieri") {
                "Italian Bersaglieri Infantry"
            } else if name.contains("Commando") || name.contains("Layforce") || name.contains("SAS") {
                "Commando"
            } else if name.contains("Folgore") || name.contains("Ramcke") || name.contains("Para") {
                "Paratroop Infantry"
            } else if name.contains("Machine Gun")
                || name.contains("Machinegun")
                || name.contains("Machinegewehr") // German MG battalions (§4.46 MG glyph, PDF p183)
                || name.contains("MG")
            {
                "Machinegun"
            } else if name.contains("Heavy Weapons") || name.contains("Heavy Wpns") {
                "Heavy Weapons"
            } else {
                "Any other Infantry"
            }
        } else {
            "Any other Infantry"
        }
    }

    /// Collapse a §20.3 requirement `kind` onto one of the four Replacement-Point pool classes:
    /// Inf → Infantry; Tank/Lt Tank → Tank; ArmR → Recce; all gun/artillery/AT/AA kinds → Gun.
    /// Confirmed by the §20.3 chart key: "Gun = Artillery, anti-tank or anti-air, whatever its
    /// ratings", ArmR = Armored Recce/Car, Lt Tank = a §20.5 light tank, Lt AT = an anti-tank gun.
    fn conversion_class_for_kind(kind: &str) -> ReplacementClass {
        match kind {
            "Inf" => ReplacementClass::Infantry,
            "Tank" | "Lt Tank" => ReplacementClass::Tank,
            "ArmR" => ReplacementClass::Recce,
            _ => ReplacementClass::Gun,
        }
    }

    /// Check a unit for breakdown on ceasing movement (§21.24–38): if it is subject and has more
    /// than 3 accumulated Breakdown Points in a higher column than last checked (§21.26/27), roll
    /// on the §21.38 table with its BAR + Hot/Sandstorm shifts and break down that percentage of
    /// its vehicles. Returns the % broken down (0 if no check / no table).
    pub fn resolve_breakdown(&mut self, id: UnitId) -> u32 {
        let Some(table) = self.reference.breakdown_table.clone() else {
            return 0;
        };
        let Some(bar) = self.unit_bar(id) else {
            return 0;
        };
        let bp = (self.units.get(&id).map_or(0, |u| u.breakdown_points) / 10).max(0) as u32;
        if bp <= 3 {
            return 0;
        }
        let base_col = table.column_index(bp) as i32;
        if base_col <= self.units.get(&id).map_or(-1, |u| u.breakdown_checked_col) {
            return 0; // §21.26: recheck only on entering a higher column
        }
        let mut shift = bar;
        if let Some(h) = self.units.get(&id).and_then(|u| u.hex()) {
            if self.weather_at(h) == Weather::Hot {
                shift += 1;
            }
            if self.sandstorm_at(h) {
                shift += 1;
            }
        }
        let roll = self.rng.sequential_read() as u32;
        let pct = table.percent(bp, shift, roll);
        if let Some(u) = self.unit_mut(id) {
            u.breakdown_checked_col = base_col;
        }
        if pct > 0 {
            self.apply_breakdown_loss(id, pct);
        }
        pct
    }

    /// The water source at a hex (§52.1): a Major City (a §64.73 victory-city hex) or an Oasis
    /// gives unlimited water; a Village ("Town") or Bir well gives a §52.7 draw and can deplete.
    pub fn hex_water_source(&self, hex: HexCoord) -> Option<WaterKind> {
        let feats = self.map.features(hex);
        if feats.contains(&Feature::Oasis) {
            return Some(WaterKind::Unlimited);
        }
        let is_major_city = self.reference.campaign_victory.as_ref().is_some_and(|cv| {
            cv.cities
                .iter()
                .any(|c| HexId::parse(&c.hex).map(|h| h.to_hex_coord()) == Some(hex))
        });
        if is_major_city {
            return Some(WaterKind::Unlimited);
        }
        // §52.23: a pipeline connected back to a major city is an unlimited source.
        if self.pipelines.contains(&hex) && self.pipeline_connected(hex) {
            return Some(WaterKind::Unlimited);
        }
        if feats.contains(&Feature::Village) {
            return Some(WaterKind::Village);
        }
        if feats.contains(&Feature::Bir) {
            return Some(WaterKind::Bir);
        }
        None
    }

    /// Whether `hex` is a built ("operating") railroad hex — it touches at least one `Railroad`
    /// hexside (§52.22/§54.33). Unbuilt track (`UnfinishedRailroad`) does not count (§24.67).
    pub fn hex_is_railroad(&self, hex: HexCoord) -> bool {
        self.map.neighbors(hex).any(|n| {
            self.map
                .hexsides_on(hex, n)
                .iter()
                .any(|h| h.kind == HexsideKind::Railroad)
        })
    }

    /// §8.72/8.73: whether `to` is reachable from `from` along contiguous **built** Railroad
    /// hexsides without passing through a hex in an enemy ZOC or occupied by an enemy unit.
    pub fn rail_reachable(&self, from: HexCoord, to: HexCoord, side: Side) -> bool {
        if from == to {
            return true;
        }
        let enemy = side.opponent();
        let mut seen = std::collections::HashSet::new();
        let mut stack = vec![from];
        while let Some(h) = stack.pop() {
            if !seen.insert(h) {
                continue;
            }
            for n in self.map.neighbors(h) {
                if !self
                    .map
                    .hexsides_on(h, n)
                    .iter()
                    .any(|hs| hs.kind == HexsideKind::Railroad)
                {
                    continue;
                }
                // §8.73: never enter an enemy ZOC or enemy-occupied hex while railing.
                let enemy_occupied = self
                    .units
                    .values()
                    .any(|u| u.hex() == Some(n) && u.nationality.side() == enemy);
                if self.hex_in_zoc_of(n, enemy) || enemy_occupied {
                    continue;
                }
                if n == to {
                    return true;
                }
                if !seen.contains(&n) {
                    stack.push(n);
                }
            }
        }
        false
    }

    /// §8.72–8.75 / §54.4: Rail Movement. Move `unit` any distance along the finished rail line to
    /// `to`. The unit starts on a rail hex having expended no CP this stage (§8.73), not in an enemy
    /// ZOC, carries ≤ 2 Stacking Points (§8.75), and `to` is a rail hex reachable along built track
    /// avoiding enemy ZOC/occupied hexes. For the **Commonwealth** (the line's owner) only one stack
    /// may rail in each direction per stage (§8.74). The **Axis** may rail only under §54.4: it must
    /// hold five contiguous controlled rail hexes (§54.41), `from` must be Axis-controlled, and it
    /// spends 900 tons of activated Rolling Stock per Stacking Point (§54.44) from its one-direction
    /// per-stage haulage budget (§54.43/46). Returns whether the unit was railed.
    pub fn rail_move(&mut self, unit: UnitId, to: HexCoord) -> bool {
        if self.rail_maintenance_this_stage {
            return false; // §54.34
        }
        let Some(u) = self.units.get(&unit) else {
            return false;
        };
        let side = u.nationality.side();
        let Some(from) = u.hex() else {
            return false;
        };
        if u.cp_expended.0 != 0 || !self.hex_is_railroad(from) || !self.hex_is_railroad(to) {
            return false;
        }
        if self.hex_in_zoc_of(from, side.opponent()) {
            return false;
        }
        let sp = self.effective_stacking_points(unit);
        if sp > 2 {
            return false;
        }
        if !self.rail_reachable(from, to, side) {
            return false;
        }
        let west = to.col < from.col;
        match side {
            Side::Commonwealth => {
                // §8.74: one stack each direction per stage.
                if (west && self.rail_west_used) || (!west && self.rail_east_used) {
                    return false;
                }
                if west {
                    self.rail_west_used = true;
                } else {
                    self.rail_east_used = true;
                }
            }
            Side::Axis => {
                // §54.41: must control `from`; §54.44: 900 tons/Stacking Point from the activated
                // one-direction haulage budget (§54.43/46). A 0-SP shell still needs live stock.
                if !self.axis_controls_rail(from) {
                    return false;
                }
                let tons = 900 * sp;
                if !self.reserve_axis_rail_tons(tons, west) {
                    return false;
                }
            }
        }
        if let Some(u) = self.unit_mut(unit) {
            u.location = Some(crate::state::unit::UnitLocation::OnMap(to));
        }
        // §54.41: the mover now controls its destination rail hex.
        self.note_rail_control(to, unit);
        true
    }

    /// §54.3: Commonwealth rail transport of supplies. Move up to `points` of one commodity from a
    /// Commonwealth Supply Dump at `from` to `to` along the finished line. Capacity is 1500 tons
    /// per direction (§54.32) — i.e. 1500 Stores (1 t), 375 Ammo (4 t), or 12000 Fuel (⅛ t) points;
    /// Water is not railed (§54.33, it is a pipeline in itself). One commodity type per shipment
    /// (§54.33), sharing the once-per-direction allowance with unit Rail Movement (§54.32: a
    /// direction carries personnel *or* supplies). Returns the number of points delivered.
    pub fn rail_move_supply(&mut self, from: HexCoord, to: HexCoord, cargo: RailCargo, points: u32) -> u32 {
        self.rail_move_supply_for(Side::Commonwealth, from, to, cargo, points)
    }

    /// §54.3 / §54.43: rail transport of supplies for `side`. Commonwealth uses its 1500-ton
    /// per-direction allowance (§54.32) and the once-each-direction stack flag (§8.74). The Axis
    /// hauls against its activated Rolling Stock (§54.43): the per-shipment cap is whatever of its
    /// one-direction haulage budget remains, converted to points by §54.5 weight (Stores 1 t, Ammo
    /// 4 t, Fuel ⅛ t), and it must hold five contiguous controlled rail hexes with `from` controlled.
    /// Water is never railed (§54.33). Returns the points delivered.
    pub fn rail_move_supply_for(&mut self, side: Side, from: HexCoord, to: HexCoord, cargo: RailCargo, points: u32) -> u32 {
        if self.rail_maintenance_this_stage {
            return 0; // §54.34
        }
        if from == to || !self.hex_is_railroad(from) || !self.hex_is_railroad(to) {
            return 0;
        }
        if !self.rail_reachable(from, to, side) {
            return 0;
        }
        let west = to.col < from.col;
        // Per-direction capacity: CW's flat 1500-ton allowance vs the Axis Rolling-Stock budget.
        let cap = match side {
            Side::Commonwealth => {
                if (west && self.rail_west_used) || (!west && self.rail_east_used) {
                    return 0;
                }
                match cargo {
                    RailCargo::Stores => 1500,
                    RailCargo::Ammo => 375,
                    RailCargo::Fuel => 12000,
                }
            }
            Side::Axis => {
                if !self.axis_controls_rail(from) || self.axis_rail_contiguous_max() < 5 {
                    return 0;
                }
                if self.axis_rolling_stock_tons == 0 {
                    return 0;
                }
                if let Some(dir) = self.axis_rail_dir {
                    if dir != west {
                        return 0; // §54.43: one direction per stage
                    }
                }
                let remaining = self.axis_rolling_stock_tons.saturating_sub(self.axis_rail_tons_used);
                // §54.5 weights → how many points of this commodity `remaining` tons can carry.
                match cargo {
                    RailCargo::Stores => remaining,
                    RailCargo::Ammo => remaining / 4,
                    RailCargo::Fuel => remaining * 8,
                }
            }
        };
        let target_from = crate::state::unit::UnitLocation::OnMap(from);
        let get = |s: &Commodities| match cargo {
            RailCargo::Fuel => s.fuel,
            RailCargo::Ammo => s.ammo,
            RailCargo::Stores => s.stores,
        };
        let avail: u32 = self
            .supply_dumps
            .iter()
            .filter(|d| !d.dummy && d.location == target_from && d.side == side)
            .map(|d| get(&d.supply))
            .sum();
        let moved = points.min(cap).min(avail);
        if moved == 0 {
            return 0;
        }
        // §54.43: charge the Axis Rolling-Stock budget for the tons actually hauled.
        if side == Side::Axis {
            let tons = match cargo {
                RailCargo::Stores => moved,
                RailCargo::Ammo => moved * 4,
                RailCargo::Fuel => moved.div_ceil(8),
            };
            self.axis_rail_tons_used += tons;
            self.axis_rail_dir = Some(west);
        }
        // Draw from the source dumps.
        let mut owed = moved;
        for d in self.supply_dumps.iter_mut().filter(|d| {
            !d.dummy && d.location == target_from && d.side == side
        }) {
            let field = match cargo {
                RailCargo::Fuel => &mut d.supply.fuel,
                RailCargo::Ammo => &mut d.supply.ammo,
                RailCargo::Stores => &mut d.supply.stores,
            };
            let take = (*field).min(owed);
            *field -= take;
            owed -= take;
            if owed == 0 {
                break;
            }
        }
        // Deliver into a dump at `to` (creating one for `side` if none exists there).
        let target_to = crate::state::unit::UnitLocation::OnMap(to);
        let dest = self
            .supply_dumps
            .iter_mut()
            .find(|d| !d.dummy && d.location == target_to && d.side == side);
        match dest {
            Some(d) => match cargo {
                RailCargo::Fuel => d.supply.fuel += moved,
                RailCargo::Ammo => d.supply.ammo += moved,
                RailCargo::Stores => d.supply.stores += moved,
            },
            None => {
                let mut supply = Commodities::default();
                match cargo {
                    RailCargo::Fuel => supply.fuel = moved,
                    RailCargo::Ammo => supply.ammo = moved,
                    RailCargo::Stores => supply.stores = moved,
                }
                self.supply_dumps.push(SupplyDump {
                    location: target_to,
                    side,
                    supply,
                    dummy: false,
                });
            }
        }
        // §8.74: mark the Commonwealth's once-each-direction allowance (the Axis budget was
        // already charged above).
        if side == Side::Commonwealth {
            if west {
                self.rail_west_used = true;
            } else {
                self.rail_east_used = true;
            }
        }
        moved
    }

    // ----- §54.4 Axis use of the Commonwealth railroad ------------------------------------------

    /// §54.41: record rail-hex control. The last Side to move a land **combat** unit through a rail
    /// hex controls it; non-combat HQs and non-rail hexes are ignored. Called from the move
    /// resolvers when a unit's new hex is a railroad hex.
    pub(crate) fn note_rail_control(&mut self, hex: HexCoord, unit: UnitId) {
        if !self.hex_is_railroad(hex) || self.is_noncombat_hq(unit) {
            return;
        }
        if let Some(side) = self.units.get(&unit).map(|u| u.nationality.side()) {
            self.rail_control.insert(hex, side);
        }
    }

    /// §54.41: whether the Axis currently controls `hex` (a rail hex it was last to pass a combat
    /// unit through). Hexes never contested are Commonwealth-owned, so absence ⇒ not Axis-controlled.
    pub fn axis_controls_rail(&self, hex: HexCoord) -> bool {
        self.rail_control.get(&hex) == Some(&Side::Axis)
    }

    /// §54.41: the size of the largest run of **contiguous** Axis-controlled, operative (built,
    /// undestroyed) rail hexes. The Axis may use the railroad only while this is ≥ 5.
    pub fn axis_rail_contiguous_max(&self) -> usize {
        use std::collections::HashSet;
        // All Axis-controlled operative rail hexes.
        let nodes: HashSet<HexCoord> = self
            .rail_control
            .iter()
            .filter(|(_, &s)| s == Side::Axis)
            .map(|(&h, _)| h)
            .filter(|&h| self.hex_is_railroad(h))
            .collect();
        let mut seen: HashSet<HexCoord> = HashSet::new();
        let mut best = 0usize;
        for &start in &nodes {
            if seen.contains(&start) {
                continue;
            }
            // Flood-fill this connected component along built Railroad hexsides between Axis hexes.
            let mut size = 0usize;
            let mut stack = vec![start];
            while let Some(h) = stack.pop() {
                if !seen.insert(h) {
                    continue;
                }
                size += 1;
                for n in self.map.neighbors(h) {
                    if nodes.contains(&n)
                        && !seen.contains(&n)
                        && self.map.hexsides_on(h, n).iter().any(|hs| hs.kind == HexsideKind::Railroad)
                    {
                        stack.push(n);
                    }
                }
            }
            best = best.max(size);
        }
        best
    }

    /// §54.43: import Rolling Stock. At an Axis-controlled operative rail hex `hex`, consume 250
    /// Stores + 100 Fuel from Axis Supply Dumps there to activate +300 tons of per-Operations-Stage
    /// haulage capacity. Returns whether the import succeeded (enough supply present + hex eligible).
    pub fn import_rolling_stock(&mut self, hex: HexCoord) -> bool {
        if !self.axis_controls_rail(hex) {
            return false;
        }
        let loc = crate::state::unit::UnitLocation::OnMap(hex);
        let (stores, fuel): (u32, u32) = self
            .supply_dumps
            .iter()
            .filter(|d| !d.dummy && d.location == loc && d.side == Side::Axis)
            .fold((0, 0), |(s, f), d| (s + d.supply.stores, f + d.supply.fuel));
        if stores < 250 || fuel < 100 {
            return false;
        }
        // Draw 250 Stores and 100 Fuel from the Axis dumps in the hex.
        let mut owed_s = 250u32;
        let mut owed_f = 100u32;
        for d in self.supply_dumps.iter_mut().filter(|d| {
            !d.dummy && d.location == loc && d.side == Side::Axis
        }) {
            let ts = d.supply.stores.min(owed_s);
            d.supply.stores -= ts;
            owed_s -= ts;
            let tf = d.supply.fuel.min(owed_f);
            d.supply.fuel -= tf;
            owed_f -= tf;
            if owed_s == 0 && owed_f == 0 {
                break;
            }
        }
        self.axis_rolling_stock_tons += 300; // §54.43: +300 tons/OpStage of haulage
        true
    }

    /// §54.43/44: reserve `tons` of Axis rail haulage this Operations Stage in direction `west`,
    /// enforcing the single-direction-per-stage rule and the activated capacity. Returns whether
    /// the haulage fits; on success the budget and locked direction are updated.
    fn reserve_axis_rail_tons(&mut self, tons: u32, west: bool) -> bool {
        if self.axis_rolling_stock_tons == 0 {
            return false; // no activated Rolling Stock (§54.43/45)
        }
        if self.axis_rail_contiguous_max() < 5 {
            return false; // §54.41: needs five contiguous controlled hexes
        }
        if let Some(dir) = self.axis_rail_dir {
            if dir != west {
                return false; // §54.43/44: one direction per stage
            }
        }
        if self.axis_rail_tons_used + tons > self.axis_rolling_stock_tons {
            return false;
        }
        self.axis_rail_tons_used += tons;
        self.axis_rail_dir = Some(west);
        true
    }

    /// The water source available to a unit of `side` in `hex`. Adds the §52.22 rule to the
    /// terrain sources: the **Commonwealth** may treat any operating Railroad hex as an unlimited
    /// pipeline (§52.23 "similar to a major city"; §54.33). The defunct Barce–Benghazi line is not
    /// a `Railroad` hexside, so it is naturally excluded.
    pub fn hex_water_source_for(&self, hex: HexCoord, side: Side) -> Option<WaterKind> {
        if side == Side::Commonwealth {
            if self.hex_is_railroad(hex) {
                return Some(WaterKind::Unlimited);
            }
            // §52.22: a pipeline connected to a railroad hex (not only a Major City) is a CW source.
            if self.pipelines.contains(&hex) && self.pipeline_connected_via(hex, true) {
                return Some(WaterKind::Unlimited);
            }
        }
        self.hex_water_source(hex)
    }

    /// The Fortification Level of `hex` (§25.1): the higher of any placed/constructed fortification
    /// and the static city level (§25.12 — a Major City is Level 2; Cairo and Alexandria are Level
    /// 3). 0 if unfortified.
    pub fn fort_level(&self, hex: HexCoord) -> u8 {
        let placed = self.fortifications.get(&hex).copied().unwrap_or(0);
        let static_lvl = if self.map.terrain(hex) == Some(crate::reference::terrain::HexTerrain::MajorCity) {
            let is_capital = ["E1430", "E3613"] // Cairo, Alexandria (§25.12)
                .iter()
                .any(|s| HexId::parse(s).map(|h| h.to_hex_coord()) == Some(hex));
            let base: u8 = if is_capital { 3 } else { 2 };
            // §41.37: bombing can knock the static city level down (tracked in `city_fort_damage`).
            base.saturating_sub(self.city_fort_damage.get(&hex).copied().unwrap_or(0))
        } else {
            0
        };
        placed.max(static_lvl)
    }

    /// The Major City's inherent §25.12 Fortification Level (before bomb damage): 3 for Cairo /
    /// Alexandria, 2 for any other Major City, 0 elsewhere.
    fn city_static_fort_base(&self, hex: HexCoord) -> u8 {
        if self.map.terrain(hex) != Some(crate::reference::terrain::HexTerrain::MajorCity) {
            return 0;
        }
        let is_capital = ["E1430", "E3613"]
            .iter()
            .any(|s| HexId::parse(s).map(|h| h.to_hex_coord()) == Some(hex));
        if is_capital { 3 } else { 2 }
    }

    /// The defensive column shift from a hex's Fortification Level (§25.11): −1 per level, in the
    /// defender's favour. 0 if unfortified.
    pub fn fort_shift(&self, hex: HexCoord) -> i32 {
        -(self.fort_level(hex) as i32)
    }

    /// §17.32/§20.43 Training area for `side` — a hex flagged with that side's training-area feature
    /// on the map. The two lists are side-locked and not interchangeable (§20.43): the CW list
    /// (§17.32: Cairo/Helwan/Alexandria/Amiriya/Aboukir/Dekheila) never transfers to the Axis or
    /// vice-versa by capture. Data-driven from the section RON.
    pub(crate) fn is_training_area(&self, hex: HexCoord, side: Side) -> bool {
        let feat = match side {
            Side::Commonwealth => Feature::CwTrainingArea,
            Side::Axis => Feature::AxisTrainingArea,
        };
        self.map.hex(hex).is_some_and(|h| h.has_feature(feat))
    }

    /// §20.43: whether a Replacement Point pool at `loc` is at a valid Training location for `side`.
    /// On-map: a hex flagged with the side's training-area feature (`is_training_area`). Off-map: the
    /// Axis may train in the Tripoli box (§20.43); the Commonwealth has no off-map training box (its
    /// §17.32 areas are all on-map). A pool with no location, or in transit elsewhere, does not train.
    pub(crate) fn is_replacement_training_area(&self, loc: Option<&UnitLocation>, side: Side) -> bool {
        match loc {
            Some(UnitLocation::OnMap(hex)) => self.is_training_area(*hex, side),
            Some(UnitLocation::OffMap { node, .. }) => side == Side::Axis && node == "Tripoli",
            None => false,
        }
    }

    /// Whether a unit holds at least a quarter of its assigned Max-TOE (§17.35 instruction-battalion
    /// threshold). Units with no listed Max-TOE are treated as meeting it.
    pub(crate) fn unit_toe_at_least_quarter(&self, id: UnitId) -> bool {
        match self.unit_max_toe(id) {
            Some(max) if max > 0 => {
                self.units.get(&id).map(|u| u.total_toe()).unwrap_or(0) * 4 >= max
            }
            _ => true,
        }
    }

    /// Whether `hex` is a Major City with a well (the only valid origin for a pipeline, §52.21).
    pub(crate) fn is_major_city(&self, hex: HexCoord) -> bool {
        self.reference.campaign_victory.as_ref().is_some_and(|cv| {
            cv.cities
                .iter()
                .any(|c| HexId::parse(&c.hex).map(|h| h.to_hex_coord()) == Some(hex))
        })
    }

    /// §52.23/25: whether a pipeline hex is still connected — via contiguous pipeline hexes — back
    /// to a Major City well. A destroyed (removed) hex can sever the line, leaving the far segment
    /// disconnected (water then reachable only from the farthest still-connected hex).
    pub fn pipeline_connected(&self, hex: HexCoord) -> bool {
        self.pipeline_connected_via(hex, false)
    }

    /// As `pipeline_connected`, but if `allow_rail` the line may also anchor on a railroad hex
    /// (§52.22 — a Commonwealth capability), not only a Major City well.
    pub fn pipeline_connected_via(&self, hex: HexCoord, allow_rail: bool) -> bool {
        if !self.pipelines.contains(&hex) {
            return false;
        }
        let mut seen = std::collections::HashSet::new();
        let mut stack = vec![hex];
        while let Some(h) = stack.pop() {
            if !seen.insert(h) {
                continue;
            }
            if self
                .map
                .neighbors(h)
                .any(|n| self.is_major_city(n) || (allow_rail && self.hex_is_railroad(n)))
            {
                return true;
            }
            for n in self.map.neighbors(h) {
                if self.pipelines.contains(&n) && !seen.contains(&n) {
                    stack.push(n);
                }
            }
        }
        false
    }

    /// Construct one hex of Water Pipeline (§52.24): the `builder` must be an Engineer battalion or
    /// HQ in `hex`, the hex must adjoin a Major City well or an existing pipeline hex, and 10
    /// Stores must be present in a Supply Dump in the hex (they are expended). A side may build at
    /// most one pipeline hex per Operations Stage (`pipeline_built_this_stage`). Returns whether it
    /// was built.
    pub fn build_pipeline(&mut self, builder: UnitId, hex: HexCoord) -> bool {
        let Some(u) = self.units.get(&builder) else {
            return false;
        };
        if u.hex() != Some(hex) || self.pipelines.contains(&hex) {
            return false;
        }
        let side = u.nationality.side();
        // §52.24: only one pipeline hex may be built per side per Operations Stage.
        if self.pipeline_built_this_stage.contains(&side) {
            return false;
        }
        let ut = self
            .reference
            .unit_characteristics
            .get(&u.nationality)
            .and_then(|m| m.get(&u.id_code))
            .map(|uc| uc.unit_type.as_str())
            .unwrap_or("");
        if !(ut.contains("Engineer") || ut.contains("Headquarters")) {
            return false;
        }
        // §52.21/22: extend from a Major City well, an existing pipeline hex, or — for the
        // Commonwealth (§52.22) — a railroad hex.
        let allow_rail = side == Side::Commonwealth;
        let adj_ok = self.map.neighbors(hex).any(|n| {
            self.is_major_city(n) || self.pipelines.contains(&n) || (allow_rail && self.hex_is_railroad(n))
        });
        if !adj_ok {
            return false;
        }
        // §52.24: 10 Stores, present in the hex, are expended.
        let target = crate::state::unit::UnitLocation::OnMap(hex);
        let available: u32 = self
            .supply_dumps
            .iter()
            .filter(|d| !d.dummy && d.location == target)
            .map(|d| d.supply.stores)
            .sum();
        if available < 10 {
            return false;
        }
        let mut owed = 10u32;
        for d in self
            .supply_dumps
            .iter_mut()
            .filter(|d| !d.dummy && d.location == target)
        {
            let take = d.supply.stores.min(owed);
            d.supply.stores -= take;
            owed -= take;
            if owed == 0 {
                break;
            }
        }
        self.pipelines.insert(hex);
        self.pipeline_built_this_stage.insert(side);
        true
    }

    /// §24.5: build one Road from an UnfinishedRoad hexside. The `builder` — an Infantry Battalion
    /// of 3+ TOE (§24.53a) or an Engineer / HQ with engineering capability (§24.53b) — must occupy
    /// `a` or `b`; the edge must be an UnfinishedRoad; and **2 Stores** (§24.54) must be present in
    /// the builder's hex (they are expended). One road hex per Construction Stage, so this
    /// completes immediately. Returns whether it was built.
    pub fn build_road(&mut self, builder: UnitId, a: HexCoord, b: HexCoord) -> bool {
        let Some(u) = self.units.get(&builder) else {
            return false;
        };
        let Some(bh) = u.hex() else {
            return false;
        };
        if bh != a && bh != b {
            return false;
        }
        if !self
            .map
            .hexsides_on(a, b)
            .iter()
            .any(|h| h.kind == HexsideKind::UnfinishedRoad)
        {
            return false;
        }
        let ut = self
            .reference
            .unit_characteristics
            .get(&u.nationality)
            .and_then(|m| m.get(&u.id_code))
            .map(|c| c.unit_type.as_str())
            .unwrap_or("");
        let is_engineer = ut.contains("Engineer") || ut.contains("Headquarters");
        let is_inf_bn = ut.contains("Infantry")
            && matches!(u.org_size, crate::state::unit::OrgSize::Battalion)
            && u.total_toe() >= 3;
        if !(is_engineer || is_inf_bn) {
            return false;
        }
        // §24.54: 2 Stores present in the builder's hex are expended.
        let target = crate::state::unit::UnitLocation::OnMap(bh);
        let avail: u32 = self
            .supply_dumps
            .iter()
            .filter(|d| !d.dummy && d.location == target)
            .map(|d| d.supply.stores)
            .sum();
        if avail < 2 {
            return false;
        }
        let mut owed = 2u32;
        for d in self
            .supply_dumps
            .iter_mut()
            .filter(|d| !d.dummy && d.location == target)
        {
            let take = d.supply.stores.min(owed);
            d.supply.stores -= take;
            owed -= take;
            if owed == 0 {
                break;
            }
        }
        // §24.52: the UnfinishedRoad (a Track until now) becomes a real Road.
        self.map.remove_hexside(a, b, HexsideKind::UnfinishedRoad);
        self.map.set_hexside(a, b, HexsideKind::Road, None);
        true
    }

    /// §24.4: begin building a Fortification (Level 1 or 2) in `hex`. Requires an Engineer-capable
    /// unit **and** an Infantry Battalion of 3+ TOE among `builders`, all in `hex` (§24.42), 30
    /// Stores present in the hex (expended now, §24.43), a legal hex (§24.44: not Mountain/Salt
    /// Marsh/Desert/Major City/Delta, and — for new construction — not in an enemy ZOC), and no
    /// other project already in the hex (§24.46). Completes after 3 clean Construction Segments
    /// (`advance_construction`). Returns whether construction started.
    pub fn begin_fortification(&mut self, builders: &[UnitId], hex: HexCoord, level: u8) -> bool {
        if !(1..=2).contains(&level) || builders.is_empty() {
            return false;
        }
        if self.construction.iter().any(|p| p.hex == hex) {
            return false; // §24.46 / §24.15
        }
        // All builders present in the hex.
        if !builders.iter().all(|&b| self.units.get(&b).and_then(|u| u.hex()) == Some(hex)) {
            return false;
        }
        let unit_type = |b: UnitId| -> &str {
            self.units
                .get(&b)
                .and_then(|u| {
                    self.reference
                        .unit_characteristics
                        .get(&u.nationality)
                        .and_then(|m| m.get(&u.id_code))
                })
                .map(|c| c.unit_type.as_str())
                .unwrap_or("")
        };
        let has_engineer = builders.iter().any(|&b| {
            let ut = unit_type(b);
            ut.contains("Engineer") || ut.contains("Headquarters")
        });
        let has_inf_bn = builders.iter().any(|&b| {
            let ut = unit_type(b);
            ut.contains("Infantry")
                && self.units.get(&b).is_some_and(|u| {
                    matches!(u.org_size, crate::state::unit::OrgSize::Battalion) && u.total_toe() >= 3
                })
        });
        if !(has_engineer && has_inf_bn) {
            return false;
        }
        // §24.44: forbidden terrain.
        use crate::reference::terrain::HexTerrain::*;
        if matches!(
            self.map.terrain(hex),
            Some(Mountain | SaltMarsh | Desert | MajorCity | Delta)
        ) {
            return false;
        }
        // §24.44: new construction may not be in an enemy ZOC.
        let side = self.units.get(&builders[0]).map(|u| u.nationality.side());
        if let Some(s) = side {
            if self.hex_in_zoc_of(hex, s.opponent()) {
                return false;
            }
        }
        // §24.43: 30 Stores present in the hex, expended now.
        let target = crate::state::unit::UnitLocation::OnMap(hex);
        let avail: u32 = self
            .supply_dumps
            .iter()
            .filter(|d| !d.dummy && d.location == target)
            .map(|d| d.supply.stores)
            .sum();
        if avail < 30 {
            return false;
        }
        let mut owed = 30u32;
        for d in self
            .supply_dumps
            .iter_mut()
            .filter(|d| !d.dummy && d.location == target)
        {
            let take = d.supply.stores.min(owed);
            d.supply.stores -= take;
            owed -= take;
            if owed == 0 {
                break;
            }
        }
        self.construction.push(ConstructionProject {
            hex,
            kind: ConstructionKind::Fortification { level },
            builders: builders.to_vec(),
            stages_done: 0,
            stages_required: 3, // §24.42: 3 complete Construction Segments
        });
        true
    }

    /// §24.6: begin extending the railhead by building one Railroad stretch from an
    /// UnfinishedRailroad edge `(a, b)`. Requires rail-construction units (NZRRC — modeled as
    /// "Construction (Road/RR)" units; the specific 10th/13th-only restriction is an OB-counter
    /// nuance) in the construction hex (`a` or `b`), the edge to be contiguous with existing built
    /// track (§24.67: extends the railhead, no skipping), and 1 Store present (expended, §24.64).
    /// Two rail-construction companies in the hex complete it in one stage; one takes two (§24.62).
    /// Returns whether construction started.
    pub fn begin_railroad(&mut self, builders: &[UnitId], a: HexCoord, b: HexCoord) -> bool {
        if builders.is_empty() {
            return false;
        }
        // §54.42: only the Commonwealth (the line's owner) may build new rail line — the Axis may
        // never lay new track, only repair controlled hexes (`rebuild_railroad`).
        if self.units.get(&builders[0]).map(|u| u.nationality.side()) != Some(Side::Commonwealth) {
            return false;
        }
        if !self
            .map
            .hexsides_on(a, b)
            .iter()
            .any(|h| h.kind == HexsideKind::UnfinishedRailroad)
        {
            return false;
        }
        // §24.67: must extend from the railhead — one endpoint already touches built track.
        if !(self.hex_is_railroad(a) || self.hex_is_railroad(b)) {
            return false;
        }
        // All builders in the same construction hex (a or b).
        let Some(ch) = self.units.get(&builders[0]).and_then(|u| u.hex()) else {
            return false;
        };
        if (ch != a && ch != b)
            || !builders.iter().all(|&x| self.units.get(&x).and_then(|u| u.hex()) == Some(ch))
        {
            return false;
        }
        if self.construction.iter().any(|p| p.hex == ch) {
            return false; // §24.15/§24.46
        }
        // Rail-construction units among the builders (§24.61).
        let companies = builders
            .iter()
            .filter(|&&x| {
                self.units
                    .get(&x)
                    .and_then(|u| {
                        self.reference
                            .unit_characteristics
                            .get(&u.nationality)
                            .and_then(|m| m.get(&u.id_code))
                    })
                    .is_some_and(|c| c.unit_type.contains("RR"))
            })
            .count();
        if companies == 0 {
            return false;
        }
        // §24.64: 1 Store present in the construction hex, expended now.
        let target = crate::state::unit::UnitLocation::OnMap(ch);
        let avail: u32 = self
            .supply_dumps
            .iter()
            .filter(|d| !d.dummy && d.location == target)
            .map(|d| d.supply.stores)
            .sum();
        if avail < 1 {
            return false;
        }
        let mut owed = 1u32;
        for d in self
            .supply_dumps
            .iter_mut()
            .filter(|d| !d.dummy && d.location == target)
        {
            let take = d.supply.stores.min(owed);
            d.supply.stores -= take;
            owed -= take;
            if owed == 0 {
                break;
            }
        }
        self.construction.push(ConstructionProject {
            hex: ch,
            kind: ConstructionKind::Railroad { a, b },
            builders: builders.to_vec(),
            stages_done: 0,
            stages_required: if companies >= 2 { 1 } else { 2 }, // §24.62
        });
        true
    }

    /// §26.25: a vehicle entering an enemy minefield without an accompanying Engineer/HQ risks a
    /// mine casualty — on a die roll of 5 or 6 one TOE Strength Point (a tank / motorized-infantry
    /// point, or a Truck Point) is destroyed. Returns whether a point was lost.
    pub fn resolve_mine_casualty(&mut self, unit: UnitId, die: u8) -> bool {
        if die < 5 {
            return false;
        }
        let Some(u) = self.unit_mut(unit) else {
            return false;
        };
        match &mut u.composition {
            Composition::Weapons { groups } => {
                if let Some(g) = groups.iter_mut().find(|g| g.count > 0) {
                    g.count -= 1;
                    return true;
                }
                false
            }
            Composition::Organic { current_toe } => {
                if *current_toe > 0 {
                    *current_toe -= 1;
                    true
                } else {
                    false
                }
            }
        }
    }

    /// §24.66 / §52.25: destroy a built Railroad stretch (air/artillery bombardment, enemy
    /// occupation, or a destroyed railroad). The edge becomes DestroyedRailroad (inert until
    /// rebuilt). Returns whether track was destroyed.
    pub fn destroy_railroad(&mut self, a: HexCoord, b: HexCoord) -> bool {
        if self.map.remove_hexside(a, b, HexsideKind::Railroad) {
            self.map.set_hexside(a, b, HexsideKind::DestroyedRailroad, None);
            true
        } else {
            false
        }
    }

    /// §24.63 / §24.45: rebuild a DestroyedRailroad stretch. Unlike new track (§24.6), *any*
    /// Engineer battalion/company — or a Commonwealth HQ with engineering — may rebuild (not only
    /// the NZRRC), it completes in one Construction Segment (modeled as immediate), and it may be
    /// done in an enemy ZOC (§24.45). Requires the builder in `a`/`b` and 1 Store present (§24.64).
    /// §54.42: an **Axis** builder may repair only a rail hex the Axis controls (§54.41), and at the
    /// same rate as the Commonwealth (immediate) — but never builds *new* line (§24.6, `begin_railroad`).
    /// Returns whether the track was rebuilt.
    pub fn rebuild_railroad(&mut self, builder: UnitId, a: HexCoord, b: HexCoord) -> bool {
        let Some(u) = self.units.get(&builder) else {
            return false;
        };
        let Some(bh) = u.hex() else {
            return false;
        };
        if bh != a && bh != b {
            return false;
        }
        // §54.42: the Axis may repair only rail hexes it controls (§54.41 — it was the last to pass
        // a land combat unit through the builder's hex). The Commonwealth owns the line unrestricted.
        if u.nationality.side() == Side::Axis && self.rail_control.get(&bh) != Some(&Side::Axis) {
            return false;
        }
        if !self
            .map
            .hexsides_on(a, b)
            .iter()
            .any(|h| h.kind == HexsideKind::DestroyedRailroad)
        {
            return false;
        }
        let ut = self
            .reference
            .unit_characteristics
            .get(&u.nationality)
            .and_then(|m| m.get(&u.id_code))
            .map(|c| c.unit_type.as_str())
            .unwrap_or("");
        // §24.6: any Engineer/HQ (or Road/RR construction unit) may repair destroyed track.
        if !(ut.contains("Engineer") || ut.contains("Headquarters") || ut.contains("RR")) {
            return false;
        }
        // §24.64: 1 Store present in the builder's hex, expended.
        let target = crate::state::unit::UnitLocation::OnMap(bh);
        let avail: u32 = self
            .supply_dumps
            .iter()
            .filter(|d| !d.dummy && d.location == target)
            .map(|d| d.supply.stores)
            .sum();
        if avail < 1 {
            return false;
        }
        for d in self
            .supply_dumps
            .iter_mut()
            .filter(|d| !d.dummy && d.location == target)
        {
            if d.supply.stores >= 1 {
                d.supply.stores -= 1;
                break;
            }
        }
        self.map.remove_hexside(a, b, HexsideKind::DestroyedRailroad);
        self.map.set_hexside(a, b, HexsideKind::Railroad, None);
        true
    }

    /// §24.66: begin ground demolition of a built Railroad stretch `(a, b)`. An Engineer, an HQ
    /// with engineering capability, or an Infantry unit of 3+ TOE must occupy `a`/`b` and spend one
    /// complete Operations Stage there without expending CP (the §24.12 rule via
    /// `advance_construction`); the track is destroyed at the end of that clean stage. Returns
    /// whether demolition started.
    pub fn begin_railroad_demolition(&mut self, builder: UnitId, a: HexCoord, b: HexCoord) -> bool {
        let Some(u) = self.units.get(&builder) else {
            return false;
        };
        let Some(bh) = u.hex() else {
            return false;
        };
        if bh != a && bh != b {
            return false;
        }
        if !self
            .map
            .hexsides_on(a, b)
            .iter()
            .any(|h| h.kind == HexsideKind::Railroad)
        {
            return false;
        }
        if self.construction.iter().any(|p| p.hex == bh) {
            return false;
        }
        let ut = self
            .reference
            .unit_characteristics
            .get(&u.nationality)
            .and_then(|m| m.get(&u.id_code))
            .map(|c| c.unit_type.as_str())
            .unwrap_or("");
        let eligible = ut.contains("Engineer")
            || ut.contains("Headquarters")
            || ut.contains("RR")
            || (ut.contains("Infantry") && u.total_toe() >= 3);
        if !eligible {
            return false;
        }
        self.construction.push(ConstructionProject {
            hex: bh,
            kind: ConstructionKind::RailroadDemolition { a, b },
            builders: vec![builder],
            stages_done: 0,
            stages_required: 1, // §24.66: one complete Operations Stage
        });
        true
    }

    /// §24.3: begin laying a minefield (real or `dummy`) in `hex`. Requires an Engineer or HQ with
    /// engineering capability in the hex (§24.31), a Clear/Sand-Gravel/Rough hex (§24.35, one
    /// minefield per hex, not a Major City), not in an enemy ZOC (§24.36), and the supplies present
    /// (expended now): a real field needs **15 Stores + 15 Ammo** (§24.33), a dummy **3 Stores**
    /// and no Ammo (§24.34). Completes after one full Operations Stage (§24.32). Returns whether
    /// laying started.
    pub fn begin_minefield(&mut self, builder: UnitId, hex: HexCoord, dummy: bool) -> bool {
        let Some(u) = self.units.get(&builder) else {
            return false;
        };
        if u.hex() != Some(hex) || self.minefields.contains_key(&hex) {
            return false;
        }
        if self.construction.iter().any(|p| p.hex == hex) {
            return false;
        }
        let side = u.nationality.side();
        let ut = self
            .reference
            .unit_characteristics
            .get(&u.nationality)
            .and_then(|m| m.get(&u.id_code))
            .map(|c| c.unit_type.as_str())
            .unwrap_or("");
        if !(ut.contains("Engineer") || ut.contains("Headquarters")) {
            return false;
        }
        use crate::reference::terrain::HexTerrain::*;
        if !matches!(self.map.terrain(hex), Some(Clear | SandGravel | Rough)) {
            return false; // §24.35
        }
        if self.hex_in_zoc_of(hex, side.opponent()) {
            return false; // §24.36
        }
        let (need_stores, need_ammo) = if dummy { (3, 0) } else { (15, 15) };
        if !self.expend_supplies_in_hex(hex, need_stores, need_ammo, 0) {
            return false;
        }
        self.construction.push(ConstructionProject {
            hex,
            kind: ConstructionKind::Minefield { side, dummy },
            builders: vec![builder],
            stages_done: 0,
            stages_required: 1,
        });
        true
    }

    /// §26.13 / §23.22: begin clearing the minefield in `hex`. An Engineer or HQ with engineering
    /// capability must occupy `hex` and spend one full Operations Stage there without expending CP;
    /// the minefield is then removed. Returns whether clearing started.
    pub fn begin_minefield_removal(&mut self, builder: UnitId, hex: HexCoord) -> bool {
        let Some(u) = self.units.get(&builder) else {
            return false;
        };
        if u.hex() != Some(hex) || !self.minefields.contains_key(&hex) {
            return false;
        }
        if self.construction.iter().any(|p| p.hex == hex) {
            return false;
        }
        let ut = self
            .reference
            .unit_characteristics
            .get(&u.nationality)
            .and_then(|m| m.get(&u.id_code))
            .map(|c| c.unit_type.as_str())
            .unwrap_or("");
        if !(ut.contains("Engineer") || ut.contains("Headquarters")) {
            return false;
        }
        self.construction.push(ConstructionProject {
            hex,
            kind: ConstructionKind::MinefieldRemoval,
            builders: vec![builder],
            stages_done: 0,
            stages_required: 1,
        });
        true
    }

    /// §24.83: begin building a new Temporary Repair Facility in `hex`. Requires an Engineer unit
    /// (any size, incl. an HQ, §24.82) in the hex, a Major City or Village hex (§24.85), fewer than
    /// two temporary facilities already in existence (§24.85), no facility/project already there,
    /// not in an enemy ZOC, and 250 Stores + 150 Fuel present (expended now, §24.83). Completes
    /// after 3 Construction Segments. Returns whether construction started.
    pub fn begin_temporary_facility(&mut self, builder: UnitId, hex: HexCoord) -> bool {
        let Some(u) = self.units.get(&builder) else {
            return false;
        };
        if u.hex() != Some(hex) || self.repair_facilities.contains_key(&hex) {
            return false;
        }
        if self.construction.iter().any(|p| p.hex == hex) {
            return false;
        }
        let ut = self
            .reference
            .unit_characteristics
            .get(&u.nationality)
            .and_then(|m| m.get(&u.id_code))
            .map(|c| c.unit_type.as_str())
            .unwrap_or("");
        if !(ut.contains("Engineer") || ut.contains("Headquarters")) {
            return false;
        }
        let ok_hex = self.map.terrain(hex) == Some(crate::reference::terrain::HexTerrain::MajorCity)
            || self.map.features(hex).contains(&Feature::Village);
        if !ok_hex {
            return false;
        }
        let temp_count = self
            .repair_facilities
            .values()
            .filter(|k| **k == FacilityKind::Temporary)
            .count();
        if temp_count >= 2 {
            return false;
        }
        let side = u.nationality.side();
        if self.hex_in_zoc_of(hex, side.opponent()) {
            return false;
        }
        if !self.expend_supplies_in_hex(hex, 250, 0, 150) {
            return false;
        }
        self.construction.push(ConstructionProject {
            hex,
            kind: ConstructionKind::RepairFacility,
            builders: vec![builder],
            stages_done: 0,
            stages_required: 3,
        });
        true
    }

    /// Expend `stores`, `ammo`, and `fuel` points from (non-dummy) Supply Dumps in `hex`, if all
    /// are available. Returns whether the expenditure succeeded (no partial spend).
    fn expend_supplies_in_hex(&mut self, hex: HexCoord, stores: u32, ammo: u32, fuel: u32) -> bool {
        let target = crate::state::unit::UnitLocation::OnMap(hex);
        let (s, m, f): (u32, u32, u32) = self
            .supply_dumps
            .iter()
            .filter(|d| !d.dummy && d.location == target)
            .fold((0, 0, 0), |(s, m, f), d| {
                (s + d.supply.stores, m + d.supply.ammo, f + d.supply.fuel)
            });
        if s < stores || m < ammo || f < fuel {
            return false;
        }
        let mut os = stores;
        let mut oa = ammo;
        let mut of = fuel;
        for d in self
            .supply_dumps
            .iter_mut()
            .filter(|d| !d.dummy && d.location == target)
        {
            let ts = d.supply.stores.min(os);
            d.supply.stores -= ts;
            os -= ts;
            let ta = d.supply.ammo.min(oa);
            d.supply.ammo -= ta;
            oa -= ta;
            let tf = d.supply.fuel.min(of);
            d.supply.fuel -= tf;
            of -= tf;
        }
        true
    }

    /// §24.11/24.12: advance every construction project by one Construction Segment. A project
    /// advances only if all its builders are still in the hex and expended no CP that stage
    /// (otherwise it is halted and the stage does not count); completed projects apply their
    /// effect and are removed. Call once per Operations Stage (the Construction Segment).
    pub fn advance_construction(&mut self) {
        let advances: Vec<bool> = self
            .construction
            .iter()
            .map(|p| {
                p.builders.iter().all(|&b| {
                    self.units
                        .get(&b)
                        .is_some_and(|u| u.hex() == Some(p.hex) && u.cp_expended.0 == 0)
                })
            })
            .collect();
        for (p, adv) in self.construction.iter_mut().zip(advances) {
            if adv {
                p.stages_done += 1;
            }
        }
        let done: Vec<(usize, HexCoord, ConstructionKind)> = self
            .construction
            .iter()
            .enumerate()
            .filter(|(_, p)| p.stages_done >= p.stages_required)
            .map(|(i, p)| (i, p.hex, p.kind))
            .collect();
        for (i, hex, kind) in done.into_iter().rev() {
            match kind {
                ConstructionKind::Fortification { level } => {
                    let e = self.fortifications.entry(hex).or_insert(0);
                    *e = (*e).max(level);
                }
                ConstructionKind::Railroad { a, b } => {
                    self.map.remove_hexside(a, b, HexsideKind::UnfinishedRailroad);
                    self.map.set_hexside(a, b, HexsideKind::Railroad, None);
                }
                ConstructionKind::RailroadDemolition { a, b } => {
                    self.destroy_railroad(a, b);
                }
                ConstructionKind::Minefield { side, dummy } => {
                    self.minefields.insert(hex, Minefield { side, dummy });
                }
                ConstructionKind::MinefieldRemoval => {
                    self.minefields.remove(&hex);
                }
                ConstructionKind::RepairFacility => {
                    self.repair_facilities.insert(hex, crate::state::game::FacilityKind::Temporary);
                }
            }
            self.construction.remove(i);
        }
    }

    /// All air facilities in `hex` (a coastal hex could hold, e.g., both an airfield and a flying-
    /// boat basin) (§36).
    pub fn facilities_at(&self, hex: HexCoord) -> impl Iterator<Item = &AirFacility> {
        self.air_facilities.iter().filter(move |f| f.hex == hex)
    }

    /// Place a new air facility (scenario setup §36.0 or completed construction §24.7).
    pub fn add_air_facility(&mut self, facility: AirFacility) {
        self.air_facilities.push(facility);
    }

    /// §34.84/§34.85: add the current Game-Turn's airplane reinforcements to each side's plane pool
    /// (from the loaded schedule, monthly totals split across the month's Game-Turns). Called on each
    /// Game-Turn rollover; a no-op if no schedule is loaded.
    pub fn accrue_air_reinforcements(&mut self) {
        let arrivals = self
            .reference
            .air_reinforcements
            .as_ref()
            .map(|s| s.arrivals_for(self.game_turn))
            .unwrap_or_default();
        for (nat, aircraft, count) in arrivals {
            *self.plane_pool.entry(nat).or_default().entry(aircraft).or_insert(0) += count;
        }
    }

    /// Total planes of `aircraft` currently in `nationality`'s reinforcement pool (unassigned).
    pub fn pool_planes(&self, nationality: Nationality, aircraft: &str) -> u32 {
        self.plane_pool.get(&nationality).and_then(|m| m.get(aircraft)).copied().unwrap_or(0)
    }

    /// Unassigned rated pilots of `rating` in `nationality`'s pool (§60.32/§60.42). Rating 0 is
    /// unlimited (§40.16) and always reports 0 here — it is not drawn from the pool.
    pub fn pool_pilots(&self, nationality: Nationality, rating: u32) -> u32 {
        self.pilot_pool.get(&nationality).and_then(|m| m.get(&rating)).copied().unwrap_or(0)
    }

    /// §34.82/§60.32/§60.42: place an unplaced SGSU counter (of `nationality`, handling aircraft
    /// `class`) at `hex`, which must host an air facility. Spends one counter from `available_sgsu`
    /// and gives it the §35.12 printed CPA for its nationality. Returns the new `SquadronId`, or
    /// `None` if no counter is available or no air facility sits in the hex.
    pub fn deploy_sgsu(
        &mut self,
        nationality: Nationality,
        class: crate::reference::squadron::SquadronClass,
        hex: HexCoord,
    ) -> Option<crate::state::air::SquadronId> {
        let supply = self.available_sgsu.get(&nationality).copied().unwrap_or(0);
        // §34.82C/D: once the printed counter supply is exhausted, a counter may still be *created*
        // if the nationality meets its shortage condition.
        if supply == 0 && !self.may_create_extra_sgsu(nationality) {
            return None;
        }
        if !self.air_facilities.iter().any(|f| f.hex == hex) {
            return None;
        }
        let id = crate::state::air::SquadronId(
            self.sgsus.keys().map(|s| s.0).max().unwrap_or(0) + 1,
        );
        let cpa = crate::reference::squadron::sgsu_cpa(nationality);
        let designation = format!("{nationality:?} SGSU {}", id.0);
        self.sgsus.insert(
            id,
            crate::state::air::Sgsu::new(id, nationality, class, designation, cpa, Some(hex)),
        );
        // Spend a printed counter if one remains; otherwise this is a §34.82C/D created counter.
        if supply > 0 {
            *self.available_sgsu.entry(nationality).or_insert(0) -= 1;
        }
        Some(id)
    }

    /// §34.82C/D: whether `nationality` may create SGSU counters beyond its printed supply (i.e.
    /// deploy once `available_sgsu` is exhausted).
    /// - Axis German/Italian (§34.82D): at least 30% of that nation's total planes are based at
    ///   off-map (Mediterranean-box) facilities — computed from `med_base` squadrons.
    /// - Commonwealth (§34.82C): every North African off-map facility must be at its printed plane
    ///   capacity — the planes based off-map (§36.53, via `off_map_box`) reach
    ///   [`Self::max_off_map_north_african_planes`].
    pub fn may_create_extra_sgsu(&self, nationality: Nationality) -> bool {
        match nationality {
            Nationality::Commonwealth => {
                // §34.82C: all North African off-map facilities must be at printed plane capacity.
                match self.max_off_map_north_african_planes() {
                    Some(max) if max > 0 => self.off_map_north_african_planes() >= max,
                    _ => false,
                }
            }
            Nationality::German | Nationality::Italian => {
                let total: u32 = self
                    .sgsus
                    .values()
                    .filter(|s| s.nationality == nationality)
                    .map(|s| s.plane_count())
                    .sum();
                if total == 0 {
                    return false;
                }
                let off_map: u32 = self
                    .sgsus
                    .values()
                    .filter(|s| s.nationality == nationality && s.med_base.is_some())
                    .map(|s| s.plane_count())
                    .sum();
                off_map * 100 >= 30 * total
            }
        }
    }

    /// §34.82C: the "maximum planes possibly based" at the Commonwealth North African off-map
    /// facilities — the §36.53 printed squadron capacity times a Commonwealth squadron's full
    /// (ready + reserve) capacity at the current Game-Turn (§35.23). `None` if the chart is not
    /// loaded. This is the threshold the §34.82C creation condition compares against once
    /// Commonwealth off-map basing is modeled.
    pub fn max_off_map_north_african_planes(&self) -> Option<u32> {
        let chart = self.reference.off_map_air_facilities.as_ref()?;
        let per = crate::reference::squadron::squadron_capacity(Nationality::Commonwealth, self.game_turn).total();
        Some(chart.north_african_squadron_capacity() * per)
    }

    /// §34.82C: planes currently based at the Commonwealth North African off-map facilities — the
    /// planes in Commonwealth SGSUs whose `off_map_box` is a `north_african` §36.53 facility.
    pub fn off_map_north_african_planes(&self) -> u32 {
        let Some(chart) = self.reference.off_map_air_facilities.as_ref() else {
            return 0;
        };
        self.sgsus
            .values()
            .filter(|s| s.nationality == Nationality::Commonwealth)
            .filter(|s| {
                s.off_map_box.as_deref().is_some_and(|b| {
                    chart.facilities.iter().any(|f| f.name == b && f.north_african)
                })
            })
            .map(|s| s.plane_count())
            .sum()
    }

    /// §36.53: base a Commonwealth squadron in an off-map air facility (Canal-zone box or Ethiopia),
    /// by chart name. Enforces the facility's printed squadron capacity (§36.5b), the Ethiopia
    /// Game-Turn gate (§36.53 note), and the Port Said flying-boat-only restriction (§36.3). Sets
    /// `off_map_box` and clears the map location. Returns whether it was based.
    pub fn base_squadron_off_map(&mut self, id: crate::state::air::SquadronId, facility: &str) -> bool {
        let Some(chart) = self.reference.off_map_air_facilities.as_ref() else {
            return false;
        };
        let Some(fac) = chart.facilities.iter().find(|f| f.name == facility) else {
            return false;
        };
        // §36.53 time gate (Ethiopia opens at Game-Turn 35).
        if fac.available_from_gt.is_some_and(|gt| self.game_turn < gt) {
            return false;
        }
        let Some(sgsu) = self.sgsus.get(&id) else {
            return false;
        };
        if sgsu.nationality != Nationality::Commonwealth {
            return false; // §36.53 is the Allied off-map chart
        }
        // §36.3: a flying-boat basin bases flying boats only — i.e. a squadron whose class accepts
        // flying boats (a bomber squadron, §35.21).
        if fac.flying_boat
            && !sgsu.class.accepts(crate::reference::aircraft::AircraftClass::FlyingBoat)
        {
            return false;
        }
        // §36.5b printed squadron capacity (unless unlimited).
        if let Some(cap) = fac.max_squadrons {
            let here = self
                .sgsus
                .values()
                .filter(|s| s.id != id && s.off_map_box.as_deref() == Some(facility))
                .count() as u32;
            if here >= cap {
                return false;
            }
        }
        let sgsu = self.sgsus.get_mut(&id).expect("checked above");
        sgsu.off_map_box = Some(facility.to_string());
        sgsu.location = None;
        sgsu.med_base = None;
        true
    }

    /// Planes of `nationality` based at *on-map* air facilities — the planes in its SGSUs that have
    /// a map location and are not in an off-map Mediterranean box (§36.5). This is the quantity the
    /// §34.82 in-play SGSU cap is measured against.
    pub fn on_map_facility_planes(&self, nationality: Nationality) -> u32 {
        self.sgsus
            .values()
            .filter(|s| s.nationality == nationality && s.location.is_some() && s.med_base.is_none())
            .map(|s| s.plane_count())
            .sum()
    }

    /// SGSUs of `nationality` currently in play (deployed on the map or in a Mediterranean box).
    pub fn sgsus_in_play(&self, nationality: Nationality) -> u32 {
        self.sgsus.values().filter(|s| s.nationality == nationality).count() as u32
    }

    /// §34.82 in-play SGSU cap: the maximum number of SGSUs a nationality may have in play, from its
    /// planes based at on-map facilities — Commonwealth = ⌈planes/12⌉ + 2 (§34.82B), Axis German and
    /// Italian = ⌈planes/10⌉ (§34.82A). This is an *end-of-placement invariant* (see
    /// [`Self::sgsu_cap_exceeded`]) exposed for the driver to honour, not a per-`DeploySgsu` gate:
    /// the deploy-then-assign order and pool-first air reinforcements (§34.84) legitimately place
    /// empty SGSUs, and the Axis formula has no `+2` cushion. §34.82C/D counter-shortage creation is
    /// not modeled.
    pub fn sgsu_plane_cap(&self, nationality: Nationality) -> u32 {
        let planes = self.on_map_facility_planes(nationality);
        match nationality {
            Nationality::Commonwealth => planes.div_ceil(12) + 2,
            Nationality::German | Nationality::Italian => planes.div_ceil(10),
        }
    }

    /// §34.82: whether `nationality` currently has more SGSUs in play than its plane-based cap allows.
    pub fn sgsu_cap_exceeded(&self, nationality: Nationality) -> bool {
        self.sgsus_in_play(nationality) > self.sgsu_plane_cap(nationality)
    }

    /// Squadron withdrawals due this Game-Turn (§34.84 `WD`) — the schedule's `withdrawals_for` at
    /// the current turn. Empty unless this turn is a period's `gt_start`, or if no schedule is loaded.
    pub fn due_squadron_withdrawals(
        &self,
    ) -> Vec<(Nationality, crate::reference::squadron::WdClass, u32)> {
        self.reference
            .air_reinforcements
            .as_ref()
            .map(|s| s.withdrawals_for(self.game_turn))
            .unwrap_or_default()
    }

    /// §34.84: withdraw a deployed SGSU to satisfy a due `WD` requirement (the squadron leaves the
    /// theatre). Its ground unit is removed — the counter returns to `available_sgsu`, its planes
    /// return to the nationality's `plane_pool` (a national asset, reassignable), and its pilots are
    /// unassigned. Allowed only when a matching WD is due this turn: a Fighter/Bomber squadron
    /// satisfies a `WdClass::Fighter`/`Bomber` (or any due `WdClass::Recon`, which is not tied to a
    /// squadron class, §35.21). Returns whether it was withdrawn.
    pub fn withdraw_squadron(&mut self, id: crate::state::air::SquadronId) -> bool {
        use crate::reference::squadron::{SquadronClass, WdClass};
        let Some(sgsu) = self.sgsus.get(&id) else {
            return false;
        };
        if sgsu.location.is_none() {
            return false; // not deployed on the map
        }
        let nat = sgsu.nationality;
        let sc = sgsu.class;
        let due = self.due_squadron_withdrawals();
        // Reset the per-turn tally when the Game-Turn changes.
        if self.squadron_wd_done_gt != self.game_turn {
            self.squadron_wd_done.clear();
            self.squadron_wd_done_gt = self.game_turn;
        }
        let owed = |c: WdClass| due.iter().find(|&&(n, cc, _)| n == nat && cc == c).map(|&(_, _, k)| k).unwrap_or(0);
        let done = |c: WdClass| {
            self.squadron_wd_done.iter().find(|&&(n, cc, _)| n == nat && cc == c).map(|&(_, _, k)| k).unwrap_or(0)
        };
        // The class this withdrawal is charged against: the squadron's own class while that class
        // still owes a withdrawal, else any still-owed Recon (recon isn't a squadron class, §35.21).
        // Capping to the owed count stops an agent over-withdrawing (a `WD: 1F` removes one squadron).
        let charge = match sc {
            SquadronClass::Fighter if owed(WdClass::Fighter) > done(WdClass::Fighter) => Some(WdClass::Fighter),
            SquadronClass::Bomber if owed(WdClass::Bomber) > done(WdClass::Bomber) => Some(WdClass::Bomber),
            _ => None,
        }
        .or_else(|| (owed(WdClass::Recon) > done(WdClass::Recon)).then_some(WdClass::Recon));
        let Some(charge) = charge else {
            return false;
        };
        // Remove the squadron: planes → pool, counter → supply, pilots unassigned.
        let sgsu = self.sgsus.remove(&id).expect("checked above");
        let pool = self.plane_pool.entry(nat).or_default();
        for g in &sgsu.planes {
            *pool.entry(g.aircraft_type.clone()).or_insert(0) += g.count;
        }
        *self.available_sgsu.entry(nat).or_insert(0) += 1;
        for p in self.pilots.iter_mut() {
            if p.squadron == Some(id) {
                p.squadron = None;
            }
        }
        // Record it against the charged class so further withdrawals this turn respect the WD count.
        if let Some(e) = self.squadron_wd_done.iter_mut().find(|(n, cc, _)| *n == nat && *cc == charge) {
            e.2 += 1;
        } else {
            self.squadron_wd_done.push((nat, charge, 1));
        }
        true
    }

    /// §56.2/§56.21: the tonnage the Axis may ship in a convoy arriving on `arrival_gt`, given the
    /// die roll — looked up by that turn's month/year (Turn Record Track) on the Convoy Level Chart
    /// then the Capacity Table. `None` if the calendar month has no scheduled level or the chart is
    /// unloaded. (GT 55 = Nov I 1941, level E, die 4 → 21,000 t — the §56.21 example.)
    pub fn allowable_convoy_tonnage(&self, arrival_gt: u32, die: u32) -> Option<u32> {
        let d = crate::state::game::date_of(arrival_gt);
        self.reference
            .convoy_levels
            .as_ref()?
            .tonnage_for(d.year, d.month, die)
    }

    /// §56.15: plan an Axis convoy on `lane` (validated available this Game-Turn, §56.14) carrying
    /// the given tonnages, arriving at the lane's `destination` on `arrival_gt`. The convoy must fit
    /// the turn's **actual** rolled allowance (§56.2/§56.21, `allowable_tonnage`) *after* everything
    /// already committed to that Game-Turn — planned Replacement/Truck production (which has priority
    /// over supplies, §20.64) and any earlier convoys' supply (§20.62). So a supply convoy may claim
    /// only the tonnage production has not already reserved. Only one convoy per lane per Game-Turn
    /// (§56.12). Returns whether the convoy was accepted.
    #[allow(clippy::too_many_arguments)]
    pub fn plan_convoy(
        &mut self,
        lane: u8,
        destination: impl Into<String>,
        arrival_gt: u32,
        fuel_tons: u32,
        ammo_tons: u32,
        stores_tons: u32,
        replacement_tons: u32,
        allowable_tonnage: u32,
    ) -> bool {
        if !crate::rules::naval::lane_available(lane, arrival_gt) {
            return false;
        }
        let total = fuel_tons + ammo_tons + stores_tons + replacement_tons;
        // §20.62/§20.64: the whole Game-Turn's lift — priority Replacement/Truck production plus any
        // convoys already booked — must fit the actual rolled allowance, so supplies yield to
        // production rather than the two merely being bounded separately.
        if self.axis_shipping_committed(arrival_gt) + total > allowable_tonnage {
            return false;
        }
        if self
            .convoys
            .iter()
            .any(|c| c.lane == lane && c.arrival_gt == arrival_gt && !c.cancelled)
        {
            return false;
        }
        self.convoys.push(crate::state::naval::Convoy {
            lane,
            destination: destination.into(),
            arrival_gt,
            fuel_tons,
            ammo_tons,
            stores_tons,
            replacement_tons,
            cancelled: false,
        });
        true
    }

    /// §56.15: cancel any not-yet-arrived convoys bound for `port` (e.g. when the Commonwealth
    /// captures it). Returns how many were cancelled.
    pub fn cancel_convoys_to(&mut self, port: &str) -> usize {
        let mut n = 0;
        for c in self.convoys.iter_mut() {
            if c.destination == port && !c.cancelled {
                c.cancelled = true;
                n += 1;
            }
        }
        n
    }

    /// §41.6/§41.66: resolve a Commonwealth strategic bombing attack on the convoy at
    /// `convoy_index`. `bomb_points` is the delivered bombload; `torpedo_planes`/`total_planes`
    /// apply the §41.73 +25% torpedo bonus when at least half the planes carry torpedoes. Rolls are
    /// supplied as a 2d6 value (encoded `10*d1+d2`). Reads the Axis-Naval-Convoy row of the Air
    /// Bombardment CRT for the percentage of cargo lost, then removes that percentage (rounded up,
    /// §41.67) evenly from every cargo type. Returns the percentage lost, or `None` if the CRT is
    /// unloaded or the index is invalid.
    pub fn resolve_convoy_bombing(
        &mut self,
        convoy_index: usize,
        bomb_points: u32,
        torpedo_planes: u32,
        total_planes: u32,
        roll_2d6: u32,
    ) -> Option<u32> {
        use crate::reference::air_bombardment::{TargetGroup, WeaponPoints};
        let effective =
            crate::rules::air::cw_convoy_torpedo_tonnage(bomb_points, torpedo_planes, total_planes);
        let percent = {
            let table = self.reference.air_bombardment.as_ref()?;
            table
                .result(TargetGroup::AxisNavalConvoy, WeaponPoints::Bomb, effective, roll_2d6)
                .max(0) as u32
        };
        let c = self.convoys.get_mut(convoy_index)?;
        if c.cancelled {
            return Some(0);
        }
        for tons in [
            &mut c.fuel_tons,
            &mut c.ammo_tons,
            &mut c.stores_tons,
            &mut c.replacement_tons,
        ] {
            let loss = crate::rules::naval::convoy_cargo_loss(*tons, percent);
            *tons = tons.saturating_sub(loss);
        }
        Some(percent)
    }

    /// §56.16: deliver an arrived convoy's surviving supply tonnage into supply points (via the
    /// §54.5 Equivalent Weights rates) and remove it from the convoy list. Returns the supply
    /// Commodities delivered (Fuel/Ammo/Stores; replacement tonnage is handled by the replacement
    /// pool, not returned here). Port receiving-capacity (§55.13) is enforced at planning time.
    /// Returns `None` if the Equivalent Weights chart is unloaded or the index is invalid; a
    /// cancelled convoy delivers nothing.
    pub fn deliver_convoy(&mut self, convoy_index: usize) -> Option<Commodities> {
        let (fuel_rate, ammo_rate, stores_rate) = {
            let ew = self.reference.equivalent_weights.as_ref()?;
            (ew.fuel, ew.ammo, ew.stores)
        };
        let c = self.convoys.get(convoy_index)?;
        if c.cancelled {
            self.convoys.remove(convoy_index);
            return Some(Commodities::default());
        }
        let delivered = Commodities {
            fuel: fuel_rate.points(c.fuel_tons),
            ammo: ammo_rate.points(c.ammo_tons),
            stores: stores_rate.points(c.stores_tons),
            water: 0,
        };
        self.convoys.remove(convoy_index);
        Some(delivered)
    }

    /// §55.22/§55.24: scuttle to block `port`'s harbour, reducing its Efficiency Level by one (the
    /// per-Operations-Stage maximum). Requires the port to be blockable (§55.21) and a Friendly
    /// engineer/CW-HQ present. Returns the Ammunition/Stores cost the caller must expend, or `None`
    /// if the port is unblockable, has no engineer, or is unknown.
    pub fn block_harbor(&mut self, port: &str, has_engineer: bool) -> Option<(u32, u32)> {
        if !has_engineer || !crate::rules::naval::harbor_blockable(port) {
            return None;
        }
        let p = self.ports.get_mut(port)?;
        p.reduce_efficiency(crate::rules::naval::MAX_PORT_LEVEL_REDUCTION_PER_STAGE);
        Some(crate::rules::naval::harbor_block_cost(port))
    }

    /// §55.26: remove one level of harbour blocking from `port`, restoring one Efficiency Level.
    /// Requires the port-specific number of engineer units (§55.26: two for Benghazi). Returns the
    /// Ammunition/Stores cost, or `None` if too few engineers or the port is unknown/at maximum.
    pub fn unblock_harbor(&mut self, port: &str, engineers: u32) -> Option<(u32, u32)> {
        if engineers < crate::rules::naval::harbor_unblock_engineers(port) {
            return None;
        }
        let p = self.ports.get_mut(port)?;
        if p.efficiency >= p.max_efficiency {
            return None;
        }
        p.efficiency += 1;
        Some(crate::rules::naval::harbor_unblock_cost(port))
    }

    /// §30.17: place the San Giorgio wreck blocking a port's harbour (scenario setup). The port's
    /// starting Efficiency Level already reflects the §55.25 −3, held down by `wreck_block`. `gun`
    /// is its Barrage Rating (vs ships) and `aa` its Anti-air Rating (vs aircraft) — §60 note c.
    pub fn place_san_giorgio(&mut self, hex: crate::state::map::HexCoord, port: &str, gun: u32, aa: u32) {
        self.san_giorgio = Some(crate::state::naval::SanGiorgio::new(hex, port, gun, aa));
    }

    /// §30.17: Commonwealth engineers reduce the San Giorgio wreck by `points`. When it is finally
    /// removed, the blocked harbour clears (its §55.25 hold-down lifts, so it may repair to its
    /// full Efficiency Level). Returns `true` when this demolition removes the wreck.
    pub fn demolish_san_giorgio(&mut self, points: u32) -> bool {
        let Some(sg) = self.san_giorgio.as_mut() else {
            return false;
        };
        sg.damage(points);
        if sg.is_removed() {
            let port = sg.blocks_port.clone();
            if let Some(p) = self.ports.get_mut(&port) {
                p.wreck_block = 0;
            }
            self.san_giorgio = None;
            true
        } else {
            false
        }
    }

    /// Apply `levels` of bombardment damage to the `kind` facility in `hex` (§36.14). Off-map
    /// fields are never removed (only reduced to zero, §36.5-3); a landing strip reduced to zero
    /// is eliminated and removed from the map (§36.2). Returns whether a facility was affected.
    pub fn damage_air_facility(&mut self, hex: HexCoord, kind: AirFacilityKind, levels: u32) -> bool {
        let Some(idx) = self
            .air_facilities
            .iter()
            .position(|f| f.hex == hex && f.kind == kind)
        else {
            return false;
        };
        self.air_facilities[idx].reduce_capacity(levels);
        if self.air_facilities[idx].capacity_level == 0
            && !self.air_facilities[idx].off_map
            && kind.removed_when_zero()
        {
            self.air_facilities.remove(idx);
        }
        true
    }

    /// Destroy the pipeline in `hex` (§52.25: desert raid, enemy occupation, strafing, or a
    /// destroyed railroad). Returns whether a pipeline was removed.
    pub fn destroy_pipeline(&mut self, hex: HexCoord) -> bool {
        self.pipelines.remove(&hex)
    }

    /// Attempt to poison a Village/Bir well (§52.16): only a die roll of 1 succeeds (Major Cities
    /// and Oases cannot be poisoned). Returns whether the well was poisoned. (The 1 CP cost is
    /// paid by the caller.)
    pub fn poison_well(&mut self, hex: HexCoord, die: u8) -> bool {
        if !matches!(self.hex_water_source(hex), Some(WaterKind::Village | WaterKind::Bir)) {
            return false;
        }
        if die == 1 {
            self.poisoned_wells.insert(hex);
            // §52.14/§52.16: the poisoning side knows secretly (the active player does the poisoning).
            self.well_known_by.insert(hex, self.active_player);
            true
        } else {
            false
        }
    }

    /// Attempt to sweeten (un-poison) a well (§52.17): a die roll of 1–3 succeeds. Returns
    /// whether the well is now usable. (The 5 CP cost is paid by the caller.)
    pub fn sweeten_well(&mut self, hex: HexCoord, die: u8) -> bool {
        if (1..=3).contains(&die) {
            self.poisoned_wells.remove(&hex);
            // §52.14 bookkeeping: a sweetened well is clean again — clear its secret/revealed state.
            if !self.depleted_wells.contains(&hex) {
                self.well_known_by.remove(&hex);
                self.revealed_wells.remove(&hex);
            }
            true
        } else {
            false
        }
    }

    /// Consume Water for an Operations Stage (§52.4/5/53). Each unit first tops up its onboard
    /// Water from a well in its hex — a Major City/Oasis is unlimited; a Village/Bir gives a
    /// §52.7 die-roll draw and may deplete on a marked result — then spends its requirement.
    /// A unit that cannot is deprived: an infantry (Organic) unit loses 1 TOE per consecutive
    /// stage after the first without water (§52.53). `acted` = units that used CPA this stage
    /// (vehicles need water only if they moved, §52.42). Off-map units draw from the boxes.
    pub fn expend_water(&mut self, acted: &std::collections::HashSet<UnitId>) {
        if self.reference.water_table.is_none() {
            return; // water consumption applies only when the §52.7 table is in play
        }
        let ids: Vec<UnitId> = self.units.keys().copied().collect();
        for id in ids {
            let mut req = self.unit_water_requirement(id, acted.contains(&id));
            if req == 0 {
                continue;
            }
            let hex = self.units.get(&id).and_then(|u| u.hex());
            // §29.35: hot weather doubles a unit's water requirement.
            if hex.is_some_and(|h| self.weather_at(h) == Weather::Hot) {
                req *= 2;
            }
            match hex {
                None => {
                    // Off-map (Tripoli/Tunisia boxes): unlimited wells (§52.11).
                    if let Some(u) = self.unit_mut(id) {
                        if u.water < req {
                            u.water = req;
                        }
                    }
                }
                Some(h) => {
                    let side = self.units.get(&id).map(|u| u.nationality.side());
                    let kind = match side {
                        Some(s) => self.hex_water_source_for(h, s),
                        None => self.hex_water_source(h),
                    };
                    match kind {
                        Some(WaterKind::Unlimited) => {
                            if let Some(u) = self.unit_mut(id) {
                                if u.water < req {
                                    u.water = req;
                                }
                            }
                        }
                        Some(WaterKind::Village) | Some(WaterKind::Bir)
                            if !self.depleted_wells.contains(&h)
                                && !self.poisoned_wells.contains(&h) =>
                        {
                            let source = if kind == Some(WaterKind::Village) {
                                WaterSource::Town
                            } else {
                                WaterSource::Bir
                            };
                            // §52.14: an opposing unit drawing at a well whose depletion/poison it
                            // does not already know reveals it publicly (a marker is placed).
                            if (self.depleted_wells.contains(&h) || self.poisoned_wells.contains(&h))
                                && side.is_some_and(|s| self.well_known_by.get(&h) != Some(&s))
                            {
                                self.revealed_wells.insert(h);
                            }
                            if let Some(table) = self.reference.water_table.clone() {
                                let die = self.rng.d6() as u32;
                                let (points, check) = table.draw(source, die);
                                if let Some(u) = self.unit_mut(id) {
                                    u.water += points;
                                }
                                if check && self.rng.d6() == 1 {
                                    self.depleted_wells.insert(h);
                                    // §52.14: the drawing side notes the depletion secretly.
                                    if let Some(s) = side {
                                        self.well_known_by.insert(h, s);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            // Spend the requirement from onboard water.
            let onboard = self.units.get(&id).map_or(0, |u| u.water);
            if onboard >= req {
                if let Some(u) = self.unit_mut(id) {
                    u.water -= req;
                    u.stages_without_water = 0;
                }
            } else {
                let s = {
                    let u = self.unit_mut(id).expect("checked");
                    u.stages_without_water += 1;
                    u.stages_without_water
                };
                if s > 1 {
                    if let Some(u) = self.unit_mut(id) {
                        if let Composition::Organic { current_toe } = &mut u.composition {
                            *current_toe = current_toe.saturating_sub(1);
                        }
                    }
                }
            }
        }
    }

    /// Evaporate `pct` percent of the Fuel and Water in every (non-dummy) Supply Dump on the map
    /// (§49.3 / §52.44 / §29.34). Ammo and Stores do not evaporate; well/pipeline water is not a
    /// dump and is unaffected.
    pub fn evaporate_supplies(&mut self, pct: u32) {
        for d in self.supply_dumps.iter_mut().filter(|d| !d.dummy) {
            d.supply.fuel -= d.supply.fuel * pct / 100;
            d.supply.water -= d.supply.water * pct / 100;
        }
    }

    /// The base per-Game-Turn Fuel/Water evaporation rate (§49.3) for a dump of `side`: normally
    /// 6%, but the Commonwealth's poorly-built containers spill at **9%** from Sept 1940 through
    /// the last Game-Turn of August 1941 (GT 46 = Aug week IV on the Turn Record Track) before the
    /// "jerry can" fix (§49.3/§52.44).
    pub fn base_evaporation_rate(&self, side: Side) -> u32 {
        let d = date_of(self.game_turn);
        let cw_high_evap = d.year < 1941 || (d.year == 1941 && d.month <= 8);
        if side == Side::Commonwealth && cw_high_evap {
            9
        } else {
            6
        }
    }

    /// Apply the §49.3 Stores-Expenditure-Stage evaporation to every dump at its side's current
    /// base rate (Commonwealth 9% early-war, else 6%).
    pub fn evaporate_base_supplies(&mut self) {
        let rates: Vec<(usize, u32)> = self
            .supply_dumps
            .iter()
            .enumerate()
            .filter(|(_, d)| !d.dummy)
            .map(|(i, d)| (i, self.base_evaporation_rate(d.side)))
            .collect();
        for (i, pct) in rates {
            let d = &mut self.supply_dumps[i];
            d.supply.fuel -= d.supply.fuel * pct / 100;
            d.supply.water -= d.supply.water * pct / 100;
        }
    }

    /// §52.6 Italian Pasta Rule: each Italian battalion must receive an extra 1 Water Point when
    /// Stores are distributed. A battalion that does not get its Pasta Point may not voluntarily
    /// exceed its CPA this turn; if its cohesion is ≤ −10 it immediately becomes Disorganized
    /// (−26), regaining its original cohesion once it next receives the Point.
    pub fn distribute_pasta(&mut self) {
        let ids: Vec<UnitId> = self.units.keys().copied().collect();
        for id in ids {
            let is_italian_bn = self.units.get(&id).is_some_and(|u| {
                u.nationality == Nationality::Italian
                    && matches!(u.org_size, crate::state::unit::OrgSize::Battalion)
            });
            if !is_italian_bn {
                continue;
            }
            if self.consume_pasta_water(id) {
                self.no_pasta.remove(&id);
                if let Some(orig) = self.pasta_saved_cohesion.remove(&id) {
                    if let Some(u) = self.unit_mut(id) {
                        u.cohesion = orig; // regain the pre-disintegration level
                    }
                }
            } else {
                self.no_pasta.insert(id);
                let coh = self.units.get(&id).map(|u| u.cohesion).unwrap_or(0);
                if coh <= -10 && !self.pasta_saved_cohesion.contains_key(&id) {
                    self.pasta_saved_cohesion.insert(id, coh);
                    if let Some(u) = self.unit_mut(id) {
                        u.cohesion = -26; // immediately Disorganized
                    }
                }
            }
        }
    }

    /// Consume one Water Point for a unit's Pasta Point (§52.6), from its own load first, then a
    /// Supply Dump in its hex. Returns whether a point was available.
    fn consume_pasta_water(&mut self, id: UnitId) -> bool {
        if let Some(u) = self.unit_mut(id) {
            if u.water >= 1 {
                u.water -= 1;
                return true;
            }
        }
        let hex = self.units.get(&id).and_then(|u| u.hex());
        if let Some(h) = hex {
            let target = crate::state::unit::UnitLocation::OnMap(h);
            for d in self
                .supply_dumps
                .iter_mut()
                .filter(|d| !d.dummy && d.location == target)
            {
                if d.supply.water >= 1 {
                    d.supply.water -= 1;
                    return true;
                }
            }
        }
        false
    }

    /// Distribute Stores at the start of a Game-Turn (§51.1/51.2): every unit draws its
    /// requirement from friendly Stores Dumps in its hex (off-map units are supplied from the
    /// unlimited boxes). A unit that goes unsupplied earns a Disorganization Point (−1 cohesion,
    /// §51.21) and, every second consecutive such turn, loses that percentage of its
    /// **infantry** TOE (§51.22 — guns and tanks are unaffected). §51.23 half-rations are modeled
    /// (`half_rations` set: requirement cut 4→2 in `unit_stores_requirement`, with the movement
    /// restrictions enforced at the move command). Prisoner Stores (§51.12) and Guard-Point Stores
    /// (§51.17, 2 Stores/point/Game-Turn) are drawn first via `feed_prisoners`/`feed_guards`.
    pub fn expend_stores(&mut self) {
        // §49.3 / §52.44: Fuel and Water evaporate/spill during the Stores Expenditure Stage
        // (6%/Game-Turn, or the Commonwealth's early-war 9% through August 1941).
        self.evaporate_base_supplies();
        // §28.15/§51.12: prisoners are fed first — 1 Store per 5 Prisoner Points, from the nearest
        // dump. §51.17: their Guard Points each cost 2 Stores per Game-Turn, likewise from the dump.
        self.feed_prisoners();
        self.feed_guards();
        // §52.6: distribute the Italian Pasta Point (1 Water) alongside Stores.
        self.distribute_pasta();
        let ids: Vec<UnitId> = self.units.keys().copied().collect();
        for id in ids {
            let (hex, side, toe) = match self.units.get(&id) {
                Some(u) => (u.hex(), u.nationality.side(), u.total_toe()),
                None => continue,
            };
            if toe == 0 {
                continue;
            }
            let required = self.unit_stores_requirement(id);
            if required == 0 {
                continue;
            }
            // Off-map units draw from the unlimited Tripoli/Tunisia boxes (§8.88 / §52.11).
            let supplied = match hex {
                None => true,
                Some(h) => {
                    let avail: u32 = self
                        .supply_dumps
                        .iter()
                        .filter(|d| d.side == side && !d.dummy && d.location.hex() == Some(h))
                        .map(|d| d.supply.stores)
                        .sum();
                    if avail >= required {
                        let mut rem = required;
                        for d in self.supply_dumps.iter_mut().filter(|d| {
                            d.side == side && !d.dummy && d.location.hex() == Some(h)
                        }) {
                            if rem == 0 {
                                break;
                            }
                            let take = rem.min(d.supply.stores);
                            d.supply.stores -= take;
                            rem -= take;
                        }
                        true
                    } else {
                        false
                    }
                }
            };
            if supplied {
                if let Some(u) = self.unit_mut(id) {
                    u.turns_without_stores = 0;
                }
                continue;
            }
            // Unsupplied: Disorganization now, attrition every second consecutive turn.
            let c = {
                let u = self.unit_mut(id).expect("checked");
                u.turns_without_stores += 1;
                u.cohesion -= 1;
                u.turns_without_stores
            };
            if c % 2 == 0 {
                if let Some(u) = self.unit_mut(id) {
                    if let Composition::Organic { current_toe } = &mut u.composition {
                        let loss = (*current_toe * c + 50) / 100; // ⌈nearest⌉ of toe × c%
                        *current_toe = current_toe.saturating_sub(loss);
                    }
                }
            }
        }
    }

    /// §49: read-only companion to [`consume_move_fuel`] — whether a move of `cp_tenths` by `unit`
    /// and its formation members could be paid (tanks + carried cargo + friendly start-hex dumps),
    /// consuming nothing. The exact legality condition `consume_move_fuel` uses, factored out so the
    /// action enumerator can test a move without cloning and applying it.
    pub fn move_fuel_available(&self, unit: UnitId, from: HexCoord, cp_tenths: i32) -> bool {
        let Some(side) = self.unit(unit).map(|u| u.nationality.side()) else {
            return true;
        };
        let mut dump_demand: u32 = 0;
        let mut any_user = false;
        for m in self.formation_members(unit) {
            let cost = self.unit_move_fuel_cost(m, cp_tenths);
            if cost == 0 {
                continue;
            }
            any_user = true;
            let onboard = self.unit(m).map_or(0, |u| u.fuel);
            let after_tank = cost - onboard.min(cost);
            let cargo = self.unit(m).map_or(0, |u| u.truck_cargo.fuel);
            dump_demand += after_tank - cargo.min(after_tank);
        }
        if !any_user {
            return true; // no fuel users
        }
        let dump_fuel: u32 = self
            .supply_dumps
            .iter()
            .filter(|d| d.side == side && !d.dummy && d.location.hex() == Some(from))
            .map(|d| d.supply.fuel)
            .sum();
        dump_demand <= dump_fuel
    }

    /// Pay the Fuel for a move of `cp_tenths` by `unit` and its formation members (§49.13/16):
    /// each vehicle draws its cost from its own tank first, then from friendly Supply Dumps in
    /// the start hex `from`. Returns `false` (consuming nothing) if the collective demand
    /// exceeds the available tanks + dump Fuel — the caller must then reject the move.
    pub fn consume_move_fuel(&mut self, unit: UnitId, from: HexCoord, cp_tenths: i32) -> bool {
        let Some(side) = self.unit(unit).map(|u| u.nationality.side()) else {
            return true;
        };
        // Per-member: how much of the cost comes from its own tank and from the fuel its own
        // Truck Points are carrying as cargo (§49.18), with the remainder owed to start-hex dumps.
        let mut draws: Vec<(UnitId, u32, u32)> = Vec::new(); // (member, from_tank, from_cargo)
        let mut dump_demand: u32 = 0;
        for m in self.formation_members(unit) {
            let cost = self.unit_move_fuel_cost(m, cp_tenths);
            if cost == 0 {
                continue;
            }
            let onboard = self.unit(m).map_or(0, |u| u.fuel);
            let tank = onboard.min(cost);
            let after_tank = cost - tank;
            // §49.18: trucks may draw on the fuel they are transporting.
            let cargo = self.unit(m).map_or(0, |u| u.truck_cargo.fuel);
            let cargo_use = cargo.min(after_tank);
            dump_demand += after_tank - cargo_use;
            draws.push((m, tank, cargo_use));
        }
        if dump_demand == 0 && draws.is_empty() {
            return true; // no fuel users
        }
        let dump_fuel: u32 = self
            .supply_dumps
            .iter()
            .filter(|d| d.side == side && !d.dummy && d.location.hex() == Some(from))
            .map(|d| d.supply.fuel)
            .sum();
        if dump_demand > dump_fuel {
            return false;
        }
        // Commit: drain tanks and truck-cargo fuel, then the start-hex dumps.
        for (m, tank, cargo_use) in draws {
            if let Some(u) = self.unit_mut(m) {
                u.fuel -= tank;
                u.truck_cargo.fuel -= cargo_use;
            }
        }
        let mut remaining = dump_demand;
        if remaining > 0 {
            for d in self.supply_dumps.iter_mut().filter(|d| {
                d.side == side && !d.dummy && d.location.hex() == Some(from)
            }) {
                if remaining == 0 {
                    break;
                }
                let take = remaining.min(d.supply.fuel);
                d.supply.fuel -= take;
                remaining -= take;
            }
        }
        true
    }

    /// Roll weather for the coming Operations Stage (§29.1): the Initiative player rolls two
    /// dice (read as larger×10+smaller, §12.42), looks up the season row of the Weather Table
    /// (§29.61), and — for a storm — rolls one die on the Foul Weather Location Table (§29.7)
    /// to pick the affected map sections. Hot is all sections; Normal leaves all sections clear.
    pub fn determine_weather(&mut self) {
        let Some(table) = self.reference.weather_table.clone() else {
            return;
        };
        let season = season_of(self.game_turn);
        let roll = self.rng.sequential_read();
        let w = table.weather_for(season, roll);
        self.weather = w;
        self.weather_by_section.clear();
        match w {
            Weather::Normal => {}
            Weather::Hot => {
                for s in ['A', 'B', 'C', 'D', 'E'] {
                    self.weather_by_section.insert(s, Weather::Hot);
                }
            }
            Weather::Sandstorm | Weather::Rainstorm => {
                let die = self.rng.d6();
                for s in table.foul_sections(die) {
                    if let Some(c) = s.chars().next() {
                        self.weather_by_section.insert(c, w);
                    }
                }
            }
        }
        // §52.15: a rainstorm replenishes every depleted well on its map section(s).
        self.replenish_wells_on_rainstorm();
        // §29.34: hot weather evaporates an extra 5% of stored Fuel and Water.
        if w == Weather::Hot {
            self.evaporate_supplies(5);
        }
    }

    /// §52.15: replenish every depleted well on a map section that currently has a rainstorm.
    pub fn replenish_wells_on_rainstorm(&mut self) {
        let rain_sections: std::collections::HashSet<char> = self
            .weather_by_section
            .iter()
            .filter(|(_, &w)| w == Weather::Rainstorm)
            .map(|(&s, _)| s)
            .collect();
        if !rain_sections.is_empty() {
            let replenished = |h: &HexCoord| rain_sections.contains(&crate::state::map::section_char_of_col(h.col));
            self.depleted_wells.retain(|h| !replenished(h));
            // §52.14 bookkeeping: a replenished (no longer poisoned) well loses its secret/revealed
            // state; a still-poisoned well keeps it.
            let poisoned = self.poisoned_wells.clone();
            self.well_known_by.retain(|h, _| !replenished(h) || poisoned.contains(h));
            self.revealed_wells.retain(|h| !replenished(h) || poisoned.contains(h));
        }
    }

    /// §29.42/§29.54: whether construction is barred in `coord` this stage — its section has a
    /// Sandstorm or a Rainstorm.
    pub fn storm_at(&self, coord: HexCoord) -> bool {
        matches!(self.weather_at(coord), Weather::Sandstorm | Weather::Rainstorm)
    }

    /// §24.21/§29.32: during Hot Weather a construction site needs 10 Water Points (beyond the
    /// builders' own requirements). Returns `false` if the site is Hot and the builders can't
    /// pool 10 Water; otherwise deducts the 10 (greedily across builders) and returns `true`.
    /// A site in Normal/storm weather has no extra water requirement.
    pub(crate) fn charge_construction_water(&mut self, site: HexCoord, builders: &[UnitId]) -> bool {
        if self.weather_at(site) != Weather::Hot {
            return true;
        }
        let available: u32 = builders
            .iter()
            .filter_map(|id| self.units.get(id))
            .map(|u| u.water)
            .sum();
        if available < 10 {
            return false;
        }
        let mut owed = 10u32;
        for id in builders {
            if owed == 0 {
                break;
            }
            if let Some(u) = self.units.get_mut(id) {
                let take = u.water.min(owed);
                u.water -= take;
                owed -= take;
            }
        }
        true
    }

    /// The weather in the map section containing `coord` (§29.1); Normal if unaffected.
    pub fn weather_at(&self, coord: HexCoord) -> Weather {
        let section = crate::state::map::section_char_of_col(coord.col);
        self.weather_by_section
            .get(&section)
            .copied()
            .unwrap_or(Weather::Normal)
    }

    /// Whether `coord` is in an active sandstorm (§29.4): its section rolled Sandstorm and the
    /// hex isn't exempt — never over delta (§29.41) and never over sea, since sandstorms stop
    /// at the coastal hexes (§29.46).
    pub fn sandstorm_at(&self, coord: HexCoord) -> bool {
        self.weather_at(coord) == Weather::Sandstorm
            && !matches!(
                self.map.terrain(coord),
                Some(HexTerrain::Delta) | Some(HexTerrain::Sea)
            )
    }

    /// Whether a unit is a "combat unit" for occupation/victory (§60.82): its characteristics
    /// carry a non-parenthesized Close Assault rating.
    pub fn is_combat_unit(&self, id: UnitId) -> bool {
        let Some(u) = self.units.get(&id) else { return false };
        // A bare HQ (no combat units attached) is non-combat (§3.33/§9.12), whatever its arm.
        // (Short-circuit on `is_hq` so the O(units) attachment scan only runs for HQs.)
        if u.is_hq && !self.has_attached_units(id) {
            return false;
        }
        match &u.composition {
            // Weapons units (tanks, guns, anti-air) fight via their §4.47 weapon systems — they are
            // combat units even though their id carries no organic §4.46 close-assault rating.
            crate::state::unit::Composition::Weapons { groups } => groups.iter().any(|g| g.count > 0),
            // Organic units: combat unless their §4.46 close-assault rating is fully parenthesized
            // (support / command counters).
            crate::state::unit::Composition::Organic { .. } => self
                .reference
                .unit_characteristics
                .get(&u.nationality)
                .and_then(|m| m.get(&u.id_code))
                .and_then(|uc| uc.close_assault)
                .is_some_and(|ca| !ca.off.parenthesized || !ca.def.parenthesized),
        }
    }

    /// §16.11: whether `id` may supply Patrol Points — a Reconnaissance/Armored-Car unit, a
    /// light-tank unit (Italian CV 33, British Mark VI Light, or German Pz I), or motorized infantry.
    pub fn patrol_source_ok(&self, id: UnitId) -> bool {
        let Some(u) = self.units.get(&id) else { return false };
        if self.is_recce_unit(id) {
            return true;
        }
        if let Composition::Weapons { groups } = &u.composition {
            if groups.iter().any(|g| matches!(g.system.as_str(), "CV 33 (L3/35)" | "Mark VI Light" | "Pz I")) {
                return true;
            }
        }
        let ut = self
            .reference
            .unit_characteristics
            .get(&u.nationality)
            .and_then(|m| m.get(&u.id_code))
            .map(|c| c.unit_type.as_str())
            .unwrap_or("");
        let infantry = matches!(u.composition, Composition::Organic { .. }) || ut.contains("Infantry");
        infantry && self.unit_is_motorized(id)
    }

    /// Remove up to `n` TOE Strength Points from `id`, returning the number removed (Organic TOE, or
    /// weapon-group counts in order). Used by §16 Patrol casualties.
    pub(crate) fn remove_unit_toe(&mut self, id: UnitId, n: u32) -> u32 {
        let Some(u) = self.units.get_mut(&id) else { return 0 };
        match &mut u.composition {
            Composition::Organic { current_toe } => {
                let t = (*current_toe).min(n);
                *current_toe -= t;
                t
            }
            Composition::Weapons { groups } => {
                let mut rem = n;
                let mut removed = 0;
                for g in groups.iter_mut() {
                    if rem == 0 {
                        break;
                    }
                    let t = g.count.min(rem);
                    g.count -= t;
                    rem -= t;
                    removed += t;
                }
                removed
            }
        }
    }

    /// §16.4: number of `side` Dummy Tank Formations at `hex`.
    pub fn dummy_tanks_in_hex(&self, hex: HexCoord, side: Side) -> usize {
        self.dummy_tanks.iter().filter(|d| d.side == side && d.hex == hex).count()
    }

    /// §16.41/§16.42/§16.43: place a Dummy Tank Formation for `side` at `hex` with the borrowed
    /// `designation` (§16.44), expending 10 Stores present in the hex (§16.42). Enforces ≤3 dummies
    /// per side and ≤1 per hex (§16.43). Returns whether it was placed. (§16.43's Italian-only
    /// exclusion is folded into the per-side capability — dummies are a per-Player asset.)
    pub fn place_dummy_tank(&mut self, side: Side, hex: HexCoord, designation: impl Into<String>) -> bool {
        if self.dummy_tanks.iter().filter(|d| d.side == side).count() >= 3 {
            return false; // §16.43: at most three in existence
        }
        if self.dummy_tanks.iter().any(|d| d.hex == hex) {
            return false; // §16.43: at most one per hex
        }
        if !self.consume_stores_at(hex, side, 10) {
            return false; // §16.42: 10 Stores must be present in the hex
        }
        self.dummy_tanks.push(DummyTank { side, hex, designation: designation.into() });
        true
    }

    /// §16.46: eliminate any Dummy Tank Formations of `side` at `hex` (e.g. the hex undergoes a
    /// close assault, or anti-armor fire finds it with no real armor). Returns the number removed.
    pub fn eliminate_dummies_in_hex(&mut self, hex: HexCoord, side: Side) -> usize {
        let before = self.dummy_tanks.len();
        self.dummy_tanks.retain(|d| !(d.side == side && d.hex == hex));
        before - self.dummy_tanks.len()
    }

    // ----- §28 Prisoners ------------------------------------------------------------------------

    /// §28.11: add `points` Prisoner Points captured by `captor` at `hex` (from surrendered infantry
    /// TOE). Merges into an existing stack of the same captor there. `original` = the prisoners' own
    /// side (captor's opponent), used for §28.24 escape → replacement.
    pub fn capture_prisoners(&mut self, captor: Side, hex: HexCoord, points: u32) {
        if points == 0 {
            return;
        }
        if let Some(g) = self.prisoner_groups.iter_mut().find(|g| g.captor == captor && g.hex == hex) {
            g.points += points;
        } else {
            self.prisoner_groups.push(PrisonerGroup {
                captor,
                original: captor.opponent(),
                hex,
                points,
                guards: 0,
                cpa_used: 0,
            });
        }
    }

    /// §28.3: record `count` captured `system` TOE (from a surrendered weapons unit of `owner`) held
    /// by `captor` at `hex` — reusable once repaired.
    pub fn capture_equipment(&mut self, captor: Side, hex: HexCoord, owner: Nationality, system: String, count: u32) {
        if count == 0 {
            return;
        }
        if let Some(e) = self
            .captured_equipment
            .iter_mut()
            .find(|e| e.captor == captor && e.hex == hex && e.nationality == owner && e.system == system && !e.repaired)
        {
            e.count += count;
        } else {
            self.captured_equipment.push(CapturedEquipment { captor, hex, nationality: owner, system, count, repaired: false, destroyed_tank: false });
        }
    }

    /// §14.46: record `count` Destroyed Tank Markers of `system` (owned by `owner`) knocked out by
    /// Anti-Armor fire at `hex`, held by their own side (`owner.side()`) until captured. Merges into
    /// an existing unrepaired destroyed-tank entry for the same owner/system/hex.
    pub(crate) fn record_destroyed_tank(&mut self, hex: HexCoord, owner: Nationality, system: String, count: u32) {
        if count == 0 {
            return;
        }
        let side = owner.side();
        if let Some(e) = self.captured_equipment.iter_mut().find(|e| {
            e.destroyed_tank && !e.repaired && e.captor == side && e.hex == hex && e.nationality == owner && e.system == system
        }) {
            e.count += count;
        } else {
            self.captured_equipment.push(CapturedEquipment {
                captor: side, hex, nationality: owner, system, count, repaired: false, destroyed_tank: true,
            });
        }
    }

    /// §14.52: a Destroyed Tank Marker in a hex the enemy now controls — an enemy combat unit is
    /// present and no unit of the current owner is — is captured (its `captor` flips to the enemy).
    /// The owner-decides-to-eliminate option (§14.55) is modeled as always capturing. Resolved once
    /// per Operations Stage.
    pub(crate) fn resolve_destroyed_tank_capture(&mut self) {
        for i in 0..self.captured_equipment.len() {
            let (owner_side, hex) = {
                let e = &self.captured_equipment[i];
                if !e.destroyed_tank {
                    continue;
                }
                (e.captor, e.hex)
            };
            let enemy = owner_side.opponent();
            let ids = self.units_in_hex(hex);
            let enemy_present = ids.iter().any(|&id| {
                self.units.get(&id).is_some_and(|u| u.nationality.side() == enemy && u.total_toe() > 0)
            });
            let owner_present = ids.iter().any(|&id| {
                self.units.get(&id).is_some_and(|u| u.nationality.side() == owner_side && u.total_toe() > 0)
            });
            if enemy_present && !owner_present {
                self.captured_equipment[i].captor = enemy;
            }
        }
    }

    /// §49.19/§50.16/§51.16: a supply dump whose hex the enemy now controls (enemy present, owner
    /// absent) changes hands. Fuel and Water are non-denominational and pass intact (§49.19); only
    /// one-third of the Ammo (rounded up, §50.16) and half the Stores (§51.16) survive capture.
    pub(crate) fn resolve_captured_dumps(&mut self) {
        for i in 0..self.supply_dumps.len() {
            let (owner, hex) = {
                let d = &self.supply_dumps[i];
                match d.location {
                    UnitLocation::OnMap(h) if !d.dummy => (d.side, h),
                    _ => continue,
                }
            };
            let enemy = owner.opponent();
            let ids = self.units_in_hex(hex);
            let enemy_present = ids.iter().any(|&id| {
                self.units.get(&id).is_some_and(|u| u.nationality.side() == enemy && u.total_toe() > 0)
            });
            let owner_present = ids.iter().any(|&id| {
                self.units.get(&id).is_some_and(|u| u.nationality.side() == owner && u.total_toe() > 0)
            });
            if enemy_present && !owner_present {
                let d = &mut self.supply_dumps[i];
                d.side = enemy;
                d.supply.ammo = d.supply.ammo.div_ceil(3); // §50.16
                d.supply.stores /= 2; // §51.16
            }
        }
    }

    /// §28.11/§28.3: when `unit` surrenders, convert its losses — infantry (Organic) TOE becomes
    /// Prisoner Points for the opponent (§28.11); weapon (tank/gun/AA) TOE becomes captured
    /// equipment (§28.3). Called from the surrender resolution before the unit's TOE is zeroed.
    pub(crate) fn capture_on_surrender(&mut self, unit: UnitId) {
        let Some(u) = self.units.get(&unit) else { return };
        let Some(hex) = u.hex() else { return };
        let captor = u.nationality.side().opponent();
        let owner = u.nationality;
        match &u.composition {
            Composition::Organic { current_toe } => {
                let pts = *current_toe;
                self.capture_prisoners(captor, hex, pts);
            }
            Composition::Weapons { groups } => {
                let items: Vec<(String, u32)> = groups.iter().map(|g| (g.system.clone(), g.count)).collect();
                for (system, count) in items {
                    self.capture_equipment(captor, hex, owner, system, count);
                }
            }
        }
    }

    /// §28.16: whether `hex` is a Prisoner Departure Point for `captor`.
    pub fn is_pow_departure(&self, hex: HexCoord, captor: Side) -> bool {
        self.pow_departure_points.get(&captor).is_some_and(|s| s.contains(&hex))
    }

    /// §28.16: remove any Prisoner stacks that have reached their captor's Departure Point (permanent).
    pub fn process_prisoner_departures(&mut self) {
        let keep: Vec<bool> = self
            .prisoner_groups
            .iter()
            .map(|g| !self.is_pow_departure(g.hex, g.captor))
            .collect();
        let mut it = keep.into_iter();
        self.prisoner_groups.retain(|_| it.next().unwrap_or(true));
    }

    /// §28.15: feed prisoners — one Store per five Prisoner Points per Operations Stage, expended
    /// (before other Stores) from the nearest friendly Supply Dump to the stack. Called in the Stores
    /// Expenditure Stage. Returns the total Stores expended.
    pub fn feed_prisoners(&mut self) -> u32 {
        let demands: Vec<(HexCoord, Side, u32)> = self
            .prisoner_groups
            .iter()
            .map(|g| (g.hex, g.captor, g.points.div_ceil(5)))
            .filter(|&(_, _, need)| need > 0)
            .collect();
        let mut spent = 0;
        for (hex, captor, need) in demands {
            spent += self.draw_stores_from_nearest_dump(hex, captor, need);
        }
        spent
    }

    /// §51.17: feed Guard Points — two Stores per Guard Point per Game-Turn, drawn (like prisoners,
    /// §51.12) from the nearest friendly Supply Dump regardless of distance. Called in the Stores
    /// Expenditure Stage. Returns the total Stores expended.
    pub fn feed_guards(&mut self) -> u32 {
        let demands: Vec<(HexCoord, Side, u32)> = self
            .prisoner_groups
            .iter()
            .map(|g| (g.hex, g.captor, g.guards * 2))
            .filter(|&(_, _, need)| need > 0)
            .collect();
        let mut spent = 0;
        for (hex, captor, need) in demands {
            spent += self.draw_stores_from_nearest_dump(hex, captor, need);
        }
        spent
    }

    /// Expend up to `amount` Stores for `side` from the non-dummy Supply Dump nearest `hex` (by hex
    /// distance), draining successive nearest dumps as needed. Returns the amount actually drawn.
    fn draw_stores_from_nearest_dump(&mut self, hex: HexCoord, side: Side, amount: u32) -> u32 {
        let mut owed = amount;
        // Order this side's dumps with stores by distance from the stack.
        let mut order: Vec<usize> = self
            .supply_dumps
            .iter()
            .enumerate()
            .filter(|(_, d)| d.side == side && !d.dummy && d.supply.stores > 0)
            .map(|(i, _)| i)
            .collect();
        order.sort_by_key(|&i| self.supply_dumps[i].location.hex().map_or(u32::MAX, |h| h.distance(hex)));
        let mut drawn = 0;
        for i in order {
            if owed == 0 {
                break;
            }
            let take = owed.min(self.supply_dumps[i].supply.stores);
            self.supply_dumps[i].supply.stores -= take;
            owed -= take;
            drawn += take;
        }
        drawn
    }

    /// §28.17: Guard Points required for a Prisoner stack — one to hold a standing camp, or one per
    /// five when moving (the moving ratio; the printed digit was ambiguous, read as the 5 the whole
    /// POW system uses — the 5-point marker, per-5 feeding §28.15, 5-standing camp §28.14).
    pub fn guards_required(&self, points: u32, moving: bool) -> u32 {
        if points == 0 {
            0
        } else if moving {
            points.div_ceil(5)
        } else {
            1
        }
    }

    /// §28.23–25: resolve escapes, rejoins and recaptures. Called at the start of each Operations
    /// Stage. (1) §28.25 an escapee whose hex holds an enemy unit is recaptured; (2) §28.24 an
    /// escapee with a friendly unit within eight Capability Points (foot movement) rejoins and returns as Replacement Points a
    /// month later; (3) §28.23 any standing Prisoner stack left with no guard has its prisoners
    /// escape; (4) §28.24 each remaining far escapee steps one hex toward the nearest friendly unit
    /// and takes 10% thirst attrition (ceil) when its hex has no water source.
    pub fn resolve_prisoner_escapes(&mut self) {
        // Order matters — escape first, then rejoin/recapture — so a just-recaptured stack (§28.25)
        // is not re-escaped in the same pass; it holds until a guard is posted (or re-escapes a
        // stage later if still unguarded).
        // (1) §28.23 escape: any Prisoner stack with no Guard Point loses its prisoners.
        let escaped: Vec<(Side, HexCoord, u32)> = self
            .prisoner_groups
            .iter()
            .filter(|g| g.guards == 0 && g.points > 0)
            .map(|g| (g.original, g.hex, g.points))
            .collect();
        self.prisoner_groups.retain(|g| !(g.guards == 0 && g.points > 0));
        for (side, hex, points) in escaped {
            self.escape_points(side, hex, points); // rejoins immediately if a friendly is within 8
        }
        // (2) §28.24 rejoin: a pre-existing escapee with a friendly unit within eight Capability Points (foot movement) rejoins
        // and returns as Replacement Points.
        let mut i = 0;
        while i < self.escapees.len() {
            let e = self.escapees[i].clone();
            if self.friendly_unit_within(e.side, e.hex, 80) {
                self.schedule_escapee_replacements(e.side, e.hex, e.points);
                self.escapees.remove(i);
            } else {
                i += 1;
            }
        }
        // (3) §28.25 recapture: an enemy unit in the escapee's hex retakes them as prisoners.
        let mut i = 0;
        while i < self.escapees.len() {
            let e = self.escapees[i].clone();
            let enemy = e.side.opponent();
            let recaptured = self.units_in_hex(e.hex).into_iter().any(|id| {
                self.units.get(&id).is_some_and(|u| u.nationality.side() == enemy && u.total_toe() > 0)
            });
            if recaptured {
                self.capture_prisoners(enemy, e.hex, e.points);
                self.escapees.remove(i);
            } else {
                i += 1;
            }
        }
        // (4) §28.24 far-escapee autonomous movement + water attrition (Truck Convoy Phase; resolved
        // once per Operations Stage). Each still-wandering escapee steps one hex toward the nearest
        // friendly combat unit over passable, non-enemy-occupied desert, then — if its hex has no
        // water source — loses 10% of its strength (rounded up) to thirst; a stack reduced to zero
        // dies out. Rejoin/recapture at the new position resolve next stage.
        let mut i = 0;
        while i < self.escapees.len() {
            let e = self.escapees[i].clone();
            // Head toward the nearest friendly combat unit, if one exists.
            let target = self
                .units
                .values()
                .filter(|u| u.nationality.side() == e.side && u.total_toe() > 0)
                .filter_map(|u| u.hex())
                .min_by_key(|h| h.distance(e.hex));
            if let Some(tgt) = target {
                let next = self
                    .map
                    .neighbors(e.hex)
                    .filter(|&n| {
                        matches!(
                            step_cost(&self.reference, &self.map, e.hex, n, MoveMode::NonMotorized),
                            Some(MoveCost::Cost(_))
                        )
                    })
                    .filter(|&n| {
                        !self.units_in_hex(n).into_iter().any(|id| {
                            self.units.get(&id).is_some_and(|u| u.nationality.side() == e.side.opponent())
                        })
                    })
                    .min_by_key(|&n| n.distance(tgt));
                if let Some(n) = next {
                    if n.distance(tgt) < e.hex.distance(tgt) {
                        self.escapees[i].hex = n;
                    }
                }
            }
            // §28.24 water: no water source in the (new) hex → lose 10% (ceil) of strength to thirst.
            let hex = self.escapees[i].hex;
            if self.hex_water_source(hex).is_none() {
                let loss = self.escapees[i].points.div_ceil(10);
                self.escapees[i].points = self.escapees[i].points.saturating_sub(loss);
            }
            if self.escapees[i].points == 0 {
                self.escapees.remove(i);
            } else {
                i += 1;
            }
        }
    }

    /// §28.24: `points` prisoners of `side` escape at `hex` — if a friendly unit is within eight
    /// Capability Points (foot movement) they rejoin immediately (→ Replacement Points a month
    /// later); otherwise they become an [`Escapee`] awaiting rejoin or recapture.
    pub(crate) fn escape_points(&mut self, side: Side, hex: HexCoord, points: u32) {
        if points == 0 {
            return;
        }
        if self.friendly_unit_within(side, hex, 80) {
            self.schedule_escapee_replacements(side, hex, points);
        } else if let Some(e) = self.escapees.iter_mut().find(|e| e.side == side && e.hex == hex) {
            e.points += points;
        } else {
            self.escapees.push(Escapee { side, hex, points });
        }
    }

    /// §28.24: escaped prisoners return as `points` Infantry Replacement Points a month later.
    fn schedule_escapee_replacements(&mut self, side: Side, hex: HexCoord, points: u32) {
        // "One month" ≈ POW_ESCAPE_REPLACEMENT_DELAY_GT Game-Turns (documented; the calendar uses
        // ~4 sub-turns per month).
        const POW_ESCAPE_REPLACEMENT_DELAY_GT: u32 = 4;
        self.plan_replacement(
            ReplacementClass::Infantry,
            points,
            side,
            self.game_turn + POW_ESCAPE_REPLACEMENT_DELAY_GT,
            0,
            Some(UnitLocation::OnMap(hex)),
            None,
        );
    }

    /// §28.24: whether a friendly (same-`side`) combat unit (TOE > 0) is within `cp_budget` tenths
    /// of a Capability Point of `hex` for a foot escapee — the true "eight Capability Points" reach
    /// over real terrain, not a flat hex radius.
    fn friendly_unit_within(&self, side: Side, hex: HexCoord, cp_budget: i32) -> bool {
        self.units
            .values()
            .filter(|u| u.nationality.side() == side && u.total_toe() > 0)
            .filter_map(|u| u.hex())
            .any(|uh| self.within_cp_budget(hex, uh, MoveMode::NonMotorized, cp_budget))
    }

    /// Whether `hex` holds any armor-protection TOE for `side` (a real armored unit). Used for the
    /// §16.46 rule that anti-armor fire eliminates a dummy only when no real armor remains.
    pub(crate) fn hex_has_real_armor(&self, hex: HexCoord, side: Side) -> bool {
        self.units_in_hex(hex).into_iter().any(|id| {
            let Some(u) = self.units.get(&id) else { return false };
            if u.nationality.side() != side {
                return false;
            }
            let Composition::Weapons { groups } = &u.composition else { return false };
            let table = self.reference.weapons_systems.get(&u.nationality);
            groups.iter().any(|g| {
                g.count > 0
                    && table
                        .and_then(|m| m.get(&g.system))
                        .and_then(|w| w.armor_protection)
                        .is_some_and(|ap| ap.value > 0)
            })
        })
    }

    /// §16.51: a unit's characteristics `unit_type` string (for reconnaissance disclosure).
    pub fn unit_type_name(&self, id: UnitId) -> String {
        self.units
            .get(&id)
            .and_then(|u| self.reference.unit_characteristics.get(&u.nationality).and_then(|m| m.get(&u.id_code)))
            .map(|c| c.unit_type.clone())
            .unwrap_or_default()
    }

    /// The side occupying a city hex for victory (§64.73): the first combat unit of at least
    /// 1 TOE effectively there that also passes the §64.73 supply-adequacy test (enough Stores/
    /// Water for a week and Fuel/Ammo to move 20 CP and fire 3×, via `unit_supply_adequate`).
    /// `None` if no such unit is present.
    fn city_holder(&self, coord: HexCoord) -> Option<Side> {
        self.units_in_hex(coord).into_iter().find_map(|id| {
            let u = self.units.get(&id)?;
            (self.is_combat_unit(id) && u.total_toe() >= 1 && self.unit_supply_adequate(id))
                .then(|| u.nationality.side())
        })
    }

    /// Evaluate the §64.7 campaign victory: Geographical Occupation Points (§64.73) summed by
    /// side, then the §64.76 level from the point ratio. Occupation requires an in-supply
    /// combat unit (§64.73, via `city_holder`). The §64.71/§64.72 auto-wins are checked
    /// separately by `campaign_auto_winner`. The §64.74 unused-replacement and §64.75 CW
    /// withdrawal points are included below; note §64.74's allotment reads the production
    /// charts (`replacement_vp`), which are not yet driven by a §20 production stage.
    pub fn evaluate_campaign_victory(&self) -> Option<CampaignResult> {
        let cv = self.reference.campaign_victory.as_ref()?;
        let mut axis = 0u32;
        let mut cw = 0u32;
        for city in &cv.cities {
            let Some(coord) = HexId::parse(&city.hex).map(|h| h.to_hex_coord()) else {
                continue;
            };
            match self.city_holder(coord) {
                Some(Side::Axis) => axis += city.axis,
                Some(Side::Commonwealth) => cw += city.cw,
                None => {}
            }
        }
        // §64.74: 1 VP per unused (allotted-minus-consumed) Replacement Point.
        axis += self.replacement_vp(Side::Axis);
        cw += self.replacement_vp(Side::Commonwealth);
        // §64.75: Commonwealth Withdrawal Points.
        cw += self.withdrawal_vp().max(0) as u32;
        let (winner, most, least) = if axis > cw {
            (Some(Side::Axis), axis, cw)
        } else if cw > axis {
            (Some(Side::Commonwealth), cw, axis)
        } else {
            (None, axis, cw)
        };
        // Explicit divide-by-zero guard with distinct 100 / u32::MAX fallbacks; not a plain checked_div.
        #[allow(clippy::manual_checked_ops)]
        let ratio_pct = if least == 0 {
            if most == 0 { 100 } else { u32::MAX }
        } else {
            most * 100 / least
        };
        Some(CampaignResult {
            axis_vp: axis,
            cw_vp: cw,
            winner,
            level: cv.level_for(ratio_pct),
        })
    }

    /// The §64.74 unused-Replacement-Point victory points for a side: the total Replacement
    /// Points allotted in that side's Production Charts (§20.66/§20.78C — combat replacements
    /// only; trucks, planes, and Commonwealth infantry are held in separate tables and excluded)
    /// minus the points consumed so far (`replacements_used`).
    pub fn replacement_vp(&self, side: Side) -> u32 {
        let allotted: u32 = self
            .reference
            .production_charts
            .iter()
            .filter(|(nat, _)| nat.side() == side)
            .map(|(_, chart)| chart.items.iter().map(|i| i.total).sum::<u32>())
            .sum();
        allotted.saturating_sub(*self.replacements_used.get(&side).unwrap_or(&0))
    }

    /// Whether a Commonwealth unit may be voluntarily withdrawn (§64.75 A): a combat **battalion**
    /// (not company) of infantry/armour/artillery/anti-tank (not AA) at ≥75% TOE, currently in
    /// Alexandria or Cairo (the §64.71 objective hexes), not already withdrawn or locked out.
    pub fn withdrawal_eligible(&self, id: UnitId) -> bool {
        let Some(u) = self.units.get(&id) else {
            return false;
        };
        if u.nationality.side() != Side::Commonwealth {
            return false;
        }
        if !matches!(u.org_size, OrgSize::Battalion) {
            return false;
        }
        if self.withdrawn.contains_key(&id) {
            return false;
        }
        if self.withdrawal_lockout.get(&id).is_some_and(|&t| self.game_turn < t) {
            return false;
        }
        let Some(hex) = u.hex() else {
            return false;
        };
        let in_alex_cairo = self.reference.campaign_victory.as_ref().is_some_and(|cv| {
            cv.axis_objective
                .iter()
                .filter_map(|s| HexId::parse(s).map(|h| h.to_hex_coord()))
                .any(|c| c == hex)
        });
        if !in_alex_cairo {
            return false;
        }
        let uc = self
            .reference
            .unit_characteristics
            .get(&u.nationality)
            .and_then(|m| m.get(&u.id_code));
        let ut = uc.map(|c| c.unit_type.as_str()).unwrap_or("");
        if ut.contains("Anti-Air") || ut.contains("Anti-air") || ut.contains("AA") {
            return false; // AA excluded (§64.75 A)
        }
        let max_toe = uc.and_then(|c| c.max_toe.as_ref()).map(|m| m.value).unwrap_or(0);
        max_toe > 0 && u.total_toe() * 4 >= max_toe * 3 // ≥ 75% TOE
    }

    /// Voluntarily withdraw an eligible Commonwealth battalion from the game (§64.75). It leaves
    /// the map. Returns whether it was withdrawn.
    pub fn withdraw_unit(&mut self, id: UnitId) -> bool {
        if !self.withdrawal_eligible(id) {
            return false;
        }
        self.withdrawn.insert(id, self.game_turn);
        if let Some(u) = self.unit_mut(id) {
            u.location = None;
        }
        true
    }

    /// Return a withdrawn battalion to play at `hex` (§64.75 B): banks its earned points (1/week
    /// gone, max 3) minus the 2-point return penalty, and locks re-withdrawal for six months.
    pub fn return_unit(&mut self, id: UnitId, hex: HexCoord) -> bool {
        let Some(withdrawn_gt) = self.withdrawn.remove(&id) else {
            return false;
        };
        let weeks = self.game_turn.saturating_sub(withdrawn_gt).min(3) as i32;
        self.withdrawal_banked += weeks - 2;
        self.withdrawal_lockout.insert(id, self.game_turn + 26); // ~six months
        if let Some(u) = self.unit_mut(id) {
            u.location = Some(UnitLocation::OnMap(hex));
        }
        true
    }

    /// The §64.75 Commonwealth Withdrawal victory points: banked cycle points plus, for each
    /// still-withdrawn battalion, 1 per week gone (max 3).
    pub fn withdrawal_vp(&self) -> i32 {
        let live: i32 = self
            .withdrawn
            .values()
            .map(|&wgt| self.game_turn.saturating_sub(wgt).min(3) as i32)
            .sum();
        self.withdrawal_banked + live
    }

    /// The §64.71/§64.72 automatic winner, if any (checked before points). §64.71: the Axis
    /// holds every Alexandria+Cairo hex and each occupying combat unit can trace ≤ `axis_supply_mp`
    /// truck MP to a source-connected Supply Unit. §64.72: from `cw_check_from_turn`, if no Axis
    /// combat unit can trace ≤ `cw_check_mp`, the Commonwealth wins. The §64.71 "one full Game-Turn"
    /// hold is enforced via `axis_objective_held_since` (the condition must have held since a prior
    /// Game-Turn, not merely this instant).
    pub fn campaign_auto_winner(&self) -> Option<Side> {
        let cv = self.reference.campaign_victory.as_ref()?;
        let sources = self.strategic_sources();

        // §64.71 — Axis automatic victory: the objective+supply condition must currently hold
        // AND have been held continuously for at least one full Game-Turn.
        if self.axis_objective_supplied()
            && self.axis_objective_held_since.is_some_and(|s| self.game_turn > s)
        {
            return Some(Side::Axis);
        }

        // §64.72 — Commonwealth automatic victory.
        if cv.cw_check_from_turn > 0 && self.game_turn >= cv.cw_check_from_turn {
            let any_axis_supplied = self.units.values().any(|u| {
                u.nationality.side() == Side::Axis
                    && self.is_combat_unit(u.id)
                    && self.unit_strategic_supply_line(u.id, cv.cw_check_mp as i32, &sources)
            });
            if !any_axis_supplied {
                return Some(Side::Commonwealth);
            }
        }
        None
    }

    /// The strategic supply sources (§64.71 "Tobruk or Tripoli") as hex coordinates.
    fn strategic_sources(&self) -> Vec<HexCoord> {
        self.reference
            .campaign_victory
            .as_ref()
            .map(|cv| {
                cv.sources
                    .iter()
                    .filter_map(|s| HexId::parse(s).map(|h| h.to_hex_coord()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The instantaneous §64.71 condition: the Axis holds (in-supply) every objective hex and
    /// each occupying combat unit can trace ≤ `axis_supply_mp` truck MP to a source.
    fn axis_objective_supplied(&self) -> bool {
        let Some(cv) = self.reference.campaign_victory.as_ref() else {
            return false;
        };
        if cv.axis_objective.is_empty() {
            return false;
        }
        let sources = self.strategic_sources();
        let objs: Vec<HexCoord> = cv
            .axis_objective
            .iter()
            .filter_map(|s| HexId::parse(s).map(|h| h.to_hex_coord()))
            .collect();
        // §64.71 supply is the ≤90-MP strategic convoy line (not the §64.73 co-located commodity
        // adequacy, which governs §64.73 scoring): every objective hex holds an Axis combat unit
        // of ≥1 TOE, and every such unit can trace the line back to a source-supplied dump.
        objs.iter().all(|&c| {
            let occupants: Vec<UnitId> = self
                .units_in_hex(c)
                .into_iter()
                .filter(|&id| {
                    self.unit(id)
                        .is_some_and(|u| u.nationality.side() == Side::Axis && u.total_toe() >= 1)
                        && self.is_combat_unit(id)
                })
                .collect();
            !occupants.is_empty()
                && occupants
                    .iter()
                    .all(|&id| self.unit_strategic_supply_line(id, cv.axis_supply_mp as i32, &sources))
        })
    }

    /// Maintain the §64.71 "held for one full Game-Turn" tracker. Called once per Game-Turn
    /// (at Initiative Determination): records the turn the Axis first satisfied the objective+
    /// supply condition, and clears it the moment the condition lapses.
    pub fn update_victory_hold(&mut self) {
        if self.axis_objective_supplied() {
            self.axis_objective_held_since.get_or_insert(self.game_turn);
        } else {
            self.axis_objective_held_since = None;
        }
    }

    /// Place a Supply Dump at a location for a side, holding the given Ammo/Fuel (Stores/Water
    /// zero). A convenience for tests; scenarios load full four-commodity dumps.
    pub fn add_supply_dump(&mut self, location: UnitLocation, side: Side, ammo: u32, fuel: u32) {
        self.supply_dumps.push(SupplyDump {
            location,
            side,
            supply: Commodities {
                fuel,
                ammo,
                stores: 0,
                water: 0,
            },
            dummy: false,
        });
    }

    /// Add a (non-dummy) Supply Dump holding an explicit set of Commodities (Fuel/Ammo/Stores/
    /// Water) — used where all four commodities matter, e.g. §64.73 supply-adequacy.
    pub fn add_supply_dump_full(&mut self, location: UnitLocation, side: Side, supply: Commodities) {
        self.supply_dumps.push(SupplyDump { location, side, supply, dummy: false });
    }

    /// Add `amount` of one Commodity to a friendly non-dummy Supply Dump in `hex`, merging into an
    /// existing dump or creating one if none is present (e.g. delivering airdropped supplies).
    pub fn add_commodity_to_dump(&mut self, hex: HexCoord, side: Side, commodity: Commodity, amount: u32) {
        if let Some(d) = self
            .supply_dumps
            .iter_mut()
            .find(|d| d.side == side && !d.dummy && d.location.hex() == Some(hex))
        {
            *d.supply.get_mut(commodity) += amount;
        } else {
            let mut supply = Commodities::default();
            *supply.get_mut(commodity) += amount;
            self.add_supply_dump_full(UnitLocation::OnMap(hex), side, supply);
        }
    }

    /// Consume `amount` Stores Points from `side`'s (non-dummy) Supply Dumps in `hex` (an airfield
    /// is itself such a dump, §36.17). Returns `false` and changes nothing if fewer than `amount`
    /// Stores are present. Used for the §38.36 per-squadron refit cost.
    pub fn consume_stores_at(&mut self, hex: HexCoord, side: Side, amount: u32) -> bool {
        self.consume_commodity_at(hex, side, Commodity::Stores, amount)
    }

    /// Points of `commodity` available in `side`'s non-dummy Supply Dumps in `hex`.
    pub fn available_commodity_at(&self, hex: HexCoord, side: Side, commodity: Commodity) -> u32 {
        self.supply_dumps
            .iter()
            .filter(|d| !d.dummy && d.side == side && d.location.hex() == Some(hex))
            .map(|d| d.supply.amount(commodity))
            .sum()
    }

    /// Consume `amount` points of `commodity` from `side`'s non-dummy Supply Dumps in `hex`,
    /// all-or-nothing.
    pub fn consume_commodity_at(&mut self, hex: HexCoord, side: Side, commodity: Commodity, amount: u32) -> bool {
        if self.available_commodity_at(hex, side, commodity) < amount {
            return false;
        }
        let mut owed = amount;
        for d in self
            .supply_dumps
            .iter_mut()
            .filter(|d| !d.dummy && d.side == side && d.location.hex() == Some(hex))
        {
            let slot = d.supply.amount_mut(commodity);
            let take = (*slot).min(owed);
            *slot -= take;
            owed -= take;
            if owed == 0 {
                break;
            }
        }
        true
    }

    /// §38.2/§38.21: refuel up to `want` of a squadron's refitted planes to ready, paying each
    /// plane's Fuel Consumption Rating (from its aircraft chart) from Fuel at the base, capped by
    /// the ready capacity (§37.24) and available Fuel. Returns the number newly readied, or `None`
    /// if the squadron is unknown or off-map.
    pub fn refuel_squadron(&mut self, squadron: SquadronId, want: u32) -> Option<u32> {
        let (nationality, hex) = {
            let s = self.sgsus.get(&squadron)?;
            (s.nationality, s.location?)
        };
        let side = nationality.side();
        let (cap, current_ready) = {
            let s = self.sgsus.get(&squadron)?;
            (s.capacity(self.game_turn).ready, s.ready_planes())
        };
        let mut budget = want.min(cap.saturating_sub(current_ready));
        // (group index, type, refittable-but-unfuelled candidates)
        let groups: Vec<(usize, String, u32)> = self.sgsus.get(&squadron)?.planes.iter().enumerate()
            .map(|(i, g)| (i, g.aircraft_type.clone(), g.refitted - g.ready))
            .collect();
        let mut readied = 0;
        for (i, atype, candidates) in groups {
            if budget == 0 {
                break;
            }
            let fuel_per = self
                .reference
                .aircraft_charts
                .get(&nationality)
                .and_then(|c| c.get(&atype))
                .and_then(|a| a.loadouts.first())
                .map_or(0, |l| l.fuel);
            let by_budget = candidates.min(budget);
            // fuel_per == 0 means "no fuel cost", so the budget is the only cap — not a checked_div.
            #[allow(clippy::manual_checked_ops)]
            let n = if fuel_per == 0 {
                by_budget
            } else {
                by_budget.min(self.available_commodity_at(hex, side, Commodity::Fuel) / fuel_per)
            };
            if n > 0 {
                if fuel_per > 0 {
                    self.consume_commodity_at(hex, side, Commodity::Fuel, n * fuel_per);
                }
                if let Some(sgsu) = self.sgsus.get_mut(&squadron) {
                    sgsu.planes[i].ready += n;
                }
                readied += n;
                budget -= n;
            }
        }
        Some(readied)
    }

    /// §38.4/§38.43: arm up to `want` of a squadron's ready planes, paying one Ammunition Point at
    /// the base for the squadron's TacAir (§38.43). Returns the number newly armed, or `None` if the
    /// squadron is unknown/off-map or no Ammo is present when arming is needed.
    pub fn arm_squadron(&mut self, squadron: SquadronId, want: u32) -> Option<u32> {
        let (side, hex) = {
            let s = self.sgsus.get(&squadron)?;
            (s.nationality.side(), s.location?)
        };
        // Only pay/act if there is at least one ready-but-unarmed plane to arm.
        let armable = {
            let s = self.sgsus.get(&squadron)?;
            (s.ready_planes() - s.armed_planes()).min(want)
        };
        if armable == 0 {
            return Some(0);
        }
        if !self.consume_commodity_at(hex, side, Commodity::Ammo, crate::rules::air::SQUADRON_TACAIR_AMMO) {
            return None;
        }
        Some(self.sgsus.get_mut(&squadron)?.arm_up_to(want))
    }

    /// §35.14: consume each SGSU's own upkeep from its base dump — 1 Fuel + 1 Water per Operations
    /// Stage, plus 1 Stores per Game-Turn (charged on the first stage). An SGSU that cannot pay is
    /// marked unsupplied and may not repair (refit) its planes this stage. Enforced only where a
    /// friendly Supply Dump exists at the base; an SGSU with no dump (e.g. box-supplied Med bases,
    /// §43.21) is presumed supplied. Called at the start of each Operations Stage.
    pub fn consume_sgsu_upkeep(&mut self) {
        let stage_one = self.operations_stage == 1;
        let ids: Vec<SquadronId> = self.sgsus.keys().copied().collect();
        for id in ids {
            let (side, hex) = match self.sgsus.get(&id) {
                Some(s) => (s.nationality.side(), s.location),
                None => continue,
            };
            let Some(hex) = hex else {
                // §43.21: a Box-based (off-map Mediterranean) squadron is auto-supplied by the Box —
                // no Fuel/Water/Stores upkeep is drawn, and it counts as supplied.
                if self.sgsus.get(&id).is_some_and(|s| s.med_base.is_some()) {
                    if let Some(s) = self.sgsus.get_mut(&id) {
                        s.supplied = true;
                    }
                }
                continue;
            };
            let has_dump = self
                .supply_dumps
                .iter()
                .any(|d| !d.dummy && d.side == side && d.location.hex() == Some(hex));
            if !has_dump {
                continue; // presumed supplied (no modeled dump at this base)
            }
            let fuel = self.consume_commodity_at(hex, side, Commodity::Fuel, Sgsu::FUEL_UPKEEP_PER_STAGE);
            let water = self.consume_commodity_at(hex, side, Commodity::Water, Sgsu::WATER_UPKEEP_PER_STAGE);
            let stores = !stage_one
                || self.consume_commodity_at(hex, side, Commodity::Stores, Sgsu::STORES_UPKEEP_PER_TURN);
            if let Some(s) = self.sgsus.get_mut(&id) {
                s.supplied = fuel && water && stores;
            }
        }
    }

    /// §30.35/§30.39: resolve surface gunfire against a Commonwealth warship on the Air
    /// Bombardment Table — the ship counts as a "unit" and the result is the number of Gun-Rating
    /// Damage Points it takes (§30.34). `points` are the Bomb-equivalent points computed by the
    /// caller (raw artillery halved per §30.35, or an emplaced gun's / the San Giorgio's full Gun
    /// Rating). Returns the damage dealt.
    pub fn resolve_gun_hit_on_ship(&mut self, points: u32, ship_index: usize, roll: u32) -> i32 {
        use crate::reference::air_bombardment::{TargetGroup, WeaponPoints};
        let dmg = self
            .reference
            .air_bombardment
            .as_ref()
            .map(|t| t.result(TargetGroup::Units, WeaponPoints::Bomb, points, roll))
            .unwrap_or(0)
            .max(0);
        if let Some(s) = self.fleet.get_mut(ship_index) {
            s.damage(dmg as u32);
        }
        dmg
    }

    /// §46.0/§46.11: total anti-aircraft points fired from `hex` at planes of `bomber_side` — the
    /// sum over enemy (non-`bomber_side`) land units in the hex of TOE Strength × AA rating (no
    /// divide-by-ten). Facility intrinsic AA (§36.18) is excluded here, as it fires only against
    /// strafing / dive-bombing (§40.7), not level bombing.
    pub fn hex_flak_points(&self, hex: HexCoord, bomber_side: Side) -> u32 {
        self.units
            .values()
            .filter(|u| u.hex() == Some(hex) && u.nationality.side() != bomber_side)
            .map(|u| {
                let rating = self
                    .reference
                    .unit_characteristics
                    .get(&u.nationality)
                    .and_then(|m| m.get(&u.id_code))
                    .and_then(|uc| uc.anti_air)
                    .map(|r| r.value.max(0) as u32)
                    .unwrap_or(0);
                crate::rules::air::aa_points(u.total_toe(), rating)
            })
            .sum()
    }

    /// §40.6: eliminate up to `points` TOE Strength Points from the first enemy (non-`attacker_side`)
    /// infantry unit in `hex` (a strafing result). Returns the points actually removed.
    pub fn strafe_infantry_loss(&mut self, hex: HexCoord, attacker_side: Side, points: u32) -> u32 {
        let victim = self.units.values().find_map(|u| {
            if u.hex() != Some(hex) || u.nationality.side() == attacker_side {
                return None;
            }
            let is_inf = self
                .reference
                .unit_characteristics
                .get(&u.nationality)
                .and_then(|m| m.get(&u.id_code))
                .is_some_and(|uc| uc.unit_type.contains("Infantry"));
            (is_inf && matches!(u.composition, Composition::Organic { .. })).then_some(u.id)
        });
        let Some(id) = victim else { return 0 };
        match &mut self.units.get_mut(&id).unwrap().composition {
            Composition::Organic { current_toe } => {
                let removed = points.min(*current_toe);
                *current_toe -= removed;
                removed
            }
            _ => 0,
        }
    }

    /// §42.44: whether `hex` is a "recognizable land feature" for airdrop targeting — a major city,
    /// a road or railroad hex (any state), a village, an air facility, or an oasis.
    pub fn is_recognizable_land_feature(&self, hex: HexCoord) -> bool {
        use crate::state::map::{Feature, HexsideKind};
        if self.map.terrain(hex) == Some(HexTerrain::MajorCity) {
            return true;
        }
        let feats = self.map.features(hex);
        if feats.contains(&Feature::Village) || feats.contains(&Feature::Oasis) {
            return true;
        }
        if self.air_facilities.iter().any(|f| f.hex == hex) {
            return true;
        }
        // A road (finished/unfinished/destroyed track) or railroad hexside touches the hex.
        self.map.neighbors(hex).any(|n| {
            self.map.hexsides_on(hex, n).iter().any(|h| {
                matches!(
                    h.kind,
                    HexsideKind::Road
                        | HexsideKind::UnfinishedRoad
                        | HexsideKind::Track
                        | HexsideKind::Railroad
                        | HexsideKind::UnfinishedRailroad
                )
            })
        })
    }

    /// §42.43/§42.44: whether `hex` is within two hexes of any recognizable land feature.
    pub fn within_two_of_land_feature(&self, hex: HexCoord) -> bool {
        if self.is_recognizable_land_feature(hex) {
            return true;
        }
        for n in self.map.neighbors(hex) {
            if self.is_recognizable_land_feature(n) {
                return true;
            }
            for n2 in self.map.neighbors(n) {
                if self.is_recognizable_land_feature(n2) {
                    return true;
                }
            }
        }
        false
    }

    /// §40.62: strafe the first-line (attached) Truck Points of the first enemy unit in `hex`,
    /// eliminating up to `points` Truck Points (Light → Medium → Heavy) and the cargo they carried,
    /// the cargo loss spread as evenly as possible across commodities (proportional to the trucks
    /// lost). Returns the Truck Points removed.
    pub fn strafe_first_line_trucks(&mut self, hex: HexCoord, attacker_side: Side, points: u32) -> u32 {
        let victim = self.units.values().find_map(|u| {
            let t = &u.trucks;
            (u.hex() == Some(hex)
                && u.nationality.side() != attacker_side
                && t.light + t.medium + t.heavy > 0)
                .then_some(u.id)
        });
        let Some(id) = victim else { return 0 };
        let u = self.units.get_mut(&id).unwrap();
        let total_before = u.trucks.light + u.trucks.medium + u.trucks.heavy;
        let mut removed = points.min(total_before);
        let taken = removed;
        for field in [&mut u.trucks.light, &mut u.trucks.medium, &mut u.trucks.heavy] {
            let n = removed.min(*field);
            *field -= n;
            removed -= n;
        }
        // Cargo the destroyed trucks were carrying is lost in proportion (§40.62 "evenly").
        // The `> 0` guard both skips the loss entirely and protects the division.
        #[allow(clippy::manual_checked_ops)]
        if total_before > 0 {
            for c in [Commodity::Fuel, Commodity::Ammo, Commodity::Stores, Commodity::Water] {
                let have = u.truck_cargo.get(c);
                *u.truck_cargo.get_mut(c) = have - have * taken / total_before;
            }
        }
        taken
    }

    /// §40.62: strafe an enemy free Truck Convoy (§8.98) in `hex` — remove up to `points` Truck
    /// Points, and the hauled cargo in proportion. Returns the Truck Points eliminated. (First-line
    /// attached trucks are the separate `strafe_first_line_trucks` target; a convoy is the unattached
    /// on-map truck stack.)
    pub fn strafe_convoy_trucks(&mut self, hex: HexCoord, attacker_side: Side, points: u32) -> u32 {
        let Some(conv) = self.truck_convoys.iter_mut().find(|c| {
            c.hex == hex
                && c.side != attacker_side
                && (c.trucks.light + c.trucks.medium + c.trucks.heavy) > 0
        }) else {
            return 0;
        };
        let total_before = conv.trucks.light + conv.trucks.medium + conv.trucks.heavy;
        let taken = points.min(total_before);
        let mut removed = taken;
        for field in [&mut conv.trucks.light, &mut conv.trucks.medium, &mut conv.trucks.heavy] {
            let n = removed.min(*field);
            *field -= n;
            removed -= n;
        }
        // Cargo the destroyed trucks were hauling is lost in proportion (§40.62 "as evenly as
        // possible ... between the various types of cargo"). `total_before` is ≥1 (the convoy was
        // found by requiring Truck Points present), so the division is safe.
        for c in [Commodity::Fuel, Commodity::Ammo, Commodity::Stores, Commodity::Water] {
            let have = conv.cargo.get(c);
            *conv.cargo.get_mut(c) = have - have * taken / total_before;
        }
        taken
    }

    /// §40.65: strafe an enemy armored unit in `hex` — roll `dice` d6; their sum is the minimum
    /// Armour Protection Points that must be removed. TOE weapon points (each contributing its
    /// system's armour-protection rating) are removed from the first enemy unit holding an
    /// armour-protection-rated weapon until the removed points' armour protection meets that sum.
    /// Returns the TOE points removed. (Callers must confirm the strafer is armour-capable, §40.65.)
    pub fn strafe_tank_loss(&mut self, hex: HexCoord, attacker_side: Side, dice: u32) -> u32 {
        let armor_of = |nat: Nationality, system: &str| -> u32 {
            self.reference
                .weapons_systems
                .get(&nat)
                .and_then(|m| m.get(system))
                .and_then(|w| w.armor_protection)
                .map(|r| r.value.max(0) as u32)
                .unwrap_or(0)
        };
        let victim = self.units.values().find_map(|u| {
            if u.hex() != Some(hex) || u.nationality.side() == attacker_side {
                return None;
            }
            let armored = matches!(&u.composition, Composition::Weapons { groups }
                if groups.iter().any(|g| armor_of(u.nationality, &g.system) > 0));
            armored.then_some(u.id)
        });
        let Some(id) = victim else { return 0 };
        let target_armor: u32 = (0..dice).map(|_| self.rng.d6() as u32).sum();
        let nat = self.units.get(&id).unwrap().nationality;
        let mut removed_armor = 0;
        let mut removed_points = 0;
        if let Composition::Weapons { groups } = &mut self.units.get_mut(&id).unwrap().composition {
            for g in groups.iter_mut() {
                let per = armor_of(nat, &g.system);
                if per == 0 {
                    continue; // only armour-protection-rated points are removed (§40.65)
                }
                while g.count > 0 && removed_armor < target_armor {
                    g.count -= 1;
                    removed_points += 1;
                    removed_armor += per;
                }
            }
            groups.retain(|g| g.count > 0);
        }
        removed_points
    }

    /// §40.6: strafe grounded aircraft — roll `dice` d6 and destroy their sum in planes from the
    /// first enemy (non-`attacker_side`) SGSU in `hex`. Returns the planes destroyed.
    pub fn strafe_grounded_aircraft_loss(&mut self, hex: HexCoord, attacker_side: Side, dice: u32) -> u32 {
        let victim = self.sgsus.values().find_map(|s| {
            (s.location == Some(hex) && s.nationality.side() != attacker_side && s.plane_count() > 0)
                .then_some(s.id)
        });
        let Some(id) = victim else { return 0 };
        let sum: u32 = (0..dice).map(|_| self.rng.d6() as u32).sum();
        self.sgsus.get_mut(&id).map_or(0, |s| s.lose_planes(sum))
    }

    /// §40.15/§40.16: the pilot Rating a squadron's flight adds to its planes' TacAir in air-to-air
    /// combat — the best *effective* Rating among the squadron's currently-flyable assigned pilots
    /// (§40.12), or 0 if it has none (an unpiloted plane flies with a zero-rated pilot, §40.16).
    /// A pilot's effective Rating is the greater of his assigned Rating and his §40.18 ace Rating
    /// earned from kills.
    pub fn squadron_pilot_bonus(&self, squadron: SquadronId) -> u32 {
        self.pilots
            .iter()
            .filter(|p| p.squadron == Some(squadron) && p.may_fly())
            .map(|p| p.rating.max(crate::rules::air::ace_rating_from_kills(p.kills)))
            .max()
            .unwrap_or(0)
    }

    /// §40.15/§40.16: every flyable pilot's effective Rating for a squadron, sorted best-first. Each
    /// flown plane is crewed by one pilot (the player assigns his best pilots first, §40.16), and a
    /// plane's Pilot Rating is added to its own TacAir in that plane's air-to-air match-up (§45.24 —
    /// planes never pool their strength). Planes beyond the roster fly with an untracked 0-rated
    /// pilot. Used to give each match-up its crewing pilot rather than one flight-wide best.
    pub fn squadron_pilot_ratings(&self, squadron: SquadronId) -> Vec<u32> {
        let mut r: Vec<u32> = self
            .pilots
            .iter()
            .filter(|p| p.squadron == Some(squadron) && p.may_fly())
            .map(|p| p.rating.max(crate::rules::air::ace_rating_from_kills(p.kills)))
            .collect();
        r.sort_unstable_by(|a, b| b.cmp(a));
        r
    }

    /// §40.18: credit `kills` air-to-air kills to a squadron's flying pilot — the same best pilot
    /// that flew the interception (`squadron_pilot_bonus`). A squadron with only zero-rated,
    /// untracked pilots (§40.16) has no pilot to credit, so the kills are dropped.
    pub fn credit_pilot_kills(&mut self, squadron: SquadronId, kills: u32) {
        if let Some(p) = self
            .pilots
            .iter_mut()
            .filter(|p| p.squadron == Some(squadron) && p.may_fly())
            .max_by_key(|p| p.rating.max(crate::rules::air::ace_rating_from_kills(p.kills)))
        {
            p.kills += kills;
        }
    }

    /// §40.3: attempt a defensive scramble against a mission over `target` by planes of
    /// §40.33: whether `hex` holds `side` fighters, for the scramble clear-path rule — a
    /// scramble-capable SGSU of `side` based there with ready planes, or a `side` Combat Air Patrol
    /// centred there.
    fn hex_has_fighters(&self, hex: HexCoord, side: Side) -> bool {
        let based = self.sgsus.values().any(|s| {
            s.nationality.side() == side
                && s.location == Some(hex)
                && s.planes.iter().any(|g| {
                    g.ready > 0
                        && self
                            .reference
                            .aircraft_charts
                            .get(&s.nationality)
                            .and_then(|c| c.get(&g.aircraft_type))
                            .is_some_and(|ac| {
                                crate::rules::air::can_scramble(&ac.missions, false)
                                    || crate::rules::air::can_scramble(&ac.missions, true)
                            })
                })
        });
        based || self.cap_patrols.iter().any(|p| p.side == side && p.center == hex)
    }

    /// §40.33: whether a scramble from `base` to `target` has a clear flight path — some route (the
    /// player may pick "any path", §37.12, so a modest detour off the straight line is allowed) that
    /// enters no hex holding enemy (`enemy_fighter_side`) fighters, except the target itself. The
    /// scrambling squadron's own base is the start and is exempt.
    fn scramble_path_clear(&self, base: HexCoord, target: HexCoord, enemy_fighter_side: Side) -> bool {
        if base == target {
            return true;
        }
        let dist = base.distance(target);
        let budget = dist + 2; // allow a small detour around a fighter screen
        let mut seen = std::collections::HashSet::new();
        let mut stack = vec![base];
        while let Some(h) = stack.pop() {
            if !seen.insert(h) {
                continue;
            }
            for n in self.map.neighbors(h) {
                if n == target {
                    return true;
                }
                if seen.contains(&n) || base.distance(n) > budget {
                    continue;
                }
                // §40.33: the path may not include a hex containing enemy fighters.
                if self.hex_has_fighters(n, enemy_fighter_side) {
                    continue;
                }
                stack.push(n);
            }
        }
        false
    }

    /// `bomber_side`. The first enemy fighter squadron within Scramble-Table range (§40.4) that
    /// has ready, scramble-capable planes and a §40.33 clear flight path to the target rolls a d6;
    /// on success its ready fighters scramble (fly) and return as the scrambling squadron and its
    /// intercepting [`Flight`]. `None` if no squadron scrambles.
    pub fn scramble_interception(&mut self, target: HexCoord, bomber_side: Side, night: bool) -> Option<(SquadronId, Flight)> {
        let table = self.reference.scramble_table.clone()?;
        let enemy = bomber_side.opponent();
        // First eligible enemy fighter squadron: within scramble range, with a ready scramble-capable
        // type, and a §40.33 clear path to the target (avoiding hexes holding the bombers' fighters).
        let mut chosen: Option<(SquadronId, u32)> = None;
        for s in self.sgsus.values() {
            if s.nationality.side() != enemy {
                continue;
            }
            let Some(base) = s.location else { continue };
            let dist = base.distance(target);
            if table.success_die_max(dist).is_none() {
                continue;
            }
            let chart = self.reference.aircraft_charts.get(&s.nationality);
            let capable = s.planes.iter().any(|g| {
                g.ready > 0
                    && chart
                        .and_then(|c| c.get(&g.aircraft_type))
                        .is_some_and(|ac| crate::rules::air::can_scramble(&ac.missions, night))
            });
            if capable && self.scramble_path_clear(base, target, bomber_side) {
                chosen = Some((s.id, dist));
                break;
            }
        }
        let (id, dist) = chosen?;
        // §40.33: roll a d6; scramble succeeds if it is within the distance threshold.
        let die = self.rng.d6() as u32;
        if !table.scramble_succeeds(dist, die) {
            return None;
        }
        // Representative TacAir/Maneuver from a ready scramble-capable type, and the ready count.
        let (tacair, maneuver, ready) = {
            let s = self.sgsus.get(&id)?;
            let chart = self.reference.aircraft_charts.get(&s.nationality);
            let rep = s
                .planes
                .iter()
                .filter(|g| g.ready > 0)
                .find_map(|g| chart.and_then(|c| c.get(&g.aircraft_type)))
                .and_then(|ac| ac.loadouts.first())
                .map(|l| (l.tac_air, l.maneuver))
                .unwrap_or((0, 0));
            (rep.0, rep.1, s.ready_planes())
        };
        let flown = self.sgsus.get_mut(&id)?.fly(ready);
        if flown == 0 {
            return None;
        }
        Some((id, Flight { count: flown, tacair, pilot: self.squadron_pilot_bonus(id), maneuver }))
    }

    /// Apply the §41.3x land-support bombing effect of a mission whose Air Bombardment CRT result
    /// is `result`, against `target` for a bomber of `bomber_side`:
    /// - **Personnel (§41.31):** Pin up to `result` battalion-equivalents in the hex (each tank or
    ///   gun — artillery/anti-tank — unit costs two of the count).
    /// - **Supply Dump (§41.35):** destroy `result` percent of every commodity in the target's
    ///   enemy Supply Dumps.
    /// - **Flak Destruction (§41.33):** remove `result` AA-rated TOE from an enemy AA unit.
    /// - **Truck Convoy (§41.32):** destroy `result` first-line Truck Points (D-capability planes).
    /// - **City/Fortification (§41.37):** knock a *placed* Fortification Level down one.
    /// - **Road / Railroad (§41.38):** destroy the road (Road→UnfinishedRoad, rebuildable §24.53) or
    ///   railroad (all Railroad edges of the hex → DestroyedRailroad, rebuildable §24.63) on `result`≥1.
    ///
    /// Ships/air facilities have dedicated commands (§41.34/§41.36); static §25.12 major-city levels
    /// are not reducible (no damage tracker). Mining-harbor is resolved on its own table.
    pub fn apply_land_bombing_effect(
        &mut self,
        mission: crate::reference::aircraft::BombingMission,
        target: HexCoord,
        bomber_side: Side,
        result: u32,
    ) {
        use crate::reference::aircraft::BombingMission as M;
        let enemy = bomber_side.opponent();
        // §16.21: an air-bombed hex may not be reconnoitred this stage.
        self.barraged_hexes_this_stage.insert(target);
        match mission {
            M::Personnel => {
                // §41.31: units in a major city may not be bombed until its Fortification Level is
                // reduced to one or less.
                if self.map.terrain(target) == Some(crate::reference::terrain::HexTerrain::MajorCity)
                    && self.fort_level(target) > 1
                {
                    return;
                }
                let mut budget = result;
                let ids: Vec<UnitId> = self
                    .units_in_hex(target)
                    .into_iter()
                    .filter(|&id| self.unit(id).is_some_and(|u| u.nationality.side() == enemy))
                    .collect();
                for id in ids {
                    let cost = if self.unit_is_tank_or_gun(id) { 2 } else { 1 };
                    if budget < cost {
                        continue; // a tank/gun needs two of the pin count (§41.31)
                    }
                    if let Some(u) = self.unit_mut(id) {
                        u.status.pinned = true;
                    }
                    budget -= cost;
                }
            }
            M::SupplyDump => {
                let pct = result.min(100);
                for d in self.supply_dumps.iter_mut().filter(|d| {
                    d.side == enemy && !d.dummy && d.location.hex() == Some(target)
                }) {
                    for c in [Commodity::Fuel, Commodity::Ammo, Commodity::Stores, Commodity::Water] {
                        let have = d.supply.get(c);
                        *d.supply.get_mut(c) = have - have * pct / 100;
                    }
                }
            }
            M::FlakDestruction => {
                // §41.33: destroy `result` TOE Strength Points that carry an Anti-Air rating, from
                // the first enemy AA-rated unit in the hex.
                self.destroy_flak_toe(target, enemy, result);
            }
            M::TruckConvoy => {
                // §41.32: destroy `result` first-line (attached) Truck Points of an enemy unit in
                // the hex, cargo lost in proportion (same mechanic as strafing, §40.62). The D-
                // capability gate is enforced at the command. (Hex-placed convoys are not modeled.)
                self.strafe_first_line_trucks(target, bomber_side, result);
            }
            M::CityFortification => {
                // §41.37: at most one Fortification Level destroyed per Operations Stage. Knock down
                // whichever level is currently effective — a placed fort if it is at least as high as
                // the (possibly already-damaged) static city level, otherwise the §25.12 city level.
                if result >= 1 {
                    let placed = self.fortifications.get(&target).copied().unwrap_or(0);
                    let city_base = self.city_static_fort_base(target);
                    let city_eff = city_base.saturating_sub(self.city_fort_damage.get(&target).copied().unwrap_or(0));
                    if placed > 0 && placed >= city_eff {
                        if let Some(l) = self.fortifications.get_mut(&target) {
                            *l = l.saturating_sub(1);
                        }
                    } else if city_eff > 0 {
                        *self.city_fort_damage.entry(target).or_insert(0) += 1;
                    }
                }
            }
            M::Road if result >= 1 => {
                // §41.38: a positive result destroys the road hex. Roads are permanent map
                // features, so "destroyed" degrades each Road edge of the hex to an UnfinishedRoad
                // (a track, §24.52) that engineers may re-pave via `build_road` (§24.53).
                let neighbors: Vec<HexCoord> = self.map.neighbors(target).collect();
                for n in neighbors {
                    if self.map.hexsides_on(target, n).iter().any(|h| h.kind == HexsideKind::Road) {
                        self.map.remove_hexside(target, n, HexsideKind::Road);
                        self.map.set_hexside(target, n, HexsideKind::UnfinishedRoad, None);
                    }
                }
            }
            M::Railroad if result >= 1 => {
                // §41.38: a positive result destroys the railroad hex — every built Railroad edge
                // of the hex becomes DestroyedRailroad (inert until rebuilt by engineers, §24.63).
                let neighbors: Vec<HexCoord> = self.map.neighbors(target).collect();
                for n in neighbors {
                    self.destroy_railroad(target, n);
                }
            }
            _ => {}
        }
    }

    /// §19.5: whether a unit is a **tank** unit — a Weapons unit holding a Tank weapon system.
    pub fn unit_is_tank(&self, id: UnitId) -> bool {
        use crate::reference::units::WeaponKind;
        let Some(u) = self.units.get(&id) else { return false };
        let Composition::Weapons { groups } = &u.composition else { return false };
        groups.iter().any(|g| {
            self.reference.weapons_systems.get(&u.nationality).and_then(|m| m.get(&g.system)).map(|w| w.kind) == Some(WeaponKind::Tank)
        })
    }

    /// §19.5: whether a unit is an **infantry**-type unit (Organic personnel, or an Infantry
    /// characteristics type).
    pub fn unit_is_infantry(&self, id: UnitId) -> bool {
        let Some(u) = self.units.get(&id) else { return false };
        if matches!(u.composition, Composition::Organic { .. }) {
            return true;
        }
        self.reference
            .unit_characteristics
            .get(&u.nationality)
            .and_then(|m| m.get(&u.id_code))
            .is_some_and(|c| c.unit_type.contains("Infantry"))
    }

    /// §19.5/§20.2 "gun class": a Weapons unit holding an Artillery / Anti-Tank / Anti-Air system
    /// and no Tank system (the §20.3 "Gun = Artillery, anti-tank or anti-air" pool class).
    pub fn unit_is_gun(&self, id: UnitId) -> bool {
        use crate::reference::units::WeaponKind;
        if self.unit_is_tank(id) {
            return false;
        }
        let Some(u) = self.units.get(&id) else { return false };
        let Composition::Weapons { groups } = &u.composition else { return false };
        groups.iter().any(|g| {
            matches!(
                self.reference
                    .weapons_systems
                    .get(&u.nationality)
                    .and_then(|m| m.get(&g.system))
                    .map(|w| w.kind),
                Some(WeaponKind::Artillery | WeaponKind::AntiTank | WeaponKind::AntiAir)
            )
        })
    }

    /// Count the non-assigned units transitively attached to `parent` that satisfy `pred` — the
    /// working set for the §19.5 per-type maxima.
    fn count_nonassigned_attached(&self, parent: UnitId, pred: impl Fn(&Self, UnitId) -> bool) -> u32 {
        self.units
            .keys()
            .filter(|&&id| {
                id != parent
                    && self.units.get(&id).and_then(|u| u.assigned_parent) != Some(parent)
                    && self.attaches_up_to(id, parent)
                    && pred(self, id)
            })
            .count() as u32
    }

    /// Whether a unit is a tank or gun (artillery/anti-tank) unit — a Weapons unit holding at least
    /// one Tank/Artillery/Anti-Tank weapon system (counts double for §41.31 bombing pins).
    fn unit_is_tank_or_gun(&self, id: UnitId) -> bool {
        use crate::reference::units::WeaponKind;
        let Some(u) = self.units.get(&id) else { return false };
        let Composition::Weapons { groups } = &u.composition else { return false };
        groups.iter().any(|g| {
            matches!(
                self.reference
                    .weapons_systems
                    .get(&u.nationality)
                    .and_then(|m| m.get(&g.system))
                    .map(|w| w.kind),
                Some(WeaponKind::Tank | WeaponKind::Artillery | WeaponKind::AntiTank)
            )
        })
    }

    /// §41.33: destroy up to `points` TOE Strength Points carrying an Anti-Air rating from the
    /// first `victim_side` unit in `hex` that has AA — an Organic unit whose characteristics carry
    /// an anti-air rating (whole-unit AA), or the anti-air weapon groups of a Weapons unit. Returns
    /// the TOE points removed.
    pub fn destroy_flak_toe(&mut self, hex: HexCoord, victim_side: Side, points: u32) -> u32 {
        use crate::reference::units::WeaponKind;
        let is_aa_system = |nat: Nationality, system: &str| -> bool {
            matches!(
                self.reference.weapons_systems.get(&nat).and_then(|m| m.get(system)).map(|w| w.kind),
                Some(WeaponKind::AntiAir | WeaponKind::LightAntiAir | WeaponKind::HeavyAntiAir)
            )
        };
        let victim = self.units.values().find_map(|u| {
            if u.hex() != Some(hex) || u.nationality.side() != victim_side {
                return None;
            }
            let has_aa = match &u.composition {
                Composition::Organic { .. } => self
                    .reference
                    .unit_characteristics
                    .get(&u.nationality)
                    .and_then(|m| m.get(&u.id_code))
                    .and_then(|c| c.anti_air)
                    .is_some_and(|r| r.value > 0),
                Composition::Weapons { groups } => groups.iter().any(|g| is_aa_system(u.nationality, &g.system)),
            };
            has_aa.then_some(u.id)
        });
        let Some(id) = victim else { return 0 };
        let nat = self.units.get(&id).unwrap().nationality;
        match &mut self.units.get_mut(&id).unwrap().composition {
            Composition::Organic { current_toe } => {
                let removed = points.min(*current_toe);
                *current_toe -= removed;
                removed
            }
            Composition::Weapons { groups } => {
                let mut budget = points;
                let mut removed = 0;
                for g in groups.iter_mut() {
                    if budget == 0 {
                        break;
                    }
                    if !is_aa_system(nat, &g.system) {
                        continue;
                    }
                    let n = budget.min(g.count);
                    g.count -= n;
                    budget -= n;
                    removed += n;
                }
                groups.retain(|g| g.count > 0);
                removed
            }
        }
    }

    /// §37.4/§36.53: air distance in hexes from a Mediterranean Box to a `target` hex, via the §37.4
    /// Air Distance Chart. The chart lists Box→named-point distances; an arbitrary target is reached
    /// by routing through the nearest on-map named reference point (chart distance to that point +
    /// hex distance from it to the target), skipping Prohibited routes. Italy/Sicily takes the more
    /// favourable of the two columns (the region is modeled as one, §43.12). `None` if no route
    /// exists (all references prohibited) or the chart is not loaded.
    pub fn box_flight_distance(&self, med_base: crate::state::air::MedBase, target: HexCoord) -> Option<u32> {
        use crate::reference::air_distance::AirDist;
        use crate::state::air::MedBase;
        let chart = self.reference.air_distance.as_ref()?;
        let sources: &[&str] = match med_base {
            MedBase::Crete => &["Crete"],
            MedBase::ItalySicily => &["Sicily", "Italy"],
        };
        // The §37.4 named targets that have a known on-map hex, used as routing references.
        const REFS: [(&str, &str); 7] = [
            ("Benghazi", "A4827"), ("Derna", "B5925"), ("Tobruk", "C4807"), ("Bardia", "C4321"),
            ("Mersa Matruh", "D3714"), ("Alexandria", "E3613"), ("Nofilia", "A2703"),
        ];
        let mut best: Option<u32> = None;
        for (name, hexid) in REFS {
            let Some(rhex) = HexId::parse(hexid).map(|h| h.to_hex_coord()) else { continue };
            let base_dist = sources
                .iter()
                .filter_map(|s| match chart.distance(s, name) {
                    Some(AirDist::Hexes(n)) => Some(n),
                    _ => None,
                })
                .min();
            if let Some(bd) = base_dist {
                let total = bd + rhex.distance(target);
                best = Some(best.map_or(total, |b| b.min(total)));
            }
        }
        best
    }

    /// §37.12: a squadron's flight distance to `target` — hex distance from its on-map base, or the
    /// §37.4 chart distance from its off-map Box. `None` if unbased or no route exists.
    pub fn flight_distance(&self, squadron: SquadronId, target: HexCoord) -> Option<u32> {
        let s = self.sgsus.get(&squadron)?;
        match s.location {
            Some(hex) => Some(hex.distance(target)),
            None => self.box_flight_distance(s.med_base?, target),
        }
    }

    /// The on-map hex whose weather grounds a squadron's flight (§37.22), or `None` for an off-map
    /// Box base (no on-map weather).
    pub fn flight_base_weather_hex(&self, squadron: SquadronId) -> Option<HexCoord> {
        self.sgsus.get(&squadron).and_then(|s| s.location)
    }

    /// §43.23: whether the current Operations Stage falls in the Crete→Suez commitment window — the
    /// first four of a month's twelve Operations Stages, during which no bombing missions may be
    /// flown from Crete (the Suez raids themselves have no game effect). A fixed-window model of the
    /// rule's "four OpStages within a given month" (a month = four Game-Turns).
    pub fn crete_suez_window(&self) -> bool {
        let week_of_month = self.game_turn.saturating_sub(1) % 4; // 0..3
        let opstage_of_month = week_of_month * 3 + self.operations_stage as u32; // 1..12
        opstage_of_month <= 4
    }

    /// Whether a squadron has any flight base (on-map or an off-map Box).
    pub fn squadron_is_based(&self, squadron: SquadronId) -> bool {
        self.sgsus
            .get(&squadron)
            .is_some_and(|s| s.location.is_some() || s.med_base.is_some())
    }

    /// §43.0: total planes based in a Mediterranean Box (`region`) across all Axis squadrons — the
    /// Italy/Sicily count is the in-play pool a Malta raid draws on (§44.25).
    pub fn med_based_planes(&self, region: crate::state::air::MedBase) -> u32 {
        self.sgsus
            .values()
            .filter(|s| s.med_base == Some(region) && s.nationality.side() == Side::Axis)
            .map(|s| s.plane_count())
            .sum()
    }

    /// §43.11–13: whether German air basing meets the date-appropriate requirements. Always: §43.11
    /// — at least 75% of the strategic bombers (He.111 / Ju.88D / Fw.200) are in a Mediterranean Box.
    /// Before Game-Turn 35 (June 1941): §43.12 — at least 75% of *all* German bombers are in the
    /// Italy/Sicily Box. From Game-Turn 35: §43.13 — at least 50% of the strategic bombers are in the
    /// Crete Box. `true` when the relevant totals are zero.
    pub fn german_basing_ok(&self) -> bool {
        use crate::reference::aircraft::AircraftClass;
        use crate::state::air::MedBase;
        let is_strat = |t: &str| matches!(t, "He.111" | "Ju.88D" | "Fw.200 C");
        let (mut strat_total, mut strat_med, mut strat_crete) = (0u32, 0u32, 0u32);
        let (mut bomb_total, mut bomb_italy_sicily) = (0u32, 0u32);
        for s in self.sgsus.values().filter(|s| s.nationality == Nationality::German) {
            for g in &s.planes {
                if is_strat(&g.aircraft_type) {
                    strat_total += g.count;
                    if s.med_base.is_some() {
                        strat_med += g.count;
                    }
                    if s.med_base == Some(MedBase::Crete) {
                        strat_crete += g.count;
                    }
                }
                let is_bomber = matches!(
                    self.reference
                        .aircraft_charts
                        .get(&Nationality::German)
                        .and_then(|c| c.get(&g.aircraft_type))
                        .map(|a| a.class),
                    Some(AircraftClass::Bomber | AircraftClass::DiveBomber)
                );
                if is_bomber {
                    bomb_total += g.count;
                    if s.med_base == Some(MedBase::ItalySicily) {
                        bomb_italy_sicily += g.count;
                    }
                }
            }
        }
        // §43.11 (always): ≥75% of strategic bombers in a Med Box.
        if strat_total > 0 && strat_med * 100 < strat_total * 75 {
            return false;
        }
        if self.game_turn < 35 {
            // §43.12 (pre-June 1941): ≥75% of all German bombers in Italy/Sicily.
            if bomb_total > 0 && bomb_italy_sicily * 100 < bomb_total * 75 {
                return false;
            }
        } else {
            // §43.13 (from June 1941): ≥50% of strategic bombers in Crete.
            if strat_total > 0 && strat_crete * 100 < strat_total * 50 {
                return false;
            }
        }
        true
    }

    /// The result of resolving a bombing mission (§39.0 procedure).
    #[allow(clippy::too_many_arguments)]
    pub fn resolve_bombing_mission(
        &mut self,
        squadron: SquadronId,
        planes: u32,
        bombload_per_plane: u32,
        group: crate::reference::air_bombardment::TargetGroup,
        interceptors: Option<Flight>,
        escort: Option<Flight>,
        flak_points: u32,
        flak_destroy_roll: u32,
        flak_abort_roll: u32,
        bomb_roll: u32,
        night: bool,
        interceptor_pilots: &[u32],
        escort_pilots: &[u32],
    ) -> Option<BombingMissionOutcome> {
        use crate::reference::air_bombardment::WeaponPoints;
        use crate::reference::anti_aircraft::AaSection;
        // §39.0(a): the ready planes fly to the target.
        let flown = self.sgsus.get_mut(&squadron)?.fly(planes);
        if flown == 0 {
            return Some(BombingMissionOutcome::default());
        }
        let mut alive = flown;
        // §39.0(c/d) / §45: intercepting CAP attacks the bombers before flak. The bombers defend
        // with the TacAir/Maneuver of their type (§41.15).
        let mut intercepted = 0;
        let mut escort_losses = 0;
        if let Some(cap) = interceptors {
            // §45: fighter-vs-escort combat first — only interceptors surviving the escort/screen
            // may then attack the bombers.
            let mut attackers = cap;
            let mut surviving_escort = 0;
            if let Some(esc) = escort {
                // §45.2: fighter-vs-fighter — the escort and CAP fight first (one-way pile-on).
                if let Some(a2a) = self.resolve_air_to_air_pilots(cap, esc, interceptor_pilots, escort_pilots, false) {
                    escort_losses = a2a.defender_losses.min(esc.count);
                    let cap_lost = a2a.attacker_losses.min(cap.count);
                    attackers = Flight { count: cap.count - cap_lost, ..cap };
                    surviving_escort = esc.count - escort_losses;
                }
            }
            // §45.32: only the CAP's *excess* over the surviving screen may attack the bombers.
            attackers = Flight { count: attackers.count.saturating_sub(surviving_escort), ..attackers };
            if attackers.count > 0 {
                let (b_tacair, b_maneuver) = {
                    let atype = self.sgsus.get(&squadron)?.planes.first().map(|g| g.aircraft_type.clone());
                    atype
                        .and_then(|t| {
                            self.reference
                                .aircraft_charts
                                .get(&self.sgsus.get(&squadron)?.nationality)
                                .and_then(|c| c.get(&t))
                                .and_then(|a| a.loadouts.first())
                                .map(|l| (l.tac_air, l.maneuver))
                        })
                        .unwrap_or((0, 0))
                };
                let bombers = Flight { count: alive, tacair: b_tacair, pilot: 0, maneuver: b_maneuver };
                // §45.3/§45.34: fighter-vs-screened bombers — mutual pile-on (bombers return fire).
                if let Some(a2a) = self.resolve_air_to_air_pilots(attackers, bombers, interceptor_pilots, &[], true) {
                    intercepted = a2a.defender_losses.min(alive);
                    if intercepted > 0 {
                        self.sgsus.get_mut(&squadron)?.lose_planes(intercepted);
                    }
                    alive -= intercepted;
                }
            }
        }
        // §46 / §39.0(e): anti-aircraft fire — planes destroyed and aborted (two rolls, §46.0).
        let (mut destroyed, mut aborted) = (0u32, 0u32);
        if flak_points > 0 && alive > 0 {
            if let Some(aa) = self.reference.anti_aircraft.as_ref() {
                let shift = if night { crate::rules::air::night_flak_column_shift() } else { 0 };
                destroyed = aa.result(AaSection::OtherDestroyed, flak_points, shift, flak_destroy_roll).max(0) as u32;
                aborted = aa.result(AaSection::OtherAborted, flak_points, shift, flak_abort_roll).max(0) as u32;
            }
        }
        destroyed = destroyed.min(alive);
        aborted = aborted.min(alive - destroyed);
        if destroyed > 0 {
            self.sgsus.get_mut(&squadron)?.lose_planes(destroyed);
        }
        // §39.0(f): the survivors bomb the target.
        let surviving = alive - destroyed - aborted;
        let mut delivered = surviving * bombload_per_plane;
        if night {
            delivered = crate::rules::air::night_bombload(delivered);
        }
        let result = self
            .reference
            .air_bombardment
            .as_ref()
            .map_or(0, |t| t.result(group, WeaponPoints::Bomb, delivered, bomb_roll));
        Some(BombingMissionOutcome { flown, intercepted, destroyed, aborted, delivered_bomb_points: delivered, result, escort_losses })
    }

    /// §45: resolve air-to-air combat between an `attacker` and `defender` flight as **individual
    /// plane-vs-plane match-ups** (§45.24 — "no planes ever add their strengths together"). Each
    /// plane carries its own Pilot Rating (§40.15/§45.24): `attacker_pilots`/`defender_pilots` are
    /// the per-plane ratings, best-first (a shorter slice means the remaining planes fly 0-rated; an
    /// **empty** slice falls back to the flight's single `pilot` for every plane — for callers that
    /// have not threaded a roster). The larger side covers every plane of the smaller side with one
    /// plane, then its extras pile on (§45.23): a piled-on plane fires one-way (the target already
    /// fired at its primary attacker, §45.24). Each match-up rolls its own 2d6 on the §45.4/§45.5
    /// tables. Returns the losses each side takes (not applied; the caller removes them via
    /// `Sgsu::lose_planes`). `None` if the tables are unloaded.
    pub fn resolve_air_to_air(&mut self, attacker: Flight, defender: Flight) -> Option<AirToAirOutcome> {
        self.resolve_air_to_air_pilots(attacker, defender, &[], &[], false)
    }

    /// [`resolve_air_to_air`] with explicit per-plane pilot rosters (best-first) and a `screened`
    /// flag distinguishing the two air-to-air waves. In **fighter-vs-fighter** (`screened == false`,
    /// §45.2) a piled-on attacker fires one-way (the defender fired at its primary attacker, §45.24)
    /// and either side may pile on. In **fighter-vs-screened non-fighters** (`screened == true`,
    /// §45.3) only the attacker (fighters) piles on and every attacker-vs-bomber pairing is mutual —
    /// a screened bomber returns fire at each fighter attacking it (§45.34); the bombers never
    /// initiate a surplus. See [`resolve_air_to_air`].
    pub fn resolve_air_to_air_pilots(
        &mut self,
        attacker: Flight,
        defender: Flight,
        attacker_pilots: &[u32],
        defender_pilots: &[u32],
        screened: bool,
    ) -> Option<AirToAirOutcome> {
        self.reference.air_to_air.as_ref()?; // require the tables
        // Per-plane pilot rating: an empty roster falls back to the flight-wide best `pilot`.
        let pilot_at = |slice: &[u32], i: u32, uniform: u32| -> u32 {
            if slice.is_empty() { uniform } else { slice.get(i as usize).copied().unwrap_or(0) }
        };
        // The differential for attacker plane `ai` vs defender plane `di` (their own pilots).
        let diff = |g: &Self, ai: u32, di: u32| -> i32 {
            let ap = pilot_at(attacker_pilots, ai, attacker.pilot);
            let dp = pilot_at(defender_pilots, di, defender.pilot);
            g.reference.air_to_air.as_ref().unwrap().differential(
                attacker.tacair + ap as i32,
                defender.tacair + dp as i32,
                attacker.maneuver,
                defender.maneuver,
            )
        };
        let mut out = AirToAirOutcome::default();
        let (a, d) = (attacker.count, defender.count);
        if a == 0 || d == 0 {
            return Some(out);
        }
        let pairs = a.min(d);
        // Track which planes are already killed so a piled-on shot at a downed plane (§45.19) and a
        // primary duel do not double-count losses.
        let mut att_dead = vec![false; a as usize];
        let mut def_dead = vec![false; d as usize];
        // Primary match-ups: plane i of each side (mutual fire, §45.5 fire order).
        for i in 0..pairs {
            let r1 = self.rng.sequential_read() as u32;
            let r2 = self.rng.sequential_read() as u32;
            let ad = diff(self, i, i);
            let (ak, dk) = crate::rules::air::air_to_air_duel(
                self.reference.air_to_air.as_ref()?,
                ad,
                pilot_at(attacker_pilots, i, attacker.pilot),
                pilot_at(defender_pilots, i, defender.pilot),
                r1,
                r2,
            );
            if ak { att_dead[i as usize] = true; out.attacker_losses += 1; }
            if dk { def_dead[i as usize] = true; out.defender_losses += 1; }
        }
        // §45.23 pile-on: the larger side's extra planes each engage a covered target of the smaller
        // side (round-robin).
        if a > d {
            for k in d..a {
                let tgt = (k % d) as usize;
                if def_dead[tgt] { continue; }
                if screened {
                    // §45.34: the screened bomber returns fire at each fighter attacking it — a full
                    // mutual duel, so the piling-on fighter is also at risk.
                    let r1 = self.rng.sequential_read() as u32;
                    let r2 = self.rng.sequential_read() as u32;
                    let ad = diff(self, k, tgt as u32);
                    let (ak, dk) = crate::rules::air::air_to_air_duel(
                        self.reference.air_to_air.as_ref()?,
                        ad,
                        pilot_at(attacker_pilots, k, attacker.pilot),
                        pilot_at(defender_pilots, tgt as u32, defender.pilot),
                        r1,
                        r2,
                    );
                    if ak { att_dead[k as usize] = true; out.attacker_losses += 1; }
                    if dk { def_dead[tgt] = true; out.defender_losses += 1; }
                } else {
                    // §45.24: one-way — the fighter target already fired at its primary attacker.
                    let r = self.rng.sequential_read() as u32;
                    if self.reference.air_to_air.as_ref()?.is_kill(diff(self, k, tgt as u32), r) {
                        def_dead[tgt] = true;
                        out.defender_losses += 1;
                    }
                }
            }
        } else if d > a && !screened {
            // Fighter-vs-fighter only (§45.24): the defender's surplus fighters pile on one-way.
            // Screened bombers never initiate a surplus (§45.33), so this is skipped when screened.
            for k in a..d {
                let tgt = (k % a) as usize;
                if att_dead[tgt] { continue; }
                let r = self.rng.sequential_read() as u32;
                // Defender plane `k` fires one-way at attacker `tgt`: the differential is from the
                // defender's perspective (negate the attacker-relative differential).
                if self.reference.air_to_air.as_ref()?.is_kill(-diff(self, tgt as u32, k), r) {
                    att_dead[tgt] = true;
                    out.attacker_losses += 1;
                }
            }
        }
        Some(out)
    }

    /// Whether `hex` is a legal placement for a player-choice Supply Unit of `side` under the
    /// constraint `c` (§59.51 / §60.92): on-map, on the required section, within the west/east
    /// bound, and clear of enemy units by the required margin.
    pub fn dump_choice_legal(&self, c: &DumpConstraint, side: Side, hex: HexCoord) -> bool {
        if self.map.terrain(hex).is_none() {
            return false;
        }
        if let Some(sec) = c.section {
            let hex_sec = crate::state::map::section_char_of_col(hex.col);
            if hex_sec != sec {
                return false;
            }
        }
        if let Some(ref wl) = c.west_of {
            if let Some(r) = HexId::parse(wl) {
                if hex.col >= r.to_hex_coord().col {
                    return false;
                }
            }
        }
        if let Some(ref el) = c.east_of {
            if let Some(r) = HexId::parse(el) {
                if hex.col <= r.to_hex_coord().col {
                    return false;
                }
            }
        }
        if let Some(n) = c.not_within_of_enemy {
            let enemy = side.opponent();
            let too_close = self.units.values().any(|u| {
                u.nationality.side() == enemy && u.hex().is_some_and(|h| h.distance(hex) <= n)
            });
            if too_close {
                return false;
            }
        }
        true
    }

    /// Place a pending player-choice dump group (by index) at `hex`, validating the
    /// constraint. On success, the Supply Units are added and the choice is consumed.
    pub fn place_dump_choice(
        &mut self,
        index: usize,
        hex: HexCoord,
    ) -> Result<u32, DumpPlacementError> {
        let choice = self
            .pending_dump_choices
            .get(index)
            .cloned()
            .ok_or(DumpPlacementError::NoSuchChoice)?;
        if !self.dump_choice_legal(&choice.constraint, choice.side, hex) {
            return Err(DumpPlacementError::IllegalHex);
        }
        for _ in 0..choice.count {
            self.supply_dumps.push(SupplyDump {
                location: UnitLocation::OnMap(hex),
                side: choice.side,
                supply: Commodities {
                    fuel: choice.fuel,
                    ammo: choice.ammo,
                    stores: choice.stores,
                    water: choice.water,
                },
                dummy: choice.dummy,
            });
        }
        self.pending_dump_choices.remove(index);
        Ok(choice.count)
    }

    /// Resolve every pending dump choice by placing it at the first legal hex found (arbitrary
    /// but always legal — for demos/tests). Real play and AI agents should instead call
    /// `place_dump_choice` with a deliberately chosen hex.
    pub fn auto_place_dump_choices(&mut self) {
        while !self.pending_dump_choices.is_empty() {
            let choice = self.pending_dump_choices[0].clone();
            let coords: Vec<HexCoord> = self.map.coords().collect();
            let hex = coords
                .into_iter()
                .find(|&h| self.dump_choice_legal(&choice.constraint, choice.side, h));
            match hex {
                Some(h) => {
                    let _ = self.place_dump_choice(0, h);
                }
                None => {
                    self.pending_dump_choices.remove(0);
                }
            }
        }
    }

    /// §64.73 supply-adequacy — whether a combat unit has enough of each commodity available at
    /// its hex to keep occupying it at game end: **Stores** and **Water** for a week, **Fuel** to
    /// move 20 CP, and **Ammo** to fire 3×. "Available" is the unit's own on-hand supply plus what
    /// it can draw at its hex — friendly non-dummy Supply Dumps for Fuel/Ammo/Stores, and a Water
    /// source (well/oasis/major city/connected pipeline) for Water. Off-map units in the
    /// Tripoli/Tunisia boxes are always adequately supplied (§8.88). This replaces the former
    /// §32.16 half-CPA dump-reachability proxy now that §49/§50/§51/§52 commodities are modeled.
    pub fn unit_supply_adequate(&self, id: UnitId) -> bool {
        let Some(u) = self.units.get(&id) else {
            return false;
        };
        // §27.74: the Layforce commando is automatically in supply for three Operations Stages after
        // an amphibious landing (its landing stage plus two more).
        if self.is_layforce(id) && self.layforce_free_supply_active() {
            return true;
        }
        // §27.84: a sea/air-transported SAS Brigade is in supply for three Operations Stages.
        if self.is_sas(id) && self.sas_free_supply_active() {
            return true;
        }
        // The Tripoli/Tunisia boxes are unlimited Supply Dumps (§8.88).
        if u.location.as_ref().is_some_and(|l| l.off_map_node().is_some()) {
            return true;
        }
        let Some(hex) = u.hex() else {
            return false;
        };
        let side = u.nationality.side();
        // Friendly, non-dummy dump commodities co-located with the unit (§49/§50/§51/§52 draw).
        let (dump_fuel, dump_ammo, dump_stores, dump_water) = self
            .supply_dumps
            .iter()
            .filter(|d| d.side == side && !d.dummy && d.location.hex() == Some(hex))
            .fold((0u32, 0u32, 0u32, 0u32), |(f, a, s, w), d| {
                (f + d.supply.fuel, a + d.supply.ammo, s + d.supply.stores, w + d.supply.water)
            });

        // Fuel to move 20 CP (200 tenths), §49.13: tanks + truck-cargo (§49.18) + dumps. Zero for
        // leg infantry (no fuel users).
        let fuel_need = self.unit_move_fuel_cost(id, 200);
        if u.fuel + u.truck_cargo.fuel + dump_fuel < fuel_need {
            return false;
        }
        // Ammo to fire 3× (§50.14): onboard reserve + dumps. Zero for plain rifle infantry (§50.2).
        let ammo_need = 3 * self.unit_combat_ammo_cost(id, CombatAction::CloseAssault);
        if u.ammo + dump_ammo < ammo_need {
            return false;
        }
        // Stores for a week (§51.11/13): units carry no field Stores, so this is the hex's dumps.
        if dump_stores < self.unit_stores_requirement(id) {
            return false;
        }
        // Water for a week (§52.4): a Water source at the hex (well/oasis/city/pipeline), else
        // enough carried on hand plus what co-located dumps hold.
        let water_need = self.unit_water_requirement(id, true);
        if self.hex_water_source_for(hex, side).is_none() && u.water + dump_water < water_need {
            return false;
        }
        true
    }

    /// Whether any hex satisfying `target` is reachable from `start` within `max_cost` tenths
    /// of a CP by medium-truck movement, never crossing impassable terrain or an enemy ZOC
    /// unoccupied by a friendly unit (the supply-line rules of §32.16 / §64.71). Pass a very
    /// large `max_cost` for an effectively unbounded connectivity trace ("in any way").
    fn truck_reachable(
        &self,
        start: HexCoord,
        side: Side,
        max_cost: i32,
        target: impl Fn(HexCoord) -> bool,
    ) -> bool {
        use std::cmp::Reverse;
        use std::collections::{BinaryHeap, HashMap};

        if target(start) {
            return true;
        }
        let enemy = side.opponent();
        let friendly_occupies = |hex: HexCoord| {
            self.units_in_hex(hex)
                .into_iter()
                .any(|x| self.unit(x).is_some_and(|xu| xu.nationality.side() == side))
        };
        let mut best: HashMap<HexCoord, i32> = HashMap::new();
        let mut pq: BinaryHeap<Reverse<(i32, HexCoord)>> = BinaryHeap::new();
        best.insert(start, 0);
        pq.push(Reverse((0, start)));
        while let Some(Reverse((cost, h))) = pq.pop() {
            if cost > max_cost {
                break;
            }
            if cost > *best.get(&h).unwrap_or(&i32::MAX) {
                continue;
            }
            for nb in self.map.neighbors(h) {
                if self.hex_in_zoc_of(nb, enemy) && !friendly_occupies(nb) {
                    continue;
                }
                let step = match step_cost(&self.reference, &self.map, h, nb, MoveMode::Motorized) {
                    Some(MoveCost::Cost(f)) => f.0,
                    _ => continue,
                };
                let nc = cost + step;
                if nc <= max_cost && nc < *best.get(&nb).unwrap_or(&i32::MAX) {
                    if target(nb) {
                        return true;
                    }
                    best.insert(nb, nc);
                    pq.push(Reverse((nc, nb)));
                }
            }
        }
        false
    }

    /// Whether a Supply Unit at `dump_hex` "can be supplied from Tobruk or Tripoli in any way"
    /// (§64.71): some `source` hex is reachable from the dump by a truck route (unbounded),
    /// not blocked by impassable terrain or an enemy ZOC.
    fn dump_connected_to_source(&self, dump_hex: HexCoord, side: Side, sources: &[HexCoord]) -> bool {
        self.truck_reachable(dump_hex, side, 10_000_000, |h| sources.contains(&h))
    }

    /// Whether a combat unit can trace a strategic line of supply (§64.71/§64.72): ≤ `max_mp`
    /// truck movement points to a friendly Supply Unit that is itself connected, in any way, to
    /// a `source` (Tobruk or Tripoli). Off-map (Tripoli-box) units are at the source already.
    pub fn unit_strategic_supply_line(&self, id: UnitId, max_mp: i32, sources: &[HexCoord]) -> bool {
        use std::collections::HashSet;
        let Some(u) = self.units.get(&id) else {
            return false;
        };
        let side = u.nationality.side();
        let Some(start) = self.effective_hex(id) else {
            return true; // off-map at Tripoli — supplied from the source
        };
        // Active dumps: friendly, non-dummy, connected to a source (§64.71 second leg).
        let active: HashSet<HexCoord> = self
            .supply_dumps
            .iter()
            .filter(|d| d.side == side && !d.dummy)
            .filter_map(|d| d.location.hex())
            .filter(|&h| self.dump_connected_to_source(h, side, sources))
            .collect();
        if active.is_empty() {
            return false;
        }
        // First leg: ≤ max_mp truck MP from the unit to any active dump.
        self.truck_reachable(start, side, max_mp * 10, |h| active.contains(&h))
    }

    /// Advance an off-map unit one Operations Stage in `dir` (§8.8/§8.84/§8.89). The unit
    /// expends its full CPA (the formation pace if it leads a stack) and progresses along the
    /// leg in the chosen direction: continuing forward completes a leg into the next box (or,
    /// toward the map from the innermost map-side box, enters the map at the junction —
    /// Nofilia, §8.85); commanding the opposite of an in-transit direction reverses back
    /// toward the box it departed (§8.84).
    pub fn advance_off_map(&mut self, id: UnitId, dir: OffMapDir) -> Result<OffMapMove, OffMapError> {
        let cpa = self
            .formation_cpa(id)
            .or_else(|| self.unit_cpa(id))
            .ok_or(OffMapError::NoCpa)?;
        let (node, transit, committed) = match self.units.get(&id).and_then(|u| u.location.as_ref()) {
            Some(UnitLocation::OffMap { node, transit, toward_map }) => (node.clone(), *transit, *toward_map),
            _ => return Err(OffMapError::NotOffMap),
        };
        let net = self.reference.off_map.as_ref().ok_or(OffMapError::NoNetwork)?;
        let i = net.index_of(&node).ok_or(OffMapError::UnknownNode)?;

        // The leg the unit is on: its committed direction if in transit, else the requested one.
        let leg_toward_map = if transit > 0 { committed } else { matches!(dir, OffMapDir::TowardMap) };
        // Leg cost + destination for that leg. Toward-map leg is stored on node i; the inland
        // leg (i→i+1) is stored on node i+1.
        let (cost, enters_map, dest_name) = if leg_toward_map {
            (net.nodes[i].leg.stages_for_cpa(cpa), i == 0, i.checked_sub(1).map(|k| net.nodes[k].name.clone()))
        } else {
            let j = i + 1;
            if j >= net.nodes.len() {
                return Err(OffMapError::NoInlandBox);
            }
            (net.nodes[j].leg.stages_for_cpa(cpa), false, Some(net.nodes[j].name.clone()))
        };

        let requested_toward_map = matches!(dir, OffMapDir::TowardMap);
        let reversing = transit > 0 && requested_toward_map != leg_toward_map;

        let (new_loc, result) = if reversing {
            // Walk back toward the box we departed (§8.84 "returning to its original location").
            let t = transit - 1;
            let loc = UnitLocation::OffMap { node: node.clone(), transit: t, toward_map: leg_toward_map };
            let res = if t == 0 { OffMapMove::Arrived(node.clone()) } else { OffMapMove::InTransit };
            (loc, res)
        } else if (transit as u32) + 1 >= cost {
            // Completed the leg.
            if leg_toward_map && enters_map {
                let sp = self.effective_stacking_points(id);
                let coord = self.map_entry_hex(sp).ok_or(OffMapError::BadJunction)?;
                (UnitLocation::OnMap(coord), OffMapMove::EnteredMap(coord))
            } else {
                let nn = dest_name.expect("adjacent box exists");
                (UnitLocation::OffMap { node: nn.clone(), transit: 0, toward_map: leg_toward_map }, OffMapMove::Arrived(nn))
            }
        } else {
            // Progress along the leg.
            (UnitLocation::OffMap { node, transit: transit + 1, toward_map: leg_toward_map }, OffMapMove::InTransit)
        };
        if let Some(u) = self.units.get_mut(&id) {
            u.location = Some(new_loc);
            u.cp_expended = Fixed(cpa * 10); // expended exactly its CPA this stage (§8.83)
        }
        Ok(result)
    }

    /// Convenience: one Operations Stage of off-map movement toward the map.
    pub fn advance_off_map_toward_map(&mut self, id: UnitId) -> Result<OffMapMove, OffMapError> {
        self.advance_off_map(id, OffMapDir::TowardMap)
    }

    /// Where an entering off-map unit of `sp` stacking points is placed on the map (§8.85
    /// road string-out): the first entry-road hex with room under its terrain's stacking
    /// limit, overflowing along the road; if every listed hex is full, the last one
    /// (best-effort). Falls back to the junction hex if no road is defined.
    pub fn map_entry_hex(&self, sp: u32) -> Option<HexCoord> {
        let net = self.reference.off_map.as_ref()?;
        let road: Vec<&String> = if net.entry_road.is_empty() {
            vec![&net.map_junction]
        } else {
            net.entry_road.iter().collect()
        };
        let mut last = None;
        for hid in road {
            let Some(coord) = HexId::parse(hid).map(|h| h.to_hex_coord()) else {
                continue;
            };
            last = Some(coord);
            let fits = match self
                .map
                .terrain(coord)
                .and_then(|t| self.reference.terrain.get(&t))
                .and_then(|p| p.stacking_limit)
            {
                Some(limit) => self.hex_stacking_points(coord) + sp <= limit,
                None => true, // no map loaded / no limit — place here
            };
            if fits {
                return Some(coord);
            }
        }
        last
    }

    /// Expend `cp` Capability Points for a unit this Operations Stage, crediting a
    /// Disorganization Point (−1 cohesion) for each whole CP used beyond its CPA (§6.21).
    /// DPs are applied incrementally, so repeated calls don't double-count.
    pub fn expend_cp(&mut self, id: UnitId, cp: Fixed) {
        let cpa = self.unit_cpa(id).unwrap_or(0);
        self.expend_cp_against(id, cp, cpa);
    }

    /// Like [`expend_cp`](Self::expend_cp) but judges over-expenditure against an explicit
    /// CPA — used for formation movement, where the pace is the formation CPA (§6.15)
    /// rather than the counter's own.
    pub fn expend_cp_against(&mut self, id: UnitId, cp: Fixed, cpa: i32) {
        let cpa_tenths = cpa * 10;
        if let Some(u) = self.units.get_mut(&id) {
            let before_over = (u.cp_expended.0 - cpa_tenths).max(0);
            u.cp_expended = Fixed(u.cp_expended.0 + cp.0);
            let after_over = (u.cp_expended.0 - cpa_tenths).max(0);
            let new_dp = after_over / 10 - before_over / 10; // whole CP newly over CPA
            u.cohesion -= new_dp;
        }
    }

    /// §11.2/§11.24: charge a unit up to `cap` whole Capability Points for combat this Operations
    /// Stage (5 phasing / 3 non-phasing / 2 probe), respecting the per-segment cap so a unit engaged
    /// in more than one combat this stage isn't overcharged. Flows through `expend_cp`, so §6.21
    /// over-CPA Disorganization applies.
    pub fn charge_combat_cp(&mut self, id: UnitId, cap: i32) {
        let cap_tenths = cap * 10;
        let already = self.units.get(&id).map(|u| u.combat_cp.0).unwrap_or(0);
        let add = (cap_tenths - already).max(0);
        if add > 0 {
            self.expend_cp(id, Fixed(add));
            if let Some(u) = self.unit_mut(id) {
                u.combat_cp = Fixed(u.combat_cp.0 + add);
            }
        }
    }

    /// §11.25: charge combat CP to *every* unit in each of `hexes` (participants and co-located
    /// non-participants alike), each at 5 CP if it belongs to the phasing (active) side else 3, or 2
    /// for a Probe (§11.23). Used by close-assault and anti-armor, which both charge attacker and
    /// defender hexes.
    pub(crate) fn charge_combat_hexes(&mut self, hexes: &[HexCoord], probe: bool) {
        let phasing = self.active_player;
        let mut ids: Vec<UnitId> = hexes.iter().flat_map(|&h| self.units_in_hex(h)).collect();
        ids.sort_unstable();
        ids.dedup();
        for id in ids {
            let cap = if probe {
                2
            } else if self.units.get(&id).map(|u| u.nationality.side()) == Some(phasing) {
                5
            } else {
                3
            };
            self.charge_combat_cp(id, cap);
        }
    }

    /// The hex a unit effectively occupies: its own location, or — if attached — the
    /// location resolved by following the attachment chain to the top counter.
    pub fn effective_hex(&self, id: UnitId) -> Option<HexCoord> {
        let mut current = id;
        for _ in 0..1024 {
            let unit = self.units.get(&current)?;
            match unit.attached_parent {
                Some(parent) => current = parent,
                None => return unit.hex(),
            }
        }
        None // attachment cycle guard
    }

    /// All units physically comprising a formation: `parent` plus every unit attached to
    /// it (transitively, §19.1). Attachment means co-located on the parent's counter, so
    /// this is the set used for formation CPA, combat aggregation, and stacking.
    pub fn formation_members(&self, parent: UnitId) -> Vec<UnitId> {
        let mut out = Vec::new();
        if self.units.contains_key(&parent) {
            out.push(parent);
        }
        let mut i = 0;
        while i < out.len() {
            let p = out[i];
            i += 1;
            for (&id, u) in &self.units {
                if u.attached_parent == Some(p) && !out.contains(&id) {
                    out.push(id);
                }
            }
        }
        out
    }

    /// A parent formation's Capability Point Allowance (§6.15): the lowest **effective** CPA among
    /// the units comprising it (it moves at the pace of its slowest member). `effective_cpa` folds
    /// in §6.17 motorization, so a leg-infantry member riding its attached Truck Points contributes
    /// the truck CPA rather than dragging the formation down to its foot pace. `None` if no member
    /// has a known CPA.
    pub fn formation_cpa(&self, parent: UnitId) -> Option<i32> {
        self.formation_members(parent)
            .iter()
            .filter_map(|&id| self.effective_cpa(id))
            .min()
    }

    /// Whether a unit is motorized (§8.53a, for Reaction). A unit is motorized if it is inherently
    /// mobile (tanks / reconnaissance), if it is leg infantry fully carried by attached first-line
    /// Truck Points (§6.17, `unit_motorized_cpa` applies), if it carries any first-line Truck Points,
    /// or — for a weapons unit whose organic vehicles give it a high pace — a CPA ≥ 20 (§8.53b: a
    /// motorized unit has a minimum CPA of 20).
    pub fn unit_is_motorized(&self, id: UnitId) -> bool {
        let Some(u) = self.units.get(&id) else {
            return false;
        };
        // Attached first-line Truck Points motorize any unit (§6.17 / §8.53b).
        if u.trucks.light + u.trucks.medium + u.trucks.heavy > 0 {
            return true;
        }
        if let Composition::Weapons { groups } = &u.composition {
            let table = self.reference.weapons_systems.get(&u.nationality);
            let mobile = groups.iter().any(|g| {
                table
                    .and_then(|m| m.get(&g.system))
                    .map(|w| matches!(w.kind, WeaponKind::Tank | WeaponKind::Reconnaissance))
                    .unwrap_or(false)
            });
            if mobile {
                return true;
            }
        }
        self.unit_motorized_cpa(id).is_some() || self.unit_cpa(id).is_some_and(|c| c >= 20)
    }

    /// A unit's maximum TOE Strength Points (§4.46 chart): the Organic `max_toe`, or for a
    /// weapon-holder the sum of its assignment limits.
    pub(crate) fn unit_max_toe(&self, id: UnitId) -> Option<u32> {
        let u = self.units.get(&id)?;
        let uc = self
            .reference
            .unit_characteristics
            .get(&u.nationality)?
            .get(&u.id_code)?;
        match &u.composition {
            Composition::Organic { .. } => uc.max_toe.map(|m| m.value),
            Composition::Weapons { .. } => {
                let s: u32 = uc.assignments.iter().map(|a| a.max_count).sum();
                if s > 0 {
                    Some(s)
                } else {
                    uc.max_toe.map(|m| m.value)
                }
            }
        }
    }

    fn unit_is_artillery(&self, id: UnitId) -> bool {
        let Some(u) = self.units.get(&id) else {
            return false;
        };
        match &u.composition {
            Composition::Weapons { groups } => {
                let table = self.reference.weapons_systems.get(&u.nationality);
                groups.iter().any(|g| {
                    table
                        .and_then(|m| m.get(&g.system))
                        .map(|ws| ws.kind == WeaponKind::Artillery)
                        .unwrap_or(false)
                })
            }
            Composition::Organic { .. } => self
                .reference
                .unit_characteristics
                .get(&u.nationality)
                .and_then(|m| m.get(&u.id_code))
                .map(|uc| uc.unit_type.contains("Artillery"))
                .unwrap_or(false),
        }
    }

    /// Whether a unit is a "shell" (§9.26). Implemented: the battalion/company clause —
    /// current TOE below 50% of maximum (25% for artillery). The division/brigade clauses
    /// (non-shell children vs OA maxima, §9.26/§9.27) are deferred until hierarchical OB
    /// and OA data exist, so larger formations are never flagged here.
    pub fn is_shell(&self, id: UnitId) -> bool {
        let Some(u) = self.units.get(&id) else {
            return false;
        };
        if !matches!(u.org_size, OrgSize::Battalion | OrgSize::Company) {
            return false;
        }
        let Some(max) = self.unit_max_toe(id) else {
            return false;
        };
        if max == 0 {
            return false;
        }
        let threshold = if self.unit_is_artillery(id) { 25 } else { 50 };
        u.total_toe() * 100 < threshold * max
    }

    /// A unit's effective organizational size for unit-size rules (§9.28): a shell is
    /// reduced one level (division→brigade, brigade→battalion, battalion→company).
    pub fn effective_org_size(&self, id: UnitId) -> OrgSize {
        let base = self.units.get(&id).map(|u| u.org_size).unwrap_or_default();
        if self.is_shell(id) {
            base.shell()
        } else {
            base
        }
    }

    /// Whether any unit is currently attached to `id` (i.e. it heads a formation).
    pub fn has_attached_units(&self, id: UnitId) -> bool {
        self.units.values().any(|u| u.attached_parent == Some(id))
    }

    /// §19.4: the number of **non-assigned** units currently attached to `parent` (units attached to
    /// it but assigned elsewhere or independent) — these are the ones the §19.5 Maximum Attachment
    /// limit governs (assigned units attach freely to their own parent).
    pub fn nonassigned_attachments(&self, parent: UnitId) -> u32 {
        self.units
            .values()
            .filter(|u| u.attached_parent == Some(parent) && u.assigned_parent != Some(parent))
            .count() as u32
    }

    /// §19.46: the formations a new attachment to `parent` counts against — `parent` itself plus
    /// every formation `parent` is (transitively) attached to.
    pub fn attachment_ancestors(&self, parent: UnitId) -> Vec<UnitId> {
        let mut chain = vec![parent];
        let mut cur = parent;
        let mut guard = 0;
        while let Some(next) = self.units.get(&cur).and_then(|u| u.attached_parent) {
            if chain.contains(&next) || guard > 32 {
                break; // cycle / depth guard
            }
            chain.push(next);
            cur = next;
            guard += 1;
        }
        chain
    }

    /// Whether `unit`'s attachment chain reaches `formation` (at any depth).
    fn attaches_up_to(&self, unit: UnitId, formation: UnitId) -> bool {
        let mut cur = unit;
        let mut guard = 0;
        while let Some(p) = self.units.get(&cur).and_then(|u| u.attached_parent) {
            if p == formation {
                return true;
            }
            if guard > 32 {
                break;
            }
            cur = p;
            guard += 1;
        }
        false
    }

    /// §19.46: units transitively attached to `formation` (at any depth) that are not assigned to it
    /// — the count the §19.5 Maximum Attachment limit governs once nesting is accounted for.
    pub fn transitive_nonassigned_attachments(&self, formation: UnitId) -> u32 {
        self.units
            .iter()
            .filter(|(&id, u)| id != formation && u.assigned_parent != Some(formation) && self.attaches_up_to(id, formation))
            .count() as u32
    }

    /// §19.5 Maximum Attachment Chart: the cap on **non-assigned** units attachable to `parent`,
    /// keyed by its effective org size, nationality, arm and the Game-Turn — including the named
    /// specials (Artillery Brigade HQ → 1; Matruh Garrison → 6 at GT 1–12, Selby Force → 5 at GT
    /// 13+). Type sub-constraints are applied separately by `attachment_type_ok`. The German
    /// "1 Brigade and 1 unit *or* 4 units" alternative uses the more permissive count, and the finer
    /// §19.3 assignment maxima (a separate `assigned_capacity` limit) are documented simplifications.
    pub fn nonassigned_attachment_limit(&self, parent: UnitId) -> u32 {
        use crate::reference::units::Side;
        use crate::state::unit::FormationRole;
        let Some(u) = self.units.get(&parent) else { return 0 };
        let side = u.nationality.side();
        let italian = u.nationality == Nationality::Italian;
        let gt = self.game_turn;
        // §19.5 named/special-arm rows override the generic numeric cap.
        match u.formation_role {
            FormationRole::ArtilleryBrigadeHq => return 1,
            FormationRole::MatruhSelby => return if gt <= 12 { 6 } else { 5 },
            FormationRole::Normal => {}
        }
        let armor = u.is_armor;
        match self.effective_org_size(parent) {
            OrgSize::Division | OrgSize::SuperBrigade => match side {
                // §19.5: CW armor division 2 (GT 1–67) / 3 (GT 68+); infantry division always 2.
                Side::Commonwealth => if armor && gt >= 68 { 3 } else { 2 },
                // German armor division 4 ("1 Bde and 1 unit *or* 4 units"); German infantry
                // division 3 ("1 Bde *or* 3 units"); Italian division 2 (armor or infantry).
                Side::Axis => if italian { 2 } else if armor { 4 } else { 3 },
            },
            OrgSize::StandardBrigade => match side {
                // §19.5: CW Tank Brigade 2 (GT 1–67) / 1 (GT 68+); any other brigade 1.
                Side::Commonwealth => if armor && gt < 68 { 2 } else { 1 },
                Side::Axis => 1, // German regiment / Italian brigade or regiment
            },
            OrgSize::BattleGroup => match side {
                Side::Commonwealth => 1,
                Side::Axis => if italian { 3 } else { 4 },
            },
            // §19.5: any Battalion may attach one Company (Italian battalions, none).
            OrgSize::Battalion => if italian { 0 } else { 1 },
            OrgSize::Company => 0,
        }
    }

    /// §19.5 type constraints for attaching `unit` to `parent`, beyond the numeric cap: the row's
    /// "no Tank" restriction and any per-type maximum (Infantry/Tank). The row is selected from the
    /// parent's org size, arm (`is_armor`, from its OB organization), nationality and the Game-Turn.
    /// Returns `false` if the attachment would violate the type rule. Covers the named/special-arm
    /// rows too: the German Artillery-Brigade-HQ "artillery only", the Italian infantry division's
    /// "1 Infantry *or* 1 Tank" (combined ≤1), and the Matruh-Garrison (GT 1–12: no tank, ≤1 gun) /
    /// Selby-Force (GT 13+: no tank, no recce, ≤3 infantry) specials, selected via `formation_role`.
    pub fn attachment_type_ok(&self, parent: UnitId, unit: UnitId) -> bool {
        use crate::reference::units::Side;
        use crate::state::unit::FormationRole;
        let Some(p) = self.units.get(&parent) else { return true };
        let side = p.nationality.side();
        let italian = p.nationality == Nationality::Italian;
        let armor = p.is_armor;
        let gt = self.game_turn;
        let is_tank = self.unit_is_tank(unit);
        let is_inf = self.unit_is_infantry(unit);

        // §19.5 named/special-arm rows.
        match p.formation_role {
            // German Artillery Brigade HQ: its one attachment must be an Artillery (gun-class) unit.
            FormationRole::ArtilleryBrigadeHq => return self.unit_is_gun(unit),
            // Mersa Matruh Garrison / Selby Force.
            FormationRole::MatruhSelby => {
                if is_tank {
                    return false; // both windows: no tank
                }
                return if gt <= 12 {
                    // Matruh Garrison: no tank, at most 1 gun class.
                    !self.unit_is_gun(unit)
                        || self.count_nonassigned_attached(parent, |g, id| g.unit_is_gun(id)) < 1
                } else {
                    // Selby Force: no tank, no recce, at most 3 infantry.
                    if self.is_recce_unit(unit) {
                        return false;
                    }
                    !is_inf
                        || self.count_nonassigned_attached(parent, |g, id| g.unit_is_infantry(id)) < 3
                };
            }
            FormationRole::Normal => {}
        }

        // Generic §19.5 rows: (no_tank, max_infantry, max_tank, max_infantry_or_tank_combined).
        let (no_tank, max_inf, max_tank, max_combo): (bool, Option<u32>, Option<u32>, Option<u32>) =
            match self.effective_org_size(parent) {
                OrgSize::Division | OrgSize::SuperBrigade => match side {
                    Side::Commonwealth => {
                        if armor {
                            if gt < 68 { (true, Some(1), None, None) } else { (false, Some(1), Some(1), None) }
                        } else {
                            (true, Some(1), None, None) // infantry division
                        }
                    }
                    // Italian infantry division: "1 Infantry *or* 1 Tank" — at most one, combined.
                    Side::Axis if italian => {
                        if armor { (false, None, None, None) } else { (false, None, None, Some(1)) }
                    }
                    Side::Axis => (true, None, None, None), // German division: no Tank
                },
                OrgSize::StandardBrigade => match side {
                    Side::Commonwealth if armor => (true, if gt < 68 { Some(1) } else { None }, None, None),
                    _ => (false, None, None, None),
                },
                _ => (false, None, None, None),
            };
        if is_tank && no_tank {
            return false;
        }
        if let Some(m) = max_combo {
            if (is_tank || is_inf)
                && self.count_nonassigned_attached(parent, |g, id| g.unit_is_tank(id) || g.unit_is_infantry(id)) >= m
            {
                return false;
            }
        }
        if is_tank {
            if let Some(m) = max_tank {
                if self.count_nonassigned_attached(parent, |g, id| g.unit_is_tank(id)) >= m {
                    return false;
                }
            }
        }
        if is_inf {
            if let Some(m) = max_inf {
                if self.count_nonassigned_attached(parent, |g, id| g.unit_is_infantry(id)) >= m {
                    return false;
                }
            }
        }
        true
    }

    /// An HQ counter with no combat units attached (§3.33/§9.12): it uses its parenthesized
    /// value — 0 Stacking Points, and it may not voluntarily attack.
    pub fn is_noncombat_hq(&self, id: UnitId) -> bool {
        self.units
            .get(&id)
            .is_some_and(|u| u.is_hq && !self.has_attached_units(id))
    }

    /// Effective Basic Stacking Points (§9.4) after any shell reduction (§9.28) and the
    /// parenthesized-HQ rule (§9.12: an HQ with nothing attached occupies 0 SP).
    pub fn effective_stacking_points(&self, id: UnitId) -> u32 {
        if self.is_noncombat_hq(id) {
            return 0;
        }
        self.effective_org_size(id).stacking_points()
    }

    /// Advance to the next step in the Sequence of Play (§5.2): through the stage steps
    /// (setup + Player A block + Player B block), rolling over Operations Stages and
    /// Game-Turns, re-determining Initiative at the start of each Game-Turn.
    pub fn advance_phase(&mut self) {
        // Stage VII — Strategic Air Recovery (after the three Operations Stages).
        if let Some(i) = self.post_turn_step {
            if i + 1 < POST_TURN_SEQUENCE.len() {
                self.enter_post_turn(i + 1);
            } else {
                self.post_turn_step = None;
                self.game_turn += 1;
                self.operations_stage = 1;
                self.phase = Phase::InitiativeDetermination;
                self.stage_step = 0;
                // §44.13: each Malta base's construction allowance refreshes each Game-Turn.
                for base in self.malta_bases.iter_mut() {
                    base.begin_turn();
                }
                // §34.84/§34.85: this Game-Turn's airplane reinforcements arrive into the pool.
                self.accrue_air_reinforcements();
            }
            return;
        }
        // Stages II–III — Strategic Air Planning + Naval Convoy (before the Operations Stages).
        if let Some(i) = self.pre_turn_step {
            if i + 1 < PRE_TURN_SEQUENCE.len() {
                self.enter_pre_turn(i + 1);
            } else {
                self.pre_turn_step = None;
                self.operations_stage = 1;
                self.begin_operations_stage();
            }
            return;
        }
        // Stage I — Initiative Determination: begin the Game-Turn, then enter Stage II.
        if self.phase == Phase::InitiativeDetermination {
            self.update_victory_hold();
            self.expend_stores();
            self.determine_initiative();
            self.operations_stage = 0;
            self.enter_pre_turn(0);
            return;
        }
        // Stages IV/V/VI — Operations: walk the stage steps, then next stage, then Stage VII.
        if self.stage_step + 1 < STAGE_SEQUENCE.len() {
            self.enter_stage_step(self.stage_step + 1);
        } else if self.operations_stage < 3 {
            self.operations_stage += 1;
            self.begin_operations_stage();
        } else {
            self.enter_post_turn(0);
        }
    }

    /// Enter step `i` of the pre-Operations stages (§33.0 II–III).
    fn enter_pre_turn(&mut self, i: usize) {
        self.pre_turn_step = Some(i);
        let (phase, actor) = PRE_TURN_SEQUENCE[i];
        self.phase = phase;
        self.active_player = self.actor_side(actor);
    }

    /// Enter step `i` of the Strategic Air Recovery stage (§33.0 VII).
    fn enter_post_turn(&mut self, i: usize) {
        self.post_turn_step = Some(i);
        let (phase, actor) = POST_TURN_SEQUENCE[i];
        self.phase = phase;
        self.active_player = self.actor_side(actor);
    }

    /// The acting side for a `StageActor` under the current initiative/declaration.
    fn actor_side(&self, actor: StageActor) -> Side {
        match actor {
            StageActor::Initiative => self.initiative,
            StageActor::PlayerA => self.player_a,
            StageActor::PlayerB => self.player_a.opponent(),
            StageActor::Axis => Side::Axis,
            StageActor::Commonwealth => Side::Commonwealth,
        }
    }

    /// Begin an Operations Stage: reset each unit's Capability Points expended (CPA is a
    /// per-Operations-Stage allowance, §6.2), default the §III.A declaration to
    /// "Initiative player moves first", then enter the first stage step.
    pub fn begin_operations_stage(&mut self) {
        // §31: refresh Rommel's 60-CP Movement Allowance (600 tenths) for the new stage, record
        // where he begins it (§31.4 requires him to remain the whole stage), and clear the previous
        // stage's +5-CPA designation.
        if let Some(r) = self.rommel.as_mut() {
            r.mcp_left = 600;
            r.stage_start_hex = r.hex;
            r.cpa_unit = None;
        }
        // §27.37: refresh each raider's 50-CP Movement Allowance for the new stage. §27.33: a raider
        // revealed by last stage's raid/spotting slips back into hiding for the new stage.
        for raider in self.raiders.iter_mut() {
            raider.cpa_used = 0;
            raider.revealed = false;
        }
        // §8.98: refresh each free truck convoy's Movement Allowance for the new Truck Convoy Phase.
        for convoy in self.truck_convoys.iter_mut() {
            convoy.cpa_used = 0;
        }
        // §27.73: track how long the Layforce commando has been mustering in Alexandria.
        self.track_layforce_alexandria();
        // §52.24: reset the per-stage pipeline-construction allowance.
        self.pipeline_built_this_stage.clear();
        // §26.23: dummy minefields revealed last stage are now removed.
        for h in self.revealed_minefields.drain().collect::<Vec<_>>() {
            self.minefields.remove(&h);
        }
        // §8.74: reset the once-per-direction Rail Movement allowance.
        self.rail_west_used = false;
        self.rail_east_used = false;
        self.rail_maintenance_this_stage = false;
        // §54.4: reset the Axis rail haulage budget for the new stage, and enforce §54.45 —
        // if the Axis no longer holds five contiguous controlled rail hexes, its Rolling Stock
        // is considered destroyed.
        self.axis_rail_tons_used = 0;
        self.axis_rail_dir = None;
        // §16.21: the Patrol eligibility (no AA/CA combat this stage) and the reconnoitre-ban on
        // barraged/bombed hexes are per-stage.
        self.land_combat_this_stage.clear();
        self.barraged_hexes_this_stage.clear();
        self.close_assaulted_hexes_this_stage.clear();
        self.anti_armor_hexes_this_stage.clear();
        self.recon_revealed.clear(); // §42.27 recon reveals are a per-stage snapshot
        // §8.62: In-Contact status is derived from enemy ZOCs at the start of the Movement Segment;
        // §8.67 clears any Engagement no longer held by an adjacent opposing combat unit.
        self.refresh_contact_status();
        // §28: prisoners reaching a Departure Point leave; then resolve escapes/rejoins/recaptures
        // (§28.23–25) and refresh each stack's movement allowance for the new Convoy Segment (§28.13).
        self.process_prisoner_departures();
        self.resolve_prisoner_escapes();
        // §14.52: Destroyed Tank Markers in hexes the enemy now controls are captured.
        self.resolve_destroyed_tank_capture();
        self.resolve_captured_dumps();
        for g in self.prisoner_groups.iter_mut() {
            g.cpa_used = 0;
        }
        if self.axis_rolling_stock_tons > 0 && self.axis_rail_contiguous_max() < 5 {
            self.axis_rolling_stock_tons = 0;
        }
        // §24.11: the Construction Segment — advance projects by the just-completed stage (read
        // before CP is reset below, so the §24.12 no-CP-expended check sees that stage).
        self.advance_construction();
        // §40.2: Combat Air Patrols last one Operations Stage — clear last stage's patrols (also
        // resetting the §40.27 per-stage interception allowance).
        self.cap_patrols.clear();
        // §40.13/§40.14/§40.17: advance each pilot's reassignment/retraining/recovery countdown.
        for p in self.pilots.iter_mut() {
            p.advance_stage();
        }
        // §43.14: advance each squadron's transfer transit countdown.
        for s in self.sgsus.values_mut() {
            s.transfer_remaining = s.transfer_remaining.saturating_sub(1);
        }
        // §35.14: each SGSU consumes its own upkeep (Fuel + Water per stage, Stores per turn).
        self.consume_sgsu_upkeep();
        // §30.16/§30.24/§30.25: per-stage ship upkeep — clear the once-per-stage fire flag and
        // pay down owed port rest.
        for ship in self.fleet.iter_mut() {
            ship.begin_stage();
        }
        // §52.4: consume Water for the stage. A vehicle needs water only if it used CPA, so we
        // take "acted" from the CP expended in the stage just completed (before it is reset).
        if self.reference.water_table.is_some() {
            let acted: std::collections::HashSet<UnitId> = self
                .units
                .values()
                .filter(|u| u.cp_expended.0 > 0)
                .map(|u| u.id)
                .collect();
            self.expend_water(&acted);
        }
        // §6.24.1: a unit that expended no Capability Points this Operations Stage (and was not
        // Training) recovers 5 Reorganization Points — but this idle recovery may never raise its
        // Cohesion above 0 (read cp_expended / in_training before both are cleared below).
        for u in self.units.values_mut() {
            if u.cp_expended.0 == 0 && !u.status.in_training && u.cohesion < 0 {
                u.cohesion = (u.cohesion + 5).min(0);
            }
        }
        // §17.3 Training: resolve the just-completed stage for each unit designated to Train (read
        // before CP is reset). §17.33: any CP expenditure interrupts, resetting progress; otherwise
        // the unit accrues a training stage, and every six raise its Basic Morale one step toward its
        // trained reference (§17.34), never above it. `in_training` is a per-stage designation.
        for u in self.units.values_mut() {
            if !u.status.in_training {
                continue;
            }
            if u.cp_expended.0 == 0 && u.basic_morale < u.basic_morale_reference {
                u.training_progress += 1;
                if u.training_progress >= 6 {
                    u.basic_morale += 1;
                    u.training_progress = 0;
                }
            } else {
                u.training_progress = 0;
            }
            u.status.in_training = false;
        }
        for u in self.units.values_mut() {
            u.cp_expended = Fixed(0);
            u.combat_cp = Fixed(0);
            u.moved_into_zoc = false;
            u.breakdown_points = 0;
            u.breakdown_checked_col = -1;
        }
        // §20.43: Replacement Points progress through Training one Operations Stage at a time — but
        // only while the pool sits at a training area for its side (CW §17.32 list, or Axis Tripoli /
        // a flagged major city). A pool elsewhere (or in transit) has its clock paused.
        let ticks: Vec<bool> = self
            .replacement_points
            .iter()
            .map(|rp| self.is_replacement_training_area(rp.location.as_ref(), rp.side))
            .collect();
        for (rp, tick) in self.replacement_points.iter_mut().zip(ticks) {
            if tick {
                rp.training_remaining = rp.training_remaining.saturating_sub(1);
            }
        }
        self.player_a = self.initiative;
        self.enter_stage_step(0);
    }

    /// Bring in reinforcements now due (§4.44 Organization at Arrival): the later-arriving
    /// units of a deployed formation whose scheduled (game_turn, stage) has been reached.
    /// They spawn and attach to their already-on-map parent, joining the formation.
    pub fn process_arrivals(&mut self) {
        let now = (self.game_turn, self.operations_stage);
        let mut due = Vec::new();
        let mut i = 0;
        while i < self.pending_arrivals.len() {
            let p = &self.pending_arrivals[i];
            if (p.game_turn, p.stage) <= now {
                due.push(self.pending_arrivals.remove(i));
            } else {
                i += 1;
            }
        }
        for p in due {
            let uh: HashMap<String, HexCoord> = p.unit_hexes.iter().cloned().collect();
            let (gt, st) = (p.game_turn, p.stage);
            let _ = self.deploy_ob_units(p.nationality, &p.formation, p.entry.clone(), &uh, move |a| {
                matches!(a, Arrival::Turn { game_turn, stage }
                    if *game_turn as u32 == gt && *stage == st)
            });
        }
    }

    /// The Initiative player's §III.A declaration for the current Operations Stage: whether
    /// they move first (Player A) or last (Player B). Choosing to move last one stage and
    /// first the next yields the §7.12 "two turns in a row". Call before the F–L blocks.
    pub fn declare_initiative_order(&mut self, initiative_moves_first: bool) {
        self.player_a = if initiative_moves_first {
            self.initiative
        } else {
            self.initiative.opponent()
        };
        // Re-resolve the acting side for the current step under the new declaration.
        self.enter_stage_step(self.stage_step);
    }

    /// Enter stage step `i`, setting the phase and the acting (Phasing) side.
    fn enter_stage_step(&mut self, i: usize) {
        self.stage_step = i;
        let (phase, actor) = STAGE_SEQUENCE[i];
        self.phase = phase;
        self.active_player = match actor {
            StageActor::Initiative => self.initiative,
            StageActor::PlayerA => self.player_a,
            StageActor::PlayerB => self.player_a.opponent(),
            StageActor::Axis => Side::Axis,
            StageActor::Commonwealth => Side::Commonwealth,
        };
        // §29.1: the Initiative player rolls weather at the Weather phase.
        if phase == Phase::Weather {
            self.determine_weather();
        }
        // §20.12: reinforcements enter at the start of the Naval Convoy Arrival Phase.
        if phase == Phase::NavalConvoyArrival {
            self.process_arrivals();
            // §20.62/§20.64: defer any Axis Replacement production beyond the turn's actual roll
            // before the survivors enter the pool.
            self.defer_over_shipped_axis_replacements();
            self.process_replacement_arrivals();
            self.process_truck_arrivals();
            self.place_rommel_if_arrived(); // §31: the Rommel counter arrives with reinforcements
        }
    }

    /// Determine Initiative for the coming Game-Turn (§7.14): each side rolls one die and
    /// adds its Initiative Rating (§7.2); the higher total wins, ties re-roll. The first
    /// Game-Turn may be predetermined by the scenario (§7.15, `first_turn_initiative`).
    fn determine_initiative(&mut self) {
        // §27.35: eliminate raiders that went a Game-Turn without Water, then reset the requirement.
        self.raider_turn_upkeep();
        // §31 recall: resolve whether Rommel is in Germany this Game-Turn before rolling Initiative
        // (it lowers the Axis Initiative Rating to the German value when he is away).
        self.roll_rommel_recall();
        if self.game_turn == 1 {
            if let Some(side) = self.first_turn_initiative {
                self.initiative = side;
                self.active_player = side;
                return;
            }
        }
        loop {
            let axis = self.rng.d6() as i32 + self.axis_initiative_rating();
            let cw = self.rng.d6() as i32 + self.commonwealth_initiative_rating();
            if axis > cw {
                self.initiative = Side::Axis;
                break;
            } else if cw > axis {
                self.initiative = Side::Commonwealth;
                break;
            }
            // Tie: roll again (§7.14).
        }
        self.active_player = self.initiative;
    }

    /// Commonwealth Initiative Rating for the current Game-Turn (§7.2).
    pub fn commonwealth_initiative_rating(&self) -> i32 {
        self.reference
            .initiative_ratings
            .as_ref()
            .map(|r| r.commonwealth(self.game_turn))
            .unwrap_or(0)
    }

    /// Axis Initiative Rating (§7.2), derived from game state: Rommel on the maps → highest;
    /// else any German land unit on the maps → middle; else the minimal rating. (The
    /// holding-box exclusion is deferred; any on-map German unit currently counts.)
    pub fn axis_initiative_rating(&self) -> i32 {
        let Some(r) = self.reference.initiative_ratings.as_ref() else {
            return 0;
        };
        // §27.64 "AI temp 3": a Raid-on-Rommel result pins the Axis rating to the German value for
        // two Game-Turns, overriding Rommel-on-map.
        if self.rommel_raid_ai3_until_gt > 0 && self.game_turn <= self.rommel_raid_ai3_until_gt {
            return r.axis_german;
        }
        if self.rommel_on_map() {
            r.axis_rommel
        } else if self
            .units
            .values()
            .any(|u| u.nationality == Nationality::German && u.hex().is_some())
        {
            r.axis_german
        } else {
            r.axis_minimal
        }
    }

    /// §31: whether the Rommel counter is present and active on the game-maps this Game-Turn —
    /// he exists in the scenario, has arrived, is placed on a hex, and has not been recalled to
    /// Germany (§31 recall). All of Rommel's effects (§7.2 initiative, §31.3 morale, §31.4 CPA)
    /// are gated on this.
    pub fn rommel_on_map(&self) -> bool {
        self.rommel.as_ref().is_some_and(|r| {
            self.game_turn >= r.arrival_game_turn && r.hex.is_some() && !r.in_germany
        })
    }

    /// §31: the hex the active Rommel counter occupies (`None` if absent / recalled / not arrived).
    pub fn rommel_hex(&self) -> Option<HexCoord> {
        if self.rommel_on_map() {
            self.rommel.as_ref().and_then(|r| r.hex)
        } else {
            None
        }
    }

    /// §31 recall roll — at the start of a Game-Turn in which Rommel is present and on the map, the
    /// Axis Player rolls 2d6; on a 12 Rommel is in Germany for that Game-Turn (all his effects are
    /// suspended, and the Axis Initiative Rating drops to the German value), else he is present.
    /// Returns whether Rommel is in Germany this Game-Turn.
    fn roll_rommel_recall(&mut self) -> bool {
        let present = self
            .rommel
            .as_ref()
            .is_some_and(|r| self.game_turn >= r.arrival_game_turn && r.hex.is_some());
        if !present {
            return false;
        }
        let recalled = self.rng.d6() as u32 + self.rng.d6() as u32 == 12;
        if let Some(r) = self.rommel.as_mut() {
            r.in_germany = recalled;
        }
        recalled
    }

    /// §31: bring the Rommel counter onto the map at its scenario `start_hex` on the Game-Turn it
    /// arrives (called at the Naval Convoy Arrival Phase, like other reinforcements).
    fn place_rommel_if_arrived(&mut self) {
        let gt = self.game_turn;
        if let Some(r) = self.rommel.as_mut() {
            if r.hex.is_none() && gt >= r.arrival_game_turn {
                r.hex = r.start_hex;
            }
        }
    }

    /// §27: the side and per-kind pool cap of a raider kind — LRDG is Commonwealth (up to two,
    /// §27.11), DSA is Axis/German (one, §27.2a).
    fn raider_limits(kind: RaiderKind) -> (Side, usize) {
        match kind {
            RaiderKind::Lrdg => (Side::Commonwealth, 2),
            RaiderKind::Dsa => (Side::Axis, 1),
        }
    }

    /// Number of `kind` raider counters currently in play (on the map).
    pub fn raiders_in_play(&self, kind: RaiderKind) -> usize {
        self.raiders.iter().filter(|r| r.kind == kind && r.hex.is_some()).count()
    }

    /// §27.12/§27.13: whether `unit` is a valid source for forming a `kind` raider — a Friendly
    /// Reconnaissance / Armored-Car unit with a TOE Strength Point to spare, trained to its maximum
    /// (Basic) Morale (§27.12: `basic_morale` has reached its trained reference — always true for
    /// non-green units, and for green units only after §17.3 Training).
    pub fn raider_source_ok(&self, kind: RaiderKind, unit: UnitId) -> bool {
        let (side, _) = Self::raider_limits(kind);
        let Some(u) = self.units.get(&unit) else {
            return false;
        };
        // §27.2b: DSA forms only from German Armored Recce/AC units (not Italian).
        let nat_ok = match kind {
            RaiderKind::Dsa => u.nationality == Nationality::German,
            RaiderKind::Lrdg => u.nationality.side() == Side::Commonwealth,
        };
        if !nat_ok || u.nationality.side() != side || u.total_toe() == 0 {
            return false;
        }
        // §27.12: the source must be trained to its maximum Morale.
        if u.basic_morale < u.basic_morale_reference {
            return false;
        }
        let is_recce = self
            .reference
            .unit_characteristics
            .get(&u.nationality)
            .and_then(|m| m.get(&u.id_code))
            .is_some_and(|c| {
                let t = &c.unit_type;
                t.contains("Reconnaissance") || t.contains("Armored Car") || t.contains("Armored Recce")
            });
        is_recce && u.hex().is_some()
    }

    /// §27.13/§27.16: form a `kind` desert raider from `unit` (a trained Recce/AC unit) in the
    /// Organization Phase — removing one TOE Strength Point and placing the raider in the unit's hex.
    /// Re-uses an eliminated counter whose §27.16 four-Game-Turn cooldown has elapsed, else creates a
    /// new one if the §27.11/§27.2a pool cap allows. Returns the raider's index, or `None` if the
    /// source is invalid or the pool is full.
    pub fn form_raider(&mut self, kind: RaiderKind, unit: UnitId) -> Option<usize> {
        if !self.raider_source_ok(kind, unit) {
            return None;
        }
        let (side, cap) = Self::raider_limits(kind);
        let hex = self.units.get(&unit)?.hex()?;
        // A cooled-down counter of this kind that may re-form now (§27.16)?
        let gt = self.game_turn;
        let reusable = self
            .raiders
            .iter()
            .position(|r| r.kind == kind && r.hex.is_none() && r.reform_game_turn <= gt);
        let total = self.raiders.iter().filter(|r| r.kind == kind).count();
        let idx = match reusable {
            Some(i) => i,
            None if self.raiders_in_play(kind) < cap && total < cap => {
                self.raiders.push(Raider {
                    kind,
                    side,
                    hex: None,
                    cpa_used: 0,
                    fuel: 0,
                    ammo: 0,
                    drew_water_this_gt: false,
                    reform_game_turn: 0,
                    revealed: false,
                });
                self.raiders.len() - 1
            }
            None => return None,
        };
        // §27.13: consume one TOE Strength Point from the source Recce/AC unit.
        if let Some(u) = self.unit_mut(unit) {
            if let crate::state::unit::Composition::Organic { current_toe } = &mut u.composition {
                *current_toe = current_toe.saturating_sub(1);
            }
        }
        let r = &mut self.raiders[idx];
        r.hex = Some(hex);
        r.cpa_used = 0;
        r.reform_game_turn = 0;
        r.drew_water_this_gt = false;
        r.revealed = false; // §27.33: a freshly formed raider is hidden again
        Some(idx)
    }

    /// §8.98: a free truck convoy's Movement Allowance (tenths of a CP) — the slowest supply CPA
    /// (§54.2 `cpa_supplies`) among the truck types it contains. 0 if the Truck Chart is unloaded or
    /// the convoy is empty.
    pub fn truck_convoy_cpa(&self, convoy: &TruckConvoy) -> u32 {
        let Some(chart) = self.reference.truck_chart.as_ref() else {
            return 0;
        };
        let t = &convoy.trucks;
        let cpa = [
            (t.light, chart.light.cpa_supplies),
            (t.medium, chart.medium.cpa_supplies),
            (t.heavy, chart.heavy.cpa_supplies),
        ]
        .into_iter()
        .filter(|(n, _)| *n > 0)
        .map(|(_, cpa)| cpa)
        .min()
        .unwrap_or(0);
        cpa.max(0) as u32 * 10
    }

    /// §60/§20.14: the Alexandria map hex (E3613) — where the Layforce commando musters and where
    /// its transport ships return (§27.73/§27.77).
    pub fn alexandria_hex(&self) -> Option<HexCoord> {
        crate::state::map::HexId::parse("E3613").map(|h| h.to_hex_coord())
    }

    /// §27.71: whether `id` is the Layforce ("A" Battalion, Special Service Brigade) commando unit,
    /// recognised by name.
    pub fn is_layforce(&self, id: UnitId) -> bool {
        self.units.get(&id).is_some_and(|u| u.name.contains("Layforce"))
    }

    /// §27.74: whether the Layforce free-supply window is still open at the current (Game-Turn,
    /// Operations Stage).
    pub fn layforce_free_supply_active(&self) -> bool {
        self.layforce_free_supply_until
            .is_some_and(|(gt, st)| (self.game_turn, self.operations_stage) <= (gt, st))
    }

    /// §27.73 residence tracker (run each Operations Stage): record when the Layforce battalion is
    /// in Alexandria (it must stay one full Game-Turn before landing), clearing the timer when it is
    /// elsewhere.
    fn track_layforce_alexandria(&mut self) {
        let alex = self.alexandria_hex();
        let in_alex = self.units.values().any(|u| u.name.contains("Layforce") && u.hex() == alex);
        if in_alex {
            if self.layforce_alexandria_since.is_none() {
                self.layforce_alexandria_since = Some(self.game_turn);
            }
        } else {
            self.layforce_alexandria_since = None;
        }
    }

    /// §27.71/§27.8: whether `id` is a commando that is hidden/moves as an LRDG when escorted
    /// (§27.88 SAS). Recognised by name.
    pub fn is_sas(&self, id: UnitId) -> bool {
        self.units.get(&id).is_some_and(|u| u.name.contains("SAS"))
    }

    /// §27.84: whether the SAS Brigade's post-transport free-supply window is still open.
    pub fn sas_free_supply_active(&self) -> bool {
        self.sas_free_supply_until
            .is_some_and(|(gt, st)| (self.game_turn, self.operations_stage) <= (gt, st))
    }

    /// §27.88: whether the SAS Brigade at `id` is stacked with a friendly LRDG raider — it then
    /// raises its Capability Point Allowance to 30 and moves as an LRDG.
    pub fn sas_escorted_by_lrdg(&self, id: UnitId) -> bool {
        let Some(hex) = self.unit(id).and_then(|u| u.hex()) else {
            return false;
        };
        self.raiders
            .iter()
            .any(|r| r.kind == RaiderKind::Lrdg && r.hex == Some(hex))
    }

    /// §27.35 Game-Turn upkeep: eliminate any in-play raider that did not draw Water during the
    /// just-ended Game-Turn, then reset every raider's Water requirement for the new Game-Turn.
    /// Called at the start of each Game-Turn (before Initiative).
    fn raider_turn_upkeep(&mut self) {
        let n = self.raiders.len();
        for i in 0..n {
            let (in_play, watered) = self
                .raiders
                .get(i)
                .map(|r| (r.hex.is_some(), r.drew_water_this_gt))
                .unwrap_or((false, true));
            if in_play && !watered {
                self.eliminate_raider(i); // §27.35: no Water this Game-Turn -> eliminated
            }
        }
        for r in self.raiders.iter_mut() {
            r.drew_water_this_gt = false;
        }
    }

    /// §27.16: eliminate the raider at `index` — remove it from the map and start its four-Game-Turn
    /// re-form cooldown.
    pub fn eliminate_raider(&mut self, index: usize) {
        let gt = self.game_turn;
        if let Some(r) = self.raiders.get_mut(index) {
            r.hex = None;
            r.reform_game_turn = gt + 4;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_state() -> GameState {
        GameState::new(ReferenceData::new(), 1)
    }

    #[test]
    fn full_sequence_includes_air_and_naval_stages() {
        let mut g = empty_state();
        // Collect the ordered phases of one full Game-Turn (until the next Initiative).
        let mut phases = Vec::new();
        g.advance_phase(); // leave Initiative Determination
        loop {
            phases.push(g.phase);
            g.advance_phase();
            if g.phase == Phase::InitiativeDetermination {
                break;
            }
        }
        // Stage II–III pre-turn phases appear once, before any Operations (Weather).
        let idx = |p: Phase| phases.iter().position(|&q| q == p);
        for p in [
            Phase::StrategicAirDesignation,
            Phase::MaltaAvailabilityDetermination,
            Phase::StrategicMissionAssignment,
            Phase::MaltaRaid,
            Phase::NavalConvoySchedule,
            Phase::ConvoyResolution,
        ] {
            assert!(idx(p).is_some(), "{p:?} missing");
            assert!(idx(p) < idx(Phase::Weather), "{p:?} not before Operations");
        }
        // Operations adds the Commonwealth Fleet, Land Support Air and Rail phases.
        assert!(idx(Phase::CommonwealthFleet).is_some());
        assert!(idx(Phase::LandSupportAir).is_some());
        assert!(idx(Phase::CommonwealthRail).is_some());
        // Stage VII recovery phases appear after the last Operations phase (Patrol).
        let last_patrol = phases.iter().rposition(|&p| p == Phase::Patrol).unwrap();
        assert!(idx(Phase::StrategicAirReturn).unwrap() > last_patrol);
        assert!(idx(Phase::AircraftMaintenance).unwrap() > last_patrol);
        // Three Operations Stages -> Land Support Air appears three times.
        assert_eq!(phases.iter().filter(|&&p| p == Phase::LandSupportAir).count(), 3);
    }

    #[test]
    fn phase_progression_over_a_full_turn() {
        let mut g = empty_state();
        assert_eq!(g.game_turn, 1);
        assert_eq!(g.phase, Phase::InitiativeDetermination);

        // Initiative -> Stage II (Strategic Air Planning), Strategic Air Designation first.
        g.advance_phase();
        assert_eq!((g.operations_stage, g.phase), (0, Phase::StrategicAirDesignation));
        let init = g.initiative;
        assert_eq!(g.active_player, init);

        // Each Operations Stage runs the F–L phases for Player A (Initiative) then
        // Player B (opponent). Count Movement & Combat phases and check the actor.
        let mut mc_players = Vec::new();
        let mut stages_seen = 1;
        loop {
            if g.phase == Phase::MovementAndCombat {
                mc_players.push(g.active_player);
            }
            g.advance_phase();
            if g.phase == Phase::InitiativeDetermination {
                break;
            }
            if g.operations_stage as usize > stages_seen {
                stages_seen = g.operations_stage as usize;
            }
        }
        // 3 stages x 2 players = 6 Movement & Combat phases, alternating A/B each stage.
        assert_eq!(stages_seen, 3);
        assert_eq!(mc_players.len(), 6);
        assert_eq!(mc_players[0], init);
        assert_eq!(mc_players[1], init.opponent());

        // Rolled over into the next Game-Turn and re-determined Initiative.
        assert_eq!(g.game_turn, 2);
        assert_eq!(g.phase, Phase::InitiativeDetermination);
    }

    #[test]
    fn spawn_and_lookup_units() {
        let mut g = empty_state();
        let a = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "1st Bn",
            Composition::Organic { current_toe: 5 },
        );
        let b = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "2nd Bn",
            Composition::Organic { current_toe: 5 },
        );
        assert_ne!(a, b);
        assert_eq!(g.units.len(), 2);
        assert_eq!(g.unit(a).unwrap().name, "1st Bn");
    }

    #[test]
    fn effective_hex_follows_attachment_chain() {
        let mut g = empty_state();
        let hq = g.spawn_unit(
            Nationality::Commonwealth,
            "a",
            "Div HQ",
            Composition::Organic { current_toe: 0 },
        );
        let bn = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "Attached Bn",
            Composition::Organic { current_toe: 5 },
        );
        // HQ is on the map; battalion is attached to it (no own location).
        g.unit_mut(hq).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(HexCoord::new(12, 8)));
        g.unit_mut(bn).unwrap().attached_parent = Some(hq);

        assert_eq!(g.effective_hex(hq), Some(HexCoord::new(12, 8)));
        assert_eq!(g.effective_hex(bn), Some(HexCoord::new(12, 8)));
    }

    #[test]
    fn cp_resets_each_operations_stage() {
        let mut g = empty_state();
        let u = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "bn",
            Composition::Organic { current_toe: 5 },
        );
        g.unit_mut(u).unwrap().cp_expended = Fixed(30);
        // Advance until the first Operations Stage begins (Weather), which resets CP via
        // begin_operations_stage (§6.2).
        while g.phase != Phase::Weather {
            g.advance_phase();
        }
        assert_eq!(g.unit(u).unwrap().cp_expended, Fixed(0));
    }

    #[test]
    fn initiative_ratings_from_the_chart() {
        // §7.2: Commonwealth by turn; Axis by situation.
        let mut refs = ReferenceData::new();
        refs.load_initiative_ratings(include_str!(
            "../../../../data/reference/initiative-ratings.ron"
        ))
        .unwrap();
        let mut g = GameState::new(refs, 1);

        // Commonwealth: 3 (GT 1-42), 4 (43-90), 5 (91-111).
        assert_eq!(g.commonwealth_initiative_rating(), 3);
        g.game_turn = 43;
        assert_eq!(g.commonwealth_initiative_rating(), 4);
        g.game_turn = 100;
        assert_eq!(g.commonwealth_initiative_rating(), 5);

        // Axis: no German units / no Rommel -> 1.
        assert_eq!(g.axis_initiative_rating(), 1);
        // A German unit on the map -> 3.
        let ge = g.spawn_unit(
            Nationality::German,
            "h",
            "pz bn",
            Composition::Organic { current_toe: 5 },
        );
        g.unit_mut(ge).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(HexCoord::new(0, 0)));
        assert_eq!(g.axis_initiative_rating(), 3);
        // Rommel on the maps -> 6 (§31: an arrived, placed, not-recalled counter).
        g.rommel = Some(RommelState {
            arrival_game_turn: 1,
            start_hex: Some(HexCoord::new(0, 0)),
            hex: Some(HexCoord::new(0, 0)),
            in_germany: false,
            mcp_left: 0,
            stage_start_hex: None,
            cpa_unit: None,
        });
        assert_eq!(g.axis_initiative_rating(), 6);
        // §31 recall: in Germany this Game-Turn -> falls back to the German rating (3).
        g.rommel.as_mut().unwrap().in_germany = true;
        assert_eq!(g.axis_initiative_rating(), 3);
    }

    #[test]
    fn first_turn_initiative_is_predetermined() {
        // §7.15: the scenario may fix the first Game-Turn's Initiative (no roll).
        let mut g = empty_state();
        g.first_turn_initiative = Some(Side::Axis);
        g.advance_phase();
        assert_eq!(g.game_turn, 1);
        assert_eq!(g.initiative, Side::Axis);
    }

    #[test]
    fn declare_initiative_order_swaps_player_a() {
        // Commonwealth has Initiative but declares to move LAST this stage (§III.A/§7.12).
        let mut g = empty_state();
        g.first_turn_initiative = Some(Side::Commonwealth);
        // Advance into the first Operations Stage; player_a defaults to Initiative (CW).
        while g.phase != Phase::Weather {
            g.advance_phase();
        }
        assert_eq!(g.player_a, Side::Commonwealth);

        g.declare_initiative_order(false); // Initiative moves last -> opponent is Player A
        assert_eq!(g.player_a, Side::Axis);

        // Player A's Movement & Combat is the Axis; Player B's is the Commonwealth.
        while g.phase != Phase::MovementAndCombat {
            g.advance_phase();
        }
        assert_eq!(g.active_player, Side::Axis);
        g.advance_phase();
        while g.phase != Phase::MovementAndCombat {
            g.advance_phase();
        }
        assert_eq!(g.active_player, Side::Commonwealth);
    }

    #[test]
    fn formation_membership_and_cpa() {
        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let hq = g.spawn_unit(Nationality::Commonwealth, "h", "Div HQ", Composition::Organic { current_toe: 1 });
        let a = g.spawn_unit(Nationality::Commonwealth, "h", "Bn A", Composition::Organic { current_toe: 5 });
        let b = g.spawn_unit(Nationality::Commonwealth, "k", "Bn B", Composition::Organic { current_toe: 5 });
        let grandchild = g.spawn_unit(Nationality::Commonwealth, "h", "Coy", Composition::Organic { current_toe: 2 });
        let indep = g.spawn_unit(Nationality::Commonwealth, "h", "Indep", Composition::Organic { current_toe: 5 });
        g.unit_mut(a).unwrap().attached_parent = Some(hq);
        g.unit_mut(b).unwrap().attached_parent = Some(hq);
        g.unit_mut(grandchild).unwrap().attached_parent = Some(a); // transitive

        let mut mem = g.formation_members(hq);
        mem.sort();
        let mut exp = vec![hq, a, b, grandchild];
        exp.sort();
        assert_eq!(mem, exp);
        assert!(!g.formation_members(hq).contains(&indep));

        // §6.15: formation CPA = the lowest member CPA.
        let expected = [hq, a, b, grandchild]
            .iter()
            .filter_map(|&id| g.unit_cpa(id))
            .min();
        assert!(expected.is_some());
        assert_eq!(g.formation_cpa(hq), expected);
    }

    #[test]
    fn shell_reduces_org_and_stacking() {
        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        // "h" infantry battalion, max TOE 5. TOE 2 (<50%) -> shell -> Company (0 SP).
        let depleted = g.spawn_unit(Nationality::Commonwealth, "h", "spent bn", Composition::Organic { current_toe: 2 });
        assert!(g.is_shell(depleted));
        assert_eq!(g.effective_org_size(depleted), OrgSize::Company);
        assert_eq!(g.effective_stacking_points(depleted), 0);
        // TOE 3 (>=50% of 5) -> not a shell -> Battalion (1 SP).
        let ok = g.spawn_unit(Nationality::Commonwealth, "h", "full bn", Composition::Organic { current_toe: 3 });
        assert!(!g.is_shell(ok));
        assert_eq!(g.effective_stacking_points(ok), 1);
        // A Division-org unit is not flagged by the battalion clause (deferred).
        let div = g.spawn_unit(Nationality::Commonwealth, "h", "div", Composition::Organic { current_toe: 1 });
        g.unit_mut(div).unwrap().org_size = OrgSize::Division;
        assert!(!g.is_shell(div));
        assert_eq!(g.effective_org_size(div), OrgSize::Division);
    }
}
