//! Record one deterministic AI-vs-AI game to a JSON event log for the spectator website.
//!
//! Run: `cargo run --release --bin record -- [out.json] [turns] [scenario]`
//!   defaults: game.json, 5 turns, skirmish
//!   scenario = `skirmish` (synthetic 6-unit demo) | `scenario1` (§60 Italian Offensive on the
//!              real section-C map + OB) | `campaign` (full §64)
//! The real scenarios load all five map sections and the historical order of battle — larger maps
//! and unit sets, so recording is slower and the JSON is bigger (snapshots per step).

use cna_engine::agent::{GreedyAgent, RandomAgent};
use cna_engine::reference::terrain::HexTerrain;
use cna_engine::reference::units::Nationality;
use cna_engine::reference::ReferenceData;
use cna_engine::state::game::GameState;
use cna_engine::state::map::HexCoord;
use cna_engine::state::unit::{Composition, UnitLocation, WeaponGroup};
use cna_replay::record_game;

/// A compact skirmish: cruisers + infantry per side on a small clear board, stocked to move/fight.
fn build_skirmish(seed: u64) -> GameState {
    let mut g = GameState::new(ReferenceData::load_campaign(), seed);
    for col in 0..10 {
        for row in 0..5 {
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
    place(&mut g, Nationality::Commonwealth, "g", "1st RTR", cruisers(), 1, 1);
    place(&mut g, Nationality::Commonwealth, "h", "1st Rifle Bde", Composition::Organic { current_toe: 8 }, 2, 2);
    place(&mut g, Nationality::Commonwealth, "h", "2nd Rifle Bde", Composition::Organic { current_toe: 8 }, 2, 3);
    place(&mut g, Nationality::Italian, "k", "Aresca Tank Bn", m11(), 8, 2);
    place(&mut g, Nationality::Italian, "p", "1 Libyan Bn", Composition::Organic { current_toe: 8 }, 8, 1);
    place(&mut g, Nationality::Italian, "p", "2 Libyan Bn", Composition::Organic { current_toe: 8 }, 8, 3);
    g
}

macro_rules! data {
    ($p:literal) => {
        include_str!(concat!("../../../../data/", $p))
    };
}

/// The full engine on the real map: reference bootstrap + all five map sections + a scenario RON.
fn build_real(seed: u64, scenario_ron: &str) -> GameState {
    let mut g = GameState::new(ReferenceData::load_campaign(), seed);
    for s in [
        data!("maps/section-a.ron"),
        data!("maps/section-b.ron"),
        data!("maps/section-c.ron"),
        data!("maps/section-d.ron"),
        data!("maps/section-e.ron"),
    ] {
        g.map.load_section(s).unwrap();
    }
    g.load_scenario(scenario_ron).expect("scenario loads");
    g.auto_place_dump_choices();
    g
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args.get(1).cloned().unwrap_or_else(|| "game.json".to_string());
    let turns: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(5);
    let scenario = args.get(3).map(|s| s.as_str()).unwrap_or("skirmish");

    let seed = 42;
    let (game, label) = match scenario {
        "scenario1" => (build_real(seed, data!("scenarios/italian-offensive.ron")), "The Italian Offensive (§60)"),
        "campaign" => (build_real(seed, data!("scenarios/campaign.ron")), "Campaign Game (§64)"),
        _ => (build_skirmish(seed), "Skirmish"),
    };
    let mut cw = GreedyAgent::new(1);
    let mut axis = RandomAgent::new(2);
    let record = record_game(game, label, seed, "Greedy", &mut cw, "Random", &mut axis, turns);

    let json = serde_json::to_string_pretty(&record).expect("record serializes");
    std::fs::write(&out, &json).unwrap_or_else(|e| panic!("write {out}: {e}"));
    eprintln!("wrote {out}: {} events, {} bytes", record.events.len(), json.len());
}
