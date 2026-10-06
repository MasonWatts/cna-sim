//! End-to-end: a real MCP client plays the Commonwealth over Streamable HTTP against `GreedyAgent`
//! (no model involved), then the command log is replayed and must reproduce the game exactly.

use std::time::Duration;

use cna_engine::reference::units::Side;
use cna_harness::{replay, run, Options, PlayerKind};
use rmcp::model::CallToolRequestParams;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::ServiceExt;
use serde_json::{json, Value};

#[derive(Debug, Default)]
struct Stats {
    windows: u32,
    plans: u32,
    ok_items: u32,
    failed_items: u32,
    enemy_seen: u32,
    phases: std::collections::BTreeSet<String>,
}

async fn call(client: &rmcp::service::RunningService<rmcp::RoleClient, ()>, name: &str, args: Value) -> Value {
    let mut p = CallToolRequestParams::new(name.to_string());
    if let Value::Object(m) = args {
        p = p.with_arguments(m);
    }
    let r = tokio::time::timeout(Duration::from_secs(30), client.call_tool(p))
        .await
        .unwrap_or_else(|_| panic!("{name}: timed out"))
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    let block = serde_json::to_value(&r.content[0]).unwrap();
    serde_json::from_str(block["text"].as_str().unwrap()).unwrap()
}

async fn play(url: String, token: String) -> Stats {
    // A wrong token is refused at the HTTP layer.
    let bad = StreamableHttpClientTransport::from_config(StreamableHttpClientTransportConfig::with_uri(url.clone()).auth_header("nope"));
    assert!(().serve(bad).await.is_err(), "unauthenticated client must be rejected");

    let transport = StreamableHttpClientTransport::from_config(StreamableHttpClientTransportConfig::with_uri(url).auth_header(token));
    let client = ().serve(transport).await.expect("connect");
    let tools: Vec<String> = client.list_all_tools().await.unwrap().into_iter().map(|t| t.name.to_string()).collect();
    for t in ["get_status", "get_observation", "list_legal_actions", "get_rule", "submit_plan", "end_phase", "wait_for_turn",
              "write_notes", "read_notes", "get_journal", "get_briefing", "get_overview",
              "rules_toc", "read_rules", "search_rules", "write_doctrine"] {
        assert!(tools.iter().any(|x| x == t), "missing tool {t}: {tools:?}");
    }
    let rule = call(&client, "get_rule", json!({"section": "10.31"})).await;
    assert!(rule["text"].as_str().unwrap_or("").contains("Zones of Control"), "rules lookup: {rule}");

    // Rulebook tools work outside any window.
    assert!(call(&client, "rules_toc", json!({})).await["chapters"].as_array().unwrap().len() > 50);
    assert!(call(&client, "read_rules", json!({"section": "10.3"})).await["text"].as_str().unwrap().contains("[10.36]"));
    assert!(!call(&client, "search_rules", json!({"query": "holding off"})).await["matches"].as_array().unwrap().is_empty());

    // The first window is the pre-game planning window: read-only, doctrine + notes, end_phase.
    let pre = call(&client, "wait_for_turn", json!({})).await;
    assert_eq!(pre["pregame_planning"], true, "first window is pre-game: {pre}");
    let refused = call(&client, "submit_plan", json!({"items": [{"id": 0}], "rationale": "test"})).await;
    assert!(refused["error"].as_str().unwrap_or("").contains("pre-game"), "plans refused pre-game: {refused}");
    let brief = call(&client, "get_briefing", json!({})).await;
    assert!(brief["window_prompt"].as_str().unwrap().contains("PRE-GAME PLANNING"));
    assert!(brief["system_prompt"].as_str().unwrap().contains("rulebook is the authority"));
    assert_eq!(call(&client, "write_doctrine", json!({"text": "DOCTRINE-XYZ: concentrate, then strike."})).await["ok"], true);
    assert_eq!(call(&client, "end_phase", json!({"rationale": "PREGAME-RATIONALE"})).await["ok"], true);

    let mut s = Stats::default();
    loop {
        let st = call(&client, "wait_for_turn", json!({})).await;
        if st.get("error").is_some() {
            break; // game over
        }
        assert_eq!(st["you_are"], "Commonwealth");
        assert_eq!(st["acting_side"], "Commonwealth");
        s.windows += 1;
        s.phases.insert(st["phase"].as_str().unwrap().to_string());

        // Continuity across windows: the briefing carries the previous window's notes + journal.
        let b = call(&client, "get_briefing", json!({})).await;
        let wp = b["window_prompt"].as_str().unwrap();
        assert!(wp.contains("DOCTRINE-XYZ"), "the pre-game doctrine is in every window prompt");
        if s.windows == 1 {
            assert!(wp.contains("first window") || wp.contains("Your notebook"), "notebook section on window 1");
            assert!(wp.contains("Opening situation"));
        } else {
            assert!(wp.contains(&format!("plan-from-window-{}", s.windows - 1)), "previous notes carried over");
            assert!(wp.contains("Actions ("), "previous window's actions are journaled");
            assert!(wp.contains(&format!("PLAN-RATIONALE-{}", s.windows - 1)), "previous rationale is journaled");
            let j = call(&client, "get_journal", json!({})).await;
            assert!(j["journal"].as_str().unwrap().contains(&format!("Window {}", s.windows - 1)));
        }

        let ov = call(&client, "get_overview", json!({})).await;
        assert!(ov["stacks"].is_array() && ov["enemy_contacts"]["by_hex"].is_array() && ov["own"]["units_on_map"].is_number(), "{ov}");
        let obs = call(&client, "get_observation", json!({"include": ["own_units", "enemy_units"]})).await;
        // Fog of war: no enemy strength unless revealed (nothing is revealed in this skirmish).
        for e in obs["enemy_units"].as_array().unwrap() {
            s.enemy_seen += 1;
            assert!(e["toe_strength"].is_null(), "enemy strength leaked: {e}");
        }
        let own = obs["own_units"].as_array().unwrap();
        assert!(own.iter().all(|u| u["toe"].is_number() && u["flags"].is_array()), "compact own units: {own:?}");
        if let Some(u0) = own.first() {
            let full = call(&client, "get_observation", json!({"units": [u0["id"]]})).await;
            assert!(full["own_units"][0]["blockers"].is_array(), "units filter returns full detail");
        }

        let legal = call(&client, "list_legal_actions", json!({"kind": "MoveStep", "limit": 2})).await;
        let mut items: Vec<Value> = legal["actions"].as_array().unwrap().iter().take(1).map(|a| json!({"id": a["id"]})).collect();
        // Macros: move the first unit's whole stack toward the first visible enemy, resupply in place,
        // and (if adjacent) assault it.
        if let (Some(u), Some(e)) = (own.first(), obs["enemy_units"].as_array().unwrap().first()) {
            items.push(json!({"move_stack": {"from": u["hex"], "to": e["hex"]["coord"]}}));
            items.push(json!({"resupply_hex": u["hex"]}));
            items.push(json!({"assault": {"target": e["hex"]["coord"]}}));
        }
        items.push(json!({"id": 999_999})); // a bad id is reported, not fatal
        let res = call(&client, "submit_plan", json!({"items": items, "rationale": format!("PLAN-RATIONALE-{}", s.windows)})).await;
        s.plans += 1;
        for r in res["results"].as_array().unwrap() {
            if r["ok"] == true { s.ok_items += 1 } else { s.failed_items += 1 }
        }
        let wn = call(&client, "write_notes", json!({"text": format!("plan-from-window-{}: hold the line", s.windows)})).await;
        assert_eq!(wn["ok"], true);
        assert_eq!(call(&client, "read_notes", json!({})).await["notes"].as_str().unwrap(), format!("plan-from-window-{}: hold the line", s.windows));
        let end = call(&client, "end_phase", json!({"rationale": format!("END-RATIONALE-{}", s.windows)})).await;
        assert_eq!(end["ok"], true);
    }
    let _ = client.cancel().await;
    s
}

