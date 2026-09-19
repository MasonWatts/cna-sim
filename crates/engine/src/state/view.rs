//! Per-player observation layer (§3.5 hidden information / D5).
//!
//! CNA leans hard on secrecy: a player sees the enemy's *counters* on the map but not their current
//! TOE Strength, cannot tell a real Supply Dump from a dummy, and never sees a hidden desert raider
//! until it raids or is spotted (§27.32/§27.33). The core `GameState` holds ground truth; this
//! module derives a *thin* per-side view of what a player is entitled to know, so an AI agent (or a
//! UI) acts on knowledge rather than omniscience. Nothing here mutates state.

use crate::reference::units::{Nationality, Side};
use crate::state::game::{Commodities, GameState, RaiderKind};
use crate::state::map::HexCoord;
use crate::state::unit::{OrgSize, UnitId, UnitLocation};

/// Tunable weights for [`GameState::heuristic_value_with`]. Each field scales one eval term; the
/// `Default` reproduces the original hand-picked blend. A self-play harness (`agent::tune_weights`)
/// searches this space for a stronger opponent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvalWeights {
    /// Per net TOE Strength point (own − enemy).
    pub material: i64,
    /// Per net §64.73 Geographical-Occupation Victory Point held.
    pub vp: i64,
    /// Per net Cohesion point (own − enemy).
    pub cohesion: i64,
    /// Subtracted per own combat unit that is out of supply.
    pub supply_penalty: i64,
    /// Subtracted per hex of summed distance from own units to the nearest enemy (advance pressure).
    pub closeness: i64,
}

impl Default for EvalWeights {
    fn default() -> Self {
        EvalWeights { material: 100, vp: 60, cohesion: 1, supply_penalty: 25, closeness: 1 }
    }
}

/// The §60.81 "Graziani's Offensive" victory level a side has achieved (Scenario 1). Ordered so a
/// higher variant is a better result. Both sides are evaluated independently; the game ends when a
/// side reaches its target level (typically Strategic).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScenarioVictory {
    None,
    Tactical,
    Decisive,
    Strategic,
}

/// What a viewing side knows about a unit (§3.5): the counter is visible (type, size, hex), but its
/// current TOE `strength` is secret unless the unit is the viewer's own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitView {
    pub id: UnitId,
    pub name: String,
    pub nationality: Nationality,
    pub org_size: OrgSize,
    pub is_hq: bool,
    pub hex: Option<HexCoord>,
    /// Current TOE Strength Points — `Some` only for the viewer's own units (§3.5 secrecy).
    pub strength: Option<u32>,
    /// Whether this is one of the viewer's own units.
    pub friendly: bool,
}

/// What a viewing side knows about a Supply Dump: a friendly dump shows its side, dummy status, and
/// contents; an enemy dump shows only that a dump marker is present (§32.18 — real vs dummy and the
/// stored commodities are hidden until raided/overrun).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DumpView {
    pub location: UnitLocation,
    pub side: Side,
    pub friendly: bool,
    /// Contents — `Some` only for a friendly dump.
    pub contents: Option<Commodities>,
    /// Dummy flag — `Some` only for a friendly dump (an enemy cannot tell).
    pub dummy: Option<bool>,
}

/// What a viewing side knows about a desert raider: its own raiders always; enemy raiders only once
/// revealed (§27.32/§27.33 — a hidden raider is off the map to the enemy).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RaiderView {
    pub kind: RaiderKind,
    pub side: Side,
    pub hex: Option<HexCoord>,
    pub friendly: bool,
}

/// What a viewing side knows about a minefield (§26): the marker's hex and owner are visible, but
/// whether it is real or a dummy is hidden until it is the viewer's own or has been revealed by an
/// enemy unit entering it (§26.15/§26.23).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinefieldView {
    pub hex: HexCoord,
    pub side: Side,
    pub friendly: bool,
    /// Dummy flag — `Some` only when the viewer owns the field or it has been revealed.
    pub dummy: Option<bool>,
}

impl GameState {
    /// §3.5: the units `viewer` can observe — every on-map counter (its own with full TOE Strength,
    /// the enemy's with `strength = None`). Off-map units (holding boxes) are included like the map.
    pub fn observe_units(&self, viewer: Side) -> Vec<UnitView> {
        self.units
            .values()
            .map(|u| {
                let friendly = u.nationality.side() == viewer;
                // §42.27: an enemy unit's TOE Strength is also known if air reconnaissance revealed
                // it to the viewer this Operations Stage.
                let known = friendly || self.recon_revealed.contains(&(viewer, u.id));
                UnitView {
                    id: u.id,
                    name: u.name.clone(),
                    nationality: u.nationality,
                    org_size: u.org_size,
                    is_hq: u.is_hq,
                    hex: u.hex(),
                    strength: known.then(|| u.total_toe()),
                    friendly,
                }
            })
            .collect()
    }

    /// §32.18: the Supply Dumps `viewer` can observe — friendly dumps in full; enemy dumps only as a
    /// present marker (contents and dummy status hidden).
    pub fn observe_supply_dumps(&self, viewer: Side) -> Vec<DumpView> {
        self.supply_dumps
            .iter()
            .map(|d| {
                let friendly = d.side == viewer;
                DumpView {
                    location: d.location.clone(),
                    side: d.side,
                    friendly,
                    contents: friendly.then_some(d.supply),
                    dummy: friendly.then_some(d.dummy),
                }
            })
            .collect()
    }

    /// §27.32/§27.33: the desert raiders `viewer` can observe — its own (wherever they are) plus any
    /// enemy raider that has been revealed by raiding or being spotted.
    pub fn observe_raiders(&self, viewer: Side) -> Vec<RaiderView> {
        self.raiders
            .iter()
            .filter(|r| r.hex.is_some())
            .filter(|r| r.side == viewer || r.revealed)
            .map(|r| RaiderView {
                kind: r.kind,
                side: r.side,
                hex: r.hex,
                friendly: r.side == viewer,
            })
            .collect()
    }

    /// §52.14: whether `viewer` knows a well at `hex` is depleted (`Some(true/false)`), or cannot
    /// tell (`None`). A side knows only its own secret (it drew the well dry) or a publicly revealed
    /// well. Wells the viewer has no knowledge of read `None` even when actually depleted.
    pub fn observe_well_depleted(&self, viewer: Side, hex: HexCoord) -> Option<bool> {
        if self.revealed_wells.contains(&hex) || self.well_known_by.get(&hex) == Some(&viewer) {
            Some(self.depleted_wells.contains(&hex))
        } else {
            None
        }
    }

    /// §52.14/§52.16: whether `viewer` knows a well at `hex` is poisoned (`Some`), or cannot tell
    /// (`None`) — same secrecy rule as depletion.
    pub fn observe_well_poisoned(&self, viewer: Side, hex: HexCoord) -> Option<bool> {
        if self.revealed_wells.contains(&hex) || self.well_known_by.get(&hex) == Some(&viewer) {
            Some(self.poisoned_wells.contains(&hex))
        } else {
            None
        }
    }

