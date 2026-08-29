//! Scenario pre-placement (§60–61): building an initial [`GameState`] from the setup charts.
//!
//! Only fully chart-backed placement lives here. Placements that depend on counter/scenario-card
//! data not present in the machine-readable rules (the exact warship roster and ratings, the San
//! Giorgio's combat ratings, the Malta base names/levels, and the initial SGSU/aircraft laydown)
//! are intentionally omitted until that source data is supplied, rather than fabricated.

use crate::state::game::GameState;
use crate::state::naval::Port;

impl GameState {
    /// §61.6: instantiate every port available at the current Game-Turn from the Port Capacity
    /// Chart (§55.3) at its full listed Efficiency Level, then apply the Campaign Game's start
    /// overrides. All ports begin at their full listed level except:
    /// - **Benghazi** begins at zero (§61.6 — held/damaged at campaign start), and
    /// - **Tobruk** begins three levels down (its `starts_blocked`), the San Giorgio still blocking
    ///   the harbour (§55.25); §55.12 fixes Tobruk's Efficiency Level at 5, so its start is 2.
    ///
    /// A port not yet available (Bizerta, §55.17 `available_from_gt`) is skipped until its
    /// Game-Turn. Returns the number of ports placed.
    pub fn setup_campaign_ports(&mut self) -> usize {
        let Some(chart) = self.reference.ports_chart.clone() else {
            return 0;
        };
        let mut placed = 0;
        for (name, spec) in chart.ports.iter() {
            if let Some(gt) = spec.available_from_gt {
                if self.game_turn < gt {
                    continue;
                }
            }
            // `from_spec` already sets efficiency = full listed level less any `starts_blocked`
            // (Tobruk's San Giorgio −3), matching §61.6's "full listed" for every other port.
            let mut port = Port::from_spec(name.clone(), spec);
            if name == "Benghazi" {
                port.efficiency = 0; // §61.6 Campaign start override.
            }
            self.ports.insert(name.clone(), port);
            placed += 1;
        }
        placed
    }
}

#[cfg(test)]
mod tests {
    use crate::reference::ReferenceData;
    use crate::state::game::GameState;

    #[test]
    fn campaign_ports_setup_61_6() {
        let mut refs = ReferenceData::new();
        refs.load_ports_chart(include_str!("../../../data/reference/port-capacity.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        let placed = g.setup_campaign_ports();
        assert!(placed >= 8);
        // §61.6: most ports at their full listed Efficiency Level.
        assert_eq!(g.ports["Alexandria"].efficiency, 10);
        assert_eq!(g.ports["Tripoli"].efficiency, 10);
        // §61.6 overrides: Benghazi at zero, Tobruk three down (5 − 3 = 2) with the wreck.
        assert_eq!(g.ports["Benghazi"].efficiency, 0);
        assert_eq!(g.ports["Benghazi"].max_efficiency, 3);
        assert_eq!(g.ports["Tobruk"].efficiency, 2);
        assert_eq!(g.ports["Tobruk"].wreck_block, 3);
        // §55.17: Bizerta is not yet available at Game-Turn 1.
        assert!(!g.ports.contains_key("Bizerta"));
    }
}
