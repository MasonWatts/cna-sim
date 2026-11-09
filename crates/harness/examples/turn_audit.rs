//! Diagnostic: replay a command log and audit one Game-Turn for engine-level anomalies.
//! Usage: turn_audit <commands.jsonl> <game-turn>
//!
//! Flags (each line starts with FLAG when it needs a human look):
//! - MISMATCH: a command that was in `legal_commands` at the moment it was issued, yet failed.
//! - STUCK-WATER: a unit deprived 3+ stages that has the water it needs on hand at a stage boundary
//!   (2 can be a legitimate short well draw that recovers next stage).
//! - REMOVED: every unit that left play, classified (scheduled withdrawal / combat phase / attrition
//!   of a 1-TOE unit); anything else is flagged.
//! - REVIVED: a unit that left play and came back.
//!
//! Info lines summarise immobilized units and truck repair state so slow-building deadlocks show up.
use cna_engine::agent::{Agent, GreedyAgent};
use cna_engine::reference::units::Side;
use cna_engine::state::command::{Command, CommandError, CommandOutcome};
use cna_engine::state::game::{GameState, Phase};
use cna_engine::state::unit::{UnitId, UnitLocation};
use cna_harness::log;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

#[derive(Default)]
struct Audit {
    turn: u32,
    pending: Option<(GameState, Side)>,
    stage: Option<(u32, u8)>,
    in_play: BTreeMap<UnitId, (String, u32, Phase, Option<(u32, u32)>)>,
    gone: BTreeSet<UnitId>,
    /// Units that left play by scheduled withdrawal (§20.8): coming back is a scheduled return.
    withdrew: BTreeSet<UnitId>,
    flags: Vec<String>,
    info: Vec<String>,
    failed: u32,
    issued: u32,
    water_suspects: BTreeSet<UnitId>,
}

fn on_map(g: &GameState, id: UnitId) -> bool {
    g.effective_hex(id).is_some() || matches!(g.root_location(id), Some(UnitLocation::OffMap { .. }))
}

impl Audit {
    fn snapshot(&mut self, g: &GameState) {
        let now: BTreeMap<UnitId, (String, u32, Phase, Option<(u32, u32)>)> = g
            .units
            .values()
            .filter(|u| on_map(g, u.id) && (u.total_toe() > 0 || u.is_hq || u.is_tds || u.toe_broken > 0 || u.weapons_broken.iter().any(|w| w.count > 0)))
            .map(|u| (u.id, (u.name.clone(), u.total_toe(), g.phase, u.mandatory_withdrawal.or(u.withdrawal_after_return))))
            .collect();
        if g.game_turn == self.turn {
            for (id, (name, toe, phase, wd)) in &self.in_play {
                if now.contains_key(id) {
                    continue;
                }
                // The schedule is cleared when a withdrawal is carried out, so use the pre-removal one.
                let why = if wd.is_some_and(|(t, _)| t <= g.game_turn) {
                    "scheduled withdrawal".to_string()
                } else if g.withdrawn.contains_key(id) {
                    "voluntary withdrawal for points (§64.75)".to_string()
                } else if matches!(phase, Phase::MovementAndCombat) || matches!(g.phase, Phase::MovementAndCombat) {
                    "combat phase".to_string()
                } else if *toe <= 1 {
                    "attrition of a 1-TOE unit (stores §51.22 / water §52.53)".to_string()
                } else {
                    String::new()
                };
                let line = format!("REMOVED {name} (id {}, last TOE {toe}) during {:?}: {}", id.0, g.phase, if why.is_empty() { "UNEXPLAINED" } else { &why });
                if why.is_empty() { self.flags.push(line) } else { self.info.push(line) }
                self.gone.insert(*id);
                if why == "scheduled withdrawal" {
                    self.withdrew.insert(*id);
                }
            }
            for (id, (name, ..)) in &now {
                if self.gone.remove(id) {
                    if self.withdrew.remove(id) {
                        self.info.push(format!("RETURNED {name} (id {}) during {:?}: scheduled return (§20.8)", id.0, g.phase));
                    } else {
                        self.flags.push(format!("REVIVED {name} (id {}) back in play during {:?}", id.0, g.phase));
                    }
                }
            }
        }
        self.in_play = now;
    }

