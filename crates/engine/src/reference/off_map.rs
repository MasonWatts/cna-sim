//! Off-map Tripoli/Tunisia land-movement complex (§8.8, §8.89).
//!
//! A linear, Axis-only chain of region boxes on the western edge of Map A, with one on-map
//! junction hex (Nofilia, A2703). Movement is abstracted into Operations Stages whose count
//! depends on the unit's CPA tier (§8.89). See `data/reference/off-map-nodes.ron` and
//! `docs/source-materials/off-map-tripoli-tunisia.md`.

use serde::{Deserialize, Serialize};

/// Operations-Stages cost of one off-map leg, by CPA tier (§8.89).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageCost {
    /// CPA ≥ 25.
    pub cpa_high: u8,
    /// CPA 15–20 (chart tier; we treat 15–24 as mid — see the source-material note).
    pub cpa_mid: u8,
    /// CPA < 15.
    pub cpa_low: u8,
}

impl StageCost {
    /// Operations Stages to traverse this leg for a unit with the given CPA (§8.89).
    pub fn stages_for_cpa(&self, cpa: i32) -> u32 {
        if cpa >= 25 {
            self.cpa_high as u32
        } else if cpa >= 15 {
            self.cpa_mid as u32
        } else {
            self.cpa_low as u32
        }
    }
}

/// One region box in the off-map complex (§8.8): Tunis, Gabes, Tripoli, or Tripolitania.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OffMapNode {
    pub name: String,
    /// §41.85 constant, non-destructable AA/Flak capacity printed in the box — the flak fired at a
    /// Commonwealth raid bombing this box (`FlyBombingMissionAtBox`).
    #[serde(default)]
    pub aa_points: u8,
    #[serde(default)]
    pub port: Option<String>,
    /// A garrison marker in the box (e.g. Tripoli's "Axis" marker), if any.
    #[serde(default)]
    pub garrison: Option<String>,
    /// Stage cost to the node one step closer to the map (the previous entry in the chain;
    /// for the first node, to the map junction).
    pub leg: StageCost,
}

/// The Axis-only off-map land-movement complex (§8.8): a chain of region boxes with a single
/// on-map junction hex. `nodes` are ordered map-side → inland (index 0 nearest the map).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct OffMapNetwork {
    /// HexId of the on-map hex where the complex connects to the board (§8.85).
    pub map_junction: String,
    /// §8.85 road string-out: consecutive on-map road hex-ids, closest-to-Tripolitania first.
    /// Entering units fill these in order, overflowing to the next when a hex is full.
    #[serde(default)]
    pub entry_road: Vec<String>,
    pub nodes: Vec<OffMapNode>,
}

impl OffMapNetwork {
    pub fn node(&self, name: &str) -> Option<&OffMapNode> {
        self.nodes.iter().find(|n| n.name == name)
    }

    /// Chain index of a node (0 = closest to the map), if it exists.
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.name == name)
    }

    /// Operations Stages for a unit of `cpa` to travel from `from` to the on-map junction,
    /// summing the legs along the chain (§8.89 gives these as cumulative distances).
    pub fn stages_to_map(&self, from: &str, cpa: i32) -> Option<u32> {
        let i = self.index_of(from)?;
        Some(self.nodes[..=i].iter().map(|n| n.leg.stages_for_cpa(cpa)).sum())
    }
}
