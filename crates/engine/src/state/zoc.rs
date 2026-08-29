//! Zones of Control — derived queries (§10.0). All computed from state + reference data
//! (store primitives, derive the rest, §3.2).
//!
//! Implemented: §10.11 (above battalion-equivalent / >1 stacking point), the §10.14
//! cohesion gate, the §10.15 "≥ 10 Raw Defensive Close Assault" gate, and the §10.21
//! projection geometry (blocked by sea/river/lake/escarpment hexsides via
//! `HexsideParams.blocks_zoc`).

use crate::reference::units::Side;
use crate::state::game::GameState;
use crate::state::map::HexCoord;
use crate::state::unit::{RatingKind, UnitId};

impl GameState {
    /// All units whose effective hex is `hex`.
    pub fn units_in_hex(&self, hex: HexCoord) -> Vec<UnitId> {
        self.units
            .keys()
            .copied()
            .filter(|&id| self.effective_hex(id) == Some(hex))
            .collect()
    }

    /// Total Raw Defensive Close Assault points of the units in `hex` (§10.15).
    pub fn hex_raw_defensive_ca(&self, hex: HexCoord) -> i32 {
        self.units_in_hex(hex)
            .iter()
            .filter_map(|id| self.units.get(id))
            .map(|u| u.raw_strength(&self.reference, RatingKind::CloseAssaultDef))
            .sum()
    }

    /// The side (if any) whose units in `hex` collectively project a Zone of Control.
    pub fn hex_projects_zoc(&self, hex: HexCoord) -> Option<Side> {
        let units = self.units_in_hex(hex);
        let first = *units.first()?;
        // §10.14: a stack of only -26-or-worse cohesion units exerts no ZOC.
        let any_capable = units
            .iter()
            .filter_map(|id| self.units.get(id))
            .any(|u| u.cohesion > -26);
        // §10.11: exerted only if above battalion-equivalent (>1 stacking point in hex);
        // §10.15: total Raw Defensive Close Assault must be ≥ 10.
        if !any_capable
            || self.hex_stacking_points(hex) <= 1
            || self.hex_raw_defensive_ca(hex) < 10
        {
            return None;
        }
        self.units.get(&first).map(|u| u.nationality.side())
    }

    /// Whether the hexside between `from` and `to` blocks ZOC projection (§10.21). Blocked if any
    /// feature on the edge blocks (a Road bridging a River does not open ZOC across the River).
    fn zoc_blocked(&self, from: HexCoord, to: HexCoord) -> bool {
        self.map.hexsides_on(from, to).iter().any(|h| {
            self.reference
                .hexsides
                .get(&h.resolve(from, to))
                .map(|p| p.blocks_zoc)
                .unwrap_or(false)
        })
    }

    /// The on-map hexes controlled by the units projecting ZOC from `from` (§10.21).
    pub fn zoc_hexes(&self, from: HexCoord) -> Vec<HexCoord> {
        if self.hex_projects_zoc(from).is_none() {
            return Vec::new();
        }
        from.neighbors()
            .into_iter()
            .filter(|&n| self.map.hex(n).is_some() && !self.zoc_blocked(from, n))
            .collect()
    }

    /// Whether `hex` lies in a Zone of Control projected by `side`.
    pub fn hex_in_zoc_of(&self, hex: HexCoord, side: Side) -> bool {
        hex.neighbors().into_iter().any(|adj| {
            self.hex_projects_zoc(adj) == Some(side) && self.zoc_hexes(adj).contains(&hex)
        })
    }
}
