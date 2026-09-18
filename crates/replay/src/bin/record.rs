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
use cna_replay::{record_game, record_game_streaming, record_rp_demo, GameRecord};

/// Write a finished [`GameRecord`] to `out`: JSONL (header line + one event per line, matching the
/// streaming format the viewer prefers) when the path ends in `.jsonl`, else a single pretty JSON doc.
fn write_record(out: &str, record: &GameRecord) {
    if out.ends_with(".jsonl") {
        use std::io::Write;
        let file = std::fs::File::create(out).unwrap_or_else(|e| panic!("create {out}: {e}"));
        let mut w = std::io::BufWriter::new(file);
        let header = serde_json::json!({
            "scenario": record.scenario, "seed": record.seed,
            "commonwealth_agent": record.commonwealth_agent, "axis_agent": record.axis_agent,
            "map": &record.map,
        });
        writeln!(w, "{}", serde_json::to_string(&header).unwrap()).unwrap();
        for e in &record.events {
            writeln!(w, "{}", serde_json::to_string(e).unwrap()).unwrap();
        }
        w.flush().ok();
        eprintln!("wrote {out}: {} events", record.events.len());
    } else {
        let json = serde_json::to_string_pretty(record).expect("record serializes");
        std::fs::write(out, &json).unwrap_or_else(|e| panic!("write {out}: {e}"));
        eprintln!("wrote {out}: {} events, {} bytes", record.events.len(), json.len());
    }
}

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

    // The scripted Replacement-Point lifecycle demo (§20) isn't agent-driven — build + write it directly.
    if scenario == "rpdemo" {
        write_record(&out, &record_rp_demo(seed));
        return;
    }

    let (game, label) = match scenario {
        "scenario1" => (build_real(seed, data!("scenarios/italian-offensive.ron")), "The Italian Offensive (§60)"),
        "campaign" => (build_real(seed, data!("scenarios/campaign.ron")), "Campaign Game (§64)"),
        _ => (build_skirmish(seed), "Skirmish"),
    };
    let mut cw = GreedyAgent::new(1);
    let mut axis = RandomAgent::new(2);

    // A `.jsonl` output streams to disk with per-turn progress (crash-safe, bounded memory, live
    // progress for long runs); any other extension buffers a single pretty-printed GameRecord JSON.
    if out.ends_with(".jsonl") {
        use std::io::Write;
        let file = std::fs::File::create(&out).unwrap_or_else(|e| panic!("create {out}: {e}"));
        let mut w = std::io::BufWriter::new(file);
        let start = std::time::Instant::now();
        let mut on_turn = |turn: u32, events: u32| {
            eprintln!("[{:>6.1}s] turn {turn}/{turns}: {events} events", start.elapsed().as_secs_f64());
        };
        let count = record_game_streaming(game, label, seed, "Greedy", &mut cw, "Random", &mut axis, turns, &mut w, &mut on_turn)
            .unwrap_or_else(|e| panic!("stream {out}: {e}"));
        w.flush().ok();
        eprintln!("wrote {out}: {count} events in {:.1}s", start.elapsed().as_secs_f64());
    } else {
        let record = record_game(game, label, seed, "Greedy", &mut cw, "Random", &mut axis, turns);
        let json = serde_json::to_string_pretty(&record).expect("record serializes");
        std::fs::write(&out, &json).unwrap_or_else(|e| panic!("write {out}: {e}"));
        eprintln!("wrote {out}: {} events, {} bytes", record.events.len(), json.len());
    }
}
