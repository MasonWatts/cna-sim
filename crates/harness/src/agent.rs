//! `McpAgent`: an [`Agent`] whose decisions come from an external player over MCP.
//!
//! The game loop calls [`Agent::choose`] on the game thread. Inside a *decision window* (a phase
//! occurrence in which this side may act and has at least one legal action) `choose` blocks on a
//! channel of [`ToolRequest`]s sent by the MCP server thread, answering queries from the side's
//! observation and returning submitted commands one at a time. The MCP server never touches the
//! `GameState` — every answer is computed here, from fog-of-war-safe views (see [`crate::observe`]).

use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use cna_engine::agent::Agent;
use cna_engine::reference::units::Side;
use cna_engine::rules::movement::MoveMode;
use cna_engine::state::command::{Command, CommandError, CommandOutcome};
use cna_engine::state::game::GameState;
use cna_engine::state::map::HexCoord;
use cna_engine::state::unit::UnitId;
use serde_json::{json, Value};
use tokio::sync::oneshot;

use crate::launcher::{Launcher, Session};
use crate::memory::Memory;
use crate::observe::{self, LegalFilter, ObserveFilter};
use crate::briefing;

/// A request from the MCP server to the game thread; `reply` carries the JSON tool result.
pub struct ToolRequest {
    pub kind: RequestKind,
    pub reply: oneshot::Sender<Value>,
}

pub enum RequestKind {
    Status,
    Observe(ObserveFilter),
    Legal(LegalFilter),
    SubmitPlan(Vec<Value>),
    EndPhase,
    WaitForTurn,
    WriteNotes(String),
    ReadNotes,
    GetJournal(Option<usize>),
    GetBriefing,
}

/// One entry of a submitted plan.
#[derive(Debug, Clone)]
enum PlanItem {
    /// An action id from the most recent `list_legal_actions` listing.
    Id(usize),
    /// A free-form command (validated by the engine on apply).
    Command(Command),
    /// Macro: step `unit` toward `hex` along legal `MoveStep`s until it arrives or can't progress.
    MoveTo { unit: UnitId, hex: HexCoord, mode: Option<MoveMode>, steps: u32 },
}

fn parse_item(v: &Value) -> Result<PlanItem, String> {
    if let Some(id) = v.get("id").and_then(|x| x.as_u64()) {
        return Ok(PlanItem::Id(id as usize));
    }
    if let Some(c) = v.get("command") {
        return serde_json::from_value::<Command>(c.clone())
            .map(PlanItem::Command)
            .map_err(|e| format!("malformed command JSON: {e}"));
    }
    if let Some(m) = v.get("move_to") {
        let unit = m.get("unit").and_then(|x| x.as_u64()).ok_or("move_to needs `unit` (id)")? as u32;
        let hex = m.get("hex").and_then(|x| x.as_array()).ok_or("move_to needs `hex` [col, row]")?;
        let (c, r) = (hex.first().and_then(|x| x.as_i64()), hex.get(1).and_then(|x| x.as_i64()));
        let (Some(c), Some(r)) = (c, r) else { return Err("move_to `hex` must be [col, row]".into()) };
        let mode = match m.get("mode").and_then(|x| x.as_str()) {
            Some("Motorized") => Some(MoveMode::Motorized),
            Some("NonMotorized") => Some(MoveMode::NonMotorized),
            None => None,
            Some(other) => return Err(format!("unknown mode `{other}` (Motorized | NonMotorized)")),
        };
        return Ok(PlanItem::MoveTo { unit: UnitId(unit), hex: HexCoord::new(c as i16, r as i16), mode, steps: 0 });
    }
    Err("each plan item must be {\"id\": n}, {\"command\": {...}} or {\"move_to\": {\"unit\", \"hex\", \"mode\"?}}".into())
}

/// Per-window bookkeeping.
struct Window {
    key: String,
    opened: Instant,
    session: Option<Session>,
    queue: VecDeque<PlanItem>,
    results: Vec<Value>,
    pending_reply: Option<oneshot::Sender<Value>>,
    listing: Vec<Command>,
    /// The item currently being executed (for result reporting).
    current: Option<(PlanItem, String)>,
    /// Every result reported this window (for the journal).
    all_results: Vec<Value>,
}

