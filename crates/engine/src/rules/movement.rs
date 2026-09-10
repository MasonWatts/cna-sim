//! Movement RULES (code) that consume terrain PARAMETERS (data).
//!
//! This module is a thin, pure cost layer (per-hex enter cost, multi-feature hexside
//! crossing cost, escarpment legality) exemplifying the parameters-vs-rules boundary
//! (design.md §3.8). The stateful movement engine — CPA expenditure/Disorganization, ZOC,
//! stacking, fuel/water gating, reaction, retreat, breakdown — lives in
//! `GameState::move_counter` and the move commands (`crate::state::command`).

use serde::{Deserialize, Serialize};
use crate::reference::terrain::{Fixed, HexTerrain, MoveCost};
use crate::reference::ReferenceData;

/// The movement mode of a unit, matching the Terrain Effects Chart's two CP columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveMode {
    NonMotorized,
    Motorized,
}

/// RULE: the cost for `mode` to enter a hex of `terrain`.
/// Returns `None` only if the terrain has no parameters loaded; a prohibited entry is
/// reported as `Some(MoveCost::Prohibited)`.
pub fn enter_cost(refs: &ReferenceData, terrain: HexTerrain, mode: MoveMode) -> Option<MoveCost> {
    let params = refs.terrain.get(&terrain)?;
    Some(match mode {
        MoveMode::NonMotorized => params.enter.non_mot,
        MoveMode::Motorized => params.enter.mot,
    })
}

/// RULE (§8.42): a vehicle may never cross an escarpment hexside in the "up" direction,
/// even via a track. Pure conditional logic — it belongs in CODE, not data.
pub fn vehicle_may_cross_escarpment(going_uphill: bool) -> bool {
    !going_uphill
}

/// RULE: the base CP cost for `mode` to enter the hex at `coord` on `map`, using the
/// hex's terrain and the loaded terrain parameters. Returns `None` if the hex isn't on
/// the map; a prohibited entry is `Some(MoveCost::Prohibited)`.
pub fn hex_entry_cost(
    refs: &ReferenceData,
    map: &crate::state::map::Map,
    coord: crate::state::map::HexCoord,
    mode: MoveMode,
) -> Option<MoveCost> {
    let terrain = map.terrain(coord)?;
    enter_cost(refs, terrain, mode)
}

/// RULE: extra CP to cross the hexside(s) between `from` and `to`, per mode. An edge may carry
/// several features (§8.37): a Road/Track/Railroad along the hexside provides a route that negates
/// the underlying obstacle, so the cheapest crossing applies; otherwise obstacles stack and the
/// most-restrictive crossing applies (a Prohibited feature prohibits the crossing).
pub fn crossing_cost(
    refs: &ReferenceData,
    map: &crate::state::map::Map,
    from: crate::state::map::HexCoord,
    to: crate::state::map::HexCoord,
    mode: MoveMode,
) -> MoveCost {
    use crate::state::map::HexsideKind;
    let sides = map.hexsides_on(from, to);
    if sides.is_empty() {
        return MoveCost::Cost(Fixed(0));
    }
    let cost_of = |h: &crate::state::map::Hexside| -> MoveCost {
        refs.hexsides
            .get(&h.resolve(from, to))
            .map(|p| match mode {
                MoveMode::NonMotorized => p.cross.non_mot,
                MoveMode::Motorized => p.cross.mot,
            })
            .unwrap_or(MoveCost::Cost(Fixed(0)))
    };
    // Order MoveCost: a fixed cost < "entire CPA" < Prohibited.
    let rank = |c: MoveCost| match c {
        MoveCost::Cost(f) => (0, f.0),
        MoveCost::EntireCpa => (1, 0),
        MoveCost::Prohibited => (2, 0),
    };
    let has_route = sides
        .iter()
        .any(|h| matches!(h.kind, HexsideKind::Road | HexsideKind::Track | HexsideKind::Railroad));
    let pick = |acc: Option<MoveCost>, c: MoveCost, want_min: bool| -> Option<MoveCost> {
        Some(match acc {
            None => c,
            Some(a) => {
                let take_c = if want_min { rank(c) < rank(a) } else { rank(c) > rank(a) };
                if take_c { c } else { a }
            }
        })
    };
    sides
        .iter()
        .map(cost_of)
        .fold(None, |acc, c| pick(acc, c, has_route))
        .unwrap_or(MoveCost::Cost(Fixed(0)))
}

/// RULE: total CP to step from `from` to adjacent `to` = hex entry cost + hexside
/// crossing cost. `None` if `to` is off the map; prohibited on either part is prohibited.
pub fn step_cost(
    refs: &ReferenceData,
    map: &crate::state::map::Map,
    from: crate::state::map::HexCoord,
    to: crate::state::map::HexCoord,
    mode: MoveMode,
) -> Option<MoveCost> {
    let entry = hex_entry_cost(refs, map, to, mode)?;
    Some(combine_cost(entry, crossing_cost(refs, map, from, to, mode)))
}

fn combine_cost(a: MoveCost, b: MoveCost) -> MoveCost {
    use MoveCost::*;
    match (a, b) {
        (Prohibited, _) | (_, Prohibited) => Prohibited,
        (EntireCpa, _) | (_, EntireCpa) => EntireCpa,
        (Cost(x), Cost(y)) => Cost(Fixed(x.0 + y.0)),
    }
}
