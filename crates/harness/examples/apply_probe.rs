//! Diagnostic: replay a command log to a decision window and try every legal action mentioning a
//! unit on a clone, printing the result. Usage: apply_probe <commands.jsonl> <key-prefix|suffix> <cw|axis> <unit name substring>
use cna_engine::agent::{Agent, GreedyAgent};
use cna_engine::reference::units::Side;
use cna_engine::state::command::{Command, CommandError, CommandOutcome};
use cna_engine::state::game::GameState;
use cna_harness::log;

struct Probe { inner: Box<dyn Agent>, key: String, side: Side, name: Option<String> }
fn hit(k: &str, pat: &str) -> bool { let (a, b) = pat.split_once('|').unwrap_or((pat, "")); k.starts_with(a) && k.ends_with(b) }
impl Probe {
    fn report(&self, g: &GameState) -> ! {
        let legal = g.legal_commands(self.side);
        let mut by: std::collections::BTreeMap<String, usize> = Default::default();
        for c in &legal { *by.entry(format!("{c:?}").split([' ', '{', '(']).next().unwrap().to_string()).or_default() += 1; }
        println!("window {} — {} legal: {by:?}", g.decision_window_key(), legal.len());
        if let Some(n) = &self.name {
            let ids: Vec<_> = g.units.values().filter(|u| u.name.contains(n.as_str())).map(|u| (u.id, u.name.clone(), u.attached_parent, u.stages_without_water, u.water, u.total_toe())).collect();
            println!("units matching {n:?}: {ids:?}");
            for (id, ..) in &ids {
                let need = g.unit_water_requirement(*id, true);
                let u = &g.units[id];
                println!("  water: id {} hex {:?} onboard {} trucks {:?} trucks_cargo_water {} need(if moved) {} on_hand {} deprived {} withdrawal {:?} parent {:?} withdrawn {}", id.0, g.effective_hex(*id), u.water, u.trucks, u.truck_cargo.water, need, g.water_on_hand_for(*id, need), u.stages_without_water, u.mandatory_withdrawal, u.attached_parent, u.status.withdrawn);
            }
            for c in &legal { let s = format!("{c:?}"); if ids.iter().any(|(id, ..)| s.contains(&format!("UnitId({})", id.0))) { println!("  {s}"); } }
            for c in &legal {
                let t = format!("{c:?}");
                if ids.iter().any(|(id, ..)| t.contains(&format!("UnitId({})", id.0))) {
                    println!("  try {t} => {:?}", g.clone().apply(c.clone()));
                }
            }
        }
        std::process::exit(0)
    }
}
impl Agent for Probe {
    fn free_form(&self) -> bool { self.inner.free_form() }
    fn choose(&mut self, g: &GameState, side: Side) -> Option<Command> { if side == self.side && hit(&g.decision_window_key(), &self.key) { self.report(g) } self.inner.choose(g, side) }
    fn choose_index(&mut self, g: &GameState, side: Side, legal: &[Command]) -> Option<usize> { if side == self.side && hit(&g.decision_window_key(), &self.key) { self.report(g) } self.inner.choose_index(g, side, legal) }
    fn on_result(&mut self, c: &Command, r: &Result<CommandOutcome, CommandError>) { self.inner.on_result(c, r) }
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let loaded = log::load(std::path::Path::new(&a[1])).unwrap();
    let side = if a[3] == "cw" { Side::Commonwealth } else { Side::Axis };
    let (mut game, _) = cna_replay::scenarios::build(&loaded.header.scenario, loaded.header.seed);
    let (cw, ax) = log::ResumeAgent::pair(loaded.attempts, Box::new(GreedyAgent::new(1)), Box::new(GreedyAgent::new(2)));
    let mut pc = Probe { inner: Box::new(cw), key: a[2].clone(), side, name: a.get(4).cloned() };
    let mut pa = Probe { inner: Box::new(ax), key: a[2].clone(), side, name: a.get(4).cloned() };
    cna_engine::agent::self_play(&mut game, &mut pc, &mut pa, loaded.header.turns);
    println!("window not reached");
}
