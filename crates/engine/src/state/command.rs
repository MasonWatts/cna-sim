//! The command boundary (design.md §3.3): all state mutations from players/AI/UI flow
//! through `GameState::apply`, which validates legality and applies the effect.
//!
//! Multi-hex/continual movement is issued as repeated single-hex steps. The boundary now
//! covers the full land turn (movement with ZOC, stacking, CPA over-expenditure, reaction,
//! retreat, breakdown, and attached-unit movement; barrage/anti-armor/close assault) plus
//! the air, naval, logistics, and construction commands — see the `Command` enum below.

use crate::reference::crt::BarrageTargetClass;
use crate::reference::terrain::{Fixed, MoveCost};
use crate::reference::units::Side;
use crate::rules::movement::{step_cost, MoveMode};
use crate::state::combat::{AntiArmorOutcome, BarrageOutcome, CloseAssaultOutcome};
use crate::state::game::{GameState, Phase};
use crate::state::map::HexCoord;
use crate::state::unit::UnitId;

/// Which broken-down component to repair (§22).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairKind {
    /// §22.23 field repair of broken-down first-line trucks.
    Trucks,
    /// §22.8 field repair of broken-down tank/SP weapon-system TOE.
    Tanks,
    /// §22.24 field repair of a broken-down self-rated Recce/Armored-Car unit.
    Recce,
    /// §22.3 repair at a Repair Facility.
    Facility,
}

/// An action a player attempts. Validated and applied by [`GameState::apply`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Move one unit one hex to an adjacent, on-map hex, in the given movement mode.
    MoveStep {
        unit: UnitId,
        to: HexCoord,
        mode: MoveMode,
    },
    /// Move a whole formation (a parent counter plus its attached units) one hex. The
    /// parent counter moves and the attached units ride along; over-expenditure is judged
    /// against the formation CPA (§6.15, the slowest member).
    FormationMove {
        formation: UnitId,
        to: HexCoord,
        mode: MoveMode,
    },
    /// Barrage (§12.6) the units of a given class in an adjacent target hex. Per §12.23–24
    /// the firer names a hex and a target *class* (firing "blind"); the engine resolves
    /// against a unit of that class there.
    Barrage {
        attackers: Vec<UnitId>,
        target: HexCoord,
        target_class: BarrageTargetClass,
    },
    /// Anti-Armor fire (§14) by the attacking units against a target hex.
    AntiArmor {
        attackers: Vec<UnitId>,
        target_hex: HexCoord,
    },
    /// Close Assault (§15) by the attacking units against an adjacent target hex; every
    /// enemy unit in that hex defends together (§15.23).
    CloseAssault {
        attackers: Vec<UnitId>,
        target: HexCoord,
    },
    /// Satisfy a mandated Retreat (§15.82) for `unit` along `path` (a sequence of adjacent
    /// hexes leading away from the assault). An empty path means the unit stands. Each hex
    /// short of the mandate costs a 10% loss.
    Retreat {
        unit: UnitId,
        path: Vec<HexCoord>,
    },
    /// Reaction (§8.5): a non-phasing unit, with an enemy combat unit moved adjacent, moves
    /// away along `path` before it can be assaulted. Never enters an enemy ZOC (§8.55).
    React {
        unit: UnitId,
        path: Vec<HexCoord>,
    },
    /// Off-map movement (§8.8): advance an off-map unit one Operations Stage in `direction`,
    /// entering the map at the junction hex (Nofilia) when it completes the last leg toward it.
    MoveOffMap {
        unit: UnitId,
        direction: crate::state::game::OffMapDir,
    },
    /// §56.15: plan an Axis naval convoy on a shipping lane (Naval Convoy Schedule Phase). The
    /// allowable tonnage is derived from the Convoy Level Chart for `arrival_gt` and `die`.
    PlanConvoy {
        lane: u8,
        destination: String,
        arrival_gt: u32,
        fuel_tons: u32,
        ammo_tons: u32,
        stores_tons: u32,
        replacement_tons: u32,
        die: u32,
    },
    /// §41.6: resolve a Commonwealth strategic bombing attack on a planned convoy (Convoy
    /// Resolution Phase). `roll` is a 2d6 value encoded `10*d1+d2`.
    BombConvoy {
        convoy_index: usize,
        bomb_points: u32,
        torpedo_planes: u32,
        total_planes: u32,
        roll: u32,
    },
    /// §56.16: deliver an arrived convoy's surviving supply tonnage into supply points (Naval
    /// Convoy Arrival Phase).
    DeliverConvoy { convoy_index: usize },
    /// §20.66/§20.78C: plan `count` Replacement Points of a Production Chart `item` for
    /// `nationality` (Organization Phase). Validated against the chart's lifetime total and
    /// per-period maximum; on success the points are scheduled to arrive after the §20.21 delay
    /// (Axis 2 / Commonwealth 4 Game-Turns) and begin Training.
    PlanProduction {
        nationality: crate::reference::units::Nationality,
        item: String,
        count: u32,
    },
    /// §55.22: block a port's harbour by scuttling (an engineer/CW-HQ must be present).
    BlockHarbor { port: String, has_engineer: bool },
    /// §55.26: remove one level of harbour blocking with the required engineer units.
    UnblockHarbor { port: String, engineers: u32 },
    /// §35.2: assign `count` planes of a type/class to a squadron (Organization Phase).
    AssignSquadronPlanes {
        squadron: crate::state::air::SquadronId,
        aircraft_type: String,
        class: crate::reference::aircraft::AircraftClass,
        count: u32,
    },
    /// §41.36: bomb an air facility, reducing its capacity by `levels` (Land Support Air Phase).
    BombAirFacility {
        hex: HexCoord,
        kind: crate::state::air::AirFacilityKind,
        levels: u32,
    },
    /// §41.39/§55.27: lay a mine in a port's harbour by air (Land Support Air Phase).
    LayHarborMine { port: String },
    /// §55.27: an engineer attempts to clear a harbour mine (Repair Phase); `die` 1–3 succeeds.
    RemoveHarborMine { port: String, die: u32 },
    /// §42.53: Commonwealth naval reconnaissance of a shipping lane (Convoy Resolution Phase).
    /// `roll` is a 2d6 value encoded `10*d1+d2`.
    ReconConvoyLane { lane: u8, planes: u32, roll: u32 },
    /// §33.0 II.B / §44.42: the Axis determines Malta raid support at Availability `level` (1–4)
    /// with a 2d6 `roll` (Malta Availability Determination Phase).
    DetermineMaltaAvailability { level: u32, roll: u32 },
    /// §38.34: roll `base_die` to refit a squadron's un-refitted planes via the §38.37 table (Land
    /// Support Air Phase). `wrong_squadron` applies the §38.35 not-own-SGSU penalty. Returns the
    /// number refitted.
    RefitSquadron {
        squadron: crate::state::air::SquadronId,
        base_die: u32,
        wrong_squadron: bool,
    },
    /// §53.24: load supply onto a unit's first-line trucks (Organization Phase). Returns the amount
    /// actually loaded.
    LoadTrucks {
        unit: UnitId,
        commodity: crate::state::game::Commodity,
        amount: u32,
    },
    /// §53.24: unload supply from a unit's trucks into the hex (Organization Phase).
    UnloadTrucks {
        unit: UnitId,
        commodity: crate::state::game::Commodity,
        amount: u32,
    },
    /// §52.16: poison a well (Movement & Combat Phase); `die` decides success.
    PoisonWell { hex: HexCoord, die: u8 },
    /// §52.17: sweeten (un-poison) a well (Movement & Combat Phase); `die` decides success.
    SweetenWell { hex: HexCoord, die: u8 },
    /// §24.3: begin laying a minefield (real or `dummy`) in a hex (Organization Phase).
    LayMinefield { builder: UnitId, hex: HexCoord, dummy: bool },
    /// §24.3: begin clearing a minefield in a hex (Organization Phase).
    ClearMinefield { builder: UnitId, hex: HexCoord },
    /// §52.2: begin building a water pipeline into a hex (Organization Phase).
    BuildPipeline { builder: UnitId, hex: HexCoord },
    /// §24.4: begin building a fortification to `level` (1–2) in a hex (Organization Phase).
    BuildFortification { builders: Vec<UnitId>, hex: HexCoord, level: u8 },
    /// §24.6: begin building a railroad along the `a`–`b` hexside (Organization Phase).
    BuildRailroad { builders: Vec<UnitId>, a: HexCoord, b: HexCoord },
    /// §24.66: begin demolishing the railroad along the `a`–`b` hexside (Organization Phase).
    DemolishRailroad { builder: UnitId, a: HexCoord, b: HexCoord },
    /// §8.72: move a unit by rail (Commonwealth Rail Movement Phase).
    RailMove { unit: UnitId, to: HexCoord },
    /// §54.3: move supply by rail (Commonwealth Rail Movement Phase). Returns the points moved.
    RailMoveSupply {
        from: HexCoord,
        to: HexCoord,
        cargo: crate::state::game::RailCargo,
        points: u32,
    },
    /// §24.5: begin building a road along the `a`–`b` hexside (Organization Phase).
    BuildRoad { builder: UnitId, a: HexCoord, b: HexCoord },
    /// §64.75: withdraw an eligible Commonwealth unit from play (Organization Phase).
    WithdrawUnit { unit: UnitId },
    /// §64.75: return a withdrawn unit to `hex` (Organization Phase).
    ReturnUnit { unit: UnitId, hex: HexCoord },
    /// §20: absorb a Replacement-Point group (by index) into a unit (Organization Phase). Returns
    /// the points absorbed.
    AbsorbReplacement { index: usize, unit: UnitId },
    /// §49.17: siphon Fuel from one unit to another (Organization Phase). Returns the amount moved.
    SiphonFuel { donor: UnitId, receiver: UnitId, amount: u32 },
    /// §22: attempt to repair a broken-down unit's vehicles (Repair Phase). Returns points repaired.
    Repair { unit: UnitId, kind: RepairKind, roll: u8 },
    /// §18: place a unit in (or out of) Reserve status (Reserve Designation Phase).
    DesignateReserve { unit: UnitId, status: crate::state::unit::ReserveStatus },
    /// §30.15/§30.18: assign a Commonwealth warship to a location for the Operations Stage
    /// (Commonwealth Fleet Phase).
    AssignShip { ship_index: usize, location: crate::state::naval::ShipLocation },
    /// §30.37: repair a warship one stage toward its next Gun level (Commonwealth Fleet Phase;
    /// only in Alexandria).
    RepairShip { ship_index: usize },
    /// §30.2: off-shore bombardment by a Commonwealth warship at sea against an enemy combat unit
    /// in its coastal hex (any ship) or an adjacent hex (heavy ships only, at half strength).
    /// Fired in a Barrage Step of the Movement & Combat Phase; the Gun Rating is used directly as
    /// Actual Barrage Points (§30.22) and no ammunition is spent.
    OffShoreBombard {
        ship_index: usize,
        target: HexCoord,
        target_class: BarrageTargetClass,
    },
    /// §30.35: land artillery/emplaced guns in the same hex as a Commonwealth warship fire at it,
    /// resolved on the Air Bombardment Table (Movement & Combat Phase). Raw Barrage Points are
    /// halved (mobile guns) or taken whole (emplaced, zero-CPA guns) and fed as Bomb Points.
    ArtilleryBombardShip {
        attackers: Vec<UnitId>,
        ship_index: usize,
    },
    /// §30.39: the San Giorgio fires at a Commonwealth warship in an adjacent (sea) hex, using its
    /// Gun Rating as Bombardment Points on the Air Bombardment Table (Movement & Combat Phase).
    SanGiorgioAttackShip {
        ship_index: usize,
    },
    /// §44.24: an Axis raid on a Maltese air facility (Strategic Air Phase / Malta Raid). Resolved
    /// as a bombing mission on the airfield CRT group; the result reduces the base's capacity
    /// levels (§44.12/§44.21).
    BombMaltaBase {
        squadron: crate::state::air::SquadronId,
        planes: u32,
        bombload_per_plane: u32,
        base_index: usize,
        interceptors: Option<crate::state::game::Flight>,
        flak_destroy_roll: u32,
        flak_abort_roll: u32,
        bomb_roll: u32,
    },
    /// §44.13: the Commonwealth builds capacity levels on a Maltese air facility using the Malta
    /// Air Facility Construction Table (Organization Phase; once per Game-Turn per base).
    ConstructMaltaFacility {
        base_index: usize,
        die: u32,
    },
    /// §30.34/§41.34: bomb a Commonwealth warship (Land Support Air Phase, not at night). Resolves
    /// as a bombing mission on the "units" CRT group; the result is applied as Gun-Rating damage.
    BombShip {
        squadron: crate::state::air::SquadronId,
        planes: u32,
        bombload_per_plane: u32,
        ship_index: usize,
        interceptors: Option<crate::state::game::Flight>,
        flak_points: u32,
        flak_destroy_roll: u32,
        flak_abort_roll: u32,
        bomb_roll: u32,
    },
    /// §49.14: fill a land unit's fuel tanks to capacity (Organization Phase). Returns new Fuel.
    RefuelUnit { unit: UnitId },
    /// §50.17: rearm a land unit's onboard ammunition to a one-fire reserve (Organization Phase).
    RearmUnit { unit: UnitId },
    /// §33.0 IV.A: the Initiative player declares whether they move first this Operations Stage
    /// (Weather Phase).
    DeclareInitiativeOrder { initiative_moves_first: bool },
    /// §38.2/§38.3: ready (refuel + refit) up to `count` planes of a squadron (Land Support Air
    /// Phase). Returns the number newly readied.
    ReadySquadron { squadron: crate::state::air::SquadronId, count: u32 },
    /// §38.4: arm up to `count` ready planes of a squadron (Land Support Air Phase).
    ArmSquadron { squadron: crate::state::air::SquadronId, count: u32 },
    /// §37: fly `count` ready planes of a squadron on a mission (Land Support Air Phase); they
    /// become unready until refitted. Returns the number that flew.
    FlySquadronMission { squadron: crate::state::air::SquadronId, count: u32 },
    /// §39.0: fly a bombing mission (Land Support Air Phase): `planes` ready bombers of `squadron`
    /// each carrying `bombload_per_plane`, against `mission`'s target, through flak (`flak_points`
    /// with two rolls) and then the Air Bombardment CRT (`bomb_roll`). Resolves plane losses/aborts
    /// and the bombing result.
    FlyBombingMission {
        squadron: crate::state::air::SquadronId,
        planes: u32,
        bombload_per_plane: u32,
        mission: crate::reference::aircraft::BombingMission,
        /// Intercepting enemy CAP that attacks the bombers before flak (§39.0/§45); `None` if
        /// unopposed.
        interceptors: Option<crate::state::game::Flight>,
        flak_points: u32,
        flak_destroy_roll: u32,
        flak_abort_roll: u32,
        bomb_roll: u32,
        night: bool,
    },
    /// §40.2: fly `planes` fighters on Combat Air Patrol over `center` (Land Support Air Phase),
    /// validating F (CAP) capability (§40.21), flight range to the patrolled hex (§37.12), and the
    /// §37.22 weather grounding. The committed fighters fly (leave `ready`) and stand as a patrol
    /// for the stage; an offensive patrol of ≥3 fighters projects an air ZOC (§40.23) and will
    /// intercept enemy missions entering it (§40.26).
    DesignateCap {
        squadron: crate::state::air::SquadronId,
        center: HexCoord,
        cap_type: crate::reference::aircraft::CapType,
        planes: u32,
    },
    /// §40.5–40.8: fly `planes` on a strafing mission from the squadron's base to `target`,
    /// validating strafing capability (§40.51), flight range (§37.12) and weather (§37.22), then
    /// resolving on the Strafing Table with the §40.8 column shift for `strafe_target` and the
    /// target hex's Fortification Level, and a 2d6 `roll` (encoded `10*d1+d2`). (Land Support Air.)
    Strafe {
        squadron: crate::state::air::SquadronId,
        target: HexCoord,
        strafe_target: crate::rules::air::StrafeTarget,
        planes: u32,
        roll: u32,
    },
    /// §40.12: assign a pilot of `rating` (0 or one of 1/2/3/4/6, §40.15) to a squadron's roster
    /// (Organization Phase). The best flyable assigned pilot's Rating adds to the squadron's TacAir
    /// in air-to-air combat.
    AssignPilot { rating: u32, squadron: crate::state::air::SquadronId },
    /// §43.14: transfer a squadron between the Mediterranean Boxes (Italy/Sicily, Crete) and on-map
    /// Africa. Takes one Operations Stage of transit (the squadron may not fly until it arrives),
    /// and may not leave the Med if it would break the §43.11 strategic-bomber basing requirement.
    /// (Organization Phase.)
    TransferSquadron {
        squadron: crate::state::air::SquadronId,
        to: crate::state::air::MedBaseDest,
    },
    /// §42.4/§42.46: airdrop `amount` of `commodity` (Ammo or Stores only) from the squadron's base
    /// dumps into a `target` hex, using `planes` transport-capable planes. Validates transport
    /// capability, flight range (§37.12), weather (§42.45), and the §42.43/§42.44 drop-zone legality
    /// (clear/gravel/desert, not a major city, not enemy-occupied or in an enemy ZOC and within two
    /// hexes of a recognizable land feature — waived for a Friendly-occupied hex, §42.46). Delivered
    /// supplies are usable immediately. (Land Support Air Phase.)
    AirdropSupplies {
        squadron: crate::state::air::SquadronId,
        target: HexCoord,
        commodity: crate::state::game::Commodity,
        amount: u32,
        planes: u32,
    },
    /// §42.3/§42.37: air-transport a `unit` (infantry-without-trucks or motorcycle/recce, co-located
    /// with the transport squadron) to a friendly air facility at `target`, landing it there. The
    /// transported unit has used its CPA and may not move further this stage (§42.34). One transport
    /// plane carries one TOE Strength Point (§42.31 note). (Land Support Air Phase.)
    TransportUnit {
        squadron: crate::state::air::SquadronId,
        unit: UnitId,
        target: HexCoord,
    },
    /// §42.41/§42.42: airdrop an airborne-eligible `unit` (co-located with the transport squadron)
    /// into a `target` hex — like a transport mission but the unit is dropped, so no air facility is
    /// needed at the target (it must still start from the transports' hex). Validates the §42.43/44
    /// drop-zone, weather (§42.45), stacking, and the §42.48 once-per-12-stages limit. (Land Support
    /// Air Phase.)
    AirdropUnit {
        squadron: crate::state::air::SquadronId,
        unit: UnitId,
        target: HexCoord,
    },
    /// §37/§41: fly a land-support bombing mission from the squadron's base hex to a `target` hex,
    /// validating flight range/distance (§37.12), mission day/night capability (§39.42), and the
    /// sandstorm/rainstorm grounding of the base or target (§37.22) against the map; computing
    /// target flak (§46) from the enemy AA units in the target hex; and resolving interception
    /// (§40.26) against any enemy offensive CAP whose air ZOC covers the target — all from map
    /// state rather than caller-supplied numbers. (Land Support Air Phase.)
    FlyBombingMissionAt {
        squadron: crate::state::air::SquadronId,
        target: HexCoord,
        mission: crate::reference::aircraft::BombingMission,
        planes: u32,
        bombload_per_plane: u32,
        night: bool,
        /// §45: an optional friendly fighter squadron escorting the bombers — intercepting CAP must
        /// fight the escort before it can attack the bombers. (§43.24: a Crete-based bomber may not
        /// take an on-map/African escort.)
        escort: Option<crate::state::air::SquadronId>,
        flak_destroy_roll: u32,
        flak_abort_roll: u32,
        bomb_roll: u32,
    },
}

