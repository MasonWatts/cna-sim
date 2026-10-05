//! Diagnostic: list hexes over their §8.37 stacking limit at the start of a scenario and at the
//! end of a command log. Usage: stacking_audit <commands.jsonl>
use cna_engine::state::game::GameState;
use cna_harness::log;

fn over(g: &GameState) -> Vec<String> {
    let mut hexes: Vec<_> = g.units.values().filter_map(|u| u.hex()).collect();
    hexes.sort_by_key(|h| (h.col, h.row));
    hexes.dedup();
    hexes
        .into_iter()
        .filter_map(|h| {
            let t = g.map.terrain(h)?;
            let lim = g.reference.terrain.get(&t)?.stacking_limit?;
            let sp = g.hex_stacking_points(h);
            (sp > lim).then(|| format!("{:?} {:?} {} > {} ({})", (h.col, h.row), t, sp, lim, ""))
        })
        .collect()
}

/// Attached units that also carry their own on-map location (normally they ride the parent's counter).
fn detached_riders(g: &GameState) -> Vec<String> {
    let mut v: Vec<String> = g
        .units
        .values()
        .filter(|u| u.attached_parent.is_some() && u.hex().is_some())
        .map(|u| {
            let p = u.attached_parent.unwrap();
            format!("{} at {:?}; parent {} at {:?}", u.name, u.hex().map(|h| (h.col, h.row)),
                g.units.get(&p).map(|x| x.name.as_str()).unwrap_or("?"), g.effective_hex(p).map(|h| (h.col, h.row)))
        })
        .collect();
    v.sort();
    v
}

fn main() {
    let path = std::env::args().nth(1).expect("commands.jsonl");
    let loaded = log::load(std::path::Path::new(&path)).unwrap();
    let (mut game, _) = cna_replay::scenarios::build(&loaded.header.scenario, loaded.header.seed);
    println!("start: {:#?}", over(&game));
    println!("start riders with own location: {:#?}", detached_riders(&game));
    for name in std::env::args().skip(2) {
        if let Some(u) = game.units.values().find(|u| u.name == name) {
            let members: Vec<String> = game.units.values().filter(|m| m.attached_parent == Some(u.id)).map(|m| format!("{} ({:?})", m.name, m.org_size)).collect();
            println!("{name}: org {:?} shell {} sp {} attached {:?}", u.org_size, game.is_shell(u.id), game.effective_stacking_points(u.id), members);
        }
    }
    let (mut cw, mut ax, _) = log::ReplayAgent::pair(loaded.attempts);
    cna_engine::agent::self_play(&mut game, &mut cw, &mut ax, loaded.header.turns);
    println!("end: {:#?}", over(&game));
    println!("end riders with own location: {:#?}", detached_riders(&game));
}
