//! A real MCP client drives a Commonwealth truck convoy with the `move_convoy_to` shortcut in the
//! campaign's first Truck Convoy window, and the engine moves it step by step (burning its fuel).
use std::time::Duration;

use cna_harness::{run, Options, PlayerKind};
use rmcp::model::CallToolRequestParams;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::ServiceExt;
use serde_json::{json, Value};

async fn call(client: &rmcp::service::RunningService<rmcp::RoleClient, ()>, name: &str, args: Value) -> Value {
    let mut p = CallToolRequestParams::new(name.to_string());
    if let Value::Object(m) = args {
        p = p.with_arguments(m);
    }
    let r = tokio::time::timeout(Duration::from_secs(120), client.call_tool(p)).await.unwrap().unwrap();
    let block = serde_json::to_value(&r.content[0]).unwrap();
    serde_json::from_str(block["text"].as_str().unwrap()).unwrap()
}

async fn play(url: String, token: String) -> Option<Value> {
    let transport = StreamableHttpClientTransport::from_config(StreamableHttpClientTransportConfig::with_uri(url).auth_header(token));
    let client = ().serve(transport).await.expect("connect");
    let mut result = None;
    loop {
        let st = call(&client, "wait_for_turn", json!({})).await;
        if st.get("error").is_some() {
            break;
        }
        if result.is_none() && st["phase"] == "TruckConvoy" {
            // The Mersa Matruh convoy (index from the listing) heading west toward Sidi Barrani.
            let legal = call(&client, "list_legal_actions", json!({"kind": "MoveTruckConvoy", "limit": 200})).await;
            let convoy = legal["actions"].as_array().unwrap().iter()
                .find_map(|a| a["command"]["MoveTruckConvoy"]["convoy"].as_u64());
            if let Some(c) = convoy {
                let res = call(&client, "submit_plan", json!({"items": [{"move_convoy_to": {"convoy": c, "hex": [97, 41]}}],
                    "rationale": "test the convoy shortcut"})).await;
                result = Some(res["results"][0].clone());
            }
        }
        call(&client, "end_phase", json!({})).await;
    }
    let _ = client.cancel().await;
    result
}

#[test]
fn move_convoy_to_steps_a_real_convoy() {
    let dir = std::env::temp_dir().join(format!("cna-harness-convoy-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let opts = Options {
        scenario: "campaign".into(),
        turns: 1,
        seed: 7,
        commonwealth: PlayerKind::Llm,
        axis: PlayerKind::Greedy,
        out: dir.join("game.jsonl"),
        external: true,
        pregame: false,
        window_timeout: Duration::from_secs(120),
        tokens: Some(("cw".into(), "ax".into())),
        ..Options::default()
    };
    let handle = std::sync::Mutex::new(None);
    run(&opts, |info| {
        let (_, url, token) = info.endpoints[0].clone();
        *handle.lock().unwrap() = Some(std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(play(url, token))
        }));
    })
    .expect("game runs");
    let r = handle.into_inner().unwrap().unwrap().join().unwrap().expect("a Truck Convoy window with a movable convoy");
    eprintln!("{r}");
    assert!(r["steps"].as_u64().unwrap_or(0) >= 1, "the convoy moved: {r}");
    assert!(r["item"].as_str().unwrap().starts_with("move convoy"), "{r}");
}