    /// §26: the minefields `viewer` can observe — the marker and owner of each, with its real/dummy
    /// status disclosed only for the viewer's own fields or ones already revealed (§26.15/§26.23).
    pub fn observe_minefields(&self, viewer: Side) -> Vec<MinefieldView> {
        self.minefields
            .iter()
            .map(|(&hex, mf)| {
                let known = mf.side == viewer || self.revealed_minefields.contains(&hex);
                MinefieldView {
                    hex,
                    side: mf.side,
                    friendly: mf.side == viewer,
                    dummy: known.then_some(mf.dummy),
                }
            })
            .collect()
    }

    /// Phase-2 agent support: enumerate the legal single-hex `MoveStep` commands for `side` in the
    /// current Movement & Combat phase — each on-map unit to each adjacent hex, in both movement
    /// modes. Every candidate is validated against the command boundary itself (applied to a clone),
    /// so the result contains only commands `apply` will accept: the engine is the correctness
    /// oracle, not a re-implementation of the movement gate.
    ///
    /// This is the first slice of a broader legal-action enumerator (combat, reorg, logistics, air
    /// categories extend it). It clones per candidate, which is fine for planning over a modest
    /// front; broad enumeration over the whole campaign map will want a dry-run/rollback mode on
    /// `GameState` rather than a full clone per candidate.
    pub fn legal_move_steps(&self, side: Side) -> Vec<crate::state::command::Command> {
        use crate::rules::movement::MoveMode;
        use crate::state::command::Command;
        use crate::state::game::Phase;

        let mut out = Vec::new();
        if self.phase != Phase::MovementAndCombat {
            return out;
        }
        let mut units: Vec<(UnitId, HexCoord)> = self
            .units
            .values()
            .filter(|u| u.nationality.side() == side)
            .filter_map(|u| Some((u.id, u.hex()?)))
            .collect();
        // Deterministic enumeration order (independent of HashMap iteration, which is randomized
        // per process) — so agents and tournaments reproduce across runs.
        units.sort_by_key(|(id, _)| id.0);
        for (unit, from) in units {
            for to in self.map.neighbors(from) {
                for mode in [MoveMode::NonMotorized, MoveMode::Motorized] {
                    // Fast path: `can_move_step` shares `plan_move_step`'s validation with the
                    // command but does not clone or execute — the enumerator's hot loop was
                    // dominated by the clone+apply the old `dry_run` paid per candidate.
                    if self.can_move_step(unit, to, mode) {
                        out.push(Command::MoveStep { unit, to, mode });
                    }
                }
            }
        }
        out
    }

    /// A heuristic value of the position from `side`'s perspective, for the lookahead / rollout /
    /// UCT agents. Higher is better for `side`. It combines:
    /// - **Material** (dominant): own minus enemy total TOE Strength, weighted heavily — inflicting
    ///   losses and preserving strength is the biggest lever.
    /// - **Objectives (§64.73)**: the net Geographical Occupation VP of the victory cities `side`
    ///   holds (with an in-supply combat unit) minus those the enemy holds — what wins the campaign.
    /// - **Cohesion**: own minus enemy summed Cohesion (a disorganized force fights poorly).
    /// - **Supply**: a small penalty per own combat unit that is out of supply (§64.73 adequacy).
    /// - **Closeness**: a tie-breaking term pressing toward contact — summed distance from each of
    ///   `side`'s units to the nearest enemy, subtracted so advancing nudges the score up.
    pub fn heuristic_value(&self, side: Side) -> i64 {
        self.heuristic_value_with(side, &EvalWeights::default())
    }

    /// As [`heuristic_value`](Self::heuristic_value) but with tunable term weights, so a self-play
    /// harness can search for a stronger blend. The five terms — material, objective VP, cohesion,
    /// out-of-supply penalty, and closeness — are each scaled by the matching field of `w`.
    pub fn heuristic_value_with(&self, side: Side, w: &EvalWeights) -> i64 {
        let mut own_toe = 0i64;
        let mut enemy_toe = 0i64;
        let mut own_cohesion = 0i64;
        let mut enemy_cohesion = 0i64;
        let mut own_hexes: Vec<HexCoord> = Vec::new();
        let mut enemy_hexes: Vec<HexCoord> = Vec::new();
        let mut out_of_supply = 0i64;
        for u in self.units.values() {
            let t = u.total_toe() as i64;
            if u.nationality.side() == side {
                own_toe += t;
                own_cohesion += u.cohesion as i64;
                if let Some(h) = u.hex() {
                    own_hexes.push(h);
                    if t > 0 && self.is_combat_unit(u.id) && !self.unit_supply_adequate(u.id) {
                        out_of_supply += 1;
                    }
                }
            } else {
                enemy_toe += t;
                enemy_cohesion += u.cohesion as i64;
                if let Some(h) = u.hex() {
                    enemy_hexes.push(h);
                }
            }
        }
        let material = (own_toe - enemy_toe) * w.material;

        // §64.73 objectives: net VP of held victory cities (an in-supply combat unit occupies).
        let mut vp = 0i64;
        if let Some(cv) = self.reference.campaign_victory.as_ref() {
            for city in &cv.cities {
                let Some(hex) = crate::state::map::HexId::parse(&city.hex).map(|h| h.to_hex_coord()) else { continue };
                let (own_vp, enemy_vp) = match side {
                    Side::Commonwealth => (city.cw as i64, city.axis as i64),
                    Side::Axis => (city.axis as i64, city.cw as i64),
                };
                match self.victory_hex_holder(hex) {
                    Some(s) if s == side => vp += own_vp,
                    Some(_) => vp -= enemy_vp,
                    None => {}
                }
            }
        }

        let mut advance = 0i64;
        if !enemy_hexes.is_empty() {
            for oh in &own_hexes {
                advance += enemy_hexes.iter().map(|e| e.distance(*oh)).min().unwrap_or(0) as i64;
            }
        }

        material + vp * w.vp + (own_cohesion - enemy_cohesion) * w.cohesion
            - out_of_supply * w.supply_penalty
            - advance * w.closeness
    }

    /// §60.81 Scenario-1 ("Graziani's Offensive") victory level for `side`. Each level requires
    /// holding a set of named locations with a supplied combat unit (`victory_hex_holder`); the
    /// highest independently-satisfied level is returned. Objective hexes are per the §60.81 text /
    /// city index; the specific "supply by convoy from Tobruk / Cairo" trace is approximated by the
    /// general §64.73 supply-adequacy test (a documented simplification).
    pub fn scenario1_victory(&self, side: Side) -> ScenarioVictory {
        let holds = |ids: &[&str]| {
            ids.iter().all(|id| {
                crate::state::map::HexId::parse(id)
                    .map(|h| h.to_hex_coord())
                    .is_some_and(|hex| self.victory_hex_holder(hex) == Some(side))
            })
        };
        let holds_any = |ids: &[&str]| {
            ids.iter().any(|id| {
                crate::state::map::HexId::parse(id)
                    .map(|h| h.to_hex_coord())
                    .is_some_and(|hex| self.victory_hex_holder(hex) == Some(side))
            })
        };
        match side {
            Side::Axis => {
                if holds_any(&["E3613", "E3714", "E1430"]) {
                    ScenarioVictory::Strategic // hold Alexandria or Cairo (any hex)
                } else if holds(&["D3714", "C0127"]) {
                    ScenarioVictory::Decisive // Mersa Matruh + Siwa
                } else if holds(&["C4131", "C4021", "C3019", "C1014"]) {
                    ScenarioVictory::Tactical // Sidi Barrani + Sollum + Fort Maddalena + Giarabub
                } else {
                    ScenarioVictory::None
                }
            }
            Side::Commonwealth => {
                if holds(&["C4807"]) {
                    ScenarioVictory::Strategic // Tobruk
                } else if holds(&["C4321", "C3019", "C3618", "C4021", "C0127"]) {
                    ScenarioVictory::Decisive // Bardia + Fort Maddalena + Sidi Omar + Sollum + Siwa
                } else if holds(&["C4021", "C3922", "C0127"]) {
                    ScenarioVictory::Tactical // Sollum + Halfaya Pass + Siwa
                } else {
                    ScenarioVictory::None
                }
            }
        }
    }