/// The result of successfully applying a [`Command`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandOutcome {
    Moved,
    Barrage(BarrageOutcome),
    AntiArmor(AntiArmorOutcome),
    CloseAssault(CloseAssaultOutcome),
    /// A retreat was carried out: `toe_removed` from any shortfall, `hexes_short` = the
    /// number of mandated hexes not retreated (§15.82).
    Retreated {
        toe_removed: u32,
        hexes_short: u8,
    },
    /// A reaction move was carried out (§8.5).
    Reacted,
    /// An off-map movement step was carried out (§8.8).
    OffMapMoved(crate::state::game::OffMapMove),
    /// A convoy was planned (§56.15).
    ConvoyPlanned,
    /// A convoy was bombed (§41.6): the percentage of cargo lost.
    ConvoyBombed(u32),
    /// A convoy was delivered (§56.16): the supply points landed.
    ConvoyDelivered(crate::state::game::Commodities),
    /// A harbour action succeeded (§55.2): the Ammunition/Stores cost to expend.
    HarborAction { ammo: u32, stores: u32 },
    /// Planes were assigned to a squadron (§35.2).
    SquadronPlanesAssigned,
    /// An air facility was bombed (§41.36).
    AirFacilityBombed,
    /// A harbour mine was laid by air (§41.39/§55.27).
    HarborMineLaid,
    /// A harbour-mine clearance attempt resolved (§55.27): whether the mine was removed.
    HarborMineRemoved(bool),
    /// §42.53 naval reconnaissance: whether the recon succeeded, and the revealed convoy tonnage in
    /// the lane if one is present (`None` = no convoy).
    ConvoyRecon { revealed: bool, convoy_tonnage: Option<u32> },
    /// §44.42 Malta availability: the (map%, strategic%) support, or `None` for "na" (no forces).
    /// §44.42/§44.25: the raw `(map_percent, strategic_percent)` for the chosen level/roll, and the
    /// `planes_available` from the map — the map-percent of the in-play Italy/Sicily bombers plus the
    /// strategic-percent added from out-of-play strategic aircraft (§43.0 basing).
    MaltaAvailability { percentages: Option<(u32, u32)>, planes_available: u32 },
    /// §38.34: the number of a squadron's planes successfully refitted.
    SquadronRefitted(u32),
    /// §53.24: supply loaded onto / unloaded from trucks (the amount moved).
    TrucksLoaded(u32),
    TrucksUnloaded(u32),
    /// §52.16/§52.17: whether a well was poisoned / sweetened.
    WellPoisoned(bool),
    WellSweetened(bool),
    /// §24: a construction/removal project was started.
    ConstructionStarted,
    /// §8.72: a unit was moved by rail.
    RailMoved,
    /// §54.3: supply moved by rail (the points moved).
    RailSupplyMoved(u32),
    /// §64.75: a unit was withdrawn / returned to play.
    UnitWithdrawn,
    UnitReturned,
    /// §20: Replacement Points absorbed into a unit.
    ReplacementAbsorbed(u32),
    /// §49.17: Fuel Points siphoned between units.
    FuelSiphoned(u32),
    /// §49.14: a land unit refuelled to capacity (new Fuel level).
    UnitRefueled(u32),
    /// §50.17: a land unit rearmed to a one-fire reserve (new Ammo level).
    UnitRearmed(u32),
    /// §33.0 IV.A: the Initiative order was declared for the Operations Stage.
    InitiativeDeclared,
    /// §38: planes readied / armed, and planes flown on a mission.
    SquadronReadied(u32),
    SquadronArmed(u32),
    SquadronFlew(u32),
    /// §39.0: a resolved bombing mission.
    BombingMission(crate::state::game::BombingMissionOutcome),
    /// §22: points repaired by a repair attempt.
    Repaired(u32),
    /// §18: a unit's Reserve status was set.
    ReserveDesignated,
    /// §30: a warship was assigned to a location / repaired one stage (new Gun Rating).
    ShipAssigned,
    ShipRepaired(u32),
    /// §30.34/§30.35/§30.39: Gun-Rating Damage Points inflicted on a warship by gunfire.
    ShipDamaged(i32),
    /// §44.24: a resolved Axis raid on a Maltese air base (with capacity levels lost).
    MaltaBaseBombed { mission: crate::state::game::BombingMissionOutcome, levels_lost: u32 },
    /// §44.13: capacity levels built on a Maltese air facility.
    MaltaFacilityConstructed(u32),
    /// §20.66/§20.78C: a production order was accepted — the points arrive on `arrival_game_turn`
    /// as `class` Replacement Points and require `tonnage` Axis Naval Convoy tons (0 for CW).
    ProductionPlanned {
        arrival_game_turn: u32,
        class: crate::state::game::ReplacementClass,
        tonnage: u32,
    },
    /// §40.2: a Combat Air Patrol was established — `fighters` planes flew to the patrolled hex.
    CapDesignated { fighters: u32 },
    /// §40.5–40.8: a strafing mission resolved with Strafing-Table `result` (0–3); `effect`
    /// describes what it did to the target (e.g. dump percent destroyed, pipeline destroyed).
    Strafed { result: u32, effect: crate::state::game::StrafeEffect },
    /// §40.12: a pilot was added to a squadron's roster.
    PilotAssigned,
    /// §42.4: supplies were airdropped into the target hex (usable immediately).
    SuppliesAirdropped { amount: u32 },
    /// §42.3: a unit was air-transported to a friendly air facility (and has used its CPA).
    UnitTransported,
    /// §42.4: an airborne unit was airdropped into the target hex.
    UnitAirdropped,
    /// §43.14: a squadron began a transfer (in transit for one Operations Stage).
    SquadronTransferred,
}

/// Why a command was rejected. The state is left unchanged on any error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    UnitNotFound,
    WrongPhase,
    UnitNotOnMap,
    NotAdjacent,
    DestinationOffMap,
    ProhibitedTerrain,
    /// §10.24: may not move directly from one enemy-controlled hex into another.
    ZocToZoc,
    /// §8.14: having moved into an enemy ZOC this stage, the unit must stop.
    MustStopInZoc,
    /// §9.14: ending the move would exceed the destination terrain's stacking limit.
    StackingLimitExceeded,
    /// §49.13/49.16: no Fuel is available (unit tanks + Supply Dumps in the start hex) to
    /// pay for the movement.
    InsufficientFuel,
    /// §50.12/50.14: no Ammunition is available (onboard reserve + Supply Dumps in the hex)
    /// for the firing unit(s) to perform the combat action.
    InsufficientAmmo,
    /// §52.51: a vehicle deprived of Water may not move.
    NoWater,
    /// §52.6: an Italian battalion that missed its Pasta Point may not voluntarily exceed CPA.
    NoPasta,
    /// §51.23: a unit on half rations may not voluntarily exceed CPA or enter an enemy ZOC.
    HalfRation,
    /// A combat command was issued with no attacking (or defending) units.
    NoParticipants,
    /// §3.33: an HQ with no combat units attached is not a combat unit and may not attack.
    NonCombatHq,
    /// The target hex holds no eligible enemy units (of the required class, for barrage).
    NoTargets,
    /// The attacking units do not all belong to the same side.
    AttackersMixedSides,
    /// A Retreat command was issued for a unit that owes no retreat.
    NoRetreatPending,
    /// §10.23: units may not retreat into an enemy Zone of Control.
    RetreatIntoZoc,
    /// §15.82: each retreat hex must lead away from the assaulted hex.
    RetreatNotAway,
    /// The unit is not eligible to React (§8.53: phasing, engaged, already in an enemy ZOC,
    /// or non-motorized).
    CannotReact,
    /// Reaction requires an enemy combat unit to have moved adjacent (§8.51).
    NoReactionTrigger,
    /// §8.55: a reacting unit may never move into an enemy Zone of Control.
    ReactIntoZoc,
    /// §8.53b/§8.54: a faster, large-enough adjacent enemy pins the unit (no reaction).
    Pinned,
    /// An off-map movement step failed (§8.8).
    OffMap(crate::state::game::OffMapError),
    /// §56.2: the convoy could not be planned (lane unavailable, over the allowable tonnage, or a
    /// convoy already occupies that lane this Game-Turn).
    ConvoyRejected,
    /// The Convoy Level Chart is not loaded, so allowable tonnage cannot be derived.
    NoConvoyTable,
    /// The referenced convoy index does not exist.
    InvalidConvoy,
    /// §55.2: the harbour action failed (port unblockable/unknown, no engineer, too few engineers,
    /// or already at maximum efficiency).
    HarborActionFailed,
    /// The referenced squadron (SGSU) does not exist.
    NoSuchSquadron,
    /// §35.21/§35.23: the planes cannot be assigned (wrong class for the squadron, or capacity
    /// would be exceeded).
    SquadronRejected,
    /// No matching air facility exists in the target hex.
    NoSuchFacility,
    /// The referenced port does not exist in play.
    NoSuchPort,
    /// A required reference table (naval recon, Malta availability, or aircraft refit) is not
    /// loaded.
    NoTable,
    /// §24: the construction/removal project could not be started (ineligible builder, terrain,
    /// supply, or conditions not met).
    ConstructionFailed,
    /// §8.72: the rail move was not allowed (no rail connection, allowance used, or ineligible).
    RailMoveFailed,
    /// §64.75: the unit is not eligible to withdraw / cannot be returned.
    WithdrawalFailed,
    /// §20: the Replacement Points could not be absorbed (still training, class/hex mismatch).
    ReplacementFailed,
    /// §20.66/§20.78C: the item is not on the acting nationality's Production Chart.
    NoSuchProductionItem,
    /// §20.66: planning this order would exceed the item's lifetime production total.
    ProductionExhausted,
    /// §20.66: planning this order would exceed the item's per-period maximum (or the item is
    /// not yet available on this Game-Turn).
    ProductionRateExceeded,
    /// §37: the squadron has no base hex (eliminated / in a pool) and cannot fly.
    SquadronNotBased,
    /// §37.16: the squadron has no ready planes to fly the mission.
    NoReadyPlanes,
    /// §37.12/§37.21: the target hex is beyond the flying planes' one-way range.
    TargetOutOfRange,
    /// §37.22: the base or target hex is under a Sandstorm/Rainstorm and flight is grounded.
    WeatherGrounded,
    /// §39.42: no flying plane type may perform this bombing mission by day/night as required.
    MissionNotPermitted,
    /// §40.15: a pilot Rating must be 0 or one of 1, 2, 3, 4, 6.
    InvalidPilotRating,
    /// §42.43: the airdrop target hex is not a legal drop zone (wrong terrain, a major city, enemy-
    /// occupied, in an enemy ZOC, or not within two hexes of a recognizable land feature).
    DropZoneIllegal,
    /// §42.48: the unit was airdropped within the last 12 Operations Stages and may not drop again.
    AirdropOnCooldown,
    /// §44.23/§44.41: this Malta Availability Level is not allowed for the scenario, or has already
    /// been used its maximum number of Game-Turns.
    MaltaLevelUnavailable,
    /// §43.11: the transfer would drop German strategic bombers below the 75% Mediterranean basing
    /// requirement.
    BasingRequirementViolated,
    /// §43.14: the squadron is in transfer transit this Operations Stage and may not fly.
    SquadronInTransit,
    /// §43.23: a Crete-based bomber may not fly (committed to Suez) this Operations Stage.
    CreteSuezCommitment,
    /// §45/§43.24: the escort squadron is ineligible (not a friendly ready fighter, in transit, or
    /// an on-map fighter escorting a Crete-based bomber).
    EscortNotPermitted,
    /// §38.36: no Stores Point is present at the squadron's base to pay for the refit attempt.
    InsufficientStores,
    /// §35.14: the SGSU lacks its own upkeep and may not repair (refit) its planes this stage.
    SgsuUnsupplied,
    /// §30: the referenced warship does not exist.
    NoSuchShip,
    /// §30.36: the ship is below 50% Gun Rating and may not leave port until repaired.
    ShipNotSeaworthy,
    /// §30.18: Valetta already holds the maximum five ships.
    ValettaFull,
    /// §30.37: the ship is not in Alexandria (repair only happens there).
    ShipNotInPort,
    /// §30.15/§30.2: the ship is not at sea, or the target is out of its bombardment range
    /// (adjacent-hex fire requires a heavy ship, §30.21).
    ShipCannotReachTarget,
    /// §30.16/§30.25: the ship still owes port rest (or has spent three consecutive stages at
    /// sea) and may not put to sea this stage.
    ShipMustRest,
    /// §30.24: the ship has already used its Barrage Strength this Operations Stage.
    ShipAlreadyFired,
    /// §44.1: the referenced Maltese air base does not exist.
    NoSuchMaltaBase,
}