pub struct McpAgent {
    side: Side,
    rx: Receiver<ToolRequest>,
    /// Skip (auto-pass) windows whose enumerated legal-action list is empty — no model call.
    pub auto_pass: bool,
    launcher: Option<Launcher>,
    window_timeout: Duration,
    window: Option<Window>,
    /// Windows opened so far (also numbers transcripts).
    pub windows_opened: u32,
    /// Continuity across window sessions: notebook + journal.
    pub memory: Memory,
    scenario: String,
    /// Where notebook/journal snapshots are written for inspection.
    memory_dir: Option<std::path::PathBuf>,
}

impl McpAgent {
    pub fn new(side: Side, rx: Receiver<ToolRequest>, launcher: Option<Launcher>, window_timeout: Duration) -> Self {
        McpAgent {
            side,
            rx,
            auto_pass: true,
            launcher,
            window_timeout,
            window: None,
            windows_opened: 0,
            memory: Memory::default(),
            scenario: String::new(),
            memory_dir: None,
        }
    }

    /// Set the scenario (for objectives) and a directory for notebook/journal snapshots.
    pub fn with_context(mut self, scenario: &str, memory_dir: Option<std::path::PathBuf>) -> Self {
        self.scenario = scenario.to_string();
        self.memory_dir = memory_dir;
        self
    }

    fn open_window(&mut self, game: &GameState) {
        self.windows_opened += 1;
        let key = game.decision_window_key();
        // With a launcher, anything still queued belongs to a previous (killed) session.
        if self.launcher.is_some() {
            while let Ok(req) = self.rx.try_recv() {
                let _ = req.reply.send(json!({"error": "stale request from a closed decision window"}));
            }
        }
        self.memory.begin_window(game, self.side);
        let session = self.launcher.as_ref().map(|l| {
            let sys = briefing::system_prompt(self.side);
            let user = briefing::window_prompt(&self.scenario, self.side, game, &self.memory);
            l.start(self.side, game, self.windows_opened, &sys, &user)
        });
        self.window = Some(Window {
            key,
            opened: Instant::now(),
            session,
            queue: VecDeque::new(),
            results: Vec::new(),
            pending_reply: None,
            listing: Vec::new(),
            current: None,
            all_results: Vec::new(),
        });
    }

    fn close_window(&mut self, game: &GameState) {
        let Some(mut w) = self.window.take() else { return };
        w.all_results.extend(w.results.iter().cloned());
        if let Some(reply) = w.pending_reply.take() {
            let _ = reply.send(json!({"results": w.results, "window_closed": true}));
        }
        if let Some(s) = w.session.take() {
            s.finish();
        }
        for r in &w.all_results {
            let ok = r["ok"] == true;
            let item = r["item"].as_str().map(str::to_string).unwrap_or_else(|| r["item"].to_string());
            let detail = if ok {
                let o = r.get("outcome").map(|o| o.to_string()).or_else(|| r.get("steps").map(|s| format!("{s} steps"))).unwrap_or_default();
                if o.is_empty() { String::new() } else { format!(" → {}", truncate(&o, 140)) }
            } else {
                format!(" — {}", truncate(r["error"].as_str().or(r["reason"].as_str()).unwrap_or("rejected"), 140))
            };
            self.memory.record_action(ok, format!("{item}{detail}"));
        }
        self.memory.end_window(game, self.side);
        if let Some(dir) = &self.memory_dir {
            let side = format!("{:?}", self.side).to_lowercase();
            let _ = std::fs::write(dir.join(format!("notes-{side}.md")), &self.memory.notes);
            let _ = std::fs::write(dir.join(format!("journal-{side}.md")), self.memory.journal_full(None));
        }
    }