    /// §64.73: which side (if any) "holds" `hex` for Geographical Occupation — the side with an
    /// in-supply combat unit there. `None` if the hex has no qualifying occupier.
    fn victory_hex_holder(&self, hex: HexCoord) -> Option<Side> {
        self.units_in_hex(hex).into_iter().find_map(|id| {
            let u = self.units.get(&id)?;
            (u.total_toe() > 0 && self.is_combat_unit(id) && self.unit_supply_adequate(id))
                .then_some(u.nationality.side())
        })
    }

    /// Test a command against the command boundary **without mutating** this state: clone, apply to
    /// the clone, and return the result. The reusable "would this be legal / what would it produce?"
    /// oracle for agents and the legal-action enumerator. The clone is O(1) — `Map` and
    /// `ReferenceData` are `Rc`-backed with copy-on-write, so probing a command that doesn't touch
    /// the map (moves, combat) shares both for free; the only real cost is running `apply` itself.
    pub fn dry_run(&self, cmd: &crate::state::command::Command) -> Result<crate::state::command::CommandOutcome, crate::state::command::CommandError> {
        self.clone().apply(cmd.clone())
    }

    /// Phase-2 agent support: enumerate the legal actions for `side` in the **current phase**,
    /// dispatching to a per-phase enumerator. Movement & Combat yields moves + close-assault +
    /// anti-armor + barrage; the Organization phase yields the bounded logistics/reorg set
    /// (refuel, rearm, attach, detach). Every candidate is validated via `dry_run`, so the result
    /// holds only commands `apply` accepts. Other phases (air, convoy, repair, …) are future
    /// extensions — the framework is here; each adds its own enumerator.
    pub fn legal_commands(&self, side: Side) -> Vec<crate::state::command::Command> {
        use crate::state::game::Phase;
        match self.phase {
            Phase::MovementAndCombat => self.legal_movement_combat_commands(side),
            Phase::Organization => self.legal_organization_commands(side),
            Phase::LandSupportAir => self.legal_air_commands(side),
            Phase::TruckConvoy => self.legal_convoy_commands(side),
            Phase::Repair => self.legal_repair_commands(side),
            Phase::CommonwealthFleet => self.legal_fleet_commands(side),
            Phase::NavalConvoyArrival => self.legal_naval_convoy_commands(side),
            Phase::NavalConvoySchedule => self.legal_convoy_planning_commands(side),
            _ => Vec::new(),
        }
    }

