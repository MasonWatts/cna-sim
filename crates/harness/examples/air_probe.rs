//! Diagnostic: replay a command log to a window and print each squadron's readiness and why
//! readying would do nothing. Usage: air_probe <commands.jsonl> <key-prefix|suffix>
use cna_engine::agent::{Agent, GreedyAgent};
use cna_engine::reference::units::Side;
use cna_engine::state::command::{Command, CommandError, CommandOutcome};
use cna_engine::state::game::GameState;
use cna_harness::log;
struct P { inner: Box<dyn Agent>, key: String }
fn hit(k: &str, pat: &str) -> bool { let (a, b) = pat.split_once('|').unwrap_or((pat, "")); k.starts_with(a) && k.ends_with(b) }
fn report(g: &GameState) -> ! {
    println!("window {}", g.decision_window_key());
    let mut v: Vec<_> = g.sgsus.values().collect(); v.sort_by_key(|s| s.id.0);
    for s in v {
        let cap = s.capacity(g.game_turn);
        println!("  {:?} sq {} at {:?}/{:?}: planes {} refitted {} ready {} armed {} (ready cap {}) supplied {} blocker {:?}",
            s.nationality, s.id.0, s.location.map(|h| (h.col, h.row)), s.off_map_box, s.plane_count(), s.refitted_planes(), s.ready_planes(), s.armed_planes(), cap.ready, s.supplied, g.ready_blocker(s.id));
    }
    std::process::exit(0)
}
impl Agent for P {
    fn free_form(&self) -> bool { self.inner.free_form() }
    fn choose(&mut self, g: &GameState, side: Side) -> Option<Command> { if hit(&g.decision_window_key(), &self.key) { report(g) } self.inner.choose(g, side) }
    fn choose_index(&mut self, g: &GameState, side: Side, l: &[Command]) -> Option<usize> { if hit(&g.decision_window_key(), &self.key) { report(g) } self.inner.choose_index(g, side, l) }
    fn on_result(&mut self, c: &Command, r: &Result<CommandOutcome, CommandError>) { self.inner.on_result(c, r) }
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let loaded = log::load(std::path::Path::new(&a[1])).unwrap();
    let (mut game, _) = cna_replay::scenarios::build(&loaded.header.scenario, loaded.header.seed);
    let (cw, ax) = log::ResumeAgent::pair(loaded.attempts, Box::new(GreedyAgent::new(1)), Box::new(GreedyAgent::new(2)));
    let mut pc = P { inner: Box::new(cw), key: a[2].clone() };
    let mut pa = P { inner: Box::new(ax), key: a[2].clone() };
    cna_engine::agent::self_play(&mut game, &mut pc, &mut pa, loaded.header.turns);
    println!("window not reached");
}
