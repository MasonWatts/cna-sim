//! Ports and the Axis Naval Convoy system (§55–56) — pure rules/logic. The port-capacity,
//! shipping-lane distance, tonnage-determination and equivalent-weight *data* live in RON charts
//! (§55.3, §56.18, §56.21, §54.5); this module holds the arithmetic and timing rules that do not
//! depend on those charts.

use serde::{Deserialize, Serialize};

// ---- Ports (§55) ----

/// §55.14: a port operating below its maximum Efficiency Level handles a proportionately reduced
/// capacity — `base_capacity × current_level / max_level`, rounded **up** to the nearest ton/point.
/// (Benghazi 2500 t at level 1 of 3 → 834 t.) A port at or above max uses its full capacity.
pub fn effective_port_capacity(base_capacity: u32, current_level: u32, max_level: u32) -> u32 {
    if max_level == 0 {
        return 0;
    }
    let level = current_level.min(max_level);
    (base_capacity * level).div_ceil(max_level)
}

/// §55.18: a port regains one Efficiency Level for every Operations Stage in which it loses none to
/// enemy bombs (never above its assigned maximum).
pub const PORT_EFFICIENCY_REGEN_PER_STAGE: u32 = 1;

/// §55.25: the Italian cruiser San Giorgio starts the game blocking Tobruk's harbour, reducing its
/// Efficiency Level by three.
pub const SAN_GIORGIO_TOBRUK_BLOCK_LEVELS: u32 = 3;

/// §55.24: a port's Efficiency Level may be reduced by at most one level per Operations Stage.
pub const MAX_PORT_LEVEL_REDUCTION_PER_STAGE: u32 = 1;

/// §55.21: these ports may never have their harbours blocked (scuttling).
pub const UNBLOCKABLE_PORTS: [&str; 5] = ["Bizerta", "Tripoli", "Alexandria", "Aboukir", "Rosetta"];

/// §55.21: may this port's harbour be blocked by scuttling?
pub fn harbor_blockable(port: &str) -> bool {
    !UNBLOCKABLE_PORTS.contains(&port)
}

/// §55.22/§55.23: the Ammunition and Stores Points to reduce a port's Efficiency Level by one via
/// scuttling. Tobruk costs 50/25; every other (blockable) port costs 25/10.
pub fn harbor_block_cost(port: &str) -> (u32, u32) {
    if port == "Tobruk" {
        (50, 25)
    } else {
        (25, 10)
    }
}

/// §55.26: the Ammunition and Stores Points to remove one level of blocking (restore one Efficiency
/// Level) with engineers. Default 50/25; Tobruk 25/10; Benghazi 100/50.
pub fn harbor_unblock_cost(port: &str) -> (u32, u32) {
    match port {
        "Tobruk" => (25, 10),
        "Benghazi" => (100, 50),
        _ => (50, 25),
    }
}

/// §55.26: engineer units required to remove one level of blocking — two for Benghazi, one for any
/// other port.
pub fn harbor_unblock_engineers(port: &str) -> u32 {
    if port == "Benghazi" {
        2
    } else {
        1
    }
}

/// §55.27: a harbour mine laid by air is removed by an engineer on a die roll of 1–3 (4–6 leaves it).
pub fn harbor_mine_removed(die: u32) -> bool {
    (1..=3).contains(&die)
}

/// §55.17 / §55.3 chart: Bizerta is non-operable until Game-Turn 35 (June week I 1941 on the Turn
/// Record Track, `state::game::date_of`). The §55.3 chart's explicit "GT 35" and the §55.17 prose
/// "June 1 1941" agree under the authoritative calendar. From GT 35 the Axis rolls 2d6 each
/// Game-Turn and, on a 12, Bizerta becomes usable from then on.
pub const BIZERTA_AVAILABLE_FROM_GT: u32 = 35;