impl GameState {
    /// Validate and apply a command. On `Err`, the game state is unchanged.
    pub fn apply(&mut self, command: Command) -> Result<CommandOutcome, CommandError> {
        match command {
            Command::MoveStep { unit, to, mode } => self.move_step(unit, to, mode),
            Command::FormationMove { formation, to, mode } => {
                self.formation_move(formation, to, mode)
            }
            Command::Barrage { attackers, target, target_class } => {
                self.cmd_barrage(&attackers, target, target_class)
            }
            Command::AntiArmor { attackers, target_hex } => {
                self.cmd_anti_armor(&attackers, target_hex)
            }
            Command::CloseAssault { attackers, target } => {
                self.cmd_close_assault(&attackers, target)
            }
            Command::Retreat { unit, path } => self.cmd_retreat(unit, &path),
            Command::React { unit, path } => self.cmd_react(unit, &path),
            Command::MoveOffMap { unit, direction } => self.cmd_move_off_map(unit, direction),
            Command::PlanConvoy {
                lane, destination, arrival_gt, fuel_tons, ammo_tons, stores_tons, replacement_tons, die,
            } => self.cmd_plan_convoy(
                lane, destination, arrival_gt, fuel_tons, ammo_tons, stores_tons, replacement_tons, die,
            ),
            Command::BombConvoy { convoy_index, bomb_points, torpedo_planes, total_planes, roll } => {
                self.cmd_bomb_convoy(convoy_index, bomb_points, torpedo_planes, total_planes, roll)
            }
            Command::DeliverConvoy { convoy_index } => self.cmd_deliver_convoy(convoy_index),
            Command::PlanProduction { nationality, item, count } => {
                self.cmd_plan_production(nationality, &item, count)
            }
            Command::BlockHarbor { port, has_engineer } => self.cmd_block_harbor(&port, has_engineer),
            Command::UnblockHarbor { port, engineers } => self.cmd_unblock_harbor(&port, engineers),
            Command::AssignSquadronPlanes { squadron, aircraft_type, class, count } => {
                self.cmd_assign_squadron_planes(squadron, &aircraft_type, class, count)
            }
            Command::BombAirFacility { hex, kind, levels } => {
                self.cmd_bomb_air_facility(hex, kind, levels)
            }
            Command::LayHarborMine { port } => self.cmd_lay_harbor_mine(&port),
            Command::RemoveHarborMine { port, die } => self.cmd_remove_harbor_mine(&port, die),
            Command::ReconConvoyLane { lane, planes, roll } => {
                self.cmd_recon_convoy_lane(lane, planes, roll)
            }
            Command::DetermineMaltaAvailability { level, roll } => {
                self.cmd_determine_malta_availability(level, roll)
            }
            Command::RefitSquadron { squadron, base_die, wrong_squadron } => {
                self.cmd_refit_squadron(squadron, base_die, wrong_squadron)
            }
            Command::LoadTrucks { unit, commodity, amount } => {
                self.require_phase(Phase::Organization)?;
                self.require_units_exist(&[unit])?;
                Ok(CommandOutcome::TrucksLoaded(self.load_trucks(unit, commodity, amount)))
            }
            Command::UnloadTrucks { unit, commodity, amount } => {
                self.require_phase(Phase::Organization)?;
                self.require_units_exist(&[unit])?;
                Ok(CommandOutcome::TrucksUnloaded(self.unload_trucks(unit, commodity, amount)))
            }
            Command::PoisonWell { hex, die } => {
                self.require_phase(Phase::MovementAndCombat)?;
                Ok(CommandOutcome::WellPoisoned(self.poison_well(hex, die)))
            }
            Command::SweetenWell { hex, die } => {
                self.require_phase(Phase::MovementAndCombat)?;
                Ok(CommandOutcome::WellSweetened(self.sweeten_well(hex, die)))
            }
            Command::LayMinefield { builder, hex, dummy } => {
                self.require_phase(Phase::Organization)?;
                self.require_units_exist(&[builder])?;
                if self.begin_minefield(builder, hex, dummy) {
                    Ok(CommandOutcome::ConstructionStarted)
                } else {
                    Err(CommandError::ConstructionFailed)
                }
            }
            Command::ClearMinefield { builder, hex } => {
                self.require_phase(Phase::Organization)?;
                self.require_units_exist(&[builder])?;
                if self.begin_minefield_removal(builder, hex) {
                    Ok(CommandOutcome::ConstructionStarted)
                } else {
                    Err(CommandError::ConstructionFailed)
                }
            }
            Command::BuildPipeline { builder, hex } => {
                self.require_phase(Phase::Organization)?;
                self.require_units_exist(&[builder])?;
                if self.build_pipeline(builder, hex) {
                    Ok(CommandOutcome::ConstructionStarted)
                } else {
                    Err(CommandError::ConstructionFailed)
                }
            }
            Command::BuildFortification { builders, hex, level } => {
                self.require_phase(Phase::Organization)?;
                self.require_units_exist(&builders)?;
                if self.begin_fortification(&builders, hex, level) {
                    Ok(CommandOutcome::ConstructionStarted)
                } else {
                    Err(CommandError::ConstructionFailed)
                }
            }
            Command::BuildRailroad { builders, a, b } => {
                self.require_phase(Phase::Organization)?;
                self.require_units_exist(&builders)?;
                if self.begin_railroad(&builders, a, b) {
                    Ok(CommandOutcome::ConstructionStarted)
                } else {
                    Err(CommandError::ConstructionFailed)
                }
            }
            Command::DemolishRailroad { builder, a, b } => {
                self.require_phase(Phase::Organization)?;
                self.require_units_exist(&[builder])?;
                if self.begin_railroad_demolition(builder, a, b) {
                    Ok(CommandOutcome::ConstructionStarted)
                } else {
                    Err(CommandError::ConstructionFailed)
                }
            }
            Command::RailMove { unit, to } => {
                self.require_phase(Phase::CommonwealthRail)?;
                self.require_units_exist(&[unit])?;
                if self.rail_move(unit, to) {
                    Ok(CommandOutcome::RailMoved)
                } else {
                    Err(CommandError::RailMoveFailed)
                }
            }
            Command::RailMoveSupply { from, to, cargo, points } => {
                self.require_phase(Phase::CommonwealthRail)?;
                Ok(CommandOutcome::RailSupplyMoved(self.rail_move_supply(from, to, cargo, points)))
            }
            Command::BuildRoad { builder, a, b } => {
                self.require_phase(Phase::Organization)?;
                self.require_units_exist(&[builder])?;
                if self.build_road(builder, a, b) {
                    Ok(CommandOutcome::ConstructionStarted)
                } else {
                    Err(CommandError::ConstructionFailed)
                }
            }
            Command::WithdrawUnit { unit } => {
                self.require_phase(Phase::Organization)?;
                self.require_units_exist(&[unit])?;
                if self.withdraw_unit(unit) {
                    Ok(CommandOutcome::UnitWithdrawn)
                } else {
                    Err(CommandError::WithdrawalFailed)
                }
            }
            Command::ReturnUnit { unit, hex } => {
                self.require_phase(Phase::Organization)?;
                self.require_units_exist(&[unit])?;
                if self.return_unit(unit, hex) {
                    Ok(CommandOutcome::UnitReturned)
                } else {
                    Err(CommandError::WithdrawalFailed)
                }
            }
            Command::AbsorbReplacement { index, unit } => {
                self.require_phase(Phase::Organization)?;
                self.require_units_exist(&[unit])?;
                match self.absorb_replacement(index, unit) {
                    Some(points) => Ok(CommandOutcome::ReplacementAbsorbed(points)),
                    None => Err(CommandError::ReplacementFailed),
                }
            }
            Command::SiphonFuel { donor, receiver, amount } => {
                self.require_phase(Phase::Organization)?;
                self.require_units_exist(&[donor, receiver])?;
                Ok(CommandOutcome::FuelSiphoned(self.siphon_fuel(donor, receiver, amount)))
            }
            Command::Repair { unit, kind, roll } => {
                self.require_phase(Phase::Repair)?;
                self.require_units_exist(&[unit])?;
                let repaired = match kind {
                    RepairKind::Trucks => self.field_repair_trucks(unit, roll),
                    RepairKind::Tanks => self.field_repair_tanks(unit, roll as u32),
                    RepairKind::Recce => self.field_repair_recce(unit, roll),
                    RepairKind::Facility => self.facility_repair(unit, roll as u32),
                };
                Ok(CommandOutcome::Repaired(repaired))
            }
            Command::DesignateReserve { unit, status } => {
                self.require_phase(Phase::ReserveDesignation)?;
                let u = self.unit_mut(unit).ok_or(CommandError::UnitNotFound)?;
                u.reserve = status;
                Ok(CommandOutcome::ReserveDesignated)
            }
            Command::AssignShip { ship_index, location } => {
                self.require_phase(Phase::CommonwealthFleet)?;
                self.cmd_assign_ship(ship_index, location)
            }
            Command::RepairShip { ship_index } => {
                self.require_phase(Phase::CommonwealthFleet)?;
                let ship = self.fleet.get_mut(ship_index).ok_or(CommandError::NoSuchShip)?;
                if ship.location != crate::state::naval::ShipLocation::Alexandria {
                    return Err(CommandError::ShipNotInPort);
                }
                ship.repair_tick();
                Ok(CommandOutcome::ShipRepaired(ship.gun_rating))
            }
            Command::OffShoreBombard { ship_index, target, target_class } => {
                self.cmd_off_shore_bombard(ship_index, target, target_class)
            }
            Command::ArtilleryBombardShip { attackers, ship_index } => {
                self.cmd_artillery_bombard_ship(&attackers, ship_index)
            }
            Command::SanGiorgioAttackShip { ship_index } => {
                self.require_combat_phase()?;
                let sg_hex = self.san_giorgio.as_ref().ok_or(CommandError::NoSuchShip)?.hex;
                let points = self.san_giorgio.as_ref().unwrap().bombardment_points();
                let crate::state::naval::ShipLocation::AtSea(hex) =
                    self.fleet.get(ship_index).ok_or(CommandError::NoSuchShip)?.location
                else {
                    return Err(CommandError::ShipCannotReachTarget);
                };
                // §30.17: the San Giorgio fires at Commonwealth ships in adjacent (sea) hexes.
                if !sg_hex.is_adjacent(hex) {
                    return Err(CommandError::ShipCannotReachTarget);
                }
                let roll = self.rng.sequential_read() as u32;
                Ok(CommandOutcome::ShipDamaged(self.resolve_gun_hit_on_ship(points, ship_index, roll)))
            }
            Command::BombMaltaBase {
                squadron, planes, bombload_per_plane, base_index, interceptors,
                flak_destroy_roll, flak_abort_roll, bomb_roll,
            } => {
                self.require_phase(Phase::MaltaRaid)?;
                if base_index >= self.malta_bases.len() {
                    return Err(CommandError::NoSuchMaltaBase);
                }
                if !self.sgsus.contains_key(&squadron) {
                    return Err(CommandError::NoSuchSquadron);
                }
                // §44.24: the raid uses the base's own diverted AA/Flak (§44.18) for flak.
                let flak_points = self.malta_bases[base_index].aa_points;
                let mission = self
                    .resolve_bombing_mission(
                        squadron, planes, bombload_per_plane,
                        crate::reference::air_bombardment::TargetGroup::AirfieldPortStrip,
                        interceptors, None, flak_points, flak_destroy_roll, flak_abort_roll, bomb_roll, false,
                    )
                    .ok_or(CommandError::NoSuchSquadron)?;
                // §44.12/§44.21: the result reduces the base's capacity levels (never destroyed).
                let levels_lost = mission.result.max(0) as u32;
                self.malta_bases[base_index].bomb(levels_lost);
                Ok(CommandOutcome::MaltaBaseBombed { mission, levels_lost })
            }
            Command::ConstructMaltaFacility { base_index, die } => {
                self.require_phase(Phase::Organization)?;
                let levels = self
                    .reference
                    .malta_construction
                    .as_ref()
                    .ok_or(CommandError::NoTable)?
                    .levels_built(die);
                let base = self.malta_bases.get_mut(base_index).ok_or(CommandError::NoSuchMaltaBase)?;
                Ok(CommandOutcome::MaltaFacilityConstructed(base.construct(levels)))
            }
            Command::BombShip {
                squadron, planes, bombload_per_plane, ship_index, interceptors,
                flak_points, flak_destroy_roll, flak_abort_roll, bomb_roll,
            } => {
                self.require_phase(Phase::LandSupportAir)?;
                if ship_index >= self.fleet.len() {
                    return Err(CommandError::NoSuchShip);
                }
                if !self.sgsus.contains_key(&squadron) {
                    return Err(CommandError::NoSuchSquadron);
                }
                // §30.34: Commonwealth-Fleet damage is read on the "units" section of the CRT; a
                // ship attack is never at night (§41.34).
                let outcome = self
                    .resolve_bombing_mission(
                        squadron, planes, bombload_per_plane,
                        crate::reference::air_bombardment::TargetGroup::Units,
                        interceptors, None, flak_points, flak_destroy_roll, flak_abort_roll, bomb_roll, false,
                    )
                    .ok_or(CommandError::NoSuchSquadron)?;
                self.fleet[ship_index].damage(outcome.result.max(0) as u32);
                Ok(CommandOutcome::BombingMission(outcome))
            }
            Command::RefuelUnit { unit } => {
                self.require_phase(Phase::Organization)?;
                self.require_units_exist(&[unit])?;
                self.refuel_to_capacity(unit);
                Ok(CommandOutcome::UnitRefueled(self.unit(unit).map_or(0, |u| u.fuel)))
            }
            Command::RearmUnit { unit } => {
                self.require_phase(Phase::Organization)?;
                self.require_units_exist(&[unit])?;
                self.rearm_fire_once(unit);
                Ok(CommandOutcome::UnitRearmed(self.unit(unit).map_or(0, |u| u.ammo)))
            }
            Command::DeclareInitiativeOrder { initiative_moves_first } => {
                self.require_phase(Phase::Weather)?;
                self.declare_initiative_order(initiative_moves_first);
                Ok(CommandOutcome::InitiativeDeclared)
            }
            Command::ReadySquadron { squadron, count } => {
                self.require_maintenance_phase()?;
                self.refuel_squadron(squadron, count)
                    .map(CommandOutcome::SquadronReadied)
                    .ok_or(CommandError::NoSuchSquadron)
            }
            Command::ArmSquadron { squadron, count } => {
                self.require_maintenance_phase()?;
                if self.sgsus.contains_key(&squadron) {
                    self.arm_squadron(squadron, count)
                        .map(CommandOutcome::SquadronArmed)
                        .ok_or(CommandError::InsufficientAmmo)
                } else {
                    Err(CommandError::NoSuchSquadron)
                }
            }
            Command::FlySquadronMission { squadron, count } => {
                self.require_phase(Phase::LandSupportAir)?;
                let sgsu = self.sgsus.get_mut(&squadron).ok_or(CommandError::NoSuchSquadron)?;
                Ok(CommandOutcome::SquadronFlew(sgsu.fly(count)))
            }
            Command::FlyBombingMission {
                squadron, planes, bombload_per_plane, mission, interceptors,
                flak_points, flak_destroy_roll, flak_abort_roll, bomb_roll, night,
            } => {
                self.require_phase(Phase::LandSupportAir)?;
                if !self.sgsus.contains_key(&squadron) {
                    return Err(CommandError::NoSuchSquadron);
                }
                let group = crate::rules::air::bombing_target_group(mission)
                    .ok_or(CommandError::NoTable)?; // mining harbours use their own table
                self.resolve_bombing_mission(
                    squadron, planes, bombload_per_plane, group, interceptors, None,
                    flak_points, flak_destroy_roll, flak_abort_roll, bomb_roll, night,
                )
                .map(CommandOutcome::BombingMission)
                .ok_or(CommandError::NoSuchSquadron)
            }
            Command::DesignateCap { squadron, center, cap_type, planes } => {
                self.cmd_designate_cap(squadron, center, cap_type, planes)
            }
            Command::Strafe { squadron, target, strafe_target, planes, roll } => {
                self.cmd_strafe(squadron, target, strafe_target, planes, roll)
            }
            Command::AssignPilot { rating, squadron } => self.cmd_assign_pilot(rating, squadron),
            Command::TransferSquadron { squadron, to } => self.cmd_transfer_squadron(squadron, to),
            Command::AirdropSupplies { squadron, target, commodity, amount, planes } => {
                self.cmd_airdrop_supplies(squadron, target, commodity, amount, planes)
            }
            Command::TransportUnit { squadron, unit, target } => {
                self.cmd_transport_unit(squadron, unit, target)
            }
            Command::AirdropUnit { squadron, unit, target } => {
                self.cmd_airdrop_unit(squadron, unit, target)
            }
            Command::FlyBombingMissionAt {
                squadron, target, mission, planes, bombload_per_plane, night, escort,
                flak_destroy_roll, flak_abort_roll, bomb_roll,
            } => self.cmd_fly_bombing_mission_at(
                squadron, target, mission, planes, bombload_per_plane, night, escort,
                flak_destroy_roll, flak_abort_roll, bomb_roll,
            ),
        }
    }

