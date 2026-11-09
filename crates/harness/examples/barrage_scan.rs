//! Diagnostic: replay a command log and report every Barrage whose attackers include a unit with no
//! Barrage Rating (and whether the whole group had none). Usage: barrage_scan <commands.jsonl>
use cna_engine::agent::Agent;
use cna_engine::reference::units::Side;
use cna_engine::state::command::{Command, CommandError, CommandOutcome};
use cna_engine::state::game::GameState;
use cna_engine::state::unit::RatingKind;
use cna_harness::log;

struct Scan(log::ResumeAgent);
fn check(g: &GameState, c: &Option<Command>) {
    if let Some(Command::Barrage { attackers, target, .. }) = c {
        let z: Vec<String> = attackers
            .iter()
            .filter_map(|a| g.units.get(a))
            .filter(|u| u.raw_strength(&g.reference, RatingKind::Barrage) <= 0)
            .map(|u| u.name.clone())
            .collect();
        if !z.is_empty() {
            let total: i32 = attackers.iter().filter_map(|a| g.units.get(a)).map(|u| u.raw_strength(&g.reference, RatingKind::Barrage)).sum();
            println!("{} {:?} -> ({},{}) total_raw {} zero: {:?}", g.decision_window_key(), g.active_player, target.col, target.row, total, z);
        }
    }
}
impl Agent for Scan {
    fn free_form(&self) -> bool { true }
    fn choose(&mut self, g: &GameState, s: Side) -> Option<Command> { let c = self.0.choose(g, s); check(g, &c); c }
    fn choose_index(&mut self, g: &GameState, s: Side, l: &[Command]) -> Option<usize> { self.0.choose_index(g, s, l) }
    fn on_result(&mut self, c: &Command, r: &Result<CommandOutcome, CommandError>) { self.0.on_result(c, r) }
}
struct Stop;
impl Agent for Stop {
    fn free_form(&self) -> bool { true }
    fn choose(&mut self, _: &GameState, _: Side) -> Option<Command> { std::process::exit(0) }
    fn choose_index(&mut self, _: &GameState, _: Side, _: &[Command]) -> Option<usize> { std::process::exit(0) }
}
fn main() {
    let p = std::env::args().nth(1).unwrap();
    let loaded = log::load(std::path::Path::new(&p)).unwrap();
    let (mut game, _) = cna_replay::scenarios::build(&loaded.header.scenario, loaded.header.seed);
    let (cw, ax) = log::ResumeAgent::pair(loaded.attempts, Box::new(Stop), Box::new(Stop));
    let (mut cw, mut ax) = (Scan(cw), Scan(ax));
    cna_engine::agent::self_play(&mut game, &mut cw, &mut ax, loaded.header.turns);
}
