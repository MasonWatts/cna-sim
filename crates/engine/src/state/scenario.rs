//! Scenario setup: load an initial order of battle (units + starting hexes) and the
//! turn/initiative state into a `GameState` from a RON file.
//!
//! Places units at their hexes and wires division org — `assigned_parent` and
//! `attached_parent` are set from the OB (§19.1), later-arriving units register as pending
//! reinforcements, and starting supply dumps / repair facilities are placed. Real starting TOE is
//! resolved from the §4.44 codes (Organic `N`→id Max-TOE, `U@x`/`O@x`→x; Weapons carry chart
//! counts). Starting trucks / air loadout beyond the OB are minimal.

use serde::Deserialize;

use crate::reference::ob::{Arrival, ObComposition, Toe};
use crate::reference::units::{Nationality, Side};
use crate::state::game::{Commodities, GameState, PendingArrival, PendingDumpChoice, SupplyDump};
use crate::state::game::FacilityKind;
use crate::state::map::{HexCoord, HexId};
use crate::state::unit::{
    Composition, FormationRole, OrgSize, TruckPoints, UnitId, UnitLocation, WeaponGroup,
};

/// Error loading a scenario.
#[derive(Debug)]
pub enum ScenarioError {
    Ron(ron::error::SpannedError),
    BadHex(String),
    /// An `attached_to`/`assigned_to` referenced a unit name that isn't in the scenario.
    UnknownParent(String),
    /// A placement failed to deploy from the Order of Battle.
    Deploy(DeployError),
}

/// Error deploying an OB formation.
#[derive(Debug, PartialEq, Eq)]
pub enum DeployError {
    NoOrderOfBattle(Nationality),
    NoFormation(String),
    BadHex(String),
    /// A reinforcement has no entry hex: no per-unit override and no nationality entry
    /// point in the reference data (§20.14–20.15) — e.g. Axis/Tripoli before off-map nodes.
    NoEntryPoint(Nationality),
}

impl GameState {
    /// Instantiate an Order-of-Battle formation onto the map at `hex`: spawn its
    /// *deployed* units (reinforcements with a later `Arrival` are left for the schedule),
    /// rebuild the parent hierarchy (attached to the counter above), place the top-level
    /// counter(s) at `hex`, and set morale (current = arrival value, reference = Basic).
    /// Returns the spawned unit ids. (§4.44 → game state.)
    pub fn deploy_formation(
        &mut self,
        nationality: Nationality,
        formation_name: &str,
        hex: HexCoord,
    ) -> Result<Vec<UnitId>, DeployError> {
        self.deploy_formation_dispersed(nationality, formation_name, hex, &std::collections::HashMap::new())
    }

    /// Deploy an OB formation, optionally dispersing named units to their own hexes.
    /// `unit_hexes` maps a unit's name to the hex it deploys at; such a unit is placed
    /// there and NOT co-located-attached to its parent, but remains organically assigned
    /// to the formation (so formation CPA / command still apply). Units absent from the
    /// map deploy concentrated at `hex` as usual.
    pub fn deploy_formation_dispersed(
        &mut self,
        nationality: Nationality,
        formation_name: &str,
        hex: HexCoord,
        unit_hexes: &std::collections::HashMap<String, HexCoord>,
    ) -> Result<Vec<UnitId>, DeployError> {
        self.deploy_formation_dispersed_at(nationality, formation_name, UnitLocation::OnMap(hex), unit_hexes)
    }

    /// As [`Self::deploy_formation_dispersed`], but the concentration point may be an off-map node
    /// (§8.8 — e.g. Tripoli) as well as an on-map hex. Used by the initial-deployment `placements`
    /// loader so §60.31/§60.41 chart lines that set up at Tripoli/Tripolitania can be honored.
    pub fn deploy_formation_dispersed_at(
        &mut self,
        nationality: Nationality,
        formation_name: &str,
        entry: UnitLocation,
        unit_hexes: &std::collections::HashMap<String, HexCoord>,
    ) -> Result<Vec<UnitId>, DeployError> {
        // Deploy the units at the start of the game (Arrival::Deployed).
        let ids = self.deploy_ob_units(nationality, formation_name, entry.clone(), unit_hexes, |a| {
            matches!(a, Arrival::Deployed)
        })?;
        // Register the formation's later-arriving units as pending reinforcements. They enter at
        // their nationality's debarkation point (§20.14 Cairo / §20.15 Tripoli), not where the
        // formation set up; only a scenario without one falls back to this placement's entry.
        let (late_entry, area) = match self
            .reference
            .reinforcement_entry_hex(nationality)
            .map(str::to_string)
            .and_then(|s| self.resolve_entry(&s).map(|e| (e, self.resolve_entry_area(&s))))
        {
            Some(found) => found,
            None => (entry, Vec::new()),
        };
        self.register_pending_arrivals(nationality, formation_name, late_entry, area, unit_hexes);
        Ok(ids)
    }

