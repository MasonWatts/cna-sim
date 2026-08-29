//! Campaign Victory Conditions (§64.7). See `data/reference/campaign-victory.ron` and
//! `docs/source-materials/campaign-victory.md`.

use serde::{Deserialize, Serialize};

/// A §64.73 Geographical Occupation Point city: occupying it awards `axis`/`cw` VP.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VictoryCity {
    /// Hex-id of the city.
    pub hex: String,
    pub name: String,
    /// Victory Points to the Axis for holding it.
    pub axis: u32,
    /// Victory Points to the Commonwealth for holding it.
    pub cw: u32,
}

/// A §64.76 victory-level band: the level awarded when the point ratio (most:least, ×100) is
/// at most `max_ratio_pct`. A ratio above the highest band is a Smashing Victory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VictoryBand {
    pub max_ratio_pct: u32,
    pub level: String,
}

/// The §64.7 campaign victory tables.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CampaignVictory {
    pub cities: Vec<VictoryCity>,
    pub levels: Vec<VictoryBand>,
    /// Hexes that count as "Tobruk or Tripoli" strategic supply sources (§64.71).
    #[serde(default)]
    pub sources: Vec<String>,
    /// Hexes the Axis must hold for the §64.71 auto-win (all of Alexandria + Cairo).
    #[serde(default)]
    pub axis_objective: Vec<String>,
    /// §64.71 Axis supply-line budget in truck MP (90).
    #[serde(default)]
    pub axis_supply_mp: u32,
    /// §64.72 Commonwealth-check supply-line budget in truck MP (60).
    #[serde(default)]
    pub cw_check_mp: u32,
    /// §64.72 first Game-Turn on which the Commonwealth auto-win check applies (35).
    #[serde(default)]
    pub cw_check_from_turn: u32,
}

impl CampaignVictory {
    /// The victory-level name for a point ratio expressed as `most*100/least` (§64.76);
    /// above the highest band → "Smashing Victory".
    pub fn level_for(&self, ratio_pct: u32) -> String {
        self.levels
            .iter()
            .find(|b| ratio_pct <= b.max_ratio_pct)
            .map(|b| b.level.clone())
            .unwrap_or_else(|| "Smashing Victory".to_string())
    }
}
