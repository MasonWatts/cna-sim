//! Route-finding for the `move_to` / `move_convoy_to` plan shortcuts: a shortest path over the
//! map's static step costs (terrain, hexsides, roads and tracks — `rules::movement::step_cost`),
//! so a mover goes around a salt marsh or an escarpment along the road instead of stalling at it.
//! Dynamic limits (ZOC, stacking, fuel, water, CP left) are left to the engine: each first step is
//! still validated before it is taken, and a refused one falls back to the next-best route.
//! Routing reads only the map, never enemy positions (fog of war).

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use cna_engine::reference::terrain::MoveCost;
use cna_engine::rules::movement::{step_cost, MoveMode};
use cna_engine::state::game::GameState;
use cna_engine::state::map::HexCoord;

/// Search budget (settled hexes) — far more than a stage's movement needs.
const MAX_SETTLED: usize = 60_000;

fn cost(game: &GameState, from: HexCoord, to: HexCoord, mode: MoveMode) -> Option<i32> {
    match step_cost(&game.reference, &game.map, from, to, mode)? {
        MoveCost::Cost(f) => Some(f.0.max(0)),
        _ => None,
    }
}

/// The neighbours of `from` that start a route to `to` in `mode`, best first, each with the total
/// route cost in tenths of a CP. Empty if `to` is unreachable (or `from == to`).
pub fn first_steps(game: &GameState, from: HexCoord, to: HexCoord, mode: MoveMode) -> Vec<(HexCoord, i32)> {
    if from == to || game.map.hex(to).is_none() {
        return Vec::new();
    }
    // Backward Dijkstra from the target: dist[x] = cheapest cost from x to `to` (edges x → y).
    let mut dist: HashMap<HexCoord, i32> = HashMap::new();
    let mut heap = BinaryHeap::new();
    dist.insert(to, 0);
    heap.push(Reverse((0, to.col, to.row)));
    let mut settled = 0usize;
    let mut bound = i32::MAX;
    while let Some(Reverse((d, c, r))) = heap.pop() {
        let y = HexCoord::new(c, r);
        if dist.get(&y).is_some_and(|&b| b < d) {
            continue;
        }
        if d > bound {
            break;
        }
        settled += 1;
        if settled > MAX_SETTLED {
            break;
        }
        if y == from {
            // Keep going a little so every neighbour of `from` on a near-best route is settled.
            bound = d + 200;
        }
        for x in game.map.neighbors(y) {
            if let Some(step) = cost(game, x, y, mode) {
                let nd = d + step;
                if dist.get(&x).is_none_or(|&b| nd < b) {
                    dist.insert(x, nd);
                    heap.push(Reverse((nd, x.col, x.row)));
                }
            }
        }
    }
    let mut out: Vec<(HexCoord, i32)> = game
        .map
        .neighbors(from)
        .filter_map(|n| Some((n, cost(game, from, n, mode)? + *dist.get(&n)?)))
        .collect();
    out.sort_by_key(|&(n, c)| (c, n.distance(to), n.col, n.row));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cna_engine::state::map::HexId;

    fn hx(s: &str) -> HexCoord {
        HexId::parse(s).unwrap().to_hex_coord()
    }

    #[test]
    fn routes_around_what_the_direct_line_cannot_cross() {
        let (g, _) = cna_replay::scenarios::build("campaign", 1);
        // The smoke run's "convoy 4 blocked by Alexandria marsh": a motorized route from
        // Alexandria west to Mersa Matruh exists even where a straight step can't be made.
        let (from, to) = (hx("E3613"), hx("D3714"));
        let t = std::time::Instant::now();
        let steps = first_steps(&g, from, to, MoveMode::Motorized);
        assert!(t.elapsed().as_millis() < 500, "fast enough per step: {:?}", t.elapsed());
        assert!(!steps.is_empty(), "a motorized route exists");
        let (first, total) = steps[0];
        assert!(cost(&g, from, first, MoveMode::Motorized).is_some(), "the first step itself is passable");
        assert!(total > 0);
        // Following the best first step repeatedly reaches the target.
        let mut at = from;
        for _ in 0..200 {
            if at == to {
                break;
            }
            at = first_steps(&g, at, to, MoveMode::Motorized)[0].0;
        }
        assert_eq!(at, to, "the route arrives");
        // A direct-line greedy step from Alexandria would hit prohibited terrain somewhere.
        let greedy_stalls = {
            let mut at = from;
            let mut stalled = false;
            for _ in 0..200 {
                if at == to {
                    break;
                }
                let here = at.distance(to);
                match g.map.neighbors(at).filter(|n| n.distance(to) < here).find(|&n| cost(&g, at, n, MoveMode::Motorized).is_some()) {
                    Some(n) => at = n,
                    None => {
                        stalled = true;
                        break;
                    }
                }
            }
            stalled
        };
        assert!(greedy_stalls, "the old closer-neighbour stepping would stall on this leg");
    }
}
