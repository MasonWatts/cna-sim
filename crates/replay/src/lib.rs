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
    /// Dynamic unit state after the action (the static map lives in the header).
    pub units: Vec<UnitView>,
}

#[derive(Serialize, TS)]
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
        })
        .collect();
    v.sort_by_key(|u| u.id);
    v
}

fn frame(game: &GameState, step: u32, side: &str, action: String, outcome: String) -> Event {
    Event {
        step,
        game_turn: game.game_turn,
        operations_stage: game.operations_stage,
        phase: format!("{:?}", game.phase),
        side: side.to_string(),
        action,
        outcome,
        rules: Vec::new(),
        units: unit_views(game),
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
    let mut events = vec![frame(&game, 0, "-", "start".into(), String::new())];
    let mut step = 0u32;
    let mut guard = 0u64;
    while game.game_turn <= max_turns {
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
                            let mut ev = frame(&game, step, &format!("{side:?}"), action, format!("{outcome:?}"));
                            ev.rules = rules;
                            events.push(ev);
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
        // Every frame snapshots both units; the game advanced past the start.
        assert!(rec.events.iter().all(|e| e.units.len() == 2));
        assert!(rec.events.last().unwrap().game_turn >= 1);
        // The whole record round-trips through serde_json.
        let json = serde_json::to_string(&rec).unwrap();
        assert!(json.contains("\"scenario\":\"T\"") && json.contains("\"map\""));
    }
}