    /// Register a formation's later-arriving (`Arrival::Turn`) units as pending
    /// reinforcements — one entry per distinct (game_turn, stage) — reusing `entry` + dispersion.
    fn register_pending_arrivals(
        &mut self,
        nationality: Nationality,
        formation_name: &str,
        entry: UnitLocation,
        area: Vec<HexCoord>,
        unit_hexes: &std::collections::HashMap<String, HexCoord>,
    ) {
        let mut times = std::collections::BTreeSet::new();
        if let Some(form) = self
            .reference
            .orders_of_battle
            .get(&nationality)
            .and_then(|ob| ob.formation(formation_name))
        {
            for u in &form.units {
                if let Arrival::Turn { game_turn, stage } = u.arrival {
                    times.insert((game_turn as u32, stage));
                }
            }
        }
        for (game_turn, stage) in times {
            self.pending_arrivals.push(PendingArrival {
                nationality,
                formation: formation_name.to_string(),
                entry: entry.clone(),
                unit_hexes: unit_hexes.iter().map(|(k, v)| (k.clone(), *v)).collect(),
                game_turn,
                stage,
                area: area.clone(),
            });
        }
    }

    /// Register a whole-formation reinforcement (§20): none of its units are on the map at
    /// start; they enter at `entry` on their scheduled (game_turn, stage) via the arrival
    /// driver, building the formation's hierarchy there.
    pub fn register_reinforcement(
        &mut self,
        nationality: Nationality,
        formation_name: &str,
        entry: UnitLocation,
        unit_hexes: &std::collections::HashMap<String, HexCoord>,
    ) -> Result<(), DeployError> {
        self.register_reinforcement_in_area(nationality, formation_name, entry, Vec::new(), unit_hexes)
    }

    /// As [`Self::register_reinforcement`], spreading the arrivals over `area` (§20.14).
    pub fn register_reinforcement_in_area(
        &mut self,
        nationality: Nationality,
        formation_name: &str,
        entry: UnitLocation,
        area: Vec<HexCoord>,
        unit_hexes: &std::collections::HashMap<String, HexCoord>,
    ) -> Result<(), DeployError> {
        self.reference
            .orders_of_battle
            .get(&nationality)
            .ok_or(DeployError::NoOrderOfBattle(nationality))?
            .formation(formation_name)
            .ok_or_else(|| DeployError::NoFormation(formation_name.to_string()))?;
        self.register_pending_arrivals(nationality, formation_name, entry, area, unit_hexes);
        Ok(())
    }

