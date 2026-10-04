//! Command log (`<out>.commands.jsonl`) and deterministic replay.
//!
//! The log records **every attempted** command in order — accepted or rejected — with the decision
//! window it was issued in. Some commands roll dice before validating, so replaying only the
//! accepted ones would desynchronize the RNG. Replaying the full attempt sequence from the same
//! scenario + seed reproduces the game exactly, whatever the original players were (LLM or not).

use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, Write};
use std::rc::Rc;

use cna_engine::agent::Agent;
use cna_engine::reference::units::Side;
use cna_engine::state::command::{Command, CommandError, CommandOutcome};
use cna_engine::state::game::GameState;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogHeader {
    pub scenario: String,
    pub seed: u64,
    pub turns: u32,
    pub commonwealth: String,
    pub axis: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attempt {
    pub n: u64,
    pub window: String,
    pub side: Side,
    pub command: Command,
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogTrailer {
    pub final_fingerprint: String,
    pub attempts: u64,
}

/// A deterministic fingerprint of the parts of the state play can change: every unit's location,
/// strength, cohesion, CP and supplies; the supply dumps; turn/phase; and the RNG state.
pub fn fingerprint(game: &GameState) -> String {
    let mut h = DefaultHasher::new();
    let mut ids: Vec<_> = game.units.keys().copied().collect();
    ids.sort();
    for id in ids {
        let u = &game.units[&id];
        (id.0, format!("{:?}", u.location), u.total_toe(), u.cohesion, u.cp_expended.0, u.fuel, u.ammo).hash(&mut h);
    }
    let mut dumps: Vec<String> = game
        .supply_dumps
        .iter()
        .map(|d| format!("{:?}|{:?}|{:?}|{}", d.location, d.side, d.supply, d.dummy))
        .collect();
    dumps.sort();
    dumps.hash(&mut h);
    (game.game_turn, game.operations_stage, format!("{:?}", game.phase), format!("{:?}", game.rng)).hash(&mut h);
    format!("{:016x}", h.finish())
}

pub struct CommandLog {
    w: std::io::BufWriter<std::fs::File>,
    pub attempts: u64,
}

impl CommandLog {
    pub fn create(path: &std::path::Path, header: &LogHeader) -> std::io::Result<Self> {
        let mut w = std::io::BufWriter::new(std::fs::File::create(path)?);
        writeln!(w, "{}", serde_json::to_string(header)?)?;
        Ok(CommandLog { w, attempts: 0 })
    }

    pub fn record(&mut self, game: &GameState, side: Side, cmd: &Command, ok: bool) {
        self.attempts += 1;
        let a = Attempt { n: self.attempts, window: game.decision_window_key(), side, command: cmd.clone(), ok };
        if let Ok(line) = serde_json::to_string(&a) {
            let _ = writeln!(self.w, "{line}");
            // Crash-safe: every attempt hits the disk, so `--resume` can pick up from here.
            let _ = self.w.flush();
        }
    }

    pub fn finish(mut self, game: &GameState) -> std::io::Result<String> {
        let fp = fingerprint(game);
        writeln!(self.w, "{}", serde_json::to_string(&LogTrailer { final_fingerprint: fp.clone(), attempts: self.attempts })?)?;
        self.w.flush()?;
        Ok(fp)
    }
}

/// A parsed command log.
pub struct LoadedLog {
    pub header: LogHeader,
    pub attempts: VecDeque<Attempt>,
    pub trailer: Option<LogTrailer>,
}

pub fn load(path: &std::path::Path) -> anyhow::Result<LoadedLog> {
    let f = std::io::BufReader::new(std::fs::File::open(path)?);
    let mut lines = f.lines();
    let header: LogHeader = serde_json::from_str(&lines.next().ok_or_else(|| anyhow::anyhow!("empty log"))??)?;
    let mut attempts = VecDeque::new();
    let mut trailer = None;
    for line in lines {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if line.contains("\"final_fingerprint\"") {
            trailer = Some(serde_json::from_str(&line)?);
        } else {
            attempts.push_back(serde_json::from_str(&line)?);
        }
    }
    Ok(LoadedLog { header, attempts, trailer })
}

/// A free-form agent that re-issues logged attempts in their original windows.
pub struct ReplayAgent {
    queue: Rc<RefCell<VecDeque<Attempt>>>,
    expected_ok: Option<bool>,
    pub mismatches: Rc<RefCell<u64>>,
}

impl ReplayAgent {
    /// Two agents (Commonwealth, Axis) sharing one attempt queue.
    pub fn pair(attempts: VecDeque<Attempt>) -> (ReplayAgent, ReplayAgent, Rc<RefCell<u64>>) {
        let q = Rc::new(RefCell::new(attempts));
        let m = Rc::new(RefCell::new(0));
        (
            ReplayAgent { queue: q.clone(), expected_ok: None, mismatches: m.clone() },
            ReplayAgent { queue: q, expected_ok: None, mismatches: m.clone() },
            m,
        )
    }
}

impl Agent for ReplayAgent {
    fn free_form(&self) -> bool {
        true
    }
    fn choose(&mut self, game: &GameState, side: Side) -> Option<Command> {
        let mut q = self.queue.borrow_mut();
        let next = q.front()?;
        if next.side != side || next.window != game.decision_window_key() {
            return None;
        }
        let a = q.pop_front()?;
        self.expected_ok = Some(a.ok);
        Some(a.command)
    }
    fn on_result(&mut self, _cmd: &Command, result: &Result<CommandOutcome, CommandError>) {
        if self.expected_ok.take().is_some_and(|ok| ok != result.is_ok()) {
            *self.mismatches.borrow_mut() += 1;
        }
    }
}

/// Resume wrapper: re-issues the logged attempts (shared queue, as [`ReplayAgent`]) and, once the
/// log is exhausted, hands control to the live agent — so a crashed or budget-stopped game picks up
/// exactly where it left off (same state, same RNG), then continues with fresh decisions.
pub struct ResumeAgent {
    queue: Rc<RefCell<VecDeque<Attempt>>>,
    live: Box<dyn Agent>,
    replaying: bool,
}

impl ResumeAgent {
    pub fn pair(attempts: VecDeque<Attempt>, cw: Box<dyn Agent>, axis: Box<dyn Agent>) -> (ResumeAgent, ResumeAgent) {
        let q = Rc::new(RefCell::new(attempts));
        (
            ResumeAgent { queue: q.clone(), live: cw, replaying: false },
            ResumeAgent { queue: q, live: axis, replaying: false },
        )
    }
}

impl Agent for ResumeAgent {
    fn free_form(&self) -> bool {
        !self.queue.borrow().is_empty() || self.live.free_form()
    }
    fn choose(&mut self, game: &GameState, side: Side) -> Option<Command> {
        {
            let mut q = self.queue.borrow_mut();
            if let Some(next) = q.front() {
                self.replaying = true;
                if next.side != side || next.window != game.decision_window_key() {
                    return None;
                }
                return q.pop_front().map(|a| a.command);
            }
        }
        self.replaying = false;
        self.live.choose(game, side)
    }
    fn choose_index(&mut self, game: &GameState, side: Side, legal: &[Command]) -> Option<usize> {
        if self.queue.borrow().is_empty() {
            self.replaying = false;
            return self.live.choose_index(game, side, legal);
        }
        let cmd = self.choose(game, side)?;
        legal.iter().position(|c| *c == cmd)
    }
    fn on_result(&mut self, cmd: &Command, result: &Result<CommandOutcome, CommandError>) {
        if !self.replaying {
            self.live.on_result(cmd, result);
        }
    }
}
