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


pub struct Launcher {
    pub template: Option<String>,
    pub model: Option<String>,
    pub budget_usd: Option<String>,
    /// Per-side MCP client config file (written by the harness).
    pub mcp_configs: HashMap<Side, PathBuf>,
    pub transcript_dir: PathBuf,
}

/// A running player process for one window.
pub struct Session {
    child: Option<Child>,
}

impl Session {
    /// Whether the player process has exited (or never started).
    pub fn exited(&mut self) -> bool {
        match self.child.as_mut() {
            None => true,
            Some(c) => !matches!(c.try_wait(), Ok(None)),
        }
    }

    /// End the window: stop the process if it is still running.
    pub fn finish(mut self) {
        if let Some(mut c) = self.child.take() {
            if matches!(c.try_wait(), Ok(None)) {
                let _ = c.kill();
            }
            let _ = c.wait();
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

impl Launcher {
    /// Launch the player for one window with the fixed `system` prompt and this window's `prompt`.
    pub fn start(&self, side: Side, game: &GameState, window: u32, system: &str, prompt: &str) -> Session {
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
                if let Some(m) = &self.model {
                    v.extend(["--model".into(), m.clone()]);
                }
                if let Some(b) = &self.budget_usd {
                    v.extend(["--max-budget-usd".into(), b.clone()]);
                }
                v
            }
        };
        let stem = format!("w{window:04}-{side:?}-{:?}", game.phase);
        let _ = std::fs::create_dir_all(&self.transcript_dir);
        let out = File::create(self.transcript_dir.join(format!("{stem}.out"))).ok();
        let err = File::create(self.transcript_dir.join(format!("{stem}.err"))).ok();
        let _ = std::fs::write(
            self.transcript_dir.join(format!("{stem}.prompt.md")),
            format!("<!-- system prompt -->\n{system}\n\n<!-- window prompt -->\n{prompt}\n"),
        );
        let Some((prog, args)) = argv.split_first() else {
            eprintln!("[harness] empty player command");
            return Session { child: None };
        };
        let mut cmd = Command::new(prog);
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(out.map(Stdio::from).unwrap_or_else(Stdio::null))
            .stderr(err.map(Stdio::from).unwrap_or_else(Stdio::null));
        match cmd.spawn() {
            Ok(child) => {
                eprintln!("[harness] window {window}: launched {side:?} player for {:?} (turn {}, stage {})", game.phase, game.game_turn, game.operations_stage);
                Session { child: Some(child) }
            }
            Err(e) => {
                eprintln!("[harness] failed to launch player `{prog}`: {e} — passing this window");
                Session { child: None }
            }
        }
    }
}
