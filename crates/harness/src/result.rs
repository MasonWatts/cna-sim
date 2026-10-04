//! End-of-game result: who is winning and by how much, per the scenario's victory conditions.

use cna_engine::reference::units::Side;
use cna_engine::state::game::GameState;
use serde_json::{json, Value};

fn strength(game: &GameState, side: Side) -> (usize, u32) {
    let units: Vec<_> = game.units.values().filter(|u| u.nationality.side() == side && u.total_toe() > 0 && game.effective_hex(u.id).is_some()).collect();
    (units.len(), units.iter().map(|u| u.total_toe()).sum())
}

/// The result as JSON: campaign §64.71/§64.72 automatic win + §64.73–§64.76 VP and level;
/// Scenario 1 (§60.81) victory level per side; plus surviving on-map strength for every scenario.
pub fn game_result(game: &GameState, scenario: &str) -> Value {
    let (cwn, cwt) = strength(game, Side::Commonwealth);
    let (axn, axt) = strength(game, Side::Axis);
    let mut out = json!({
        "scenario": scenario,
        "final_game_turn": game.game_turn,
        "strength_on_map": {
            "Commonwealth": {"units": cwn, "toe": cwt},
            "Axis": {"units": axn, "toe": axt},
        },
    });
    match scenario {
        "campaign" => {
            out["automatic_winner"] = json!(game.campaign_auto_winner().map(|s| format!("{s:?}")));
            if let Some(r) = game.evaluate_campaign_victory() {
                out["victory_points"] = json!({"Axis": r.axis_vp, "Commonwealth": r.cw_vp});
                out["leader"] = json!(r.winner.map(|s| format!("{s:?}")));
                out["level"] = json!(r.level);
            }
        }
        "scenario1" => {
            out["victory_level"] = json!({
                "Axis": format!("{:?}", game.scenario1_victory(Side::Axis)),
                "Commonwealth": format!("{:?}", game.scenario1_victory(Side::Commonwealth)),
            });
        }
        _ => {
            out["leader"] = json!(match cwt.cmp(&axt) {
                std::cmp::Ordering::Greater => Some("Commonwealth"),
                std::cmp::Ordering::Less => Some("Axis"),
                std::cmp::Ordering::Equal => None,
            });
        }
    }
    out
}

/// One-line human summary.
pub fn summary(r: &Value) -> String {
    let s = &r["strength_on_map"];
    let base = format!(
        "after Game-Turn {}: CW {} units / {} TOE, Axis {} units / {} TOE",
        r["final_game_turn"].as_u64().unwrap_or(0).saturating_sub(1),
        s["Commonwealth"]["units"], s["Commonwealth"]["toe"], s["Axis"]["units"], s["Axis"]["toe"]
    );
    match r["scenario"].as_str() {
        Some("campaign") => format!(
            "{base}; VP Axis {} / CW {} ({}{}){}",
            r["victory_points"]["Axis"], r["victory_points"]["Commonwealth"],
            r["leader"].as_str().unwrap_or("level"), r["level"].as_str().map(|l| format!(" — {l}")).unwrap_or_default(),
            r["automatic_winner"].as_str().map(|w| format!("; AUTOMATIC WIN: {w}")).unwrap_or_default()
        ),
        Some("scenario1") => format!("{base}; §60.81 level Axis {} / CW {}", r["victory_level"]["Axis"], r["victory_level"]["Commonwealth"]),
        _ => format!("{base}; leader {}", r["leader"].as_str().unwrap_or("none")),
    }
}
