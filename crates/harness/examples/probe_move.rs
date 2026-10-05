//! Diagnostic: replay a command log up to a decision window, then report why each unit in a hex
//! cannot step toward a target hex. Usage: probe_move <commands.jsonl> <window-key-prefix> <side> <from c,r> <to c,r>
use cna_engine::agent::{Agent, GreedyAgent};
use cna_engine::reference::units::Side;
use cna_engine::state::command::Command;
use cna_engine::rules::movement::MoveMode;
use cna_engine::state::game::GameState;
use cna_engine::state::map::HexCoord;
use cna_harness::log;

struct Probe { inner: Box<dyn Agent>, key: String, side: Side, from: HexCoord, to: HexCoord }

impl Agent for Probe {
    fn free_form(&self) -> bool { self.inner.free_form() }
    fn choose(&mut self, g: &GameState, side: Side) -> Option<Command> {
        if side == self.side && hit(&g.decision_window_key(), &self.key) {
            report(g, self.from, self.to);
            std::process::exit(0);
        }
        self.inner.choose(g, side)
    }
    fn choose_index(&mut self, g: &GameState, side: Side, legal: &[Command]) -> Option<usize> {
        if side == self.side && hit(&g.decision_window_key(), &self.key) {
            report(g, self.from, self.to);
            std::process::exit(0);
        }
        self.inner.choose_index(g, side, legal)
    }
    fn on_result(&mut self, c: &Command, r: &Result<cna_engine::state::command::CommandOutcome, cna_engine::state::command::CommandError>) { self.inner.on_result(c, r) }
}

fn report(g: &GameState, from: HexCoord, to: HexCoord) {
    println!("window {}", g.decision_window_key());
    let ids: Vec<_> = { let mut v: Vec<_> = g.units.values().filter(|u| u.hex() == Some(from)).map(|u| u.id).collect(); v.sort(); v };
    let there: Vec<String> = g.units.values().filter(|u| u.hex() == Some(to)).map(|u| u.name.clone()).collect();
    println!("target {:?}: {} counters: {:?}", to, there.len(), there);
    let t = g.map.terrain(to);
    let lim = t.and_then(|t| g.reference.terrain.get(&t)).and_then(|p| p.stacking_limit);
    println!("terrain {:?} limit {:?} hex_sp {}", t, lim, g.hex_stacking_points(to));
    for u in g.units.values().filter(|u| u.hex() == Some(to)) {
        println!("   in target: {:<28} org={:?} sp={} garrison={:?} attached_parent={:?}", u.name, g.effective_org_size(u.id), g.effective_stacking_points(u.id), u.garrison_location, u.attached_parent);
    }
    for id in ids {
        let u = &g.units[&id];
        print!("{:>4} {:<32} sp={} org={:?} motor={} fuel={} water={} cp_exp={:?} cpa={:?}", id.0, u.name, g.effective_stacking_points(id), g.effective_org_size(id), g.unit_is_motorized(id), u.fuel, u.water, u.cp_expended, g.effective_cpa(id));
        for m in [MoveMode::Motorized, MoveMode::NonMotorized] {
            let r = g.clone().apply(Command::MoveStep { unit: id, to, mode: m });
            print!(" | {m:?}: {}", match r { Ok(_) => "OK".to_string(), Err(e) => format!("{e:?}") });
        }
        println!();
    }
}

fn hit(k: &str, pat: &str) -> bool { let (a, b) = pat.split_once('|').unwrap_or((pat, "")); k.starts_with(a) && k.ends_with(b) }

fn hex(s: &str) -> HexCoord { let v: Vec<i16> = s.split(',').map(|x| x.parse().unwrap()).collect(); HexCoord::new(v[0], v[1]) }

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let loaded = log::load(std::path::Path::new(&a[1])).unwrap();
    let side = if a[3] == "cw" { Side::Commonwealth } else { Side::Axis };
    let (mut game, _) = cna_replay::scenarios::build(&loaded.header.scenario, loaded.header.seed);
    let (cw, ax) = log::ResumeAgent::pair(loaded.attempts, Box::new(GreedyAgent::new(1)), Box::new(GreedyAgent::new(2)));
    let (inner_cw, inner_ax): (Box<dyn Agent>, Box<dyn Agent>) = (Box::new(cw), Box::new(ax));
    let mk = |inner, s| Probe { inner, key: a[2].clone(), side: s, from: hex(&a[4]), to: hex(&a[5]) };
    let mut pc = mk(inner_cw, Side::Commonwealth);
    let mut pa = mk(inner_ax, Side::Axis);
    pc.side = side; pa.side = side;
    cna_engine::agent::self_play(&mut game, &mut pc, &mut pa, loaded.header.turns);
    println!("window not reached");
}
