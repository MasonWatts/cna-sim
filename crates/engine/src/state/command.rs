//! The command boundary (design.md §3.3): all state mutations from players/AI/UI flow
//! through `GameState::apply`, which validates legality and applies the effect.
//!
//! Multi-hex/continual movement is issued as repeated single-hex steps. The boundary now
//! covers the full land turn (movement with ZOC, stacking, CPA over-expenditure, reaction,
//! retreat, breakdown, and attached-unit movement; barrage/anti-armor/close assault) plus
//! the air, naval, logistics, and construction commands — see the `Command` enum below.

use crate::reference::crt::BarrageTargetClass;
use crate::reference::terrain::{Fixed, MoveCost};
use serde::{Deserialize, Serialize};
use crate::reference::units::Side;
use crate::rules::movement::{step_cost, MoveMode};
use crate::state::combat::{AntiArmorOutcome, BarrageOutcome, CloseAssaultOutcome};
use crate::state::game::{GameState, Phase};
use crate::state::map::HexCoord;
use crate::state::unit::UnitId;

/// §36.3: may a plane of `class` base at a facility of `kind`? Flying-boats need a basin/alighting
/// area; every other class needs a normal airfield/strip.
fn facility_serves(kind: crate::state::air::AirFacilityKind, class: crate::reference::aircraft::AircraftClass) -> bool {
    if class == crate::reference::aircraft::AircraftClass::FlyingBoat {
        kind.serves_flying_boats()
    } else {
        kind.serves_normal_aircraft()
    }
}

/// Which broken-down component to repair (§22).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

/// §12.5/§12.51: the kind of Facility an artillery barrage targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FacilityTarget {
    /// A Major City or a placed Fortification (§12.53 — Artillery Barrage Points column).
    Fortification,
    /// A Road hex (§12.53).
    Road,
    /// A Railroad hex (§12.53).
    Railroad,
    /// A Supply Dump (§12.54 — Bomb column, Raw Barrage Points).
    SupplyDump,
    /// An Air Facility — airfield / landing strip / port (§12.54).
    AirFacility,
}

/// An action a player attempts. Validated and applied by [`GameState::apply`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// §20.46: move the on-map Replacement-Point pool at index `index` one hex to adjacent `to`
    /// (Truck Convoy Phase — pools move toward the front as truck convoys). May not voluntarily enter
    /// an enemy ZOC unless a Friendly unit already holds the hex (§20.42). (Per-pool CPA budget and
    /// §20.45 Stores/Fuel upkeep for the haul are deferred.)
    MoveReplacement { index: usize, to: HexCoord },
    /// §20.44: load (pick up) a co-located Replacement-Point pool onto a free truck convoy, binding
    /// the two so the pool rides the convoy (and the convoy gains §27.58 escort effects). Infantry
    /// pools must be loaded before they can move; the convoy must have enough §54.2 infantry lift.
    LoadReplacement { convoy: usize, pool: usize },
    /// §20.44: unload (drop off) a Replacement-Point pool from its carrying convoy, clearing the
    /// binding so the pool becomes a standalone (stranded) counter again.
    UnloadReplacement { pool: usize },
    /// §24.9: construct a Supply Dump at a co-located TOE Strength Point's hex, turning loose supplies
    /// there into a convoy-loadable dump. A real dump costs 3 CP + 20 Store Points (Logistics Game); a
    /// `dummy` dump costs 3 CP only. (Organization Phase.)
    ConstructSupplyDump { unit: UnitId, hex: HexCoord, dummy: bool },
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
    RaidAirfieldSas { unit: UnitId },
    /// §27.13: form a desert raider (LRDG / DSA) from a Friendly Recce/AC `from_unit`, removing one
    /// TOE Strength Point and placing the counter in its hex (Organization Phase).
    FormRaider { kind: crate::state::game::RaiderKind, from_unit: UnitId },
    /// §27.3: move the raider at index `raider` one hex to adjacent `to` (Movement & Combat Phase).
    /// Raiders ignore enemy Zones of Control (§27.34) but may not enter an Enemy-occupied hex except
    /// to raid (§27.42). Entering an enemy-controlled (ZOC) hex risks being spotted: `spot_roll` is a
    /// d6 and a 6 eliminates the raider (§27.41/§27.43).
    MoveRaider { raider: usize, to: HexCoord },
    /// §27.44: an Enemy attempts to spot the raider at index `raider` (spending 1 CP); `roll` is a
    /// d6 and a 6 eliminates it (§27.43).
    SpotRaider { raider: usize },
    /// §27.35: the raider at index `raider` resupplies where it stands — drawing 2 Fuel + 1 Ammo
    /// from a co-located Friendly supply dump, and Water from an oasis/bir in the hex. It must draw
    /// Water each Game-Turn or be eliminated, and needs 2 Fuel to move (§27.35).
    ResupplyRaider { raider: usize },
    /// §27.6: a Commonwealth LRDG in Rommel's hex attempts a Raid on Rommel (Movement & Combat
    /// Phase). `roll` is 2d6 (2–12) resolved on the §27.64 table. Costs 10 CP; max four per campaign.
    RaidRommel { raider: usize },
    /// §27.52: the raider blows the water pipeline in its hex (no enemy combat units present); a d6
    /// `roll` of 1-4 blows it. Costs 5 CP. (Movement & Combat Phase.)
    RaidPipeline { raider: usize },
    /// §27.53: the raider cripples the (non-strip, non-major-city) airfield in its hex (no enemy
    /// combat units present); a d6 `roll` of 1-2 reduces its capacity one level. Costs 5 CP.
    RaidAirfield { raider: usize },
    /// §27.54: the raider destroys grounded enemy planes in its hex (no enemy combat units / not a
    /// major city); a d6 `roll` of 1-2 destroys 10% of each enemy squadron's planes, a 6 eliminates
    /// the raider. Costs 5 CP.
    RaidPlanes { raider: usize },
    /// §27.55: the raider attacks the enemy Supply Dump in its hex (not a major city). If guarded by
    /// enemy combat units, `attack_roll` (2d6) must exceed the guards' Raw Defensive Assault Strength
    /// or the raider is eliminated; then `destroy_roll` (d6) of 1-2 destroys 10% of the dump. Costs
    /// 5 CP. (§27.56: a dummy dump is revealed instead.)
    RaidSupplyDump { raider: usize },
    /// §27.57-58: the raider attacks the enemy free truck convoy at index `convoy` in its hex; a d6
    /// `roll` of 1-2 destroys one Truck Point + its cargo. Replacement-carrying convoys add 1 to the
    /// roll and a 6 eliminates the raider (§27.58). Costs 5 CP.
    RaidTruckConvoy { raider: usize, convoy: usize },
    /// §27.59: during the enemy's Truck Convoy Phase, a raider within four Movement CPs of the
    /// passing convoy at index `convoy` halts and attacks it (§27.58 mechanics, a d6 `roll`).
    InterceptTruckConvoy { raider: usize, convoy: usize },
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
        /// §12.23: a specific target unit the firer *knows* is present (via Patrol or previous
        /// combat). When `None` the firer barrages "blind" at `target_class` (§12.24). When `Some`
        /// the unit must be a valid target of `target_class` in `target` and known to the firer
        /// (currently In-Contact/Engaged with the firing side — the "previous combat" path).
        target_unit: Option<UnitId>,
    },
    /// Anti-Armor fire (§14) by the attacking units against a target hex.
    AntiArmor {
        attackers: Vec<UnitId>,
        target_hex: HexCoord,
        /// §14.21: defending units the defender has withheld from *both* Anti-Armor and Close
        /// Assault — they take no Anti-Armor damage. Empty in the common case (fire hits the hex).
        withheld: Vec<UnitId>,
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
    /// §12.11/§12.12 declare an artillery unit's gun position Forward (`true`) or Back (`false`) for
    /// the coming Combat Segment. Forward guns may coordinate barrage fire (§12.13) but are more
    /// vulnerable in close assault (§15.84). Set in the Organization / Movement-and-Combat phase.
    SetGunPosition { unit: UnitId, forward: bool },
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
    /// §17.3 designate a Commonwealth unit to undergo Training this Operations Stage (raising its
    /// Basic Morale toward its trained reference). Organization Phase.
    TrainUnit { unit: UnitId },
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
    },
    /// §41.6: resolve a Commonwealth strategic bombing attack on a planned convoy (Convoy
    /// Resolution Phase). `roll` is a 2d6 value encoded `10*d1+d2`.
    BombConvoy {
        convoy_index: usize,
        bomb_points: u32,
        torpedo_planes: u32,
        total_planes: u32,
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
    ProduceCommonwealthTrucks {},
    /// §20.78B ProduceCommonwealthInfantry: roll the Commonwealth Infantry Production Table once
    /// (Organization Phase) with a 2d6 `roll` (2–12). The produced Infantry Replacement Points
    /// arrive four Game-Turns later and begin Training.
    ProduceCommonwealthInfantry {},
    /// §20.66a ProduceAxisTrucks: plan `count` Truck Points of `kind` (Organization Phase),
    /// validated against the Axis Truck Production row's lifetime total and per-Game-Turn maximum.
    /// On success the trucks arrive two Game-Turns later (into the Axis free-truck pool).
    ProduceAxisTrucks { kind: crate::reference::logistics::TruckKind, count: u32 },
    /// §55.22: block a port's harbour by scuttling (an engineer/CW-HQ must be present).
    BlockHarbor { port: String, has_engineer: bool },
    /// §55.26: remove one level of harbour blocking with the required engineer units.
    UnblockHarbor { port: String, engineers: u32 },
    /// §34.82/§60.32/§60.42: place an available SGSU counter at an air-facility hex (Organization
    /// Phase), spending one from the nationality's counter supply.
    DeploySgsu {
        nationality: crate::reference::units::Nationality,
        class: crate::reference::squadron::SquadronClass,
        at: HexCoord,
    },
    /// §34.84: withdraw a deployed squadron to satisfy a due `WD` requirement (Organization Phase).
    WithdrawSquadron { squadron: crate::state::air::SquadronId },
    /// §36.53: base a Commonwealth squadron in an off-map air facility by chart name (Organization
    /// Phase).
    BaseSquadronOffMap { squadron: crate::state::air::SquadronId, facility: String },
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
    RemoveHarborMine { port: String },
    /// §42.53: Commonwealth naval reconnaissance of a shipping lane (Convoy Resolution Phase).
    /// `roll` is a 2d6 value encoded `10*d1+d2`.
    ReconConvoyLane { lane: u8, planes: u32 },
    /// §33.0 II.B / §44.42: the Axis determines Malta raid support at Availability `level` (1–4)
    /// with a 2d6 `roll` (Malta Availability Determination Phase).
    DetermineMaltaAvailability { level: u32 },
    /// §38.34: roll `base_die` to refit a squadron's un-refitted planes via the §38.37 table (Land
    /// Support Air Phase). `wrong_squadron` applies the §38.35 not-own-SGSU penalty. Returns the
    /// number refitted.
    RefitSquadron {
        squadron: crate::state::air::SquadronId,
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
    /// §53.14/§53.24: load supply from a co-located Friendly Supply Dump onto the free truck convoy
    /// at index `convoy` (Truck Convoy Phase) — the pick-up end of a dump→dump/unit haul. Returns the
    /// amount loaded.
    LoadConvoy { convoy: usize, commodity: crate::state::game::Commodity, amount: u32 },
    /// §53.14/§53.24: off-load supply from the free truck convoy at index `convoy` into a Friendly
    /// Supply Dump in its hex (creating one if none) — the drop-off end. Returns the amount unloaded.
    UnloadConvoy { convoy: usize, commodity: crate::state::game::Commodity, amount: u32 },
    /// §22.13: field-repair the broken-down Truck Points of the free truck convoy at `convoy` (Repair
    /// Phase); `roll` (1-6) drives how many are recovered.
    RepairConvoyTrucks { convoy: usize },
    /// §52.16: poison a well (Movement & Combat Phase); `die` decides success.
    PoisonWell { hex: HexCoord },
    /// §52.17: sweeten (un-poison) a well (Movement & Combat Phase); `die` decides success.
    SweetenWell { hex: HexCoord },
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
    /// §4.44a footnote b: refit the 102nd Anti-tank Regt with Deacons (Organization Phase). Repeatable;
    /// requires the unit + trained SP-6-pdr AT Replacement Points at Cairo for two continuous
    /// Game-Turns. Returns the number of TOE points converted to Deacons.
    RefitDeacons { unit: UnitId },
    /// §49.17: siphon Fuel from one unit to another (Organization Phase). Returns the amount moved.
    SiphonFuel { donor: UnitId, receiver: UnitId, amount: u32 },
    /// §22: attempt to repair a broken-down unit's vehicles (Repair Phase). Returns points repaired.
    Repair { unit: UnitId, kind: RepairKind },
    /// §18: place a unit in (or out of) Reserve status (Reserve Designation Phase).
    DesignateReserve { unit: UnitId, status: crate::state::unit::ReserveStatus },
    /// §19.8: form an ad-hoc Axis anti-tank battery by augmenting a Brigade-level infantry-type HQ
    /// with `count` TOE points of `gun_system` (a single AT gun type), drawn from co-located trained
    /// Gun-class Replacement Points (Organization Phase).
    AugmentAntiTank { hq: UnitId, gun_system: String, count: u32 },
    /// §19.9: augment a Commonwealth infantry battalion (from GT 75) with `count` TOE points of
    /// `gun_system` Anti-Tank guns, drawn from co-located trained Gun Replacement Points
    /// (Organization Phase).
    AugmentBattalionAntiTank { battalion: UnitId, gun_system: String, count: u32 },
    /// §20.5: upgrade an armored Recce / Armored-Car unit with a light-tank TOE point (§20.53),
    /// drawn from a co-located trained Tank (or, for Italy, Recce) Replacement pool (Organization
    /// Phase). Raises its per-TOE ratings to Anti-Armor 1 / Offensive Close Assault 3 (§20.54).
    UpgradeRecce { unit: UnitId },
    /// §12.5: artillery barrage against a Facility in `target` (rather than a unit). §12.53 uses the
    /// §41.5 Artillery Barrage Points column for structures; §12.54 uses the Bomb column with Raw
    /// Barrage Points for Supply Dumps and Air Facilities.
    BarrageFacility { barragers: Vec<UnitId>, target: HexCoord, facility: FacilityTarget },
    /// §21.6: tow the Broken Down Vehicle marker at index `marker` one hex to adjacent `to`
    /// (Repair Phase; 10 CP budget, 20 with a co-located Tank Delivery Squadron).
    TowBrokenVehicle { marker: usize, to: HexCoord },
    /// §20.81/§20.82: satisfy a by-type Mandatory Withdrawal choice by designating the `units` to
    /// withdraw (must match the choice's type/count, at Cairo/Alexandria at ≥75% TOE).
    SatisfyWithdrawalChoice { choice: usize, units: Vec<UnitId> },
    /// §14.54/§21.6: tow the Destroyed Tank marker at `equipment` one hex to adjacent `to` (Repair
    /// Phase; 10-CP budget, 20 with a co-located Tank Delivery Squadron).
    TowDestroyedTank { equipment: usize, to: HexCoord },
    /// §30.4: (Axis) plan the once-per-game Italian Chariot raid on Alexandria (from July 1941).
    PlanChariotRaid,
    /// §30.44: (Axis) execute the planned Chariot raid — resolves the §30.46 table and damages
    /// Commonwealth ships at Alexandria.
    ExecuteChariotRaid,
    /// §17.5: voluntarily surrender the entire stack in `hex` to an adjacent enemy combat unit
    /// (`captor`, which pays 2 CP to accept, §17.54). If `destroy_material`, the owner first
    /// attempts §17.55/§17.56 pre-capture destruction of tanks/guns ((die−1)×10%) and
    /// trucks ((die−1)×20%).
    VoluntarySurrender { hex: HexCoord, captor: UnitId, destroy_material: bool },
    /// §30.15/§30.18: assign a Commonwealth warship to a location for the Operations Stage
    /// (Commonwealth Fleet Phase).
    AssignShip { ship_index: usize, location: crate::state::naval::ShipLocation },
    /// §30.37: repair a warship one stage toward its next Gun level (Commonwealth Fleet Phase;
    /// only in Alexandria).
    RepairShip { ship_index: usize },
    /// §30.51/§30.52: transfer a Commonwealth combat `unit` by sea from the port it is in to another
    /// port `to_port` (Truck Convoy Movement Phase). One transfer per port per Operations Stage;
    /// limited by the ports' personnel (Stacking-Point) capacity; inoperable ports may not be used.
    NavalTransferTroops { unit: UnitId, to_port: String },
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
        /// §40.74: Axis fighters flying flak suppression against the field's AA. Every 3 suppress
        /// one *light* flak point; heavy flak (§40.74) is never suppressed.
        #[serde(default)]
        flak_suppression: u32,
    },
    /// §44.13: the Commonwealth builds capacity levels on a Maltese air facility using the Malta
    /// Air Facility Construction Table (Organization Phase; once per Game-Turn per base).
    ConstructMaltaFacility { base_index: usize },
    /// §44.18: (Commonwealth) divert one AA Replacement Point to a specific Malta base — Light = 1 AA
    /// point, Heavy (`heavy: true`) = 4 — consuming one matching Light/Heavy AA point from the
    /// Commonwealth replacement pool. Points are held per-base (§44.24) but the Malta-wide light+heavy
    /// total is capped at 48; at most one diversion per month (Organization Phase).
    DivertMaltaAA { base_index: usize, heavy: bool },
    /// §44.15/§44.18: (Commonwealth) redistribute already-diverted flak points between two Malta
    /// fields (a free, out-of-sequence move, like §44.15 plane moves). Organization Phase.
    MoveMaltaFlak { from_base: usize, to_base: usize, light: u32, heavy: u32 },
    /// §44.14: (Commonwealth) base `count` planes of `aircraft_type` from the CW plane pool onto a
    /// Malta facility (Organization Phase). Capped at the base's plane capacity (levels × 18) and
    /// gated by §36.3 (flying-boats only at basins/alighting areas; land planes only at airfields).
    BaseMaltaPlanes { base_index: usize, aircraft_type: String, count: u32 },
    /// §44.15: (Commonwealth) move `count` planes of `aircraft_type` between two Malta facilities,
    /// free of cost (Organization Phase); capacity- and §36.3-checked at the destination.
    MoveMaltaPlanes { from_base: usize, to_base: usize, aircraft_type: String, count: u32 },
    /// §40.24/§44: (Commonwealth) fly `planes` of `aircraft_type` from a Malta field to interdict a
    /// planned Axis convoy (Convoy Resolution Phase). The type must have Convoy/Land bombing
    /// capability (chart column B) and range to reach the convoy's destination (§37.4 Malta
    /// distance); `torpedo` uses the loadout's torpedo rating instead of its bombload. Resolves on
    /// the Axis-Naval-Convoy CRT (tonnage lost); the convoy's flak may down attackers (§56.16), which
    /// are removed from the field. Malta planes auto-refuel/rearm (§44.16), so no supply is spent.
    InterdictConvoy { base_index: usize, aircraft_type: String, planes: u32, torpedo: bool, convoy_index: usize },
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
    /// §42.27: fly `planes` on an air-reconnaissance mission from the squadron's base to `target`,
    /// validating Recon capability (§42.21), flight range (§37.12) and weather (§37.22). Rolls a d6
    /// `die`; the §42.27 table (with +1 per 4 planes) gives how many enemy battalion-equivalents in
    /// the hex have their TOE Strength revealed to the flier this Operations Stage (§42.24a order:
    /// Tanks, then Infantry, then Artillery). (Land Support Air.)
    ReconHex {
        squadron: crate::state::air::SquadronId,
        target: HexCoord,
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
    },
}

