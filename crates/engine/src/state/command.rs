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
    /// §31: move the Rommel counter one hex to an adjacent `to` (Movement & Combat Phase). He moves
    /// as a motorized medium truck, spending his 60-CP Movement Allowance; he may not enter an enemy
    /// Zone of Control unoccupied by a Friendly unit (a normal move).
    MoveRommel { to: HexCoord },
    /// §31.2: Rommel reacts/retreats one hex to an adjacent `to`, which may pass through an enemy
    /// Zone of Control "with impunity" (before an assault reaches his hex).
    RetreatRommel { to: HexCoord },
    /// §31.4: designate the co-located `unit` to receive Rommel's +5 CPA this Operations Stage
    /// (he must be with it at the start of the stage and remain — see `rommel_cpa_bonus`).
    DesignateRommelUnit { unit: UnitId },
    /// §8.97/§8.98: detach `light`/`medium`/`heavy` first-line Truck Points (and their loaded cargo)
    /// from `from_unit` into a new free truck convoy in the unit's hex (Organization Phase).
    FormTruckConvoy { from_unit: UnitId, light: u32, medium: u32, heavy: u32 },
    /// §8.98/§53: move the free truck convoy at index `convoy` one hex to adjacent `to` (Truck
    /// Convoy Phase). It spends its supply CPA and may not enter an enemy ZOC unoccupied by a
    /// Friendly unit (§10.29).
    MoveTruckConvoy { convoy: usize, to: HexCoord },
    /// §27.72/§27.73: land the Layforce commando (which has spent a full Game-Turn in Alexandria)
    /// in any coastal hex `to` (Movement & Combat Phase). Costs 5 CP per 50 hexes the transporting
    /// ships moved (`ship_hexes`) plus the hex's terrain entry cost; grants the §27.74 supply window.
    LandLayforce { unit: UnitId, to: HexCoord, ship_hexes: u32 },
    /// §27.77: re-embark the Layforce commando from its coastal hex onto its ship, which returns it
    /// to Alexandria (Movement & Combat Phase).
    EmbarkLayforce { unit: UnitId },
    /// §27.83/§27.84: land the SAS Brigade by sea in any coastal hex `to` (Movement & Combat Phase),
    /// costing 5 CP per 50 ship-hexes plus terrain; grants the §27.84 three-Operations-Stage supply.
    LandSas { unit: UnitId, to: HexCoord, ship_hexes: u32 },
    /// §27.85/§27.86: the SAS Brigade raids the Axis air facility in its hex (no Axis combat units
    /// present), destroying the §27.93-table percentage of grounded planes; `roll` is a d6. Subject
    /// to the §27.87 once-a-month / ≥6-Operations-Stage frequency limit.
    RaidAirfieldSas { unit: UnitId, roll: u32 },
    /// §27.13: form a desert raider (LRDG / DSA) from a Friendly Recce/AC `from_unit`, removing one
    /// TOE Strength Point and placing the counter in its hex (Organization Phase).
    FormRaider { kind: crate::state::game::RaiderKind, from_unit: UnitId },
    /// §27.3: move the raider at index `raider` one hex to adjacent `to` (Movement & Combat Phase).
    /// Raiders ignore enemy Zones of Control (§27.34) but may not enter an Enemy-occupied hex except
    /// to raid (§27.42). Entering an enemy-controlled (ZOC) hex risks being spotted: `spot_roll` is a
    /// d6 and a 6 eliminates the raider (§27.41/§27.43).
    MoveRaider { raider: usize, to: HexCoord, spot_roll: u32 },
    /// §27.44: an Enemy attempts to spot the raider at index `raider` (spending 1 CP); `roll` is a
    /// d6 and a 6 eliminates it (§27.43).
    SpotRaider { raider: usize, roll: u32 },
    /// §27.35: the raider at index `raider` resupplies where it stands — drawing 2 Fuel + 1 Ammo
    /// from a co-located Friendly supply dump, and Water from an oasis/bir in the hex. It must draw
    /// Water each Game-Turn or be eliminated, and needs 2 Fuel to move (§27.35).
    ResupplyRaider { raider: usize },
    /// §27.6: a Commonwealth LRDG in Rommel's hex attempts a Raid on Rommel (Movement & Combat
    /// Phase). `roll` is 2d6 (2–12) resolved on the §27.64 table. Costs 10 CP; max four per campaign.
    RaidRommel { raider: usize, roll: u32 },
    /// §27.52: the raider blows the water pipeline in its hex (no enemy combat units present); a d6
    /// `roll` of 1-4 blows it. Costs 5 CP. (Movement & Combat Phase.)
    RaidPipeline { raider: usize, roll: u32 },
    /// §27.53: the raider cripples the (non-strip, non-major-city) airfield in its hex (no enemy
    /// combat units present); a d6 `roll` of 1-2 reduces its capacity one level. Costs 5 CP.
    RaidAirfield { raider: usize, roll: u32 },
    /// §27.54: the raider destroys grounded enemy planes in its hex (no enemy combat units / not a
    /// major city); a d6 `roll` of 1-2 destroys 10% of each enemy squadron's planes, a 6 eliminates
    /// the raider. Costs 5 CP.
    RaidPlanes { raider: usize, roll: u32 },
    /// §27.55: the raider attacks the enemy Supply Dump in its hex (not a major city). If guarded by
    /// enemy combat units, `attack_roll` (2d6) must exceed the guards' Raw Defensive Assault Strength
    /// or the raider is eliminated; then `destroy_roll` (d6) of 1-2 destroys 10% of the dump. Costs
    /// 5 CP. (§27.56: a dummy dump is revealed instead.)
    RaidSupplyDump { raider: usize, attack_roll: u32, destroy_roll: u32 },
    /// §27.57-58: the raider attacks the enemy free truck convoy at index `convoy` in its hex; a d6
    /// `roll` of 1-2 destroys one Truck Point + its cargo. Replacement-carrying convoys add 1 to the
    /// roll and a 6 eliminates the raider (§27.58). Costs 5 CP.
    RaidTruckConvoy { raider: usize, convoy: usize, roll: u32 },
    /// §27.59: during the enemy's Truck Convoy Phase, a raider within four Movement CPs of the
    /// passing convoy at index `convoy` halts and attacks it (§27.58 mechanics, a d6 `roll`).
    InterceptTruckConvoy { raider: usize, convoy: usize, roll: u32 },
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
    /// Retreat Before Assault (§13): a non-phasing, unpinned unit voluntarily withdraws along
    /// `path` after barrages, before the assault sub-segments. Ordinary voluntary movement (CP +
    /// Fuel + breakdown + §8.6 break-off), but capped at 4 CP / one hex if it did not begin the
    /// step adjacent to an enemy combat unit (§13.24). Distinct from React (§8.5).
    RetreatBeforeAssault {
        unit: UnitId,
        path: Vec<HexCoord>,
    },
    /// §16 Patrol & Reconnaissance: the phasing player detaches `patrol_points` (origin unit → TOE
    /// Strength Points) to reconnoitre the enemy-occupied `target` (≤5 hexes). Resolved on the §16.6
    /// Survival, §16.7 Reconnaissance and §16.8 Objective-Loss tables with the supplied dice. The
    /// non-phasing player's disclosures are given in `reveals` (unit → reported TOE, within ±2 of
    /// actual, §16.51) and his Objective-Loss casualties in `loss_choice` (unit → TOE removed);
    /// `exclude` names units the patroller already knows and does not wish revealed (§16.55). Patrol
    /// Phase. Returns a one-time [`PatrolReport`] (§16 disclosures are not persisted; decision 2026-08-31).
    Patrol {
        patrol_points: Vec<(UnitId, u32)>,
        target: HexCoord,
        exclude: Vec<UnitId>,
        reveals: Vec<(UnitId, i32)>,
        loss_choice: Vec<(UnitId, u32)>,
        survival_die: u32,
        recon_die: u32,
        loss_die: u32,
    },
    /// §16.4 place a Dummy Tank Formation for the phasing side at `hex` with a borrowed
    /// `designation` (§16.44), expending 10 Stores (§16.42). Organization (Construction) Phase.
    PlaceDummyTank {
        hex: HexCoord,
        designation: String,
    },
    /// §19.41/§19.42: attach `unit` to a co-located `parent` formation. An assigned unit attaches
    /// for 1 CP each any time (§19.41); a non-assigned unit for 2 CP each in the Reorganization
    /// (Organization) Phase only, subject to the §19.5 Maximum-Attachment limit (§19.42).
    AttachUnit { unit: UnitId, parent: UnitId },
    /// §19.43/§19.44: detach `unit` from its Parent Formation for 1 CP each (Organization or
    /// Movement Phase).
    DetachUnit { unit: UnitId },
    /// §19.21/§19.28: (re)assign `unit` — currently attached to `parent` and not assigned elsewhere
    /// — to that Parent Formation, recorded in the Reorganization (Organization) Phase (no CP cost).
    ReassignUnit { unit: UnitId, parent: UnitId },
    /// §28.22 form a Guard Point for the phasing side's Prisoner stack at `group_hex`, subtracting
    /// one TOE from a co-located infantry combat unit. Organization Phase.
    FormGuard { group_hex: HexCoord },
    /// §28.22 return one Guard Point from the stack at `group_hex` to a co-located infantry unit
    /// (restoring 1 TOE). Organization Phase.
    ReturnGuard { group_hex: HexCoord },
    /// §28.13 move the phasing side's Prisoner stack one hex from `from` to `to` in the Truck Convoy
    /// Phase (CPA 8), requiring enough guards to escort them (§28.17); un-escortable excess escapes
    /// (§28.23). Not into an enemy-controlled/occupied hex (§28.12).
    MovePrisoners { from: HexCoord, to: HexCoord },
    /// §28.3 repair a piece of captured equipment (`system`) at `hex`, making it reusable. Repair
    /// Phase.
    RepairCapturedEquipment { hex: HexCoord, system: String },
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
    /// §20.78A ProduceCommonwealthTrucks: roll the Commonwealth Truck Production Table once
    /// (Organization Phase) — one d6 per truck type. The produced Truck Points arrive four
    /// Game-Turns later at Alexandria (into the Commonwealth free-truck pool).
    ProduceCommonwealthTrucks { light_die: u32, medium_die: u32, heavy_die: u32 },
    /// §20.78B ProduceCommonwealthInfantry: roll the Commonwealth Infantry Production Table once
    /// (Organization Phase) with a 2d6 `roll` (2–12). The produced Infantry Replacement Points
    /// arrive four Game-Turns later and begin Training.
    ProduceCommonwealthInfantry { roll: u32 },
    /// §20.66a ProduceAxisTrucks: plan `count` Truck Points of `kind` (Organization Phase),
    /// validated against the Axis Truck Production row's lifetime total and per-Game-Turn maximum.
    /// On success the trucks arrive two Game-Turns later (into the Axis free-truck pool).
    ProduceAxisTrucks { kind: crate::reference::logistics::TruckKind, count: u32 },
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
    /// §6.17/§53.11: assign free Truck Points from the unit's side's pool to a unit as first-line
    /// trucks (Organization Phase) — the trucks that carry it (motorization) and its supply.
    AssignTrucks { unit: UnitId, light: u32, medium: u32, heavy: u32 },
    /// §53.11: return a unit's first-line Truck Points to its side's free pool (Organization Phase),
    /// so they can be reassigned. Any supply currently loaded on those trucks blocks the release.
    ReleaseTrucks { unit: UnitId, light: u32, medium: u32, heavy: u32 },
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
    /// §54.3 / §54.43: move supply by rail. Commonwealth in its Rail Movement Phase; Axis in the
    /// Truck Convoy Phase against its Rolling-Stock budget (§54.4). Returns the points moved.
    RailMoveSupply {
        from: HexCoord,
        to: HexCoord,
        cargo: crate::state::game::RailCargo,
        points: u32,
    },
    /// §54.43: activate imported Axis Rolling Stock at a controlled operative rail hex (consuming
    /// 250 Stores + 100 Fuel from Axis dumps there for +300 tons of haulage). Organization Phase.
    ImportRollingStock { hex: HexCoord },
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
        /// The enemy squadron flying `interceptors`, if any — its pilot earns §40.18 ace credit for
        /// the bombers it downs. `None` leaves the interception uncredited.
        interceptor_squadron: Option<crate::state::air::SquadronId>,
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
        /// The enemy squadron flying `interceptors`, if any — its pilot earns §40.18 ace credit for
        /// the bombers it downs. `None` leaves the interception uncredited.
        interceptor_squadron: Option<crate::state::air::SquadronId>,
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
        /// §39.3/§39.34: how many of `planes` voluntarily abort before any combat (turn back). 0 =
        /// none abort; `>= planes` = the whole mission aborts; in between is a §39.34 partial abort
        /// (the aborting planes turn back, the rest press on). Any abort is allowed only if the
        /// escort screen (§39.36/§39.37) or bomber Maneuver (§39.35) qualifies.
        abort_planes: u32,
        flak_destroy_roll: u32,
        flak_abort_roll: u32,
        bomb_roll: u32,
    },
}

/// §16.51: the information a Patrol garners about one enemy unit — its historical designation,
/// type, whether it is motorized, and its reported TOE Strength (the defender's chosen value within
/// ±2 of actual). A one-time snapshot; not stored in game state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevealedUnit {
    pub unit: UnitId,
    pub designation: String,
    pub unit_type: String,
    pub motorized: bool,
    pub reported_strength: i32,
}