#[test]
fn mcp_player_plays_and_command_log_replays() {
    let dir = std::env::temp_dir().join(format!("cna-harness-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let opts = Options {
        scenario: "skirmish".into(),
        turns: 1,
        seed: 7,
        commonwealth: PlayerKind::Llm,
        axis: PlayerKind::Greedy,
        out: dir.join("game.jsonl"),
        external: true,
        window_timeout: Duration::from_secs(60),
        tokens: Some(("cw-token".into(), "ax-token".into())),
        ..Options::default()
    };
    let client = std::sync::Mutex::new(None);
    let report = run(&opts, |info| {
        assert_eq!(info.endpoints.len(), 1, "only the LLM side gets an endpoint");
        let (side, url, token) = info.endpoints[0].clone();
        assert_eq!(side, Side::Commonwealth);
        let h = std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(play(url, token))
        });
        *client.lock().unwrap() = Some(h);
    })
    .expect("game runs");
    let stats = client.into_inner().unwrap().unwrap().join().expect("client thread");
    eprintln!("{stats:?} steps={}", report.steps);

    assert!(stats.windows >= 1, "the client got at least one decision window");
    // No hybrid play: the model side is consulted in every phase with something to do, not just Movement.
    assert!(stats.phases.contains("MovementAndCombat") && stats.phases.len() >= 2, "{:?}", stats.phases);
    assert_eq!(stats.plans, stats.windows);
    assert!(stats.ok_items >= 1, "some plan items applied: {stats:?}");
    assert!(stats.failed_items >= stats.windows, "the bogus id is reported each window");
    assert!(stats.enemy_seen >= 1);
    assert!(std::fs::metadata(&opts.out).unwrap().len() > 0, "viewer log written");
    assert!(report.result["strength_on_map"]["Commonwealth"]["toe"].is_number(), "{}", report.result);
    assert!(dir.join("game.result.json").exists());
    assert!(dir.join("game.transcript").join("memory-commonwealth.json").exists(), "memory checkpointed");
    // Every window's stated rationale (plans with their results, closing summary) is recorded.
    let rat = std::fs::read_to_string(dir.join("game.transcript").join("rationale.jsonl")).expect("rationale.jsonl");
    let lines: Vec<Value> = rat.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert!(lines.iter().any(|l| l["pregame"] == true && l["rationales"][0]["text"] == "PREGAME-RATIONALE"), "{rat}");
    let w1 = lines.iter().find(|l| l["pregame"] == false).expect("a game window");
    assert_eq!(w1["rationales"][0]["kind"], "plan");
    assert!(w1["rationales"][0]["text"].as_str().unwrap().starts_with("PLAN-RATIONALE-"));
    assert!(w1["rationales"][0]["results"].as_array().is_some_and(|r| !r.is_empty()), "plan results attached: {w1}");
    assert!(w1["key"].is_string() && w1["at"]["phase"].is_string());

    // The command log reproduces the game exactly.
    let (fp, expected, mismatches) = replay(&report.commands_path).expect("replay");
    assert_eq!(mismatches, 0, "every attempt had the same accept/reject verdict on replay");
    assert_eq!(expected.as_deref(), Some(fp.as_str()));
    assert_eq!(fp, report.fingerprint);
}


