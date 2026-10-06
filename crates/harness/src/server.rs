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
pub struct ReadRulesParams {
    /// Chapter ("15"), section ("15.2") or case ("15.29").
    pub section: String,
    /// Character offset for long chapters/sections (use the returned `next_offset`).
    pub offset: Option<usize>,
    /// Max characters to return (default 24000).
    pub max_chars: Option<usize>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchRulesParams {
    /// Keywords; every word must appear (case-insensitive).
    pub query: String,
    /// Max matches (default 20).
    pub limit: Option<usize>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DoctrineParams {
    /// Your complete doctrine (replaces the previous one; ≤15,000 characters): mechanics assessment,
    /// campaign strategy, opening plan by formation/sector, logistics plan.
    pub text: String,
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
    /// — step the unit toward the hex along legal moves until it arrives or cannot get closer;
    /// {"move_stack": {"from": [col,row], "to": [col,row], "mode"?}} — move_to for every own counter in a hex;
    /// {"resupply_hex": [col,row]} — RefuelUnit + RearmUnit for every own unit there below capacity
    /// (Organization phase; draws from dumps in that hex);
    /// {"assault": {"target": [col,row], "units"?: [ids]}} — close assault with the listed units, or
    /// with every own combat unit adjacent to the target.
    pub items: Vec<Value>,
    /// REQUIRED: one to three sentences on what this plan is meant to achieve and why — your
    /// reading of the situation and the intent behind these actions. Recorded for the game's
    /// commentary and shown back to you in later windows.
    pub rationale: String,
}

#[derive(Debug, Default, serde::Deserialize, schemars::JsonSchema)]
pub struct EndPhaseParams {
    /// A one- or two-sentence summary of this window: what you decided (including deciding to do
    /// nothing) and why. Recorded for the game's commentary.
    pub rationale: Option<String>,
}

/// One side's MCP tool surface.
#[derive(Clone)]
pub struct CnaServer {
    side: Side,
    tx: Sender<ToolRequest>,
    rules: Arc<RulesIndex>,
    book: Arc<crate::rules::Rulebook>,
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

impl CnaServer {
    pub fn new(side: Side, tx: Sender<ToolRequest>, rules: Arc<RulesIndex>, book: Arc<crate::rules::Rulebook>) -> Self {
        CnaServer { side, tx, rules, book, tool_router: Self::tool_router() }
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

    #[tool(description = "Your side's view of the board under fog of war. Own units are compact one-liners by default (id, hex, toe, cohesion, cp, fuel, ammo, flags, legal-action count); pass `detail: true` or `units: [ids]` for full records incl. rule-cited blockers. Enemy units by position (strength only if revealed); dumps (enemy contents hidden), minefields, raiders. Filters: near [col,row] + radius, include sections, units.")]
    async fn get_observation(&self, Parameters(f): Parameters<ObserveFilter>) -> String {
        self.ask(RequestKind::Observe(f)).await
    }

    #[tool(description = "Theatre summary — start here in large scenarios: your stacks by hex (contact first), formations, supply problems (out of water/fuel/ammo, disorganized, pinned), enemy contacts by hex, and your largest dumps. A few KB instead of the full unit list.")]
    async fn get_overview(&self) -> String {
        self.ask(RequestKind::Overview).await
    }

    #[tool(description = "Legal actions for your side right now, with stable ids, a plain-English description, and the exact command JSON. Filter by unit id or command kind; paginate with offset/limit. Also returns counts by kind.")]
    async fn list_legal_actions(&self, Parameters(f): Parameters<LegalFilter>) -> String {
        self.ask(RequestKind::Legal(f)).await
    }

    #[tool(description = "Look up a rules case from The Campaign for North Africa rulebook by number (e.g. '10.31'), or every case under a prefix (e.g. '15.9').")]
    async fn get_rule(&self, Parameters(p): Parameters<RuleParams>) -> String {
        self.rules.lookup(&p.section).to_string()
    }

    #[tool(description = "Execute a plan: a list of actions applied in order, with a short rationale (required) explaining the intent. Returns a result per item (ok + outcome, or the engine's rejection reason and why). Failed items don't stop the rest. You may submit several plans in one window; call end_phase when done.")]
    async fn submit_plan(&self, Parameters(p): Parameters<PlanParams>) -> String {
        self.ask(RequestKind::SubmitPlan { items: p.items, rationale: p.rationale }).await
    }

    #[tool(description = "Table of contents of The Campaign for North Africa rulebook: every chapter and section with its title. Available at any time.")]
    async fn rules_toc(&self) -> String {
        self.book.toc().to_string()
    }

    #[tool(description = "Read the rulebook: a whole chapter (\"15\"), a section (\"15.2\") or one case (\"15.29\"). Long text is paginated — pass the returned next_offset as offset. Available at any time.")]
    async fn read_rules(&self, Parameters(p): Parameters<ReadRulesParams>) -> String {
        self.book.read(&p.section, p.offset.unwrap_or(0), p.max_chars.unwrap_or(crate::rules::PAGE_CHARS).min(60_000)).to_string()
    }

    #[tool(description = "Search the rulebook for cases containing all the given keywords (e.g. \"water well\", \"retreat before assault\"). Returns ids, titles and snippets.")]
    async fn search_rules(&self, Parameters(p): Parameters<SearchRulesParams>) -> String {
        self.book.search(&p.query, p.limit.unwrap_or(20).min(60)).to_string()
    }

    #[tool(description = "Replace your DOCTRINE — your own assessment of the mechanics that matter and your campaign strategy (≤15,000 chars). Written in the pre-game planning window; revise when you learn something important. Shown at the start of every window.")]
    async fn write_doctrine(&self, Parameters(p): Parameters<DoctrineParams>) -> String {
        self.ask(RequestKind::WriteDoctrine(p.text)).await
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

    #[tool(description = "Finish acting in the current phase, with an optional one- or two-sentence rationale summarizing what you decided and why. The game advances; you will be asked again at your next decision window.")]
    async fn end_phase(&self, Parameters(p): Parameters<EndPhaseParams>) -> String {
        self.ask(RequestKind::EndPhase { rationale: p.rationale }).await
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
            "You are playing the {:?} side of SPI's The Campaign for North Africa. The rulebook is available through \
             rules_toc / read_rules / search_rules. Each decision window ends with end_phase.",
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
pub fn start(endpoints: Vec<Endpoint>, rules: Arc<RulesIndex>, book: Arc<crate::rules::Rulebook>, port: u16) -> anyhow::Result<RunningServer> {
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
                let (side, tx, rules, book) = (ep.side, ep.tx.clone(), rules.clone(), book.clone());
                let service: StreamableHttpService<CnaServer, LocalSessionManager> = StreamableHttpService::new(
                    move || Ok(CnaServer::new(side, tx.clone(), rules.clone(), book.clone())),
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