    /// Movement & Combat actions: single-hex moves plus, for each enemy-occupied hex adjacent to a
    /// friendly combat unit, a close assault, anti-armor fire, and a barrage per target class.
    fn legal_movement_combat_commands(&self, side: Side) -> Vec<crate::state::command::Command> {
        use crate::state::command::Command;

        let mut out = self.legal_move_steps(side);
        // Enemy-occupied hexes with at least one adjacent friendly combat unit.
        let enemy = side.opponent();
        let mut friendly: Vec<(UnitId, HexCoord)> = self
            .units
            .values()
            .filter(|u| u.nationality.side() == side && u.total_toe() > 0)
            .filter_map(|u| Some((u.id, u.hex()?)))
            .collect();
        // Deterministic attacker ordering (HashMap iteration is randomized per process).
        friendly.sort_by_key(|(id, _)| id.0);
        let mut enemy_hexes: Vec<HexCoord> = self
            .units
            .values()
            .filter(|u| u.nationality.side() == enemy && u.total_toe() > 0)
            .filter_map(|u| u.hex())
            .collect();
        enemy_hexes.sort_by_key(|h| (h.col, h.row));
        enemy_hexes.dedup();
        for target in enemy_hexes {
            let attackers: Vec<UnitId> = friendly
                .iter()
                .filter(|(_, h)| h.is_adjacent(target))
                .map(|(id, _)| *id)
                .collect();
            if attackers.is_empty() {
                continue;
            }
            let mut candidates = vec![
                Command::CloseAssault { attackers: attackers.clone(), target },
                Command::AntiArmor { attackers: attackers.clone(), target_hex: target, withheld: Vec::new() },
            ];
            // §12.6 Barrage: one candidate per distinct target class present in the hex (fired
            // "blind" at the class, no specific-unit designation, §12.24).
            let mut classes: Vec<crate::reference::crt::BarrageTargetClass> = self
                .units_in_hex(target)
                .into_iter()
                .filter(|&id| self.unit(id).is_some_and(|u| u.nationality.side() == enemy && u.total_toe() > 0))
                .map(|id| self.barrage_class(id))
                .collect();
            classes.sort_by_key(|c| *c as u8);
            classes.dedup();
            for target_class in classes {
                candidates.push(Command::Barrage { attackers: attackers.clone(), target, target_class, target_unit: None });
            }
            for cmd in candidates {
                if self.dry_run(&cmd).is_ok() {
                    out.push(cmd);
                }
            }
        }

        // §31.4 Rommel: the Axis may step his counter to an adjacent hex (or retreat), and designate
        // a co-located Axis unit to receive his +5 CPA this stage. Agents otherwise never move him.
        if side == Side::Axis {
            if let Some(rhex) = self.rommel_hex() {
                for to in self.map.neighbors(rhex) {
                    for cmd in [Command::MoveRommel { to }, Command::RetreatRommel { to }] {
                        if self.dry_run(&cmd).is_ok() {
                            out.push(cmd);
                        }
                    }
                }
                let mut here: Vec<UnitId> = self
                    .units_in_hex(rhex)
                    .into_iter()
                    .filter(|&id| self.unit(id).is_some_and(|u| u.nationality.side() == Side::Axis))
                    .collect();
                here.sort_by_key(|id| id.0);
                for unit in here {
                    let cmd = Command::DesignateRommelUnit { unit };
                    if self.dry_run(&cmd).is_ok() {
                        out.push(cmd);
                    }
                }
            }
        }

        // §6.15 formation move: a parent counter (HQ + its attached units) advances one hex as a
        // body. Without this the agent can only move independent counters, so whole divisions —
        // and units riding a parent — sit dormant. `dry_run` enforces the formation CPA (slowest
        // member) and stacking.
        {
            use crate::rules::movement::MoveMode;
            let mut formations: Vec<(UnitId, HexCoord)> = self
                .units
                .values()
                .filter(|u| u.nationality.side() == side && self.has_attached_units(u.id))
                .filter_map(|u| Some((u.id, u.hex()?)))
                .collect();
            formations.sort_by_key(|(id, _)| id.0);
            for (formation, from) in formations {
                for to in self.map.neighbors(from) {
                    for mode in [MoveMode::NonMotorized, MoveMode::Motorized] {
                        let cmd = Command::FormationMove { formation, to, mode };
                        if self.dry_run(&cmd).is_ok() {
                            out.push(cmd);
                        }
                    }
                }
            }
        }

        // §30.21/§30.35 off-shore bombardment: a heavy Commonwealth ship at sea shells an adjacent
        // enemy-held coastal hex, per target class present. (The Fleet is Commonwealth-only.)
        if side == Side::Commonwealth {
            use crate::state::naval::ShipLocation;
            for (ship_index, ship) in self.fleet.iter().enumerate() {
                let ShipLocation::AtSea(sea) = ship.location else { continue };
                if !ship.is_heavy() {
                    continue;
                }
                for target in self.map.neighbors(sea) {
                    let mut classes: Vec<crate::reference::crt::BarrageTargetClass> = self
                        .units_in_hex(target)
                        .into_iter()
                        .filter(|&id| self.unit(id).is_some_and(|u| u.nationality.side() == enemy && u.total_toe() > 0))
                        .map(|id| self.barrage_class(id))
                        .collect();
                    classes.sort_by_key(|c| *c as u8);
                    classes.dedup();
                    for target_class in classes {
                        let cmd = Command::OffShoreBombard { ship_index, target, target_class };
                        if self.dry_run(&cmd).is_ok() {
                            out.push(cmd);
                        }
                    }
                }
            }
        }

        // Attacks on the Commonwealth Fleet — Axis-only, since the fleet is Commonwealth. Each
        // candidate is validated via dry_run (which clones, so no RNG/state side effect here).
        if side == Side::Axis {
            use crate::state::naval::ShipLocation;
            for (ship_index, ship) in self.fleet.iter().enumerate() {
                // §30.35: each Axis gun unit co-located with a ship at sea may fire at it.
                if let ShipLocation::AtSea(hex) = ship.location {
                    for id in self.units_in_hex(hex) {
                        if self.unit(id).is_some_and(|u| u.nationality.side() == Side::Axis && u.total_toe() > 0) {
                            let cmd = Command::ArtilleryBombardShip { attackers: vec![id], ship_index };
                            if self.dry_run(&cmd).is_ok() {
                                out.push(cmd);
                            }
                        }
                    }
                }
                // §30.39: the San Giorgio firing at a ship in an adjacent sea hex.
                let cmd = Command::SanGiorgioAttackShip { ship_index };
                if self.dry_run(&cmd).is_ok() {
                    out.push(cmd);
                }
            }
            // §30.4: the once-per-game Italian Chariot raid on Alexandria (plan, then execute).
            for cmd in [Command::PlanChariotRaid, Command::ExecuteChariotRaid] {
                if self.dry_run(&cmd).is_ok() {
                    out.push(cmd);
                }
            }
        }
        // §27.77/§30.5: a Layforce battalion (Commonwealth) at Alexandria may embark onto the fleet.
        if side == Side::Commonwealth {
            for &id in self.units.keys() {
                if self.is_layforce(id) {
                    let cmd = Command::EmbarkLayforce { unit: id };
                    if self.dry_run(&cmd).is_ok() {
                        out.push(cmd);
                    }
                }
            }
        }
        // §8.8: an off-map unit (Tripoli/Tunisia complex, Axis-only) may march one Operations Stage
        // toward the map (Nofilia) or inland. dry_run enforces the §8.84 transit/stage rules.
        for u in self.units.values() {
            if u.nationality.side() == side
                && matches!(u.location, Some(UnitLocation::OffMap { .. }))
            {
                for direction in [crate::state::game::OffMapDir::TowardMap, crate::state::game::OffMapDir::Inland] {
                    let cmd = Command::MoveOffMap { unit: u.id, direction };
                    if self.dry_run(&cmd).is_ok() {
                        out.push(cmd);
                    }
                }
            }
        }

        // §27 desert raiders — roll-free now that dice moved inside the boundary. An own raider may
        // move a hex and attempt each raid type; the enemy may try to spot a raider (§27.44). All
        // dry_run-gated so only situationally-legal candidates survive.
        for (raider, r) in self.raiders.iter().enumerate() {
            if r.side == side {
                if let Some(rhex) = r.hex {
                    for to in self.map.neighbors(rhex) {
                        let cmd = Command::MoveRaider { raider, to };
                        if self.dry_run(&cmd).is_ok() {
                            out.push(cmd);
                        }
                    }
                }
                for cmd in [
                    Command::RaidPipeline { raider },
                    Command::RaidAirfield { raider },
                    Command::RaidPlanes { raider },
                    Command::RaidSupplyDump { raider },
                    Command::RaidRommel { raider },
                    Command::ResupplyRaider { raider },
                ] {
                    if self.dry_run(&cmd).is_ok() {
                        out.push(cmd);
                    }
                }
                for convoy in 0..self.truck_convoys.len() {
                    for cmd in [
                        Command::RaidTruckConvoy { raider, convoy },
                        Command::InterceptTruckConvoy { raider, convoy },
                    ] {
                        if self.dry_run(&cmd).is_ok() {
                            out.push(cmd);
                        }
                    }
                }
            } else {
                let cmd = Command::SpotRaider { raider };
                if self.dry_run(&cmd).is_ok() {
                    out.push(cmd);
                }
            }
        }

        // §27.9: an own SAS unit raids grounded planes at the airfield in its hex.
        // §52.16/§52.17: an own unit poisons or sweetens the well in its hex.
        let mut own_hexes: Vec<HexCoord> = Vec::new();
        for u in self.units.values() {
            if u.nationality.side() != side {
                continue;
            }
            if self.is_sas(u.id) {
                let cmd = Command::RaidAirfieldSas { unit: u.id };
                if self.dry_run(&cmd).is_ok() {
                    out.push(cmd);
                }
            }
            if let Some(h) = u.hex() {
                own_hexes.push(h);
            }
        }
        own_hexes.sort_by_key(|h| (h.col, h.row));
        own_hexes.dedup();
        for hex in own_hexes {
            // Only a hex with a poisonable well (§52.7 Village/Bir) is a real target — the commands
            // are otherwise accepted-but-no-op. Poison a clean well; sweeten a poisoned one.
            use crate::state::game::WaterKind;
            if !matches!(self.hex_water_source(hex), Some(WaterKind::Village | WaterKind::Bir)) {
                continue;
            }
            let poisoned = self.poisoned_wells.contains(&hex);
            let cmd = if poisoned { Command::SweetenWell { hex } } else { Command::PoisonWell { hex } };
            if self.dry_run(&cmd).is_ok() {
                out.push(cmd);
            }
        }
        out
    }

