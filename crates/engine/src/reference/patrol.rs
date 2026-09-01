//! Patrol & Reconnaissance tables (§16). Small fixed charts encoded directly in code (like the
//! §9.4 Stacking Point Values); the human-readable provenance is docs/source-materials/
//! patrol-tables-16.md. All three are read off PDF pages 96 & 98.

/// The Killed / Captured TOE Strength Points a die roll inflicts on one side, used by both the
/// §16.6 Patrol Survival Table (patrolling force) and the §16.8 Objective Loss Table (patrollee).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PatrolLoss {
    pub killed: u32,
    pub captured: u32,
}

/// §16.6 Patrol Survival Table: losses to the **patrolling** force. `all_recce` subtracts one from
/// the die (§16.33); the resulting 0 reads as the `0…3` row. `die` is 1..=6.
pub fn patrol_survival(die: u32, all_recce: bool) -> PatrolLoss {
    let d = if all_recce { die.saturating_sub(1) } else { die };
    match d {
        4 => PatrolLoss { killed: 0, captured: 1 },
        5 => PatrolLoss { killed: 1, captured: 0 },
        6 => PatrolLoss { killed: 1, captured: 1 },
        _ => PatrolLoss { killed: 0, captured: 0 }, // 0…3
    }
}

/// §16.8 Objective Loss Table: losses inflicted on the **patrollee**. `die` is 1..=6.
pub fn objective_loss(die: u32) -> PatrolLoss {
    match die {
        5 => PatrolLoss { killed: 0, captured: 1 },
        6 => PatrolLoss { killed: 1, captured: 0 },
        _ => PatrolLoss { killed: 0, captured: 0 }, // 1…4
    }
}

/// §16.7 Patrol Reconnaissance Table result: the number of battalion-size-equivalent enemy units
/// the defender must reveal, or `All`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconReveal {
    Count(u32),
    All,
}

/// §16.7 Patrol Reconnaissance Table: `die` (1..=6) × surviving `net_points` (capped at 3, §16.14)
/// → how many units the non-Phasing player must reveal (§16.5).
pub fn recon_reveal(die: u32, net_points: u32) -> ReconReveal {
    use ReconReveal::{All, Count};
    let col = net_points.clamp(1, 3);
    // Rows are the die (1..=6); columns the net Patrol Points (1/2/3).
    let table: [[ReconReveal; 3]; 6] = [
        [Count(0), Count(0), Count(1)], // die 1
        [Count(0), Count(1), Count(1)], // die 2
        [Count(1), Count(1), Count(2)], // die 3
        [Count(1), Count(2), Count(3)], // die 4
        [Count(2), Count(3), Count(4)], // die 5
        [Count(3), Count(4), All],      // die 6
    ];
    let row = die.clamp(1, 6) as usize - 1;
    table[row][col as usize - 1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patrol_survival_16_6() {
        assert_eq!(patrol_survival(3, false), PatrolLoss { killed: 0, captured: 0 });
        assert_eq!(patrol_survival(4, false), PatrolLoss { killed: 0, captured: 1 });
        assert_eq!(patrol_survival(5, false), PatrolLoss { killed: 1, captured: 0 });
        assert_eq!(patrol_survival(6, false), PatrolLoss { killed: 1, captured: 1 });
        // §16.33: all-Recce subtracts one — a 5 reads as the 4 row.
        assert_eq!(patrol_survival(5, true), PatrolLoss { killed: 0, captured: 1 });
        assert_eq!(patrol_survival(1, true), PatrolLoss { killed: 0, captured: 0 });
    }

    #[test]
    fn objective_loss_16_8() {
        assert_eq!(objective_loss(4), PatrolLoss { killed: 0, captured: 0 });
        assert_eq!(objective_loss(5), PatrolLoss { killed: 0, captured: 1 });
        assert_eq!(objective_loss(6), PatrolLoss { killed: 1, captured: 0 });
    }

    #[test]
    fn recon_reveal_16_7() {
        assert_eq!(recon_reveal(1, 1), ReconReveal::Count(0));
        assert_eq!(recon_reveal(4, 2), ReconReveal::Count(2));
        assert_eq!(recon_reveal(6, 1), ReconReveal::Count(3));
        assert_eq!(recon_reveal(6, 3), ReconReveal::All);
        // Net points cap at 3 (§16.14).
        assert_eq!(recon_reveal(6, 9), ReconReveal::All);
    }
}