    /// Spawn + place the units of an OB formation whose `Arrival` satisfies `arrives`.
    /// Parents are resolved among units spawned this pass, then among units already on the
    /// map — so scheduled reinforcements attach to their already-deployed parent. Used for
    /// both the initial deploy (Deployed units) and scheduled arrivals (a given turn/stage).
    pub(crate) fn deploy_ob_units(
        &mut self,
        nationality: Nationality,
        formation_name: &str,
        entry: UnitLocation,
        unit_hexes: &std::collections::HashMap<String, HexCoord>,
        arrives: impl Fn(&Arrival) -> bool,
    ) -> Result<Vec<UnitId>, DeployError> {
        use std::collections::HashMap;

        let form = self
            .reference
            .orders_of_battle
            .get(&nationality)
            .ok_or(DeployError::NoOrderOfBattle(nationality))?
            .formation(formation_name)
            .ok_or_else(|| DeployError::NoFormation(formation_name.to_string()))?
            .clone();

        let arrival_morale = form.arrival_morale.unwrap_or(form.basic_morale);
        let mut by_name: HashMap<&str, UnitId> = HashMap::new();
        let mut ids = Vec::new();

        // Pass 1: spawn the units whose arrival matches.
        for u in &form.units {
            if !arrives(&u.arrival) {
                continue;
            }
            let placeholder = match &u.comp {
                ObComposition::Organic(_) => Composition::Organic { current_toe: 0 },
                ObComposition::Weapons(groups) => Composition::Weapons {
                    groups: groups
                        .iter()
                        .map(|(system, count)| WeaponGroup { system: system.clone(), count: *count })
                        .collect(),
                },
            };
            let id = self.spawn_unit(nationality, u.id.clone(), u.name.clone(), placeholder);
            {
                let unit = self.unit_mut(id).expect("just spawned");
                unit.abbr = u.abbr.clone(); // §4.44 Counter Abbreviation (short counter name)
                unit.org_size = u.org_size;
                unit.basic_morale = arrival_morale;
                unit.basic_morale_reference = form.basic_morale;
                // §9.12: "HQ units, whether Division, Brigade, or Regiment" — every HQ counter in the
                // OB (all are named "… HQ"/"… HQE"), not only the top of a formation.
                unit.is_hq = u.name.contains("HQ");
                unit.airdroppable = u.airdroppable; // §42.41 (from the OA sheet)
                unit.is_tds = u.is_tds; // §22.6 Tank Delivery Squadron / Mobile Tank Repair Squad
                unit.unit_type = u.unit_type; // leaf type, if the OB sets it
                unit.mandatory_withdrawal = u.withdrawal; // §20.8 [4.43a] withdrawal deadline
                unit.pending_refit = u.refit.clone(); // §4.44a scheduled in-place equipment refit
                unit.can_refit_deacons = u.deacon_refit; // §4.44a footnote b (102nd AT Regt)
                unit.return_at = u.return_at; // §20.8 [4.43a] scheduled return ("Rtn")
                unit.return_toe = u.return_toe;
                unit.withdrawal_after_return = u.withdrawal_after_return; // "[again]" re-withdrawal
                unit.return_at_2 = u.return_at_2; // second return (double-cycle units)
                // §9.16a: a garrison unit's assigned hex (per-unit override, else the formation's).
                unit.garrison_location = u
                    .garrison
                    .as_deref()
                    .or(form.garrison_hex.as_deref())
                    .and_then(|s| crate::state::map::HexId::parse(s).map(|h| h.to_hex_coord()));
            }
            // Resolve Organic TOE now that characteristics are known.
            if let ObComposition::Organic(toe) = &u.comp {
                let n = match toe {
                    Toe::Normal => self.unit_max_toe(id).unwrap_or(0),
                    Toe::Under(x) | Toe::Over(x) => *x,
                };
                if let Some(unit) = self.unit_mut(id) {
                    unit.composition = Composition::Organic { current_toe: n };
                }
                // §49.14: full tanks for the real TOE (organic Recce/AC burn fuel per TOE point).
                self.refuel_to_capacity(id);
            }
            by_name.insert(&u.name, id);
            ids.push(id);
        }

        // Pass 2: place top-level counters at `entry` (on-map hex or off-map node); attach
        // sub-units up the chain. A unit with a per-unit hex override deploys dispersed at
        // its own hex (assigned but not co-located; on-map only). Parents may be from this
        // pass or already on the map (arrivals).
        for u in &form.units {
            let Some(&id) = by_name.get(u.name.as_str()) else {
                continue;
            };
            if u.parent.is_empty() {
                let loc = match unit_hexes.get(&u.name) {
                    Some(&h) => UnitLocation::OnMap(h),
                    None => entry.clone(),
                };
                self.unit_mut(id).expect("spawned").location = Some(loc);
                continue;
            }
            let parent = by_name
                .get(u.parent.as_str())
                .copied()
                .or_else(|| self.units.values().find(|x| x.name == u.parent).map(|x| x.id));
            if let Some(&own) = unit_hexes.get(&u.name) {
                let unit = self.unit_mut(id).expect("spawned");
                unit.location = Some(UnitLocation::OnMap(own));
                if !u.attached {
                    unit.assigned_parent = parent;
                }
            } else if let Some(p) = parent.filter(|&p| by_name.values().any(|&x| x == p) || self.root_location(p) == Some(entry.clone())) {
                let unit = self.unit_mut(id).expect("spawned");
                unit.attached_parent = Some(p); // co-located; effective_hex follows up
                if !u.attached {
                    unit.assigned_parent = Some(p); // organically assigned (§19.1)
                }
            } else if let Some(p) = parent {
                // §20.14/§20.15: a reinforcement whose parent is already elsewhere arrives at the
                // debarkation point on its own counter (still organically assigned) and must travel
                // to rejoin — it does not appear on its parent's counter.
                let unit = self.unit_mut(id).expect("spawned");
                unit.location = Some(entry.clone());
                if !u.attached {
                    unit.assigned_parent = Some(p);
                }
            } else {
                // Parent not on the map (unexpected for internal arrivals): stand at `entry`.
                self.unit_mut(id).expect("spawned").location = Some(entry.clone());
            }
        }
        Ok(ids)
    }
}


fn default_turn() -> u32 {
    1
}

#[derive(Deserialize)]
/// §31 Rommel placement in a scenario: his starting hex and the Game-Turn his counter enters.
struct RommelPlacement {
    hex: String,
    #[serde(default = "default_turn")]
    arrival_game_turn: u32,
}

