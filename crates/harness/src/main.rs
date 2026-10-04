//! `cna-harness` — play CNA-Sim with AI models as players (see docs/llm-player-harness.md).
//!
//! ```text
//! cna-harness [--scenario skirmish|scenario1|campaign] [--turns N] [--seed S]
//!             [--commonwealth llm|greedy|random] [--axis llm|greedy|random]
//!             [--llm-phases movement,organization,...|all] [--out game.jsonl]
//!             [--model M] [--budget-usd X] [--player-cmd '<template>']
//!             [--external] [--window-timeout SECS] [--port P]
//! cna-harness --replay game.commands.jsonl
//! ```

use std::time::Duration;

use cna_harness::{Options, PlayerKind};

fn usage() -> ! {
    eprintln!(
        "usage: cna-harness [--scenario skirmish|scenario1|campaign] [--turns N] [--seed S]\n\
         \x20                 [--commonwealth llm|greedy|random] [--axis llm|greedy|random]\n\
         \x20                 [--llm-phases movement,organization,air,convoy,repair,fleet,naval|all]\n\
         \x20                 [--out game.jsonl] [--model M] [--budget-usd X] [--player-cmd '<tmpl>']\n\
         \x20                 [--external] [--window-timeout SECS] [--port P]\n\
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
            "--llm-phases" => o.llm_phases = val(),
            "--out" => o.out = val().into(),
            "--model" => o.model = Some(val()),
            "--budget-usd" => o.budget_usd = Some(val()),
            "--player-cmd" => o.player_cmd = Some(val()),
            "--external" => o.external = true,
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
    Ok(())
}