/// `--resume` from a complete log reproduces the game exactly; from a truncated log it replays the
/// logged prefix identically and then finishes the game live.
#[test]
fn resume_from_command_log() {
    let dir = std::env::temp_dir().join(format!("cna-harness-resume-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let base = Options {
        scenario: "skirmish".into(),
        turns: 1,
        seed: 5,
        commonwealth: PlayerKind::Greedy,
        axis: PlayerKind::Random,
        external: true,
        ..Options::default()
    };
    let full = run(&Options { out: dir.join("full.jsonl"), ..base.clone() }, |_| {}).unwrap();
    let log_lines: Vec<String> = std::fs::read_to_string(&full.commands_path).unwrap().lines().map(String::from).collect();
    assert!(log_lines.len() > 10, "need a log with a few attempts");

    // Complete log → identical game.
    let copy = dir.join("again.commands.jsonl");
    std::fs::copy(&full.commands_path, &copy).unwrap();
    let again = run(&Options { out: dir.join("again.jsonl"), resume: Some(copy), ..base.clone() }, |_| {}).unwrap();
    assert_eq!(again.fingerprint, full.fingerprint, "resuming a finished game reproduces it");

    // Truncated log (as after a crash): the prefix replays identically, then play continues.
    let cut = dir.join("cut.commands.jsonl");
    let keep = log_lines.len() / 2; // header + some attempts (drop the trailer)
    std::fs::write(&cut, log_lines[..keep].join("\n") + "\n").unwrap();
    let resumed = run(&Options { out: dir.join("cut.jsonl"), resume: Some(cut.clone()), ..base }, |_| {}).unwrap();
    let new_lines: Vec<String> = std::fs::read_to_string(&resumed.commands_path).unwrap().lines().map(String::from).collect();
    assert_eq!(&new_lines[1..keep], &log_lines[1..keep], "logged prefix replayed exactly");
    assert!(new_lines.last().unwrap().contains("final_fingerprint"), "the resumed game ran to completion");
}

/// A player process that crashes is relaunched once, then the window closes and the game goes on.
#[test]
fn crashed_player_is_retried_once() {
    let dir = std::env::temp_dir().join(format!("cna-harness-retry-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let opts = Options {
        scenario: "skirmish".into(),
        turns: 1,
        seed: 3,
        commonwealth: PlayerKind::Llm,
        axis: PlayerKind::Greedy,
        player_cmd: Some("false".into()), // exits non-zero immediately
        window_timeout: Duration::from_secs(20),
        out: dir.join("game.jsonl"),
        ..Options::default()
    };
    let report = run(&opts, |_| {}).expect("game completes despite the crashing player");
    assert!(report.result["final_game_turn"].as_u64().unwrap() >= 2);
    let names: Vec<String> = std::fs::read_dir(dir.join("game.transcript")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    let first = names.iter().filter(|n| n.ends_with(".out") && !n.contains("-try")).count();
    let retries = names.iter().filter(|n| n.ends_with("-try2.out")).count();
    assert!(first >= 1 && retries == first, "every crashed window retried exactly once: {names:?}");
    assert!(!names.iter().any(|n| n.contains("-try3")), "no second retry");
}