#[derive(Deserialize)]
struct ScenarioData {
    #[allow(dead_code)]
    name: String,
    #[serde(default = "default_turn")]
    game_turn: u32,
    #[serde(default)]
    first_turn_initiative: Option<Side>,
    /// §31: Rommel's counter, if this scenario includes it — his starting hex and the Game-Turn he
    /// arrives (`arrival_game_turn`). Omit when Rommel is not in the scenario.
    #[serde(default)]
    rommel: Option<RommelPlacement>,
    /// Formations deployed from the Order of Battle (§4.44) at a hex.
    #[serde(default)]
    placements: Vec<Placement>,
    /// Individually-listed units (flat), for forces not yet in the OB.
    #[serde(default)]
    units: Vec<ScenarioUnit>,
    /// Whole-formation reinforcements (§20): not on the map at start; they enter at their
    /// nationality's entry point (or an explicit `hex` override) on their OB arrival turn.
    #[serde(default)]
    reinforcements: Vec<Reinforcement>,
    /// §53.11 First Line Trucks from the deployment chart's "Trucks:" entries, attached to the
    /// named (on-map) unit — normally the parent combat unit of that chart line.
    #[serde(default)]
    first_line_trucks: Vec<FirstLineTrucksSpec>,
    /// §53.12/§53.13 Second/Third Line Trucks (§60.33 / §60.43): free truck convoys at a hex.
    #[serde(default)]
    truck_convoys: Vec<TruckConvoySpec>,
    /// Initial Supply Units (§32.1) placed at setup, at a hex label or off-map node.
    #[serde(default)]
    supply_dumps: Vec<SupplyDumpSpec>,
    /// Player-choice Supply Unit groups (§59.51 / §60.92): location decided later, validated
    /// against a constraint. Loaded into `GameState::pending_dump_choices`.
    #[serde(default)]
    supply_dump_choices: Vec<PendingDumpChoice>,
    /// §20.81/§20.82: by-type Mandatory Withdrawal choices (e.g. the 1st Armored Bde, "any 3 bns")
    /// the player must satisfy. Loaded into `GameState::withdrawal_choices`.
    #[serde(default)]
    withdrawal_choices: Vec<crate::state::game::WithdrawalChoice>,
    /// Repair Facilities present at start (§22.3 / §60.31 / §60.44).
    #[serde(default)]
    repair_facilities: Vec<FacilitySpec>,
    /// Fortifications placed at setup (§61.6 / §63.7x): hex label + level (1 or 2).
    #[serde(default)]
    fortifications: Vec<FortSpec>,
    /// Air Ground Support Units on the map (or in a Med Box) at start (§35 / §60 air setup).
    #[serde(default)]
    squadrons: Vec<SquadronSpec>,
    /// Air facilities (airfields / landing strips / flying-boat basins) present at start (§36).
    #[serde(default)]
    air_facilities: Vec<AirFieldSpec>,
    /// §30.17/§55.25: the scuttled San Giorgio blocking a harbour at start (Tobruk), if present.
    #[serde(default)]
    san_giorgio: Option<SanGiorgioSpec>,
    /// §30/§30.6: the Commonwealth Mediterranean Fleet — ships deployed at start and scheduled
    /// arrivals. Each seeds `GameState.fleet` (Deployed) or `pending_ships` (a Turn arrival).
    #[serde(default)]
    ships: Vec<ShipSpec>,
    /// §44.1: the Malta air facilities (Takali/Luqa/Safi/Hal Far airfields, Kalafrana basin, Valetta
    /// alighting area). Seeds `GameState.malta_bases`.
    #[serde(default)]
    malta_bases: Vec<MaltaBaseSpec>,
    /// Initial plane pools (§60.32 Italian / §60.42 Commonwealth): the aircraft each nationality has
    /// available at Game-Turn 1, which the player assigns to squadrons/airfields (§35.2). Seeds
    /// `GameState.plane_pool`.
    #[serde(default)]
    initial_planes: Vec<PlanePoolSpec>,
    /// Initial pilot pools (§60.32 Italian / §60.42 Commonwealth): the rated pilots each nationality
    /// has available at Game-Turn 1, which the player assigns to squadrons (§40.12). Seeds
    /// `GameState.pilot_pool`.
    #[serde(default)]
    initial_pilots: Vec<PilotPoolSpec>,
    /// Available SGSU counters per nationality (§60.32 Italian 39 / §60.42 Commonwealth 14): the
    /// supply the player deploys with `DeploySgsu`. Seeds `GameState.available_sgsu`.
    #[serde(default)]
    available_sgsu: Vec<AvailSgsuSpec>,
}

/// A nationality's starting plane pool (§60.32 / §60.42): `(aircraft-type key, count)` pairs.
#[derive(Deserialize)]
struct PlanePoolSpec {
    nationality: Nationality,
    planes: Vec<(String, u32)>,
}

/// A nationality's starting pilot pool (§60.32 / §60.42): `(rating, count)` pairs.
#[derive(Deserialize)]
struct PilotPoolSpec {
    nationality: Nationality,
    pilots: Vec<(u32, u32)>,
}

/// A nationality's available (unplaced) SGSU counter supply (§60.32 / §60.42).
#[derive(Deserialize)]
struct AvailSgsuSpec {
    nationality: Nationality,
    count: u32,
}

/// A squadron (SGSU) placed at setup (§35). Based on the map (`at`) or in an off-map Med Box
/// (`med_base`, for Axis strategic bombers, §43.0). Its planes start ready + armed (flyable).
#[derive(Deserialize)]
struct SquadronSpec {
    nationality: Nationality,
    class: crate::reference::squadron::SquadronClass,
    designation: String,
    cpa: u32,
    #[serde(default)]
    at: Option<String>,
    #[serde(default)]
    med_base: Option<crate::state::air::MedBase>,
    #[serde(default)]
    planes: Vec<PlaneSpec>,
}

/// A plane group in a squadron's composition: an aircraft-type key (§4.44A chart) and a count.
#[derive(Deserialize)]
struct PlaneSpec {
    aircraft_type: String,
    count: u32,
}

/// An air facility placed at setup (§36).
#[derive(Deserialize)]
struct AirFieldSpec {
    at: String,
    kind: crate::state::air::AirFacilityKind,
    /// Capacity level in squadrons; defaults to the field's nominal max.
    #[serde(default)]
    capacity: Option<u32>,
    #[serde(default)]
    off_map: bool,
}

/// §30.17/§55.25: the scuttled San Giorgio blocking a harbour at start. `at` is its hex label,
/// `blocks` the harbour name (its efficiency block comes from the port chart's `starts_blocked`).
#[derive(Deserialize)]
struct SanGiorgioSpec {
    at: String,
    blocks: String,
    gun: u32,
    aa: u32,
}

