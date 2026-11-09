//! Diagnostic: replay a (truncated) command log and, at the first decision after it runs out, print
//! the Pin state of the given unit ids (or, with STOP=<window-key prefix>, at the first decision in
//! that window). Usage: [STOP=31:1:None:8:] pin_probe <commands.jsonl> <id> [<id>...]
use cna_engine::agent::Agent;
use cna_engine::reference::units::Side;
use cna_engine::state::command::Command;
use cna_engine::state::game::GameState;
use cna_engine::state::unit::UnitId;
use cna_harness::log;

struct Stop(Vec<u32>);
impl Stop {
    fn report(&self, g: &GameState) -> ! {
        println!("window {}", g.decision_window_key());
        for &i in &self.0 {
            let id = UnitId(i);
            let u = &g.units[&id];
            println!("{i} {:<28} hex {:?} pinned {} air_pinned {} is_pinned {} parent {:?} cohesion {} withdrawn {} return_at {:?}", u.name, g.effective_hex(id),
                u.status.pinned, u.status.air_pinned, g.is_pinned(id), u.attached_parent, u.cohesion, u.status.withdrawn, u.return_at);
        }
        if let Ok(t) = std::env::var("TRYBOMB") {
            let v: Vec<i16> = t.split(',').map(|x| x.parse().unwrap()).collect();
            let cmd = Command::FlyBombingMissionAt {
                squadron: cna_engine::state::air::SquadronId(v[0] as _),
                target: cna_engine::state::map::HexCoord { col: v[1], row: v[2] },
                mission: cna_engine::reference::aircraft::BombingMission::Personnel,
                planes: 20, bombload_per_plane: 10, night: false, escort: None, abort_planes: 0,
            };
            println!("  trybomb {:?}", g.dry_run(&cmd));
        }
        if let Ok(pat) = std::env::var("LEGAL") {
            let side = g.active_player;
            for c in g.legal_commands(side) {
                let t = format!("{c:?}");
                if t.contains(&pat) { println!("  legal {t}"); }
            }
        }
        if let Ok(t) = std::env::var("DUMPS") {
            let (c, r) = t.split_once(',').unwrap();
            let h = cna_engine::state::map::HexCoord { col: c.parse().unwrap(), row: r.parse().unwrap() };
            println!("hex {:?} label {:?} port {:?}", h, g.map.hex(h).and_then(|x| x.label.clone()), g.map.hex(h).map(|x| x.is_port()));
            for d in g.supply_dumps.iter().filter(|d| d.location.hex() == Some(h)) {
                println!("  dump {:?} supply {:?} constructed {} unlimited {}", d.side, d.supply, d.constructed, d.unlimited);
            }
            println!("  Tripoli port hex: {:?}", g.port_hex("Tripoli"));
        }
        if let Ok(t) = std::env::var("TARGET") {
            let (c, r) = t.split_once(',').unwrap();
            let target = cna_engine::state::map::HexCoord { col: c.parse().unwrap(), row: r.parse().unwrap() };
            for &i in &self.0 {
                let a = vec![UnitId(i)];
                println!("  {i} CA {:?} | AA {:?}", g.dry_run(&Command::CloseAssault { attackers: a.clone(), target }).err(),
                    g.dry_run(&Command::AntiArmor { attackers: a, target_hex: target, withheld: vec![] }).err());
            }
        }
        std::process::exit(0)
    }
}
impl Agent for Stop {
    fn free_form(&self) -> bool { true }
    fn choose(&mut self, g: &GameState, _: Side) -> Option<Command> { self.report(g) }
    fn choose_index(&mut self, g: &GameState, _: Side, _: &[Command]) -> Option<usize> { self.report(g) }
}

struct At(log::ResumeAgent, Stop, String);
impl Agent for At {
    fn free_form(&self) -> bool { true }
    fn choose(&mut self, g: &GameState, s: Side) -> Option<Command> {
        if !self.2.is_empty() && g.decision_window_key().starts_with(&self.2) { self.1.report(g) }
        self.0.choose(g, s)
    }
    fn choose_index(&mut self, g: &GameState, s: Side, l: &[Command]) -> Option<usize> {
        if !self.2.is_empty() && g.decision_window_key().starts_with(&self.2) { self.1.report(g) }
        self.0.choose_index(g, s, l)
    }
    fn on_result(&mut self, c: &Command, r: &Result<cna_engine::state::command::CommandOutcome, cna_engine::state::command::CommandError>) { self.0.on_result(c, r) }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let loaded = log::load(std::path::Path::new(&a[1])).unwrap();
    let (mut game, _) = cna_replay::scenarios::build(&loaded.header.scenario, loaded.header.seed);
    let ids: Vec<u32> = a[2..].iter().map(|s| s.parse().unwrap()).collect();
    let stop = std::env::var("STOP").unwrap_or_default();
    let (cw, ax) = log::ResumeAgent::pair(loaded.attempts, Box::new(Stop(ids.clone())), Box::new(Stop(ids.clone())));
    let (mut cw, mut ax) = (At(cw, Stop(ids.clone()), stop.clone()), At(ax, Stop(ids), stop));
    cna_engine::agent::self_play(&mut game, &mut cw, &mut ax, loaded.header.turns);
}
