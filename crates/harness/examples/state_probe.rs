//! Diagnostic: summary of logistics state (trucks, convoys, pools) at the start of a scenario and at
//! the end of a command log. Usage: state_probe <commands.jsonl>
use cna_engine::reference::units::Side;
use cna_engine::state::game::GameState;
use cna_harness::log;

fn summary(tag: &str, g: &GameState) {
    for side in [Side::Commonwealth, Side::Axis] {
        let (mut l, mut m, mut h, mut n) = (0, 0, 0, 0);
        for u in g.units.values().filter(|u| u.nationality.side() == side) {
            let t = &u.trucks;
            if t.light + t.medium + t.heavy > 0 { n += 1; }
            l += t.light; m += t.medium; h += t.heavy;
        }
        let pool = g.truck_pools.get(&side).map(|t| (t.light, t.medium, t.heavy));
        let convoys = g.truck_convoys.iter().filter(|c| c.side == side).count();
        let legal = g.legal_commands(side).len();
        let on: Vec<_> = g.units.values().filter(|u| u.nationality.side() == side && g.effective_hex(u.id).is_some()).collect();
        let c = |f: &dyn Fn(&&cna_engine::state::unit::Unit) -> bool| on.iter().filter(|u| f(u)).count();
        println!("{tag} {side:?}: on-map {} | cohesion<0 {} | <=-10 {} | no water {} | no stores {} | 0 TOE (non-HQ) {} | motorized w/o fuel {}",
            on.len(), c(&|u| u.cohesion < 0), c(&|u| u.cohesion <= -10), c(&|u| u.stages_without_water > 0),
            c(&|u| u.turns_without_stores > 0), c(&|u| u.total_toe() == 0 && !u.is_hq), c(&|u| g.unit_is_motorized(u.id) && u.fuel == 0));
        println!("{tag} {side:?}: unit trucks L{l} M{m} H{h} on {n} units; free pool {pool:?}; truck convoys {convoys}; legal now {legal}");
    }
}

fn main() {
    let path = std::env::args().nth(1).expect("commands.jsonl");
    let loaded = log::load(std::path::Path::new(&path)).unwrap();
    let (mut game, _) = cna_replay::scenarios::build(&loaded.header.scenario, loaded.header.seed);
    summary("start", &game);
    if std::env::var("HQ").is_ok() {
        let mut v: Vec<_> = game.units.values().filter(|u| u.name.contains("HQ")).collect();
        v.sort_by_key(|u| u.id);
        for u in v {
            let attached = game.units.values().filter(|m| m.attached_parent == Some(u.id)).count();
            println!("  {:<34} is_hq {:<5} parent {:<5} org {:?} attached {} sp {} combat {} toe {} on_map {}", u.name, u.is_hq,
                u.attached_parent.is_some() || u.assigned_parent.is_some(), u.org_size, attached, game.effective_stacking_points(u.id),
                game.is_combat_unit(u.id), u.total_toe(), game.effective_hex(u.id).is_some());
        }
    }
    if std::env::var("LIST").is_ok() {
        let mut v: Vec<_> = game.units.values().filter(|u| game.effective_hex(u.id).is_some()).collect();
        v.sort_by_key(|u| u.id);
        for u in v {
            let cap = game.unit_fuel_capacity(u.id);
            let zero_toe = u.total_toe() == 0 && !u.is_hq;
            let empty = game.unit_is_motorized(u.id) && u.fuel == 0;
            if zero_toe || empty {
                println!("  {:?} {:<34} toe {} fuel {}/{} ammo {} comp {:?}{}{}", u.nationality, u.name, u.total_toe(), u.fuel, cap, u.ammo,
                    matches!(u.composition, cna_engine::state::unit::Composition::Organic { .. }),
                    if zero_toe { " ZERO_TOE" } else { "" }, if empty { " NO_FUEL" } else { "" });
            }
        }
    }
    let (mut cw, mut ax, _) = log::ReplayAgent::pair(loaded.attempts);
    cna_engine::agent::self_play(&mut game, &mut cw, &mut ax, loaded.header.turns);
    summary("end", &game);
}