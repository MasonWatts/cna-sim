//! Initiative Ratings (§7.2) — DATA. The Commonwealth rating depends on the Game-Turn
//! (date bands); the Axis rating is situational (derived from the game state by code).
//! Human-readable companion: `docs/source-materials/initiative-ratings-chart.md`.

use serde::{Deserialize, Serialize};

/// A Commonwealth Initiative Rating for an inclusive Game-Turn band.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CommonwealthBand {
    pub from: u32,
    pub to: u32,
    pub rating: i32,
}

/// The Initiative Ratings Chart (§7.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitiativeRatings {
    /// Commonwealth rating by inclusive Game-Turn band.
    pub commonwealth_by_turn: Vec<CommonwealthBand>,
    /// Axis rating with the Rommel counter on the game-maps.
    pub axis_rommel: i32,
    /// Axis rating with German land combat units but no Rommel on the maps.
    pub axis_german: i32,
    /// Axis rating with no German combat units or Rommel on the maps.
    pub axis_minimal: i32,
}

impl InitiativeRatings {
    /// The Commonwealth Initiative Rating for a Game-Turn (0 if outside all bands).
    pub fn commonwealth(&self, game_turn: u32) -> i32 {
        self.commonwealth_by_turn
            .iter()
            .find(|b| game_turn >= b.from && game_turn <= b.to)
            .map(|b| b.rating)
            .unwrap_or(0)
    }
}