    /// Organization-phase actions: a bounded logistics/reorg set for `side`. For each on-map own
    /// unit — refuel (§49) and rearm (§50) from a co-located dump, and detach from a parent (§19.43);
    /// and for co-located own pairs, attach one to the other (§19.41). Each candidate is validated
    /// via `dry_run`, so only those the command boundary accepts survive (e.g. refuel appears only
    /// where a unit actually needs and can draw fuel). The supply term in `heuristic_value` gives
    /// value-based agents a reason to use these (refuelling clears an out-of-supply penalty).
    fn legal_organization_commands(&self, side: Side) -> Vec<crate::state::command::Command> {
        use crate::state::command::Command;

        // Own on-map units, grouped by hex for the co-located attach pairs.
        let own: Vec<(UnitId, HexCoord)> = self
            .units
            .values()
            .filter(|u| u.nationality.side() == side)
            .filter_map(|u| Some((u.id, u.hex()?)))
            .collect();
        let mut out = Vec::new();
        // §27.13: a raider (Commonwealth LRDG / Axis DSA) is formed from a Recce/AC unit.
        let raider_kind = match side {
            Side::Commonwealth => crate::state::game::RaiderKind::Lrdg,
            Side::Axis => crate::state::game::RaiderKind::Dsa,
        };
        for &(unit, _) in &own {
            for cmd in [
                Command::RefuelUnit { unit },
                Command::RearmUnit { unit },
                Command::DetachUnit { unit },
                Command::FormRaider { kind: raider_kind, from_unit: unit },
            ] {
                if self.dry_run(&cmd).is_ok() {
                    out.push(cmd);
                }
            }
        }
        // Attach a unit to a co-located friendly unit. Only same-hex ordered pairs are candidates,
        // so group by hex first — O(Σ group²) — instead of scanning all own-unit pairs (O(units²),
        // which recomputed every action becomes an O(units³)/phase cliff once the army is large).
        let mut by_hex: std::collections::HashMap<HexCoord, Vec<UnitId>> = std::collections::HashMap::new();
        for &(unit, h) in &own {
            by_hex.entry(h).or_default().push(unit);
        }
        // Deterministic order: sort each co-located group by id (hex-group order does not affect the
        // candidate set, only its listing; sorting the units keeps pairing reproducible).
        for group in by_hex.values_mut() {
            if group.len() < 2 {
                continue;
            }
            group.sort_by_key(|u| u.0);
            for &unit in group.iter() {
                for &parent in group.iter() {
                    if unit != parent {
                        let cmd = Command::AttachUnit { unit, parent };
                        if self.dry_run(&cmd).is_ok() {
                            out.push(cmd);
                        }
                    }
                }
            }
        }
        // §44.18: the Commonwealth may divert AA (Light = +1 / Heavy = +4) to a specific Malta base —
        // once per month, capped at 48 Malta-wide — and (§44.15) freely redistribute already-diverted
        // flak between fields. dry_run enforces timing, cap, and availability; move candidates are
        // enumerated at single-point granularity per ordered field pair.
        if side == Side::Commonwealth {
            for base_index in 0..self.malta_bases.len() {
                for heavy in [false, true] {
                    let cmd = Command::DivertMaltaAA { base_index, heavy };
                    if self.dry_run(&cmd).is_ok() {
                        out.push(cmd);
                    }
                }
            }
            for from_base in 0..self.malta_bases.len() {
                for to_base in 0..self.malta_bases.len() {
                    if from_base == to_base {
                        continue;
                    }
                    for (light, heavy) in [(1, 0), (0, 1)] {
                        let cmd = Command::MoveMaltaFlak { from_base, to_base, light, heavy };
                        if self.dry_run(&cmd).is_ok() {
                            out.push(cmd);
                        }
                    }
                }
            }
        }
        out.extend(self.legal_air_setup_commands(side));
        out.extend(self.legal_reorg_commands(side));
        out
    }

    /// Organization-phase reorganisation, replacement, and construction actions (§17/§19/§20/§24/
    /// §64.75). Each candidate is `dry_run`-validated so only legal ones survive. Deterministic
    /// (units/hexes visited in sorted order). This gives agents the non-combat agency they otherwise
    /// lack: reassigning and reserving units, training, voluntary withdrawal, rebuilding from
    /// replacement pools, and building forts / minefields / pipelines.
    fn legal_reorg_commands(&self, side: Side) -> Vec<crate::state::command::Command> {
        use crate::state::command::{Command, CommandOutcome};
        use crate::state::game::Commodity;
        use crate::state::unit::ReserveStatus;

        let mut own: Vec<(UnitId, HexCoord)> = self
            .units
            .values()
            .filter(|u| u.nationality.side() == side)
            .filter_map(|u| Some((u.id, u.hex()?)))
            .collect();
        own.sort_by_key(|(id, _)| id.0);
        let mut out = Vec::new();

        for &(unit, _) in &own {
            // §19.21 reassign to a co-located friendly parent; §Reserve status; §17.3 training;
            // §64.75 voluntary withdrawal.
            for &(parent, _) in &own {
                if unit != parent {
                    let cmd = Command::ReassignUnit { unit, parent };
                    if self.dry_run(&cmd).is_ok() {
                        out.push(cmd);
                    }
                }
            }
            for status in [ReserveStatus::ReserveI, ReserveStatus::ReserveII, ReserveStatus::None] {
                let cmd = Command::DesignateReserve { unit, status };
                if self.dry_run(&cmd).is_ok() {
                    out.push(cmd);
                }
            }
            for cmd in [Command::TrainUnit { unit }, Command::WithdrawUnit { unit }] {
                if self.dry_run(&cmd).is_ok() {
                    out.push(cmd);
                }
            }
            // §24.3/§24.4/§52.2: build a minefield (real or dummy), or begin a pipeline, at the
            // builder's own hex.
            if let Some(h) = self.unit(unit).and_then(|u| u.hex()) {
                for cmd in [
                    Command::LayMinefield { builder: unit, hex: h, dummy: false },
                    Command::LayMinefield { builder: unit, hex: h, dummy: true },
                    Command::BuildPipeline { builder: unit, hex: h },
                    // §24.9: construct a real or dummy Supply Dump at the builder's hex.
                    Command::ConstructSupplyDump { unit, hex: h, dummy: false },
                    Command::ConstructSupplyDump { unit, hex: h, dummy: true },
                ] {
                    if self.dry_run(&cmd).is_ok() {
                        out.push(cmd);
                    }
                }
            }
            // §8.97/§8.98: detach all of a unit's first-line Truck Points into a new free convoy, or
            // release them to the side's free pool. Represented as "all of them" (one candidate each).
            if let Some(t) = self.unit(unit).map(|u| u.trucks) {
                if t.light + t.medium + t.heavy > 0 {
                    for cmd in [
                        Command::FormTruckConvoy { from_unit: unit, light: t.light, medium: t.medium, heavy: t.heavy },
                        Command::ReleaseTrucks { unit, light: t.light, medium: t.medium, heavy: t.heavy },
                    ] {
                        if self.dry_run(&cmd).is_ok() {
                            out.push(cmd);
                        }
                    }
                }
            }
            // §8.97: assign the side's whole free Truck-Point pool to this unit's first line.
            if let Some(pool) = self.truck_pools.get(&side).copied() {
                if pool.light + pool.medium + pool.heavy > 0 {
                    let cmd = Command::AssignTrucks { unit, light: pool.light, medium: pool.medium, heavy: pool.heavy };
                    if self.dry_run(&cmd).is_ok() {
                        out.push(cmd);
                    }
                }
            }
            // §53.14/§24.9: load a commodity onto the unit's first-line trucks from a co-located dump, or
            // off-load one into the hex. Probe the max with a huge amount; emit the moved quantity if > 0.
            if self.unit(unit).map(|u| u.trucks.light + u.trucks.medium + u.trucks.heavy > 0).unwrap_or(false) {
                for cm in [Commodity::Fuel, Commodity::Ammo, Commodity::Stores, Commodity::Water] {
                    if let Ok(CommandOutcome::TrucksLoaded(n)) = self.dry_run(&Command::LoadTrucks { unit, commodity: cm, amount: u32::MAX }) {
                        if n > 0 {
                            out.push(Command::LoadTrucks { unit, commodity: cm, amount: n });
                        }
                    }
                    if let Ok(CommandOutcome::TrucksUnloaded(n)) = self.dry_run(&Command::UnloadTrucks { unit, commodity: cm, amount: u32::MAX }) {
                        if n > 0 {
                            out.push(Command::UnloadTrucks { unit, commodity: cm, amount: n });
                        }
                    }
                }
            }
        }

        // §20.4 absorb a co-located, trained, assigned replacement pool into a unit.
        for (index, rp) in self.replacement_points.iter().enumerate() {
            let Some(rhex) = rp.location.as_ref().and_then(|l| l.hex()) else { continue };
            for &(unit, uhex) in &own {
                if uhex == rhex {
                    let cmd = Command::AbsorbReplacement { index, unit };
                    if self.dry_run(&cmd).is_ok() {
                        out.push(cmd);
                    }
                }
            }
        }

        // §24.4 build a Level-1 fortification: the co-located friendly units at a hex are the
        // builder set (dry_run enforces the §24.42 engineer + infantry requirement).
        let mut hexes: Vec<HexCoord> = own.iter().map(|(_, h)| *h).collect();
        hexes.sort_by_key(|h| (h.col, h.row));
        hexes.dedup();
        for h in hexes {
            let builders: Vec<UnitId> = own.iter().filter(|(_, uh)| *uh == h).map(|(id, _)| *id).collect();
            let cmd = Command::BuildFortification { builders, hex: h, level: 1 };
            if self.dry_run(&cmd).is_ok() {
                out.push(cmd);
            }
        }
        out
    }