    /// Gate a command to a specific Sequence-of-Play phase.
    fn require_phase(&self, phase: Phase) -> Result<(), CommandError> {
        if self.phase == phase {
            Ok(())
        } else {
            Err(CommandError::WrongPhase)
        }
    }

    /// §30.15/§30.18 AssignShip: place a warship, honoring the seaworthiness (§30.36) and Valetta
    /// five-ship (§30.18) limits, and managing repair/at-sea counters.
    fn cmd_assign_ship(
        &mut self,
        ship_index: usize,
        location: crate::state::naval::ShipLocation,
    ) -> Result<CommandOutcome, CommandError> {
        use crate::state::naval::ShipLocation;
        let current = self.fleet.get(ship_index).ok_or(CommandError::NoSuchShip)?.location.clone();
        if matches!(location, ShipLocation::Valetta) && !matches!(current, ShipLocation::Valetta) {
            let in_valetta = self.fleet.iter().filter(|s| s.location == ShipLocation::Valetta).count();
            if in_valetta >= 5 {
                return Err(CommandError::ValettaFull);
            }
        }
        let ship = self.fleet.get_mut(ship_index).ok_or(CommandError::NoSuchShip)?;
        match &location {
            ShipLocation::AtSea(_) => {
                // §30.36 seaworthiness, §30.16 max-3-consecutive + owed rest.
                if !ship.seaworthy() {
                    return Err(CommandError::ShipNotSeaworthy);
                }
                if !ship.can_put_to_sea() {
                    return Err(CommandError::ShipMustRest);
                }
                ship.at_sea_stages += 1;
                ship.repair_stages = 0; // leaving port interrupts repair (§30.37)
            }
            ShipLocation::Alexandria | ShipLocation::Valetta => {
                // Returning from a run at sea accrues two port stages of rest per sea stage
                // (§30.16), preserving any post-bombardment debt (§30.25).
                if ship.at_sea_stages > 0 {
                    ship.rest_owed = ship.rest_owed.max(2 * ship.at_sea_stages);
                    ship.at_sea_stages = 0;
                }
                // Repair happens only in Alexandria (§30.37); moving to Valetta interrupts it.
                if matches!(location, ShipLocation::Valetta) {
                    ship.repair_stages = 0;
                }
            }
        }
        ship.location = location;
        Ok(CommandOutcome::ShipAssigned)
    }

    /// §30.2 off-shore bombardment: a warship at sea fires its Gun Rating (as Actual Barrage
    /// Points, §30.22) at an enemy combat unit of `target_class` in its own coastal hex, or — for
    /// heavy ships (Gun 4+) — an adjacent hex at half strength (§30.21). No ammunition is spent.
    fn cmd_off_shore_bombard(
        &mut self,
        ship_index: usize,
        target: HexCoord,
        target_class: BarrageTargetClass,
    ) -> Result<CommandOutcome, CommandError> {
        use crate::state::naval::ShipLocation;
        self.require_combat_phase()?;
        let ship = self.fleet.get(ship_index).ok_or(CommandError::NoSuchShip)?;
        let ShipLocation::AtSea(hex) = ship.location else {
            return Err(CommandError::ShipCannotReachTarget);
        };
        // §30.24: each ship uses its Barrage Strength only once per Operations Stage.
        if ship.fired_this_stage {
            return Err(CommandError::ShipAlreadyFired);
        }
        // §30.21: same hex (any ship) or an adjacent hex (heavy ships only, half strength §30.22).
        let two_hexes = if hex == target {
            false
        } else if hex.is_adjacent(target) && ship.is_heavy() {
            true
        } else {
            return Err(CommandError::ShipCannotReachTarget);
        };
        let points = ship.bombardment_strength(two_hexes) as i32;
        if points == 0 {
            return Err(CommandError::ShipCannotReachTarget);
        }
        // §30.2: ships fire on the Axis (Commonwealth is always the naval side).
        let target_unit = self
            .enemy_units_in_hex(target, Side::Commonwealth)
            .into_iter()
            .find(|&id| !self.is_noncombat_hq(id) && self.barrage_class(id) == target_class)
            .ok_or(CommandError::NoTargets)?;
        let dice = self.rng.two_dice_raw();
        let outcome = self.resolve_barrage_points(points, target_unit, dice);
        // §30.24/§30.25: mark the once-per-stage fire and require two stages' rest in Alexandria.
        let ship = &mut self.fleet[ship_index];
        ship.fired_this_stage = true;
        ship.rest_owed = ship.rest_owed.max(2);
        Ok(CommandOutcome::Barrage(outcome))
    }

    /// §30.35 land artillery / emplaced guns bombarding a Commonwealth warship in the same hex.
    fn cmd_artillery_bombard_ship(
        &mut self,
        attackers: &[UnitId],
        ship_index: usize,
    ) -> Result<CommandOutcome, CommandError> {
        use crate::state::naval::ShipLocation;
        use crate::state::unit::RatingKind;
        self.require_combat_phase()?;
        if attackers.is_empty() {
            return Err(CommandError::NoParticipants);
        }
        self.require_units_exist(attackers)?;
        // §30.35: only the enemy of the (Commonwealth) fleet fires on it.
        if self.attackers_side(attackers)? == Side::Commonwealth {
            return Err(CommandError::NoTargets);
        }
        let ShipLocation::AtSea(ship_hex) =
            self.fleet.get(ship_index).ok_or(CommandError::NoSuchShip)?.location
        else {
            return Err(CommandError::ShipCannotReachTarget);
        };
        // §30.35: the firing artillery must be in the same hex as the ship.
        for &a in attackers {
            let ah = self.effective_hex(a).ok_or(CommandError::UnitNotOnMap)?;
            if ah != ship_hex {
                return Err(CommandError::NotAdjacent);
            }
        }
        // §30.35: Raw Barrage Points, halved for mobile guns, whole for emplaced (zero-CPA) guns.
        let mut mobile_raw = 0i32;
        let mut emplaced_raw = 0i32;
        for &a in attackers {
            let raw = self
                .unit(a)
                .map(|u| u.raw_strength(&self.reference, RatingKind::Barrage))
                .unwrap_or(0);
            if self.effective_cpa(a) == Some(0) {
                emplaced_raw += raw;
            } else {
                mobile_raw += raw;
            }
        }
        let points = (emplaced_raw + mobile_raw / 2).max(0) as u32;
        if points == 0 {
            return Err(CommandError::NoTargets);
        }
        // §50.14: firing at the ship expends Ammo (barrage) from reserve / hex dumps.
        if !self.consume_combat_ammo(attackers, crate::state::game::CombatAction::Barrage) {
            return Err(CommandError::InsufficientAmmo);
        }
        let roll = self.rng.sequential_read() as u32;
        Ok(CommandOutcome::ShipDamaged(self.resolve_gun_hit_on_ship(points, ship_index, roll)))
    }

    /// Aircraft maintenance (refit/refuel/arm) happens in the Land Support Air Phase's Tactical
    /// Maintenance Segment (§38.32) and in the Strategic Air Recovery Stage's Aircraft Maintenance
    /// Phase (§33.0 VII.B).
    fn require_maintenance_phase(&self) -> Result<(), CommandError> {
        if matches!(self.phase, Phase::LandSupportAir | Phase::AircraftMaintenance) {
            Ok(())
        } else {
            Err(CommandError::WrongPhase)
        }
    }

