//! Scenario-1 demo: loads "The Italian Offensive" order of battle onto the digitized
//! section-C map, runs the sequence of play, advances an Italian mobile group to contact,
//! and resolves the opening close assault — all through the engine's command boundary.
//!
//! Run: `cargo run --bin demo`. Deterministic (fixed seed).

use cna_engine::reference::units::{Nationality, Side};
use cna_engine::reference::ReferenceData;
use cna_engine::state::command::{Command, CommandOutcome};
use cna_engine::state::game::{GameState, Phase};
use cna_engine::state::map::HexCoord;
use cna_engine::state::unit::UnitId;
use cna_engine::rules::movement::MoveMode;

macro_rules! data {
    ($p:literal) => {
        include_str!(concat!("../../../../data/", $p))
    };
}

fn reference() -> ReferenceData {
    let mut r = ReferenceData::new();
    r.load_terrain(data!("reference/terrain.ron")).unwrap();
    r.load_unit_characteristics(Nationality::Commonwealth, data!("reference/unit-characteristics/commonwealth.ron")).unwrap();
    r.load_unit_characteristics(Nationality::Italian, data!("reference/unit-characteristics/italian.ron")).unwrap();
    r.load_weapons_systems(Nationality::Commonwealth, data!("reference/weapons/commonwealth.ron")).unwrap();
    r.load_weapons_systems(Nationality::Italian, data!("reference/weapons/italian.ron")).unwrap();
    r.load_anti_armor_crt(data!("reference/crt/anti-armor.ron")).unwrap();
    r.load_barrage_crt(data!("reference/crt/barrage.ron")).unwrap();
    r.load_close_assault_crt(data!("reference/crt/close-assault.ron")).unwrap();
    r.load_morale_modifier_table(data!("reference/crt/morale-modifier.ron")).unwrap();
    r.load_prisoners_table(data!("reference/crt/prisoners.ron")).unwrap();
    r.load_initiative_ratings(data!("reference/initiative-ratings.ron")).unwrap();
    r.load_order_of_battle(Nationality::Italian, data!("ob/italian.ron")).unwrap();
    r.load_order_of_battle(Nationality::Commonwealth, data!("ob/commonwealth.ron")).unwrap();
    r.load_off_map_nodes(data!("reference/off-map-nodes.ron")).unwrap();
    r
}

/// Render a HexCoord back to its CNA label (section + RRCC).
fn label(c: HexCoord) -> String {
    let section = cna_engine::state::map::section_char_of_col(c.col);
    format!("{}{:02}{:02}", section, c.row, c.col % 100)
}

fn name(g: &GameState, id: UnitId) -> String {
    g.unit(id).map(|u| u.name.clone()).unwrap_or_default()
}

fn find(g: &GameState, name: &str) -> UnitId {
    g.units.values().find(|u| u.name == name).map(|u| u.id).expect("unit exists")
}

/// Greedily step `unit` toward `target` (up to `max_steps`), through on-map passable
/// hexes, stopping when adjacent. Returns the number of steps taken.
fn advance_toward(g: &mut GameState, unit: UnitId, target: HexCoord, max_steps: u32) -> u32 {
    let mut steps = 0;
    for _ in 0..max_steps {
        let cur = g.unit(unit).unwrap().hex().unwrap();
        if cur.is_adjacent(target) {
            break;
        }
        // Pick the on-map neighbour that most reduces distance to the target.
        let best = g
            .map
            .neighbors(cur)
            .filter(|n| n.distance(target) < cur.distance(target))
            .min_by_key(|n| n.distance(target));
        let Some(to) = best else { break };
        match g.apply(Command::FormationMove { formation: unit, to, mode: MoveMode::NonMotorized }) {
            Ok(_) => {
                let terr = g.map.terrain(to).map(|t| format!("{t:?}")).unwrap_or_default();
                println!("     {} -> {} ({})", label(cur), label(to), terr);
                steps += 1;
            }
            Err(e) => {
                println!("     blocked at {}: {e:?}", label(cur));
                break;
            }
        }
    }
    steps
}