    /// Organization-phase air build-up (§34.82 / §35.2 / §40.12 / §34.84 / §36.53): deploy available
    /// SGSU counters at friendly air facilities, assign pooled planes and pilots to own squadrons,
    /// base Commonwealth squadrons in off-map facilities, and honour due squadron withdrawals. Each
    /// candidate is `dry_run`-validated, so class/capacity/supply/timing rules gate the set. Squadrons
    /// are visited in id order for determinism.
    fn legal_air_setup_commands(&self, side: Side) -> Vec<crate::state::command::Command> {
        use crate::reference::squadron::SquadronClass;
        use crate::reference::units::Nationality;
        use crate::state::command::Command;

        let mut out = Vec::new();
        // Air facilities not in an enemy-occupied hex (a plausible friendly base, §34.82).
        let enemy_hexes = self.side_occupied_hexes(side.opponent());
        let facilities: Vec<HexCoord> = self
            .air_facilities
            .iter()
            .map(|f| f.hex)
            .filter(|h| !enemy_hexes.contains(h))
            .collect();
        // Deploy an available (or §34.82C/D-creatable) SGSU counter of a friendly nationality.
        for nat in [Nationality::Commonwealth, Nationality::German, Nationality::Italian] {
            if nat.side() != side {
                continue;
            }
            for &hex in &facilities {
                for class in [SquadronClass::Fighter, SquadronClass::Bomber] {
                    let cmd = Command::DeploySgsu { nationality: nat, class, at: hex };
                    if self.dry_run(&cmd).is_ok() {
                        out.push(cmd);
                    }
                }
            }
        }
        // Own squadrons in id order.
        let mut own_sgsus: Vec<crate::state::air::SquadronId> = self
            .sgsus
            .iter()
            .filter(|(_, s)| s.nationality.side() == side)
            .map(|(&id, _)| id)
            .collect();
        own_sgsus.sort_by_key(|s| s.0);
        for &squadron in &own_sgsus {
            let nat = self.sgsus[&squadron].nationality;
            // Assign pooled planes (one candidate per accepted aircraft type; fill remaining capacity).
            let remaining = self.sgsus[&squadron]
                .capacity(self.game_turn)
                .total()
                .saturating_sub(self.sgsus[&squadron].plane_count());
            if remaining > 0 {
                if let Some(pool) = self.plane_pool.get(&nat) {
                    let mut types: Vec<(String, u32)> =
                        pool.iter().filter(|(_, &n)| n > 0).map(|(t, &n)| (t.clone(), n)).collect();
                    types.sort();
                    for (aircraft_type, avail) in types {
                        let Some(class) = self
                            .reference
                            .aircraft_charts
                            .get(&nat)
                            .and_then(|c| c.get(&aircraft_type))
                            .map(|a| a.class)
                        else {
                            continue;
                        };
                        let cmd = Command::AssignSquadronPlanes {
                            squadron,
                            aircraft_type,
                            class,
                            count: remaining.min(avail),
                        };
                        if self.dry_run(&cmd).is_ok() {
                            out.push(cmd);
                        }
                    }
                }
            }
            // Assign a rated pilot from the pool (one candidate per available rating).
            if let Some(pp) = self.pilot_pool.get(&nat) {
                let mut ratings: Vec<u32> = pp.iter().filter(|(_, &n)| n > 0).map(|(&r, _)| r).collect();
                ratings.sort_unstable();
                for rating in ratings {
                    let cmd = Command::AssignPilot { rating, squadron };
                    if self.dry_run(&cmd).is_ok() {
                        out.push(cmd);
                    }
                }
            }
            // Honour a due §34.84 squadron withdrawal.
            let wcmd = Command::WithdrawSquadron { squadron };
            if self.dry_run(&wcmd).is_ok() {
                out.push(wcmd);
            }
            // Commonwealth: base a squadron at an off-map §36.53 facility.
            if nat == Nationality::Commonwealth {
                if let Some(chart) = self.reference.off_map_air_facilities.as_ref() {
                    let mut names: Vec<String> = chart.facilities.iter().map(|f| f.name.clone()).collect();
                    names.sort();
                    for facility in names {
                        let cmd = Command::BaseSquadronOffMap { squadron, facility };
                        if self.dry_run(&cmd).is_ok() {
                            out.push(cmd);
                        }
                    }
                }
            }
        }
        out
    }