impl Command {
    /// The primary rulebook section(s) that govern this command — surfaced in the viewer's "Relevant
    /// Rules" panel (the sole home for § citations; the friendly action description carries none).
    /// Exhaustive (no wildcard) so every command has its citation and new variants must add one.
    pub fn rule_refs(&self) -> &'static [&'static str] {
        match self {
            // ── Movement & Rommel ──
            Command::MoveStep { .. } | Command::FormationMove { .. } => &["§8.13", "§8.14"],
            Command::MoveRommel { .. } | Command::RetreatRommel { .. } => &["§31"],
            Command::DesignateRommelUnit { .. } => &["§31.4"],
            Command::RailMove { .. } => &["§8.72"],
            Command::MoveOffMap { .. } => &["§8.8"],
            // ── Combat ──
            Command::CloseAssault { .. } => &["§15"],
            Command::AntiArmor { .. } => &["§14"],
            Command::Barrage { .. } => &["§12.6", "§12.23"],
            Command::BarrageFacility { .. } => &["§12.5", "§12.53", "§12.54"],
            Command::Retreat { .. } => &["§15.82"],
            Command::React { .. } => &["§8.5"],
            Command::RetreatBeforeAssault { .. } => &["§13"],
            Command::Patrol { .. } => &["§16"],
            Command::SetGunPosition { .. } => &["§12.11", "§12.12"],
            Command::PlaceDummyTank { .. } => &["§16.4"],
            // ── Reorganization ──
            Command::AttachUnit { .. } => &["§19.41", "§19.5"],
            Command::DetachUnit { .. } => &["§19.43"],
            Command::ReassignUnit { .. } => &["§19.21"],
            Command::TrainUnit { .. } => &["§17.3"],
            Command::DesignateReserve { .. } => &["§18"],
            Command::WithdrawUnit { .. } | Command::ReturnUnit { .. } => &["§64.75"],
            Command::UpgradeRecce { .. } => &["§20.5", "§20.53", "§20.54"],
            Command::AugmentAntiTank { .. } => &["§19.8", "§19.82", "§19.83"],
            Command::AugmentBattalionAntiTank { .. } => &["§19.9", "§19.92", "§19.94"],
            Command::SatisfyWithdrawalChoice { .. } => &["§20.81", "§20.82"],
            // ── Unit supply / logistics ──
            Command::RefuelUnit { .. } => &["§49"],
            Command::RearmUnit { .. } => &["§50"],
            Command::SiphonFuel { .. } => &["§49.17"],
            Command::LoadTrucks { .. } | Command::UnloadTrucks { .. } => &["§53.24"],
            Command::AssignTrucks { .. } | Command::ReleaseTrucks { .. } => &["§53.11"],
            Command::Repair { .. } => &["§22"],
            // ── Dumps / convoys / replacements ──
            Command::ConstructSupplyDump { .. } => &["§24.9"],
            Command::FormTruckConvoy { .. } => &["§8.97"],
            Command::MoveTruckConvoy { .. } => &["§8.98"],
            Command::LoadConvoy { .. } | Command::UnloadConvoy { .. } => &["§53.14", "§53.24"],
            Command::RepairConvoyTrucks { .. } => &["§22.13"],
            Command::MoveReplacement { .. } => &["§20.46", "§20.42"],
            Command::LoadReplacement { .. } | Command::UnloadReplacement { .. } => &["§20.44"],
            Command::AbsorbReplacement { .. } => &["§20"],
            Command::RefitDeacons { .. } => &["§4.44a"],
            // ── Construction ──
            Command::LayMinefield { .. } | Command::ClearMinefield { .. } => &["§24.3"],
            Command::BuildPipeline { .. } => &["§52.2"],
            Command::BuildFortification { .. } => &["§24.4"],
            Command::BuildRailroad { .. } => &["§24.6"],
            Command::DemolishRailroad { .. } => &["§24.66"],
            Command::BuildRoad { .. } => &["§24.5"],
            Command::RailMoveSupply { .. } => &["§54.3"],
            Command::ImportRollingStock { .. } => &["§54.43"],
            // ── Wells ──
            Command::PoisonWell { .. } => &["§52.16"],
            Command::SweetenWell { .. } => &["§52.17"],
            // ── Prisoners / captured / surrender ──
            Command::FormGuard { .. } | Command::ReturnGuard { .. } => &["§28.22"],
            Command::MovePrisoners { .. } => &["§28.13"],
            Command::RepairCapturedEquipment { .. } => &["§28.3"],
            Command::VoluntarySurrender { .. } => &["§17.5", "§17.54", "§17.56"],
            // ── Raiders / commandos ──
            Command::FormRaider { .. } => &["§27.13"],
            Command::MoveRaider { .. } => &["§27.3"],
            Command::SpotRaider { .. } => &["§27.44"],
            Command::ResupplyRaider { .. } => &["§27.35"],
            Command::RaidRommel { .. } => &["§27.6"],
            Command::RaidPipeline { .. } => &["§27.52"],
            Command::RaidAirfield { .. } => &["§27.53"],
            Command::RaidPlanes { .. } => &["§27.54"],
            Command::RaidSupplyDump { .. } => &["§27.55"],
            Command::RaidTruckConvoy { .. } => &["§27.57"],
            Command::InterceptTruckConvoy { .. } => &["§27.59"],
            Command::LandLayforce { .. } => &["§27.72"],
            Command::EmbarkLayforce { .. } => &["§27.77"],
            Command::LandSas { .. } => &["§27.83"],
            Command::RaidAirfieldSas { .. } => &["§27.85"],
            // ── Naval ──
            Command::AssignShip { .. } => &["§30.15"],
            Command::RepairShip { .. } => &["§30.37"],
            Command::NavalTransferTroops { .. } => &["§30.51", "§30.52"],
            Command::OffShoreBombard { .. } => &["§30.2"],
            Command::ArtilleryBombardShip { .. } => &["§30.35"],
            Command::SanGiorgioAttackShip { .. } => &["§30.39"],
            Command::BlockHarbor { .. } => &["§55.22"],
            Command::UnblockHarbor { .. } => &["§55.26"],
            Command::LayHarborMine { .. } => &["§41.39", "§55.27"],
            Command::RemoveHarborMine { .. } => &["§55.27"],
            Command::PlanChariotRaid => &["§30.41", "§30.42"],
            Command::ExecuteChariotRaid => &["§30.44", "§30.46"],
            Command::TowBrokenVehicle { .. } => &["§21.6", "§21.63", "§22.65"],
            Command::TowDestroyedTank { .. } => &["§14.54", "§21.6"],
            // ── Axis convoys / production ──
            Command::PlanConvoy { .. } => &["§56.15"],
            Command::BombConvoy { .. } => &["§41.6"],
            Command::DeliverConvoy { .. } => &["§56.16"],
            Command::ReconConvoyLane { .. } => &["§42.53"],
            Command::PlanProduction { .. } => &["§20.66", "§20.78C"],
            Command::ProduceCommonwealthTrucks { .. } => &["§20.78A"],
            Command::ProduceCommonwealthInfantry { .. } => &["§20.78B"],
            Command::ProduceAxisTrucks { .. } => &["§20.66a"],
            // ── Air: squadrons ──
            Command::DeploySgsu { .. } => &["§34.82"],
            Command::WithdrawSquadron { .. } => &["§34.84"],
            Command::BaseSquadronOffMap { .. } => &["§36.53"],
            Command::AssignSquadronPlanes { .. } => &["§35.2"],
            Command::RefitSquadron { .. } => &["§38.34"],
            Command::ReadySquadron { .. } => &["§38.2", "§38.3"],
            Command::ArmSquadron { .. } => &["§38.4"],
            Command::FlySquadronMission { .. } => &["§37"],
            Command::AssignPilot { .. } => &["§40.12"],
            Command::TransferSquadron { .. } => &["§43.14"],
            Command::DeclareInitiativeOrder { .. } => &["§33.0"],
            Command::DetermineMaltaAvailability { .. } => &["§44.42"],
            // ── Air: missions ──
            Command::DesignateCap { .. } => &["§40.2"],
            Command::ReconHex { .. } => &["§42.27"],
            Command::Strafe { .. } => &["§40.5"],
            Command::FlyBombingMission { .. } | Command::FlyBombingMissionAt { .. } => &["§39", "§41.3"],
            Command::BombAirFacility { .. } => &["§41.36"],
            Command::BombShip { .. } => &["§30.34", "§41.34"],
            Command::AirdropSupplies { .. } => &["§42.4"],
            Command::TransportUnit { .. } => &["§42.3"],
            Command::AirdropUnit { .. } => &["§42.41"],
            // ── Malta ──
            Command::BombMaltaBase { .. } => &["§44.24"],
            Command::ConstructMaltaFacility { .. } => &["§44.13"],
            Command::DivertMaltaAA { .. } => &["§44.18"],
            Command::MoveMaltaFlak { .. } => &["§44.15", "§44.18"],
            Command::BaseMaltaPlanes { .. } => &["§44.14"],
            Command::MoveMaltaPlanes { .. } => &["§44.15"],
            Command::InterdictConvoy { .. } => &["§40.24"],
        }
    }
}

/// §16.51: the information a Patrol garners about one enemy unit — its historical designation,
/// type, whether it is motorized, and its reported TOE Strength (the defender's chosen value within
/// ±2 of actual). A one-time snapshot; not stored in game state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevealedUnit {
    pub unit: UnitId,
    pub designation: String,
    pub unit_type: String,
    pub motorized: bool,
    pub reported_strength: i32,
}