    /// Turn the next queued plan item into a command, recording results for items that resolve
    /// without one (bad id, arrived, blocked). `None` = queue exhausted.
    fn next_command(&mut self, game: &GameState) -> Option<Command> {
        let w = self.window.as_mut()?;
        while let Some(item) = w.queue.pop_front() {
            match item {
                PlanItem::Id(i) => match w.listing.get(i) {
                    Some(c) => {
                        w.current = Some((PlanItem::Id(i), cna_replay::describe_action(game, c)));
                        return Some(c.clone());
                    }
                    None => w.results.push(json!({"item": format!("id {i}"), "ok": false,
                        "error": "no such id in the latest list_legal_actions listing"})),
                },
                PlanItem::Command(c) => {
                    w.current = Some((PlanItem::Command(c.clone()), cna_replay::describe_action(game, &c)));
                    return Some(c);
                }
                PlanItem::MoveTo { unit, hex, mode, steps } => {
                    let name = game.units.get(&unit).map(|u| u.name.clone()).unwrap_or_else(|| format!("unit {}", unit.0));
                    let label = format!("move {name} toward {}", observe::hex_label(hex));
                    let Some(at) = game.units.get(&unit).and_then(|u| u.hex()) else {
                        w.results.push(json!({"item": label, "ok": false, "error": "unit is not an on-map counter"}));
                        continue;
                    };
                    if at == hex {
                        w.results.push(json!({"item": label, "ok": true, "steps": steps, "arrived": true}));
                        continue;
                    }
                    if steps >= 60 {
                        w.results.push(json!({"item": label, "ok": steps > 0, "steps": steps, "arrived": false,
                            "stopped_at": observe::hex_json(at), "reason": "step limit"}));
                        continue;
                    }
                    let modes: Vec<MoveMode> = match mode {
                        Some(m) => vec![m],
                        None if game.unit_is_motorized(unit) => vec![MoveMode::Motorized, MoveMode::NonMotorized],
                        None => vec![MoveMode::NonMotorized],
                    };
                    let here = at.distance(hex);
                    let best = at
                        .neighbors()
                        .into_iter()
                        .filter(|n| n.distance(hex) < here)
                        .flat_map(|n| modes.iter().map(move |&m| (n, m)))
                        .find(|&(n, m)| game.can_move_step(unit, n, m));
                    match best {
                        Some((n, m)) => {
                            let next = PlanItem::MoveTo { unit, hex, mode, steps: steps + 1 };
                            w.current = Some((next, label));
                            return Some(Command::MoveStep { unit, to: n, mode: m });
                        }
                        None => w.results.push(json!({"item": label, "ok": steps > 0, "steps": steps,
                            "arrived": false, "stopped_at": observe::hex_json(at),
                            "reason": "no legal step that gets closer (terrain, ZOC, stacking, CP or fuel)"})),
                    }
                }
            }
        }
        None
    }

    fn handle(&mut self, game: &GameState, req: ToolRequest) -> Option<Command> {
        let side = self.side;
        match req.kind {
            RequestKind::Status | RequestKind::WaitForTurn => {
                let _ = req.reply.send(observe::status(game, side, true));
            }
            RequestKind::Observe(f) => {
                let _ = req.reply.send(observe::observation(game, side, &f));
            }
            RequestKind::Legal(f) => {
                let legal = game.legal_commands(side);
                let page = observe::legal_page(game, &legal, &f);
                if let Some(w) = self.window.as_mut() {
                    w.listing = legal;
                }
                let _ = req.reply.send(page);
            }
            RequestKind::SubmitPlan(items) => {
                let w = self.window.as_mut().expect("window open");
                if w.pending_reply.is_some() {
                    let _ = req.reply.send(json!({"error": "a plan is already executing"}));
                    return None;
                }
                w.results.clear();
                for (i, v) in items.iter().enumerate() {
                    match parse_item(v) {
                        Ok(item) => w.queue.push_back(item),
                        Err(e) => w.results.push(json!({"item": i, "ok": false, "error": e})),
                    }
                }
                w.pending_reply = Some(req.reply);
                return self.next_command(game);
            }
            RequestKind::EndPhase => {
                let _ = req.reply.send(json!({"ok": true, "message": "phase ended; your window is closed"}));
                self.close_window(game);
                return None;
            }
            RequestKind::WriteNotes(text) => {
                let truncated = self.memory.write_notes(&text);
                let _ = req.reply.send(json!({"ok": true, "chars": self.memory.notes.chars().count(),
                    "truncated": truncated, "max_chars": crate::memory::NOTES_MAX_CHARS}));
            }
            RequestKind::ReadNotes => {
                let _ = req.reply.send(json!({"notes": self.memory.notes}));
            }
            RequestKind::GetJournal(last) => {
                let _ = req.reply.send(json!({"journal": self.memory.journal_full(last)}));
            }
            RequestKind::GetBriefing => {
                let _ = req.reply.send(json!({
                    "system_prompt": briefing::system_prompt(side),
                    "window_prompt": briefing::window_prompt(&self.scenario, side, game, &self.memory),
                }));
            }
        }
        None
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() } else { format!("{}…", s.chars().take(n).collect::<String>()) }
}