    /// §56.15 PlanConvoy: gated to the Naval Convoy Schedule Phase; derives the allowable tonnage
    /// from the Convoy Level Chart for the arrival turn/die and plans the convoy.
    #[allow(clippy::too_many_arguments)]
    fn cmd_plan_convoy(
        &mut self,
        lane: u8,
        destination: String,
        arrival_gt: u32,
        fuel_tons: u32,
        ammo_tons: u32,
        stores_tons: u32,
        replacement_tons: u32,
        die: u32,
    ) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::NavalConvoySchedule)?;
        let allowable = self
            .allowable_convoy_tonnage(arrival_gt, die)
            .ok_or(CommandError::NoConvoyTable)?;
        if self.plan_convoy(
            lane, destination, arrival_gt, fuel_tons, ammo_tons, stores_tons, replacement_tons, allowable,
        ) {
            Ok(CommandOutcome::ConvoyPlanned)
        } else {
            Err(CommandError::ConvoyRejected)
        }
    }

    /// §41.6 BombConvoy: gated to the Convoy Resolution Phase.
    fn cmd_bomb_convoy(
        &mut self,
        convoy_index: usize,
        bomb_points: u32,
        torpedo_planes: u32,
        total_planes: u32,
        roll: u32,
    ) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::ConvoyResolution)?;
        match self.resolve_convoy_bombing(convoy_index, bomb_points, torpedo_planes, total_planes, roll) {
            Some(pct) => Ok(CommandOutcome::ConvoyBombed(pct)),
            None => Err(CommandError::InvalidConvoy),
        }
    }

    /// §56.16 DeliverConvoy: gated to the Naval Convoy Arrival Phase.
    fn cmd_deliver_convoy(&mut self, convoy_index: usize) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::NavalConvoyArrival)?;
        match self.deliver_convoy(convoy_index) {
            Some(c) => Ok(CommandOutcome::ConvoyDelivered(c)),
            None => Err(CommandError::InvalidConvoy),
        }
    }

    /// §55.22 BlockHarbor: an engineer/demolition action, gated to the Organization Phase
    /// like the other construction commands (BuildFortification/BuildRailroad/BuildRoad).
    fn cmd_block_harbor(&mut self, port: &str, has_engineer: bool) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::Organization)?;
        match self.block_harbor(port, has_engineer) {
            Some((ammo, stores)) => Ok(CommandOutcome::HarborAction { ammo, stores }),
            None => Err(CommandError::HarborActionFailed),
        }
    }

    /// §55.26 UnblockHarbor: an engineer action, gated to the Organization Phase (see BlockHarbor).
    fn cmd_unblock_harbor(&mut self, port: &str, engineers: u32) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::Organization)?;
        match self.unblock_harbor(port, engineers) {
            Some((ammo, stores)) => Ok(CommandOutcome::HarborAction { ammo, stores }),
            None => Err(CommandError::HarborActionFailed),
        }
    }

    /// §20.66/§20.78C PlanProduction: validate `count` Replacement Points of `item` against the
    /// nationality's Production Chart (lifetime total + per-period maximum), then schedule them to
    /// arrive after the §20.21 planning delay (Axis 2 / Commonwealth 4 Game-Turns) and begin
    /// Training. Convoy tonnage is recorded on the order; consuming Axis convoy capacity to ship
    /// it is deferred pending fuller convoy integration (D15 step 8).
    fn cmd_plan_production(
        &mut self,
        nationality: crate::reference::units::Nationality,
        item: &str,
        count: u32,
    ) -> Result<CommandOutcome, CommandError> {
        use crate::state::game::{replacement_class_for_item, ProductionOrder};
        self.require_phase(Phase::Organization)?;
        if count == 0 {
            return Err(CommandError::NoParticipants);
        }
        let gt = self.game_turn;
        let chart = self
            .reference
            .production_charts
            .get(&nationality)
            .ok_or(CommandError::NoTable)?;
        let def = chart.item(item).ok_or(CommandError::NoSuchProductionItem)?;
        let (total, tonnage_per, span) =
            (def.total, def.tonnage, def.period_at(gt).span_game_turns());
        let cap = chart.available(item, gt);

        // §20.66: the item's lifetime production total may not be exceeded.
        let lifetime: u32 = self
            .production_orders
            .iter()
            .filter(|o| o.nationality == nationality && o.item == item)
            .map(|o| o.count)
            .sum();
        if lifetime + count > total {
            return Err(CommandError::ProductionExhausted);
        }
        // §20.66: at most `cap` points may be planned in any `span` consecutive Game-Turns.
        let lo = gt.saturating_sub(span - 1);
        let in_window: u32 = self
            .production_orders
            .iter()
            .filter(|o| o.nationality == nationality && o.item == item)
            .filter(|o| o.game_turn >= lo && o.game_turn <= gt)
            .map(|o| o.count)
            .sum();
        if in_window + count > cap {
            return Err(CommandError::ProductionRateExceeded);
        }

        // Accepted. Record the order, then schedule the points to arrive after the §20.21 delay.
        let tonnage = tonnage_per * count;
        self.production_orders.push(ProductionOrder {
            nationality,
            item: item.to_string(),
            game_turn: gt,
            count,
            tonnage,
        });
        let delay = if nationality.side() == crate::reference::units::Side::Commonwealth {
            4
        } else {
            2
        };
        let arrival_game_turn = gt + delay;
        let class = replacement_class_for_item(item).unwrap_or(crate::state::game::ReplacementClass::Gun);
        let location = self
            .reference
            .reinforcement_entry_hex(nationality)
            .map(str::to_string)
            .and_then(|s| self.resolve_entry(&s));
        self.plan_replacement(class, count, arrival_game_turn, self.operations_stage as u32, location, None);
        Ok(CommandOutcome::ProductionPlanned { arrival_game_turn, class, tonnage })
    }

    /// §40.2 DesignateCap: commit `planes` fighters of a located squadron to Combat Air Patrol over
    /// `center`. Validates F (CAP) capability (§40.21), flight range to the patrolled hex (§37.12),
    /// and §37.22 weather; the fighters fly (leave `ready`) and stand as a patrol for the stage.
    fn cmd_designate_cap(
        &mut self,
        squadron: crate::state::air::SquadronId,
        center: HexCoord,
        cap_type: crate::reference::aircraft::CapType,
        planes: u32,
    ) -> Result<CommandOutcome, CommandError> {
        use crate::state::game::{CapPatrol, Flight, Weather};
        self.require_phase(Phase::LandSupportAir)?;
        let sgsu = self.sgsus.get(&squadron).ok_or(CommandError::NoSuchSquadron)?;
        if sgsu.in_transit() {
            return Err(CommandError::SquadronInTransit); // §43.14: grounded during transfer
        }
        let side = sgsu.nationality.side();
        let nationality = sgsu.nationality;
        if !self.squadron_is_based(squadron) {
            return Err(CommandError::SquadronNotBased);
        }
        let grounded = |g: &Self, h: HexCoord| matches!(g.weather_at(h), Weather::Sandstorm | Weather::Rainstorm);
        if grounded(self, center) || self.flight_base_weather_hex(squadron).is_some_and(|b| grounded(self, b)) {
            return Err(CommandError::WeatherGrounded);
        }
        let distance = self.flight_distance(squadron, center);
        // Shortest range among ready plane types, and a representative fighter's TacAir/Maneuver.
        let chart = self.reference.aircraft_charts.get(&nationality);
        let mut min_range = u32::MAX;
        let mut rep: Option<(i32, i32)> = None;
        let mut can_cap = false;
        for g in sgsu.planes.iter().filter(|g| g.ready > 0) {
            if let Some(ac) = chart.and_then(|c| c.get(&g.aircraft_type)) {
                min_range = min_range.min(ac.max_range());
                if crate::rules::air::can_fly_cap(&ac.missions) {
                    can_cap = true;
                    rep = rep.or_else(|| ac.loadouts.first().map(|l| (l.tac_air, l.maneuver)));
                }
            }
        }
        if min_range == u32::MAX {
            return Err(CommandError::NoReadyPlanes);
        }
        if !can_cap {
            return Err(CommandError::MissionNotPermitted);
        }
        if !distance.is_some_and(|d| crate::rules::air::can_fly(min_range, d, false)) {
            return Err(CommandError::TargetOutOfRange);
        }
        let flown = self.sgsus.get_mut(&squadron).ok_or(CommandError::NoSuchSquadron)?.fly(planes);
        if flown == 0 {
            return Err(CommandError::NoReadyPlanes);
        }
        let (tacair, maneuver) = rep.unwrap_or((0, 0));
        let pilot = self.squadron_pilot_bonus(squadron);
        self.cap_patrols.push(CapPatrol {
            squadron,
            side,
            cap_type,
            center,
            flight: Flight { count: flown, tacair, pilot, maneuver },
            intercepts_used: 0,
        });
        Ok(CommandOutcome::CapDesignated { fighters: flown })
    }

    /// §40.12 AssignPilot: add a pilot of `rating` (0 or one of the §40.15 ratings) to a squadron's
    /// roster (Organization Phase). The pilot is ready to fly.
    fn cmd_assign_pilot(
        &mut self,
        rating: u32,
        squadron: crate::state::air::SquadronId,
    ) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::Organization)?;
        if rating != 0 && !crate::rules::air::PILOT_RATINGS.contains(&rating) {
            return Err(CommandError::InvalidPilotRating);
        }
        if !self.sgsus.contains_key(&squadron) {
            return Err(CommandError::NoSuchSquadron);
        }
        self.pilots.push(crate::state::air::Pilot::new(rating, Some(squadron)));
        Ok(CommandOutcome::PilotAssigned)
    }

    /// §43.14 TransferSquadron: move a squadron to a Mediterranean Box or on-map Africa, with one
    /// Operations Stage of transit, refusing a move that would break the §43.11 basing requirement.
    fn cmd_transfer_squadron(
        &mut self,
        squadron: crate::state::air::SquadronId,
        to: crate::state::air::MedBaseDest,
    ) -> Result<CommandOutcome, CommandError> {
        use crate::state::air::{MedBase, MedBaseDest};
        self.require_phase(Phase::Organization)?;
        // An African destination must be a friendly air facility (§43 — the planes must land).
        if let MedBaseDest::Africa { hex } = to {
            if !self.air_facilities.iter().any(|f| f.hex == hex) {
                return Err(CommandError::NoSuchFacility);
            }
        }
        let sgsu = self.sgsus.get_mut(&squadron).ok_or(CommandError::NoSuchSquadron)?;
        let prior = (sgsu.med_base, sgsu.location, sgsu.transfer_remaining);
        match to {
            MedBaseDest::ItalySicily => {
                sgsu.med_base = Some(MedBase::ItalySicily);
                sgsu.location = None;
            }
            MedBaseDest::Crete => {
                sgsu.med_base = Some(MedBase::Crete);
                sgsu.location = None;
            }
            MedBaseDest::Africa { hex } => {
                sgsu.med_base = None;
                sgsu.location = Some(hex);
            }
        }
        sgsu.transfer_remaining = 1; // §43.14: one Operations Stage of transit.
        // §43.11–13: the move may not break the date-appropriate German basing requirements.
        if !self.german_basing_ok() {
            let sgsu = self.sgsus.get_mut(&squadron).expect("just borrowed");
            (sgsu.med_base, sgsu.location, sgsu.transfer_remaining) = prior;
            return Err(CommandError::BasingRequirementViolated);
        }
        Ok(CommandOutcome::SquadronTransferred)
    }

    /// §42.4/§42.46 AirdropSupplies: drop Ammo/Stores from the squadron's base dumps into a target
    /// hex, validating transport capability, range, weather, and the §42.43/§42.44 drop-zone rules.
    fn cmd_airdrop_supplies(
        &mut self,
        squadron: crate::state::air::SquadronId,
        target: HexCoord,
        commodity: crate::state::game::Commodity,
        amount: u32,
        planes: u32,
    ) -> Result<CommandOutcome, CommandError> {
        use crate::reference::aircraft::Mission;
        use crate::reference::terrain::HexTerrain;
        use crate::state::game::{Commodity, Weather};
        self.require_phase(Phase::LandSupportAir)?;
        // §42.46: only ammunition and stores may be airdropped.
        if !matches!(commodity, Commodity::Ammo | Commodity::Stores) {
            return Err(CommandError::MissionNotPermitted);
        }
        let sgsu = self.sgsus.get(&squadron).ok_or(CommandError::NoSuchSquadron)?;
        let base = sgsu.location.ok_or(CommandError::SquadronNotBased)?;
        if sgsu.in_transit() {
            return Err(CommandError::SquadronInTransit); // §43.14: grounded during transfer
        }
        let side = sgsu.nationality.side();
        let nationality = sgsu.nationality;

        let grounded = |g: &Self, h: HexCoord| matches!(g.weather_at(h), Weather::Sandstorm | Weather::Rainstorm);
        if grounded(self, base) || grounded(self, target) {
            return Err(CommandError::WeatherGrounded);
        }
        // Transport-capable ready planes and their shortest range (§42.31/§42.36).
        let chart = self.reference.aircraft_charts.get(&nationality);
        let mut min_range = u32::MAX;
        let mut ready_transport = 0;
        for g in sgsu.planes.iter().filter(|g| g.ready > 0) {
            if let Some(ac) = chart.and_then(|c| c.get(&g.aircraft_type)) {
                if crate::rules::air::can_perform(&ac.missions, Mission::Transport, false) {
                    ready_transport += g.ready;
                    min_range = min_range.min(ac.max_range());
                }
            }
        }
        if min_range == u32::MAX {
            return Err(CommandError::MissionNotPermitted); // no transport-capable planes
        }
        if planes == 0 || planes > ready_transport {
            return Err(CommandError::NoReadyPlanes);
        }
        if !crate::rules::air::can_fly(min_range, base.distance(target), false) {
            return Err(CommandError::TargetOutOfRange);
        }
        // §42.43/§42.46 drop-zone legality.
        let enemy = side.opponent();
        let occupants = self.units_in_hex(target);
        let enemy_occ = occupants.iter().any(|&id| self.unit(id).is_some_and(|u| u.nationality.side() == enemy));
        let friendly_occ = occupants.iter().any(|&id| self.unit(id).is_some_and(|u| u.nationality.side() == side));
        if enemy_occ {
            return Err(CommandError::DropZoneIllegal);
        }
        if !friendly_occ {
            // §42.43: clear/gravel/desert, not a major city, not in an enemy ZOC, and within two
            // hexes of a recognizable land feature (§42.44).
            let terrain_ok = matches!(
                self.map.terrain(target),
                Some(HexTerrain::Clear | HexTerrain::SandGravel | HexTerrain::Desert)
            );
            if !terrain_ok
                || self.hex_in_zoc_of(target, enemy)
                || !self.within_two_of_land_feature(target)
            {
                return Err(CommandError::DropZoneIllegal);
            }
        }
        // §42.32: the cargo comes from the transport's base hex; consume it there.
        if !self.consume_commodity_at(base, side, commodity, amount) {
            return Err(if matches!(commodity, Commodity::Ammo) {
                CommandError::InsufficientAmmo
            } else {
                CommandError::InsufficientStores
            });
        }
        self.sgsus.get_mut(&squadron).ok_or(CommandError::NoSuchSquadron)?.fly(planes);
        // §42.46: airdropped supplies are usable immediately.
        self.add_commodity_to_dump(target, side, commodity, amount);
        Ok(CommandOutcome::SuppliesAirdropped { amount })
    }

    /// §42.3/§42.37 TransportUnit: air-transport an eligible unit co-located with the transport
    /// squadron to a friendly air facility, landing it there and marking it as having used its CPA
    /// (§42.34). One transport plane carries one TOE Strength Point (§42.31 note; a documented
    /// simplification of the aircraft-chart cargo capacity).
    fn cmd_transport_unit(
        &mut self,
        squadron: crate::state::air::SquadronId,
        unit: UnitId,
        target: HexCoord,
    ) -> Result<CommandOutcome, CommandError> {
        use crate::reference::aircraft::Mission;
        use crate::state::game::Weather;
        use crate::state::unit::UnitLocation;
        self.require_phase(Phase::LandSupportAir)?;
        let sgsu = self.sgsus.get(&squadron).ok_or(CommandError::NoSuchSquadron)?;
        let base = sgsu.location.ok_or(CommandError::SquadronNotBased)?;
        if sgsu.in_transit() {
            return Err(CommandError::SquadronInTransit); // §43.14: grounded during transfer
        }
        let nationality = sgsu.nationality;
        // §42.33: transports must land at a Friendly air facility.
        if !self.air_facilities.iter().any(|f| f.hex == target) {
            return Err(CommandError::NoSuchFacility);
        }
        let grounded = |g: &Self, h: HexCoord| matches!(g.weather_at(h), Weather::Sandstorm | Weather::Rainstorm);
        if grounded(self, base) || grounded(self, target) {
            return Err(CommandError::WeatherGrounded);
        }
        // §42.32: the cargo unit must start in the transport's hex; §42.37: eligible types only.
        let u = self.units.get(&unit).ok_or(CommandError::UnitNotFound)?;
        if u.hex() != Some(base) || u.nationality.side() != nationality.side() {
            return Err(CommandError::MissionNotPermitted);
        }
        let toe = u.total_toe();
        let ut = self
            .reference
            .unit_characteristics
            .get(&u.nationality)
            .and_then(|m| m.get(&u.id_code))
            .map(|c| c.unit_type.as_str())
            .unwrap_or("");
        let trucks_empty = { let t = &u.trucks; t.light + t.medium + t.heavy == 0 };
        let eligible = (ut.contains("Infantry") && trucks_empty)
            || ut.contains("Reconnaissance")
            || ut.contains("Recce")
            || ut.contains("Motorcycle");
        if !eligible || toe == 0 {
            return Err(CommandError::MissionNotPermitted);
        }
        // Transport-capable ready planes; one plane per TOE Strength Point (§42.31 note).
        let chart = self.reference.aircraft_charts.get(&nationality);
        let mut min_range = u32::MAX;
        let mut ready_transport = 0;
        for g in sgsu.planes.iter().filter(|g| g.ready > 0) {
            if let Some(ac) = chart.and_then(|c| c.get(&g.aircraft_type)) {
                if crate::rules::air::can_perform(&ac.missions, Mission::Transport, false) {
                    ready_transport += g.ready;
                    min_range = min_range.min(ac.max_range());
                }
            }
        }
        if min_range == u32::MAX {
            return Err(CommandError::MissionNotPermitted);
        }
        if ready_transport < toe {
            return Err(CommandError::NoReadyPlanes);
        }
        if !crate::rules::air::can_fly(min_range, base.distance(target), false) {
            return Err(CommandError::TargetOutOfRange);
        }
        // Execute: fly the transports, land the unit, and exhaust its CPA (§42.34).
        self.sgsus.get_mut(&squadron).ok_or(CommandError::NoSuchSquadron)?.fly(toe);
        let cpa = self.unit_cpa(unit).unwrap_or(0).max(0);
        if let Some(u) = self.unit_mut(unit) {
            u.location = Some(UnitLocation::OnMap(target));
            u.cp_expended = Fixed(cpa * 10);
        }
        Ok(CommandOutcome::UnitTransported)
    }

    /// §42.41/§42.42 AirdropUnit: airdrop an airborne-eligible unit into a legal drop-zone hex.
    fn cmd_airdrop_unit(
        &mut self,
        squadron: crate::state::air::SquadronId,
        unit: UnitId,
        target: HexCoord,
    ) -> Result<CommandOutcome, CommandError> {
        use crate::reference::aircraft::Mission;
        use crate::reference::terrain::HexTerrain;
        use crate::state::game::Weather;
        use crate::state::unit::UnitLocation;
        self.require_phase(Phase::LandSupportAir)?;
        let sgsu = self.sgsus.get(&squadron).ok_or(CommandError::NoSuchSquadron)?;
        let base = sgsu.location.ok_or(CommandError::SquadronNotBased)?;
        if sgsu.in_transit() {
            return Err(CommandError::SquadronInTransit); // §43.14: grounded during transfer
        }
        let nationality = sgsu.nationality;
        let side = nationality.side();

        // §42.45: no airdrop in a Sandstorm/Rainstorm (base or target).
        let grounded = |g: &Self, h: HexCoord| matches!(g.weather_at(h), Weather::Sandstorm | Weather::Rainstorm);
        if grounded(self, base) || grounded(self, target) {
            return Err(CommandError::WeatherGrounded);
        }
        // §42.41: the unit must be airdrop-eligible, friendly, and start in the transports' hex.
        let u = self.units.get(&unit).ok_or(CommandError::UnitNotFound)?;
        if u.hex() != Some(base) || u.nationality.side() != side || !u.airdroppable {
            return Err(CommandError::MissionNotPermitted);
        }
        let toe = u.total_toe();
        // §42.48: a unit may not be airdropped more than once per 12 Operations Stages.
        let abs_stage = self.game_turn * 3 + self.operations_stage as u32;
        if let Some(&last) = self.airdrop_last_stage.get(&unit) {
            if abs_stage < last + 12 {
                return Err(CommandError::AirdropOnCooldown);
            }
        }
        // Transport-capable ready planes; one plane per TOE Strength Point (§42.48 note).
        let chart = self.reference.aircraft_charts.get(&nationality);
        let mut min_range = u32::MAX;
        let mut ready_transport = 0;
        for g in sgsu.planes.iter().filter(|g| g.ready > 0) {
            if let Some(ac) = chart.and_then(|c| c.get(&g.aircraft_type)) {
                if crate::rules::air::can_perform(&ac.missions, Mission::Transport, false) {
                    ready_transport += g.ready;
                    min_range = min_range.min(ac.max_range());
                }
            }
        }
        if min_range == u32::MAX {
            return Err(CommandError::MissionNotPermitted);
        }
        if toe == 0 || ready_transport < toe {
            return Err(CommandError::NoReadyPlanes);
        }
        if !crate::rules::air::can_fly(min_range, base.distance(target), false) {
            return Err(CommandError::TargetOutOfRange);
        }
        // §42.43/§42.44 drop-zone legality.
        let enemy = side.opponent();
        let occupants = self.units_in_hex(target);
        if occupants.iter().any(|&id| self.unit(id).is_some_and(|u| u.nationality.side() == enemy)) {
            return Err(CommandError::DropZoneIllegal); // enemy-occupied
        }
        let terrain_ok = matches!(
            self.map.terrain(target),
            Some(HexTerrain::Clear | HexTerrain::SandGravel | HexTerrain::Desert)
        );
        if !terrain_ok
            || self.hex_in_zoc_of(target, enemy)
            || !self.within_two_of_land_feature(target)
        {
            return Err(CommandError::DropZoneIllegal);
        }
        // §42.43: within stacking restrictions.
        if let Some(limit) = self
            .map
            .terrain(target)
            .and_then(|t| self.reference.terrain.get(&t))
            .and_then(|p| p.stacking_limit)
        {
            if self.hex_stacking_points(target) + self.effective_stacking_points(unit) > limit {
                return Err(CommandError::StackingLimitExceeded);
            }
        }
        // §42.45: no drift/casualties — the unit lands in the target hex. §42.47: it may act but not
        // over-expend CPA this stage (the no-over-CPA nuance is not separately enforced yet).
        self.sgsus.get_mut(&squadron).ok_or(CommandError::NoSuchSquadron)?.fly(toe);
        if let Some(u) = self.unit_mut(unit) {
            u.location = Some(UnitLocation::OnMap(target));
        }
        self.airdrop_last_stage.insert(unit, abs_stage);
        Ok(CommandOutcome::UnitAirdropped)
    }

    /// §40.5–40.8 Strafe: validate and resolve a strafing mission against the map, applying the
    /// effect for supply dumps (§40.64) and water pipelines (§40.67). Other target types (infantry,
    /// trucks, grounded aircraft, tanks, ports) are not yet modeled and are rejected.
    fn cmd_strafe(
        &mut self,
        squadron: crate::state::air::SquadronId,
        target: HexCoord,
        strafe_target: crate::rules::air::StrafeTarget,
        planes: u32,
        roll: u32,
    ) -> Result<CommandOutcome, CommandError> {
        use crate::rules::air::StrafeTarget;
        use crate::state::game::{Commodity, StrafeEffect, Weather};
        self.require_phase(Phase::LandSupportAir)?;
        // Only the target types with modeled state effects are supported for now.
        if !matches!(
            strafe_target,
            StrafeTarget::SupplyDump
                | StrafeTarget::WaterPipeline
                | StrafeTarget::Infantry
                | StrafeTarget::GroundedAircraft
                | StrafeTarget::FirstLineTrucks
                | StrafeTarget::Tanks
        ) {
            return Err(CommandError::MissionNotPermitted);
        }
        let sgsu = self.sgsus.get(&squadron).ok_or(CommandError::NoSuchSquadron)?;
        if sgsu.in_transit() {
            return Err(CommandError::SquadronInTransit); // §43.14: grounded during transfer
        }
        let side = sgsu.nationality.side();
        let nationality = sgsu.nationality;
        if !self.squadron_is_based(squadron) {
            return Err(CommandError::SquadronNotBased);
        }
        let grounded = |g: &Self, h: HexCoord| matches!(g.weather_at(h), Weather::Sandstorm | Weather::Rainstorm);
        if grounded(self, target) || self.flight_base_weather_hex(squadron).is_some_and(|b| grounded(self, b)) {
            return Err(CommandError::WeatherGrounded);
        }
        let distance = self.flight_distance(squadron, target);
        // Shortest range and total strafing TacAir among ready, strafe-capable plane types.
        let chart = self.reference.aircraft_charts.get(&nationality);
        let mut min_range = u32::MAX;
        let mut rep_tacair = 0i32;
        let mut can_strafe = false;
        let mut can_strafe_armor = false;
        for g in sgsu.planes.iter().filter(|g| g.ready > 0) {
            if let Some(ac) = chart.and_then(|c| c.get(&g.aircraft_type)) {
                min_range = min_range.min(ac.max_range());
                if crate::rules::air::can_strafe(&ac.missions) {
                    can_strafe = true;
                    if rep_tacair == 0 {
                        rep_tacair = ac.loadouts.first().map(|l| l.tac_air).unwrap_or(0);
                    }
                }
                if crate::rules::air::can_strafe_armor(&ac.missions) {
                    can_strafe_armor = true;
                }
            }
        }
        if min_range == u32::MAX {
            return Err(CommandError::NoReadyPlanes);
        }
        if !can_strafe {
            return Err(CommandError::MissionNotPermitted);
        }
        // §40.65: only armour-capable planes (Hurricane Mk.IID) may strafe tanks.
        if matches!(strafe_target, StrafeTarget::Tanks) && !can_strafe_armor {
            return Err(CommandError::MissionNotPermitted);
        }
        if !distance.is_some_and(|d| crate::rules::air::can_fly(min_range, d, false)) {
            return Err(CommandError::TargetOutOfRange);
        }
        let table = self.reference.strafing_table.as_ref().ok_or(CommandError::NoTable)?.clone();

        let flown = self.sgsus.get_mut(&squadron).ok_or(CommandError::NoSuchSquadron)?.fly(planes);
        if flown == 0 {
            return Err(CommandError::NoReadyPlanes);
        }
        let tacair = flown * rep_tacair.max(0) as u32;
        // §40.8: the Fortification Level shift comes from the target hex (§25.1), not the caller.
        let shift = crate::rules::air::strafe_column_shift(strafe_target, self.fort_level(target) as u32);
        let result = table.result(tacair, shift, roll);

        let effect = match strafe_target {
            StrafeTarget::SupplyDump => {
                let pct = crate::rules::air::supply_dump_percent_destroyed(result);
                if pct > 0 {
                    let enemy = side.opponent();
                    for d in self.supply_dumps.iter_mut().filter(|d| {
                        d.side == enemy && !d.dummy && d.location.hex() == Some(target)
                    }) {
                        for c in [Commodity::Fuel, Commodity::Ammo, Commodity::Stores, Commodity::Water] {
                            let have = d.supply.get(c);
                            *d.supply.get_mut(c) = have - have * pct / 100;
                        }
                    }
                    StrafeEffect::SupplyDumpDamaged { percent: pct }
                } else {
                    StrafeEffect::None
                }
            }
            StrafeTarget::WaterPipeline => {
                if crate::rules::air::pipeline_destroyed_by_strafe(result) && self.destroy_pipeline(target) {
                    StrafeEffect::PipelineDestroyed
                } else {
                    StrafeEffect::None
                }
            }
            StrafeTarget::Infantry => {
                // §40.6: the result is the TOE Strength Points eliminated from an infantry unit.
                let removed = self.strafe_infantry_loss(target, side, result);
                if removed > 0 { StrafeEffect::InfantryEliminated { points: removed } } else { StrafeEffect::None }
            }
            StrafeTarget::GroundedAircraft => {
                // §40.6: the result is the number of d6 to roll; their sum is the planes destroyed.
                let planes = self.strafe_grounded_aircraft_loss(target, side, result);
                if planes > 0 { StrafeEffect::GroundedAircraftDestroyed { planes } } else { StrafeEffect::None }
            }
            StrafeTarget::FirstLineTrucks => {
                // §40.62: the result is the first-line attached Truck Points eliminated.
                let removed = self.strafe_first_line_trucks(target, side, result);
                if removed > 0 { StrafeEffect::TrucksEliminated { points: removed } } else { StrafeEffect::None }
            }
            StrafeTarget::Tanks => {
                // §40.65: the result is the number of d6 to roll; their sum is the minimum Armour
                // Protection Points removed from an enemy armored unit.
                let removed = self.strafe_tank_loss(target, side, result);
                if removed > 0 { StrafeEffect::TankPointsEliminated { points: removed } } else { StrafeEffect::None }
            }
            _ => StrafeEffect::None, // unreachable (guarded above)
        };
        Ok(CommandOutcome::Strafed { result, effect })
    }

    /// §37/§41 FlyBombingMissionAt: validate a land-support bombing mission against the map and
    /// resolve it. Enforces flight range (§37.12), day/night mission capability (§39.42), and the
    /// §37.22 sandstorm/rainstorm grounding of the base or target hex; derives the target's flak
    /// (§46) from the enemy AA units present; resolves interception (§40.26) against enemy offensive
    /// CAP covering the target; then delegates to `resolve_bombing_mission`.
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_arguments)]
    fn cmd_fly_bombing_mission_at(
        &mut self,
        squadron: crate::state::air::SquadronId,
        target: HexCoord,
        mission: crate::reference::aircraft::BombingMission,
        planes: u32,
        bombload_per_plane: u32,
        night: bool,
        escort: Option<crate::state::air::SquadronId>,
        flak_destroy_roll: u32,
        flak_abort_roll: u32,
        bomb_roll: u32,
    ) -> Result<CommandOutcome, CommandError> {
        use crate::reference::aircraft::{CapType, Mission};
        use crate::state::game::Weather;
        self.require_phase(Phase::LandSupportAir)?;
        let sgsu = self.sgsus.get(&squadron).ok_or(CommandError::NoSuchSquadron)?;
        if sgsu.in_transit() {
            return Err(CommandError::SquadronInTransit); // §43.14: grounded during transfer
        }
        let nationality = sgsu.nationality;
        if !self.squadron_is_based(squadron) {
            return Err(CommandError::SquadronNotBased);
        }
        // §43.23: Crete bombers are committed to Suez for four Operations Stages a month.
        if sgsu.med_base == Some(crate::state::air::MedBase::Crete) && self.crete_suez_window() {
            return Err(CommandError::CreteSuezCommitment);
        }
        // §37.22: no flight into a Sandstorm/Rainstorm target (or out of an on-map base in one).
        let grounded = |g: &Self, h: HexCoord| matches!(g.weather_at(h), Weather::Sandstorm | Weather::Rainstorm);
        if grounded(self, target) || self.flight_base_weather_hex(squadron).is_some_and(|b| grounded(self, b)) {
            return Err(CommandError::WeatherGrounded);
        }

        // §37.12/§37.4: distance (hexes, from an on-map base or off-map Box) may not exceed the
        // shortest range among the flying plane types, and (§39.42) at least one flying type must be
        // able to bomb by day/night as required.
        let distance = self.flight_distance(squadron, target);
        let chart = self.reference.aircraft_charts.get(&nationality);
        let mut min_range = u32::MAX;
        let mut any_can_bomb = false;
        let mut any_strike = false;
        for g in sgsu.planes.iter().filter(|g| g.ready > 0) {
            if let Some(ac) = chart.and_then(|c| c.get(&g.aircraft_type)) {
                min_range = min_range.min(ac.max_range());
                let strike = crate::rules::air::can_perform(&ac.missions, Mission::Strike, night);
                if crate::rules::air::can_perform(&ac.missions, Mission::ConvoyLandBombing, night) || strike {
                    any_can_bomb = true;
                }
                any_strike |= strike;
            }
        }
        if min_range == u32::MAX {
            return Err(CommandError::NoReadyPlanes);
        }
        if !distance.is_some_and(|d| crate::rules::air::can_fly(min_range, d, false)) {
            return Err(CommandError::TargetOutOfRange); // out of range, or no §37.4 route from a Box
        }
        if !any_can_bomb {
            return Err(CommandError::MissionNotPermitted);
        }
        // §41.32: first-line (attached) trucks may be bombed only by a plane with the "D" (strike)
        // capability. (A hex-placed truck convoy is not modeled; this bombs a unit's own trucks.)
        if matches!(mission, crate::reference::aircraft::BombingMission::TruckConvoy) && !any_strike {
            return Err(CommandError::MissionNotPermitted);
        }

        let group = crate::rules::air::bombing_target_group(mission).ok_or(CommandError::NoTable)?;
        // §40.26/§40.27: an enemy offensive CAP whose air ZOC covers the target intercepts (using
        // its one interception this stage); pick the first eligible patrol and consume its intercept.
        let enemy = nationality.side().opponent();
        let mut interceptors = None;
        let mut interceptor_sq = None;
        if let Some(i) = self.cap_patrols.iter().position(|p| {
            p.side == enemy
                && matches!(p.cap_type, CapType::Offensive)
                && p.intercepts_used < crate::rules::air::OFFENSIVE_CAP_INTERCEPTS_PER_STAGE
                && p.covers(target)
        }) {
            self.cap_patrols[i].intercepts_used += 1;
            interceptor_sq = Some(self.cap_patrols[i].squadron);
            interceptors = Some(self.cap_patrols[i].flight);
        }
        // §40.3: with no CAP covering the target, an enemy fighter base may scramble to intercept.
        if interceptors.is_none() {
            if let Some((sq, fl)) = self.scramble_interception(target, nationality.side(), night) {
                interceptor_sq = Some(sq);
                interceptors = Some(fl);
            }
        }
        // §45/§43.24: an optional fighter escort — intercepting CAP must fight it before the bombers.
        let mut escort_sq = None;
        let escort_flight = if let Some(esq) = escort {
            let bomber_on_crete =
                self.sgsus.get(&squadron).and_then(|s| s.med_base) == Some(crate::state::air::MedBase::Crete);
            let es = self.sgsus.get(&esq).ok_or(CommandError::NoSuchSquadron)?;
            // A friendly, based, not-in-transit fighter squadron with ready CAP-capable planes.
            if es.nationality.side() != nationality.side() || es.in_transit() {
                return Err(CommandError::EscortNotPermitted);
            }
            // §43.24: a Crete-based bomber may not be escorted by an on-map (African) fighter.
            if bomber_on_crete && es.location.is_some() {
                return Err(CommandError::EscortNotPermitted);
            }
            let echart = self.reference.aircraft_charts.get(&es.nationality);
            let mut rep: Option<(i32, i32)> = None;
            let mut cap_capable = false;
            for g in es.planes.iter().filter(|g| g.ready > 0) {
                if let Some(ac) = echart.and_then(|c| c.get(&g.aircraft_type)) {
                    if crate::rules::air::can_fly_cap(&ac.missions) {
                        cap_capable = true;
                        rep = rep.or_else(|| ac.loadouts.first().map(|l| (l.tac_air, l.maneuver)));
                    }
                }
            }
            if !cap_capable {
                return Err(CommandError::EscortNotPermitted);
            }
            let flown = self.sgsus.get_mut(&esq).ok_or(CommandError::NoSuchSquadron)?.fly(u32::MAX);
            if flown == 0 {
                return Err(CommandError::NoReadyPlanes);
            }
            let (tacair, maneuver) = rep.unwrap_or((0, 0));
            escort_sq = Some(esq);
            Some(crate::state::game::Flight { count: flown, tacair, pilot: self.squadron_pilot_bonus(esq), maneuver })
        } else {
            None
        };
        // §46: flak fired by the enemy AA units in the target hex.
        let flak_points = self.hex_flak_points(target, nationality.side());
        let outcome = self
            .resolve_bombing_mission(
                squadron, planes, bombload_per_plane, group, interceptors, escort_flight,
                flak_points, flak_destroy_roll, flak_abort_roll, bomb_roll, night,
            )
            .ok_or(CommandError::NoSuchSquadron)?;
        // §40.18: credit the intercepting squadron's pilot with the bombers it shot down.
        if let Some(sq) = interceptor_sq {
            if outcome.intercepted > 0 {
                self.credit_pilot_kills(sq, outcome.intercepted);
            }
        }
        // §45: apply escort losses to the escorting squadron.
        if let Some(esq) = escort_sq {
            if outcome.escort_losses > 0 {
                if let Some(s) = self.sgsus.get_mut(&esq) {
                    s.lose_planes(outcome.escort_losses);
                }
            }
        }
        // §41.3x: apply the mission's land effect (pin / supply-dump destruction) at the target.
        self.apply_land_bombing_effect(mission, target, nationality.side(), outcome.result.max(0) as u32);
        Ok(CommandOutcome::BombingMission(outcome))
    }

    /// §35.2 AssignSquadronPlanes: gated to the Organization Phase.
    fn cmd_assign_squadron_planes(
        &mut self,
        squadron: crate::state::air::SquadronId,
        aircraft_type: &str,
        class: crate::reference::aircraft::AircraftClass,
        count: u32,
    ) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::Organization)?;
        let gt = self.game_turn;
        let sgsu = self.sgsus.get_mut(&squadron).ok_or(CommandError::NoSuchSquadron)?;
        if sgsu.add_planes(aircraft_type, class, count, gt) {
            Ok(CommandOutcome::SquadronPlanesAssigned)
        } else {
            Err(CommandError::SquadronRejected)
        }
    }

    /// §41.36 BombAirFacility: gated to the Land Support Air Phase.
    fn cmd_bomb_air_facility(
        &mut self,
        hex: HexCoord,
        kind: crate::state::air::AirFacilityKind,
        levels: u32,
    ) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::LandSupportAir)?;
        if self.damage_air_facility(hex, kind, levels) {
            Ok(CommandOutcome::AirFacilityBombed)
        } else {
            Err(CommandError::NoSuchFacility)
        }
    }

    /// §41.39/§55.27 LayHarborMine: gated to the Land Support Air Phase.
    fn cmd_lay_harbor_mine(&mut self, port: &str) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::LandSupportAir)?;
        let p = self.ports.get_mut(port).ok_or(CommandError::NoSuchPort)?;
        p.lay_mine();
        Ok(CommandOutcome::HarborMineLaid)
    }

    /// §55.27 RemoveHarborMine: gated to the Repair Phase.
    fn cmd_remove_harbor_mine(&mut self, port: &str, die: u32) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::Repair)?;
        let p = self.ports.get_mut(port).ok_or(CommandError::NoSuchPort)?;
        Ok(CommandOutcome::HarborMineRemoved(p.remove_mine(die)))
    }

    /// §42.53 ReconConvoyLane: gated to the Convoy Resolution Phase. On a successful reveal, reports
    /// the tonnage of any (not-cancelled) convoy arriving in that lane this Game-Turn.
    fn cmd_recon_convoy_lane(
        &mut self,
        lane: u8,
        planes: u32,
        roll: u32,
    ) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::ConvoyResolution)?;
        let table = self.reference.naval_recon.as_ref().ok_or(CommandError::NoTable)?;
        let revealed = table.reveals(planes, roll);
        let convoy_tonnage = if revealed {
            self.convoys
                .iter()
                .find(|c| c.lane == lane && !c.cancelled && c.arrival_gt == self.game_turn)
                .map(|c| c.total_tonnage())
        } else {
            None
        };
        Ok(CommandOutcome::ConvoyRecon { revealed, convoy_tonnage })
    }

    /// §44.42 DetermineMaltaAvailability: gated to the Malta Availability Determination Phase.
    fn cmd_determine_malta_availability(
        &mut self,
        level: u32,
        roll: u32,
    ) -> Result<CommandOutcome, CommandError> {
        use crate::reference::squadron::MaltaCommitment;
        self.require_phase(Phase::MaltaAvailabilityDetermination)?;
        if !(1..=4).contains(&level) {
            return Err(CommandError::MaltaLevelUnavailable);
        }
        // §44.23/§44.41: no Availability Level may be used more Game-Turns than the scenario's
        // commitment chart allows. Level I is always unlimited (§44.25).
        if let Some(chart) = self.reference.malta_commitment.as_ref() {
            match chart.commitment(&self.scenario_name, level) {
                Some(MaltaCommitment::NotAllowed) => return Err(CommandError::MaltaLevelUnavailable),
                Some(MaltaCommitment::Turns(limit)) => {
                    if self.malta_level_used.get(&level).copied().unwrap_or(0) >= limit {
                        return Err(CommandError::MaltaLevelUnavailable);
                    }
                }
                _ => {} // Unlimited, or scenario/level not listed — not limited here.
            }
        }
        let cell = self
            .reference
            .malta_availability
            .as_ref()
            .ok_or(CommandError::NoTable)?
            .cell(roll, level);
        // §44.25/§43.0: absolute planes available = map-percent of the in-play Italy/Sicily bombers,
        // plus the strategic-percent added from out-of-play strategic aircraft.
        let in_play = self.med_based_planes(crate::state::air::MedBase::ItalySicily);
        let planes_available = cell.map_or(0, |(map, strat)| {
            crate::rules::air::malta_map_usable(map, in_play)
                + crate::rules::air::malta_strategic_addable(strat, in_play)
        });
        // §44.29: the level counts as used this Game-Turn whether or not the raid is flown.
        *self.malta_level_used.entry(level).or_insert(0) += 1;
        Ok(CommandOutcome::MaltaAvailability { percentages: cell, planes_available })
    }

    /// §38.34 RefitSquadron: gated to the Land Support Air Phase (Tactical Maintenance Segment);
    /// rolls once and refits the squadron's un-refitted planes via the §38.37 table.
    fn cmd_refit_squadron(
        &mut self,
        squadron: crate::state::air::SquadronId,
        base_die: u32,
        wrong_squadron: bool,
    ) -> Result<CommandOutcome, CommandError> {
        self.require_maintenance_phase()?;
        let (nationality, location) = {
            let s = self.sgsus.get(&squadron).ok_or(CommandError::NoSuchSquadron)?;
            (s.nationality, s.location)
        };
        // §35.14: an unsupplied SGSU may not repair its planes.
        if !self.sgsus.get(&squadron).is_some_and(|s| s.supplied) {
            return Err(CommandError::SgsuUnsupplied);
        }
        // §38.36: expend one Stores Point (present at the base) per squadron attempting refit.
        let paid = location.is_some_and(|hex| self.consume_stores_at(hex, nationality.side(), 1));
        if !paid {
            return Err(CommandError::InsufficientStores);
        }
        let modifier = crate::rules::air::refit_die_modifier(nationality, wrong_squadron);
        let table = self.reference.aircraft_refit.as_ref().ok_or(CommandError::NoTable)?;
        let sgsu = self.sgsus.get_mut(&squadron).ok_or(CommandError::NoSuchSquadron)?;
        Ok(CommandOutcome::SquadronRefitted(sgsu.refit(table, base_die, modifier)))
    }

    /// Combat may only be resolved in the Movement & Combat phase (§3.6).
    fn require_combat_phase(&self) -> Result<(), CommandError> {
        if self.phase == Phase::MovementAndCombat {
            Ok(())
        } else {
            Err(CommandError::WrongPhase)
        }
    }

    /// All listed units must exist.
    fn require_units_exist(&self, ids: &[UnitId]) -> Result<(), CommandError> {
        if ids.iter().all(|&id| self.unit(id).is_some()) {
            Ok(())
        } else {
            Err(CommandError::UnitNotFound)
        }
    }

    /// §3.33: an empty HQ (no combat units attached) may not voluntarily attack.
    fn require_no_noncombat_hq(&self, attackers: &[UnitId]) -> Result<(), CommandError> {
        if attackers.iter().any(|&id| self.is_noncombat_hq(id)) {
            Err(CommandError::NonCombatHq)
        } else {
            Ok(())
        }
    }

    /// The common side of the attacking units (they must all be one side).
    fn attackers_side(&self, attackers: &[UnitId]) -> Result<Side, CommandError> {
        let mut sides = attackers
            .iter()
            .filter_map(|&id| self.unit(id))
            .map(|u| u.nationality.side());
        let first = sides.next().ok_or(CommandError::NoParticipants)?;
        if sides.all(|s| s == first) {
            Ok(first)
        } else {
            Err(CommandError::AttackersMixedSides)
        }
    }

    /// Enemy (relative to `attacker_side`) units effectively in `hex`.
    fn enemy_units_in_hex(&self, hex: HexCoord, attacker_side: Side) -> Vec<UnitId> {
        let enemy = attacker_side.opponent();
        self.units_in_hex(hex)
            .into_iter()
            .filter(|&id| self.unit(id).is_some_and(|u| u.nationality.side() == enemy))
            .collect()
    }

    /// Every attacker must be an on-map counter adjacent to `hex`.
    fn require_all_adjacent(&self, attackers: &[UnitId], hex: HexCoord) -> Result<(), CommandError> {
        for &a in attackers {
            let ah = self.effective_hex(a).ok_or(CommandError::UnitNotOnMap)?;
            if !ah.is_adjacent(hex) {
                return Err(CommandError::NotAdjacent);
            }
        }
        Ok(())
    }

    /// Barrage (§12.6). The firer names an adjacent hex and a target class (§12.23–24);
    /// the engine resolves against an enemy unit of that class there. Detailed
    /// line-of-fire legality is deferred (§12.35: no LOS restrictions in the desert).
    fn cmd_barrage(
        &mut self,
        attackers: &[UnitId],
        target: HexCoord,
        target_class: BarrageTargetClass,
    ) -> Result<CommandOutcome, CommandError> {
        self.require_combat_phase()?;
        if attackers.is_empty() {
            return Err(CommandError::NoParticipants);
        }
        self.require_units_exist(attackers)?;
        self.require_no_noncombat_hq(attackers)?;
        let side = self.attackers_side(attackers)?;
        // §12.31: artillery barrages an *adjacent* occupied hex.
        self.require_all_adjacent(attackers, target)?;
        // §12.22–24: eligible targets are enemy combat counters of the named class in the
        // hex (empty HQs excluded). The firer picks by class; we resolve against one such
        // unit (a specific-unit designation via Patrol, §12.23, is deferred).
        let target_unit = self
            .enemy_units_in_hex(target, side)
            .into_iter()
            .find(|&id| !self.is_noncombat_hq(id) && self.barrage_class(id) == target_class)
            .ok_or(CommandError::NoTargets)?;
        // §50.14: each barraging unit expends Ammo (rate × TOE) from its reserve / hex dumps.
        if !self.consume_combat_ammo(attackers, crate::state::game::CombatAction::Barrage) {
            return Err(CommandError::InsufficientAmmo);
        }
        Ok(CommandOutcome::Barrage(self.resolve_barrage(attackers, target_unit)))
    }

    /// Anti-Armor fire (§14) at a target hex. Range/line-of-fire legality is deferred.
    fn cmd_anti_armor(
        &mut self,
        attackers: &[UnitId],
        target_hex: HexCoord,
    ) -> Result<CommandOutcome, CommandError> {
        self.require_combat_phase()?;
        if attackers.is_empty() {
            return Err(CommandError::NoParticipants);
        }
        self.require_units_exist(attackers)?;
        self.require_no_noncombat_hq(attackers)?;
        if self.map.terrain(target_hex).is_none() {
            return Err(CommandError::DestinationOffMap);
        }
        // Anti-Armor fire is against an adjacent hex (§14), like other close combat.
        if !self.attackers_adjacent_to(attackers, target_hex) {
            return Err(CommandError::NotAdjacent);
        }
        // §50.14: anti-armor fire expends Ammo (rate × TOE) from reserve / hex dumps.
        if !self.consume_combat_ammo(attackers, crate::state::game::CombatAction::AntiArmor) {
            return Err(CommandError::InsufficientAmmo);
        }
        Ok(CommandOutcome::AntiArmor(
            self.resolve_anti_armor(attackers, target_hex),
        ))
    }

    /// Whether at least one attacker is an on-map counter adjacent to `hex`.
    fn attackers_adjacent_to(&self, attackers: &[UnitId], hex: HexCoord) -> bool {
        attackers
            .iter()
            .filter_map(|&a| self.effective_hex(a))
            .any(|ah| ah.is_adjacent(hex))
    }

    /// Close Assault (§15) against a target hex: every enemy unit in the hex defends
    /// together (§15.23), so the caller need not (and cannot) enumerate the defenders.
    fn cmd_close_assault(
        &mut self,
        attackers: &[UnitId],
        target: HexCoord,
    ) -> Result<CommandOutcome, CommandError> {
        self.require_combat_phase()?;
        if attackers.is_empty() {
            return Err(CommandError::NoParticipants);
        }
        self.require_units_exist(attackers)?;
        self.require_no_noncombat_hq(attackers)?;
        // §52.51/52: a water-deprived unit may not engage in offensive close assault.
        if attackers.iter().any(|&a| self.unit_water_deprived(a)) {
            return Err(CommandError::NoWater);
        }
        let side = self.attackers_side(attackers)?;
        // Every attacker must be adjacent to the attacked hex (§15.23).
        self.require_all_adjacent(attackers, target)?;
        // The defenders are the entire enemy stack in the attacked hex.
        let defenders = self.enemy_units_in_hex(target, side);
        if defenders.is_empty() {
            return Err(CommandError::NoTargets);
        }
        // §50.14: the assaulting units expend Ammo (armor/gun-class only; rifle infantry pays
        // none). Defensive-assault consumption and no-ammo defender penalties are deferred.
        if !self.consume_combat_ammo(attackers, crate::state::game::CombatAction::CloseAssault) {
            return Err(CommandError::InsufficientAmmo);
        }
        Ok(CommandOutcome::CloseAssault(
            self.resolve_close_assault(attackers, &defenders),
        ))
    }

    /// Satisfy a mandated Retreat (§15.82). The path is a sequence of adjacent hexes, each
    /// leading strictly away from the assaulted hex, on-map, enterable, not enemy-occupied,
    /// and not into an enemy ZOC (§10.23). Each mandated hex not retreated costs 10% loss;
    /// an empty path means the unit stands and absorbs the full penalty. Splitting a stack
    /// into different hexes (§9.35) and supply-direction "least resistance" are deferred.
    fn cmd_retreat(&mut self, unit: UnitId, path: &[HexCoord]) -> Result<CommandOutcome, CommandError> {
        self.require_combat_phase()?;
        let u = self.unit(unit).ok_or(CommandError::UnitNotFound)?;
        let start = u.hex().ok_or(CommandError::UnitNotOnMap)?;
        let pending = u.pending_retreat;
        if pending == 0 {
            return Err(CommandError::NoRetreatPending);
        }
        let side = u.nationality.side();
        let enemy = side.opponent();
        let origin = u.retreat_origin.unwrap_or(start);

        // Validate the path.
        let mut prev = start;
        let mut total_cp = Fixed(0);
        for &h in path {
            if !prev.is_adjacent(h) {
                return Err(CommandError::NotAdjacent);
            }
            let cp = match self.move_step_cost(prev, h, MoveMode::NonMotorized)
                .ok_or(CommandError::DestinationOffMap)?
            {
                MoveCost::Cost(f) => f,
                MoveCost::Prohibited | MoveCost::EntireCpa => {
                    return Err(CommandError::ProhibitedTerrain)
                }
            };
            if !self.enemy_units_in_hex(h, side).is_empty() {
                return Err(CommandError::ProhibitedTerrain); // §8.13: not into an enemy-held hex
            }
            if self.hex_in_zoc_of(h, enemy) {
                return Err(CommandError::RetreatIntoZoc);
            }
            if h.distance(origin) <= prev.distance(origin) {
                return Err(CommandError::RetreatNotAway);
            }
            total_cp = Fixed(total_cp.0 + cp.0);
            prev = h;
        }
        // Stacking at the destination (§9.35).
        if let Some(&dest) = path.last() {
            let terr = self.map.terrain(dest).ok_or(CommandError::DestinationOffMap)?;
            if let Some(limit) = self.reference.terrain.get(&terr).and_then(|p| p.stacking_limit) {
                if self.hex_stacking_points(dest) + self.effective_stacking_points(unit) > limit {
                    return Err(CommandError::StackingLimitExceeded);
                }
            }
        }

        // §15.82: 10% additional loss per mandated hex not retreated, via the §15.83
        // raw-points model (defensive Close Assault strength, rounded down).
        let hexes_short = pending.saturating_sub(path.len() as u8);
        let toe_removed = if hexes_short > 0 {
            let pct = (hexes_short as u16 * 10).min(100) as u8;
            self.apply_raw_loss(&[unit], pct, false, false)
        } else {
            0
        };

        // Apply the move + CP cost and clear the pending retreat.
        if let Some(&dest) = path.last() {
            self.unit_mut(unit).expect("checked").location = Some(crate::state::unit::UnitLocation::OnMap(dest));
        }
        let cpa = self.unit_cpa(unit).unwrap_or(0);
        self.expend_cp_against(unit, total_cp, cpa);
        let u = self.unit_mut(unit).expect("checked");
        u.pending_retreat = 0;
        u.retreat_origin = None;
        Ok(CommandOutcome::Retreated { toe_removed, hexes_short })
    }

    /// Reaction (§8.5): a non-phasing unit evades before it can be assaulted. Gated by
    /// §8.53 (must be non-phasing, motorized, not Engaged, and not already in an enemy ZOC)
    /// and triggered by an adjacent enemy (§8.51); the reaction path may never enter an
    /// enemy ZOC (§8.55). Deferred: the CPA/size "pinning" rules (§8.53b/§8.54, which need a
    /// declared close assault), the truck-stacking exception (§8.53a), and detach-to-react
    /// (§8.56).
    /// Per-step move cost from `from` into `to`, including the §29.44 sandstorm penalty:
    /// movement costs are doubled when entering a hex under an active sandstorm.
    fn move_step_cost(&self, from: HexCoord, to: HexCoord, mode: MoveMode) -> Option<MoveCost> {
        let base = step_cost(&self.reference, &self.map, from, to, mode)?;
        if self.sandstorm_at(to) {
            if let MoveCost::Cost(f) = base {
                return Some(MoveCost::Cost(Fixed(f.0 * 2)));
            }
        }
        Some(base)
    }

    /// Off-map movement toward the map (§8.8), gated to the Movement & Combat phase. Only
    /// Axis units are ever in the off-map complex (§8.82); `advance_off_map_toward_map`
    /// enforces this by returning `NotOffMap` for any on-map unit.
    fn cmd_move_off_map(&mut self, unit: UnitId, direction: crate::state::game::OffMapDir) -> Result<CommandOutcome, CommandError> {
        if self.phase != Phase::MovementAndCombat {
            return Err(CommandError::WrongPhase);
        }
        self.unit(unit).ok_or(CommandError::UnitNotFound)?;
        self.advance_off_map(unit, direction)
            .map(CommandOutcome::OffMapMoved)
            .map_err(CommandError::OffMap)
    }

    fn cmd_react(&mut self, unit: UnitId, path: &[HexCoord]) -> Result<CommandOutcome, CommandError> {
        if self.phase != Phase::MovementAndCombat {
            return Err(CommandError::WrongPhase);
        }
        let u = self.unit(unit).ok_or(CommandError::UnitNotFound)?;
        let start = u.hex().ok_or(CommandError::UnitNotOnMap)?;
        let side = u.nationality.side();
        // Reaction is by the non-phasing player (§8.5).
        if side == self.active_player {
            return Err(CommandError::CannotReact);
        }
        let enemy = self.active_player;
        if u.status.engaged {
            return Err(CommandError::CannotReact); // §8.53d
        }
        if self.hex_in_zoc_of(start, enemy) {
            return Err(CommandError::CannotReact); // §8.53c
        }
        if !self.unit_is_motorized(unit) {
            return Err(CommandError::CannotReact); // §8.53a
        }
        // §8.51: an enemy combat unit must have moved adjacent.
        let triggered = self
            .map
            .neighbors(start)
            .any(|h| !self.enemy_units_in_hex(h, side).is_empty());
        if !triggered {
            return Err(CommandError::NoReactionTrigger);
        }
        // §8.53b/§8.54: a fast enough (CPA ≥6 higher), large enough adjacent enemy pins the
        // unit and prevents reaction. §8.54: a unit cannot pin one two or more size levels
        // larger (a battalion cannot pin a division, a company cannot pin a brigade+). Size
        // uses the shell-reduced org (§9.28). (The §8.53b "declared close assault"
        // precondition is assumed available to the phasing player.)
        let react_cpa = self.unit_cpa(unit).unwrap_or(0);
        let react_rank = self.effective_org_size(unit).size_rank();
        let pinned = self.map.neighbors(start).any(|h| {
            self.enemy_units_in_hex(h, side).iter().any(|&e| {
                let e_cpa = self.unit_cpa(e).unwrap_or(0);
                let e_rank = self.effective_org_size(e).size_rank();
                e_cpa >= react_cpa + 6 && react_rank - e_rank <= 1
            })
        });
        if pinned {
            return Err(CommandError::Pinned);
        }
        // §8.55: reaction moves normally but may never enter an enemy ZOC (and expends no
        // Break-Off cost, §8.52).
        let mut prev = start;
        let mut total_cp = Fixed(0);
        for &h in path {
            if !prev.is_adjacent(h) {
                return Err(CommandError::NotAdjacent);
            }
            let cp = match self.move_step_cost(prev, h, MoveMode::Motorized)
                .ok_or(CommandError::DestinationOffMap)?
            {
                MoveCost::Cost(f) => f,
                MoveCost::Prohibited | MoveCost::EntireCpa => {
                    return Err(CommandError::ProhibitedTerrain)
                }
            };
            if !self.enemy_units_in_hex(h, side).is_empty() {
                return Err(CommandError::ProhibitedTerrain); // §8.13: not into an enemy-held hex
            }
            if self.hex_in_zoc_of(h, enemy) {
                return Err(CommandError::ReactIntoZoc);
            }
            total_cp = Fixed(total_cp.0 + cp.0);
            prev = h;
        }
        if let Some(&dest) = path.last() {
            let terr = self.map.terrain(dest).ok_or(CommandError::DestinationOffMap)?;
            if let Some(limit) = self.reference.terrain.get(&terr).and_then(|p| p.stacking_limit) {
                if self.hex_stacking_points(dest) + self.effective_stacking_points(unit) > limit {
                    return Err(CommandError::StackingLimitExceeded);
                }
            }
            self.unit_mut(unit).expect("checked").location = Some(crate::state::unit::UnitLocation::OnMap(dest));
        }
        let cpa = self.effective_cpa(unit).unwrap_or(0);
        self.expend_cp_against(unit, total_cp, cpa);
        Ok(CommandOutcome::Reacted)
    }

    fn move_step(
        &mut self,
        unit: UnitId,
        to: HexCoord,
        mode: MoveMode,
    ) -> Result<CommandOutcome, CommandError> {
        let cpa = self.effective_cpa(unit).unwrap_or(0);
        self.move_counter(unit, to, mode, cpa)
    }

    /// Move a formation one hex: the parent counter moves (attached units ride along via
    /// `effective_hex`), and over-expenditure is judged against the formation CPA (§6.15).
    fn formation_move(
        &mut self,
        formation: UnitId,
        to: HexCoord,
        mode: MoveMode,
    ) -> Result<CommandOutcome, CommandError> {
        let cpa = self.formation_cpa(formation).unwrap_or(0);
        self.move_counter(formation, to, mode, cpa)
    }

    /// Shared single-hex move for a counter. `cpa` governs the CPA over-expenditure
    /// penalty (§6.21) — the unit's own CPA for a lone counter, or the formation CPA for a
    /// formation move.
    fn move_counter(
        &mut self,
        unit: UnitId,
        to: HexCoord,
        mode: MoveMode,
        cpa: i32,
    ) -> Result<CommandOutcome, CommandError> {
        // Phase gate (§3.6).
        if self.phase != Phase::MovementAndCombat {
            return Err(CommandError::WrongPhase);
        }
        // The unit must be an independent on-map counter.
        let from = self
            .unit(unit)
            .ok_or(CommandError::UnitNotFound)?
            .hex()
            .ok_or(CommandError::UnitNotOnMap)?;
        // §8.14: a unit that moved into an enemy ZOC this stage must stop.
        if self.unit(unit).expect("checked").moved_into_zoc {
            return Err(CommandError::MustStopInZoc);
        }
        if !from.is_adjacent(to) {
            return Err(CommandError::NotAdjacent);
        }
        // §10.24 / §8.15.1: a unit may not move directly from one enemy-controlled hex
        // into another.
        let enemy = self
            .unit(unit)
            .expect("unit existence checked above")
            .nationality
            .side()
            .opponent();
        let from_zoc = self.hex_in_zoc_of(from, enemy);
        let to_zoc = self.hex_in_zoc_of(to, enemy);
        if from_zoc && to_zoc {
            return Err(CommandError::ZocToZoc);
        }
        // Total step cost = hex entry + hexside crossing (also validates on-map dest).
        let cost = self.move_step_cost(from, to, mode)
            .ok_or(CommandError::DestinationOffMap)?;
        let mut cp = match cost {
            MoveCost::Cost(f) => f,
            MoveCost::Prohibited | MoveCost::EntireCpa => {
                return Err(CommandError::ProhibitedTerrain)
            }
        };
        // §8.15.2/3: leaving an enemy ZOC costs +2 CP if in Contact, +4 CP if Engaged.
        if from_zoc {
            let u = self.unit(unit).expect("checked");
            let extra = if u.status.engaged {
                40
            } else if u.status.in_contact {
                20
            } else {
                0
            };
            cp = Fixed(cp.0 + extra);
        }
        // §26.21/26.24: entering an enemy minefield. In addition to terrain, a non-engineer
        // expends its full CPA (§26.22 "far over CPA"); an Engineer/HQ pays only +4 CP. A Friendly
        // minefield (known paths, §26.12) costs nothing extra.
        let mut entered_enemy_dummy = false;
        let mut mine_casualty_check = false;
        if let Some(mf) = self.minefields.get(&to).copied() {
            let mover_side = self.unit(unit).map(|u| u.nationality.side());
            if Some(mf.side) != mover_side {
                let eng = self
                    .unit(unit)
                    .and_then(|u| {
                        self.reference
                            .unit_characteristics
                            .get(&u.nationality)
                            .and_then(|m| m.get(&u.id_code))
                    })
                    .is_some_and(|c| {
                        c.unit_type.contains("Engineer") || c.unit_type.contains("Headquarters")
                    });
                cp = Fixed(cp.0 + if eng { 40 } else { cpa * 10 });
                entered_enemy_dummy = mf.dummy;
                // §26.25: a vehicle without an engineer risks a mine casualty.
                let is_vehicle = self.unit(unit).is_some_and(|u| {
                    matches!(u.composition, crate::state::unit::Composition::Weapons { .. })
                        || u.trucks.light + u.trucks.medium + u.trucks.heavy > 0
                });
                mine_casualty_check = !eng && is_vehicle;
            }
        }
        // §9.14/§9.31: may not end a move where total stacking points exceed the terrain
        // limit. (The mover is currently at `from`, so it is not yet counted at `to`.)
        let to_terrain = self.map.terrain(to).ok_or(CommandError::DestinationOffMap)?;
        if let Some(limit) = self
            .reference
            .terrain
            .get(&to_terrain)
            .and_then(|p| p.stacking_limit)
        {
            let mover_sp = self.effective_stacking_points(unit);
            if self.hex_stacking_points(to) + mover_sp > limit {
                return Err(CommandError::StackingLimitExceeded);
            }
        }
        // §52.51: a vehicle (or truck-motorized unit) deprived of Water may not move.
        if self.unit_water_deprived(unit) {
            let is_vehicle = matches!(
                self.unit(unit).map(|u| &u.composition),
                Some(crate::state::unit::Composition::Weapons { .. })
            ) || self
                .unit(unit)
                .is_some_and(|u| u.trucks.light + u.trucks.medium + u.trucks.heavy > 0);
            if is_vehicle {
                return Err(CommandError::NoWater);
            } else {
                // §52.52: deprived infantry may not voluntarily exceed their CPA this stage.
                let already = self.unit(unit).map(|u| u.cp_expended.0).unwrap_or(0);
                if already + cp.0 > cpa * 10 {
                    return Err(CommandError::NoWater);
                }
            }
        }
        // §52.6: an Italian battalion that missed its Pasta Point may not voluntarily exceed CPA.
        if self.no_pasta.contains(&unit) {
            let already = self.unit(unit).map(|u| u.cp_expended.0).unwrap_or(0);
            if already + cp.0 > cpa * 10 {
                return Err(CommandError::NoPasta);
            }
        }
        // §51.23: a unit on half rations may not voluntarily exceed its CPA, nor enter an enemy ZOC.
        if self.half_rations.contains(&unit) {
            if to_zoc {
                return Err(CommandError::HalfRation);
            }
            let already = self.unit(unit).map(|u| u.cp_expended.0).unwrap_or(0);
            if already + cp.0 > cpa * 10 {
                return Err(CommandError::HalfRation);
            }
        }
        // §49.13/49.16: pay the move's Fuel from the mover's (and riders') tanks and any
        // Supply Dump in the start hex; a move with no available Fuel is illegal.
        if !self.consume_move_fuel(unit, from, cp.0) {
            return Err(CommandError::InsufficientFuel);
        }
        // Apply: move and accumulate CP expended this Operations Stage (with any
        // Disorganization for exceeding the governing CPA).
        self.unit_mut(unit)
            .expect("unit existence checked above")
            .location = Some(crate::state::unit::UnitLocation::OnMap(to));
        self.expend_cp_against(unit, cp, cpa);
        // §26.15/26.23: entering an enemy dummy minefield reveals it (removed at the end of the
        // Movement Phase — i.e. at the next Operations Stage — so same-phase movers still pay).
        if entered_enemy_dummy {
            self.revealed_minefields.insert(to);
        }
        // §26.25: mine-casualty check for a non-engineer vehicle entering an enemy minefield.
        if mine_casualty_check {
            let die = self.rng.d6();
            self.resolve_mine_casualty(unit, die);
        }
        // §21.21/24: accumulate the entered hex's Breakdown Points for the mover and its riders,
        // then check for breakdown on ceasing movement (no-op unless the §21.38 table is loaded).
        let hex_bp = self
            .map
            .terrain(to)
            .and_then(|t| self.reference.terrain.get(&t))
            .map(|p| p.breakdown.0)
            .unwrap_or(0);
        for m in self.formation_members(unit) {
            if let Some(u) = self.unit_mut(m) {
                u.breakdown_points += hex_bp;
            }
            self.resolve_breakdown(m);
        }
        // §8.14: having entered an enemy ZOC, the unit must stop for the rest of the stage.
        if to_zoc {
            self.unit_mut(unit).expect("checked").moved_into_zoc = true;
        }
        Ok(CommandOutcome::Moved)
    }
}
