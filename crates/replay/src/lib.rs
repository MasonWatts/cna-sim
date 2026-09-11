//! Records a deterministic CNA-Sim game as a JSON event log for the spectator website.
//!
//! See `docs/visualization-architecture.md`. The static map is serialized **once**; each event
//! carries the acting side, a human-readable action + outcome, and the dynamic unit state *after*
//! the action, so a dumb (no-engine) frontend can render the board at every step. Commands are
//! rendered as strings for this v0 — a round-trippable `Command` schema (shared with the future
//! MCP interface) and per-action `§` rule citations are follow-on passes.

use cna_engine::agent::Agent;
use cna_engine::reference::units::Side;
use cna_engine::state::game::{GameState, Phase};
use serde::Serialize;
use ts_rs::TS;

/// A complete recorded game: header (scenario, seed, agents, static map) + the ordered event log.
///
/// The `#[derive(TS)]` on this and the types below generates the frontend's TypeScript types into
/// `crates/replay/bindings/` when `cargo test` runs — the `game.json` schema is single-sourced here.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct GameRecord {
    pub scenario: String,
    pub seed: u64,
    pub commonwealth_agent: String,
    pub axis_agent: String,
    pub map: MapView,
    pub events: Vec<Event>,
}

/// The static terrain grid, serialized once in the header.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct MapView {
    pub hexes: Vec<HexView>,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct HexView {
    pub col: i16,
    pub row: i16,
    pub terrain: String,
}

/// One recorded step: the state *after* an accepted command (or the initial frame).
#[derive(Serialize, TS)]
#[ts(export)]
pub struct Event {
    pub step: u32,
    pub game_turn: u32,
    pub operations_stage: u8,
    pub phase: String,
    /// Acting side (`"-"` for the initial frame).
    pub side: String,
    /// Human-readable command (v0: `Debug`).
    pub action: String,
    /// Human-readable command outcome (v0: `Debug`).
    pub outcome: String,
    /// `§` rule citations for this action — populated in a later pass.
    pub rules: Vec<String>,
    /// **Delta**: only the units whose view changed since the previous event (step 0 lists them
    /// all). To render the board at step N, fold `units` from step 0 through N. Keeps the log small
    /// on large maps — most steps touch one or two units.
    pub units: Vec<UnitView>,
    /// Unit ids removed (eliminated / left the map) since the previous event.
    pub removed: Vec<u32>,
}

#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct UnitView {
    pub id: u32,
    pub name: String,
    pub nationality: String,
    pub side: String,
    /// `[col, row]` of the unit's effective hex (attached units ride their parent), or `None`.
    pub hex: Option<[i16; 2]>,
    pub org_size: String,
    pub toe: u32,
    pub fuel: u32,
    pub ammo: u32,
    pub cohesion: i32,
    /// True if the unit has broken-down vehicles / weapons or lost TOE to breakdown (§21) — i.e. it
    /// is (partly) immobilized until repaired (§22). Drives the "broken" marker in the viewer.
    pub broken: bool,
}

/// The eight phases in which agents act (mirrors `agent::self_play`).
fn is_decision_phase(p: Phase) -> bool {
    matches!(
        p,
        Phase::MovementAndCombat
            | Phase::Organization
            | Phase::LandSupportAir
            | Phase::TruckConvoy
            | Phase::Repair
            | Phase::CommonwealthFleet
            | Phase::NavalConvoyArrival
            | Phase::NavalConvoySchedule
    )
}

fn map_view(g: &GameState) -> MapView {
    let mut hexes: Vec<HexView> = g
        .map
        .coords()
        .map(|c| HexView {
            col: c.col,
            row: c.row,
            terrain: g.map.terrain(c).map(|t| format!("{t:?}")).unwrap_or_default(),
        })
        .collect();
    hexes.sort_by_key(|h| (h.col, h.row));
    MapView { hexes }
}

fn unit_views(g: &GameState) -> Vec<UnitView> {
    let mut v: Vec<UnitView> = g
        .units
        .values()
        .map(|u| UnitView {
            id: u.id.0,
            name: u.name.clone(),
            nationality: format!("{:?}", u.nationality),
            side: format!("{:?}", u.nationality.side()),
            hex: g.effective_hex(u.id).map(|h| [h.col, h.row]),
            org_size: format!("{:?}", u.org_size),
            toe: u.total_toe(),
            fuel: u.fuel,
            ammo: u.ammo,
            cohesion: u.cohesion,
            broken: !u.weapons_broken.is_empty()
                || u.toe_broken > 0
                || u.trucks_broken.light + u.trucks_broken.medium + u.trucks_broken.heavy > 0,
        })
        .collect();
    v.sort_by_key(|u| u.id);
    v
}

