//! CNA-Sim LLM player harness — see `docs/llm-player-harness.md`.
//!
//! [`run`] builds a scenario, starts the MCP server (one endpoint per model-controlled side),
//! plays the game through the standard recorder loop (so the viewer can replay it), and writes a
//! command log that [`replay`] can re-apply to reproduce the game exactly.

pub mod agent;
pub mod briefing;
pub mod launcher;
pub mod log;
pub mod memory;
pub mod observe;
pub mod server;

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use cna_engine::agent::{Agent, GreedyAgent, RandomAgent};
use cna_engine::reference::units::Side;

use crate::agent::McpAgent;
use crate::launcher::Launcher;
use crate::server::{Endpoint, RulesIndex};

/// Who plays a side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerKind {
    /// A model through MCP (launched per window, or an external client).
    Llm,
    Greedy,
    Random,
}

impl PlayerKind {
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        match s {
            "llm" | "mcp" => Ok(PlayerKind::Llm),
            "greedy" => Ok(PlayerKind::Greedy),
            "random" => Ok(PlayerKind::Random),
            o => anyhow::bail!("unknown player `{o}` (llm | greedy | random)"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Options {
    pub scenario: String,
    pub turns: u32,
    pub seed: u64,
    pub commonwealth: PlayerKind,
    pub axis: PlayerKind,
    /// Viewer event log (`.jsonl`); the command log and transcripts sit beside it.
    pub out: PathBuf,
    /// `None` = launch headless Claude Code per window; `Some(t)` = custom command template.
    pub player_cmd: Option<String>,
    pub model: Option<String>,
    pub budget_usd: Option<String>,
    /// Don't launch players; an external MCP client connects and uses `wait_for_turn`.
    pub external: bool,
    pub window_timeout: Duration,
    /// Skip windows with no enumerated legal actions (default true).
    pub auto_pass: bool,
    pub port: u16,
    /// Fixed bearer tokens (tests); default random per run.
    pub tokens: Option<(String, String)>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            scenario: "skirmish".into(),
            turns: 1,
            seed: 42,
            commonwealth: PlayerKind::Llm,
            axis: PlayerKind::Greedy,
            out: PathBuf::from("game.jsonl"),
            player_cmd: None,
            model: None,
            budget_usd: None,
            external: false,
            window_timeout: Duration::from_secs(600),
            auto_pass: true,
            port: 0,
            tokens: None,
        }
    }
}

/// Connection details for the model-controlled sides, available once the server is up.
#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub endpoints: Vec<(Side, String, String)>, // (side, url, bearer token)
}

#[derive(Debug, Clone)]
pub struct RunReport {
    pub steps: u32,
    pub fingerprint: String,
    pub commands_path: PathBuf,
}

/// The rulebook (for `get_rule`), embedded so the binary works from any directory.
const RULES_MD: &str = include_str!("../../../docs/source-materials/rules.md");

fn random_token(salt: u64) -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u64(salt);
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    h.write_u32(std::process::id());
    let a = h.finish();
    h.write_u64(a);
    format!("{:016x}{:016x}", a, h.finish())
}

fn sibling(out: &std::path::Path, suffix: &str) -> PathBuf {
    let stem = out.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "game".into());
    out.with_file_name(format!("{stem}{suffix}"))
}

fn scripted(kind: &PlayerKind, seed: u64) -> Box<dyn Agent> {
    match kind {
        PlayerKind::Random => Box::new(RandomAgent::new(seed)),
        _ => Box::new(GreedyAgent::new(seed)),
    }
}