    /// Land-Support-Air actions: for each of `side`'s squadrons, an air-reconnaissance mission
    /// (§42.27) over each enemy-occupied hex, and an offensive Combat Air Patrol (§40.2) over each
    /// friendly-occupied hex. Range/weather/capability all fall out of `dry_run` validation, so a
    /// grounded, out-of-range, or non-recon/-CAP squadron simply contributes nothing. (Bombing and
    /// strafing — which fan out over mission type and loadout — are the next air extensions.)
    fn legal_air_commands(&self, side: Side) -> Vec<crate::state::command::Command> {
        use crate::reference::aircraft::CapType;
        use crate::state::command::Command;

        use crate::reference::squadron::SquadronClass;
        let squadrons: Vec<(crate::state::air::SquadronId, u32)> = self
            .sgsus
            .iter()
            .filter(|(_, s)| s.nationality.side() == side && s.plane_count() > 0)
            .map(|(&id, s)| (id, s.plane_count()))
            .collect();
        // Fighter squadrons available to escort bombers (§45); `None` = fly unescorted.
        let mut escorts: Vec<Option<crate::state::air::SquadronId>> = vec![None];
        escorts.extend(
            self.sgsus
                .iter()
                .filter(|(_, s)| s.nationality.side() == side && s.class == SquadronClass::Fighter && s.plane_count() > 0)
                .map(|(&id, _)| Some(id)),
        );
        let enemy = side.opponent();
        let enemy_hexes = self.side_occupied_hexes(enemy);
        let own_hexes = self.side_occupied_hexes(side);
        use crate::reference::aircraft::BombingMission;
        use crate::rules::air::StrafeTarget;
        // The land-effect bombing missions (§41.3x) worth aiming at an enemy-held hex, and the
        // strafing targets (§40.5x) that hit ground forces there. Ship/harbor/air-facility bombing
        // and standalone Port strafing take special targets and are handled elsewhere.
        const BOMB_MISSIONS: [BombingMission; 6] = [
            BombingMission::Personnel,
            BombingMission::TruckConvoy,
            BombingMission::FlakDestruction,
            BombingMission::SupplyDump,
            BombingMission::CityFortification,
            BombingMission::Road,
        ];
        const STRAFE_TARGETS: [StrafeTarget; 6] = [
            StrafeTarget::Infantry,
            StrafeTarget::FirstLineTrucks,
            StrafeTarget::SupplyDump,
            StrafeTarget::GroundedAircraft,
            StrafeTarget::Tanks,
            StrafeTarget::WaterPipeline,
        ];

        let mut out = Vec::new();
        for (squadron, planes) in squadrons {
            for &center in &own_hexes {
                let cmd = Command::DesignateCap { squadron, center, cap_type: CapType::Offensive, planes };
                if self.dry_run(&cmd).is_ok() {
                    out.push(cmd);
                }
            }
            for &target in &enemy_hexes {
                let mut candidates = vec![Command::ReconHex { squadron, target, planes }];
                for mission in BOMB_MISSIONS {
                    // Parameter breadth: fly the mission unescorted or with each available fighter
                    // escort (§45), at a light or heavy per-plane bombload; dry_run rejects
                    // ineligible escorts (e.g. the §43.24 Crete rule) and over-capacity loads.
                    for &escort in &escorts {
                        if escort == Some(squadron) {
                            continue;
                        }
                        for bombload_per_plane in [10, 20] {
                            candidates.push(Command::FlyBombingMissionAt {
                                squadron, target, mission, planes, bombload_per_plane,
                                night: false, escort, abort_planes: 0,
                            });
                        }
                    }
                }
                for strafe_target in STRAFE_TARGETS {
                    candidates.push(Command::Strafe { squadron, target, strafe_target, planes });
                }
                for cmd in candidates {
                    if self.dry_run(&cmd).is_ok() {
                        out.push(cmd);
                    }
                }
            }
        }
        // Air maintenance (§38): ready (refuel), arm, and refit each own squadron so its assigned
        // planes become flyable. Only offered when there is actually work left to do — otherwise the
        // command stays legal and an agent re-issues it every call, burning the per-phase action cap
        // (the maintenance spam). Counts are the shortfall; `dry_run` gates supply/eligibility.
        // Visited in id order for determinism.
        let mut own_sgsus: Vec<crate::state::air::SquadronId> = self
            .sgsus
            .iter()
            .filter(|(_, s)| s.nationality.side() == side && s.plane_count() > 0)
            .map(|(&id, _)| id)
            .collect();
        own_sgsus.sort_by_key(|s| s.0);
        for squadron in own_sgsus {
            let s = &self.sgsus[&squadron];
            let n = s.plane_count();
            let mut candidates = Vec::new();
            if s.ready_planes() < n {
                candidates.push(Command::ReadySquadron { squadron, count: n - s.ready_planes() });
            }
            if s.armed_planes() < n {
                candidates.push(Command::ArmSquadron { squadron, count: n - s.armed_planes() });
            }
            if s.refitted_planes() < n {
                candidates.push(Command::RefitSquadron { squadron, wrong_squadron: false });
            }
            for cmd in candidates {
                if self.dry_run(&cmd).is_ok() {
                    out.push(cmd);
                }
            }
        }
        out
    }

    /// Truck-Convoy-phase actions: move each of `side`'s free truck convoys one hex (§8.98), and
    /// escort each of its Prisoner stacks one hex toward a Departure Point (§28.13). Legality
    /// (CPA/escort/terrain) is enforced by `dry_run`.
    fn legal_convoy_commands(&self, side: Side) -> Vec<crate::state::command::Command> {
        use crate::state::command::Command;

        use crate::state::command::CommandOutcome;
        use crate::state::game::Commodity;

        let mut out = Vec::new();
        for (i, c) in self.truck_convoys.iter().enumerate() {
            if c.side != side {
                continue;
            }
            for to in self.map.neighbors(c.hex) {
                let cmd = Command::MoveTruckConvoy { convoy: i, to };
                if self.dry_run(&cmd).is_ok() {
                    out.push(cmd);
                }
            }
            // §53.14/§24.9: load a commodity from a co-located constructed dump, or off-load one into
            // the hex. Probe the maximum with a huge amount; emit the concrete moved quantity if > 0.
            for cm in [Commodity::Fuel, Commodity::Ammo, Commodity::Stores, Commodity::Water] {
                if let Ok(CommandOutcome::TrucksLoaded(n)) = self.dry_run(&Command::LoadConvoy { convoy: i, commodity: cm, amount: u32::MAX }) {
                    if n > 0 {
                        out.push(Command::LoadConvoy { convoy: i, commodity: cm, amount: n });
                    }
                }
                if let Ok(CommandOutcome::TrucksUnloaded(n)) = self.dry_run(&Command::UnloadConvoy { convoy: i, commodity: cm, amount: u32::MAX }) {
                    if n > 0 {
                        out.push(Command::UnloadConvoy { convoy: i, commodity: cm, amount: n });
                    }
                }
            }
        }
        // §20.44/§20.46: pick up / drop / haul Replacement-Point pools during the Truck Convoy Phase.
        for (pi, p) in self.replacement_points.iter().enumerate() {
            if p.side != side {
                continue;
            }
            let Some(phex) = p.location.as_ref().and_then(|l| l.hex()) else { continue };
            // Move the pool one hex toward the front (self-moving classes, or an already-loaded pool).
            for to in self.map.neighbors(phex) {
                let cmd = Command::MoveReplacement { index: pi, to };
                if self.dry_run(&cmd).is_ok() {
                    out.push(cmd);
                }
            }
            // Load the pool onto a co-located convoy (binds them), or drop it off if already carried.
            if p.carried_by.is_some() {
                let cmd = Command::UnloadReplacement { pool: pi };
                if self.dry_run(&cmd).is_ok() {
                    out.push(cmd);
                }
            } else {
                for (ci, c) in self.truck_convoys.iter().enumerate() {
                    if c.side == side && c.hex == phex {
                        let cmd = Command::LoadReplacement { convoy: ci, pool: pi };
                        if self.dry_run(&cmd).is_ok() {
                            out.push(cmd);
                        }
                    }
                }
            }
        }
        for g in self.prisoner_groups.iter().filter(|g| g.captor == side) {
            for to in self.map.neighbors(g.hex) {
                let cmd = Command::MovePrisoners { from: g.hex, to };
                if self.dry_run(&cmd).is_ok() {
                    out.push(cmd);
                }
            }
        }
        // §30.51/§30.52: transfer a Commonwealth combat unit by sea between ports — each own combat
        // unit sitting in a port to every other men-capable, operable port. dry_run enforces the
        // capacity / one-per-port-per-stage / operability rules.
        if side == Side::Commonwealth {
            let dests: Vec<String> = self
                .ports
                .values()
                .filter(|p| p.is_operable() && p.stacking_in.unwrap_or(0) > 0)
                .map(|p| p.name.clone())
                .collect();
            if !dests.is_empty() {
                for u in self.units.values() {
                    if u.nationality.side() != side || !self.is_combat_unit(u.id) {
                        continue;
                    }
                    let Some(hex) = self.effective_hex(u.id) else { continue };
                    if !self.ports.values().any(|p| self.port_hex(&p.name) == Some(hex)) {
                        continue; // not in a port
                    }
                    for to_port in &dests {
                        let cmd = Command::NavalTransferTroops { unit: u.id, to_port: to_port.clone() };
                        if self.dry_run(&cmd).is_ok() {
                            out.push(cmd);
                        }
                    }
                }
            }
        }
        out
    }

