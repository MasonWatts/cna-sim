//! Launches one model session per decision window.
//!
//! Default player: headless Claude Code (`claude -p`) restricted to the `cna` MCP server's tools.
//! A custom `--player-cmd` template may be supplied instead; it is split on whitespace and the
//! placeholders `{prompt}`, `{system_prompt}`, `{mcp_config}` and `{side}` are substituted per window.
//! The prompts themselves are built by [`crate::briefing`].

use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use cna_engine::reference::units::Side;
use cna_engine::state::game::GameState;


/// Model spend across a game (shared by both sides' launchers).
#[derive(Debug, Default)]
pub struct Spend {
    pub total_usd: f64,
    pub by_side: HashMap<Side, f64>,
    pub windows: HashMap<Side, u32>,
}

pub struct Launcher {
    pub template: Option<String>,
    /// Model for Movement & Combat (the decisive phase); `None` = the client default.
    pub model: Option<String>,
    /// Cheaper model for routine phases (Organization, Air, Truck Convoy, Repair, Fleet, Naval);
    /// `None` = same as `model`.
    pub routine_model: Option<String>,
    /// Per-window spend cap passed to the client.
    pub budget_usd: Option<String>,
    /// Whole-game cap: once reached, no more windows are launched (they auto-pass).
    pub max_cost_usd: Option<f64>,
    pub spend: std::sync::Arc<std::sync::Mutex<Spend>>,
    /// Per-side MCP client config file (written by the harness).
    pub mcp_configs: HashMap<Side, PathBuf>,
    pub transcript_dir: PathBuf,
}

/// A running player process for one window.
pub struct Session {
    child: Option<Child>,
    out_path: Option<PathBuf>,
}

impl Session {
    /// `None` while running; `Some(success)` once the process has exited (or never started).
    pub fn poll(&mut self) -> Option<bool> {
        match self.child.as_mut() {
            None => Some(false),
            Some(c) => match c.try_wait() {
                Ok(None) => None,
                Ok(Some(st)) => Some(st.success()),
                Err(_) => Some(false),
            },
        }
    }

    /// Whether the player process has exited (or never started).
    pub fn exited(&mut self) -> bool {
        self.poll().is_some()
    }

    /// End the window: give the player a short grace period to exit on its own (it usually writes
    /// a final message and its cost line right after `end_phase`), then kill its whole process
    /// group — the client may run child processes that would otherwise outlive it. Returns the
    /// session's reported cost (Claude Code `total_cost_usd`; 0 if unknown).
    pub fn finish(mut self) -> f64 {
        if let Some(mut c) = self.child.take() {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(45);
            while matches!(c.try_wait(), Ok(None)) && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            if matches!(c.try_wait(), Ok(None)) {
                kill_group(&c);
                let _ = c.kill();
            }
            let _ = c.wait();
        }
        self.out_path.as_ref().map_or(0.0, |p| session_cost(p))
    }
}

/// SIGKILL the child's process group (it was spawned as a group leader).
fn kill_group(c: &Child) {
    let _ = Command::new("kill").arg("-KILL").arg(format!("-{}", c.id())).stdout(Stdio::null()).stderr(Stdio::null()).status();
}