fn main() {
    let mut g = GameState::new(reference(), 20260709);
    g.map.load_section(data!("maps/section-a.ron")).expect("map a");
    g.map.load_section(data!("maps/section-b.ron")).expect("map b");
    g.map.load_section(data!("maps/section-c.ron")).expect("map c");
    g.map.load_section(data!("maps/section-d.ron")).expect("map d");
    g.map.load_section(data!("maps/section-e.ron")).expect("map e");
    g.load_scenario(data!("scenarios/italian-offensive.ron")).expect("scenario");
    g.auto_place_dump_choices();

    println!("=== The Campaign for North Africa — Scenario 1: The Italian Offensive ===");
    println!("Map loaded, sections A-E ({} hexes). Game-Turn {}.\n", g.map.len(), g.game_turn);

    // --- Order of battle (top-level counters, with attached sub-units indented) ---
    let mut ids: Vec<UnitId> = g.units.keys().copied().collect();
    ids.sort();
    for side in [Side::Axis, Side::Commonwealth] {
        println!("-- {side:?} forces --");
        for &id in &ids {
            let u = g.unit(id).unwrap();
            if u.nationality.side() != side || u.attached_parent.is_some() {
                continue;
            }
            let loc = u.hex().map(label).unwrap_or_default();
            let mut children: Vec<UnitId> =
                g.formation_members(id).into_iter().filter(|&m| m != id).collect();
            children.sort();
            if children.is_empty() {
                println!("   {:34} {}  (TOE {})", u.name, loc, u.total_toe());
            } else {
                let ftoe: u32 = g.formation_members(id).iter().map(|&m| g.unit(m).unwrap().total_toe()).sum();
                println!(
                    "   {:34} {}  (formation TOE {}, CPA {})",
                    u.name,
                    loc,
                    ftoe,
                    g.formation_cpa(id).unwrap_or(0)
                );
                for m in children {
                    let c = g.unit(m).unwrap();
                    let shell = if g.is_shell(m) { "  [shell]" } else { "" };
                    println!("        \\ {:28} (TOE {}){}", c.name, c.total_toe(), shell);
                }
            }
        }
        println!();
    }

    // --- Sequence of play: determine initiative, reach the Movement & Combat phase ---
    g.advance_phase(); // Initiative Determination
    println!("-- Sequence of Play --");
    println!(
        "   Initiative: {:?} (rated Axis {} vs Commonwealth {}) — Player A moves first",
        g.initiative,
        g.axis_initiative_rating(),
        g.commonwealth_initiative_rating()
    );
    while g.phase != Phase::MovementAndCombat {
        g.advance_phase();
    }
    println!("   Phase: Movement & Combat — Player A = {:?}\n", g.active_player);

    // --- Opening move: the Maletti Group advances on the Commonwealth screen ---
    let maletti = find(&g, "Maletti Group");
    let tank_bn = find(&g, "Maletti Tank Bn");
    let target_unit = find(&g, "2nd Rifle Brigade");
    let target = g.unit(target_unit).unwrap().hex().unwrap();
    println!(
        "-- Movement: {} advances toward {} ({}) at formation CPA {} (§6.15, slowest member) --",
        name(&g, maletti),
        name(&g, target_unit),
        label(target),
        g.formation_cpa(maletti).unwrap_or(0)
    );
    let steps = advance_toward(&mut g, maletti, target, 8);
    let cur = g.unit(maletti).unwrap().hex().unwrap();
    println!(
        "   HQ counter moved {steps} hex(es) to {}; attached Tank Bn rides along at {} (effective hex follows the parent)\n",
        label(cur),
        g.effective_hex(tank_bn).map(label).unwrap_or_default()
    );

    // --- Close assault, resolved as the whole formation (§9: aggregated sub-units) ---
    if cur.is_adjacent(target) {
        let attackers = g.formation_members(maletti);
        let defenders = g.formation_members(target_unit);
        let atk_before: u32 = attackers.iter().map(|&m| g.unit(m).unwrap().total_toe()).sum();
        let def_before: u32 = defenders.iter().map(|&m| g.unit(m).unwrap().total_toe()).sum();
        println!(
            "-- Close Assault (§15): {} ({} sub-units, TOE {}) assaults {} ({} sub-units, TOE {}) --",
            name(&g, maletti),
            attackers.len(),
            atk_before,
            name(&g, target_unit),
            defenders.len(),
            def_before
        );
        match g.apply(Command::CloseAssault { attackers: attackers.clone(), target }) {
            Ok(CommandOutcome::CloseAssault(ca)) => {
                println!(
                    "   combined-arms reduction {} (§15.4 — 0 means the tanks are supported by infantry)",
                    ca.attacker_combined_arms
                );
                println!(
                    "   attacker strength {} vs defender {}; differential {}  (column {} -> {})",
                    ca.attacker_strength, ca.defender_strength, ca.differential, ca.base_column, ca.adjusted_column
                );
                println!(
                    "   attacker {}% loss; defender {}% loss{}",
                    ca.attacker.loss_percent,
                    ca.defender.loss_percent,
                    if ca.defender.retreat_hexes > 0 {
                        format!(", defender retreats {}", ca.defender.retreat_hexes)
                    } else {
                        String::new()
                    }
                );
                let atk_after: u32 = attackers.iter().map(|&m| g.unit(m).unwrap().total_toe()).sum();
                let def_after: u32 = defenders.iter().map(|&m| g.unit(m).unwrap().total_toe()).sum();
                println!("   {} formation TOE: {} -> {} ({} lost across sub-units)", name(&g, maletti), atk_before, atk_after, ca.attacker_toe_removed);
                println!("   {} formation TOE: {} -> {} ({} lost across sub-units)", name(&g, target_unit), def_before, def_after, ca.defender_toe_removed);
            }
            other => println!("   unexpected: {other:?}"),
        }
    } else {
        println!("-- {} halted short of contact (no-man's-land) --", name(&g, maletti));
    }

    println!("\n=== end of demo ===");
}