/// Outcome of waiting on the request channel inside a window.
enum Wake {
    Command(Command),
    Close,
}

impl McpAgent {
    fn wait(&mut self, game: &GameState) -> Wake {
        loop {
            // The plan may still have items (e.g. after a result) — drain those first.
            if let Some(c) = self.next_command(game) {
                return Wake::Command(c);
            }
            // Plan exhausted: answer the submit_plan call with the per-item results.
            if let Some(w) = self.window.as_mut() {
                if let Some(reply) = w.pending_reply.take() {
                    let results = std::mem::take(&mut w.results);
                    w.all_results.extend(results.iter().cloned());
                    let _ = reply.send(json!({
                        "results": results,
                        "status": observe::status(game, self.side, true),
                        "hint": "Observe again if needed; submit another plan or call end_phase.",
                    }));
                }
            }
            let Some(w) = self.window.as_mut() else { return Wake::Close };
            let elapsed = w.opened.elapsed();
            if elapsed >= self.window_timeout {
                eprintln!("[harness] {:?} window {} timed out", self.side, w.key);
                return Wake::Close;
            }
            if w.session.as_mut().is_some_and(|s| s.exited()) {
                return Wake::Close; // the player process ended without end_phase
            }
            let slice = (self.window_timeout - elapsed).min(Duration::from_millis(250));
            match self.rx.recv_timeout(slice) {
                Ok(req) => {
                    if let Some(c) = self.handle(game, req) {
                        return Wake::Command(c);
                    }
                    if self.window.is_none() {
                        return Wake::Close; // end_phase
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Wake::Close,
            }
        }
    }
}

impl Agent for McpAgent {
    fn free_form(&self) -> bool {
        true
    }

    fn choose(&mut self, game: &GameState, side: Side) -> Option<Command> {
        let key = game.decision_window_key();
        if self.window.as_ref().map(|w| &w.key) != Some(&key) {
            self.close_window(game);
            // Auto-pass windows with nothing enumerable to do — no model call (unless disabled).
            if self.auto_pass && game.legal_commands(side).is_empty() {
                return None;
            }
            self.open_window(game);
        }
        match self.wait(game) {
            Wake::Command(c) => Some(c),
            Wake::Close => {
                self.close_window(game);
                None
            }
        }
    }

    fn on_result(&mut self, cmd: &Command, result: &Result<CommandOutcome, CommandError>) {
        let Some(w) = self.window.as_mut() else { return };
        let Some((item, label)) = w.current.take() else { return };
        match item {
            // A move_to keeps going while its steps succeed; a failed step ends it.
            PlanItem::MoveTo { .. } if result.is_ok() => w.queue.push_front(item),
            PlanItem::MoveTo { unit, hex, steps, .. } => w.results.push(json!({
                "item": label, "ok": steps > 1, "steps": steps.saturating_sub(1), "arrived": false,
                "error": format!("{:?}", result.as_ref().err()), "unit": unit.0, "target": observe::hex_json(hex),
            })),
            _ => w.results.push(match result {
                Ok(o) => json!({"item": label, "ok": true, "kind": observe::command_kind(cmd), "outcome": o}),
                Err(e) => json!({"item": label, "ok": false, "kind": observe::command_kind(cmd), "error": format!("{e:?}")}),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_items_parse() {
        assert!(matches!(parse_item(&json!({"id": 3})), Ok(PlanItem::Id(3))));
        let c = json!({"command": {"MoveStep": {"unit": 5, "to": {"col": 2, "row": 3}, "mode": "Motorized"}}});
        assert!(matches!(parse_item(&c), Ok(PlanItem::Command(Command::MoveStep { .. }))));
        let m = json!({"move_to": {"unit": 5, "hex": [9, 4], "mode": "NonMotorized"}});
        match parse_item(&m) {
            Ok(PlanItem::MoveTo { unit, hex, mode, steps }) => {
                assert_eq!((unit, hex, mode, steps), (UnitId(5), HexCoord::new(9, 4), Some(MoveMode::NonMotorized), 0));
            }
            other => panic!("{other:?}"),
        }
        assert!(parse_item(&json!({"command": {"NoSuchCommand": {}}})).is_err());
        assert!(parse_item(&json!({"move_to": {"unit": 1}})).is_err());
        assert!(parse_item(&json!({"bogus": 1})).is_err());
    }


}