/// Build an event as a **delta** vs `prev` (the units as of the last event): only changed units go
/// in `units`, removed ids in `removed`. `prev` is updated to the current full set.
fn delta_frame(
    game: &GameState,
    step: u32,
    side: &str,
    action: String,
    outcome: String,
    rules: Vec<String>,
    prev: &mut std::collections::HashMap<u32, UnitView>,
) -> Event {
    let cur = unit_views(game);
    let cur_ids: std::collections::HashSet<u32> = cur.iter().map(|u| u.id).collect();
    let units: Vec<UnitView> = cur.iter().filter(|u| prev.get(&u.id) != Some(*u)).cloned().collect();
    let mut removed: Vec<u32> = prev.keys().copied().filter(|id| !cur_ids.contains(id)).collect();
    removed.sort_unstable();
    *prev = cur.into_iter().map(|u| (u.id, u)).collect();
    Event {
        step,
        game_turn: game.game_turn,
        operations_stage: game.operations_stage,
        phase: format!("{:?}", game.phase),
        side: side.to_string(),
        action,
        outcome,
        rules,
        units,
        removed,
    }
}

/// Run `game` to `max_turns` with the two agents, recording an [`Event`] per accepted command.
/// Deterministic given the game seed and the agents' seeds.
#[allow(clippy::too_many_arguments)]
pub fn record_game(
    mut game: GameState,
    scenario: &str,
    seed: u64,
    commonwealth_name: &str,
    commonwealth: &mut dyn Agent,
    axis_name: &str,
    axis: &mut dyn Agent,
    max_turns: u32,
) -> GameRecord {
    let map = map_view(&game);
    let mut prev: std::collections::HashMap<u32, UnitView> = std::collections::HashMap::new();
    let mut events = vec![delta_frame(&game, 0, "-", "start".into(), String::new(), Vec::new(), &mut prev)];
    let mut step = 0u32;
    let mut guard = 0u64;
    while game.game_turn <= max_turns {
        // Stop early on a §64.71/§64.72 automatic campaign win (None for non-campaign scenarios).
        if game.campaign_auto_winner().is_some() {
            break;
        }
        if is_decision_phase(game.phase) {
            let side = game.active_player;
            let on_map = game
                .units
                .values()
                .filter(|u| u.nationality.side() == side && u.hex().is_some())
                .count();
            let cap = 3 * on_map as u32 + 8;
            let mut acted = 0u32;
            while acted < cap {
                let agent: &mut dyn Agent = if side == Side::Commonwealth { commonwealth } else { axis };
                match agent.choose(&game, side) {
                    Some(cmd) => {
                        let action = format!("{cmd:?}");
                        let rules: Vec<String> = cmd.rule_refs().iter().map(|s| s.to_string()).collect();
                        if let Ok(outcome) = game.apply(cmd) {
                            step += 1;
                            events.push(delta_frame(&game, step, &format!("{side:?}"), action, format!("{outcome:?}"), rules, &mut prev));
                        }
                        acted += 1;
                    }
                    None => break,
                }
            }
        }
        game.advance_phase();
        guard += 1;
        assert!(guard < 5_000_000, "record loop failed to progress");
    }
    GameRecord {
        scenario: scenario.to_string(),
        seed,
        commonwealth_agent: commonwealth_name.to_string(),
        axis_agent: axis_name.to_string(),
        map,
        events,
    }
}