/// §55.17: does this Game-Turn's 2d6 roll (once eligible) bring Bizerta into use?
pub fn bizerta_opens(game_turn: u32, two_dice_total: u32) -> bool {
    game_turn >= BIZERTA_AVAILABLE_FROM_GT && two_dice_total == 12
}

// ---- Axis Naval Convoys (§56) ----

/// §56.11/§56.12: there are six shipping lanes; the Axis may run from one to six convoys (one per
/// lane) in a Game-Turn.
pub const NUM_SHIPPING_LANES: u8 = 6;
pub const MAX_CONVOYS_PER_TURN: u8 = 6;

/// §56.14: shipping lanes 4 and 5 are the routes originating from Greece.
pub fn is_greece_lane(lane: u8) -> bool {
    lane == 4 || lane == 5
}

/// §56.14: the Greece lanes (4 and 5) may not be used until the second Game-Turn of May 1941 —
/// GT 32 (May week II) on the Turn Record Track (`state::game::date_of`). (An earlier uniform-
/// calendar estimate of GT 34 was wrong: September 1940 begins at week III, so May 1941 spans
/// GT 31–34 and its second turn is GT 32.)
pub const GREECE_LANES_AVAILABLE_FROM_GT: u32 = 32;

/// Is shipping `lane` (1–6) available on `game_turn`? Only the Greece lanes are date-restricted
/// (§56.14).
pub fn lane_available(lane: u8, game_turn: u32) -> bool {
    if !(1..=NUM_SHIPPING_LANES).contains(&lane) {
        return false;
    }
    !is_greece_lane(lane) || game_turn >= GREECE_LANES_AVAILABLE_FROM_GT
}

/// §56.16: a convoy under Commonwealth air bombardment fires flak equal to its total tonnage
/// divided by 1000 (15 000 t → 15 Flak Points).
pub fn convoy_flak_points(total_tonnage: u32) -> u32 {
    total_tonnage / 1000
}

/// §42.52/§56.18: a plane may fly a mission to a shipping lane only if its range covers the distance
/// from its air facility to a rally point plus the distance from that rally point to the lane. For a
/// plane based in a box (Sicily/Italy/Crete/Malta) the `facility_to_rally` leg is zero.
pub fn convoy_lane_reachable(plane_range: u32, facility_to_rally: u32, rally_to_lane: u32) -> bool {
    plane_range >= facility_to_rally + rally_to_lane
}

/// §41.67: cargo lost from a convoy bombing result — `percent` of a cargo type's `tonnage`, with
/// fractions rounded up. Losses are applied evenly (the same percentage) across every cargo type.
pub fn convoy_cargo_loss(tonnage: u32, percent: u32) -> u32 {
    (tonnage * percent).div_ceil(100)
}

/// §42.53 convoy size class the Axis reveals on a successful naval reconnaissance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConvoySize {
    Small,
    Medium,
    Large,
}

/// §42.53: the size classes a convoy of `tonnage` may be reported as — Small (≤5,000), Medium
/// (3,000–10,000), Large (7,000+). The ranges overlap deliberately, so the Axis may choose among
/// the returned options.
pub fn convoy_size_options(tonnage: u32) -> Vec<ConvoySize> {
    let mut v = Vec::new();
    if tonnage <= 5000 {
        v.push(ConvoySize::Small);
    }
    if (3000..=10000).contains(&tonnage) {
        v.push(ConvoySize::Medium);
    }
    if tonnage >= 7000 {
        v.push(ConvoySize::Large);
    }
    v
}

