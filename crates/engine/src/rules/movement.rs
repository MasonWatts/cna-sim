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
    crossing_cost_with(refs, map, from, to, mode, false)
}

/// `crossing_cost`, but when `road_as_track` a Road hexside is charged the Track crossing cost
/// (§29.56: during a rainstorm road hexes cost the same as tracks).
pub fn crossing_cost_with(
    refs: &ReferenceData,
    map: &crate::state::map::Map,
    from: crate::state::map::HexCoord,
    to: crate::state::map::HexCoord,
    mode: MoveMode,
    road_as_track: bool,
) -> MoveCost {
    use crate::reference::terrain::HexsideTerrain;
    use crate::state::map::HexsideKind;
    let sides = map.hexsides_on(from, to);
    if sides.is_empty() {
        return MoveCost::Cost(Fixed(0));
    }
    let cost_of = |h: &crate::state::map::Hexside| -> MoveCost {
        let mut key = h.resolve(from, to);
        if road_as_track && key == HexsideTerrain::Road {
            key = HexsideTerrain::Track;
        }
        refs.hexsides
            .get(&key)
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

/// RULE (§8.37 chart footnotes [6]/[8], §8.46): total CP to step from `from` into adjacent `to`.
/// A **Road** hexside negates the hex terrain and *all* hexside features — the step costs the flat
/// road rate (footnote 6). A **Track** (or an unfinished road, §24.52) *halves* the entered hex's
/// terrain cost and its hexside-crossing costs, except the down-escarpment CP (footnote 8, §8.42).
/// Otherwise the cost is the terrain entry cost plus the most-restrictive hexside obstacle. `None`
/// if `to` is off the map; Prohibited on either part is Prohibited (a Track never makes a Prohibited
/// crossing legal — halving Prohibited stays Prohibited).
pub fn step_cost(
    refs: &ReferenceData,
    map: &crate::state::map::Map,
    from: crate::state::map::HexCoord,
    to: crate::state::map::HexCoord,
    mode: MoveMode,
) -> Option<MoveCost> {
    step_cost_with(refs, map, from, to, mode, false)
}

/// `step_cost`, but when `road_as_track` a Road hexside behaves like a Track (§29.56: during a
/// rainstorm road hexes cost the same as tracks).
pub fn step_cost_with(
    refs: &ReferenceData,
    map: &crate::state::map::Map,
    from: crate::state::map::HexCoord,
    to: crate::state::map::HexCoord,
    mode: MoveMode,
    road_as_track: bool,
) -> Option<MoveCost> {
    use crate::state::map::HexsideKind;
    let entry = hex_entry_cost(refs, map, to, mode)?;
    let sides = map.hexsides_on(from, to);
    let mut has_road = sides.iter().any(|h| h.kind == HexsideKind::Road);
    let mut has_track = sides
        .iter()
        .any(|h| matches!(h.kind, HexsideKind::Track | HexsideKind::UnfinishedRoad));
    if road_as_track && has_road {
        has_road = false;
        has_track = true;
    }
    let (obstacle, down_escarp) = obstacle_crossing(refs, map, from, to, mode);
    if has_road {
        // §8.37 [6]: a road negates terrain + all hexside features → flat road rate.
        Some(road_rate(refs, mode))
    } else if has_track {
        // §8.37 [8]: a track halves the terrain entry and hexside costs, except a down-escarpment.
        let obs = if down_escarp { obstacle } else { halve_cost(obstacle) };
        Some(combine_cost(halve_cost(entry), obs))
    } else {
        Some(combine_cost(entry, obstacle))
    }
}

/// §21.21/§8.37: the Breakdown Points a vehicle garners for the step `from`→`to`, following the
/// same route choice as `step_cost`: along a Road, the flat Road value (½ BP — note 6: it negates
/// the hex's and hexsides' values); along a Track, half the hex and hexside values except a
/// down-escarpment (note 8; §8.41: a wadi on a track is 4, not 8); otherwise the hex value plus each
/// crossed hexside feature's value (§21.23 example: Rough 8 + Ridge 2). `road_as_track` applies the
/// rainstorm rule (§21.37c/§29.56); `offroad` the §9.33 move-around (no route benefit).
pub fn step_breakdown(
    refs: &ReferenceData,
    map: &crate::state::map::Map,
    from: crate::state::map::HexCoord,
    to: crate::state::map::HexCoord,
    road_as_track: bool,
    offroad: bool,
) -> Fixed {
    use crate::reference::terrain::HexsideTerrain;
    use crate::state::map::HexsideKind;
    let hex = map.terrain(to).and_then(|t| refs.terrain.get(&t)).map_or(0, |p| p.breakdown.0.max(0));
    let sides = map.hexsides_on(from, to);
    let mut road = !offroad && sides.iter().any(|h| h.kind == HexsideKind::Road);
    let mut track = !offroad && sides.iter().any(|h| matches!(h.kind, HexsideKind::Track | HexsideKind::UnfinishedRoad));
    if road_as_track && road {
        road = false;
        track = true;
    }
    if road {
        return refs.hexsides.get(&HexsideTerrain::Road).map_or(Fixed(5), |p| p.breakdown);
    }
    let mut features = 0;
    let mut down_escarp = 0;
    for h in sides {
        if matches!(
            h.kind,
            HexsideKind::Road
                | HexsideKind::Track
                | HexsideKind::UnfinishedRoad
                | HexsideKind::Railroad
                | HexsideKind::UnfinishedRailroad
                | HexsideKind::DestroyedRailroad
        ) {
            continue;
        }
        let key = h.resolve(from, to);
        let bp = refs.hexsides.get(&key).map_or(0, |p| p.breakdown.0.max(0));
        if key == HexsideTerrain::DownEscarpment {
            down_escarp += bp;
        } else {
            features += bp;
        }
    }
    if track {
        Fixed((hex + features) / 2 + down_escarp)
    } else {
        Fixed(hex + features + down_escarp)
    }
}

/// §9.29/§9.33: the Road/Track Stacking Limit — road-space points a road/track hex can hold.
pub const ROAD_TRACK_STACK_LIMIT: u32 = 5;

/// §9.33 "move around": the step cost *ignoring* any Road/Track route benefit — the full terrain
/// entry cost plus the worst hexside obstacle. Used when a motorized unit is denied the road benefit
/// because the destination road/track hex is already at its Road/Track Stacking Limit.
pub fn step_cost_offroad(
    refs: &ReferenceData,
    map: &crate::state::map::Map,
    from: crate::state::map::HexCoord,
    to: crate::state::map::HexCoord,
    mode: MoveMode,
) -> Option<MoveCost> {
    let entry = hex_entry_cost(refs, map, to, mode)?;
    let (obstacle, _) = obstacle_crossing(refs, map, from, to, mode);
    Some(combine_cost(entry, obstacle))
}

/// The flat Road along-move rate for `mode` (§8.37 Road row).
fn road_rate(refs: &ReferenceData, mode: MoveMode) -> MoveCost {
    use crate::reference::terrain::HexsideTerrain;
    refs.hexsides
        .get(&HexsideTerrain::Road)
        .map(|p| match mode {
            MoveMode::NonMotorized => p.cross.non_mot,
            MoveMode::Motorized => p.cross.mot,
        })
        .unwrap_or(MoveCost::Cost(Fixed(0)))
}

/// Halve a movement cost (§8.37 [8]); EntireCpa/Prohibited are not halved (Prohibited stays so).
fn halve_cost(c: MoveCost) -> MoveCost {
    match c {
        MoveCost::Cost(f) => MoveCost::Cost(Fixed(f.0 / 2)),
        other => other,
    }
}

/// The most-restrictive **non-route** hexside crossing obstacle (rivers, escarpments, ridges,
/// slopes, wadi, lake) between `from` and `to`, and whether that worst obstacle is a down-escarpment
/// (which a Track does not halve, §8.37 [8]). Route features (road/track/railroad) are excluded.
fn obstacle_crossing(
    refs: &ReferenceData,
    map: &crate::state::map::Map,
    from: crate::state::map::HexCoord,
    to: crate::state::map::HexCoord,
    mode: MoveMode,
) -> (MoveCost, bool) {
    use crate::reference::terrain::HexsideTerrain;
    use crate::state::map::HexsideKind;
    let rank = |c: MoveCost| match c {
        MoveCost::Cost(f) => (0, f.0),
        MoveCost::EntireCpa => (1, 0),
        MoveCost::Prohibited => (2, 0),
    };
    let mut worst = MoveCost::Cost(Fixed(0));
    let mut worst_down_escarp = false;
    for h in map.hexsides_on(from, to) {
        if matches!(
            h.kind,
            HexsideKind::Road
                | HexsideKind::Track
                | HexsideKind::UnfinishedRoad
                | HexsideKind::Railroad
                | HexsideKind::UnfinishedRailroad
                | HexsideKind::DestroyedRailroad
        ) {
            continue;
        }
        let key = h.resolve(from, to);
        let c = refs
            .hexsides
            .get(&key)
            .map(|p| match mode {
                MoveMode::NonMotorized => p.cross.non_mot,
                MoveMode::Motorized => p.cross.mot,
            })
            .unwrap_or(MoveCost::Cost(Fixed(0)));
        if rank(c) > rank(worst) {
            worst = c;
            worst_down_escarp = key == HexsideTerrain::DownEscarpment;
        }
    }
    (worst, worst_down_escarp)
}

fn combine_cost(a: MoveCost, b: MoveCost) -> MoveCost {
    use MoveCost::*;
    match (a, b) {
        (Prohibited, _) | (_, Prohibited) => Prohibited,
        (EntireCpa, _) | (_, EntireCpa) => EntireCpa,
        (Cost(x), Cost(y)) => Cost(Fixed(x.0 + y.0)),
    }
}
