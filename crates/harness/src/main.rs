//! `cna-harness` — play CNA-Sim with AI models as players (see docs/llm-player-harness.md).
//!
//! ```text
//! cna-harness [--scenario skirmish|scenario1|campaign] [--turns N] [--seed S]
//!             [--commonwealth llm|greedy|random] [--axis llm|greedy|random]
//!             [--out game.jsonl]
//!             [--model M] [--routine-model M] [--budget-usd X] [--max-cost-usd X] [--player-cmd '<template>']
//!             [--external] [--no-auto-pass] [--window-timeout SECS] [--port P]
//!             [--no-pregame] [--pregame-timeout SECS] [--pregame-model M] [--pregame-budget-usd X]
//!             [--coaching]
//! cna-harness --resume game.commands.jsonl [same player flags]   # continue a crashed/stopped game
//! cna-harness --replay game.commands.jsonl
//! ```

use std::time::Duration;

use cna_harness::{Options, PlayerKind};

fn usage() -> ! {
    eprintln!(
        "usage: cna-harness [--scenario skirmish|scenario1|campaign] [--turns N] [--seed S]\n\
         \x20                 [--commonwealth llm|greedy|random] [--axis llm|greedy|random]\n\
         \x20                 [--out game.jsonl] [--model M] [--routine-model M] [--budget-usd X]\n\
         \x20                 [--max-cost-usd X] [--resume <commands.jsonl>] [--player-cmd '<tmpl>']\n\
         \x20                 [--external] [--no-auto-pass] [--window-timeout SECS] [--port P]\n\
         \x20                 [--no-pregame] [--pregame-timeout SECS] [--pregame-model M]\n\
         \x20                 [--pregame-budget-usd X|none] [--coaching]\n\
         \x20      cna-harness --replay <game.commands.jsonl>"
    );
    std::process::exit(2)
}

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let mut o = Options::default();
    let mut replay = None;
    while let Some(a) = args.next() {
        let mut val = || args.next().unwrap_or_else(|| usage());
        match a.as_str() {
            "--scenario" => o.scenario = val(),
            "--turns" => o.turns = val().parse()?,
            "--seed" => o.seed = val().parse()?,
            "--commonwealth" | "--cw" => o.commonwealth = PlayerKind::parse(&val())?,
            "--axis" => o.axis = PlayerKind::parse(&val())?,
            "--out" => o.out = val().into(),
            "--model" => o.model = Some(val()),
            "--routine-model" => o.routine_model = Some(val()),
            "--max-cost-usd" => o.max_cost_usd = Some(val().parse()?),
            "--resume" => o.resume = Some(val().into()),
            // "none" removes the cap (and its mention in the prompt) entirely.
            "--budget-usd" => o.budget_usd = Some(val()).filter(|v| v != "none"),
            "--player-cmd" => o.player_cmd = Some(val()),
            "--external" => o.external = true,
            "--no-auto-pass" => o.auto_pass = false,
            "--no-pregame" => o.pregame = false,
            "--pregame-timeout" => o.pregame_timeout = Duration::from_secs(val().parse()?),
            "--pregame-model" => o.pregame_model = Some(val()),
            "--pregame-budget-usd" => o.pregame_budget_usd = Some(val()).filter(|v| v != "none"),
            "--coaching" => o.coaching = true,
            "--window-timeout" => o.window_timeout = Duration::from_secs(val().parse()?),
            "--port" => o.port = val().parse()?,
            "--replay" => replay = Some(val()),
            "-h" | "--help" => usage(),
            other => {
                eprintln!("unknown argument `{other}`");
                usage()
            }
        }
    }

    if let Some(path) = replay {
        let (fp, expected, mismatches) = cna_harness::replay(std::path::Path::new(&path))?;
        println!("replayed fingerprint: {fp}");
        match expected {
            Some(e) if e == fp && mismatches == 0 => println!("OK — matches the recorded game ({e})"),
            Some(e) => {
                println!("MISMATCH — recorded {e}, {mismatches} accept/reject differences");
                std::process::exit(1);
            }
            None => println!("(log has no trailer; game may not have finished)"),
        }
        return Ok(());
    }

    let external = o.external;
    let report = cna_harness::run(&o, |info| {
        for (side, url, token) in &info.endpoints {
            eprintln!("[harness] {side:?} MCP endpoint: {url}");
            if external {
                eprintln!("[harness]   Authorization: Bearer {token}");
            }
        }
    })?;
    eprintln!(
        "[harness] done: {} events → {} ; commands → {} ; fingerprint {}",
        report.steps,
        o.out.display(),
        report.commands_path.display(),
        report.fingerprint
    );
    eprintln!("[harness] RESULT {}", cna_harness::result::summary(&report.result));
    let mut spend: Vec<_> = report.spend_usd.iter().map(|(s, c)| format!("{s:?} ${c:.2}")).collect();
    spend.sort();
    if !spend.is_empty() {
        eprintln!("[harness] model spend: {}", spend.join(", "));
    }
    Ok(())
}