/// Like [`record_game`] but streams the log as **JSONL** to `out` instead of buffering the whole
/// `GameRecord` in memory: the first line is a header object (scenario/seed/agents/map), then one
/// [`Event`] per line, flushed at each Game-Turn boundary. This gives live progress (via `on_turn`,
/// called with `(game_turn, events_so_far)` whenever the turn advances) and crash/kill-safety, and
/// keeps memory bounded for a full-campaign run. Returns the number of event lines written (excluding
/// the header). The viewer reconstructs a `GameRecord` from `{...header, events: <rest of lines>}`.
#[allow(clippy::too_many_arguments)]
pub fn record_game_streaming(
    mut game: GameState,
    scenario: &str,
    seed: u64,
    commonwealth_name: &str,
    commonwealth: &mut dyn Agent,
    axis_name: &str,
    axis: &mut dyn Agent,
    max_turns: u32,
    out: &mut dyn std::io::Write,
    on_turn: &mut dyn FnMut(u32, u32),
) -> std::io::Result<u32> {
    let header = serde_json::json!({
        "scenario": scenario,
        "seed": seed,
        "commonwealth_agent": commonwealth_name,
        "axis_agent": axis_name,
        "map": map_view(&game),
    });
    writeln!(out, "{}", serde_json::to_string(&header)?)?;

    let mut prev: std::collections::HashMap<u32, UnitView> = std::collections::HashMap::new();
    let start = delta_frame(&game, 0, "-", "start".into(), String::new(), Vec::new(), &mut prev);
    writeln!(out, "{}", serde_json::to_string(&start)?)?;

    let mut step = 0u32;
    let mut guard = 0u64;
    let mut last_turn = game.game_turn;
    on_turn(game.game_turn, step);
    while game.game_turn <= max_turns {
        if game.campaign_auto_winner().is_some() {
            break;
        }
        if is_decision_phase(game.phase) {
            let side = game.active_player;
            let on_map = game
                .units
                .values()
                .filter(|u| u.nationality.side() == side && u.hex().is_some())
                .count();
            let cap = 3 * on_map as u32 + 8;
            let mut acted = 0u32;
            while acted < cap {
                let agent: &mut dyn Agent = if side == Side::Commonwealth { commonwealth } else { axis };
                match agent.choose(&game, side) {
                    Some(cmd) => {
                        let action = format!("{cmd:?}");
                        let rules: Vec<String> = cmd.rule_refs().iter().map(|s| s.to_string()).collect();
                        if let Ok(outcome) = game.apply(cmd) {
                            step += 1;
                            let ev = delta_frame(&game, step, &format!("{side:?}"), action, format!("{outcome:?}"), rules, &mut prev);
                            writeln!(out, "{}", serde_json::to_string(&ev)?)?;
                        }
                        acted += 1;
                    }
                    None => break,
                }
            }
        }
        game.advance_phase();
        if game.game_turn != last_turn {
            last_turn = game.game_turn;
            out.flush()?;
            on_turn(game.game_turn, step);
        }
        guard += 1;
        assert!(guard < 5_000_000, "record loop failed to progress");
    }
    out.flush()?;
    Ok(step)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cna_engine::agent::{GreedyAgent, RandomAgent};
    use cna_engine::reference::terrain::HexTerrain;
    use cna_engine::reference::units::Nationality;
    use cna_engine::reference::ReferenceData;
    use cna_engine::state::map::HexCoord;
    use cna_engine::state::unit::{Composition, UnitLocation};

    #[test]
    fn records_a_game_to_a_wellformed_log() {
        let mut g = GameState::new(ReferenceData::load_campaign(), 3);
        for col in 0..6 {
            g.map.set_hex(HexCoord::new(col, 1), HexTerrain::Clear);
        }
        for (nat, id, col) in [
            (Nationality::Commonwealth, "h", 1),
            (Nationality::Italian, "p", 4),
        ] {
            let u = g.spawn_unit(nat, id, "u", Composition::Organic { current_toe: 8 });
            let unit = g.unit_mut(u).unwrap();
            unit.location = Some(UnitLocation::OnMap(HexCoord::new(col, 1)));
            unit.fuel = 400;
            unit.ammo = 400;
        }
        let mut cw = GreedyAgent::new(1);
        let mut ax = RandomAgent::new(2);
        let rec = record_game(g, "T", 3, "Greedy", &mut cw, "Random", &mut ax, 2);

        // Header: static map serialized once, both agent names present.
        assert!(!rec.map.hexes.is_empty(), "map is captured in the header");
        assert_eq!((rec.commonwealth_agent.as_str(), rec.axis_agent.as_str()), ("Greedy", "Random"));
        // Events: an initial frame, then per-command frames with monotonically increasing steps.
        assert!(rec.events.len() >= 2, "at least a start frame and some play");
        assert_eq!(rec.events[0].step, 0);
        assert_eq!(rec.events[0].action, "start");
        for w in rec.events.windows(2) {
            assert_eq!(w[1].step, w[0].step + 1, "steps are contiguous");
        }
        // Step 0 lists all units; later events are deltas (≤ 2 changed). Folding reconstructs both.
        assert_eq!(rec.events[0].units.len(), 2, "step 0 lists all units");
        assert!(rec.events.iter().all(|e| e.units.len() <= 2), "later events are deltas");
        let mut folded: std::collections::HashMap<u32, UnitView> = std::collections::HashMap::new();
        for e in &rec.events {
            for uv in &e.units {
                folded.insert(uv.id, uv.clone());
            }
            for r in &e.removed {
                folded.remove(r);
            }
        }
        assert_eq!(folded.len(), 2, "folding the deltas reconstructs both units");
        assert!(rec.events.last().unwrap().game_turn >= 1);
        // The whole record round-trips through serde_json.
        let json = serde_json::to_string(&rec).unwrap();
        assert!(json.contains("\"scenario\":\"T\"") && json.contains("\"map\""));
    }
}
