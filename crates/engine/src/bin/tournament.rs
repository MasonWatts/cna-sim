//! Agent-strength round-robin over the Campaign Game (§64).
//!
//! Every agent pair plays `seeds` seeds × both side assignments (to cancel the side / initiative
//! bias); a game is decided by the final `outcome_score` margin. Answers what the Phase-2 toolkit
//! could not measure on its own: which agents are actually strong, and whether the pricier search
//! agents (MCTS / cross-phase MCTS) earn their cost.
//!
//! Run: `cargo run --release --bin tournament -- [turns] [seeds] [roster] [game]`
//!   defaults: turns=6  seeds=4  roster=all  game=skirmish
//!   roster = `all` (all six agents) | `fast` (Random/Greedy/Lookahead/Rollout — no MCTS)
//!   game   = `skirmish` (compact, fast — the useful default) | `campaign` (full §64)
//! Use `--release` — MCTS is far too slow in a debug build. NB: the full `campaign` has a ~100-unit
//! action space, so even Greedy/Lookahead cost seconds per move there and MCTS is impractical; the
//! `skirmish` board is sized so a full six-agent round-robin finishes in seconds. Deterministic.

use std::time::Instant;

use cna_engine::agent::{
    format_standings, round_robin, Contender, CrossPhaseMctsAgent, GreedyAgent, LookaheadAgent,
    MctsAgent, RandomAgent, RolloutAgent,
};
use cna_engine::reference::ReferenceData;
use cna_engine::state::game::GameState;

macro_rules! data {
    ($p:literal) => {
        include_str!(concat!("../../../../data/", $p))
    };
}

/// A compact but non-trivial skirmish: a few supplied units per side on a small clear map — large
/// enough for agent skill to separate, small enough to run a full round-robin in seconds (the full
/// campaign's ~100-unit action space makes even Greedy/Lookahead impractical for a tournament).
fn build_skirmish() -> GameState {
    use cna_engine::reference::terrain::HexTerrain;
    use cna_engine::reference::units::Nationality;
    use cna_engine::state::map::HexCoord;
    use cna_engine::state::unit::{Composition, UnitLocation, WeaponGroup};

    let mut g = GameState::new(ReferenceData::load_campaign(), 20);
    for col in 0..12 {
        for row in 0..6 {
            g.map.set_hex(HexCoord::new(col, row), HexTerrain::Clear);
        }
    }
    let place = |g: &mut GameState, nat, id: &str, name: &str, comp, col, row| {
        let u = g.spawn_unit(nat, id, name, comp);
        let unit = g.unit_mut(u).unwrap();
        unit.location = Some(UnitLocation::OnMap(HexCoord::new(col, row)));
        unit.fuel = 800;
        unit.ammo = 800;
    };
    let cruisers = || Composition::Weapons { groups: vec![WeaponGroup { system: "A13 Cruiser".into(), count: 6 }] };
    let m11 = || Composition::Weapons { groups: vec![WeaponGroup { system: "M 11/39".into(), count: 6 }] };
    // Commonwealth (west): two cruiser squadrons + two infantry battalions.
    place(&mut g, Nationality::Commonwealth, "g", "1st RTR", cruisers(), 1, 1);
    place(&mut g, Nationality::Commonwealth, "g", "2nd RTR", cruisers(), 1, 4);
    place(&mut g, Nationality::Commonwealth, "h", "1st Rifle Bde", Composition::Organic { current_toe: 8 }, 2, 2);
    place(&mut g, Nationality::Commonwealth, "h", "2nd Rifle Bde", Composition::Organic { current_toe: 8 }, 2, 3);
    // Axis (east): a tank battalion + three infantry battalions.
    place(&mut g, Nationality::Italian, "k", "Aresca Tank Bn", m11(), 10, 2);
    place(&mut g, Nationality::Italian, "p", "1 Libyan Bn", Composition::Organic { current_toe: 8 }, 10, 1);
    place(&mut g, Nationality::Italian, "p", "2 Libyan Bn", Composition::Organic { current_toe: 8 }, 10, 4);
    place(&mut g, Nationality::Italian, "p", "3 Libyan Bn", Composition::Organic { current_toe: 8 }, 9, 3);
    g
}

/// Full engine for the Campaign Game: reference bootstrap + all five map sections + the campaign
/// scenario + player-choice dump placement. Cloned per game by the harness (O(1) via `Rc`).
fn build_campaign() -> GameState {
    let mut g = GameState::new(ReferenceData::load_campaign(), 7);
    for s in [
        data!("maps/section-a.ron"),
        data!("maps/section-b.ron"),
        data!("maps/section-c.ron"),
        data!("maps/section-d.ron"),
        data!("maps/section-e.ron"),
    ] {
        g.map.load_section(s).unwrap();
    }
    g.load_scenario(data!("scenarios/campaign.ron")).unwrap();
    g.auto_place_dump_choices();
    g
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let turns: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(6);
    let seeds: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(4);
    let roster = args.get(3).map(|s| s.as_str()).unwrap_or("all");
    let game = args.get(4).map(|s| s.as_str()).unwrap_or("skirmish");
    let draw_margin: i64 = 0;

    let template = if game == "campaign" { build_campaign() } else { build_skirmish() };
    let make_game = || template.clone();

    let mut contenders = vec![
        Contender::new("Random", |s| Box::new(RandomAgent::new(s))),
        Contender::new("Greedy", |s| Box::new(GreedyAgent::new(s))),
        Contender::new("Lookahead", |s| Box::new(LookaheadAgent::new(s))),
        Contender::new("Rollout", |s| Box::new(RolloutAgent::flat(s))),
    ];
    if roster != "fast" {
        contenders.push(Contender::new("MCTS", |s| Box::new(MctsAgent::standard(s))));
        contenders.push(Contender::new("CrossMCTS", |s| Box::new(CrossPhaseMctsAgent::standard(s))));
    }

    let n = contenders.len();
    let games = (n * (n - 1) / 2) as u32 * seeds * 2;
    eprintln!(
        "{} round-robin: {} agents ({}), {} seeds, {} turns/game → {} games. Running…",
        game, n, roster, seeds, turns, games
    );

    let t0 = Instant::now();
    let standings = round_robin(&make_game, &contenders, seeds, turns, draw_margin);
    let dt = t0.elapsed();

    println!("\n{}", format_standings(&standings));
    println!("({} games in {:.1}s)", games, dt.as_secs_f64());
}