/// Play one game. `on_ready` is called once the MCP server is listening (before play starts) —
/// external clients/tests use it to learn the endpoints.
pub fn run(opts: &Options, on_ready: impl FnOnce(&ServerInfo)) -> anyhow::Result<RunReport> {
    let (game, label) = cna_replay::scenarios::build(&opts.scenario, opts.seed);

    // One request channel + endpoint per model-controlled side.
    let mut endpoints = Vec::new();
    let mut receivers = HashMap::new();
    let mut tokens = HashMap::new();
    for (i, (side, kind)) in [(Side::Commonwealth, &opts.commonwealth), (Side::Axis, &opts.axis)].into_iter().enumerate() {
        if *kind == PlayerKind::Llm {
            let (tx, rx) = std::sync::mpsc::channel();
            let token = match &opts.tokens {
                Some((cw, ax)) => if side == Side::Commonwealth { cw.clone() } else { ax.clone() },
                None => random_token(i as u64 + opts.seed),
            };
            tokens.insert(side, token.clone());
            endpoints.push(Endpoint { side, tx, token });
            receivers.insert(side, rx);
        }
    }
    let rules = Arc::new(RulesIndex::parse(RULES_MD));
    let server = server::start(endpoints, rules, opts.port)?;

    // MCP client configs for launched players.
    let transcript_dir = sibling(&opts.out, ".transcript");
    std::fs::create_dir_all(&transcript_dir)?;
    let mut mcp_configs = HashMap::new();
    for (&side, token) in &tokens {
        let cfg = serde_json::json!({ "mcpServers": { "cna": {
            "type": "http", "url": server.url(side), "headers": { "Authorization": format!("Bearer {token}") }
        }}});
        let path = transcript_dir.join(format!("mcp-{side:?}.json").to_lowercase());
        std::fs::write(&path, serde_json::to_string_pretty(&cfg)?)?;
        mcp_configs.insert(side, path);
    }
    let info = ServerInfo { endpoints: tokens.iter().map(|(&s, t)| (s, server.url(s), t.clone())).collect() };
    on_ready(&info);

    let make = |side: Side, kind: &PlayerKind, seed: u64, rx: Option<std::sync::mpsc::Receiver<_>>| -> Box<dyn Agent> {
        match rx {
            Some(rx) => {
                let launcher = (!opts.external).then(|| Launcher {
                    template: opts.player_cmd.clone(),
                    model: opts.model.clone(),
                    budget_usd: opts.budget_usd.clone(),
                    mcp_configs: mcp_configs.clone(),
                    transcript_dir: transcript_dir.clone(),
                });
                let mut a = McpAgent::new(side, rx, launcher, opts.window_timeout)
                    .with_context(&opts.scenario, Some(transcript_dir.clone()));
                a.auto_pass = opts.auto_pass;
                Box::new(a)
            }
            None => scripted(kind, seed),
        }
    };
    let mut cw = make(Side::Commonwealth, &opts.commonwealth, 1, receivers.remove(&Side::Commonwealth));
    let mut ax = make(Side::Axis, &opts.axis, 2, receivers.remove(&Side::Axis));
    let name = |k: &PlayerKind| match k {
        PlayerKind::Llm => match (&opts.external, &opts.model) {
            (true, _) => "LLM (external MCP client)".to_string(),
            (false, Some(m)) => format!("LLM ({m})"),
            (false, None) => "LLM (claude)".to_string(),
        },
        PlayerKind::Greedy => "Greedy".into(),
        PlayerKind::Random => "Random".into(),
    };

    let commands_path = sibling(&opts.out, ".commands.jsonl");
    let header = log::LogHeader {
        scenario: opts.scenario.clone(),
        seed: opts.seed,
        turns: opts.turns,
        commonwealth: name(&opts.commonwealth),
        axis: name(&opts.axis),
    };
    let mut cmdlog = log::CommandLog::create(&commands_path, &header)?;
    let mut w = std::io::BufWriter::new(std::fs::File::create(&opts.out)?);
    let start = std::time::Instant::now();
    let turns = opts.turns;
    let mut on_turn = |turn: u32, events: u32| {
        eprintln!("[{:>6.1}s] turn {turn}/{turns}: {events} events", start.elapsed().as_secs_f64());
    };
    let (steps, final_game) = cna_replay::record_game_streaming_ext(
        game,
        label,
        opts.seed,
        &header.commonwealth,
        cw.as_mut(),
        &header.axis,
        ax.as_mut(),
        opts.turns,
        &mut w,
        &mut on_turn,
        &mut |g, side, cmd, result| cmdlog.record(g, side, cmd, result.is_ok()),
    )?;
    w.flush()?;
    let fingerprint = cmdlog.finish(&final_game)?;
    // Dropping the agents drops their request channels, so any parked tool call (e.g. a final
    // wait_for_turn) resolves to "game over"; give the handlers a moment to send that before the
    // server stops.
    drop(cw);
    drop(ax);
    std::thread::sleep(std::time::Duration::from_millis(500));
    drop(server);
    Ok(RunReport { steps, fingerprint, commands_path })
}

/// Re-apply a command log from its scenario + seed; returns (fingerprint, expected, ok/rejected mismatches).
pub fn replay(path: &std::path::Path) -> anyhow::Result<(String, Option<String>, u64)> {
    let loaded = log::load(path)?;
    let (mut game, _) = cna_replay::scenarios::build(&loaded.header.scenario, loaded.header.seed);
    let (mut cw, mut ax, mismatches) = log::ReplayAgent::pair(loaded.attempts);
    cna_engine::agent::self_play(&mut game, &mut cw, &mut ax, loaded.header.turns);
    let fp = log::fingerprint(&game);
    let m = *mismatches.borrow();
    Ok((fp, loaded.trailer.map(|t| t.final_fingerprint), m))
}
