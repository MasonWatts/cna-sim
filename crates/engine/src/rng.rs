//! Deterministic, serializable pseudo-random number generator (design.md §3.7).
//!
//! A tiny SplitMix64 — no external dependency, identical output on every platform,
//! and it serializes as a single `u64`. All game randomness flows through this so that
//! `(seed, commands)` fully reproduces a game (for replay, testing, and AI rollouts).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rng {
    state: u64,
    /// Test/replay-only queue of forced die results consumed before generated ones. Empty in normal
    /// play (and never serialized), so `(seed, commands)` still reproduces a game. It lets tests pin
    /// exact dice now that dice are rolled *inside* the command boundary (agents can't supply their
    /// own rolls — see design.md §3.7 / the anti-cheat rationale).
    #[serde(skip)]
    forced: std::collections::VecDeque<u8>,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed, forced: std::collections::VecDeque::new() }
    }

    /// Queue die results (1..=6) to be returned by the next `d6` calls, in order. For tests that need
    /// a specific outcome from an engine-rolled command.
    pub fn force<I: IntoIterator<Item = u8>>(&mut self, dice: I) {
        self.forced.extend(dice);
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Roll a single six-sided die (1..=6) — a forced value first, else generated.
    pub fn d6(&mut self) -> u8 {
        if let Some(v) = self.forced.pop_front() {
            return v;
        }
        (self.next_u64() % 6) as u8 + 1
    }

    /// Roll two dice read sequentially as CNA's CRTs require: the larger die is the tens
    /// digit and the smaller the ones (§12.42), giving a value in 11..=66 where the tens
    /// digit is always ≥ the ones digit. Returns `(larger, smaller)`.
    pub fn two_dice_sequential(&mut self) -> (u8, u8) {
        let a = self.d6();
        let b = self.d6();
        if a >= b {
            (a, b)
        } else {
            (b, a)
        }
    }

    /// The 11..=66 "sequential read" value of two dice (larger die × 10 + smaller).
    pub fn sequential_read(&mut self) -> u8 {
        let (hi, lo) = self.two_dice_sequential();
        hi * 10 + lo
    }

    /// Two dice read straight (not reordered): first die × 10 + second, 11..=66. Used by
    /// tables that cover all 36 outcomes (e.g. the Anti-Armor CRT, §14.6).
    pub fn two_dice_raw(&mut self) -> u8 {
        self.d6() * 10 + self.d6()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_for_a_seed() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.d6(), b.d6());
        }
    }

    #[test]
    fn d6_in_range() {
        let mut r = Rng::new(7);
        for _ in 0..1000 {
            let v = r.d6();
            assert!((1..=6).contains(&v));
        }
    }

    #[test]
    fn sequential_read_has_high_ge_low() {
        let mut r = Rng::new(123);
        for _ in 0..1000 {
            let (hi, lo) = r.two_dice_sequential();
            assert!((1..=6).contains(&hi) && (1..=6).contains(&lo));
            assert!(hi >= lo);
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        let sa: Vec<u8> = (0..20).map(|_| a.d6()).collect();
        let sb: Vec<u8> = (0..20).map(|_| b.d6()).collect();
        assert_ne!(sa, sb);
    }
}