/// §16 Patrol outcome: the patroller's losses, the reconnaissance disclosures (§16.5/§16.51), and
/// the casualties the patrol inflicted on the reconnoitred hex (§16.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatrolReport {
    pub patroller_killed: u32,
    pub patroller_captured: u32,
    /// True if every Patrol Point was lost (§16.33 — no information; §16.34 Captured→Killed).
    pub wiped_out: bool,
    pub revealed: Vec<RevealedUnit>,
    pub patrollee_killed: u32,
    pub patrollee_captured: u32,
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
    /// A Retreat Before Assault was carried out (§13); `cp` = tenths of a CP expended.
    RetreatedBeforeAssault { cp: i32 },
    /// §16 Patrol result — a one-time reconnaissance report plus the casualties on both sides.
    Patrol(PatrolReport),
    /// §16.4 a Dummy Tank Formation was placed.
    DummyTankPlaced,
    /// §19.41/§19.42 a unit was attached to a Parent Formation.
    UnitAttached,
    /// §19.43/§19.44 a unit was detached from its Parent Formation.
    UnitDetached,
    /// §19.2 a unit was (re)assigned to a Parent Formation.
    UnitReassigned,
    /// §28.22 a Guard Point was formed.
    GuardFormed,
    /// §28.22 a Guard Point was returned to its unit.
    GuardReturned,
    /// §28.13 a Prisoner stack moved; `escaped` = prisoners lost to §28.23 for want of escort.
    PrisonersMoved { escaped: u32 },
    /// §28.3 a piece of captured equipment was repaired (now reusable).
    CapturedEquipmentRepaired,
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
    /// §42.53 naval reconnaissance: whether the recon succeeded, and — if it did and a convoy is in
    /// the lane — the convoy's revealed **size class** (Small/Medium/Large), not its exact tonnage.
    /// The Axis chooses the class it reports among the overlapping §42.53 ranges; `size` is that
    /// adversarial choice (`None` = recon failed or no convoy present).
    ConvoyRecon { revealed: bool, size: Option<crate::rules::naval::ConvoySize> },
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
    /// §6.17/§53.11: Truck Points assigned to / released from a unit — the unit's resulting
    /// first-line Truck Points (Light, Medium, Heavy), and its §6.17 motorized CPA if it is now
    /// leg infantry fully carried by them (`None` = not motorized).
    TrucksAssigned { light: u32, medium: u32, heavy: u32, motorized_cpa: Option<i32> },
    /// §31: the Rommel counter moved/retreated to `hex`, with `mcp_left` tenths of a CP remaining
    /// this Operations Stage.
    RommelMoved { hex: HexCoord, mcp_left: u32 },
    /// §31.4: a unit was designated to receive Rommel's +5 CPA this Operations Stage.
    RommelUnitDesignated,
    /// §27.13: a desert raider was formed; carries its index in `raiders`.
    RaiderFormed { raider: usize },
    /// §27.3: a raider moved to `hex`; `eliminated` = it was removed (spotted §27.43, or forced over
    /// its 50-CP allowance §27.37); `cpa_used` is tenths of a CP spent this Operations Stage.
    RaiderMoved { hex: HexCoord, eliminated: bool, cpa_used: u32 },
    /// §27.43/§27.44: a spotting attempt resolved; `eliminated` = the raider was spotted and removed.
    RaiderSpotChecked { eliminated: bool },
    /// §27.35: a raider resupplied — Fuel/Ammo drawn from a dump and whether Water was taken on.
    RaiderResupplied { fuel: u32, ammo: u32, water: bool },
    /// §27.6/§27.64: a Raid on Rommel resolved — `raider_eliminated` (§27.64 "LRDG elim"),
    /// `rommel_removed` ("AI perm 3", captured), and `axis_temp3` ("AI temp 3", two-turn rating drop).
    RommelRaided { raider_eliminated: bool, rommel_removed: bool, axis_temp3: bool },
    /// §8.97: a free truck convoy was formed; carries its index in `truck_convoys`.
    TruckConvoyFormed { convoy: usize },
    /// §8.98: a free truck convoy moved to `hex`, with `cpa_used` tenths of a CP spent this phase.
    TruckConvoyMoved { hex: HexCoord, cpa_used: u32 },
    /// §27.72: the Layforce commando made an amphibious landing at `hex`.
    LayforceLanded { hex: HexCoord },
    /// §27.77: the Layforce commando re-embarked and was returned to Alexandria.
    LayforceEmbarked,
    /// §27.83: the SAS Brigade landed by sea at `hex`.
    SasLanded { hex: HexCoord },
    /// §27.86: an SAS airfield raid resolved — `planes_pct` of the grounded planes were destroyed.
    SasRaided { planes_pct: u32 },
    /// §27.5: a raider raid resolved — `success` = the raid achieved its effect (pipeline blown,
    /// airfield level lost, planes/supply/truck destroyed); `raider_eliminated` = the raider was
    /// lost (spotted, over-extended §27.37, or a §27.5x elimination result).
    RaidResolved { success: bool, raider_eliminated: bool },
    /// §52.16/§52.17: whether a well was poisoned / sweetened.
    WellPoisoned(bool),
    WellSweetened(bool),
    /// §24: a construction/removal project was started.
    ConstructionStarted,
    /// §8.72: a unit was moved by rail.
    RailMoved,
    /// §54.43: Axis Rolling Stock imported/activated (+300 tons of haulage).
    RollingStockImported,
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
    /// §20.78A/§20.66a: Truck Production resolved — the given Truck Points of each type were
    /// produced. `arrival_game_turn` is the *first* delivery Game-Turn; Commonwealth batches then
    /// spread over four Game-Turns (§20.78A ≤25%/GT), Axis trucks arrive in one (§20.66a +2 GT).
    TrucksProduced { arrival_game_turn: u32, light: u32, medium: u32, heavy: u32 },
    /// §20.78B: Commonwealth Infantry Production rolled — `points` Infantry Replacement Points
    /// were produced and scheduled to arrive on `arrival_game_turn` (0 = a "none" roll this era).
    InfantryProduced { arrival_game_turn: u32, points: u32 },
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
    /// §39.3: a mission was voluntarily aborted before combat — `planes` bombers turned back.
    MissionAborted { planes: u32 },
}

