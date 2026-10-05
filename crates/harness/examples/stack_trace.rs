//! Diagnostic: replay a command log and report every change in the counters/Stacking Points of the
//! given hexes, with the decision window and the last command applied before it.
//! Usage: stack_trace <commands.jsonl> <c,r> [<c,r> ...]
use std::collections::BTreeMap;

use cna_engine::agent::Agent;
use cna_engine::reference::units::Side;
use cna_engine::state::command::{Command, CommandError, CommandOutcome};
use cna_engine::state::game::GameState;
use cna_engine::state::map::HexCoord;
use cna_harness::log;

type Snap = BTreeMap<(i16, i16), (u32, Vec<String>)>;

fn snap(g: &GameState, hexes: &[HexCoord]) -> Snap {
    hexes
        .iter()
        .map(|&h| {
            let mut names: Vec<String> = g
                .units
                .values()
                .filter(|u| u.hex() == Some(h))
                .map(|u| format!("{} [{}sp {:?}{}{}]", u.name, g.effective_stacking_points(u.id), u.org_size,
                    if u.attached_parent.is_some() { " attached" } else { "" },
                    if u.garrison_location.is_some() { " G" } else { "" }))
                .collect();
            names.sort();
            ((h.col, h.row), (g.hex_stacking_points(h), names))
        })
        .collect()
}

struct Tracer<'a> { inner: Box<dyn Agent + 'a>, hexes: Vec<HexCoord>, last: std::rc::Rc<std::cell::RefCell<(Snap, String)>> }

impl Tracer<'_> {
    fn check(&mut self, g: &GameState) {
        let now = snap(g, &self.hexes);
        let mut st = self.last.borrow_mut();
        for (k, (sp, names)) in &now {
            let (old_sp, old_names) = st.0.get(k).cloned().unwrap_or_default();
            if *sp != old_sp || *names != old_names {
                let added: Vec<_> = names.iter().filter(|n| !old_names.contains(n)).collect();
                let removed: Vec<_> = old_names.iter().filter(|n| !names.contains(n)).collect();
                println!("{} {:?}: {} -> {} sp  (after: {})\n    + {:?}\n    - {:?}", g.decision_window_key(), k, old_sp, sp, st.1, added, removed);
            }
        }
        st.0 = now;
    }
}

impl Agent for Tracer<'_> {
    fn free_form(&self) -> bool { self.inner.free_form() }
    fn choose(&mut self, g: &GameState, side: Side) -> Option<Command> {
        self.check(g);
        let c = self.inner.choose(g, side);
        if let Some(c) = &c { self.last.borrow_mut().1 = format!("{side:?} {}", format!("{c:?}").chars().take(90).collect::<String>()); }
        c
    }
    fn choose_index(&mut self, g: &GameState, side: Side, legal: &[Command]) -> Option<usize> {
        self.check(g);
        let i = self.inner.choose_index(g, side, legal);
        if let Some(c) = i.and_then(|i| legal.get(i)) { self.last.borrow_mut().1 = format!("{side:?} {}", format!("{c:?}").chars().take(90).collect::<String>()); }
        i
    }
    fn on_result(&mut self, c: &Command, r: &Result<CommandOutcome, CommandError>) { self.inner.on_result(c, r) }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let hexes: Vec<HexCoord> = a[2..].iter().map(|s| { let v: Vec<i16> = s.split(',').map(|x| x.parse().unwrap()).collect(); HexCoord::new(v[0], v[1]) }).collect();
    let loaded = log::load(std::path::Path::new(&a[1])).unwrap();
    let (mut game, _) = cna_replay::scenarios::build(&loaded.header.scenario, loaded.header.seed);
    let s0 = snap(&game, &hexes);
    for (k, (sp, n)) in &s0 { println!("START {k:?}: {sp} sp\n    {n:#?}"); }
    let last = std::rc::Rc::new(std::cell::RefCell::new((s0, "setup".to_string())));
    let (cw, ax, _) = log::ReplayAgent::pair(loaded.attempts);
    let mut tc = Tracer { inner: Box::new(cw), hexes: hexes.clone(), last: last.clone() };
    let mut ta = Tracer { inner: Box::new(ax), hexes: hexes.clone(), last };
    cna_engine::agent::self_play(&mut game, &mut tc, &mut ta, loaded.header.turns);
    tc.check(&game);
}
