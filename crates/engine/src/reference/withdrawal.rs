//! §20.8 Commonwealth Mandatory Withdrawal Schedule ([4.43a] chart, PDF p114–115). DATA types.
//!
//! The withdrawal *mechanics* live in `state/game.rs` (`enforce_mandatory_withdrawals`); this module
//! only models the transcribed schedule table so it can be loaded and inspected. Binding each entry
//! to the scenario Order-of-Battle units (resolving `[Less …]` exclusions and "any N …" selectors)
//! is a scenario-loader concern — see docs/deferred-items.md AU24.

use serde::{Deserialize, Serialize};

/// One row of the §20.8 withdrawal schedule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WithdrawalOrder {
    /// Game-Turn the withdrawal must be completed by (§20.83).
    pub game_turn: u32,
    /// Operations Stage within that Game-Turn.
    pub ops_stage: u32,
    /// (truck value points, motorization points) that must accompany the withdrawal (§20.8 "Tpt").
    pub tpt: (u32, u32),
    /// Verbatim withdrawal text: the formation(s), `[Less …]` exclusions and "any N …" selectors.
    pub units: String,
}

/// The whole Commonwealth Mandatory Withdrawal Schedule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct WithdrawalSchedule {
    pub withdrawals: Vec<WithdrawalOrder>,
}

impl WithdrawalSchedule {
    /// The orders due at exactly `(game_turn, ops_stage)`.
    pub fn due_at(&self, game_turn: u32, ops_stage: u32) -> impl Iterator<Item = &WithdrawalOrder> {
        self.withdrawals
            .iter()
            .filter(move |w| w.game_turn == game_turn && w.ops_stage == ops_stage)
    }
}
