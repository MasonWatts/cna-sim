//! Aircraft Characteristics (§4.44A / §34) — DATA layer for the Air Game.
//!
//! Each aircraft type has one or more loadout rows (the "or" variants: extra fuel or bombload,
//! §34.12), plus its mission capabilities (§39). Mission-capability symbols on the chart map to
//! [`MissionCap`]: `.` No, `!` Day (may not fly night missions), `N` Night-capable, `A`
//! Day + may strafe armour, `S` Strafe-only, `0` night-Scramble-only.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The seven plane types (§34.0). NightBomber is a bomber additionally rated for night bombing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AircraftClass {
    Fighter,
    FighterBomber,
    Bomber,
    DiveBomber,
    Transport,
    Reconnaissance,
    FlyingBoat,
}

/// A mission-capability level for a given role (the chart's `.`/`!`/`N`/`A`/`S`/`0` symbols).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissionCap {
    /// `.` — does not possess this mission capability.
    #[default]
    No,
    /// `!` — possesses it, but may not be flown on night missions.
    Day,
    /// `N` — possesses it and may be flown on night missions.
    Night,
    /// `A` — as Day, and in addition may strafe armored vehicles.
    DayStrafeArmor,
    /// `S` — may only strafe (may not be assigned bombing missions).
    StrafeOnly,
    /// `0` — may be assigned only Night Scramble missions (no daylight Scramble).
    NightScrambleOnly,
}

/// A specific air mission a plane may be assigned (§39). Every mission except [`Mission::Transfer`]
/// requires the matching capability on the Aircraft Characteristics Chart; transfer is available to
/// any plane (§39.18/§42.1). These correspond to the chart's mission columns (F/S/R/D/B/Transport).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mission {
    /// Offensive or Defensive Combat Air Patrol / strafing (chart column F).
    Cap,
    /// Scramble (chart column S).
    Scramble,
    /// Reconnaissance (chart column R).
    Recon,
    /// Strafe and/or any type of bombing — the "dual" column (chart column D).
    Strike,
    /// Maltese and land-support bombing (chart column B).
    ConvoyLandBombing,
    /// Transport mission (chart Transport column).
    Transport,
    /// Transfer — flying to a new base; any plane may transfer (§39.18/§42.1).
    Transfer,
}

/// The two kinds of Combat Air Patrol (§40.2). Offensive CAP flies to a hex to shoot down enemy
/// planes and — with 3+ fighters — projects an air Zone of Control (§40.23); it may initiate and
/// must engage enemies entering its hex/ZOC, and may not abort (§40.26). Defensive CAP screens
/// friendly planes/installations, has no air ZOC, does not auto-initiate, and may abort (§40.26).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CapType {
    Offensive,
    Defensive,
}

/// The land-support bombing mission types (§41.3), each resolved on a portion of the Air
/// Bombardment CRT (or, for mining, the Mining Harbors Table). The chart shorthand is noted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BombingMission {
    /// B-CU: anti-personnel — pins units (§41.31).
    Personnel,
    /// B-TC: truck convoys — truck points destroyed (§41.32).
    TruckConvoy,
    /// B-FS: flak destruction — AA TOE points destroyed (§41.33).
    FlakDestruction,
    /// B-CF: ships — damage points (§41.34).
    Ships,
    /// B-SD: supply dumps — percentage destroyed (§41.35).
    SupplyDump,
    /// B-AF: air facilities — capacity levels reduced (§41.36).
    AirFacility,
    /// B-F/C: major cities / fortifications — one level max per stage (§41.37).
    CityFortification,
    /// B-R: roads (§41.38).
    Road,
    /// B-RR: railroads (§41.38).
    Railroad,
    /// B-MH: mining harbors — resolved on the Mining Harbors Table (§41.39A).
    MiningHarbor,
    /// B-P: ports — efficiency-level loss (§41.39B).
    Port,
}

/// The anti-aircraft target groups planes are sorted into for flak fire (§46.26).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AaTargetGroup {
    /// (a) non-fighter planes on daytime missions.
    DayOtherMission,
    /// (b) non-fighter planes on night missions.
    NightOtherMission,
    /// (c) fighter-type CAP/scramble/interception by day.
    DayFighter,
    /// (d) fighter-type CAP/scramble/interception at night.
    NightFighter,
    /// (e) planes flying fighter-type strafing and/or dive-bombing.
    StrafeDiveBomb,
}

/// One loadout row of an aircraft (§34.12). Bombload/torpedo are in points ("-" → 0); `transfer`
/// notes a distinct transfer range if the listed one differs (usually 2× range, §34.11); a
/// `drop_tanks` range is one marked `*` on the chart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Loadout {
    pub range: u32,
    #[serde(default)]
    pub drop_tanks: bool,
    pub tac_air: i32,
    /// The chart's parenthesized TacAir: the plane may never be flown on a mission that permits it
    /// to *initiate* air-to-air combat (§4.44A).
    #[serde(default)]
    pub no_air_to_air: bool,
    pub maneuver: i32,
    #[serde(default)]
    pub bombload: u32,
    #[serde(default)]
    pub torpedo: u32,
    pub fuel: u32,
}

/// Mission capabilities across the six roles (§39): F = offensive/defensive CAP or Strafing;
/// S = Scramble; R = Reconnaissance; D = Strafe and/or any bombing; B = Axis Naval Convoy and Land
/// Support bombing; T = Transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Missions {
    #[serde(default)]
    pub cap: MissionCap,
    #[serde(default)]
    pub scramble: MissionCap,
    #[serde(default)]
    pub recon: MissionCap,
    #[serde(default)]
    pub strike: MissionCap,
    #[serde(default)]
    pub convoy_land_bombing: MissionCap,
    #[serde(default)]
    pub transport: MissionCap,
}

/// An aircraft type row of the §4.44A chart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Aircraft {
    pub class: AircraftClass,
    pub loadouts: Vec<Loadout>,
    #[serde(default)]
    pub missions: Missions,
    #[serde(default)]
    pub manufacturer: String,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl Missions {
    /// The capability level for a given mission role. [`Mission::Transfer`] is not a chart column
    /// (any plane may transfer, §39.18) and returns [`MissionCap::No`]; callers handle transfer
    /// separately.
    pub fn cap_for(&self, mission: Mission) -> MissionCap {
        match mission {
            Mission::Cap => self.cap,
            Mission::Scramble => self.scramble,
            Mission::Recon => self.recon,
            Mission::Strike => self.strike,
            Mission::ConvoyLandBombing => self.convoy_land_bombing,
            Mission::Transport => self.transport,
            Mission::Transfer => MissionCap::No,
        }
    }

    /// §39.2 combined mission: a plane with the "dual" (D) Strike capability may strafe and bomb the
    /// same target in one mission. True when the Strike column is any capability other than `No`.
    pub fn can_combine_strike_and_bomb(&self) -> bool {
        self.strike != MissionCap::No
    }
}

impl Aircraft {
    /// The maximum listed Range across the aircraft's loadouts (§34.11).
    pub fn max_range(&self) -> u32 {
        self.loadouts.iter().map(|l| l.range).max().unwrap_or(0)
    }
}

/// A nationality's Aircraft Characteristics Chart: name → aircraft.
pub type AircraftChart = HashMap<String, Aircraft>;