    fn stage_checks(&mut self, g: &GameState) {
        if g.game_turn != self.turn {
            return;
        }
        let tag = format!("GT{} st{}", g.game_turn, g.operations_stage);
        let mut deprived = 0;
        let mut suspects = BTreeSet::new();
        for u in g.units.values() {
            if !on_map(g, u.id) {
                continue;
            }
            if g.unit_water_deprived(u.id) {
                deprived += 1;
                let need = g.unit_water_requirement(u.id, true);
                // Strict "water on hand": carried, in the unit's trucks, in a friendly dump, or an unlimited
                // well — not a Village/Bir, whose §52.7 draw can legitimately come up short.
                let strict = g.effective_hex(u.id).is_some_and(|h| {
                    let side = u.nationality.side();
                    u.water + u.truck_cargo.water + g.available_commodity_at(h, side, cna_engine::state::game::Commodity::Water) >= need
                        || matches!(g.hex_water_source_for(h, side), Some(cna_engine::state::game::WaterKind::Unlimited))
                });
                // Water can arrive between the stage-end draw and this check, so only flag a unit that
                // was still deprived-with-water-on-hand at the previous stage boundary too.
                if u.stages_without_water >= 3 && strict {
                    if self.water_suspects.contains(&u.id) {
                        self.flags.push(format!("STUCK-WATER {tag}: {} (id {}) deprived {} stages with water on hand", u.name, u.id.0, u.stages_without_water));
                    }
                    suspects.insert(u.id);
                }
            }
        }
        self.water_suspects = suspects;
        let mut trucks = BTreeMap::<String, (u32, u32)>::new();
        for c in &g.truck_convoys {
            let e = trucks.entry(format!("{:?}", c.side)).or_default();
            e.0 += c.trucks.light + c.trucks.medium + c.trucks.heavy;
            e.1 += c.trucks_broken.light + c.trucks_broken.medium + c.trucks_broken.heavy;
        }
        self.info.push(format!("{tag}: {deprived} units water-deprived; convoy Truck Points working/broken {trucks:?}"));
    }

    fn tick(&mut self, g: &GameState) {
        let key = (g.game_turn, g.operations_stage);
        if self.stage != Some(key) {
            self.stage_checks(g);
            self.stage = Some(key);
        }
        self.snapshot(g);
    }
}

struct Hook {
    inner: Box<dyn Agent>,
    a: Rc<RefCell<Audit>>,
}
impl Agent for Hook {
    fn free_form(&self) -> bool {
        self.inner.free_form()
    }
    fn choose(&mut self, g: &GameState, side: Side) -> Option<Command> {
        self.a.borrow_mut().tick(g);
        let c = self.inner.choose(g, side);
        if c.is_some() && g.game_turn == self.a.borrow().turn {
            self.a.borrow_mut().pending = Some((g.clone(), side));
        }
        c
    }
    fn choose_index(&mut self, g: &GameState, side: Side, legal: &[Command]) -> Option<usize> {
        self.a.borrow_mut().tick(g);
        self.inner.choose_index(g, side, legal)
    }
    fn on_result(&mut self, c: &Command, r: &Result<CommandOutcome, CommandError>) {
        let mut a = self.a.borrow_mut();
        if let Some((g, side)) = a.pending.take() {
            a.issued += 1;
            if let Err(e) = r {
                a.failed += 1;
                if g.legal_commands(side).contains(c) {
                    a.flags.push(format!(
                        "MISMATCH {} {side:?}: {c:?} was listed as legal but failed with {e:?} (dry_run on the pre-command state: {:?}; active player {:?})",
                        g.decision_window_key(), g.dry_run(c), g.active_player
                    ));
                    let own: Vec<String> = g.pending_air_missions.iter().filter(|(s, _)| *s == side).map(|(_, m)| format!("{m:?}")).collect();
                    if !own.is_empty() {
                        a.flags.push(format!("  (that side's queued air missions: {own:?})"));
                    }
                }
            }
        }
        drop(a);
        self.inner.on_result(c, r)
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let turn: u32 = args[2].parse().expect("game-turn");
    let loaded = log::load(std::path::Path::new(&args[1])).unwrap();
    let (mut game, _) = cna_replay::scenarios::build(&loaded.header.scenario, loaded.header.seed);
    let (cw, ax) = log::ResumeAgent::pair(loaded.attempts, Box::new(GreedyAgent::new(1)), Box::new(GreedyAgent::new(2)));
    let a = Rc::new(RefCell::new(Audit { turn, ..Default::default() }));
    let mut pc = Hook { inner: Box::new(cw), a: a.clone() };
    let mut pa = Hook { inner: Box::new(ax), a: a.clone() };
    cna_engine::agent::self_play(&mut game, &mut pc, &mut pa, turn.min(loaded.header.turns));
    let mut a = a.borrow_mut();
    a.tick(&game);
    a.stage_checks(&game);
    println!("== turn {turn} audit: {} commands, {} failed ==", a.issued, a.failed);
    for l in &a.info {
        println!("  {l}");
    }
    for l in &a.flags {
        println!("FLAG {l}");
    }
    if a.flags.is_empty() {
        println!("no flags");
    }
}
