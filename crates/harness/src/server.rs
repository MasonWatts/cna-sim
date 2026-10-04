//! The MCP server: one Streamable-HTTP endpoint per side (`/mcp/commonwealth`, `/mcp/axis`), each
//! guarded by its own bearer token. Tool handlers never see the `GameState`; they forward a
//! [`ToolRequest`] to the game thread and return its JSON reply.

use std::collections::BTreeMap;
use std::sync::mpsc::Sender;
use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use cna_engine::reference::units::Side;
use rmcp::handler::server::{router::tool::ToolRouter, wrapper::Parameters};
use rmcp::model::{ServerCapabilities, ServerConfig};
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use serde_json::{json, Value};
use tokio::sync::oneshot;

use crate::agent::{RequestKind, ToolRequest};
use crate::observe::{LegalFilter, ObserveFilter};

/// `§` sections of the rulebook (`docs/source-materials/rules.md`), keyed `"15.29"` → text.
#[derive(Default)]
pub struct RulesIndex(BTreeMap<String, String>);

impl RulesIndex {
    pub fn parse(md: &str) -> Self {
        let mut map = BTreeMap::new();
        let mut cur: Option<(String, String)> = None;
        for line in md.lines() {
            let t = line.trim_start();
            let starts_case = t.starts_with("**[") && t.contains("]**");
            if starts_case || t.starts_with('#') {
                if let Some((k, v)) = cur.take() {
                    map.insert(k, v.trim().to_string());
                }
            }
            if starts_case {
                let key = t[3..t.find("]**").unwrap_or(3)].to_string();
                cur = Some((key, line.to_string()));
            } else if let Some((_, v)) = cur.as_mut() {
                v.push('\n');
                v.push_str(line);
            }
        }
        if let Some((k, v)) = cur.take() {
            map.insert(k, v.trim().to_string());
        }
        RulesIndex(map)
    }

    /// Exact case (`15.29`), or every case under a prefix (`15.2` → 15.2, 15.21 … 15.29), capped.
    pub fn lookup(&self, section: &str) -> Value {
        let s = section.trim().trim_start_matches('§').trim_start_matches('[').trim_end_matches(']');
        if let Some(t) = self.0.get(s) {
            return json!({ "section": s, "text": t });
        }
        let hits: Vec<Value> = self
            .0
            .iter()
            .filter(|(k, _)| k.starts_with(s))
            .take(12)
            .map(|(k, v)| json!({ "section": k, "text": v }))
            .collect();
        if hits.is_empty() {
            json!({ "error": format!("no rules case §{s}") })
        } else {
            json!({ "matches": hits })
        }
    }
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RuleParams {
    /// Rules case number, e.g. "15.29", or a prefix such as "10.3" for every case under it.
    pub section: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct NotesParams {
    /// Your complete updated notebook (replaces the previous one; keep it under ~3000 characters).
    pub text: String,
}

#[derive(Debug, Default, serde::Deserialize, schemars::JsonSchema)]
pub struct JournalParams {
    /// Only the most recent N windows (default: all).
    pub last: Option<usize>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct PlanParams {
    /// Actions to apply in order. Each item is one of:
    /// {"id": n} — an action id from the latest list_legal_actions result;
    /// {"command": {...}} — a command in the engine's JSON form (as shown in listings);
    /// {"move_to": {"unit": <id>, "hex": [col, row], "mode": "Motorized"|"NonMotorized" (optional)}}
    /// — step the unit toward the hex along legal moves until it arrives or cannot get closer.
    pub items: Vec<Value>,
}

/// One side's MCP tool surface.
#[derive(Clone)]
pub struct CnaServer {
    side: Side,
    tx: Sender<ToolRequest>,
    rules: Arc<RulesIndex>,
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

impl CnaServer {
    pub fn new(side: Side, tx: Sender<ToolRequest>, rules: Arc<RulesIndex>) -> Self {
        CnaServer { side, tx, rules, tool_router: Self::tool_router() }
    }

    async fn ask(&self, kind: RequestKind) -> String {
        let (reply, rx) = oneshot::channel();
        if self.tx.send(ToolRequest { kind, reply }).is_err() {
            return json!({"error": "the game is over"}).to_string();
        }
        match rx.await {
            Ok(v) => v.to_string(),
            Err(_) => json!({"error": "the game is over"}).to_string(),
        }
    }
}

#[tool_router]
impl CnaServer {
    #[tool(description = "Current game turn, date, operations stage, phase, weather, and whether your decision window is open.")]
    async fn get_status(&self) -> String {
        self.ask(RequestKind::Status).await
    }

    #[tool(description = "Your side's view of the board under fog of war: your units in full detail; enemy units by position (TOE strength only if revealed by reconnaissance); supply dumps (enemy contents hidden), minefields, and raiders you can see. Optional filters: near [col,row] + radius, include sections.")]
    async fn get_observation(&self, Parameters(f): Parameters<ObserveFilter>) -> String {
        self.ask(RequestKind::Observe(f)).await
    }

    #[tool(description = "Legal actions for your side right now, with stable ids, a plain-English description, and the exact command JSON. Filter by unit id or command kind; paginate with offset/limit. Also returns counts by kind.")]
    async fn list_legal_actions(&self, Parameters(f): Parameters<LegalFilter>) -> String {
        self.ask(RequestKind::Legal(f)).await
    }

    #[tool(description = "Look up a rules case from The Campaign for North Africa rulebook by number (e.g. '10.31'), or every case under a prefix (e.g. '15.9').")]
    async fn get_rule(&self, Parameters(p): Parameters<RuleParams>) -> String {
        self.rules.lookup(&p.section).to_string()
    }

    #[tool(description = "Execute a plan: a list of actions applied in order. Returns a result per item (ok + outcome, or the engine's rejection reason). Failed items don't stop the rest. You may submit several plans in one window; call end_phase when done.")]
    async fn submit_plan(&self, Parameters(p): Parameters<PlanParams>) -> String {
        self.ask(RequestKind::SubmitPlan(p.items)).await
    }

    #[tool(description = "Replace your commander's notebook — strategy, formation tasks, enemy intel, supply concerns, lessons. It is shown to you at the start of every later window (each window is a fresh session). Call it before end_phase.")]
    async fn write_notes(&self, Parameters(p): Parameters<NotesParams>) -> String {
        self.ask(RequestKind::WriteNotes(p.text)).await
    }

    #[tool(description = "Read your current notebook.")]
    async fn read_notes(&self) -> String {
        self.ask(RequestKind::ReadNotes).await
    }

    #[tool(description = "The harness journal of earlier windows: what you observed change since each window (own losses, enemy contacts) and the actions you took with results. Optionally only the last N windows.")]
    async fn get_journal(&self, Parameters(p): Parameters<JournalParams>) -> String {
        self.ask(RequestKind::GetJournal(p.last)).await
    }

    #[tool(description = "The full briefing for the current window (system prompt + window prompt with objectives, journal and notebook). Launched players already receive it; external clients should call it at the start of each window.")]
    async fn get_briefing(&self) -> String {
        self.ask(RequestKind::GetBriefing).await
    }

    #[tool(description = "Finish acting in the current phase. The game advances; you will be asked again at your next decision window.")]
    async fn end_phase(&self) -> String {
        self.ask(RequestKind::EndPhase).await
    }

    #[tool(description = "Block until your side's next decision window opens, then return the status. For long-running clients that are not launched per window.")]
    async fn wait_for_turn(&self) -> String {
        self.ask(RequestKind::WaitForTurn).await
    }
}

#[tool_handler]
impl ServerHandler for CnaServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(format!(
            "You are playing the {:?} side of SPI's The Campaign for North Africa. Workflow per decision \
             window: get_status → get_observation → list_legal_actions → submit_plan (one or more) → end_phase.",
            self.side
        ))
    }
}

async fn require_bearer(State(token): State<Arc<String>>, req: Request, next: Next) -> Response {
    let ok = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v == format!("Bearer {token}"));
    if ok {
        next.run(req).await
    } else {
        StatusCode::UNAUTHORIZED.into_response()
    }
}

/// Endpoint wiring for one side.
pub struct Endpoint {
    pub side: Side,
    pub tx: Sender<ToolRequest>,
    pub token: String,
}

pub fn side_path(side: Side) -> &'static str {
    match side {
        Side::Commonwealth => "/mcp/commonwealth",
        Side::Axis => "/mcp/axis",
    }
}