/// Why a command was rejected. The state is left unchanged on any error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    UnitNotFound,
    /// §6.17/§53.11: the side's free-truck pool has fewer Truck Points of a type than requested,
    /// or the unit has fewer assigned than a release asked to return.
    InsufficientTrucks,
    /// §53.11: cannot release Truck Points that still carry loaded supply (unload it first).
    TrucksLoadedCannotRelease,
    /// §31: the Rommel counter is not present on the map (absent from the scenario, not yet
    /// arrived, or recalled to Germany this Game-Turn).
    RommelNotPresent,
    /// §31: Rommel cannot make the requested move (target not adjacent/on-map, blocked by an enemy
    /// Zone of Control, or not enough Movement Allowance left this stage).
    RommelCannotMove,
    /// §27: a desert raider could not be formed (invalid Recce/AC source, or the §27.11/§27.2a pool
    /// is full), or the referenced raider index is not in play.
    RaiderNotAvailable,
    /// §27.3: the raider cannot make the requested move (not adjacent/on-map, or into an
    /// Enemy-occupied hex without raiding, or over its 50-CP Movement Allowance).
    RaiderCannotMove,
    /// §8.98/§10.29: the free truck convoy cannot make the requested move (bad index, not
    /// adjacent/on-map, blocked by an enemy ZOC, or over its Movement Allowance).
    TruckConvoyCannotMove,
    /// §27.7: the Layforce commando cannot make the requested landing/embarkation (not Layforce,
    /// not in Alexandria / not a full Game-Turn there, target not a coastal hex, or over its CPA).
    LayforceCannotLand,
    /// §27.8: the SAS Brigade cannot act (not the SAS unit, no valid air facility / planes / clear
    /// hex to raid, the §27.87 frequency limit, or an invalid sea landing).
    SasCannotAct,
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
    /// §13: the unit may not Retreat Before Assault — it is phasing, Pinned, or its Cohesion Level
    /// is −26 or worse (§13.1).
    CannotRetreatBeforeAssault,
    /// §13.24: a unit not beginning the step adjacent to an enemy combat unit may retreat at most
    /// 4 CP (or one hex).
    RetreatBeforeAssaultTooFar,
    /// §16: the patrol is not permitted — the side engaged in Anti-Armor/Close Assault this stage
    /// (§16.21), an origin unit is ineligible (§16.11) or at Cohesion ≤ −8 (§16.15), or an
    /// origin/target hex is in a storm (§16.17).
    PatrolNotAllowed,
    /// §16: the target is not a legal reconnaissance hex — not enemy-occupied, over five hexes away
    /// (§16.22), or barraged/bombed this stage (§16.21).
    PatrolTargetInvalid,
    /// §16.13/§16.14: at most 2 Patrol Points per origin hex and 3 in total.
    PatrolPointLimit,
    /// §16: the non-Phasing player's disclosures do not satisfy the Reconnaissance result — wrong
    /// number of units, an ineligible or excluded unit (§16.53/§16.54/§16.55), or a reported
    /// strength not within ±2 of actual (§16.51).
    PatrolRevealMismatch,
    /// §16.8/§16.34: the chosen Objective-Loss casualties do not match the rolled result, or are not
    /// taken from a legal unit.
    PatrolLossMismatch,
    /// §16.42/§16.43: a Dummy Tank could not be placed — already 3 in play, one already in the hex,
    /// or fewer than 10 Stores present.
    DummyTankRejected,
    /// §19: an (re)organization was illegal — units not co-located or of different sides, the unit
    /// is already attached elsewhere (§19.13), the §19.5 attachment limit is reached, or a reassign
    /// target is not the unit's current attachment / the unit is assigned elsewhere (§19.21/28).
    ReorgIllegal,
    /// §28: a prisoner action was illegal — no such stack, no eligible infantry unit to form/receive
    /// a guard, an illegal move hex, or insufficient guards to escort a move.
    PrisonerActionIllegal,
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
    /// §20.62: the planned Axis Replacement-Point tonnage for its arrival Game-Turn would exceed
    /// that turn's convoy Shipping Tonnage allowance (there is not enough sea lift to carry it).
    InsufficientConvoyCapacity,
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
    /// §54.43: no eligible controlled operative rail hex, or insufficient Stores/Fuel, to import.
    RollingStockImportFailed,
    /// §42.47: an airdropped unit may not voluntarily exceed its CPA in the stage it was dropped.
    AirdropCpaLimit,
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
    /// §39.36/§39.37: the mission may not voluntarily abort — no sufficient fighter screen and the
    /// bombers' Maneuver does not qualify against the covering enemy CAP.
    AbortNotPermitted,
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
            Command::MoveRommel { to } => self.cmd_move_rommel(to, false),
            Command::RetreatRommel { to } => self.cmd_move_rommel(to, true),
            Command::DesignateRommelUnit { unit } => self.cmd_designate_rommel_unit(unit),
            Command::FormTruckConvoy { from_unit, light, medium, heavy } => {
                self.cmd_form_truck_convoy(from_unit, light, medium, heavy)
            }
            Command::MoveTruckConvoy { convoy, to } => self.cmd_move_truck_convoy(convoy, to),
            Command::LandLayforce { unit, to, ship_hexes } => self.cmd_land_layforce(unit, to, ship_hexes),
            Command::EmbarkLayforce { unit } => self.cmd_embark_layforce(unit),
            Command::LandSas { unit, to, ship_hexes } => self.cmd_land_sas(unit, to, ship_hexes),
            Command::RaidAirfieldSas { unit, roll } => self.cmd_raid_airfield_sas(unit, roll),
            Command::FormRaider { kind, from_unit } => self.cmd_form_raider(kind, from_unit),
            Command::MoveRaider { raider, to, spot_roll } => self.cmd_move_raider(raider, to, spot_roll),
            Command::SpotRaider { raider, roll } => self.cmd_spot_raider(raider, roll),
            Command::ResupplyRaider { raider } => self.cmd_resupply_raider(raider),
            Command::RaidRommel { raider, roll } => self.cmd_raid_rommel(raider, roll),
            Command::RaidPipeline { raider, roll } => self.cmd_raid_pipeline(raider, roll),
            Command::RaidAirfield { raider, roll } => self.cmd_raid_airfield(raider, roll),
            Command::RaidPlanes { raider, roll } => self.cmd_raid_planes(raider, roll),
            Command::RaidSupplyDump { raider, attack_roll, destroy_roll } => {
                self.cmd_raid_supply_dump(raider, attack_roll, destroy_roll)
            }
            Command::RaidTruckConvoy { raider, convoy, roll } => self.cmd_raid_truck_convoy(raider, convoy, roll),
            Command::InterceptTruckConvoy { raider, convoy, roll } => self.cmd_intercept_truck_convoy(raider, convoy, roll),
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
            Command::RetreatBeforeAssault { unit, path } => self.cmd_retreat_before_assault(unit, &path),
            Command::Patrol {
                patrol_points, target, exclude, reveals, loss_choice, survival_die, recon_die, loss_die,
            } => self.cmd_patrol(
                &patrol_points, target, &exclude, &reveals, &loss_choice, survival_die, recon_die, loss_die,
            ),
            Command::PlaceDummyTank { hex, designation } => {
                self.require_phase(Phase::Organization)?;
                if self.place_dummy_tank(self.active_player, hex, designation) {
                    Ok(CommandOutcome::DummyTankPlaced)
                } else {
                    Err(CommandError::DummyTankRejected)
                }
            }
            Command::AttachUnit { unit, parent } => self.cmd_attach_unit(unit, parent),
            Command::DetachUnit { unit } => self.cmd_detach_unit(unit),
            Command::ReassignUnit { unit, parent } => self.cmd_reassign_unit(unit, parent),
            Command::FormGuard { group_hex } => self.cmd_form_guard(group_hex),
            Command::ReturnGuard { group_hex } => self.cmd_return_guard(group_hex),
            Command::MovePrisoners { from, to } => self.cmd_move_prisoners(from, to),
            Command::RepairCapturedEquipment { hex, system } => self.cmd_repair_captured_equipment(hex, system),
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
            Command::ProduceCommonwealthTrucks { light_die, medium_die, heavy_die } => {
                self.cmd_produce_commonwealth_trucks(light_die, medium_die, heavy_die)
            }
            Command::ProduceCommonwealthInfantry { roll } => {
                self.cmd_produce_commonwealth_infantry(roll)
            }
            Command::ProduceAxisTrucks { kind, count } => self.cmd_produce_axis_trucks(kind, count),
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
            Command::AssignTrucks { unit, light, medium, heavy } => {
                self.cmd_assign_trucks(unit, light, medium, heavy)
            }
            Command::ReleaseTrucks { unit, light, medium, heavy } => {
                self.cmd_release_trucks(unit, light, medium, heavy)
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
                self.require_units_exist(&[unit])?;
                // §33.0 IV.K: the Commonwealth rails in its own phase; §54.43: the Axis rails in the
                // Truck Convoy Phase ("the Convoy Stage").
                match self.unit(unit).map(|u| u.nationality.side()) {
                    Some(Side::Commonwealth) => self.require_phase(Phase::CommonwealthRail)?,
                    Some(Side::Axis) => self.require_phase(Phase::TruckConvoy)?,
                    None => return Err(CommandError::UnitNotFound),
                }
                if self.rail_move(unit, to) {
                    Ok(CommandOutcome::RailMoved)
                } else {
                    Err(CommandError::RailMoveFailed)
                }
            }
            Command::RailMoveSupply { from, to, cargo, points } => {
                let side = match self.phase {
                    Phase::CommonwealthRail => Side::Commonwealth,
                    Phase::TruckConvoy => Side::Axis,
                    _ => return Err(CommandError::WrongPhase),
                };
                Ok(CommandOutcome::RailSupplyMoved(self.rail_move_supply_for(side, from, to, cargo, points)))
            }
            Command::ImportRollingStock { hex } => {
                self.require_phase(Phase::Organization)?;
                if self.import_rolling_stock(hex) {
                    Ok(CommandOutcome::RollingStockImported)
                } else {
                    Err(CommandError::RollingStockImportFailed)
                }
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
                        &[], &[],
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
                squadron, planes, bombload_per_plane, ship_index, interceptors, interceptor_squadron,
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
                        &[], &[],
                    )
                    .ok_or(CommandError::NoSuchSquadron)?;
                // §40.18: credit the intercepting squadron's pilot with the bombers it shot down.
                if let Some(sq) = interceptor_squadron {
                    if outcome.intercepted > 0 {
                        self.credit_pilot_kills(sq, outcome.intercepted);
                    }
                }
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
                squadron, planes, bombload_per_plane, mission, interceptors, interceptor_squadron,
                flak_points, flak_destroy_roll, flak_abort_roll, bomb_roll, night,
            } => {
                self.require_phase(Phase::LandSupportAir)?;
                if !self.sgsus.contains_key(&squadron) {
                    return Err(CommandError::NoSuchSquadron);
                }
                let group = crate::rules::air::bombing_target_group(mission)
                    .ok_or(CommandError::NoTable)?; // mining harbours use their own table
                let outcome = self
                    .resolve_bombing_mission(
                        squadron, planes, bombload_per_plane, group, interceptors, None,
                        flak_points, flak_destroy_roll, flak_abort_roll, bomb_roll, night,
                        &[], &[],
                    )
                    .ok_or(CommandError::NoSuchSquadron)?;
                // §40.18: credit the intercepting squadron's pilot with the bombers it shot down.
                if let Some(sq) = interceptor_squadron {
                    if outcome.intercepted > 0 {
                        self.credit_pilot_kills(sq, outcome.intercepted);
                    }
                }
                Ok(CommandOutcome::BombingMission(outcome))
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
                squadron, target, mission, planes, bombload_per_plane, night, escort, abort_planes,
                flak_destroy_roll, flak_abort_roll, bomb_roll,
            } => self.cmd_fly_bombing_mission_at(
                squadron, target, mission, planes, bombload_per_plane, night, escort, abort_planes,
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
        // §56.21: the tonnage allowance is rolled once per Game-Turn. Record the first roll so both
        // supply convoys and §20.62 production arrivals bind against the same figure.
        self.axis_shipping_rolled.entry(arrival_gt).or_insert(allowable);
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

        let tonnage = tonnage_per * count;
        let side = nationality.side();
        let delay = if side == crate::reference::units::Side::Commonwealth { 4 } else { 2 };
        let arrival_game_turn = gt + delay;

        // §20.62: each Axis Replacement Point is counted against the convoy Shipping Tonnage
        // allowance for its arrival Game-Turn — it rides the Axis naval convoys and competes with
        // planned truck production (§20.66a) and supplies for the same tonnage. Reject if this
        // order, plus everything already committed to that Game-Turn (replacement production, Axis
        // truck production, and supply convoys — see `axis_shipping_committed`), would exceed the
        // best-case (die-6) allowance for the turn's month. This plan-time check is only the
        // upper bound (production is planned two turns out, §20.63, before the tonnage die is rolled
        // §56.21); the *actual* binding happens at convoy time, where a supply convoy must fit the
        // rolled allowance after this priority production lift (§20.64 — Replacement Points have
        // priority over supplies; see `plan_convoy`). Commonwealth production lands via the strategic
        // supply line (Alexandria), not Axis convoys, so it is not gated. With no Convoy Level Chart
        // loaded the gate is skipped. (Residual: production planned to the die-6 ceiling that the
        // actual roll cannot cover still all arrives — deferring that excess needs a stored per-turn
        // roll and is left as the one bounded simplification.)
        if side == crate::reference::units::Side::Axis && tonnage > 0 {
            if let Some(chart) = self.reference.convoy_levels.as_ref() {
                let d = crate::state::game::date_of(arrival_game_turn);
                if let Some(ceiling) = chart.max_tonnage_for(d.year, d.month) {
                    // Replacement production, Axis planned-truck production, and supply convoys all
                    // draw from the arrival turn's allowance (§20.62).
                    if self.axis_shipping_committed(arrival_game_turn) + tonnage > ceiling {
                        return Err(CommandError::InsufficientConvoyCapacity);
                    }
                }
            }
        }

        // Accepted. Record the order, then schedule the points to arrive after the §20.21 delay.
        self.production_orders.push(ProductionOrder {
            nationality,
            item: item.to_string(),
            game_turn: gt,
            arrival_game_turn,
            count,
            tonnage,
        });
        let class = replacement_class_for_item(item).unwrap_or(crate::state::game::ReplacementClass::Gun);
        let location = self
            .reference
            .reinforcement_entry_hex(nationality)
            .map(str::to_string)
            .and_then(|s| self.resolve_entry(&s));
        let idx = self.plan_replacement(class, count, arrival_game_turn, self.operations_stage as u32, location, None);
        // §20.62: record the Axis convoy tonnage on the pending pool so an over-committed arrival
        // can be deferred against the turn's actual rolled allowance (§20.64).
        self.pending_replacements[idx].tonnage = tonnage;
        Ok(CommandOutcome::ProductionPlanned { arrival_game_turn, class, tonnage })
    }

    /// §20.78A ProduceCommonwealthTrucks: roll the Commonwealth Truck Production Table once, one d6
    /// per truck type; the produced Truck Points arrive at Alexandria (into the Commonwealth
    /// free-truck pool) beginning four Game-Turns later. Gated to the Organization Phase like the
    /// other production commands.
    ///
    /// §20.78A "no more than 25% of the total trucks produced may arrive… during the Game-Turn"
    /// (project-owner ruling 2026-08-30: read as *this roll's batch*): each type's produced count is
    /// divided as evenly as possible across four consecutive Game-Turns (GT+4…GT+7), so no more than
    /// ~25% lands per Game-Turn — matching §20.74's "divided… as evenly as possible" smoothing.
    fn cmd_produce_commonwealth_trucks(
        &mut self,
        light_die: u32,
        medium_die: u32,
        heavy_die: u32,
    ) -> Result<CommandOutcome, CommandError> {
        use crate::reference::logistics::TruckKind;
        use crate::reference::units::Side;
        self.require_phase(Phase::Organization)?;
        let table = self.reference.cw_truck_production.as_ref().ok_or(CommandError::NoTable)?;
        let gt = self.game_turn;
        let light = table.produce(TruckKind::Light, gt, light_die);
        let medium = table.produce(TruckKind::Medium, gt, medium_die);
        let heavy = table.produce(TruckKind::Heavy, gt, heavy_die);
        let first_arrival = gt + 4;
        // §20.78A: spread each type over four Game-Turns (≤25%/GT), remainders on the earliest
        // turns. §20.74: within each of those Game-Turns, divide the arriving Truck Points among the
        // three OpStages as evenly as possible (remainders on the earliest stages).
        const BUCKETS: u32 = 4;
        for (kind, total) in [(TruckKind::Light, light), (TruckKind::Medium, medium), (TruckKind::Heavy, heavy)] {
            for b in 0..BUCKETS {
                let turn_share = total / BUCKETS + u32::from(b < total % BUCKETS);
                for s in 1..=3u32 {
                    let stage_share = turn_share / 3 + u32::from(s - 1 < turn_share % 3);
                    self.schedule_truck_delivery(Side::Commonwealth, kind, stage_share, first_arrival + b, s);
                }
            }
        }
        Ok(CommandOutcome::TrucksProduced { arrival_game_turn: first_arrival, light, medium, heavy })
    }

    /// §20.78B ProduceCommonwealthInfantry: roll the Commonwealth Infantry Production Table once
    /// (2d6 sum, 2–12); the produced Infantry Replacement Points arrive four Game-Turns later and
    /// begin Training via the standard replacement lifecycle. Gated to the Organization Phase.
    fn cmd_produce_commonwealth_infantry(&mut self, roll: u32) -> Result<CommandOutcome, CommandError> {
        use crate::reference::units::Nationality;
        use crate::state::game::ReplacementClass;
        self.require_phase(Phase::Organization)?;
        let table = self.reference.cw_infantry_production.as_ref().ok_or(CommandError::NoTable)?;
        let gt = self.game_turn;
        let points = table.produce(gt, roll);
        let arrival_game_turn = gt + 4;
        if points > 0 {
            let location = self
                .reference
                .reinforcement_entry_hex(Nationality::Commonwealth)
                .map(str::to_string)
                .and_then(|s| self.resolve_entry(&s));
            // §20.74: divide the arriving Infantry Replacement Points among the arrival turn's three
            // OpStages as evenly as possible (remainders on the earliest stages).
            for s in 1..=3u32 {
                let stage_share = points / 3 + u32::from(s - 1 < points % 3);
                if stage_share > 0 {
                    self.plan_replacement(
                        ReplacementClass::Infantry, stage_share, arrival_game_turn, s, location.clone(), None,
                    );
                }
            }
        }
        Ok(CommandOutcome::InfantryProduced { arrival_game_turn, points })
    }

    /// §20.66a ProduceAxisTrucks: plan `count` Truck Points of `kind`, validated against the Axis
    /// Truck Production row's lifetime total and per-Game-Turn maximum. On success the trucks
    /// arrive two Game-Turns later (§20.21 Axis delay) into the Axis free-truck pool. Gated to the
    /// Organization Phase. (Bounded, D15: the row's convoy tonnage is not yet cross-counted against
    /// the §20.62 replacement-shipping ceiling — a documented simplification.)
    fn cmd_produce_axis_trucks(
        &mut self,
        kind: crate::reference::logistics::TruckKind,
        count: u32,
    ) -> Result<CommandOutcome, CommandError> {
        use crate::reference::units::Side;
        use crate::state::game::PendingTruckDelivery;
        self.require_phase(Phase::Organization)?;
        if count == 0 {
            return Err(CommandError::NoParticipants);
        }
        let gt = self.game_turn;
        let row = self
            .reference
            .axis_truck_production
            .as_ref()
            .ok_or(CommandError::NoTable)?
            .row(kind)
            .clone();
        // §20.66a lifetime total for this truck type.
        let lifetime: u32 = self
            .axis_truck_orders
            .iter()
            .filter(|o| o.side == Side::Axis && o.kind == kind)
            .map(|o| o.count)
            .sum();
        if lifetime + count > row.total {
            return Err(CommandError::ProductionExhausted);
        }
        // §20.66a per-Game-Turn maximum (orders are stamped with their plan Game-Turn here).
        let this_turn: u32 = self
            .axis_truck_orders
            .iter()
            .filter(|o| o.side == Side::Axis && o.kind == kind && o.game_turn == gt)
            .map(|o| o.count)
            .sum();
        if this_turn + count > row.max_per_turn(gt) {
            return Err(CommandError::ProductionRateExceeded);
        }
        let arrival_game_turn = gt + 2;
        // §20.62: Axis Truck Points ride the naval convoys too and share the arrival turn's Shipping
        // Tonnage allowance with replacement production and supplies. Reject if this order's tonnage,
        // added to everything already committed to that turn, exceeds the (die-6) allowance ceiling.
        let tonnage = row.tonnage * count;
        if tonnage > 0 {
            if let Some(chart) = self.reference.convoy_levels.as_ref() {
                let d = crate::state::game::date_of(arrival_game_turn);
                if let Some(ceiling) = chart.max_tonnage_for(d.year, d.month) {
                    if self.axis_shipping_committed(arrival_game_turn) + tonnage > ceiling {
                        return Err(CommandError::InsufficientConvoyCapacity);
                    }
                }
            }
        }
        // Record the order (stamped with the plan turn for the accounting above) and schedule the
        // trucks to arrive two Game-Turns later.
        self.axis_truck_orders.push(PendingTruckDelivery {
            side: Side::Axis, kind, count, game_turn: gt, stage: self.operations_stage as u32,
        });
        self.schedule_truck_delivery(Side::Axis, kind, count, arrival_game_turn, self.operations_stage as u32);
        let (mut light, mut medium, mut heavy) = (0, 0, 0);
        match kind {
            crate::reference::logistics::TruckKind::Light => light = count,
            crate::reference::logistics::TruckKind::Medium => medium = count,
            crate::reference::logistics::TruckKind::Heavy => heavy = count,
        }
        Ok(CommandOutcome::TrucksProduced { arrival_game_turn, light, medium, heavy })
    }

    /// §6.17/§53.11 AssignTrucks: move `light`/`medium`/`heavy` Truck Points from the unit's side's
    /// free-truck pool onto the unit as first-line trucks (Organization Phase). These carry the unit
    /// (motorization, §6.17) and its supply (§53.24). Reports the unit's resulting trucks and its
    /// §6.17 motorized CPA (if it is now leg infantry the trucks can fully carry).
    fn cmd_assign_trucks(
        &mut self,
        unit: UnitId,
        light: u32,
        medium: u32,
        heavy: u32,
    ) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::Organization)?;
        let side = self.unit(unit).ok_or(CommandError::UnitNotFound)?.nationality.side();
        let pool = self.truck_pools.get(&side).copied().unwrap_or_default();
        if pool.light < light || pool.medium < medium || pool.heavy < heavy {
            return Err(CommandError::InsufficientTrucks);
        }
        {
            let p = self.truck_pools.entry(side).or_default();
            p.light -= light;
            p.medium -= medium;
            p.heavy -= heavy;
        }
        let u = self.unit_mut(unit).ok_or(CommandError::UnitNotFound)?;
        u.trucks.light += light;
        u.trucks.medium += medium;
        u.trucks.heavy += heavy;
        let t = u.trucks;
        Ok(CommandOutcome::TrucksAssigned {
            light: t.light, medium: t.medium, heavy: t.heavy,
            motorized_cpa: self.unit_motorized_cpa(unit),
        })
    }

    /// §53.11 ReleaseTrucks: return `light`/`medium`/`heavy` first-line Truck Points from a unit to
    /// its side's free-truck pool (Organization Phase), so they can be reassigned. Rejected if the
    /// unit has fewer of a type assigned, or if any supply is still loaded on its trucks (§53.24 —
    /// unload it first, or the loaded supply would exceed the reduced cargo capacity).
    fn cmd_release_trucks(
        &mut self,
        unit: UnitId,
        light: u32,
        medium: u32,
        heavy: u32,
    ) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::Organization)?;
        let u = self.unit(unit).ok_or(CommandError::UnitNotFound)?;
        if u.trucks.light < light || u.trucks.medium < medium || u.trucks.heavy < heavy {
            return Err(CommandError::InsufficientTrucks);
        }
        let c = &u.truck_cargo;
        if c.fuel + c.ammo + c.stores + c.water > 0 {
            return Err(CommandError::TrucksLoadedCannotRelease);
        }
        let side = u.nationality.side();
        {
            let um = self.unit_mut(unit).ok_or(CommandError::UnitNotFound)?;
            um.trucks.light -= light;
            um.trucks.medium -= medium;
            um.trucks.heavy -= heavy;
        }
        {
            let p = self.truck_pools.entry(side).or_default();
            p.light += light;
            p.medium += medium;
            p.heavy += heavy;
        }
        let t = self.unit(unit).ok_or(CommandError::UnitNotFound)?.trucks;
        Ok(CommandOutcome::TrucksAssigned {
            light: t.light, medium: t.medium, heavy: t.heavy,
            motorized_cpa: self.unit_motorized_cpa(unit),
        })
    }

    /// §31 MoveRommel / RetreatRommel: move the Rommel counter one hex to adjacent `to` (Movement &
    /// Combat Phase, on the Axis turn). A normal move (`retreat=false`) spends his motorized
    /// medium-truck cost from his 60-CP stage allowance and may not enter an enemy ZOC unoccupied by
    /// a Friendly unit; a §31.2 retreat (`retreat=true`) passes through enemy ZOC "with impunity" and
    /// spends no allowance (it is a reaction before assault). Moving off his start-of-stage hex
    /// forfeits any §31.4 CPA designation.
    fn cmd_move_rommel(&mut self, to: HexCoord, retreat: bool) -> Result<CommandOutcome, CommandError> {
        use crate::reference::terrain::MoveCost;
        use crate::reference::units::Side;
        use crate::rules::movement::{step_cost, MoveMode};
        self.require_phase(Phase::MovementAndCombat)?;
        if self.active_player != Side::Axis {
            return Err(CommandError::WrongPhase);
        }
        let from = self.rommel_hex().ok_or(CommandError::RommelNotPresent)?;
        if !self.map.neighbors(from).any(|n| n == to) {
            return Err(CommandError::RommelCannotMove);
        }
        if !retreat {
            let friendly = self
                .units_in_hex(to)
                .into_iter()
                .any(|x| self.unit(x).is_some_and(|u| u.nationality.side() == Side::Axis));
            if self.hex_in_zoc_of(to, Side::Commonwealth) && !friendly {
                return Err(CommandError::RommelCannotMove);
            }
        }
        let cost = match step_cost(&self.reference, &self.map, from, to, MoveMode::Motorized) {
            Some(MoveCost::Cost(f)) => f.0.max(0) as u32,
            _ => return Err(CommandError::RommelCannotMove), // Prohibited / EntireCpa / off-map
        };
        let r = self.rommel.as_mut().ok_or(CommandError::RommelNotPresent)?;
        if !retreat {
            if cost > r.mcp_left {
                return Err(CommandError::RommelCannotMove);
            }
            r.mcp_left -= cost;
        }
        r.hex = Some(to);
        if r.hex != r.stage_start_hex {
            r.cpa_unit = None; // §31.4: he no longer "remains" with the designated unit
        }
        let mcp_left = r.mcp_left;
        Ok(CommandOutcome::RommelMoved { hex: to, mcp_left })
    }

    /// §31.4 DesignateRommelUnit: nominate the co-located `unit` for Rommel's +5 CPA this Operations
    /// Stage. Valid only while Rommel is at the hex he began the stage in and the unit shares it.
    fn cmd_designate_rommel_unit(&mut self, unit: UnitId) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::MovementAndCombat)?;
        let rhex = self.rommel_hex().ok_or(CommandError::RommelNotPresent)?;
        let at_start = self.rommel.as_ref().is_some_and(|r| r.hex == r.stage_start_hex);
        let co_located = self.unit(unit).ok_or(CommandError::UnitNotFound)?.hex() == Some(rhex);
        if !at_start || !co_located {
            return Err(CommandError::RommelCannotMove);
        }
        self.rommel.as_mut().unwrap().cpa_unit = Some(unit);
        Ok(CommandOutcome::RommelUnitDesignated)
    }

    /// §27.13 FormRaider: form a desert raider from a Friendly Recce/AC unit (Organization Phase).
    fn cmd_form_raider(
        &mut self,
        kind: crate::state::game::RaiderKind,
        from_unit: UnitId,
    ) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::Organization)?;
        self.form_raider(kind, from_unit)
            .map(|raider| CommandOutcome::RaiderFormed { raider })
            .ok_or(CommandError::RaiderNotAvailable)
    }

    /// §27.3 MoveRaider: move a raider one hex (Movement & Combat Phase, on its owner's turn). It
    /// ignores enemy ZOC (§27.34) but may not enter an Enemy-occupied hex except to raid (§27.42);
    /// exceeding its 50-CP allowance eliminates it (§27.37); entering an enemy-controlled hex risks a
    /// §27.43 spot (a `spot_roll` of 6 eliminates it).
    fn cmd_move_raider(
        &mut self,
        raider: usize,
        to: HexCoord,
        spot_roll: u32,
    ) -> Result<CommandOutcome, CommandError> {
        use crate::reference::terrain::MoveCost;
        use crate::rules::movement::{step_cost, MoveMode};
        self.require_phase(Phase::MovementAndCombat)?;
        let r = self.raiders.get(raider).ok_or(CommandError::RaiderNotAvailable)?;
        let from = r.hex.ok_or(CommandError::RaiderNotAvailable)?;
        let side = r.side;
        if self.active_player != side {
            return Err(CommandError::WrongPhase);
        }
        // §27.35: a raider may not travel without its two Fuel Points.
        if r.fuel < 2 {
            return Err(CommandError::RaiderCannotMove);
        }
        if !self.map.neighbors(from).any(|n| n == to) {
            return Err(CommandError::RaiderCannotMove);
        }
        let enemy = side.opponent();
        // §27.42: a normal move may not enter an Enemy-occupied hex (that would be a raid).
        let enemy_occupied = self
            .units_in_hex(to)
            .into_iter()
            .any(|x| self.unit(x).is_some_and(|u| u.nationality.side() == enemy));
        if enemy_occupied {
            return Err(CommandError::RaiderCannotMove);
        }
        let cost = match step_cost(&self.reference, &self.map, from, to, MoveMode::Motorized) {
            Some(MoveCost::Cost(f)) => f.0.max(0) as u32,
            _ => return Err(CommandError::RaiderCannotMove),
        };
        let controlled = self.hex_in_zoc_of(to, enemy);
        let r = self.raiders.get_mut(raider).unwrap();
        r.cpa_used += cost;
        r.hex = Some(to);
        let cpa_used = r.cpa_used;
        // §27.37: exceeding the 50-CP (500-tenths) allowance eliminates the raider.
        let over = cpa_used > 500;
        // §27.41/§27.43: entering an enemy-controlled hex risks being spotted.
        let spotted = controlled && spot_roll == 6;
        let eliminated = over || spotted;
        if eliminated {
            self.eliminate_raider(raider);
        }
        Ok(CommandOutcome::RaiderMoved { hex: to, eliminated, cpa_used })
    }

    /// §27.44 SpotRaider: an Enemy unit attempts to spot the raider in its hex (§27.43 — a `roll` of
    /// 6 eliminates it). The 1-CP cost and once-per-stage limit (§27.44) are not tracked here.
    fn cmd_spot_raider(&mut self, raider: usize, roll: u32) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::MovementAndCombat)?;
        let present = self.raiders.get(raider).is_some_and(|r| r.hex.is_some());
        if !present {
            return Err(CommandError::RaiderNotAvailable);
        }
        let eliminated = roll == 6;
        if eliminated {
            self.eliminate_raider(raider);
        }
        Ok(CommandOutcome::RaiderSpotChecked { eliminated })
    }

    /// §27.35 ResupplyRaider: the raider draws its weekly supply where it stands — 2 Fuel + 1 Ammo
    /// from a co-located Friendly supply dump (if it holds them), and Water from an oasis/bir in the
    /// hex (setting the Game-Turn's Water requirement met). Movement & Combat or Organization Phase.
    fn cmd_resupply_raider(&mut self, raider: usize) -> Result<CommandOutcome, CommandError> {
        use crate::state::game::Commodity;
        let r = self.raiders.get(raider).ok_or(CommandError::RaiderNotAvailable)?;
        let hex = r.hex.ok_or(CommandError::RaiderNotAvailable)?;
        let side = r.side;
        // §27.35: 2 Fuel + 1 Ammo come from a co-located Friendly supply dump that holds them.
        let dump = self
            .supply_dumps
            .iter_mut()
            .find(|d| d.side == side && !d.dummy && d.location.hex() == Some(hex) && d.supply.fuel >= 2 && d.supply.ammo >= 1);
        let (mut fuel, mut ammo) = (0, 0);
        if let Some(d) = dump {
            *d.supply.get_mut(Commodity::Fuel) -= 2;
            *d.supply.get_mut(Commodity::Ammo) -= 1;
            fuel = 2;
            ammo = 1;
        }
        // §27.35: Water from any oasis or bir in the hex.
        let water = self.hex_water_source_for(hex, side).is_some();
        let rr = self.raiders.get_mut(raider).unwrap();
        rr.fuel += fuel;
        rr.ammo += ammo;
        if water {
            rr.drew_water_this_gt = true;
        }
        Ok(CommandOutcome::RaiderResupplied { fuel, ammo, water })
    }

    /// §27.6 RaidRommel: a Commonwealth LRDG in Rommel's hex attempts to assassinate/capture him
    /// (Movement & Combat Phase). Costs 10 CP (§27.63); at most four per campaign (§27.64). `roll`
    /// (2d6) resolves on the §27.64 table: 2 = "AI temp 3" (Axis rating 3 for two Game-Turns); 3/9/11
    /// = "LRDG elim" (raider eliminated unless no combat/HQ units share Rommel's hex); 12 = "AI perm
    /// 3" (Rommel captured — counter removed); 4-8/10 = no effect. Over-spending the 50-CP allowance
    /// also eliminates the raider (§27.37).
    fn cmd_raid_rommel(&mut self, raider: usize, roll: u32) -> Result<CommandOutcome, CommandError> {
        use crate::reference::units::Side;
        self.require_phase(Phase::MovementAndCombat)?;
        // §27.61: only a Commonwealth LRDG, in play, on the Commonwealth turn.
        let r = self.raiders.get(raider).ok_or(CommandError::RaiderNotAvailable)?;
        let rhex = r.hex.ok_or(CommandError::RaiderNotAvailable)?;
        if r.kind != crate::state::game::RaiderKind::Lrdg || self.active_player != Side::Commonwealth {
            return Err(CommandError::RaiderCannotMove);
        }
        // §27.62: the LRDG must be in the hex the Rommel counter occupies.
        if self.rommel_hex() != Some(rhex) {
            return Err(CommandError::RaiderCannotMove);
        }
        // §27.64: at most four Raids on Rommel per campaign.
        if self.raids_on_rommel >= 4 {
            return Err(CommandError::RaiderCannotMove);
        }
        self.raids_on_rommel += 1;
        // §27.63: the raid costs 10 CP; over-extending the 50-CP allowance eliminates the raider.
        let over = {
            let rr = self.raiders.get_mut(raider).unwrap();
            rr.cpa_used += 100;
            rr.revealed = true; // §27.33: the LRDG reveals itself
            rr.cpa_used > 500
        };
        // §27.64 table result.
        let mut raider_eliminated = over;
        let mut rommel_removed = false;
        let mut axis_temp3 = false;
        match roll {
            2 => {
                axis_temp3 = true;
                self.rommel_raid_ai3_until_gt = self.game_turn + 2;
            }
            3 | 9 | 11 => {
                // "LRDG elim": eliminated unless there are no combat/HQ units in Rommel's hex.
                if !self.units_in_hex(rhex).is_empty() {
                    raider_eliminated = true;
                }
            }
            12 => {
                rommel_removed = true;
                self.rommel = None; // captured — remove the counter from the game-maps
            }
            _ => {} // 4-8, 10: no effect
        }
        if raider_eliminated {
            self.eliminate_raider(raider);
        }
        Ok(CommandOutcome::RommelRaided { raider_eliminated, rommel_removed, axis_temp3 })
    }

    /// Common raid preamble (§27.5): the raider must be in play on its owner's turn; returns its hex
    /// and its own side. Raids occur in the Movement & Combat Phase.
    fn raider_raid_context(&self, raider: usize) -> Result<(HexCoord, crate::reference::units::Side), CommandError> {
        let r = self.raiders.get(raider).ok_or(CommandError::RaiderNotAvailable)?;
        let hex = r.hex.ok_or(CommandError::RaiderNotAvailable)?;
        if self.active_player != r.side {
            return Err(CommandError::WrongPhase);
        }
        Ok((hex, r.side))
    }

    /// §27.5x / §27.37: charge a raid's 5 Capability Points; eliminate the raider if that pushes it
    /// over its 50-CP allowance. Returns whether it was eliminated.
    fn raider_pay_raid_cp(&mut self, raider: usize) -> bool {
        let over = {
            let r = &mut self.raiders[raider];
            r.cpa_used += 50;
            r.revealed = true; // §27.33: a raider reveals itself when it raids
            r.cpa_used > 500
        };
        if over {
            self.eliminate_raider(raider);
        }
        over
    }

    /// §27.52 RaidPipeline: blow the water pipeline in the raider's hex (a d6 1-4).
    fn cmd_raid_pipeline(&mut self, raider: usize, roll: u32) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::MovementAndCombat)?;
        let (hex, side) = self.raider_raid_context(raider)?;
        if !self.pipelines.contains(&hex) || !self.enemy_units_in_hex(hex, side).is_empty() {
            return Err(CommandError::RaiderCannotMove);
        }
        let eliminated = self.raider_pay_raid_cp(raider);
        let success = (1..=4).contains(&roll);
        if success {
            self.pipelines.remove(&hex);
        }
        Ok(CommandOutcome::RaidResolved { success, raider_eliminated: eliminated })
    }

    /// §27.53 RaidAirfield: reduce the airfield in the raider's hex one capacity level (a d6 1-2).
    fn cmd_raid_airfield(&mut self, raider: usize, roll: u32) -> Result<CommandOutcome, CommandError> {
        use crate::state::air::AirFacilityKind;
        self.require_phase(Phase::MovementAndCombat)?;
        let (hex, side) = self.raider_raid_context(raider)?;
        // §27.53: an on-map Airfield (not a landing strip), not in a major city, no enemy combat units.
        let has_field = self
            .air_facilities
            .iter()
            .any(|f| f.hex == hex && f.kind == AirFacilityKind::Airfield && !f.off_map);
        if !has_field || self.is_major_city(hex) || !self.enemy_units_in_hex(hex, side).is_empty() {
            return Err(CommandError::RaiderCannotMove);
        }
        let eliminated = self.raider_pay_raid_cp(raider);
        let success = (1..=2).contains(&roll);
        if success {
            if let Some(f) = self
                .air_facilities
                .iter_mut()
                .find(|f| f.hex == hex && f.kind == AirFacilityKind::Airfield && !f.off_map)
            {
                f.capacity_level = f.capacity_level.saturating_sub(1);
            }
        }
        Ok(CommandOutcome::RaidResolved { success, raider_eliminated: eliminated })
    }

    /// §27.54 RaidPlanes: destroy grounded enemy planes in the raider's hex (a d6 1-2 → 10% of each
    /// enemy squadron; a 6 eliminates the raider).
    fn cmd_raid_planes(&mut self, raider: usize, roll: u32) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::MovementAndCombat)?;
        let (hex, side) = self.raider_raid_context(raider)?;
        let enemy = side.opponent();
        if self.is_major_city(hex) || !self.enemy_units_in_hex(hex, side).is_empty() {
            return Err(CommandError::RaiderCannotMove);
        }
        let targets: Vec<_> = self
            .sgsus
            .iter()
            .filter(|(_, s)| s.location == Some(hex) && s.nationality.side() == enemy && s.plane_count() > 0)
            .map(|(id, _)| *id)
            .collect();
        if targets.is_empty() {
            return Err(CommandError::RaiderCannotMove);
        }
        let mut eliminated = self.raider_pay_raid_cp(raider);
        let success = (1..=2).contains(&roll);
        if success {
            for id in targets {
                if let Some(s) = self.sgsus.get_mut(&id) {
                    let ten_pct = s.plane_count().div_ceil(10); // §27.54 "10% (at least)"
                    s.lose_planes(ten_pct);
                }
            }
        }
        // §27.54: a die roll of 6 eliminates the raider.
        if roll == 6 && !eliminated {
            self.eliminate_raider(raider);
            eliminated = true;
        }
        Ok(CommandOutcome::RaidResolved { success, raider_eliminated: eliminated })
    }

    /// §27.55 RaidSupplyDump: attack the enemy Supply Dump in the raider's hex (guarded or not).
    fn cmd_raid_supply_dump(
        &mut self,
        raider: usize,
        attack_roll: u32,
        destroy_roll: u32,
    ) -> Result<CommandOutcome, CommandError> {
        use crate::state::game::Commodity;
        self.require_phase(Phase::MovementAndCombat)?;
        let (hex, side) = self.raider_raid_context(raider)?;
        let enemy = side.opponent();
        if self.is_major_city(hex) {
            return Err(CommandError::RaiderCannotMove);
        }
        let dump_idx = self
            .supply_dumps
            .iter()
            .position(|d| d.side == enemy && d.location.hex() == Some(hex))
            .ok_or(CommandError::RaiderCannotMove)?;
        let mut eliminated = self.raider_pay_raid_cp(raider);
        // §27.55b: a guarded dump requires 2d6 > the guards' Raw Defensive Assault Strength.
        let guards = self.enemy_units_in_hex(hex, side);
        let guarded = !guards.is_empty();
        if guarded {
            let def = self.close_assault_raw(&guards, false).max(0);
            if (attack_roll as i32) <= def {
                if !eliminated {
                    self.eliminate_raider(raider);
                }
                return Ok(CommandOutcome::RaidResolved { success: false, raider_eliminated: true });
            }
        }
        // §27.56: a dummy dump is revealed (removed) — the raid is off for destruction.
        let dummy = self.supply_dumps[dump_idx].dummy;
        let hit = (1..=2).contains(&destroy_roll);
        if dummy {
            self.supply_dumps.remove(dump_idx);
        } else if hit {
            // §27.55a: destroy 10% of every commodity in the dump.
            let d = &mut self.supply_dumps[dump_idx];
            for c in [Commodity::Fuel, Commodity::Ammo, Commodity::Stores, Commodity::Water] {
                let have = d.supply.get(c);
                *d.supply.get_mut(c) = have - have.div_ceil(10);
            }
        }
        // §27.55b: at a guarded dump, a 6 on the destroy roll eliminates the raider.
        if guarded && destroy_roll == 6 && !eliminated {
            self.eliminate_raider(raider);
            eliminated = true;
        }
        Ok(CommandOutcome::RaidResolved { success: hit && !dummy, raider_eliminated: eliminated })
    }

    /// §27.57-58 RaidTruckConvoy: destroy a Truck Point (+cargo) of the enemy free convoy at index
    /// `convoy` in the raider's hex (a d6 1-2; replacement cargo adds 1 and a 6 eliminates the raider).
    fn cmd_raid_truck_convoy(&mut self, raider: usize, convoy: usize, roll: u32) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::MovementAndCombat)?;
        let (hex, side) = self.raider_raid_context(raider)?;
        let c = self.truck_convoys.get(convoy).ok_or(CommandError::RaiderCannotMove)?;
        if c.hex != hex || c.side != side.opponent() {
            return Err(CommandError::RaiderCannotMove);
        }
        self.resolve_truck_convoy_raid(raider, convoy, roll)
    }

    /// §27.59 InterceptTruckConvoy: during the enemy's Truck Convoy Phase, a raider within four
    /// Movement CPs of a passing convoy may halt and attack it (§27.58 mechanics). Distance is
    /// approximated by hex range ≤ 4 (a documented simplification of "within 4 Movement CPs").
    fn cmd_intercept_truck_convoy(&mut self, raider: usize, convoy: usize, roll: u32) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::TruckConvoy)?;
        let r = self.raiders.get(raider).ok_or(CommandError::RaiderNotAvailable)?;
        let rhex = r.hex.ok_or(CommandError::RaiderNotAvailable)?;
        let rside = r.side;
        let c = self.truck_convoys.get(convoy).ok_or(CommandError::RaiderCannotMove)?;
        // The convoy must be the enemy's and within interception range of the raider.
        if c.side != rside.opponent() || rhex.distance(c.hex) > 4 {
            return Err(CommandError::RaiderCannotMove);
        }
        self.resolve_truck_convoy_raid(raider, convoy, roll)
    }

    /// Shared §27.58 truck-convoy raid resolution (used by both the direct raid and the §27.59
    /// interception): 5 CP; a d6 1-2 destroys one Truck Point + its cargo share; replacement cargo
    /// adds 1 to the roll and a resulting 6 eliminates the raider.
    fn resolve_truck_convoy_raid(&mut self, raider: usize, convoy: usize, roll: u32) -> Result<CommandOutcome, CommandError> {
        use crate::state::game::Commodity;
        let carries_repl = self.truck_convoys[convoy].carries_replacements;
        let mut eliminated = self.raider_pay_raid_cp(raider);
        let eff = roll + u32::from(carries_repl);
        let success = (1..=2).contains(&eff);
        if success {
            let c = &mut self.truck_convoys[convoy];
            let total = c.trucks.light + c.trucks.medium + c.trucks.heavy;
            if c.trucks.heavy > 0 {
                c.trucks.heavy -= 1;
            } else if c.trucks.medium > 0 {
                c.trucks.medium -= 1;
            } else if c.trucks.light > 0 {
                c.trucks.light -= 1;
            }
            for cm in [Commodity::Fuel, Commodity::Ammo, Commodity::Stores, Commodity::Water] {
                let have = c.cargo.get(cm);
                *c.cargo.get_mut(cm) = have - have.checked_div(total).unwrap_or(0);
            }
        }
        if carries_repl && eff >= 6 && !eliminated {
            self.eliminate_raider(raider);
            eliminated = true;
        }
        Ok(CommandOutcome::RaidResolved { success, raider_eliminated: eliminated })
    }

    /// §8.97/§8.98 FormTruckConvoy: detach first-line Truck Points (and their loaded cargo) from a
    /// unit into a new free truck convoy in the unit's hex (Organization Phase).
    fn cmd_form_truck_convoy(
        &mut self,
        from_unit: UnitId,
        light: u32,
        medium: u32,
        heavy: u32,
    ) -> Result<CommandOutcome, CommandError> {
        use crate::state::game::TruckConvoy;
        self.require_phase(Phase::Organization)?;
        let u = self.unit(from_unit).ok_or(CommandError::UnitNotFound)?;
        let hex = u.hex().ok_or(CommandError::UnitNotFound)?;
        let side = u.nationality.side();
        if u.trucks.light < light || u.trucks.medium < medium || u.trucks.heavy < heavy {
            return Err(CommandError::InsufficientTrucks);
        }
        if light + medium + heavy == 0 {
            return Err(CommandError::InsufficientTrucks);
        }
        // Detach the trucks and carry over the unit's loaded truck cargo into the convoy.
        let cargo = {
            let um = self.unit_mut(from_unit).unwrap();
            um.trucks.light -= light;
            um.trucks.medium -= medium;
            um.trucks.heavy -= heavy;
            std::mem::take(&mut um.truck_cargo)
        };
        self.truck_convoys.push(TruckConvoy {
            side,
            hex,
            trucks: crate::state::unit::TruckPoints { light, medium, heavy },
            cargo,
            cpa_used: 0,
            carries_replacements: false,
        });
        Ok(CommandOutcome::TruckConvoyFormed { convoy: self.truck_convoys.len() - 1 })
    }

    /// §8.98/§10.29 MoveTruckConvoy: move a free truck convoy one hex (Truck Convoy Phase, on its
    /// owner's turn), spending its supply CPA and observing the §10.29 ZOC restriction.
    fn cmd_move_truck_convoy(&mut self, convoy: usize, to: HexCoord) -> Result<CommandOutcome, CommandError> {
        use crate::reference::terrain::MoveCost;
        use crate::rules::movement::{step_cost, MoveMode};
        self.require_phase(Phase::TruckConvoy)?;
        let c = self.truck_convoys.get(convoy).ok_or(CommandError::TruckConvoyCannotMove)?;
        let from = c.hex;
        let side = c.side;
        if self.active_player != side {
            return Err(CommandError::WrongPhase);
        }
        if !self.map.neighbors(from).any(|n| n == to) {
            return Err(CommandError::TruckConvoyCannotMove);
        }
        // §10.29: a truck convoy may not enter an enemy ZOC unless a Friendly unit already holds it.
        let enemy = side.opponent();
        let friendly = self
            .units_in_hex(to)
            .into_iter()
            .any(|x| self.unit(x).is_some_and(|u| u.nationality.side() == side));
        if self.hex_in_zoc_of(to, enemy) && !friendly {
            return Err(CommandError::TruckConvoyCannotMove);
        }
        let cost = match step_cost(&self.reference, &self.map, from, to, MoveMode::Motorized) {
            Some(MoveCost::Cost(f)) => f.0.max(0) as u32,
            _ => return Err(CommandError::TruckConvoyCannotMove),
        };
        let cpa = self.truck_convoy_cpa(&self.truck_convoys[convoy]);
        let c = self.truck_convoys.get_mut(convoy).unwrap();
        if c.cpa_used + cost > cpa {
            return Err(CommandError::TruckConvoyCannotMove); // §8.98: may not exceed CPA
        }
        c.cpa_used += cost;
        c.hex = to;
        Ok(CommandOutcome::TruckConvoyMoved { hex: to, cpa_used: c.cpa_used })
    }

    /// §27.72/§27.73 LandLayforce: the Layforce commando, having spent a full Game-Turn in
    /// Alexandria, lands in a coastal hex (Movement & Combat Phase). The landing costs 5 CP per 50
    /// hexes the ships moved plus the hex's terrain entry cost — capped at the unit's CPA — and opens
    /// the §27.74 three-Operations-Stage free-supply window.
    fn cmd_land_layforce(&mut self, unit: UnitId, to: HexCoord, ship_hexes: u32) -> Result<CommandOutcome, CommandError> {
        use crate::reference::terrain::MoveCost;
        use crate::rules::movement::{hex_entry_cost, MoveMode};
        use crate::state::unit::UnitLocation;
        self.require_phase(Phase::MovementAndCombat)?;
        if !self.is_layforce(unit) {
            return Err(CommandError::LayforceCannotLand);
        }
        // §27.73: must be in Alexandria a full Game-Turn (entered on an earlier Game-Turn).
        let ready = self
            .layforce_alexandria_since
            .is_some_and(|since| self.game_turn > since);
        let at_alex = self.unit(unit).and_then(|u| u.hex()) == self.alexandria_hex();
        if !ready || !at_alex {
            return Err(CommandError::LayforceCannotLand);
        }
        // §27.72: the destination must be a coastal hex on the map.
        if !self.map.is_coastal(to) {
            return Err(CommandError::LayforceCannotLand);
        }
        // §27.73 cost: 5 CP per 50 ship-hexes (tenths = 50 each) + the hex's terrain entry cost.
        let ship_cp = ship_hexes.div_ceil(50) * 50;
        let terrain = match hex_entry_cost(&self.reference, &self.map, to, MoveMode::NonMotorized) {
            Some(MoveCost::Cost(f)) => f.0.max(0) as u32,
            _ => return Err(CommandError::LayforceCannotLand), // prohibited / off-map
        };
        let cap = self.effective_cpa(unit).unwrap_or(0).max(0) as u32 * 10;
        if ship_cp + terrain > cap {
            return Err(CommandError::LayforceCannotLand); // §6: may not exceed its CPA
        }
        // Land it, and open the §27.74 free-supply window (this stage plus the next two).
        if let Some(u) = self.unit_mut(unit) {
            u.location = Some(UnitLocation::OnMap(to));
        }
        let idx = self.game_turn * 3 + (self.operations_stage as u32 - 1) + 2;
        self.layforce_free_supply_until = Some((idx / 3, (idx % 3) as u8 + 1));
        self.layforce_alexandria_since = None; // no longer in Alexandria
        Ok(CommandOutcome::LayforceLanded { hex: to })
    }

    /// §27.77 EmbarkLayforce: re-embark the Layforce commando from its coastal hex; the ship returns
    /// it to Alexandria (Movement & Combat Phase).
    fn cmd_embark_layforce(&mut self, unit: UnitId) -> Result<CommandOutcome, CommandError> {
        use crate::state::unit::UnitLocation;
        self.require_phase(Phase::MovementAndCombat)?;
        if !self.is_layforce(unit) {
            return Err(CommandError::LayforceCannotLand);
        }
        let alex = self.alexandria_hex().ok_or(CommandError::LayforceCannotLand)?;
        let on_coast = self.unit(unit).and_then(|u| u.hex()).is_some_and(|h| self.map.is_coastal(h));
        if !on_coast {
            return Err(CommandError::LayforceCannotLand);
        }
        if let Some(u) = self.unit_mut(unit) {
            u.location = Some(UnitLocation::OnMap(alex)); // the ship carries it back to Alexandria
        }
        self.layforce_free_supply_until = None;
        Ok(CommandOutcome::LayforceEmbarked)
    }

    /// §27.83/§27.84 LandSas: land the SAS Brigade by sea in a coastal hex (Movement & Combat Phase),
    /// costing 5 CP per 50 ship-hexes plus the hex's terrain entry cost (capped at its CPA) and
    /// opening the §27.84 three-Operations-Stage supply window.
    fn cmd_land_sas(&mut self, unit: UnitId, to: HexCoord, ship_hexes: u32) -> Result<CommandOutcome, CommandError> {
        use crate::reference::terrain::MoveCost;
        use crate::rules::movement::{hex_entry_cost, MoveMode};
        use crate::state::unit::UnitLocation;
        self.require_phase(Phase::MovementAndCombat)?;
        if !self.is_sas(unit) || !self.map.is_coastal(to) {
            return Err(CommandError::SasCannotAct);
        }
        let ship_cp = ship_hexes.div_ceil(50) * 50;
        let terrain = match hex_entry_cost(&self.reference, &self.map, to, MoveMode::NonMotorized) {
            Some(MoveCost::Cost(f)) => f.0.max(0) as u32,
            _ => return Err(CommandError::SasCannotAct),
        };
        let cap = self.effective_cpa(unit).unwrap_or(0).max(0) as u32 * 10;
        if ship_cp + terrain > cap {
            return Err(CommandError::SasCannotAct);
        }
        if let Some(u) = self.unit_mut(unit) {
            u.location = Some(UnitLocation::OnMap(to));
        }
        let idx = self.game_turn * 3 + (self.operations_stage as u32 - 1) + 2;
        self.sas_free_supply_until = Some((idx / 3, (idx % 3) as u8 + 1));
        Ok(CommandOutcome::SasLanded { hex: to })
    }

    /// §27.85/§27.86 RaidAirfieldSas: the SAS Brigade destroys grounded Axis planes at the air
    /// facility in its hex (no Axis combat units present), removing the §27.93-table percentage from
    /// each enemy squadron. Subject to the §27.87 frequency limit (≥6 Operations Stages between raids).
    fn cmd_raid_airfield_sas(&mut self, unit: UnitId, roll: u32) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::MovementAndCombat)?;
        // §27.93 SAS Raid Table: % of grounded planes destroyed by a d6 (1-6). Verified against the
        // physical chart (owner-confirmed 2026-08-30): die 6 = 50%.
        const SAS_RAID_PCT: [u32; 6] = [0, 10, 25, 25, 33, 50];
        if !self.is_sas(unit) {
            return Err(CommandError::SasCannotAct);
        }
        let side = self.unit(unit).map(|u| u.nationality.side()).ok_or(CommandError::UnitNotFound)?;
        if self.active_player != side {
            return Err(CommandError::WrongPhase);
        }
        let hex = self.unit(unit).and_then(|u| u.hex()).ok_or(CommandError::SasCannotAct)?;
        let enemy = side.opponent();
        // §27.85: an Axis air facility in the hex, no Axis combat units present.
        let has_facility = self.air_facilities.iter().any(|f| f.hex == hex);
        if !has_facility || !self.enemy_units_in_hex(hex, side).is_empty() {
            return Err(CommandError::SasCannotAct);
        }
        // §27.87: at least six Operations Stages since the last SAS raid.
        let now_idx = self.game_turn * 3 + (self.operations_stage as u32 - 1);
        if let Some((gt, st)) = self.sas_last_raid {
            let last_idx = gt * 3 + (st as u32 - 1);
            if now_idx < last_idx + 6 {
                return Err(CommandError::SasCannotAct);
            }
        }
        let pct = SAS_RAID_PCT[(roll.clamp(1, 6) - 1) as usize];
        let targets: Vec<_> = self
            .sgsus
            .iter()
            .filter(|(_, s)| s.location == Some(hex) && s.nationality.side() == enemy && s.plane_count() > 0)
            .map(|(id, _)| *id)
            .collect();
        if targets.is_empty() {
            return Err(CommandError::SasCannotAct);
        }
        for id in targets {
            if let Some(s) = self.sgsus.get_mut(&id) {
                let lost = (s.plane_count() * pct).div_ceil(100);
                s.lose_planes(lost);
            }
        }
        self.sas_last_raid = Some((self.game_turn, self.operations_stage));
        Ok(CommandOutcome::SasRaided { planes_pct: pct })
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
        mut planes: u32,
        bombload_per_plane: u32,
        night: bool,
        escort: Option<crate::state::air::SquadronId>,
        abort_planes: u32,
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
        let enemy = nationality.side().opponent();
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
        // §39.3 voluntary abort: before any combat, some or all of the bombers may turn back if a
        // sufficient fighter escort screens them (§39.36/§39.37) or their Maneuver qualifies
        // (§39.35). The decision is made against the covering offensive CAP, which is neither engaged
        // nor consumed. §39.34: the player need not abort the whole mission — `abort_planes` may be a
        // subset, in which case the rest press on and resolve normally below.
        let aborting = abort_planes.min(planes);
        if aborting > 0 {
            let cap = self.cap_patrols.iter().find(|p| {
                p.side == enemy
                    && matches!(p.cap_type, CapType::Offensive)
                    && p.intercepts_used < crate::rules::air::OFFENSIVE_CAP_INTERCEPTS_PER_STAGE
                    && p.covers(target)
            });
            let cap_count = cap.map_or(0, |p| p.flight.count);
            let cap_maneuver = cap.map(|p| p.flight.maneuver);
            let escort_count = escort_flight.map_or(0, |f| f.count);
            let sufficient = crate::rules::air::sufficient_fighter_screen(escort_count, cap_count);
            let bomber_maneuver = self
                .sgsus
                .get(&squadron)
                .and_then(|s| s.planes.first())
                .and_then(|g| self.reference.aircraft_charts.get(&nationality).and_then(|c| c.get(&g.aircraft_type)))
                .and_then(|a| a.loadouts.first())
                .map_or(0, |l| l.maneuver);
            if !crate::rules::air::nonfighter_may_abort(sufficient, bomber_maneuver, cap_maneuver) {
                return Err(CommandError::AbortNotPermitted);
            }
            // §39.31/§39.33: the aborting planes fly and dump their bombload, but do not bomb or fight.
            let flown = self.sgsus.get_mut(&squadron).ok_or(CommandError::NoSuchSquadron)?.fly(aborting);
            if aborting >= planes {
                return Ok(CommandOutcome::MissionAborted { planes: flown });
            }
            // §39.34 partial abort: the remainder continues the mission (resolved below).
            planes -= aborting;
        }
        // §40.26/§40.27: an enemy offensive CAP whose air ZOC covers the target intercepts (consuming
        // its one interception this stage); else a fighter base may scramble (§40.3).
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
        if interceptors.is_none() {
            if let Some((sq, fl)) = self.scramble_interception(target, nationality.side(), night) {
                interceptor_sq = Some(sq);
                interceptors = Some(fl);
            }
        }
        // §46: flak fired by the enemy AA units in the target hex.
        let flak_points = self.hex_flak_points(target, nationality.side());
        // §40.15/§45.24: each intercepting/escorting plane fights its own match-up with its own
        // pilot — pass the per-plane rosters (best-first) so the duels are not all crewed by the
        // squadron's single best pilot.
        let interceptor_pilots = interceptor_sq.map(|sq| self.squadron_pilot_ratings(sq)).unwrap_or_default();
        let escort_pilots = escort_sq.map(|sq| self.squadron_pilot_ratings(sq)).unwrap_or_default();
        let outcome = self
            .resolve_bombing_mission(
                squadron, planes, bombload_per_plane, group, interceptors, escort_flight,
                flak_points, flak_destroy_roll, flak_abort_roll, bomb_roll, night,
                &interceptor_pilots, &escort_pilots,
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
        // §42.53: on a successful reveal the Axis discloses only the convoy's *size class* (chosen
        // adversarially among the overlapping ranges), never the exact tonnage.
        let size = if revealed {
            self.convoys
                .iter()
                .find(|c| c.lane == lane && !c.cancelled && c.arrival_gt == self.game_turn)
                .and_then(|c| crate::rules::naval::concealing_size(c.total_tonnage()))
        } else {
            None
        };
        Ok(CommandOutcome::ConvoyRecon { revealed, size })
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
    /// the engine resolves against an enemy unit of that class there. There is no
    /// line-of-fire check by rule (§12.35: no LOS restrictions in the desert).
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
        // §16.21: a barraged hex may not be reconnoitred this stage.
        self.barraged_hexes_this_stage.insert(target);
        Ok(CommandOutcome::Barrage(self.resolve_barrage(attackers, target_unit)))
    }

    /// Anti-Armor fire (§14) at a target hex. Fire is adjacent-only (enforced here) and the §14.34
    /// prohibited-hexside blocking is applied in `resolve_anti_armor` (there is no ranged LOS rule).
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
        // §16.21: engaging in Anti-Armor combat bars this side from Patrolling this stage.
        let atk_side = attackers.first().and_then(|&a| self.unit(a)).map(|u| u.nationality.side());
        if let Some(s) = atk_side {
            self.land_combat_this_stage.insert(s);
        }
        let outcome = self.resolve_anti_armor(attackers, target_hex);
        // §16.46: anti-armor fire eliminates a Dummy Tank once the defender has no real armor left.
        if let Some(s) = atk_side {
            let defender = s.opponent();
            if !self.hex_has_real_armor(target_hex, defender) {
                self.eliminate_dummies_in_hex(target_hex, defender);
            }
        }
        Ok(CommandOutcome::AntiArmor(outcome))
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
        // none). A shortfall forbids the assault (§50.12).
        if !self.consume_combat_ammo(attackers, crate::state::game::CombatAction::CloseAssault) {
            return Err(CommandError::InsufficientAmmo);
        }
        let outcome = self.resolve_close_assault(attackers, &defenders);
        // §50.14: the defenders fire defensively too — their melee weapons units spend defensive
        // close-assault Ammo (best-effort; the defender is already committed). (The broader §50.12
        // no-ammo → surrender / no-ZOC effect is a ZOC-wide rule left for the ZOC layer.)
        self.expend_defensive_assault_ammo(&defenders);
        // §16.21: engaging in Close Assault bars this side from Patrolling this stage.
        self.land_combat_this_stage.insert(side);
        // §16.46: a formation with a Dummy Tank that undergoes Close Assault loses the dummy.
        self.eliminate_dummies_in_hex(target, side.opponent());
        Ok(CommandOutcome::CloseAssault(outcome))
    }

    /// Satisfy a mandated Retreat (§15.82). The path is a sequence of adjacent hexes, each
    /// leading strictly away from the assaulted hex, on-map, enterable, not enemy-occupied,
    /// and not into an enemy ZOC (§10.23). The destination Stacking Limit is enforced (§9.35).
    /// Each mandated hex not retreated costs 10% loss; an empty path means the unit stands and
    /// absorbs the full penalty. §9.35 stack-splitting is supported implicitly — retreat is issued
    /// per unit, so a stack is split into different hexes by giving each unit its own path. (The
    /// finer §15.82 "least resistance"/toward-supply direction *preference* among legal away-paths
    /// is left to the caller; only the away-from-origin constraint is enforced.)
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
        let out = self.advance_off_map(unit, direction)
            .map(CommandOutcome::OffMapMoved)
            .map_err(CommandError::OffMap)?;
        // §8.62/§8.67: a unit leaving the map can no longer hold any enemy in Contact/Engaged.
        self.refresh_contact_status();
        Ok(out)
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

    /// §13 Retreat Before Assault: a non-phasing, unpinned unit voluntarily withdraws along `path`
    /// (resolved after barrages, before the assault sub-segments). It is ordinary voluntary
    /// movement — CP + Fuel, vehicle breakdown, the §8.6 break-off cost on leaving an enemy ZOC, and
    /// the §8.14/§10.24 ZOC rules — but a unit that did *not* begin the step adjacent to an enemy
    /// combat unit may spend at most 4 CP (or move one hex, §13.24). Not to be confused with
    /// Reaction (§8.5). §13.1: it may not be Pinned, and a Cohesion Level of −26 or worse bars it.
    fn cmd_retreat_before_assault(&mut self, unit: UnitId, path: &[HexCoord]) -> Result<CommandOutcome, CommandError> {
        if self.phase != Phase::MovementAndCombat {
            return Err(CommandError::WrongPhase);
        }
        if path.is_empty() {
            return Err(CommandError::NotAdjacent);
        }
        let (side, start, pinned, cohesion, engaged) = {
            let u = self.unit(unit).ok_or(CommandError::UnitNotFound)?;
            let start = u.hex().ok_or(CommandError::UnitNotOnMap)?;
            (u.nationality.side(), start, u.status.pinned, u.cohesion, u.status.engaged)
        };
        let enemy = side.opponent();
        // §13.1: only a non-Phasing, non-Pinned unit whose Cohesion is better than −26 may retreat.
        if side == self.active_player || pinned || cohesion <= -26 {
            return Err(CommandError::CannotRetreatBeforeAssault);
        }
        let mode = if self.unit_is_motorized(unit) { MoveMode::Motorized } else { MoveMode::NonMotorized };
        // §13.23/§13.24: unlimited CP if adjacent to an enemy combat unit at the start; else ≤4 CP.
        let adjacent_at_start = start
            .neighbors()
            .into_iter()
            .any(|n| !self.enemy_units_in_hex(n, side).is_empty());
        // Validate and cost the path.
        let mut prev = start;
        let mut total = Fixed(0);
        for (i, &h) in path.iter().enumerate() {
            if !prev.is_adjacent(h) {
                return Err(CommandError::NotAdjacent);
            }
            if !self.enemy_units_in_hex(h, side).is_empty() {
                return Err(CommandError::ProhibitedTerrain); // §8.13: not into an enemy-held hex
            }
            let from_zoc = self.hex_in_zoc_of(prev, enemy);
            let to_zoc = self.hex_in_zoc_of(h, enemy);
            if from_zoc && to_zoc {
                return Err(CommandError::ZocToZoc); // §13.26 / §8.15.1
            }
            if to_zoc && i + 1 != path.len() {
                return Err(CommandError::MustStopInZoc); // §13.26: stop upon entering a ZOC
            }
            let mut cp = match self.move_step_cost(prev, h, mode).ok_or(CommandError::DestinationOffMap)? {
                MoveCost::Cost(f) => f,
                MoveCost::Prohibited | MoveCost::EntireCpa => return Err(CommandError::ProhibitedTerrain),
            };
            // §8.6 break-off on the first step: in an enemy ZOC at the start = In Contact (§8.62,
            // +2 CP); Engaged (§8.63) costs +4 CP and applies even outside a ZOC (§8.66).
            if i == 0 && (from_zoc || engaged) {
                cp = Fixed(cp.0 + if engaged { 40 } else { 20 });
            }
            total = Fixed(total.0 + cp.0);
            prev = h;
        }
        // §13.24: the 4-CP cap when not starting adjacent to an enemy (one hex is always allowed).
        if !adjacent_at_start && path.len() > 1 && total.0 > 40 {
            return Err(CommandError::RetreatBeforeAssaultTooFar);
        }
        // Destination Stacking Limit (§9).
        let dest = *path.last().expect("non-empty");
        if let Some(limit) = self
            .map
            .terrain(dest)
            .and_then(|t| self.reference.terrain.get(&t))
            .and_then(|p| p.stacking_limit)
        {
            if self.hex_stacking_points(dest) + self.effective_stacking_points(unit) > limit {
                return Err(CommandError::StackingLimitExceeded);
            }
        }
        // §13.21: pay the move's Fuel from the mover's tanks / start-hex dump; no Fuel → illegal.
        if !self.consume_move_fuel(unit, start, total.0) {
            return Err(CommandError::InsufficientFuel);
        }
        // Apply: relocate, expend CP (with §6.21 over-CPA Disorganization), breakdown, ZOC status.
        self.unit_mut(unit).expect("checked").location = Some(crate::state::unit::UnitLocation::OnMap(dest));
        let cpa = self.effective_cpa(unit).unwrap_or(0);
        self.expend_cp_against(unit, total, cpa);
        for &h in path {
            let bp = self
                .map
                .terrain(h)
                .and_then(|t| self.reference.terrain.get(&t))
                .map(|p| p.breakdown.0)
                .unwrap_or(0);
            for m in self.formation_members(unit) {
                if let Some(mu) = self.unit_mut(m) {
                    mu.breakdown_points += bp;
                }
                self.resolve_breakdown(m);
            }
        }
        if self.hex_in_zoc_of(dest, enemy) {
            self.unit_mut(unit).expect("checked").moved_into_zoc = true;
        }
        // §8.62/§8.67: recompute Contact from ZOC and clear any engagement this retreat broke off.
        self.refresh_contact_status();
        Ok(CommandOutcome::RetreatedBeforeAssault { cp: total.0 })
    }

    /// §16 Patrol & Reconnaissance. The phasing player detaches `patrol_points` to reconnoitre the
    /// enemy-occupied `target`, resolving on the §16.6/§16.7/§16.8 tables with the supplied dice.
    /// The non-Phasing player's disclosures (`reveals`: unit → reported TOE within ±2, §16.51) and
    /// Objective-Loss casualties (`loss_choice`: unit → TOE) are validated against the rolled
    /// results. Returns a one-time [`PatrolReport`] — §16 information is not persisted (decision
    /// 2026-08-31: revealed once at patrol completion; the player "remembers" it externally).
    #[allow(clippy::too_many_arguments)]
    fn cmd_patrol(
        &mut self,
        patrol_points: &[(UnitId, u32)],
        target: HexCoord,
        exclude: &[UnitId],
        reveals: &[(UnitId, i32)],
        loss_choice: &[(UnitId, u32)],
        survival_die: u32,
        recon_die: u32,
        loss_die: u32,
    ) -> Result<CommandOutcome, CommandError> {
        use crate::reference::patrol::{objective_loss, patrol_survival, recon_reveal, ReconReveal};
        use crate::state::game::Weather;
        self.require_phase(Phase::Patrol)?;
        if patrol_points.is_empty() {
            return Err(CommandError::NoParticipants);
        }
        let side = self.active_player;
        let enemy = side.opponent();
        // §16.21: a side that fought Anti-Armor/Close Assault this stage may not Patrol.
        if self.land_combat_this_stage.contains(&side) {
            return Err(CommandError::PatrolNotAllowed);
        }
        // Validate each origin (§16.11 eligible type, §16.15 Cohesion ≥ −7, on-map, phasing side).
        for &(u, pts) in patrol_points {
            let unit = self.unit(u).ok_or(CommandError::UnitNotFound)?;
            if unit.nationality.side() != side || unit.hex().is_none() {
                return Err(CommandError::PatrolNotAllowed);
            }
            if pts == 0 || unit.cohesion <= -8 || !self.patrol_source_ok(u) {
                return Err(CommandError::PatrolNotAllowed);
            }
        }
        // §16.13/§16.14: ≤2 Patrol Points per origin hex, ≤3 total.
        let total: u32 = patrol_points.iter().map(|&(_, p)| p).sum();
        if total > 3 {
            return Err(CommandError::PatrolPointLimit);
        }
        let mut per_hex: std::collections::HashMap<HexCoord, u32> = std::collections::HashMap::new();
        for &(u, pts) in patrol_points {
            if let Some(h) = self.unit(u).and_then(|x| x.hex()) {
                *per_hex.entry(h).or_insert(0) += pts;
            }
        }
        if per_hex.values().any(|&p| p > 2) {
            return Err(CommandError::PatrolPointLimit);
        }
        // §16.17: no origin hex may be in a storm.
        let storm = |g: &Self, h: HexCoord| matches!(g.weather_at(h), Weather::Sandstorm | Weather::Rainstorm);
        if per_hex.keys().any(|&h| storm(self, h)) {
            return Err(CommandError::PatrolNotAllowed);
        }
        // §16.17/§16.22/§16.21: the target must be a non-storm, enemy-occupied hex within 5 hexes of
        // every origin (a documented range simplification of the §16.22 path), not barraged/bombed.
        if self.map.terrain(target).is_none() || storm(self, target) {
            return Err(CommandError::PatrolTargetInvalid);
        }
        if self.barraged_hexes_this_stage.contains(&target) {
            return Err(CommandError::PatrolTargetInvalid);
        }
        if per_hex.keys().any(|&h| h.distance(target) > 5) {
            return Err(CommandError::PatrolTargetInvalid);
        }
        let enemy_units: Vec<UnitId> = self
            .units_in_hex(target)
            .into_iter()
            .filter(|&id| self.unit(id).is_some_and(|u| u.nationality.side() == enemy))
            .collect();
        if enemy_units.is_empty() {
            return Err(CommandError::PatrolTargetInvalid); // §16.22: must be enemy-occupied
        }
        // §16.32: AA/Flak and Engineer (non-combat) units do not trigger a Patrol Survival check.
        let has_combat = enemy_units.iter().any(|&id| self.is_combat_unit(id));
        // §16.16: 1 Ammo + 2 Fuel per Patrol Point, present in each origin hex (unit reserve plus
        // co-located dumps) — check all, then expend, so a shortfall aborts before any change.
        use crate::state::game::Commodity;
        let origin_supply = |g: &Self, u: UnitId| -> (HexCoord, Side, u32, u32) {
            let x = g.unit(u).expect("origin checked");
            let (hex, s) = (x.hex().expect("on-map"), x.nationality.side());
            let ammo = x.ammo + g.available_commodity_at(hex, s, Commodity::Ammo);
            let fuel = x.fuel + g.available_commodity_at(hex, s, Commodity::Fuel);
            (hex, s, ammo, fuel)
        };
        for &(u, pts) in patrol_points {
            let (_, _, ammo, fuel) = origin_supply(self, u);
            if ammo < pts || fuel < 2 * pts {
                return Err(CommandError::InsufficientAmmo);
            }
        }
        // ---- Compute and VALIDATE the whole resolution before mutating anything, so an invalid
        //      command (bad disclosures/loss choice) aborts cleanly. Supply is expended regardless
        //      of the *die result* (§16.16) but not for an illegal command. ----
        let all_recce = patrol_points.iter().all(|&(u, _)| self.is_recce_unit(u));
        // §16.6 Patrol Survival (skipped if no combat units in the hex, §16.32).
        let survival = if has_combat {
            patrol_survival(survival_die, all_recce)
        } else {
            crate::reference::patrol::PatrolLoss::default()
        };
        let patrol_lost = (survival.killed + survival.captured).min(total);
        let surviving = total - patrol_lost;
        let wiped_out = has_combat && surviving == 0;

        // §16.5/§16.7 Reconnaissance disclosures (only if Patrol Points survive, §16.33).
        let mut revealed = Vec::new();
        if surviving > 0 {
            // §16.53/§16.54/§16.55: eligible units — combat units (never HQ/AA unless no combat
            // units present), not on the patroller's exclusion list.
            let any_combat = enemy_units.iter().any(|&id| self.is_combat_unit(id));
            let eligible: Vec<UnitId> = enemy_units
                .iter()
                .copied()
                .filter(|&id| !exclude.contains(&id))
                .filter(|&id| if any_combat { self.is_combat_unit(id) } else { true })
                .collect();
            let required = match recon_reveal(recon_die, surviving) {
                ReconReveal::All => eligible.len() as u32,
                ReconReveal::Count(n) => n.min(eligible.len() as u32),
            };
            if reveals.len() as u32 != required {
                return Err(CommandError::PatrolRevealMismatch);
            }
            let mut seen = std::collections::HashSet::new();
            for &(ru, reported) in reveals {
                if !eligible.contains(&ru) || !seen.insert(ru) {
                    return Err(CommandError::PatrolRevealMismatch);
                }
                let actual = self.unit(ru).map(|u| u.total_toe() as i32).unwrap_or(0);
                if (reported - actual).abs() > 2 {
                    return Err(CommandError::PatrolRevealMismatch); // §16.51: within ±2
                }
                let (designation, motorized) = {
                    let u = self.unit(ru).expect("checked");
                    (u.name.clone(), self.unit_is_motorized(ru))
                };
                revealed.push(RevealedUnit {
                    unit: ru,
                    designation,
                    unit_type: self.unit_type_name(ru),
                    motorized,
                    reported_strength: reported,
                });
            }
        } else if !reveals.is_empty() {
            return Err(CommandError::PatrolRevealMismatch); // no survivors → no information (§16.33)
        }

        // §16.8 Objective Loss on the patrollee; §16.34: Captured→Killed if the patrol was wiped out.
        let mut oloss = objective_loss(loss_die);
        if wiped_out {
            oloss = crate::reference::patrol::PatrolLoss { killed: oloss.killed + oloss.captured, captured: 0 };
        }
        let oloss_total = oloss.killed + oloss.captured;
        let chosen: u32 = loss_choice.iter().map(|&(_, n)| n).sum();
        if chosen != oloss_total {
            return Err(CommandError::PatrolLossMismatch);
        }
        // §16.34: casualties come from a combat unit if any are present.
        let any_combat = enemy_units.iter().any(|&id| self.is_combat_unit(id));
        for &(lu, _) in loss_choice {
            if !enemy_units.contains(&lu) || (any_combat && !self.is_combat_unit(lu)) {
                return Err(CommandError::PatrolLossMismatch);
            }
        }

        // ---- Everything validated; now apply the mutations. ----
        // §16.16: expend 1 Ammo + 2 Fuel per Patrol Point (reserve first, then co-located dumps).
        for &(u, pts) in patrol_points {
            let (hex, s, _, _) = origin_supply(self, u);
            let a_ob = self.unit(u).map_or(0, |x| x.ammo).min(pts);
            if let Some(x) = self.unit_mut(u) { x.ammo -= a_ob; }
            if pts - a_ob > 0 { self.consume_commodity_at(hex, s, Commodity::Ammo, pts - a_ob); }
            let need_f = 2 * pts;
            let f_ob = self.unit(u).map_or(0, |x| x.fuel).min(need_f);
            if let Some(x) = self.unit_mut(u) { x.fuel -= f_ob; }
            if need_f - f_ob > 0 { self.consume_commodity_at(hex, s, Commodity::Fuel, need_f - f_ob); }
        }
        // §16.33 patroller losses across the origins in order (Prisoner-Point accounting is §28).
        let mut owed = patrol_lost;
        for &(u, _) in patrol_points {
            if owed == 0 {
                break;
            }
            owed -= self.remove_unit_toe(u, owed);
        }
        // §16.8 patrollee losses.
        for &(lu, n) in loss_choice {
            self.remove_unit_toe(lu, n);
        }

        Ok(CommandOutcome::Patrol(PatrolReport {
            patroller_killed: survival.killed.min(total),
            patroller_captured: patrol_lost.saturating_sub(survival.killed.min(total)),
            wiped_out,
            revealed,
            patrollee_killed: oloss.killed,
            patrollee_captured: oloss.captured,
        }))
    }

    /// §19.41/§19.42 attach `unit` to a co-located `parent`. An assigned unit attaches for 1 CP each
    /// at any time (§19.41); a non-assigned unit for 2 CP each in the Reorganization (Organization)
    /// Phase only, subject to the §19.5 Maximum-Attachment limit (§19.42). A unit already attached
    /// elsewhere must detach first (§19.13).
    fn cmd_attach_unit(&mut self, unit: UnitId, parent: UnitId) -> Result<CommandOutcome, CommandError> {
        if unit == parent {
            return Err(CommandError::ReorgIllegal);
        }
        let (u_side, u_hex, u_attached, u_assigned) = {
            let u = self.unit(unit).ok_or(CommandError::UnitNotFound)?;
            (u.nationality.side(), u.hex(), u.attached_parent, u.assigned_parent)
        };
        let (p_side, p_hex) = {
            let p = self.unit(parent).ok_or(CommandError::UnitNotFound)?;
            (p.nationality.side(), p.hex())
        };
        // §19.1: same hex, same side; not already attached to some parent (§19.13).
        if u_side != p_side || u_hex.is_none() || u_hex != p_hex || u_attached.is_some() {
            return Err(CommandError::ReorgIllegal);
        }
        let cost: i32 = if u_assigned == Some(parent) {
            // §19.41: an assigned unit attaches for 1 CP each, at any time.
            if !matches!(self.phase, Phase::Organization | Phase::MovementAndCombat) {
                return Err(CommandError::WrongPhase);
            }
            1
        } else {
            // §19.42: a non-assigned unit — 2 CP each, Reorganization Phase only, within §19.5.
            // §19.46: the attachment counts against `parent` and every formation it is transitively
            // attached to — each must have room under its §19.5 limit.
            self.require_phase(Phase::Organization)?;
            for f in self.attachment_ancestors(parent) {
                if self.transitive_nonassigned_attachments(f) + 1 > self.nonassigned_attachment_limit(f) {
                    return Err(CommandError::ReorgIllegal); // §19.5 numeric cap (with §19.46 nesting)
                }
                if !self.attachment_type_ok(f, unit) {
                    return Err(CommandError::ReorgIllegal); // §19.5 type constraint (no Tank, etc.)
                }
            }
            2
        };
        self.unit_mut(unit).expect("checked").attached_parent = Some(parent);
        for id in [unit, parent] {
            let cpa = self.unit_cpa(id).unwrap_or(0);
            self.expend_cp_against(id, Fixed(cost * 10), cpa);
        }
        Ok(CommandOutcome::UnitAttached)
    }

    /// §19.43/§19.44 detach `unit` from its Parent Formation for 1 CP each, in the Organization or
    /// Movement Phase.
    fn cmd_detach_unit(&mut self, unit: UnitId) -> Result<CommandOutcome, CommandError> {
        if !matches!(self.phase, Phase::Organization | Phase::MovementAndCombat) {
            return Err(CommandError::WrongPhase);
        }
        let parent = self
            .unit(unit)
            .ok_or(CommandError::UnitNotFound)?
            .attached_parent
            .ok_or(CommandError::ReorgIllegal)?;
        self.unit_mut(unit).expect("checked").attached_parent = None;
        for id in [unit, parent] {
            let cpa = self.unit_cpa(id).unwrap_or(0);
            self.expend_cp_against(id, Fixed(10), cpa);
        }
        Ok(CommandOutcome::UnitDetached)
    }

    /// §19.21/§19.28 (re)assign `unit` to `parent`: it must currently be attached to `parent`
    /// (§19.21) and not assigned to another formation (§19.28). Reorganization (Organization) Phase;
    /// assignment is bookkeeping and costs no CP (the §6.3 costs are for the physical attachment).
    fn cmd_reassign_unit(&mut self, unit: UnitId, parent: UnitId) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::Organization)?;
        let cap = self.unit(parent).ok_or(CommandError::UnitNotFound)?.assigned_capacity;
        let (attached, assigned) = {
            let u = self.unit(unit).ok_or(CommandError::UnitNotFound)?;
            (u.attached_parent, u.assigned_parent)
        };
        if attached != Some(parent) {
            return Err(CommandError::ReorgIllegal); // §19.21: must be attached first
        }
        if assigned.is_some() && assigned != Some(parent) {
            return Err(CommandError::ReorgIllegal); // §19.28: not while assigned elsewhere
        }
        // §19.21/§19.3/§19.23: a new assignment may not push the parent's assigned count above its
        // historical capacity (eliminated units — 0 TOE — do not count).
        if assigned != Some(parent) && cap > 0 {
            let current = self
                .units
                .values()
                .filter(|x| x.assigned_parent == Some(parent) && x.total_toe() > 0)
                .count() as u32;
            if current >= cap {
                return Err(CommandError::ReorgIllegal);
            }
        }
        self.unit_mut(unit).expect("checked").assigned_parent = Some(parent);
        Ok(CommandOutcome::UnitReassigned)
    }

    /// Whether `unit` is an infantry-type unit (Organic personnel, or an Infantry characteristics
    /// type) — the source/recipient for §28.22 Guard Points.
    fn is_infantry_unit(&self, unit: UnitId) -> bool {
        let Some(u) = self.unit(unit) else { return false };
        if matches!(u.composition, crate::state::unit::Composition::Organic { .. }) {
            return true;
        }
        self.reference
            .unit_characteristics
            .get(&u.nationality)
            .and_then(|m| m.get(&u.id_code))
            .is_some_and(|c| c.unit_type.contains("Infantry"))
    }

    /// §28.22 form a Guard Point for the phasing side's Prisoner stack at `group_hex` by subtracting
    /// one TOE from a co-located infantry combat unit.
    fn cmd_form_guard(&mut self, group_hex: HexCoord) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::Organization)?;
        let side = self.active_player;
        let gi = self
            .prisoner_groups
            .iter()
            .position(|g| g.captor == side && g.hex == group_hex)
            .ok_or(CommandError::PrisonerActionIllegal)?;
        // A co-located infantry unit of the captor's side with a TOE to spare.
        let donor = self
            .units_in_hex(group_hex)
            .into_iter()
            .find(|&id| {
                self.unit(id).is_some_and(|u| u.nationality.side() == side && u.total_toe() > 0)
                    && self.is_infantry_unit(id)
            })
            .ok_or(CommandError::PrisonerActionIllegal)?;
        if self.remove_unit_toe(donor, 1) == 0 {
            return Err(CommandError::PrisonerActionIllegal);
        }
        self.prisoner_groups[gi].guards += 1;
        Ok(CommandOutcome::GuardFormed)
    }

    /// §28.22 return one Guard Point from the stack at `group_hex` to a co-located infantry unit.
    fn cmd_return_guard(&mut self, group_hex: HexCoord) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::Organization)?;
        let side = self.active_player;
        let gi = self
            .prisoner_groups
            .iter()
            .position(|g| g.captor == side && g.hex == group_hex && g.guards > 0)
            .ok_or(CommandError::PrisonerActionIllegal)?;
        let recipient = self
            .units_in_hex(group_hex)
            .into_iter()
            .find(|&id| self.unit(id).is_some_and(|u| u.nationality.side() == side) && self.is_infantry_unit(id))
            .ok_or(CommandError::PrisonerActionIllegal)?;
        // Restore one TOE (Organic infantry), capped at its maximum (§19.6).
        let max = self.unit_max_toe(recipient).unwrap_or(u32::MAX);
        if let Some(u) = self.unit_mut(recipient) {
            if let crate::state::unit::Composition::Organic { current_toe } = &mut u.composition {
                if *current_toe < max {
                    *current_toe += 1;
                }
            }
        }
        self.prisoner_groups[gi].guards -= 1;
        Ok(CommandOutcome::GuardReturned)
    }

    /// §28.13 move the phasing side's Prisoner stack one hex (Truck Convoy Phase, CPA 8). Escort is
    /// required (§28.17); prisoners beyond what the guards can escort escape at the origin (§28.23).
    fn cmd_move_prisoners(&mut self, from: HexCoord, to: HexCoord) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::TruckConvoy)?;
        let side = self.active_player;
        let enemy = side.opponent();
        let gi = self
            .prisoner_groups
            .iter()
            .position(|g| g.captor == side && g.hex == from)
            .ok_or(CommandError::PrisonerActionIllegal)?;
        // §28.12/§28.13: adjacent, on-map, not enemy-occupied or enemy-controlled.
        if !from.is_adjacent(to) || self.map.terrain(to).is_none() {
            return Err(CommandError::PrisonerActionIllegal);
        }
        if !self.enemy_units_in_hex(to, side).is_empty() || self.hex_in_zoc_of(to, enemy) {
            return Err(CommandError::PrisonerActionIllegal);
        }
        // §28.13: CPA 8 (80 tenths). Each hex costs 1 CP here (a documented simplification).
        if self.prisoner_groups[gi].cpa_used + 10 > 80 {
            return Err(CommandError::PrisonerActionIllegal);
        }
        // §28.17/§28.23: the guards can escort 5 prisoners each; the excess escapes at the origin.
        let (points, guards) = { let g = &self.prisoner_groups[gi]; (g.points, g.guards) };
        let escortable = guards.saturating_mul(5);
        let escaped = points.saturating_sub(escortable);
        if escaped > 0 {
            self.prisoner_groups[gi].points -= escaped;
            let original = self.prisoner_groups[gi].original;
            self.escape_points(original, from, escaped);
        }
        // Move the escorted remainder.
        if self.prisoner_groups[gi].points == 0 {
            self.prisoner_groups.remove(gi);
            return Ok(CommandOutcome::PrisonersMoved { escaped });
        }
        self.prisoner_groups[gi].hex = to;
        self.prisoner_groups[gi].cpa_used += 10;
        // §28.16: reaching a Departure Point removes them permanently.
        if self.is_pow_departure(to, side) {
            self.prisoner_groups.remove(gi);
        }
        Ok(CommandOutcome::PrisonersMoved { escaped })
    }

    /// §28.3 repair a piece of captured equipment (`system`) at `hex`, making it reusable.
    fn cmd_repair_captured_equipment(&mut self, hex: HexCoord, system: String) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::Repair)?;
        let side = self.active_player;
        let e = self
            .captured_equipment
            .iter_mut()
            .find(|e| e.captor == side && e.hex == hex && e.system == system && !e.repaired)
            .ok_or(CommandError::PrisonerActionIllegal)?;
        e.repaired = true;
        Ok(CommandOutcome::CapturedEquipmentRepaired)
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
        // §8.6 break-off: in an enemy ZOC at segment start = In Contact (§8.62, +2 CP); Engaged
        // (§8.63) costs +4 CP and applies even when the unit is not in a ZOC (§8.66).
        let engaged = self.unit(unit).expect("checked").status.engaged;
        if from_zoc || engaged {
            cp = Fixed(cp.0 + if engaged { 40 } else { 20 });
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
        // §42.47: an airdropped combat unit may not voluntarily exceed its CPA in the Operations
        // Stage it was dropped (it may still move/fight otherwise). Reuses the §42.48 drop record.
        let abs_stage = self.game_turn * 3 + self.operations_stage as u32;
        if self.airdrop_last_stage.get(&unit) == Some(&abs_stage) {
            let already = self.unit(unit).map(|u| u.cp_expended.0).unwrap_or(0);
            if already + cp.0 > cpa * 10 {
                return Err(CommandError::AirdropCpaLimit);
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
        // §54.41: a land combat unit that enters a rail hex becomes the last to control it.
        self.note_rail_control(to, unit);
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
        // §8.62/§8.67: recompute Contact from ZOC and clear any engagement this move broke off.
        self.refresh_contact_status();
        Ok(CommandOutcome::Moved)
    }
}