/// §30/§30.6: a Commonwealth fleet ship. `gun`/`aa` are its Gun and AA ratings (from the counter).
/// `valetta` starts it at Valetta (Malta) instead of Alexandria. `arrival` is Deployed (GT1) or a
/// scheduled Turn arrival.
#[derive(Deserialize)]
struct ShipSpec {
    name: String,
    gun: u32,
    aa: u32,
    #[serde(default)]
    valetta: bool,
    #[serde(default)]
    arrival: crate::reference::ob::Arrival,
}

/// §44.1: a Malta air facility — name, §36 kind, and its (full) capacity level.
#[derive(Deserialize)]
struct MaltaBaseSpec {
    name: String,
    kind: crate::state::air::AirFacilityKind,
    capacity: u32,
}

/// A fortification placed at setup: hex label + level.
#[derive(Deserialize)]
struct FortSpec {
    at: String,
    level: u8,
}

/// A Repair Facility placed at setup: hex label + Temporary/Major.
#[derive(Deserialize)]
struct FacilitySpec {
    at: String,
    kind: FacilityKind,
}

#[derive(Deserialize)]
struct Reinforcement {
    nationality: Nationality,
    formation: String,
    /// Explicit entry hex (§20.14 exceptions, e.g. Alexandria). Omitted = the nationality
    /// reinforcement entry point (§20.14–20.15) from the reference data.
    #[serde(default)]
    hex: Option<String>,
    #[serde(default)]
    unit_hexes: Vec<(String, String)>,
}

/// An initial Supply Unit (§32.1) placed at setup. `at` is a hex label (e.g. "C4807") or an
/// off-map node name (e.g. "Tripoli"). `count` places several identical dumps at one location
/// (§32.14 limits: ≤5 in a Major City, ≤3 elsewhere, unlimited in the Tripoli/Tunisia boxes).
/// First-line trucks on one unit at setup (§53.11).
#[derive(Deserialize)]
struct FirstLineTrucksSpec {
    unit: String,
    #[serde(default)]
    light: u32,
    #[serde(default)]
    medium: u32,
    #[serde(default)]
    heavy: u32,
}

/// A free (second/third-line) truck convoy at setup (§53.12/§53.13).
#[derive(Deserialize)]
struct TruckConvoySpec {
    side: Side,
    at: String,
    #[serde(default)]
    light: u32,
    #[serde(default)]
    medium: u32,
    #[serde(default)]
    heavy: u32,
}

#[derive(Deserialize)]
struct SupplyDumpSpec {
    side: Side,
    at: String,
    #[serde(default = "default_dump_ammo")]
    ammo: u32,
    #[serde(default = "default_dump_fuel")]
    fuel: u32,
    #[serde(default)]
    stores: u32,
    #[serde(default)]
    water: u32,
    #[serde(default = "one_dump")]
    count: u32,
    /// A dummy Supply Unit (§32.18) — no real supplies; removed on discovery.
    #[serde(default)]
    dummy: bool,
    /// §57: an effectively bottomless base (Cairo/Alexandria) — never depletes, shows as ∞.
    #[serde(default)]
    unlimited: bool,
}

fn default_dump_ammo() -> u32 {
    40
}
fn default_dump_fuel() -> u32 {
    60
}
fn one_dump() -> u32 {
    1
}
#[derive(Deserialize)]
struct Placement {
    nationality: Nationality,
    formation: String,
    hex: String,
    /// Per-unit hex overrides for a dispersed formation (e.g. a covering screen). A named
    /// unit is placed at its own hex and detached from the stack, but stays organically
    /// assigned to the formation (§6.15 CPA / command). Units not listed use `hex`.
    #[serde(default)]
    unit_hexes: Vec<(String, String)>,
}

#[derive(Deserialize)]
struct ScenarioUnit {
    nationality: Nationality,
    id_code: String,
    name: String,
    /// §4.44 Counter Abbreviation — short counter name; empty falls back to `name` for display.
    #[serde(default)]
    abbr: String,
    #[serde(default)]
    org_size: OrgSize,
    /// Whether this counter is an HQ with a parenthesized Stacking Point value (§9.12).
    #[serde(default)]
    is_hq: bool,
    /// Leaf unit type (like the OB `unit_type`).
    #[serde(default)]
    unit_type: Option<crate::reference::formation_org::UnitType>,
    /// Starting hex label, e.g. `"C3920"`. Empty for attached units (they are represented
    /// by their parent's counter — no own location).
    #[serde(default)]
    hex: String,
    composition: Composition,
    /// The unit's trained/target Basic Morale (§17.31) — the reference it trains up to.
    #[serde(default)]
    basic_morale: i32,
    /// Green units (§17.31): the lower morale they start at and train up from. `None` = arrives at
    /// `basic_morale` (not green). Mirrors the OB `arrival_morale`.
    #[serde(default)]
    arrival_morale: Option<i32>,
    #[serde(default)]
    cohesion: i32,
    /// Name of the parent this unit is *attached* to (co-located, §19.1). Empty = none.
    #[serde(default)]
    attached_to: String,
    /// Name of the parent this unit is *assigned* to (organizational, §19.1). Empty = none.
    #[serde(default)]
    assigned_to: String,
    /// First-line Truck Points attached at setup (§53.11 / §60.31/60.41).
    #[serde(default)]
    trucks: TruckPoints,
}