/// The `total_cost_usd` a Claude Code stream-json transcript reports (last occurrence), or 0.
pub fn session_cost(path: &std::path::Path) -> f64 {
    let Ok(text) = std::fs::read_to_string(path) else { return 0.0 };
    text.lines()
        .rev()
        .find_map(|l| {
            let i = l.find("\"total_cost_usd\":")?;
            let rest = &l[i + 17..];
            let end = rest.find(|ch: char| !(ch.is_ascii_digit() || ch == '.' || ch == 'e' || ch == '-')).unwrap_or(rest.len());
            rest[..end].parse::<f64>().ok()
        })
        .unwrap_or(0.0)
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(mut c) = self.child.take() {
            kill_group(&c);
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

impl Launcher {
    /// Whether the whole-game spend cap has been reached.
    pub fn budget_exhausted(&self) -> bool {
        self.max_cost_usd.is_some_and(|cap| self.spend.lock().map_or(false, |s| s.total_usd >= cap))
    }

    /// Record a finished window's cost; returns (side total, game total).
    pub fn record_cost(&self, side: Side, cost: f64) -> (f64, f64) {
        let mut s = self.spend.lock().expect("spend lock");
        s.total_usd += cost;
        *s.by_side.entry(side).or_default() += cost;
        *s.windows.entry(side).or_default() += 1;
        (s.by_side[&side], s.total_usd)
    }

    /// Launch the player for one window with the fixed `system` prompt and this window's `prompt`.
    pub fn start(&self, side: Side, game: &GameState, window: u32, system: &str, prompt: &str) -> Session {
        self.start_with(side, game, window, system, prompt, None, None, false)
    }

    /// `start` with optional model / per-session budget overrides; `pregame` names the transcript
    /// and uses the Movement model (planning is not a routine phase).
    #[allow(clippy::too_many_arguments)]
    pub fn start_with(
        &self,
        side: Side,
        game: &GameState,
        window: u32,
        system: &str,
        prompt: &str,
        model_override: Option<&str>,
        budget_override: Option<&str>,
        pregame: bool,
    ) -> Session {
        let routine = !pregame && game.phase != cna_engine::state::game::Phase::MovementAndCombat;
        let base = if routine { self.routine_model.as_ref().or(self.model.as_ref()) } else { self.model.as_ref() };
        let model_owned = model_override.map(String::from).or_else(|| base.cloned());
        let model = model_owned.as_ref();
        let budget = budget_override.map(String::from).or_else(|| self.budget_usd.clone());
        let prompt = prompt.to_string();
        let cfg = self.mcp_configs.get(&side).cloned().unwrap_or_default();
        let argv: Vec<String> = match &self.template {
            Some(t) => t
                .split_whitespace()
                .map(|a| {
                    a.replace("{prompt}", &prompt)
                        .replace("{system_prompt}", system)
                        .replace("{mcp_config}", &cfg.display().to_string())
                        .replace("{side}", &format!("{side:?}"))
                })
                .collect(),
            None => {
                let mut v: Vec<String> = vec![
                    "claude".into(),
                    "-p".into(),
                    prompt.clone(),
                    "--system-prompt".into(),
                    system.to_string(),
                    "--mcp-config".into(),
                    cfg.display().to_string(),
                    "--strict-mcp-config".into(),
                    "--tools".into(),
                    "".into(),
                    "--allowedTools".into(),
                    "mcp__cna".into(),
                    "--output-format".into(),
                    "stream-json".into(),
                    "--verbose".into(),
                    "--no-session-persistence".into(),
                ];
                if let Some(m) = model {
                    v.extend(["--model".into(), m.clone()]);
                }
                if let Some(b) = &budget {
                    v.extend(["--max-budget-usd".into(), b.clone()]);
                }
                v
            }
        };
        let phase_tag = if pregame { "PreGame".to_string() } else { format!("{:?}", game.phase) };
        let mut stem = format!("w{window:04}-{side:?}-{phase_tag}");
        // A retry of the same window gets its own transcript.
        let mut k = 1;
        while self.transcript_dir.join(format!("{stem}.out")).exists() {
            k += 1;
            stem = format!("w{window:04}-{side:?}-{phase_tag}-try{k}");
        }
        let _ = std::fs::create_dir_all(&self.transcript_dir);
        let out_path = self.transcript_dir.join(format!("{stem}.out"));
        let out = File::create(&out_path).ok();
        let err = File::create(self.transcript_dir.join(format!("{stem}.err"))).ok();
        let _ = std::fs::write(
            self.transcript_dir.join(format!("{stem}.prompt.md")),
            format!("<!-- system prompt -->\n{system}\n\n<!-- window prompt -->\n{prompt}\n"),
        );
        let Some((prog, args)) = argv.split_first() else {
            eprintln!("[harness] empty player command");
            return Session { child: None, out_path: None };
        };
        let mut cmd = Command::new(prog);
        // Own process group, so the whole player tree can be stopped at window end.
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(out.map(Stdio::from).unwrap_or_else(Stdio::null))
            .stderr(err.map(Stdio::from).unwrap_or_else(Stdio::null));
        match cmd.spawn() {
            Ok(child) => {
                eprintln!(
                    "[harness] window {window}: launched {side:?} player for {} (turn {}, stage {}){}",
                    phase_tag,
                    game.game_turn,
                    game.operations_stage,
                    model.map(|m| format!(" [{m}]")).unwrap_or_default()
                );
                Session { child: Some(child), out_path: Some(out_path) }
            }
            Err(e) => {
                eprintln!("[harness] failed to launch player `{prog}`: {e} — passing this window");
                Session { child: None, out_path: None }
            }
        }
    }
}
