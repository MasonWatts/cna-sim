//! Weather Table (§29.61) and Foul Weather Location Table (§29.7). See
//! `data/reference/weather-table.ron` and `docs/source-materials/weather-tables.md`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::state::game::{Season, Weather};

/// The §29.61 Weather Table + §29.7 Foul Weather Location Table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct WeatherTable {
    /// Per season, ordered `(max_roll, weather)` over the two-dice sequential value
    /// (larger×10+smaller, 11..=66). The result is the first entry with `max_roll >= roll`.
    pub seasons: HashMap<Season, Vec<(u8, Weather)>>,
    /// §29.7: one die (1..6, indexed 0..5) → the map-section letters the storm hits.
    pub foul_location: Vec<Vec<String>>,
}

impl WeatherTable {
    /// The weather type for a season and a two-dice sequential roll (§29.61).
    pub fn weather_for(&self, season: Season, roll: u8) -> Weather {
        self.seasons
            .get(&season)
            .and_then(|rows| rows.iter().find(|(max, _)| roll <= *max))
            .map(|(_, w)| *w)
            .unwrap_or(Weather::Normal)
    }

    /// The map sections a foul-weather die (1..6) hits (§29.7); empty if out of range.
    pub fn foul_sections(&self, die: u8) -> &[String] {
        self.foul_location
            .get(die.wrapping_sub(1) as usize)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}