impl GameState {
    /// Load a scenario RON: sets the turn/initiative state, spawns each unit (at its
    /// starting hex, unless it is attached and therefore represented by its parent), then
    /// resolves attachment/assignment links by unit name. Existing units are left in place
    /// (call on a fresh `GameState`).
    pub fn load_scenario(&mut self, ron_str: &str) -> Result<(), ScenarioError> {
        use std::collections::HashMap;

        let data: ScenarioData = ron::from_str(ron_str).map_err(ScenarioError::Ron)?;
        self.game_turn = data.game_turn;
        self.first_turn_initiative = data.first_turn_initiative;
        // §55.3: seed the port table from the reference chart so port efficiency/blocking is tracked.
        self.init_ports_from_chart();
        // §31: place the Rommel counter, if the scenario includes it.
        self.rommel = match data.rommel {
            Some(rp) => {
                let start = HexId::parse(&rp.hex)
                    .ok_or_else(|| ScenarioError::BadHex(rp.hex.clone()))?
                    .to_hex_coord();
                Some(crate::state::game::RommelState {
                    arrival_game_turn: rp.arrival_game_turn,
                    start_hex: Some(start),
                    hex: None,
                    in_germany: false,
                    mcp_left: 0,
                    stage_start_hex: None,
                    cpa_unit: None,
                })
            }
            None => None,
        };

        // Pass 1: spawn every unit and record its name -> id.
        let mut by_name: HashMap<&str, crate::state::unit::UnitId> = HashMap::new();
        for u in &data.units {
            let id = self.spawn_unit(
                u.nationality,
                u.id_code.clone(),
                u.name.clone(),
                u.composition.clone(),
            );
            let unit = self.unit_mut(id).expect("just spawned");
            unit.abbr = u.abbr.clone(); // §4.44 Counter Abbreviation
            unit.org_size = u.org_size;
            unit.is_hq = u.is_hq;
            unit.unit_type = u.unit_type;
            // §17.31: `basic_morale` in the scenario is the trained reference; a green unit may start
            // lower via `arrival_morale`. Non-green units have current == reference.
            unit.basic_morale_reference = u.basic_morale;
            unit.basic_morale = u.arrival_morale.unwrap_or(u.basic_morale);
            unit.cohesion = u.cohesion;
            unit.trucks = u.trucks;
            // A unit with its own hex gets a location; attached units (no hex) stay
            // locationless and are placed via their parent's counter (effective_hex).
            if !u.hex.is_empty() {
                let coord = HexId::parse(&u.hex)
                    .ok_or_else(|| ScenarioError::BadHex(u.hex.clone()))?
                    .to_hex_coord();
                self.unit_mut(id).expect("just spawned").location = Some(crate::state::unit::UnitLocation::OnMap(coord));
            }
            // §49.14: full tanks, now that its first-line trucks are known.
            if u.trucks.light + u.trucks.medium + u.trucks.heavy > 0 {
                self.refuel_to_capacity(id);
            }
            by_name.insert(&u.name, id);
        }

        // Pass 2: resolve attachment/assignment links (parents referenced by name).
        for u in &data.units {
            let id = by_name[u.name.as_str()];
            if !u.attached_to.is_empty() {
                let parent = *by_name
                    .get(u.attached_to.as_str())
                    .ok_or_else(|| ScenarioError::UnknownParent(u.attached_to.clone()))?;
                self.unit_mut(id).expect("spawned in pass 1").attached_parent = Some(parent);
            }
            if !u.assigned_to.is_empty() {
                let parent = *by_name
                    .get(u.assigned_to.as_str())
                    .ok_or_else(|| ScenarioError::UnknownParent(u.assigned_to.clone()))?;
                self.unit_mut(id).expect("spawned in pass 1").assigned_parent = Some(parent);
            }
        }

        // Deploy OB formations (§4.44) at their hexes.
        for p in &data.placements {
            // A placement's concentration point may be an on-map hex-id or an off-map node
            // name (§8.8 — e.g. "Tripoli"); per-unit overrides remain on-map hexes.
            let entry = self
                .resolve_entry(&p.hex)
                .ok_or_else(|| ScenarioError::BadHex(p.hex.clone()))?;
            let mut unit_hexes = std::collections::HashMap::new();
            for (name, h) in &p.unit_hexes {
                let c = HexId::parse(h)
                    .ok_or_else(|| ScenarioError::BadHex(h.clone()))?
                    .to_hex_coord();
                unit_hexes.insert(name.clone(), c);
            }
            self.deploy_formation_dispersed_at(p.nationality, &p.formation, entry, &unit_hexes)
                .map_err(ScenarioError::Deploy)?;
        }
        // Whole-formation reinforcements (§20): register them to enter at their nationality
        // entry point (or an explicit override) on their OB arrival turn.
        for r in &data.reinforcements {
            let entry_str = r
                .hex
                .clone()
                .or_else(|| self.reference.reinforcement_entry_hex(r.nationality).map(str::to_string))
                .ok_or(ScenarioError::Deploy(DeployError::NoEntryPoint(r.nationality)))?;
            let entry = self
                .resolve_entry(&entry_str)
                .ok_or_else(|| ScenarioError::BadHex(entry_str.clone()))?;
            let mut unit_hexes = std::collections::HashMap::new();
            for (name, h) in &r.unit_hexes {
                let c = HexId::parse(h)
                    .ok_or_else(|| ScenarioError::BadHex(h.clone()))?
                    .to_hex_coord();
                unit_hexes.insert(name.clone(), c);
            }
            let area = self.resolve_entry_area(&entry_str);
            self.register_reinforcement_in_area(r.nationality, &r.formation, entry, area, &unit_hexes)
                .map_err(ScenarioError::Deploy)?;
        }
        // §53.11 First Line Trucks on their chart units, and §53.12/13 free convoys.
        for t in &data.first_line_trucks {
            let id = self
                .units
                .values()
                .find(|u| u.name == t.unit && u.location.is_some())
                .map(|u| u.id)
                .ok_or_else(|| ScenarioError::UnknownParent(t.unit.clone()))?;
            let u = self.unit_mut(id).expect("found");
            u.trucks.light += t.light;
            u.trucks.medium += t.medium;
            u.trucks.heavy += t.heavy;
            // §49.14: a unit enters play with full tanks — including its trucks' consumption.
            self.refuel_to_capacity(id);
        }
        for c in &data.truck_convoys {
            let hex = HexId::parse(&c.at).ok_or_else(|| ScenarioError::BadHex(c.at.clone()))?.to_hex_coord();
            self.truck_convoys.push(crate::state::game::TruckConvoy {
                side: c.side,
                hex,
                trucks: crate::state::unit::TruckPoints { light: c.light, medium: c.medium, heavy: c.heavy },
                trucks_broken: Default::default(),
                cargo: Default::default(),
                cpa_used: 0,
                breakdown_points: 0,
                breakdown_checked_col: 0,
            });
        }
        // Initial Supply Units (§32.1).
        for d in &data.supply_dumps {
            let loc = self
                .resolve_entry(&d.at)
                .ok_or_else(|| ScenarioError::BadHex(d.at.clone()))?;
            for _ in 0..d.count {
                self.supply_dumps.push(SupplyDump {
                    location: loc.clone(),
                    side: d.side,
                    supply: Commodities {
                        fuel: d.fuel,
                        ammo: d.ammo,
                        stores: d.stores,
                        water: d.water,
                    },
                    dummy: d.dummy,
                    constructed: true, unlimited: d.unlimited,
                });
            }
        }
        // Player-choice Supply Units (§59.51 / §60.92): stored unplaced for a later decision.
        self.pending_dump_choices
            .extend(data.supply_dump_choices);
        // §20.81/§20.82 by-type Mandatory Withdrawal choices.
        self.withdrawal_choices.extend(data.withdrawal_choices);
        // Repair Facilities (§22.3).
        for f in &data.repair_facilities {
            let coord = HexId::parse(&f.at)
                .ok_or_else(|| ScenarioError::BadHex(f.at.clone()))?
                .to_hex_coord();
            self.repair_facilities.insert(coord, f.kind);
        }
        for f in &data.fortifications {
            let coord = HexId::parse(&f.at)
                .ok_or_else(|| ScenarioError::BadHex(f.at.clone()))?
                .to_hex_coord();
            // Keep the strongest if a hex is listed more than once.
            let e = self.fortifications.entry(coord).or_insert(0);
            *e = (*e).max(f.level);
        }
        // Air: starting SGSUs (§35) with their planes ready + armed, and air facilities (§36).
        let base_sqid = self.sgsus.keys().map(|k| k.0).max().unwrap_or(0) + 1;
        for (i, s) in data.squadrons.iter().enumerate() {
            let loc = match &s.at {
                Some(id) => Some(
                    HexId::parse(id).ok_or_else(|| ScenarioError::BadHex(id.clone()))?.to_hex_coord(),
                ),
                None => None,
            };
            let id = crate::state::air::SquadronId(base_sqid + i as u32);
            let mut sg = crate::state::air::Sgsu::new(id, s.nationality, s.class, s.designation.clone(), s.cpa, loc);
            sg.med_base = s.med_base;
            sg.planes = s
                .planes
                .iter()
                .map(|p| crate::state::air::PlaneGroup {
                    aircraft_type: p.aircraft_type.clone(),
                    count: p.count,
                    refitted: 0,
                    ready: p.count,
                    armed: p.count,
                })
                .collect();
            self.sgsus.insert(id, sg);
        }
        for a in &data.air_facilities {
            let hex = HexId::parse(&a.at)
                .ok_or_else(|| ScenarioError::BadHex(a.at.clone()))?
                .to_hex_coord();
            let mut f = crate::state::air::AirFacility::new(a.kind, hex);
            if let Some(c) = a.capacity {
                f.capacity_level = c;
            }
            f.off_map = a.off_map;
            self.add_air_facility(f);
        }
        // §30.17/§55.25: place the scuttled San Giorgio blocking Tobruk (the port's efficiency block
        // is already applied from the chart's `starts_blocked`; this adds the on-map wreck itself).
        if let Some(sg) = &data.san_giorgio {
            let hex = HexId::parse(&sg.at)
                .ok_or_else(|| ScenarioError::BadHex(sg.at.clone()))?
                .to_hex_coord();
            self.place_san_giorgio(hex, &sg.blocks, sg.gun, sg.aa);
        }
        // §30/§30.6: the Commonwealth fleet — deploy GT1 ships into `fleet`, schedule later arrivals.
        for s in &data.ships {
            let mut ship = crate::state::naval::Ship::new(s.name.clone(), s.gun, s.aa);
            if s.valetta {
                ship.location = crate::state::naval::ShipLocation::Valetta;
            }
            match s.arrival {
                crate::reference::ob::Arrival::Deployed => self.fleet.push(ship),
                crate::reference::ob::Arrival::Turn { game_turn, stage } => {
                    self.pending_ships.push(crate::state::game::PendingShip {
                        ship,
                        game_turn: game_turn as u32,
                        stage,
                    });
                }
            }
        }
        // §44.1: the Malta air facilities (start at full capacity).
        for m in &data.malta_bases {
            self.malta_bases.push(crate::state::air::MaltaAirBase::new(m.name.clone(), m.kind, m.capacity, m.capacity));
        }
        // §60.32/§60.42: seed each side's starting plane pool.
        for p in &data.initial_planes {
            let pool = self.plane_pool.entry(p.nationality).or_default();
            for (aircraft, n) in &p.planes {
                *pool.entry(aircraft.clone()).or_insert(0) += n;
            }
        }
        // §60.32/§60.42: seed each side's starting rated-pilot pool.
        for p in &data.initial_pilots {
            let pool = self.pilot_pool.entry(p.nationality).or_default();
            for (rating, n) in &p.pilots {
                *pool.entry(*rating).or_insert(0) += n;
            }
        }
        // §60.32/§60.42: seed each side's available SGSU counter supply.
        for s in &data.available_sgsu {
            *self.available_sgsu.entry(s.nationality).or_insert(0) += s.count;
        }
        // §19.3: record each formation's historical assignment capacity (its OA-sheet organization)
        // from the units assigned to it at start, for the §19.21 reassignment limit.
        self.initialize_assignment_capacities();
        Ok(())
    }

