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
              "write_notes", "read_notes", "get_journal", "get_briefing"] {
        assert!(tools.iter().any(|x| x == t), "missing tool {t}: {tools:?}");
    }
    let rule = call(&client, "get_rule", json!({"section": "10.31"})).await;
    assert!(rule["text"].as_str().unwrap_or("").contains("Zones of Control"), "rules lookup: {rule}");

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
        assert!(b["system_prompt"].as_str().unwrap().contains("Memory contract"));
        if s.windows == 1 {
            assert!(wp.contains("first window"), "fresh notebook on window 1");
            assert!(wp.contains("Opening situation"));
        } else {
            assert!(wp.contains(&format!("plan-from-window-{}", s.windows - 1)), "previous notes carried over");
            assert!(wp.contains("Actions ("), "previous window's actions are journaled");
            let j = call(&client, "get_journal", json!({})).await;
            assert!(j["journal"].as_str().unwrap().contains(&format!("Window {}", s.windows - 1)));
        }

        let obs = call(&client, "get_observation", json!({"include": ["own_units", "enemy_units"]})).await;
        // Fog of war: no enemy strength unless revealed (nothing is revealed in this skirmish).
        for e in obs["enemy_units"].as_array().unwrap() {
            s.enemy_seen += 1;
            assert!(e["toe_strength"].is_null(), "enemy strength leaked: {e}");
        }
        let own = obs["own_units"].as_array().unwrap();
        assert!(own.iter().all(|u| u["toe_strength"].is_number()), "own strength must be visible");

        let legal = call(&client, "list_legal_actions", json!({"kind": "MoveStep", "limit": 2})).await;
        let mut items: Vec<Value> = legal["actions"].as_array().unwrap().iter().take(1).map(|a| json!({"id": a["id"]})).collect();
        // A macro move toward the first visible enemy.
        if let (Some(u), Some(e)) = (own.first(), obs["enemy_units"].as_array().unwrap().first()) {
            items.push(json!({"move_to": {"unit": u["id"], "hex": e["hex"]["coord"]}}));
        }
        items.push(json!({"id": 999_999})); // a bad id is reported, not fatal
        let res = call(&client, "submit_plan", json!({"items": items})).await;
        s.plans += 1;
        for r in res["results"].as_array().unwrap() {
            if r["ok"] == true { s.ok_items += 1 } else { s.failed_items += 1 }
        }
        let wn = call(&client, "write_notes", json!({"text": format!("plan-from-window-{}: hold the line", s.windows)})).await;
        assert_eq!(wn["ok"], true);
        assert_eq!(call(&client, "read_notes", json!({})).await["notes"].as_str().unwrap(), format!("plan-from-window-{}: hold the line", s.windows));
        let end = call(&client, "end_phase", json!({})).await;
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

    // The command log reproduces the game exactly.
    let (fp, expected, mismatches) = replay(&report.commands_path).expect("replay");
    assert_eq!(mismatches, 0, "every attempt had the same accept/reject verdict on replay");
    assert_eq!(expected.as_deref(), Some(fp.as_str()));
    assert_eq!(fp, report.fingerprint);
}