/// §16 Patrol outcome: the patroller's losses, the reconnaissance disclosures (§16.5/§16.51), and
/// the casualties the patrol inflicted on the reconnoitred hex (§16.8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// §17.5 a stack voluntarily surrendered; `toe_lost` = total TOE Strength Points surrendered
    /// (after any §17.56 pre-capture destruction), `destroyed` = TOE/truck points self-destroyed.
    VoluntarilySurrendered { toe_lost: u32, destroyed: u32 },
    /// §17.3 a Commonwealth unit is training this stage; `basic_morale` is its Basic Morale after
    /// this stage's progression (raised toward its trained reference every six stages).
    UnitTraining { basic_morale: i32 },
    /// §12.11: an artillery unit's gun position was set (Forward/Back).
    GunPositionSet,
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
    /// An SGSU counter was deployed to an air facility (§34.82); carries the new squadron's id.
    SgsuDeployed(crate::state::air::SquadronId),
    /// A squadron was withdrawn from play to satisfy a §34.84 `WD` requirement.
    SquadronWithdrawn,
    /// A Commonwealth squadron was based in an off-map air facility (§36.53).
    SquadronBasedOffMap,
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
    /// §4.44a footnote b: the number of the 102nd's TOE points converted to Deacons.
    DeaconsRefitted(u32),
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
    /// §20.46: a Replacement-Point pool moved one hex to `hex`.
    ReplacementMoved { index: usize, hex: HexCoord },
    /// §20.44: a Replacement-Point pool was loaded onto (`convoy: Some`) or unloaded from a convoy.
    ReplacementBinding { pool: usize, convoy: Option<usize> },
    /// §24.9: a Supply Dump was constructed at `hex` (`dummy` = a §32.18 decoy).
    SupplyDumpConstructed { hex: HexCoord, dummy: bool },
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
    /// §19.8: AT TOE points added to a Brigade-level HQ (ad-hoc anti-tank battery).
    AntiTankAugmented(u32),
    /// §12.5: an artillery barrage against a facility resolved with this raw table result.
    FacilityBarrage { result: i32 },
    /// §21.6: a Broken Down Vehicle marker was towed one hex.
    BrokenVehicleTowed,
    /// §20.81: a by-type Mandatory Withdrawal choice was satisfied (units withdrawn).
    WithdrawalChoiceSatisfied(u32),
    /// §30.4: the Chariot raid was planned.
    ChariotRaidPlanned,
    /// §30.44: the Chariot raid was executed, inflicting `damage` Gun-Rating Damage Points.
    ChariotRaidExecuted { damage: u32 },
    /// §20.5: an armored Recce / Armored-Car unit was upgraded.
    RecceUpgraded,
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
    /// §30.51/§30.52: a Commonwealth combat unit was transferred by sea from one port to another.
    TroopsTransferred,
    /// §44.24: a resolved Axis raid on a Maltese air base (with capacity levels lost).
    MaltaBaseBombed { mission: crate::state::game::BombingMissionOutcome, levels_lost: u32 },
    /// §44.13: capacity levels built on a Maltese air facility.
    MaltaFacilityConstructed(u32),
    /// §44.18: AA points added to a Malta base by a diversion.
    MaltaAaDiverted(u32),
    /// §44.15/§44.18: flak points redistributed between two Malta fields.
    MaltaFlakMoved { light: u32, heavy: u32 },
    /// §40.24/§44: a Malta convoy-interdiction strike — `percent` of the convoy's tonnage lost and
    /// `attackers_lost` planes downed by the convoy's flak.
    ConvoyInterdicted { percent: u32, attackers_lost: u32 },
    /// §44.14: planes based onto / moved between Malta facilities.
    MaltaPlanesBased(u32),
    MaltaPlanesMoved(u32),
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
    /// §42.27: air reconnaissance revealed these enemy units' TOE Strength to the flier this stage.
    ReconRevealed { revealed: Vec<UnitId> },
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
    /// §19.8: the anti-tank augmentation is structurally illegal (not an Axis Brigade-level
    /// infantry HQ, wrong/mixed gun type, below the 3-point initial / above the 6-point cap, or
    /// insufficient co-located trained Gun Replacement Points).
    AntiTankAugmentIllegal,
    /// §19.82: augmentation is barred because some Axis anti-tank unit is below 67% of its max TOE.
    AntiTankReadinessGate,
    /// §20.5: the recce upgrade is illegal (not an eligible armored Recce/AC unit with Anti-Armor 0,
    /// already upgraded, a CW recce in an infantry division, or no co-located upgrade Replacement Point).
    RecceUpgradeIllegal,
    /// §17.5: the designated captor is not an enemy combat unit adjacent to the surrendering hex.
    InvalidCaptor,
    /// §17.54: the captor cannot expend the 2 CP required to accept a voluntary surrender.
    CaptorCannotAccept,
    /// §12.23: a specific barrage target was designated but the firer does not know it is there
    /// (it is not In-Contact/Engaged, i.e. not revealed through previous combat).
    TargetNotKnown,
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
    /// §17.3: a Training designation was illegal — not a green Commonwealth unit, not in a §17.32
    /// training area, already acted (§17.33 CP expended), or no qualifying §17.35 instruction
    /// battalion present.
    TrainingIllegal,
    /// §28: a prisoner action was illegal — no such stack, no eligible infantry unit to form/receive
    /// a guard, an illegal move hex, or insufficient guards to escort a move.
    PrisonerActionIllegal,
    /// Reaction requires an enemy combat unit to have moved adjacent (§8.51).
    NoReactionTrigger,
    /// §8.55: a reacting unit may never move into an enemy Zone of Control.
    ReactIntoZoc,
    /// §8.53b/§8.54: a faster, large-enough adjacent enemy pins the unit (no reaction).
    Pinned,
    /// §6.26: a unit with a Cohesion Level of −26 or worse may not move, attack, or defend.
    Disorganized,
    /// §15.23/§14.22: a hex may not be close-assaulted / anti-armor-fired more than once per Combat
    /// Segment.
    HexAlreadyInCombat,
    /// §12.11: only an artillery/gun unit has a Forward/Back gun position.
    NotArtillery,
    /// §13.28: a unit that Retreated Before Assault may not *direct* anti-armor fire (though it
    /// remains susceptible to it).
    CannotDirectAntiArmor,
    /// §18.22: a Reserve II unit may never move (until released in a Reserve Release Segment).
    InReserve,
    /// §8.44: a motorized unit or AFV may not Close Assault a defender in a Salt Marsh hex.
    SaltMarshAssault,
    /// §21.6: the Broken Down Vehicle marker could not be towed (bad index, non-adjacent, enemy-
    /// controlled destination, prohibited terrain, or the tow budget is exhausted).
    TowFailed,
    /// §14.54/§22.62: a Destroyed Tank could not be repaired here — no Repair Facility, and no
    /// co-located same-nationality Tank Delivery Squadron rolling a 1.
    RepairFailed,
    /// §20.81/§20.82: the by-type Mandatory Withdrawal choice could not be satisfied (bad index,
    /// already satisfied, wrong count, or a chosen unit isn't the right type / at a depot / ≥75% TOE).
    WithdrawalChoiceIllegal,
    /// §30.4: the Chariot raid cannot be planned/executed now (already used, wrong window, not yet
    /// planned, or the seven-stage delay has not elapsed).
    ChariotRaidIllegal,
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
    /// §35.2: the nationality's plane pool does not hold enough planes of that type to assign.
    InsufficientPlanePool,
    /// §40.12: the nationality's pilot pool has no unassigned pilot of that rating to assign.
    InsufficientPilotPool,
    /// §34.82: cannot deploy an SGSU — no counter available for that nationality, or no air facility
    /// at the target hex.
    SgsuDeployFailed,
    /// §34.84: the squadron cannot be withdrawn — it does not exist, is not deployed, or no matching
    /// `WD` requirement is due this Game-Turn.
    WithdrawalNotDue,
    /// §36.53: the squadron cannot be based off-map — unknown/ineligible facility, not Commonwealth,
    /// facility at capacity, time-gated, or a flying-boat-basin/plane-class mismatch.
    OffMapBasingFailed,
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
    /// §44.18: AA can't be diverted to Malta now — already diverted this month, or the 48-point cap
    /// is full.
    MaltaAaUnavailable,
    /// §44.14: the Malta facility can't hold that many more planes (levels × 18 capacity).
    MaltaCapacityExceeded,
    /// §36.3: this aircraft class may not base at this facility kind (flying-boats need a
    /// basin/alighting area; land planes need an airfield/strip).
    WrongAircraftForFacility,
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
    /// §24.9: a Supply Dump cannot be constructed here — the builder is not co-located with `hex`, or
    /// (for a real dump) is not a TOE Strength Point.
    DumpConstructionIllegal,
    /// §35.14: the SGSU lacks its own upkeep and may not repair (refit) its planes this stage.
    SgsuUnsupplied,
    /// §30.51: the unit is not at a port, the destination is not a port, or a port is inoperable.
    PortCannotTransfer,
    /// §30.52/§55.11: the port's personnel capacity can't take the unit (minor port / over capacity),
    /// or a port has already made its one transfer this Operations Stage.
    PortTransferUnavailable,
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
    /// §39: the aircraft type may not fly the requested mission (e.g. no Convoy/Land bombing
    /// capability, chart column B).
    WrongAircraftForMission,
    /// The field does not hold that many planes of the requested type.
    NotEnoughPlanes,
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
            Command::MoveReplacement { index, to } => self.cmd_move_replacement(index, to),
            Command::LoadReplacement { convoy, pool } => self.cmd_load_replacement(convoy, pool),
            Command::UnloadReplacement { pool } => self.cmd_unload_replacement(pool),
            Command::ConstructSupplyDump { unit, hex, dummy } => self.cmd_construct_supply_dump(unit, hex, dummy),
            Command::LandLayforce { unit, to, ship_hexes } => self.cmd_land_layforce(unit, to, ship_hexes),
            Command::EmbarkLayforce { unit } => self.cmd_embark_layforce(unit),
            Command::LandSas { unit, to, ship_hexes } => self.cmd_land_sas(unit, to, ship_hexes),
            Command::RaidAirfieldSas { unit } => { let roll = self.rng.d6() as u32; self.cmd_raid_airfield_sas(unit, roll) }
            Command::FormRaider { kind, from_unit } => self.cmd_form_raider(kind, from_unit),
            Command::MoveRaider { raider, to } => { let spot_roll = self.rng.d6() as u32; self.cmd_move_raider(raider, to, spot_roll) }
            Command::SpotRaider { raider } => { let roll = self.rng.d6() as u32; self.cmd_spot_raider(raider, roll) }
            Command::ResupplyRaider { raider } => self.cmd_resupply_raider(raider),
            Command::RaidRommel { raider } => { let roll = (self.rng.d6() + self.rng.d6()) as u32; self.cmd_raid_rommel(raider, roll) }
            Command::RaidPipeline { raider } => { let roll = self.rng.d6() as u32; self.cmd_raid_pipeline(raider, roll) }
            Command::RaidAirfield { raider } => { let roll = self.rng.d6() as u32; self.cmd_raid_airfield(raider, roll) }
            Command::RaidPlanes { raider } => { let roll = self.rng.d6() as u32; self.cmd_raid_planes(raider, roll) }
            Command::RaidSupplyDump { raider } => {
                let attack_roll = (self.rng.d6() + self.rng.d6()) as u32;
                let destroy_roll = self.rng.d6() as u32;
                self.cmd_raid_supply_dump(raider, attack_roll, destroy_roll)
            }
            Command::RaidTruckConvoy { raider, convoy } => { let roll = self.rng.d6() as u32; self.cmd_raid_truck_convoy(raider, convoy, roll) }
            Command::InterceptTruckConvoy { raider, convoy } => { let roll = self.rng.d6() as u32; self.cmd_intercept_truck_convoy(raider, convoy, roll) }
            Command::FormationMove { formation, to, mode } => {
                self.formation_move(formation, to, mode)
            }
            Command::Barrage { attackers, target, target_class, target_unit } => {
                self.cmd_barrage(&attackers, target, target_class, target_unit)
            }
            Command::AntiArmor { attackers, target_hex, withheld } => {
                self.cmd_anti_armor(&attackers, target_hex, &withheld)
            }
            Command::CloseAssault { attackers, target } => {
                self.cmd_close_assault(&attackers, target)
            }
            Command::Retreat { unit, path } => self.cmd_retreat(unit, &path),
            Command::React { unit, path } => self.cmd_react(unit, &path),
            Command::RetreatBeforeAssault { unit, path } => self.cmd_retreat_before_assault(unit, &path),
            Command::Patrol {
                patrol_points, target, exclude, reveals, loss_choice,
            } => {
                let survival_die = self.rng.d6() as u32;
                let recon_die = self.rng.d6() as u32;
                let loss_die = self.rng.d6() as u32;
                self.cmd_patrol(&patrol_points, target, &exclude, &reveals, &loss_choice, survival_die, recon_die, loss_die)
            }
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
            Command::TrainUnit { unit } => self.cmd_train_unit(unit),
            Command::SetGunPosition { unit, forward } => self.cmd_set_gun_position(unit, forward),
            Command::MoveOffMap { unit, direction } => self.cmd_move_off_map(unit, direction),
            Command::PlanConvoy {
                lane, destination, arrival_gt, fuel_tons, ammo_tons, stores_tons, replacement_tons,
            } => {
                let die = self.rng.d6() as u32;
                self.cmd_plan_convoy(lane, destination, arrival_gt, fuel_tons, ammo_tons, stores_tons, replacement_tons, die)
            }
            Command::BombConvoy { convoy_index, bomb_points, torpedo_planes, total_planes } => {
                let roll = self.rng.two_dice_raw() as u32;
                self.cmd_bomb_convoy(convoy_index, bomb_points, torpedo_planes, total_planes, roll)
            }
            Command::InterdictConvoy { base_index, aircraft_type, planes, torpedo, convoy_index } => {
                self.cmd_interdict_convoy(base_index, &aircraft_type, planes, torpedo, convoy_index)
            }
            Command::DeliverConvoy { convoy_index } => self.cmd_deliver_convoy(convoy_index),
            Command::PlanProduction { nationality, item, count } => {
                self.cmd_plan_production(nationality, &item, count)
            }
            Command::ProduceCommonwealthTrucks {} => {
                let light_die = self.rng.d6() as u32;
                let medium_die = self.rng.d6() as u32;
                let heavy_die = self.rng.d6() as u32;
                self.cmd_produce_commonwealth_trucks(light_die, medium_die, heavy_die)
            }
            Command::ProduceCommonwealthInfantry {} => {
                let roll = (self.rng.d6() + self.rng.d6()) as u32;
                self.cmd_produce_commonwealth_infantry(roll)
            }
            Command::ProduceAxisTrucks { kind, count } => self.cmd_produce_axis_trucks(kind, count),
            Command::BlockHarbor { port, has_engineer } => self.cmd_block_harbor(&port, has_engineer),
            Command::UnblockHarbor { port, engineers } => self.cmd_unblock_harbor(&port, engineers),
            Command::DeploySgsu { nationality, class, at } => {
                self.require_phase(Phase::Organization)?;
                match self.deploy_sgsu(nationality, class, at) {
                    Some(id) => Ok(CommandOutcome::SgsuDeployed(id)),
                    None => Err(CommandError::SgsuDeployFailed),
                }
            }
            Command::WithdrawSquadron { squadron } => {
                self.require_phase(Phase::Organization)?;
                if self.withdraw_squadron(squadron) {
                    Ok(CommandOutcome::SquadronWithdrawn)
                } else {
                    Err(CommandError::WithdrawalNotDue)
                }
            }
            Command::BaseSquadronOffMap { squadron, facility } => {
                self.require_phase(Phase::Organization)?;
                if self.base_squadron_off_map(squadron, &facility) {
                    Ok(CommandOutcome::SquadronBasedOffMap)
                } else {
                    Err(CommandError::OffMapBasingFailed)
                }
            }
            Command::AssignSquadronPlanes { squadron, aircraft_type, class, count } => {
                self.cmd_assign_squadron_planes(squadron, &aircraft_type, class, count)
            }
            Command::BombAirFacility { hex, kind, levels } => {
                self.cmd_bomb_air_facility(hex, kind, levels)
            }
            Command::LayHarborMine { port } => self.cmd_lay_harbor_mine(&port),
            Command::RemoveHarborMine { port } => { let die = self.rng.d6() as u32; self.cmd_remove_harbor_mine(&port, die) }
            Command::ReconConvoyLane { lane, planes } => {
                let roll = self.rng.two_dice_raw() as u32;
                self.cmd_recon_convoy_lane(lane, planes, roll)
            }
            Command::DetermineMaltaAvailability { level } => {
                let roll = (self.rng.d6() + self.rng.d6()) as u32;
                self.cmd_determine_malta_availability(level, roll)
            }
            Command::RefitSquadron { squadron, wrong_squadron } => {
                self.cmd_refit_squadron(squadron, wrong_squadron)
            }
            Command::LoadTrucks { unit, commodity, amount } => {
                self.require_phase(Phase::Organization)?;
                self.require_units_exist(&[unit])?;
                // §6.3/§53.24: loading/off-loading during the Organization Phase costs 0 CP (the
                // Supply-Distribution free-load exception); a CP cost applies only in other phases.
                Ok(CommandOutcome::TrucksLoaded(self.load_trucks(unit, commodity, amount)))
            }
            Command::UnloadTrucks { unit, commodity, amount } => {
                self.require_phase(Phase::Organization)?;
                self.require_units_exist(&[unit])?;
                Ok(CommandOutcome::TrucksUnloaded(self.unload_trucks(unit, commodity, amount)))
            }
            Command::LoadConvoy { convoy, commodity, amount } => {
                self.require_phase(Phase::TruckConvoy)?;
                let side = self.truck_convoys.get(convoy).map(|c| c.side).ok_or(CommandError::TruckConvoyCannotMove)?;
                if self.active_player != side {
                    return Err(CommandError::WrongPhase);
                }
                Ok(CommandOutcome::TrucksLoaded(self.load_convoy(convoy, commodity, amount)))
            }
            Command::UnloadConvoy { convoy, commodity, amount } => {
                self.require_phase(Phase::TruckConvoy)?;
                let side = self.truck_convoys.get(convoy).map(|c| c.side).ok_or(CommandError::TruckConvoyCannotMove)?;
                if self.active_player != side {
                    return Err(CommandError::WrongPhase);
                }
                Ok(CommandOutcome::TrucksUnloaded(self.unload_convoy(convoy, commodity, amount)))
            }
            Command::RepairConvoyTrucks { convoy } => {
                self.require_phase(Phase::Repair)?;
                let side = self.truck_convoys.get(convoy).map(|c| c.side).ok_or(CommandError::TruckConvoyCannotMove)?;
                if self.active_player != side {
                    return Err(CommandError::WrongPhase);
                }
                let roll = self.rng.d6();
                Ok(CommandOutcome::Repaired(self.repair_convoy_trucks(convoy, roll)))
            }
            Command::AssignTrucks { unit, light, medium, heavy } => {
                self.cmd_assign_trucks(unit, light, medium, heavy)
            }
            Command::ReleaseTrucks { unit, light, medium, heavy } => {
                self.cmd_release_trucks(unit, light, medium, heavy)
            }
            Command::PoisonWell { hex } => {
                self.require_phase(Phase::MovementAndCombat)?;
                let die = self.rng.d6();
                Ok(CommandOutcome::WellPoisoned(self.poison_well(hex, die)))
            }
            Command::SweetenWell { hex } => {
                self.require_phase(Phase::MovementAndCombat)?;
                let die = self.rng.d6();
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
                if self.storm_at(hex) {
                    return Err(CommandError::ConstructionFailed); // §29.42/§29.54
                }
                if !self.charge_construction_water(hex, &builders) {
                    return Err(CommandError::ConstructionFailed); // §24.21 hot-weather water
                }
                if self.begin_fortification(&builders, hex, level) {
                    Ok(CommandOutcome::ConstructionStarted)
                } else {
                    Err(CommandError::ConstructionFailed)
                }
            }
            Command::BuildRailroad { builders, a, b } => {
                self.require_phase(Phase::Organization)?;
                self.require_units_exist(&builders)?;
                if self.storm_at(a) || self.storm_at(b) {
                    return Err(CommandError::ConstructionFailed); // §29.42/§29.54
                }
                if !self.charge_construction_water(a, &builders) {
                    return Err(CommandError::ConstructionFailed); // §24.21 hot-weather water
                }
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
                if self.storm_at(a) || self.storm_at(b) {
                    return Err(CommandError::ConstructionFailed); // §29.42/§29.54
                }
                if !self.charge_construction_water(a, &[builder]) {
                    return Err(CommandError::ConstructionFailed); // §24.21 hot-weather water
                }
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
            Command::RefitDeacons { unit } => {
                self.require_phase(Phase::Organization)?;
                if self.active_player != Side::Commonwealth {
                    return Err(CommandError::WrongPhase);
                }
                self.require_units_exist(&[unit])?;
                match self.refit_deacons(unit) {
                    Some(n) => Ok(CommandOutcome::DeaconsRefitted(n)),
                    None => Err(CommandError::ReplacementFailed),
                }
            }
            Command::SiphonFuel { donor, receiver, amount } => {
                self.require_phase(Phase::Organization)?;
                self.require_units_exist(&[donor, receiver])?;
                Ok(CommandOutcome::FuelSiphoned(self.siphon_fuel(donor, receiver, amount)))
            }
            Command::Repair { unit, kind } => {
                self.require_phase(Phase::Repair)?;
                self.require_units_exist(&[unit])?;
                let roll = self.rng.d6(); // §22 field-repair die, rolled inside the boundary
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
                // §18.23/§18.24: releasing a unit from Reserve (to None) marks it so this stage's
                // release restrictions (CPA cap, one assault, §18.24.3 DP) apply.
                use crate::state::unit::ReserveStatus;
                if status == ReserveStatus::None && u.reserve != ReserveStatus::None {
                    u.released_from_reserve = Some(u.reserve);
                }
                u.reserve = status;
                Ok(CommandOutcome::ReserveDesignated)
            }
            Command::BarrageFacility { barragers, target, facility } => {
                self.cmd_barrage_facility(&barragers, target, facility)
            }
            Command::TowBrokenVehicle { marker, to } => {
                self.require_phase(Phase::Repair)?;
                if self.tow_broken_vehicle(marker, to) {
                    Ok(CommandOutcome::BrokenVehicleTowed)
                } else {
                    Err(CommandError::TowFailed)
                }
            }
            Command::SatisfyWithdrawalChoice { choice, units } => {
                self.require_phase(Phase::Organization)?;
                self.satisfy_withdrawal_choice(choice, &units).map(CommandOutcome::WithdrawalChoiceSatisfied)
            }
            Command::TowDestroyedTank { equipment, to } => {
                self.require_phase(Phase::Repair)?;
                if self.tow_destroyed_tank(equipment, to) {
                    Ok(CommandOutcome::BrokenVehicleTowed)
                } else {
                    Err(CommandError::TowFailed)
                }
            }
            Command::PlanChariotRaid => {
                if self.active_player != Side::Axis {
                    return Err(CommandError::WrongPhase);
                }
                self.plan_chariot_raid().map(|()| CommandOutcome::ChariotRaidPlanned)
            }
            Command::ExecuteChariotRaid => {
                if self.active_player != Side::Axis {
                    return Err(CommandError::WrongPhase);
                }
                self.execute_chariot_raid().map(|damage| CommandOutcome::ChariotRaidExecuted { damage })
            }
            Command::VoluntarySurrender { hex, captor, destroy_material } => {
                self.cmd_voluntary_surrender(hex, captor, destroy_material)
            }
            Command::AugmentAntiTank { hq, gun_system, count } => {
                self.require_phase(Phase::Organization)?;
                self.augment_anti_tank(hq, &gun_system, count)
                    .map(CommandOutcome::AntiTankAugmented)
            }
            Command::AugmentBattalionAntiTank { battalion, gun_system, count } => {
                self.require_phase(Phase::Organization)?;
                self.augment_battalion_anti_tank(battalion, &gun_system, count)
                    .map(CommandOutcome::AntiTankAugmented)
            }
            Command::UpgradeRecce { unit } => {
                self.require_phase(Phase::Organization)?;
                self.upgrade_recce(unit).map(|()| CommandOutcome::RecceUpgraded)
            }
            Command::AssignShip { ship_index, location } => {
                self.require_phase(Phase::CommonwealthFleet)?;
                self.cmd_assign_ship(ship_index, location)
            }
            Command::NavalTransferTroops { unit, to_port } => self.cmd_naval_transfer_troops(unit, &to_port),
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
                squadron, planes, bombload_per_plane, base_index, interceptors, flak_suppression,
            } => {
                self.require_phase(Phase::MaltaRaid)?;
                if base_index >= self.malta_bases.len() {
                    return Err(CommandError::NoSuchMaltaBase);
                }
                if !self.sgsus.contains_key(&squadron) {
                    return Err(CommandError::NoSuchSquadron);
                }
                // §39.3x/§41.3: flak (two rolls) + Air-Bombardment dice rolled inside the boundary.
                let flak_destroy_roll = self.rng.sequential_read() as u32;
                let flak_abort_roll = self.rng.sequential_read() as u32;
                let bomb_roll = self.rng.sequential_read() as u32;
                // §44.24: the raid draws only the *raided* field's diverted flak (§44.18). §40.74:
                // escorting fighters suppress one Light point per three planes; Heavy flak is never
                // suppressed. The intrinsic §36.18 AA point fires only vs strafing/dive-bombing, so it
                // does not add to this level-bombing (AirfieldPortStrip) raid.
                let base = &self.malta_bases[base_index];
                let suppressed = (flak_suppression / 3).min(base.flak_light);
                let flak_points = base.flak_heavy + (base.flak_light - suppressed);
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
            Command::ConstructMaltaFacility { base_index } => {
                self.require_phase(Phase::Organization)?;
                let die = self.rng.d6() as u32;
                let levels = self
                    .reference
                    .malta_construction
                    .as_ref()
                    .ok_or(CommandError::NoTable)?
                    .levels_built(die);
                let base = self.malta_bases.get_mut(base_index).ok_or(CommandError::NoSuchMaltaBase)?;
                Ok(CommandOutcome::MaltaFacilityConstructed(base.construct(levels)))
            }
            Command::DivertMaltaAA { base_index, heavy } => {
                self.require_phase(Phase::Organization)?;
                if self.active_player != Side::Commonwealth {
                    return Err(CommandError::WrongPhase);
                }
                if base_index >= self.malta_bases.len() {
                    return Err(CommandError::NoSuchMaltaBase);
                }
                let d = crate::state::game::date_of(self.game_turn);
                let month_key = d.year * 12 + d.month;
                if self.malta_aa_diverted_month == Some(month_key) {
                    return Err(CommandError::MaltaAaUnavailable); // §44.18: one diversion per month
                }
                // §44.18: the 48-point cap is Malta-wide (sum of every field's light + heavy).
                let total: u32 = self.malta_bases.iter().map(|b| b.flak_total()).sum();
                let room = crate::state::air::MaltaAirBase::MAX_AA_POINTS.saturating_sub(total);
                if room == 0 {
                    return Err(CommandError::MaltaAaUnavailable); // §44.18: 48-point Malta cap
                }
                // §44.18: consume one Commonwealth AA Replacement Point of the matching type from the
                // pool (Light or Heavy). Each Light → 1 Malta AA point, each Heavy → 4.
                let want = if heavy {
                    crate::state::game::GunSub::HeavyAa
                } else {
                    crate::state::game::GunSub::LightAa
                };
                let src = self.replacement_points.iter_mut().find(|p| {
                    p.side == Side::Commonwealth
                        && p.class == crate::state::game::ReplacementClass::Gun
                        && p.sub == Some(want)
                        && p.count > 0
                });
                let Some(src) = src else {
                    return Err(CommandError::MaltaAaUnavailable); // no AA replacement point to divert
                };
                src.count -= 1;
                let add = if heavy { 4 } else { 1 };
                let added = add.min(room);
                if heavy {
                    self.malta_bases[base_index].flak_heavy += added;
                } else {
                    self.malta_bases[base_index].flak_light += added;
                }
                self.malta_aa_diverted_month = Some(month_key);
                self.replacement_points.retain(|p| p.count > 0);
                Ok(CommandOutcome::MaltaAaDiverted(added))
            }
            Command::MoveMaltaFlak { from_base, to_base, light, heavy } => {
                self.require_phase(Phase::Organization)?;
                if self.active_player != Side::Commonwealth {
                    return Err(CommandError::WrongPhase);
                }
                let n = self.malta_bases.len();
                if from_base >= n || to_base >= n || from_base == to_base {
                    return Err(CommandError::NoSuchMaltaBase);
                }
                if light == 0 && heavy == 0 {
                    return Err(CommandError::MaltaAaUnavailable); // nothing to move
                }
                // §44.15/§44.18: a free redistribution — only limited by what the source field holds.
                let src = &self.malta_bases[from_base];
                if src.flak_light < light || src.flak_heavy < heavy {
                    return Err(CommandError::MaltaAaUnavailable);
                }
                self.malta_bases[from_base].flak_light -= light;
                self.malta_bases[from_base].flak_heavy -= heavy;
                self.malta_bases[to_base].flak_light += light;
                self.malta_bases[to_base].flak_heavy += heavy;
                Ok(CommandOutcome::MaltaFlakMoved { light, heavy })
            }
            Command::BaseMaltaPlanes { base_index, aircraft_type, count } => {
                self.cmd_base_malta_planes(base_index, &aircraft_type, count)
            }
            Command::MoveMaltaPlanes { from_base, to_base, aircraft_type, count } => {
                self.cmd_move_malta_planes(from_base, to_base, &aircraft_type, count)
            }
            Command::BombShip {
                squadron, planes, bombload_per_plane, ship_index, interceptors, interceptor_squadron,
                flak_points,
            } => {
                self.require_phase(Phase::LandSupportAir)?;
                if ship_index >= self.fleet.len() {
                    return Err(CommandError::NoSuchShip);
                }
                if !self.sgsus.contains_key(&squadron) {
                    return Err(CommandError::NoSuchSquadron);
                }
                // §39.3x/§41.3: flak (two rolls) + Air-Bombardment dice rolled inside the boundary.
                let flak_destroy_roll = self.rng.sequential_read() as u32;
                let flak_abort_roll = self.rng.sequential_read() as u32;
                let bomb_roll = self.rng.sequential_read() as u32;
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
                flak_points, night,
            } => {
                self.require_phase(Phase::LandSupportAir)?;
                if !self.sgsus.contains_key(&squadron) {
                    return Err(CommandError::NoSuchSquadron);
                }
                let group = crate::rules::air::bombing_target_group(mission)
                    .ok_or(CommandError::NoTable)?; // mining harbours use their own table
                // §39.3x/§41.3: flak (two rolls) + Air-Bombardment dice rolled inside the boundary.
                let flak_destroy_roll = self.rng.sequential_read() as u32;
                let flak_abort_roll = self.rng.sequential_read() as u32;
                let bomb_roll = self.rng.sequential_read() as u32;
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
            Command::ReconHex { squadron, target, planes } => {
                let die = self.rng.d6() as u32; // §42.27 recon die, rolled inside the boundary
                self.cmd_recon_hex(squadron, target, planes, die)
            }
            Command::DesignateCap { squadron, center, cap_type, planes } => {
                self.cmd_designate_cap(squadron, center, cap_type, planes)
            }
            Command::Strafe { squadron, target, strafe_target, planes } => {
                let roll = self.rng.sequential_read() as u32; // §40.8 2d6, rolled inside the boundary
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
            } => {
                // §39.3x/§41.3: the flak (two rolls) and Air-Bombardment dice are rolled inside the
                // boundary so an agent cannot supply favourable values.
                let flak_destroy_roll = self.rng.sequential_read() as u32;
                let flak_abort_roll = self.rng.sequential_read() as u32;
                let bomb_roll = self.rng.sequential_read() as u32;
                self.cmd_fly_bombing_mission_at(
                    squadron, target, mission, planes, bombload_per_plane, night, escort, abort_planes,
                    flak_destroy_roll, flak_abort_roll, bomb_roll,
                )
            }
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

    /// §30.51/§30.52 NavalTransferTroops: move a Commonwealth combat unit by sea from the port it
    /// occupies to `to_port` (Truck Convoy Movement Phase). Only the Commonwealth ships troops by
    /// sea (§30.5). Both ports must be operable; the destination must be able to receive men (a
    /// major port, `stacking_in > 0`) and the origin to ship them out, with the unit's Stacking
    /// Points fitting within each port's personnel capacity (§30.52/§55.11/§55.13); and neither port
    /// may already have made its one transfer this Operations Stage (§30.52). (§30.53 supply-shipment
    /// interaction is deferred.)
    fn cmd_naval_transfer_troops(&mut self, unit: UnitId, to_port: &str) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::TruckConvoy)?;
        let side = self.unit(unit).ok_or(CommandError::UnitNotFound)?.nationality.side();
        if side != Side::Commonwealth || !self.is_combat_unit(unit) {
            return Err(CommandError::PortCannotTransfer);
        }
        let hex = self.effective_hex(unit).ok_or(CommandError::PortCannotTransfer)?;
        // Origin = the port whose hex the unit currently occupies.
        let port_names: Vec<String> = self.ports.keys().cloned().collect();
        let origin = port_names
            .into_iter()
            .find(|n| self.port_hex(n) == Some(hex))
            .ok_or(CommandError::PortCannotTransfer)?;
        if origin == to_port {
            return Err(CommandError::PortCannotTransfer);
        }
        let dest_hex = self.port_hex(to_port).ok_or(CommandError::PortCannotTransfer)?;
        // §30.5/§55: troops may only be shipped through friendly ports — the destination must not be
        // held by an enemy combat unit (you can't land men in an enemy-garrisoned port, e.g. Tobruk
        // while the Axis holds it).
        let enemy_holds = self
            .units_in_hex(dest_hex)
            .into_iter()
            .any(|id| self.unit(id).is_some_and(|u| u.nationality.side() != side && u.total_toe() > 0));
        if enemy_holds {
            return Err(CommandError::PortCannotTransfer);
        }
        // Copy out the port capacities/operability (scalars) so the ports borrow ends before mutation.
        let (origin_out, origin_op) = {
            let p = self.ports.get(&origin).ok_or(CommandError::PortCannotTransfer)?;
            (p.stacking_out.unwrap_or(0), p.is_operable())
        };
        let (dest_in, dest_op) = {
            let p = self.ports.get(to_port).ok_or(CommandError::PortCannotTransfer)?;
            (p.stacking_in.unwrap_or(0), p.is_operable())
        };
        if !origin_op || !dest_op {
            return Err(CommandError::PortCannotTransfer);
        }
        let sp = self.effective_stacking_points(unit);
        // §55.11: a minor port (stacking_in 0) can't receive men; §30.52/§55.13 capacity must fit.
        if dest_in == 0 || dest_in < sp || origin_out < sp {
            return Err(CommandError::PortTransferUnavailable);
        }
        // §30.52: one transfer per port per Operations Stage (either end).
        if self.ports_transferred_this_stage.contains(&origin) || self.ports_transferred_this_stage.contains(to_port) {
            return Err(CommandError::PortTransferUnavailable);
        }
        self.unit_mut(unit).expect("unit exists").location = Some(crate::state::unit::UnitLocation::OnMap(dest_hex));
        self.ports_transferred_this_stage.insert(origin);
        self.ports_transferred_this_stage.insert(to_port.to_string());
        Ok(CommandOutcome::TroopsTransferred)
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

    /// §40.24/§44: a Malta-based convoy-interdiction strike. Draws real planes from a Malta field,
    /// range-gates via the §37.4 Malta distance to the convoy's destination, resolves the attack on
    /// the Axis-Naval-Convoy CRT, then applies the convoy's flak (§56.16) as attacker losses removed
    /// from the field. Malta planes auto-refuel/rearm (§44.16), so no supply is expended and
    /// survivors stay based (the strike is "outside" the sequence like §44.15 moves).
    fn cmd_interdict_convoy(
        &mut self,
        base_index: usize,
        aircraft_type: &str,
        planes: u32,
        torpedo: bool,
        convoy_index: usize,
    ) -> Result<CommandOutcome, CommandError> {
        use crate::reference::aircraft::{Mission, MissionCap};
        use crate::reference::units::Nationality;
        self.require_phase(Phase::ConvoyResolution)?;
        if self.active_player != Side::Commonwealth {
            return Err(CommandError::WrongPhase);
        }
        if planes == 0 {
            return Err(CommandError::NotEnoughPlanes);
        }
        // Valid Malta field holding enough planes of the type.
        let base = self.malta_bases.get(base_index).ok_or(CommandError::NoSuchMaltaBase)?;
        let have = base.planes.iter().find(|(t, _)| t == aircraft_type).map(|(_, n)| *n).unwrap_or(0);
        if have < planes {
            return Err(CommandError::NotEnoughPlanes);
        }
        // Valid, live convoy.
        let convoy = self.convoys.get(convoy_index).ok_or(CommandError::InvalidConvoy)?;
        if convoy.cancelled {
            return Err(CommandError::InvalidConvoy);
        }
        let destination = convoy.destination.clone();
        // §39: the aircraft type must have Convoy/Land bombing capability (chart column B), and — for
        // a torpedo run — a torpedo rating. Look up the CW Aircraft Characteristics.
        let ac = self
            .reference
            .aircraft_charts
            .get(&Nationality::Commonwealth)
            .and_then(|c| c.get(aircraft_type))
            .ok_or(CommandError::WrongAircraftForMission)?;
        if ac.missions.cap_for(Mission::ConvoyLandBombing) == MissionCap::No {
            return Err(CommandError::WrongAircraftForMission);
        }
        let loadout = ac.loadouts.first().ok_or(CommandError::WrongAircraftForMission)?;
        let bombload = loadout.bombload;
        let torp = loadout.torpedo;
        if torpedo && torp == 0 {
            return Err(CommandError::WrongAircraftForMission);
        }
        // §37.4/§44.17: the strike must be within the aircraft's range of the convoy's destination.
        let dist = self.malta_flight_distance(&destination).ok_or(CommandError::TargetOutOfRange)?;
        if dist > ac.max_range() {
            return Err(CommandError::TargetOutOfRange);
        }
        // Resolve on the Axis-Naval-Convoy CRT: a torpedo run uses torpedo planes; else bombload
        // points across the striking planes.
        let (bomb_points, torpedo_planes) = if torpedo { (0, planes) } else { (planes * bombload, 0) };
        let roll = self.rng.two_dice_raw() as u32;
        let percent = self
            .resolve_convoy_bombing(convoy_index, bomb_points, torpedo_planes, planes, roll)
            .ok_or(CommandError::InvalidConvoy)?;
        // §56.16: the convoy's flak fires at the attackers; losses are removed from the field.
        let flak = self.convoys.get(convoy_index).map(|c| c.flak_points()).unwrap_or(0);
        let mut attackers_lost = 0u32;
        if flak > 0 {
            if let Some(aa) = self.reference.anti_aircraft.as_ref() {
                use crate::reference::anti_aircraft::AaSection;
                let flak_roll = self.rng.sequential_read() as u32;
                attackers_lost = aa.result(AaSection::OtherDestroyed, flak, 0, flak_roll).max(0) as u32;
            }
        }
        attackers_lost = attackers_lost.min(planes);
        if attackers_lost > 0 {
            self.malta_bases[base_index].remove_planes(aircraft_type, attackers_lost);
        }
        Ok(CommandOutcome::ConvoyInterdicted { percent, attackers_lost })
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
    /// Training. Each Axis order's convoy tonnage is cross-counted against the arrival turn's
    /// Shipping allowance (§20.62, via `axis_shipping_committed` — shared with Axis truck production
    /// and supply convoys); Commonwealth production lands via the strategic supply line and is not
    /// gated.
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
        let idx = self.plan_replacement(class, count, nationality.side(), arrival_game_turn, self.operations_stage as u32, location, None);
        // §20.62: record the Axis convoy tonnage on the pending pool so an over-committed arrival
        // can be deferred against the turn's actual rolled allowance (§20.64).
        self.pending_replacements[idx].tonnage = tonnage;
        // §20.3/§44.18: tag the Gun sub-type (Light/Heavy AA, ParaArt, Lt AT) so the arrived pool
        // feeds Malta AA diversion and pins airborne rebuilds.
        self.pending_replacements[idx].sub = crate::state::game::replacement_gun_sub_for_item(item);
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
                        ReplacementClass::Infantry, stage_share, Side::Commonwealth, arrival_game_turn, s, location.clone(), None,
                    );
                }
            }
        }
        Ok(CommandOutcome::InfantryProduced { arrival_game_turn, points })
    }

    /// §20.66a ProduceAxisTrucks: plan `count` Truck Points of `kind`, validated against the Axis
    /// Truck Production row's lifetime total and per-Game-Turn maximum. On success the trucks
    /// arrive two Game-Turns later (§20.21 Axis delay) into the Axis free-truck pool. Gated to the
    /// Organization Phase. The order's convoy tonnage is cross-counted against the arrival turn's
    /// §20.62 Shipping allowance (via `axis_shipping_committed`, shared with replacement and supply
    /// convoys).
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
    /// Movement CPs of a passing convoy may halt and attack it (§27.58 mechanics). Range is the
    /// true "within 4 Movement CPs" — a cost-limited reach over the raider's (motorized) movement,
    /// paying real terrain costs, not a flat hex distance.
    fn cmd_intercept_truck_convoy(&mut self, raider: usize, convoy: usize, roll: u32) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::TruckConvoy)?;
        let r = self.raiders.get(raider).ok_or(CommandError::RaiderNotAvailable)?;
        let rhex = r.hex.ok_or(CommandError::RaiderNotAvailable)?;
        let rside = r.side;
        let c = self.truck_convoys.get(convoy).ok_or(CommandError::RaiderCannotMove)?;
        let chex = c.hex;
        // The convoy must be the enemy's and within 4 Movement CPs of the raider (§27.59).
        if c.side != rside.opponent() || !self.within_cp_budget(rhex, chex, MoveMode::Motorized, 40) {
            return Err(CommandError::RaiderCannotMove);
        }
        self.resolve_truck_convoy_raid(raider, convoy, roll)
    }

    /// Shared §27.58 truck-convoy raid resolution (used by both the direct raid and the §27.59
    /// interception): 5 CP; a d6 1-2 destroys one Truck Point + its cargo share. §27.58 replacement
    /// cargo: if *any* truck carries Replacement Points the raider adds 1 to the roll and a resulting
    /// 6 eliminates the raider; if *all* trucks carry Replacement Points (the convoy hauls
    /// Replacement Points and no commodity cargo) the raider is automatically eliminated with no
    /// truck losses.
    fn resolve_truck_convoy_raid(&mut self, raider: usize, convoy: usize, roll: u32) -> Result<CommandOutcome, CommandError> {
        use crate::state::game::Commodity;
        let carries_repl = self.convoy_carries_replacements(convoy);
        // §27.58: "all trucks carrying Replacement Points" — the convoy hauls replacements and has
        // no remaining commodity cargo.
        let all_repl = carries_repl && self.truck_convoys[convoy].cargo.is_empty();
        let mut eliminated = self.raider_pay_raid_cp(raider);
        if all_repl {
            if !eliminated {
                self.eliminate_raider(raider);
                eliminated = true;
            }
            return Ok(CommandOutcome::RaidResolved { success: false, raider_eliminated: eliminated });
        }
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
            // §27.58: if that was the convoy's last Truck Point, any hauled pool can no longer ride —
            // drop it back onto the hex so it becomes a standalone (stranded) counter again.
            let c = &self.truck_convoys[convoy];
            if c.trucks.light + c.trucks.medium + c.trucks.heavy == 0 {
                self.drop_convoy_replacements(convoy);
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
            trucks_broken: crate::state::unit::TruckPoints { light: 0, medium: 0, heavy: 0 }, breakdown_points: 0, breakdown_checked_col: -1,
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
        let cpa_used = c.cpa_used;
        // §21.21/24 + §53.22: the entered hex's Breakdown Points accrue to the convoy; a breakdown
        // check (truck BAR 2L) then loses a proportion of its Truck Points.
        let hex_bp = self
            .map
            .terrain(to)
            .and_then(|t| self.reference.terrain.get(&t))
            .map(|p| p.breakdown.0)
            .unwrap_or(0);
        self.truck_convoys[convoy].breakdown_points += hex_bp.max(0) as u32;
        self.resolve_convoy_breakdown(convoy);
        Ok(CommandOutcome::TruckConvoyMoved { hex: to, cpa_used })
    }

    /// §20.46: move an on-map Replacement-Point pool one hex toward the front (Truck Convoy Phase),
    /// as a truck convoy would. §20.42: it may not voluntarily enter an enemy ZOC unless a Friendly
    /// unit already holds the destination. Terrain must be enterable (motorized step cost). The
    /// per-pool CPA budget and §20.45 Stores/Fuel upkeep of the haul are deferred.
    /// Tenths of infantry TOE Strength Points a truck convoy can lift (§54.2 = Σ trucks × per-point
    /// infantry capacity), used to decide whether it can haul a Replacement-Point pool (§20.44).
    fn convoy_inf_capacity_tenths(&self, convoy: &crate::state::game::TruckConvoy) -> u32 {
        let Some(chart) = self.reference.truck_chart.as_ref() else {
            return 0;
        };
        let t = &convoy.trucks;
        t.light * chart.light.toe_inf_tenths.unwrap_or(0)
            + t.medium * chart.medium.toe_inf_tenths.unwrap_or(0)
            + t.heavy * chart.heavy.toe_inf_tenths.unwrap_or(0)
    }

    fn cmd_move_replacement(&mut self, index: usize, to: HexCoord) -> Result<CommandOutcome, CommandError> {
        use crate::reference::terrain::MoveCost;
        use crate::rules::movement::{step_cost, MoveMode};
        use crate::state::game::{Commodity, ReplacementClass};
        self.require_phase(Phase::TruckConvoy)?;
        let p = self.replacement_points.get(index).ok_or(CommandError::ReplacementFailed)?;
        let side = p.side;
        if self.active_player != side {
            return Err(CommandError::WrongPhase);
        }
        let from = p.location.as_ref().and_then(|l| l.hex()).ok_or(CommandError::ReplacementFailed)?;
        // §20.42d/§20.46: a pool moves "for purposes of assignment"; its CPA is that of its assigned unit.
        let assigned = p.assigned_to.ok_or(CommandError::ReplacementFailed)?;
        let pool_cpa_used = p.cpa_used;
        let class = p.class;
        let lift_tenths = p.count * 10; // §20.45: 1 Replacement Point ≈ 1 TOE Strength Point to lift
        if !self.map.neighbors(from).any(|n| n == to) {
            return Err(CommandError::ReplacementFailed);
        }
        // §20.42a: no voluntary entry into an enemy ZOC unless a Friendly unit already holds the hex.
        let enemy = side.opponent();
        let friendly = self
            .units_in_hex(to)
            .into_iter()
            .any(|x| self.unit(x).is_some_and(|u| u.nationality.side() == side));
        if self.hex_in_zoc_of(to, enemy) && !friendly {
            return Err(CommandError::ReplacementFailed);
        }
        // Motorized truck-convoy step cost (§20.46), in tenths of a CP (`Fixed`); terrain must be enterable.
        let cost = match step_cost(&self.reference, &self.map, from, to, MoveMode::Motorized) {
            Some(MoveCost::Cost(f)) => f.0.max(0),
            _ => return Err(CommandError::ReplacementFailed),
        };
        // §20.42d: the pool may never exceed its assigned unit's CPA (tenths, like `cp_expended`).
        if pool_cpa_used as i32 + cost > self.effective_cpa(assigned).unwrap_or(0).max(0) * 10 {
            return Err(CommandError::ReplacementFailed);
        }
        // §20.44/§20.46 haulage: an Infantry pool must ride a co-located Friendly truck convoy with
        // enough §54.2 infantry lift and movement allowance for the step; that convoy travels with the
        // pool and its Truck Points burn the Fuel. Tank/Gun/Recce points may move on their own (§20.44),
        // costing the Fuel of the minimum Truck Points that would lift them.
        let light_inf = self.reference.truck_chart.as_ref().and_then(|c| c.light.toe_inf_tenths).unwrap_or(5).max(1);
        let (carrier, truck_points) = if class == ReplacementClass::Infantry {
            // A co-located Friendly convoy can lift the pool and still afford the step (§54.2/§8.98).
            let can_carry = |s: &Self, i: usize| {
                let c = &s.truck_convoys[i];
                c.side == side
                    && c.hex == from
                    && s.convoy_inf_capacity_tenths(c) >= lift_tenths
                    && c.cpa_used as i32 + cost <= s.truck_convoy_cpa(c) as i32
            };
            // §20.46: ride the already-bound convoy when it can still make the step; otherwise pick up
            // (auto-load onto) a co-located one and bind to it — the binding then persists across the
            // haul (and drives §27.58 raid effects) until the pool is dropped or absorbed.
            let bound = self.replacement_points[index]
                .carried_by
                .filter(|&i| i < self.truck_convoys.len() && can_carry(self, i));
            let carrier = bound.or_else(|| (0..self.truck_convoys.len()).find(|&i| can_carry(self, i)));
            match carrier {
                Some(i) => {
                    self.replacement_points[index].carried_by = Some(i);
                    let c = &self.truck_convoys[i];
                    (Some(i), c.trucks.light + c.trucks.medium + c.trucks.heavy)
                }
                None => return Err(CommandError::ReplacementFailed),
            }
        } else {
            (None, (lift_tenths + light_inf - 1) / light_inf)
        };
        // §49.13/§20.45: the hauling Truck Points burn Fuel (FCF 1) = trucks × ⌈CP/5⌉. §49.18: a carrying
        // convoy burns the Fuel it is transporting first, then draws the shortfall from a co-located
        // Friendly Supply Dump (§49.15; a self-moving pool has no tank, so it draws entirely from the dump).
        let fuel_cost = truck_points * (((cost + 49) / 50) as u32);
        let from_convoy = carrier.map_or(0, |i| self.truck_convoys[i].cargo.fuel).min(fuel_cost);
        let from_dump = fuel_cost - from_convoy;
        if self.available_commodity_at(from, side, Commodity::Fuel) < from_dump {
            return Err(CommandError::InsufficientFuel);
        }
        // Commit: draw the Fuel, advance the carrying convoy (if any) with the pool, and move the pool.
        self.consume_commodity_at(from, side, Commodity::Fuel, from_dump);
        if let Some(i) = carrier {
            self.truck_convoys[i].cargo.fuel -= from_convoy;
            self.truck_convoys[i].cpa_used += cost as u32;
            self.truck_convoys[i].hex = to;
        }
        let p = &mut self.replacement_points[index];
        p.cpa_used += cost as u32;
        p.location = Some(crate::state::unit::UnitLocation::OnMap(to));
        Ok(CommandOutcome::ReplacementMoved { index, hex: to })
    }

    /// §20.44 LoadReplacement: pick up a co-located Replacement-Point pool onto a free convoy, binding
    /// the two. Both must be the active player's, on the same hex; an Infantry pool needs the convoy to
    /// have enough §54.2 infantry lift (Tank/Gun/Recce pools self-move, so loading is a no-op offer we
    /// still honor for display). The binding drives movement (§20.46) and §27.58 raid effects.
    fn cmd_load_replacement(&mut self, convoy: usize, pool: usize) -> Result<CommandOutcome, CommandError> {
        use crate::state::game::ReplacementClass;
        self.require_phase(Phase::TruckConvoy)?;
        let p = self.replacement_points.get(pool).ok_or(CommandError::ReplacementFailed)?;
        let c = self.truck_convoys.get(convoy).ok_or(CommandError::ReplacementFailed)?;
        if self.active_player != p.side || p.side != c.side {
            return Err(CommandError::WrongPhase);
        }
        let phex = p.location.as_ref().and_then(|l| l.hex());
        if phex != Some(c.hex) {
            return Err(CommandError::ReplacementFailed);
        }
        if p.class == ReplacementClass::Infantry && self.convoy_inf_capacity_tenths(c) < p.count * 10 {
            return Err(CommandError::ReplacementFailed);
        }
        self.replacement_points[pool].carried_by = Some(convoy);
        Ok(CommandOutcome::ReplacementBinding { pool, convoy: Some(convoy) })
    }

    /// §20.44 UnloadReplacement: drop a pool from its carrying convoy, clearing the binding so it
    /// becomes a standalone (stranded) counter again.
    fn cmd_unload_replacement(&mut self, pool: usize) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::TruckConvoy)?;
        let p = self.replacement_points.get_mut(pool).ok_or(CommandError::ReplacementFailed)?;
        if self.active_player != p.side {
            return Err(CommandError::WrongPhase);
        }
        p.carried_by = None;
        Ok(CommandOutcome::ReplacementBinding { pool, convoy: None })
    }

    /// §24.9 ConstructSupplyDump: a co-located TOE Strength Point turns the loose supplies at its hex
    /// into a convoy-loadable dump. A real dump costs **3 CP + 20 Store Points** (consumed from the
    /// hex's supplies, Logistics Game); a `dummy` costs **3 CP** only. (Organization Phase.)
    fn cmd_construct_supply_dump(&mut self, unit: UnitId, hex: HexCoord, dummy: bool) -> Result<CommandOutcome, CommandError> {
        use crate::state::game::{Commodities, Commodity, SupplyDump};
        use crate::state::unit::UnitLocation;
        self.require_phase(Phase::Organization)?;
        let u = self.unit(unit).ok_or(CommandError::UnitNotFound)?;
        let side = u.nationality.side();
        if self.active_player != side {
            return Err(CommandError::WrongPhase);
        }
        // Must be co-located with the target hex; a real dump additionally needs a TOE Strength Point.
        if u.hex() != Some(hex) || (!dummy && u.total_toe() == 0) {
            return Err(CommandError::DumpConstructionIllegal);
        }
        if !dummy {
            // §24.9: expend 20 Store Points in the hex (Logistics Game).
            if self.available_commodity_at(hex, side, Commodity::Stores) < 20 {
                return Err(CommandError::InsufficientStores);
            }
            self.consume_commodity_at(hex, side, Commodity::Stores, 20);
        }
        // §24.9: 3 Capability Points (over-CPA spend disorganizes the unit per §6.21).
        self.expend_cp(unit, crate::reference::terrain::Fixed(30));
        if dummy {
            self.supply_dumps.push(SupplyDump {
                location: UnitLocation::OnMap(hex), side, supply: Commodities::default(), dummy: true, constructed: true, unlimited: false,
            });
        } else {
            // Promote every loose pile of this side at the hex to a constructed (loadable) dump; if
            // none exist, establish an empty constructed dump.
            let mut promoted = false;
            for d in self.supply_dumps.iter_mut().filter(|d| d.side == side && !d.dummy && d.location.hex() == Some(hex)) {
                d.constructed = true;
                promoted = true;
            }
            if !promoted {
                self.supply_dumps.push(SupplyDump {
                    location: UnitLocation::OnMap(hex), side, supply: Commodities::default(), dummy: false, constructed: true, unlimited: false,
                });
            }
        }
        Ok(CommandOutcome::SupplyDumpConstructed { hex, dummy })
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

    /// §42.27 ReconHex: fly `planes` on an air-reconnaissance mission to `target` and reveal enemy
    /// TOE Strength there. Validates Recon capability (§42.21), range (§37.12), and weather (§37.22).
    /// The §42.27 table (with +1 per four planes) converts the d6 `die` into how many enemy
    /// battalion-equivalents are revealed (`None` = all), taken in §42.24a order (Tanks, Infantry,
    /// Artillery). Revealed units' strength is visible to the flier via `observe_units` this stage.
    fn cmd_recon_hex(&mut self, squadron: crate::state::air::SquadronId, target: HexCoord, planes: u32, die: u32) -> Result<CommandOutcome, CommandError> {
        use crate::state::game::Weather;
        self.require_phase(Phase::LandSupportAir)?;
        let sgsu = self.sgsus.get(&squadron).ok_or(CommandError::NoSuchSquadron)?;
        if sgsu.in_transit() {
            return Err(CommandError::SquadronInTransit);
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
        // §42.25: no Recon over a hex undergoing air bombardment (barraged/bombed this stage).
        if self.barraged_hexes_this_stage.contains(&target) {
            return Err(CommandError::MissionNotPermitted);
        }
        // Shortest range among ready plane types; at least one must have Recon capability (§42.21).
        let chart = self.reference.aircraft_charts.get(&nationality);
        let mut min_range = u32::MAX;
        let mut can_r = false;
        for g in sgsu.planes.iter().filter(|g| g.ready > 0) {
            if let Some(ac) = chart.and_then(|c| c.get(&g.aircraft_type)) {
                min_range = min_range.min(ac.max_range());
                if crate::rules::air::can_recon(&ac.missions) {
                    can_r = true;
                }
            }
        }
        if min_range == u32::MAX {
            return Err(CommandError::NoReadyPlanes);
        }
        if !can_r {
            return Err(CommandError::MissionNotPermitted);
        }
        let distance = self.flight_distance(squadron, target);
        if !distance.is_some_and(|d| crate::rules::air::can_fly(min_range, d, false)) {
            return Err(CommandError::TargetOutOfRange);
        }
        let flown = self.sgsus.get_mut(&squadron).ok_or(CommandError::NoSuchSquadron)?.fly(planes);
        if flown == 0 {
            return Err(CommandError::NoReadyPlanes);
        }
        // §42.27: how many battalion-equivalents are revealed (None = all).
        let reveal_n = self
            .reference
            .air_recon
            .as_ref()
            .and_then(|t| t.reveal(die, flown));
        // Enemy units in the hex, in §42.24a reveal order: Tanks, then Infantry, then Artillery.
        let enemy = side.opponent();
        let mut targets: Vec<UnitId> = self
            .units_in_hex(target)
            .into_iter()
            .filter(|&id| self.unit(id).is_some_and(|u| u.nationality.side() == enemy && u.total_toe() > 0))
            .collect();
        let order_key = |g: &Self, id: UnitId| -> u8 {
            if g.unit_has_tank(id) { 0 } else if g.is_gun_unit(id) { 2 } else { 1 }
        };
        targets.sort_by_key(|&id| order_key(self, id));
        let take = match reveal_n {
            None => targets.len(),
            Some(n) => (n as usize).min(targets.len()),
        };
        let revealed: Vec<UnitId> = targets.into_iter().take(take).collect();
        for &id in &revealed {
            self.recon_revealed.insert((side, id));
        }
        Ok(CommandOutcome::ReconRevealed { revealed })
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
        let nationality = self.sgsus.get(&squadron).ok_or(CommandError::NoSuchSquadron)?.nationality;
        // §40.16: rating-0 pilots are an unlimited supply. Rated pilots come from the finite
        // §60.32/§60.42 pool — draw one, refusing if none of that rating remains.
        if rating != 0 {
            if self.pool_pilots(nationality, rating) == 0 {
                return Err(CommandError::InsufficientPilotPool);
            }
            if let Some(n) = self.pilot_pool.get_mut(&nationality).and_then(|m| m.get_mut(&rating)) {
                *n -= 1;
            }
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

    /// §40.5–40.8 Strafe: validate and resolve a strafing mission against the map. Handles every
    /// §40.54 standalone target: infantry (§40.61), truck convoys and first-line trucks (§40.62),
    /// grounded aircraft (§40.63), supply dumps (§40.64), tanks (§40.65) and the water pipeline
    /// (§40.67). Ports (§40.66) are *not* a standalone strafing mission — port strafing folds its
    /// half-value TacAir into a bombing raid on the port (§41.5), so it is rejected here and handled
    /// as part of a bombing mission. §40.55: strafing may never occur in a major-city hex.
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
        // §40.66: Ports are not a standalone strafing target — they are strafed only as part of a
        // bombing raid on the port. Reject; the caller folds the TacAir into a bombing mission.
        if matches!(strafe_target, StrafeTarget::Port) {
            return Err(CommandError::MissionNotPermitted);
        }
        // §40.55: no strafing in a major-city hex.
        if self.is_major_city(target) {
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
            StrafeTarget::TrucksInConvoy => {
                // §40.62: the result is the Truck Points eliminated from an enemy free convoy.
                let removed = self.strafe_convoy_trucks(target, side, result);
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
        let nationality = self.sgsus.get(&squadron).ok_or(CommandError::NoSuchSquadron)?.nationality;
        // §36.3: a squadron may only take planes its facility kind can service — flying-boats need a
        // basin/alighting area, land planes need an airfield/strip. Applies both on the map and at an
        // off-map §36.53 facility (Port Said is a flying-boat basin; the rest are airfields). Axis
        // Med-box squadrons (no hex, no §36.53 box) are unrestricted here.
        {
            use crate::state::air::AirFacilityKind;
            let s = self.sgsus.get(&squadron).ok_or(CommandError::NoSuchSquadron)?;
            let kind = if let Some(hex) = s.location {
                self.air_facilities.iter().find(|f| f.hex == hex).map(|f| f.kind)
            } else if let Some(box_name) = s.off_map_box.as_deref() {
                self.reference.off_map_air_facilities.as_ref()
                    .and_then(|c| c.facilities.iter().find(|f| f.name == box_name))
                    .map(|f| if f.flying_boat { AirFacilityKind::FlyingBoatBasin } else { AirFacilityKind::Airfield })
            } else {
                None
            };
            if let Some(kind) = kind {
                if !facility_serves(kind, class) {
                    return Err(CommandError::WrongAircraftForFacility);
                }
            }
        }
        // §35.2: planes come from the nationality's pool (the §60.32/§60.42 starting pool plus
        // §34.84/§34.85 reinforcements). Draw only what is available.
        if self.pool_planes(nationality, aircraft_type) < count {
            return Err(CommandError::InsufficientPlanePool);
        }
        let sgsu = self.sgsus.get_mut(&squadron).ok_or(CommandError::NoSuchSquadron)?;
        if !sgsu.add_planes(aircraft_type, class, count, gt) {
            return Err(CommandError::SquadronRejected);
        }
        // Assignment succeeded — deduct from the pool.
        if let Some(m) = self.plane_pool.get_mut(&nationality) {
            if let Some(n) = m.get_mut(aircraft_type) {
                *n -= count;
            }
        }
        Ok(CommandOutcome::SquadronPlanesAssigned)
    }

    /// §44.14: base Commonwealth planes on a Malta facility, from the CW plane pool.
    fn cmd_base_malta_planes(&mut self, base_index: usize, aircraft_type: &str, count: u32) -> Result<CommandOutcome, CommandError> {
        use crate::reference::units::Nationality;
        self.require_phase(Phase::Organization)?;
        if self.active_player != Side::Commonwealth {
            return Err(CommandError::WrongPhase);
        }
        let base = self.malta_bases.get(base_index).ok_or(CommandError::NoSuchMaltaBase)?;
        let class = self
            .reference
            .aircraft_charts
            .get(&Nationality::Commonwealth)
            .and_then(|c| c.get(aircraft_type))
            .map(|ac| ac.class)
            .ok_or(CommandError::WrongAircraftForFacility)?;
        if !facility_serves(base.kind, class) {
            return Err(CommandError::WrongAircraftForFacility); // §36.3
        }
        if base.plane_count() + count > base.plane_capacity() {
            return Err(CommandError::MaltaCapacityExceeded); // §44.14
        }
        if self.pool_planes(Nationality::Commonwealth, aircraft_type) < count {
            return Err(CommandError::InsufficientPlanePool);
        }
        if let Some(m) = self.plane_pool.get_mut(&Nationality::Commonwealth) {
            if let Some(n) = m.get_mut(aircraft_type) {
                *n -= count;
            }
        }
        self.malta_bases[base_index].add_planes(aircraft_type, count);
        Ok(CommandOutcome::MaltaPlanesBased(count))
    }

    /// §44.15: move planes between two Malta facilities (free), capacity- and §36.3-checked.
    fn cmd_move_malta_planes(&mut self, from: usize, to: usize, aircraft_type: &str, count: u32) -> Result<CommandOutcome, CommandError> {
        use crate::reference::units::Nationality;
        self.require_phase(Phase::Organization)?;
        if self.active_player != Side::Commonwealth {
            return Err(CommandError::WrongPhase);
        }
        if from == to || from >= self.malta_bases.len() || to >= self.malta_bases.len() {
            return Err(CommandError::NoSuchMaltaBase);
        }
        let class = self
            .reference
            .aircraft_charts
            .get(&Nationality::Commonwealth)
            .and_then(|c| c.get(aircraft_type))
            .map(|ac| ac.class)
            .ok_or(CommandError::WrongAircraftForFacility)?;
        if !facility_serves(self.malta_bases[to].kind, class) {
            return Err(CommandError::WrongAircraftForFacility); // §36.3
        }
        let have = self.malta_bases[from].planes.iter().find(|(t, _)| t == aircraft_type).map(|(_, n)| *n).unwrap_or(0);
        if have < count {
            return Err(CommandError::InsufficientPlanePool);
        }
        if self.malta_bases[to].plane_count() + count > self.malta_bases[to].plane_capacity() {
            return Err(CommandError::MaltaCapacityExceeded); // §44.14
        }
        self.malta_bases[from].remove_planes(aircraft_type, count);
        self.malta_bases[to].add_planes(aircraft_type, count);
        Ok(CommandOutcome::MaltaPlanesMoved(count))
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
                Some(MaltaCommitment::Turns(limit))
                    if self.malta_level_used.get(&level).copied().unwrap_or(0) >= limit =>
                {
                    return Err(CommandError::MaltaLevelUnavailable);
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
        // §38.34: roll the refit die inside the boundary (agents cannot supply it).
        let base_die = self.rng.d6() as u32;
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

    /// §12.44 Pinned / §6.26 Disorganized (Cohesion ≤ −26): such a unit may not fire anti-armor or
    /// close-assault. (Pinned units may still barrage — §12.44 lists only move / anti-armor / close
    /// assault — so barrage checks the −26 gate on its own.)
    fn require_can_attack(&self, attackers: &[UnitId]) -> Result<(), CommandError> {
        for &a in attackers {
            if let Some(u) = self.unit(a) {
                if u.status.pinned {
                    return Err(CommandError::Pinned);
                }
                if u.cohesion <= -26 {
                    return Err(CommandError::Disorganized);
                }
            }
        }
        Ok(())
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

    /// Barrage (§12.6). The firer names an adjacent hex and a target class (§12.23–24); the engine
    /// resolves against an enemy unit of that class there. There is no line-of-fire check by rule
    /// (§12.35: no LOS restrictions in the desert). §12.23: the firer may instead designate a
    /// *specific* known target unit (`target_unit`) — allowed only when that unit is a valid target
    /// of `target_class` in the hex and known to the firer (In-Contact/Engaged, the "previous
    /// combat" path; the Patrol-knowledge path is not persisted, §8.11).
    fn cmd_barrage(
        &mut self,
        attackers: &[UnitId],
        target: HexCoord,
        target_class: BarrageTargetClass,
        target_unit: Option<UnitId>,
    ) -> Result<CommandOutcome, CommandError> {
        self.require_combat_phase()?;
        if attackers.is_empty() {
            return Err(CommandError::NoParticipants);
        }
        self.require_units_exist(attackers)?;
        self.require_no_noncombat_hq(attackers)?;
        // §6.26: a Disorganized (≤ −26) unit may not attack. (Pinned units *may* barrage, §12.44.)
        if attackers.iter().any(|&a| self.unit(a).is_some_and(|u| u.cohesion <= -26)) {
            return Err(CommandError::Disorganized);
        }
        let side = self.attackers_side(attackers)?;
        // §12.31: artillery barrages an *adjacent* occupied hex.
        self.require_all_adjacent(attackers, target)?;
        // §12.22–24: eligible targets are enemy combat counters of the named class in the hex
        // (empty HQs excluded). Normally the firer picks by class and we resolve against one such
        // unit; §12.23 lets the firer designate a *specific* known unit instead.
        let candidates: Vec<UnitId> = self
            .enemy_units_in_hex(target, side)
            .into_iter()
            .filter(|&id| !self.is_noncombat_hq(id) && self.barrage_class(id) == target_class)
            .collect();
        let target_unit = match target_unit {
            Some(designated) => {
                // §12.23: the designated unit must be one of the eligible targets, and the firer
                // must actually know it is there — modeled as In-Contact/Engaged (previous combat).
                if !candidates.contains(&designated) {
                    return Err(CommandError::NoTargets);
                }
                let known = self
                    .unit(designated)
                    .map(|u| u.status.in_contact || u.status.engaged)
                    .unwrap_or(false);
                if !known {
                    return Err(CommandError::TargetNotKnown);
                }
                designated
            }
            None => candidates.into_iter().next().ok_or(CommandError::NoTargets)?,
        };
        // §50.14: each barraging unit expends Ammo (rate × TOE) from its reserve / hex dumps.
        if !self.consume_combat_ammo(attackers, crate::state::game::CombatAction::Barrage) {
            return Err(CommandError::InsufficientAmmo);
        }
        // §16.21: a barraged hex may not be reconnoitred this stage.
        self.barraged_hexes_this_stage.insert(target);
        // §11.22.1/§11.21a: the barraging units pay 5 CP if phasing (active side) else 3.
        let cap = if side == self.active_player { 5 } else { 3 };
        for &a in attackers {
            self.charge_combat_cp(a, cap);
        }
        Ok(CommandOutcome::Barrage(self.resolve_barrage(attackers, target_unit)))
    }

    /// §12.5 Barrage against a Facility. §12.53: structures (fortifications/cities, roads, railroads)
    /// read the §41.5 Artillery Barrage Points column with **Actual** Barrage Points; §12.54: Supply
    /// Dumps and Air Facilities read the Bomb column with **Raw** Barrage Points. The result is
    /// applied through the shared facility-effect machinery.
    fn cmd_barrage_facility(
        &mut self,
        barragers: &[UnitId],
        target: HexCoord,
        facility: FacilityTarget,
    ) -> Result<CommandOutcome, CommandError> {
        use crate::reference::aircraft::BombingMission as M;
        use crate::reference::air_bombardment::{TargetGroup, WeaponPoints};
        use crate::state::unit::RatingKind;
        self.require_combat_phase()?;
        if barragers.is_empty() {
            return Err(CommandError::NoParticipants);
        }
        self.require_units_exist(barragers)?;
        self.require_no_noncombat_hq(barragers)?;
        // §6.26: a Disorganized unit may not attack (Pinned units may still barrage, §12.44).
        if barragers.iter().any(|&a| self.unit(a).is_some_and(|u| u.cohesion <= -26)) {
            return Err(CommandError::Disorganized);
        }
        let side = self.attackers_side(barragers)?;
        // §12.31: artillery barrages an adjacent hex.
        self.require_all_adjacent(barragers, target)?;
        // §50.14: each barraging unit expends Ammo.
        if !self.consume_combat_ammo(barragers, crate::state::game::CombatAction::Barrage) {
            return Err(CommandError::InsufficientAmmo);
        }
        self.barraged_hexes_this_stage.insert(target);
        // Raw & Actual Barrage Points across the firing units (§11.34 / §11.3).
        let raw: i32 = barragers
            .iter()
            .filter_map(|&id| self.unit(id))
            .map(|u| u.raw_strength(&self.reference, RatingKind::Barrage))
            .sum();
        let actual = ((raw + 5) / 10).max(0) as u32;
        let raw = raw.max(0) as u32;
        // §12.53 (structures → Artillery Barrage column, Actual) vs §12.54 (dump/air → Bomb column, Raw).
        let (group, mission, weapon, points) = match facility {
            FacilityTarget::Fortification => (TargetGroup::Fortification, M::CityFortification, WeaponPoints::Barrage, actual),
            FacilityTarget::Road => (TargetGroup::Road, M::Road, WeaponPoints::Barrage, actual),
            FacilityTarget::Railroad => (TargetGroup::Railroad, M::Railroad, WeaponPoints::Barrage, actual),
            FacilityTarget::SupplyDump => (TargetGroup::SupplyDump, M::SupplyDump, WeaponPoints::Bomb, raw),
            FacilityTarget::AirFacility => (TargetGroup::AirfieldPortStrip, M::AirFacility, WeaponPoints::Bomb, raw),
        };
        let roll = self.rng.two_dice_raw() as u32;
        let result = self
            .reference
            .air_bombardment
            .as_ref()
            .map(|t| t.result(group, weapon, points, roll))
            .unwrap_or(0);
        // §11.22.1: barraging units pay 5 CP if phasing else 3.
        let cap = if side == self.active_player { 5 } else { 3 };
        for &a in barragers {
            self.charge_combat_cp(a, cap);
        }
        if result > 0 {
            match facility {
                // Air facilities are reduced through their own capacity model, not the land-effect
                // dispatcher; reduce every air facility in the hex by the result's levels.
                FacilityTarget::AirFacility => {
                    let kinds: Vec<crate::state::air::AirFacilityKind> = self
                        .air_facilities
                        .iter()
                        .filter(|f| f.hex == target)
                        .map(|f| f.kind)
                        .collect();
                    for kind in kinds {
                        self.damage_air_facility(target, kind, result as u32);
                    }
                }
                _ => self.apply_land_bombing_effect(mission, target, side, result as u32),
            }
        }
        Ok(CommandOutcome::FacilityBarrage { result })
    }

    /// Anti-Armor fire (§14) at a target hex. Fire is adjacent-only (enforced here) and the §14.34
    /// prohibited-hexside blocking is applied in `resolve_anti_armor` (there is no ranged LOS rule).
    fn cmd_anti_armor(
        &mut self,
        attackers: &[UnitId],
        target_hex: HexCoord,
        withheld: &[UnitId],
    ) -> Result<CommandOutcome, CommandError> {
        self.require_combat_phase()?;
        if attackers.is_empty() {
            return Err(CommandError::NoParticipants);
        }
        self.require_units_exist(attackers)?;
        self.require_no_noncombat_hq(attackers)?;
        // §12.44 Pinned / §6.26 Disorganized (≤ −26): may not fire anti-armor.
        self.require_can_attack(attackers)?;
        // §13.28: a unit that Retreated Before Assault this stage may not direct anti-armor fire.
        if attackers.iter().any(|&a| self.unit(a).is_some_and(|u| u.retreated_before_assault)) {
            return Err(CommandError::CannotDirectAntiArmor);
        }
        if self.map.terrain(target_hex).is_none() {
            return Err(CommandError::DestinationOffMap);
        }
        // Anti-Armor fire is against an adjacent hex (§14), like other close combat.
        if !self.attackers_adjacent_to(attackers, target_hex) {
            return Err(CommandError::NotAdjacent);
        }
        // §14: anti-armor fire needs an enemy in the target hex. Reject a hex with no live enemy
        // combat unit (e.g. one a Close Assault just cleared) — otherwise the shot resolves to a
        // null result against nothing.
        let enemy_side = attackers
            .first()
            .and_then(|&a| self.unit(a))
            .map(|u| u.nationality.side().opponent())
            .ok_or(CommandError::NoTargets)?;
        if !self
            .units_in_hex(target_hex)
            .into_iter()
            .any(|id| self.unit(id).is_some_and(|u| u.nationality.side() == enemy_side && u.total_toe() > 0))
        {
            return Err(CommandError::NoTargets);
        }
        // §14.22: a hex may be anti-armor-fired only once per Combat Segment.
        if self.anti_armor_hexes_this_stage.contains(&target_hex) {
            return Err(CommandError::HexAlreadyInCombat);
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
        // §11.22/§11.25: anti-armor charges the firing hexes and the target hex — 5 CP for the
        // phasing side, 3 for the non-phasing — before resolution (§6.22).
        let mut cp_hexes: Vec<HexCoord> = attackers.iter().filter_map(|&a| self.effective_hex(a)).collect();
        cp_hexes.push(target_hex);
        self.charge_combat_hexes(&cp_hexes, false);
        self.anti_armor_hexes_this_stage.insert(target_hex); // §14.22
        // §50.12 (Sep 1979 erratum): a unit fired upon by Anti-Armor Fire while out of ammunition
        // surrenders (merely being in a ZOC or barraged does not — only an assault). Mirrors the
        // close-assault auto-surrender; if the fire found only out-of-ammo armor, the hex is cleared.
        let surrendered: Vec<UnitId> = self
            .units_in_hex(target_hex)
            .into_iter()
            .filter(|&d| {
                self.unit(d).is_some_and(|u| u.nationality.side() == enemy_side && u.total_toe() > 0) && self.out_of_close_assault_ammo(d)
            })
            .collect();
        if !surrendered.is_empty() {
            let toe: u32 = surrendered.iter().filter_map(|&d| self.unit(d)).map(|u| u.total_toe()).sum();
            self.surrender_units(&surrendered);
            let live = self
                .units_in_hex(target_hex)
                .into_iter()
                .any(|id| self.unit(id).is_some_and(|u| u.nationality.side() == enemy_side && u.total_toe() > 0));
            if !live {
                return Ok(CommandOutcome::AntiArmor(crate::state::combat::AntiArmorOutcome {
                    dice: 0, raw_points: 0, actual_points: 0, terrain_shift: 0, column: 0, damage: 0, armor_toe_removed: toe, phasing_penalty: false,
                }));
            }
        }
        let dice = self.rng.two_dice_raw();
        let outcome = self.resolve_anti_armor_core(attackers, target_hex, dice, withheld);
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
        // §12.44 Pinned / §6.26 Disorganized (≤ −26): may not close-assault.
        self.require_can_attack(attackers)?;
        // §52.51/52: a water-deprived unit may not engage in offensive close assault.
        if attackers.iter().any(|&a| self.unit_water_deprived(a)) {
            return Err(CommandError::NoWater);
        }
        // §49.4 (Addenda): a fuel-using unit (infantry-in-trucks / mechanized / tank-AC-recce) with
        // no Fuel available may not make an offensive Close Assault. Foot infantry (no fuel cost) is
        // unaffected — `move_fuel_available` reports true for non-fuel-users.
        if attackers.iter().any(|&a| self.effective_hex(a).is_some_and(|ah| !self.move_fuel_available(a, ah, 1))) {
            return Err(CommandError::InsufficientFuel);
        }
        let side = self.attackers_side(attackers)?;
        // §18.23.2/§18.24.2: a unit released from Reserve this stage may make only one offensive
        // Close Assault.
        if attackers.iter().any(|&a| {
            self.unit(a).is_some_and(|u| u.released_from_reserve.is_some() && u.made_offensive_assault)
        }) {
            return Err(CommandError::InReserve);
        }
        // §8.44: no motorized unit or AFV may Assault a defender in a Salt Marsh hex.
        if self.map.terrain(target) == Some(crate::reference::terrain::HexTerrain::SaltMarsh)
            && attackers.iter().any(|&a| self.unit_is_motorized(a) || self.unit_is_afv(a))
        {
            return Err(CommandError::SaltMarshAssault);
        }
        // Every attacker must be adjacent to the attacked hex (§15.23).
        self.require_all_adjacent(attackers, target)?;
        // §15.23: a hex may be close-assaulted only once per Combat Segment.
        if self.close_assaulted_hexes_this_stage.contains(&target) {
            return Err(CommandError::HexAlreadyInCombat);
        }
        // The defenders are the entire enemy stack in the attacked hex.
        let mut defenders = self.enemy_units_in_hex(target, side);
        if defenders.is_empty() {
            return Err(CommandError::NoTargets);
        }
        // §15.88/§15.15: assaulted defenders that are out of Close-Assault Ammunition or at cohesion
        // ≤ −17 automatically surrender (en masse if all of them). The rest defend normally.
        let auto_surrender: Vec<UnitId> = defenders
            .iter()
            .copied()
            .filter(|&d| self.out_of_close_assault_ammo(d) || self.unit(d).is_some_and(|u| u.cohesion <= -17))
            .collect();
        if !auto_surrender.is_empty() {
            self.surrender_units(&auto_surrender);
            defenders.retain(|&d| self.unit(d).is_some_and(|u| u.total_toe() > 0));
        }
        // §50.14: the assaulting units expend Ammo (armor/gun-class only; rifle infantry pays
        // none). A shortfall forbids the assault (§50.12).
        if !self.consume_combat_ammo(attackers, crate::state::game::CombatAction::CloseAssault) {
            return Err(CommandError::InsufficientAmmo);
        }
        // §11.22/§11.25: the assaulting hexes pay 5 CP (phasing), the assaulted hex 3 (non-phasing);
        // applied before resolution so any §6.21 over-CPA Disorganization feeds the morale check.
        let mut cp_hexes: Vec<HexCoord> = attackers.iter().filter_map(|&a| self.effective_hex(a)).collect();
        cp_hexes.push(target);
        self.charge_combat_hexes(&cp_hexes, false);
        // Snapshot TOE of all involved units for the §15.87 loss check.
        let involved: Vec<(UnitId, u32)> = attackers
            .iter()
            .chain(defenders.iter())
            .map(|&id| (id, self.unit(id).map(|u| u.total_toe()).unwrap_or(0)))
            .collect();
        let outcome = self.resolve_close_assault(attackers, &defenders);
        // §15.87/§6.21: a unit that loses ≥30% of its TOE in one close assault earns 3 Disorganization
        // Points (−3 cohesion).
        for (id, before) in &involved {
            if *before == 0 {
                continue;
            }
            let after = self.unit(*id).map(|u| u.total_toe()).unwrap_or(0);
            if (before - after) * 100 >= before * 30 {
                if let Some(u) = self.unit_mut(*id) {
                    u.cohesion -= 3;
                }
            }
        }
        // §6.24.2: if the assault clears the hex of enemy combat units (defender vacates), each
        // assaulting unit earns 3 Reorganization Points (cohesion, capped at +10).
        let enemy_cleared = !self
            .enemy_units_in_hex(target, side)
            .iter()
            .any(|&id| self.unit(id).is_some_and(|u| u.total_toe() > 0 && !u.is_hq));
        if enemy_cleared {
            for &a in attackers {
                if let Some(u) = self.unit_mut(a) {
                    u.cohesion = (u.cohesion + 3).min(10);
                }
            }
        }
        // §50.14: the defenders fire defensively too — their melee weapons units spend defensive
        // close-assault Ammo (best-effort; the defender is already committed). (The broader §50.12
        // no-ammo → surrender / no-ZOC effect is a ZOC-wide rule left for the ZOC layer.)
        self.expend_defensive_assault_ammo(&defenders);
        // §15.23: record the hex as close-assaulted this segment.
        self.close_assaulted_hexes_this_stage.insert(target);
        // §16.21: engaging in Close Assault bars this side from Patrolling this stage.
        self.land_combat_this_stage.insert(side);
        // §16.46: a formation with a Dummy Tank that undergoes Close Assault loses the dummy.
        self.eliminate_dummies_in_hex(target, side.opponent());
        // §18.23.2/§18.24.2: record each attacker's one offensive assault; §18.24.3: a unit released
        // from Reserve II that engages in combat this stage automatically earns one Disorganization Point.
        for &a in attackers {
            if let Some(u) = self.unit_mut(a) {
                let reserve_ii = u.released_from_reserve == Some(crate::state::unit::ReserveStatus::ReserveII);
                let first = !u.made_offensive_assault;
                u.made_offensive_assault = true;
                if reserve_ii && first {
                    u.cohesion -= 1;
                }
            }
        }
        Ok(CommandOutcome::CloseAssault(outcome))
    }

    /// §17.5 Voluntary Surrender of the whole stack in `hex` to an adjacent enemy combat unit.
    fn cmd_voluntary_surrender(
        &mut self,
        hex: HexCoord,
        captor: UnitId,
        destroy_material: bool,
    ) -> Result<CommandOutcome, CommandError> {
        // §17.52a: not during a Combat Segment's steps b–f. The engine resolves combat atomically
        // (no such sub-steps), so allow it in the Organization / Movement & Combat phases.
        if !matches!(self.phase, Phase::Organization | Phase::MovementAndCombat) {
            return Err(CommandError::WrongPhase);
        }
        let captor_hex = self.effective_hex(captor).ok_or(CommandError::UnitNotOnMap)?;
        let captor_side = self.unit(captor).ok_or(CommandError::UnitNotFound)?.nationality.side();
        // §17.52c/§17.53: the captor must be an enemy combat unit adjacent to the surrendering hex.
        if !self.is_combat_unit(captor) || !captor_hex.is_adjacent(hex) {
            return Err(CommandError::InvalidCaptor);
        }
        // §17.52b: the *entire* enemy-of-captor stack in `hex` is surrendered.
        let surrendering = self.enemy_units_in_hex(hex, captor_side);
        if surrendering.is_empty() {
            return Err(CommandError::NoTargets);
        }
        // §17.54: the captor must be able to expend 2 CP (within its remaining CPA) to accept.
        let cpa = self.effective_cpa(captor).unwrap_or(0);
        let remaining = cpa * 10 - self.unit(captor).map(|u| u.cp_expended.0).unwrap_or(0);
        if remaining < 20 {
            return Err(CommandError::CaptorCannotAccept);
        }
        // §17.55/§17.56: optional pre-capture destruction. One roll for tanks/guns ((die−1)×10%),
        // one for trucks/Motorization Points ((die−1)×20%); fractions round down. Applied per unit.
        let mut destroyed = 0u32;
        if destroy_material {
            let tg_pct = (self.rng.d6() as u32).saturating_sub(1) * 10;
            let tk_pct = (self.rng.d6() as u32).saturating_sub(1) * 20;
            for &id in &surrendering {
                // Tanks/guns are Weapons-composition TOE; destroy tg_pct% (removed group-by-group).
                let is_material = self.is_gun_unit(id) || self.unit_is_tank(id);
                if is_material && tg_pct > 0 {
                    let toe = self.unit(id).map(|u| u.total_toe()).unwrap_or(0);
                    let mut kill = toe * tg_pct / 100;
                    if let Some(u) = self.unit_mut(id) {
                        if let crate::state::unit::Composition::Weapons { groups } = &mut u.composition {
                            for g in groups.iter_mut() {
                                let take = g.count.min(kill);
                                g.count -= take;
                                kill -= take;
                                destroyed += take;
                                if kill == 0 {
                                    break;
                                }
                            }
                        }
                    }
                }
                // Trucks / Motorization Points: destroy tk_pct% (evenly across truck types).
                if tk_pct > 0 {
                    if let Some(u) = self.unit_mut(id) {
                        let total = u.trucks.light + u.trucks.medium + u.trucks.heavy;
                        let mut kill = total * tk_pct / 100;
                        for slot in [&mut u.trucks.heavy, &mut u.trucks.medium, &mut u.trucks.light] {
                            let take = (*slot).min(kill);
                            *slot -= take;
                            kill -= take;
                            destroyed += take;
                            if kill == 0 {
                                break;
                            }
                        }
                    }
                }
            }
        }
        // §17.51: the rest is treated as a capture (prisoners for infantry, replacements for
        // tanks/guns/aircraft) — surrender_units records the capture and zeroes their TOE.
        let toe_lost = self.surrender_units(&surrendering);
        // §17.54: the captor pays 2 CP to accept.
        self.expend_cp_against(captor, Fixed(20), cpa);
        Ok(CommandOutcome::VoluntarilySurrendered { toe_lost, destroyed })
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

    /// Per-step move cost from `from` into `to`, including the §29.44 sandstorm penalty:
    /// movement costs are doubled when entering a hex under an active sandstorm.
    pub(crate) fn move_step_cost(&self, from: HexCoord, to: HexCoord, mode: MoveMode) -> Option<MoveCost> {
        use crate::rules::movement::step_cost_with;
        use crate::state::game::Weather;
        use crate::state::map::HexsideKind;
        let mut base = step_cost(&self.reference, &self.map, from, to, mode)?;
        // §29.5x Rainstorm crossing restrictions (the destination's map section is stormed).
        if self.weather_at(to) == Weather::Rainstorm {
            let sides = self.map.hexsides_on(from, to);
            let has = |k: HexsideKind| sides.iter().any(|h| h.kind == k);
            let road = has(HexsideKind::Road);
            let rail = has(HexsideKind::Railroad);
            // §29.56: during a rainstorm road hexes cost the same as tracks — recompute treating the
            // road as a track (which now halves the hex terrain cost, §8.37 [8]).
            if road {
                base = step_cost_with(&self.reference, &self.map, from, to, mode, true)?;
            }
            // §29.55: wadi hexsides are uncrossable except by a Road.
            if has(HexsideKind::Wadi) && !road {
                return Some(MoveCost::Prohibited);
            }
            // §29.57: river hexsides not crossed by a road or railroad are impassable.
            if (has(HexsideKind::MajorRiver) || has(HexsideKind::MinorRiver)) && !(road || rail) {
                return Some(MoveCost::Prohibited);
            }
            // §29.58: no motorized unit may enter a delta hex unless on a road or railroad.
            if mode == MoveMode::Motorized
                && self.map.terrain(to) == Some(crate::reference::terrain::HexTerrain::Delta)
                && !(road || rail)
            {
                return Some(MoveCost::Prohibited);
            }
        }
        if self.sandstorm_at(to) {
            if let MoveCost::Cost(f) = base {
                return Some(MoveCost::Cost(Fixed(f.0 * 2)));
            }
        }
        Some(base)
    }

    /// Whether `target` is reachable from `origin` within `cp_budget` tenths of a Capability Point,
    /// paying real terrain/hexside movement costs for `mode` (a cost-limited Dijkstra over
    /// `move_step_cost`). Prohibited / EntireCpa steps are impassable. This replaces the earlier
    /// hex-distance proxies for the "within N Movement CPs" ranges (§16.22 / §27.59 / §28.24).
    pub(crate) fn within_cp_budget(
        &self,
        origin: HexCoord,
        target: HexCoord,
        mode: MoveMode,
        cp_budget: i32,
    ) -> bool {
        use std::cmp::Reverse;
        use std::collections::{BinaryHeap, HashMap};
        if origin == target {
            return true;
        }
        let mut best: HashMap<HexCoord, i32> = HashMap::new();
        best.insert(origin, 0);
        // Heap of (cost, col, row); col/row give a deterministic tie-break without needing Ord on
        // HexCoord.
        let mut heap: BinaryHeap<Reverse<(i32, i16, i16)>> = BinaryHeap::new();
        heap.push(Reverse((0, origin.col, origin.row)));
        while let Some(Reverse((cost, col, row))) = heap.pop() {
            let here = HexCoord::new(col, row);
            if cost > *best.get(&here).unwrap_or(&i32::MAX) {
                continue; // stale heap entry
            }
            if here == target {
                return cost <= cp_budget;
            }
            for n in self.map.neighbors(here) {
                let step = match self.move_step_cost(here, n, mode) {
                    Some(MoveCost::Cost(f)) => f.0,
                    _ => continue, // off-map / prohibited / entire-CPA: impassable for range
                };
                let nc = cost + step;
                if nc <= cp_budget && nc < *best.get(&n).unwrap_or(&i32::MAX) {
                    best.insert(n, nc);
                    heap.push(Reverse((nc, n.col, n.row)));
                }
            }
        }
        false
    }

    /// §16.22: whether a legal Patrol path of at most five hexes runs from `origin` to `target`.
    /// The path may not cross terrain impassable to the (motorized) patrol points (§16.11), and no
    /// hex along the way — other than the enemy-occupied `target` itself — may be enemy-occupied.
    /// (The rule also bars "enemy-controlled" hexes, but the target's own ZOC blankets every hex
    /// adjacent to it, so enforcing a ZOC-free approach would make any ZOC-projecting target
    /// un-patrollable; we therefore gate on enemy *occupancy*, which is unambiguous.) A
    /// breadth-first search bounded to five steps.
    pub(crate) fn patrol_path_exists(&self, origin: HexCoord, target: HexCoord, enemy: Side) -> bool {
        use std::collections::VecDeque;
        if origin == target {
            return true;
        }
        let mut seen = std::collections::HashSet::new();
        seen.insert(origin);
        let mut q: VecDeque<(HexCoord, u32)> = VecDeque::new();
        q.push_back((origin, 0));
        while let Some((here, depth)) = q.pop_front() {
            if depth >= 5 {
                continue;
            }
            for n in self.map.neighbors(here) {
                if seen.contains(&n) {
                    continue;
                }
                // The patrol points must be able to enter `n` (impassable terrain blocks, §16.11).
                if !matches!(self.move_step_cost(here, n, MoveMode::Motorized), Some(MoveCost::Cost(_))) {
                    continue;
                }
                if n == target {
                    return true; // the enemy-occupied objective, reached in ≤5 hexes
                }
                // §16.22: may not pass through an enemy-occupied hex.
                if !self.enemy_units_in_hex(n, enemy.opponent()).is_empty() {
                    continue;
                }
                seen.insert(n);
                q.push_back((n, depth + 1));
            }
        }
        false
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

    /// Reaction (§8.5): a non-phasing unit evades before it can be assaulted. Gated by §8.53 (must
    /// be non-phasing, motorized, not Engaged, not already in an enemy ZOC), triggered by an adjacent
    /// enemy (§8.51), and the §8.53b/§8.54 CPA-and-size "pinning" is enforced (a fast, large-enough
    /// adjacent enemy bars the reaction); the reaction path may never enter an enemy ZOC (§8.55).
    /// §8.56: an attached unit may react by detaching first (1 CP for the detachment). The §8.53a
    /// Truck-Convoy stacking exception is N/A here — truck convoys move via `MoveTruckConvoy`, not
    /// `React`; the motorized gate is the faithful §8.53a rule for regular units.
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
        // §8.56: a unit attached to a parent formation may react — it detaches first (1 CP for the
        // detachment, §19.43) and then reacts. A unit that is itself a parent of other attachments
        // (e.g. a brigade HQ with units attached to it) may not detach-and-react unless those units
        // also detach, which we do not orchestrate mid-reaction, so it is refused here. (The §8.54
        // "parent becomes a pinned shell" and "no detaching low-CPA units purely to free the parent"
        // guards concern the *parent's* own reaction and are documented simplifications.)
        let detaches = self.unit(unit).and_then(|u| u.attached_parent).is_some();
        if detaches && self.units.values().any(|o| o.attached_parent == Some(unit)) {
            return Err(CommandError::CannotReact);
        }
        // §8.55: reaction moves normally but may never enter an enemy ZOC (and expends no
        // Break-Off cost, §8.52). §8.56: the detachment CP is charged with the reaction CP.
        let mut prev = start;
        let mut total_cp = if detaches && !path.is_empty() { Fixed(10) } else { Fixed(0) };
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
            // §8.56: reacting away from the parent's counter completes the detachment.
            if detaches {
                self.unit_mut(unit).expect("checked").attached_parent = None;
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
        // §13.28: flag the unit so an ensuing assault in its hex excludes its Close Assault
        // ratings / bars it from directing anti-armor fire (its TOE still takes % losses).
        self.unit_mut(unit).expect("checked").retreated_before_assault = true;
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
        // §16.17/§16.22/§16.21: the target must be a non-storm, enemy-occupied hex reachable by a
        // legal ≤5-hex Patrol path from every origin (passable terrain, not through an
        // enemy-occupied/controlled hex), and not barraged/bombed this stage.
        if self.map.terrain(target).is_none() || storm(self, target) {
            return Err(CommandError::PatrolTargetInvalid);
        }
        if self.barraged_hexes_this_stage.contains(&target) {
            return Err(CommandError::PatrolTargetInvalid);
        }
        if per_hex.keys().any(|&h| !self.patrol_path_exists(h, target, enemy)) {
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
        // §19.1 invariant: an attached unit rides its parent's counter and has NO independent
        // location — its physical hex is resolved via `effective_hex`. Clearing `location` here (it
        // was equal to the parent's hex at attach time) keeps own-hex from going stale when the
        // parent later moves, so `effective_hex` stays the single source of truth.
        {
            let u = self.unit_mut(unit).expect("checked");
            u.attached_parent = Some(parent);
            u.location = None;
        }
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
        // On detach the unit regains its own counter at the stack's current physical hex (§19.43).
        let hex = self.effective_hex(parent);
        {
            let u = self.unit_mut(unit).expect("checked");
            u.attached_parent = None;
            u.location = hex.map(crate::state::unit::UnitLocation::OnMap);
        }
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
        // §19.33 (German Formation Organization Chart) text-stated constraints — the per-slot numeric
        // type maxima are deferred (see docs/deferred-items.md D2), but two rules are unambiguous:
        if assigned != Some(parent) {
            use crate::reference::units::Nationality;
            use crate::state::unit::OrgSize;
            let (p_nat, p_size, p_name) = {
                let p = self.unit(parent).ok_or(CommandError::UnitNotFound)?;
                (p.nationality, p.org_size, p.name.clone())
            };
            if p_nat == Nationality::German {
                // "No Battle Group may be assigned to a division."
                let u_size = self.unit(unit).expect("checked").org_size;
                if u_size == OrgSize::BattleGroup && p_size == OrgSize::Division {
                    return Err(CommandError::ReorgIllegal);
                }
                // "The [15th Infantry] brigade may never contain more than six battalions of infantry."
                if p_name.contains("15th Schutzen Bde")
                    && self.unit(unit).expect("checked").org_size == OrgSize::Battalion
                    && self.unit_is_infantry(unit)
                {
                    let inf_bns = self
                        .units
                        .values()
                        .filter(|x| x.assigned_parent == Some(parent) && x.org_size == OrgSize::Battalion && x.total_toe() > 0)
                        .map(|x| x.id)
                        .filter(|&id| self.unit_is_infantry(id))
                        .count() as u32;
                    if inf_bns >= 6 {
                        return Err(CommandError::ReorgIllegal);
                    }
                }
            }
            // §19.3 Formation Organization Chart per-type maxima (German + Italian rows modeled).
            if !self.formation_org_assign_ok(parent, unit) {
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
        // §28.13: CPA 8 (80 tenths). Entering `to` costs its real foot (non-motorized) movement
        // cost — the guarded column marches, paying terrain, not a flat 1 CP per hex.
        let step = match self.move_step_cost(from, to, MoveMode::NonMotorized) {
            Some(MoveCost::Cost(f)) => f.0.max(0) as u32,
            _ => return Err(CommandError::PrisonerActionIllegal), // prohibited / off-map
        };
        if self.prisoner_groups[gi].cpa_used + step > 80 {
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
        self.prisoner_groups[gi].cpa_used += step;
        // §28.16: reaching a Departure Point removes them permanently.
        if self.is_pow_departure(to, side) {
            self.prisoner_groups.remove(gi);
        }
        Ok(CommandOutcome::PrisonersMoved { escaped })
    }

    /// §28.3/§22.5 repair a piece of captured equipment (`system`) at `hex`, returning it to play.
    /// A repaired vehicle is "treated as Replacement Points for that type of unit ... except that
    /// they do not have to undergo Training" (§22.5): the captured TOE becomes a trained Replacement
    /// Point pool of the matching class at `hex`, available to the captor to rebuild against the
    /// former owner (§28.3). The `count` is 1 Replacement Point per captured TOE Strength Point.
    fn cmd_repair_captured_equipment(&mut self, hex: HexCoord, system: String) -> Result<CommandOutcome, CommandError> {
        self.require_phase(Phase::Repair)?;
        let side = self.active_player;
        let idx = self
            .captured_equipment
            .iter()
            .position(|e| e.captor == side && e.hex == hex && e.system == system && !e.repaired)
            .ok_or(CommandError::PrisonerActionIllegal)?;
        let (owner, count, destroyed) = {
            let e = &self.captured_equipment[idx];
            (e.nationality, e.count, e.destroyed_tank)
        };
        // §14.54/§22.62: a Destroyed Tank marker is not field-repairable just anywhere — it needs a
        // Repair Facility (reached by towing, §14.54) or a co-located same-nationality Tank Delivery
        // Squadron, which repairs a destroyed tank only on a die roll of 1 (§22.62). Surrendered
        // captured equipment (§28.3, `destroyed_tank == false`) repairs as before.
        if destroyed && !self.repair_facilities.contains_key(&hex) {
            let tds_here = self
                .units
                .values()
                .any(|u| u.is_tds && u.hex() == Some(hex) && u.nationality == owner);
            if !tds_here || self.rng.d6() != 1 {
                return Err(CommandError::RepairFailed);
            }
        }
        self.captured_equipment[idx].repaired = true;
        // §22.5: enter the Replacement Point pool already trained (no Training clock).
        let class = self.replacement_class_for_system(owner, &system);
        let ri = self.add_replacement_points(class, count, side, Some(crate::state::unit::UnitLocation::OnMap(hex)), None);
        self.replacement_points[ri].training_remaining = 0;
        Ok(CommandOutcome::CapturedEquipmentRepaired)
    }

    /// §17.3 designate a Commonwealth unit to Train this Operations Stage (raising its Basic Morale
    /// toward its trained reference, §17.31). Validates: a green Commonwealth unit (current Basic
    /// Morale below its reference); in a §17.32 training area (a `Feature::TrainingArea` hex — see
    /// `is_training_area`); no Capability
    /// Points expended this stage (§17.33); and a co-located §17.35 instruction
    /// battalion — a fully-trained Commonwealth combat battalion of ≥¼ its assigned TOE and matching
    /// arm (a Tank battalion for an armored trainee, else an Infantry battalion). Sets `in_training`;
    /// the +1 Basic Morale after every six uninterrupted stages (§17.34) is applied at stage rollover.
    /// §12.11/§12.12: set an artillery unit's gun position (Forward/Back) for the coming Combat
    /// Segment. Only a gun/artillery unit has a meaningful position.
    fn cmd_set_gun_position(&mut self, unit: UnitId, forward: bool) -> Result<CommandOutcome, CommandError> {
        use crate::state::unit::GunPosition;
        if !matches!(self.phase, Phase::Organization | Phase::MovementAndCombat) {
            return Err(CommandError::WrongPhase);
        }
        if self.unit(unit).is_none() {
            return Err(CommandError::UnitNotFound);
        }
        if !self.is_gun_unit(unit) {
            return Err(CommandError::NotArtillery);
        }
        self.unit_mut(unit).expect("checked").gun_position =
            if forward { GunPosition::Forward } else { GunPosition::Back };
        Ok(CommandOutcome::GunPositionSet)
    }

    fn cmd_train_unit(&mut self, unit: UnitId) -> Result<CommandOutcome, CommandError> {
        use crate::reference::units::Side;
        use crate::state::unit::OrgSize;
        self.require_phase(Phase::Organization)?;
        let u = self.unit(unit).ok_or(CommandError::UnitNotFound)?;
        let hex = u.hex().ok_or(CommandError::UnitNotOnMap)?;
        // §17.3 (Commonwealth-only) / §17.31 (must be green) / §17.33 (has not yet acted this stage).
        if u.nationality.side() != Side::Commonwealth
            || u.basic_morale >= u.basic_morale_reference
            || u.cp_expended.0 != 0
        {
            return Err(CommandError::TrainingIllegal);
        }
        // §17.32 training area (the Commonwealth's own list — Axis areas don't qualify, §20.43).
        if !self.is_training_area(hex, Side::Commonwealth) {
            return Err(CommandError::TrainingIllegal);
        }
        // §17.35 qualifying instruction battalion co-located.
        let armor = u.is_armor;
        let has_instructor = self.units.values().any(|b| {
            b.id != unit
                && b.nationality.side() == Side::Commonwealth
                && b.hex() == Some(hex)
                && self.effective_org_size(b.id) == OrgSize::Battalion
                && b.basic_morale >= b.basic_morale_reference
                && self.unit_toe_at_least_quarter(b.id)
                && if armor { self.unit_is_tank(b.id) } else { self.unit_is_infantry(b.id) }
        });
        if !has_instructor {
            return Err(CommandError::TrainingIllegal);
        }
        self.unit_mut(unit).expect("checked").status.in_training = true;
        let basic_morale = self.unit(unit).expect("checked").basic_morale;
        Ok(CommandOutcome::UnitTraining { basic_morale })
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
    /// Immutable validation + cost/flag computation for a single-hex [`Command::MoveStep`], shared
    /// by `move_counter` (which then executes) and `can_move_step` (the enumerator's fast path).
    /// Returns the CP cost and the flags execution needs, or the rejection reason — **without any
    /// mutation**, so it neither clones nor runs the move's side effects. The single source of truth
    /// for move legality: `move_counter` executes exactly the moves this accepts.
    fn plan_move_step(
        &self,
        unit: UnitId,
        to: HexCoord,
        mode: MoveMode,
        cpa: i32,
    ) -> Result<MoveStepPlan, CommandError> {
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
        // §12.44 Pinned / §6.26 Disorganized (cohesion ≤ −26): may not voluntarily move.
        {
            let u = self.unit(unit).expect("checked");
            if u.status.pinned {
                return Err(CommandError::Pinned);
            }
            if u.cohesion <= -26 {
                return Err(CommandError::Disorganized);
            }
            // §18.22: a Reserve II unit may never move; a Reserve I unit may move only one hex per
            // Operations Stage (CP-free), so once it has taken that hex it may not move again.
            match u.reserve {
                crate::state::unit::ReserveStatus::ReserveII => return Err(CommandError::InReserve),
                crate::state::unit::ReserveStatus::ReserveI if u.reserve_moved_this_stage => {
                    return Err(CommandError::InReserve)
                }
                _ => {}
            }
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
        // §8.13: a unit may not move into a hex occupied by an enemy combat unit (you attack it,
        // you do not enter it). Non-combat enemy markers do not block entry.
        if self.enemy_units_in_hex(to, enemy.opponent()).iter().any(|&e| self.is_combat_unit(e)) {
            return Err(CommandError::ProhibitedTerrain);
        }
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
                let is_engineer = |g: &Self, id: UnitId| {
                    g.unit(id)
                        .and_then(|u| {
                            g.reference
                                .unit_characteristics
                                .get(&u.nationality)
                                .and_then(|m| m.get(&u.id_code))
                        })
                        .is_some_and(|c| c.unit_type.contains("Engineer") || c.unit_type.contains("Headquarters"))
                };
                // §23.21: the reduced cost extends to any unit stacked (at its origin) with an
                // unpinned engineer / engineer-capable HQ, not just the engineer itself.
                let eng = is_engineer(self, unit)
                    || self.units_in_hex(from).iter().any(|&o| {
                        o != unit && is_engineer(self, o) && self.unit(o).is_some_and(|u| !u.status.pinned)
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
        let to_terrain = self.map.terrain(to).ok_or(CommandError::DestinationOffMap)?;
        // §8.45: Desert hexes are forbidden to Light Trucks and motorcycle units (too light for the
        // soft sand) — they may not enter, whether or not a track runs through.
        if to_terrain == crate::reference::terrain::HexTerrain::Desert
            && (self.unit_is_light_truck_borne(unit) || self.unit_is_motorcycle(unit))
        {
            return Err(CommandError::ProhibitedTerrain);
        }
        // §8.44: a Salt Marsh may be entered/left off-track only by Light Trucks, Recce units,
        // motorcycle infantry, or (as infantry) the camel; any other motorized unit or AFV must use
        // a Road or Track hexside.
        if to_terrain == crate::reference::terrain::HexTerrain::SaltMarsh {
            use crate::state::map::HexsideKind;
            let on_track = self.map.hexsides_on(from, to).iter().any(|h| {
                matches!(h.kind, HexsideKind::Road | HexsideKind::Track | HexsideKind::UnfinishedRoad)
            });
            let exempt = self.is_recce_unit(unit)
                || self.unit_is_motorcycle(unit)
                || self.unit_is_camel(unit)
                || self.unit_is_light_truck_borne(unit);
            let prohibited_vehicle = (self.unit_is_motorized(unit) || self.unit_is_afv(unit)) && !exempt;
            if prohibited_vehicle && !on_track {
                return Err(CommandError::ProhibitedTerrain);
            }
        }
        // §9.14/§9.31: may not end a move where total stacking points exceed the terrain
        // limit. (The mover is currently at `from`, so it is not yet counted at `to`.)
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
        // §9.25: at most five independent 0-Stacking-Point companies/batteries may stack in one hex
        // (unlimited only in a Major City). Attached/assigned 0-SP units ride their parent and don't
        // count. Applies when the mover is itself such a unit.
        let mover_zero_sp_company = self.effective_stacking_points(unit) == 0
            && self.effective_org_size(unit) == crate::state::unit::OrgSize::Company
            && self.unit(unit).is_some_and(|u| u.attached_parent.is_none());
        if mover_zero_sp_company && to_terrain != crate::reference::terrain::HexTerrain::MajorCity {
            let existing = self
                .units_in_hex(to)
                .into_iter()
                .filter(|&o| {
                    o != unit
                        && self.effective_stacking_points(o) == 0
                        && self.effective_org_size(o) == crate::state::unit::OrgSize::Company
                        && self.unit(o).is_some_and(|u| u.attached_parent.is_none())
                })
                .count();
            if existing >= 5 {
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
        // §18.23.1/§18.24.1: a unit released from Reserve this stage may not voluntarily exceed its
        // CPA — the full CPA for Reserve I, one-half (rounded down) for Reserve II.
        if let Some(level) = self.unit(unit).and_then(|u| u.released_from_reserve) {
            let cap = if level == crate::state::unit::ReserveStatus::ReserveII { cpa / 2 } else { cpa };
            let already = self.unit(unit).map(|u| u.cp_expended.0).unwrap_or(0);
            if already + cp.0 > cap * 10 {
                return Err(CommandError::InReserve);
            }
        }
        // §49.13/49.16: the move's Fuel must be payable from the mover's (and riders') tanks and any
        // start-hex Supply Dump; a move with no available Fuel is illegal. (Availability only —
        // `move_counter` performs the actual consumption.)
        if !self.move_fuel_available(unit, from, cp.0) {
            return Err(CommandError::InsufficientFuel);
        }
        Ok(MoveStepPlan { cp, to_zoc, entered_enemy_dummy, mine_casualty_check })
    }

    /// Whether `unit` may legally step to adjacent `to` in `mode` in the current state — the action
    /// enumerator's fast path, sharing `plan_move_step`'s validation with the command (no clone/apply).
    pub fn can_move_step(&self, unit: UnitId, to: HexCoord, mode: MoveMode) -> bool {
        let cpa = self.effective_cpa(unit).unwrap_or(0);
        self.plan_move_step(unit, to, mode, cpa).is_ok()
    }

    /// Move a single counter one hex (§8): validate via `plan_move_step`, then pay Fuel/CP and apply
    /// the move's side effects (rail control, minefield reveal/casualty, breakdown, ZOC-stop, and the
    /// §8.62/§8.67 contact refresh).
    fn move_counter(
        &mut self,
        unit: UnitId,
        to: HexCoord,
        mode: MoveMode,
        cpa: i32,
    ) -> Result<CommandOutcome, CommandError> {
        let MoveStepPlan { cp, to_zoc, entered_enemy_dummy, mine_casualty_check } =
            self.plan_move_step(unit, to, mode, cpa)?;
        let from = self
            .unit(unit)
            .expect("validated by plan_move_step")
            .hex()
            .expect("validated by plan_move_step");
        // §49.13/49.16: pay the move's Fuel (availability already verified by plan_move_step).
        let paid = self.consume_move_fuel(unit, from, cp.0);
        debug_assert!(paid, "plan_move_step verified fuel availability");
        // Apply: move and accumulate CP expended this Operations Stage (with any
        // Disorganization for exceeding the governing CPA).
        self.unit_mut(unit)
            .expect("unit existence checked above")
            .location = Some(crate::state::unit::UnitLocation::OnMap(to));
        self.expend_cp_against(unit, cp, cpa);
        // §18.22: a Reserve I unit spends its single one-hex move for the stage.
        if self.unit(unit).is_some_and(|u| u.reserve == crate::state::unit::ReserveStatus::ReserveI) {
            self.unit_mut(unit).expect("checked").reserve_moved_this_stage = true;
        }
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
        // §6.26: a Disorganized enemy unit (Cohesion ≤ −26) that the mover is now adjacent to
        // surrenders, regardless of the mover's size.
        if self.is_combat_unit(unit) {
            let mover_side = self.unit(unit).map(|u| u.nationality.side());
            if let Some(ms) = mover_side {
                let surrender: Vec<UnitId> = to
                    .neighbors()
                    .into_iter()
                    .flat_map(|h| self.units_in_hex(h))
                    .filter(|&e| {
                        self.units.get(&e).is_some_and(|u| {
                            u.nationality.side() != ms && u.total_toe() > 0 && u.cohesion <= -26
                        })
                    })
                    .collect();
                if !surrender.is_empty() {
                    self.surrender_units(&surrender);
                }
                // §35.12: an enemy SGSU the mover is now on or adjacent to, with no friendly combat
                // unit stacked with it, cannot react during the enemy's move and is eliminated;
                // §35.27: its grounded planes are captured. (§35.13: it may be redeployed one
                // Game-Turn later.) The destination hex itself is included — a unit moving directly
                // onto an undefended SGSU overruns it, not just one it ends up beside.
                let overrun: Vec<crate::state::air::SquadronId> = std::iter::once(to)
                    .chain(to.neighbors())
                    .flat_map(|h| {
                        self.sgsus
                            .iter()
                            .filter(|(_, sg)| {
                                sg.location == Some(h) && sg.nationality.side() != ms
                            })
                            .filter(|(_, sg)| {
                                !self.units_in_hex(h).iter().any(|&u| {
                                    self.units.get(&u).is_some_and(|x| {
                                        x.nationality.side() == sg.nationality.side() && x.total_toe() > 0
                                    }) && self.is_combat_unit(u)
                                })
                            })
                            .map(|(id, _)| *id)
                            .collect::<Vec<_>>()
                    })
                    .collect();
                let capture_gt = self.game_turn + 1;
                for id in overrun {
                    if let Some(sg) = self.sgsus.get_mut(&id) {
                        sg.location = None;
                        // §35.27: grounded planes are captured by the overrunning side (usable one
                        // Game-Turn later), not simply destroyed.
                        let planes = std::mem::take(&mut sg.planes);
                        if !planes.is_empty() {
                            self.captured_planes.push(crate::state::game::CapturedPlanes {
                                captor: ms,
                                planes,
                                available_game_turn: capture_gt,
                            });
                        }
                    }
                }
                // §22.63: a lone enemy Tank Delivery Squadron the mover (a combat unit) places in
                // its ZOC — no friendly combat unit stacked with it — is eliminated; it may return
                // to play eight Game-Turns later.
                if self.is_combat_unit(unit) {
                    let tds_hit: Vec<UnitId> = to
                        .neighbors()
                        .into_iter()
                        .flat_map(|h| {
                            self.units
                                .values()
                                .filter(|t| {
                                    t.is_tds && t.hex() == Some(h) && t.nationality.side() != ms
                                })
                                .filter(|t| {
                                    !self.units_in_hex(h).iter().any(|&u| {
                                        self.units.get(&u).is_some_and(|x| {
                                            x.nationality.side() == t.nationality.side() && x.total_toe() > 0
                                        }) && self.is_combat_unit(u)
                                    })
                                })
                                .map(|t| t.id)
                                .collect::<Vec<_>>()
                        })
                        .collect();
                    let return_gt = self.game_turn + 8;
                    for id in tds_hit {
                        if let Some(t) = self.units.get_mut(&id) {
                            t.location = None;
                            t.tds_return_game_turn = Some(return_gt);
                            t.tds_tank_reserve = 0; // any held reserves are lost with it
                        }
                    }
                }
            }
        }
        // §21.52: a moving battalion-or-larger combat unit captures adjacent unprotected enemy
        // Broken Down Vehicle markers.
        self.capture_broken_vehicles(unit);
        // §8.62/§8.67: recompute Contact from ZOC and clear any engagement this move broke off.
        self.refresh_contact_status();
        Ok(CommandOutcome::Moved)
    }
}

/// The validated cost and side-effect flags of a single-hex move, produced by
/// `GameState::plan_move_step` and consumed by `move_counter` when it executes.
struct MoveStepPlan {
    /// Total CP cost (tenths) of the step, after §8.6 contact/engagement and §26 minefield surcharges.
    cp: Fixed,
    /// The destination is in an enemy ZOC (§8.14: the unit must then stop for the stage).
    to_zoc: bool,
    /// The step enters an enemy **dummy** minefield (§26.15/26.23: revealed on entry).
    entered_enemy_dummy: bool,
    /// A non-engineer vehicle entered an enemy minefield (§26.25: roll a mine casualty).
    mine_casualty_check: bool,
}