    /// §19.3: set each unit's `assigned_capacity` to the number of units assigned to it at scenario
    /// start (its OA-sheet organization). Called once after deployment.
    pub fn initialize_assignment_capacities(&mut self) {
        let mut counts: std::collections::HashMap<UnitId, u32> = std::collections::HashMap::new();
        // §19.5: a formation is armored if any unit assigned to it is a tank unit (its OB org).
        let mut armored: std::collections::HashSet<UnitId> = std::collections::HashSet::new();
        for (&id, u) in &self.units {
            if let Some(p) = u.assigned_parent {
                *counts.entry(p).or_insert(0) += 1;
                if self.unit_is_tank(id) {
                    armored.insert(p);
                }
            }
        }
        for (id, n) in counts {
            if let Some(u) = self.units.get_mut(&id) {
                u.assigned_capacity = n;
            }
        }
        for id in armored {
            if let Some(u) = self.units.get_mut(&id) {
                u.is_armor = true;
            }
        }
        // §19.5 named/special-arm roles, derived from the formation HQ's designation (the OB).
        for u in self.units.values_mut() {
            if !u.is_hq {
                continue;
            }
            let n = u.name.to_ascii_lowercase();
            if u.nationality.side() == crate::reference::units::Side::Commonwealth
                && (n.contains("selby") || n.contains("matruh"))
            {
                u.formation_role = FormationRole::MatruhSelby;
            } else if u.nationality == Nationality::German
                && (n.contains("artillery brigade")
                    || n.contains("artillerie")
                    || n.contains("arko"))
            {
                u.formation_role = FormationRole::ArtilleryBrigadeHq;
            }
        }
    }

    /// The hexes of a multi-hex entry area ("E1830/E1730/E1829") — empty for a single hex or node.
    pub(crate) fn resolve_entry_area(&self, s: &str) -> Vec<HexCoord> {
        if !s.contains('/') {
            return Vec::new();
        }
        s.split('/').filter_map(|p| HexId::parse(p.trim())).map(|h| h.to_hex_coord()).collect()
    }

    /// Resolve a reinforcement entry string to a `UnitLocation`: an on-map hex-id (e.g.
    /// "E1830"), or the name of an off-map node in the §8.8 complex (e.g. "Tripoli").
    /// An area of hexes ("E1830/E1730/…", §20.14 "any hex(es) of Cairo") resolves to its first hex.
    pub(crate) fn resolve_entry(&self, s: &str) -> Option<UnitLocation> {
        let s = s.split('/').next().unwrap_or(s).trim();
        if let Some(h) = HexId::parse(s) {
            return Some(UnitLocation::OnMap(h.to_hex_coord()));
        }
        if self
            .reference
            .off_map
            .as_ref()
            .and_then(|net| net.node(s))
            .is_some()
        {
            return Some(UnitLocation::OffMap { node: s.to_string(), transit: 0, toward_map: true });
        }
        None
    }
}
