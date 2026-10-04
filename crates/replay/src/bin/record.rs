//! Record one deterministic AI-vs-AI game to an event log for the spectator website.
//!
//! Run: `cargo run --release --bin record -- [out] [turns] [scenario]`
//!   defaults: game.json, 5 turns, skirmish
//!   An `out` path ending in `.jsonl` streams the delta-encoded JSONL the viewer prefers (header
//!   line + one event per line); any other extension writes a single pretty-printed JSON doc.
//!   scenario = `skirmish` (synthetic 6-unit demo) | `scenario1` (§60 Italian Offensive on the
//!              real section-C map + OB) | `campaign` (full §64)
//! The real scenarios load all five map sections and the historical order of battle — larger maps
//! and unit sets, so recording is slower and the log is bigger. For the web asset, gzip the `.jsonl`
//! to `web/public/game.jsonl.gz`.

use cna_engine::agent::{GreedyAgent, RandomAgent};
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

    let (game, label) = cna_replay::scenarios::build(scenario, seed);
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