    /// Repair-phase actions: for each piece of captured equipment `side` holds, a repair that
    /// returns it to play (§28.3/§22.5). (Unit breakdown/destroyed-tank repair via `Repair`, which
    /// carries a die roll, is a further extension.)
    fn legal_repair_commands(&self, side: Side) -> Vec<crate::state::command::Command> {
        use crate::state::command::{Command, RepairKind};

        let mut out = Vec::new();
        // §28.3/§22.5: repair captured equipment back into play (deduped by hex+system).
        let mut seen: std::collections::HashSet<(HexCoord, String)> = std::collections::HashSet::new();
        for e in self.captured_equipment.iter().filter(|e| e.captor == side && !e.repaired) {
            if !seen.insert((e.hex, e.system.clone())) {
                continue;
            }
            let cmd = Command::RepairCapturedEquipment { hex: e.hex, system: e.system.clone() };
            if self.dry_run(&cmd).is_ok() {
                out.push(cmd);
            }
        }
        // §21/§22: field-repair a unit's broken-down trucks or its destroyed tanks / recce points.
        // A representative die (4) is used — legality does not depend on the roll, only the outcome.
        let own: Vec<UnitId> = self
            .units
            .values()
            .filter(|u| u.nationality.side() == side && u.hex().is_some())
            .map(|u| u.id)
            .collect();
        for unit in own {
            for kind in [RepairKind::Trucks, RepairKind::Tanks, RepairKind::Recce] {
                let cmd = Command::Repair { unit, kind };
                if self.dry_run(&cmd).is_ok() {
                    out.push(cmd);
                }
            }
        }
        // §22.13: field-repair a free truck convoy's broken-down Truck Points. A representative die (4)
        // is used — legality (broken Truck Points present) does not depend on the roll.
        for i in 0..self.truck_convoys.len() {
            if self.truck_convoys[i].side != side {
                continue;
            }
            let cmd = Command::RepairConvoyTrucks { convoy: i };
            if self.dry_run(&cmd).is_ok() {
                out.push(cmd);
            }
        }
        out
    }

    /// Commonwealth-Fleet-phase actions (§30): the Fleet is Commonwealth, so this is empty for the
    /// Axis. For each ship — return it to Alexandria or Valetta, and repair it (§30.37). Sea-hex
    /// assignment for bombardment (`ShipLocation::AtSea`) needs a target hex and is a further step.
    fn legal_fleet_commands(&self, side: Side) -> Vec<crate::state::command::Command> {
        use crate::state::command::Command;
        use crate::state::naval::ShipLocation;
        use crate::reference::terrain::HexTerrain;
        if side != Side::Commonwealth {
            return Vec::new();
        }
        // Sea hexes next to an enemy-held hex: bombardment/AA stations (§30 off-shore support).
        let enemy = side.opponent();
        let mut sea_stations: Vec<HexCoord> = self
            .side_occupied_hexes(enemy)
            .into_iter()
            .flat_map(|h| self.map.neighbors(h))
            .filter(|&n| self.map.terrain(n) == Some(HexTerrain::Sea))
            .collect();
        sea_stations.sort_by_key(|h| (h.col, h.row));
        sea_stations.dedup();

        let mut out = Vec::new();
        for ship_index in 0..self.fleet.len() {
            let mut candidates = vec![Command::RepairShip { ship_index }];
            for location in [ShipLocation::Alexandria, ShipLocation::Valetta] {
                candidates.push(Command::AssignShip { ship_index, location });
            }
            // Parameter breadth: send the ship to a sea station off an enemy-held coast.
            for &hex in &sea_stations {
                candidates.push(Command::AssignShip { ship_index, location: ShipLocation::AtSea(hex) });
            }
            for cmd in candidates {
                if self.dry_run(&cmd).is_ok() {
                    out.push(cmd);
                }
            }
        }
        out
    }

    /// Naval-Convoy-Schedule-phase actions (§56.18): a **policy** rather than a brute grid — for
    /// each Axis shipping lane, one modest balanced supply convoy to that lane's destination
    /// arriving after the §20.21 delay. Full tonnage/destination optimisation is left to a
    /// purpose-built logistics planner; this just gives an agent a sensible convoy option per lane.
    /// (Convoys are Axis; empty for the Commonwealth, whose supply comes via the strategic line.)
    fn legal_convoy_planning_commands(&self, side: Side) -> Vec<crate::state::command::Command> {
        use crate::state::command::Command;
        if side != Side::Axis {
            return Vec::new();
        }
        let Some(chart) = self.reference.shipping_lanes.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for lane in &chart.lanes {
            // A modest balanced load; dry_run enforces lane availability and the §56.21 tonnage cap.
            let cmd = Command::PlanConvoy {
                lane: lane.nr,
                destination: lane.destination.clone(),
                arrival_gt: self.game_turn + 2,
                fuel_tons: 30,
                ammo_tons: 10,
                stores_tons: 20,
                replacement_tons: 0,
            };
            if self.dry_run(&cmd).is_ok() {
                out.push(cmd);
            }
        }
        out
    }

    /// Naval-Convoy-Arrival-phase actions (§56): deliver each pending convoy's tonnage to port.
    fn legal_naval_convoy_commands(&self, side: Side) -> Vec<crate::state::command::Command> {
        use crate::state::command::Command;
        let _ = side; // convoy ownership is validated by `dry_run`
        let mut out = Vec::new();
        for convoy_index in 0..self.convoys.len() {
            let cmd = Command::DeliverConvoy { convoy_index };
            if self.dry_run(&cmd).is_ok() {
                out.push(cmd);
            }
        }
        out
    }

    /// The distinct on-map hexes occupied by a side's live units (helper for the air enumerator).
    fn side_occupied_hexes(&self, side: Side) -> Vec<HexCoord> {
        let mut hexes: Vec<HexCoord> = self
            .units
            .values()
            .filter(|u| u.nationality.side() == side && u.total_toe() > 0)
            .filter_map(|u| u.hex())
            .collect();
        hexes.sort_by_key(|h| (h.col, h.row));
        hexes.dedup();
        hexes
    }
}