/// §42.53: the size class the Axis actually reports for a convoy of `tonnage` when forced to reveal.
/// The choice is the Axis player's; absent an Axis decision, this is the deterministic adversarial
/// default — the Axis reports the most open-ended class among its legal options (Large > Medium >
/// Small), which conceals the true tonnage best. `None` only if `tonnage` matches no class (never,
/// since the classes cover all tonnages). This is what recon reveals, not the exact tonnage.
pub fn concealing_size(tonnage: u32) -> Option<ConvoySize> {
    let opts = convoy_size_options(tonnage);
    [ConvoySize::Large, ConvoySize::Medium, ConvoySize::Small]
        .into_iter()
        .find(|class| opts.contains(class))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_capacity_55_14() {
        // Benghazi: 2500 t, level 1 of 3 -> 834 (round up).
        assert_eq!(effective_port_capacity(2500, 1, 3), 834);
        assert_eq!(effective_port_capacity(2500, 3, 3), 2500); // full
        assert_eq!(effective_port_capacity(2500, 5, 3), 2500); // clamp at max
        assert_eq!(effective_port_capacity(2500, 0, 3), 0); // destroyed
    }

    #[test]
    fn harbor_blocking_55_2() {
        assert!(!harbor_blockable("Tripoli"));
        assert!(!harbor_blockable("Alexandria"));
        assert!(harbor_blockable("Tobruk"));
        assert!(harbor_blockable("Benghazi"));
        assert_eq!(harbor_block_cost("Tobruk"), (50, 25));
        assert_eq!(harbor_block_cost("Benghazi"), (25, 10));
        assert!(harbor_mine_removed(3));
        assert!(!harbor_mine_removed(4));
        assert_eq!(SAN_GIORGIO_TOBRUK_BLOCK_LEVELS, 3);
    }

    #[test]
    fn bizerta_timing_55_17() {
        assert!(!bizerta_opens(34, 12)); // before GT 35
        assert!(bizerta_opens(35, 12));
        assert!(!bizerta_opens(40, 11)); // needs a 12
    }

    #[test]
    fn convoy_lanes_56() {
        assert!(is_greece_lane(4));
        assert!(is_greece_lane(5));
        assert!(!is_greece_lane(1));
        // Non-Greece lanes always available; Greece lanes only from GT 32 (May II 1941, §56.14).
        assert!(lane_available(1, 1));
        assert!(!lane_available(4, 31));
        assert!(lane_available(4, 32));
        assert!(!lane_available(7, 100)); // invalid lane
        // §56.16 flak from tonnage.
        assert_eq!(convoy_flak_points(15000), 15);
        assert_eq!(convoy_flak_points(999), 0);
    }

    #[test]
    fn convoy_size_options_42_53() {
        // Non-overlapping cases.
        assert_eq!(convoy_size_options(2000), vec![ConvoySize::Small]);
        assert_eq!(convoy_size_options(12000), vec![ConvoySize::Large]);
        // Overlaps give the Axis a choice.
        assert_eq!(convoy_size_options(4000), vec![ConvoySize::Small, ConvoySize::Medium]);
        assert_eq!(convoy_size_options(8000), vec![ConvoySize::Medium, ConvoySize::Large]);
        assert_eq!(convoy_size_options(6000), vec![ConvoySize::Medium]);
    }

    #[test]
    fn concealing_size_picks_most_open_ended() {
        // No choice: report the only legal class.
        assert_eq!(concealing_size(2000), Some(ConvoySize::Small));
        assert_eq!(concealing_size(6000), Some(ConvoySize::Medium));
        assert_eq!(concealing_size(12000), Some(ConvoySize::Large));
        // With a choice, the Axis reports the more concealing (open-ended) class.
        assert_eq!(concealing_size(4000), Some(ConvoySize::Medium)); // vs Small
        assert_eq!(concealing_size(8000), Some(ConvoySize::Large)); // vs Medium
    }

    #[test]
    fn lane_reachability_42_52() {
        // From a box: only the rally->lane leg (facility_to_rally = 0).
        assert!(convoy_lane_reachable(45, 0, 45));
        assert!(!convoy_lane_reachable(44, 0, 45));
        // From N. Africa: facility->rally + rally->lane.
        assert!(convoy_lane_reachable(60, 22, 38));
        assert!(!convoy_lane_reachable(59, 22, 38));
    }
}