/// A running server; dropping it stops the server thread.
pub struct RunningServer {
    pub port: u16,
    cancel: tokio_util::sync::CancellationToken,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl RunningServer {
    pub fn url(&self, side: Side) -> String {
        format!("http://127.0.0.1:{}{}", self.port, side_path(side))
    }
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        self.cancel.cancel();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Start the HTTP server on `127.0.0.1:port` (0 = ephemeral) on its own thread + runtime.
pub fn start(endpoints: Vec<Endpoint>, rules: Arc<RulesIndex>, port: u16) -> anyhow::Result<RunningServer> {
    let config = StreamableHttpServerConfig::default();
    let cancel = config.cancellation_token.clone();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<anyhow::Result<u16>>();
    let thread = std::thread::Builder::new().name("cna-mcp".into()).spawn(move || {
        let rt = match tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build() {
            Ok(rt) => rt,
            Err(e) => {
                let _ = ready_tx.send(Err(e.into()));
                return;
            }
        };
        rt.block_on(async move {
            let mut router = axum::Router::new();
            for ep in endpoints {
                let (side, tx, rules) = (ep.side, ep.tx.clone(), rules.clone());
                let service: StreamableHttpService<CnaServer, LocalSessionManager> = StreamableHttpService::new(
                    move || Ok(CnaServer::new(side, tx.clone(), rules.clone())),
                    Default::default(),
                    config.clone(),
                );
                let sub = axum::Router::new()
                    .nest_service(side_path(side), service)
                    .layer(axum::middleware::from_fn_with_state(Arc::new(ep.token), require_bearer));
                router = router.merge(sub);
            }
            let listener = match tokio::net::TcpListener::bind(("127.0.0.1", port)).await {
                Ok(l) => l,
                Err(e) => {
                    let _ = ready_tx.send(Err(e.into()));
                    return;
                }
            };
            let _ = ready_tx.send(Ok(listener.local_addr().map(|a| a.port()).unwrap_or(port)));
            let ct = config.cancellation_token.clone();
            let _ = axum::serve(listener, router).with_graceful_shutdown(async move { ct.cancelled().await }).await;
        });
    })?;
    let port = ready_rx.recv()??;
    Ok(RunningServer { port, cancel, thread: Some(thread) })
}
