//! Core rules engine for the Campaign for North Africa (CNA) simulation.
//!
//! Architecture (see `docs/design.md`):
//! - `reference` — Tier 1: immutable rulebook DATA (parameters), loaded at startup.
//! - `rules`     — the RULES/logic (code) that consume reference data and game state.
//! - `state`     — Tier 2: mutable game state (map, units, combat, turn driver) and the
//!   phase-gated `Command`/`CommandOutcome`/`CommandError` boundary.
//! - `scenario`  — loads an order of battle + turn state into a `GameState`.

pub mod agent;
pub mod reference;
pub mod rng;
pub mod rules;
pub mod scenario;
pub mod state;

#[cfg(test)]
mod tests {
    use crate::reference::terrain::{CombatMod, Fixed, HexTerrain, HexsideTerrain, MoveCost};
    use crate::reference::units::{Nationality, WeaponKind};
    use crate::reference::ReferenceData;
    use crate::rules::movement::{enter_cost, vehicle_may_cross_escarpment, MoveMode};

    // Load the real terrain reference data at compile time (path-independent test).
    const TERRAIN_RON: &str = include_str!("../../../data/reference/terrain.ron");

    fn refs() -> ReferenceData {
        let mut r = ReferenceData::new();
        r.load_terrain(TERRAIN_RON).expect("valid terrain RON");
        r
    }

    #[test]
    fn clear_costs_two_cp() {
        assert_eq!(
            enter_cost(&refs(), HexTerrain::Clear, MoveMode::NonMotorized),
            Some(MoveCost::Cost(Fixed(20)))
        );
    }

    #[test]
    fn road_motorized_is_half_a_cp() {
        assert_eq!(
            refs().hexsides[&HexsideTerrain::Road].cross.mot,
            MoveCost::Cost(Fixed(5))
        );
    }

    #[test]
    fn up_escarpment_prohibits_motorized() {
        assert_eq!(
            refs().hexsides[&HexsideTerrain::UpEscarpment].cross.mot,
            MoveCost::Prohibited
        );
    }

    #[test]
    fn rough_favors_defender_two_columns_in_close_assault() {
        assert_eq!(
            refs().terrain[&HexTerrain::Rough].close_assault,
            CombatMod::Shift(-2)
        );
    }

    #[test]
    fn escarpment_rule_is_code_not_data() {
        assert!(vehicle_may_cross_escarpment(false));
        assert!(!vehicle_may_cross_escarpment(true));
    }

    #[test]
    fn mountain_and_major_city_confirmed_values() {
        let r = refs();
        assert_eq!(r.terrain[&HexTerrain::Mountain].stacking_limit, Some(3));
        assert_eq!(r.terrain[&HexTerrain::MajorCity].enter.mot, MoveCost::Cost(Fixed(5)));
        assert_eq!(r.terrain[&HexTerrain::MajorCity].breakdown, Fixed(5));
        assert_eq!(r.terrain[&HexTerrain::MajorCity].stacking_limit, Some(8));
    }

    #[test]
    fn commonwealth_unit_characteristics_load() {
        const CW_UNITS_RON: &str =
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron");
        let mut r = ReferenceData::new();
        r.load_unit_characteristics(Nationality::Commonwealth, CW_UNITS_RON)
            .expect("valid CW unit-characteristics RON");

        let cw = &r.unit_characteristics[&Nationality::Commonwealth];
        assert_eq!(cw.len(), 44);
        // 'n' Infantry Bn-Equivalent (added from §4.46a): motorized, CA 1/2, Max TOE 6.
        assert_eq!(cw["n"].close_assault.unwrap().off.value, 1);
        assert_eq!(cw["n"].close_assault.unwrap().def.value, 2);
        assert_eq!(cw["n"].max_toe.unwrap().value, 6);
        // Infantry 'h': Close Assault 2/2, Max TOE 5.
        assert_eq!(cw["h"].close_assault.unwrap().off.value, 2);
        assert_eq!(cw["h"].max_toe.unwrap().value, 5);
        // 'l' is motorized-capable ('+').
        assert!(cw["l"].cpa.motorized_capable);
        // Tank 'g': US-tank special ('**'), may assign 10 tank TOE points.
        assert!(cw["g"].max_toe.unwrap().us_tank_special);
        assert_eq!(cw["g"].assignments[0].max_count, 10);
        // Anti-tank 'z': two assignment kinds (8 AT + 3 artillery = 11).
        assert_eq!(cw["z"].assignments.len(), 2);
        assert_eq!(cw["z"].max_toe.unwrap().value, 11);
        // Recce 'tt': anti-armor 2, armor protection 3.
        assert_eq!(cw["tt"].anti_armor.unwrap().value, 2);
        assert_eq!(cw["tt"].armor_protection.unwrap().value, 3);
        // Construction 'uu': parenthesized defensive Close Assault and Max TOE.
        assert!(cw["uu"].close_assault.unwrap().def.parenthesized);
        assert!(cw["uu"].max_toe.unwrap().parenthesized);
    }

    #[test]
    fn commonwealth_weapons_systems_load() {
        const CW_WEAPONS_RON: &str =
            include_str!("../../../data/reference/weapons/commonwealth.ron");
        let mut r = ReferenceData::new();
        r.load_weapons_systems(Nationality::Commonwealth, CW_WEAPONS_RON)
            .expect("valid CW weapons RON");

        let ws = &r.weapons_systems[&Nationality::Commonwealth];
        assert_eq!(ws.len(), 32);
        // Churchill II: heavy armor 7, Close Assault 5/6.
        assert_eq!(ws["Churchill II"].armor_protection.unwrap().value, 7);
        assert_eq!(ws["Churchill II"].close_assault.unwrap().def.value, 6);
        // 25-pounder: Barrage 8, Anti-Armor 5.
        assert_eq!(ws["25-pounder Gun"].barrage.unwrap().value, 8);
        assert_eq!(ws["25-pounder Gun"].anti_armor.unwrap().value, 5);
        // SP 6-pounder BAR "1L" -> -1.
        assert_eq!(ws["SP 6-pounder"].breakdown_adjustment, Some(-1));
        // Light AA anti-armor is a parenthesized (1).
        assert!(ws["Light AA (Bofors 40mm)"].anti_armor.unwrap().parenthesized);
        // Scorpion (mineclearing) has no anti-armor rating.
        assert!(ws["Scorpion"].anti_armor.is_none());
        // Weapon kinds.
        assert_eq!(ws["Churchill II"].kind, WeaponKind::Tank);
        assert_eq!(ws["25-pounder Gun"].kind, WeaponKind::Artillery);
        assert_eq!(ws["6-pounder"].kind, WeaponKind::AntiTank);
        assert_eq!(ws["Light AA (Bofors 40mm)"].kind, WeaponKind::LightAntiAir);
        assert_eq!(ws["Heavy AA (3.7\")"].kind, WeaponKind::HeavyAntiAir);
    }

    #[test]
    fn italian_unit_characteristics_load() {
        const IT_UNITS_RON: &str =
            include_str!("../../../data/reference/unit-characteristics/italian.ron");
        let mut r = ReferenceData::new();
        r.load_unit_characteristics(Nationality::Italian, IT_UNITS_RON)
            .expect("valid Italian unit-characteristics RON");

        let it = &r.unit_characteristics[&Nationality::Italian];
        assert_eq!(it.len(), 50);
        // Infantry 'p' carries direct combat ratings.
        assert_eq!(it["p"].anti_armor.unwrap().value, 5);
        assert_eq!(it["p"].barrage.unwrap().value, 2);
        // 'c' regimental HQ: parenthesized close-assault rating.
        assert!(it["c"].close_assault.unwrap().off.parenthesized);
        // 'a' HQ may assign a reconnaissance TOE point.
        assert_eq!(it["a"].assignments[0].weapon_kind, WeaponKind::Reconnaissance);
        // 'pp' emplaced coastal guns: CPA 0.
        assert_eq!(it["pp"].cpa.value, 0);
        // 'kk' has three assignment kinds (artillery + Heavy/Light AA).
        assert_eq!(it["kk"].assignments.len(), 3);
    }

    #[test]
    fn italian_weapons_systems_load() {
        const IT_WEAPONS_RON: &str =
            include_str!("../../../data/reference/weapons/italian.ron");
        let mut r = ReferenceData::new();
        r.load_weapons_systems(Nationality::Italian, IT_WEAPONS_RON)
            .expect("valid Italian weapons RON");

        let ws = &r.weapons_systems[&Nationality::Italian];
        assert_eq!(ws.len(), 21);
        // 90/53 Heavy AA: strong (non-parenthesized) anti-armor 9.
        assert_eq!(ws["Heavy AA (90/53)"].anti_armor.unwrap().value, 9);
        // Semovente SP 75/18: has armor protection and a BAR.
        assert_eq!(ws["75/18 Gun (Semovente)"].armor_protection.unwrap().value, 3);
        assert_eq!(ws["75/18 Gun (Semovente)"].breakdown_adjustment, Some(1));
        // CV 33 BAR "2R" -> 2.
        assert_eq!(ws["CV 33 (L3/35)"].breakdown_adjustment, Some(2));
        // Heavy 75/46 anti-armor is a parenthesized (5).
        assert!(ws["Heavy AA (75/46 Mod. 34)"].anti_armor.unwrap().parenthesized);
    }

    #[test]
    fn german_unit_characteristics_load() {
        const DE_UNITS_RON: &str =
            include_str!("../../../data/reference/unit-characteristics/german.ron");
        let mut r = ReferenceData::new();
        r.load_unit_characteristics(Nationality::German, DE_UNITS_RON)
            .expect("valid German unit-characteristics RON");

        let de = &r.unit_characteristics[&Nationality::German];
        assert_eq!(de.len(), 31);
        // §4.46c (PDF p171, verified 2026-08-30): Tank Bn-Eq 'f' may assign up to 15 tank TOE.
        assert_eq!(de["f"].unit_type, "Tank Bn-Equivalent");
        assert_eq!(de["f"].assignments[0].max_count, 15);
        // 'g' is the strong (CPA 25) Infantry Bn-Eq, close assault 3/2 — NOT a tank code.
        assert_eq!(de["g"].unit_type, "Infantry Bn-Equivalent");
        assert_eq!((de["g"].cpa.value, de["g"].close_assault.unwrap().off.value), (25, 3));
        // The halftrack infantry code is 'i' (the chart has no 'j'): motorized, armor protection 1.
        assert!(de["i"].cpa.motorized_capable);
        assert_eq!(de["i"].armor_protection.unwrap().value, 1);
        assert!(!de.contains_key("j"), "the §4.46c chart has no 'j' code");
        // 'h' carries the paradropable-artillery assignment.
        assert_eq!(de["h"].assignments[0].weapon_kind, crate::reference::units::WeaponKind::Artillery);
        // 'z' light-AA bn has two assignment kinds.
        assert_eq!(de["z"].assignments.len(), 2);
        // Recce 'ee' close assault 3/3, anti-armor 1, armor protection 2.
        assert_eq!(de["ee"].close_assault.unwrap().off.value, 3);
        assert_eq!(de["ee"].armor_protection.unwrap().value, 2);
    }

    #[test]
    fn german_weapons_systems_load() {
        const DE_WEAPONS_RON: &str =
            include_str!("../../../data/reference/weapons/german.ron");
        let mut r = ReferenceData::new();
        r.load_weapons_systems(Nationality::German, DE_WEAPONS_RON)
            .expect("valid German weapons RON");

        let ws = &r.weapons_systems[&Nationality::German];
        assert_eq!(ws.len(), 28);
        // Pz IV F2: anti-armor 8, armor 4.
        assert_eq!(ws["Pz IV F2 (Special)"].anti_armor.unwrap().value, 8);
        // 88mm Flak: anti-armor 13.
        assert_eq!(ws["Heavy AA (88mm Flak)"].anti_armor.unwrap().value, 13);
        // Captured 7.62cm Pak(R): anti-armor 9, CPA is "-" (None).
        assert_eq!(ws["7.62cm Pak(R)"].anti_armor.unwrap().value, 9);
        assert!(ws["7.62cm Pak(R)"].cpa.is_none());
        // Pz III E carries the only non-zero tank BAR (3R).
        assert_eq!(ws["Pz III E"].breakdown_adjustment, Some(3));
    }

    #[test]
    fn hex_entry_cost_reads_terrain() {
        use crate::rules::movement::{hex_entry_cost, MoveMode};
        use crate::state::map::{HexCoord, Map};
        let r = refs(); // terrain loaded
        let mut m = Map::new();
        m.set_hex(HexCoord::new(0, 0), HexTerrain::Clear);
        m.set_hex(HexCoord::new(1, 0), HexTerrain::Sea);
        // Clear: non-motorized entry = 2.0 CP.
        assert_eq!(
            hex_entry_cost(&r, &m, HexCoord::new(0, 0), MoveMode::NonMotorized),
            Some(MoveCost::Cost(Fixed(20)))
        );
        // Sea: motorized entry prohibited (per terrain data).
        assert_eq!(
            hex_entry_cost(&r, &m, HexCoord::new(1, 0), MoveMode::Motorized),
            Some(MoveCost::Prohibited)
        );
        // Off-map hex yields None.
        assert_eq!(
            hex_entry_cost(&r, &m, HexCoord::new(9, 9), MoveMode::NonMotorized),
            None
        );
    }

    fn cw_refs() -> ReferenceData {
        let mut r = ReferenceData::new();
        r.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .expect("cw unit chars");
        r.load_weapons_systems(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/weapons/commonwealth.ron"),
        )
        .expect("cw weapons");
        r
    }

    #[test]
    fn organic_infantry_close_assault_strength() {
        use crate::state::unit::{Composition, RatingKind, Unit, UnitId};
        let refs = cw_refs();
        // Infantry ID 'h': Close Assault 2/2. TOE 5 -> raw 10 -> actual 1.
        let u = Unit::new(
            UnitId(1),
            Nationality::Commonwealth,
            "h",
            "test infantry bn",
            Composition::Organic { current_toe: 5 },
        );
        assert_eq!(u.total_toe(), 5);
        assert_eq!(u.actual_strength(&refs, RatingKind::CloseAssaultOff), 1);
        assert_eq!(u.actual_strength(&refs, RatingKind::CloseAssaultDef), 1);
        // Infantry has no Barrage rating -> 0.
        assert_eq!(u.actual_strength(&refs, RatingKind::Barrage), 0);
    }

    #[test]
    fn weapons_unit_mixed_and_rounding() {
        use crate::state::unit::{Composition, RatingKind, Unit, UnitId, WeaponGroup};
        let refs = cw_refs();
        // Tank bn: 8× Crusader Mk.II (anti-armor 3) -> raw 24 -> actual 2 (round).
        let tanks = Unit::new(
            UnitId(2),
            Nationality::Commonwealth,
            "g",
            "test tank bn",
            Composition::Weapons {
                groups: vec![WeaponGroup { system: "Crusader Mk.II".into(), count: 8 }],
            },
        );
        assert_eq!(tanks.total_toe(), 8);
        assert_eq!(tanks.actual_strength(&refs, RatingKind::AntiArmor), 2);
        // Artillery: 6× 25-pdr (barrage 8) + 2× 18-pdr (barrage 7) -> raw 62 -> actual 6.
        let arty = Unit::new(
            UnitId(3),
            Nationality::Commonwealth,
            "x",
            "test artillery",
            Composition::Weapons {
                groups: vec![
                    WeaponGroup { system: "25-pounder Gun".into(), count: 6 },
                    WeaponGroup { system: "18-pounder Gun".into(), count: 2 },
                ],
            },
        );
        assert_eq!(arty.total_toe(), 8);
        assert_eq!(arty.actual_strength(&refs, RatingKind::Barrage), 6);
    }

    #[test]
    fn anti_air_is_not_divided_by_ten() {
        use crate::state::unit::{Composition, RatingKind, Unit, UnitId, WeaponGroup};
        let refs = cw_refs();
        // Heavy AA (3.7"): AA rating 4, ×3 points -> AA actual 12 (no ÷10, §11.37).
        let aa = Unit::new(
            UnitId(4),
            Nationality::Commonwealth,
            "gg",
            "test heavy AA",
            Composition::Weapons {
                groups: vec![WeaponGroup { system: "Heavy AA (3.7\")".into(), count: 3 }],
            },
        );
        assert_eq!(aa.actual_strength(&refs, RatingKind::AntiAir), 12);
    }

    #[test]
    fn turn_record_track_calendar() {
        use crate::state::game::{date_of, GameDate};
        // Campaign opens Sep III 1940; Sep 1940 has only weeks III-IV.
        assert_eq!(date_of(1), GameDate { year: 1940, month: 9, week: 3 });
        assert_eq!(date_of(2), GameDate { year: 1940, month: 9, week: 4 });
        assert_eq!(date_of(3), GameDate { year: 1940, month: 10, week: 1 });
        // Anchors resolving the prior discrepancies.
        assert_eq!(date_of(32), GameDate { year: 1941, month: 5, week: 2 }); // Greece lanes: 2nd GT of May 41
        assert_eq!(date_of(35), GameDate { year: 1941, month: 6, week: 1 }); // Bizerta opens (Jun I 41)
        assert_eq!(date_of(46), GameDate { year: 1941, month: 8, week: 4 }); // last CW high-evap turn
        assert_eq!(date_of(110), GameDate { year: 1942, month: 12, week: 4 }); // end of track
    }

    #[test]
    fn convoy_planning_and_cancellation_56() {        use crate::state::game::GameState;
        let refs = ReferenceData::new();
        let mut g = GameState::new(refs, 1);
        // Plan a Tripoli-bound convoy on lane 2 within a 21,000 t budget.
        assert!(g.plan_convoy(2, "Tripoli", 5, 10000, 8000, 1000, 0, 21000));
        assert_eq!(g.convoys.len(), 1);
        assert_eq!(g.convoys[0].total_tonnage(), 19000);
        assert_eq!(g.convoys[0].flak_points(), 19); // §56.16
        // Over budget is rejected.
        assert!(!g.plan_convoy(1, "Bizerta", 5, 20000, 5000, 0, 0, 21000));
        // A second convoy on the same lane/turn is rejected (§56.12).
        assert!(!g.plan_convoy(2, "Tripoli", 5, 1000, 0, 0, 0, 21000));
        // Greece lane 4 before GT 32 is rejected (§56.14 = May II 1941).
        assert!(!g.plan_convoy(4, "Benghazi", 31, 1000, 0, 0, 0, 21000));
        assert!(g.plan_convoy(4, "Benghazi", 32, 1000, 0, 0, 0, 21000));
        // §56.15: capturing Tripoli cancels its convoy.
        assert_eq!(g.cancel_convoys_to("Tripoli"), 1);
        assert!(g.convoys.iter().find(|c| c.destination == "Tripoli").unwrap().cancelled);
    }

    #[test]
    fn convoy_bombing_resolution_41_6() {
        use crate::state::game::GameState;
        let mut refs = ReferenceData::new();
        refs.load_air_bombardment(include_str!("../../../data/reference/air-bombardment.ron"))
            .unwrap();
        let mut g = GameState::new(refs, 1);
        g.plan_convoy(2, "Tripoli", 5, 10000, 8000, 1000, 1000, 25000);
        // 250 bomb points -> column 6 (201-260); roll 66 on the Axis-Naval-Convoy rows -> 25%.
        let pct = g.resolve_convoy_bombing(0, 250, 0, 10, 66).unwrap();
        assert_eq!(pct, 25);
        // 25% lost, rounded up per cargo type (§41.67).
        let c = &g.convoys[0];
        assert_eq!(c.fuel_tons, 7500); // 10000 - 2500
        assert_eq!(c.ammo_tons, 6000); // 8000 - 2000
        assert_eq!(c.stores_tons, 750); // 1000 - 250
        assert_eq!(c.replacement_tons, 750);
    }

    #[test]
    fn convoy_delivery_56_16() {
        use crate::state::game::GameState;
        let mut refs = ReferenceData::new();
        refs.load_equivalent_weights(include_str!(
            "../../../data/reference/equivalent-weights.ron"
        ))
        .unwrap();
        let mut g = GameState::new(refs, 1);
        g.plan_convoy(2, "Tripoli", 5, 8000, 4000, 500, 0, 25000);
        // Surviving tonnage -> supply points via §54.5: Fuel x8, Ammo /4, Stores x1.
        let delivered = g.deliver_convoy(0).unwrap();
        assert_eq!(delivered.fuel, 64000); // 8000 * 8
        assert_eq!(delivered.ammo, 1000); // 4000 / 4
        assert_eq!(delivered.stores, 500);
        assert!(g.convoys.is_empty()); // removed after delivery
    }

    #[test]
    fn allowable_convoy_tonnage_56_21() {
        use crate::state::game::GameState;
        let mut refs = ReferenceData::new();
        refs.load_convoy_levels(include_str!("../../../data/reference/convoy-levels.ron"))
            .unwrap();
        let g = GameState::new(refs, 1);
        // §56.21 example: planning for GT 55 (Nov I 1941) = level E; die 4 -> 21,000 t.
        assert_eq!(g.allowable_convoy_tonnage(55, 4), Some(21000));
        // GT 15 = Jan I 1941 = level B; die 1 -> 7000 + 1500 = 8500 -> round up 9000.
        assert_eq!(g.allowable_convoy_tonnage(15, 1), Some(9000));
        // GT 1 = Sep III 1940 = level B; die 3 -> 7000 + 4500 = 11500 -> round up 12000.
        assert_eq!(g.allowable_convoy_tonnage(1, 3), Some(12000));
    }

    #[test]
    fn harbor_blocking_55_2() {
        use crate::reference::ports::PortsChart;
        use crate::state::game::GameState;
        use crate::state::naval::Port;
        let refs = ReferenceData::new();
        let mut g = GameState::new(refs, 1);
        let chart: PortsChart =
            ron::from_str(include_str!("../../../data/reference/port-capacity.ron")).unwrap();
        g.ports.insert("Benghazi".into(), Port::from_spec("Benghazi", &chart.spec("Benghazi")));
        g.ports.insert("Tripoli".into(), Port::from_spec("Tripoli", &chart.spec("Tripoli")));

        // §55.21: Tripoli may not be blocked.
        assert_eq!(g.block_harbor("Tripoli", true), None);
        // §55.22: Benghazi blockable with an engineer; cost 25/10, efficiency 3 -> 2.
        assert_eq!(g.block_harbor("Benghazi", true), Some((25, 10)));
        assert_eq!(g.ports["Benghazi"].efficiency, 2);
        // No engineer -> no block.
        assert_eq!(g.block_harbor("Benghazi", false), None);
        // §55.26: Benghazi needs TWO engineers to unblock; one is insufficient.
        assert_eq!(g.unblock_harbor("Benghazi", 1), None);
        assert_eq!(g.unblock_harbor("Benghazi", 2), Some((100, 50)));
        assert_eq!(g.ports["Benghazi"].efficiency, 3);

        // BlockHarbor/UnblockHarbor are engineer actions gated to the Organization Phase.
        use crate::state::command::{Command, CommandError};
        use crate::state::game::Phase;
        assert_eq!(
            g.apply(Command::BlockHarbor { port: "Benghazi".into(), has_engineer: true }),
            Err(CommandError::WrongPhase)
        );
        g.phase = Phase::Organization;
        assert!(g.apply(Command::BlockHarbor { port: "Benghazi".into(), has_engineer: true }).is_ok());
        assert!(g.apply(Command::UnblockHarbor { port: "Benghazi".into(), engineers: 2 }).is_ok());
    }

    #[test]
    fn air_facility_placement_and_damage() {        use crate::state::air::{AirFacility, AirFacilityKind};
        use crate::state::game::GameState;
        use crate::state::map::HexCoord;
        let refs = ReferenceData::new();
        let mut g = GameState::new(refs, 1);
        let h = HexCoord::new(5, 5);

        // A coastal hex may hold both an airfield and a flying-boat basin (§36).
        g.add_air_facility(AirFacility::new(AirFacilityKind::Airfield, h));
        g.add_air_facility(AirFacility::new(AirFacilityKind::FlyingBoatBasin, h));
        g.add_air_facility(AirFacility::new(AirFacilityKind::AirLandingStrip, HexCoord::new(6, 5)));
        assert_eq!(g.facilities_at(h).count(), 2);

        // Bombing the airfield to zero leaves it in play (rebuildable, §36.14).
        assert!(g.damage_air_facility(h, AirFacilityKind::Airfield, 6));
        assert_eq!(g.facilities_at(h).count(), 2);
        assert!(g
            .facilities_at(h)
            .find(|f| f.kind == AirFacilityKind::Airfield)
            .unwrap()
            .is_destroyed());

        // Bombing the landing strip to zero removes it from the map (§36.2).
        let strip = HexCoord::new(6, 5);
        assert!(g.damage_air_facility(strip, AirFacilityKind::AirLandingStrip, 1));
        assert_eq!(g.facilities_at(strip).count(), 0);
    }

    #[test]
    fn move_step_command() {
        use crate::reference::terrain::{Fixed, HexTerrain};
        use crate::rules::movement::MoveMode;
        use crate::state::command::{Command, CommandError};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::Composition;

        // Reference with terrain loaded.
        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        let mut g = GameState::new(refs, 1);
        // Two adjacent Clear hexes plus a Sea hex; leave (2,0) off-map.
        let a = HexCoord::new(0, 0);
        let b = HexCoord::new(1, 0); // adjacent to a
        g.map.set_hex(a, HexTerrain::Clear);
        g.map.set_hex(b, HexTerrain::Clear);
        assert!(a.is_adjacent(b));

        let unit = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "mover",
            Composition::Organic { current_toe: 5 },
        );
        g.unit_mut(unit).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(a));

        // Wrong phase (still InitiativeDetermination).
        assert_eq!(
            g.apply(Command::MoveStep { unit, to: b, mode: MoveMode::NonMotorized }),
            Err(CommandError::WrongPhase)
        );

        // Enter the Movement & Combat phase.
        g.phase = Phase::MovementAndCombat;

        // Non-adjacent destination.
        assert_eq!(
            g.apply(Command::MoveStep { unit, to: HexCoord::new(5, 5), mode: MoveMode::NonMotorized }),
            Err(CommandError::NotAdjacent)
        );

        // Successful step onto Clear: location updates, cost (2 CP = 20 tenths) accrues.
        assert_eq!(
            g.apply(Command::MoveStep { unit, to: b, mode: MoveMode::NonMotorized }),
            Ok(crate::state::command::CommandOutcome::Moved)
        );
        assert_eq!(g.unit(unit).unwrap().hex(), Some(b));
        assert_eq!(g.unit(unit).unwrap().cp_expended, Fixed(20));

        // Prohibited terrain: a Sea hex adjacent to b.
        let sea = HexCoord::new(2, 0);
        assert!(b.is_adjacent(sea));
        g.map.set_hex(sea, HexTerrain::Sea);
        assert_eq!(
            g.apply(Command::MoveStep { unit, to: sea, mode: MoveMode::Motorized }),
            Err(CommandError::ProhibitedTerrain)
        );

        // Off-map adjacent hex.
        let off = HexCoord::new(1, 1);
        assert!(b.is_adjacent(off) && g.map.hex(off).is_none());
        assert_eq!(
            g.apply(Command::MoveStep { unit, to: off, mode: MoveMode::NonMotorized }),
            Err(CommandError::DestinationOffMap)
        );
    }

    #[test]
    fn over_cpa_movement_earns_disorganization() {
        use crate::reference::terrain::{Fixed, HexTerrain};
        use crate::rules::movement::MoveMode;
        use crate::state::command::Command;
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::Composition;

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let a = HexCoord::new(0, 0);
        let b = HexCoord::new(1, 0);
        g.map.set_hex(a, HexTerrain::Clear);
        g.map.set_hex(b, HexTerrain::Clear);

        // Infantry ID 'l' has CPA 10.
        let u = g.spawn_unit(
            Nationality::Commonwealth,
            "l",
            "over-mover",
            Composition::Organic { current_toe: 6 },
        );
        assert_eq!(g.unit_cpa(u), Some(10));
        g.unit_mut(u).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(a));
        g.unit_mut(u).unwrap().cp_expended = Fixed(90); // 9 CP already spent
        g.phase = Phase::MovementAndCombat;

        // Enter Clear (+2 CP -> 11 total, 1 over CPA 10) => 1 Disorganization Point.
        assert_eq!(
            g.apply(Command::MoveStep { unit: u, to: b, mode: MoveMode::NonMotorized }),
            Ok(crate::state::command::CommandOutcome::Moved)
        );
        assert_eq!(g.unit(u).unwrap().cp_expended, Fixed(110));
        assert_eq!(g.unit(u).unwrap().cohesion, -1);
    }

    #[test]
    fn hexside_crossing_costs_and_directional_escarpment() {
        use crate::reference::terrain::{Fixed, HexTerrain};
        use crate::rules::movement::MoveMode;
        use crate::state::command::{Command, CommandError};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::{HexCoord, HexsideKind};
        use crate::state::unit::Composition;

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        let mut g = GameState::new(refs, 1);
        let a = HexCoord::new(0, 0);
        let b = HexCoord::new(1, 0);
        let c = HexCoord::new(2, 0);
        for h in [a, b, c] {
            g.map.set_hex(h, HexTerrain::Clear);
        }
        // Wadi on the a-b edge; Escarpment on b-c with c as the "up" side.
        g.map.set_hexside(a, b, HexsideKind::Wadi, None);
        g.map.set_hexside(b, c, HexsideKind::Escarpment, Some(c));

        let u = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "mover",
            Composition::Organic { current_toe: 5 },
        );
        g.unit_mut(u).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(a));
        g.phase = Phase::MovementAndCombat;

        // a->b: Clear entry (2) + Wadi non-mot crossing (1) = 3 CP.
        assert_eq!(
            g.apply(Command::MoveStep { unit: u, to: b, mode: MoveMode::NonMotorized }),
            Ok(crate::state::command::CommandOutcome::Moved)
        );
        assert_eq!(g.unit(u).unwrap().cp_expended, Fixed(30));

        // b->c up an escarpment: motorized is prohibited (§8.42).
        assert_eq!(
            g.apply(Command::MoveStep { unit: u, to: c, mode: MoveMode::Motorized }),
            Err(CommandError::ProhibitedTerrain)
        );

        // b->c up-escarpment on foot: Clear entry (2) + up-escarpment non-mot (6) = 8 CP.
        assert_eq!(
            g.apply(Command::MoveStep { unit: u, to: c, mode: MoveMode::NonMotorized }),
            Ok(crate::state::command::CommandOutcome::Moved)
        );
        assert_eq!(g.unit(u).unwrap().cp_expended, Fixed(110));
    }

    #[test]
    fn zones_of_control() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::game::GameState;
        use crate::state::map::{HexCoord, HexsideKind};
        use crate::state::unit::Composition;

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);

        let x = HexCoord::new(0, 0);
        g.map.set_hex(x, HexTerrain::Clear);
        for n in x.neighbors() {
            g.map.set_hex(n, HexTerrain::Clear);
        }

        // Infantry 'h' (Defensive CA 2) × 5 TOE = 10 Raw Def CA → meets the §10.15 gate.
        let u = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "zoc bn",
            Composition::Organic { current_toe: 5 },
        );
        g.unit_mut(u).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(x));
        // Above battalion-equivalent (2 SP) so it exerts a ZOC (§10.11).
        g.unit_mut(u).unwrap().org_size = crate::state::unit::OrgSize::StandardBrigade;

        assert_eq!(g.hex_raw_defensive_ca(x), 10);
        assert_eq!(g.hex_projects_zoc(x), Some(Side::Commonwealth));
        assert_eq!(g.zoc_hexes(x).len(), 6);

        // An escarpment on one edge blocks ZOC into that neighbor (§10.21).
        let blocked = x.neighbors()[0];
        let open = x.neighbors()[1];
        g.map.set_hexside(x, blocked, HexsideKind::Escarpment, Some(blocked));
        assert_eq!(g.zoc_hexes(x).len(), 5);
        assert!(!g.zoc_hexes(x).contains(&blocked));
        assert!(g.hex_in_zoc_of(open, Side::Commonwealth));
        assert!(!g.hex_in_zoc_of(blocked, Side::Commonwealth));

        // Weaken below the §10.15 threshold: 2 TOE → 4 Raw Def CA < 10 → no ZOC.
        g.unit_mut(u).unwrap().composition = Composition::Organic { current_toe: 2 };
        assert_eq!(g.hex_projects_zoc(x), None);
        assert!(g.zoc_hexes(x).is_empty());
    }

    #[test]
    fn cannot_move_zoc_to_zoc() {
        use crate::reference::terrain::HexTerrain;
        use crate::rules::movement::MoveMode;
        use crate::state::command::{Command, CommandError};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::Composition;

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);

        // E projects a CW ZOC; `from` and `to` are both neighbors of E and adjacent to
        // each other, so both are enemy-controlled for an Axis mover.
        let e = HexCoord::new(0, 0);
        let from = HexCoord::new(1, 0);
        let to = HexCoord::new(1, 1);
        for h in [e, from, to] {
            g.map.set_hex(h, HexTerrain::Clear);
        }
        assert!(from.is_adjacent(to));

        let enemy = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "zoc bn",
            Composition::Organic { current_toe: 5 },
        );
        g.unit_mut(enemy).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(e));
        // Above battalion-equivalent (2 SP) so it exerts a ZOC (§10.11).
        g.unit_mut(enemy).unwrap().org_size = crate::state::unit::OrgSize::StandardBrigade;

        // Italian (Axis) mover starting in the CW ZOC hex `from`.
        let mover = g.spawn_unit(
            Nationality::Italian,
            "n",
            "axis bn",
            Composition::Organic { current_toe: 4 },
        );
        g.unit_mut(mover).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(from));
        g.phase = Phase::MovementAndCombat;

        assert!(g.hex_in_zoc_of(from, crate::reference::units::Side::Commonwealth));
        assert!(g.hex_in_zoc_of(to, crate::reference::units::Side::Commonwealth));
        assert_eq!(
            g.apply(Command::MoveStep { unit: mover, to, mode: MoveMode::NonMotorized }),
            Err(CommandError::ZocToZoc)
        );
    }

    #[test]
    fn must_stop_on_entering_enemy_zoc() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::rules::movement::MoveMode;
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, OrgSize};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_unit_characteristics(
            Nationality::Italian,
            include_str!("../../../data/reference/unit-characteristics/italian.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        g.phase = Phase::MovementAndCombat;

        let e = HexCoord::new(0, 0);
        let zoc = HexCoord::new(1, 0); // neighbour of E -> in its ZOC
        let start = HexCoord::new(2, 0); // distance 2 -> outside the ZOC
        for h in [e, zoc, start] {
            g.map.set_hex(h, HexTerrain::Clear);
        }
        let enemy = g.spawn_unit(Nationality::Commonwealth, "h", "zoc bn", Composition::Organic { current_toe: 5 });
        g.unit_mut(enemy).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(e));
        g.unit_mut(enemy).unwrap().org_size = OrgSize::StandardBrigade;
        let mover = g.spawn_unit(Nationality::Italian, "n", "axis bn", Composition::Organic { current_toe: 4 });
        g.unit_mut(mover).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(start));

        assert!(!g.hex_in_zoc_of(start, Side::Commonwealth));
        assert!(g.hex_in_zoc_of(zoc, Side::Commonwealth));

        // Entering the ZOC hex is allowed and flags the unit (§8.14).
        assert_eq!(
            g.apply(Command::MoveStep { unit: mover, to: zoc, mode: MoveMode::NonMotorized }),
            Ok(CommandOutcome::Moved)
        );
        assert!(g.unit(mover).unwrap().moved_into_zoc);
        // Any further movement this stage is refused.
        assert_eq!(
            g.apply(Command::MoveStep { unit: mover, to: start, mode: MoveMode::NonMotorized }),
            Err(CommandError::MustStopInZoc)
        );
    }

    #[test]
    fn leaving_enemy_zoc_costs_extra_when_in_contact() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::rules::movement::MoveMode;
        use crate::state::command::{Command, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, OrgSize};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_unit_characteristics(
            Nationality::Italian,
            include_str!("../../../data/reference/unit-characteristics/italian.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        g.phase = Phase::MovementAndCombat;

        let e = HexCoord::new(0, 0);
        let inzoc = HexCoord::new(1, 0); // in E's ZOC
        let out = HexCoord::new(2, 0); // out of the ZOC
        let far1 = HexCoord::new(6, 0); // control lane, no ZOC
        let far2 = HexCoord::new(7, 0);
        for h in [e, inzoc, out, far1, far2] {
            g.map.set_hex(h, HexTerrain::Clear);
        }
        let enemy = g.spawn_unit(Nationality::Commonwealth, "h", "zoc bn", Composition::Organic { current_toe: 5 });
        g.unit_mut(enemy).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(e));
        g.unit_mut(enemy).unwrap().org_size = OrgSize::StandardBrigade;

        assert!(g.hex_in_zoc_of(inzoc, Side::Commonwealth));
        assert!(!g.hex_in_zoc_of(out, Side::Commonwealth));
        assert!(!g.hex_in_zoc_of(far1, Side::Commonwealth));
        assert!(!g.hex_in_zoc_of(far2, Side::Commonwealth));

        // Mover A is In Contact and leaves the ZOC; Mover B is an identical control move.
        let a = g.spawn_unit(Nationality::Italian, "n", "a", Composition::Organic { current_toe: 4 });
        g.unit_mut(a).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(inzoc));
        g.unit_mut(a).unwrap().status.in_contact = true;
        let b = g.spawn_unit(Nationality::Italian, "n", "b", Composition::Organic { current_toe: 4 });
        g.unit_mut(b).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(far1));

        assert_eq!(
            g.apply(Command::MoveStep { unit: a, to: out, mode: MoveMode::NonMotorized }),
            Ok(CommandOutcome::Moved)
        );
        assert_eq!(
            g.apply(Command::MoveStep { unit: b, to: far2, mode: MoveMode::NonMotorized }),
            Ok(CommandOutcome::Moved)
        );
        // §8.15.2: In-Contact exit costs +2 CP (20 tenths) over the identical base move.
        assert_eq!(
            g.unit(a).unwrap().cp_expended.0,
            g.unit(b).unwrap().cp_expended.0 + 20
        );
    }

    #[test]
    fn training_and_raider_source_17_3_27_12() {
        use crate::reference::terrain::{Fixed, HexTerrain};
        use crate::reference::units::Side;
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase, RaiderKind};
        use crate::state::map::{HexCoord, HexId};
        use crate::state::unit::{Composition, OrgSize, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(Nationality::Commonwealth, include_str!("../../../data/reference/unit-characteristics/commonwealth.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        g.phase = Phase::Organization;
        g.active_player = Side::Commonwealth;
        let cairo = HexId::parse("E1430").unwrap().to_hex_coord();
        let elsewhere = HexCoord::new(0, 0);
        g.map.set_hex(cairo, HexTerrain::Clear);
        g.map.set_hex(elsewhere, HexTerrain::Clear);

        // A green Commonwealth recce unit at Cairo: current Basic Morale 0, trained reference +1.
        let recce = g.spawn_unit(Nationality::Commonwealth, "hh", "LRDG recce", Composition::Organic { current_toe: 4 });
        {
            let u = g.unit_mut(recce).unwrap();
            u.location = Some(UnitLocation::OnMap(cairo));
            u.basic_morale = 0;
            u.basic_morale_reference = 1;
        }
        // §27.12: an untrained (green) source may not form a desert raider.
        assert!(!g.raider_source_ok(RaiderKind::Lrdg, recce));

        // A fully-trained infantry instruction battalion co-located (§17.35).
        let instr = g.spawn_unit(Nationality::Commonwealth, "a", "instr bn", Composition::Organic { current_toe: 100 });
        {
            let u = g.unit_mut(instr).unwrap();
            u.location = Some(UnitLocation::OnMap(cairo));
            u.org_size = OrgSize::Battalion;
            u.basic_morale = 1;
            u.basic_morale_reference = 1;
        }

        // §17.32: training outside a training area is illegal.
        g.unit_mut(recce).unwrap().location = Some(UnitLocation::OnMap(elsewhere));
        assert_eq!(g.apply(Command::TrainUnit { unit: recce }), Err(CommandError::TrainingIllegal));
        g.unit_mut(recce).unwrap().location = Some(UnitLocation::OnMap(cairo));

        // Designate training: sets in_training; morale is unchanged this stage.
        assert_eq!(g.apply(Command::TrainUnit { unit: recce }), Ok(CommandOutcome::UnitTraining { basic_morale: 0 }));
        assert!(g.unit(recce).unwrap().status.in_training);

        // §17.34: the sixth uninterrupted training stage raises Basic Morale one step.
        g.unit_mut(recce).unwrap().training_progress = 5;
        g.unit_mut(recce).unwrap().cp_expended = Fixed(0);
        g.begin_operations_stage();
        assert_eq!(g.unit(recce).unwrap().basic_morale, 1);
        assert_eq!(g.unit(recce).unwrap().training_progress, 0);
        assert!(!g.unit(recce).unwrap().status.in_training);
        // Now trained to its maximum → §27.12 source is valid.
        assert!(g.raider_source_ok(RaiderKind::Lrdg, recce));

        // §17.33: expending CP during a training stage interrupts it (progress resets).
        {
            let u = g.unit_mut(recce).unwrap();
            u.basic_morale = 0; // pretend still green
            u.training_progress = 3;
            u.status.in_training = true;
            u.cp_expended = Fixed(30);
        }
        g.begin_operations_stage();
        assert_eq!(g.unit(recce).unwrap().training_progress, 0, "§17.33 interrupted by CP expenditure");
        assert_eq!(g.unit(recce).unwrap().basic_morale, 0, "no step gained when interrupted");

        // §17.32: all six training areas are recognized — Cairo/Helwan, both Alexandria hexes,
        // and the three minor areas (Amiriya, Abougir, Deghelia).
        for id in ["E1430", "E3613", "E3714", "E2212", "E3815", "E3512"] {
            let h = HexId::parse(id).unwrap().to_hex_coord();
            assert!(g.is_cw_training_area(h), "{id} should be a §17.32 training area");
        }
        assert!(!g.is_cw_training_area(HexCoord::new(0, 0)), "arbitrary hex is not a training area");
    }

    #[test]
    fn contact_engaged_lifecycle_8_67() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::rules::movement::MoveMode;
        use crate::state::command::{Command, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, OrgSize, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(Nationality::Commonwealth, include_str!("../../../data/reference/unit-characteristics/commonwealth.ron")).unwrap();
        refs.load_unit_characteristics(Nationality::Italian, include_str!("../../../data/reference/unit-characteristics/italian.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        g.phase = Phase::MovementAndCombat;

        let e = HexCoord::new(0, 0);
        let inzoc = HexCoord::new(1, 0); // in the big enemy's ZOC
        let out = HexCoord::new(2, 0);
        let p = HexCoord::new(10, 0); // small enemy (no ZOC)
        let q = HexCoord::new(11, 0); // adjacent to p — engaged lane
        let r = HexCoord::new(12, 0); // away from p
        let f1 = HexCoord::new(6, 0); // control lane
        let f2 = HexCoord::new(7, 0);
        for h in [e, inzoc, out, p, q, r, f1, f2] {
            g.map.set_hex(h, HexTerrain::Clear);
        }
        let at = |g: &mut GameState, u, h| g.unit_mut(u).unwrap().location = Some(UnitLocation::OnMap(h));

        // A brigade projects ZOC into `inzoc`; a lone battalion at `p` projects none (§10.11).
        let big = g.spawn_unit(Nationality::Commonwealth, "h", "zoc bde", Composition::Organic { current_toe: 5 });
        at(&mut g, big, e);
        g.unit_mut(big).unwrap().org_size = OrgSize::StandardBrigade;
        let small = g.spawn_unit(Nationality::Commonwealth, "h", "small bn", Composition::Organic { current_toe: 4 });
        at(&mut g, small, p);
        g.unit_mut(small).unwrap().org_size = OrgSize::Battalion;
        assert!(g.hex_in_zoc_of(inzoc, Side::Commonwealth));
        assert!(!g.hex_in_zoc_of(q, Side::Commonwealth), "a lone battalion projects no ZOC");

        // §8.62: In-Contact is derived from enemy ZOCs on refresh — no combat needed.
        let c = g.spawn_unit(Nationality::Italian, "n", "c", Composition::Organic { current_toe: 4 });
        at(&mut g, c, inzoc);
        assert!(!g.unit(c).unwrap().status.in_contact);
        g.refresh_contact_status();
        assert!(g.unit(c).unwrap().status.in_contact, "§8.62 in an enemy ZOC = In Contact");
        // Leaving the ZOC clears it (the refresh after the move).
        assert_eq!(g.apply(Command::MoveStep { unit: c, to: out, mode: MoveMode::NonMotorized }), Ok(CommandOutcome::Moved));
        assert!(!g.unit(c).unwrap().status.in_contact, "§8.62 left the ZOC");

        // §8.66/§8.67: an Engaged unit adjacent to a non-ZOC enemy.
        let x = g.spawn_unit(Nationality::Italian, "n", "x", Composition::Organic { current_toe: 4 });
        at(&mut g, x, q);
        g.unit_mut(x).unwrap().status.engaged = true;
        let y = g.spawn_unit(Nationality::Italian, "n", "y", Composition::Organic { current_toe: 4 });
        at(&mut g, y, f1);
        // §8.67: refresh does not clear an engagement while an opposing combat unit is still adjacent.
        g.refresh_contact_status();
        assert!(g.unit(x).unwrap().status.engaged);
        // §8.66: breaking off costs +4 CP even though `x` is not in a ZOC.
        assert_eq!(g.apply(Command::MoveStep { unit: x, to: r, mode: MoveMode::NonMotorized }), Ok(CommandOutcome::Moved));
        assert_eq!(g.apply(Command::MoveStep { unit: y, to: f2, mode: MoveMode::NonMotorized }), Ok(CommandOutcome::Moved));
        assert_eq!(g.unit(x).unwrap().cp_expended.0, g.unit(y).unwrap().cp_expended.0 + 40);
        // §8.67: having moved away from the last enemy, `x` is no longer Engaged.
        assert!(!g.unit(x).unwrap().status.engaged);
    }

    #[test]
    fn retreat_full_and_shortfall() {
        use crate::reference::terrain::HexTerrain;
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::Composition;

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        g.phase = Phase::MovementAndCombat;
        for c in 0..3 {
            g.map.set_hex(HexCoord::new(c, 0), HexTerrain::Clear);
        }
        g.map.set_hex(HexCoord::new(0, 5), HexTerrain::Clear);

        // Full 2-hex retreat away from the origin: no shortfall loss.
        let o1 = HexCoord::new(0, 0);
        let u1 = g.spawn_unit(Nationality::Commonwealth, "h", "u1", Composition::Organic { current_toe: 10 });
        g.unit_mut(u1).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(o1));
        g.unit_mut(u1).unwrap().pending_retreat = 2;
        g.unit_mut(u1).unwrap().retreat_origin = Some(o1);
        assert_eq!(
            g.apply(Command::Retreat { unit: u1, path: vec![HexCoord::new(1, 0), HexCoord::new(2, 0)] }),
            Ok(CommandOutcome::Retreated { toe_removed: 0, hexes_short: 0 })
        );
        assert_eq!(g.unit(u1).unwrap().hex(), Some(HexCoord::new(2, 0)));
        assert_eq!(g.unit(u1).unwrap().pending_retreat, 0);

        // Standing (empty path) with 2 hexes owed: 2 x 10% = 20% of TOE 10 -> 2 lost.
        let o2 = HexCoord::new(0, 5);
        let u2 = g.spawn_unit(Nationality::Commonwealth, "h", "u2", Composition::Organic { current_toe: 10 });
        g.unit_mut(u2).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(o2));
        g.unit_mut(u2).unwrap().pending_retreat = 2;
        g.unit_mut(u2).unwrap().retreat_origin = Some(o2);
        assert_eq!(
            g.apply(Command::Retreat { unit: u2, path: vec![] }),
            Ok(CommandOutcome::Retreated { toe_removed: 2, hexes_short: 2 })
        );
        assert_eq!(g.unit(u2).unwrap().hex(), Some(o2));
        assert_eq!(g.unit(u2).unwrap().total_toe(), 8);

        // A unit that owes no retreat is rejected.
        let u3 = g.spawn_unit(Nationality::Commonwealth, "h", "u3", Composition::Organic { current_toe: 5 });
        g.unit_mut(u3).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(o2));
        assert_eq!(
            g.apply(Command::Retreat { unit: u3, path: vec![] }),
            Err(CommandError::NoRetreatPending)
        );
    }

    #[test]
    fn retreat_cannot_enter_enemy_zoc() {
        use crate::reference::terrain::HexTerrain;
        use crate::state::command::{Command, CommandError};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, OrgSize};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_unit_characteristics(
            Nationality::Italian,
            include_str!("../../../data/reference/unit-characteristics/italian.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        g.phase = Phase::MovementAndCombat;
        for c in 0..4 {
            g.map.set_hex(HexCoord::new(c, 0), HexTerrain::Clear);
        }
        // CW brigade at (3,0) exerts a ZOC over (2,0).
        let e = g.spawn_unit(Nationality::Commonwealth, "h", "zoc", Composition::Organic { current_toe: 5 });
        g.unit_mut(e).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(HexCoord::new(3, 0)));
        g.unit_mut(e).unwrap().org_size = OrgSize::StandardBrigade;

        let o = HexCoord::new(0, 0);
        let u = g.spawn_unit(Nationality::Italian, "n", "u", Composition::Organic { current_toe: 4 });
        g.unit_mut(u).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(o));
        g.unit_mut(u).unwrap().pending_retreat = 2;
        g.unit_mut(u).unwrap().retreat_origin = Some(o);

        // (2,0) lies in the CW ZOC -> retreat into it is illegal (§10.23).
        assert_eq!(
            g.apply(Command::Retreat { unit: u, path: vec![HexCoord::new(1, 0), HexCoord::new(2, 0)] }),
            Err(CommandError::RetreatIntoZoc)
        );
    }

    #[test]
    fn reaction_moves_non_phasing_unit_away() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, OrgSize, WeaponGroup};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_weapons_systems(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/weapons/commonwealth.ron"),
        )
        .unwrap();
        refs.load_unit_characteristics(
            Nationality::Italian,
            include_str!("../../../data/reference/unit-characteristics/italian.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        g.phase = Phase::MovementAndCombat;
        g.active_player = Side::Axis; // Axis (Italian) is phasing; Commonwealth reacts
        for c in 0..6 {
            g.map.set_hex(HexCoord::new(c, 0), HexTerrain::Clear);
            g.map.set_hex(HexCoord::new(c, 2), HexTerrain::Clear);
        }
        let cruiser = || Composition::Weapons { groups: vec![WeaponGroup { system: "A13 Cruiser".into(), count: 6 }] };

        // A phasing Italian battalion moved adjacent to a motorized CW tank.
        let enemy = g.spawn_unit(Nationality::Italian, "p", "enemy", Composition::Organic { current_toe: 4 });
        g.unit_mut(enemy).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(HexCoord::new(1, 0)));
        let tank = g.spawn_unit(Nationality::Commonwealth, "g", "tank", cruiser());
        g.unit_mut(tank).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(HexCoord::new(2, 0)));
        assert!(g.unit_is_motorized(tank));

        // (1) The tank reacts away one hex.
        assert_eq!(
            g.apply(Command::React { unit: tank, path: vec![HexCoord::new(3, 0)] }),
            Ok(CommandOutcome::Reacted)
        );
        assert_eq!(g.unit(tank).unwrap().hex(), Some(HexCoord::new(3, 0)));

        // (2) A non-motorized infantry may not react (§8.53a).
        let inf = g.spawn_unit(Nationality::Commonwealth, "h", "inf", Composition::Organic { current_toe: 4 });
        g.unit_mut(inf).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(HexCoord::new(2, 0)));
        assert!(!g.unit_is_motorized(inf));
        assert_eq!(
            g.apply(Command::React { unit: inf, path: vec![HexCoord::new(3, 0)] }),
            Err(CommandError::CannotReact)
        );

        // (3) A phasing-side unit may not react.
        assert_eq!(
            g.apply(Command::React { unit: enemy, path: vec![] }),
            Err(CommandError::CannotReact)
        );

        // (4) With no enemy adjacent there is no trigger (§8.51).
        assert_eq!(
            g.apply(Command::React { unit: tank, path: vec![HexCoord::new(4, 0)] }),
            Err(CommandError::NoReactionTrigger)
        );

        // (5) A reaction may not move into an enemy ZOC (§8.55).
        let trig = g.spawn_unit(Nationality::Italian, "p", "trig", Composition::Organic { current_toe: 4 });
        g.unit_mut(trig).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(HexCoord::new(2, 2))); // battalion: no ZOC, but triggers
        let brig = g.spawn_unit(Nationality::Italian, "p", "brig", Composition::Organic { current_toe: 6 });
        g.unit_mut(brig).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(HexCoord::new(5, 2)));
        g.unit_mut(brig).unwrap().org_size = OrgSize::StandardBrigade; // exerts a ZOC over (4,2)
        let tank2 = g.spawn_unit(Nationality::Commonwealth, "g", "tank2", cruiser());
        g.unit_mut(tank2).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(HexCoord::new(3, 2)));
        assert_eq!(
            g.apply(Command::React { unit: tank2, path: vec![HexCoord::new(4, 2)] }),
            Err(CommandError::ReactIntoZoc)
        );
    }

    #[test]
    fn far_escapee_moves_and_thirsts_28_24() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::game::{Escapee, GameState};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(Nationality::Commonwealth, include_str!("../../../data/reference/unit-characteristics/commonwealth.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        // A waterless Clear corridor from the escapee at (10,5) toward a friendly unit at (5,5).
        for c in 5..=10 {
            g.map.set_hex(HexCoord::new(c, 5), HexTerrain::Clear);
        }
        // Friendly Commonwealth unit at (5,5): 5 Clear hexes = 100 tenths > the 80-tenth (8 CP)
        // rejoin range, so the escapee is a "far" escapee that must wander.
        let friend = g.spawn_unit(Nationality::Commonwealth, "h", "friend", Composition::Organic { current_toe: 4 });
        g.unit_mut(friend).unwrap().location = Some(UnitLocation::OnMap(HexCoord::new(5, 5)));
        g.escapees.push(Escapee { side: Side::Commonwealth, hex: HexCoord::new(10, 5), points: 10 });

        // One Operations Stage: steps one hex toward the friend (10→9,5) and, with no water there,
        // loses 10% (ceil(10/10)=1) of its strength to thirst.
        g.resolve_prisoner_escapes();
        assert_eq!(g.escapees.len(), 1, "still too far to rejoin");
        assert_eq!(g.escapees[0].hex, HexCoord::new(9, 5), "§28.24 moved one hex toward the friend");
        assert_eq!(g.escapees[0].points, 9, "§28.24 lost 10% to thirst (no water source)");
    }

    #[test]
    fn within_cp_budget_respects_terrain() {
        use crate::reference::terrain::HexTerrain;
        use crate::rules::movement::MoveMode;
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        let mut g = GameState::new(refs, 1);
        g.phase = Phase::MovementAndCombat;
        // A clear corridor hexes 0..=4; hex (5,0) is left unset (a gap), (6,0) is an isolated island.
        for c in 0..=4 {
            g.map.set_hex(HexCoord::new(c, 0), HexTerrain::Clear);
        }
        g.map.set_hex(HexCoord::new(6, 0), HexTerrain::Clear);
        let a = HexCoord::new(0, 0);
        // Non-motorized enter cost is 2 CP (20 tenths) per Clear hex: two hexes (40) fit a 40 budget,
        // three (60) do not.
        assert!(g.within_cp_budget(a, HexCoord::new(2, 0), MoveMode::NonMotorized, 40));
        assert!(!g.within_cp_budget(a, HexCoord::new(3, 0), MoveMode::NonMotorized, 40));
        // The (5,0) gap is impassable, so the (6,0) island is unreachable at any budget.
        assert!(!g.within_cp_budget(a, HexCoord::new(6, 0), MoveMode::NonMotorized, 400));
    }

    #[test]
    fn reaction_detach_to_react_8_56() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation, WeaponGroup};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(Nationality::Commonwealth, include_str!("../../../data/reference/unit-characteristics/commonwealth.ron")).unwrap();
        refs.load_weapons_systems(Nationality::Commonwealth, include_str!("../../../data/reference/weapons/commonwealth.ron")).unwrap();
        refs.load_unit_characteristics(Nationality::Italian, include_str!("../../../data/reference/unit-characteristics/italian.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        g.phase = Phase::MovementAndCombat;
        g.active_player = Side::Axis; // Italian phasing; Commonwealth reacts
        for c in 0..5 {
            g.map.set_hex(HexCoord::new(c, 0), HexTerrain::Clear);
        }
        let cruiser = || Composition::Weapons { groups: vec![WeaponGroup { system: "A13 Cruiser".into(), count: 6 }] };

        // Phasing Italian battalion adjacent to a CW parent stack (triggers reaction).
        let enemy = g.spawn_unit(Nationality::Italian, "p", "enemy", Composition::Organic { current_toe: 4 });
        g.unit_mut(enemy).unwrap().location = Some(UnitLocation::OnMap(HexCoord::new(1, 0)));

        // A motorized CW tank attached to a co-located parent.
        let parent = g.spawn_unit(Nationality::Commonwealth, "g", "parent", cruiser());
        g.unit_mut(parent).unwrap().location = Some(UnitLocation::OnMap(HexCoord::new(2, 0)));
        let tank = g.spawn_unit(Nationality::Commonwealth, "g", "tank", cruiser());
        {
            let u = g.unit_mut(tank).unwrap();
            u.location = Some(UnitLocation::OnMap(HexCoord::new(2, 0)));
            u.attached_parent = Some(parent);
        }

        // §8.56: the attached tank reacts by detaching first — it moves and is no longer attached.
        assert_eq!(
            g.apply(Command::React { unit: tank, path: vec![HexCoord::new(3, 0)] }),
            Ok(CommandOutcome::Reacted)
        );
        assert_eq!(g.unit(tank).unwrap().hex(), Some(HexCoord::new(3, 0)));
        assert_eq!(g.unit(tank).unwrap().attached_parent, None, "§8.56 detaches on reacting");

        // A unit that is itself a parent of an attachment may not detach-and-react (§8.56: its
        // attachments would have to detach too). Attach a child to `parent`, then have `parent`
        // (attached to a grandparent) try to react.
        let grand = g.spawn_unit(Nationality::Commonwealth, "g", "grand", cruiser());
        g.unit_mut(grand).unwrap().location = Some(UnitLocation::OnMap(HexCoord::new(2, 0)));
        let child = g.spawn_unit(Nationality::Commonwealth, "g", "child", cruiser());
        {
            let u = g.unit_mut(child).unwrap();
            u.location = Some(UnitLocation::OnMap(HexCoord::new(2, 0)));
            u.attached_parent = Some(parent);
        }
        g.unit_mut(parent).unwrap().attached_parent = Some(grand);
        assert_eq!(
            g.apply(Command::React { unit: parent, path: vec![HexCoord::new(3, 0)] }),
            Err(CommandError::CannotReact),
            "§8.56: a unit with its own attachments cannot detach-and-react"
        );
    }

    #[test]
    fn reaction_pinning() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, OrgSize};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_unit_characteristics(
            Nationality::Italian,
            include_str!("../../../data/reference/unit-characteristics/italian.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        g.phase = Phase::MovementAndCombat;
        g.active_player = Side::Axis;
        for c in 0..4 {
            g.map.set_hex(HexCoord::new(c, 0), HexTerrain::Clear);
            g.map.set_hex(HexCoord::new(c, 2), HexTerrain::Clear);
        }

        // A fast Italian recce (CPA 45) adjacent to a motorized CW tank battalion (CPA 25):
        // 45 >= 25 + 6 and same size -> pinned (§8.53b).
        let recce = g.spawn_unit(Nationality::Italian, "vv", "recce", Composition::Organic { current_toe: 4 });
        g.unit_mut(recce).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(HexCoord::new(1, 0)));
        let bn = g.spawn_unit(Nationality::Commonwealth, "g", "bn", Composition::Organic { current_toe: 4 });
        g.unit_mut(bn).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(HexCoord::new(2, 0)));
        assert!(g.unit_is_motorized(bn));
        assert_eq!(
            g.apply(Command::React { unit: bn, path: vec![HexCoord::new(3, 0)] }),
            Err(CommandError::Pinned)
        );

        // §8.54: a battalion may not pin a division, so a motorized division reacts even
        // though the recce is >6 CPA faster.
        let recce2 = g.spawn_unit(Nationality::Italian, "vv", "recce2", Composition::Organic { current_toe: 4 });
        g.unit_mut(recce2).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(HexCoord::new(1, 2)));
        let div = g.spawn_unit(Nationality::Commonwealth, "g", "div", Composition::Organic { current_toe: 4 });
        g.unit_mut(div).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(HexCoord::new(2, 2)));
        g.unit_mut(div).unwrap().org_size = OrgSize::Division;
        assert_eq!(
            g.apply(Command::React { unit: div, path: vec![HexCoord::new(3, 2)] }),
            Ok(CommandOutcome::Reacted)
        );
        assert_eq!(g.unit(div).unwrap().hex(), Some(HexCoord::new(3, 2)));
    }

    #[test]
    fn overrun_detection_and_defender_round_up() {
        use crate::state::game::GameState;
        use crate::state::unit::Composition;

        let mut g = GameState::new(close_assault_refs(), 3);

        // §15.83 raw-points rounding: "h" has Def CA 2, so TOE 7 -> raw 14; 30% = 4.2 raw.
        // Rounded up (5 raw -> 3 TOE) vs down (4 raw -> 2 TOE).
        let up = g.spawn_unit(Nationality::Commonwealth, "h", "up", Composition::Organic { current_toe: 7 });
        assert_eq!(g.apply_raw_loss(&[up], 30, false, true), 3);
        let dn = g.spawn_unit(Nationality::Commonwealth, "h", "dn", Composition::Organic { current_toe: 7 });
        assert_eq!(g.apply_raw_loss(&[dn], 30, false, false), 2);

        // An overwhelming attacker lands in the Overrun columns (differential >= +11), and
        // the defender takes losses rounded up (§15.77).
        let atk = g.spawn_unit(Nationality::Commonwealth, "h", "atk", Composition::Organic { current_toe: 100 });
        g.unit_mut(atk).unwrap().basic_morale = 8;
        let def = g.spawn_unit(Nationality::Commonwealth, "h", "def", Composition::Organic { current_toe: 7 });
        g.unit_mut(def).unwrap().basic_morale = 8;
        let def_toe = g.unit(def).unwrap().total_toe();
        let d_raw_before = g.close_assault_raw(&[def], false);
        let ca_per_toe = (d_raw_before / def_toe as i32) as u32;

        let ca = g.resolve_close_assault_with_rolls(&[atk], &[def], 11, 11, 33, 33);
        assert!(ca.is_overrun, "differential {} should be an overrun", ca.differential);
        // §15.77: raw points lost rounded up, then TOE removed to absorb them (§15.83d).
        let raw_lost = (d_raw_before as u32 * ca.defender.loss_percent as u32).div_ceil(100);
        let expected = raw_lost.div_ceil(ca_per_toe).min(def_toe);
        assert_eq!(ca.defender_toe_removed, expected);
    }

    #[test]
    fn guns_take_vulnerability_losses_by_position() {
        use crate::state::game::GameState;
        use crate::state::unit::{Composition, GunPosition, WeaponGroup};

        let mut g = GameState::new(close_assault_refs(), 3);
        let arty = |g: &mut GameState, name: &str| {
            g.spawn_unit(
                Nationality::Commonwealth,
                "x",
                name,
                Composition::Weapons { groups: vec![WeaponGroup { system: "25-pounder Gun".into(), count: 4 }] },
            )
        };
        let gun_count = |g: &GameState, id| match &g.unit(id).unwrap().composition {
            Composition::Weapons { groups } => groups[0].count,
            _ => 0,
        };

        // Direct Vulnerability mechanic: 25-pdr Vulnerability 6 -> 6 damage removes 1 TOE,
        // 7 damage removes 2 TOE (§14.42 absorption).
        let g1 = arty(&mut g, "g1");
        assert!(g.is_gun_unit(g1));
        assert_eq!(g.apply_vulnerability_damage(&[g1], 6), 1);
        let g2 = arty(&mut g, "g2");
        assert_eq!(g.apply_vulnerability_damage(&[g2], 7), 2);
        // AA/Flak is exempt (§15.84b).
        let aa = g.spawn_unit(
            Nationality::Commonwealth,
            "x",
            "aa",
            Composition::Weapons { groups: vec![WeaponGroup { system: "Light AA (Bofors 40mm)".into(), count: 3 }] },
        );
        assert!(!g.is_gun_unit(aa));
        assert_eq!(g.apply_vulnerability_damage(&[aa], 100), 0);

        // Non-overrun assault on infantry + a Back gun: the Back gun is untouched (§15.84b:
        // only Forward guns take Vulnerability losses outside an Overrun).
        let atk = g.spawn_unit(Nationality::Commonwealth, "h", "atk", Composition::Organic { current_toe: 40 });
        let troops = g.spawn_unit(Nationality::Commonwealth, "h", "troops", Composition::Organic { current_toe: 20 });
        let back = arty(&mut g, "back");
        g.unit_mut(back).unwrap().gun_position = GunPosition::Back;
        let ca = g.resolve_close_assault_with_rolls(&[atk], &[troops, back], 11, 11, 33, 33);
        assert!(!ca.is_overrun);
        assert_eq!(gun_count(&g, back), 4);
        assert_eq!(ca.defender_vulnerability_removed, 0);

        // Overrun on infantry + a Back gun: §15.84a the gun is caught in the breakthrough.
        let atk2 = g.spawn_unit(Nationality::Commonwealth, "h", "atk2", Composition::Organic { current_toe: 200 });
        let troops2 = g.spawn_unit(Nationality::Commonwealth, "h", "troops2", Composition::Organic { current_toe: 5 });
        let back2 = arty(&mut g, "back2");
        g.unit_mut(back2).unwrap().gun_position = GunPosition::Back;
        let ca2 = g.resolve_close_assault_with_rolls(&[atk2], &[troops2, back2], 11, 11, 33, 33);
        assert!(ca2.is_overrun);
        assert!(gun_count(&g, back2) < 4, "an overrun catches even a Back gun (§15.84a)");

        // §15.84a/§15.21: a defending gun adds nothing to the melee, so the differential is
        // the same with or without it (fresh units to avoid carry-over losses).
        let ax = g.spawn_unit(Nationality::Commonwealth, "h", "ax", Composition::Organic { current_toe: 40 });
        let t1 = g.spawn_unit(Nationality::Commonwealth, "h", "t1", Composition::Organic { current_toe: 20 });
        let d_troops_only = g.resolve_close_assault_with_rolls(&[ax], &[t1], 11, 11, 33, 33).differential;
        let ax2 = g.spawn_unit(Nationality::Commonwealth, "h", "ax2", Composition::Organic { current_toe: 40 });
        let t2b = g.spawn_unit(Nationality::Commonwealth, "h", "t2b", Composition::Organic { current_toe: 20 });
        let gun = arty(&mut g, "gun");
        let d_with_gun = g.resolve_close_assault_with_rolls(&[ax2], &[t2b, gun], 11, 11, 33, 33).differential;
        assert_eq!(d_troops_only, d_with_gun);
    }

    #[test]
    fn stacking_limit_enforced() {
        use crate::reference::terrain::HexTerrain;
        use crate::rules::movement::MoveMode;
        use crate::state::command::{Command, CommandError};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, OrgSize};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        let mut g = GameState::new(refs, 1);
        let from = HexCoord::new(0, 0);
        let to = HexCoord::new(1, 0);
        g.map.set_hex(from, HexTerrain::Clear);
        g.map.set_hex(to, HexTerrain::Mountain); // stacking limit 3
        g.phase = Phase::MovementAndCombat;

        // A Division (5 SP) cannot end in Mountain (limit 3).
        let div = g.spawn_unit(
            Nationality::Commonwealth,
            "a",
            "div",
            Composition::Organic { current_toe: 0 },
        );
        g.unit_mut(div).unwrap().org_size = OrgSize::Division;
        g.unit_mut(div).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(from));
        assert_eq!(
            g.apply(Command::MoveStep { unit: div, to, mode: MoveMode::NonMotorized }),
            Err(CommandError::StackingLimitExceeded)
        );

        // A battalion (1 SP, default) fits.
        let bn = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "bn",
            Composition::Organic { current_toe: 5 },
        );
        g.unit_mut(bn).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(from));
        assert_eq!(
            g.apply(Command::MoveStep { unit: bn, to, mode: MoveMode::NonMotorized }),
            Ok(crate::state::command::CommandOutcome::Moved)
        );
        assert_eq!(g.hex_stacking_points(to), 1);
    }

    #[test]
    fn anti_armor_crt_lookup() {
        use crate::reference::crt::AntiArmorCrt;
        const AA_CRT: &str = include_str!("../../../data/reference/crt/anti-armor.ron");

        let mut refs = ReferenceData::new();
        refs.load_anti_armor_crt(AA_CRT).unwrap();
        let crt = refs.anti_armor_crt.as_ref().unwrap();

        // Row indexing: 11,12 -> row 0; 33,34 -> row 7; 65,66 -> row 17.
        assert_eq!(AntiArmorCrt::row_index(11), 0);
        assert_eq!(AntiArmorCrt::row_index(34), 7);
        assert_eq!(AntiArmorCrt::row_index(66), 17);

        // Best roll (65,66), 16+ points -> 32 damage.
        assert_eq!(crt.damage(66, 16, 16), 32);
        // Worst roll (11,12), 3 actual points -> 1 damage.
        assert_eq!(crt.damage(11, 3, 3), 1);
        // 0* column: 0 actual but 3 raw points, row 51,52 -> 1 damage.
        assert_eq!(crt.damage(51, 0, 3), 1);
        // No effective fire (0 actual, 0 raw) -> 0.
        assert_eq!(crt.damage(11, 0, 0), 0);
        // Points cap at 16+: 20 actual uses the 16+ column.
        assert_eq!(crt.damage(66, 20, 20), 32);
    }

    #[test]
    fn barrage_crt_lookup() {
        use crate::reference::crt::{BarrageCrt, BarrageResult, BarrageTargetClass};
        const BARRAGE_CRT: &str = include_str!("../../../data/reference/crt/barrage.ron");

        let mut refs = ReferenceData::new();
        refs.load_barrage_crt(BARRAGE_CRT).unwrap();
        let crt = refs.barrage_crt.as_ref().unwrap();

        // Column mapping: 1,2->0 ; 11,12->5 ; 17+->8.
        assert_eq!(BarrageCrt::column(1), Some(0));
        assert_eq!(BarrageCrt::column(12), Some(5));
        assert_eq!(BarrageCrt::column(17), Some(8));
        assert_eq!(BarrageCrt::column(30), Some(8));
        assert_eq!(BarrageCrt::column(0), None);

        use BarrageResult::*;
        use BarrageTargetClass::*;
        // Infantry, 11-12 pts column: 11..16 none, 21..44 pinned, 45..66 result 1.
        assert_eq!(crt.result(Infantry, 12, 16), NoEffect);
        assert_eq!(crt.result(Infantry, 12, 44), Pinned);
        assert_eq!(crt.result(Infantry, 12, 45), One);
        // Infantry, 17+ column: 65..66 -> 2.
        assert_eq!(crt.result(Infantry, 20, 66), Two);
        assert_eq!(crt.result(Infantry, 20, 31), Pinned);
        // Armor, 1-2 pts: 63..66 pinned (armor has no "2" result at all).
        assert_eq!(crt.result(Armor, 1, 62), NoEffect);
        assert_eq!(crt.result(Armor, 1, 66), Pinned);
        // Gun, 1-2 pts: 62..66 -> 1 (guns are never Pinned).
        assert_eq!(crt.result(Gun, 2, 61), NoEffect);
        assert_eq!(crt.result(Gun, 2, 62), One);
        // Truck, 1-2 pts: always No effect.
        assert_eq!(crt.result(Truck, 1, 66), NoEffect);
        // Truck, 9-10 pts: 64..66 -> 2.
        assert_eq!(crt.result(Truck, 9, 64), Two);
        // Zero barrage points -> No effect.
        assert_eq!(crt.result(Infantry, 0, 66), NoEffect);
    }

    #[test]
    fn morale_modifier_table_lookup() {
        use crate::reference::crt::{MoraleModifier, MoraleModifierTable};
        const MORALE_RON: &str = include_str!("../../../data/reference/crt/morale-modifier.ron");

        let mut refs = ReferenceData::new();
        refs.load_morale_modifier_table(MORALE_RON).unwrap();
        let t = refs.morale_modifier_table.as_ref().unwrap();

        // Level indexing (clamped +8..-17).
        assert_eq!(MoraleModifierTable::level_index(8), 0);
        assert_eq!(MoraleModifierTable::level_index(5), 3);
        assert_eq!(MoraleModifierTable::level_index(0), 8);
        assert_eq!(MoraleModifierTable::level_index(-1), 9);
        assert_eq!(MoraleModifierTable::level_index(-17), 25);
        assert_eq!(MoraleModifierTable::level_index(15), 0); // clamp high
        assert_eq!(MoraleModifierTable::level_index(-30), 25); // clamp low

        use MoraleModifier::*;
        // +8+ row.
        assert_eq!(t.modifier(8, 11), Delta(4));
        assert_eq!(t.modifier(8, 31), Delta(3));
        assert_eq!(t.modifier(8, 66), Delta(1));
        assert_eq!(t.modifier(12, 11), Delta(4)); // clamp to +8+
        // 0 row: NO band then a -1 on 66.
        assert_eq!(t.modifier(0, 11), Delta(1));
        assert_eq!(t.modifier(0, 12), Delta(0));
        assert_eq!(t.modifier(0, 66), Delta(-1));
        // +5 row: 66 is NO.
        assert_eq!(t.modifier(5, 66), Delta(0));
        // -7 row: 11 is NO, 64..66 Surrender.
        assert_eq!(t.modifier(-7, 11), Delta(0));
        assert_eq!(t.modifier(-7, 64), Surrender);
        // -17- (and below) always Surrender.
        assert_eq!(t.modifier(-17, 11), Surrender);
        assert_eq!(t.modifier(-25, 66), Surrender);
    }

    #[test]
    fn close_assault_crt_lookup() {
        use crate::reference::crt::{CloseAssaultCrt, CloseAssaultSide};
        const CA_RON: &str = include_str!("../../../data/reference/crt/close-assault.ron");

        let mut refs = ReferenceData::new();
        refs.load_close_assault_crt(CA_RON).unwrap();
        let crt = refs.close_assault_crt.as_ref().unwrap();

        // Differential -> column bucketing.
        assert_eq!(CloseAssaultCrt::column_for_differential(-11), 0);
        assert_eq!(CloseAssaultCrt::column_for_differential(-15), 0);
        assert_eq!(CloseAssaultCrt::column_for_differential(-8), 1);
        assert_eq!(CloseAssaultCrt::column_for_differential(-3), 4);
        assert_eq!(CloseAssaultCrt::column_for_differential(0), 7);
        assert_eq!(CloseAssaultCrt::column_for_differential(6), 12);
        assert_eq!(CloseAssaultCrt::column_for_differential(9), 14);
        assert_eq!(CloseAssaultCrt::column_for_differential(13), 15);
        assert_eq!(CloseAssaultCrt::column_for_differential(17), 17);
        assert_eq!(CloseAssaultCrt::column_for_differential(30), 17);

        use CloseAssaultSide::*;

        // Attacker at differential 0: read drives loss %.
        assert_eq!(crt.resolve(Attacker, 0, 11, 7).loss_percent, 25);
        assert_eq!(crt.resolve(Attacker, 0, 30, 7).loss_percent, 10);
        assert_eq!(crt.resolve(Attacker, 0, 55, 7).loss_percent, 0);
        // Attacker at <= -11: sum drives Capt / Eng / In-Contact.
        let r = crt.resolve(Attacker, -11, 11, 3);
        assert!(r.captured && !r.engaged);
        let r = crt.resolve(Attacker, -11, 11, 11);
        assert!(r.engaged && !r.captured);
        let r = crt.resolve(Attacker, -11, 11, 8);
        assert!(r.in_contact && !r.captured && !r.engaged);

        // Defender at +17: read loss + sum secondary (Capt/Retreat can co-occur).
        assert_eq!(crt.resolve(Defender, 17, 66, 5).loss_percent, 5);
        assert_eq!(crt.resolve(Defender, 17, 11, 5).loss_percent, 50);
        let r = crt.resolve(Defender, 17, 66, 5);
        assert!(r.captured && r.retreat_hexes == 1);
        assert_eq!(crt.resolve(Defender, 17, 66, 10).retreat_hexes, 2);
        assert_eq!(crt.resolve(Defender, 17, 66, 12).retreat_hexes, 3);
        // Defender at 0: sum 4 hits nothing -> In Contact.
        let r = crt.resolve(Defender, 0, 11, 4);
        assert_eq!(r.loss_percent, 20);
        assert!(r.in_contact);
        assert!(crt.resolve(Defender, 0, 11, 2).captured);
        assert_eq!(crt.resolve(Defender, 0, 11, 5).retreat_hexes, 1);
    }

    #[test]
    fn prisoners_table_lookup() {
        const PRIS_RON: &str = include_str!("../../../data/reference/crt/prisoners.ron");
        let mut refs = ReferenceData::new();
        refs.load_prisoners_table(PRIS_RON).unwrap();
        let t = refs.prisoners_table.as_ref().unwrap();
        assert_eq!(t.percent(1), 10);
        assert_eq!(t.percent(2), 25);
        assert_eq!(t.percent(3), 33);
        assert_eq!(t.percent(4), 50);
        assert_eq!(t.percent(5), 50);
        assert_eq!(t.percent(6), 75);
    }

    #[test]
    fn close_assault_resolver_strength_and_losses() {
        use crate::state::game::GameState;
        use crate::state::unit::Composition;

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_close_assault_crt(include_str!(
            "../../../data/reference/crt/close-assault.ron"
        ))
        .unwrap();
        refs.load_prisoners_table(include_str!("../../../data/reference/crt/prisoners.ron"))
            .unwrap();
        refs.load_morale_modifier_table(include_str!(
            "../../../data/reference/crt/morale-modifier.ron"
        ))
        .unwrap();
        let mut g = GameState::new(refs, 1);

        // Attacker: infantry "h" (CA off 2), TOE 25 -> raw 50 -> actual 5.
        let atk = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "attacker",
            Composition::Organic { current_toe: 25 },
        );
        // Defender: infantry "h" (CA def 2), TOE 20 -> raw 40 -> actual 4.
        let def = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "defender",
            Composition::Organic { current_toe: 20 },
        );

        // Strengths & differential.
        assert_eq!(g.close_assault_strength(&[atk], true), 5);
        assert_eq!(g.close_assault_strength(&[def], false), 4);
        assert_eq!(g.close_assault_differential(&[atk], &[def]), 1);

        // Differential +1 (column +1). Morale reads 33 (cohesion 0 -> NO change, basic 0)
        // give zero morale shift; combat reads 11 each.
        // Attacker col+1: read 11 -> 25% loss; sum 2 -> no Capt/Eng -> In Contact.
        // Defender col+1: read 11 -> 20% loss; sum 2 -> Capt.
        let out = g.resolve_close_assault_with_rolls(&[atk], &[def], 33, 33, 11, 11);
        assert_eq!(out.differential, 1);
        assert_eq!(out.morale_shift, 0);
        assert_eq!(out.base_column, out.adjusted_column);
        assert_eq!(out.attacker_strength, 5);
        assert_eq!(out.defender_strength, 4);

        // Attacker (§15.83c rounds raw points up): 25% of raw 50 = 12.5 -> 13 raw ->
        // 7 TOE (each infantry TOE absorbs its CA-off 2). In Contact; not captured.
        assert_eq!(out.attacker.loss_percent, 25);
        assert!(out.attacker.in_contact);
        assert_eq!(out.attacker_toe_removed, 7);
        assert_eq!(g.unit(atk).unwrap().total_toe(), 18);
        assert!(g.unit(atk).unwrap().status.in_contact);
        assert_eq!(out.attacker_prisoners, 0);
        assert!(out.attacker_prisoner_roll.is_none());

        // Defender: 20% of 20 = 4; Capt -> prisoners = round(4 * table% / 100).
        assert_eq!(out.defender.loss_percent, 20);
        assert!(out.defender.captured);
        assert_eq!(out.defender_toe_removed, 4);
        assert_eq!(g.unit(def).unwrap().total_toe(), 16);
        let roll = out.defender_prisoner_roll.expect("prisoner roll on Capt");
        let pris_ron = include_str!("../../../data/reference/crt/prisoners.ron");
        let mut pr = ReferenceData::new();
        pr.load_prisoners_table(pris_ron).unwrap();
        let pct = pr.prisoners_table.as_ref().unwrap().percent(roll) as u32;
        assert_eq!(out.defender_prisoners, (4 * pct + 50) / 100);
    }

    #[test]
    fn barrage_terrain_shift() {
        use crate::reference::crt::BarrageResult;
        use crate::state::game::GameState;
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, WeaponGroup};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_weapons_systems(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/weapons/commonwealth.ron"),
        )
        .unwrap();
        refs.load_barrage_crt(include_str!("../../../data/reference/crt/barrage.ron"))
            .unwrap();
        let mut g = GameState::new(refs, 1);

        // 15x 25-pdr -> 12 Actual Barrage points (column "11,12"). dice 21.
        let arty = g.spawn_unit(
            Nationality::Commonwealth,
            "x",
            "arty",
            Composition::Weapons {
                groups: vec![WeaponGroup { system: "25-pounder Gun".into(), count: 15 }],
            },
        );
        let inf = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "inf",
            Composition::Organic { current_toe: 10 },
        );

        // Target in Clear: column "11,12", dice 21 -> Pinned.
        let clear = HexCoord::new(0, 0);
        g.map.set_hex(clear, HexTerrain::Clear);
        g.unit_mut(inf).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(clear));
        let out = g.resolve_barrage_with_dice(&[arty], inf, 21);
        assert_eq!(out.terrain_shift, 0);
        assert_eq!(out.result, BarrageResult::Pinned);

        // Target in Rough (barrage Shift(-1)): column shifts to "9,10", dice 21 -> No effect.
        let rough = HexCoord::new(2, 0);
        g.map.set_hex(rough, HexTerrain::Rough);
        g.unit_mut(inf).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(rough));
        let out = g.resolve_barrage_with_dice(&[arty], inf, 21);
        assert_eq!(out.terrain_shift, -1);
        assert_eq!(out.result, BarrageResult::NoEffect);
        assert_eq!(out.toe_removed, 0);
    }

    fn close_assault_refs() -> ReferenceData {
        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_weapons_systems(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/weapons/commonwealth.ron"),
        )
        .unwrap();
        refs.load_close_assault_crt(include_str!(
            "../../../data/reference/crt/close-assault.ron"
        ))
        .unwrap();
        refs.load_morale_modifier_table(include_str!(
            "../../../data/reference/crt/morale-modifier.ron"
        ))
        .unwrap();
        refs.load_prisoners_table(include_str!("../../../data/reference/crt/prisoners.ron"))
            .unwrap();
        refs
    }

    #[test]
    fn close_assault_morale_column_shift() {
        use crate::state::game::GameState;
        use crate::state::unit::Composition;

        let mut g = GameState::new(close_assault_refs(), 1);
        // Equal strengths -> differential 0 -> base column 7.
        let atk = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "atk",
            Composition::Organic { current_toe: 10 },
        );
        let def = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "def",
            Composition::Organic { current_toe: 10 },
        );
        // Attacker morale +2 @ cohesion 0, roll 11 -> Delta(+1) -> adjusted +3.
        g.unit_mut(atk).unwrap().basic_morale = 2;
        // Defender morale 0 @ cohesion 0, roll 66 -> Delta(-1) -> adjusted -1.
        let out = g.resolve_close_assault_with_rolls(&[atk], &[def], 11, 66, 33, 33);
        assert_eq!(out.differential, 0);
        assert_eq!(out.base_column, 7);
        assert_eq!(out.attacker_morale.adjusted, 3);
        assert_eq!(out.defender_morale.adjusted, -1);
        // Shift = +3 - (-1) = +4 -> column 11.
        assert_eq!(out.morale_shift, 4);
        assert_eq!(out.adjusted_column, 11);
    }

    #[test]
    fn close_assault_surrender() {
        use crate::state::game::GameState;
        use crate::state::unit::Composition;

        let mut g = GameState::new(close_assault_refs(), 1);
        let atk = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "atk",
            Composition::Organic { current_toe: 10 },
        );
        let def = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "def",
            Composition::Organic { current_toe: 8 },
        );
        // Defender at Cohesion -17, Basic Morale 0: roll 11 -> Surrender (no §17.26 mercy).
        g.unit_mut(def).unwrap().cohesion = -17;
        let out = g.resolve_close_assault_with_rolls(&[atk], &[def], 33, 11, 33, 33);
        assert!(out.defender_morale.surrendered);
        assert_eq!(out.defender_toe_removed, 8);
        assert_eq!(out.defender_prisoners, 8);
        assert_eq!(g.unit(def).unwrap().total_toe(), 0);
        assert!(g.unit(def).unwrap().status.surrendered);
        // Attacker takes no losses in a surrender; is left In Contact.
        assert_eq!(out.attacker_toe_removed, 0);
        assert!(g.unit(atk).unwrap().status.in_contact);
    }

    #[test]
    fn close_assault_unit_size_shifts() {
        use crate::state::game::GameState;
        use crate::state::unit::{Composition, OrgSize};

        // §15.53: Division vs Battalion -> +4 columns; equal raw so no §15.51.
        let mut g = GameState::new(close_assault_refs(), 1);
        let atk = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "atk",
            Composition::Organic { current_toe: 10 },
        );
        let def = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "def",
            Composition::Organic { current_toe: 10 },
        );
        g.unit_mut(atk).unwrap().org_size = OrgSize::Division;
        // morale reads 33/33 -> no morale shift.
        let out = g.resolve_close_assault_with_rolls(&[atk], &[def], 33, 33, 33, 33);
        assert_eq!(out.differential, 0); // base column 7
        assert_eq!(out.size_shift, 4);
        assert_eq!(out.morale_shift, 0);
        assert_eq!(out.adjusted_column, 11);

        // §15.51: attacker with >=2x the defender's Raw CA points -> +2 (same org size).
        let mut g = GameState::new(close_assault_refs(), 1);
        let atk = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "atk",
            Composition::Organic { current_toe: 20 }, // raw off 40
        );
        let def = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "def",
            Composition::Organic { current_toe: 8 }, // raw def 16; 40 >= 32
        );
        let out = g.resolve_close_assault_with_rolls(&[atk], &[def], 33, 33, 33, 33);
        // actual 4 vs 2 -> differential 2 -> base column 9.
        assert_eq!(out.differential, 2);
        assert_eq!(out.base_column, 9);
        assert_eq!(out.size_shift, 2);
        assert_eq!(out.adjusted_column, 11);
    }

    #[test]
    fn close_assault_terrain_shift() {
        use crate::state::game::GameState;
        use crate::state::map::{HexCoord, HexsideKind};
        use crate::state::unit::Composition;

        let def_hex = HexCoord::new(0, 0);
        let atk_hex = HexCoord::new(1, 0);

        // Defender in a Mountain hex: close_assault Shift(-3). Equal strengths, no morale
        // or size shift -> base column 7 -> adjusted 4.
        let mut g = GameState::new(close_assault_refs(), 1);
        g.map.set_hex(def_hex, HexTerrain::Mountain);
        g.map.set_hex(atk_hex, HexTerrain::Clear);
        let atk = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "atk",
            Composition::Organic { current_toe: 10 },
        );
        let def = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "def",
            Composition::Organic { current_toe: 10 },
        );
        g.unit_mut(atk).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(atk_hex));
        g.unit_mut(def).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(def_hex));
        let out = g.resolve_close_assault_with_rolls(&[atk], &[def], 33, 33, 33, 33);
        assert_eq!(out.base_column, 7);
        assert_eq!(out.terrain_shift, -3);
        assert_eq!(out.adjusted_column, 4);

        // Attacking downhill into a Clear hex: DownSlope hexside close_assault Shift(+1).
        let mut g = GameState::new(close_assault_refs(), 1);
        g.map.set_hex(def_hex, HexTerrain::Clear);
        g.map.set_hex(atk_hex, HexTerrain::Clear);
        // high_side = attacker hex, so attacker->defender crossing is "down".
        g.map
            .set_hexside(atk_hex, def_hex, HexsideKind::Slope, Some(atk_hex));
        let atk = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "atk",
            Composition::Organic { current_toe: 10 },
        );
        let def = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "def",
            Composition::Organic { current_toe: 10 },
        );
        g.unit_mut(atk).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(atk_hex));
        g.unit_mut(def).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(def_hex));
        let out = g.resolve_close_assault_with_rolls(&[atk], &[def], 33, 33, 33, 33);
        assert_eq!(out.terrain_shift, 1);
        assert_eq!(out.adjusted_column, 8);

        // §15.35 two-terrain-hexside exception: when one crossed edge carries two terrain
        // features (a Wadi running along an up-Slope), the defender benefits from BOTH — the
        // edge's shift is the sum, not the single best. Measure each feature alone, then together.
        let build = |sides: &[(HexsideKind, Option<HexCoord>)]| -> i32 {
            let mut g = GameState::new(close_assault_refs(), 1);
            g.map.set_hex(def_hex, HexTerrain::Clear);
            g.map.set_hex(atk_hex, HexTerrain::Clear);
            for &(k, hs) in sides {
                g.map.set_hexside(atk_hex, def_hex, k, hs);
            }
            let a = g.spawn_unit(Nationality::Commonwealth, "h", "atk", Composition::Organic { current_toe: 10 });
            let d = g.spawn_unit(Nationality::Commonwealth, "h", "def", Composition::Organic { current_toe: 10 });
            g.unit_mut(a).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(atk_hex));
            g.unit_mut(d).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(def_hex));
            g.resolve_close_assault_with_rolls(&[a], &[d], 33, 33, 33, 33).terrain_shift
        };
        // Up-slope: high_side = defender hex (attacker crosses uphill → defender benefit).
        let wadi = build(&[(HexsideKind::Wadi, None)]);
        let slope = build(&[(HexsideKind::Slope, Some(def_hex))]);
        let both = build(&[(HexsideKind::Wadi, None), (HexsideKind::Slope, Some(def_hex))]);
        assert!(wadi < 0 && slope < 0, "both features favor the defender");
        assert_eq!(both, wadi + slope, "§15.35: two features on one edge both apply");
    }

    #[test]
    fn close_assault_combined_arms() {
        use crate::state::game::GameState;
        use crate::state::unit::{Composition, WeaponGroup};

        // Attacker: 2 unsupported tank TOE (Crusader Mk.III, CA off 4) -> raw off 8 ->
        // actual 1; combined arms: 2 unsupported -> ceil(2/3)=1 reduction -> actual 0.
        let mut g = GameState::new(close_assault_refs(), 1);
        let tanks = g.spawn_unit(
            Nationality::Commonwealth,
            "g",
            "tanks",
            Composition::Weapons {
                groups: vec![WeaponGroup { system: "Crusader Mk.III".into(), count: 2 }],
            },
        );
        // Defender: infantry "h" (CA def 2), TOE 10 -> actual 2.
        let def = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "def",
            Composition::Organic { current_toe: 10 },
        );
        let out = g.resolve_close_assault_with_rolls(&[tanks], &[def], 33, 33, 33, 33);
        assert_eq!(out.attacker_combined_arms, 1);
        assert_eq!(out.attacker_strength, 0);
        assert_eq!(out.differential, -2);

        // Supported: add 2 infantry TOE in the (same, unplaced) hex -> no reduction.
        let mut g = GameState::new(close_assault_refs(), 1);
        let tanks = g.spawn_unit(
            Nationality::Commonwealth,
            "g",
            "tanks",
            Composition::Weapons {
                groups: vec![WeaponGroup { system: "Crusader Mk.III".into(), count: 2 }],
            },
        );
        let inf = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "inf",
            Composition::Organic { current_toe: 2 },
        );
        let def = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "def",
            Composition::Organic { current_toe: 10 },
        );
        let out = g.resolve_close_assault_with_rolls(&[tanks, inf], &[def], 33, 33, 33, 33);
        assert_eq!(out.attacker_combined_arms, 0);
    }

    #[test]
    fn close_assault_zero_def_loner() {
        use crate::state::game::GameState;
        use crate::state::unit::{Composition, WeaponGroup};

        let mut g = GameState::new(close_assault_refs(), 1);
        // Attacker: infantry "h" (CA off 2), TOE 10 -> actual 2 -> base column 9 (+2).
        let atk = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "atk",
            Composition::Organic { current_toe: 10 },
        );
        // Defender: a lone 5.5" howitzer unit (Defensive Close Assault 0).
        let def = g.spawn_unit(
            Nationality::Commonwealth,
            "x",
            "medium arty",
            Composition::Weapons {
                groups: vec![WeaponGroup { system: "5.5\" Gun/Howitzer".into(), count: 3 }],
            },
        );
        let out = g.resolve_close_assault_with_rolls(&[atk], &[def], 33, 33, 33, 33);
        assert!(out.zero_def_loner);
        assert_eq!(out.differential, 2);
        assert_eq!(out.base_column, 9);
        // §15.54: +3 columns (no §15.51), so column 9 -> 12; attacker takes no casualties.
        assert_eq!(out.size_shift, 3);
        assert_eq!(out.adjusted_column, 12);
        assert_eq!(out.attacker_toe_removed, 0);
    }

    #[test]
    fn close_assault_rommel_morale_bonus() {
        use crate::state::game::GameState;
        use crate::state::unit::Composition;

        let mut g = GameState::new(close_assault_refs(), 1);
        let atk = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "atk",
            Composition::Organic { current_toe: 10 },
        );
        let def = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "def",
            Composition::Organic { current_toe: 10 },
        );
        // Attacker at Basic Morale +3, stacked with Rommel: cohesion 0 + roll 33 -> Delta 0
        // -> clamp(+3) = +3, then §17.28 +1 -> +4 (exceeds the normal +3 cap).
        g.unit_mut(atk).unwrap().basic_morale = 3;
        g.unit_mut(atk).unwrap().status.with_rommel = true;
        let out = g.resolve_close_assault_with_rolls(&[atk], &[def], 33, 33, 33, 33);
        assert_eq!(out.attacker_morale.adjusted, 4);
        assert_eq!(out.defender_morale.adjusted, 0);
        assert_eq!(out.morale_shift, 4);
    }

    #[test]
    fn close_assault_division_averaged_morale() {
        use crate::state::game::GameState;
        use crate::state::unit::{Composition, OrgSize};

        let mut g = GameState::new(close_assault_refs(), 1);
        // Two attacking divisions: morale +3/+1 -> avg +2; cohesion -4/-2 -> avg -3.
        let d1 = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "div1",
            Composition::Organic { current_toe: 10 },
        );
        let d2 = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "div2",
            Composition::Organic { current_toe: 10 },
        );
        let def = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "def",
            Composition::Organic { current_toe: 10 },
        );
        for (id, m, c) in [(d1, 3, -4), (d2, 1, -2)] {
            let u = g.unit_mut(id).unwrap();
            u.org_size = OrgSize::Division;
            u.basic_morale = m;
            u.cohesion = c;
        }
        // roll 33 at cohesion -3 -> row -3 "NO" band -> Delta 0 -> adjusted = clamp(+2) = 2.
        let out = g.resolve_close_assault_with_rolls(&[d1, d2], &[def], 33, 33, 33, 33);
        assert_eq!(out.attacker_morale.basic, 2);
        assert_eq!(out.attacker_morale.cohesion, -3);
        assert_eq!(out.attacker_morale.adjusted, 2);
    }

    #[test]
    fn combat_commands_through_the_boundary() {
        use crate::reference::crt::BarrageTargetClass;
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, WeaponGroup};

        let mut refs = close_assault_refs();
        refs.load_barrage_crt(include_str!("../../../data/reference/crt/barrage.ron"))
            .unwrap();
        let mut g = GameState::new(refs, 5);

        let hex_d = HexCoord::new(0, 0);
        let hex_a = HexCoord::new(1, 0);
        g.map.set_hex(hex_d, HexTerrain::Clear);
        g.map.set_hex(hex_a, HexTerrain::Clear);

        let def = g.spawn_unit(
            Nationality::Italian,
            "p",
            "def",
            Composition::Organic { current_toe: 10 },
        );
        g.unit_mut(def).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(hex_d));
        let atk = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "atk",
            Composition::Organic { current_toe: 12 },
        );
        g.unit_mut(atk).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(hex_a));
        let arty = g.spawn_unit(
            Nationality::Commonwealth,
            "x",
            "arty",
            Composition::Weapons {
                groups: vec![WeaponGroup { system: "25-pounder Gun".into(), count: 8 }],
            },
        );
        g.unit_mut(arty).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(hex_a));

        // Combat is illegal outside the Movement & Combat phase.
        assert_eq!(
            g.apply(Command::Barrage { attackers: vec![arty], target: hex_d, target_class: BarrageTargetClass::Infantry, target_unit: None }),
            Err(CommandError::WrongPhase)
        );

        g.phase = Phase::MovementAndCombat;

        // Empty attacker list is rejected.
        assert_eq!(
            g.apply(Command::CloseAssault { attackers: vec![], target: hex_d }),
            Err(CommandError::NoParticipants)
        );

        // Barrage resolves through the boundary.
        assert!(matches!(
            g.apply(Command::Barrage { attackers: vec![arty], target: hex_d, target_class: BarrageTargetClass::Infantry, target_unit: None }),
            Ok(CommandOutcome::Barrage(_))
        ));

        // Adjacent close assault resolves.
        assert!(matches!(
            g.apply(Command::CloseAssault { attackers: vec![atk], target: hex_d }),
            Ok(CommandOutcome::CloseAssault(_))
        ));

        // A non-adjacent attacker is rejected.
        let far = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "far",
            Composition::Organic { current_toe: 10 },
        );
        g.unit_mut(far).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(HexCoord::new(5, 5)));
        assert_eq!(
            g.apply(Command::CloseAssault { attackers: vec![far], target: hex_d }),
            Err(CommandError::NotAdjacent)
        );

        // Barrage from a non-adjacent attacker is rejected (§12.31 adjacency).
        assert_eq!(
            g.apply(Command::Barrage { attackers: vec![far], target: hex_d, target_class: BarrageTargetClass::Infantry, target_unit: None }),
            Err(CommandError::NotAdjacent)
        );

        // Anti-armor at the adjacent attacked hex resolves; from far it is rejected.
        assert!(matches!(
            g.apply(Command::AntiArmor { attackers: vec![atk], target_hex: hex_d, withheld: vec![] }),
            Ok(CommandOutcome::AntiArmor(_))
        ));
        assert_eq!(
            g.apply(Command::AntiArmor { attackers: vec![far], target_hex: hex_d, withheld: vec![] }),
            Err(CommandError::NotAdjacent)
        );
    }

    #[test]
    fn barrage_specific_target_designation_12_23() {
        use crate::reference::crt::BarrageTargetClass;
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation, WeaponGroup};

        let mut refs = close_assault_refs();
        refs.load_barrage_crt(include_str!("../../../data/reference/crt/barrage.ron")).unwrap();
        let mut g = GameState::new(refs, 5);
        g.phase = Phase::MovementAndCombat;
        let hex_d = HexCoord::new(0, 0);
        let hex_a = HexCoord::new(1, 0);
        g.map.set_hex(hex_d, HexTerrain::Clear);
        g.map.set_hex(hex_a, HexTerrain::Clear);

        // Two Italian infantry in the target hex; one is In-Contact (known via previous combat).
        let known = g.spawn_unit(Nationality::Italian, "p", "known", Composition::Organic { current_toe: 10 });
        let hidden = g.spawn_unit(Nationality::Italian, "p", "hidden", Composition::Organic { current_toe: 10 });
        for id in [known, hidden] {
            g.unit_mut(id).unwrap().location = Some(UnitLocation::OnMap(hex_d));
        }
        g.unit_mut(known).unwrap().status.in_contact = true;
        let arty = g.spawn_unit(
            Nationality::Commonwealth, "x", "arty",
            Composition::Weapons { groups: vec![WeaponGroup { system: "25-pounder Gun".into(), count: 8 }] },
        );
        g.unit_mut(arty).unwrap().location = Some(UnitLocation::OnMap(hex_a));

        // §12.23: designating the In-Contact unit is allowed and hits exactly that unit.
        let before_known = g.unit(known).unwrap().total_toe();
        let before_hidden = g.unit(hidden).unwrap().total_toe();
        match g.apply(Command::Barrage { attackers: vec![arty], target: hex_d, target_class: BarrageTargetClass::Infantry, target_unit: Some(known) }) {
            Ok(CommandOutcome::Barrage(_)) => {}
            other => panic!("expected barrage, got {other:?}"),
        }
        assert!(g.unit(known).unwrap().total_toe() <= before_known);
        assert_eq!(g.unit(hidden).unwrap().total_toe(), before_hidden, "the un-designated unit is untouched");

        // §12.23: designating a unit the firer does not know is present is rejected.
        assert_eq!(
            g.apply(Command::Barrage { attackers: vec![arty], target: hex_d, target_class: BarrageTargetClass::Infantry, target_unit: Some(hidden) }),
            Err(CommandError::TargetNotKnown)
        );

        // Designating a unit of the wrong class (none here) → NoTargets.
        let gun = g.spawn_unit(Nationality::Italian, "x", "gun", Composition::Weapons { groups: vec![WeaponGroup { system: "25-pounder Gun".into(), count: 2 }] });
        g.unit_mut(gun).unwrap().location = Some(UnitLocation::OnMap(hex_d));
        assert_eq!(
            g.apply(Command::Barrage { attackers: vec![arty], target: hex_d, target_class: BarrageTargetClass::Infantry, target_unit: Some(gun) }),
            Err(CommandError::NoTargets)
        );
    }

    #[test]
    fn load_scenario_places_units() {
        use crate::reference::units::Side;
        use crate::state::game::GameState;
        use crate::state::map::HexId;
        use crate::state::unit::OrgSize;

        let scenario = r#"(
            name: "Test Offensive",
            game_turn: 1,
            first_turn_initiative: Some(Axis),
            units: [
                ( nationality: Italian, id_code: "p", name: "2 Libyan Div",
                  org_size: Division, hex: "C3920", composition: Organic(current_toe: 20) ),
                ( nationality: Commonwealth, id_code: "h", name: "2nd Rifle Bde",
                  org_size: StandardBrigade, hex: "C3320",
                  composition: Organic(current_toe: 8), basic_morale: 2 ),
            ],
        )"#;
        let mut g = GameState::new(ReferenceData::new(), 1);
        g.load_scenario(scenario).unwrap();
        assert_eq!(g.units.len(), 2);
        assert_eq!(g.game_turn, 1);
        assert_eq!(g.first_turn_initiative, Some(Side::Axis));

        let libyan = g.units.values().find(|u| u.name == "2 Libyan Div").unwrap();
        assert_eq!(libyan.hex(), Some(HexId::parse("C3920").unwrap().to_hex_coord()));
        assert_eq!(libyan.org_size, OrgSize::Division);
        assert_eq!(libyan.total_toe(), 20);

        // §7.15 predetermined first-turn initiative applies on the first advance.
        g.advance_phase();
        assert_eq!(g.initiative, Side::Axis);
    }

    #[test]
    fn air_reinforcements_split_and_accrue_into_pool() {
        use crate::reference::units::Nationality;
        use crate::state::game::GameState;

        let refs = ReferenceData::load_campaign();
        let sched = refs.air_reinforcements.as_ref().expect("air reinforcement schedule loads");

        // GT2 is a single-turn period: the CW gets its full 2× Hurricane I + 3× Fulmar.
        let a2 = sched.arrivals_for(2);
        assert!(a2.iter().any(|(n, ac, c)| *n == Nationality::Commonwealth && ac == "Hurricane Mk.I" && *c == 2));
        // A four-Game-Turn period splits evenly: 1941 Jan (GT15–18), 44× Hurricane I → 11/turn.
        assert!(sched.arrivals_for(15).iter().any(|(_, ac, c)| ac == "Hurricane Mk.I" && *c == 11));
        // …and the per-turn splits sum back to the monthly total.
        let total: u32 = (15..=18)
            .flat_map(|gt| sched.arrivals_for(gt))
            .filter(|(_, ac, _)| ac == "Hurricane Mk.I")
            .map(|(_, _, c)| c)
            .sum();
        assert_eq!(total, 44);

        // The game accrues the current Game-Turn's planes into the side's pool.
        let mut g = GameState::new(ReferenceData::load_campaign(), 1);
        g.game_turn = 2;
        g.accrue_air_reinforcements();
        assert_eq!(g.pool_planes(Nationality::Commonwealth, "Hurricane Mk.I"), 2);
        assert_eq!(g.pool_planes(Nationality::Italian, "Ba 65"), 12);

        // §34.84 WD: squadron withdrawals fire once, at the month's gt_start. GT8 = 2F + 2B (CW).
        use crate::reference::squadron::WdClass;
        let wd8 = sched.withdrawals_for(8);
        assert!(wd8.contains(&(Nationality::Commonwealth, WdClass::Fighter, 2)));
        assert!(wd8.contains(&(Nationality::Commonwealth, WdClass::Bomber, 2)));
        // GT39 withdraws a recon squadron; a mid-period turn (GT40) fires nothing.
        assert_eq!(sched.withdrawals_for(39), vec![(Nationality::Commonwealth, WdClass::Recon, 1)]);
        assert!(sched.withdrawals_for(40).is_empty());
        // The Axis chart lists no withdrawals at all.
        assert!((1..=110).flat_map(|gt| sched.withdrawals_for(gt)).all(|(n, _, _)| n == Nationality::Commonwealth));
    }

    #[test]
    fn load_scenario_deploys_air_forces() {
        use crate::state::air::{AirFacilityKind, MedBase};
        use crate::state::game::GameState;
        // On-map fighter squadron + off-map (Med Box) bomber squadron + two air facilities.
        let scenario = r#"(
            name: "Air Test",
            game_turn: 1,
            squadrons: [
                ( nationality: Commonwealth, class: Fighter, designation: "33 Sqn", cpa: 5,
                  at: Some("C4131"), planes: [ ( aircraft_type: "Hurricane Mk.I", count: 12 ) ] ),
                ( nationality: German, class: Bomber, designation: "KG 26", cpa: 5,
                  med_base: Some(ItalySicily), planes: [ ( aircraft_type: "He.111", count: 9 ) ] ),
            ],
            air_facilities: [
                ( at: "C4131", kind: Airfield ),
                ( at: "C4807", kind: AirLandingStrip, capacity: Some(1) ),
            ],
        )"#;
        let mut g = GameState::new(ReferenceData::new(), 1);
        g.load_scenario(scenario).unwrap();

        assert_eq!(g.sgsus.len(), 2);
        // On-map fighter: located, planes start ready + armed (flyable).
        let cw = g.sgsus.values().find(|s| s.designation == "33 Sqn").unwrap();
        assert!(cw.location.is_some() && cw.med_base.is_none());
        assert_eq!((cw.planes[0].count, cw.planes[0].ready, cw.planes[0].armed), (12, 12, 12));
        // Off-map bomber: in the Italy/Sicily Box, no on-map hex.
        let de = g.sgsus.values().find(|s| s.designation == "KG 26").unwrap();
        assert_eq!(de.med_base, Some(MedBase::ItalySicily));
        assert!(de.location.is_none());
        // Facilities: airfield defaults to its max capacity; strip is explicit 1.
        assert_eq!(g.air_facilities.len(), 2);
        let airfield = g.air_facilities.iter().find(|f| matches!(f.kind, AirFacilityKind::Airfield)).unwrap();
        assert!(airfield.capacity_level >= 1);
    }

    #[test]
    fn scenario_1_loads_on_section_c() {
        use crate::reference::units::Side;
        use crate::state::game::GameState;
        use crate::state::map::HexId;

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_unit_characteristics(
            Nationality::Italian,
            include_str!("../../../data/reference/unit-characteristics/italian.ron"),
        )
        .unwrap();
        refs.load_weapons_systems(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/weapons/commonwealth.ron"),
        )
        .unwrap();
        refs.load_weapons_systems(
            Nationality::Italian,
            include_str!("../../../data/reference/weapons/italian.ron"),
        )
        .unwrap();
        refs.load_order_of_battle(
            Nationality::Italian,
            include_str!("../../../data/ob/italian.ron"),
        )
        .unwrap();
        refs.load_order_of_battle(
            Nationality::Commonwealth,
            include_str!("../../../data/ob/commonwealth.ron"),
        )
        .unwrap();
        refs.load_off_map_nodes(include_str!(
            "../../../data/reference/off-map-nodes.ron"
        ))
        .unwrap();
        let mut g = GameState::new(refs, 1);
        for s in [
            include_str!("../../../data/maps/section-a.ron"),
            include_str!("../../../data/maps/section-b.ron"),
            include_str!("../../../data/maps/section-c.ron"),
            include_str!("../../../data/maps/section-d.ron"),
            include_str!("../../../data/maps/section-e.ron"),
        ] {
            g.map.load_section(s).unwrap();
        }
        g.load_scenario(include_str!("../../../data/scenarios/italian-offensive.ron"))
            .unwrap();

        // 16 flat units + three Italian OB divisions (13 each) + CW 7th Armoured (13) +
        // CW 4th Indian (12 deployed of 17; 7th Ind Bde & 1st Fld Arty arrive later) via placements.
        assert_eq!(g.units.len(), 80);
        // A deployed OB division is a real hierarchy: its HQ heads a 13-unit formation.
        let cat_hq = g.units.values().find(|u| u.name == "64th Catanzaro Div HQ").unwrap().id;
        assert_eq!(g.formation_members(cat_hq).len(), 13);
        // Maletti Group starts at C3617, on a real (loaded) map hex.
        let maletti = g.units.values().find(|u| u.name == "Maletti Group").unwrap();
        let hex = HexId::parse("C3617").unwrap().to_hex_coord();
        assert_eq!(maletti.hex(), Some(hex));
        assert!(g.map.terrain(hex).is_some());
        // A CW tank regiment (deployed from the 7th Armoured OB) has real weapons strength
        // from the loaded charts, and is dispersed forward to its §60.41 screen hex.
        let rtr = g.units.values().find(|u| u.name == "1st Royal Tank Regt").unwrap();
        assert!(rtr.raw_strength(&g.reference, crate::state::unit::RatingKind::AntiArmor) > 0);
        assert_eq!(rtr.hex(), Some(HexId::parse("C3520").unwrap().to_hex_coord()));
        // Division main bodies deploy off section C onto real section-D terrain (§60.41).
        let armd_hq = g.units.values().find(|u| u.name == "7th Armored Div HQE").unwrap();
        assert_eq!(armd_hq.hex(), Some(HexId::parse("D3612").unwrap().to_hex_coord()));
        assert!(g.map.terrain(armd_hq.hex().unwrap()).is_some());
        let ind_hq = g.units.values().find(|u| u.name == "4th Indian Div HQ").unwrap();
        assert_eq!(ind_hq.hex(), Some(HexId::parse("D3615").unwrap().to_hex_coord()));
        assert!(g.map.terrain(ind_hq.hex().unwrap()).is_some());
        // 31st Field Arty is detached forward from 4th Indian to C4131.
        let fld31 = g.units.values().find(|u| u.name == "31st Field Artillery Regt").unwrap();
        assert_eq!(fld31.hex(), Some(HexId::parse("C4131").unwrap().to_hex_coord()));

        // Fixed initial Supply (§60.34/§60.44): 6 Axis + 4 CW = 10 dumps at named locations;
        // the 5 player-choice groups (Dump 1/2, Axis dummies, Dump I, CW dummy) are pending.
        assert_eq!(g.supply_dumps.len(), 10);
        assert_eq!(g.pending_dump_choices.len(), 5);
        // Tobruk (C4807) is one Axis dump with 200 Ammo / 2000 Fuel / 500 Stores (§60.34).
        let tobruk = HexId::parse("C4807").unwrap().to_hex_coord();
        let tobruk_dumps: Vec<_> = g
            .supply_dumps
            .iter()
            .filter(|d| d.side == Side::Axis && !d.dummy && d.location.hex() == Some(tobruk))
            .collect();
        assert_eq!(tobruk_dumps.len(), 1);
        let td = tobruk_dumps[0];
        assert_eq!((td.supply.ammo, td.supply.fuel, td.supply.stores), (200, 2000, 500));
        // The Tripoli box is one off-map Axis dump (250 / 5000 / 250).
        let tripoli = g
            .supply_dumps
            .iter()
            .filter(|d| matches!(&d.location, crate::state::unit::UnitLocation::OffMap { node, .. } if node == "Tripoli"))
            .count();
        assert_eq!(tripoli, 1);

        // Auto-placing the 5 player-choice groups (3 real + 3 dummy units) → 16 total. CW
        // Dump I (§60.44) must be in Egypt on map C or D (east of Sollum C4021, west of E).
        let sollum_col = HexId::parse("C4021").unwrap().to_hex_coord().col;
        g.auto_place_dump_choices();
        assert_eq!(g.supply_dumps.len(), 16);
        assert!(g.pending_dump_choices.is_empty());
        let cw_dump_i: Vec<_> = g
            .supply_dumps
            .iter()
            .filter(|d| d.side == Side::Commonwealth && !d.dummy && d.supply.fuel == 750)
            .collect();
        assert_eq!(cw_dump_i.len(), 1);
        let h = cw_dump_i[0].location.hex().unwrap();
        let sec = crate::state::map::section_char_of_col(h.col);
        assert!((sec == 'C' || sec == 'D') && h.col > sollum_col);

        // §60.31 first-line trucks load onto the units: 1 CCNN Division has 10 Light / 25
        // Medium / 10 Heavy Truck Points.
        let ccnn = g.units.values().find(|u| u.name == "1 CCNN Division").unwrap();
        assert_eq!((ccnn.trucks.light, ccnn.trucks.medium, ccnn.trucks.heavy), (10, 25, 10));

        // §60.31/§60.44 Repair Facilities: Tobruk Temporary, Alexandria Major.
        use crate::state::game::FacilityKind;
        assert_eq!(
            g.repair_facilities.get(&HexId::parse("C4807").unwrap().to_hex_coord()),
            Some(&FacilityKind::Temporary)
        );
        assert_eq!(
            g.repair_facilities.get(&HexId::parse("E3613").unwrap().to_hex_coord()),
            Some(&FacilityKind::Major)
        );

        // The Italian offensive: Axis has the predetermined initiative. (Advancing the turn
        // also runs the §51 Stores Expenditure Stage, which draws from the dumps above.)
        g.advance_phase();
        assert_eq!(g.initiative, Side::Axis);
    }

    /// Build the full engine for the Campaign Game (§64): reference bootstrap + all five map
    /// sections + the campaign scenario + player-choice dump placement.
    #[cfg(test)]
    fn build_campaign_game() -> crate::state::game::GameState {
        use crate::state::game::GameState;
        let mut g = GameState::new(ReferenceData::load_campaign(), 7);
        for s in [
            include_str!("../../../data/maps/section-a.ron"),
            include_str!("../../../data/maps/section-b.ron"),
            include_str!("../../../data/maps/section-c.ron"),
            include_str!("../../../data/maps/section-d.ron"),
            include_str!("../../../data/maps/section-e.ron"),
        ] {
            g.map.load_section(s).unwrap();
        }
        g.load_scenario(include_str!("../../../data/scenarios/campaign.ron"))
            .expect("campaign scenario loads (all formation names resolve against the OBs)");
        g.auto_place_dump_choices();
        g
    }

    #[test]
    fn campaign_seeds_initial_air_pool() {
        use crate::reference::units::Nationality;
        // §60.32/§60.42 starting plane pools are seeded at GT1 (before any reinforcement accrual).
        let g = build_campaign_game();
        assert_eq!(g.pool_planes(Nationality::Italian, "S.M.79 Sparviero"), 133);
        assert_eq!(g.pool_planes(Nationality::Italian, "C.R.32"), 70);
        assert_eq!(g.pool_planes(Nationality::Commonwealth, "Gladiator Mk.II"), 36);
        assert_eq!(g.pool_planes(Nationality::Commonwealth, "Hurricane Mk.I"), 6);
        // The Luftwaffe is not present at Sept-1940 start (arrives with the DAK, GT26+).
        let german: u32 = g.plane_pool.get(&Nationality::German).map_or(0, |m| m.values().sum());
        assert_eq!(german, 0);
        // §60.32/§60.42 pilot pools: Italian 3/10/15 and Commonwealth 1/5/8 by rating, no Germans.
        assert_eq!(g.pool_pilots(Nationality::Italian, 3), 3);
        assert_eq!(g.pool_pilots(Nationality::Italian, 2), 10);
        assert_eq!(g.pool_pilots(Nationality::Italian, 1), 15);
        assert_eq!(g.pool_pilots(Nationality::Commonwealth, 3), 1);
        assert_eq!(g.pool_pilots(Nationality::Commonwealth, 2), 5);
        assert_eq!(g.pool_pilots(Nationality::Commonwealth, 1), 8);
        assert!(g.pilot_pool.get(&Nationality::German).is_none_or(|m| m.values().sum::<u32>() == 0));
        // §60.32/§60.42 available SGSU counters: 39 Italian, 14 Commonwealth, none German at start.
        assert_eq!(g.available_sgsu.get(&Nationality::Italian).copied().unwrap_or(0), 39);
        assert_eq!(g.available_sgsu.get(&Nationality::Commonwealth).copied().unwrap_or(0), 14);
        assert_eq!(g.available_sgsu.get(&Nationality::German).copied().unwrap_or(0), 0);
    }

    #[test]
    fn sgsu_in_play_cap_34_82() {
        use crate::reference::aircraft::AircraftClass;
        use crate::reference::squadron::SquadronClass;
        use crate::state::air::{AirFacility, AirFacilityKind};
        use crate::state::command::{Command, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;

        let mut g = GameState::new(ReferenceData::new(), 1);
        let field = HexCoord::new(4, 4);
        g.add_air_facility(AirFacility::new(AirFacilityKind::Airfield, field));
        g.available_sgsu.insert(Nationality::Commonwealth, 5);
        g.available_sgsu.insert(Nationality::Italian, 5);
        g.plane_pool.entry(Nationality::Commonwealth).or_default().insert("Gladiator Mk.II".into(), 12);
        g.plane_pool.entry(Nationality::Italian).or_default().insert("C.R.42 Falco".into(), 10);
        while g.phase != Phase::Organization {
            g.advance_phase();
        }

        // Commonwealth: with 0 on-map planes the cap is 0/12 + 2 = 2.
        assert_eq!(g.sgsu_plane_cap(Nationality::Commonwealth), 2);
        let mut cw = vec![];
        for _ in 0..3 {
            if let Ok(CommandOutcome::SgsuDeployed(id)) =
                g.apply(Command::DeploySgsu { nationality: Nationality::Commonwealth, class: SquadronClass::Fighter, at: field })
            {
                cw.push(id);
            }
        }
        // Three empty SGSUs exceed the 2 cap (deploy itself is not gated).
        assert_eq!(g.sgsus_in_play(Nationality::Commonwealth), 3);
        assert!(g.sgsu_cap_exceeded(Nationality::Commonwealth));
        // Base 12 planes: cap rises to ⌈12/12⌉ + 2 = 3, and the invariant is satisfied.
        g.apply(Command::AssignSquadronPlanes { squadron: cw[0], aircraft_type: "Gladiator Mk.II".into(), class: AircraftClass::Fighter, count: 12 }).unwrap();
        assert_eq!(g.on_map_facility_planes(Nationality::Commonwealth), 12);
        assert_eq!(g.sgsu_plane_cap(Nationality::Commonwealth), 3);
        assert!(!g.sgsu_cap_exceeded(Nationality::Commonwealth));

        // Axis has no +2 cushion: ⌈planes/10⌉. One empty Italian SGSU exceeds the 0 cap…
        let it = match g.apply(Command::DeploySgsu { nationality: Nationality::Italian, class: SquadronClass::Fighter, at: field }) {
            Ok(CommandOutcome::SgsuDeployed(id)) => id,
            other => panic!("expected SgsuDeployed, got {other:?}"),
        };
        assert_eq!(g.sgsu_plane_cap(Nationality::Italian), 0);
        assert!(g.sgsu_cap_exceeded(Nationality::Italian));
        // …basing 10 planes lifts the cap to ⌈10/10⌉ = 1, exactly matching the one SGSU.
        g.apply(Command::AssignSquadronPlanes { squadron: it, aircraft_type: "C.R.42 Falco".into(), class: AircraftClass::Fighter, count: 10 }).unwrap();
        assert_eq!(g.sgsu_plane_cap(Nationality::Italian), 1);
        assert!(!g.sgsu_cap_exceeded(Nationality::Italian));
    }

    #[test]
    fn commonwealth_off_map_basing_and_34_82c() {
        use crate::reference::aircraft::AircraftClass;
        use crate::reference::squadron::SquadronClass;
        use crate::state::air::{Sgsu, SquadronId};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;

        // A small custom §36.53 chart so the §34.82C threshold is reachable in a test: one N. African
        // box (capacity 1), a flying-boat basin (marked non-N.African here to isolate the threshold),
        // and a GT35-gated East African box. (The real 48-squadron chart is tested in squadron.rs.)
        let mut refs = ReferenceData::new();
        refs.load_aircraft_chart(Nationality::Commonwealth, include_str!("../../../data/reference/aircraft/commonwealth.ron")).unwrap();
        refs.load_off_map_air_facilities(r#"(facilities: [
            (entry_hex: "E1833", name: "Deversoir", distance: 12, max_squadrons: Some(1), north_african: true),
            (entry_hex: "E4033", name: "Port Said", distance: 12, max_squadrons: Some(3), flying_boat: true, north_african: false),
            (entry_hex: "E0127", name: "Ethiopia", distance: 60, max_squadrons: None, available_from_gt: Some(35), north_african: false),
        ])"#).unwrap();
        let mut g = GameState::new(refs, 1);

        // A CW fighter squadron with a full 20-plane complement (15 ready + 5 reserve, §35.23).
        let mut fighter = Sgsu::new(SquadronId(1), Nationality::Commonwealth, SquadronClass::Fighter, "33 Sqn", 25, Some(HexCoord::new(2, 2)));
        fighter.add_planes("Gladiator Mk.II", AircraftClass::Fighter, 20, 1);
        g.sgsus.insert(SquadronId(1), fighter);
        // A flying-boat squadron and a second fighter.
        let mut boats = Sgsu::new(SquadronId(2), Nationality::Commonwealth, SquadronClass::Bomber, "230 Sqn", 25, Some(HexCoord::new(2, 2)));
        boats.add_planes("Sunderland", AircraftClass::FlyingBoat, 3, 1);
        g.sgsus.insert(SquadronId(2), boats);
        g.sgsus.insert(SquadronId(3), Sgsu::new(SquadronId(3), Nationality::Commonwealth, SquadronClass::Fighter, "112 Sqn", 25, Some(HexCoord::new(2, 2))));
        while g.phase != Phase::Organization {
            g.advance_phase();
        }

        // Threshold: 1 N. African squadron slot × 20 (CW ready+reserve) = 20 planes.
        assert_eq!(g.max_off_map_north_african_planes(), Some(20));
        assert!(!g.may_create_extra_sgsu(Nationality::Commonwealth));

        // Base the 20-plane fighter squadron at Deversoir.
        assert_eq!(g.apply(Command::BaseSquadronOffMap { squadron: SquadronId(1), facility: "Deversoir".into() }), Ok(CommandOutcome::SquadronBasedOffMap));
        assert_eq!(g.sgsus[&SquadronId(1)].off_map_box.as_deref(), Some("Deversoir"));
        assert_eq!(g.sgsus[&SquadronId(1)].location, None);
        assert_eq!(g.off_map_north_african_planes(), 20);
        // Now the North African off-map facilities are at printed capacity → §34.82C is satisfied.
        assert!(g.may_create_extra_sgsu(Nationality::Commonwealth));

        // Deversoir is full (capacity 1) — a second squadron there is refused.
        assert_eq!(g.apply(Command::BaseSquadronOffMap { squadron: SquadronId(3), facility: "Deversoir".into() }), Err(CommandError::OffMapBasingFailed));
        // A fighter squadron cannot use the Port Said flying-boat basin; a flying-boat squadron can.
        assert_eq!(g.apply(Command::BaseSquadronOffMap { squadron: SquadronId(3), facility: "Port Said".into() }), Err(CommandError::OffMapBasingFailed));
        assert_eq!(g.apply(Command::BaseSquadronOffMap { squadron: SquadronId(2), facility: "Port Said".into() }), Ok(CommandOutcome::SquadronBasedOffMap));
        // Ethiopia is gated until Game-Turn 35.
        assert_eq!(g.apply(Command::BaseSquadronOffMap { squadron: SquadronId(3), facility: "Ethiopia".into() }), Err(CommandError::OffMapBasingFailed));
        g.game_turn = 35;
        assert_eq!(g.apply(Command::BaseSquadronOffMap { squadron: SquadronId(3), facility: "Ethiopia".into() }), Ok(CommandOutcome::SquadronBasedOffMap));
    }

    #[test]
    fn axis_may_create_extra_sgsu_when_30pct_off_map_34_82d() {
        use crate::reference::aircraft::AircraftClass;
        use crate::reference::squadron::SquadronClass;
        use crate::state::air::{AirFacility, AirFacilityKind, MedBase, Sgsu, SquadronId};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;

        let mut g = GameState::new(ReferenceData::new(), 1);
        let field = HexCoord::new(3, 3);
        g.add_air_facility(AirFacility::new(AirFacilityKind::Airfield, field));
        // German counter supply already exhausted (0 available).
        g.available_sgsu.insert(Nationality::German, 0);
        g.plane_pool.entry(Nationality::German).or_default().insert("Bf.109F".into(), 20);
        while g.phase != Phase::Organization {
            g.advance_phase();
        }

        // No German planes in play yet → cannot create (§34.82D 30%-off-map fails on 0 total).
        assert!(!g.may_create_extra_sgsu(Nationality::German));
        assert_eq!(
            g.apply(Command::DeploySgsu { nationality: Nationality::German, class: SquadronClass::Fighter, at: field }),
            Err(CommandError::SgsuDeployFailed)
        );

        // Put a 10-plane German squadron in an off-map Med box → 100% off-map, condition met.
        let mut med = Sgsu::new(SquadronId(50), Nationality::German, SquadronClass::Bomber, "KG 26", 25, None);
        med.med_base = Some(MedBase::ItalySicily);
        med.add_planes("Ju.88c", AircraftClass::Bomber, 10, 1);
        g.sgsus.insert(SquadronId(50), med);
        assert!(g.may_create_extra_sgsu(Nationality::German));
        // Now an extra SGSU may be created despite the empty supply; supply stays at 0.
        assert!(matches!(
            g.apply(Command::DeploySgsu { nationality: Nationality::German, class: SquadronClass::Fighter, at: field }),
            Ok(CommandOutcome::SgsuDeployed(_))
        ));
        assert_eq!(g.available_sgsu[&Nationality::German], 0);
    }

    #[test]
    fn withdraw_squadron_satisfies_due_wd_34_84() {
        use crate::reference::aircraft::AircraftClass;
        use crate::reference::squadron::{SquadronClass, WdClass};
        use crate::state::air::{AirFacility, AirFacilityKind};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;

        // Campaign ReferenceData carries the §34.84 schedule (GT8 CW WD = 2F + 2B).
        let mut g = GameState::new(ReferenceData::load_campaign(), 1);
        let field = HexCoord::new(7, 7);
        g.add_air_facility(AirFacility::new(AirFacilityKind::Airfield, field));
        g.available_sgsu.insert(Nationality::Commonwealth, 1);
        g.plane_pool.entry(Nationality::Commonwealth).or_default().insert("Gladiator Mk.II".into(), 12);
        while g.phase != Phase::Organization {
            g.advance_phase();
        }
        // Deploy a CW fighter squadron and fill it from the pool.
        let id = match g.apply(Command::DeploySgsu { nationality: Nationality::Commonwealth, class: SquadronClass::Fighter, at: field }) {
            Ok(CommandOutcome::SgsuDeployed(id)) => id,
            other => panic!("expected SgsuDeployed, got {other:?}"),
        };
        g.apply(Command::AssignSquadronPlanes { squadron: id, aircraft_type: "Gladiator Mk.II".into(), class: AircraftClass::Fighter, count: 12 }).unwrap();
        assert_eq!(g.pool_planes(Nationality::Commonwealth, "Gladiator Mk.II"), 0);
        assert_eq!(g.available_sgsu[&Nationality::Commonwealth], 0);

        // At GT1 no WD is due — withdrawal is refused.
        assert_eq!(g.apply(Command::WithdrawSquadron { squadron: id }), Err(CommandError::WithdrawalNotDue));

        // GT8 is a period start with a CW fighter WD — now it succeeds.
        g.game_turn = 8;
        assert!(g.due_squadron_withdrawals().iter().any(|&(n, c, _)| n == Nationality::Commonwealth && c == WdClass::Fighter));
        assert_eq!(g.apply(Command::WithdrawSquadron { squadron: id }), Ok(CommandOutcome::SquadronWithdrawn));
        // The squadron is gone; its counter returned to supply and its planes to the pool; no pilots
        // are left dangling on the removed squadron.
        assert!(!g.sgsus.contains_key(&id));
        assert_eq!(g.available_sgsu[&Nationality::Commonwealth], 1);
        assert_eq!(g.pool_planes(Nationality::Commonwealth, "Gladiator Mk.II"), 12);
        assert!(g.pilots.iter().all(|p| p.squadron != Some(id)));
    }

    #[test]
    fn deploy_sgsu_spends_counter_and_sets_cpa_34_82() {
        use crate::reference::squadron::SquadronClass;
        use crate::state::air::{AirFacility, AirFacilityKind};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;

        let mut g = GameState::new(ReferenceData::new(), 1);
        let field = HexCoord::new(6, 6);
        g.add_air_facility(AirFacility::new(AirFacilityKind::Airfield, field));
        g.available_sgsu.insert(Nationality::Italian, 2);
        while g.phase != Phase::Organization {
            g.advance_phase();
        }
        // Deploy at the airfield: spends a counter, and the SGSU carries the §35.12 Italian CPA (20).
        let out = g.apply(Command::DeploySgsu { nationality: Nationality::Italian, class: SquadronClass::Fighter, at: field });
        let id = match out {
            Ok(CommandOutcome::SgsuDeployed(id)) => id,
            other => panic!("expected SgsuDeployed, got {other:?}"),
        };
        assert_eq!(g.available_sgsu[&Nationality::Italian], 1);
        assert_eq!(g.sgsus[&id].cpa, 20);
        assert_eq!(g.sgsus[&id].location, Some(field));
        assert_eq!(g.sgsus[&id].nationality, Nationality::Italian);
        // Deploying at a hex with no air facility fails (§34.82 — SGSUs base at facilities).
        assert_eq!(
            g.apply(Command::DeploySgsu { nationality: Nationality::Italian, class: SquadronClass::Bomber, at: HexCoord::new(9, 9) }),
            Err(CommandError::SgsuDeployFailed)
        );
        assert_eq!(g.available_sgsu[&Nationality::Italian], 1);
        // Spend the last counter, then the supply is exhausted.
        assert!(matches!(
            g.apply(Command::DeploySgsu { nationality: Nationality::Italian, class: SquadronClass::Fighter, at: field }),
            Ok(CommandOutcome::SgsuDeployed(_))
        ));
        assert_eq!(g.available_sgsu[&Nationality::Italian], 0);
        assert_eq!(
            g.apply(Command::DeploySgsu { nationality: Nationality::Italian, class: SquadronClass::Fighter, at: field }),
            Err(CommandError::SgsuDeployFailed)
        );
    }

    #[test]
    fn campaign_game_deploys_and_reaches_rommel() {
        // §64.3: the campaign starts from the §60.0 Sept-1940 deployment and runs on. Verify the
        // start force is Italian + Commonwealth only (no Germans in Sept 1940), then advance past
        // General Rommel's Game-Turn 26 arrival (§64.2) and confirm the DAK has entered.
        use crate::reference::units::{Nationality, Side};

        let mut g = build_campaign_game();
        assert_eq!(g.game_turn, 1);

        let on_map_sides = |g: &crate::state::game::GameState, nat: Nationality| {
            g.units.values().any(|u| u.nationality == nat && u.hex().is_some())
        };
        assert!(on_map_sides(&g, Nationality::Italian), "Italian border force on map at start");
        assert!(g.units.values().any(|u| u.nationality.side() == Side::Commonwealth && u.hex().is_some()), "CW border force on map at start");
        assert!(!on_map_sides(&g, Nationality::German), "no Germans on the map in Sept 1940 (§60.0)");
        assert!(g.rommel.is_some(), "Rommel is scheduled");
        assert_eq!(g.rommel.as_ref().unwrap().arrival_game_turn, 26);

        // Advance through Game-Turn 27 (past Rommel's GT26 arrival and the DAK's GT21+ schedule).
        let mut guard = 0;
        while g.game_turn <= 27 {
            g.advance_phase();
            guard += 1;
            assert!(guard < 2_000_000, "campaign turn loop failed to progress");
        }
        assert!(g.game_turn >= 28, "advanced past Game-Turn 27");
        // The DAK arrived on schedule: German units are now in play.
        assert!(g.units.values().any(|u| u.nationality == Nationality::German), "DAK reinforcements entered by GT27");
        // Rommel is on the map (arrived GT26, off the Germany recall).
        assert!(g.rommel.as_ref().and_then(|r| r.hex).is_some() || g.rommel.as_ref().is_some_and(|r| r.in_germany),
            "Rommel has entered play (on map or recalled)");
    }

    /// A deterministic, order-independent digest of the key game state (sorted, so it does not
    /// depend on HashMap iteration order) — used to compare two runs or a save/load round-trip.
    #[cfg(test)]
    fn campaign_digest(g: &crate::state::game::GameState) -> String {
        let mut units: Vec<String> = g
            .units
            .values()
            .map(|u| format!("{:?}|{:?}|{:?}|{}", u.id, u.nationality, u.hex(), u.total_toe()))
            .collect();
        units.sort();
        format!(
            "gt{} os{} phase{:?} active{:?} units{} rommel_gt{:?} | {}",
            g.game_turn, g.operations_stage, g.phase, g.active_player, g.units.len(),
            g.rommel.as_ref().map(|r| r.arrival_game_turn),
            units.join(";"),
        )
    }

    #[test]
    fn campaign_is_deterministic_and_serializes_round_trip() {
        // §3.7 determinism + §3.3 serialization: (1) two fresh builds with the same seed, advanced
        // through the same phase sequence, reach identical state; (2) serializing mid-game and
        // reloading yields a state that continues identically.
        use crate::state::game::GameState;

        let advance = |g: &mut GameState, n: usize| for _ in 0..n { g.advance_phase(); };

        // (1) Determinism: same seed + same commands → identical digest.
        let mut a = build_campaign_game();
        let mut b = build_campaign_game();
        advance(&mut a, 400);
        advance(&mut b, 400);
        assert_eq!(campaign_digest(&a), campaign_digest(&b), "same seed must reproduce the game (§3.7)");

        // (2) Serialization round-trip: save mid-game, reload, and confirm identical continuation.
        let saved = ron::to_string(&a).expect("GameState serializes");
        let mut reloaded: GameState = ron::from_str(&saved).expect("GameState deserializes");
        assert_eq!(campaign_digest(&a), campaign_digest(&reloaded), "reloaded state matches the save");
        advance(&mut a, 60);
        advance(&mut reloaded, 60);
        assert_eq!(campaign_digest(&a), campaign_digest(&reloaded), "reloaded game continues identically (§3.3)");
    }

    #[test]
    fn legal_move_steps_enumerates_valid_moves() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::command::{Command, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation, WeaponGroup};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(Nationality::Commonwealth, include_str!("../../../data/reference/unit-characteristics/commonwealth.ron")).unwrap();
        refs.load_weapons_systems(Nationality::Commonwealth, include_str!("../../../data/reference/weapons/commonwealth.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        g.active_player = Side::Commonwealth;
        // Clear terrain around a motorized cruiser tank at (2,0).
        for c in 0..5 {
            g.map.set_hex(HexCoord::new(c, 0), HexTerrain::Clear);
            g.map.set_hex(HexCoord::new(c, 1), HexTerrain::Clear);
        }
        let tank = g.spawn_unit(Nationality::Commonwealth, "g", "tank", Composition::Weapons { groups: vec![WeaponGroup { system: "A13 Cruiser".into(), count: 6 }] });
        g.unit_mut(tank).unwrap().location = Some(UnitLocation::OnMap(HexCoord::new(2, 0)));

        // Outside the Movement & Combat phase, nothing is enumerable.
        g.phase = Phase::Organization;
        assert!(g.legal_move_steps(Side::Commonwealth).is_empty());

        // In Movement & Combat, the tank has legal single-hex moves — and every enumerated command
        // is accepted by the command boundary (the engine is the oracle).
        g.phase = Phase::MovementAndCombat;
        let moves = g.legal_move_steps(Side::Commonwealth);
        assert!(!moves.is_empty(), "a mobile unit on open ground has legal moves");
        assert!(moves.iter().all(|c| matches!(c, Command::MoveStep { unit, .. } if *unit == tank)));
        for cmd in &moves {
            let mut probe = g.clone();
            assert!(matches!(probe.apply(cmd.clone()), Ok(CommandOutcome::Moved)),
                "every enumerated move applies: {cmd:?}");
        }
        // The enemy has no units, hence no legal moves.
        assert!(g.legal_move_steps(Side::Axis).is_empty());
    }

    #[test]
    #[ignore] // timing probe, run explicitly with --ignored
    fn bench_legal_commands_on_campaign() {
        use crate::reference::units::Side;
        use crate::state::game::Phase;
        let mut g = build_campaign_game();
        let mut guard = 0;
        while g.phase != Phase::MovementAndCombat { g.advance_phase(); guard += 1; assert!(guard < 100); }
        let n_units = g.units.values().filter(|u| u.nationality.side() == g.active_player && u.hex().is_some()).count();
        let t = std::time::Instant::now();
        let cmds = g.legal_commands(g.active_player);
        let dt = t.elapsed();
        // Isolate clone-only cost vs full dry_run (clone + apply).
        let tc = std::time::Instant::now();
        for _ in 0..1000 { let _c = g.clone(); }
        let clone_dt = tc.elapsed() / 1000;
        println!("BENCH: {} active units, {} legal cmds in {:?} | clone-only {:?}", n_units, cmds.len(), dt, clone_dt);
        let _ = Side::Axis;
    }

    #[test]
    fn lookahead_agent_improves_its_position() {
        use crate::agent::{Agent, LookaheadAgent};
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::command::Command;
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut g = GameState::new(ReferenceData::load_campaign(), 9);
        for col in 0..8 {
            g.map.set_hex(HexCoord::new(col, 1), HexTerrain::Clear);
        }
        // Foot infantry as the mover (no §21 vehicle breakdown to muddy the material term).
        let inf = g.spawn_unit(Nationality::Commonwealth, "h", "infantry", Composition::Organic { current_toe: 8 });
        let foe = g.spawn_unit(Nationality::Italian, "p", "infantry", Composition::Organic { current_toe: 10 });
        { let u = g.unit_mut(inf).unwrap(); u.location = Some(UnitLocation::OnMap(HexCoord::new(1, 1))); u.fuel = 500; u.ammo = 500; }
        g.unit_mut(foe).unwrap().location = Some(UnitLocation::OnMap(HexCoord::new(6, 1)));
        g.phase = Phase::MovementAndCombat;
        g.active_player = Side::Commonwealth;

        // The eval rewards material: wiping the enemy's TOE strictly raises the Commonwealth value.
        let base = g.heuristic_value(Side::Commonwealth);
        {
            let mut probe = g.clone();
            probe.unit_mut(foe).unwrap().composition = Composition::Organic { current_toe: 0 };
            assert!(probe.heuristic_value(Side::Commonwealth) > base, "fewer enemy TOE → higher value");
        }

        // The lookahead agent chooses a move that improves its position (advancing toward contact).
        let mut agent = LookaheadAgent::new(5);
        let cmd = agent.choose(&g, Side::Commonwealth).expect("a position-improving action exists");
        assert!(matches!(cmd, Command::MoveStep { .. }), "no adjacent enemy yet → it maneuvers");
        let dist_before = HexCoord::new(1, 1).distance(HexCoord::new(6, 1));
        g.apply(cmd).unwrap();
        let dist_after = g.unit(inf).unwrap().hex().unwrap().distance(HexCoord::new(6, 1));
        assert!(dist_after < dist_before, "the agent advanced toward the enemy");
        assert!(g.heuristic_value(Side::Commonwealth) > base, "the chosen action improved its eval");
    }

    #[test]
    fn rollout_agent_advances_and_is_reproducible() {
        use crate::agent::{Agent, RolloutAgent};
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::{Nationality, Side};
        use crate::state::command::Command;
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut g = GameState::new(ReferenceData::load_campaign(), 3);
        for col in 0..8 {
            g.map.set_hex(HexCoord::new(col, 1), HexTerrain::Clear);
        }
        // Foot infantry approaching a distant enemy (no §21 breakdown; deterministic advance so the
        // averaged rollout value is stable). Combat de-noising is the agent's motivation (documented
        // in `agent::RolloutAgent`); here we assert the robust, reproducible behavior.
        let inf = g.spawn_unit(Nationality::Commonwealth, "h", "infantry", Composition::Organic { current_toe: 10 });
        let foe = g.spawn_unit(Nationality::Italian, "p", "infantry", Composition::Organic { current_toe: 10 });
        { let u = g.unit_mut(inf).unwrap(); u.location = Some(UnitLocation::OnMap(HexCoord::new(1, 1))); u.fuel = 500; u.ammo = 500; }
        g.unit_mut(foe).unwrap().location = Some(UnitLocation::OnMap(HexCoord::new(6, 1)));
        g.phase = Phase::MovementAndCombat;
        g.active_player = Side::Commonwealth;

        let mut agent = RolloutAgent::new(77, 6, 0);
        let cmd = agent.choose(&g, Side::Commonwealth).expect("rollouts find a value-improving action");
        assert!(matches!(cmd, Command::MoveStep { .. }), "no favorable adjacent target → it advances");
        // Reproducible: a fresh agent with the same seed makes the same choice.
        let mut agent2 = RolloutAgent::new(77, 6, 0);
        assert_eq!(agent2.choose(&g, Side::Commonwealth), Some(cmd.clone()));
        // The chosen move presses toward the enemy.
        g.apply(cmd).unwrap();
        assert!(g.unit(inf).unwrap().hex().unwrap().distance(HexCoord::new(6, 1)) < 5, "advanced toward the enemy");
    }

    #[test]
    fn legal_commands_air_convoy_repair_phases() {
        use crate::reference::aircraft::AircraftClass;
        use crate::reference::squadron::SquadronClass;
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::{Nationality, Side};
        use crate::state::air::{Sgsu, SquadronId};
        use crate::state::command::Command;
        use crate::state::game::{GameState, Phase, TruckConvoy};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, TruckPoints, UnitLocation};

        let mut g = GameState::new(ReferenceData::load_campaign(), 1);
        g.active_player = Side::Commonwealth;
        for c in 0..5 {
            for r in 0..3 {
                g.map.set_hex(HexCoord::new(c, r), HexTerrain::Clear);
            }
        }

        // --- Truck Convoy phase: a free CW convoy can be moved one hex. ---
        g.truck_convoys.push(TruckConvoy {
            side: Side::Commonwealth,
            hex: HexCoord::new(2, 1),
            trucks: TruckPoints { light: 5, medium: 0, heavy: 0 },
            cargo: Default::default(),
            cpa_used: 0,
            carries_replacements: false,
        });
        g.phase = Phase::TruckConvoy;
        assert!(g.legal_commands(Side::Commonwealth).iter().any(|c| matches!(c, Command::MoveTruckConvoy { convoy: 0, .. })),
            "Truck Convoy phase enumerates moving the free convoy");

        // --- Repair phase: captured enemy equipment can be repaired back into play (§28.3). ---
        let city = HexCoord::new(2, 1);
        g.capture_equipment(Side::Commonwealth, city, Nationality::Italian, "M 13/40".into(), 3);
        g.phase = Phase::Repair;
        assert!(g.legal_commands(Side::Commonwealth).iter().any(|c| matches!(c, Command::RepairCapturedEquipment { system, .. } if system == "M 13/40")),
            "Repair phase enumerates repairing the captured equipment");

        // --- Land Support Air phase: a based, ready recon squadron reconnoiters an enemy hex. ---
        let base = HexCoord::new(1, 1);
        let foe_hex = HexCoord::new(3, 1);
        let mut sq = Sgsu::new(SquadronId(1), Nationality::Commonwealth, SquadronClass::Bomber, "AC", 5, Some(base));
        assert!(sq.add_planes("Lysander Mk.I", AircraftClass::Reconnaissance, 2, 1));
        for pg in sq.planes.iter_mut() { pg.refitted = pg.count; pg.ready = pg.count; }
        g.sgsus.insert(SquadronId(1), sq);
        let foe = g.spawn_unit(Nationality::Italian, "p", "foe", Composition::Organic { current_toe: 6 });
        g.unit_mut(foe).unwrap().location = Some(UnitLocation::OnMap(foe_hex));
        g.phase = Phase::LandSupportAir;
        let air = g.legal_commands(Side::Commonwealth);
        assert!(air.iter().any(|c| matches!(c, Command::ReconHex { target, .. } if *target == foe_hex)),
            "Land Support Air phase enumerates an air-recon mission over the enemy hex");
        // The Lysander is strafe-capable (strike: StrafeOnly), so a strafing run is enumerated too.
        assert!(air.iter().any(|c| matches!(c, Command::Strafe { target, .. } if *target == foe_hex)),
            "air depth: strafing missions are enumerated for a strike-capable squadron");
    }

    #[test]
    fn self_play_weight_tuning_runs_and_does_not_regress() {
        use crate::agent::{evaluate_weights, tune_weights};
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::{Nationality, Side};
        use crate::state::game::GameState;
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};
        use crate::state::view::EvalWeights;

        // A small, self-contained battlefield built once; cloned (O(1)) for each self-play game.
        let mut template = GameState::new(ReferenceData::load_campaign(), 1);
        for c in 0..6 {
            template.map.set_hex(HexCoord::new(c, 1), HexTerrain::Clear);
        }
        for (nat, col) in [(Nationality::Commonwealth, 1), (Nationality::Italian, 4)] {
            let id = template.spawn_unit(nat, if nat == Nationality::Commonwealth { "h" } else { "p" }, "inf", Composition::Organic { current_toe: 8 });
            let u = template.unit_mut(id).unwrap();
            u.location = Some(UnitLocation::OnMap(HexCoord::new(col, 1)));
            u.fuel = 500;
            u.ammo = 500;
        }
        let make = || template.clone();

        // Tiny budget: one coordinate-ascent round, one seed, one Game-Turn.
        let default = EvalWeights::default();
        let base = evaluate_weights(&make, default, default, 1, 1);
        let tuned = tune_weights(&make, 1, 1, 1, 20);
        let tuned_score = evaluate_weights(&make, tuned, default, 1, 1);

        // Coordinate ascent only accepts improvements, so the tuned blend never scores below default.
        assert!(tuned_score >= base, "tuning does not regress against the fixed objective");
        // Deterministic: same budget + same games → same tuned weights.
        assert_eq!(tune_weights(&make, 1, 1, 1, 20), tuned);
        let _ = Side::Axis;
    }

    #[test]
    fn air_bombing_enumerates_escort_variants() {
        use crate::reference::aircraft::AircraftClass;
        use crate::reference::squadron::SquadronClass;
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::{Nationality, Side};
        use crate::state::air::{Sgsu, SquadronId};
        use crate::state::command::Command;
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut g = GameState::new(ReferenceData::load_campaign(), 1);
        g.active_player = Side::Commonwealth;
        for c in 0..4 {
            g.map.set_hex(HexCoord::new(c, 1), HexTerrain::Clear);
        }
        let base = HexCoord::new(1, 1);
        let target = HexCoord::new(2, 1);
        // A Commonwealth day-bomber and a Hurricane fighter, both based and ready.
        let mut bomber = Sgsu::new(SquadronId(1), Nationality::Commonwealth, SquadronClass::Bomber, "bombers", 5, Some(base));
        assert!(bomber.add_planes("167 Maryland", AircraftClass::Bomber, 6, 1));
        let mut fighter = Sgsu::new(SquadronId(2), Nationality::Commonwealth, SquadronClass::Fighter, "fighters", 5, Some(base));
        assert!(fighter.add_planes("Hurricane Mk.I", AircraftClass::Fighter, 4, 1));
        for sq in [&mut bomber, &mut fighter] {
            for pg in sq.planes.iter_mut() { pg.refitted = pg.count; pg.armed = pg.count; pg.ready = pg.count; }
        }
        g.sgsus.insert(SquadronId(1), bomber);
        g.sgsus.insert(SquadronId(2), fighter);
        let foe = g.spawn_unit(Nationality::Italian, "p", "foe", Composition::Organic { current_toe: 8 });
        g.unit_mut(foe).unwrap().location = Some(UnitLocation::OnMap(target));
        g.phase = Phase::LandSupportAir;

        let air = g.legal_commands(Side::Commonwealth);
        // The bomber can attack the enemy hex both unescorted and escorted by the Hurricane.
        assert!(air.iter().any(|c| matches!(c, Command::FlyBombingMissionAt { squadron, escort: None, target: t, .. } if *squadron == SquadronId(1) && *t == target)),
            "an unescorted bombing mission is enumerated");
        assert!(air.iter().any(|c| matches!(c, Command::FlyBombingMissionAt { squadron, escort: Some(e), target: t, .. } if *squadron == SquadronId(1) && *e == SquadronId(2) && *t == target)),
            "parameter breadth: an escorted variant (Hurricane escort) is enumerated");
    }

    #[test]
    fn enumerator_offshore_bombard_and_convoy_planning() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::{Nationality, Side};
        use crate::state::command::Command;
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::naval::{Ship, ShipLocation};
        use crate::state::unit::{Composition, UnitLocation};

        let mut g = GameState::new(ReferenceData::load_campaign(), 1);
        for c in 0..5 {
            for r in 0..3 {
                g.map.set_hex(HexCoord::new(c, r), HexTerrain::Clear);
            }
        }
        // A heavy Commonwealth ship at sea, with an enemy unit on an adjacent hex.
        let sea = HexCoord::new(2, 1);
        let target = g.map.neighbors(sea).next().unwrap();
        g.fleet.push(Ship {
            name: "HMS Heavy".into(), max_gun: 8, gun_rating: 8, max_aa: 3,
            location: ShipLocation::AtSea(sea), at_sea_stages: 0, repair_stages: 0,
            fired_this_stage: false, rest_owed: 0,
        });
        let foe = g.spawn_unit(Nationality::Italian, "p", "foe", Composition::Organic { current_toe: 6 });
        g.unit_mut(foe).unwrap().location = Some(UnitLocation::OnMap(target));

        // Movement & Combat: the ship can shell the adjacent enemy hex (off-shore bombardment).
        g.phase = Phase::MovementAndCombat;
        assert!(g.legal_commands(Side::Commonwealth).iter().any(|c| matches!(c, Command::OffShoreBombard { target: t, .. } if *t == target)),
            "off-shore bombardment of the adjacent enemy hex is enumerated");

        // Naval Convoy Schedule: the Axis can plan a supply convoy on a shipping lane; the CW cannot.
        g.phase = Phase::NavalConvoySchedule;
        assert!(g.legal_commands(Side::Axis).iter().any(|c| matches!(c, Command::PlanConvoy { .. })),
            "convoy-planning policy offers an Axis convoy on an available lane");
        assert!(g.legal_commands(Side::Commonwealth).is_empty(), "the Commonwealth plans no Axis convoys");
    }

    #[test]
    fn legal_commands_fleet_phase_repairs_ships() {
        use crate::reference::units::Side;
        use crate::state::command::Command;
        use crate::state::game::{GameState, Phase};
        use crate::state::naval::{Ship, ShipLocation};

        let mut g = GameState::new(ReferenceData::load_campaign(), 1);
        g.active_player = Side::Commonwealth;
        // A damaged Commonwealth cruiser resting in Alexandria (gun rating below its maximum).
        g.fleet.push(Ship {
            name: "HMS Test".into(),
            max_gun: 8,
            gun_rating: 4,
            max_aa: 2,
            location: ShipLocation::Alexandria,
            at_sea_stages: 0,
            repair_stages: 0,
            fired_this_stage: false,
            rest_owed: 0,
        });
        g.phase = Phase::CommonwealthFleet;
        assert!(g.legal_commands(Side::Commonwealth).iter().any(|c| matches!(c, Command::RepairShip { ship_index: 0 })),
            "Commonwealth Fleet phase enumerates repairing the damaged ship");
        // The Axis has no Fleet, so it has no fleet-phase actions.
        assert!(g.legal_commands(Side::Axis).is_empty());
    }

    #[test]
    fn legal_commands_are_phase_aware_organization_logistics() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::{Nationality, Side};
        use crate::state::command::Command;
        use crate::state::game::{Commodities, GameState, Phase, SupplyDump};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation, WeaponGroup};

        let mut g = GameState::new(ReferenceData::load_campaign(), 1);
        g.active_player = Side::Commonwealth;
        let hex = HexCoord::new(2, 2);
        g.map.set_hex(hex, HexTerrain::Clear);
        // A fuel-starved cruiser squadron co-located with a Commonwealth fuel dump.
        let tank = g.spawn_unit(Nationality::Commonwealth, "g", "cruisers", Composition::Weapons { groups: vec![WeaponGroup { system: "A13 Cruiser".into(), count: 4 }] });
        { let u = g.unit_mut(tank).unwrap(); u.location = Some(UnitLocation::OnMap(hex)); u.fuel = 0; }
        g.supply_dumps.push(SupplyDump { location: UnitLocation::OnMap(hex), side: Side::Commonwealth, supply: Commodities { fuel: 9999, ammo: 9999, stores: 9999, water: 9999 }, dummy: false });

        // In the Organization phase the enumerator offers logistics — refuelling the starved unit.
        g.phase = Phase::Organization;
        let org = g.legal_commands(Side::Commonwealth);
        assert!(org.iter().any(|c| matches!(c, Command::RefuelUnit { unit } if *unit == tank)),
            "Organization phase enumerates a refuel for the fuel-starved unit at the dump");
        assert!(org.iter().all(|c| !matches!(c, Command::MoveStep { .. })), "no movement in the Organization phase");

        // The same query in Movement & Combat yields movement, not logistics.
        g.phase = Phase::MovementAndCombat;
        let mc = g.legal_commands(Side::Commonwealth);
        assert!(mc.iter().all(|c| !matches!(c, Command::RefuelUnit { .. })), "no refuel in Movement & Combat");
    }

    #[test]
    fn scenario1_victory_60_81() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::{Nationality, Side};
        use crate::state::game::{Commodities, GameState, SupplyDump};
        use crate::state::map::{HexCoord, HexId};
        use crate::state::unit::{Composition, UnitLocation};
        use crate::state::view::ScenarioVictory;

        let mut g = GameState::new(ReferenceData::load_campaign(), 1);
        let tobruk = HexId::parse("C4807").unwrap().to_hex_coord();
        let off = HexCoord::new(0, 0);
        g.map.set_hex(tobruk, HexTerrain::MajorCity); // water source
        g.map.set_hex(off, HexTerrain::Clear);
        g.supply_dumps.push(SupplyDump { location: UnitLocation::OnMap(tobruk), side: Side::Commonwealth, supply: Commodities { fuel: 9999, ammo: 9999, stores: 9999, water: 9999 }, dummy: false });

        // No one holds anything yet.
        assert_eq!(g.scenario1_victory(Side::Commonwealth), ScenarioVictory::None);
        assert_eq!(g.scenario1_victory(Side::Axis), ScenarioVictory::None);

        // §60.81: a supplied Commonwealth combat unit holding Tobruk is a Strategic Victory.
        let cw = g.spawn_unit(Nationality::Commonwealth, "h", "cw", Composition::Organic { current_toe: 8 });
        g.unit_mut(cw).unwrap().location = Some(UnitLocation::OnMap(tobruk));
        assert_eq!(g.scenario1_victory(Side::Commonwealth), ScenarioVictory::Strategic);
        assert_eq!(g.scenario1_victory(Side::Axis), ScenarioVictory::None);

        // Step off Tobruk → no longer a victory.
        g.unit_mut(cw).unwrap().location = Some(UnitLocation::OnMap(off));
        assert_eq!(g.scenario1_victory(Side::Commonwealth), ScenarioVictory::None);
    }

    #[test]
    fn heuristic_value_rewards_objectives_and_cohesion() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::{Nationality, Side};
        use crate::state::game::{Commodities, GameState, SupplyDump};
        use crate::state::map::{HexCoord, HexId};
        use crate::state::unit::{Composition, UnitLocation};

        let mut g = GameState::new(ReferenceData::load_campaign(), 1);
        let tobruk = HexId::parse("C4807").unwrap().to_hex_coord(); // §64.73 city, CW VP 100
        let off = HexCoord::new(0, 0);
        g.map.set_hex(tobruk, HexTerrain::MajorCity); // a Major City is a water source
        g.map.set_hex(off, HexTerrain::Clear);
        let stock = || Commodities { fuel: 9999, ammo: 9999, stores: 9999, water: 9999 };
        for at in [tobruk, off] {
            g.supply_dumps.push(SupplyDump { location: UnitLocation::OnMap(at), side: Side::Commonwealth, supply: stock(), dummy: false });
        }
        // A single in-supply Commonwealth combat unit; no enemy (isolates the objective term).
        let u = g.spawn_unit(Nationality::Commonwealth, "h", "inf", Composition::Organic { current_toe: 8 });
        g.unit_mut(u).unwrap().location = Some(UnitLocation::OnMap(tobruk));

        // Holding Tobruk is worth its CW Geographical-Occupation VP (100) × the objective weight (60).
        let eval_hold = g.heuristic_value(Side::Commonwealth);
        g.unit_mut(u).unwrap().location = Some(UnitLocation::OnMap(off)); // step off the objective
        let eval_off = g.heuristic_value(Side::Commonwealth);
        assert_eq!(eval_hold - eval_off, 100 * 60, "holding a §64.73 objective adds its VP × weight");

        // Cohesion: a steadier force scores higher, all else equal.
        let before = g.heuristic_value(Side::Commonwealth);
        g.unit_mut(u).unwrap().cohesion += 10;
        assert_eq!(g.heuristic_value(Side::Commonwealth) - before, 10, "cohesion contributes at weight 1");
    }

    #[test]
    fn mcts_agent_plans_and_is_reproducible() {
        use crate::agent::{Agent, MctsAgent};
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::{Nationality, Side};
        use crate::state::command::{Command, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut g = GameState::new(ReferenceData::load_campaign(), 3);
        for col in 0..8 {
            g.map.set_hex(HexCoord::new(col, 1), HexTerrain::Clear);
        }
        let inf = g.spawn_unit(Nationality::Commonwealth, "h", "infantry", Composition::Organic { current_toe: 10 });
        let foe = g.spawn_unit(Nationality::Italian, "p", "infantry", Composition::Organic { current_toe: 10 });
        { let u = g.unit_mut(inf).unwrap(); u.location = Some(UnitLocation::OnMap(HexCoord::new(1, 1))); u.fuel = 500; u.ammo = 500; }
        g.unit_mut(foe).unwrap().location = Some(UnitLocation::OnMap(HexCoord::new(6, 1)));
        g.phase = Phase::MovementAndCombat;
        g.active_player = Side::Commonwealth;

        // UCT plans the phase and commits to an action; advancing improves the position, so it does
        // not simply Pass.
        let mut mcts = MctsAgent::new(101, 80, 4, 4);
        let cmd = mcts.choose(&g, Side::Commonwealth).expect("MCTS commits to an action, not Pass");
        assert!(matches!(cmd, Command::MoveStep { .. }), "no favorable adjacent target → it maneuvers");
        // Reproducible for a fixed seed.
        let mut mcts2 = MctsAgent::new(101, 80, 4, 4);
        assert_eq!(mcts2.choose(&g, Side::Commonwealth), Some(cmd.clone()));
        // The committed action is one the command boundary accepts.
        assert_eq!(g.apply(cmd), Ok(CommandOutcome::Moved));
    }

    #[test]
    fn cross_phase_mcts_plans_across_the_sequence_of_play() {
        use crate::agent::{Agent, CrossPhaseMctsAgent};
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::{Nationality, Side};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut g = GameState::new(ReferenceData::load_campaign(), 3);
        for col in 0..6 {
            g.map.set_hex(HexCoord::new(col, 1), HexTerrain::Clear);
        }
        let cw = g.spawn_unit(Nationality::Commonwealth, "h", "cw", Composition::Organic { current_toe: 10 });
        let it = g.spawn_unit(Nationality::Italian, "p", "it", Composition::Organic { current_toe: 10 });
        { let u = g.unit_mut(cw).unwrap(); u.location = Some(UnitLocation::OnMap(HexCoord::new(1, 1))); u.fuel = 500; u.ammo = 500; }
        { let u = g.unit_mut(it).unwrap(); u.location = Some(UnitLocation::OnMap(HexCoord::new(4, 1))); u.fuel = 500; u.ammo = 500; }
        g.phase = Phase::MovementAndCombat;
        g.active_player = Side::Commonwealth;

        // Small search (rollouts advance the whole SoP, so keep it bounded): it looks one Game-Turn
        // ahead and models the Axis reply. The search runs to a valid decision without panicking.
        let mut mcts = CrossPhaseMctsAgent::new(202, 30, 1, 4, 10);
        let decision = mcts.choose(&g, Side::Commonwealth);
        // Same seed reproduces the plan (deterministic search over reseeded rollouts).
        let mut mcts2 = CrossPhaseMctsAgent::new(202, 30, 1, 4, 10);
        assert_eq!(mcts2.choose(&g, Side::Commonwealth), decision);
        // Whatever it commits to is accepted by the command boundary (or it yields).
        if let Some(cmd) = decision {
            assert!(g.apply(cmd).is_ok(), "the planned action is legal");
        }
    }

    #[test]
    fn agents_self_play_drives_real_moves() {
        // Phase-2 smoke: greedy Axis vs random Commonwealth play a few Game-Turns on a small front
        // (full reference data, tiny action space so it is fast). Confirms the agent → enumerator →
        // command loop drives real, accepted play — units maneuver — without stalling or panicking.
        use crate::agent::{self, GreedyAgent, RandomAgent};
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::{Nationality, Side};
        use crate::state::game::GameState;
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation, WeaponGroup};

        let mut g = GameState::new(ReferenceData::load_campaign(), 9);
        // A small clear battlefield.
        for col in 0..8 {
            for row in 0..3 {
                g.map.set_hex(HexCoord::new(col, row), HexTerrain::Clear);
            }
        }
        // A Commonwealth cruiser squadron and an Italian infantry battalion a few hexes apart, each
        // stocked with fuel + ammo so the agents can actually move and fight.
        let cw_tank = g.spawn_unit(Nationality::Commonwealth, "g", "cruisers", Composition::Weapons { groups: vec![WeaponGroup { system: "A13 Cruiser".into(), count: 6 }] });
        let it_inf = g.spawn_unit(Nationality::Italian, "p", "infantry", Composition::Organic { current_toe: 10 });
        {
            let u = g.unit_mut(cw_tank).unwrap();
            u.location = Some(UnitLocation::OnMap(HexCoord::new(1, 1)));
            u.fuel = 500; u.ammo = 500;
        }
        {
            let u = g.unit_mut(it_inf).unwrap();
            u.location = Some(UnitLocation::OnMap(HexCoord::new(6, 1)));
            u.fuel = 500; u.ammo = 500;
        }
        let start_cw = g.unit(cw_tank).unwrap().hex();
        let start_it = g.unit(it_inf).unwrap().hex();

        let mut axis = GreedyAgent::new(42);
        let mut cw = RandomAgent::new(1234);
        let applied = agent::self_play(&mut g, &mut cw, &mut axis, 3);

        assert!(g.game_turn >= 4, "self-play advanced through the sequence of play");
        assert!(applied > 0, "agents issued accepted commands");
        let moved = g.unit(cw_tank).and_then(|u| u.hex()) != start_cw
            || g.unit(it_inf).and_then(|u| u.hex()) != start_it;
        assert!(moved, "agents maneuvered at least one unit");
        let _ = Side::Axis;
    }

    /// A fresh micro-battlefield (CW cruisers vs an Italian battalion, both stocked) for the
    /// tournament harness — full reference data, tiny action space so a round-robin is fast.
    #[cfg(test)]
    fn tournament_micro_game() -> crate::state::game::GameState {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Nationality;
        use crate::state::game::GameState;
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation, WeaponGroup};
        let mut g = GameState::new(ReferenceData::load_campaign(), 9);
        for col in 0..8 {
            for row in 0..3 {
                g.map.set_hex(HexCoord::new(col, row), HexTerrain::Clear);
            }
        }
        let cw = g.spawn_unit(Nationality::Commonwealth, "g", "cruisers", Composition::Weapons { groups: vec![WeaponGroup { system: "A13 Cruiser".into(), count: 6 }] });
        let it = g.spawn_unit(Nationality::Italian, "p", "infantry", Composition::Organic { current_toe: 10 });
        {
            let u = g.unit_mut(cw).unwrap();
            u.location = Some(UnitLocation::OnMap(HexCoord::new(1, 1)));
            u.fuel = 500;
            u.ammo = 500;
        }
        {
            let u = g.unit_mut(it).unwrap();
            u.location = Some(UnitLocation::OnMap(HexCoord::new(6, 1)));
            u.fuel = 500;
            u.ammo = 500;
        }
        g
    }

    #[test]
    fn can_move_step_matches_dry_run() {
        // The enumerator's fast path (`can_move_step` → `plan_move_step`) must accept exactly the
        // single-hex moves the command boundary (clone + `apply`) accepts. Guards against the
        // shared-validation refactor silently diverging from execution.
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::{Nationality, Side};
        use crate::rules::movement::MoveMode;
        use crate::state::command::Command;
        use crate::state::game::GameState;
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation, WeaponGroup};

        let mut g = GameState::new(ReferenceData::load_campaign(), 3);
        for col in 0..8 {
            for row in 0..4 {
                g.map.set_hex(HexCoord::new(col, row), HexTerrain::Clear);
            }
        }
        // A cluster: cruisers with fuel, infantry with fuel, one fuel-starved tank (motorized cost),
        // and enemy units adjacent (enemy-occupied hexes, ZOC) — varied accept/reject cases.
        let mk = |g: &mut GameState, nat, id: &str, comp, col, row, fuel| {
            let u = g.spawn_unit(nat, id, "u", comp);
            let unit = g.unit_mut(u).unwrap();
            unit.location = Some(UnitLocation::OnMap(HexCoord::new(col, row)));
            unit.fuel = fuel;
            unit.ammo = 200;
            u
        };
        let cruiser = || Composition::Weapons { groups: vec![WeaponGroup { system: "A13 Cruiser".into(), count: 6 }] };
        mk(&mut g, Nationality::Commonwealth, "g", cruiser(), 2, 1, 500);
        mk(&mut g, Nationality::Commonwealth, "h", Composition::Organic { current_toe: 6 }, 2, 2, 500);
        mk(&mut g, Nationality::Commonwealth, "g", cruiser(), 3, 1, 0); // fuel-starved
        mk(&mut g, Nationality::Italian, "p", Composition::Organic { current_toe: 6 }, 3, 2, 500);
        mk(&mut g, Nationality::Italian, "p", Composition::Organic { current_toe: 6 }, 4, 1, 500);
        g.advance_to_movement_combat(120);

        let own: Vec<crate::state::unit::UnitId> = g
            .units
            .values()
            .filter(|u| u.nationality.side() == Side::Commonwealth)
            .map(|u| u.id)
            .collect();
        let mut checked = 0;
        for unit in own {
            let Some(from) = g.unit(unit).and_then(|u| u.hex()) else { continue };
            for to in g.map.neighbors(from) {
                for mode in [MoveMode::NonMotorized, MoveMode::Motorized] {
                    let fast = g.can_move_step(unit, to, mode);
                    let slow = g.dry_run(&Command::MoveStep { unit, to, mode }).is_ok();
                    assert_eq!(fast, slow, "fast/dry_run disagree: unit {unit:?} to {to:?} {mode:?} (fast={fast}, dry_run={slow})");
                    checked += 1;
                }
            }
        }
        assert!(checked > 20, "exercised a range of move candidates ({checked})");
    }

    #[test]
    fn can_move_step_matches_dry_run_over_evolving_game() {
        // Stronger guard: drive a small game forward with random legal moves and, at every state,
        // assert `can_move_step` agrees with `dry_run` for every unit/neighbor/mode on BOTH sides.
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::{Nationality, Side};
        use crate::rules::movement::MoveMode;
        use crate::state::command::Command;
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation, WeaponGroup};

        let mut g = GameState::new(ReferenceData::load_campaign(), 7);
        for col in 0..10 {
            for row in 0..5 {
                g.map.set_hex(HexCoord::new(col, row), HexTerrain::Clear);
            }
        }
        let mk = |g: &mut GameState, nat, id: &str, comp, col, row| {
            let u = g.spawn_unit(nat, id, "u", comp);
            let unit = g.unit_mut(u).unwrap();
            unit.location = Some(UnitLocation::OnMap(HexCoord::new(col, row)));
            unit.fuel = 400;
            unit.ammo = 400;
        };
        let cruiser = || Composition::Weapons { groups: vec![WeaponGroup { system: "A13 Cruiser".into(), count: 6 }] };
        for i in 0..4 {
            mk(&mut g, Nationality::Commonwealth, "g", cruiser(), i, i % 5);
            mk(&mut g, Nationality::Commonwealth, "h", Composition::Organic { current_toe: 6 }, i, (i + 1) % 5);
            mk(&mut g, Nationality::Italian, "p", Composition::Organic { current_toe: 6 }, 9 - i, i % 5);
            mk(&mut g, Nationality::Italian, "k", Composition::Weapons { groups: vec![WeaponGroup { system: "M 11/39".into(), count: 6 }] }, 9 - i, (i + 1) % 5);
        }
        g.advance_to_movement_combat(120);

        let mut rng: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };
        let mut states = 0;
        for _ in 0..120 {
            if g.phase == Phase::MovementAndCombat {
                states += 1;
                for side in [Side::Commonwealth, Side::Axis] {
                    let ids: Vec<_> = g.units.values().filter(|u| u.nationality.side() == side).map(|u| u.id).collect();
                    for unit in ids {
                        let Some(from) = g.unit(unit).and_then(|u| u.hex()) else { continue };
                        for to in g.map.neighbors(from) {
                            for mode in [MoveMode::NonMotorized, MoveMode::Motorized] {
                                assert_eq!(
                                    g.can_move_step(unit, to, mode),
                                    g.dry_run(&Command::MoveStep { unit, to, mode }).is_ok(),
                                    "divergence: {unit:?} {from:?}->{to:?} {mode:?} at state {states}"
                                );
                            }
                        }
                    }
                }
                // Advance the game by a random legal move for the active side (mutates real state).
                let moves = g.legal_move_steps(g.active_player);
                if !moves.is_empty() {
                    let pick = moves[(next() as usize) % moves.len()].clone();
                    let _ = g.apply(pick);
                }
            }
            g.advance_phase();
        }
        assert!(states > 5, "visited several Movement & Combat states ({states})");
    }

    #[test]
    #[ignore = "microbenchmark: prints hot-path timings; run with --ignored --nocapture"]
    fn perf_hotpath_microbench() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::{Nationality, Side};
        use crate::state::game::GameState;
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation, WeaponGroup};
        use std::time::Instant;

        // ~40 units per side on a 20×10 clear board — a mid-size state that stresses the
        // O(units) dry_run clone the enumerator pays per candidate.
        let mut g = GameState::new(ReferenceData::load_campaign(), 5);
        for col in 0..20 {
            for row in 0..10 {
                g.map.set_hex(HexCoord::new(col, row), HexTerrain::Clear);
            }
        }
        let place = |g: &mut GameState, nat, id: &str, comp, col, row| {
            let u = g.spawn_unit(nat, id, "u", comp);
            let unit = g.unit_mut(u).unwrap();
            unit.location = Some(UnitLocation::OnMap(HexCoord::new(col, row)));
            unit.fuel = 900;
            unit.ammo = 900;
        };
        for i in 0..40 {
            place(&mut g, Nationality::Commonwealth, "h", Composition::Organic { current_toe: 6 }, i % 18, i % 5);
            place(&mut g, Nationality::Italian, "p", Composition::Organic { current_toe: 6 }, 19 - (i % 18), 5 + i % 5);
        }
        let _ = g.spawn_unit(Nationality::Commonwealth, "g", "t", Composition::Weapons { groups: vec![WeaponGroup { system: "A13 Cruiser".into(), count: 6 }] });
        g.advance_to_movement_combat(120);
        let n_units = g.units.len();

        let t = Instant::now();
        for _ in 0..50 {
            let _ = std::hint::black_box(g.clone());
        }
        let clone_us = t.elapsed().as_secs_f64() * 1e6 / 50.0;

        let cmd = crate::state::command::Command::MoveStep {
            unit: crate::state::unit::UnitId(0),
            to: HexCoord::new(0, 0),
            mode: crate::rules::movement::MoveMode::NonMotorized,
        };
        let t = Instant::now();
        for _ in 0..200 {
            let _ = std::hint::black_box(g.dry_run(&cmd));
        }
        let dry_us = t.elapsed().as_secs_f64() * 1e6 / 200.0;

        let t = Instant::now();
        let mut moves = 0;
        for _ in 0..20 {
            moves += std::hint::black_box(g.legal_move_steps(Side::Commonwealth)).len();
        }
        let moves_ms = t.elapsed().as_secs_f64() * 1e3 / 20.0;

        let t = Instant::now();
        let mut total = 0;
        for _ in 0..20 {
            total += std::hint::black_box(g.legal_commands(Side::Commonwealth)).len();
        }
        let legal_ms = t.elapsed().as_secs_f64() * 1e3 / 20.0;

        println!("\n[perf] units={n_units}  clone={clone_us:.1}µs  dry_run={dry_us:.1}µs  legal_move_steps={moves_ms:.2}ms ({} mv)  legal_commands={legal_ms:.2}ms ({} cmds)", moves / 20, total / 20);

        // One MctsAgent::standard decision — the search cost that gates campaign tournaments.
        use crate::agent::{Agent, MctsAgent};
        let mut mcts = MctsAgent::standard(1);
        let t = Instant::now();
        let _ = std::hint::black_box(mcts.choose(&g, Side::Commonwealth));
        let mcts_ms = t.elapsed().as_secs_f64() * 1e3;
        println!("[perf] MctsAgent::standard.choose = {mcts_ms:.1}ms for one move");
    }

    #[test]
    fn tournament_round_robin_is_consistent_and_deterministic() {
        // The round-robin harness: cheap agents on the micro-board. Asserts the bookkeeping
        // invariants (every contender plays the same count; wins == losses; margins cancel; sorted
        // best-first) and that the whole tournament is deterministic (identical when replayed).
        use crate::agent::{round_robin, Contender, GreedyAgent, LookaheadAgent, RandomAgent};

        let template = tournament_micro_game();
        let make_game = || template.clone();
        let roster = || {
            vec![
                Contender::new("Random", |s| Box::new(RandomAgent::new(s))),
                Contender::new("Greedy", |s| Box::new(GreedyAgent::new(s))),
                Contender::new("Lookahead", |s| Box::new(LookaheadAgent::new(s))),
            ]
        };
        let (seeds, turns) = (2u32, 2u32);
        let t = round_robin(&make_game, &roster(), seeds, turns, 0);
        let st = &t.standings;

        assert_eq!(st.len(), 3);
        let per_agent = (3 - 1) * seeds * 2; // (n-1) * seeds * both sides
        let mut wins = 0u32;
        let mut losses = 0u32;
        let mut margin = 0i64;
        for s in st {
            assert_eq!(s.games, per_agent, "{} played {} games, expected {}", s.name, s.games, per_agent);
            assert_eq!(s.wins + s.draws + s.losses, s.games, "W+D+L must equal games");
            wins += s.wins;
            losses += s.losses;
            margin += s.margin_sum;
        }
        assert_eq!(wins, losses, "every win is someone's loss");
        assert_eq!(margin, 0, "per-game margins are equal and opposite, so they cancel");
        for w in st.windows(2) {
            assert!(w[0].points() >= w[1].points(), "standings are sorted best-first");
        }
        // Head-to-head grid is consistent: i's wins vs j == j's losses vs i, draws symmetric, and
        // each contender's grid row sums to its overall record.
        let n = t.names.len();
        for i in 0..n {
            assert_eq!(t.grid[i][i].wins + t.grid[i][i].draws + t.grid[i][i].losses, 0, "no self games");
            let (mut w, mut d, mut l) = (0, 0, 0);
            for j in 0..n {
                assert_eq!(t.grid[i][j].wins, t.grid[j][i].losses, "i's win vs j is j's loss vs i");
                assert_eq!(t.grid[i][j].draws, t.grid[j][i].draws, "draws are symmetric");
                w += t.grid[i][j].wins;
                d += t.grid[i][j].draws;
                l += t.grid[i][j].losses;
            }
            let s = st.iter().find(|s| s.name == t.names[i]).unwrap();
            assert_eq!((w, d, l), (s.wins, s.draws, s.losses), "grid row sums to the standing");
        }
        // Determinism (within a process): replaying the identical tournament matches.
        let t2 = round_robin(&make_game, &roster(), seeds, turns, 0);
        for (a, b) in st.iter().zip(&t2.standings) {
            assert_eq!((&a.name, a.games, a.wins, a.draws, a.losses, a.margin_sum),
                       (&b.name, b.games, b.wins, b.draws, b.losses, b.margin_sum));
        }
    }

    #[test]
    fn command_serde_round_trips() {
        // Command/CommandOutcome are Serialize + Deserialize — the round-trippable schema behind
        // WASM replay and the MCP action interface. Round-trip a representative set (via ron, the
        // engine's existing serde format) and confirm equality.
        use crate::rules::movement::MoveMode;
        use crate::state::command::Command;
        use crate::state::map::HexCoord;
        use crate::state::unit::UnitId;
        let cmds = [
            Command::MoveStep { unit: UnitId(3), to: HexCoord::new(4, 5), mode: MoveMode::Motorized },
            Command::CloseAssault { attackers: vec![UnitId(1), UnitId(2)], target: HexCoord::new(0, 0) },
            Command::AntiArmor { attackers: vec![UnitId(7)], target_hex: HexCoord::new(2, 2), withheld: vec![UnitId(8)] },
            Command::RefuelUnit { unit: UnitId(5) },
        ];
        for cmd in cmds {
            let s = ron::to_string(&cmd).expect("Command serializes");
            let back: Command = ron::from_str(&s).expect("Command deserializes");
            assert_eq!(cmd, back, "Command round-trips: {s}");
        }
    }

    #[test]
    fn cannot_move_onto_enemy_weapons_unit_hex_8_13() {
        // §8.13: an enemy tank/gun (a Weapons unit) is a combat unit — you attack its hex, you do
        // not enter it. Regression: is_combat_unit ignored Weapons units (they carry no organic
        // §4.46 close-assault rating), so infantry could walk onto an enemy tank (seen in a replay).
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::{Nationality, Side};
        use crate::rules::movement::MoveMode;
        use crate::state::command::{Command, CommandError};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation, WeaponGroup};

        let mut g = GameState::new(ReferenceData::load_campaign(), 1);
        g.phase = Phase::MovementAndCombat;
        g.active_player = Side::Commonwealth;
        let inf_hex = HexCoord::new(1, 1);
        let tank_hex = HexCoord::new(2, 1);
        g.map.set_hex(inf_hex, HexTerrain::Clear);
        g.map.set_hex(tank_hex, HexTerrain::Clear);
        let inf = g.spawn_unit(Nationality::Commonwealth, "h", "inf", Composition::Organic { current_toe: 8 });
        {
            let u = g.unit_mut(inf).unwrap();
            u.location = Some(UnitLocation::OnMap(inf_hex));
            u.fuel = 400;
            u.ammo = 400;
        }
        let tank = g.spawn_unit(Nationality::Italian, "k", "tank", Composition::Weapons { groups: vec![WeaponGroup { system: "M 11/39".into(), count: 6 }] });
        g.unit_mut(tank).unwrap().location = Some(UnitLocation::OnMap(tank_hex));

        assert!(g.is_combat_unit(tank), "a tank (Weapons unit) is a combat unit");
        assert!(!g.can_move_step(inf, tank_hex, MoveMode::NonMotorized), "may not step onto the tank's hex");
        assert_eq!(
            g.apply(Command::MoveStep { unit: inf, to: tank_hex, mode: MoveMode::NonMotorized }),
            Err(CommandError::ProhibitedTerrain),
            "§8.13 rejects entering an enemy combat unit's hex"
        );
    }

    #[test]
    fn command_rule_refs_map_the_enumerated_commands() {
        use crate::rules::movement::MoveMode;
        use crate::state::command::Command;
        use crate::state::map::HexCoord;
        use crate::state::unit::UnitId;
        let mv = Command::MoveStep { unit: UnitId(0), to: HexCoord::new(0, 0), mode: MoveMode::NonMotorized };
        assert_eq!(mv.rule_refs(), &["§8.13", "§8.14"]);
        let ca = Command::CloseAssault { attackers: vec![UnitId(0)], target: HexCoord::new(0, 0) };
        assert_eq!(ca.rule_refs(), &["§15"]);
        // A variant not in the map returns empty (the map is extensible, not exhaustive).
        let well = Command::PoisonWell { hex: HexCoord::new(0, 0), die: 1 };
        assert!(well.rule_refs().is_empty());
    }

    #[test]
    fn standing_win_rate_ci_is_sane() {
        use crate::agent::Standing;
        let s = Standing { name: "x".into(), games: 24, wins: 16, draws: 0, losses: 8, margin_sum: 0 };
        let (lo, hi) = s.win_rate_ci95();
        let m = s.win_rate();
        assert!(lo <= m && m <= hi, "CI brackets the mean ({lo} <= {m} <= {hi})");
        assert!((0.0..=1.0).contains(&lo) && (0.0..=1.0).contains(&hi), "CI clamped to [0,1]");
        // 10× the games (same rate) gives a strictly narrower interval.
        let s2 = Standing { name: "x".into(), games: 240, wins: 160, draws: 0, losses: 80, margin_sum: 0 };
        let (lo2, hi2) = s2.win_rate_ci95();
        assert!(hi2 - lo2 < hi - lo, "more games → narrower CI");
        // A perfect record still yields a valid, clamped interval (no panic / out-of-range).
        let perfect = Standing { name: "x".into(), games: 10, wins: 10, draws: 0, losses: 0, margin_sum: 0 };
        let (plo, phi) = perfect.win_rate_ci95();
        assert!(plo <= 1.0 && (phi - 1.0).abs() < 1e-9);
    }

    #[test]
    #[ignore = "slow: full-campaign agent tournament (all six agents incl. MCTS); run with --ignored --nocapture"]
    fn tournament_campaign_agent_strength() {
        // Full round-robin over the Campaign Game with the whole agent roster. Prints the standings
        // (see with --nocapture) and asserts the sanity floor: RandomAgent must not finish top-half.
        use crate::agent::{
            format_standings, round_robin, Contender, CrossPhaseMctsAgent, GreedyAgent,
            LookaheadAgent, MctsAgent, RandomAgent, RolloutAgent,
        };
        let template = build_campaign_game();
        let make_game = || template.clone();
        let contenders = vec![
            Contender::new("Random", |s| Box::new(RandomAgent::new(s))),
            Contender::new("Greedy", |s| Box::new(GreedyAgent::new(s))),
            Contender::new("Lookahead", |s| Box::new(LookaheadAgent::new(s))),
            Contender::new("Rollout", |s| Box::new(RolloutAgent::flat(s))),
            Contender::new("MCTS", |s| Box::new(MctsAgent::standard(s))),
            Contender::new("CrossMCTS", |s| Box::new(CrossPhaseMctsAgent::standard(s))),
        ];
        let t = round_robin(&make_game, &contenders, 2, 4, 0);
        let st = &t.standings;
        println!("\n{}", format_standings(st));
        println!("\n{}", crate::agent::format_head_to_head(&t));
        let random_rank = st.iter().position(|s| s.name == "Random").unwrap();
        assert!(random_rank >= st.len() / 2, "RandomAgent should not finish in the top half");
    }

    #[test]
    fn legal_commands_includes_adjacent_combat() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::command::Command;
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(Nationality::Commonwealth, include_str!("../../../data/reference/unit-characteristics/commonwealth.ron")).unwrap();
        refs.load_unit_characteristics(Nationality::Italian, include_str!("../../../data/reference/unit-characteristics/italian.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        g.phase = Phase::MovementAndCombat;
        g.active_player = Side::Commonwealth;
        let a = HexCoord::new(2, 0);
        let d = HexCoord::new(3, 0);
        g.map.set_hex(a, HexTerrain::Clear);
        g.map.set_hex(d, HexTerrain::Clear);
        g.map.set_hex(HexCoord::new(1, 0), HexTerrain::Clear); // a free hex to move into (not the enemy's)
        // A rifle-infantry attacker (no ammo cost) adjacent to an enemy-held hex.
        let inf = g.spawn_unit(Nationality::Commonwealth, "h", "inf", Composition::Organic { current_toe: 8 });
        g.unit_mut(inf).unwrap().location = Some(UnitLocation::OnMap(a));
        let foe = g.spawn_unit(Nationality::Italian, "p", "foe", Composition::Organic { current_toe: 6 });
        g.unit_mut(foe).unwrap().location = Some(UnitLocation::OnMap(d));

        let cmds = g.legal_commands(Side::Commonwealth);
        assert!(cmds.iter().any(|c| matches!(c, Command::CloseAssault { target, .. } if *target == d)),
            "a close assault against the adjacent enemy hex is enumerated");
        assert!(cmds.iter().any(|c| matches!(c, Command::MoveStep { .. })), "moves are enumerated too");
        // dry_run does not mutate the caller's state.
        let before = g.unit(foe).unwrap().total_toe();
        let _ = g.dry_run(&Command::CloseAssault { attackers: vec![inf], target: d });
        assert_eq!(g.unit(foe).unwrap().total_toe(), before, "dry_run leaves state untouched");
    }

    #[test]
    fn campaign_fuzz_survives_random_play() {
        // Robustness: drive the full campaign with thousands of random (often illegal) commands and
        // confirm the command boundary never panics/overflows — illegal commands must return Err,
        // not crash. Deterministic (fixed local PRNG) so a failure is reproducible.
        use crate::rules::movement::MoveMode;
        use crate::state::command::Command;
        use crate::state::game::Phase;
        use crate::state::map::HexCoord;

        struct Lcg(u64);
        impl Lcg {
            fn next(&mut self) -> u64 {
                self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                self.0
            }
            fn below(&mut self, n: usize) -> usize {
                if n == 0 { 0 } else { ((self.next() >> 33) as usize) % n }
            }
        }

        let mut g = build_campaign_game();
        let mut r = Lcg(0x00C0_FFEE_1234_5678);
        let mut steps = 0u32;
        while g.game_turn <= 6 && steps < 40_000 {
            steps += 1;
            if g.phase == Phase::MovementAndCombat && r.below(3) != 0 {
                let side = g.active_player;
                let ids: Vec<_> = g
                    .units
                    .values()
                    .filter(|u| u.nationality.side() == side && u.hex().is_some())
                    .map(|u| u.id)
                    .collect();
                if !ids.is_empty() {
                    let unit = ids[r.below(ids.len())];
                    let from = g.unit(unit).unwrap().hex().unwrap();
                    let neighbors: Vec<HexCoord> = g.map.neighbors(from).collect();
                    let to = neighbors[r.below(neighbors.len())];
                    let cmd = match r.below(4) {
                        0 => Command::MoveStep { unit, to, mode: MoveMode::Motorized },
                        1 => Command::MoveStep { unit, to, mode: MoveMode::NonMotorized },
                        2 => Command::CloseAssault { attackers: vec![unit], target: to },
                        _ => Command::AntiArmor { attackers: vec![unit], target_hex: to, withheld: vec![] },
                    };
                    let _ = g.apply(cmd); // Err is expected and fine; a panic would fail the test
                    continue;
                }
            }
            g.advance_phase();
        }
        assert!(g.game_turn >= 2, "the fuzzed campaign still advanced through the sequence of play");
        // No unit should have been left with an impossible strength by any accepted command.
        assert!(g.units.values().all(|u| u.total_toe() <= 1000), "TOE strengths stayed sane");
    }

    #[test]
    fn campaign_driver_advances_five_turns() {
        // End-to-end integration: build the whole engine from the reference bootstrap, deploy a
        // scenario onto the full map, and walk five complete Game-Turns through the Sequence of
        // Play. Exercises the turn driver + every per-stage/per-turn hook (weather, SGSU upkeep,
        // §51 Stores Expenditure, replacement/truck arrivals, victory checks) against real data.
        use crate::state::game::{GameState, Phase};

        let mut g = GameState::new(ReferenceData::load_campaign(), 1);
        for s in [
            include_str!("../../../data/maps/section-a.ron"),
            include_str!("../../../data/maps/section-b.ron"),
            include_str!("../../../data/maps/section-c.ron"),
            include_str!("../../../data/maps/section-d.ron"),
            include_str!("../../../data/maps/section-e.ron"),
        ] {
            g.map.load_section(s).unwrap();
        }
        g.load_scenario(include_str!("../../../data/scenarios/italian-offensive.ron")).unwrap();
        g.auto_place_dump_choices();
        assert!(!g.units.is_empty(), "scenario deployed units");

        let mut phases: Vec<Phase> = Vec::new();
        let mut max_stage = 0u8;
        let mut guard = 0;
        while g.game_turn <= 5 {
            if !phases.contains(&g.phase) {
                phases.push(g.phase);
            }
            max_stage = max_stage.max(g.operations_stage);
            g.advance_phase();
            guard += 1;
            assert!(guard < 100_000, "turn loop failed to progress");
        }
        assert_eq!(g.game_turn, 6, "advanced through five full Game-Turns");
        assert_eq!(max_stage, 3, "three Operations Stages per Game-Turn");
        for p in [
            Phase::Weather,
            Phase::Organization,
            Phase::MovementAndCombat,
            Phase::NavalConvoyArrival,
            Phase::InitiativeDetermination,
        ] {
            assert!(phases.contains(&p), "phase {p:?} was never entered");
        }
    }

    #[test]
    fn campaign_bootstrap_enables_production_commands() {
        // The reference bootstrap makes commands that previously returned NoTable work end-to-end.
        use crate::reference::units::Nationality;
        use crate::state::command::{Command, CommandOutcome};
        use crate::state::game::{GameState, Phase};

        let mut g = GameState::new(ReferenceData::load_campaign(), 1);
        let mut guard = 0;
        while g.phase != Phase::Organization {
            g.advance_phase();
            guard += 1;
            assert!(guard < 1000, "never reached an Organization phase");
        }
        // §20.78A: works only because load_campaign loaded the Commonwealth Truck Production table.
        assert!(matches!(
            g.apply(Command::ProduceCommonwealthTrucks { light_die: 3, medium_die: 3, heavy_die: 3 }),
            Ok(CommandOutcome::TrucksProduced { .. })
        ));
        // §20.66: PlanProduction needs the German production chart AND the Convoy Level Chart for the
        // §20.62 tonnage gate — both bootstrapped. German Infantry is available from GT38.
        g.game_turn = 40;
        assert!(matches!(
            g.apply(Command::PlanProduction { nationality: Nationality::German, item: "Infantry".into(), count: 1 }),
            Ok(CommandOutcome::ProductionPlanned { .. })
        ));
    }

    #[test]
    fn generated_ob_drafts_parse() {
        for (nat, ron, expect) in [
            (Nationality::Italian, include_str!("../../../docs/source-materials/ob-generated/italian.ron"), 34usize),
            (Nationality::German, include_str!("../../../docs/source-materials/ob-generated/german.ron"), 11),
            (Nationality::Commonwealth, include_str!("../../../docs/source-materials/ob-generated/commonwealth.ron"), 36),
        ] {
            let mut refs = ReferenceData::new();
            refs.load_order_of_battle(nat, ron)
                .unwrap_or_else(|e| panic!("{nat:?} OB draft failed to parse: {e}"));
            assert_eq!(refs.orders_of_battle[&nat].formations.len(), expect);
        }
    }

    #[test]
    fn ob_weapon_systems_are_valid() {
        // Every weapon system named in a loaded OB must exist in that nation's §4.47-49 chart.
        use crate::reference::ob::ObComposition;
        let mut refs = ReferenceData::new();
        let nations = [
            (
                Nationality::German,
                include_str!("../../../data/reference/weapons/german.ron"),
                include_str!("../../../data/ob/german.ron"),
            ),
            (
                Nationality::Italian,
                include_str!("../../../data/reference/weapons/italian.ron"),
                include_str!("../../../data/ob/italian.ron"),
            ),
            (
                Nationality::Commonwealth,
                include_str!("../../../data/reference/weapons/commonwealth.ron"),
                include_str!("../../../data/ob/commonwealth.ron"),
            ),
        ];
        for (nat, wpn, ob) in nations {
            refs.load_weapons_systems(nat, wpn).unwrap();
            refs.load_order_of_battle(nat, ob).unwrap();
        }
        for (nat, _, _) in nations {
            let chart = &refs.weapons_systems[&nat];
            for form in &refs.orders_of_battle[&nat].formations {
                for u in &form.units {
                    if let ObComposition::Weapons(groups) = &u.comp {
                        for (system, _) in groups {
                            assert!(
                                chart.contains_key(system),
                                "{:?}: OB unit {:?} weapon {:?} not in the weapons chart",
                                nat, u.name, system
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn ob_ids_are_valid_characteristics_keys() {
        // Every id code used in a loaded OB must exist in the §4.46 chart, and every
        // parent reference must resolve to a unit within the same formation.
        use std::collections::HashSet;
        let mut refs = ReferenceData::new();
        let nations = [
            (
                Nationality::German,
                include_str!("../../../data/reference/unit-characteristics/german.ron"),
                include_str!("../../../data/ob/german.ron"),
            ),
            (
                Nationality::Italian,
                include_str!("../../../data/reference/unit-characteristics/italian.ron"),
                include_str!("../../../data/ob/italian.ron"),
            ),
            (
                Nationality::Commonwealth,
                include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
                include_str!("../../../data/ob/commonwealth.ron"),
            ),
        ];
        for (nat, chars_ron, ob_ron) in nations {
            refs.load_unit_characteristics(nat, chars_ron).unwrap();
            refs.load_order_of_battle(nat, ob_ron).unwrap();
        }
        for (nat, _, _) in nations {
            let chars = &refs.unit_characteristics[&nat];
            for form in &refs.orders_of_battle[&nat].formations {
                let names: HashSet<&str> = form.units.iter().map(|u| u.name.as_str()).collect();
                for u in &form.units {
                    assert!(
                        !u.id.is_empty() && chars.contains_key(&u.id),
                        "{:?}: OB unit {:?} (formation {:?}) has id {:?} not in characteristics",
                        nat, u.name, form.name, u.id
                    );
                    assert!(
                        u.parent.is_empty() || names.contains(u.parent.as_str()),
                        "{:?}: OB unit {:?} parent {:?} does not resolve in formation {:?}",
                        nat, u.name, u.parent, form.name
                    );
                    // A unit must never be organizationally larger than its parent HQ.
                    if let Some(p) = form.units.iter().find(|p| p.name == u.parent) {
                        assert!(
                            u.org_size.size_rank() <= p.org_size.size_rank(),
                            "{:?}: OB unit {:?} ({:?}) is larger than its parent {:?} ({:?})",
                            nat, u.name, u.org_size, p.name, p.org_size
                        );
                    }
                    // §9.24: every artillery unit is a battalion-equivalent (Stacking Point 1),
                    // except scattered batteries (company, SP 0) and the rare Artillery HQ — it may
                    // never be brigade-or-larger regardless of a "Regiment"/"Reggimento" designation
                    // (§9.22: game size follows the printed Stacking Point value, not the counter name).
                    if chars[&u.id].unit_type.contains("Artillery") && !u.name.ends_with("HQ") {
                        assert!(
                            matches!(u.org_size, crate::state::unit::OrgSize::Battalion | crate::state::unit::OrgSize::Company),
                            "{:?}: §9.24 artillery unit {:?} has org_size {:?} (must be Battalion or Company)",
                            nat, u.name, u.org_size
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn loads_italian_order_of_battle() {
        use crate::reference::ob::{Arrival, ObComposition, Toe};

        let mut refs = ReferenceData::new();
        refs.load_order_of_battle(
            Nationality::Italian,
            include_str!("../../../data/ob/italian.ron"),
        )
        .unwrap();
        let ob = refs.orders_of_battle.get(&Nationality::Italian).unwrap();
        assert_eq!(ob.formations.len(), 38);

        let cat = ob.formation("64 Catanzaro Division").expect("Catanzaro");
        assert_eq!(cat.units.len(), 13);
        assert_eq!(cat.basic_morale, -1);
        assert!(!cat.abstracted);
        // Div HQ is the root (no parent); a battalion parents to its regiment.
        let hq = cat.units.iter().find(|u| u.abbr == "64 Cat").unwrap();
        assert_eq!(hq.parent, "");
        assert!(matches!(hq.comp, ObComposition::Organic(Toe::Normal)));
        assert!(matches!(hq.arrival, Arrival::Deployed));
        let bn = cat.units.iter().find(|u| u.abbr == "1/141").unwrap();
        assert_eq!(bn.parent, "141st Infantry Regt HQ");
        // Artillery regiment carries explicit weapon groups.
        let arty = cat.units.iter().find(|u| u.abbr == "203").unwrap();
        assert!(matches!(&arty.comp, ObComposition::Weapons(w) if w.len() == 3));

        // The template divisions are flagged abstracted (D13).
        assert!(ob.formation("62 Marmarica Division").unwrap().abstracted);
        assert!(ob.formation("63 Cirene Division").unwrap().abstracted);

        // Ariete (verified from its §4.44b sheet): a reinforcement (arrives GT18), so not
        // abstracted and not deployed at Scenario-1 start.
        let ariete = ob.formation("132 Ariete Armored Division").expect("Ariete");
        assert!(!ariete.abstracted);
        assert_eq!(ariete.units.len(), 14);
        assert!(matches!(
            ariete.units[0].arrival,
            crate::reference::ob::Arrival::Turn { game_turn: 18, stage: 1 }
        ));

        // 185 Folgore (§42.41): the airborne infantry battalions are airdrop-eligible; the
        // division/regiment HQs and artillery are not. This is what makes AirdropUnit usable.
        let folgore = ob.formation("185 Folgore Airborne Division").expect("Folgore");
        assert_eq!(folgore.basic_morale, 1);
        for bn in ["V/186", "VI/186", "VII/186", "II/187", "IV/187", "IX/187"] {
            let u = folgore.units.iter().find(|u| u.abbr == bn).unwrap();
            assert!(u.airdroppable, "{bn} should be airdroppable");
        }
        for hq in ["185 Folg", "186", "187", "185"] {
            let u = folgore.units.iter().find(|u| u.abbr == hq).unwrap();
            assert!(!u.airdroppable, "{hq} should not be airdroppable");
        }
    }

    #[test]
    fn loads_german_order_of_battle() {
        let mut refs = ReferenceData::new();
        refs.load_order_of_battle(Nationality::German, include_str!("../../../data/ob/german.ron"))
            .unwrap();
        let ob = refs.orders_of_battle.get(&Nationality::German).unwrap();
        // Full §4.44c roster: all 11 German OA formations.
        assert_eq!(ob.formations.len(), 11);

        // Fallschirmjaeger-Brigade Ramcke (§42.41): its five airborne battalions are airdrop-
        // eligible; the brigade HQ is not. This gives the German side an AirdropUnit formation.
        let ramcke = ob.formation("Fallschirmjaeger-Brigade Ramcke").expect("Ramcke");
        let airborne = ramcke.units.iter().filter(|u| u.airdroppable).count();
        assert_eq!(airborne, 5, "five Ramcke battalions airdroppable");
        let hq = ramcke.units.iter().find(|u| u.abbr == "Ramcke").unwrap();
        assert!(!hq.airdroppable, "the brigade HQ is not airdroppable");
    }

    #[test]
    fn rommel_cpa_designation_and_recall_31_4() {
        // §31.4: Rommel grants +5 CPA to one designated co-located unit while he stays with it;
        // §31 recall (in Germany) suspends it; moving off his start-of-stage hex forfeits it.
        use crate::reference::units::{Nationality, Side};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase, RommelState};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::German,
            include_str!("../../../data/reference/unit-characteristics/german.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let hex = HexCoord::new(5, 5);
        let u = g.spawn_unit(Nationality::German, "h", "II/104", Composition::Organic { current_toe: 5 });
        g.unit_mut(u).unwrap().location = Some(UnitLocation::OnMap(hex));
        assert_eq!(g.effective_cpa(u), Some(10), "code h leg infantry CPA");

        g.rommel = Some(RommelState {
            arrival_game_turn: 1, start_hex: Some(hex), hex: Some(hex), in_germany: false,
            mcp_left: 600, stage_start_hex: Some(hex), cpa_unit: None,
        });
        g.phase = Phase::MovementAndCombat;
        g.active_player = Side::Axis;

        // Designating a non-co-located unit is rejected; the co-located one is accepted.
        let elsewhere = g.spawn_unit(Nationality::German, "h", "far", Composition::Organic { current_toe: 5 });
        g.unit_mut(elsewhere).unwrap().location = Some(UnitLocation::OnMap(HexCoord::new(9, 9)));
        assert_eq!(g.apply(Command::DesignateRommelUnit { unit: elsewhere }), Err(CommandError::RommelCannotMove));
        assert_eq!(g.apply(Command::DesignateRommelUnit { unit: u }), Ok(CommandOutcome::RommelUnitDesignated));
        assert_eq!(g.effective_cpa(u), Some(15), "§31.4 +5");

        // §31 recall suspends every Rommel effect this Game-Turn.
        g.rommel.as_mut().unwrap().in_germany = true;
        assert_eq!(g.effective_cpa(u), Some(10));
        g.rommel.as_mut().unwrap().in_germany = false;
        assert_eq!(g.effective_cpa(u), Some(15));

        // §31.4: if Rommel leaves the hex he began the stage in, the bonus lapses.
        g.rommel.as_mut().unwrap().hex = Some(HexCoord::new(6, 5));
        assert_eq!(g.effective_cpa(u), Some(10));
    }

    #[test]
    fn rommel_moves_and_retreats_through_zoc_31() {
        // §31.0-1/§31.2: the Rommel counter moves as a motorized medium truck spending his 60-CP
        // allowance and is blocked by an enemy ZOC on a normal move, but may retreat through it.
        use crate::reference::units::{Nationality, Side};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase, RommelState};
        use crate::state::map::HexId;
        use crate::state::unit::{Composition, UnitLocation};

        let mut g = GameState::new(ReferenceData::load_campaign(), 1);
        for s in [
            include_str!("../../../data/maps/section-a.ron"),
            include_str!("../../../data/maps/section-b.ron"),
            include_str!("../../../data/maps/section-c.ron"),
            include_str!("../../../data/maps/section-d.ron"),
            include_str!("../../../data/maps/section-e.ron"),
        ] {
            g.map.load_section(s).unwrap();
        }
        let start = HexId::parse("C3617").unwrap().to_hex_coord();
        g.rommel = Some(RommelState {
            arrival_game_turn: 1, start_hex: Some(start), hex: Some(start), in_germany: false,
            mcp_left: 600, stage_start_hex: Some(start), cpa_unit: None,
        });
        g.phase = Phase::MovementAndCombat;
        g.active_player = Side::Axis;

        // Pick an on-map neighbour to move into, and a second CW-occupied hex adjacent to it so the
        // target lies in a Commonwealth ZOC.
        let target = g.map.neighbors(start).find(|&h| g.map.terrain(h).is_some()).expect("a neighbour on map");
        let cw_hex = g.map.neighbors(target).find(|&h| h != start && g.map.terrain(h).is_some()).expect("a second neighbour");
        let cw = g.spawn_unit(Nationality::Commonwealth, "k", "guard", Composition::Organic { current_toe: 5 });
        g.unit_mut(cw).unwrap().location = Some(UnitLocation::OnMap(cw_hex));

        if g.hex_in_zoc_of(target, Side::Commonwealth) {
            // §31.2: a normal move into the enemy ZOC is blocked, but a retreat passes with impunity.
            assert_eq!(g.apply(Command::MoveRommel { to: target }), Err(CommandError::RommelCannotMove));
            match g.apply(Command::RetreatRommel { to: target }) {
                Ok(CommandOutcome::RommelMoved { hex, mcp_left }) => {
                    assert_eq!(hex, target);
                    assert_eq!(mcp_left, 600, "a retreat spends no Movement Allowance");
                }
                other => panic!("unexpected: {other:?}"),
            }
        } else {
            // No ZOC on the target: a normal move succeeds and spends Movement Allowance.
            match g.apply(Command::MoveRommel { to: target }) {
                Ok(CommandOutcome::RommelMoved { hex, mcp_left }) => {
                    assert_eq!(hex, target);
                    assert!(mcp_left < 600, "a normal move spends Movement Allowance");
                }
                other => panic!("unexpected: {other:?}"),
            }
        }
        // A distant, non-adjacent target is always rejected (Rommel moves one hex at a time).
        assert_eq!(
            g.apply(Command::MoveRommel { to: HexId::parse("E3613").unwrap().to_hex_coord() }),
            Err(CommandError::RommelCannotMove)
        );
    }

    #[test]
    fn desert_raider_form_move_spot_27() {
        // §27.1/§27.13 formation limits, §27.3/§27.34 ZOC-ignoring movement, §27.43 spotting, and
        // §27.16 four-Game-Turn re-form cooldown.
        use crate::reference::units::{Nationality, Side};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase, RaiderKind};
        use crate::state::map::HexId;
        use crate::state::unit::{Composition, UnitLocation};

        let mut g = GameState::new(ReferenceData::load_campaign(), 1);
        for s in [
            include_str!("../../../data/maps/section-a.ron"),
            include_str!("../../../data/maps/section-b.ron"),
            include_str!("../../../data/maps/section-c.ron"),
            include_str!("../../../data/maps/section-d.ron"),
            include_str!("../../../data/maps/section-e.ron"),
        ] {
            g.map.load_section(s).unwrap();
        }
        let base = HexId::parse("C3617").unwrap().to_hex_coord();
        // A Commonwealth reconnaissance unit ("hh") and a plain infantry unit ("k").
        let recce = g.spawn_unit(Nationality::Commonwealth, "hh", "KDG", Composition::Organic { current_toe: 6 });
        g.unit_mut(recce).unwrap().location = Some(UnitLocation::OnMap(base));
        let inf = g.spawn_unit(Nationality::Commonwealth, "k", "1 Bn", Composition::Organic { current_toe: 6 });
        g.unit_mut(inf).unwrap().location = Some(UnitLocation::OnMap(base));

        g.phase = Phase::Organization;
        // §27.13: infantry cannot form an LRDG; a Recce unit can (consuming one TOE point).
        assert_eq!(g.apply(Command::FormRaider { kind: RaiderKind::Lrdg, from_unit: inf }), Err(CommandError::RaiderNotAvailable));
        let idx = match g.apply(Command::FormRaider { kind: RaiderKind::Lrdg, from_unit: recce }) {
            Ok(CommandOutcome::RaiderFormed { raider }) => raider,
            other => panic!("unexpected: {other:?}"),
        };
        assert_eq!(g.unit(recce).unwrap().total_toe(), 5, "one TOE point removed");
        assert_eq!(g.raiders_in_play(RaiderKind::Lrdg), 1);
        assert_eq!(g.raiders[idx].hex, Some(base));
        g.raiders[idx].fuel = 2; // §27.35: fuelled so it may travel this Game-Turn

        // Move the raider one hex on the Commonwealth turn, ignoring enemy ZOC.
        g.phase = Phase::MovementAndCombat;
        g.active_player = Side::Commonwealth;
        let target = g.map.neighbors(base).find(|&h| g.map.terrain(h).is_some()).unwrap();
        // Put an Axis unit adjacent to `target` so `target` lies in an Axis ZOC.
        let axis_hex = g.map.neighbors(target).find(|&h| h != base && g.map.terrain(h).is_some()).unwrap();
        let axis = g.spawn_unit(Nationality::German, "k", "guard", Composition::Organic { current_toe: 5 });
        g.unit_mut(axis).unwrap().location = Some(UnitLocation::OnMap(axis_hex));

        let controlled = g.hex_in_zoc_of(target, Side::Axis);
        // A non-6 spot roll: the raider moves (through the ZOC) and survives.
        match g.apply(Command::MoveRaider { raider: idx, to: target, spot_roll: 1 }) {
            Ok(CommandOutcome::RaiderMoved { hex, eliminated, .. }) => {
                assert_eq!(hex, target);
                assert!(!eliminated);
            }
            other => panic!("unexpected: {other:?}"),
        }

        if controlled {
            // §27.43: a 6 while in an enemy-controlled hex spots and eliminates the raider.
            let back = base;
            match g.apply(Command::MoveRaider { raider: idx, to: back, spot_roll: 1 }) {
                Ok(CommandOutcome::RaiderMoved { .. }) => {}
                other => panic!("unexpected: {other:?}"),
            }
            match g.apply(Command::MoveRaider { raider: idx, to: target, spot_roll: 6 }) {
                Ok(CommandOutcome::RaiderMoved { eliminated, .. }) => assert!(eliminated),
                other => panic!("unexpected: {other:?}"),
            }
            assert_eq!(g.raiders_in_play(RaiderKind::Lrdg), 0, "spotted raider removed");

            // §27.16: cannot re-form this Game-Turn; can four Game-Turns later.
            g.phase = Phase::Organization;
            g.unit_mut(recce).unwrap().composition = Composition::Organic { current_toe: 5 };
            assert_eq!(g.apply(Command::FormRaider { kind: RaiderKind::Lrdg, from_unit: recce }), Err(CommandError::RaiderNotAvailable));
            g.game_turn += 4;
            assert!(matches!(g.apply(Command::FormRaider { kind: RaiderKind::Lrdg, from_unit: recce }), Ok(CommandOutcome::RaiderFormed { .. })));
        }
    }

    #[test]
    fn raid_on_rommel_table_27_64() {
        use crate::reference::units::{Nationality, Side};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase, Raider, RaiderKind, RommelState};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let hex = HexCoord::new(5, 5);
        let mut refs = ReferenceData::new();
        refs.load_initiative_ratings(include_str!("../../../data/reference/initiative-ratings.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        g.game_turn = 10;
        g.phase = Phase::MovementAndCombat;
        g.active_player = Side::Commonwealth;

        // Helper: (re)place Rommel and one Commonwealth LRDG in `hex`, raid count reset.
        let reset = |g: &mut GameState| {
            g.rommel = Some(RommelState {
                arrival_game_turn: 1, start_hex: Some(hex), hex: Some(hex), in_germany: false,
                mcp_left: 600, stage_start_hex: Some(hex), cpa_unit: None,
            });
            g.raiders = vec![Raider {
                kind: RaiderKind::Lrdg, side: Side::Commonwealth, hex: Some(hex),
                cpa_used: 0, fuel: 0, ammo: 0, drew_water_this_gt: false, reform_game_turn: 0, revealed: false,
            }];
            g.raids_on_rommel = 0;
            g.rommel_raid_ai3_until_gt = 0;
        };

        // Roll 12: "AI perm 3" — Rommel is captured and removed from the maps.
        reset(&mut g);
        assert_eq!(
            g.apply(Command::RaidRommel { raider: 0, roll: 12 }),
            Ok(CommandOutcome::RommelRaided { raider_eliminated: false, rommel_removed: true, axis_temp3: false })
        );
        assert!(g.rommel.is_none());

        // Roll 2: "AI temp 3" — Rommel stays, but the Axis rating is 3 for the next two Game-Turns.
        reset(&mut g);
        assert_eq!(
            g.apply(Command::RaidRommel { raider: 0, roll: 2 }),
            Ok(CommandOutcome::RommelRaided { raider_eliminated: false, rommel_removed: false, axis_temp3: true })
        );
        assert!(g.rommel_on_map());
        assert_eq!(g.axis_initiative_rating(), 3, "temp-3 overrides Rommel's 6");
        g.game_turn = 13; // window was GT10..12
        assert_eq!(g.axis_initiative_rating(), 6, "window expired, Rommel on map -> 6");
        g.game_turn = 10;

        // Roll 9 "LRDG elim": survives when Rommel is alone; eliminated when combat/HQ units share.
        reset(&mut g);
        assert_eq!(
            g.apply(Command::RaidRommel { raider: 0, roll: 9 }),
            Ok(CommandOutcome::RommelRaided { raider_eliminated: false, rommel_removed: false, axis_temp3: false })
        );
        reset(&mut g);
        let guard = g.spawn_unit(Nationality::German, "k", "guard", Composition::Organic { current_toe: 5 });
        g.unit_mut(guard).unwrap().location = Some(UnitLocation::OnMap(hex));
        assert_eq!(
            g.apply(Command::RaidRommel { raider: 0, roll: 9 }),
            Ok(CommandOutcome::RommelRaided { raider_eliminated: true, rommel_removed: false, axis_temp3: false })
        );
        assert_eq!(g.raiders_in_play(RaiderKind::Lrdg), 0);

        // §27.64: at most four Raids on Rommel per campaign.
        reset(&mut g);
        g.units.clear();
        for _ in 0..4 {
            assert!(g.apply(Command::RaidRommel { raider: 0, roll: 5 }).is_ok());
        }
        assert_eq!(g.apply(Command::RaidRommel { raider: 0, roll: 5 }), Err(CommandError::RaiderCannotMove));
    }

    #[test]
    fn free_truck_convoy_form_and_move_8_98() {
        // §8.97/§8.98: detach first-line trucks into a free convoy and move it in the Truck Convoy
        // Phase, spending its supply CPA and observing the §10.29 ZOC restriction.
        use crate::reference::units::{Nationality, Side};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexId;
        use crate::state::unit::{Composition, TruckPoints, UnitLocation};

        let mut g = GameState::new(ReferenceData::load_campaign(), 1);
        for s in [
            include_str!("../../../data/maps/section-a.ron"),
            include_str!("../../../data/maps/section-b.ron"),
            include_str!("../../../data/maps/section-c.ron"),
            include_str!("../../../data/maps/section-d.ron"),
            include_str!("../../../data/maps/section-e.ron"),
        ] {
            g.map.load_section(s).unwrap();
        }
        let base = HexId::parse("C3617").unwrap().to_hex_coord();
        let u = g.spawn_unit(Nationality::Commonwealth, "k", "supply col", Composition::Organic { current_toe: 5 });
        g.unit_mut(u).unwrap().location = Some(UnitLocation::OnMap(base));
        g.unit_mut(u).unwrap().trucks = TruckPoints { light: 0, medium: 10, heavy: 0 };

        g.phase = Phase::Organization;
        // Detach 6 Medium Truck Points into a free convoy.
        let convoy = match g.apply(Command::FormTruckConvoy { from_unit: u, light: 0, medium: 6, heavy: 0 }) {
            Ok(CommandOutcome::TruckConvoyFormed { convoy }) => convoy,
            other => panic!("unexpected: {other:?}"),
        };
        assert_eq!(g.unit(u).unwrap().trucks.medium, 4, "trucks left the unit");
        assert_eq!(g.truck_convoys[convoy].trucks.medium, 6);
        assert_eq!(g.truck_convoys[convoy].hex, base);

        // Move it one hex in the Truck Convoy Phase; it spends supply CPA.
        g.phase = Phase::TruckConvoy;
        g.active_player = Side::Commonwealth;
        let target = g.map.neighbors(base).find(|&h| g.map.terrain(h).is_some()).unwrap();
        match g.apply(Command::MoveTruckConvoy { convoy, to: target }) {
            Ok(CommandOutcome::TruckConvoyMoved { hex, cpa_used }) => {
                assert_eq!(hex, target);
                assert!(cpa_used > 0);
            }
            other => panic!("unexpected: {other:?}"),
        }
        // A non-adjacent target is rejected.
        assert_eq!(
            g.apply(Command::MoveTruckConvoy { convoy, to: HexId::parse("E3613").unwrap().to_hex_coord() }),
            Err(CommandError::TruckConvoyCannotMove)
        );
    }

    #[test]
    fn desert_raider_raids_27_5() {
        // §27.52 pipeline, §27.55 supply dump (unguarded + guarded), §27.57 truck convoy.
        use crate::reference::units::{Nationality, Side};
        use crate::state::command::{Command, CommandOutcome};
        use crate::state::game::{Commodities, GameState, Phase, Raider, RaiderKind, TruckConvoy};
        use crate::state::map::HexId;
        use crate::state::unit::{Composition, TruckPoints, UnitLocation};

        let mut g = GameState::new(ReferenceData::load_campaign(), 1);
        for s in [
            include_str!("../../../data/maps/section-a.ron"),
            include_str!("../../../data/maps/section-b.ron"),
            include_str!("../../../data/maps/section-c.ron"),
            include_str!("../../../data/maps/section-d.ron"),
            include_str!("../../../data/maps/section-e.ron"),
        ] {
            g.map.load_section(s).unwrap();
        }
        let hex = HexId::parse("C3617").unwrap().to_hex_coord();
        g.phase = Phase::MovementAndCombat;
        g.active_player = Side::Commonwealth;
        let place_raider = |g: &mut GameState| {
            g.raiders = vec![Raider {
                kind: RaiderKind::Lrdg, side: Side::Commonwealth, hex: Some(hex),
                cpa_used: 0, fuel: 0, ammo: 0, drew_water_this_gt: false, reform_game_turn: 0, revealed: false,
            }];
        };

        // §27.52: blow a pipeline (a 1-4 succeeds); the pipeline is removed.
        place_raider(&mut g);
        g.pipelines.insert(hex);
        assert_eq!(
            g.apply(Command::RaidPipeline { raider: 0, roll: 3 }),
            Ok(CommandOutcome::RaidResolved { success: true, raider_eliminated: false })
        );
        assert!(!g.pipelines.contains(&hex));

        // §27.55a: an unguarded dump — a 1-2 destroys 10% of each commodity.
        place_raider(&mut g);
        g.supply_dumps.push(crate::state::game::SupplyDump {
            location: UnitLocation::OnMap(hex), side: Side::Axis,
            supply: Commodities { fuel: 100, ammo: 100, stores: 100, water: 0 }, dummy: false,
        });
        assert_eq!(
            g.apply(Command::RaidSupplyDump { raider: 0, attack_roll: 0, destroy_roll: 1 }),
            Ok(CommandOutcome::RaidResolved { success: true, raider_eliminated: false })
        );
        let d = g.supply_dumps.last().unwrap();
        assert_eq!((d.supply.fuel, d.supply.ammo), (90, 90), "10% destroyed");

        // §27.55b: a guarded dump — a low 2d6 attack roll (≤ guards' defence) eliminates the raider.
        place_raider(&mut g);
        let guard = g.spawn_unit(Nationality::German, "k", "guard", Composition::Organic { current_toe: 5 });
        g.unit_mut(guard).unwrap().location = Some(UnitLocation::OnMap(hex));
        match g.apply(Command::RaidSupplyDump { raider: 0, attack_roll: 2, destroy_roll: 1 }) {
            Ok(CommandOutcome::RaidResolved { success: false, raider_eliminated: true }) => {}
            other => panic!("unexpected: {other:?}"),
        }
        assert_eq!(g.raiders_in_play(RaiderKind::Lrdg), 0);

        // §27.57: raid a free truck convoy — a 1-2 destroys one Truck Point.
        place_raider(&mut g);
        g.units.clear(); // remove the guard so the convoy raid isn't blocked by combat units
        g.truck_convoys.push(TruckConvoy {
            side: Side::Axis, hex, trucks: TruckPoints { light: 0, medium: 4, heavy: 0 },
            cargo: Commodities::default(), cpa_used: 0, carries_replacements: false,
        });
        let ci = g.truck_convoys.len() - 1;
        assert_eq!(
            g.apply(Command::RaidTruckConvoy { raider: 0, convoy: ci, roll: 1 }),
            Ok(CommandOutcome::RaidResolved { success: true, raider_eliminated: false })
        );
        assert_eq!(g.truck_convoys[ci].trucks.medium, 3, "one Truck Point destroyed");
    }

    #[test]
    fn desert_raider_supply_27_35() {
        // §27.35: draw 2 Fuel + 1 Ammo from a co-located Friendly dump; a raider that draws no Water
        // in a Game-Turn is eliminated at the next Game-Turn rollover.
        use crate::reference::units::Side;
        use crate::state::command::{Command, CommandOutcome};
        use crate::state::game::{Commodities, GameState, Phase, Raider, RaiderKind, SupplyDump};
        use crate::state::map::HexCoord;
        use crate::state::unit::UnitLocation;

        let mut g = GameState::new(ReferenceData::new(), 1);
        let hex = HexCoord::new(4, 4);
        g.supply_dumps.push(SupplyDump {
            location: UnitLocation::OnMap(hex), side: Side::Commonwealth,
            supply: Commodities { fuel: 10, ammo: 5, stores: 0, water: 0 }, dummy: false,
        });
        let mk = |water: bool| Raider {
            kind: RaiderKind::Lrdg, side: Side::Commonwealth, hex: Some(hex),
            cpa_used: 0, fuel: 0, ammo: 0, drew_water_this_gt: water, reform_game_turn: 0, revealed: false,
        };
        g.raiders = vec![mk(false), mk(true)]; // one un-watered, one watered
        g.phase = Phase::MovementAndCombat;
        g.active_player = Side::Commonwealth;

        // §27.35: resupply draws 2 Fuel + 1 Ammo from the dump (no water source at this hex).
        assert_eq!(
            g.apply(Command::ResupplyRaider { raider: 0 }),
            Ok(CommandOutcome::RaiderResupplied { fuel: 2, ammo: 1, water: false })
        );
        assert_eq!((g.raiders[0].fuel, g.raiders[0].ammo), (2, 1));
        let d = g.supply_dumps.last().unwrap();
        assert_eq!((d.supply.fuel, d.supply.ammo), (8, 4), "drawn from the dump");

        // Advance a full Game-Turn: the un-watered raider is eliminated, the watered one survives.
        let start_gt = g.game_turn;
        let mut guard = 0;
        // Advance until the new turn's Initiative Determination has run (which does §27.35 upkeep).
        while !(g.game_turn > start_gt && g.phase != Phase::InitiativeDetermination) {
            g.advance_phase();
            guard += 1;
            assert!(guard < 2000, "turn did not roll over");
        }
        assert_eq!(g.raiders_in_play(RaiderKind::Lrdg), 1, "only the watered raider survived");
        assert!(g.raiders[1].hex.is_some());
        assert!(g.raiders[0].hex.is_none());
    }

    #[test]
    fn layforce_amphibious_landing_27_7() {
        // §27.72/§27.73/§27.74/§27.77: the Layforce commando lands from Alexandria into a coastal
        // hex after a full Game-Turn there, is auto-supplied for three OpStages, and can re-embark.
        use crate::reference::units::{Nationality, Side};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexId;
        use crate::state::unit::{Composition, UnitLocation};

        let mut g = GameState::new(ReferenceData::load_campaign(), 1);
        for s in [
            include_str!("../../../data/maps/section-a.ron"),
            include_str!("../../../data/maps/section-b.ron"),
            include_str!("../../../data/maps/section-c.ron"),
            include_str!("../../../data/maps/section-d.ron"),
            include_str!("../../../data/maps/section-e.ron"),
        ] {
            g.map.load_section(s).unwrap();
        }
        let alex = HexId::parse("E3613").unwrap().to_hex_coord();
        let coast = HexId::parse("C4021").unwrap().to_hex_coord(); // Sollum — coastal
        assert!(g.map.is_coastal(coast));
        let ly = g.spawn_unit(Nationality::Commonwealth, "k", "Layforce A Bn", Composition::Organic { current_toe: 5 });
        g.unit_mut(ly).unwrap().location = Some(UnitLocation::OnMap(alex));
        g.phase = Phase::MovementAndCombat;
        g.active_player = Side::Commonwealth;

        // §27.73: not yet a full Game-Turn in Alexandria → cannot land.
        g.layforce_alexandria_since = Some(2);
        g.game_turn = 2;
        assert_eq!(
            g.apply(Command::LandLayforce { unit: ly, to: coast, ship_hexes: 50 }),
            Err(CommandError::LayforceCannotLand)
        );
        // A non-coastal target is rejected too.
        g.layforce_alexandria_since = Some(1); // entered last Game-Turn → ready this turn
        assert_eq!(
            g.apply(Command::LandLayforce { unit: ly, to: HexId::parse("C0101").unwrap().to_hex_coord(), ship_hexes: 50 }),
            Err(CommandError::LayforceCannotLand)
        );

        // §27.72: land in the coastal hex.
        assert_eq!(
            g.apply(Command::LandLayforce { unit: ly, to: coast, ship_hexes: 50 }),
            Ok(CommandOutcome::LayforceLanded { hex: coast })
        );
        assert_eq!(g.unit(ly).unwrap().hex(), Some(coast));
        // §27.74: auto-supplied on landing despite no dumps present.
        assert!(g.layforce_free_supply_active());
        assert!(g.unit_supply_adequate(ly));

        // §27.77: re-embark → returned to Alexandria, free-supply window closed.
        assert_eq!(g.apply(Command::EmbarkLayforce { unit: ly }), Ok(CommandOutcome::LayforceEmbarked));
        assert_eq!(g.unit(ly).unwrap().hex(), Some(alex));
        assert!(!g.layforce_free_supply_active());
    }

    #[test]
    fn truck_convoy_interception_27_59() {
        // §27.59: a raider within four Movement CPs of a passing enemy convoy may halt and attack it
        // during the Truck Convoy Phase.
        use crate::reference::units::Side;
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{Commodities, GameState, Phase, Raider, RaiderKind, TruckConvoy};
        use crate::state::map::HexId;
        use crate::state::unit::TruckPoints;

        let mut g = GameState::new(ReferenceData::load_campaign(), 1);
        for s in [
            include_str!("../../../data/maps/section-a.ron"),
            include_str!("../../../data/maps/section-b.ron"),
            include_str!("../../../data/maps/section-c.ron"),
            include_str!("../../../data/maps/section-d.ron"),
            include_str!("../../../data/maps/section-e.ron"),
        ] {
            g.map.load_section(s).unwrap();
        }
        let a = HexId::parse("C3617").unwrap().to_hex_coord();
        g.raiders.push(Raider {
            kind: RaiderKind::Lrdg, side: Side::Commonwealth, hex: Some(a),
            cpa_used: 0, fuel: 2, ammo: 0, drew_water_this_gt: false, reform_game_turn: 0, revealed: false,
        });
        // An Axis convoy adjacent (within range) and one far away (out of range).
        let near = g.map.neighbors(a).find(|&h| g.map.terrain(h).is_some()).unwrap();
        g.truck_convoys.push(TruckConvoy {
            side: Side::Axis, hex: near, trucks: TruckPoints { light: 0, medium: 3, heavy: 0 },
            cargo: Commodities::default(), cpa_used: 0, carries_replacements: false,
        });
        g.truck_convoys.push(TruckConvoy {
            side: Side::Axis, hex: HexId::parse("E3613").unwrap().to_hex_coord(),
            trucks: TruckPoints { light: 0, medium: 3, heavy: 0 },
            cargo: Commodities::default(), cpa_used: 0, carries_replacements: false,
        });

        // The Truck Convoy Phase is the Axis (convoy owner's) phase; the CW raider reacts.
        g.phase = Phase::TruckConvoy;
        g.active_player = Side::Axis;
        assert_eq!(
            g.apply(Command::InterceptTruckConvoy { raider: 0, convoy: 0, roll: 1 }),
            Ok(CommandOutcome::RaidResolved { success: true, raider_eliminated: false })
        );
        assert_eq!(g.truck_convoys[0].trucks.medium, 2, "one Truck Point destroyed");
        // The far convoy is out of interception range.
        assert_eq!(
            g.apply(Command::InterceptTruckConvoy { raider: 0, convoy: 1, roll: 1 }),
            Err(CommandError::RaiderCannotMove)
        );
    }

    #[test]
    fn sas_brigade_raid_and_escort_27_8() {
        // §27.85/86/87 airfield raid via the §27.93 table, §27.88 LRDG-escort CPA 30.
        use crate::reference::aircraft::AircraftClass;
        use crate::reference::squadron::SquadronClass;
        use crate::reference::units::{Nationality, Side};
        use crate::state::air::{AirFacility, AirFacilityKind, Sgsu, SquadronId};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase, Raider, RaiderKind};
        use crate::state::map::HexId;
        use crate::state::unit::{Composition, UnitLocation};

        let mut g = GameState::new(ReferenceData::load_campaign(), 1);
        let hex = HexId::parse("C3617").unwrap().to_hex_coord();
        let sas = g.spawn_unit(Nationality::Commonwealth, "k", "SAS Brigade", Composition::Organic { current_toe: 5 });
        g.unit_mut(sas).unwrap().location = Some(UnitLocation::OnMap(hex));
        g.air_facilities.push(AirFacility::new(AirFacilityKind::Airfield, hex));
        let mut sq = Sgsu::new(SquadronId(1), Nationality::German, SquadronClass::Bomber, "KG 26", 5, Some(hex));
        assert!(sq.add_planes("Ju.88D", AircraftClass::Bomber, 12, 1), "planes added");
        g.sgsus.insert(SquadronId(1), sq);
        g.game_turn = 10;
        g.operations_stage = 1;
        g.phase = Phase::MovementAndCombat;
        g.active_player = Side::Commonwealth;

        // §27.86: a d6 of 3 → 25% of the 12 grounded planes destroyed (3).
        assert_eq!(
            g.apply(Command::RaidAirfieldSas { unit: sas, roll: 3 }),
            Ok(CommandOutcome::SasRaided { planes_pct: 25 })
        );
        assert_eq!(g.sgsus[&SquadronId(1)].plane_count(), 9);
        // §27.87: a second raid within six Operations Stages is refused.
        assert_eq!(g.apply(Command::RaidAirfieldSas { unit: sas, roll: 3 }), Err(CommandError::SasCannotAct));

        // §27.88: stacked with an LRDG, the SAS Brigade moves as an LRDG (CPA 30).
        assert_eq!(g.effective_cpa(sas), Some(10), "base infantry CPA");
        g.raiders.push(Raider {
            kind: RaiderKind::Lrdg, side: Side::Commonwealth, hex: Some(hex),
            cpa_used: 0, fuel: 2, ammo: 0, drew_water_this_gt: false, reform_game_turn: 0, revealed: false,
        });
        assert_eq!(g.effective_cpa(sas), Some(30));
    }

    #[test]
    fn observation_layer_hides_enemy_secrets_3_5() {
        // §3.5/D5: a side sees enemy counters but not their TOE Strength, enemy dump contents, or
        // hidden raiders.
        use crate::reference::units::{Nationality, Side};
        use crate::state::game::{Commodities, GameState, Raider, RaiderKind, SupplyDump};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut g = GameState::new(ReferenceData::new(), 1);
        let hex = HexCoord::new(3, 3);
        let cw = g.spawn_unit(Nationality::Commonwealth, "k", "1 Bn", Composition::Organic { current_toe: 5 });
        let ax = g.spawn_unit(Nationality::German, "k", "II/104", Composition::Organic { current_toe: 4 });
        for id in [cw, ax] {
            g.unit_mut(id).unwrap().location = Some(UnitLocation::OnMap(hex));
        }

        // §3.5: from the Commonwealth view, own strength is visible; the enemy's is hidden.
        let units = g.observe_units(Side::Commonwealth);
        let cw_view = units.iter().find(|v| v.id == cw).unwrap();
        let ax_view = units.iter().find(|v| v.id == ax).unwrap();
        assert_eq!((cw_view.friendly, cw_view.strength), (true, Some(5)));
        assert_eq!((ax_view.friendly, ax_view.strength), (false, None), "enemy TOE Strength hidden");

        // §32.18: enemy dump contents/dummy hidden; own dump shown in full.
        g.supply_dumps.push(SupplyDump {
            location: UnitLocation::OnMap(hex), side: Side::Commonwealth,
            supply: Commodities { fuel: 9, ammo: 0, stores: 0, water: 0 }, dummy: false,
        });
        g.supply_dumps.push(SupplyDump {
            location: UnitLocation::OnMap(hex), side: Side::Axis,
            supply: Commodities { fuel: 7, ammo: 0, stores: 0, water: 0 }, dummy: true,
        });
        let dumps = g.observe_supply_dumps(Side::Commonwealth);
        assert!(dumps.iter().any(|d| d.friendly && d.contents == Some(Commodities { fuel: 9, ammo: 0, stores: 0, water: 0 })));
        let enemy_dump = dumps.iter().find(|d| !d.friendly).unwrap();
        assert_eq!((enemy_dump.contents, enemy_dump.dummy), (None, None), "enemy dump contents/dummy hidden");

        // §27.32: a hidden enemy raider is not observed; a revealed one is; own raiders always are.
        let mk = |side, revealed| Raider {
            kind: if side == Side::Commonwealth { RaiderKind::Lrdg } else { RaiderKind::Dsa },
            side, hex: Some(hex), cpa_used: 0, fuel: 0, ammo: 0, drew_water_this_gt: false,
            reform_game_turn: 0, revealed,
        };
        g.raiders.push(mk(Side::Commonwealth, false)); // own — always visible
        g.raiders.push(mk(Side::Axis, false)); // enemy, hidden
        let seen = g.observe_raiders(Side::Commonwealth);
        assert_eq!(seen.len(), 1, "only the own raider is visible");
        assert!(seen[0].friendly);
        g.raiders[1].revealed = true; // §27.33: now spotted/raided
        assert_eq!(g.observe_raiders(Side::Commonwealth).len(), 2, "revealed enemy raider now visible");

        // §27.33: a revealed raider slips back into hiding at the start of the next Operations Stage.
        g.begin_operations_stage();
        assert!(!g.raiders[1].revealed);
        assert_eq!(g.observe_raiders(Side::Commonwealth).len(), 1, "enemy raider hidden again");
    }

    #[test]
    fn observation_minefields_26() {
        // §26: a minefield marker + owner are visible, but real/dummy is hidden until it is the
        // viewer's own or has been revealed.
        use crate::reference::units::Side;
        use crate::state::game::{GameState, Minefield};
        use crate::state::map::HexCoord;

        let mut g = GameState::new(ReferenceData::new(), 1);
        let own = HexCoord::new(1, 1);
        let enemy = HexCoord::new(2, 2);
        g.minefields.insert(own, Minefield { side: Side::Commonwealth, dummy: false });
        g.minefields.insert(enemy, Minefield { side: Side::Axis, dummy: true });

        let mf = g.observe_minefields(Side::Commonwealth);
        let own_v = mf.iter().find(|m| m.hex == own).unwrap();
        let enemy_v = mf.iter().find(|m| m.hex == enemy).unwrap();
        assert_eq!((own_v.friendly, own_v.dummy), (true, Some(false)));
        assert_eq!((enemy_v.friendly, enemy_v.dummy), (false, None), "enemy real/dummy hidden");

        // §26.15: once an enemy field is revealed, its dummy status becomes known.
        g.revealed_minefields.insert(enemy);
        let enemy_v = g.observe_minefields(Side::Commonwealth).into_iter().find(|m| m.hex == enemy).unwrap();
        assert_eq!(enemy_v.dummy, Some(true));
    }

    #[test]
    fn well_depletion_secrecy_52_14() {
        // §52.14: a depleted/poisoned well is secret to everyone but the side that caused it, until
        // an opposing draw reveals it publicly.
        use crate::reference::units::Side;
        use crate::state::game::GameState;
        use crate::state::map::HexCoord;

        let mut g = GameState::new(ReferenceData::new(), 1);
        let hex = HexCoord::new(3, 3);
        g.depleted_wells.insert(hex);
        g.well_known_by.insert(hex, Side::Axis); // the Axis drew it dry and notes it secretly

        assert_eq!(g.observe_well_depleted(Side::Axis, hex), Some(true), "the depleting side knows");
        assert_eq!(g.observe_well_depleted(Side::Commonwealth, hex), None, "secret to the enemy");
        // A well the viewer knows nothing about reads None (not "not depleted").
        assert_eq!(g.observe_well_depleted(Side::Commonwealth, HexCoord::new(9, 9)), None);

        // §52.14: once revealed (an opposing unit drew there), the marker is public.
        g.revealed_wells.insert(hex);
        assert_eq!(g.observe_well_depleted(Side::Commonwealth, hex), Some(true));

        // §52.16 poison secrecy works the same way.
        let bir = HexCoord::new(4, 4);
        g.poisoned_wells.insert(bir);
        g.well_known_by.insert(bir, Side::Commonwealth);
        assert_eq!(g.observe_well_poisoned(Side::Commonwealth, bir), Some(true));
        assert_eq!(g.observe_well_poisoned(Side::Axis, bir), None, "secret to the enemy");
    }

    #[test]
    fn deploy_formation_builds_hierarchy() {
        use crate::state::game::GameState;
        use crate::state::map::HexCoord;

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Italian,
            include_str!("../../../data/reference/unit-characteristics/italian.ron"),
        )
        .unwrap();
        refs.load_order_of_battle(
            Nationality::Italian,
            include_str!("../../../data/ob/italian.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);

        let hex = HexCoord::new(207, 47);
        let ids = g.deploy_formation(Nationality::Italian, "64 Catanzaro Division", hex).unwrap();
        assert_eq!(ids.len(), 13);

        let id = |n: &str| g.units.values().find(|u| u.name == n).unwrap().id;
        let hq = id("64th Catanzaro Div HQ");
        // The Div HQ is the placed, is_hq counter; the whole division is one formation there.
        assert!(g.unit(hq).unwrap().is_hq);
        assert_eq!(g.unit(hq).unwrap().hex(), Some(hex));
        assert_eq!(g.formation_members(hq).len(), 13);
        // A battalion has no own location but resolves (transitively) to the HQ hex.
        let bn = id("I/141 Infantry Bn");
        assert!(g.unit(bn).unwrap().hex().is_none());
        assert_eq!(g.effective_hex(bn), Some(hex));
        // Attachment chain: battalion -> regiment -> division HQ.
        assert_eq!(g.unit(bn).unwrap().attached_parent, Some(id("141st Infantry Regt HQ")));
        assert_eq!(g.unit(id("141st Infantry Regt HQ")).unwrap().attached_parent, Some(hq));
        // Explicit weapon groups preserved (203rd Artillery: 6+3+3 = 12 TOE).
        assert_eq!(g.unit(id("203rd Artillery Regt")).unwrap().total_toe(), 12);
        // Attached units add no stacking; only the Division counter (5 SP) at the hex.
        assert_eq!(g.hex_stacking_points(hex), 5);
    }

    #[test]
    fn deploy_formation_disperses_named_units() {
        use crate::state::game::GameState;
        use crate::state::map::HexId;
        use std::collections::HashMap;

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_order_of_battle(
            Nationality::Commonwealth,
            include_str!("../../../data/ob/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);

        let div_hex = HexId::parse("C3922").unwrap().to_hex_coord();
        let hus_hex = HexId::parse("C3020").unwrap().to_hex_coord();
        let mut unit_hexes = HashMap::new();
        unit_hexes.insert("11th Hussars".to_string(), hus_hex);
        let ids = g
            .deploy_formation_dispersed(
                Nationality::Commonwealth,
                "7th Armored Division",
                div_hex,
                &unit_hexes,
            )
            .unwrap();
        assert_eq!(ids.len(), 13);

        let id = |n: &str| g.units.values().find(|u| u.name == n).unwrap().id;
        let hq = id("7th Armored Div HQE");
        assert_eq!(g.unit(hq).unwrap().hex(), Some(div_hex));

        // 11th Hussars deploys dispersed: its own hex, not co-located-attached, but still
        // organically assigned to its parent (and thus off the division's on-map CPA stack).
        let hus = id("11th Hussars");
        assert_eq!(g.unit(hus).unwrap().hex(), Some(hus_hex));
        assert_eq!(g.effective_hex(hus), Some(hus_hex));
        assert!(g.unit(hus).unwrap().attached_parent.is_none());
        assert_eq!(g.unit(hus).unwrap().assigned_parent, Some(id("7th Support Group HQ")));
        assert!(!g.formation_members(hq).contains(&hus));

        // A non-dispersed sibling stays co-located at the division hex.
        let krrc = id("1st KRRC");
        assert!(g.unit(krrc).unwrap().hex().is_none());
        assert_eq!(g.effective_hex(krrc), Some(div_hex));
    }

    #[test]
    fn reinforcements_arrive_on_schedule() {
        use crate::state::game::GameState;
        use crate::state::map::HexId;

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_order_of_battle(
            Nationality::Commonwealth,
            include_str!("../../../data/ob/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let hex = HexId::parse("D3615").unwrap().to_hex_coord();

        // At start, 4th Indian deploys 12 of 17 units; the 7th Indian Bde (GT5) and
        // 1st Field Arty (GT3.3) are registered as pending arrivals, not on the map.
        g.deploy_formation(Nationality::Commonwealth, "4th Indian Division", hex).unwrap();
        assert_eq!(g.units.len(), 12);
        assert_eq!(g.pending_arrivals.len(), 2);
        let named = |g: &GameState, n: &str| g.units.values().find(|u| u.name == n).map(|u| u.id);
        assert!(named(&g, "1st Field Artillery Regt").is_none());
        assert!(named(&g, "7th Indian Bde HQ").is_none());
        let div_hq = named(&g, "4th Indian Div HQ").unwrap();

        // GT3, stage 3: 1st Field Artillery Regt arrives and attaches to the division.
        g.game_turn = 3;
        g.operations_stage = 3;
        g.process_arrivals();
        let fld = named(&g, "1st Field Artillery Regt").expect("1st Fld Arty arrived");
        assert_eq!(g.unit(fld).unwrap().attached_parent, Some(div_hq));
        assert!(named(&g, "7th Indian Bde HQ").is_none()); // not yet

        // GT5, stage 1: the whole 7th Indian Bde (HQ + 3 battalions) arrives, hierarchy intact.
        g.game_turn = 5;
        g.operations_stage = 1;
        g.process_arrivals();
        let bde = named(&g, "7th Indian Bde HQ").expect("7th Indian Bde arrived");
        assert_eq!(g.unit(bde).unwrap().attached_parent, Some(div_hq));
        let sussex = named(&g, "1st Royal Sussex").expect("battalion arrived");
        assert_eq!(g.unit(sussex).unwrap().attached_parent, Some(bde));
        assert_eq!(g.units.len(), 17); // all 17 now on the map
        assert!(g.pending_arrivals.is_empty());

        // Idempotent: re-processing brings nobody new in.
        g.process_arrivals();
        assert_eq!(g.units.len(), 17);
    }

    #[test]
    fn weather_table_lookups() {
        use crate::state::game::{Season, Weather};
        let mut refs = ReferenceData::new();
        refs.load_weather_table(include_str!("../../../data/reference/weather-table.ron"))
            .unwrap();
        let t = refs.weather_table.as_ref().unwrap();
        // §29.1 worked example: a roll of 53 in Summer is Hot Weather.
        assert_eq!(t.weather_for(Season::Summer, 53), Weather::Hot);
        assert_eq!(t.weather_for(Season::Fall, 11), Weather::Normal);
        assert_eq!(t.weather_for(Season::Spring, 66), Weather::Rainstorm);
        assert_eq!(t.weather_for(Season::Winter, 52), Weather::Normal);
        assert_eq!(t.weather_for(Season::Winter, 53), Weather::Rainstorm);
        assert_eq!(t.weather_for(Season::Summer, 66), Weather::Sandstorm);
        // §29.7 Foul Weather Location.
        assert_eq!(t.foul_sections(3).to_vec(), vec!["D".to_string(), "E".to_string()]);
        assert_eq!(t.foul_sections(6).to_vec(), vec!["B".to_string(), "C".to_string(), "D".to_string()]);
    }

    #[test]
    fn weather_determination_sets_consistent_state() {
        use crate::state::game::{GameState, Weather};
        let mut refs = ReferenceData::new();
        refs.load_weather_table(include_str!("../../../data/reference/weather-table.ron"))
            .unwrap();
        let mut g = GameState::new(refs, 42);
        g.game_turn = 13; // Summer (§29.61 GT13-24)
        g.determine_weather();
        match g.weather {
            Weather::Normal => assert!(g.weather_by_section.is_empty()),
            Weather::Hot => assert_eq!(g.weather_by_section.len(), 5),
            Weather::Sandstorm | Weather::Rainstorm => {
                assert!(!g.weather_by_section.is_empty());
                assert!(g.weather_by_section.values().all(|&w| w == g.weather));
            }
        }
        // Summer has no Rainstorm column (§29.61), so it can never be rolled in Summer.
        assert_ne!(g.weather, Weather::Rainstorm);
    }

    #[test]
    fn campaign_victory_occupation_points() {
        use crate::reference::units::Side;
        use crate::state::game::GameState;
        use crate::state::map::HexId;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Italian,
            include_str!("../../../data/reference/unit-characteristics/italian.ron"),
        )
        .unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_campaign_victory(include_str!("../../../data/reference/campaign-victory.ron"))
            .unwrap();
        let mut g = GameState::new(refs, 1);

        let place = |g: &mut GameState, nat, id: &str, hex: &str| {
            let u = g.spawn_unit(nat, id, "x", Composition::Organic { current_toe: 4 });
            g.unit_mut(u).unwrap().location =
                Some(UnitLocation::OnMap(HexId::parse(hex).unwrap().to_hex_coord()));
            u
        };
        // Axis holds Tobruk (200) + Bardia (100) = 300; CW holds Benghazi (100) + Derna (50) = 150.
        let axis_unit = place(&mut g, Nationality::Italian, "u", "C4807");
        place(&mut g, Nationality::Italian, "u", "C4321");
        place(&mut g, Nationality::Commonwealth, "h", "A4827");
        place(&mut g, Nationality::Commonwealth, "h", "B5925");
        assert!(g.is_combat_unit(axis_unit)); // sanity: infantry is a combat unit
        // §64.73: each occupying unit must be adequately supplied — give each a co-located Supply
        // Unit with Stores for a week (Water is covered by the major-city hexes; leg infantry
        // needs no Fuel/Ammo). toe 4 → 16 Stores required.
        for (side, hex) in [
            (Side::Axis, "C4807"),
            (Side::Axis, "C4321"),
            (Side::Commonwealth, "A4827"),
            (Side::Commonwealth, "B5925"),
        ] {
            let c = HexId::parse(hex).unwrap().to_hex_coord();
            g.add_supply_dump_full(
                UnitLocation::OnMap(c),
                side,
                crate::state::game::Commodities { fuel: 60, ammo: 40, stores: 100, water: 0 },
            );
        }

        let r = g.evaluate_campaign_victory().unwrap();
        assert_eq!(r.axis_vp, 300);
        assert_eq!(r.cw_vp, 150);
        assert_eq!(r.winner, Some(Side::Axis));
        assert_eq!(r.level, "Decisive Victory"); // 300:150 = 2.0 (ratio_pct 200)
    }

    #[test]
    fn victory_occupation_requires_supply() {
        use crate::reference::units::Side;
        use crate::state::game::GameState;
        use crate::state::map::HexId;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Italian,
            include_str!("../../../data/reference/unit-characteristics/italian.ron"),
        )
        .unwrap();
        refs.load_campaign_victory(include_str!("../../../data/reference/campaign-victory.ron"))
            .unwrap();
        let mut g = GameState::new(refs, 1);
        let tobruk = HexId::parse("C4807").unwrap().to_hex_coord();
        let u = g.spawn_unit(
            Nationality::Italian,
            "u",
            "x",
            Composition::Organic { current_toe: 4 },
        );
        g.unit_mut(u).unwrap().location = Some(UnitLocation::OnMap(tobruk));

        // §64.73: an out-of-supply combat unit does not occupy — no Occupation Points.
        assert_eq!(g.evaluate_campaign_victory().unwrap().axis_vp, 0);
        // A dump with only Fuel/Ammo (no Stores) still fails the supply-adequacy test.
        g.add_supply_dump(UnitLocation::OnMap(tobruk), Side::Axis, 40, 60);
        assert_eq!(g.evaluate_campaign_victory().unwrap().axis_vp, 0);
        // A co-located Supply Unit with a week of Stores makes it adequately supplied → it now
        // occupies Tobruk (200 Axis VP); Water is covered by the major-city hex.
        g.supply_dumps.clear();
        g.add_supply_dump_full(
            UnitLocation::OnMap(tobruk),
            Side::Axis,
            crate::state::game::Commodities { fuel: 60, ammo: 40, stores: 100, water: 0 },
        );
        assert_eq!(g.evaluate_campaign_victory().unwrap().axis_vp, 200);
    }

    #[test]
    fn victory_level_bands() {
        let mut refs = ReferenceData::new();
        refs.load_campaign_victory(include_str!("../../../data/reference/campaign-victory.ron"))
            .unwrap();
        let cv = refs.campaign_victory.as_ref().unwrap();
        // §64.76: Draw 1:1; Marginal up to 1½:1; Decisive up to 2½:1; Smashing beyond.
        assert_eq!(cv.level_for(100), "Draw");
        assert_eq!(cv.level_for(150), "Marginal Victory");
        assert_eq!(cv.level_for(151), "Decisive Victory");
        assert_eq!(cv.level_for(250), "Decisive Victory");
        assert_eq!(cv.level_for(251), "Smashing Victory");
    }

    #[test]
    fn unit_supply_adequacy_64_73() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::game::{Commodities, GameState};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let a = HexCoord::new(0, 0);
        g.map.set_hex(a, HexTerrain::Clear); // a plain hex — no Water source of its own
        // "h" leg infantry, toe 5 → needs 20 Stores/week and 1 Water; no Fuel/Ammo (§50.2).
        let unit = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "inf",
            Composition::Organic { current_toe: 5 },
        );
        g.unit_mut(unit).unwrap().location = Some(UnitLocation::OnMap(a));

        // No dump and no Water source → not adequately supplied.
        assert!(!g.unit_supply_adequate(unit));
        // A co-located dump short on Stores (10 < 20) still fails.
        g.add_supply_dump_full(
            UnitLocation::OnMap(a),
            Side::Commonwealth,
            Commodities { fuel: 0, ammo: 0, stores: 10, water: 5 },
        );
        assert!(!g.unit_supply_adequate(unit));
        // A dump with a week of Stores (and Water) → adequate.
        g.supply_dumps.clear();
        g.add_supply_dump_full(
            UnitLocation::OnMap(a),
            Side::Commonwealth,
            Commodities { fuel: 0, ammo: 0, stores: 20, water: 1 },
        );
        assert!(g.unit_supply_adequate(unit));
        // Enough Stores but no Water anywhere → fails on the Water leg.
        g.supply_dumps.clear();
        g.add_supply_dump_full(
            UnitLocation::OnMap(a),
            Side::Commonwealth,
            Commodities { fuel: 0, ammo: 0, stores: 20, water: 0 },
        );
        assert!(!g.unit_supply_adequate(unit));
        // An enemy dump does not supply it.
        g.supply_dumps.clear();
        g.add_supply_dump_full(
            UnitLocation::OnMap(a),
            Side::Axis,
            Commodities { fuel: 0, ammo: 0, stores: 20, water: 1 },
        );
        assert!(!g.unit_supply_adequate(unit));
    }


    #[test]
    fn dump_choice_legality_and_placement() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::game::{DumpConstraint, DumpPlacementError, GameState, PendingDumpChoice};
        use crate::state::map::HexId;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let west = HexId::parse("D3310").unwrap().to_hex_coord(); // col 310, west of Mersa Matruh
        let east = HexId::parse("D3316").unwrap().to_hex_coord(); // col 316, east (the old bug)
        let c_near = HexId::parse("C0810").unwrap().to_hex_coord();
        let c_far = HexId::parse("C4830").unwrap().to_hex_coord();
        for h in [west, east, c_near, c_far] {
            g.map.set_hex(h, HexTerrain::Clear);
        }

        // §60.92 C: section D, strictly west of Mersa Matruh (D3714).
        let dump_i = DumpConstraint {
            section: Some('D'),
            west_of: Some("D3714".into()),
            ..Default::default()
        };
        assert!(g.dump_choice_legal(&dump_i, Side::Commonwealth, west));
        // The previously hardcoded D3316 is east of Mersa Matruh — correctly rejected.
        assert!(!g.dump_choice_legal(&dump_i, Side::Commonwealth, east));
        // Wrong section is rejected.
        assert!(!g.dump_choice_legal(&dump_i, Side::Commonwealth, c_near));

        // §60.92 B: section C, not within 4 hexes of an enemy (Commonwealth) unit.
        let cw = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "cw",
            Composition::Organic { current_toe: 1 },
        );
        g.unit_mut(cw).unwrap().location = Some(UnitLocation::OnMap(c_near));
        let dump_12 = DumpConstraint {
            section: Some('C'),
            not_within_of_enemy: Some(4),
            ..Default::default()
        };
        assert!(!g.dump_choice_legal(&dump_12, Side::Axis, c_near)); // on the CW unit
        assert!(g.dump_choice_legal(&dump_12, Side::Axis, c_far)); // far away

        // Placement validates the constraint and consumes the choice.
        g.pending_dump_choices.push(PendingDumpChoice {
            side: Side::Commonwealth,
            label: "Dump I".into(),
            count: 3,
            ammo: 40,
            fuel: 30,
            stores: 0,
            water: 0,
            dummy: false,
            constraint: dump_i,
        });
        assert_eq!(g.place_dump_choice(0, east), Err(DumpPlacementError::IllegalHex));
        assert_eq!(g.place_dump_choice(0, west), Ok(3));
        assert!(g.pending_dump_choices.is_empty());
        assert_eq!(
            g.supply_dumps.iter().filter(|d| d.location.hex() == Some(west)).count(),
            3
        );
    }

    #[test]
    fn strategic_supply_line_two_legs() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::game::GameState;
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        let mut g = GameState::new(refs, 1);
        for c in 0..11 {
            g.map.set_hex(HexCoord::new(c, 0), HexTerrain::Clear);
        }
        let source = HexCoord::new(0, 0);
        let unit = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "u",
            Composition::Organic { current_toe: 1 },
        );
        g.unit_mut(unit).unwrap().location = Some(UnitLocation::OnMap(HexCoord::new(4, 0)));
        // A friendly dump at (2,0), connected to the source at (0,0) by truck (2 Clear hexes).
        g.add_supply_dump(UnitLocation::OnMap(HexCoord::new(2, 0)), Side::Commonwealth, 40, 60);
        let sources = [source];

        // Unit (4,0) → dump (2,0) is 2 Clear hexes (40 tenths); within a 5-MP budget.
        assert!(g.unit_strategic_supply_line(unit, 5, &sources));
        // Within only 3 MP (30 tenths) the dump is out of reach.
        assert!(!g.unit_strategic_supply_line(unit, 3, &sources));
        // With no source, the dump is not "active" (second leg fails) → not supplied.
        assert!(!g.unit_strategic_supply_line(unit, 5, &[]));
        // A source the dump cannot reach (off-map hex) also fails the second leg.
        assert!(!g.unit_strategic_supply_line(unit, 5, &[HexCoord::new(50, 0)]));
    }

    #[test]
    fn fuel_capacity_rate_and_cost() {
        use crate::state::game::GameState;
        use crate::state::unit::{Composition, WeaponGroup};

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_weapons_systems(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/weapons/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        // A tank unit ("g") of 3 TOE of A9 Cruiser (§4.47 fuel_rate 2).
        let tank = g.spawn_unit(
            Nationality::Commonwealth,
            "g",
            "t",
            Composition::Weapons {
                groups: vec![WeaponGroup { system: "A9 Cruiser".into(), count: 3 }],
            },
        );
        // §49.13 total rate = 3 TOE × 2 = 6 Fuel per 5 CP.
        assert_eq!(g.unit_fuel_rate_total(tank), 6);
        let cpa = g.unit_cpa(tank).unwrap() as u32;
        // §49.14 capacity = CPA × ⅕ × total rate.
        assert_eq!(g.unit_fuel_capacity(tank), cpa * 6 / 5);
        // §49.13: 12 CP (120 tenths) → ⌈12/5⌉ = 3 groups → 6 × 3 = 18 Fuel.
        assert_eq!(g.unit_move_fuel_cost(tank, 120), 18);
        g.refuel_to_capacity(tank);
        assert_eq!(g.unit(tank).unwrap().fuel, cpa * 6 / 5);
        // Leg infantry (Organic, no vehicles) tracks and consumes no fuel.
        let inf = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "i",
            Composition::Organic { current_toe: 3 },
        );
        assert_eq!(g.unit_fuel_rate_total(inf), 0);
        assert_eq!(g.unit_fuel_capacity(inf), 0);
        assert_eq!(g.unit_move_fuel_cost(inf, 120), 0);
    }

    #[test]
    fn movement_consumes_and_requires_fuel() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::rules::movement::MoveMode;
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation, WeaponGroup};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_weapons_systems(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/weapons/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let (a, b, c) = (HexCoord::new(0, 0), HexCoord::new(1, 0), HexCoord::new(2, 0));
        for h in [a, b, c] {
            g.map.set_hex(h, HexTerrain::Clear);
        }
        let tank = g.spawn_unit(
            Nationality::Commonwealth,
            "g",
            "t",
            Composition::Weapons {
                groups: vec![WeaponGroup { system: "A9 Cruiser".into(), count: 1 }],
            },
        );
        g.unit_mut(tank).unwrap().location = Some(UnitLocation::OnMap(a));
        g.phase = Phase::MovementAndCombat;
        // Total rate 2; a Clear motorized move (2 CP = 20 tenths) costs 2 Fuel. Start with 3.
        g.unit_mut(tank).unwrap().fuel = 3;

        assert_eq!(
            g.apply(Command::MoveStep { unit: tank, to: b, mode: MoveMode::Motorized }),
            Ok(CommandOutcome::Moved)
        );
        assert_eq!(g.unit(tank).unwrap().fuel, 1);
        // Only 1 Fuel left and no dump in the hex → the next move is illegal, and no move happens.
        assert_eq!(
            g.apply(Command::MoveStep { unit: tank, to: c, mode: MoveMode::Motorized }),
            Err(CommandError::InsufficientFuel)
        );
        assert_eq!(g.unit(tank).unwrap().hex(), Some(b));
        // A dump in the start hex covers the 1-Fuel shortfall (§49.16), and the move proceeds.
        g.add_supply_dump(UnitLocation::OnMap(b), Side::Commonwealth, 0, 10);
        assert_eq!(
            g.apply(Command::MoveStep { unit: tank, to: c, mode: MoveMode::Motorized }),
            Ok(CommandOutcome::Moved)
        );
        assert_eq!(g.unit(tank).unwrap().fuel, 0);
        assert_eq!(g.supply_dumps[0].supply.fuel, 9);
    }

    #[test]
    fn ammo_rates_and_consumption() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::game::{CombatAction, GameState};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation, WeaponGroup};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_ammo_consumption(include_str!(
            "../../../data/reference/ammo-consumption.ron"
        ))
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let a = HexCoord::new(0, 0);
        g.map.set_hex(a, HexTerrain::Clear);
        let tank = g.spawn_unit(
            Nationality::Commonwealth,
            "g",
            "t",
            Composition::Weapons {
                groups: vec![WeaponGroup { system: "A9 Cruiser".into(), count: 2 }],
            },
        );
        g.unit_mut(tank).unwrap().location = Some(UnitLocation::OnMap(a));

        // §50.17 one-fire reserve on spawn = barrage rate 4 × 2 TOE = 8.
        assert_eq!(g.unit(tank).unwrap().ammo, 8);
        // §50.14 costs (rate × 2 TOE): Barrage 8, Anti-armor 6, Close Assault 4.
        assert_eq!(g.unit_combat_ammo_cost(tank, CombatAction::Barrage), 8);
        assert_eq!(g.unit_combat_ammo_cost(tank, CombatAction::AntiArmor), 6);
        assert_eq!(g.unit_combat_ammo_cost(tank, CombatAction::CloseAssault), 4);
        // One anti-armor fire (6) drawn from the onboard 8 → 2 left.
        assert!(g.consume_combat_ammo(&[tank], CombatAction::AntiArmor));
        assert_eq!(g.unit(tank).unwrap().ammo, 2);
        // A second anti-armor fire (6) with only 2 onboard and no dump → cannot pay, no change.
        assert!(!g.consume_combat_ammo(&[tank], CombatAction::AntiArmor));
        assert_eq!(g.unit(tank).unwrap().ammo, 2);
        // A dump in the hex covers the shortfall (§50.15): onboard 2 → 0, dump 10 → 6.
        g.add_supply_dump(UnitLocation::OnMap(a), Side::Commonwealth, 10, 0);
        assert!(g.consume_combat_ammo(&[tank], CombatAction::AntiArmor));
        assert_eq!(g.unit(tank).unwrap().ammo, 0);
        assert_eq!(g.supply_dumps[0].supply.ammo, 6);
        // Plain rifle infantry (Organic) pays no Ammo for close assault (§50.2).
        let inf = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "i",
            Composition::Organic { current_toe: 3 },
        );
        assert_eq!(g.unit_combat_ammo_cost(inf, CombatAction::CloseAssault), 0);

        // §50.14 defensive assault: a defending melee tank spends its defensive close-assault Ammo
        // (best-effort). Rearm the tank to a full reserve, then defend: 4 Ammo drawn from onboard 8.
        g.rearm_fire_once(tank); // onboard = 8
        assert_eq!(g.unit(tank).unwrap().ammo, 8);
        let spent = g.expend_defensive_assault_ammo(&[tank, inf]);
        assert_eq!(spent, 4); // tank pays 4; the Organic infantry pays nothing
        assert_eq!(g.unit(tank).unwrap().ammo, 4);
        // A defender with only 1 Ammo and no dump pays what it has (1), never failing.
        g.unit_mut(tank).unwrap().ammo = 1;
        // Drain the co-located dump so onboard is the only source.
        g.supply_dumps[0].supply.ammo = 0;
        assert_eq!(g.expend_defensive_assault_ammo(&[tank]), 1);
        assert_eq!(g.unit(tank).unwrap().ammo, 0);
    }

    #[test]
    fn stores_consumption_and_starvation() {
        use crate::reference::units::Side;
        use crate::state::game::{Commodities, GameState, SupplyDump};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let a = HexCoord::new(0, 0);
        let inf = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "i",
            Composition::Organic { current_toe: 50 },
        );
        g.unit_mut(inf).unwrap().location = Some(UnitLocation::OnMap(a));

        // §51.11: 4 Stores per TOE SP → 4 × 50 = 200. §51.13: an HQ needs a flat 1.
        assert_eq!(g.unit_stores_requirement(inf), 200);
        let hq = g.spawn_unit(
            Nationality::Commonwealth,
            "a",
            "hq",
            Composition::Organic { current_toe: 2 },
        );
        assert_eq!(g.unit_stores_requirement(hq), 1);

        // Turn 1 unsupplied: +1 turn, one Disorganization Point (−1 cohesion), no attrition yet.
        g.expend_stores();
        assert_eq!(g.unit(inf).unwrap().turns_without_stores, 1);
        assert_eq!(g.unit(inf).unwrap().cohesion, -1);
        assert_eq!(g.unit(inf).unwrap().total_toe(), 50);
        // Turn 2 unsupplied: 2 consecutive → §51.22 attrition = round(50 × 2%) = 1 → TOE 49.
        g.expend_stores();
        assert_eq!(g.unit(inf).unwrap().turns_without_stores, 2);
        assert_eq!(g.unit(inf).unwrap().cohesion, -2);
        assert_eq!(g.unit(inf).unwrap().total_toe(), 49);

        // A Stores Dump in the hex now covers the requirement (4 × 49 = 196) and resets the count.
        g.supply_dumps.push(SupplyDump {
            location: UnitLocation::OnMap(a),
            side: Side::Commonwealth,
            supply: Commodities { stores: 300, ..Default::default() },
            dummy: false,
        });
        g.expend_stores();
        assert_eq!(g.unit(inf).unwrap().turns_without_stores, 0);
        assert_eq!(g.unit(inf).unwrap().cohesion, -2); // no further Disorganization
        assert_eq!(g.supply_dumps[0].supply.stores, 300 - 196);
    }

    #[test]
    fn water_table_and_usage_rates() {
        use crate::reference::logistics::WaterSource;
        use crate::state::game::GameState;
        use crate::state::unit::{Composition, WeaponGroup};

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_water_availability(include_str!(
            "../../../data/reference/water-availability.ron"
        ))
        .unwrap();
        let table = refs.water_table.clone().unwrap();
        // §52.7: Water Points drawn, and whether a depletion check applies (the "*" results).
        assert_eq!(table.draw(WaterSource::Town, 1), (100, false));
        assert_eq!(table.draw(WaterSource::Town, 5), (350, true));
        assert_eq!(table.draw(WaterSource::Town, 6), (500, true));
        assert_eq!(table.draw(WaterSource::Bir, 1), (50, false));
        assert_eq!(table.draw(WaterSource::Bir, 3), (150, false));
        assert_eq!(table.draw(WaterSource::Bir, 4), (200, true));

        let mut g = GameState::new(refs, 1);
        // §52.41: an infantry unit needs 1 Water/OpStage regardless of TOE or activity.
        let inf = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "i",
            Composition::Organic { current_toe: 5 },
        );
        assert_eq!(g.unit_water_requirement(inf, false), 1);
        assert_eq!(g.unit_water_requirement(inf, true), 1);
        // §52.42: a vehicle unit needs 1 per TOE SP, but only if it used any CPA this stage.
        let tank = g.spawn_unit(
            Nationality::Commonwealth,
            "g",
            "t",
            Composition::Weapons {
                groups: vec![WeaponGroup { system: "A9 Cruiser".into(), count: 3 }],
            },
        );
        assert_eq!(g.unit_water_requirement(tank, false), 0);
        assert_eq!(g.unit_water_requirement(tank, true), 3);
    }

    #[test]
    fn water_consumption_with_wells() {
        use crate::reference::terrain::HexTerrain;
        use crate::state::game::GameState;
        use crate::state::map::{Feature, HexCoord};
        use crate::state::unit::{Composition, UnitLocation};
        use std::collections::HashSet;

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_water_availability(include_str!(
            "../../../data/reference/water-availability.ron"
        ))
        .unwrap();
        let mut g = GameState::new(refs, 7);
        let (oasis, village, dry) = (HexCoord::new(0, 0), HexCoord::new(1, 0), HexCoord::new(2, 0));
        g.map.set_hex_with_features(oasis, HexTerrain::Clear, vec![Feature::Oasis]);
        g.map.set_hex_with_features(village, HexTerrain::Clear, vec![Feature::Village]);
        g.map.set_hex(dry, HexTerrain::Clear);
        let none: HashSet<crate::state::unit::UnitId> = HashSet::new();

        // Oasis: unlimited water — the unit is supplied.
        let a = g.spawn_unit(Nationality::Commonwealth, "h", "a", Composition::Organic { current_toe: 4 });
        g.unit_mut(a).unwrap().location = Some(UnitLocation::OnMap(oasis));
        g.expend_water(&none);
        assert_eq!(g.unit(a).unwrap().stages_without_water, 0);

        // Village: draws a §52.7 Town amount (≥100) into onboard, then spends 1.
        let b = g.spawn_unit(Nationality::Commonwealth, "h", "b", Composition::Organic { current_toe: 4 });
        g.unit_mut(b).unwrap().location = Some(UnitLocation::OnMap(village));
        g.expend_water(&none);
        assert_eq!(g.unit(b).unwrap().stages_without_water, 0);
        assert!(g.unit(b).unwrap().water >= 99);

        // Dry hex, no source: deprived. §52.53 — no attrition the first stage, −1 TOE the second.
        let c = g.spawn_unit(Nationality::Commonwealth, "h", "c", Composition::Organic { current_toe: 4 });
        g.unit_mut(c).unwrap().location = Some(UnitLocation::OnMap(dry));
        g.expend_water(&none);
        assert_eq!(g.unit(c).unwrap().stages_without_water, 1);
        assert_eq!(g.unit(c).unwrap().total_toe(), 4);
        g.expend_water(&none);
        assert_eq!(g.unit(c).unwrap().stages_without_water, 2);
        assert_eq!(g.unit(c).unwrap().total_toe(), 3);
    }

    #[test]
    fn well_poisoning_and_sweetening() {
        use crate::reference::terrain::HexTerrain;
        use crate::state::game::GameState;
        use crate::state::map::{Feature, HexCoord};
        use crate::state::unit::{Composition, UnitLocation};
        use std::collections::HashSet;

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_water_availability(include_str!(
            "../../../data/reference/water-availability.ron"
        ))
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let bir = HexCoord::new(1, 0);
        g.map.set_hex_with_features(bir, HexTerrain::Clear, vec![Feature::Bir]);
        let none: HashSet<crate::state::unit::UnitId> = HashSet::new();

        // §52.16: only a die roll of 1 poisons.
        assert!(!g.poison_well(bir, 2));
        assert!(g.poison_well(bir, 1));
        assert!(g.poisoned_wells.contains(&bir));

        // A unit at the poisoned bir gets no water and is deprived.
        let u = g.spawn_unit(Nationality::Commonwealth, "h", "u", Composition::Organic { current_toe: 4 });
        g.unit_mut(u).unwrap().location = Some(UnitLocation::OnMap(bir));
        g.expend_water(&none);
        assert_eq!(g.unit(u).unwrap().stages_without_water, 1);

        // §52.17: a die roll of 1–3 sweetens it; then the well supplies water again.
        assert!(!g.sweeten_well(bir, 4));
        assert!(g.sweeten_well(bir, 3));
        assert!(!g.poisoned_wells.contains(&bir));
        g.expend_water(&none);
        assert_eq!(g.unit(u).unwrap().stages_without_water, 0);
    }

    #[test]
    fn rainstorm_replenishes_depleted_wells() {
        use crate::state::game::{GameState, Weather};
        use crate::state::map::HexCoord;

        let mut g = GameState::new(ReferenceData::new(), 1);
        let bir_c = HexCoord::new(80, 0); // section C (cols 67-99)
        let bir_d = HexCoord::new(110, 0); // section D (cols 100-132)
        g.depleted_wells.insert(bir_c);
        g.depleted_wells.insert(bir_d);
        // A rainstorm over section C only.
        g.weather_by_section.insert('C', Weather::Rainstorm);
        g.replenish_wells_on_rainstorm();
        // §52.15: section C's depleted well is replenished; section D's is not.
        assert!(!g.depleted_wells.contains(&bir_c));
        assert!(g.depleted_wells.contains(&bir_d));
    }

    #[test]
    fn hot_weather_doubles_water() {
        use crate::reference::terrain::HexTerrain;
        use crate::state::game::{GameState, Weather};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};
        use std::collections::HashSet;

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_water_availability(include_str!(
            "../../../data/reference/water-availability.ron"
        ))
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let a = HexCoord::new(0, 0); // section A, no well
        g.map.set_hex(a, HexTerrain::Clear);
        let none: HashSet<crate::state::unit::UnitId> = HashSet::new();
        let u = g.spawn_unit(Nationality::Commonwealth, "h", "u", Composition::Organic { current_toe: 4 });
        g.unit_mut(u).unwrap().location = Some(UnitLocation::OnMap(a));
        g.unit_mut(u).unwrap().water = 2;

        // Normal weather: an infantry unit needs 1 Water — supplied (2 → 1).
        g.expend_water(&none);
        assert_eq!(g.unit(u).unwrap().stages_without_water, 0);
        assert_eq!(g.unit(u).unwrap().water, 1);
        // §29.35 hot weather doubles the requirement to 2 — 1 onboard is not enough → deprived.
        g.weather_by_section.insert('A', Weather::Hot);
        g.expend_water(&none);
        assert_eq!(g.unit(u).unwrap().stages_without_water, 1);
    }

    #[test]
    fn supplies_evaporate() {
        use crate::reference::units::Side;
        use crate::state::game::{Commodities, GameState, SupplyDump};
        use crate::state::map::HexCoord;
        use crate::state::unit::UnitLocation;

        let mut g = GameState::new(ReferenceData::new(), 1);
        g.supply_dumps.push(SupplyDump {
            location: UnitLocation::OnMap(HexCoord::new(0, 0)),
            side: Side::Axis,
            supply: Commodities { fuel: 1000, water: 500, ammo: 100, stores: 200 },
            dummy: false,
        });
        // §49.3/§52.44: 6% of Fuel and Water evaporates; Ammo and Stores are unaffected.
        g.evaporate_supplies(6);
        let s = g.supply_dumps[0].supply;
        assert_eq!((s.fuel, s.water, s.ammo, s.stores), (940, 470, 100, 200));
    }

    #[test]
    fn commonwealth_early_war_evaporation() {
        use crate::reference::units::Side;
        use crate::state::game::{Commodities, GameState, SupplyDump};
        use crate::state::map::HexCoord;
        use crate::state::unit::UnitLocation;

        let mut g = GameState::new(ReferenceData::new(), 1);
        for side in [Side::Commonwealth, Side::Axis] {
            g.supply_dumps.push(SupplyDump {
                location: UnitLocation::OnMap(HexCoord::new(0, 0)),
                side,
                supply: Commodities { fuel: 1000, water: 1000, ammo: 100, stores: 100 },
                dummy: false,
            });
        }
        // §49.3: through August 1941 (GT ≤ 48) the Commonwealth spills at 9%; the Axis at 6%.
        g.game_turn = 10;
        assert_eq!(g.base_evaporation_rate(Side::Commonwealth), 9);
        assert_eq!(g.base_evaporation_rate(Side::Axis), 6);
        g.evaporate_base_supplies();
        assert_eq!((g.supply_dumps[0].supply.fuel, g.supply_dumps[0].supply.water), (910, 910)); // CW 9%
        assert_eq!((g.supply_dumps[1].supply.fuel, g.supply_dumps[1].supply.water), (940, 940)); // Axis 6%
        // From September 1941 (GT 49) the Commonwealth rate drops to 6%.
        g.game_turn = 49;
        assert_eq!(g.base_evaporation_rate(Side::Commonwealth), 6);
    }

    #[test]
    fn commonwealth_railroad_is_a_water_pipeline() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::game::{GameState, WaterKind};
        use crate::state::map::{HexCoord, HexsideKind};
        use crate::state::unit::{Composition, UnitLocation};
        use std::collections::HashSet;

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_water_availability(include_str!(
            "../../../data/reference/water-availability.ron"
        ))
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let (a, b) = (HexCoord::new(0, 0), HexCoord::new(1, 0));
        g.map.set_hex(a, HexTerrain::Desert); // no natural well
        g.map.set_hex(b, HexTerrain::Desert);
        g.map.set_hexside(a, b, HexsideKind::Railroad, None);

        // §52.22/§54.33: a built railroad hex is an unlimited source — for the Commonwealth only.
        assert!(g.hex_is_railroad(a) && g.hex_is_railroad(b));
        assert_eq!(g.hex_water_source_for(a, Side::Commonwealth), Some(WaterKind::Unlimited));
        assert_eq!(g.hex_water_source_for(a, Side::Axis), None);

        // §24.67: unbuilt track is not a railroad hex → no water.
        let (c, d) = (HexCoord::new(0, 1), HexCoord::new(1, 1));
        g.map.set_hex(c, HexTerrain::Desert);
        g.map.set_hex(d, HexTerrain::Desert);
        g.map.set_hexside(c, d, HexsideKind::UnfinishedRailroad, None);
        assert!(!g.hex_is_railroad(c));
        assert_eq!(g.hex_water_source_for(c, Side::Commonwealth), None);

        // Integration: a CW unit on the railroad hex stays supplied through the Water phase.
        let none: HashSet<crate::state::unit::UnitId> = HashSet::new();
        let cw = g.spawn_unit(Nationality::Commonwealth, "h", "u", Composition::Organic { current_toe: 4 });
        g.unit_mut(cw).unwrap().location = Some(UnitLocation::OnMap(a));
        g.expend_water(&none);
        assert_eq!(g.unit(cw).unwrap().stages_without_water, 0);
    }

    #[test]
    fn pipeline_built_from_railroad_hex() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::game::{Commodities, GameState, SupplyDump, WaterKind};
        use crate::state::map::{HexCoord, HexsideKind};
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_unit_characteristics(
            Nationality::Italian,
            include_str!("../../../data/reference/unit-characteristics/italian.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let dump = |g: &mut GameState, h, side| {
            g.supply_dumps.push(SupplyDump {
                location: UnitLocation::OnMap(h),
                side,
                supply: Commodities { fuel: 0, ammo: 0, stores: 10, water: 0 },
                dummy: false,
            });
        };
        // Rail hex `a` (touches a Railroad edge a|b); `p` is a desert neighbour of `a`.
        let a = HexCoord::new(5, 5);
        let ns = a.neighbors();
        let (b, p, q) = (ns[0], ns[1], ns[4]);
        for h in [a, b, p, q] {
            g.map.set_hex(h, HexTerrain::Desert);
        }
        g.map.set_hexside(a, b, HexsideKind::Railroad, None);

        // §52.22: the Commonwealth may build a pipeline from a railroad hex.
        let hq = g.spawn_unit(Nationality::Commonwealth, "a", "hq", Composition::Organic { current_toe: 0 });
        g.unit_mut(hq).unwrap().location = Some(UnitLocation::OnMap(p));
        dump(&mut g, p, Side::Commonwealth);
        assert!(g.build_pipeline(hq, p));
        // The rail-anchored pipeline is a CW water source — but not for the Axis, and not a generic
        // (major-city) source.
        assert_eq!(g.hex_water_source_for(p, Side::Commonwealth), Some(WaterKind::Unlimited));
        assert_eq!(g.hex_water_source_for(p, Side::Axis), None);
        assert_eq!(g.hex_water_source(p), None);

        // The Axis may NOT build a pipeline from a railroad hex (q adjoins only the rail hex a).
        g.pipeline_built_this_stage.clear();
        let ihq = g.spawn_unit(Nationality::Italian, "a", "ihq", Composition::Organic { current_toe: 0 });
        g.unit_mut(ihq).unwrap().location = Some(UnitLocation::OnMap(q));
        dump(&mut g, q, Side::Axis);
        assert!(!g.build_pipeline(ihq, q));
    }

    #[test]
    fn field_repair_blocked_by_foul_weather() {
        use crate::reference::terrain::HexTerrain;
        use crate::state::game::{GameState, Weather};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation, WeaponGroup};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_weapons_systems(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/weapons/commonwealth.ron"),
        )
        .unwrap();
        refs.load_repair_table(include_str!("../../../data/reference/repair-table.ron"))
            .unwrap();
        let mut g = GameState::new(refs, 1);
        let a = HexCoord::new(0, 0); // section A
        g.map.set_hex(a, HexTerrain::Clear);
        let tank = g.spawn_unit(
            Nationality::Commonwealth,
            "g",
            "t",
            Composition::Weapons { groups: vec![WeaponGroup { system: "A9 Cruiser".into(), count: 0 }] },
        );
        g.unit_mut(tank).unwrap().location = Some(UnitLocation::OnMap(a));
        g.unit_mut(tank).unwrap().weapons_broken = vec![WeaponGroup { system: "A9 Cruiser".into(), count: 20 }];
        g.unit_mut(tank).unwrap().fuel = 20;

        // §22.13d: a Sandstorm over the hex forbids field repairs.
        g.weather_by_section.insert('A', Weather::Sandstorm);
        assert!(g.field_repair_blocked_by_weather(tank));
        assert_eq!(g.field_repair_tanks(tank, 0), 0);
        // Clear weather → the repair proceeds (⌈20 × 25%⌉ = 5).
        g.weather_by_section.clear();
        assert_eq!(g.field_repair_tanks(tank, 0), 5);
    }

    #[test]
    fn field_repair_recce_points() {
        use crate::reference::terrain::HexTerrain;
        use crate::state::game::{GameState, Weather};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let a = HexCoord::new(0, 0); // section A
        g.map.set_hex(a, HexTerrain::Clear);
        // "hh" is a Reconnaissance Bn-Equivalent (max TOE 8); 3 of its 8 points are broken down.
        let recce = g.spawn_unit(Nationality::Commonwealth, "hh", "r", Composition::Organic { current_toe: 5 });
        g.unit_mut(recce).unwrap().location = Some(UnitLocation::OnMap(a));
        g.unit_mut(recce).unwrap().toe_broken = 3;

        // §22.24: only a die roll of 1 repairs, and it recovers exactly one Point.
        assert_eq!(g.field_repair_recce(recce, 2), 0);
        assert_eq!(g.unit(recce).unwrap().toe_broken, 3);
        assert_eq!(g.field_repair_recce(recce, 1), 1);
        assert_eq!(g.unit(recce).unwrap().toe_broken, 2);
        assert_eq!(g.unit(recce).unwrap().total_toe(), 6);

        // §22.13d: a Sandstorm over the hex blocks the repair.
        g.weather_by_section.insert('A', Weather::Sandstorm);
        assert_eq!(g.field_repair_recce(recce, 1), 0);
        g.weather_by_section.clear();

        // A non-recce unit (infantry "h") may not use recce field repair.
        let inf = g.spawn_unit(Nationality::Commonwealth, "h", "i", Composition::Organic { current_toe: 4 });
        g.unit_mut(inf).unwrap().location = Some(UnitLocation::OnMap(a));
        g.unit_mut(inf).unwrap().toe_broken = 2;
        assert_eq!(g.field_repair_recce(inf, 1), 0);
    }

    #[test]
    fn recce_toe_breaks_down_then_repairs() {
        use crate::state::game::GameState;
        use crate::state::unit::Composition;

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let recce = g.spawn_unit(Nationality::Commonwealth, "hh", "r", Composition::Organic { current_toe: 8 });
        let inf = g.spawn_unit(Nationality::Commonwealth, "h", "i", Composition::Organic { current_toe: 8 });

        // §21.14: a recce/AC unit's Breakdown Adjustment Rating is 1 Left (−1); foot infantry has none.
        assert_eq!(g.unit_bar(recce), Some(-1));
        assert_eq!(g.unit_bar(inf), None);

        // §21.11: 25% of the recce unit's 8 TOE Points break down into the recoverable pool.
        g.apply_breakdown_loss(recce, 25);
        assert_eq!(g.unit(recce).unwrap().total_toe(), 6);
        assert_eq!(g.unit(recce).unwrap().toe_broken, 2);
        // Infantry TOE does not break down (§21.11 lists only vehicles).
        g.apply_breakdown_loss(inf, 25);
        assert_eq!(g.unit(inf).unwrap().total_toe(), 8);
        assert_eq!(g.unit(inf).unwrap().toe_broken, 0);

        // §22.24: field repair recovers one broken recce Point.
        assert_eq!(g.field_repair_recce(recce, 1), 1);
        assert_eq!(g.unit(recce).unwrap().total_toe(), 7);
        assert_eq!(g.unit(recce).unwrap().toe_broken, 1);
    }

    #[test]
    fn multi_feature_hexside_crossing() {
        use crate::reference::terrain::{Fixed, HexTerrain, MoveCost};
        use crate::rules::movement::{crossing_cost, MoveMode};
        use crate::state::map::{HexCoord, HexsideKind, Map};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        let mut m = Map::new();
        let edge = |m: &mut Map, r: i16, kinds: &[HexsideKind]| {
            let (a, b) = (HexCoord::new(0, r), HexCoord::new(1, r));
            m.set_hex(a, HexTerrain::Clear);
            m.set_hex(b, HexTerrain::Clear);
            for &k in kinds {
                m.set_hexside(a, b, k, None);
            }
            (a, b)
        };
        let mot = MoveMode::Motorized;

        // §8.37: a Road bridging a MinorRiver negates the obstacle → cheapest crossing (Road 0.5).
        let (a, b) = edge(&mut m, 0, &[HexsideKind::MinorRiver, HexsideKind::Road]);
        assert_eq!(crossing_cost(&refs, &m, a, b, mot), MoveCost::Cost(Fixed(5)));
        // A Road even bridges a MajorRiver (otherwise Prohibited to vehicles).
        let (c, d) = edge(&mut m, 1, &[HexsideKind::MajorRiver, HexsideKind::Road]);
        assert_eq!(crossing_cost(&refs, &m, c, d, mot), MoveCost::Cost(Fixed(5)));
        // No route: obstacles stack to the most restrictive (MinorRiver 6 > Wadi 4).
        let (e, f) = edge(&mut m, 2, &[HexsideKind::MinorRiver, HexsideKind::Wadi]);
        assert_eq!(crossing_cost(&refs, &m, e, f, mot), MoveCost::Cost(Fixed(60)));
        // No route, a MajorRiver present → Prohibited to vehicles.
        let (g, h) = edge(&mut m, 3, &[HexsideKind::MajorRiver, HexsideKind::Wadi]);
        assert_eq!(crossing_cost(&refs, &m, g, h, mot), MoveCost::Prohibited);

        // §24.52: an unfinished road crosses as a Track (motorized Cost 1).
        let (i, j) = edge(&mut m, 4, &[HexsideKind::UnfinishedRoad]);
        assert_eq!(crossing_cost(&refs, &m, i, j, mot), MoveCost::Cost(Fixed(10)));
        // §24.67: unbuilt track is inert — no crossing cost or benefit.
        let (k, l) = edge(&mut m, 5, &[HexsideKind::UnfinishedRailroad]);
        assert_eq!(crossing_cost(&refs, &m, k, l, mot), MoveCost::Cost(Fixed(0)));
    }

    #[test]
    fn italian_pasta_rule() {
        use crate::reference::terrain::{Fixed, HexTerrain};
        use crate::rules::movement::MoveMode;
        use crate::state::command::{Command, CommandError};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, OrgSize, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Italian,
            include_str!("../../../data/reference/unit-characteristics/italian.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let a = HexCoord::new(0, 0);
        g.map.set_hex(a, HexTerrain::Clear);
        let bn = g.spawn_unit(Nationality::Italian, "p", "bn", Composition::Organic { current_toe: 4 });
        g.unit_mut(bn).unwrap().org_size = OrgSize::Battalion;
        g.unit_mut(bn).unwrap().location = Some(UnitLocation::OnMap(a));

        // No Water anywhere → the battalion misses its Pasta Point; cohesion 0 (> −10) is unchanged.
        g.distribute_pasta();
        assert!(g.no_pasta.contains(&bn));
        assert_eq!(g.unit(bn).unwrap().cohesion, 0);

        // Give it Water → it receives the Pasta Point and is cleared.
        g.unit_mut(bn).unwrap().water = 1;
        g.distribute_pasta();
        assert!(!g.no_pasta.contains(&bn));
        assert_eq!(g.unit(bn).unwrap().water, 0);

        // A pasta-less battalion at cohesion ≤ −10 is immediately Disorganized (−26)…
        g.unit_mut(bn).unwrap().cohesion = -10;
        g.distribute_pasta();
        assert_eq!(g.unit(bn).unwrap().cohesion, -26);
        // …and regains its original cohesion when it next gets its Point.
        g.unit_mut(bn).unwrap().water = 1;
        g.distribute_pasta();
        assert_eq!(g.unit(bn).unwrap().cohesion, -10);
        assert!(!g.no_pasta.contains(&bn));

        // §52.6 movement: a pasta-less battalion may not voluntarily exceed its CPA.
        g.no_pasta.insert(bn);
        g.phase = Phase::MovementAndCombat;
        let cpa = g.unit_cpa(bn).unwrap();
        g.unit_mut(bn).unwrap().cp_expended = Fixed(cpa * 10);
        let b = HexCoord::new(1, 0);
        g.map.set_hex(b, HexTerrain::Clear);
        assert_eq!(
            g.apply(Command::MoveStep { unit: bn, to: b, mode: MoveMode::NonMotorized }),
            Err(CommandError::NoPasta)
        );
    }

    #[test]
    fn truck_chart_and_motorization() {
        use crate::reference::logistics::TruckKind;
        use crate::state::game::GameState;
        use crate::state::unit::Composition;

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_truck_chart(include_str!(
            "../../../data/reference/truck-characteristics.ron"
        ))
        .unwrap();
        let chart = refs.truck_chart.clone().unwrap();
        // §54.2 capacities + points-to-motorize infantry.
        assert_eq!(chart.get(TruckKind::Medium).cargo_fuel, 120);
        assert_eq!(chart.get(TruckKind::Heavy).cargo_water, 200);
        assert_eq!(chart.get(TruckKind::Light).cpa_guns, None); // na
        assert_eq!(chart.inf_points_needed(TruckKind::Light, 3), Some(6)); // 2 Light per SP
        assert_eq!(chart.inf_points_needed(TruckKind::Medium, 3), Some(3));
        assert_eq!(chart.inf_points_needed(TruckKind::Heavy, 3), Some(2)); // ⌈3/2⌉

        let mut g = GameState::new(refs, 1);
        // "u" is infantry with CPA 10 (motorizable per §6.17).
        let inf = g.spawn_unit(
            Nationality::Commonwealth,
            "u",
            "i",
            Composition::Organic { current_toe: 4 },
        );
        assert_eq!(g.unit_motorized_cpa(inf), None); // no trucks
        // 4 Medium points carry 4 inf SP → fully motorized at Medium Inf CPA 20 (§6.17).
        g.unit_mut(inf).unwrap().trucks.medium = 4;
        assert_eq!(g.unit_truck_inf_capacity(inf), 4);
        assert_eq!(g.unit_motorized_cpa(inf), Some(20));
        // 4 Light points carry only 2 inf SP (½ each) → not enough for 4 TOE.
        g.unit_mut(inf).unwrap().trucks.medium = 0;
        g.unit_mut(inf).unwrap().trucks.light = 4;
        assert_eq!(g.unit_truck_inf_capacity(inf), 2);
        assert_eq!(g.unit_motorized_cpa(inf), None);
        // 8 Light points carry 4 inf SP → motorized at the Light Inf CPA 25.
        g.unit_mut(inf).unwrap().trucks.light = 8;
        assert_eq!(g.unit_motorized_cpa(inf), Some(25));

        // §54.2 cargo capacity: 2 Medium points → 240 Fuel / 8 Ammo / 30 Stores / 200 Water.
        let hauler = g.spawn_unit(
            Nationality::Commonwealth,
            "u",
            "h",
            Composition::Organic { current_toe: 1 },
        );
        g.unit_mut(hauler).unwrap().trucks.medium = 2;
        let cap = g.unit_truck_cargo_capacity(hauler);
        assert_eq!((cap.fuel, cap.ammo, cap.stores, cap.water), (240, 8, 30, 200));
    }

    #[test]
    fn truck_cargo_load_and_unload() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::game::{Commodities, Commodity, GameState, SupplyDump};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_truck_chart(include_str!(
            "../../../data/reference/truck-characteristics.ron"
        ))
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let (rear, front) = (HexCoord::new(0, 0), HexCoord::new(5, 0));
        g.map.set_hex(rear, HexTerrain::Clear);
        g.map.set_hex(front, HexTerrain::Clear);
        let convoy = g.spawn_unit(
            Nationality::Commonwealth,
            "u",
            "convoy",
            Composition::Organic { current_toe: 1 },
        );
        g.unit_mut(convoy).unwrap().trucks.medium = 2; // cargo capacity 240 Fuel
        g.unit_mut(convoy).unwrap().location = Some(UnitLocation::OnMap(rear));
        // A rear Fuel dump with 500.
        g.supply_dumps.push(SupplyDump {
            location: UnitLocation::OnMap(rear),
            side: Side::Commonwealth,
            supply: Commodities { fuel: 500, ..Default::default() },
            dummy: false,
        });

        // Load 300 Fuel — capped at the 240 cargo capacity; dump drops to 260.
        assert_eq!(g.load_trucks(convoy, Commodity::Fuel, 300), 240);
        assert_eq!(g.unit(convoy).unwrap().truck_cargo.fuel, 240);
        assert_eq!(g.supply_dumps[0].supply.fuel, 260);

        // Haul to the front and unload 100 Fuel — a new dump appears there.
        g.unit_mut(convoy).unwrap().location = Some(UnitLocation::OnMap(front));
        assert_eq!(g.unload_trucks(convoy, Commodity::Fuel, 100), 100);
        assert_eq!(g.unit(convoy).unwrap().truck_cargo.fuel, 140);
        let front_dump = g
            .supply_dumps
            .iter()
            .find(|d| d.location.hex() == Some(front))
            .unwrap();
        assert_eq!(front_dump.supply.fuel, 100);
    }

    #[test]
    fn attached_trucks_consume_fuel() {
        use crate::state::game::GameState;
        use crate::state::unit::{Composition, WeaponGroup};

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_weapons_systems(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/weapons/commonwealth.ron"),
        )
        .unwrap();
        refs.load_truck_chart(include_str!(
            "../../../data/reference/truck-characteristics.ron"
        ))
        .unwrap();
        let mut g = GameState::new(refs, 1);

        // Leg infantry burns no fuel until it is carried by trucks (§49.12/18).
        let inf = g.spawn_unit(
            Nationality::Commonwealth,
            "u",
            "i",
            Composition::Organic { current_toe: 4 },
        );
        assert_eq!(g.unit_fuel_rate_total(inf), 0);
        g.unit_mut(inf).unwrap().trucks.medium = 4;
        assert_eq!(g.unit_fuel_rate_total(inf), 4); // 4 Medium × FCF 1
        assert_eq!(g.unit_move_fuel_cost(inf, 120), 12); // 4 × ⌈12/5⌉ = 12

        // A tank unit's trucks add to its weapon-system fuel.
        let tank = g.spawn_unit(
            Nationality::Commonwealth,
            "g",
            "t",
            Composition::Weapons {
                groups: vec![WeaponGroup { system: "A9 Cruiser".into(), count: 2 }],
            },
        );
        assert_eq!(g.unit_fuel_rate_total(tank), 4); // A9 rate 2 × 2 TOE
        g.unit_mut(tank).unwrap().trucks.heavy = 3;
        assert_eq!(g.unit_fuel_rate_total(tank), 7); // + 3 Heavy trucks
    }

    #[test]
    fn breakdown_table_lookup() {
        let mut refs = ReferenceData::new();
        refs.load_breakdown_table(include_str!(
            "../../../data/reference/breakdown-table.ron"
        ))
        .unwrap();
        let bt = refs.breakdown_table.clone().unwrap();

        // §21.38 direct reads (no shift).
        assert_eq!(bt.percent(2, 0, 66), 0); // ≤3 BP never breaks (§21.27)
        assert_eq!(bt.percent(8, 0, 42), 0); // col 4–10: 11–42 → 0
        assert_eq!(bt.percent(8, 0, 43), 10); // 43–64 → 10
        assert_eq!(bt.percent(8, 0, 65), 25);
        assert_eq!(bt.percent(8, 0, 66), 33);
        assert_eq!(bt.percent(75, 0, 11), 10); // 71+ col has no 0% band
        assert_eq!(bt.percent(75, 0, 66), 75);
        // Column shifts: a Truck's BAR (−2, Left) makes 8 BP behave like the 0–3 column.
        assert_eq!(bt.percent(8, -2, 66), 0);
        // Hot weather (+1 Right) pushes 8 BP into the 11–20 column: roll 66 → 50%.
        assert_eq!(bt.percent(8, 1, 66), 50);
    }

    #[test]
    fn breakdown_check_and_application() {
        use crate::state::game::GameState;
        use crate::state::unit::{Composition, WeaponGroup};

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_weapons_systems(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/weapons/commonwealth.ron"),
        )
        .unwrap();
        refs.load_breakdown_table(include_str!(
            "../../../data/reference/breakdown-table.ron"
        ))
        .unwrap();
        let mut g = GameState::new(refs, 3);
        let tank = g.spawn_unit(
            Nationality::Commonwealth,
            "g",
            "t",
            Composition::Weapons {
                groups: vec![WeaponGroup { system: "A9 Cruiser".into(), count: 20 }],
            },
        );
        // §21.14: the A9's own Breakdown Adjustment Rating (chart value).
        assert_eq!(g.unit_bar(tank), Some(1));
        // §21.27: ≤ 3 BP never checks.
        g.unit_mut(tank).unwrap().breakdown_points = 20; // 2 BP
        assert_eq!(g.resolve_breakdown(tank), 0);
        // 80 BP → the 71+ column, which has no 0% band, so vehicles break down.
        g.unit_mut(tank).unwrap().breakdown_points = 800;
        let pct = g.resolve_breakdown(tank);
        assert!(pct >= 10);
        assert!(g.unit(tank).unwrap().total_toe() < 20);
        // §21/§22.8: broken-down tanks are held recoverable, not deleted — active + broken = 20.
        let broken: u32 = g.unit(tank).unwrap().weapons_broken.iter().map(|g| g.count).sum();
        assert!(broken > 0);
        assert_eq!(g.unit(tank).unwrap().total_toe() + broken, 20);
        // §21.26: an immediate recheck in the same column does nothing.
        assert_eq!(g.resolve_breakdown(tank), 0);

        // Leg infantry is not subject to breakdown until motorized by trucks (§21.11).
        let inf = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "i",
            Composition::Organic { current_toe: 4 },
        );
        assert_eq!(g.unit_bar(inf), None);
        g.unit_mut(inf).unwrap().trucks.medium = 4;
        assert_eq!(g.unit_bar(inf), Some(-2)); // All Trucks: 2 Left
    }

    #[test]
    fn field_repair_trucks() {
        use crate::state::game::GameState;
        use crate::state::unit::Composition;

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let u = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "u",
            Composition::Organic { current_toe: 4 },
        );
        {
            let m = g.unit_mut(u).unwrap();
            m.trucks_broken.light = 3;
            m.trucks_broken.medium = 2;
            m.trucks_broken.heavy = 1;
        }
        // §22.23: a 2 repairs one Light/Medium (Medium preferred).
        assert_eq!(g.field_repair_trucks(u, 2), 1);
        assert_eq!((g.unit(u).unwrap().trucks_broken.medium, g.unit(u).unwrap().trucks.medium), (1, 1));
        // A 1 repairs one Heavy (preferred over two light/medium).
        assert_eq!(g.field_repair_trucks(u, 1), 1);
        assert_eq!((g.unit(u).unwrap().trucks_broken.heavy, g.unit(u).unwrap().trucks.heavy), (0, 1));
        // With no broken Heavy left, a 1 repairs up to two Light/Medium.
        assert_eq!(g.field_repair_trucks(u, 1), 2);
        // Other rolls repair nothing.
        assert_eq!(g.field_repair_trucks(u, 4), 0);
    }

    #[test]
    fn field_repair_tanks() {
        use crate::state::game::GameState;
        use crate::state::unit::{Composition, WeaponGroup};

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_weapons_systems(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/weapons/commonwealth.ron"),
        )
        .unwrap();
        refs.load_repair_table(include_str!(
            "../../../data/reference/repair-table.ron"
        ))
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let tank = g.spawn_unit(
            Nationality::Commonwealth,
            "g",
            "t",
            Composition::Weapons {
                groups: vec![WeaponGroup { system: "A9 Cruiser".into(), count: 0 }],
            },
        );
        // 20 A9 Cruisers broken down.
        g.unit_mut(tank).unwrap().weapons_broken = vec![WeaponGroup { system: "A9 Cruiser".into(), count: 20 }];
        // §22.26: 1 Fuel Point per Tank Point attempted — enough for all 20.
        g.unit_mut(tank).unwrap().fuel = 20;

        // §22.8 field, die 0 → 25%: ⌈20 × 25%⌉ = 5 repaired; 20 Fuel expended for the attempt.
        assert_eq!(g.field_repair_tanks(tank, 0), 5);
        assert_eq!(g.unit(tank).unwrap().fuel, 0);
        assert_eq!(g.unit(tank).unwrap().total_toe(), 5);
        assert_eq!(g.unit(tank).unwrap().weapons_broken[0].count, 15);
        // No Fuel left → nothing can be attempted even at a repairable roll.
        assert_eq!(g.field_repair_tanks(tank, 0), 0);
        // die 5 → 0%: nothing repaired.
        assert_eq!(g.field_repair_tanks(tank, 5), 0);
        // die 7 (na) → 0%.
        assert_eq!(g.field_repair_tanks(tank, 7), 0);
    }

    #[test]
    fn tank_repair_fuel_limited() {
        use crate::reference::units::Side;
        use crate::state::game::{Commodities, GameState, SupplyDump};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation, WeaponGroup};

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_weapons_systems(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/weapons/commonwealth.ron"),
        )
        .unwrap();
        refs.load_repair_table(include_str!("../../../data/reference/repair-table.ron"))
            .unwrap();
        let mut g = GameState::new(refs, 1);
        let a = HexCoord::new(0, 0);
        let tank = g.spawn_unit(
            Nationality::Commonwealth,
            "g",
            "t",
            Composition::Weapons { groups: vec![WeaponGroup { system: "A9 Cruiser".into(), count: 0 }] },
        );
        g.unit_mut(tank).unwrap().location = Some(UnitLocation::OnMap(a));
        g.unit_mut(tank).unwrap().weapons_broken = vec![WeaponGroup { system: "A9 Cruiser".into(), count: 20 }];

        // Only 4 Fuel on the unit → §22.26 caps the attempt at 4: ⌈4 × 25%⌉ = 1 repaired, 4 spent.
        g.unit_mut(tank).unwrap().fuel = 4;
        assert_eq!(g.field_repair_tanks(tank, 0), 1);
        assert_eq!(g.unit(tank).unwrap().fuel, 0);

        // Fuel in a Supply Dump in the hex is also usable: 8 Fuel → ⌈8 × 25%⌉ = 2 repaired.
        g.supply_dumps.push(SupplyDump {
            location: UnitLocation::OnMap(a),
            side: Side::Commonwealth,
            supply: Commodities { fuel: 8, ammo: 0, stores: 0, water: 0 },
            dummy: false,
        });
        assert_eq!(g.field_repair_tanks(tank, 0), 2);
        assert_eq!(g.supply_dumps[0].supply.fuel, 0);
    }

    #[test]
    fn water_pipeline_source_and_severing() {
        use crate::reference::terrain::HexTerrain;
        use crate::state::game::{GameState, WaterKind};
        use crate::state::map::HexId;

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_campaign_victory(include_str!("../../../data/reference/campaign-victory.ron"))
            .unwrap();
        let mut g = GameState::new(refs, 1);
        let city = HexId::parse("C4807").unwrap().to_hex_coord(); // Tobruk (major city)
        let p1 = city.neighbors()[0]; // adjacent to the city
        let p2 = p1
            .neighbors()
            .into_iter()
            .find(|h| *h != city && !city.neighbors().contains(h))
            .unwrap(); // one hex further out, not itself adjacent to the city
        for h in [city, p1, p2] {
            g.map.set_hex(h, HexTerrain::Clear);
        }
        g.pipelines.insert(p1);
        g.pipelines.insert(p2);

        // §52.23: both pipeline hexes are unlimited sources while connected to the city.
        assert!(g.pipeline_connected(p1) && g.pipeline_connected(p2));
        assert_eq!(g.hex_water_source(p1), Some(WaterKind::Unlimited));
        assert_eq!(g.hex_water_source(p2), Some(WaterKind::Unlimited));

        // §52.25: destroying the near hex severs the far one from the source.
        assert!(g.destroy_pipeline(p1));
        assert!(!g.pipeline_connected(p2));
        assert_eq!(g.hex_water_source(p2), None);
    }

    #[test]
    fn build_water_pipeline() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::game::{Commodities, GameState, SupplyDump, WaterKind};
        use crate::state::map::{HexCoord, HexId};
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_campaign_victory(include_str!("../../../data/reference/campaign-victory.ron"))
            .unwrap();
        let mut g = GameState::new(refs, 1);
        let city = HexId::parse("C4807").unwrap().to_hex_coord();
        let adj = city.neighbors()[0]; // adjoins the city
        g.map.set_hex(city, HexTerrain::Clear);
        g.map.set_hex(adj, HexTerrain::Clear);

        // A Commonwealth HQ ("a") in the hex with a 10-Stores dump can build (§52.24).
        let hq = g.spawn_unit(Nationality::Commonwealth, "a", "hq", Composition::Organic { current_toe: 0 });
        g.unit_mut(hq).unwrap().location = Some(UnitLocation::OnMap(adj));
        g.supply_dumps.push(SupplyDump {
            location: UnitLocation::OnMap(adj),
            side: Side::Commonwealth,
            supply: Commodities { fuel: 0, ammo: 0, stores: 10, water: 0 },
            dummy: false,
        });
        assert!(g.build_pipeline(hq, adj));
        assert!(g.pipelines.contains(&adj));
        assert_eq!(g.supply_dumps[0].supply.stores, 0); // 10 Stores expended
        assert_eq!(g.hex_water_source(adj), Some(WaterKind::Unlimited));

        // A hex that adjoins neither the city nor a pipeline cannot be built.
        g.pipeline_built_this_stage.clear(); // isolate this from the §52.24 per-stage cap
        let far = HexCoord::new(50, 50);
        g.map.set_hex(far, HexTerrain::Clear);
        let hq2 = g.spawn_unit(Nationality::Commonwealth, "a", "hq2", Composition::Organic { current_toe: 0 });
        g.unit_mut(hq2).unwrap().location = Some(UnitLocation::OnMap(far));
        g.supply_dumps.push(SupplyDump {
            location: UnitLocation::OnMap(far),
            side: Side::Commonwealth,
            supply: Commodities { fuel: 0, ammo: 0, stores: 10, water: 0 },
            dummy: false,
        });
        assert!(!g.build_pipeline(hq2, far));
    }

    #[test]
    fn one_pipeline_per_operations_stage() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::game::{Commodities, GameState, SupplyDump};
        use crate::state::map::HexId;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_campaign_victory(include_str!("../../../data/reference/campaign-victory.ron"))
            .unwrap();
        let mut g = GameState::new(refs, 1);
        let city = HexId::parse("C4807").unwrap().to_hex_coord();
        let n = city.neighbors();
        let (adj1, adj2) = (n[0], n[1]); // both adjoin the city
        for h in [city, adj1, adj2] {
            g.map.set_hex(h, HexTerrain::Clear);
        }
        let build_setup = |g: &mut GameState, hex, name: &str| {
            let hq = g.spawn_unit(Nationality::Commonwealth, "a", name, Composition::Organic { current_toe: 0 });
            g.unit_mut(hq).unwrap().location = Some(UnitLocation::OnMap(hex));
            g.supply_dumps.push(SupplyDump {
                location: UnitLocation::OnMap(hex),
                side: Side::Commonwealth,
                supply: Commodities { fuel: 0, ammo: 0, stores: 10, water: 0 },
                dummy: false,
            });
            hq
        };
        let hq1 = build_setup(&mut g, adj1, "hq1");
        let hq2 = build_setup(&mut g, adj2, "hq2");

        // §52.24: the first pipeline hex builds; a second by the same side this stage is blocked.
        assert!(g.build_pipeline(hq1, adj1));
        assert!(!g.build_pipeline(hq2, adj2));
        // A fresh Operations Stage resets the allowance.
        g.begin_operations_stage();
        assert!(g.build_pipeline(hq2, adj2));
    }

    #[test]
    fn facility_repair_by_percentage() {
        use crate::state::game::{FacilityKind, GameState};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation, WeaponGroup};

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_weapons_systems(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/weapons/commonwealth.ron"),
        )
        .unwrap();
        refs.load_repair_table(include_str!(
            "../../../data/reference/repair-table.ron"
        ))
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let a = HexCoord::new(0, 0);
        let unit = g.spawn_unit(
            Nationality::Commonwealth,
            "g",
            "u",
            Composition::Weapons {
                groups: vec![WeaponGroup { system: "A9 Cruiser".into(), count: 0 }],
            },
        );
        g.unit_mut(unit).unwrap().location = Some(UnitLocation::OnMap(a));
        g.unit_mut(unit).unwrap().weapons_broken = vec![WeaponGroup { system: "A9 Cruiser".into(), count: 20 }];
        g.unit_mut(unit).unwrap().trucks_broken.medium = 4;
        g.repair_facilities.insert(a, FacilityKind::Major);

        // §22.8 Major facility, die 0 → 75%: ⌈4×75%⌉=3 trucks + ⌈20×75%⌉=15 tanks = 18 repaired.
        assert_eq!(g.facility_repair(unit, 0), 18);
        assert_eq!(g.unit(unit).unwrap().trucks.medium, 3);
        assert_eq!(g.unit(unit).unwrap().trucks_broken.medium, 1);
        assert_eq!(g.unit(unit).unwrap().total_toe(), 15);
        let broken: u32 = g.unit(unit).unwrap().weapons_broken.iter().map(|g| g.count).sum();
        assert_eq!(broken, 5);
        // No facility → no repair.
        g.repair_facilities.remove(&a);
        assert_eq!(g.facility_repair(unit, 0), 0);
    }

    #[test]
    fn replacement_points_train_and_absorb() {
        use crate::state::game::{GameState, ReplacementClass};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let a = HexCoord::new(0, 0);
        // "h" infantry, max TOE 5, depleted to 2.
        let unit = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "u",
            Composition::Organic { current_toe: 2 },
        );
        g.unit_mut(unit).unwrap().location = Some(UnitLocation::OnMap(a));

        // §20.43: Infantry Replacement Points need 3 Operations Stages of training.
        let rp = g.add_replacement_points(
            ReplacementClass::Infantry,
            2,
            Some(UnitLocation::OnMap(a)),
            Some(unit),
        );
        assert_eq!(g.replacement_points[rp].training_remaining, 3);
        // Untrained → cannot be absorbed.
        assert_eq!(g.absorb_replacement(rp, unit), None);
        // Advance three Operations Stages of training (begin_operations_stage ticks it).
        for _ in 0..3 {
            g.begin_operations_stage();
        }
        assert_eq!(g.replacement_points[rp].training_remaining, 0);
        // Absorb → restores TOE, capped at max_toe 5 (2 + 2 = 4).
        assert_eq!(g.absorb_replacement(rp, unit), Some(2));
        assert_eq!(g.unit(unit).unwrap().total_toe(), 4);
        assert!(g.replacement_points.is_empty());
    }

    #[test]
    fn replacement_planning_arrives() {
        use crate::state::game::{GameState, ReplacementClass};
        use crate::state::map::HexCoord;
        use crate::state::unit::UnitLocation;

        let mut g = GameState::new(ReferenceData::new(), 1);
        let a = HexCoord::new(0, 0);
        // §20.21: plan Infantry Replacement Points to arrive at Game-Turn 3, OpStage 1.
        g.plan_replacement(ReplacementClass::Infantry, 5, 3, 1, Some(UnitLocation::OnMap(a)), None);

        // Not yet due (GT1) — stays pending.
        g.game_turn = 1;
        g.operations_stage = 1;
        g.process_replacement_arrivals();
        assert!(g.replacement_points.is_empty());
        assert_eq!(g.pending_replacements.len(), 1);

        // At GT3 / OpStage 1 the Naval Convoy Arrival Phase brings it into the pool.
        g.game_turn = 3;
        g.operations_stage = 1;
        g.process_replacement_arrivals();
        assert!(g.pending_replacements.is_empty());
        assert_eq!(g.replacement_points.len(), 1);
        let rp = &g.replacement_points[0];
        assert_eq!(rp.count, 5);
        assert_eq!(rp.class, ReplacementClass::Infantry);
        assert_eq!(rp.training_remaining, 3); // Infantry Training (§20.43)
        assert_eq!(rp.location, Some(UnitLocation::OnMap(a)));
    }

    #[test]
    fn unused_replacement_victory_points() {
        use crate::reference::units::Side;
        use crate::state::game::{GameState, ReplacementClass};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Italian,
            include_str!("../../../data/reference/unit-characteristics/italian.ron"),
        )
        .unwrap();
        refs.load_production_chart(
            Nationality::Italian,
            include_str!("../../../data/reference/italian-production.ron"),
        )
        .unwrap();
        let allotted: u32 = refs
            .production_charts
            .get(&Nationality::Italian)
            .unwrap()
            .items
            .iter()
            .map(|i| i.total)
            .sum();
        let mut g = GameState::new(refs, 1);
        // §64.74: initially all allotted Replacement Points are unused.
        assert_eq!(g.replacement_vp(Side::Axis), allotted);
        assert_eq!(g.replacement_vp(Side::Commonwealth), 0);

        // Absorb a 5-point pool → 5 points consumed → 5 fewer unused-replacement VP.
        let a = HexCoord::new(0, 0);
        let unit = g.spawn_unit(
            Nationality::Italian,
            "p",
            "u",
            Composition::Organic { current_toe: 1 },
        );
        g.unit_mut(unit).unwrap().location = Some(UnitLocation::OnMap(a));
        let rp = g.add_replacement_points(ReplacementClass::Infantry, 5, Some(UnitLocation::OnMap(a)), None);
        g.replacement_points[rp].training_remaining = 0;
        assert!(g.absorb_replacement(rp, unit).is_some());
        assert_eq!(g.replacement_vp(Side::Axis), allotted - 5);
    }

    #[test]
    fn squadron_ready_arm_fly_commands_through_apply() {
        use crate::reference::aircraft::AircraftClass;
        use crate::reference::squadron::SquadronClass;
        use crate::state::air::{Sgsu, SquadronId};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;

        let refs = ReferenceData::new();
        let mut g = GameState::new(refs, 1);
        g.reference_mut().load_aircraft_refit(include_str!("../../../data/reference/aircraft-refit.ron")).unwrap();
        g.reference_mut().load_aircraft_chart(Nationality::German, include_str!("../../../data/reference/aircraft/german.ron")).unwrap();
        let mut sq = Sgsu::new(SquadronId(1), Nationality::German, SquadronClass::Fighter, "JG 27", 5, Some(HexCoord::new(1, 1)));
        sq.add_planes("Bf.109F", AircraftClass::Fighter, 16, 1);
        g.sgsus.insert(SquadronId(1), sq);

        // These are Land Support Air Phase actions.
        let ready = Command::ReadySquadron { squadron: SquadronId(1), count: 20 };
        assert_eq!(g.apply(ready.clone()), Err(CommandError::WrongPhase));
        while g.phase != Phase::LandSupportAir {
            g.advance_phase();
        }
        // §38.36/§38.21/§38.43: seed Stores/Fuel/Ammo at the base now (after any turn-start fuel
        // evaporation), so the consumption below is exact.
        g.add_supply_dump(crate::state::unit::UnitLocation::OnMap(HexCoord::new(1, 1)), crate::reference::units::Side::Axis, 5, 20);
        g.supply_dumps.last_mut().unwrap().supply.stores = 5;
        // Refit first: German +1 (§38.35), roll 1+1=2 -> 80% -> ceil(16*0.8) = 13 refitted.
        assert_eq!(
            g.apply(Command::RefitSquadron { squadron: SquadronId(1), base_die: 1, wrong_squadron: false }),
            Ok(CommandOutcome::SquadronRefitted(13))
        );
        // Ready (capped at 12, §37.24), arm 8, fly 10.
        assert_eq!(g.apply(ready), Ok(CommandOutcome::SquadronReadied(12)));
        assert_eq!(g.apply(Command::ArmSquadron { squadron: SquadronId(1), count: 8 }), Ok(CommandOutcome::SquadronArmed(8)));
        assert_eq!(g.apply(Command::FlySquadronMission { squadron: SquadronId(1), count: 10 }), Ok(CommandOutcome::SquadronFlew(10)));
        assert_eq!(g.sgsus[&SquadronId(1)].ready_planes(), 2);
        // §38.21: 12 Bf.109F refuelled at 1 Fuel each -> 20-12 = 8 Fuel left. §38.43: 1 Ammo spent.
        let dump = g.supply_dumps.last().unwrap();
        assert_eq!(dump.supply.fuel, 8);
        assert_eq!(dump.supply.ammo, 4);
        assert_eq!(dump.supply.stores, 4); // 1 spent on refit
    }

    #[test]
    fn commonwealth_fleet_commands_30() {
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::naval::{Ship, ShipLocation};

        let refs = ReferenceData::new();
        let mut g = GameState::new(refs, 1);
        g.fleet.push(Ship::new("Valiant", 6, 12)); // BB
        g.fleet.push(Ship::new("Ajax", 4, 8)); // heavy cruiser, will be damaged
        g.fleet[1].damage(3); // Gun 1 (< 50%) -> not seaworthy

        let sail = Command::AssignShip { ship_index: 0, location: ShipLocation::AtSea(HexCoord::new(5, 5)) };
        // Fleet actions only in the Commonwealth Fleet Phase.
        assert_eq!(g.apply(sail.clone()), Err(CommandError::WrongPhase));
        while g.phase != Phase::CommonwealthFleet {
            g.advance_phase();
        }
        // Seaworthy BB can put to sea.
        assert_eq!(g.apply(sail), Ok(CommandOutcome::ShipAssigned));
        assert_eq!(g.fleet[0].location, ShipLocation::AtSea(HexCoord::new(5, 5)));
        assert_eq!(g.fleet[0].at_sea_stages, 1);
        // The damaged Ajax (<50%) may not leave port (§30.36).
        assert_eq!(
            g.apply(Command::AssignShip { ship_index: 1, location: ShipLocation::AtSea(HexCoord::new(6, 6)) }),
            Err(CommandError::ShipNotSeaworthy)
        );
        // Ajax repairs in Alexandria (already there): one stage toward the next Gun level.
        assert_eq!(g.apply(Command::RepairShip { ship_index: 1 }), Ok(CommandOutcome::ShipRepaired(1)));
        assert_eq!(g.fleet[1].repair_stages, 1);
    }

    #[test]
    fn malta_raid_and_construction_44() {
        use crate::reference::aircraft::AircraftClass;
        use crate::reference::squadron::SquadronClass;
        use crate::state::air::{MaltaAirBase, Sgsu, SquadronId};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;

        let mut refs = ReferenceData::new();
        refs.load_air_bombardment(include_str!("../../../data/reference/air-bombardment.ron")).unwrap();
        refs.load_malta_construction(include_str!("../../../data/reference/malta-construction.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        g.malta_bases.push(MaltaAirBase::new("Luqa", 6, 6));
        let mut sq = Sgsu::new(SquadronId(1), Nationality::German, SquadronClass::Bomber, "KG 54", 5, Some(HexCoord::new(1, 1)));
        sq.add_planes("Ju.88A", AircraftClass::Bomber, 12, 1);
        g.sgsus.insert(SquadronId(1), sq);
        for grp in g.sgsus.get_mut(&SquadronId(1)).unwrap().planes.iter_mut() {
            grp.refitted = grp.count;
            grp.ready = grp.count;
        }

        // §44.24: the raid happens in the Malta Raid Phase.
        let raid = Command::BombMaltaBase {
            squadron: SquadronId(1), planes: 12, bombload_per_plane: 20, base_index: 0,
            interceptors: None, flak_destroy_roll: 11, flak_abort_roll: 11, bomb_roll: 66,
        };
        assert_eq!(g.apply(raid.clone()), Err(CommandError::WrongPhase));
        let mut guard = 0;
        while g.phase != Phase::MaltaRaid {
            g.advance_phase();
            guard += 1;
            assert!(guard < 60, "never reached MaltaRaid");
        }
        match g.apply(raid) {
            Ok(CommandOutcome::MaltaBaseBombed { levels_lost, .. }) => {
                assert!(levels_lost >= 1, "a heavy raid should reduce the base");
                assert_eq!(g.malta_bases[0].levels, 6 - levels_lost);
            }
            other => panic!("unexpected: {other:?}"),
        }

        // §44.13: the Commonwealth rebuilds levels in the Organization Phase (die 6 -> 2 levels).
        let before = g.malta_bases[0].levels;
        guard = 0;
        while g.phase != Phase::Organization {
            g.advance_phase();
            guard += 1;
            assert!(guard < 60, "never reached Organization");
        }
        match g.apply(Command::ConstructMaltaFacility { base_index: 0, die: 6 }) {
            Ok(CommandOutcome::MaltaFacilityConstructed(built)) => {
                assert_eq!(built, (6 - before).min(2));
                assert_eq!(g.malta_bases[0].levels, before + built);
            }
            other => panic!("unexpected: {other:?}"),
        }
        // §44.13: only once per Game-Turn per base.
        assert_eq!(g.apply(Command::ConstructMaltaFacility { base_index: 0, die: 6 }), Ok(CommandOutcome::MaltaFacilityConstructed(0)));
    }

    #[test]
    fn artillery_and_san_giorgio_vs_ship_30_35_39() {
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::naval::{Ship, ShipLocation};
        use crate::state::unit::{Composition, UnitLocation, WeaponGroup};

        let mut refs = ReferenceData::new();
        refs.load_air_bombardment(include_str!("../../../data/reference/air-bombardment.ron")).unwrap();
        refs.load_weapons_systems(
            Nationality::Italian,
            include_str!("../../../data/reference/weapons/italian.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        g.fleet.push(Ship::new("Ajax", 4, 8));
        let ship_hex = HexCoord::new(9, 9);

        // §30.39: the San Giorgio fires at a Commonwealth ship in an adjacent hex.
        let sg_hex = HexCoord::new(9, 8);
        assert!(sg_hex.is_adjacent(ship_hex));
        g.place_san_giorgio(sg_hex, "Tobruk", 5, 3);

        // Axis artillery: 10x 149/13 Howitzer (Barrage 15 -> raw 150; mobile -> halved to 75).
        let arty = g.spawn_unit(
            Nationality::Italian,
            "x",
            "arty",
            Composition::Weapons { groups: vec![WeaponGroup { system: "149/13 Howitzer".into(), count: 10 }] },
        );

        let atk = Command::ArtilleryBombardShip { attackers: vec![arty], ship_index: 0 };
        assert_eq!(g.apply(atk.clone()), Err(CommandError::WrongPhase));
        while g.phase != Phase::MovementAndCombat {
            g.advance_phase();
        }
        // The ship must be at sea for either attack.
        assert_eq!(g.apply(Command::SanGiorgioAttackShip { ship_index: 0 }), Err(CommandError::ShipCannotReachTarget));
        g.fleet[0].location = ShipLocation::AtSea(ship_hex);

        // Artillery not co-located with the ship can't fire on it.
        g.unit_mut(arty).unwrap().location = Some(UnitLocation::OnMap(HexCoord::new(0, 0)));
        assert_eq!(g.apply(atk.clone()), Err(CommandError::NotAdjacent));
        // Co-locate the artillery with the ship and fire (no ammo economy seeded -> no dump means
        // the barrage ammo check may block; seed a dump so the fire resolves).
        g.unit_mut(arty).unwrap().location = Some(UnitLocation::OnMap(ship_hex));
        g.add_supply_dump(UnitLocation::OnMap(ship_hex), crate::reference::units::Side::Axis, 999, 0);
        let before = g.fleet[0].gun_rating;
        match g.apply(atk) {
            Ok(CommandOutcome::ShipDamaged(d)) => {
                assert!(d >= 0);
                assert_eq!(g.fleet[0].gun_rating, before.saturating_sub(d as u32));
            }
            other => panic!("unexpected: {other:?}"),
        }

        // San Giorgio now fires at the (adjacent) ship too.
        match g.apply(Command::SanGiorgioAttackShip { ship_index: 0 }) {
            Ok(CommandOutcome::ShipDamaged(_)) => {}
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn san_giorgio_blocks_aa_and_is_removed_30_17() {
        use crate::reference::ports::PortSpec;
        use crate::state::game::GameState;
        use crate::state::map::HexCoord;
        use crate::state::naval::Port;

        let refs = ReferenceData::new();
        let mut g = GameState::new(refs, 1);

        // Tobruk: max Efficiency 5, San Giorgio holds it three levels down (starts at 2, §55.25).
        let tobruk = Port::from_spec("Tobruk", &PortSpec { efficiency_level: 5, stacking_in: Some(1), stacking_out: Some(3), max_tonnage: 1700, starts_blocked: 3, available_from_gt: None });
        assert_eq!(tobruk.efficiency, 2);
        assert_eq!(tobruk.wreck_block, 3);
        g.ports.insert("Tobruk".into(), tobruk);
        let sg_hex = HexCoord::new(7, 7);
        g.place_san_giorgio(sg_hex, "Tobruk", 4, 6); // Barrage 4 (vs ships), AA 6 (§60 note c)

        // While the wreck holds it down, Tobruk can only repair back up to 2.
        for _ in 0..10 { g.ports.get_mut("Tobruk").unwrap().regain_efficiency(); }
        assert_eq!(g.ports["Tobruk"].efficiency, 2);

        // §60 note c: it fires at aircraft with its AA rating (but never at land targets).
        assert_eq!(g.san_giorgio.as_ref().unwrap().aa_points(), 6);
        assert_eq!(g.san_giorgio.as_ref().unwrap().bombardment_points(), 4);

        // §30.17: engineers finally remove the wreck; its AA falls to zero and Tobruk's hold-down
        // lifts, so it can repair toward its full Efficiency Level.
        assert!(!g.demolish_san_giorgio(3)); // 4 -> 1, still present
        assert!(g.demolish_san_giorgio(1)); // removed
        assert!(g.san_giorgio.is_none());
        assert_eq!(g.ports["Tobruk"].wreck_block, 0);
        for _ in 0..10 { g.ports.get_mut("Tobruk").unwrap().regain_efficiency(); }
        assert_eq!(g.ports["Tobruk"].efficiency, 5);
    }

    #[test]
    fn off_shore_bombardment_30_2() {
        use crate::reference::crt::BarrageTargetClass;
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::reference::terrain::HexTerrain;
        use crate::state::naval::{Ship, ShipLocation};
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_barrage_crt(include_str!("../../../data/reference/crt/barrage.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        g.fleet.push(Ship::new("Valiant", 6, 12)); // BB, heavy

        let target_hex = HexCoord::new(4, 4);
        g.map.set_hex(target_hex, HexTerrain::Clear);
        let axis = g.spawn_unit(Nationality::Italian, "h", "inf", Composition::Organic { current_toe: 10 });
        g.unit_mut(axis).unwrap().location = Some(UnitLocation::OnMap(target_hex));

        let cmd = Command::OffShoreBombard { ship_index: 0, target: target_hex, target_class: BarrageTargetClass::Infantry };
        // Off-shore bombardment fires in the Movement & Combat Phase's Barrage Step.
        assert_eq!(g.apply(cmd.clone()), Err(CommandError::WrongPhase));
        while g.phase != Phase::MovementAndCombat {
            g.advance_phase();
        }
        // In Alexandria (not at sea) the ship can't reach the target.
        assert_eq!(g.apply(cmd.clone()), Err(CommandError::ShipCannotReachTarget));
        // Placed at sea in the target's own hex: fires its full Gun Rating (6) as Actual Barrage
        // Points (§30.22), no ammunition spent.
        g.fleet[0].location = ShipLocation::AtSea(target_hex);
        match g.apply(cmd.clone()) {
            Ok(CommandOutcome::Barrage(o)) => assert_eq!(o.actual_points, 6),
            other => panic!("unexpected: {other:?}"),
        }
        // §30.24: a ship fires its Barrage Strength only once per Operations Stage.
        assert_eq!(g.apply(cmd), Err(CommandError::ShipAlreadyFired));
        // Next stage's reset clears the fire flag; a damaged ship (Gun 6 -> 3) contributes fewer
        // points: 3 Actual Barrage Points.
        g.fleet[0].fired_this_stage = false;
        g.fleet[0].damage(3);
        match g.apply(Command::OffShoreBombard { ship_index: 0, target: target_hex, target_class: BarrageTargetClass::Infantry }) {
            Ok(CommandOutcome::Barrage(o)) => assert_eq!(o.actual_points, 3),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn bomb_ship_applies_gun_damage_30_34() {
        use crate::reference::aircraft::AircraftClass;
        use crate::reference::squadron::SquadronClass;
        use crate::state::air::{Sgsu, SquadronId};
        use crate::state::command::{Command, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::naval::Ship;

        let mut refs = ReferenceData::new();
        refs.load_air_bombardment(include_str!("../../../data/reference/air-bombardment.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        g.fleet.push(Ship::new("Queen Elizabeth", 6, 12));
        let mut sq = Sgsu::new(SquadronId(1), Nationality::German, SquadronClass::Bomber, "KG 26", 5, Some(HexCoord::new(1, 1)));
        sq.add_planes("Ju.88D", AircraftClass::Bomber, 12, 1);
        g.sgsus.insert(SquadronId(1), sq);
        for grp in g.sgsus.get_mut(&SquadronId(1)).unwrap().planes.iter_mut() {
            grp.refitted = grp.count;
            grp.ready = grp.count;
        }

        while g.phase != Phase::LandSupportAir {
            g.advance_phase();
        }
        // 12 bombers x 20 = 240 bomb points, no flak, roll 66 -> a "units" result of some damage.
        match g.apply(Command::BombShip {
            squadron: SquadronId(1), planes: 12, bombload_per_plane: 20, ship_index: 0,
            interceptors: None, interceptor_squadron: None, flak_points: 0, flak_destroy_roll: 11, flak_abort_roll: 11, bomb_roll: 66,
        }) {
            Ok(CommandOutcome::BombingMission(o)) => {
                assert!(o.result >= 1, "heavy raid should score damage");
                // §30.34: damage points reduce the ship's Gun Rating.
                assert_eq!(g.fleet[0].gun_rating, 6u32.saturating_sub(o.result as u32));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn repair_reserve_and_strategic_maintenance_commands() {
        use crate::reference::aircraft::AircraftClass;
        use crate::reference::squadron::SquadronClass;
        use crate::state::air::{Sgsu, SquadronId};
        use crate::state::command::{Command, CommandError, CommandOutcome, RepairKind};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, ReserveStatus, UnitLocation};

        let refs = ReferenceData::new();
        let mut g = GameState::new(refs, 1);
        g.reference_mut().load_aircraft_refit(include_str!("../../../data/reference/aircraft-refit.ron")).unwrap();
        let u = g.spawn_unit(Nationality::Commonwealth, "h", "bn", Composition::Organic { current_toe: 5 });
        g.unit_mut(u).unwrap().location = Some(UnitLocation::OnMap(HexCoord::new(3, 3)));
        let sq = Sgsu::new(SquadronId(1), Nationality::German, SquadronClass::Fighter, "JG 27", 5, Some(HexCoord::new(1, 1)));
        g.sgsus.insert(SquadronId(1), sq);
        g.sgsus.get_mut(&SquadronId(1)).unwrap().add_planes("Bf.109F", AircraftClass::Fighter, 12, 1);

        // DesignateReserve — Reserve Designation Phase only.
        let res = Command::DesignateReserve { unit: u, status: ReserveStatus::ReserveI };
        assert_eq!(g.apply(res.clone()), Err(CommandError::WrongPhase));
        while g.phase != Phase::ReserveDesignation {
            g.advance_phase();
        }
        assert_eq!(g.apply(res), Ok(CommandOutcome::ReserveDesignated));
        assert_eq!(g.unit(u).unwrap().reserve, ReserveStatus::ReserveI);

        // Repair — Repair Phase (no broken vehicles -> 0 repaired, but the command path runs).
        while g.phase != Phase::Repair {
            g.advance_phase();
        }
        assert_eq!(g.apply(Command::Repair { unit: u, kind: RepairKind::Trucks, roll: 1 }), Ok(CommandOutcome::Repaired(0)));

        // RefitSquadron is now also allowed in the Strategic Air Recovery / Aircraft Maintenance
        // phase (§33.0 VII.B). No dump at the base -> lenient supply -> §38.36 InsufficientStores.
        while g.phase != Phase::AircraftMaintenance {
            g.advance_phase();
        }
        assert_eq!(
            g.apply(Command::RefitSquadron { squadron: SquadronId(1), base_die: 1, wrong_squadron: false }),
            Err(CommandError::InsufficientStores)
        );
    }

    #[test]
    fn bombing_mission_intercepted_by_cap_39() {
        use crate::reference::aircraft::{AircraftClass, BombingMission};
        use crate::reference::squadron::SquadronClass;
        use crate::state::air::{Sgsu, SquadronId};
        use crate::state::command::{Command, CommandOutcome};
        use crate::state::game::{Flight, GameState, Phase};
        use crate::state::map::HexCoord;

        let mut refs = ReferenceData::new();
        refs.load_air_bombardment(include_str!("../../../data/reference/air-bombardment.ron")).unwrap();
        refs.load_air_to_air(include_str!("../../../data/reference/air-to-air.ron")).unwrap();
        refs.load_aircraft_chart(Nationality::German, include_str!("../../../data/reference/aircraft/german.ron")).unwrap();
        let mut g = GameState::new(refs, 7);
        let mut sq = Sgsu::new(SquadronId(1), Nationality::German, SquadronClass::Bomber, "KG 26", 5, Some(HexCoord::new(1, 1)));
        sq.add_planes("Ju.88D", AircraftClass::Bomber, 12, 1);
        g.sgsus.insert(SquadronId(1), sq);
        for grp in g.sgsus.get_mut(&SquadronId(1)).unwrap().planes.iter_mut() {
            grp.refitted = grp.count;
            grp.ready = grp.count;
        }

        // Strong CAP (high TacAir + pilots + maneuver) intercepts, no flak.
        let cap = Flight { count: 12, tacair: 8, pilot: 3, maneuver: 40 };
        // §40.18: a Commonwealth pilot flies the intercepting CAP squadron and earns ace credit.
        let cw_sq = SquadronId(9);
        g.pilots.push(crate::state::air::Pilot::new(0, Some(cw_sq)));
        while g.phase != Phase::LandSupportAir {
            g.advance_phase();
        }
        match g.apply(Command::FlyBombingMission {
            squadron: SquadronId(1), planes: 12, bombload_per_plane: 20, mission: BombingMission::Personnel,
            interceptors: Some(cap), interceptor_squadron: Some(cw_sq), flak_points: 0, flak_destroy_roll: 11, flak_abort_roll: 11, bomb_roll: 66, night: false,
        }) {
            Ok(CommandOutcome::BombingMission(o)) => {
                assert_eq!(o.flown, 12);
                assert!(o.intercepted > 0, "strong CAP should down some bombers");
                // Intercepted bombers are removed; survivors deliver the bombs.
                assert_eq!(g.sgsus[&SquadronId(1)].plane_count(), 12 - o.intercepted);
                assert_eq!(o.delivered_bomb_points, (12 - o.intercepted) * 20);
                // §40.18: the intercepting squadron's pilot is credited with the kills.
                assert_eq!(g.pilots.iter().find(|p| p.squadron == Some(cw_sq)).unwrap().kills, o.intercepted);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn resolve_air_to_air_45() {
        use crate::state::game::{Flight, GameState};
        let mut refs = ReferenceData::new();
        refs.load_air_to_air(include_str!("../../../data/reference/air-to-air.ron")).unwrap();
        let mut g = GameState::new(refs, 42);
        // 8 Hurricanes (tacair 6, pilot 2, maneuver 39) vs 8 Ju88 fighters (tacair 6, maneuver 30).
        let attacker = Flight { count: 8, tacair: 6, pilot: 2, maneuver: 39 };
        let defender = Flight { count: 8, tacair: 6, pilot: 0, maneuver: 30 };
        let out = g.resolve_air_to_air(attacker, defender).unwrap();
        // 8 matched duels: losses on each side are bounded by the pair count.
        assert!(out.attacker_losses <= 8);
        assert!(out.defender_losses <= 8);
        // The maneuver-advantaged, higher-pilot Hurricanes should not fare worse than the Ju88s.
        assert!(out.defender_losses >= out.attacker_losses);
    }

    #[test]
    fn air_to_air_per_plane_pilots_and_pileon_45() {
        use crate::state::game::{Flight, GameState};
        let refs = || {
            let mut r = ReferenceData::new();
            r.load_air_to_air(include_str!("../../../data/reference/air-to-air.ron")).unwrap();
            r
        };

        // §45.24 empty-roster fallback: with no per-plane roster, resolve_air_to_air_pilots is
        // identical to resolve_air_to_air (same seed → same RNG stream, uniform flight pilot).
        let attacker = Flight { count: 6, tacair: 6, pilot: 2, maneuver: 39 };
        let defender = Flight { count: 6, tacair: 6, pilot: 0, maneuver: 30 };
        let mut g1 = GameState::new(refs(), 7);
        let mut g2 = GameState::new(refs(), 7);
        assert_eq!(
            g1.resolve_air_to_air(attacker, defender),
            g2.resolve_air_to_air_pilots(attacker, defender, &[], &[], false),
        );

        // §45.23 pile-on: 5 attackers vs 2 defenders. Only the 2 primary match-ups involve mutual
        // fire, so at most 2 attackers can be lost — the 3 piled-on attackers fire one-way and take
        // no return fire — while the 2 defenders may both be shot down.
        let many = Flight { count: 5, tacair: 8, pilot: 3, maneuver: 39 };
        let few = Flight { count: 2, tacair: 4, pilot: 0, maneuver: 25 };
        let mut worst_att = 0;
        let mut best_def = 0;
        for seed in 0..40u64 {
            let mut g = GameState::new(refs(), seed);
            let o = g.resolve_air_to_air(many, few).unwrap();
            assert!(o.attacker_losses <= 2, "piled-on attackers take no return fire (§45.24)");
            assert!(o.defender_losses <= 2, "only two defenders present");
            worst_att = worst_att.max(o.attacker_losses);
            best_def = best_def.max(o.defender_losses);
        }
        // Over many seeds the pile-on should down both defenders at least once (5 shots at 2 planes).
        assert_eq!(best_def, 2, "the pile-on can eliminate both defenders");
        // And the strong, outnumbering attackers should sometimes take zero losses.
        assert!(worst_att <= 2);

        // §40.15/§45.24 per-plane pilots: a defender roster where only plane 0 is a 6-rated ace, the
        // rest 0-rated. The call is accepted and losses stay bounded by the counts.
        let mut g = GameState::new(refs(), 3);
        let a = Flight { count: 3, tacair: 6, pilot: 0, maneuver: 30 };
        let d = Flight { count: 3, tacair: 6, pilot: 0, maneuver: 30 };
        let out = g.resolve_air_to_air_pilots(a, d, &[0, 0, 0], &[6, 0, 0], false).unwrap();
        assert!(out.attacker_losses <= 3 && out.defender_losses <= 3);

        // §45.24 vs §45.34: 4 fighters pile onto one tough plane. In fighter-vs-fighter the piled-on
        // attackers fire one-way, so at most the single primary attacker can be lost. When the target
        // is a *screened* plane it returns fire at every attacker (§45.34), so more attackers can die.
        let four = Flight { count: 4, tacair: 3, pilot: 0, maneuver: 25 };
        let tough = Flight { count: 1, tacair: 14, pilot: 0, maneuver: 40 };
        let mut ff_max = 0;
        let mut screened_max = 0;
        for seed in 0..80u64 {
            let mut g1 = GameState::new(refs(), seed);
            ff_max = ff_max.max(g1.resolve_air_to_air_pilots(four, tough, &[], &[], false).unwrap().attacker_losses);
            let mut g2 = GameState::new(refs(), seed);
            screened_max = screened_max.max(g2.resolve_air_to_air_pilots(four, tough, &[], &[], true).unwrap().attacker_losses);
        }
        assert!(ff_max <= 1, "one-way pile-on: only the primary attacker is at risk (§45.24)");
        assert!(screened_max >= 2, "a screened plane returns fire at each attacker (§45.34)");
    }

    #[test]
    fn bombing_mission_resolution_39() {
        use crate::reference::aircraft::{AircraftClass, BombingMission};
        use crate::reference::squadron::SquadronClass;
        use crate::state::air::{Sgsu, SquadronId};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;

        let mut refs = ReferenceData::new();
        refs.load_air_bombardment(include_str!("../../../data/reference/air-bombardment.ron")).unwrap();
        refs.load_anti_aircraft(include_str!("../../../data/reference/anti-aircraft.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        let mut sq = Sgsu::new(SquadronId(1), Nationality::German, SquadronClass::Bomber, "KG 26", 5, Some(HexCoord::new(1, 1)));
        sq.add_planes("Ju.88D", AircraftClass::Bomber, 12, 1);
        g.sgsus.insert(SquadronId(1), sq);
        // Make 12 planes ready to fly (bypassing the maintenance economy for this test).
        for grp in g.sgsus.get_mut(&SquadronId(1)).unwrap().planes.iter_mut() {
            grp.refitted = grp.count;
            grp.ready = grp.count;
        }

        let mission = Command::FlyBombingMission {
            squadron: SquadronId(1), planes: 12, bombload_per_plane: 20, mission: BombingMission::Personnel,
            interceptors: None, interceptor_squadron: None,
            flak_points: 6, flak_destroy_roll: 66, flak_abort_roll: 66, bomb_roll: 66, night: false,
        };
        assert_eq!(g.apply(mission.clone()), Err(CommandError::WrongPhase));
        while g.phase != Phase::LandSupportAir {
            g.advance_phase();
        }
        match g.apply(mission) {
            Ok(CommandOutcome::BombingMission(o)) => {
                assert_eq!(o.flown, 12);
                // With flak (col 6-8 pts, roll 66) some are destroyed/aborted; survivors bomb.
                assert!(o.destroyed + o.aborted <= 12);
                assert_eq!(o.delivered_bomb_points, (12 - o.destroyed - o.aborted) * 20);
                // Planes destroyed by flak are removed from the squadron.
                assert_eq!(g.sgsus[&SquadronId(1)].plane_count(), 12 - o.destroyed);
                // Survivors flew -> no longer ready.
                assert_eq!(g.sgsus[&SquadronId(1)].ready_planes(), 0);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn hex_flak_points_from_enemy_aa_46() {
        use crate::reference::units::Side;
        use crate::state::game::GameState;
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::German,
            include_str!("../../../data/reference/unit-characteristics/german.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let hex = HexCoord::new(5, 5);
        // German "p" infantry: AA rating 1 (§46.11 → TOE × 1). toe 4 → 4 flak points.
        let u = g.spawn_unit(Nationality::German, "p", "flak", Composition::Organic { current_toe: 4 });
        g.unit_mut(u).unwrap().location = Some(UnitLocation::OnMap(hex));

        // To a Commonwealth bomber the German unit is enemy AA → 4 points.
        assert_eq!(g.hex_flak_points(hex, Side::Commonwealth), 4);
        // To an Axis bomber the same unit is friendly → no flak.
        assert_eq!(g.hex_flak_points(hex, Side::Axis), 0);
        // An empty hex has no flak.
        assert_eq!(g.hex_flak_points(HexCoord::new(9, 9), Side::Commonwealth), 0);
    }

    #[test]
    fn fly_bombing_mission_at_map_validation_37() {
        use crate::reference::aircraft::{AircraftClass, BombingMission};
        use crate::reference::squadron::SquadronClass;
        use crate::state::air::{Sgsu, SquadronId};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase, Weather};
        use crate::state::map::{section_char_of_col, HexCoord};

        let mut refs = ReferenceData::new();
        refs.load_aircraft_chart(
            Nationality::German,
            include_str!("../../../data/reference/aircraft/german.ron"),
        )
        .unwrap();
        refs.load_air_bombardment(include_str!("../../../data/reference/air-bombardment.ron")).unwrap();
        refs.load_anti_aircraft(include_str!("../../../data/reference/anti-aircraft.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        let base = HexCoord::new(1, 1);
        // Ju.88D bomber: range 118, day Strike (bombing) capable.
        let mut sq = Sgsu::new(SquadronId(1), Nationality::German, SquadronClass::Bomber, "KG 26", 5, Some(base));
        sq.add_planes("Ju.88D", AircraftClass::Bomber, 12, 1);
        g.sgsus.insert(SquadronId(1), sq);
        for grp in g.sgsus.get_mut(&SquadronId(1)).unwrap().planes.iter_mut() {
            grp.refitted = grp.count;
            grp.ready = grp.count;
        }

        let mission = |target: HexCoord| Command::FlyBombingMissionAt {
            squadron: SquadronId(1),
            target,
            mission: BombingMission::Personnel,
            planes: 12,
            bombload_per_plane: 20,
            night: false,
            escort: None,
            abort_planes: 0,
            flak_destroy_roll: 66,
            flak_abort_roll: 66,
            bomb_roll: 66,
        };
        let near = HexCoord::new(3, 1); // distance 2 ≤ 118
        let far = HexCoord::new(200, 1); // well beyond range 118

        // Wrong phase.
        assert_eq!(g.apply(mission(near)), Err(CommandError::WrongPhase));
        while g.phase != Phase::LandSupportAir {
            g.advance_phase();
        }
        // Beyond one-way range (§37.12).
        assert_eq!(g.apply(mission(far)), Err(CommandError::TargetOutOfRange));
        // §37.22: a Sandstorm over the base grounds the flight.
        g.weather_by_section.insert(section_char_of_col(base.col), Weather::Sandstorm);
        assert_eq!(g.apply(mission(near)), Err(CommandError::WeatherGrounded));
        g.weather_by_section.clear();
        // In range, clear weather, no enemy AA at the target → all 12 bomb unopposed.
        match g.apply(mission(near)) {
            Ok(CommandOutcome::BombingMission(o)) => {
                assert_eq!(o.flown, 12);
                assert_eq!(o.destroyed + o.aborted, 0); // flak computed from the empty target = 0
                assert_eq!(o.delivered_bomb_points, 12 * 20);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn cap_designation_and_interception_40() {
        use crate::reference::aircraft::{AircraftClass, BombingMission, CapType};
        use crate::reference::squadron::SquadronClass;
        use crate::state::air::{Sgsu, SquadronId};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;

        let mut refs = ReferenceData::new();
        refs.load_aircraft_chart(Nationality::German, include_str!("../../../data/reference/aircraft/german.ron")).unwrap();
        refs.load_aircraft_chart(Nationality::Commonwealth, include_str!("../../../data/reference/aircraft/commonwealth.ron")).unwrap();
        refs.load_air_bombardment(include_str!("../../../data/reference/air-bombardment.ron")).unwrap();
        refs.load_anti_aircraft(include_str!("../../../data/reference/anti-aircraft.ron")).unwrap();
        refs.load_air_to_air(include_str!("../../../data/reference/air-to-air.ron")).unwrap();
        let mut g = GameState::new(refs, 1);

        let center = HexCoord::new(10, 10);
        // Commonwealth CAP fighters (Beaufighter Mk.IF, range 117, CAP-capable) based at `center`.
        let mut cw = Sgsu::new(SquadronId(1), Nationality::Commonwealth, SquadronClass::Fighter, "252 Sqn", 5, Some(center));
        cw.add_planes("Beaufighter Mk.IF", AircraftClass::Fighter, 6, 1);
        g.sgsus.insert(SquadronId(1), cw);
        // German bombers (Ju.88D, range 118) based adjacent.
        let mut ge = Sgsu::new(SquadronId(2), Nationality::German, SquadronClass::Bomber, "KG 26", 5, Some(HexCoord::new(11, 10)));
        ge.add_planes("Ju.88D", AircraftClass::Bomber, 6, 1);
        g.sgsus.insert(SquadronId(2), ge);
        // Bring both squadrons' planes to ready (bypassing the maintenance economy for this test).
        let ready_all = |g: &mut GameState, id: SquadronId| {
            for grp in g.sgsus.get_mut(&id).unwrap().planes.iter_mut() {
                grp.refitted = grp.count;
                grp.ready = grp.count;
            }
        };
        ready_all(&mut g, SquadronId(1));
        ready_all(&mut g, SquadronId(2));

        let cap = Command::DesignateCap { squadron: SquadronId(1), center, cap_type: CapType::Offensive, planes: 4 };
        // Wrong phase first.
        assert_eq!(g.apply(cap.clone()), Err(CommandError::WrongPhase));
        while g.phase != Phase::LandSupportAir {
            g.advance_phase();
        }
        // Designate an offensive CAP with 4 fighters → an air ZOC (§40.23).
        assert_eq!(g.apply(cap), Ok(CommandOutcome::CapDesignated { fighters: 4 }));
        assert_eq!(g.cap_patrols.len(), 1);
        assert_eq!(g.cap_patrols[0].intercepts_used, 0);
        assert_eq!(g.sgsus[&SquadronId(1)].ready_planes(), 2); // 6 - 4 flew to CAP

        let bomb = |target: HexCoord| Command::FlyBombingMissionAt {
            squadron: SquadronId(2), target, mission: BombingMission::Personnel,
            planes: 6, bombload_per_plane: 20, night: false,
            escort: None, abort_planes: 0, flak_destroy_roll: 66, flak_abort_roll: 66, bomb_roll: 66,
        };
        // A target OUTSIDE the 7-hex air ZOC is not intercepted (§40.23).
        let outside = HexCoord::new(14, 10); // 4 hexes from center
        ready_all(&mut g, SquadronId(2));
        assert!(matches!(g.apply(bomb(outside)), Ok(CommandOutcome::BombingMission(_))));
        assert_eq!(g.cap_patrols[0].intercepts_used, 0);
        // A target INSIDE the ZOC is intercepted — the patrol spends its one intercept (§40.27).
        ready_all(&mut g, SquadronId(2));
        assert!(matches!(g.apply(bomb(center)), Ok(CommandOutcome::BombingMission(_))));
        assert_eq!(g.cap_patrols[0].intercepts_used, 1);
        // A second mission into the ZOC finds no interception left this stage (§40.27).
        ready_all(&mut g, SquadronId(2));
        assert!(matches!(g.apply(bomb(center)), Ok(CommandOutcome::BombingMission(_))));
        assert_eq!(g.cap_patrols[0].intercepts_used, 1);

        // Patrols are cleared at the start of the next Operations Stage (§40.2).
        g.begin_operations_stage();
        assert!(g.cap_patrols.is_empty());
    }

    #[test]
    fn strafe_supply_dump_and_pipeline_40() {
        use crate::reference::aircraft::AircraftClass;
        use crate::reference::squadron::SquadronClass;
        use crate::reference::units::Side;
        use crate::rules::air::StrafeTarget;
        use crate::state::air::{Sgsu, SquadronId};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{Commodities, GameState, Phase, StrafeEffect};
        use crate::state::map::HexCoord;
        use crate::state::unit::UnitLocation;

        let mut refs = ReferenceData::new();
        refs.load_aircraft_chart(Nationality::German, include_str!("../../../data/reference/aircraft/german.ron")).unwrap();
        refs.load_strafing_table(include_str!("../../../data/reference/strafing-table.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        let base = HexCoord::new(1, 1);
        let target = HexCoord::new(3, 1);
        // Ju.88D has D (strike) capability → may strafe (§40.51).
        let mut sq = Sgsu::new(SquadronId(1), Nationality::German, SquadronClass::Bomber, "KG 26", 5, Some(base));
        sq.add_planes("Ju.88D", AircraftClass::Bomber, 6, 1);
        g.sgsus.insert(SquadronId(1), sq);
        let ready = |g: &mut GameState| {
            for grp in g.sgsus.get_mut(&SquadronId(1)).unwrap().planes.iter_mut() {
                grp.refitted = grp.count;
                grp.ready = grp.count;
            }
        };
        ready(&mut g);
        let strafe_dump = Command::Strafe {
            squadron: SquadronId(1), target, strafe_target: StrafeTarget::SupplyDump,
            planes: 6, roll: 66,
        };
        // Wrong phase first.
        assert_eq!(g.apply(strafe_dump.clone()), Err(CommandError::WrongPhase));
        while g.phase != Phase::LandSupportAir {
            g.advance_phase();
        }
        // Enemy (Commonwealth) supply dump at the target, 100 of each commodity (placed now so the
        // stage-advance evaporation above doesn't touch it).
        g.add_supply_dump_full(UnitLocation::OnMap(target), Side::Commonwealth, Commodities { fuel: 100, ammo: 100, stores: 100, water: 100 });
        // §40.64: result r destroys r*10% of every commodity in the dump.
        match g.apply(strafe_dump) {
            Ok(CommandOutcome::Strafed { result, effect }) => {
                let pct = result * 10;
                if pct > 0 {
                    assert_eq!(effect, StrafeEffect::SupplyDumpDamaged { percent: pct });
                } else {
                    assert_eq!(effect, StrafeEffect::None);
                }
                assert_eq!(g.supply_dumps[0].supply.fuel, 100 - 100 * pct / 100);
            }
            other => panic!("unexpected: {other:?}"),
        }

        // §40.67: strafing a water pipeline destroys it on a result of 2 or 3.
        ready(&mut g);
        g.pipelines.insert(target);
        match g.apply(Command::Strafe {
            squadron: SquadronId(1), target, strafe_target: StrafeTarget::WaterPipeline,
            planes: 6, roll: 66,
        }) {
            Ok(CommandOutcome::Strafed { result, effect }) => {
                if result >= 2 {
                    assert_eq!(effect, StrafeEffect::PipelineDestroyed);
                    assert!(!g.pipelines.contains(&target));
                } else {
                    assert!(g.pipelines.contains(&target));
                }
            }
            other => panic!("unexpected: {other:?}"),
        }

        // §40.62: strafing an enemy free Truck Convoy removes Truck Points (and cargo in proportion).
        ready(&mut g);
        g.truck_convoys.push(crate::state::game::TruckConvoy {
            side: Side::Commonwealth,
            hex: target,
            trucks: crate::state::unit::TruckPoints { light: 0, medium: 20, heavy: 0 },
            cargo: Commodities { fuel: 100, ammo: 0, stores: 0, water: 0 },
            cpa_used: 0,
            carries_replacements: false,
        });
        match g.apply(Command::Strafe {
            squadron: SquadronId(1), target, strafe_target: StrafeTarget::TrucksInConvoy,
            planes: 6, roll: 66,
        }) {
            Ok(CommandOutcome::Strafed { result, effect }) => {
                let removed = result.min(20);
                assert_eq!(g.truck_convoys[0].trucks.medium, 20 - removed);
                assert_eq!(g.truck_convoys[0].cargo.fuel, 100 - 100 * removed / 20);
                if removed > 0 {
                    assert_eq!(effect, StrafeEffect::TrucksEliminated { points: removed });
                } else {
                    assert_eq!(effect, StrafeEffect::None);
                }
            }
            other => panic!("unexpected: {other:?}"),
        }

        // §40.66: Ports are not a standalone strafing mission (they fold into a bombing raid).
        ready(&mut g);
        assert_eq!(
            g.apply(Command::Strafe {
                squadron: SquadronId(1), target, strafe_target: StrafeTarget::Port,
                planes: 6, roll: 66,
            }),
            Err(CommandError::MissionNotPermitted)
        );
    }

    #[test]
    fn strafe_infantry_and_grounded_aircraft_40_6() {
        use crate::reference::aircraft::AircraftClass;
        use crate::reference::squadron::SquadronClass;
        use crate::rules::air::StrafeTarget;
        use crate::state::air::{Sgsu, SquadronId};
        use crate::state::command::{Command, CommandOutcome};
        use crate::state::game::{GameState, Phase, StrafeEffect};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_aircraft_chart(Nationality::German, include_str!("../../../data/reference/aircraft/german.ron")).unwrap();
        refs.load_aircraft_chart(Nationality::Commonwealth, include_str!("../../../data/reference/aircraft/commonwealth.ron")).unwrap();
        refs.load_unit_characteristics(Nationality::Commonwealth, include_str!("../../../data/reference/unit-characteristics/commonwealth.ron")).unwrap();
        refs.load_strafing_table(include_str!("../../../data/reference/strafing-table.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        let base = HexCoord::new(1, 1);
        let target = HexCoord::new(3, 1);
        // German strafer (Ju.88D has D capability).
        let mut sq = Sgsu::new(SquadronId(1), Nationality::German, SquadronClass::Bomber, "KG 26", 5, Some(base));
        sq.add_planes("Ju.88D", AircraftClass::Bomber, 6, 1);
        g.sgsus.insert(SquadronId(1), sq);
        let ready = |g: &mut GameState| {
            for grp in g.sgsus.get_mut(&SquadronId(1)).unwrap().planes.iter_mut() {
                grp.refitted = grp.count;
                grp.ready = grp.count;
            }
        };
        ready(&mut g);
        while g.phase != Phase::LandSupportAir {
            g.advance_phase();
        }

        // §40.6 Infantry: the result is TOE Strength Points eliminated from an enemy infantry unit.
        let inf = g.spawn_unit(Nationality::Commonwealth, "h", "inf", Composition::Organic { current_toe: 5 });
        g.unit_mut(inf).unwrap().location = Some(UnitLocation::OnMap(target));
        match g.apply(Command::Strafe {
            squadron: SquadronId(1), target, strafe_target: StrafeTarget::Infantry, planes: 6, roll: 66,
        }) {
            Ok(CommandOutcome::Strafed { result, effect }) => {
                let removed = result.min(5);
                if removed > 0 {
                    assert_eq!(effect, StrafeEffect::InfantryEliminated { points: removed });
                }
                assert_eq!(g.unit(inf).unwrap().total_toe(), 5 - removed);
            }
            other => panic!("unexpected: {other:?}"),
        }

        // §40.6 Grounded Aircraft: the result is the number of d6 rolled; their sum is planes lost.
        ready(&mut g);
        let mut cw = Sgsu::new(SquadronId(2), Nationality::Commonwealth, SquadronClass::Fighter, "sitting duck", 5, Some(target));
        cw.add_planes("Beaufighter Mk.IF", AircraftClass::Fighter, 12, 1);
        g.sgsus.insert(SquadronId(2), cw);
        let before = g.sgsus[&SquadronId(2)].plane_count();
        match g.apply(Command::Strafe {
            squadron: SquadronId(1), target, strafe_target: StrafeTarget::GroundedAircraft, planes: 6, roll: 66,
        }) {
            Ok(CommandOutcome::Strafed { effect, .. }) => {
                let lost = before - g.sgsus[&SquadronId(2)].plane_count();
                if lost > 0 {
                    assert_eq!(effect, StrafeEffect::GroundedAircraftDestroyed { planes: lost });
                } else {
                    assert_eq!(effect, StrafeEffect::None);
                }
            }
            other => panic!("unexpected: {other:?}"),
        }

        // §40.62 First-line trucks: the result is Truck Points eliminated, cargo lost in proportion.
        ready(&mut g);
        let mover = g.spawn_unit(Nationality::Commonwealth, "h", "trucked", Composition::Organic { current_toe: 3 });
        {
            let u = g.unit_mut(mover).unwrap();
            u.location = Some(UnitLocation::OnMap(target));
            u.trucks.light = 10;
            u.truck_cargo.fuel = 100;
        }
        match g.apply(Command::Strafe {
            squadron: SquadronId(1), target, strafe_target: StrafeTarget::FirstLineTrucks, planes: 6, roll: 66,
        }) {
            Ok(CommandOutcome::Strafed { result, effect }) => {
                let removed = result.min(10);
                if removed > 0 {
                    assert_eq!(effect, StrafeEffect::TrucksEliminated { points: removed });
                }
                assert_eq!(g.unit(mover).unwrap().trucks.light, 10 - removed);
                assert_eq!(g.unit(mover).unwrap().truck_cargo.fuel, 100 - 100 * removed / 10);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn airdrop_supplies_42_4() {
        use crate::reference::aircraft::AircraftClass;
        use crate::reference::squadron::SquadronClass;
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::air::{AirFacility, AirFacilityKind, Sgsu, SquadronId};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{Commodities, Commodity, GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::UnitLocation;

        let mut refs = ReferenceData::new();
        refs.load_aircraft_chart(Nationality::German, include_str!("../../../data/reference/aircraft/german.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        let base = HexCoord::new(5, 6);
        let target = HexCoord::new(5, 5);
        g.map.set_hex(target, HexTerrain::Clear);
        // A recognizable land feature at the target (an air facility) satisfies §42.44.
        g.air_facilities.push(AirFacility::new(AirFacilityKind::Airfield, target));
        // Ju.52/3m transport (transport: Day, range 72).
        let mut sq = Sgsu::new(SquadronId(1), Nationality::German, SquadronClass::Transport, "KGrzbV", 5, Some(base));
        sq.add_planes("Ju.52/3m", AircraftClass::Transport, 3, 1);
        g.sgsus.insert(SquadronId(1), sq);
        let ready = |g: &mut GameState| {
            for grp in g.sgsus.get_mut(&SquadronId(1)).unwrap().planes.iter_mut() {
                grp.refitted = grp.count;
                grp.ready = grp.count;
            }
        };
        ready(&mut g);
        while g.phase != Phase::LandSupportAir {
            g.advance_phase();
        }
        // A base dump with Stores to drop (placed after the stage-advance so SGSU upkeep §35.14
        // doesn't nibble it).
        g.add_supply_dump_full(UnitLocation::OnMap(base), Side::Axis, Commodities { fuel: 0, ammo: 0, stores: 100, water: 0 });
        // §42.46: Fuel may not be airdropped.
        assert_eq!(
            g.apply(Command::AirdropSupplies { squadron: SquadronId(1), target, commodity: Commodity::Fuel, amount: 10, planes: 1 }),
            Err(CommandError::MissionNotPermitted)
        );
        // A legal Stores drop: base dump loses 50, target gains a dump with 50.
        assert_eq!(
            g.apply(Command::AirdropSupplies { squadron: SquadronId(1), target, commodity: Commodity::Stores, amount: 50, planes: 1 }),
            Ok(CommandOutcome::SuppliesAirdropped { amount: 50 })
        );
        let base_stores: u32 = g.supply_dumps.iter().filter(|d| d.location.hex() == Some(base)).map(|d| d.supply.stores).sum();
        let target_stores: u32 = g.supply_dumps.iter().filter(|d| d.location.hex() == Some(target)).map(|d| d.supply.stores).sum();
        assert_eq!(base_stores, 50);
        assert_eq!(target_stores, 50);

        // A drop zone with no land feature nearby is illegal (§42.43/§42.44). Within Ju.52 range
        // (72) of the base, but far from the only land feature (the airfield at the target).
        ready(&mut g);
        let nowhere = HexCoord::new(5, 20);
        g.map.set_hex(nowhere, HexTerrain::Clear);
        assert_eq!(
            g.apply(Command::AirdropSupplies { squadron: SquadronId(1), target: nowhere, commodity: Commodity::Stores, amount: 10, planes: 1 }),
            Err(CommandError::DropZoneIllegal)
        );
    }

    #[test]
    fn strafe_tanks_40_65() {
        use crate::reference::aircraft::AircraftClass;
        use crate::reference::squadron::SquadronClass;
        use crate::rules::air::StrafeTarget;
        use crate::state::air::{Sgsu, SquadronId};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase, StrafeEffect};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation, WeaponGroup};

        let mut refs = ReferenceData::new();
        refs.load_aircraft_chart(Nationality::Commonwealth, include_str!("../../../data/reference/aircraft/commonwealth.ron")).unwrap();
        refs.load_aircraft_chart(Nationality::German, include_str!("../../../data/reference/aircraft/german.ron")).unwrap();
        refs.load_weapons_systems(Nationality::German, include_str!("../../../data/reference/weapons/german.ron")).unwrap();
        refs.load_strafing_table(include_str!("../../../data/reference/strafing-table.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        let base = HexCoord::new(1, 1);
        let target = HexCoord::new(3, 1);
        // A German tank unit (5× Pz II, armour protection 2 each).
        let tank = g.spawn_unit(Nationality::German, "tk", "5th Pz", Composition::Weapons {
            groups: vec![WeaponGroup { system: "Pz II".into(), count: 5 }],
        });
        g.unit_mut(tank).unwrap().location = Some(UnitLocation::OnMap(target));
        // Only the Hurricane Mk.IID (DayStrafeArmor) may strafe tanks (§40.65).
        let mut iid = Sgsu::new(SquadronId(1), Nationality::Commonwealth, SquadronClass::Fighter, "6 Sqn", 5, Some(base));
        iid.add_planes("Hurricane Mk.IID", AircraftClass::Fighter, 6, 1);
        g.sgsus.insert(SquadronId(1), iid);
        // A German dive-bomber squadron cannot strafe tanks.
        let mut stuka = Sgsu::new(SquadronId(2), Nationality::German, SquadronClass::Bomber, "StG", 5, Some(base));
        stuka.add_planes("Ju.87D", AircraftClass::DiveBomber, 6, 1);
        g.sgsus.insert(SquadronId(2), stuka);
        for id in [SquadronId(1), SquadronId(2)] {
            for grp in g.sgsus.get_mut(&id).unwrap().planes.iter_mut() {
                grp.refitted = grp.count;
                grp.ready = grp.count;
            }
        }
        while g.phase != Phase::LandSupportAir {
            g.advance_phase();
        }

        // A non-armour-capable strafer is rejected for the Tanks target.
        assert_eq!(
            g.apply(Command::Strafe { squadron: SquadronId(2), target, strafe_target: StrafeTarget::Tanks, planes: 6, roll: 66 }),
            Err(CommandError::MissionNotPermitted)
        );
        // The Hurricane IID strafes the tanks, removing TOE points to meet the armour-points roll.
        let before = g.unit(tank).unwrap().total_toe();
        match g.apply(Command::Strafe { squadron: SquadronId(1), target, strafe_target: StrafeTarget::Tanks, planes: 4, roll: 66 }) {
            Ok(CommandOutcome::Strafed { effect, .. }) => {
                let removed = before - g.unit(tank).unwrap().total_toe();
                if removed > 0 {
                    assert_eq!(effect, StrafeEffect::TankPointsEliminated { points: removed });
                } else {
                    assert_eq!(effect, StrafeEffect::None);
                }
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn transport_unit_42_3() {
        use crate::reference::aircraft::AircraftClass;
        use crate::reference::squadron::SquadronClass;
        use crate::state::air::{AirFacility, AirFacilityKind, Sgsu, SquadronId};
        use crate::state::command::{Command, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_aircraft_chart(Nationality::German, include_str!("../../../data/reference/aircraft/german.ron")).unwrap();
        refs.load_unit_characteristics(Nationality::German, include_str!("../../../data/reference/unit-characteristics/german.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        let base = HexCoord::new(2, 2);
        let dest = HexCoord::new(4, 2);
        g.air_facilities.push(AirFacility::new(AirFacilityKind::Airfield, dest));
        // A German leg-infantry unit (no trucks) co-located with the transports.
        let inf = g.spawn_unit(Nationality::German, "p", "II/104", Composition::Organic { current_toe: 2 });
        g.unit_mut(inf).unwrap().location = Some(UnitLocation::OnMap(base));
        let mut tp = Sgsu::new(SquadronId(1), Nationality::German, SquadronClass::Transport, "KGrzbV", 5, Some(base));
        tp.add_planes("Ju.52/3m", AircraftClass::Transport, 3, 1);
        g.sgsus.insert(SquadronId(1), tp);
        for grp in g.sgsus.get_mut(&SquadronId(1)).unwrap().planes.iter_mut() {
            grp.refitted = grp.count;
            grp.ready = grp.count;
        }
        while g.phase != Phase::LandSupportAir {
            g.advance_phase();
        }
        // §42.3: transport the infantry to the friendly airfield.
        assert_eq!(
            g.apply(Command::TransportUnit { squadron: SquadronId(1), unit: inf, target: dest }),
            Ok(CommandOutcome::UnitTransported)
        );
        assert_eq!(g.unit(inf).unwrap().hex(), Some(dest));
        // §42.34: the transported unit has used its CPA and cannot move further this stage.
        let cpa = g.unit_cpa(inf).unwrap();
        assert_eq!(g.unit(inf).unwrap().cp_expended.0, cpa * 10);
    }

    #[test]
    fn airdrop_unit_42_41() {
        use crate::reference::aircraft::AircraftClass;
        use crate::reference::squadron::SquadronClass;
        use crate::reference::terrain::HexTerrain;
        use crate::state::air::{AirFacility, AirFacilityKind, Sgsu, SquadronId};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_aircraft_chart(Nationality::German, include_str!("../../../data/reference/aircraft/german.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        let base = HexCoord::new(2, 2);
        let target = HexCoord::new(4, 2);
        let nowhere = HexCoord::new(2, 10);
        g.map.set_hex(target, HexTerrain::Clear);
        g.map.set_hex(nowhere, HexTerrain::Clear);
        // A recognizable land feature at the drop zone (§42.44).
        g.air_facilities.push(AirFacility::new(AirFacilityKind::Airfield, target));
        // Ju.52 transports + a German airborne unit co-located at the base.
        let mut tp = Sgsu::new(SquadronId(1), Nationality::German, SquadronClass::Transport, "KGrzbV", 5, Some(base));
        tp.add_planes("Ju.52/3m", AircraftClass::Transport, 3, 1);
        g.sgsus.insert(SquadronId(1), tp);
        let ready = |g: &mut GameState| {
            for grp in g.sgsus.get_mut(&SquadronId(1)).unwrap().planes.iter_mut() {
                grp.refitted = grp.count;
                grp.ready = grp.count;
            }
        };
        ready(&mut g);
        let para = g.spawn_unit(Nationality::German, "p", "V/186 Bn", Composition::Organic { current_toe: 2 });
        g.unit_mut(para).unwrap().location = Some(UnitLocation::OnMap(base));
        while g.phase != Phase::LandSupportAir {
            g.advance_phase();
        }

        let drop = |target| Command::AirdropUnit { squadron: SquadronId(1), unit: para, target };
        // §42.41: a non-airborne unit may not be airdropped.
        assert_eq!(g.apply(drop(target)), Err(CommandError::MissionNotPermitted));
        g.unit_mut(para).unwrap().airdroppable = true;
        // §42.43/44: a drop zone with no land feature nearby is illegal.
        assert_eq!(g.apply(drop(nowhere)), Err(CommandError::DropZoneIllegal));
        // A legal drop lands the unit in the target hex (§42.45 — no drift).
        assert_eq!(g.apply(drop(target)), Ok(CommandOutcome::UnitAirdropped));
        assert_eq!(g.unit(para).unwrap().hex(), Some(target));
        // §42.48: it may not be airdropped again within 12 Operations Stages.
        g.unit_mut(para).unwrap().location = Some(UnitLocation::OnMap(base));
        ready(&mut g);
        assert_eq!(g.apply(drop(target)), Err(CommandError::AirdropOnCooldown));
    }

    #[test]
    fn land_bombing_effects_41() {
        use crate::reference::aircraft::{AircraftClass, BombingMission};
        use crate::reference::squadron::SquadronClass;
        use crate::reference::units::Side;
        use crate::state::air::{Sgsu, SquadronId};
        use crate::state::command::{Command, CommandOutcome};
        use crate::state::game::{Commodities, GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_aircraft_chart(Nationality::German, include_str!("../../../data/reference/aircraft/german.ron")).unwrap();
        refs.load_unit_characteristics(Nationality::Commonwealth, include_str!("../../../data/reference/unit-characteristics/commonwealth.ron")).unwrap();
        refs.load_air_bombardment(include_str!("../../../data/reference/air-bombardment.ron")).unwrap();
        refs.load_anti_aircraft(include_str!("../../../data/reference/anti-aircraft.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        let base = HexCoord::new(1, 1);
        let target = HexCoord::new(3, 1);
        let mut sq = Sgsu::new(SquadronId(1), Nationality::German, SquadronClass::Bomber, "KG 26", 5, Some(base));
        sq.add_planes("Ju.88D", AircraftClass::Bomber, 12, 1);
        g.sgsus.insert(SquadronId(1), sq);
        let ready = |g: &mut GameState| {
            for grp in g.sgsus.get_mut(&SquadronId(1)).unwrap().planes.iter_mut() {
                grp.refitted = grp.count;
                grp.ready = grp.count;
            }
        };
        ready(&mut g);
        let cw = g.spawn_unit(Nationality::Commonwealth, "h", "target", Composition::Organic { current_toe: 5 });
        g.unit_mut(cw).unwrap().location = Some(UnitLocation::OnMap(target));
        while g.phase != Phase::LandSupportAir {
            g.advance_phase();
        }

        // §41.31 Personnel bombing pins battalion-equivalents in the target hex.
        let bomb = |mission| Command::FlyBombingMissionAt {
            squadron: SquadronId(1), target, mission,
            planes: 12, bombload_per_plane: 20, night: false,
            escort: None, abort_planes: 0, flak_destroy_roll: 66, flak_abort_roll: 66, bomb_roll: 66,
        };
        match g.apply(bomb(BombingMission::Personnel)) {
            Ok(CommandOutcome::BombingMission(o)) => {
                assert_eq!(g.unit(cw).unwrap().status.pinned, o.result > 0);
            }
            other => panic!("unexpected: {other:?}"),
        }

        // §41.35 Supply-dump bombing destroys `result` percent of each commodity.
        ready(&mut g);
        g.add_supply_dump_full(UnitLocation::OnMap(target), Side::Commonwealth, Commodities { fuel: 100, ammo: 100, stores: 100, water: 100 });
        match g.apply(bomb(BombingMission::SupplyDump)) {
            Ok(CommandOutcome::BombingMission(o)) => {
                let pct = (o.result.max(0) as u32).min(100);
                let dump = g.supply_dumps.iter().find(|d| d.location.hex() == Some(target)).unwrap();
                assert_eq!(dump.supply.fuel, 100 - 100 * pct / 100);
            }
            other => panic!("unexpected: {other:?}"),
        }

        // §41.32 Truck-convoy bombing hits the target unit's first-line attached trucks (Ju.88D has
        // the "D"/strike capability required for first-line trucks).
        ready(&mut g);
        g.unit_mut(cw).unwrap().trucks.light = 8;
        match g.apply(bomb(BombingMission::TruckConvoy)) {
            Ok(CommandOutcome::BombingMission(o)) => {
                let removed = (o.result.max(0) as u32).min(8);
                assert_eq!(g.unit(cw).unwrap().trucks.light, 8 - removed);
            }
            other => panic!("unexpected: {other:?}"),
        }

        // §41.38 Road/Railroad bombing destroys the feature on a positive result. Resolve the
        // effect directly (deterministic) rather than through the CRT roll.
        use crate::reference::terrain::HexTerrain;
        use crate::state::map::HexsideKind;
        let n = target.neighbors()[0]; // a real geometric neighbour of `target`
        g.map.set_hex(target, HexTerrain::Clear);
        g.map.set_hex(n, HexTerrain::Clear);
        g.map.set_hexside(target, n, HexsideKind::Road, None);
        g.map.set_hexside(target, n, HexsideKind::Railroad, None);
        assert!(g.hex_is_railroad(target));
        // Zero result is a no-op.
        g.apply_land_bombing_effect(BombingMission::Railroad, target, Side::Axis, 0);
        assert!(g.hex_is_railroad(target));
        // Positive railroad result → DestroyedRailroad on the hex's edges.
        g.apply_land_bombing_effect(BombingMission::Railroad, target, Side::Axis, 1);
        assert!(!g.hex_is_railroad(target));
        // Positive road result → the Road edge degrades to an UnfinishedRoad (track, §24.52).
        assert!(g.map.hexsides_on(target, n).iter().any(|h| h.kind == HexsideKind::Road));
        g.apply_land_bombing_effect(BombingMission::Road, target, Side::Axis, 1);
        assert!(!g.map.hexsides_on(target, n).iter().any(|h| h.kind == HexsideKind::Road));
        assert!(g.map.hexsides_on(target, n).iter().any(|h| h.kind == HexsideKind::UnfinishedRoad));
    }

    #[test]
    fn replacement_conversion_absorb_20_3() {
        use crate::reference::units::Side;
        use crate::state::game::{GameState, ReplacementClass};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(Nationality::German, include_str!("../../../data/reference/unit-characteristics/german.ron")).unwrap();
        refs.load_weapons_systems(Nationality::German, include_str!("../../../data/reference/weapons/german.ron")).unwrap();
        refs.load_replacement_conversion(include_str!("../../../data/reference/replacement-conversion.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        let a = HexCoord::new(0, 0);

        // §20.3 classifier: recce/HQ/infantry rows, incl. name-based specials.
        // An Organic (unarmored) recce unit is the light Motorcycle/Camel/Reconnaissance row; a
        // recce unit carrying armor-protected points is Armored Reconnaissance (composition-derived).
        let recce = g.spawn_unit(Nationality::German, "ee", "recce", Composition::Organic { current_toe: 2 });
        let armored_recce = g.spawn_unit(Nationality::German, "ee", "armored recce", Composition::Weapons { groups: vec![crate::state::unit::WeaponGroup { system: "Pz II".into(), count: 2 }] });
        let hq = g.spawn_unit(Nationality::German, "e", "hq", Composition::Weapons { groups: vec![] });
        let commando = g.spawn_unit(Nationality::German, "p", "Layforce Commando", Composition::Organic { current_toe: 1 });
        assert_eq!(g.conversion_row_for_unit(recce), "Motorcycle/Camel/Reconnaissance");
        assert_eq!(g.conversion_row_for_unit(armored_recce), "Armored Reconnaissance");
        assert_eq!(g.conversion_row_for_unit(hq), "Any Headquarters Unit");
        assert_eq!(g.conversion_row_for_unit(commando), "Commando");
        // §20.3 footnote-driven name detection.
        let mg = g.spawn_unit(Nationality::German, "p", "II/155 Machine Gun Bn", Composition::Organic { current_toe: 1 });
        let mc = g.spawn_unit(Nationality::German, "ee", "Motorcycle Recon Bn", Composition::Organic { current_toe: 1 });
        assert_eq!(g.conversion_row_for_unit(mg), "Machinegun");
        assert_eq!(g.conversion_row_for_unit(mc), "Motorcycle/Camel/Reconnaissance");
        // German MG battalions are named "Machinegewehr" (§4.46 MG glyph, verified on PDF p183) —
        // detected via the German-language token, not just the English "Machine Gun"/"MG".
        let mgw = g.spawn_unit(Nationality::German, "n", "2nd Machinegewehr Bn", Composition::Organic { current_toe: 4 });
        assert_eq!(g.conversion_row_for_unit(mgw), "Machinegun");

        // §20.3: a Commando costs 3 Infantry Replacement Points per TOE Strength Point rebuilt.
        g.unit_mut(commando).unwrap().location = Some(UnitLocation::OnMap(a));
        let rp = g.add_replacement_points(ReplacementClass::Infantry, 6, Some(UnitLocation::OnMap(a)), Some(commando));
        g.replacement_points[rp].training_remaining = 0;
        // "p" Max-TOE is 4; from TOE 1 there is room for 3 → 6 points / 3-per-SP = 2 SPs rebuilt.
        assert_eq!(g.absorb_replacement(rp, commando), Some(2));
        assert_eq!(g.unit(commando).unwrap().total_toe(), 3);
        // §64.74: all 6 consumed points count against the side's unused-replacement pool.
        assert_eq!(*g.replacements_used.get(&Side::Axis).unwrap(), 6);
        assert!(g.replacement_points.is_empty());
    }

    #[test]
    fn fortification_bombing_and_city_gate_41() {
        use crate::reference::aircraft::{AircraftClass, BombingMission};
        use crate::reference::squadron::SquadronClass;
        use crate::reference::terrain::HexTerrain;
        use crate::state::air::{Sgsu, SquadronId};
        use crate::state::command::{Command, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_aircraft_chart(Nationality::German, include_str!("../../../data/reference/aircraft/german.ron")).unwrap();
        refs.load_unit_characteristics(Nationality::Commonwealth, include_str!("../../../data/reference/unit-characteristics/commonwealth.ron")).unwrap();
        refs.load_air_bombardment(include_str!("../../../data/reference/air-bombardment.ron")).unwrap();
        refs.load_anti_aircraft(include_str!("../../../data/reference/anti-aircraft.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        let base = HexCoord::new(1, 1);
        let fort = HexCoord::new(3, 1);
        let city = HexCoord::new(2, 1);
        g.map.set_hex(city, HexTerrain::MajorCity); // §25.12: static Fortification Level 2
        g.fortifications.insert(fort, 2); // a placed Level-2 fortification
        let mut sq = Sgsu::new(SquadronId(1), Nationality::German, SquadronClass::Bomber, "KG 26", 5, Some(base));
        sq.add_planes("Ju.88D", AircraftClass::Bomber, 12, 1);
        g.sgsus.insert(SquadronId(1), sq);
        let ready = |g: &mut GameState| {
            for grp in g.sgsus.get_mut(&SquadronId(1)).unwrap().planes.iter_mut() {
                grp.refitted = grp.count;
                grp.ready = grp.count;
            }
        };
        ready(&mut g);
        let cw = g.spawn_unit(Nationality::Commonwealth, "h", "garrison", Composition::Organic { current_toe: 5 });
        g.unit_mut(cw).unwrap().location = Some(UnitLocation::OnMap(city));
        while g.phase != Phase::LandSupportAir {
            g.advance_phase();
        }
        let bomb = |target, mission| Command::FlyBombingMissionAt {
            squadron: SquadronId(1), target, mission, planes: 12, bombload_per_plane: 20,
            night: false, escort: None, abort_planes: 0, flak_destroy_roll: 66, flak_abort_roll: 66, bomb_roll: 66,
        };

        // §41.37: bombing a placed Level-2 fortification knocks it down at most one level per stage.
        match g.apply(bomb(fort, BombingMission::CityFortification)) {
            Ok(CommandOutcome::BombingMission(o)) => {
                let expected = if o.result >= 1 { 1 } else { 2 };
                assert_eq!(g.fort_level(fort), expected);
            }
            other => panic!("unexpected: {other:?}"),
        }
        // §41.31: a unit in a major city (Fortification Level 2) may not be pinned by bombing.
        ready(&mut g);
        assert!(g.apply(bomb(city, BombingMission::Personnel)).is_ok());
        assert!(!g.unit(cw).unwrap().status.pinned);

        // §41.37 (B5): bombing the Major City itself knocks its static §25.12 Fortification Level
        // down (tracked in `city_fort_damage`), even with no placed fort present.
        ready(&mut g);
        let before = g.fort_level(city);
        assert_eq!(before, 2, "a Major City starts at static Level 2");
        match g.apply(bomb(city, BombingMission::CityFortification)) {
            Ok(CommandOutcome::BombingMission(o)) => {
                let expected = if o.result >= 1 { before - 1 } else { before };
                assert_eq!(g.fort_level(city), expected, "§41.37 damaged the static city level");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn air_recon_reveals_enemy_strength_42_27() {
        use crate::reference::aircraft::AircraftClass;
        use crate::reference::squadron::SquadronClass;
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::air::{Sgsu, SquadronId};
        use crate::state::command::{Command, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation, WeaponGroup};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_aircraft_chart(Nationality::Commonwealth, include_str!("../../../data/reference/aircraft/commonwealth.ron")).unwrap();
        refs.load_air_recon(include_str!("../../../data/reference/air-recon.ron")).unwrap();
        refs.load_unit_characteristics(Nationality::Italian, include_str!("../../../data/reference/unit-characteristics/italian.ron")).unwrap();
        refs.load_weapons_systems(Nationality::Italian, include_str!("../../../data/reference/weapons/italian.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        g.phase = Phase::LandSupportAir;
        g.active_player = Side::Commonwealth;
        let base = HexCoord::new(0, 0);
        let target = HexCoord::new(1, 0);
        g.map.set_hex(base, HexTerrain::Clear);
        g.map.set_hex(target, HexTerrain::Clear);

        // A Commonwealth squadron holding two ready recon-capable Lysanders (§42.21).
        let mut sq = Sgsu::new(SquadronId(1), Nationality::Commonwealth, SquadronClass::Bomber, "AC recon", 5, Some(base));
        assert!(sq.add_planes("Lysander Mk.I", AircraftClass::Reconnaissance, 2, 1), "recon planes added");
        for pg in sq.planes.iter_mut() { pg.refitted = pg.count; pg.ready = pg.count; }
        g.sgsus.insert(SquadronId(1), sq);

        // Two Italian units in the target hex.
        let inf = g.spawn_unit(Nationality::Italian, "p", "inf", Composition::Organic { current_toe: 4 });
        let tank = g.spawn_unit(Nationality::Italian, "tank", "tank", Composition::Weapons { groups: vec![WeaponGroup { system: "M 13/40".into(), count: 3 }] });
        for id in [inf, tank] {
            g.unit_mut(id).unwrap().location = Some(UnitLocation::OnMap(target));
        }

        // Before recon, the Commonwealth cannot see enemy TOE strength (§3.5).
        let hidden = |g: &GameState, id| g.observe_units(Side::Commonwealth).into_iter().find(|v| v.id == id).unwrap().strength;
        assert!(hidden(&g, inf).is_none() && hidden(&g, tank).is_none());

        // die 5 with 2 planes (no bonus) → the §42.27 table reveals 2 battalion-equivalents.
        match g.apply(Command::ReconHex { squadron: SquadronId(1), target, planes: 2, die: 5 }) {
            Ok(CommandOutcome::ReconRevealed { revealed }) => assert_eq!(revealed.len(), 2),
            other => panic!("unexpected: {other:?}"),
        }
        // Now the Commonwealth sees both units' strength; the Axis view is unchanged.
        assert!(hidden(&g, inf).is_some() && hidden(&g, tank).is_some(), "§42.27 revealed strength to the flier");
        let axis_sees = g.observe_units(Side::Axis).into_iter().find(|v| v.id == inf).unwrap().strength;
        assert!(axis_sees.is_some(), "the Axis owns the unit, so it always sees its own strength");
    }

    #[test]
    fn malta_commitment_enforcement_44_41() {
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};

        let mut refs = ReferenceData::new();
        refs.load_malta_availability(include_str!("../../../data/reference/malta-availability.ron")).unwrap();
        refs.load_malta_commitment(include_str!("../../../data/reference/malta-commitment.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        g.scenario_name = "Crusader".to_string(); // Level III = Turns(2), Level II = Turns(4)
        g.phase = Phase::MaltaAvailabilityDetermination;

        let avail = |level| Command::DetermineMaltaAvailability { level, roll: 7 };
        // §44.23: Level III may be used at most 2 Game-Turns in Crusader.
        assert!(matches!(g.apply(avail(3)), Ok(CommandOutcome::MaltaAvailability { .. })));
        assert!(matches!(g.apply(avail(3)), Ok(CommandOutcome::MaltaAvailability { .. })));
        assert_eq!(g.apply(avail(3)), Err(CommandError::MaltaLevelUnavailable));
        assert_eq!(*g.malta_level_used.get(&3).unwrap(), 2);
        // §44.25: Level I is unlimited — always usable.
        for _ in 0..5 {
            assert!(g.apply(avail(1)).is_ok());
        }
        // A level marked "na" for the scenario is not allowed at all (Graziani's Level II).
        g.scenario_name = "Graziani's Offensive".to_string();
        assert_eq!(g.apply(avail(2)), Err(CommandError::MaltaLevelUnavailable));
        // Out-of-range level is rejected.
        assert_eq!(g.apply(avail(5)), Err(CommandError::MaltaLevelUnavailable));
    }

    #[test]
    fn squadron_transfer_and_basing_43() {
        use crate::reference::aircraft::AircraftClass;
        use crate::reference::squadron::SquadronClass;
        use crate::state::air::{AirFacility, AirFacilityKind, MedBase, MedBaseDest, Sgsu, SquadronId};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;

        let mut refs = ReferenceData::new();
        refs.load_aircraft_chart(Nationality::German, include_str!("../../../data/reference/aircraft/german.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        let afr = HexCoord::new(5, 5);
        g.air_facilities.push(AirFacility::new(AirFacilityKind::Airfield, afr));
        // Strategic bombers: 4 He.111 in the Italy/Sicily Box, 1 in Africa → 80% Med (§43.11 ok).
        let mut kg1 = Sgsu::new(SquadronId(1), Nationality::German, SquadronClass::Bomber, "KG 1", 5, None);
        kg1.med_base = Some(MedBase::ItalySicily);
        kg1.add_planes("He.111", AircraftClass::Bomber, 4, 1);
        g.sgsus.insert(SquadronId(1), kg1);
        let mut kg2 = Sgsu::new(SquadronId(2), Nationality::German, SquadronClass::Bomber, "KG 2", 5, Some(afr));
        kg2.add_planes("He.111", AircraftClass::Bomber, 1, 1);
        g.sgsus.insert(SquadronId(2), kg2);

        g.phase = Phase::Organization;
        // §43.11: moving the 4-bomber Italy/Sicily squadron to Africa drops Med to 0% → refused, and
        // the squadron is left where it was.
        assert_eq!(
            g.apply(Command::TransferSquadron { squadron: SquadronId(1), to: MedBaseDest::Africa { hex: afr } }),
            Err(CommandError::BasingRequirementViolated)
        );
        assert_eq!(g.sgsus[&SquadronId(1)].med_base, Some(MedBase::ItalySicily));
        assert_eq!(g.sgsus[&SquadronId(1)].transfer_remaining, 0);
        // A legal transfer (Africa → Crete keeps 100% Med) starts one stage of transit (§43.14).
        assert_eq!(
            g.apply(Command::TransferSquadron { squadron: SquadronId(2), to: MedBaseDest::Crete }),
            Ok(CommandOutcome::SquadronTransferred)
        );
        assert_eq!(g.sgsus[&SquadronId(2)].med_base, Some(MedBase::Crete));
        assert!(g.sgsus[&SquadronId(2)].in_transit());
        // An African destination with no air facility is rejected.
        assert_eq!(
            g.apply(Command::TransferSquadron { squadron: SquadronId(1), to: MedBaseDest::Africa { hex: HexCoord::new(9, 9) } }),
            Err(CommandError::NoSuchFacility)
        );

        // §43.14 transit gate: transfer a fighter to Africa, then it may not fly until it arrives.
        let mut jg = Sgsu::new(SquadronId(3), Nationality::German, SquadronClass::Fighter, "JG 27", 5, None);
        jg.med_base = Some(MedBase::ItalySicily);
        jg.add_planes("Bf.109F", AircraftClass::Fighter, 6, 1);
        g.sgsus.insert(SquadronId(3), jg);
        for grp in g.sgsus.get_mut(&SquadronId(3)).unwrap().planes.iter_mut() {
            grp.refitted = grp.count;
            grp.ready = grp.count;
        }
        assert!(g.apply(Command::TransferSquadron { squadron: SquadronId(3), to: MedBaseDest::Africa { hex: afr } }).is_ok());
        g.phase = Phase::LandSupportAir;
        assert_eq!(
            g.apply(Command::DesignateCap { squadron: SquadronId(3), center: afr, cap_type: crate::reference::aircraft::CapType::Offensive, planes: 4 }),
            Err(CommandError::SquadronInTransit)
        );
    }

    #[test]
    fn bomber_escort_45_43_24() {
        use crate::reference::aircraft::{AircraftClass, BombingMission};
        use crate::reference::squadron::SquadronClass;
        use crate::state::air::{MedBase, Sgsu, SquadronId};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::{HexCoord, HexId};

        let mut refs = ReferenceData::new();
        refs.load_aircraft_chart(Nationality::German, include_str!("../../../data/reference/aircraft/german.ron")).unwrap();
        refs.load_air_bombardment(include_str!("../../../data/reference/air-bombardment.ron")).unwrap();
        refs.load_anti_aircraft(include_str!("../../../data/reference/anti-aircraft.ron")).unwrap();
        refs.load_air_to_air(include_str!("../../../data/reference/air-to-air.ron")).unwrap();
        refs.load_air_distance(include_str!("../../../data/reference/air-distance.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        let base = HexCoord::new(1, 1);
        let target = HexCoord::new(3, 1);
        let mut kg = Sgsu::new(SquadronId(1), Nationality::German, SquadronClass::Bomber, "KG 26", 5, Some(base));
        kg.add_planes("Ju.88D", AircraftClass::Bomber, 12, 1);
        g.sgsus.insert(SquadronId(1), kg);
        let mut jg = Sgsu::new(SquadronId(2), Nationality::German, SquadronClass::Fighter, "JG 27", 5, Some(base));
        jg.add_planes("Bf.109F", AircraftClass::Fighter, 6, 1);
        g.sgsus.insert(SquadronId(2), jg);
        for id in [SquadronId(1), SquadronId(2)] {
            for grp in g.sgsus.get_mut(&id).unwrap().planes.iter_mut() {
                grp.refitted = grp.count;
                grp.ready = grp.count;
            }
        }
        while g.phase != Phase::LandSupportAir {
            g.advance_phase();
        }
        let bomb = |escort| Command::FlyBombingMissionAt {
            squadron: SquadronId(1), target, mission: BombingMission::Personnel,
            planes: 12, bombload_per_plane: 20, night: false, escort, abort_planes: 0,
            flak_destroy_roll: 66, flak_abort_roll: 66, bomb_roll: 66,
        };
        // A non-fighter escort (a bomber squadron) is not permitted (§45: fighters escort).
        assert_eq!(g.apply(bomb(Some(SquadronId(1)))), Err(CommandError::EscortNotPermitted));
        // A valid fighter escort flies with the bombers (its ready planes leave for the mission).
        assert!(matches!(g.apply(bomb(Some(SquadronId(2)))), Ok(CommandOutcome::BombingMission(_))));
        assert_eq!(g.sgsus[&SquadronId(2)].ready_planes(), 0, "the escort flew");

        // §43.24: a Crete-based bomber may not take an on-map (African) fighter escort.
        let mut crete = Sgsu::new(SquadronId(3), Nationality::German, SquadronClass::Bomber, "KG 100", 5, None);
        crete.med_base = Some(MedBase::Crete);
        crete.add_planes("Ju.88D", AircraftClass::Bomber, 6, 1);
        g.sgsus.insert(SquadronId(3), crete);
        for grp in g.sgsus.get_mut(&SquadronId(3)).unwrap().planes.iter_mut() {
            grp.refitted = grp.count;
            grp.ready = grp.count;
        }
        for grp in g.sgsus.get_mut(&SquadronId(2)).unwrap().planes.iter_mut() {
            grp.refitted = grp.count;
            grp.ready = grp.count;
        }
        g.game_turn = 2; // out of the §43.23 Suez window
        g.operations_stage = 2;
        assert_eq!(
            g.apply(Command::FlyBombingMissionAt {
                squadron: SquadronId(3), target: HexId::parse("B5925").unwrap().to_hex_coord(),
                mission: BombingMission::Personnel, planes: 6, bombload_per_plane: 20, night: false,
                escort: Some(SquadronId(2)), abort_planes: 0,
                flak_destroy_roll: 66, flak_abort_roll: 66, bomb_roll: 66,
            }),
            Err(CommandError::EscortNotPermitted)
        );
    }

    #[test]
    fn voluntary_abort_39_3() {
        use crate::reference::aircraft::{AircraftClass, BombingMission, CapType};
        use crate::reference::squadron::SquadronClass;
        use crate::state::air::{Sgsu, SquadronId};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;

        let mut refs = ReferenceData::new();
        refs.load_aircraft_chart(Nationality::German, include_str!("../../../data/reference/aircraft/german.ron")).unwrap();
        refs.load_aircraft_chart(Nationality::Commonwealth, include_str!("../../../data/reference/aircraft/commonwealth.ron")).unwrap();
        refs.load_air_bombardment(include_str!("../../../data/reference/air-bombardment.ron")).unwrap();
        refs.load_anti_aircraft(include_str!("../../../data/reference/anti-aircraft.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        let base = HexCoord::new(1, 1);
        let target = HexCoord::new(3, 1);
        // German bombers + a German fighter escort, and a Commonwealth CAP over the target.
        let mut kg = Sgsu::new(SquadronId(1), Nationality::German, SquadronClass::Bomber, "KG 26", 5, Some(base));
        kg.add_planes("Ju.88D", AircraftClass::Bomber, 12, 1);
        g.sgsus.insert(SquadronId(1), kg);
        let mut esc = Sgsu::new(SquadronId(2), Nationality::German, SquadronClass::Fighter, "JG 27", 5, Some(base));
        esc.add_planes("Bf.109F", AircraftClass::Fighter, 6, 1);
        g.sgsus.insert(SquadronId(2), esc);
        let mut cw = Sgsu::new(SquadronId(3), Nationality::Commonwealth, SquadronClass::Fighter, "252 Sqn", 5, Some(target));
        cw.add_planes("Beaufighter Mk.IF", AircraftClass::Fighter, 6, 1);
        g.sgsus.insert(SquadronId(3), cw);
        let ready = |g: &mut GameState, id| {
            for grp in g.sgsus.get_mut(&id).unwrap().planes.iter_mut() {
                grp.refitted = grp.count;
                grp.ready = grp.count;
            }
        };
        for id in [SquadronId(1), SquadronId(2), SquadronId(3)] { ready(&mut g, id); }
        while g.phase != Phase::LandSupportAir {
            g.advance_phase();
        }
        // A Commonwealth offensive CAP of 4 fighters covers the target (§40.23 air ZOC).
        assert!(g.apply(Command::DesignateCap { squadron: SquadronId(3), center: target, cap_type: CapType::Offensive, planes: 4 }).is_ok());

        let abort = |escort| Command::FlyBombingMissionAt {
            squadron: SquadronId(1), target, mission: BombingMission::Personnel,
            planes: 12, bombload_per_plane: 20, night: false, escort, abort_planes: 12,
            flak_destroy_roll: 66, flak_abort_roll: 66, bomb_roll: 66,
        };
        // §39.36: with no escort screen and inferior Maneuver, the bombers may not abort.
        assert_eq!(g.apply(abort(None)), Err(CommandError::AbortNotPermitted));
        // §39.37: a sufficient fighter escort (6 ≥ 25% of 4) lets them abort — the CAP is not engaged.
        match g.apply(abort(Some(SquadronId(2)))) {
            Ok(CommandOutcome::MissionAborted { planes }) => assert_eq!(planes, 12),
            other => panic!("unexpected: {other:?}"),
        }
        assert_eq!(g.cap_patrols[0].intercepts_used, 0, "an aborted mission does not engage CAP");
    }

    #[test]
    fn partial_abort_39_34() {
        // §39.34: the player may abort a subset of the bombers and press on with the rest. The
        // aborting planes turn back (fly + dump); the remainder resolve the mission normally.
        use crate::reference::aircraft::{AircraftClass, BombingMission, CapType};
        use crate::reference::squadron::SquadronClass;
        use crate::state::air::{Sgsu, SquadronId};
        use crate::state::command::{Command, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;

        let mut refs = ReferenceData::new();
        refs.load_aircraft_chart(Nationality::German, include_str!("../../../data/reference/aircraft/german.ron")).unwrap();
        refs.load_aircraft_chart(Nationality::Commonwealth, include_str!("../../../data/reference/aircraft/commonwealth.ron")).unwrap();
        refs.load_air_bombardment(include_str!("../../../data/reference/air-bombardment.ron")).unwrap();
        refs.load_anti_aircraft(include_str!("../../../data/reference/anti-aircraft.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        let base = HexCoord::new(1, 1);
        let target = HexCoord::new(3, 1);
        let mut kg = Sgsu::new(SquadronId(1), Nationality::German, SquadronClass::Bomber, "KG 26", 5, Some(base));
        kg.add_planes("Ju.88D", AircraftClass::Bomber, 12, 1);
        g.sgsus.insert(SquadronId(1), kg);
        let mut esc = Sgsu::new(SquadronId(2), Nationality::German, SquadronClass::Fighter, "JG 27", 5, Some(base));
        esc.add_planes("Bf.109F", AircraftClass::Fighter, 6, 1);
        g.sgsus.insert(SquadronId(2), esc);
        let mut cw = Sgsu::new(SquadronId(3), Nationality::Commonwealth, SquadronClass::Fighter, "252 Sqn", 5, Some(target));
        cw.add_planes("Beaufighter Mk.IF", AircraftClass::Fighter, 6, 1);
        g.sgsus.insert(SquadronId(3), cw);
        for id in [SquadronId(1), SquadronId(2), SquadronId(3)] {
            for grp in g.sgsus.get_mut(&id).unwrap().planes.iter_mut() {
                grp.refitted = grp.count;
                grp.ready = grp.count;
            }
        }
        while g.phase != Phase::LandSupportAir {
            g.advance_phase();
        }
        // A Commonwealth offensive CAP covers the target so a screen is required for the abort.
        assert!(g.apply(Command::DesignateCap { squadron: SquadronId(3), center: target, cap_type: CapType::Offensive, planes: 4 }).is_ok());
        // Abort 4 of 12 (sufficient escort screen); the other 8 press on and resolve.
        match g.apply(Command::FlyBombingMissionAt {
            squadron: SquadronId(1), target, mission: BombingMission::Personnel,
            planes: 12, bombload_per_plane: 20, night: false, escort: Some(SquadronId(2)), abort_planes: 4,
            flak_destroy_roll: 66, flak_abort_roll: 66, bomb_roll: 66,
        }) {
            Ok(CommandOutcome::BombingMission(o)) => assert_eq!(o.flown, 8, "8 bombers pressed on"),
            other => panic!("unexpected: {other:?}"),
        }
        // All 12 left the ready pool (4 turned back + 8 flew the mission).
        assert_eq!(g.sgsus[&SquadronId(1)].ready_planes(), 0);
    }

    #[test]
    fn crete_suez_commitment_43_23() {
        use crate::reference::aircraft::{AircraftClass, BombingMission};
        use crate::reference::squadron::SquadronClass;
        use crate::state::air::{MedBase, Sgsu, SquadronId};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexId;

        let mut refs = ReferenceData::new();
        refs.load_aircraft_chart(Nationality::German, include_str!("../../../data/reference/aircraft/german.ron")).unwrap();
        refs.load_air_bombardment(include_str!("../../../data/reference/air-bombardment.ron")).unwrap();
        refs.load_anti_aircraft(include_str!("../../../data/reference/anti-aircraft.ron")).unwrap();
        refs.load_air_distance(include_str!("../../../data/reference/air-distance.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        let mut kg = Sgsu::new(SquadronId(1), Nationality::German, SquadronClass::Bomber, "KG 26", 5, None);
        kg.med_base = Some(MedBase::Crete);
        kg.add_planes("Ju.88D", AircraftClass::Bomber, 12, 1);
        g.sgsus.insert(SquadronId(1), kg);
        let ready = |g: &mut GameState| {
            for grp in g.sgsus.get_mut(&SquadronId(1)).unwrap().planes.iter_mut() {
                grp.refitted = grp.count;
                grp.ready = grp.count;
            }
        };
        ready(&mut g);
        let derna = HexId::parse("B5925").unwrap().to_hex_coord();
        g.phase = Phase::LandSupportAir;
        let bomb = Command::FlyBombingMissionAt {
            squadron: SquadronId(1), target: derna, mission: BombingMission::Personnel,
            planes: 12, bombload_per_plane: 20, night: false,
            escort: None, abort_planes: 0, flak_destroy_roll: 66, flak_abort_roll: 66, bomb_roll: 66,
        };
        // §43.23: in the first 4 Operations Stages of the month, Crete is committed to Suez.
        g.game_turn = 1;
        g.operations_stage = 1; // op-stage-of-month 1 → in the window
        assert_eq!(g.apply(bomb.clone()), Err(CommandError::CreteSuezCommitment));
        // Later in the month (op-stage-of-month 5) the Crete bombers are free.
        g.game_turn = 2;
        g.operations_stage = 2; // (2-1)%4*3 + 2 = 5 → out of the window
        ready(&mut g);
        assert!(matches!(g.apply(bomb), Ok(CommandOutcome::BombingMission(_))));
    }

    #[test]
    fn box_based_flight_37_4() {
        use crate::reference::aircraft::{AircraftClass, BombingMission};
        use crate::reference::squadron::SquadronClass;
        use crate::state::air::{MedBase, Sgsu, SquadronId};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::{HexCoord, HexId};

        let mut refs = ReferenceData::new();
        refs.load_aircraft_chart(Nationality::German, include_str!("../../../data/reference/aircraft/german.ron")).unwrap();
        refs.load_air_bombardment(include_str!("../../../data/reference/air-bombardment.ron")).unwrap();
        refs.load_anti_aircraft(include_str!("../../../data/reference/anti-aircraft.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        // A Crete-Box-based bomber squadron (no on-map location). Ju.88D range 118.
        let mut kg = Sgsu::new(SquadronId(1), Nationality::German, SquadronClass::Bomber, "KG 26", 5, None);
        kg.med_base = Some(MedBase::Crete);
        kg.add_planes("Ju.88D", AircraftClass::Bomber, 12, 1);
        g.sgsus.insert(SquadronId(1), kg);
        let ready = |g: &mut GameState| {
            for grp in g.sgsus.get_mut(&SquadronId(1)).unwrap().planes.iter_mut() {
                grp.refitted = grp.count;
                grp.ready = grp.count;
            }
        };
        ready(&mut g);
        let derna = HexId::parse("B5925").unwrap().to_hex_coord();
        while g.phase != Phase::LandSupportAir {
            g.advance_phase();
        }
        // Move outside the §43.23 Crete→Suez window so this Crete squadron may bomb.
        g.game_turn = 2;
        g.operations_stage = 2;
        let bomb = |target| Command::FlyBombingMissionAt {
            squadron: SquadronId(1), target, mission: BombingMission::Personnel,
            planes: 12, bombload_per_plane: 20, night: false,
            escort: None, abort_planes: 0, flak_destroy_roll: 66, flak_abort_roll: 66, bomb_roll: 66,
        };
        // With no §37.4 chart loaded, a Box squadron has no computable flight distance.
        assert_eq!(g.apply(bomb(derna)), Err(CommandError::TargetOutOfRange));
        // §37.4: Crete→Derna is 27 hexes (Ju.88D range 118) → the mission flies.
        g.reference_mut().load_air_distance(include_str!("../../../data/reference/air-distance.ron")).unwrap();
        assert_eq!(g.box_flight_distance(MedBase::Crete, derna), Some(27));
        assert!(matches!(g.apply(bomb(derna)), Ok(CommandOutcome::BombingMission(_))));
        // A target far past every reference point exceeds the Ju.88D's 118-hex range.
        ready(&mut g);
        assert_eq!(g.apply(bomb(HexCoord::new(5, 400))), Err(CommandError::TargetOutOfRange));
    }

    #[test]
    fn basing_date_bands_43_11_43_12_43_13() {
        use crate::reference::aircraft::AircraftClass;
        use crate::reference::squadron::SquadronClass;
        use crate::state::air::{MedBase, Sgsu, SquadronId};
        use crate::state::game::GameState;

        let mut refs = ReferenceData::new();
        refs.load_aircraft_chart(Nationality::German, include_str!("../../../data/reference/aircraft/german.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        let mut kg = Sgsu::new(SquadronId(1), Nationality::German, SquadronClass::Bomber, "KG", 5, None);
        kg.med_base = Some(MedBase::ItalySicily);
        kg.add_planes("He.111", AircraftClass::Bomber, 4, 1);
        g.sgsus.insert(SquadronId(1), kg);
        let set = |g: &mut GameState, b: Option<MedBase>| g.sgsus.get_mut(&SquadronId(1)).unwrap().med_base = b;

        // §43.12 (pre-GT35): all German bombers must be ≥75% in Italy/Sicily.
        g.game_turn = 1;
        assert!(g.german_basing_ok()); // 100% Italy/Sicily
        set(&mut g, Some(MedBase::Crete));
        assert!(!g.german_basing_ok()); // Crete satisfies §43.11 but fails §43.12 before GT35

        // §43.13 (from GT35): ≥50% of strategic bombers must be in Crete.
        g.game_turn = 35;
        assert!(g.german_basing_ok()); // 100% Crete
        set(&mut g, Some(MedBase::ItalySicily));
        assert!(!g.german_basing_ok()); // Italy/Sicily fails §43.13 from GT35

        // §43.11 (always): moving out of the Med entirely fails at any date.
        set(&mut g, None);
        g.game_turn = 1;
        assert!(!g.german_basing_ok());
    }

    #[test]
    fn malta_raid_plane_count_43() {
        use crate::reference::aircraft::AircraftClass;
        use crate::reference::squadron::SquadronClass;
        use crate::state::air::{MedBase, Sgsu, SquadronId};
        use crate::state::command::{Command, CommandOutcome};
        use crate::state::game::{GameState, Phase};

        let mut refs = ReferenceData::new();
        refs.load_aircraft_chart(Nationality::Italian, include_str!("../../../data/reference/aircraft/italian.ron")).unwrap();
        refs.load_malta_availability(include_str!("../../../data/reference/malta-availability.ron")).unwrap();
        refs.load_malta_commitment(include_str!("../../../data/reference/malta-commitment.ron")).unwrap();
        let mut g = GameState::new(refs, 1); // scenario defaults to "Campaign Game"
        // 12 Italian bombers based in the Italy/Sicily Box (§43.0).
        let mut it = Sgsu::new(SquadronId(1), Nationality::Italian, SquadronClass::Bomber, "205 Gruppo", 5, None);
        it.med_base = Some(MedBase::ItalySicily);
        it.add_planes("S.M.79", AircraftClass::Bomber, 12, 1);
        g.sgsus.insert(SquadronId(1), it);
        // A Crete-based squadron doesn't feed a Malta raid.
        let mut cr = Sgsu::new(SquadronId(2), Nationality::Italian, SquadronClass::Bomber, "Crete", 5, None);
        cr.med_base = Some(MedBase::Crete);
        cr.add_planes("S.M.79", AircraftClass::Bomber, 6, 1);
        g.sgsus.insert(SquadronId(2), cr);
        assert_eq!(g.med_based_planes(MedBase::ItalySicily), 12);
        assert_eq!(g.med_based_planes(MedBase::Crete), 6);

        g.phase = Phase::MaltaAvailabilityDetermination;
        // §44.25/§43.0: Level III / roll 6 → (50% map, 100% strategic) of the 12 Italy/Sicily
        // bombers = 6 + 12 = 18 planes available.
        match g.apply(Command::DetermineMaltaAvailability { level: 3, roll: 6 }) {
            Ok(CommandOutcome::MaltaAvailability { percentages, planes_available }) => {
                assert_eq!(percentages, Some((50, 100)));
                assert_eq!(planes_available, 18);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn flak_destruction_41_33() {
        use crate::reference::units::Side;
        use crate::state::game::GameState;
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(Nationality::German, include_str!("../../../data/reference/unit-characteristics/german.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        let hex = HexCoord::new(4, 4);
        // German "p" carries an Anti-Air rating (§4.46); as an Organic unit its whole TOE is AA-rated.
        let u = g.spawn_unit(Nationality::German, "p", "flak", Composition::Organic { current_toe: 5 });
        g.unit_mut(u).unwrap().location = Some(UnitLocation::OnMap(hex));

        // §41.33: destroy 3 AA-rated TOE points (capped at the unit's TOE).
        assert_eq!(g.destroy_flak_toe(hex, Side::Axis, 3), 3);
        assert_eq!(g.unit(u).unwrap().total_toe(), 2);
        // A wrong-side call finds no victim.
        assert_eq!(g.destroy_flak_toe(hex, Side::Commonwealth, 3), 0);
    }

    #[test]
    fn scramble_interception_40_3() {
        use crate::reference::aircraft::{AircraftClass, BombingMission};
        use crate::reference::squadron::SquadronClass;
        use crate::state::air::{Sgsu, SquadronId};
        use crate::state::command::{Command, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;

        let build = |cw_base: HexCoord| -> GameState {
            let mut refs = ReferenceData::new();
            refs.load_aircraft_chart(Nationality::German, include_str!("../../../data/reference/aircraft/german.ron")).unwrap();
            refs.load_aircraft_chart(Nationality::Commonwealth, include_str!("../../../data/reference/aircraft/commonwealth.ron")).unwrap();
            refs.load_air_bombardment(include_str!("../../../data/reference/air-bombardment.ron")).unwrap();
            refs.load_anti_aircraft(include_str!("../../../data/reference/anti-aircraft.ron")).unwrap();
            refs.load_air_to_air(include_str!("../../../data/reference/air-to-air.ron")).unwrap();
            refs.load_scramble_table(include_str!("../../../data/reference/scramble-table.ron")).unwrap();
            let mut g = GameState::new(refs, 1);
            // German bombers based adjacent to the target.
            let mut ge = Sgsu::new(SquadronId(1), Nationality::German, SquadronClass::Bomber, "KG 26", 5, Some(HexCoord::new(11, 10)));
            ge.add_planes("Ju.88D", AircraftClass::Bomber, 6, 1);
            g.sgsus.insert(SquadronId(1), ge);
            // Commonwealth scramble-capable fighters (Beaufighter Mk.IF, scramble: Day).
            let mut cw = Sgsu::new(SquadronId(2), Nationality::Commonwealth, SquadronClass::Fighter, "252 Sqn", 5, Some(cw_base));
            cw.add_planes("Beaufighter Mk.IF", AircraftClass::Fighter, 4, 1);
            g.sgsus.insert(SquadronId(2), cw);
            for id in [SquadronId(1), SquadronId(2)] {
                for grp in g.sgsus.get_mut(&id).unwrap().planes.iter_mut() {
                    grp.refitted = grp.count;
                    grp.ready = grp.count;
                }
            }
            while g.phase != Phase::LandSupportAir {
                g.advance_phase();
            }
            g
        };
        let target = HexCoord::new(10, 10);
        let bomb = Command::FlyBombingMissionAt {
            squadron: SquadronId(1), target, mission: BombingMission::Personnel,
            planes: 6, bombload_per_plane: 20, night: false,
            escort: None, abort_planes: 0, flak_destroy_roll: 66, flak_abort_roll: 66, bomb_roll: 66,
        };

        // CW fighters based AT the target (distance 0 → Scramble Table success on any d6, §40.4).
        let mut g = build(target);
        assert!(matches!(g.apply(bomb.clone()), Ok(CommandOutcome::BombingMission(_))));
        assert_eq!(g.sgsus[&SquadronId(2)].ready_planes(), 0, "the fighters scrambled to intercept");

        // CW fighters based far outside Scramble Table range (>15 hexes) cannot scramble.
        let mut g = build(HexCoord::new(60, 10));
        assert!(matches!(g.apply(bomb), Ok(CommandOutcome::BombingMission(_))));
        assert_eq!(g.sgsus[&SquadronId(2)].ready_planes(), 4, "too far to scramble");
    }

    #[test]
    fn scramble_clear_path_40_33() {
        use crate::reference::aircraft::{AircraftClass, BombingMission};
        use crate::reference::squadron::SquadronClass;
        use crate::reference::terrain::HexTerrain;
        use crate::state::air::{Sgsu, SquadronId};
        use crate::state::command::{Command, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;

        // A one-hex-wide corridor: CW fighter base (12,10) — mid (11,10) — target (10,10) — bomber
        // base (13,10). The mid hex is the only route from the fighters to the target.
        let build = |block_mid: bool| -> GameState {
            let mut refs = ReferenceData::new();
            refs.load_aircraft_chart(Nationality::German, include_str!("../../../data/reference/aircraft/german.ron")).unwrap();
            refs.load_aircraft_chart(Nationality::Commonwealth, include_str!("../../../data/reference/aircraft/commonwealth.ron")).unwrap();
            refs.load_air_bombardment(include_str!("../../../data/reference/air-bombardment.ron")).unwrap();
            refs.load_anti_aircraft(include_str!("../../../data/reference/anti-aircraft.ron")).unwrap();
            refs.load_air_to_air(include_str!("../../../data/reference/air-to-air.ron")).unwrap();
            refs.load_scramble_table(include_str!("../../../data/reference/scramble-table.ron")).unwrap();
            let mut g = GameState::new(refs, 1);
            for c in 10..=13 {
                g.map.set_hex(HexCoord::new(c, 10), HexTerrain::Clear);
            }
            // German bombers.
            let mut ge = Sgsu::new(SquadronId(1), Nationality::German, SquadronClass::Bomber, "KG 26", 5, Some(HexCoord::new(13, 10)));
            ge.add_planes("Ju.88D", AircraftClass::Bomber, 6, 1);
            g.sgsus.insert(SquadronId(1), ge);
            // Commonwealth scramble-capable fighters two hexes from the target.
            let mut cw = Sgsu::new(SquadronId(2), Nationality::Commonwealth, SquadronClass::Fighter, "252 Sqn", 5, Some(HexCoord::new(12, 10)));
            cw.add_planes("Beaufighter Mk.IF", AircraftClass::Fighter, 4, 1);
            g.sgsus.insert(SquadronId(2), cw);
            if block_mid {
                // German fighters sitting on the mid hex — the only path to the target (§40.33).
                let mut blk = Sgsu::new(SquadronId(3), Nationality::German, SquadronClass::Fighter, "JG 27", 5, Some(HexCoord::new(11, 10)));
                blk.add_planes("Bf.109E", AircraftClass::Fighter, 4, 1);
                g.sgsus.insert(SquadronId(3), blk);
            }
            for id in [SquadronId(1), SquadronId(2), SquadronId(3)] {
                if let Some(s) = g.sgsus.get_mut(&id) {
                    for grp in s.planes.iter_mut() {
                        grp.refitted = grp.count;
                        grp.ready = grp.count;
                    }
                }
            }
            while g.phase != Phase::LandSupportAir {
                g.advance_phase();
            }
            g
        };
        let target = HexCoord::new(10, 10);
        let bomb = |g: &mut GameState| g.apply(Command::FlyBombingMissionAt {
            squadron: SquadronId(1), target, mission: BombingMission::Personnel,
            planes: 6, bombload_per_plane: 20, night: false,
            escort: None, abort_planes: 0, flak_destroy_roll: 66, flak_abort_roll: 66, bomb_roll: 66,
        });

        // §40.33: enemy fighters blocking the only path prevent the scramble.
        let mut g = build(true);
        assert!(matches!(bomb(&mut g), Ok(CommandOutcome::BombingMission(_))));
        assert_eq!(g.sgsus[&SquadronId(2)].ready_planes(), 4, "no clear path — fighters could not scramble");

        // With the corridor clear, the same fighters scramble to intercept.
        let mut g = build(false);
        assert!(matches!(bomb(&mut g), Ok(CommandOutcome::BombingMission(_))));
        assert_eq!(g.sgsus[&SquadronId(2)].ready_planes(), 0, "clear path — fighters scrambled");
    }

    #[test]
    fn pilot_roster_and_cap_bonus_40() {
        use crate::reference::aircraft::{AircraftClass, CapType};
        use crate::reference::squadron::SquadronClass;
        use crate::state::air::{Pilot, Sgsu, SquadronId};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;

        let mut refs = ReferenceData::new();
        refs.load_aircraft_chart(Nationality::Commonwealth, include_str!("../../../data/reference/aircraft/commonwealth.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        let base = HexCoord::new(10, 10);
        let mut cw = Sgsu::new(SquadronId(1), Nationality::Commonwealth, SquadronClass::Fighter, "252 Sqn", 5, Some(base));
        cw.add_planes("Beaufighter Mk.IF", AircraftClass::Fighter, 6, 1);
        g.sgsus.insert(SquadronId(1), cw);
        for grp in g.sgsus.get_mut(&SquadronId(1)).unwrap().planes.iter_mut() {
            grp.refitted = grp.count;
            grp.ready = grp.count;
        }

        // §40.12 AssignPilot (Organization Phase): a rating-4 pilot; an invalid rating is rejected.
        // §60.42: rated pilots come from the finite pool — seed one rating-4 pilot.
        g.pilot_pool.entry(Nationality::Commonwealth).or_default().insert(4, 1);
        g.phase = Phase::Organization;
        assert_eq!(g.apply(Command::AssignPilot { rating: 4, squadron: SquadronId(1) }), Ok(CommandOutcome::PilotAssigned));
        assert_eq!(g.apply(Command::AssignPilot { rating: 5, squadron: SquadronId(1) }), Err(CommandError::InvalidPilotRating));
        // §40.15/§40.16: the squadron's TacAir pilot bonus is its best flyable pilot Rating.
        assert_eq!(g.squadron_pilot_bonus(SquadronId(1)), 4);

        // A higher-rated pilot who is still grounded (§40.13/14/17) does not yet count.
        let mut grounded = Pilot::new(6, Some(SquadronId(1)));
        grounded.grounded_stages = 1;
        g.pilots.push(grounded);
        assert_eq!(g.squadron_pilot_bonus(SquadronId(1)), 4);

        // §40.15: the pilot Rating flows into the CAP flight's TacAir.
        g.phase = Phase::LandSupportAir;
        assert_eq!(
            g.apply(Command::DesignateCap { squadron: SquadronId(1), center: base, cap_type: CapType::Offensive, planes: 4 }),
            Ok(CommandOutcome::CapDesignated { fighters: 4 })
        );
        assert_eq!(g.cap_patrols[0].flight.pilot, 4);

        // The grounded rating-6 pilot becomes flyable after one Operations Stage (§40.13).
        g.begin_operations_stage();
        assert_eq!(g.squadron_pilot_bonus(SquadronId(1)), 6);
    }

    #[test]
    fn ace_progression_40_18() {
        use crate::state::air::{Pilot, SquadronId};
        use crate::state::game::GameState;

        let mut g = GameState::new(ReferenceData::new(), 1);
        let sq = SquadronId(1);
        // A zero-rated pilot earns one Rating per five kills (§40.18).
        g.pilots.push(Pilot::new(0, Some(sq)));
        assert_eq!(g.squadron_pilot_bonus(sq), 0);
        g.credit_pilot_kills(sq, 5);
        assert_eq!(g.squadron_pilot_bonus(sq), 1); // 5 kills → ace Rating 1
        g.credit_pilot_kills(sq, 5);
        assert_eq!(g.squadron_pilot_bonus(sq), 2); // 10 kills → 2

        // A pilot's assigned Rating floors his effective Rating; kills credit the best flyer.
        g.pilots.push(Pilot::new(4, Some(sq)));
        assert_eq!(g.squadron_pilot_bonus(sq), 4); // best of {ace 2, assigned 4}
        g.credit_pilot_kills(sq, 25); // goes to the rating-4 pilot → 25 kills → ace 5 > 4
        assert_eq!(g.squadron_pilot_bonus(sq), 5);
    }

    #[test]
    fn sgsu_upkeep_and_supply_35_14() {
        use crate::reference::squadron::SquadronClass;
        use crate::state::air::{Sgsu, SquadronId};
        use crate::state::command::{Command, CommandError};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::reference::units::Side;
        use crate::state::unit::UnitLocation;

        let refs = ReferenceData::new();
        let mut g = GameState::new(refs, 1);
        let supplied_hex = HexCoord::new(1, 1);
        let starved_hex = HexCoord::new(2, 2);
        g.sgsus.insert(SquadronId(1), Sgsu::new(SquadronId(1), Nationality::German, SquadronClass::Fighter, "JG 27", 5, Some(supplied_hex)));
        g.sgsus.insert(SquadronId(2), Sgsu::new(SquadronId(2), Nationality::German, SquadronClass::Fighter, "JG 53", 5, Some(starved_hex)));
        // A stocked dump at squadron 1's base; an empty dump at squadron 2's.
        g.add_supply_dump(UnitLocation::OnMap(supplied_hex), Side::Axis, 0, 5);
        { let d = g.supply_dumps.last_mut().unwrap(); d.supply.stores = 5; d.supply.water = 5; }
        g.add_supply_dump(UnitLocation::OnMap(starved_hex), Side::Axis, 0, 0);

        // Advance into Operations Stage 1 (begin_operations_stage runs SGSU upkeep, §35.14).
        while g.phase != Phase::Weather {
            g.advance_phase();
        }
        // Squadron 1 paid upkeep (1 Fuel + 1 Water + 1 Stores) and stays supplied.
        assert!(g.sgsus[&SquadronId(1)].supplied);
        let d = g.supply_dumps.iter().find(|d| d.location.hex() == Some(supplied_hex)).unwrap();
        assert_eq!((d.supply.fuel, d.supply.water, d.supply.stores), (4, 4, 4));
        // Squadron 2 could not pay -> unsupplied.
        assert!(!g.sgsus[&SquadronId(2)].supplied);

        // §35.14: the unsupplied squadron may not refit.
        while g.phase != Phase::LandSupportAir {
            g.advance_phase();
        }
        assert_eq!(
            g.apply(Command::RefitSquadron { squadron: SquadronId(2), base_die: 1, wrong_squadron: false }),
            Err(CommandError::SgsuUnsupplied)
        );
    }

    #[test]
    fn refit_requires_stores_38_36() {
        use crate::reference::aircraft::AircraftClass;
        use crate::reference::squadron::SquadronClass;
        use crate::state::air::{Sgsu, SquadronId};
        use crate::state::command::{Command, CommandError};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;

        let refs = ReferenceData::new();
        let mut g = GameState::new(refs, 1);
        g.reference_mut().load_aircraft_refit(include_str!("../../../data/reference/aircraft-refit.ron")).unwrap();
        let mut sq = Sgsu::new(SquadronId(1), Nationality::German, SquadronClass::Fighter, "JG 27", 5, Some(HexCoord::new(2, 2)));
        sq.add_planes("Bf.109F", AircraftClass::Fighter, 12, 1);
        g.sgsus.insert(SquadronId(1), sq);

        while g.phase != Phase::LandSupportAir {
            g.advance_phase();
        }
        // No Stores at the base -> refit rejected (§38.36).
        assert_eq!(
            g.apply(Command::RefitSquadron { squadron: SquadronId(1), base_die: 1, wrong_squadron: false }),
            Err(CommandError::InsufficientStores)
        );
        // Provide a Stores Point; the refit now runs and consumes it.
        g.add_supply_dump(crate::state::unit::UnitLocation::OnMap(HexCoord::new(2, 2)), crate::reference::units::Side::Axis, 0, 0);
        g.supply_dumps.last_mut().unwrap().supply.stores = 1;
        assert!(g.apply(Command::RefitSquadron { squadron: SquadronId(1), base_die: 1, wrong_squadron: false }).is_ok());
        assert_eq!(g.supply_dumps.last().unwrap().supply.stores, 0);
    }

    #[test]
    fn readying_and_initiative_commands_through_apply() {
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::unit::Composition;

        let refs = ReferenceData::new();
        let mut g = GameState::new(refs, 1);
        let u = g.spawn_unit(Nationality::Commonwealth, "h", "bn", Composition::Organic { current_toe: 5 });

        // DeclareInitiativeOrder is a Weather-Phase action.
        let decl = Command::DeclareInitiativeOrder { initiative_moves_first: false };
        assert_eq!(g.apply(decl.clone()), Err(CommandError::WrongPhase));
        while g.phase != Phase::Weather {
            g.advance_phase();
        }
        assert_eq!(g.apply(decl), Ok(CommandOutcome::InitiativeDeclared));

        // RefuelUnit is an Organization-Phase action (a fuel-less infantry unit -> 0).
        while g.phase != Phase::Organization {
            g.advance_phase();
        }
        assert_eq!(g.apply(Command::RefuelUnit { unit: u }), Ok(CommandOutcome::UnitRefueled(0)));
    }

    #[test]
    fn withdrawal_command_through_apply_64_75() {
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexId;
        use crate::state::unit::{Composition, OrgSize, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_campaign_victory(include_str!("../../../data/reference/campaign-victory.ron"))
            .unwrap();
        let mut g = GameState::new(refs, 1);
        let cairo = HexId::parse("E1430").unwrap().to_hex_coord();
        let bn = g.spawn_unit(Nationality::Commonwealth, "h", "bn", Composition::Organic { current_toe: 5 });
        g.unit_mut(bn).unwrap().org_size = OrgSize::Battalion;
        g.unit_mut(bn).unwrap().location = Some(UnitLocation::OnMap(cairo));
        g.game_turn = 10;

        // Withdrawal is an Organization-Phase action.
        let cmd = Command::WithdrawUnit { unit: bn };
        assert_eq!(g.apply(cmd.clone()), Err(CommandError::WrongPhase));
        while g.phase != Phase::Organization {
            g.advance_phase();
        }
        assert_eq!(g.apply(cmd), Ok(CommandOutcome::UnitWithdrawn));
        assert_eq!(g.unit(bn).unwrap().hex(), None);
    }

    #[test]
    fn land_action_commands_through_apply() {
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{Commodity, GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let refs = ReferenceData::new();
        let mut g = GameState::new(refs, 1);
        let u = g.spawn_unit(Nationality::Commonwealth, "h", "bn", Composition::Organic { current_toe: 5 });
        let hex = HexCoord::new(3, 3);
        g.unit_mut(u).unwrap().location = Some(UnitLocation::OnMap(hex));

        // LoadTrucks is an Organization-Phase action.
        let load = Command::LoadTrucks { unit: u, commodity: Commodity::Fuel, amount: 100 };
        assert_eq!(g.apply(load.clone()), Err(CommandError::WrongPhase));
        while g.phase != Phase::Organization {
            g.advance_phase();
        }
        // No trucks/dump -> nothing loaded, but the command path succeeds.
        assert_eq!(g.apply(load), Ok(CommandOutcome::TrucksLoaded(0)));
        // A minefield needs an eligible engineer -> construction fails here.
        assert_eq!(
            g.apply(Command::LayMinefield { builder: u, hex, dummy: false }),
            Err(CommandError::ConstructionFailed)
        );

        // PoisonWell is a Movement & Combat action; no Village/Bir well here -> false.
        while g.phase != Phase::MovementAndCombat {
            g.advance_phase();
        }
        assert_eq!(
            g.apply(Command::PoisonWell { hex, die: 1 }),
            Ok(CommandOutcome::WellPoisoned(false))
        );
        // RailMove is a Commonwealth Rail Phase action; wrong here (Movement & Combat).
        assert_eq!(
            g.apply(Command::RailMove { unit: u, to: HexCoord::new(4, 3) }),
            Err(CommandError::WrongPhase)
        );
        // In the Commonwealth Rail Phase, a move with no rail connection fails cleanly.
        while g.phase != Phase::CommonwealthRail {
            g.advance_phase();
        }
        assert_eq!(
            g.apply(Command::RailMove { unit: u, to: HexCoord::new(4, 3) }),
            Err(CommandError::RailMoveFailed)
        );
    }

    #[test]
    fn strategic_air_and_recon_commands_through_apply() {
        use crate::reference::squadron::SquadronClass;
        use crate::state::air::{Sgsu, SquadronId};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;

        let mut refs = ReferenceData::new();
        refs.load_naval_recon(include_str!("../../../data/reference/naval-recon.ron")).unwrap();
        refs.load_malta_availability(include_str!("../../../data/reference/malta-availability.ron"))
            .unwrap();
        refs.load_aircraft_refit(include_str!("../../../data/reference/aircraft-refit.ron")).unwrap();
        refs.load_convoy_levels(include_str!("../../../data/reference/convoy-levels.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        g.sgsus.insert(
            SquadronId(1),
            Sgsu::new(SquadronId(1), Nationality::German, SquadronClass::Fighter, "JG 27", 5, Some(HexCoord::new(1, 1))),
        );
        // §38.36 Stores + §35.14 SGSU upkeep (Fuel/Water) present at the base.
        g.add_supply_dump(crate::state::unit::UnitLocation::OnMap(HexCoord::new(1, 1)), crate::reference::units::Side::Axis, 0, 5);
        {
            let d = g.supply_dumps.last_mut().unwrap();
            d.supply.stores = 5;
            d.supply.water = 5;
        }

        // Malta availability (Level III, roll 6 -> 50/100) in its phase.
        while g.phase != Phase::MaltaAvailabilityDetermination {
            g.advance_phase();
        }
        assert_eq!(
            g.apply(Command::DetermineMaltaAvailability { level: 3, roll: 6 }),
            // No Italy/Sicily-based bombers here, so the absolute count is 0.
            Ok(CommandOutcome::MaltaAvailability { percentages: Some((50, 100)), planes_available: 0 })
        );
        // Wrong phase for recon here.
        assert_eq!(
            g.apply(Command::ReconConvoyLane { lane: 2, planes: 1, roll: 12 }),
            Err(CommandError::WrongPhase)
        );

        // Plan a convoy (Schedule), then recon it (Resolution).
        while g.phase != Phase::NavalConvoySchedule {
            g.advance_phase();
        }
        // A convoy arriving THIS game-turn (arrival_gt = current) so recon can see it.
        let gt = g.game_turn;
        g.plan_convoy(2, "Tripoli", gt, 5000, 0, 0, 0, 21000);
        while g.phase != Phase::ConvoyResolution {
            g.advance_phase();
        }
        // 1 plane reveals on 2d6 <= 12; a convoy of 5000 t is present. §42.53: the Axis reveals only
        // a size class, not the tonnage — 5000 t is legally Small or Medium, so the adversarial Axis
        // reports the more concealing Medium.
        assert_eq!(
            g.apply(Command::ReconConvoyLane { lane: 2, planes: 1, roll: 12 }),
            Ok(CommandOutcome::ConvoyRecon { revealed: true, size: Some(crate::rules::naval::ConvoySize::Medium) })
        );
        // Roll 13 fails to reveal.
        assert_eq!(
            g.apply(Command::ReconConvoyLane { lane: 2, planes: 1, roll: 13 }),
            Ok(CommandOutcome::ConvoyRecon { revealed: false, size: None })
        );

        // Refit the squadron in the Land Support Air Phase. German +1 (§38.35): 12 planes
        // undergoing refit, roll 1+1=2 -> 80% -> ceil(12*0.8) = 10 refitted.
        g.sgsus.get_mut(&SquadronId(1)).unwrap().add_planes("Ju.88c", crate::reference::aircraft::AircraftClass::Fighter, 12, g.game_turn);
        while g.phase != Phase::LandSupportAir {
            g.advance_phase();
        }
        assert_eq!(
            g.apply(Command::RefitSquadron { squadron: SquadronId(1), base_die: 1, wrong_squadron: false }),
            Ok(CommandOutcome::SquadronRefitted(10))
        );
    }

    #[test]
    fn air_and_harbor_commands_through_apply() {
        use crate::reference::aircraft::AircraftClass;
        use crate::reference::ports::PortsChart;
        use crate::state::air::{AirFacility, AirFacilityKind, Sgsu, SquadronId};
        use crate::reference::squadron::SquadronClass;
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::naval::Port;

        let refs = ReferenceData::new();
        let mut g = GameState::new(refs, 1);
        let h = HexCoord::new(5, 5);
        g.sgsus.insert(
            SquadronId(1),
            Sgsu::new(SquadronId(1), Nationality::German, SquadronClass::Fighter, "JG 27", 5, Some(h)),
        );
        g.add_air_facility(AirFacility::new(AirFacilityKind::Airfield, h));
        // §35.2: assignment draws from the nationality's plane pool — seed it.
        let pool = g.plane_pool.entry(Nationality::German).or_default();
        pool.insert("Bf.109F".into(), 12);
        pool.insert("Ju.87D".into(), 4);
        let chart: PortsChart =
            ron::from_str(include_str!("../../../data/reference/port-capacity.ron")).unwrap();
        g.ports.insert("Benghazi".into(), Port::from_spec("Benghazi", &chart.spec("Benghazi")));

        // AssignSquadronPlanes only in the Organization Phase.
        let assign = Command::AssignSquadronPlanes {
            squadron: SquadronId(1), aircraft_type: "Bf.109F".into(), class: AircraftClass::Fighter, count: 12,
        };
        assert_eq!(g.apply(assign.clone()), Err(CommandError::WrongPhase));
        while g.phase != Phase::Organization {
            g.advance_phase();
        }
        assert_eq!(g.apply(assign), Ok(CommandOutcome::SquadronPlanesAssigned));
        assert_eq!(g.sgsus[&SquadronId(1)].plane_count(), 12);
        // A bomber into the fighter squadron is rejected (§35.21).
        assert_eq!(
            g.apply(Command::AssignSquadronPlanes { squadron: SquadronId(1), aircraft_type: "Ju.87D".into(), class: AircraftClass::DiveBomber, count: 1 }),
            Err(CommandError::SquadronRejected)
        );

        // Bomb the airfield + lay a harbour mine in the Land Support Air Phase.
        while g.phase != Phase::LandSupportAir {
            g.advance_phase();
        }
        assert_eq!(
            g.apply(Command::BombAirFacility { hex: h, kind: AirFacilityKind::Airfield, levels: 2 }),
            Ok(CommandOutcome::AirFacilityBombed)
        );
        assert_eq!(g.facilities_at(h).next().unwrap().capacity_level, 4);
        assert_eq!(g.apply(Command::LayHarborMine { port: "Benghazi".into() }), Ok(CommandOutcome::HarborMineLaid));
        assert_eq!(g.ports["Benghazi"].mines, 1);

        // Clear the mine in the Repair Phase (die 2 succeeds).
        while g.phase != Phase::Repair {
            g.advance_phase();
        }
        assert_eq!(
            g.apply(Command::RemoveHarborMine { port: "Benghazi".into(), die: 2 }),
            Ok(CommandOutcome::HarborMineRemoved(true))
        );
        assert_eq!(g.ports["Benghazi"].mines, 0);
    }

    #[test]
    fn assigning_squadron_planes_draws_from_pool_35_2() {
        use crate::reference::aircraft::AircraftClass;
        use crate::reference::squadron::SquadronClass;
        use crate::state::air::{Sgsu, SquadronId};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;

        let mut g = GameState::new(ReferenceData::new(), 1);
        g.sgsus.insert(
            SquadronId(1),
            Sgsu::new(SquadronId(1), Nationality::Italian, SquadronClass::Fighter, "1 Stormo", 5, Some(HexCoord::new(5, 5))),
        );
        // Seed a §60.32-style pool of 9 CR.42s.
        g.plane_pool.entry(Nationality::Italian).or_default().insert("C.R.42 Falco".into(), 9);
        while g.phase != Phase::Organization {
            g.advance_phase();
        }
        let assign6 = Command::AssignSquadronPlanes {
            squadron: SquadronId(1), aircraft_type: "C.R.42 Falco".into(), class: AircraftClass::Fighter, count: 6,
        };
        // First assignment succeeds and deducts from the pool (9 -> 3).
        assert_eq!(g.apply(assign6.clone()), Ok(CommandOutcome::SquadronPlanesAssigned));
        assert_eq!(g.pool_planes(Nationality::Italian, "C.R.42 Falco"), 3);
        assert_eq!(g.sgsus[&SquadronId(1)].plane_count(), 6);
        // A second draw of 6 exceeds the remaining pool of 3 — rejected, pool untouched.
        assert_eq!(g.apply(assign6), Err(CommandError::InsufficientPlanePool));
        assert_eq!(g.pool_planes(Nationality::Italian, "C.R.42 Falco"), 3);
        assert_eq!(g.sgsus[&SquadronId(1)].plane_count(), 6);
        // The remaining 3 can be drawn, emptying the pool.
        assert_eq!(
            g.apply(Command::AssignSquadronPlanes { squadron: SquadronId(1), aircraft_type: "C.R.42 Falco".into(), class: AircraftClass::Fighter, count: 3 }),
            Ok(CommandOutcome::SquadronPlanesAssigned)
        );
        assert_eq!(g.pool_planes(Nationality::Italian, "C.R.42 Falco"), 0);
        assert_eq!(g.sgsus[&SquadronId(1)].plane_count(), 9);
    }

    #[test]
    fn assigning_pilots_draws_from_pool_40_12() {
        use crate::reference::squadron::SquadronClass;
        use crate::state::air::{Sgsu, SquadronId};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;

        let mut g = GameState::new(ReferenceData::new(), 1);
        g.sgsus.insert(
            SquadronId(1),
            Sgsu::new(SquadronId(1), Nationality::Commonwealth, SquadronClass::Fighter, "33 Sqn", 5, Some(HexCoord::new(5, 5))),
        );
        // §60.42-style pool: a single rating-3 pilot.
        g.pilot_pool.entry(Nationality::Commonwealth).or_default().insert(3, 1);
        while g.phase != Phase::Organization {
            g.advance_phase();
        }
        // First rating-3 assignment succeeds and empties that rating.
        assert_eq!(g.apply(Command::AssignPilot { rating: 3, squadron: SquadronId(1) }), Ok(CommandOutcome::PilotAssigned));
        assert_eq!(g.pool_pilots(Nationality::Commonwealth, 3), 0);
        // A second rating-3 is refused — the pool is empty.
        assert_eq!(g.apply(Command::AssignPilot { rating: 3, squadron: SquadronId(1) }), Err(CommandError::InsufficientPilotPool));
        // Rating-0 pilots are unlimited (§40.16) — always assignable, pool untouched.
        assert_eq!(g.apply(Command::AssignPilot { rating: 0, squadron: SquadronId(1) }), Ok(CommandOutcome::PilotAssigned));
        assert_eq!(g.pilots.iter().filter(|p| p.rating == 3).count(), 1);
    }

    #[test]
    fn convoy_commands_through_apply_56() {
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        let mut refs = ReferenceData::new();
        refs.load_convoy_levels(include_str!("../../../data/reference/convoy-levels.ron"))
            .unwrap();
        refs.load_air_bombardment(include_str!("../../../data/reference/air-bombardment.ron"))
            .unwrap();
        refs.load_equivalent_weights(include_str!(
            "../../../data/reference/equivalent-weights.ron"
        ))
        .unwrap();
        let mut g = GameState::new(refs, 1);

        let plan = Command::PlanConvoy {
            lane: 2, destination: "Tripoli".into(), arrival_gt: 55,
            fuel_tons: 10000, ammo_tons: 8000, stores_tons: 1000, replacement_tons: 0, die: 4,
        };
        // Wrong phase (still Initiative Determination) is rejected.
        assert_eq!(g.apply(plan.clone()), Err(CommandError::WrongPhase));

        // Advance to the Naval Convoy Schedule Phase, then plan (allowable = 21,000 for GT55/E/4).
        while g.phase != Phase::NavalConvoySchedule {
            g.advance_phase();
        }
        assert_eq!(g.apply(plan), Ok(CommandOutcome::ConvoyPlanned));
        assert_eq!(g.convoys.len(), 1);

        // Bomb the convoy in the Convoy Resolution Phase (250 pts, roll 66 -> 25%).
        while g.phase != Phase::ConvoyResolution {
            g.advance_phase();
        }
        assert_eq!(
            g.apply(Command::BombConvoy { convoy_index: 0, bomb_points: 250, torpedo_planes: 0, total_planes: 10, roll: 66 }),
            Ok(CommandOutcome::ConvoyBombed(25))
        );
        assert_eq!(g.convoys[0].fuel_tons, 7500);

        // Deliver at the Naval Convoy Arrival Phase.
        while g.phase != Phase::NavalConvoyArrival {
            g.advance_phase();
        }
        match g.apply(Command::DeliverConvoy { convoy_index: 0 }) {
            Ok(CommandOutcome::ConvoyDelivered(c)) => {
                assert_eq!(c.fuel, 60000); // 7500 t * 8
            }
            other => panic!("unexpected: {other:?}"),
        }
        assert!(g.convoys.is_empty());
    }

    #[test]
    fn commonwealth_withdrawal_points() {
        use crate::state::game::GameState;
        use crate::state::map::HexId;
        use crate::state::unit::{Composition, OrgSize, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_campaign_victory(include_str!("../../../data/reference/campaign-victory.ron"))
            .unwrap();
        let mut g = GameState::new(refs, 1);
        let cairo = HexId::parse("E1430").unwrap().to_hex_coord();
        // "h" infantry battalion, max TOE 5, at full strength in Cairo.
        let bn = g.spawn_unit(Nationality::Commonwealth, "h", "bn", Composition::Organic { current_toe: 5 });
        g.unit_mut(bn).unwrap().org_size = OrgSize::Battalion;
        g.unit_mut(bn).unwrap().location = Some(UnitLocation::OnMap(cairo));
        g.game_turn = 10;

        // §64.75 A: eligible and withdrawn — leaves play.
        assert!(g.withdrawal_eligible(bn));
        assert!(g.withdraw_unit(bn));
        assert_eq!(g.unit(bn).unwrap().hex(), None);
        // 1 VP per week gone …
        g.game_turn = 12;
        assert_eq!(g.withdrawal_vp(), 2);
        // … capped at 3.
        g.game_turn = 15;
        assert_eq!(g.withdrawal_vp(), 3);
        // §64.75 B: returning banks (3 earned − 2 penalty) = 1, and locks re-withdrawal.
        assert!(g.return_unit(bn, cairo));
        assert_eq!(g.withdrawal_vp(), 1);
        assert_eq!(g.unit(bn).unwrap().hex(), Some(cairo));
        assert!(!g.withdrawal_eligible(bn)); // six-month lockout

        // Ineligible: a company (not a battalion), and a battalion below 75% TOE.
        let coy = g.spawn_unit(Nationality::Commonwealth, "h", "coy", Composition::Organic { current_toe: 5 });
        g.unit_mut(coy).unwrap().org_size = OrgSize::Company;
        g.unit_mut(coy).unwrap().location = Some(UnitLocation::OnMap(cairo));
        assert!(!g.withdrawal_eligible(coy));
        let weak = g.spawn_unit(Nationality::Commonwealth, "h", "weak", Composition::Organic { current_toe: 3 });
        g.unit_mut(weak).unwrap().org_size = OrgSize::Battalion;
        g.unit_mut(weak).unwrap().location = Some(UnitLocation::OnMap(cairo));
        assert!(!g.withdrawal_eligible(weak)); // 3/5 = 60% < 75%
    }

    #[test]
    fn axis_truck_production_schedule() {
        use crate::reference::logistics::TruckKind;

        let mut refs = ReferenceData::new();
        refs.load_axis_truck_production(include_str!(
            "../../../data/reference/axis-truck-production.ron"
        ))
        .unwrap();
        let prod = refs.axis_truck_production.clone().unwrap();
        // §20.66a phased maxima and totals/tonnage.
        assert_eq!(prod.row(TruckKind::Light).max_per_turn(3), 0); // before GT 4
        assert_eq!(prod.row(TruckKind::Light).max_per_turn(4), 4);
        assert_eq!(prod.row(TruckKind::Light).max_per_turn(12), 4);
        assert_eq!(prod.row(TruckKind::Light).max_per_turn(13), 15);
        assert_eq!((prod.row(TruckKind::Light).total, prod.row(TruckKind::Light).tonnage), (835, 45));
        assert_eq!(prod.row(TruckKind::Medium).max_per_turn(6), 20);
        assert_eq!(prod.row(TruckKind::Medium).max_per_turn(13), 50);
        assert_eq!(prod.row(TruckKind::Heavy).max_per_turn(4), 0);
        assert_eq!(prod.row(TruckKind::Heavy).max_per_turn(5), 3);
        assert_eq!(prod.row(TruckKind::Heavy).max_per_turn(13), 8);
        assert_eq!(prod.row(TruckKind::Heavy).tonnage, 150);
    }

    #[test]
    fn german_production_chart() {
        use crate::reference::logistics::ProductionPeriod;

        let mut refs = ReferenceData::new();
        refs.load_production_chart(
            Nationality::German,
            include_str!("../../../data/reference/german-production.ron"),
        )
        .unwrap();
        let chart = refs.production_charts.get(&Nationality::German).unwrap();
        assert_eq!(chart.items.len(), 23);
        // §20.66: Infantry — 400 total, 12/turn from GT38, 30 tons.
        let inf = chart.item("Infantry").unwrap();
        assert_eq!((inf.total, inf.max, inf.first_gt, inf.tonnage), (400, 12, 38, 30));
        assert_eq!(inf.period, ProductionPeriod::GameTurn);
        assert_eq!(chart.available("Infantry", 37), 0); // before its date
        assert_eq!(chart.available("Infantry", 38), 12);
        // PzIII E — 38 total, 5/turn from GT53, 190 tons.
        let pz = chart.item("PzIII E").unwrap();
        assert_eq!((pz.total, pz.max, pz.first_gt, pz.tonnage), (38, 5, 53, 190));
        // Note markers: Heavy AA is per-Month (†), Armed Recce per-Two-Weeks (*).
        assert_eq!(chart.item("Heavy AA").unwrap().period, ProductionPeriod::Month);
        assert_eq!(chart.item("Armed Recce").unwrap().period, ProductionPeriod::TwoWeeks);
    }

    #[test]
    fn replacement_class_classification_20_2() {
        use crate::state::game::{replacement_class_for_item, ReplacementClass};
        // Recce / Infantry / Gun (artillery, AA, anti-tank, pounders) / Tank (incl. inferred SP).
        assert_eq!(replacement_class_for_item("Infantry"), Some(ReplacementClass::Infantry));
        assert_eq!(replacement_class_for_item("Armed Recce"), Some(ReplacementClass::Recce));
        assert_eq!(
            replacement_class_for_item("Armored Reconnaissance"),
            Some(ReplacementClass::Recce)
        );
        assert_eq!(replacement_class_for_item("Heavy AA"), Some(ReplacementClass::Gun));
        assert_eq!(replacement_class_for_item("25-pounders"), Some(ReplacementClass::Gun));
        assert_eq!(replacement_class_for_item("5cm Pak38"), Some(ReplacementClass::Gun));
        assert_eq!(replacement_class_for_item("ParaArt"), Some(ReplacementClass::Gun));
        assert_eq!(replacement_class_for_item("PzIV F2 Special"), Some(ReplacementClass::Tank));
        assert_eq!(replacement_class_for_item("CA M 13/40"), Some(ReplacementClass::Tank));
        assert_eq!(replacement_class_for_item("Crusader Mk III"), Some(ReplacementClass::Tank));
        assert_eq!(replacement_class_for_item("Matilda"), Some(ReplacementClass::Tank));
        // SP-gun / tankette calls (owner-confirmed 2026-08-29): tracked AFVs take Tank points.
        assert_eq!(replacement_class_for_item("Marder III"), Some(ReplacementClass::Tank));
        assert_eq!(replacement_class_for_item("SP 6-pounders"), Some(ReplacementClass::Tank));
        assert_eq!(
            replacement_class_for_item("Semovente 75/18 SP Gun"),
            Some(ReplacementClass::Tank)
        );
        assert_eq!(replacement_class_for_item("CV L.3 or CV 33/35"), Some(ReplacementClass::Tank));
        // A plain anti-tank rifle (starts with a calibre, not "Pz") stays a Gun.
        assert_eq!(replacement_class_for_item("2.8cm s.Pz.B.41"), Some(ReplacementClass::Gun));
    }

    #[test]
    fn plan_production_validates_and_schedules_20_66() {
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase, ReplacementClass};

        let mut refs = ReferenceData::new();
        refs.load_production_chart(
            Nationality::German,
            include_str!("../../../data/reference/german-production.ron"),
        )
        .unwrap();
        // §20.66 German Infantry: 12/turn (GameTurn period) from GT38, total 400, 30 tons/point.
        let mut g = GameState::new(refs, 1);
        g.game_turn = 40;

        let plan = |item: &str, count: u32| Command::PlanProduction {
            nationality: Nationality::German,
            item: item.into(),
            count,
        };

        // Wrong phase (still InitiativeDetermination).
        assert_eq!(g.apply(plan("Infantry", 5)), Err(CommandError::WrongPhase));
        g.phase = Phase::Organization;

        // Unknown item.
        assert_eq!(g.apply(plan("Panther", 1)), Err(CommandError::NoSuchProductionItem));
        // Item not yet available on this Game-Turn (PzIII E starts GT53) → rate error.
        assert_eq!(g.apply(plan("PzIII E", 1)), Err(CommandError::ProductionRateExceeded));

        // A valid order: 10 Infantry points. Arrives GT42 (Axis +2), 300 tons, Infantry class.
        assert_eq!(
            g.apply(plan("Infantry", 10)),
            Ok(CommandOutcome::ProductionPlanned {
                arrival_game_turn: 42,
                class: ReplacementClass::Infantry,
                tonnage: 300,
            })
        );
        assert_eq!(g.pending_replacements.len(), 1);
        assert_eq!(g.production_orders.len(), 1);

        // §20.66 per-Game-Turn cap is 12: 10 already planned this turn, so 3 more is too many...
        assert_eq!(g.apply(plan("Infantry", 3)), Err(CommandError::ProductionRateExceeded));
        // ...but 2 more fits (10 + 2 = 12).
        assert!(g.apply(plan("Infantry", 2)).is_ok());

        // Next Game-Turn the per-turn window resets (12 available again), but the lifetime total
        // (400) is enforced across the whole game.
        g.game_turn = 41;
        assert_eq!(g.apply(plan("Infantry", 12)), Ok(CommandOutcome::ProductionPlanned {
            arrival_game_turn: 43,
            class: ReplacementClass::Infantry,
            tonnage: 360,
        }));
        // Drive the arrival: at GT42 the first planned pool enters the Replacement Point pool and
        // begins Training (Infantry = 3 Operations Stages, §20.43).
        g.game_turn = 42;
        g.process_replacement_arrivals();
        let pool = g
            .replacement_points
            .iter()
            .find(|rp| rp.class == ReplacementClass::Infantry && rp.count == 10)
            .expect("the GT40 order should have arrived");
        assert_eq!(pool.training_remaining, 3);
    }

    #[test]
    fn plan_production_consumes_convoy_capacity_20_62() {
        // §20.62: Axis Replacement-Point tonnage competes with supply tonnage for the arrival
        // Game-Turn's convoy Shipping Tonnage allowance. A production order is rejected if it would
        // push the turn's booked tonnage over the (die-6) allowance ceiling.
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{date_of, GameState, Phase};

        let mut refs = ReferenceData::new();
        refs.load_production_chart(
            Nationality::German,
            include_str!("../../../data/reference/german-production.ron"),
        )
        .unwrap();
        refs.load_convoy_levels(include_str!("../../../data/reference/convoy-levels.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        g.game_turn = 40;
        g.phase = Phase::Organization;

        let plan = |count: u32| Command::PlanProduction {
            nationality: Nationality::German,
            item: "Infantry".into(),
            count,
        };
        // German Infantry costs 30 tons/point and arrives at GT42 (Axis +2).
        let d = date_of(42);
        let ceiling = g.reference.convoy_levels.as_ref().unwrap().max_tonnage_for(d.year, d.month).unwrap();

        // Book supply convoys arriving GT42 that leave only 100 tons of headroom.
        assert!(g.plan_convoy(4, "Tripoli", 42, 0, 0, ceiling - 100, 0, ceiling));

        // 4 Infantry points = 120 tons > 100 headroom -> rejected for want of sea lift.
        assert_eq!(g.apply(plan(4)), Err(CommandError::InsufficientConvoyCapacity));
        // 3 points = 90 tons fits within the 100-ton headroom.
        assert!(matches!(g.apply(plan(3)), Ok(CommandOutcome::ProductionPlanned { .. })));
        // Now only 10 tons remain; even 1 more point (30 tons) is too much.
        assert_eq!(g.apply(plan(1)), Err(CommandError::InsufficientConvoyCapacity));
    }

    #[test]
    fn over_shipped_production_defers_to_next_turn_20_62() {
        // §20.62/§20.64: Axis Replacement production planned beyond the arrival turn's *actual*
        // rolled Shipping Tonnage cannot all arrive — the excess defers to the next turn (kept).
        use crate::state::command::{Command, CommandOutcome};
        use crate::state::game::{GameState, Phase};

        let mut refs = ReferenceData::new();
        refs.load_production_chart(
            Nationality::German,
            include_str!("../../../data/reference/german-production.ron"),
        )
        .unwrap();
        refs.load_convoy_levels(include_str!("../../../data/reference/convoy-levels.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        g.game_turn = 40;
        g.phase = Phase::Organization;

        // Two German Infantry pools (30 t/pt) arriving GT42: 3 points (90 t) and 1 point (30 t).
        let plan = |n| Command::PlanProduction { nationality: Nationality::German, item: "Infantry".into(), count: n };
        assert!(matches!(g.apply(plan(3)), Ok(CommandOutcome::ProductionPlanned { .. })));
        assert!(matches!(g.apply(plan(1)), Ok(CommandOutcome::ProductionPlanned { .. })));

        // The actual roll for GT42 comes up at only 100 tons — enough for one pool, not both.
        g.game_turn = 42;
        g.axis_shipping_rolled.insert(42, 100);
        g.defer_over_shipped_axis_replacements();

        // §20.62: the 90-ton pool fits and stays; the 30-ton pool is deferred to GT43 (over the roll).
        let mut kept = 0;
        let mut deferred = 0;
        for p in &g.pending_replacements {
            if p.tonnage == 90 { assert_eq!(p.game_turn, 42); kept += 1; }
            if p.tonnage == 30 { assert_eq!(p.game_turn, 43); deferred += 1; }
        }
        assert_eq!((kept, deferred), (1, 1));
    }

    #[test]
    fn convoy_supply_yields_to_production_20_64() {
        // §20.64: Replacement Points have priority in Shipping Space over supplies. Once production
        // is planned for an arrival Game-Turn, a supply convoy may claim only the tonnage that
        // production has left — binding against the actual rolled allowance.
        use crate::state::command::{Command, CommandOutcome};
        use crate::state::game::{date_of, GameState, Phase};

        let mut refs = ReferenceData::new();
        refs.load_production_chart(
            Nationality::German,
            include_str!("../../../data/reference/german-production.ron"),
        )
        .unwrap();
        refs.load_convoy_levels(include_str!("../../../data/reference/convoy-levels.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        g.game_turn = 40;
        g.phase = Phase::Organization;

        // Plan 3 German Infantry Replacement Points (30 t each = 90 t) arriving GT42 (Axis +2).
        assert!(matches!(
            g.apply(Command::PlanProduction { nationality: Nationality::German, item: "Infantry".into(), count: 3 }),
            Ok(CommandOutcome::ProductionPlanned { .. })
        ));
        assert_eq!(g.axis_shipping_committed(42), 90);

        let d = date_of(42);
        let allowable = g.reference.convoy_levels.as_ref().unwrap().max_tonnage_for(d.year, d.month).unwrap();
        // §20.64: supplies get only `allowable − 90`. One ton over is refused …
        assert!(!g.plan_convoy(4, "Tripoli", 42, 0, 0, allowable - 90 + 1, 0, allowable));
        // … exactly the remaining tonnage is accepted.
        assert!(g.plan_convoy(4, "Tripoli", 42, 0, 0, allowable - 90, 0, allowable));
    }

    #[test]
    fn truck_and_infantry_production_20_78() {
        use crate::reference::logistics::TruckKind;
        use crate::reference::units::Side;
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase, ReplacementClass};

        let mut refs = ReferenceData::new();
        refs.load_cw_truck_production(include_str!("../../../data/reference/cw-truck-production.ron")).unwrap();
        refs.load_cw_infantry_production(include_str!("../../../data/reference/cw-infantry-production.ron")).unwrap();
        refs.load_axis_truck_production(include_str!("../../../data/reference/axis-truck-production.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        g.game_turn = 5;
        g.operations_stage = 1;

        // Wrong phase (still InitiativeDetermination).
        assert_eq!(
            g.apply(Command::ProduceCommonwealthInfantry { roll: 12 }),
            Err(CommandError::WrongPhase)
        );
        g.phase = Phase::Organization;

        // §20.78A CW trucks (early era, GT5): light die4=4, medium die4=15, heavy die4=3; first
        // arrival GT9, then spread over four Game-Turns (≤25%/GT).
        assert_eq!(
            g.apply(Command::ProduceCommonwealthTrucks { light_die: 4, medium_die: 4, heavy_die: 4 }),
            Ok(CommandOutcome::TrucksProduced { arrival_game_turn: 9, light: 4, medium: 15, heavy: 3 })
        );
        // §20.78B CW infantry (era GT3–30, roll 12): 20 Infantry Replacement Points, arrive GT9.
        assert_eq!(
            g.apply(Command::ProduceCommonwealthInfantry { roll: 12 }),
            Ok(CommandOutcome::InfantryProduced { arrival_game_turn: 9, points: 20 })
        );
        // §20.74: the 20 points are divided among GT9's three OpStages as evenly as possible (7/7/6).
        let inf_gt9: Vec<_> = g
            .pending_replacements
            .iter()
            .filter(|p| p.class == ReplacementClass::Infantry && p.game_turn == 9)
            .collect();
        assert_eq!(inf_gt9.iter().map(|p| p.count).sum::<u32>(), 20);
        let mut stages: Vec<_> = inf_gt9.iter().map(|p| (p.stage, p.count)).collect();
        stages.sort();
        assert_eq!(stages, vec![(1, 7), (2, 7), (3, 6)]);

        // §20.66a Axis trucks (planned): Light max/turn at GT5 is 4; a 4-point order is accepted,
        // arriving GT7. A further point this turn exceeds the per-turn max.
        assert_eq!(
            g.apply(Command::ProduceAxisTrucks { kind: TruckKind::Light, count: 4 }),
            Ok(CommandOutcome::TrucksProduced { arrival_game_turn: 7, light: 4, medium: 0, heavy: 0 })
        );
        assert_eq!(
            g.apply(Command::ProduceAxisTrucks { kind: TruckKind::Light, count: 1 }),
            Err(CommandError::ProductionRateExceeded)
        );

        // Drive the arrivals. The CW batch spreads over four Game-Turns (§20.78A ≤25%/GT) and, within
        // each, over three OpStages (§20.74): at GT9 OpStage 1 only the first (turn, stage) fragment
        // lands — light bucket 1→stage[1,0,0]=1; medium bucket 4→stage[2,1,1]=2; heavy bucket 1→1.
        // Meanwhile the Axis GT7 delivery (single stage) has fully landed.
        g.game_turn = 9;
        g.operations_stage = 1;
        g.process_truck_arrivals();
        let cw = g.truck_pools.get(&Side::Commonwealth).copied().unwrap_or_default();
        assert_eq!((cw.light, cw.medium, cw.heavy), (1, 2, 1), "only GT9 OpStage 1 has arrived");
        let axis = g.truck_pools.get(&Side::Axis).copied().unwrap_or_default();
        assert_eq!((axis.light, axis.medium, axis.heavy), (4, 0, 0));
        assert!(!g.pending_trucks.is_empty(), "later fragments still pending");

        // Process the full four Game-Turns (each through OpStage 3); the whole CW batch has landed.
        for gt in [9, 10, 11, 12] {
            g.game_turn = gt;
            g.operations_stage = 3;
            g.process_truck_arrivals();
        }
        let cw = g.truck_pools.get(&Side::Commonwealth).copied().unwrap_or_default();
        assert_eq!((cw.light, cw.medium, cw.heavy), (4, 15, 3));
        assert!(g.pending_trucks.is_empty());
    }

    #[test]
    fn axis_trucks_and_production_share_convoy_capacity_20_62() {
        // §20.62: Axis Truck Production and Replacement Production draw from the same arrival-turn
        // convoy Shipping Tonnage allowance, so they compete.
        use crate::reference::logistics::TruckKind;
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{date_of, GameState, Phase};

        let mut refs = ReferenceData::new();
        refs.load_production_chart(
            Nationality::German,
            include_str!("../../../data/reference/german-production.ron"),
        )
        .unwrap();
        refs.load_axis_truck_production(include_str!("../../../data/reference/axis-truck-production.ron")).unwrap();
        refs.load_convoy_levels(include_str!("../../../data/reference/convoy-levels.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        g.game_turn = 40;
        g.phase = Phase::Organization;

        // Both a truck order and an infantry order planned at GT40 arrive at GT42 (Axis +2).
        let d = date_of(42);
        let ceiling = g.reference.convoy_levels.as_ref().unwrap().max_tonnage_for(d.year, d.month).unwrap();
        // Book supply convoys arriving GT42 leaving only 200 tons of headroom.
        assert!(g.plan_convoy(4, "Tripoli", 42, 0, 0, ceiling - 200, 0, ceiling));

        // A Heavy truck costs 150 convoy tons/point (axis-truck-production.ron). One point fits the
        // 200-ton headroom and consumes 150 of it.
        assert!(matches!(
            g.apply(Command::ProduceAxisTrucks { kind: TruckKind::Heavy, count: 1 }),
            Ok(CommandOutcome::TrucksProduced { .. })
        ));
        // Only ~50 tons remain, so 2 German Infantry points (60 tons) no longer fit — the truck
        // tonnage was counted against the same GT42 allowance.
        assert_eq!(
            g.apply(Command::PlanProduction { nationality: Nationality::German, item: "Infantry".into(), count: 2 }),
            Err(CommandError::InsufficientConvoyCapacity)
        );
        // A single Infantry point (30 tons) still fits.
        assert!(matches!(
            g.apply(Command::PlanProduction { nationality: Nationality::German, item: "Infantry".into(), count: 1 }),
            Ok(CommandOutcome::ProductionPlanned { .. })
        ));
    }

    #[test]
    fn assign_trucks_motorizes_and_releases_6_17() {
        use crate::reference::units::{Nationality, Side};
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::unit::{Composition, TruckPoints};

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_truck_chart(include_str!("../../../data/reference/truck-characteristics.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        // CW code "k": leg infantry, basic CPA 10, max TOE 7 — motorizable (§6.17).
        let u = g.spawn_unit(Nationality::Commonwealth, "k", "1 Bn", Composition::Organic { current_toe: 3 });
        g.truck_pools.insert(Side::Commonwealth, TruckPoints { light: 0, medium: 5, heavy: 0 });

        // Wrong phase.
        assert_eq!(
            g.apply(Command::AssignTrucks { unit: u, light: 0, medium: 3, heavy: 0 }),
            Err(CommandError::WrongPhase)
        );
        g.phase = Phase::Organization;

        // More than the pool holds -> rejected.
        assert_eq!(
            g.apply(Command::AssignTrucks { unit: u, light: 0, medium: 6, heavy: 0 }),
            Err(CommandError::InsufficientTrucks)
        );

        // 3 Medium Truck Points carry the 3-TOE infantry: motorized CPA 20 (§6.17), pool debited.
        assert_eq!(
            g.apply(Command::AssignTrucks { unit: u, light: 0, medium: 3, heavy: 0 }),
            Ok(CommandOutcome::TrucksAssigned { light: 0, medium: 3, heavy: 0, motorized_cpa: Some(20) })
        );
        assert_eq!(g.truck_pools[&Side::Commonwealth].medium, 2);
        assert_eq!(g.effective_cpa(u), Some(20), "moves at the truck's CPA");

        // Release them: back to the pool, no longer motorized.
        assert_eq!(
            g.apply(Command::ReleaseTrucks { unit: u, light: 0, medium: 3, heavy: 0 }),
            Ok(CommandOutcome::TrucksAssigned { light: 0, medium: 0, heavy: 0, motorized_cpa: None })
        );
        assert_eq!(g.truck_pools[&Side::Commonwealth].medium, 5);
        assert_eq!(g.effective_cpa(u), Some(10), "back to its own leg CPA");
        // Releasing more than held -> rejected.
        assert_eq!(
            g.apply(Command::ReleaseTrucks { unit: u, light: 0, medium: 1, heavy: 0 }),
            Err(CommandError::InsufficientTrucks)
        );
    }

    #[test]
    fn load_campaign_populates_full_reference_layer() {
        use crate::reference::units::Nationality;
        let r = ReferenceData::load_campaign();

        // Per-nationality datasets for all three nationalities.
        for n in [Nationality::German, Nationality::Italian, Nationality::Commonwealth] {
            assert!(r.unit_characteristics.contains_key(&n), "unit chars {n:?}");
            assert!(r.weapons_systems.contains_key(&n), "weapons {n:?}");
            assert!(r.aircraft_charts.contains_key(&n), "aircraft {n:?}");
            assert!(r.production_charts.contains_key(&n), "production {n:?}");
        }
        // All three OBs are loaded (German is a curated subset).
        assert!(r.orders_of_battle.contains_key(&Nationality::German));
        assert!(r.orders_of_battle.contains_key(&Nationality::Italian));
        assert!(r.orders_of_battle.contains_key(&Nationality::Commonwealth));
        assert!(!r.terrain.is_empty() && !r.hexsides.is_empty());
        assert!(!r.reinforcement_entry.is_empty());

        // Every optional chart/table is present (each previously required an on-demand load).
        let checks: [(&str, bool); 36] = [
            ("aircraft_refit", r.aircraft_refit.is_some()),
            ("scramble_table", r.scramble_table.is_some()),
            ("strafing_table", r.strafing_table.is_some()),
            ("air_bombardment", r.air_bombardment.is_some()),
            ("mining_harbor", r.mining_harbor.is_some()),
            ("air_to_air", r.air_to_air.is_some()),
            ("anti_aircraft", r.anti_aircraft.is_some()),
            ("air_recon", r.air_recon.is_some()),
            ("ports_chart", r.ports_chart.is_some()),
            ("malta_availability", r.malta_availability.is_some()),
            ("malta_construction", r.malta_construction.is_some()),
            ("malta_commitment", r.malta_commitment.is_some()),
            ("shipping_lanes", r.shipping_lanes.is_some()),
            ("air_distance", r.air_distance.is_some()),
            ("convoy_levels", r.convoy_levels.is_some()),
            ("naval_recon", r.naval_recon.is_some()),
            ("equivalent_weights", r.equivalent_weights.is_some()),
            ("anti_armor_crt", r.anti_armor_crt.is_some()),
            ("barrage_crt", r.barrage_crt.is_some()),
            ("morale_modifier_table", r.morale_modifier_table.is_some()),
            ("close_assault_crt", r.close_assault_crt.is_some()),
            ("prisoners_table", r.prisoners_table.is_some()),
            ("initiative_ratings", r.initiative_ratings.is_some()),
            ("off_map", r.off_map.is_some()),
            ("weather_table", r.weather_table.is_some()),
            ("campaign_victory", r.campaign_victory.is_some()),
            ("ammo_rates", r.ammo_rates.is_some()),
            ("water_table", r.water_table.is_some()),
            ("truck_chart", r.truck_chart.is_some()),
            ("breakdown_table", r.breakdown_table.is_some()),
            ("repair_table", r.repair_table.is_some()),
            ("axis_truck_production", r.axis_truck_production.is_some()),
            ("cw_truck_production", r.cw_truck_production.is_some()),
            ("cw_infantry_production", r.cw_infantry_production.is_some()),
            ("replacement_conversion", r.replacement_conversion.is_some()),
            ("convoy_levels_flows", r.convoy_levels.as_ref().and_then(|c| c.max_tonnage_for(1940, 9)).is_some()),
        ];
        for (name, ok) in checks {
            assert!(ok, "load_campaign left {name} unloaded");
        }
    }

    #[test]
    fn italian_production_chart() {
        use crate::reference::logistics::ProductionPeriod;

        let mut refs = ReferenceData::new();
        refs.load_production_chart(
            Nationality::Italian,
            include_str!("../../../data/reference/italian-production.ron"),
        )
        .unwrap();
        let chart = refs.production_charts.get(&Nationality::Italian).unwrap();
        assert_eq!(chart.items.len(), 20);
        // §20.66 Infantry phased maxima: 0 before GT5, 5 (GT5–8), 10 (GT9–24), 25 (GT25+).
        assert_eq!(chart.available("Infantry", 4), 0);
        assert_eq!(chart.available("Infantry", 5), 5);
        assert_eq!(chart.available("Infantry", 8), 5);
        assert_eq!(chart.available("Infantry", 9), 10);
        assert_eq!(chart.available("Infantry", 24), 10);
        assert_eq!(chart.available("Infantry", 25), 25);
        assert_eq!(chart.item("Infantry").unwrap().total, 1200);
        // Light AA: 2/turn (GT2–24) → 1/turn (GT25+); total 60.
        assert_eq!(chart.available("Light AA", 2), 2);
        assert_eq!(chart.available("Light AA", 25), 1);
        assert_eq!(chart.item("Light AA").unwrap().total, 60);
        // CV L.3 tanks: flat 5/turn from GT3.
        assert_eq!(chart.available("CV L.3 or CV 33/35", 3), 5);
        // 105/28 Gun's first tier is per-Month (†).
        assert_eq!(chart.item("105/28 Gun").unwrap().tiers[0].period, ProductionPeriod::Month);
    }

    #[test]
    fn cw_truck_production_table() {
        use crate::reference::logistics::TruckKind;

        let mut refs = ReferenceData::new();
        refs.load_cw_truck_production(include_str!(
            "../../../data/reference/cw-truck-production.ron"
        ))
        .unwrap();
        let t = refs.cw_truck_production.clone().unwrap();
        // §20.78A era GT1–30: Light die1=0/die6=5, Medium die6=20, Heavy die1=0.
        assert_eq!(t.produce(TruckKind::Light, 5, 1), 0);
        assert_eq!(t.produce(TruckKind::Light, 5, 6), 5);
        assert_eq!(t.produce(TruckKind::Medium, 30, 6), 20);
        assert_eq!(t.produce(TruckKind::Heavy, 5, 1), 0);
        // Lend-Lease era GT31+: Light die1=3, Medium die6=60, Heavy die6=13.
        assert_eq!(t.produce(TruckKind::Light, 31, 1), 3);
        assert_eq!(t.produce(TruckKind::Medium, 40, 6), 60);
        assert_eq!(t.produce(TruckKind::Heavy, 31, 6), 13);
    }

    #[test]
    fn cw_infantry_production_table() {
        let mut refs = ReferenceData::new();
        refs.load_cw_infantry_production(include_str!(
            "../../../data/reference/cw-infantry-production.ron"
        ))
        .unwrap();
        let t = refs.cw_infantry_production.clone().unwrap();
        // §20.78B by 2d6, per era.
        assert_eq!(t.produce(20, 2), 0); // GT3-30, roll 2 = none
        assert_eq!(t.produce(20, 4), 15);
        assert_eq!(t.produce(20, 12), 20);
        assert_eq!(t.produce(40, 5), 22); // GT31-46
        assert_eq!(t.produce(50, 2), 5); // GT47-102
        assert_eq!(t.produce(105, 8), 13); // GT103-107
        assert_eq!(t.produce(200, 6), 0); // beyond all eras
    }

    #[test]
    fn cw_production_chart() {
        use crate::reference::logistics::ProductionPeriod;

        let mut refs = ReferenceData::new();
        refs.load_production_chart(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/cw-production.ron"),
        )
        .unwrap();
        let chart = refs.production_charts.get(&Nationality::Commonwealth).unwrap();
        assert_eq!(chart.items.len(), 24);
        // 25-pounders: 250 total, 4/turn from GT11, no convoy tonnage.
        let p25 = chart.item("25-pounders").unwrap();
        assert_eq!((p25.total, p25.max, p25.first_gt, p25.tonnage), (250, 4, 11, 0));
        assert_eq!(chart.available("25-pounders", 10), 0);
        assert_eq!(chart.available("25-pounders", 11), 4);
        // 155mm Howitzers: only GT51–59.
        assert_eq!(chart.available("155mm Howitzers", 50), 0);
        assert_eq!(chart.available("155mm Howitzers", 51), 1);
        assert_eq!(chart.available("155mm Howitzers", 59), 1);
        assert_eq!(chart.available("155mm Howitzers", 60), 0);
        // §20.78C's inverted legend: A9 Cruiser `*` = per-Month; Crusader Mk III `†` = per-Two-Weeks.
        assert_eq!(chart.item("A9 Cruiser").unwrap().period, ProductionPeriod::Month);
        assert_eq!(chart.item("Crusader Mk III").unwrap().period, ProductionPeriod::TwoWeeks);
    }

    #[test]
    fn replacement_point_conversion_chart() {
        let mut refs = ReferenceData::new();
        refs.load_replacement_conversion(include_str!(
            "../../../data/reference/replacement-conversion.ron"
        ))
        .unwrap();
        let conv = refs.replacement_conversion.clone().unwrap();
        assert_eq!(conv.entries.len(), 20);
        // §20.3: any other infantry = 1 Inf.
        let inf = conv.cost("Any other Infantry").unwrap();
        assert_eq!(inf.options, vec![vec![crate::reference::logistics::ConversionReq { kind: "Inf".into(), count: 1 }]]);
        // Heavy Weapons = 1 Inf & any 1 Gun (a single option with two reqs).
        assert_eq!(conv.cost("Heavy Weapons").unwrap().options[0].len(), 2);
        // Armored Car = 2 ArmR OR 1 Lt Tank (two alternative options).
        assert_eq!(conv.cost("Armored Car").unwrap().options.len(), 2);
        // Road/Railroad Construction needs no Replacement Points.
        assert!(conv.cost("Road/Railroad Construction").unwrap().options.is_empty());
        // Squadron Ground Support Unit also expends Supplies.
        assert!(conv.cost("Squadron Ground Support Unit").unwrap().supplies);
    }

    #[test]
    fn water_deprived_vehicle_cannot_move() {
        use crate::reference::terrain::HexTerrain;
        use crate::rules::movement::MoveMode;
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation, WeaponGroup};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let (a, b, c) = (HexCoord::new(0, 0), HexCoord::new(1, 0), HexCoord::new(2, 0));
        for h in [a, b, c] {
            g.map.set_hex(h, HexTerrain::Clear);
        }
        let tank = g.spawn_unit(
            Nationality::Commonwealth,
            "g",
            "t",
            Composition::Weapons {
                groups: vec![WeaponGroup { system: "A9 Cruiser".into(), count: 1 }],
            },
        );
        g.unit_mut(tank).unwrap().location = Some(UnitLocation::OnMap(a));
        g.unit_mut(tank).unwrap().fuel = 100;
        g.phase = Phase::MovementAndCombat;

        // Not deprived → the tank moves.
        assert!(!g.unit_water_deprived(tank));
        assert_eq!(
            g.apply(Command::MoveStep { unit: tank, to: b, mode: MoveMode::Motorized }),
            Ok(CommandOutcome::Moved)
        );
        // Deprived of water → a vehicle may not move (§52.51).
        g.unit_mut(tank).unwrap().stages_without_water = 1;
        assert!(g.unit_water_deprived(tank));
        assert_eq!(
            g.apply(Command::MoveStep { unit: tank, to: c, mode: MoveMode::Motorized }),
            Err(CommandError::NoWater)
        );
        assert_eq!(g.unit(tank).unwrap().hex(), Some(b)); // did not move
    }

    #[test]
    fn deprived_infantry_capped_at_cpa() {
        use crate::reference::terrain::{Fixed, HexTerrain};
        use crate::rules::movement::MoveMode;
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let (a, b) = (HexCoord::new(0, 0), HexCoord::new(1, 0));
        g.map.set_hex(a, HexTerrain::Clear);
        g.map.set_hex(b, HexTerrain::Clear);
        let inf = g.spawn_unit(Nationality::Commonwealth, "h", "i", Composition::Organic { current_toe: 4 });
        g.unit_mut(inf).unwrap().location = Some(UnitLocation::OnMap(a));
        g.phase = Phase::MovementAndCombat;

        // Pretend the unit has already spent its full CPA this Operations Stage.
        let cpa = g.unit_cpa(inf).unwrap();
        g.unit_mut(inf).unwrap().cp_expended = Fixed(cpa * 10);

        // §52.52: deprived infantry may not voluntarily exceed CPA → the move is blocked.
        g.unit_mut(inf).unwrap().stages_without_water = 1;
        assert_eq!(
            g.apply(Command::MoveStep { unit: inf, to: b, mode: MoveMode::NonMotorized }),
            Err(CommandError::NoWater)
        );
        assert_eq!(g.unit(inf).unwrap().hex(), Some(a)); // did not move

        // Not deprived → it may exceed CPA (taking Disorganization instead).
        g.unit_mut(inf).unwrap().stages_without_water = 0;
        assert_eq!(
            g.apply(Command::MoveStep { unit: inf, to: b, mode: MoveMode::NonMotorized }),
            Ok(CommandOutcome::Moved)
        );
    }

    #[test]
    fn water_deprived_unit_cannot_assault() {
        use crate::state::command::{Command, CommandError};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let (a, b) = (HexCoord::new(0, 0), HexCoord::new(1, 0));
        let atk = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "a",
            Composition::Organic { current_toe: 4 },
        );
        g.unit_mut(atk).unwrap().location = Some(UnitLocation::OnMap(a));
        g.phase = Phase::MovementAndCombat;
        g.unit_mut(atk).unwrap().stages_without_water = 1; // deprived of water

        // §52.51/52: a water-deprived unit may not assault offensively.
        assert_eq!(
            g.apply(Command::CloseAssault { attackers: vec![atk], target: b }),
            Err(CommandError::NoWater)
        );
    }

    #[test]
    fn water_deprived_defender_halves_defense() {
        use crate::state::game::GameState;
        use crate::state::unit::Composition;

        let mut g = GameState::new(close_assault_refs(), 1);
        let atk = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "atk",
            Composition::Organic { current_toe: 10 },
        );
        let def = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "def",
            Composition::Organic { current_toe: 10 },
        );
        let base = g.resolve_close_assault_with_rolls(&[atk], &[def], 33, 33, 33, 33);
        // §52.51/52: a water-deprived defender defends at half raw strength.
        g.unit_mut(def).unwrap().stages_without_water = 1;
        let deprived = g.resolve_close_assault_with_rolls(&[atk], &[def], 33, 33, 33, 33);
        assert!(deprived.defender_strength < base.defender_strength);
        assert!(deprived.differential > base.differential);
    }

    #[test]
    fn campaign_auto_wins() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::reference::victory::CampaignVictory;
        use crate::state::game::GameState;
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        // Build a linear Clear map: source (0,0), a source-connected Axis dump at (2,0), and
        // an Axis combat unit at (4,0). Vary the objective / thresholds / turn per case.
        let build = |objective: Vec<String>, axis_mp: u32, cw_mp: u32, turn: u32| -> GameState {
            let mut refs = ReferenceData::new();
            refs.load_terrain(TERRAIN_RON).unwrap();
            refs.load_unit_characteristics(
                Nationality::Italian,
                include_str!("../../../data/reference/unit-characteristics/italian.ron"),
            )
            .unwrap();
            refs.campaign_victory = Some(CampaignVictory {
                cities: vec![],
                levels: vec![],
                sources: vec!["A0000".into()], // hex (0,0)
                axis_objective: objective,
                axis_supply_mp: axis_mp,
                cw_check_mp: cw_mp,
                cw_check_from_turn: 35,
            });
            let mut g = GameState::new(refs, 1);
            g.game_turn = turn;
            for c in 0..7 {
                g.map.set_hex(HexCoord::new(c, 0), HexTerrain::Clear);
            }
            let ax = g.spawn_unit(
                Nationality::Italian,
                "p",
                "ax",
                Composition::Organic { current_toe: 5 },
            );
            g.unit_mut(ax).unwrap().location = Some(UnitLocation::OnMap(HexCoord::new(4, 0)));
            g.add_supply_dump(UnitLocation::OnMap(HexCoord::new(2, 0)), Side::Axis, 40, 60);
            g
        };

        // §64.71: Axis holds the objective (4,0) and traces ≤5 MP to the source-connected dump.
        // The auto-win requires the condition held for a full Game-Turn.
        let mut g = build(vec!["A0004".into()], 5, 3, 1);
        g.update_victory_hold(); // turn 1: condition first satisfied
        assert!(g.campaign_auto_winner().is_none()); // not yet a full turn
        g.game_turn = 2;
        g.update_victory_hold(); // still holding a turn later
        assert_eq!(g.campaign_auto_winner(), Some(Side::Axis));
        // Abandoning the objective resets the full-turn tracker (§64.71 must be continuous).
        let ax = g.units.values().find(|u| u.nationality.side() == Side::Axis).unwrap().id;
        g.unit_mut(ax).unwrap().location = Some(UnitLocation::OnMap(HexCoord::new(6, 0)));
        g.update_victory_hold();
        assert_eq!(g.axis_objective_held_since, None);
        assert!(g.campaign_auto_winner().is_none());

        // §64.72: from GT35, objective unheld and no Axis unit within 3 MP of a source dump.
        let g = build(vec!["A0006".into()], 5, 3, 35);
        assert_eq!(g.campaign_auto_winner(), Some(Side::Commonwealth));

        // Neither: GT35, objective unheld, but the Axis unit IS within a 5-MP line.
        let g = build(vec!["A0006".into()], 5, 5, 35);
        assert_eq!(g.campaign_auto_winner(), None);
    }

    #[test]
    fn sandstorm_doubles_movement_cost() {
        use crate::reference::terrain::{Fixed, HexTerrain};
        use crate::rules::movement::MoveMode;
        use crate::state::command::{Command, CommandOutcome};
        use crate::state::game::{GameState, Phase, Weather};
        use crate::state::map::HexCoord;
        use crate::state::unit::Composition;

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        let mut g = GameState::new(refs, 1);
        let a = HexCoord::new(0, 0); // section A (col / 100 == 0)
        let b = HexCoord::new(1, 0);
        g.map.set_hex(a, HexTerrain::Clear);
        g.map.set_hex(b, HexTerrain::Clear);
        let unit = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "mover",
            Composition::Organic { current_toe: 5 },
        );
        g.unit_mut(unit).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(a));
        g.phase = Phase::MovementAndCombat;

        // Section A is in a sandstorm. A Clear hex there is affected; a Sea hex is not
        // (§29.46 — sandstorms stop at the coast).
        g.weather_by_section.insert('A', Weather::Sandstorm);
        assert!(g.sandstorm_at(b));
        let sea = HexCoord::new(0, 1);
        g.map.set_hex(sea, HexTerrain::Sea);
        assert!(!g.sandstorm_at(sea));

        // §29.44: entering the sandstorm Clear hex costs double (2 CP → 4 CP = 40 tenths).
        assert_eq!(
            g.apply(Command::MoveStep { unit, to: b, mode: MoveMode::NonMotorized }),
            Ok(CommandOutcome::Moved)
        );
        assert_eq!(g.unit(unit).unwrap().cp_expended, Fixed(40));
    }

    #[test]
    fn season_mapping_matches_the_calendar() {
        use crate::state::game::{season_of, Season};
        // §29.61 Weather Table Game-Turn column: 12-GT blocks over a 48-GT year.
        assert_eq!(season_of(1), Season::Spring); // GT1-12
        assert_eq!(season_of(12), Season::Spring);
        assert_eq!(season_of(13), Season::Summer); // GT13-24
        assert_eq!(season_of(25), Season::Fall); // GT25-36
        assert_eq!(season_of(37), Season::Winter); // GT37-48
        assert_eq!(season_of(49), Season::Spring); // wraps
        assert_eq!(season_of(109), Season::Summer); // GT109-110
    }

    #[test]
    fn reinforcements_arrive_at_naval_convoy_phase() {
        use crate::state::game::{GameState, Phase};
        use crate::reference::units::Side;
        use crate::state::map::HexId;

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_order_of_battle(
            Nationality::Commonwealth,
            include_str!("../../../data/ob/commonwealth.ron"),
        )
        .unwrap();
        refs.load_initiative_ratings(include_str!("../../../data/reference/initiative-ratings.ron"))
            .unwrap();
        let mut g = GameState::new(refs, 7);
        g.first_turn_initiative = Some(Side::Axis);
        g.deploy_formation(
            Nationality::Commonwealth,
            "4th Indian Division",
            HexId::parse("D3615").unwrap().to_hex_coord(),
        )
        .unwrap();

        // 1st Field Arty arrives GT3, stage 3 (§4.44 arrival). Drive the turn loop and record
        // exactly when it appears — it must be the Naval Convoy Arrival Phase (§20.12).
        let present = |g: &GameState| g.units.values().any(|u| u.name == "1st Field Artillery Regt");
        assert!(!present(&g));
        let mut seen: Option<(u32, u8, Phase)> = None;
        for _ in 0..600 {
            g.advance_phase();
            if present(&g) {
                seen = Some((g.game_turn, g.operations_stage, g.phase));
                break;
            }
        }
        assert_eq!(seen, Some((3, 3, Phase::NavalConvoyArrival)));
    }

    #[test]
    fn off_map_network_loads() {
        let mut refs = ReferenceData::new();
        refs.load_off_map_nodes(include_str!("../../../data/reference/off-map-nodes.ron"))
            .unwrap();
        let net = refs.off_map.as_ref().unwrap();
        assert_eq!(net.map_junction, "A2802"); // §8.85 map/off-map junction
        assert_eq!(net.entry_road.first().map(String::as_str), Some("A2802"));
        assert!(net.entry_road.contains(&"A2703".to_string())); // Nofilia, second on the road
        assert_eq!(net.nodes.len(), 4);
        // Tripoli: AA 26, a port, and the Axis garrison marker (§8.8 / Map A).
        let tripoli = net.node("Tripoli").unwrap();
        assert_eq!(tripoli.aa_points, 26);
        assert!(tripoli.port.is_some());
        assert_eq!(tripoli.garrison.as_deref(), Some("Axis"));
        // §8.89 cumulative stages to the map (matches the Nofilia row of the chart):
        // Tunis for CPA<15 = 5+3+3+3 = 14; a fast (CPA≥25) unit = 1+1+1+1 = 4.
        assert_eq!(net.stages_to_map("Tunis", 10), Some(14));
        assert_eq!(net.stages_to_map("Tunis", 30), Some(4));
        // Tripoli for a mid-CPA unit = 1+1 = 2 (Nofilia→Tripoli column).
        assert_eq!(net.stages_to_map("Tripoli", 18), Some(2));
    }

    #[test]
    fn reinforcement_entry_table_loads() {
        let mut refs = ReferenceData::new();
        refs.load_reinforcement_entry(include_str!(
            "../../../data/reference/reinforcement-entry.ron"
        ))
        .unwrap();
        // §20.14: Commonwealth reinforcements enter at Cairo (E1430).
        assert_eq!(refs.reinforcement_entry_hex(Nationality::Commonwealth), Some("E1430"));
        // §20.15: Axis reinforcements enter at the Tripoli off-map node.
        assert_eq!(refs.reinforcement_entry_hex(Nationality::German), Some("Tripoli"));
        assert_eq!(refs.reinforcement_entry_hex(Nationality::Italian), Some("Tripoli"));
    }

    #[test]
    fn whole_formation_reinforcement_enters_at_entry() {
        use crate::state::game::GameState;
        use crate::state::map::HexId;

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Italian,
            include_str!("../../../data/reference/unit-characteristics/italian.ron"),
        )
        .unwrap();
        refs.load_order_of_battle(
            Nationality::Italian,
            include_str!("../../../data/ob/italian.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);

        // Ariete is a whole-formation reinforcement (all units arrive GT18). Faithfully it
        // debarks at Tripoli (off-map); here we use an explicit on-map entry override.
        let entry = HexId::parse("A4827").unwrap().to_hex_coord();
        g.register_reinforcement(Nationality::Italian, "132 Ariete Armored Division", crate::state::unit::UnitLocation::OnMap(entry), &std::collections::HashMap::new())
            .unwrap();
        assert_eq!(g.units.len(), 0); // nothing on the map yet
        assert_eq!(g.pending_arrivals.len(), 1); // one distinct arrival time (GT18/1)

        // GT18, stage 1: the whole division enters at the entry hex as a full hierarchy.
        g.game_turn = 18;
        g.operations_stage = 1;
        g.process_arrivals();
        assert_eq!(g.units.len(), 14);
        let named = |n: &str| g.units.values().find(|u| u.name == n).map(|u| u.id);
        let div = named("Ariete (132nd) Div HQE").expect("Div HQE entered");
        assert_eq!(g.unit(div).unwrap().hex(), Some(entry));
        let regt = named("132 Regt Corazzata HQ").expect("regt HQ");
        assert_eq!(g.unit(regt).unwrap().attached_parent, Some(div));
        let tank = named("VII(M) Tank Bn").expect("tank bn");
        assert_eq!(g.unit(tank).unwrap().attached_parent, Some(regt));
    }

    #[test]
    fn cw_reinforcement_defaults_to_cairo() {
        use crate::state::game::GameState;
        use crate::state::map::HexId;

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_order_of_battle(
            Nationality::Commonwealth,
            include_str!("../../../data/ob/commonwealth.ron"),
        )
        .unwrap();
        refs.load_reinforcement_entry(include_str!(
            "../../../data/reference/reinforcement-entry.ron"
        ))
        .unwrap();
        let mut g = GameState::new(refs, 1);

        // A CW reinforcement with no explicit hex resolves to the nationality entry (Cairo).
        g.load_scenario(
            r#"(name: "T", reinforcements: [(nationality: Commonwealth, formation: "4th Indian Division")])"#,
        )
        .unwrap();
        let cairo = HexId::parse("E1430").unwrap().to_hex_coord();
        assert!(!g.pending_arrivals.is_empty());
        assert!(g.pending_arrivals.iter().all(|p| p.entry == crate::state::unit::UnitLocation::OnMap(cairo)));
    }

    #[test]
    fn axis_reinforcement_enters_at_tripoli_off_map() {
        use crate::state::game::GameState;
        use crate::state::unit::UnitLocation;

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Italian,
            include_str!("../../../data/reference/unit-characteristics/italian.ron"),
        )
        .unwrap();
        refs.load_order_of_battle(
            Nationality::Italian,
            include_str!("../../../data/ob/italian.ron"),
        )
        .unwrap();
        refs.load_reinforcement_entry(include_str!(
            "../../../data/reference/reinforcement-entry.ron"
        ))
        .unwrap();
        refs.load_off_map_nodes(include_str!("../../../data/reference/off-map-nodes.ron"))
            .unwrap();
        let mut g = GameState::new(refs, 1);

        // Ariete (Italian = Axis, arrives GT18) with no explicit hex resolves to the Axis
        // entry point — the Tripoli off-map node (§20.15).
        g.load_scenario(
            r#"(name: "T", reinforcements: [(nationality: Italian, formation: "132 Ariete Armored Division")])"#,
        )
        .unwrap();
        let tripoli = UnitLocation::OffMap { node: "Tripoli".to_string(), transit: 0, toward_map: true };
        assert!(g.pending_arrivals.iter().all(|p| p.entry == tripoli));

        // GT18: the division debarks in the Tripoli box — off-map, not on any hex yet.
        g.game_turn = 18;
        g.operations_stage = 1;
        g.process_arrivals();
        let named = |n: &str| g.units.values().find(|u| u.name == n).map(|u| u.id);
        let div = named("Ariete (132nd) Div HQE").expect("Ariete arrived");
        assert_eq!(g.unit(div).unwrap().location, Some(tripoli.clone()));
        assert_eq!(g.unit(div).unwrap().hex(), None); // off the board
        // Its regiment HQ is attached, so the formation is together in the box.
        let regt = named("132 Regt Corazzata HQ").expect("regt HQ");
        assert_eq!(g.unit(regt).unwrap().attached_parent, Some(div));
    }

    #[test]
    fn off_map_unit_traverses_to_the_map() {
        use crate::state::game::{GameState, OffMapMove};
        use crate::state::map::HexId;
        use crate::state::unit::UnitLocation;

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Italian,
            include_str!("../../../data/reference/unit-characteristics/italian.ron"),
        )
        .unwrap();
        refs.load_order_of_battle(
            Nationality::Italian,
            include_str!("../../../data/ob/italian.ron"),
        )
        .unwrap();
        refs.load_reinforcement_entry(include_str!(
            "../../../data/reference/reinforcement-entry.ron"
        ))
        .unwrap();
        refs.load_off_map_nodes(include_str!("../../../data/reference/off-map-nodes.ron"))
            .unwrap();
        let mut g = GameState::new(refs, 1);
        g.load_scenario(
            r#"(name: "T", reinforcements: [(nationality: Italian, formation: "132 Ariete Armored Division")])"#,
        )
        .unwrap();
        g.game_turn = 18;
        g.operations_stage = 1;
        g.process_arrivals();

        let div = g.units.values().find(|u| u.name == "Ariete (132nd) Div HQE").unwrap().id;
        let cpa = g.formation_cpa(div).expect("formation cpa");
        let expected = g.reference.off_map.as_ref().unwrap().stages_to_map("Tripoli", cpa).unwrap();

        // Advance one Operations Stage at a time until the division reaches the map.
        let mut stages = 0u32;
        loop {
            stages += 1;
            assert!(stages < 30, "runaway off-map traversal");
            if let OffMapMove::EnteredMap(_) = g.advance_off_map_toward_map(div).unwrap() {
                break;
            }
        }
        assert_eq!(stages, expected); // matches the §8.89 cumulative distance Tripoli→map

        // The division is now on the map at the Nofilia junction (A2703, §8.85), and its
        // attached tank battalion rode along (effective hex follows the parent counter).
        let nofilia = HexId::parse("A2802").unwrap().to_hex_coord();
        assert_eq!(g.unit(div).unwrap().location, Some(UnitLocation::OnMap(nofilia)));
        let tank = g.units.values().find(|u| u.name == "VII(M) Tank Bn").unwrap().id;
        assert_eq!(g.effective_hex(tank), Some(nofilia));
    }

    #[test]
    fn move_off_map_command_reaches_the_map() {
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, OffMapMove, Phase};
        use crate::state::map::HexId;

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Italian,
            include_str!("../../../data/reference/unit-characteristics/italian.ron"),
        )
        .unwrap();
        refs.load_order_of_battle(
            Nationality::Italian,
            include_str!("../../../data/ob/italian.ron"),
        )
        .unwrap();
        refs.load_reinforcement_entry(include_str!(
            "../../../data/reference/reinforcement-entry.ron"
        ))
        .unwrap();
        refs.load_off_map_nodes(include_str!("../../../data/reference/off-map-nodes.ron"))
            .unwrap();
        let mut g = GameState::new(refs, 1);
        g.load_scenario(
            r#"(name: "T", reinforcements: [(nationality: Italian, formation: "132 Ariete Armored Division")])"#,
        )
        .unwrap();
        g.game_turn = 18;
        g.operations_stage = 1;
        g.process_arrivals();
        let div = g.units.values().find(|u| u.name == "Ariete (132nd) Div HQE").unwrap().id;

        // Gated to the Movement & Combat phase.
        assert_eq!(g.apply(Command::MoveOffMap { unit: div, direction: crate::state::game::OffMapDir::TowardMap }), Err(CommandError::WrongPhase));

        g.phase = Phase::MovementAndCombat;
        let mut entered = false;
        for _ in 0..30 {
            match g.apply(Command::MoveOffMap { unit: div, direction: crate::state::game::OffMapDir::TowardMap }).unwrap() {
                CommandOutcome::OffMapMoved(OffMapMove::EnteredMap(_)) => {
                    entered = true;
                    break;
                }
                CommandOutcome::OffMapMoved(_) => {}
                other => panic!("unexpected outcome {other:?}"),
            }
        }
        assert!(entered);
        assert_eq!(
            g.unit(div).unwrap().hex(),
            Some(HexId::parse("A2802").unwrap().to_hex_coord())
        );
    }

    #[test]
    fn off_map_movement_inland_and_reverse() {
        use crate::state::game::{GameState, OffMapDir, OffMapMove};
        use crate::state::unit::UnitLocation;

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Italian,
            include_str!("../../../data/reference/unit-characteristics/italian.ron"),
        )
        .unwrap();
        refs.load_order_of_battle(
            Nationality::Italian,
            include_str!("../../../data/ob/italian.ron"),
        )
        .unwrap();
        refs.load_reinforcement_entry(include_str!(
            "../../../data/reference/reinforcement-entry.ron"
        ))
        .unwrap();
        refs.load_off_map_nodes(include_str!("../../../data/reference/off-map-nodes.ron"))
            .unwrap();
        let mut g = GameState::new(refs, 1);
        g.load_scenario(
            r#"(name: "T", reinforcements: [(nationality: Italian, formation: "132 Ariete Armored Division")])"#,
        )
        .unwrap();
        g.game_turn = 18;
        g.operations_stage = 1;
        g.process_arrivals();
        let div = g.units.values().find(|u| u.name == "Ariete (132nd) Div HQE").unwrap().id;
        let cpa = g.formation_cpa(div).unwrap();

        // Inland from Tripoli reaches Gabes after that leg's §8.89 stage count (Gabes.leg).
        let inland_cost = g.reference.off_map.as_ref().unwrap().node("Gabes").unwrap().leg.stages_for_cpa(cpa);
        let mut s = 0u32;
        loop {
            s += 1;
            assert!(s < 30);
            match g.advance_off_map(div, OffMapDir::Inland).unwrap() {
                OffMapMove::Arrived(n) => {
                    assert_eq!(n, "Gabes");
                    break;
                }
                OffMapMove::InTransit => {}
                OffMapMove::EnteredMap(_) => panic!("inland must not enter the map"),
            }
        }
        assert_eq!(s, inland_cost);
        assert_eq!(
            g.unit(div).unwrap().location.as_ref().unwrap().off_map_node(),
            Some("Gabes")
        );

        // Reversal (§8.84): a unit one stage into the Gabes→Tunis leg, commanded back toward
        // the map, returns to the box it departed (Gabes).
        g.unit_mut(div).unwrap().location =
            Some(UnitLocation::OffMap { node: "Gabes".to_string(), transit: 1, toward_map: false });
        match g.advance_off_map(div, OffMapDir::TowardMap).unwrap() {
            OffMapMove::Arrived(n) => assert_eq!(n, "Gabes"),
            other => panic!("expected return to Gabes, got {other:?}"),
        }
    }

    #[test]
    fn off_map_entry_strings_out_when_hex_full() {
        use crate::state::game::{GameState, OffMapMove};
        use crate::state::map::HexId;
        use crate::state::unit::UnitLocation;

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Italian,
            include_str!("../../../data/reference/unit-characteristics/italian.ron"),
        )
        .unwrap();
        refs.load_order_of_battle(
            Nationality::Italian,
            include_str!("../../../data/ob/italian.ron"),
        )
        .unwrap();
        refs.load_off_map_nodes(include_str!("../../../data/reference/off-map-nodes.ron"))
            .unwrap();
        let mut g = GameState::new(refs, 1);
        g.map
            .load_section(include_str!("../../../data/maps/section-a.ron"))
            .unwrap();

        // Fill the first entry road hex (A2802, Clear, limit 6) with a division (5 SP).
        let a2802 = HexId::parse("A2802").unwrap().to_hex_coord();
        let a2703 = HexId::parse("A2703").unwrap().to_hex_coord();
        g.deploy_formation(Nationality::Italian, "64 Catanzaro Division", a2802).unwrap();
        assert_eq!(g.hex_stacking_points(a2802), 5);

        // Bring Ariete in from Tripolitania (nearest box) and drive it onto the map.
        g.register_reinforcement(
            Nationality::Italian,
            "132 Ariete Armored Division",
            UnitLocation::OffMap { node: "Tripolitania".to_string(), transit: 0, toward_map: true },
            &std::collections::HashMap::new(),
        )
        .unwrap();
        g.game_turn = 18;
        g.operations_stage = 1;
        g.process_arrivals();
        let div = g.units.values().find(|u| u.name == "Ariete (132nd) Div HQE").unwrap().id;
        for _ in 0..30 {
            if let OffMapMove::EnteredMap(_) = g.advance_off_map_toward_map(div).unwrap() {
                break;
            }
        }
        // A2802 was full (5 + 5 > 6), so Ariete strings out to the next road hex, A2703 (§8.85).
        assert_eq!(g.unit(div).unwrap().location, Some(UnitLocation::OnMap(a2703)));
        assert_eq!(g.hex_stacking_points(a2703), 5);
    }

    #[test]
    fn scenario_deploys_ob_placements() {
        use crate::state::game::GameState;
        use crate::state::map::HexId;

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Italian,
            include_str!("../../../data/reference/unit-characteristics/italian.ron"),
        )
        .unwrap();
        refs.load_order_of_battle(
            Nationality::Italian,
            include_str!("../../../data/ob/italian.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);

        let scenario = r#"(
            name: "OB placement test",
            placements: [
                (nationality: Italian, formation: "64 Catanzaro Division", hex: "C4707"),
                (nationality: Italian, formation: "62 Marmarica Division", hex: "C3918"),
            ],
        )"#;
        g.load_scenario(scenario).unwrap();

        let hq = g.units.values().find(|u| u.name == "64th Catanzaro Div HQ").unwrap().id;
        assert!(g.unit(hq).unwrap().is_hq);
        assert_eq!(g.formation_members(hq).len(), 13);
        let hex = HexId::parse("C4707").unwrap().to_hex_coord();
        assert_eq!(g.unit(hq).unwrap().hex(), Some(hex));
        // Both divisions deployed => 26 units total.
        assert_eq!(g.units.len(), 26);
    }

    #[test]
    fn loads_formation_hierarchy() {
        use crate::state::game::GameState;
        use crate::state::unit::OrgSize;

        let mut g = GameState::new(cw_refs(), 1);
        g.load_scenario(include_str!("../../../data/scenarios/example-formation.ron"))
            .unwrap();

        let id = |n: &str| g.units.values().find(|u| u.name == n).unwrap().id;
        let hq = id("Example Division HQ");
        let first = id("1st Battalion");
        let spent = id("Spent Battalion");

        // Membership: HQ + 3 attached battalions.
        assert_eq!(g.formation_members(hq).len(), 4);
        // Attached units carry no own location, but resolve to the parent's hex.
        assert!(g.unit(first).unwrap().hex().is_none());
        assert_eq!(g.effective_hex(first), g.effective_hex(hq));
        assert!(g.effective_hex(first).is_some());
        // §6.15: the formation moves at its slowest member — id "k" (CPA 10) < "h" (CPA 15).
        assert_eq!(g.formation_cpa(hq), Some(10));
        // §9.26/§9.28: the depleted battalion (TOE 2 of max 5) is a shell -> company.
        assert!(g.is_shell(spent));
        assert_eq!(g.effective_org_size(spent), OrgSize::Company);
        assert!(!g.is_shell(first));
        // Attached units add no stacking at the HQ hex; only the Division counter (5 SP).
        let hex = g.unit(hq).unwrap().hex().unwrap();
        assert_eq!(g.hex_stacking_points(hex), 5);
    }

    #[test]
    fn formation_move_uses_formation_cpa_and_carries_attached() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Nationality;
        use crate::rules::movement::MoveMode;
        use crate::state::command::{Command, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, OrgSize};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        g.phase = Phase::MovementAndCombat;

        let from = HexCoord { col: 0, row: 0 };
        let to = HexCoord { col: 1, row: 0 };
        g.map.set_hex(from, HexTerrain::Clear);
        g.map.set_hex(to, HexTerrain::Clear);

        // HQ (CPA 15) + attached "k" (CPA 10) -> formation CPA 10.
        let hq = g.spawn_unit(Nationality::Commonwealth, "h", "HQ", Composition::Organic { current_toe: 5 });
        g.unit_mut(hq).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(from));
        g.unit_mut(hq).unwrap().org_size = OrgSize::StandardBrigade;
        let child = g.spawn_unit(Nationality::Commonwealth, "k", "slow bn", Composition::Organic { current_toe: 5 });
        g.unit_mut(child).unwrap().attached_parent = Some(hq);
        assert_eq!(g.formation_cpa(hq), Some(10));

        // Sit just past the formation CPA (10.0) but well under the HQ's own CPA (15.0), so
        // this step causes Disorganization only because the formation pace governs.
        g.unit_mut(hq).unwrap().cp_expended = Fixed(109);
        let coh_before = g.unit(hq).unwrap().cohesion;

        assert_eq!(
            g.apply(Command::FormationMove { formation: hq, to, mode: MoveMode::NonMotorized }),
            Ok(CommandOutcome::Moved)
        );
        // Parent moved; attached child has no own location but rides along to `to`.
        assert_eq!(g.unit(hq).unwrap().hex(), Some(to));
        assert!(g.unit(child).unwrap().hex().is_none());
        assert_eq!(g.effective_hex(child), Some(to));
        // Exceeding the formation CPA (10) inflicted a Disorganization point (§6.21).
        assert!(g.unit(hq).unwrap().cohesion < coh_before);
    }

    #[test]
    fn empty_hq_zero_sp_and_cannot_attack() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Nationality;
        use crate::state::command::{Command, CommandError};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, OrgSize};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        g.phase = Phase::MovementAndCombat;

        let a = HexCoord { col: 0, row: 0 };
        let b = HexCoord { col: 1, row: 0 };
        g.map.set_hex(a, HexTerrain::Clear);
        g.map.set_hex(b, HexTerrain::Clear);

        // A brigade HQ with nothing attached (§9.12/§3.33): 0 SP and non-combat.
        let hq = g.spawn_unit(Nationality::Commonwealth, "h", "HQ", Composition::Organic { current_toe: 1 });
        g.unit_mut(hq).unwrap().org_size = OrgSize::StandardBrigade;
        g.unit_mut(hq).unwrap().is_hq = true;
        g.unit_mut(hq).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(a));
        assert!(g.is_noncombat_hq(hq));
        assert_eq!(g.effective_stacking_points(hq), 0);

        // §3.33: the empty HQ may not voluntarily attack.
        let foe = g.spawn_unit(Nationality::Commonwealth, "h", "foe", Composition::Organic { current_toe: 4 });
        g.unit_mut(foe).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(b));
        assert_eq!(
            g.apply(Command::CloseAssault { attackers: vec![hq], target: b }),
            Err(CommandError::NonCombatHq)
        );

        // Attach a combat battalion: the HQ now represents a formation — full SP, rule lifts.
        let bn = g.spawn_unit(Nationality::Commonwealth, "h", "bn", Composition::Organic { current_toe: 5 });
        g.unit_mut(bn).unwrap().attached_parent = Some(hq);
        assert!(!g.is_noncombat_hq(hq));
        assert_eq!(g.effective_stacking_points(hq), 2); // StandardBrigade
    }

    #[test]
    fn multi_unit_defender_close_assault() {
        use crate::reference::terrain::HexTerrain;
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, OrgSize};

        let mut refs = close_assault_refs();
        refs.load_unit_characteristics(
            Nationality::Italian,
            include_str!("../../../data/reference/unit-characteristics/italian.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 7);
        g.phase = Phase::MovementAndCombat;
        let atk_hex = HexCoord { col: 0, row: 0 };
        let def_hex = HexCoord { col: 1, row: 0 };
        let friendly_hex = HexCoord { col: -1, row: 0 };
        for h in [atk_hex, def_hex, friendly_hex] {
            g.map.set_hex(h, HexTerrain::Clear);
        }

        let atk = g.spawn_unit(Nationality::Commonwealth, "h", "atk", Composition::Organic { current_toe: 20 });
        g.unit_mut(atk).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(atk_hex));
        // A friendly unit sharing the front (used to check the enemy-only target rule).
        let friend = g.spawn_unit(Nationality::Commonwealth, "h", "friend", Composition::Organic { current_toe: 4 });
        g.unit_mut(friend).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(friendly_hex));

        // Defending Italian formation: brigade HQ + two battalions, co-located at def_hex.
        let hq = g.spawn_unit(Nationality::Italian, "p", "def HQ", Composition::Organic { current_toe: 1 });
        g.unit_mut(hq).unwrap().org_size = OrgSize::StandardBrigade;
        g.unit_mut(hq).unwrap().is_hq = true;
        g.unit_mut(hq).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(def_hex));
        let d1 = g.spawn_unit(Nationality::Italian, "p", "def bn1", Composition::Organic { current_toe: 6 });
        let d2 = g.spawn_unit(Nationality::Italian, "p", "def bn2", Composition::Organic { current_toe: 6 });
        g.unit_mut(d1).unwrap().attached_parent = Some(hq);
        g.unit_mut(d2).unwrap().attached_parent = Some(hq);

        // The attached sub-units resolve to the parent's hex.
        let mut stack = g.units_in_hex(def_hex);
        stack.sort();
        let mut expected = vec![hq, d1, d2];
        expected.sort();
        assert_eq!(stack, expected);

        // Assaulting an adjacent hex with no *enemy* units is rejected.
        assert_eq!(
            g.apply(Command::CloseAssault { attackers: vec![atk], target: friendly_hex }),
            Err(CommandError::NoTargets)
        );

        // Assaulting the defended hex: the whole enemy stack defends together.
        let def_before: u32 = expected.iter().map(|&m| g.unit(m).unwrap().total_toe()).sum();
        match g.apply(Command::CloseAssault { attackers: vec![atk], target: def_hex }) {
            Ok(CommandOutcome::CloseAssault(ca)) => {
                assert!(ca.defender_strength > 0, "3 sub-units aggregate a defense");
                let def_after: u32 = expected.iter().map(|&m| g.unit(m).unwrap().total_toe()).sum();
                assert_eq!(def_after, def_before - ca.defender_toe_removed);
            }
            other => panic!("expected close assault, got {other:?}"),
        }
    }

    #[test]
    fn morale_check_applies_modifier_and_surrender() {
        use crate::reference::crt::MoraleModifier;
        use crate::state::game::GameState;
        use crate::state::unit::Composition;

        let mut refs = ReferenceData::new();
        refs.load_morale_modifier_table(include_str!(
            "../../../data/reference/crt/morale-modifier.ron"
        ))
        .unwrap();
        let mut g = GameState::new(refs, 1);

        let u = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "unit",
            Composition::Organic { current_toe: 8 },
        );

        // Level 0, roll 66 -> Delta(-1); no surrender, TOE intact.
        let out = g.morale_check_with_dice(u, 0, 66);
        assert_eq!(out.modifier, MoraleModifier::Delta(-1));
        assert_eq!(out.morale_delta, -1);
        assert!(!out.surrendered);
        assert_eq!(g.unit(u).unwrap().total_toe(), 8);

        // Level +8, roll 11 -> Delta(4).
        assert_eq!(g.morale_check_with_dice(u, 8, 11).morale_delta, 4);

        // Level -17, roll 11 -> Surrender: TOE eliminated, flagged.
        let out = g.morale_check_with_dice(u, -17, 11);
        assert!(out.surrendered);
        assert_eq!(out.morale_delta, 0);
        assert_eq!(g.unit(u).unwrap().total_toe(), 0);
        assert!(g.unit(u).unwrap().status.surrendered);
    }

    fn combat_refs() -> ReferenceData {
        let mut refs = ReferenceData::new();
        refs.load_weapons_systems(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/weapons/commonwealth.ron"),
        )
        .unwrap();
        refs.load_weapons_systems(
            Nationality::Italian,
            include_str!("../../../data/reference/weapons/italian.ron"),
        )
        .unwrap();
        refs.load_anti_armor_crt(include_str!("../../../data/reference/crt/anti-armor.ron"))
            .unwrap();
        refs
    }

    #[test]
    fn anti_armor_resolver_removes_target_armor() {
        use crate::state::game::GameState;
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, WeaponGroup};

        let mut g = GameState::new(combat_refs(), 1);
        let target_hex = HexCoord::new(0, 0);

        // Attacker: 5× 6-pounder (anti-armor 7) -> raw 35 -> actual 4.
        let attacker = g.spawn_unit(
            Nationality::Commonwealth,
            "gun",
            "AT bn",
            Composition::Weapons {
                groups: vec![WeaponGroup { system: "6-pounder".into(), count: 5 }],
            },
        );
        // Target: 4× M 13/40 (armor protection 3).
        let target = g.spawn_unit(
            Nationality::Italian,
            "tank",
            "tank bn",
            Composition::Weapons {
                groups: vec![WeaponGroup { system: "M 13/40".into(), count: 4 }],
            },
        );
        g.unit_mut(target).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(target_hex));

        // Dice 66 (row 65,66); the phasing Commonwealth attacker fires one row lower (§14.6),
        // row 63,64 — col 4 -> 9 Damage Points on both rows here; AP 3 -> ceil(9/3)=3 TOE removed.
        let out = g.resolve_anti_armor_with_dice(&[attacker], target_hex, 66);
        assert_eq!(out.raw_points, 35);
        assert_eq!(out.actual_points, 4);
        assert_eq!(out.dice, 66);
        assert!(out.phasing_penalty, "§14.6: phasing player fires one row lower");
        assert_eq!(out.damage, 9);
        assert_eq!(out.armor_toe_removed, 3);

        let remaining = match &g.unit(target).unwrap().composition {
            Composition::Weapons { groups } => groups[0].count,
            _ => unreachable!(),
        };
        assert_eq!(remaining, 1);
    }

    #[test]
    fn non_phasing_defensive_anti_armor_14_12() {
        use crate::reference::units::Side;
        use crate::state::command::{Command, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation, WeaponGroup};

        // §14.12: the non-phasing player may fire Anti-Armor at the phasing armor assaulting it.
        // The AntiArmor command is symmetric (not gated to the phasing player), so a non-phasing
        // defender simply issues it against the phasing armor's hex.
        let mut g = GameState::new(combat_refs(), 3);
        g.phase = Phase::MovementAndCombat;
        g.active_player = Side::Axis; // Axis is phasing; the Commonwealth gun fires defensively
        let atk_hex = HexCoord::new(0, 0);
        let def_hex = HexCoord::new(1, 0);
        g.map.set_hex(atk_hex, HexTerrain::Clear);
        g.map.set_hex(def_hex, HexTerrain::Clear);
        // Phasing Axis tanks in def_hex; a non-phasing CW anti-tank gun adjacent in atk_hex.
        let tanks = g.spawn_unit(Nationality::Italian, "tank", "M13", Composition::Weapons { groups: vec![WeaponGroup { system: "M 13/40".into(), count: 4 }] });
        g.unit_mut(tanks).unwrap().location = Some(UnitLocation::OnMap(def_hex));
        let gun = g.spawn_unit(Nationality::Commonwealth, "x", "AT", Composition::Weapons { groups: vec![WeaponGroup { system: "6-pounder".into(), count: 5 }] });
        {
            let u = g.unit_mut(gun).unwrap();
            u.location = Some(UnitLocation::OnMap(atk_hex));
            u.ammo = 20;
        }
        let before = match &g.unit(tanks).unwrap().composition { Composition::Weapons { groups } => groups[0].count, _ => unreachable!() };
        let out = g.apply(Command::AntiArmor { attackers: vec![gun], target_hex: def_hex, withheld: vec![] });
        assert!(matches!(out, Ok(CommandOutcome::AntiArmor(_))), "non-phasing AA fire resolves: {out:?}");
        let after = match &g.unit(tanks).unwrap().composition { Composition::Weapons { groups } => groups[0].count, _ => unreachable!() };
        assert!(after < before, "§14.12 defensive fire damaged the phasing armor");
    }

    #[test]
    fn anti_armor_withheld_armor_immune_14_21() {
        use crate::reference::units::Side;
        use crate::state::game::GameState;
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation, WeaponGroup};

        // Two Italian tank units in the target hex; the defender withholds one (§14.21), so only
        // the committed unit takes Anti-Armor damage.
        let mut g = GameState::new(combat_refs(), 1);
        g.active_player = Side::Axis; // CW attacker is non-phasing (no §14.6 penalty)
        let hex = HexCoord::new(0, 0);
        let atk = g.spawn_unit(
            Nationality::Commonwealth, "gun", "AT bn",
            Composition::Weapons { groups: vec![WeaponGroup { system: "6-pounder".into(), count: 5 }] },
        );
        let committed = g.spawn_unit(Nationality::Italian, "tank", "committed", Composition::Weapons { groups: vec![WeaponGroup { system: "M 13/40".into(), count: 4 }] });
        let withheld = g.spawn_unit(Nationality::Italian, "tank", "withheld", Composition::Weapons { groups: vec![WeaponGroup { system: "M 13/40".into(), count: 4 }] });
        for id in [committed, withheld] {
            g.unit_mut(id).unwrap().location = Some(UnitLocation::OnMap(hex));
        }

        // With `withheld` excluded, dice 66 (9 damage, AP 3) removes 3 TOE — all from the committed
        // unit; the withheld unit is untouched.
        let out = g.resolve_anti_armor_core(&[atk], hex, 66, &[withheld]);
        assert_eq!(out.armor_toe_removed, 3);
        let committed_left = match &g.unit(committed).unwrap().composition {
            Composition::Weapons { groups } => groups[0].count, _ => unreachable!(),
        };
        let withheld_left = match &g.unit(withheld).unwrap().composition {
            Composition::Weapons { groups } => groups[0].count, _ => unreachable!(),
        };
        assert_eq!(committed_left, 1, "committed unit absorbed all 3 losses");
        assert_eq!(withheld_left, 4, "§14.21 withheld unit is immune to Anti-Armor fire");
    }

    #[test]
    fn destroyed_tank_markers_and_capture_14_46() {
        use crate::reference::units::Side;
        use crate::state::game::GameState;
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation, WeaponGroup};

        let mut g = GameState::new(combat_refs(), 1);
        g.active_player = Side::Axis; // CW gun is the non-phasing firer (no §14.6 row penalty)
        let hex = HexCoord::new(0, 0);
        let atk = g.spawn_unit(
            Nationality::Commonwealth, "gun", "AT bn",
            Composition::Weapons { groups: vec![WeaponGroup { system: "6-pounder".into(), count: 5 }] },
        );
        let _ = atk;
        let target = g.spawn_unit(
            Nationality::Italian, "tank", "tank bn",
            Composition::Weapons { groups: vec![WeaponGroup { system: "M 13/40".into(), count: 4 }] },
        );
        g.unit_mut(target).unwrap().location = Some(UnitLocation::OnMap(hex));

        // Dice 66, col 4 -> 9 Damage; AP 3 -> 3 TOE removed -> 3 Destroyed Tank Markers (§14.46),
        // owned by the tanks' side (Axis) until captured.
        let out = g.resolve_anti_armor_with_dice(&[atk], hex, 66);
        assert_eq!(out.armor_toe_removed, 3);
        let marker = g.captured_equipment.iter().find(|e| e.destroyed_tank).expect("a destroyed-tank marker");
        assert_eq!(marker.captor, Side::Axis, "§14.46 markers start owned by the tank's side");
        assert_eq!(marker.system, "M 13/40");
        assert_eq!(marker.count, 3);

        // §14.52: a Commonwealth unit now controls the hex (no Axis unit present) — the markers are
        // captured (captor flips to Commonwealth).
        g.unit_mut(target).unwrap().location = None; // the destroyed Italian unit leaves the hex
        let cw = g.spawn_unit(Nationality::Commonwealth, "h", "inf", Composition::Organic { current_toe: 4 });
        g.unit_mut(cw).unwrap().location = Some(UnitLocation::OnMap(hex));
        g.resolve_destroyed_tank_capture();
        let marker = g.captured_equipment.iter().find(|e| e.destroyed_tank).expect("marker persists");
        assert_eq!(marker.captor, Side::Commonwealth, "§14.52 enemy control captures the markers");
    }

    #[test]
    fn anti_armor_phasing_row_penalty_14_6() {
        use crate::reference::crt::AntiArmorCrt;
        use crate::reference::units::Side;
        use crate::state::game::GameState;
        use crate::state::unit::{Composition, UnitLocation, WeaponGroup};
        use crate::state::map::HexCoord;

        // The row-shift helper: down one row, top row (11,12) unaffected.
        assert_eq!(AntiArmorCrt::phasing_row_dice(66), 63); // row 65,66 -> row 63,64
        assert_eq!(AntiArmorCrt::phasing_row_dice(15), 13); // row 15,16 -> row 13,14
        assert_eq!(AntiArmorCrt::phasing_row_dice(12), 11); // row 11,12 -> itself
        assert_eq!(AntiArmorCrt::phasing_row_dice(11), 11);

        let mut g = GameState::new(combat_refs(), 1);
        let hex = HexCoord::new(0, 0);
        // 3× 6-pounder (anti-armor 7) -> raw 21 -> actual 2.
        let atk = g.spawn_unit(
            Nationality::Commonwealth, "gun", "AT bn",
            Composition::Weapons { groups: vec![WeaponGroup { system: "6-pounder".into(), count: 3 }] },
        );
        let make_target = |g: &mut GameState| {
            let t = g.spawn_unit(
                Nationality::Italian, "tank", "tank bn",
                Composition::Weapons { groups: vec![WeaponGroup { system: "M 13/40".into(), count: 4 }] },
            );
            g.unit_mut(t).unwrap().location = Some(UnitLocation::OnMap(hex));
            t
        };
        let target = make_target(&mut g);

        // Non-phasing (active player is the Axis, so the CW attacker is the non-phasing firer):
        // dice 15, row 15,16, col 2 -> 1 Damage Point, removes 1 TOE. No row penalty.
        g.active_player = Side::Axis;
        let np = g.resolve_anti_armor_with_dice(&[atk], hex, 15);
        assert!(!np.phasing_penalty);
        assert_eq!(np.actual_points, 2);
        assert_eq!(np.damage, 1);
        assert_eq!(np.armor_toe_removed, 1);

        // Phasing (active player is the Commonwealth firer): dice 15 fires one row lower,
        // row 13,14 col 2 -> 0 Damage Points, so no TOE is lost.
        let mut g2 = GameState::new(combat_refs(), 1);
        g2.active_player = Side::Commonwealth;
        let atk2 = g2.spawn_unit(
            Nationality::Commonwealth, "gun", "AT bn",
            Composition::Weapons { groups: vec![WeaponGroup { system: "6-pounder".into(), count: 3 }] },
        );
        let _t2 = make_target(&mut g2);
        let ph = g2.resolve_anti_armor_with_dice(&[atk2], hex, 15);
        assert!(ph.phasing_penalty);
        assert_eq!(ph.damage, 0, "§14.6 phasing row shift drops damage below the threshold");
        assert_eq!(ph.armor_toe_removed, 0);

        let _ = target;
    }

    #[test]
    fn anti_armor_roll_is_deterministic() {
        use crate::state::combat::AntiArmorOutcome;
        use crate::state::game::GameState;
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, WeaponGroup};

        fn run() -> AntiArmorOutcome {
            let mut g = GameState::new(combat_refs(), 7);
            let hex = HexCoord::new(0, 0);
            let atk = g.spawn_unit(
                Nationality::Commonwealth,
                "gun",
                "AT bn",
                Composition::Weapons {
                    groups: vec![WeaponGroup { system: "6-pounder".into(), count: 5 }],
                },
            );
            let tgt = g.spawn_unit(
                Nationality::Italian,
                "tank",
                "tank bn",
                Composition::Weapons {
                    groups: vec![WeaponGroup { system: "M 13/40".into(), count: 4 }],
                },
            );
            g.unit_mut(tgt).unwrap().location = Some(crate::state::unit::UnitLocation::OnMap(hex));
            g.resolve_anti_armor(&[atk], hex)
        }
        assert_eq!(run(), run());
    }

    #[test]
    fn anti_armor_terrain_column_shift() {
        use crate::reference::terrain::HexTerrain;
        use crate::state::combat::AntiArmorOutcome;
        use crate::state::game::GameState;
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation, WeaponGroup};

        fn setup(terrain: HexTerrain) -> AntiArmorOutcome {
            let mut refs = combat_refs();
            refs.load_terrain(TERRAIN_RON).unwrap();
            let mut g = GameState::new(refs, 7);
            let (atk_hex, tgt_hex) = (HexCoord::new(0, 0), HexCoord::new(1, 0));
            g.map.set_hex(atk_hex, HexTerrain::Clear);
            g.map.set_hex(tgt_hex, terrain);
            let atk = g.spawn_unit(
                Nationality::Commonwealth,
                "gun",
                "AT bn",
                Composition::Weapons { groups: vec![WeaponGroup { system: "6-pounder".into(), count: 10 }] },
            );
            g.unit_mut(atk).unwrap().location = Some(UnitLocation::OnMap(atk_hex));
            let tgt = g.spawn_unit(
                Nationality::Italian,
                "tank",
                "tank bn",
                Composition::Weapons { groups: vec![WeaponGroup { system: "M 13/40".into(), count: 6 }] },
            );
            g.unit_mut(tgt).unwrap().location = Some(UnitLocation::OnMap(tgt_hex));
            g.resolve_anti_armor_with_dice(&[atk], tgt_hex, 44)
        }
        let clear = setup(HexTerrain::Clear);
        let rough = setup(HexTerrain::Rough);
        // Clear terrain: no shift.
        assert_eq!(clear.terrain_shift, 0);
        assert_eq!(clear.column, clear.actual_points);
        // §14.32: armor in Rough shifts the Anti-Armor column down one in the defender's favour.
        assert_eq!(rough.terrain_shift, -1);
        assert_eq!(rough.column, (clear.actual_points - 1).max(0));
    }

    #[test]
    fn fortification_levels() {
        use crate::reference::terrain::HexTerrain;
        use crate::state::game::GameState;
        use crate::state::map::{HexCoord, HexId};

        let mut g = GameState::new(ReferenceData::new(), 1);
        let plain = HexCoord::new(0, 0);
        let city = HexCoord::new(1, 0);
        let cairo = HexId::parse("E1430").unwrap().to_hex_coord();
        g.map.set_hex(plain, HexTerrain::Clear);
        g.map.set_hex(city, HexTerrain::MajorCity);
        g.map.set_hex(cairo, HexTerrain::MajorCity);
        // §25.12 static levels: Major City = 2, Cairo/Alexandria = 3.
        assert_eq!(g.fort_level(plain), 0);
        assert_eq!(g.fort_level(city), 2);
        assert_eq!(g.fort_level(cairo), 3);
        // A placed fort on open ground; fort_shift = −level.
        g.fortifications.insert(plain, 1);
        assert_eq!(g.fort_level(plain), 1);
        assert_eq!(g.fort_shift(plain), -1);
        // A placed level below the static city level: the static level wins.
        g.fortifications.insert(city, 1);
        assert_eq!(g.fort_level(city), 2);
    }

    #[test]
    fn fortification_combat_shifts() {
        use crate::reference::terrain::HexTerrain;
        use crate::state::game::GameState;
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation, WeaponGroup};

        let mut refs = combat_refs();
        refs.load_terrain(TERRAIN_RON).unwrap();
        let mut g = GameState::new(refs, 7);
        let (ah, dh) = (HexCoord::new(0, 0), HexCoord::new(1, 0));
        g.map.set_hex(ah, HexTerrain::Clear);
        g.map.set_hex(dh, HexTerrain::Rough); // Rough: close_assault −2, anti_armor −1
        g.fortifications.insert(dh, 2); // Level 2 fort → −2
        let atk = g.spawn_unit(Nationality::Commonwealth, "h", "inf", Composition::Organic { current_toe: 5 });
        g.unit_mut(atk).unwrap().location = Some(UnitLocation::OnMap(ah));
        let def = g.spawn_unit(
            Nationality::Italian,
            "tank",
            "tk",
            Composition::Weapons { groups: vec![WeaponGroup { system: "M 13/40".into(), count: 5 }] },
        );
        g.unit_mut(def).unwrap().location = Some(UnitLocation::OnMap(dh));

        // §15.35 close assault: fort is CUMULATIVE with terrain → Rough(−2) + Fort L2(−2) = −4.
        let ca = g.resolve_close_assault_with_rolls(&[atk], &[def], 33, 33, 33, 33);
        assert_eq!(ca.terrain_shift, -4);

        // §14.33 anti-armor: fort is NOT cumulative with terrain → best of Rough(−1), Fort(−2) = −2.
        let gun = g.spawn_unit(
            Nationality::Commonwealth,
            "gun",
            "at",
            Composition::Weapons { groups: vec![WeaponGroup { system: "6-pounder".into(), count: 5 }] },
        );
        g.unit_mut(gun).unwrap().location = Some(UnitLocation::OnMap(ah));
        let aa = g.resolve_anti_armor_with_dice(&[gun], dh, 44);
        assert_eq!(aa.terrain_shift, -2);
    }

    #[test]
    fn anti_armor_fire_prohibition() {
        use crate::reference::terrain::HexTerrain;
        use crate::state::game::GameState;
        use crate::state::map::{HexCoord, HexsideKind};
        use crate::state::unit::{Composition, UnitLocation, WeaponGroup};

        // §14.34 hexside legality (no refs needed — just the map).
        let mut g = GameState::new(ReferenceData::new(), 1);
        let (a, b) = (HexCoord::new(0, 0), HexCoord::new(1, 0));
        g.map.set_hexside(a, b, HexsideKind::Ridge, None);
        assert!(g.anti_armor_fire_blocked(a, b));
        assert!(g.anti_armor_fire_blocked(b, a)); // ridge is non-directional
        g.map.set_hexside(a, b, HexsideKind::Road, None); // a road negates the ridge
        assert!(!g.anti_armor_fire_blocked(a, b));

        let (c, d) = (HexCoord::new(0, 1), HexCoord::new(1, 1));
        g.map.set_hexside(c, d, HexsideKind::Slope, Some(c)); // c is the high side
        assert!(g.anti_armor_fire_blocked(c, d)); // firing DOWN the slope → blocked
        assert!(!g.anti_armor_fire_blocked(d, c)); // firing UP → allowed

        let (e, f) = (HexCoord::new(0, 2), HexCoord::new(1, 2));
        g.map.set_hexside(e, f, HexsideKind::Escarpment, Some(e));
        g.map.set_hexside(e, f, HexsideKind::Road, None); // a road does NOT help an escarpment
        assert!(g.anti_armor_fire_blocked(e, f));
        g.map.set_hexside(e, f, HexsideKind::Track, None); // only a Track negates it
        assert!(!g.anti_armor_fire_blocked(e, f));

        // Integration: an AT unit firing across a Ridge contributes no Raw points.
        let mut refs = combat_refs();
        refs.load_terrain(TERRAIN_RON).unwrap();
        let mut g2 = GameState::new(refs, 7);
        let (ah, dh) = (HexCoord::new(0, 0), HexCoord::new(1, 0));
        g2.map.set_hex(ah, HexTerrain::Clear);
        g2.map.set_hex(dh, HexTerrain::Clear);
        let gun = g2.spawn_unit(
            Nationality::Commonwealth,
            "gun",
            "at",
            Composition::Weapons { groups: vec![WeaponGroup { system: "6-pounder".into(), count: 5 }] },
        );
        g2.unit_mut(gun).unwrap().location = Some(UnitLocation::OnMap(ah));
        let tank = g2.spawn_unit(
            Nationality::Italian,
            "tank",
            "tk",
            Composition::Weapons { groups: vec![WeaponGroup { system: "M 13/40".into(), count: 5 }] },
        );
        g2.unit_mut(tank).unwrap().location = Some(UnitLocation::OnMap(dh));
        assert!(g2.resolve_anti_armor_with_dice(&[gun], dh, 44).raw_points > 0);
        g2.map.set_hexside(ah, dh, HexsideKind::Ridge, None);
        assert_eq!(g2.resolve_anti_armor_with_dice(&[gun], dh, 44).raw_points, 0);
    }

    #[test]
    fn build_road_from_unfinished() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::game::{Commodities, GameState, SupplyDump};
        use crate::state::map::{HexCoord, HexsideKind};
        use crate::state::unit::{Composition, OrgSize, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let (a, b) = (HexCoord::new(0, 0), HexCoord::new(1, 0));
        g.map.set_hex(a, HexTerrain::Clear);
        g.map.set_hex(b, HexTerrain::Clear);
        g.map.set_hexside(a, b, HexsideKind::UnfinishedRoad, None);
        let inf = g.spawn_unit(Nationality::Commonwealth, "h", "inf", Composition::Organic { current_toe: 4 });
        g.unit_mut(inf).unwrap().org_size = OrgSize::Battalion;
        g.unit_mut(inf).unwrap().location = Some(UnitLocation::OnMap(a));
        g.supply_dumps.push(SupplyDump {
            location: UnitLocation::OnMap(a),
            side: Side::Commonwealth,
            supply: Commodities { fuel: 0, ammo: 0, stores: 2, water: 0 },
            dummy: false,
        });

        // §24.5: the Infantry Bn builds the road, expending 2 Stores; UnfinishedRoad becomes Road.
        assert!(g.build_road(inf, a, b));
        let kinds: Vec<_> = g.map.hexsides_on(a, b).iter().map(|h| h.kind).collect();
        assert!(kinds.contains(&HexsideKind::Road));
        assert!(!kinds.contains(&HexsideKind::UnfinishedRoad));
        assert_eq!(g.supply_dumps[0].supply.stores, 0);

        // Nothing left to build (no UnfinishedRoad, and no Stores).
        assert!(!g.build_road(inf, a, b));
    }

    #[test]
    fn build_fortification_multistage() {
        use crate::reference::terrain::{Fixed, HexTerrain};
        use crate::reference::units::Side;
        use crate::state::game::{Commodities, GameState, SupplyDump};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, OrgSize, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let hex = HexCoord::new(0, 0);
        g.map.set_hex(hex, HexTerrain::Clear);
        let hq = g.spawn_unit(Nationality::Commonwealth, "a", "hq", Composition::Organic { current_toe: 0 }); // Headquarters = engineer-capable
        g.unit_mut(hq).unwrap().location = Some(UnitLocation::OnMap(hex));
        let inf = g.spawn_unit(Nationality::Commonwealth, "h", "inf", Composition::Organic { current_toe: 4 });
        g.unit_mut(inf).unwrap().org_size = OrgSize::Battalion;
        g.unit_mut(inf).unwrap().location = Some(UnitLocation::OnMap(hex));
        g.supply_dumps.push(SupplyDump {
            location: UnitLocation::OnMap(hex),
            side: Side::Commonwealth,
            supply: Commodities { fuel: 0, ammo: 0, stores: 30, water: 0 },
            dummy: false,
        });

        // §24.4: start a Level 2 fort — 30 Stores expended immediately.
        assert!(g.begin_fortification(&[hq, inf], hex, 2));
        assert_eq!(g.supply_dumps[0].supply.stores, 0);

        // §24.42: needs 3 clean Construction Segments — two is not enough.
        g.advance_construction();
        g.advance_construction();
        assert_eq!(g.fort_level(hex), 0);

        // §24.12: a builder that expends CP halts the stage (it does not count).
        g.unit_mut(inf).unwrap().cp_expended = Fixed(10);
        g.advance_construction();
        assert_eq!(g.fort_level(hex), 0);

        // A third clean segment completes the Level 2 fort and clears the project.
        g.unit_mut(inf).unwrap().cp_expended = Fixed(0);
        g.advance_construction();
        assert_eq!(g.fort_level(hex), 2);
        assert!(g.construction.is_empty());
    }

    #[test]
    fn build_railroad_extends_railhead() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::game::{Commodities, GameState, SupplyDump};
        use crate::state::map::{HexCoord, HexsideKind};
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let (w, x, y, y2) = (HexCoord::new(0, 0), HexCoord::new(1, 0), HexCoord::new(2, 0), HexCoord::new(3, 0));
        for h in [w, x, y, y2] {
            g.map.set_hex(h, HexTerrain::Clear);
        }
        g.map.set_hexside(w, x, HexsideKind::Railroad, None); // existing railhead touches x
        g.map.set_hexside(x, y, HexsideKind::UnfinishedRailroad, None);
        let store = |g: &mut GameState, h| {
            g.supply_dumps.push(SupplyDump {
                location: UnitLocation::OnMap(h),
                side: Side::Commonwealth,
                supply: Commodities { fuel: 0, ammo: 0, stores: 1, water: 0 },
                dummy: false,
            });
        };
        let nz = |g: &mut GameState, h| {
            let u = g.spawn_unit(Nationality::Commonwealth, "uu", "nzrrc", Composition::Organic { current_toe: 1 });
            g.unit_mut(u).unwrap().location = Some(UnitLocation::OnMap(h));
            u
        };

        // §24.62: one rail-construction company → two Construction Segments.
        let c1 = nz(&mut g, x);
        store(&mut g, x);
        assert!(g.begin_railroad(&[c1], x, y));
        assert_eq!(g.supply_dumps[0].supply.stores, 0);
        g.advance_construction();
        assert!(g.map.hexsides_on(x, y).iter().any(|h| h.kind == HexsideKind::UnfinishedRailroad));
        g.advance_construction();
        let kinds: Vec<_> = g.map.hexsides_on(x, y).iter().map(|h| h.kind).collect();
        assert!(kinds.contains(&HexsideKind::Railroad) && !kinds.contains(&HexsideKind::UnfinishedRailroad));

        // §24.62: two companies → one Construction Segment (the railhead is now at y).
        g.map.set_hexside(y, y2, HexsideKind::UnfinishedRailroad, None);
        let a1 = nz(&mut g, y);
        let a2 = nz(&mut g, y);
        store(&mut g, y);
        assert!(g.begin_railroad(&[a1, a2], y, y2));
        g.advance_construction();
        assert!(g.map.hexsides_on(y, y2).iter().any(|h| h.kind == HexsideKind::Railroad));

        // §24.67: cannot build a stretch not contiguous with the railhead (no skipping).
        let (p, q) = (HexCoord::new(20, 20), HexCoord::new(21, 20));
        g.map.set_hex(p, HexTerrain::Clear);
        g.map.set_hex(q, HexTerrain::Clear);
        g.map.set_hexside(p, q, HexsideKind::UnfinishedRailroad, None);
        let c3 = nz(&mut g, p);
        store(&mut g, p);
        assert!(!g.begin_railroad(&[c3], p, q));
    }

    #[test]
    fn destroy_and_rebuild_railroad() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::game::{Commodities, GameState, SupplyDump, WaterKind};
        use crate::state::map::{HexCoord, HexsideKind};
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let (a, b) = (HexCoord::new(0, 0), HexCoord::new(1, 0));
        g.map.set_hex(a, HexTerrain::Clear);
        g.map.set_hex(b, HexTerrain::Clear);
        g.map.set_hexside(a, b, HexsideKind::Railroad, None);
        assert!(g.hex_is_railroad(a));
        // The built rail feeds the CW water pipeline (§52.22).
        assert_eq!(g.hex_water_source_for(a, Side::Commonwealth), Some(WaterKind::Unlimited));

        // §24.66: destroy the track — it goes inert (no longer a rail hex, no rail-water).
        assert!(g.destroy_railroad(a, b));
        let kinds: Vec<_> = g.map.hexsides_on(a, b).iter().map(|h| h.kind).collect();
        assert!(kinds.contains(&HexsideKind::DestroyedRailroad) && !kinds.contains(&HexsideKind::Railroad));
        assert!(!g.hex_is_railroad(a));
        assert_eq!(g.hex_water_source_for(a, Side::Commonwealth), None);

        // §24.63: an engineer/construction unit rebuilds it, expending 1 Store.
        let eng = g.spawn_unit(Nationality::Commonwealth, "uu", "eng", Composition::Organic { current_toe: 1 });
        g.unit_mut(eng).unwrap().location = Some(UnitLocation::OnMap(a));
        g.supply_dumps.push(SupplyDump {
            location: UnitLocation::OnMap(a),
            side: Side::Commonwealth,
            supply: Commodities { fuel: 0, ammo: 0, stores: 1, water: 0 },
            dummy: false,
        });
        assert!(g.rebuild_railroad(eng, a, b));
        assert!(g.hex_is_railroad(a));
        assert_eq!(g.supply_dumps[0].supply.stores, 0);

        // Destroying a non-railroad edge does nothing.
        let (c, d) = (HexCoord::new(0, 1), HexCoord::new(1, 1));
        g.map.set_hex(c, HexTerrain::Clear);
        g.map.set_hex(d, HexTerrain::Clear);
        assert!(!g.destroy_railroad(c, d));
    }

    #[test]
    fn railroad_ground_demolition() {
        use crate::reference::terrain::{Fixed, HexTerrain};
        use crate::state::game::GameState;
        use crate::state::map::{HexCoord, HexsideKind};
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let (a, b) = (HexCoord::new(0, 0), HexCoord::new(1, 0));
        g.map.set_hex(a, HexTerrain::Clear);
        g.map.set_hex(b, HexTerrain::Clear);
        g.map.set_hexside(a, b, HexsideKind::Railroad, None);

        // §24.66: an Infantry unit of 3+ TOE spends one clean Operations Stage to demolish.
        let inf = g.spawn_unit(Nationality::Commonwealth, "h", "inf", Composition::Organic { current_toe: 4 });
        g.unit_mut(inf).unwrap().location = Some(UnitLocation::OnMap(a));
        assert!(g.begin_railroad_demolition(inf, a, b));

        // Expending CP that stage halts it — the track survives.
        g.unit_mut(inf).unwrap().cp_expended = Fixed(10);
        g.advance_construction();
        assert!(g.hex_is_railroad(a));

        // A clean stage completes the demolition.
        g.unit_mut(inf).unwrap().cp_expended = Fixed(0);
        g.advance_construction();
        assert!(!g.hex_is_railroad(a));
        assert!(g.map.hexsides_on(a, b).iter().any(|h| h.kind == HexsideKind::DestroyedRailroad));

        // A 2-TOE infantry unit is not eligible (needs 3+).
        let (c, d) = (HexCoord::new(0, 1), HexCoord::new(1, 1));
        g.map.set_hex(c, HexTerrain::Clear);
        g.map.set_hex(d, HexTerrain::Clear);
        g.map.set_hexside(c, d, HexsideKind::Railroad, None);
        let weak = g.spawn_unit(Nationality::Commonwealth, "h", "weak", Composition::Organic { current_toe: 2 });
        g.unit_mut(weak).unwrap().location = Some(UnitLocation::OnMap(c));
        assert!(!g.begin_railroad_demolition(weak, c, d));
    }

    #[test]
    fn lay_and_clear_minefield() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::game::{Commodities, GameState, Minefield, SupplyDump};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let hex = HexCoord::new(0, 0);
        g.map.set_hex(hex, HexTerrain::Clear);
        let eng = g.spawn_unit(Nationality::Commonwealth, "a", "hq", Composition::Organic { current_toe: 0 }); // HQ = engineering
        g.unit_mut(eng).unwrap().location = Some(UnitLocation::OnMap(hex));
        g.supply_dumps.push(SupplyDump {
            location: UnitLocation::OnMap(hex),
            side: Side::Commonwealth,
            supply: Commodities { fuel: 0, ammo: 15, stores: 30, water: 0 },
            dummy: false,
        });

        // §24.33: a real minefield needs 15 Stores + 15 Ammo, expended at the start.
        assert!(g.begin_minefield(eng, hex, false));
        assert_eq!(g.supply_dumps[0].supply.stores, 15);
        assert_eq!(g.supply_dumps[0].supply.ammo, 0);
        assert!(!g.minefields.contains_key(&hex)); // §24.32: takes one full stage
        g.advance_construction();
        assert_eq!(g.minefields.get(&hex), Some(&Minefield { side: Side::Commonwealth, dummy: false }));

        // §26.13: an engineer clears it in one clean stage.
        assert!(g.begin_minefield_removal(eng, hex));
        g.advance_construction();
        assert!(!g.minefields.contains_key(&hex));

        // §24.34: a dummy needs only 3 Stores and no Ammo.
        assert!(g.begin_minefield(eng, hex, true));
        assert_eq!(g.supply_dumps[0].supply.stores, 12);
        g.advance_construction();
        assert!(g.minefields.get(&hex).unwrap().dummy);

        // §24.35: minefields may not be laid in Mountain terrain.
        let m = HexCoord::new(5, 5);
        g.map.set_hex(m, HexTerrain::Mountain);
        let eng2 = g.spawn_unit(Nationality::Commonwealth, "a", "hq2", Composition::Organic { current_toe: 0 });
        g.unit_mut(eng2).unwrap().location = Some(UnitLocation::OnMap(m));
        g.supply_dumps.push(SupplyDump {
            location: UnitLocation::OnMap(m),
            side: Side::Commonwealth,
            supply: Commodities { fuel: 0, ammo: 15, stores: 15, water: 0 },
            dummy: false,
        });
        assert!(!g.begin_minefield(eng2, m, false));
    }

    #[test]
    fn enemy_minefield_movement_cost() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::rules::movement::MoveMode;
        use crate::state::command::{Command, CommandOutcome};
        use crate::state::game::{GameState, Minefield, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        g.phase = Phase::MovementAndCombat;
        let clear = |g: &mut GameState, h| g.map.set_hex(h, HexTerrain::Clear);
        let mv = |g: &mut GameState, u, to| g.apply(Command::MoveStep { unit: u, to, mode: MoveMode::NonMotorized });

        // (1) Non-engineer into an ENEMY minefield: Clear entry (2 CP) + full CPA (§26.21/22).
        let (a, b) = (HexCoord::new(0, 0), HexCoord::new(1, 0));
        clear(&mut g, a);
        clear(&mut g, b);
        g.minefields.insert(b, Minefield { side: Side::Axis, dummy: false });
        let inf = g.spawn_unit(Nationality::Commonwealth, "h", "inf", Composition::Organic { current_toe: 4 });
        g.unit_mut(inf).unwrap().location = Some(UnitLocation::OnMap(a));
        let cpa = g.unit_cpa(inf).unwrap();
        assert_eq!(mv(&mut g, inf, b), Ok(CommandOutcome::Moved));
        assert_eq!(g.unit(inf).unwrap().cp_expended.0, 20 + cpa * 10);

        // (2) An Engineer/HQ into an ENEMY minefield pays only +4 CP (§26.24).
        let (c, d) = (HexCoord::new(0, 1), HexCoord::new(1, 1));
        clear(&mut g, c);
        clear(&mut g, d);
        g.minefields.insert(d, Minefield { side: Side::Axis, dummy: false });
        let hq = g.spawn_unit(Nationality::Commonwealth, "a", "hq", Composition::Organic { current_toe: 0 });
        g.unit_mut(hq).unwrap().location = Some(UnitLocation::OnMap(c));
        assert_eq!(mv(&mut g, hq, d), Ok(CommandOutcome::Moved));
        assert_eq!(g.unit(hq).unwrap().cp_expended.0, 20 + 40);

        // (3) A FRIENDLY minefield costs nothing extra (known paths, §26.12).
        let (e, f) = (HexCoord::new(0, 2), HexCoord::new(1, 2));
        clear(&mut g, e);
        clear(&mut g, f);
        g.minefields.insert(f, Minefield { side: Side::Commonwealth, dummy: false });
        let inf2 = g.spawn_unit(Nationality::Commonwealth, "h", "inf2", Composition::Organic { current_toe: 4 });
        g.unit_mut(inf2).unwrap().location = Some(UnitLocation::OnMap(e));
        assert_eq!(mv(&mut g, inf2, f), Ok(CommandOutcome::Moved));
        assert_eq!(g.unit(inf2).unwrap().cp_expended.0, 20);
    }

    #[test]
    fn dummy_minefield_reveal_and_removal() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::rules::movement::MoveMode;
        use crate::state::command::{Command, CommandOutcome};
        use crate::state::game::{GameState, Minefield, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        g.phase = Phase::MovementAndCombat;
        let (a, b) = (HexCoord::new(0, 0), HexCoord::new(1, 0));
        g.map.set_hex(a, HexTerrain::Clear);
        g.map.set_hex(b, HexTerrain::Clear);
        g.minefields.insert(b, Minefield { side: Side::Axis, dummy: true }); // enemy dummy

        let inf = g.spawn_unit(Nationality::Commonwealth, "h", "inf", Composition::Organic { current_toe: 4 });
        g.unit_mut(inf).unwrap().location = Some(UnitLocation::OnMap(a));
        let cpa = g.unit_cpa(inf).unwrap();

        // §26.23: a dummy costs the same as a real minefield to enter (full CPA).
        assert_eq!(
            g.apply(Command::MoveStep { unit: inf, to: b, mode: MoveMode::NonMotorized }),
            Ok(CommandOutcome::Moved)
        );
        assert_eq!(g.unit(inf).unwrap().cp_expended.0, 20 + cpa * 10);
        // Revealed, but still present this stage — same-phase movers would still pay.
        assert!(g.revealed_minefields.contains(&b));
        assert!(g.minefields.contains_key(&b));

        // §26.23: removed at the end of the Movement Phase (next Operations Stage).
        g.begin_operations_stage();
        assert!(!g.minefields.contains_key(&b));
        assert!(g.revealed_minefields.is_empty());
    }

    #[test]
    fn mine_casualty_roll() {
        use crate::state::game::GameState;
        use crate::state::unit::{Composition, WeaponGroup};

        let mut g = GameState::new(ReferenceData::new(), 1);
        // §26.25: a tank unit loses one TOE point on a 5 or 6, nothing on 1–4.
        let tank = g.spawn_unit(
            Nationality::Commonwealth,
            "g",
            "tk",
            Composition::Weapons { groups: vec![WeaponGroup { system: "A9 Cruiser".into(), count: 4 }] },
        );
        assert!(!g.resolve_mine_casualty(tank, 4));
        assert_eq!(g.unit(tank).unwrap().total_toe(), 4);
        assert!(g.resolve_mine_casualty(tank, 5));
        assert_eq!(g.unit(tank).unwrap().total_toe(), 3);
        assert!(g.resolve_mine_casualty(tank, 6));
        assert_eq!(g.unit(tank).unwrap().total_toe(), 2);
        // Motorized infantry loses an Organic TOE point.
        let inf = g.spawn_unit(Nationality::Commonwealth, "h", "inf", Composition::Organic { current_toe: 3 });
        assert!(g.resolve_mine_casualty(inf, 6));
        assert_eq!(g.unit(inf).unwrap().total_toe(), 2);
    }

    #[test]
    fn build_temporary_repair_facility() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::game::{Commodities, FacilityKind, GameState, SupplyDump};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let hex = HexCoord::new(0, 0);
        g.map.set_hex(hex, HexTerrain::MajorCity);
        let eng = g.spawn_unit(Nationality::Commonwealth, "a", "hq", Composition::Organic { current_toe: 0 });
        g.unit_mut(eng).unwrap().location = Some(UnitLocation::OnMap(hex));
        g.supply_dumps.push(SupplyDump {
            location: UnitLocation::OnMap(hex),
            side: Side::Commonwealth,
            supply: Commodities { fuel: 150, ammo: 0, stores: 250, water: 0 },
            dummy: false,
        });

        // §24.83: 250 Stores + 150 Fuel expended; 3 Construction Segments.
        assert!(g.begin_temporary_facility(eng, hex));
        assert_eq!(g.supply_dumps[0].supply.stores, 0);
        assert_eq!(g.supply_dumps[0].supply.fuel, 0);
        g.advance_construction();
        g.advance_construction();
        assert!(!g.repair_facilities.contains_key(&hex));
        g.advance_construction();
        assert_eq!(g.repair_facilities.get(&hex), Some(&FacilityKind::Temporary));

        // §24.85: not buildable in Desert (only Major City / Village).
        let d = HexCoord::new(5, 5);
        g.map.set_hex(d, HexTerrain::Desert);
        let e2 = g.spawn_unit(Nationality::Commonwealth, "a", "hq2", Composition::Organic { current_toe: 0 });
        g.unit_mut(e2).unwrap().location = Some(UnitLocation::OnMap(d));
        g.supply_dumps.push(SupplyDump {
            location: UnitLocation::OnMap(d),
            side: Side::Commonwealth,
            supply: Commodities { fuel: 150, ammo: 0, stores: 250, water: 0 },
            dummy: false,
        });
        assert!(!g.begin_temporary_facility(e2, d));
    }

    #[test]
    fn commonwealth_rail_movement() {
        use crate::reference::terrain::{Fixed, HexTerrain};
        use crate::state::game::GameState;
        use crate::state::map::{HexCoord, HexsideKind};
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let hs: Vec<HexCoord> = (0..4).map(|i| HexCoord::new(i, 0)).collect();
        for h in &hs {
            g.map.set_hex(*h, HexTerrain::Clear);
        }
        for i in 0..3 {
            g.map.set_hexside(hs[i], hs[i + 1], HexsideKind::Railroad, None);
        }
        let mk = |g: &mut GameState, h| {
            let u = g.spawn_unit(Nationality::Commonwealth, "h", "u", Composition::Organic { current_toe: 1 });
            g.unit_mut(u).unwrap().location = Some(UnitLocation::OnMap(h));
            u
        };

        // §8.72: rail a unit any distance east along the finished line.
        let a = mk(&mut g, hs[1]);
        assert!(g.rail_move(a, hs[3]));
        assert_eq!(g.unit(a).unwrap().hex(), Some(hs[3]));
        assert!(g.rail_east_used);

        // §8.74: the other direction is still available; a second east move is not.
        let b = mk(&mut g, hs[2]);
        assert!(g.rail_move(b, hs[0])); // west
        let c = mk(&mut g, hs[1]);
        assert!(!g.rail_move(c, hs[3])); // east already used this stage

        // §8.73: a unit that has expended CP may not use Rail Movement.
        g.begin_operations_stage(); // resets the direction flags (and CP)
        let d = mk(&mut g, hs[1]);
        g.unit_mut(d).unwrap().cp_expended = Fixed(10);
        assert!(!g.rail_move(d, hs[3]));
    }

    #[test]
    fn axis_use_of_commonwealth_railroad_54_4() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::game::{Commodities, GameState, RailCargo, SupplyDump};
        use crate::state::map::{HexCoord, HexsideKind};
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::German,
            include_str!("../../../data/reference/unit-characteristics/german.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        // A six-hex coastal rail line, all Clear, contiguous Railroad hexsides.
        let hs: Vec<HexCoord> = (0..6).map(|i| HexCoord::new(i, 0)).collect();
        for h in &hs {
            g.map.set_hex(*h, HexTerrain::Clear);
        }
        for i in 0..5 {
            g.map.set_hexside(hs[i], hs[i + 1], HexsideKind::Railroad, None);
        }

        // §54.41: fewer than five contiguous controlled hexes ⇒ no Axis rail.
        for h in hs.iter().take(4) {
            g.rail_control.insert(*h, Side::Axis);
        }
        assert_eq!(g.axis_rail_contiguous_max(), 4);
        // §54.43: cannot even import without a controlled hex … then take control of a fifth.
        g.rail_control.insert(hs[4], Side::Axis);
        assert_eq!(g.axis_rail_contiguous_max(), 5);

        // §54.43: import Rolling Stock — 250 Stores + 100 Fuel from an Axis dump → +300 tons.
        g.supply_dumps.push(SupplyDump {
            location: UnitLocation::OnMap(hs[0]),
            side: Side::Axis,
            supply: Commodities { fuel: 400, ammo: 0, stores: 800, water: 0 },
            dummy: false,
        });
        // A Commonwealth-controlled hex is not eligible for Axis import.
        assert!(!g.import_rolling_stock(hs[5]));
        assert!(g.import_rolling_stock(hs[0]));
        assert_eq!(g.axis_rolling_stock_tons, 300);
        let dump = |g: &GameState| {
            let d = g.supply_dumps.iter().find(|d| d.location.hex() == Some(hs[0]) && d.side == Side::Axis).unwrap();
            (d.supply.stores, d.supply.fuel)
        };
        assert_eq!(dump(&g), (550, 300)); // 800-250, 400-100

        // Spawn a German unit on the line and rail it. Import enough stock for 900 t/Stacking Point.
        let u = g.spawn_unit(Nationality::German, "i", "pz gren", Composition::Organic { current_toe: 8 });
        g.unit_mut(u).unwrap().location = Some(UnitLocation::OnMap(hs[1]));
        let sp = g.effective_stacking_points(u);
        assert!(sp >= 1, "the mover must not be a shell for this test");
        while g.axis_rolling_stock_tons < 900 * sp {
            assert!(g.import_rolling_stock(hs[0]));
        }
        let stock = g.axis_rolling_stock_tons;
        // §54.44: rail the unit west to hs[0] — 900 × SP tons consumed from the one-direction budget.
        assert!(g.rail_move(u, hs[0]));
        assert_eq!(g.unit(u).unwrap().hex(), Some(hs[0]));
        assert_eq!(g.axis_rail_tons_used, 900 * sp);
        assert_eq!(g.axis_rail_dir, Some(true)); // west

        // §54.43: haulage is one direction per stage — an eastbound supply shipment now fails.
        assert_eq!(g.rail_move_supply_for(Side::Axis, hs[0], hs[4], RailCargo::Stores, 10), 0);
        // A westbound shipment within the remaining budget works: source dump at hs[4].
        g.supply_dumps.push(SupplyDump {
            location: UnitLocation::OnMap(hs[4]),
            side: Side::Axis,
            supply: Commodities { fuel: 0, ammo: 0, stores: 50, water: 0 },
            dummy: false,
        });
        let remaining = stock - g.axis_rail_tons_used;
        let want = remaining.min(20);
        assert_eq!(g.rail_move_supply_for(Side::Axis, hs[4], hs[0], RailCargo::Stores, want), want);

        // §54.45: lose control of two hexes so fewer than five are contiguous — the next stage's
        // upkeep destroys the Rolling Stock.
        g.rail_control.remove(&hs[2]);
        g.rail_control.remove(&hs[3]);
        assert!(g.axis_rail_contiguous_max() < 5);
        g.begin_operations_stage();
        assert_eq!(g.axis_rolling_stock_tons, 0);
    }

    #[test]
    fn axis_rail_repair_and_no_new_line_54_42() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::game::{Commodities, GameState, SupplyDump};
        use crate::state::map::{HexCoord, HexsideKind};
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::German,
            include_str!("../../../data/reference/unit-characteristics/german.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let hs: Vec<HexCoord> = (0..4).map(|i| HexCoord::new(i, 0)).collect();
        for h in &hs {
            g.map.set_hex(*h, HexTerrain::Clear);
        }
        g.map.set_hexside(hs[0], hs[1], HexsideKind::Railroad, None);
        g.map.set_hexside(hs[1], hs[2], HexsideKind::Railroad, None);
        // Destroy the hs[0]–hs[1] stretch.
        assert!(g.destroy_railroad(hs[0], hs[1]));

        // A German Engineer with a Store present tries to repair it.
        let eng = g.spawn_unit(Nationality::German, "gg", "pio", Composition::Organic { current_toe: 1 });
        g.unit_mut(eng).unwrap().location = Some(UnitLocation::OnMap(hs[0]));
        g.supply_dumps.push(SupplyDump {
            location: UnitLocation::OnMap(hs[0]),
            side: Side::Axis,
            supply: Commodities { fuel: 0, ammo: 0, stores: 5, water: 0 },
            dummy: false,
        });

        // §54.42: the Axis may repair only a rail hex it controls — hs[0] is not yet controlled.
        assert!(!g.rebuild_railroad(eng, hs[0], hs[1]));
        // Take control (§54.41), then the repair succeeds at the Commonwealth rate (immediate).
        g.rail_control.insert(hs[0], Side::Axis);
        assert!(g.rebuild_railroad(eng, hs[0], hs[1]));
        assert!(g.map.hexsides_on(hs[0], hs[1]).iter().any(|h| h.kind == HexsideKind::Railroad));

        // §54.42: the Axis may never build *new* rail line. Set an unlaid stretch off the railhead
        // and try to build it with the Axis engineer — rejected regardless of units/supply.
        g.map.set_hexside(hs[2], hs[3], HexsideKind::UnfinishedRailroad, None);
        g.unit_mut(eng).unwrap().location = Some(UnitLocation::OnMap(hs[2]));
        assert!(!g.begin_railroad(&[eng], hs[2], hs[3]));
    }

    #[test]
    fn rail_supply_transport() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::game::{Commodities, GameState, RailCargo, SupplyDump};
        use crate::state::map::{HexCoord, HexsideKind};
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let hs: Vec<HexCoord> = (0..4).map(|i| HexCoord::new(i, 0)).collect();
        for h in &hs {
            g.map.set_hex(*h, HexTerrain::Clear);
        }
        for i in 0..3 {
            g.map.set_hexside(hs[i], hs[i + 1], HexsideKind::Railroad, None);
        }
        // A Commonwealth dump of 2000 Stores at the east end.
        g.supply_dumps.push(SupplyDump {
            location: UnitLocation::OnMap(hs[3]),
            side: Side::Commonwealth,
            supply: Commodities { fuel: 0, ammo: 0, stores: 2000, water: 0 },
            dummy: false,
        });

        // §54.32: rail Stores west, capped at 1500 tons (= 1500 Store points) for the direction.
        let moved = g.rail_move_supply(hs[3], hs[0], RailCargo::Stores, 2000);
        assert_eq!(moved, 1500);
        let src: u32 = g.supply_dumps.iter().filter(|d| d.location == UnitLocation::OnMap(hs[3])).map(|d| d.supply.stores).sum();
        assert_eq!(src, 500);
        let dst: u32 = g.supply_dumps.iter().filter(|d| d.location == UnitLocation::OnMap(hs[0])).map(|d| d.supply.stores).sum();
        assert_eq!(dst, 1500);
        assert!(g.rail_west_used);

        // §54.32: supplies and personnel share the once-per-direction allowance — west is now used,
        // but east is still free.
        let u = g.spawn_unit(Nationality::Commonwealth, "h", "u", Composition::Organic { current_toe: 1 });
        g.unit_mut(u).unwrap().location = Some(UnitLocation::OnMap(hs[1]));
        assert!(!g.rail_move(u, hs[0])); // west taken by the supply shipment
        assert!(g.rail_move(u, hs[3])); // east still available
    }

    #[test]
    fn motorized_infantry_assumes_truck_cpa() {
        use crate::state::game::GameState;
        use crate::state::unit::{Composition, TruckPoints};

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_truck_chart(include_str!("../../../data/reference/truck-characteristics.ron"))
            .unwrap();
        let mut g = GameState::new(refs, 1);

        // "p" is an Infantry Bn-Equivalent with a basic CPA of 10.
        let inf = g.spawn_unit(Nationality::Commonwealth, "p", "inf", Composition::Organic { current_toe: 3 });
        assert_eq!(g.effective_cpa(inf), Some(10));
        assert_eq!(g.unit_motorized_cpa(inf), None);

        // §6.17: loaded onto Medium Trucks it assumes the truck CPA (20).
        g.unit_mut(inf).unwrap().trucks = TruckPoints { light: 0, medium: 20, heavy: 0 };
        assert_eq!(g.unit_motorized_cpa(inf), Some(20));
        assert_eq!(g.effective_cpa(inf), Some(20));

        // §6.17: a unit with a basic CPA over 10 ("h" = 15) cannot be motorized this way.
        let h = g.spawn_unit(Nationality::Commonwealth, "h", "h", Composition::Organic { current_toe: 3 });
        g.unit_mut(h).unwrap().trucks = TruckPoints { light: 0, medium: 20, heavy: 0 };
        assert_eq!(g.unit_motorized_cpa(h), None);
        assert_eq!(g.effective_cpa(h), Some(15));
    }

    #[test]
    fn truck_carried_fuel_is_a_source() {
        use crate::state::game::GameState;
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, TruckPoints, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_truck_chart(include_str!("../../../data/reference/truck-characteristics.ron"))
            .unwrap();
        let mut g = GameState::new(refs, 1);
        let h = HexCoord::new(0, 0);
        let u = g.spawn_unit(Nationality::Commonwealth, "p", "u", Composition::Organic { current_toe: 3 });
        g.unit_mut(u).unwrap().location = Some(UnitLocation::OnMap(h));
        g.unit_mut(u).unwrap().trucks = TruckPoints { light: 0, medium: 5, heavy: 0 };
        let cost = g.unit_move_fuel_cost(u, 100);
        assert!(cost > 0);

        // §49.18: with no tank fuel and no dump, the move draws on the fuel the trucks carry.
        g.unit_mut(u).unwrap().fuel = 0;
        g.unit_mut(u).unwrap().truck_cargo.fuel = 1000;
        assert!(g.consume_move_fuel(u, h, 100));
        assert_eq!(g.unit(u).unwrap().truck_cargo.fuel, 1000 - cost);

        // With no fuel anywhere, the move cannot be fuelled.
        g.unit_mut(u).unwrap().truck_cargo.fuel = 0;
        assert!(!g.consume_move_fuel(u, h, 100));
    }

    #[test]
    fn rail_maintenance_stage_blocks_rail() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::game::{Commodities, GameState, RailCargo, SupplyDump};
        use crate::state::map::{HexCoord, HexsideKind};
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let hs: Vec<HexCoord> = (0..3).map(|i| HexCoord::new(i, 0)).collect();
        for h in &hs {
            g.map.set_hex(*h, HexTerrain::Clear);
        }
        for i in 0..2 {
            g.map.set_hexside(hs[i], hs[i + 1], HexsideKind::Railroad, None);
        }
        g.supply_dumps.push(SupplyDump {
            location: UnitLocation::OnMap(hs[2]),
            side: Side::Commonwealth,
            supply: Commodities { fuel: 0, ammo: 0, stores: 100, water: 0 },
            dummy: false,
        });
        let u = g.spawn_unit(Nationality::Commonwealth, "h", "u", Composition::Organic { current_toe: 1 });
        g.unit_mut(u).unwrap().location = Some(UnitLocation::OnMap(hs[1]));

        // §54.34: on the monthly maintenance stage, neither units nor supplies may rail.
        g.rail_maintenance_this_stage = true;
        assert!(!g.rail_move(u, hs[0]));
        assert_eq!(g.rail_move_supply(hs[2], hs[0], RailCargo::Stores, 50), 0);

        // The next Operations Stage clears it and rail works again.
        g.begin_operations_stage();
        assert!(g.rail_move(u, hs[0]));
    }

    #[test]
    fn load_aircraft_chart() {
        use crate::reference::aircraft::{AircraftClass, MissionCap};

        let mut refs = ReferenceData::new();
        refs.load_aircraft_chart(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/aircraft/commonwealth.ron"),
        )
        .unwrap();
        let chart = refs.aircraft_charts.get(&Nationality::Commonwealth).unwrap();

        // Single-loadout fighter.
        let hurri = &chart["Hurricane Mk.I"];
        assert_eq!(hurri.class, AircraftClass::Fighter);
        assert_eq!(hurri.max_range(), 52);
        assert_eq!(hurri.missions.cap, MissionCap::Day);

        // Multi-loadout fighter-bomber: the bomb variant carries 2 bombload; max range from the
        // drop-tanks row.
        let kitty = &chart["P-40 Kittyhawk I"];
        assert_eq!(kitty.class, AircraftClass::FighterBomber);
        assert_eq!(kitty.loadouts.len(), 3);
        assert_eq!(kitty.max_range(), 94);
        assert!(kitty.loadouts[0].drop_tanks);
        assert_eq!(kitty.loadouts[2].bombload, 2);

        // Night-capable CAP.
        assert_eq!(chart["Beaufighter Mk.IF"].missions.cap, MissionCap::Night);
        // Full fighters roster loaded; a few spot checks.
        assert_eq!(chart.len(), 45);
        assert_eq!(chart["Hurricane Mk.IID"].missions.cap, MissionCap::DayStrafeArmor);
        assert_eq!(chart["Spitfire Mk.VC"].loadouts.len(), 2);
        assert_eq!(chart["Skua Mk.II"].class, AircraftClass::DiveBomber);
        assert_eq!(chart["Hurricane Mk.IIC"].loadouts[2].bombload, 6);
        // Bombers / recon / transports / flying boats.
        let alba = &chart["Albacore"];
        assert_eq!(alba.class, AircraftClass::Bomber);
        assert!(alba.loadouts[0].no_air_to_air); // parenthesized TacAir
        assert_eq!(alba.loadouts[0].torpedo, 8);
        assert_eq!(chart["Sunderland"].class, AircraftClass::FlyingBoat);
        assert_eq!(chart["Valentia"].class, AircraftClass::Transport);
        assert_eq!(chart["Lysander Mk.I"].missions.strike, MissionCap::StrafeOnly);
        assert_eq!(chart["Wellington Mk.IV"].missions.convoy_land_bombing, MissionCap::Night);
        assert_eq!(chart["Halifax Mk.II"].loadouts[1].bombload, 65);

        // Italian roster (§4.44b): 8 fighters + 12 bombers/flying-boats.
        refs.load_aircraft_chart(
            Nationality::Italian,
            include_str!("../../../data/reference/aircraft/italian.ron"),
        )
        .unwrap();
        let it = refs.aircraft_charts.get(&Nationality::Italian).unwrap();
        assert_eq!(it.len(), 20);
        assert_eq!(it["C.202 Folgore"].loadouts[0].maneuver, 35);
        assert_eq!(it["S.M.79 Sparviero"].loadouts[0].torpedo, 6);
        assert_eq!(it["Cant Z.501 Gabbiano"].class, AircraftClass::FlyingBoat);
        assert_eq!(it["B.R.20 Cicogna"].missions.convoy_land_bombing, MissionCap::Night);

        // German roster (§4.44c): 5 fighters + 8 bombers/flying-boat/transport.
        refs.load_aircraft_chart(
            Nationality::German,
            include_str!("../../../data/reference/aircraft/german.ron"),
        )
        .unwrap();
        let de = refs.aircraft_charts.get(&Nationality::German).unwrap();
        assert_eq!(de.len(), 13);
        assert_eq!(de["Bf.109E"].loadouts[0].maneuver, 34);
        // Bf.110 & Ju.88c: night CAP, Night-Scramble only (§4.44c "*").
        assert_eq!(de["Bf.110"].missions.scramble, MissionCap::NightScrambleOnly);
        assert_eq!(de["Bf.110"].loadouts[1].bombload, 5);
        assert_eq!(de["Ju.88c"].missions.scramble, MissionCap::NightScrambleOnly);
        assert_eq!(de["Ju.87D"].class, AircraftClass::DiveBomber);
        assert_eq!(de["Ar.196"].class, AircraftClass::FlyingBoat);
    }

    #[test]
    fn fuel_siphoning() {
        use crate::state::game::GameState;
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation, WeaponGroup};

        let mut refs = ReferenceData::new();
        refs.load_weapons_systems(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/weapons/commonwealth.ron"),
        )
        .unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        let h = HexCoord::new(0, 0);
        let mk = |g: &mut GameState, name: &str| {
            let u = g.spawn_unit(
                Nationality::Commonwealth,
                "g",
                name,
                Composition::Weapons { groups: vec![WeaponGroup { system: "Crusader Mk.I".into(), count: 4 }] },
            );
            g.unit_mut(u).unwrap().location = Some(UnitLocation::OnMap(h));
            u
        };
        let donor = mk(&mut g, "donor");
        let recv = mk(&mut g, "recv");
        let cap = g.unit_fuel_capacity(recv);
        assert!(cap > 0);
        g.unit_mut(donor).unwrap().fuel = 100;
        g.unit_mut(recv).unwrap().fuel = 0;

        // §49.17: siphon fills the receiver to capacity and costs 3 CP to both units.
        let moved = g.siphon_fuel(donor, recv, 100);
        assert_eq!(moved, cap);
        assert_eq!(g.unit(recv).unwrap().fuel, cap);
        assert_eq!(g.unit(donor).unwrap().fuel, 100 - cap);
        assert_eq!(g.unit(donor).unwrap().cp_expended.0, 30);
        assert_eq!(g.unit(recv).unwrap().cp_expended.0, 30);

        // Not co-located → no transfer.
        let far = HexCoord::new(5, 5);
        let d2 = mk(&mut g, "d2");
        let r2 = g.spawn_unit(
            Nationality::Commonwealth,
            "g",
            "r2",
            Composition::Weapons { groups: vec![WeaponGroup { system: "Crusader Mk.I".into(), count: 4 }] },
        );
        g.unit_mut(r2).unwrap().location = Some(UnitLocation::OnMap(far));
        g.unit_mut(d2).unwrap().fuel = 50;
        assert_eq!(g.siphon_fuel(d2, r2, 50), 0);
    }

    #[test]
    fn half_rations() {
        use crate::reference::terrain::{Fixed, HexTerrain};
        use crate::rules::movement::MoveMode;
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, OrgSize, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_unit_characteristics(
            Nationality::Italian,
            include_str!("../../../data/reference/unit-characteristics/italian.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);

        // §51.23 stores: 4 per TOE normally, 2 per TOE on half rations.
        let s = g.spawn_unit(Nationality::Commonwealth, "h", "s", Composition::Organic { current_toe: 4 });
        assert_eq!(g.unit_stores_requirement(s), 16);
        g.half_rations.insert(s);
        assert_eq!(g.unit_stores_requirement(s), 8);

        g.phase = Phase::MovementAndCombat;
        // §51.23 (1): a half-ration unit may not voluntarily exceed its CPA.
        let (a, b) = (HexCoord::new(0, 0), HexCoord::new(1, 0));
        g.map.set_hex(a, HexTerrain::Clear);
        g.map.set_hex(b, HexTerrain::Clear);
        let m = g.spawn_unit(Nationality::Commonwealth, "h", "m", Composition::Organic { current_toe: 4 });
        g.unit_mut(m).unwrap().location = Some(UnitLocation::OnMap(a));
        g.half_rations.insert(m);
        let cpa = g.unit_cpa(m).unwrap();
        g.unit_mut(m).unwrap().cp_expended = Fixed(cpa * 10);
        assert_eq!(
            g.apply(Command::MoveStep { unit: m, to: b, mode: MoveMode::NonMotorized }),
            Err(CommandError::HalfRation)
        );
        g.unit_mut(m).unwrap().cp_expended = Fixed(0);
        assert_eq!(
            g.apply(Command::MoveStep { unit: m, to: b, mode: MoveMode::NonMotorized }),
            Ok(CommandOutcome::Moved)
        );

        // §51.23 (2): a half-ration unit may not voluntarily enter an enemy ZOC.
        for (c, r) in [(3, 2), (4, 2), (5, 2)] {
            g.map.set_hex(HexCoord::new(c, r), HexTerrain::Clear);
        }
        let brig = g.spawn_unit(Nationality::Italian, "p", "brig", Composition::Organic { current_toe: 6 });
        g.unit_mut(brig).unwrap().location = Some(UnitLocation::OnMap(HexCoord::new(5, 2)));
        g.unit_mut(brig).unwrap().org_size = OrgSize::StandardBrigade; // ZOC over (4,2)
        let z2 = g.spawn_unit(Nationality::Commonwealth, "h", "z2", Composition::Organic { current_toe: 4 });
        g.unit_mut(z2).unwrap().location = Some(UnitLocation::OnMap(HexCoord::new(3, 2)));
        g.half_rations.insert(z2);
        assert_eq!(
            g.apply(Command::MoveStep { unit: z2, to: HexCoord::new(4, 2), mode: MoveMode::NonMotorized }),
            Err(CommandError::HalfRation)
        );
        // A unit not on half rations may enter the ZOC (must stop) — proves the hex is enterable.
        let z = g.spawn_unit(Nationality::Commonwealth, "h", "z", Composition::Organic { current_toe: 4 });
        g.unit_mut(z).unwrap().location = Some(UnitLocation::OnMap(HexCoord::new(3, 2)));
        assert_eq!(
            g.apply(Command::MoveStep { unit: z, to: HexCoord::new(4, 2), mode: MoveMode::NonMotorized }),
            Ok(CommandOutcome::Moved)
        );
    }

    #[test]
    fn airdropped_unit_may_not_exceed_cpa_this_stage_42_47() {
        use crate::reference::terrain::{Fixed, HexTerrain};
        use crate::rules::movement::MoveMode;
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::German,
            include_str!("../../../data/reference/unit-characteristics/german.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        g.phase = Phase::MovementAndCombat;
        let (a, b) = (HexCoord::new(0, 0), HexCoord::new(1, 0));
        g.map.set_hex(a, HexTerrain::Clear);
        g.map.set_hex(b, HexTerrain::Clear);

        let para = g.spawn_unit(Nationality::German, "p", "para", Composition::Organic { current_toe: 4 });
        g.unit_mut(para).unwrap().location = Some(UnitLocation::OnMap(a));
        // §42.47: mark the unit as dropped this Operations Stage (as cmd_airdrop_unit records).
        let abs_stage = g.game_turn * 3 + g.operations_stage as u32;
        g.airdrop_last_stage.insert(para, abs_stage);

        let cpa = g.unit_cpa(para).unwrap();
        // A move that would carry it past its CPA this stage is refused (§42.47).
        g.unit_mut(para).unwrap().cp_expended = Fixed(cpa * 10);
        assert_eq!(
            g.apply(Command::MoveStep { unit: para, to: b, mode: MoveMode::NonMotorized }),
            Err(CommandError::AirdropCpaLimit)
        );
        // Within its CPA it moves normally (§42.47: it "may move and fight normally").
        g.unit_mut(para).unwrap().cp_expended = Fixed(0);
        assert_eq!(
            g.apply(Command::MoveStep { unit: para, to: b, mode: MoveMode::NonMotorized }),
            Ok(CommandOutcome::Moved)
        );
        // A unit dropped in an *earlier* stage is unrestricted.
        g.airdrop_last_stage.insert(para, abs_stage.saturating_sub(1));
        g.unit_mut(para).unwrap().cp_expended = Fixed(cpa * 10);
        g.map.set_hex(HexCoord::new(2, 0), HexTerrain::Clear);
        assert_eq!(
            g.apply(Command::MoveStep { unit: para, to: HexCoord::new(2, 0), mode: MoveMode::NonMotorized }),
            Ok(CommandOutcome::Moved)
        );
    }

    #[test]
    fn retreat_before_assault_13() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_unit_characteristics(
            Nationality::German,
            include_str!("../../../data/reference/unit-characteristics/german.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        g.phase = Phase::MovementAndCombat;
        g.active_player = Side::Axis; // the Commonwealth is the non-phasing player

        let start = HexCoord::new(5, 5);
        // A straight retreat line directly away from the (future) enemy: repeatedly take neighbour 3.
        let mut line = vec![start];
        for _ in 0..6 {
            line.push(line.last().unwrap().neighbors()[3]);
        }
        for h in &line {
            g.map.set_hex(*h, HexTerrain::Clear);
        }
        let mk = |g: &mut GameState| {
            let u = g.spawn_unit(Nationality::Commonwealth, "h", "cw", Composition::Organic { current_toe: 4 });
            g.unit_mut(u).unwrap().location = Some(UnitLocation::OnMap(start));
            u
        };
        let rba = |unit, path: Vec<HexCoord>| Command::RetreatBeforeAssault { unit, path };

        // §13.1: a Pinned unit may not Retreat Before Assault.
        let u = mk(&mut g);
        g.unit_mut(u).unwrap().status.pinned = true;
        assert_eq!(g.apply(rba(u, vec![line[1]])), Err(CommandError::CannotRetreatBeforeAssault));
        g.unit_mut(u).unwrap().status.pinned = false;
        // §13.1: Cohesion of −26 or worse bars it.
        g.unit_mut(u).unwrap().cohesion = -26;
        assert_eq!(g.apply(rba(u, vec![line[1]])), Err(CommandError::CannotRetreatBeforeAssault));
        g.unit_mut(u).unwrap().cohesion = 0;
        // §13: a phasing unit may not (the Axis is phasing here).
        let axis = g.spawn_unit(Nationality::German, "i", "de", Composition::Organic { current_toe: 4 });
        g.unit_mut(axis).unwrap().location = Some(UnitLocation::OnMap(HexCoord::new(20, 20)));
        g.map.set_hex(HexCoord::new(20, 20), HexTerrain::Clear);
        g.map.set_hex(HexCoord::new(21, 20), HexTerrain::Clear);
        assert_eq!(
            g.apply(rba(axis, vec![HexCoord::new(21, 20)])),
            Err(CommandError::CannotRetreatBeforeAssault),
        );

        // A single-hex retreat by the non-phasing unit is always allowed (§13.24 "or one hex").
        assert!(matches!(g.apply(rba(u, vec![line[1]])), Ok(CommandOutcome::RetreatedBeforeAssault { .. })));
        assert_eq!(g.unit(u).unwrap().hex(), Some(line[1]));

        // §13.24: with no enemy adjacent at the start, a multi-hex retreat over 4 CP is refused.
        g.unit_mut(u).unwrap().location = Some(UnitLocation::OnMap(start));
        g.unit_mut(u).unwrap().cp_expended = crate::reference::terrain::Fixed(0);
        assert_eq!(
            g.apply(rba(u, line[1..=6].to_vec())),
            Err(CommandError::RetreatBeforeAssaultTooFar),
        );

        // §13.23: adjacent to an enemy combat unit at the start, the 4-CP cap is lifted. Place an
        // enemy on the opposite neighbour (so the retreat line never re-enters its ZOC).
        let enemy_hex = start.neighbors()[0];
        g.map.set_hex(enemy_hex, HexTerrain::Clear);
        g.unit_mut(axis).unwrap().location = Some(UnitLocation::OnMap(enemy_hex));
        g.unit_mut(u).unwrap().location = Some(UnitLocation::OnMap(start));
        g.unit_mut(u).unwrap().cp_expended = crate::reference::terrain::Fixed(0);
        assert!(matches!(
            g.apply(rba(u, line[1..=6].to_vec())),
            Ok(CommandOutcome::RetreatedBeforeAssault { .. })
        ));
        assert_eq!(g.unit(u).unwrap().hex(), Some(line[6]));
    }

    #[test]
    fn patrol_and_reconnaissance_16() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_unit_characteristics(
            Nationality::German,
            include_str!("../../../data/reference/unit-characteristics/german.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        g.phase = Phase::Patrol;
        g.active_player = Side::Commonwealth;
        let (a, b) = (HexCoord::new(5, 5), HexCoord::new(8, 5));
        // Clear the whole a→b corridor so a legal ≤5-hex Patrol path exists (§16.22).
        for c in 5..=8 {
            g.map.set_hex(HexCoord::new(c, 5), HexTerrain::Clear);
        }

        // Commonwealth Recce battalion ("hh") at A with ammo + fuel for the patrol (§16.16).
        let recce = g.spawn_unit(Nationality::Commonwealth, "hh", "Recce", Composition::Organic { current_toe: 8 });
        g.unit_mut(recce).unwrap().location = Some(UnitLocation::OnMap(a));
        g.unit_mut(recce).unwrap().ammo = 10;
        g.unit_mut(recce).unwrap().fuel = 10;
        // Two German combat battalions at the target.
        let g1 = g.spawn_unit(Nationality::German, "i", "1/104", Composition::Organic { current_toe: 5 });
        let g2 = g.spawn_unit(Nationality::German, "i", "2/104", Composition::Organic { current_toe: 6 });
        for u in [g1, g2] {
            g.unit_mut(u).unwrap().location = Some(UnitLocation::OnMap(b));
        }

        // Dice: survival 3 (all-Recce −1 → row 0…3, no loss); recon 4 × 2 net points → reveal 2;
        // objective loss 1 → 0/0.
        let patrol = |reveals: Vec<(_, i32)>, loss: Vec<(_, u32)>| Command::Patrol {
            patrol_points: vec![(recce, 2)], target: b, exclude: vec![],
            reveals, loss_choice: loss, survival_die: 3, recon_die: 4, loss_die: 1,
        };

        // §16.51: reported TOE must be within ±2 — 8 for a 5-strength unit (diff 3) is out of band.
        assert_eq!(
            g.apply(patrol(vec![(g1, 8), (g2, 6)], vec![])),
            Err(CommandError::PatrolRevealMismatch),
        );
        // Wrong number of units revealed (recon required 2).
        assert_eq!(
            g.apply(patrol(vec![(g1, 5)], vec![])),
            Err(CommandError::PatrolRevealMismatch),
        );
        // A faithful patrol: reveal both units within ±2 (g1 exact 5, g2 as 4 = 6−2).
        match g.apply(patrol(vec![(g1, 5), (g2, 4)], vec![])) {
            Ok(CommandOutcome::Patrol(r)) => {
                assert!(!r.wiped_out);
                assert_eq!(r.patroller_killed + r.patroller_captured, 0);
                assert_eq!(r.revealed.len(), 2);
                let rep: Vec<_> = r.revealed.iter().map(|u| (u.unit, u.reported_strength)).collect();
                assert!(rep.contains(&(g1, 5)) && rep.contains(&(g2, 4)));
                assert!(r.revealed.iter().all(|u| u.unit_type.contains("Infantry")));
            }
            other => panic!("unexpected: {other:?}"),
        }
        // Ammo/Fuel were expended (§16.16): 2 Ammo + 4 Fuel from the reserve.
        assert_eq!(g.unit(recce).unwrap().ammo, 8);
        assert_eq!(g.unit(recce).unwrap().fuel, 6);

        // §16.21: a side that fought Close Assault this stage may not Patrol.
        g.land_combat_this_stage.insert(Side::Commonwealth);
        assert_eq!(
            g.apply(patrol(vec![(g1, 5), (g2, 6)], vec![])),
            Err(CommandError::PatrolNotAllowed),
        );
    }

    #[test]
    fn dummy_tank_formations_16_4() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{Commodities, GameState, Phase, SupplyDump};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        refs.load_unit_characteristics(
            Nationality::German,
            include_str!("../../../data/reference/unit-characteristics/german.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        g.phase = Phase::Organization;
        g.active_player = Side::Axis;
        let hexes: Vec<HexCoord> = (0..5).map(|c| HexCoord::new(c, 0)).collect();
        for h in &hexes {
            g.map.set_hex(*h, HexTerrain::Clear);
        }
        let stock = |g: &mut GameState, h: HexCoord, stores: u32| {
            g.supply_dumps.push(SupplyDump {
                location: UnitLocation::OnMap(h),
                side: Side::Axis,
                supply: Commodities { fuel: 0, ammo: 0, stores, water: 0 },
                dummy: false,
            });
        };
        let place = |h| Command::PlaceDummyTank { hex: h, designation: "8 RTR".into() };

        // §16.42: 10 Stores must be present — a hex with only 5 is refused.
        stock(&mut g, hexes[0], 5);
        assert_eq!(g.apply(place(hexes[0])), Err(CommandError::DummyTankRejected));
        // With 10 Stores it is placed and the Stores are consumed.
        stock(&mut g, hexes[0], 5); // now 10 total
        assert_eq!(g.apply(place(hexes[0])), Ok(CommandOutcome::DummyTankPlaced));
        assert_eq!(g.dummy_tanks_in_hex(hexes[0], Side::Axis), 1);
        assert_eq!(g.supply_dumps.iter().filter(|d| d.location.hex() == Some(hexes[0])).map(|d| d.supply.stores).sum::<u32>(), 0);
        // §16.43: no second dummy in the same hex.
        stock(&mut g, hexes[0], 10);
        assert_eq!(g.apply(place(hexes[0])), Err(CommandError::DummyTankRejected));
        // §16.43: at most three in existence — place two more (hexes 1,2), the fourth (hex 3) fails.
        for h in [hexes[1], hexes[2], hexes[3]] {
            stock(&mut g, h, 10);
        }
        assert_eq!(g.apply(place(hexes[1])), Ok(CommandOutcome::DummyTankPlaced));
        assert_eq!(g.apply(place(hexes[2])), Ok(CommandOutcome::DummyTankPlaced));
        assert_eq!(g.apply(place(hexes[3])), Err(CommandError::DummyTankRejected));

        // §16.46: a Close Assault against the hex eliminates its dummy. A German combat unit shares
        // hexes[2] (so there is a real defender); a Commonwealth attacker adjacent assaults it.
        let de = g.spawn_unit(Nationality::German, "i", "def", Composition::Organic { current_toe: 4 });
        g.unit_mut(de).unwrap().location = Some(UnitLocation::OnMap(hexes[2]));
        let cw = g.spawn_unit(Nationality::Commonwealth, "h", "atk", Composition::Organic { current_toe: 4 });
        g.unit_mut(cw).unwrap().location = Some(UnitLocation::OnMap(hexes[1]));
        g.phase = Phase::MovementAndCombat;
        g.active_player = Side::Commonwealth;
        assert_eq!(g.dummy_tanks_in_hex(hexes[2], Side::Axis), 1);
        assert!(matches!(
            g.apply(Command::CloseAssault { attackers: vec![cw], target: hexes[2] }),
            Ok(CommandOutcome::CloseAssault(_))
        ));
        assert_eq!(g.dummy_tanks_in_hex(hexes[2], Side::Axis), 0, "§16.46: the dummy is eliminated");
    }

    #[test]
    fn reorganization_attach_detach_reassign_19() {
        use crate::reference::terrain::{Fixed, HexTerrain};
        use crate::reference::units::Side;
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, OrgSize, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(
            Nationality::German,
            include_str!("../../../data/reference/unit-characteristics/german.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);
        g.phase = Phase::Organization;
        g.active_player = Side::Axis;
        let (a, b) = (HexCoord::new(0, 0), HexCoord::new(9, 9));
        g.map.set_hex(a, HexTerrain::Clear);
        g.map.set_hex(b, HexTerrain::Clear);

        let div = g.spawn_unit(Nationality::German, "a", "15 Pz", Composition::Weapons { groups: vec![] });
        g.unit_mut(div).unwrap().org_size = OrgSize::Division;
        g.unit_mut(div).unwrap().location = Some(UnitLocation::OnMap(a));
        let mk = |g: &mut GameState, h| {
            let u = g.spawn_unit(Nationality::German, "i", "bn", Composition::Organic { current_toe: 4 });
            g.unit_mut(u).unwrap().location = Some(UnitLocation::OnMap(h));
            u
        };

        // §19.41: an assigned unit attaches for 1 CP each (both units).
        let assigned = mk(&mut g, a);
        g.unit_mut(assigned).unwrap().assigned_parent = Some(div);
        assert_eq!(g.apply(Command::AttachUnit { unit: assigned, parent: div }), Ok(CommandOutcome::UnitAttached));
        assert_eq!(g.unit(assigned).unwrap().attached_parent, Some(div));
        assert_eq!(g.unit(assigned).unwrap().cp_expended, Fixed(10));
        assert_eq!(g.unit(div).unwrap().cp_expended, Fixed(10));

        // §19.13: a unit already attached may not attach again without detaching.
        assert_eq!(g.apply(Command::AttachUnit { unit: assigned, parent: div }), Err(CommandError::ReorgIllegal));
        // §19.43: detach for 1 CP each.
        assert_eq!(g.apply(Command::DetachUnit { unit: assigned }), Ok(CommandOutcome::UnitDetached));
        assert_eq!(g.unit(assigned).unwrap().attached_parent, None);

        // §19.1: a non-co-located unit may not attach.
        let far = mk(&mut g, b);
        assert_eq!(g.apply(Command::AttachUnit { unit: far, parent: div }), Err(CommandError::ReorgIllegal));

        // §19.42: a non-assigned unit attaches for 2 CP in the Organization phase, within §19.5.
        let indep = mk(&mut g, a);
        assert_eq!(g.apply(Command::AttachUnit { unit: indep, parent: div }), Ok(CommandOutcome::UnitAttached));
        assert_eq!(g.unit(indep).unwrap().cp_expended, Fixed(20));
        // §19.21/§19.28: reassign the now-attached independent unit to the division.
        assert_eq!(g.apply(Command::ReassignUnit { unit: indep, parent: div }), Ok(CommandOutcome::UnitReassigned));
        assert_eq!(g.unit(indep).unwrap().assigned_parent, Some(div));

        // §19.3: with the division's historical assignment capacity reached, a further assignment is
        // refused (§19.21) — even though the unit may still attach (§19.5).
        g.unit_mut(div).unwrap().assigned_capacity = 1; // one unit (indep) already assigned
        let extra = mk(&mut g, a);
        assert_eq!(g.apply(Command::AttachUnit { unit: extra, parent: div }), Ok(CommandOutcome::UnitAttached));
        assert_eq!(g.apply(Command::ReassignUnit { unit: extra, parent: div }), Err(CommandError::ReorgIllegal));

        // §19.5 limit: a Battalion parent may take only one non-assigned attachment (German).
        let bn_parent = mk(&mut g, a);
        assert_eq!(g.nonassigned_attachment_limit(bn_parent), 1);
        let c1 = mk(&mut g, a);
        let c2 = mk(&mut g, a);
        assert_eq!(g.apply(Command::AttachUnit { unit: c1, parent: bn_parent }), Ok(CommandOutcome::UnitAttached));
        assert_eq!(g.apply(Command::AttachUnit { unit: c2, parent: bn_parent }), Err(CommandError::ReorgIllegal));

        // §19.42: a non-assigned attach is refused outside the Organization phase.
        g.phase = Phase::MovementAndCombat;
        let m = mk(&mut g, a);
        assert_eq!(g.apply(Command::AttachUnit { unit: m, parent: div }), Err(CommandError::WrongPhase));
    }

    #[test]
    fn prisoners_28() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::command::{Command, CommandOutcome};
        use crate::state::game::{Commodities, GameState, Phase, SupplyDump};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, UnitLocation, WeaponGroup};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(Nationality::German, include_str!("../../../data/reference/unit-characteristics/german.ron")).unwrap();
        refs.load_unit_characteristics(Nationality::Commonwealth, include_str!("../../../data/reference/unit-characteristics/commonwealth.ron")).unwrap();
        refs.load_weapons_systems(Nationality::German, include_str!("../../../data/reference/weapons/german.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        let h = HexCoord::new(5, 5);
        for c in 3..=8 { g.map.set_hex(HexCoord::new(c, 5), HexTerrain::Clear); }

        // §28.11/§28.3: a German infantry unit and a German tank unit surrender at `h` → the
        // Commonwealth captures infantry as Prisoner Points and armor as captured equipment.
        let inf = g.spawn_unit(Nationality::German, "i", "pris", Composition::Organic { current_toe: 6 });
        g.unit_mut(inf).unwrap().location = Some(UnitLocation::OnMap(h));
        let tank = g.spawn_unit(Nationality::German, "f", "pz", Composition::Weapons { groups: vec![WeaponGroup { system: "Pz III H".into(), count: 5 }] });
        g.unit_mut(tank).unwrap().location = Some(UnitLocation::OnMap(h));
        g.capture_on_surrender(inf);
        g.capture_on_surrender(tank);
        assert_eq!(g.prisoner_groups.iter().filter(|x| x.captor == Side::Commonwealth).map(|x| x.points).sum::<u32>(), 6);
        assert_eq!(g.captured_equipment.iter().filter(|e| e.captor == Side::Commonwealth && e.system == "Pz III H").map(|e| e.count).sum::<u32>(), 5);

        // §28.15: feeding drains 1 Store per 5 prisoners (ceil(6/5)=2) from the nearest CW dump.
        g.supply_dumps.push(SupplyDump { location: UnitLocation::OnMap(h), side: Side::Commonwealth, supply: Commodities { fuel: 0, ammo: 0, stores: 10, water: 0 }, dummy: false });
        assert_eq!(g.feed_prisoners(), 2);
        assert_eq!(g.supply_dumps.iter().find(|d| d.location.hex() == Some(h)).unwrap().supply.stores, 8);

        // §28.22: form two Guard Points from a co-located CW infantry unit (enough to escort 6).
        g.phase = Phase::Organization;
        g.active_player = Side::Commonwealth;
        let guardsrc = g.spawn_unit(Nationality::Commonwealth, "h", "guard", Composition::Organic { current_toe: 4 });
        g.unit_mut(guardsrc).unwrap().location = Some(UnitLocation::OnMap(h));
        assert_eq!(g.apply(Command::FormGuard { group_hex: h }), Ok(CommandOutcome::GuardFormed));
        assert_eq!(g.apply(Command::FormGuard { group_hex: h }), Ok(CommandOutcome::GuardFormed));
        assert_eq!(g.unit(guardsrc).unwrap().total_toe(), 2); // two TOE became guards

        // §51.17: each Guard Point costs 2 Stores per Game-Turn from the nearest dump (2 guards → 4).
        assert_eq!(g.feed_guards(), 4);
        assert_eq!(g.supply_dumps.iter().find(|d| d.location.hex() == Some(h)).unwrap().supply.stores, 4);

        // §28.13: escort the stack one hex; two guards cover 10 ≥ 6, so none escape.
        g.phase = Phase::TruckConvoy;
        let dest = HexCoord::new(6, 5);
        assert_eq!(g.apply(Command::MovePrisoners { from: h, to: dest }), Ok(CommandOutcome::PrisonersMoved { escaped: 0 }));
        assert_eq!(g.prisoner_groups.iter().find(|x| x.captor == Side::Commonwealth).unwrap().hex, dest);

        // §28.16: reaching a Departure Point removes them permanently.
        g.pow_departure_points.entry(Side::Commonwealth).or_default().insert(HexCoord::new(7, 5));
        assert!(matches!(g.apply(Command::MovePrisoners { from: dest, to: HexCoord::new(7, 5) }), Ok(CommandOutcome::PrisonersMoved { .. })));
        assert!(g.prisoner_groups.is_empty());

        // §28.3: repair the captured tank.
        g.phase = Phase::Repair;
        assert_eq!(g.apply(Command::RepairCapturedEquipment { hex: h, system: "Pz III H".into() }), Ok(CommandOutcome::CapturedEquipmentRepaired));
        assert!(g.captured_equipment.iter().any(|e| e.repaired && e.system == "Pz III H"));
        // §22.5: it returns to play as a trained Tank Replacement Point pool for the captor at `h`,
        // one point per captured TOE, absorbable immediately (no Training).
        use crate::state::game::ReplacementClass;
        let pool = g
            .replacement_points
            .iter()
            .find(|p| p.class == ReplacementClass::Tank && p.location.as_ref().and_then(|l| l.hex()) == Some(h))
            .expect("§22.5 repaired tank → Replacement Points");
        assert_eq!(pool.count, 5);
        assert_eq!(pool.training_remaining, 0, "§22.5: repaired vehicles skip Training");

        // §28.23/§28.24: an unguarded stack escapes; with a friendly (Axis) unit within 8 it rejoins
        // and is scheduled to return as Replacement Points.
        g.capture_prisoners(Side::Commonwealth, h, 4); // original side = Axis
        let axis = g.spawn_unit(Nationality::German, "i", "rescue", Composition::Organic { current_toe: 4 });
        g.unit_mut(axis).unwrap().location = Some(UnitLocation::OnMap(HexCoord::new(8, 5)));
        let before = g.pending_replacements.len();
        g.resolve_prisoner_escapes();
        assert!(g.prisoner_groups.is_empty(), "unguarded prisoners escaped");
        assert_eq!(g.pending_replacements.len(), before + 1, "§28.24 escapees return as Replacement Points");

        // §28.25: an escapee with no friendly nearby waits, and is recaptured if an enemy enters.
        let far = HexCoord::new(40, 40);
        g.map.set_hex(far, HexTerrain::Clear);
        g.escape_points(Side::Axis, far, 3);
        assert_eq!(g.escapees.iter().map(|e| e.points).sum::<u32>(), 3);
        let cw = g.spawn_unit(Nationality::Commonwealth, "h", "recap", Composition::Organic { current_toe: 4 });
        g.unit_mut(cw).unwrap().location = Some(UnitLocation::OnMap(far));
        g.resolve_prisoner_escapes();
        assert!(g.escapees.is_empty(), "§28.25 recaptured");
        assert_eq!(g.prisoner_groups.iter().find(|x| x.hex == far).unwrap().captor, Side::Commonwealth);
    }

    #[test]
    fn reorganization_type_constraints_19_5() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, OrgSize, UnitLocation, WeaponGroup};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(Nationality::German, include_str!("../../../data/reference/unit-characteristics/german.ron")).unwrap();
        refs.load_unit_characteristics(Nationality::Commonwealth, include_str!("../../../data/reference/unit-characteristics/commonwealth.ron")).unwrap();
        refs.load_weapons_systems(Nationality::German, include_str!("../../../data/reference/weapons/german.ron")).unwrap();
        refs.load_weapons_systems(Nationality::Commonwealth, include_str!("../../../data/reference/weapons/commonwealth.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        g.phase = Phase::Organization;
        let a = HexCoord::new(0, 0);
        g.map.set_hex(a, HexTerrain::Clear);

        // §19.5: a German infantry division may take no Tank attachment.
        g.active_player = Side::Axis;
        let inf_div = g.spawn_unit(Nationality::German, "a", "inf div", Composition::Weapons { groups: vec![] });
        g.unit_mut(inf_div).unwrap().org_size = OrgSize::Division; // is_armor defaults false
        g.unit_mut(inf_div).unwrap().location = Some(UnitLocation::OnMap(a));
        let de_tank = g.spawn_unit(Nationality::German, "f", "pz bn", Composition::Weapons { groups: vec![WeaponGroup { system: "Pz III H".into(), count: 5 }] });
        g.unit_mut(de_tank).unwrap().location = Some(UnitLocation::OnMap(a));
        assert_eq!(g.apply(Command::AttachUnit { unit: de_tank, parent: inf_div }), Err(CommandError::ReorgIllegal));
        // An infantry battalion attaches fine.
        let de_inf = g.spawn_unit(Nationality::German, "i", "inf bn", Composition::Organic { current_toe: 4 });
        g.unit_mut(de_inf).unwrap().location = Some(UnitLocation::OnMap(a));
        assert_eq!(g.apply(Command::AttachUnit { unit: de_inf, parent: inf_div }), Ok(CommandOutcome::UnitAttached));

        // §19.5: a Commonwealth armor division (GT68+) may attach at most one Tank and one Infantry.
        g.game_turn = 70;
        g.active_player = Side::Commonwealth;
        let arm_div = g.spawn_unit(Nationality::Commonwealth, "a", "armd div", Composition::Weapons { groups: vec![] });
        g.unit_mut(arm_div).unwrap().org_size = OrgSize::Division;
        g.unit_mut(arm_div).unwrap().is_armor = true;
        g.unit_mut(arm_div).unwrap().location = Some(UnitLocation::OnMap(a));
        let mk_tank = |g: &mut GameState| {
            let u = g.spawn_unit(Nationality::Commonwealth, "g", "rtr", Composition::Weapons { groups: vec![WeaponGroup { system: "A9 Cruiser".into(), count: 4 }] });
            g.unit_mut(u).unwrap().location = Some(UnitLocation::OnMap(a));
            u
        };
        let t1 = mk_tank(&mut g);
        let t2 = mk_tank(&mut g);
        assert_eq!(g.apply(Command::AttachUnit { unit: t1, parent: arm_div }), Ok(CommandOutcome::UnitAttached));
        assert_eq!(g.apply(Command::AttachUnit { unit: t2, parent: arm_div }), Err(CommandError::ReorgIllegal)); // §19.5 max 1 Tank
    }

    #[test]
    fn attachment_count_distinguishes_arm_19_5() {
        // §19.5 Maximum Attachment Chart: the numeric cap depends on the parent's ARM
        // (armor vs infantry/other), not just its org size, nationality and Game-Turn.
        use crate::state::game::{GameState, Phase};
        use crate::state::unit::{Composition, OrgSize};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        for nat in [Nationality::German, Nationality::Commonwealth, Nationality::Italian] {
            let uc: &str = match nat {
                Nationality::German => include_str!("../../../data/reference/unit-characteristics/german.ron"),
                Nationality::Commonwealth => include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
                _ => include_str!("../../../data/reference/unit-characteristics/italian.ron"),
            };
            refs.load_unit_characteristics(nat, uc).unwrap();
        }
        let mut g = GameState::new(refs, 1);
        g.phase = Phase::Organization;

        let mk = |g: &mut GameState, nat, size, armor| {
            let u = g.spawn_unit(nat, "a", "hq", Composition::Weapons { groups: vec![] });
            g.unit_mut(u).unwrap().org_size = size;
            g.unit_mut(u).unwrap().is_armor = armor;
            u
        };

        // Commonwealth divisions: armor 2 (GT 1–67) / 3 (GT 68+); infantry division always 2.
        let cw_arm = mk(&mut g, Nationality::Commonwealth, OrgSize::Division, true);
        let cw_inf = mk(&mut g, Nationality::Commonwealth, OrgSize::Division, false);
        g.game_turn = 20;
        assert_eq!(g.nonassigned_attachment_limit(cw_arm), 2);
        assert_eq!(g.nonassigned_attachment_limit(cw_inf), 2);
        g.game_turn = 70;
        assert_eq!(g.nonassigned_attachment_limit(cw_arm), 3);
        assert_eq!(g.nonassigned_attachment_limit(cw_inf), 2); // infantry division stays 2 (was wrongly 3)

        // Commonwealth brigades: Tank Brigade 2 (GT 1–67) / 1 (GT 68+); any other brigade 1.
        let cw_tank_bde = mk(&mut g, Nationality::Commonwealth, OrgSize::StandardBrigade, true);
        let cw_other_bde = mk(&mut g, Nationality::Commonwealth, OrgSize::StandardBrigade, false);
        g.game_turn = 20;
        assert_eq!(g.nonassigned_attachment_limit(cw_tank_bde), 2);
        assert_eq!(g.nonassigned_attachment_limit(cw_other_bde), 1); // other brigade 1 (was wrongly 2)
        g.game_turn = 70;
        assert_eq!(g.nonassigned_attachment_limit(cw_tank_bde), 1);
        assert_eq!(g.nonassigned_attachment_limit(cw_other_bde), 1);

        // German divisions: armor 4 ("1 Bde and 1 unit or 4 units"); infantry 3 ("1 Bde or 3 units").
        let de_arm = mk(&mut g, Nationality::German, OrgSize::Division, true);
        let de_inf = mk(&mut g, Nationality::German, OrgSize::Division, false);
        assert_eq!(g.nonassigned_attachment_limit(de_arm), 4);
        assert_eq!(g.nonassigned_attachment_limit(de_inf), 3); // infantry division 3 (was wrongly 4)

        // Italian divisions: both 2 (the chart draws no arm distinction).
        let it_arm = mk(&mut g, Nationality::Italian, OrgSize::Division, true);
        let it_inf = mk(&mut g, Nationality::Italian, OrgSize::Division, false);
        assert_eq!(g.nonassigned_attachment_limit(it_arm), 2);
        assert_eq!(g.nonassigned_attachment_limit(it_inf), 2);
    }

    #[test]
    fn reorganization_special_roles_19_5() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, FormationRole, OrgSize, UnitLocation, WeaponGroup};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        for nat in [Nationality::German, Nationality::Commonwealth, Nationality::Italian] {
            let (uc, wp): (&str, &str) = match nat {
                Nationality::German => (
                    include_str!("../../../data/reference/unit-characteristics/german.ron"),
                    include_str!("../../../data/reference/weapons/german.ron"),
                ),
                Nationality::Commonwealth => (
                    include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
                    include_str!("../../../data/reference/weapons/commonwealth.ron"),
                ),
                _ => (
                    include_str!("../../../data/reference/unit-characteristics/italian.ron"),
                    include_str!("../../../data/reference/weapons/italian.ron"),
                ),
            };
            refs.load_unit_characteristics(nat, uc).unwrap();
            refs.load_weapons_systems(nat, wp).unwrap();
        }
        let mut g = GameState::new(refs, 1);
        g.phase = Phase::Organization;
        let a = HexCoord::new(0, 0);
        g.map.set_hex(a, HexTerrain::Clear);
        let at_a = |g: &mut GameState, u| {
            g.unit_mut(u).unwrap().location = Some(UnitLocation::OnMap(a));
        };
        let wpn = |sys: &str| Composition::Weapons { groups: vec![WeaponGroup { system: sys.into(), count: 4 }] };

        // §19.5 Italian infantry division: "1 Infantry *or* 1 Tank" — at most one, combined.
        g.active_player = Side::Axis;
        let it_div = g.spawn_unit(Nationality::Italian, "a", "it inf div", Composition::Weapons { groups: vec![] });
        g.unit_mut(it_div).unwrap().org_size = OrgSize::Division; // is_armor false → infantry division
        at_a(&mut g, it_div);
        let it_inf = g.spawn_unit(Nationality::Italian, "i", "it inf", Composition::Organic { current_toe: 4 });
        let it_tank = g.spawn_unit(Nationality::Italian, "f", "it tank", wpn("M 13/40"));
        let it_gun = g.spawn_unit(Nationality::Italian, "jj", "it gun", wpn("75/27 Gun"));
        for u in [it_inf, it_tank, it_gun] { at_a(&mut g, u); }
        assert_eq!(g.apply(Command::AttachUnit { unit: it_inf, parent: it_div }), Ok(CommandOutcome::UnitAttached));
        // The infantry already used the single Infantry-or-Tank slot: a tank is refused.
        assert_eq!(g.apply(Command::AttachUnit { unit: it_tank, parent: it_div }), Err(CommandError::ReorgIllegal));
        // A gun is neither infantry nor tank — it fits the second of the two unit slots.
        assert_eq!(g.apply(Command::AttachUnit { unit: it_gun, parent: it_div }), Ok(CommandOutcome::UnitAttached));

        // §19.5 German Artillery Brigade HQ: its one attachment must be an Artillery (gun) unit.
        let arty_hq = g.spawn_unit(Nationality::German, "b", "Artillerie HQ", Composition::Weapons { groups: vec![] });
        g.unit_mut(arty_hq).unwrap().org_size = OrgSize::StandardBrigade;
        g.unit_mut(arty_hq).unwrap().formation_role = FormationRole::ArtilleryBrigadeHq;
        at_a(&mut g, arty_hq);
        let de_inf = g.spawn_unit(Nationality::German, "i", "de inf", Composition::Organic { current_toe: 4 });
        let de_gun = g.spawn_unit(Nationality::German, "v", "de gun", wpn("10.5cm (K18) Medium Gun"));
        for u in [de_inf, de_gun] { at_a(&mut g, u); }
        assert_eq!(g.apply(Command::AttachUnit { unit: de_inf, parent: arty_hq }), Err(CommandError::ReorgIllegal));
        assert_eq!(g.apply(Command::AttachUnit { unit: de_gun, parent: arty_hq }), Ok(CommandOutcome::UnitAttached));

        // §19.5 Mersa Matruh Garrison (GT 1–12): no tank, at most 1 gun class.
        g.active_player = Side::Commonwealth;
        g.game_turn = 1;
        let matruh = g.spawn_unit(Nationality::Commonwealth, "b", "Matruh Garrison HQ", Composition::Weapons { groups: vec![] });
        g.unit_mut(matruh).unwrap().org_size = OrgSize::StandardBrigade;
        g.unit_mut(matruh).unwrap().formation_role = FormationRole::MatruhSelby;
        at_a(&mut g, matruh);
        let cw_tank = g.spawn_unit(Nationality::Commonwealth, "g", "cw tank", wpn("A9 Cruiser"));
        let cw_inf = g.spawn_unit(Nationality::Commonwealth, "i", "cw inf", Composition::Organic { current_toe: 4 });
        let cw_gun1 = g.spawn_unit(Nationality::Commonwealth, "x", "cw gun1", wpn("25-pounder Gun"));
        let cw_gun2 = g.spawn_unit(Nationality::Commonwealth, "x", "cw gun2", wpn("25-pounder Gun"));
        for u in [cw_tank, cw_inf, cw_gun1, cw_gun2] { at_a(&mut g, u); }
        assert_eq!(g.apply(Command::AttachUnit { unit: cw_tank, parent: matruh }), Err(CommandError::ReorgIllegal)); // no tank
        assert_eq!(g.apply(Command::AttachUnit { unit: cw_inf, parent: matruh }), Ok(CommandOutcome::UnitAttached));
        assert_eq!(g.apply(Command::AttachUnit { unit: cw_gun1, parent: matruh }), Ok(CommandOutcome::UnitAttached));
        assert_eq!(g.apply(Command::AttachUnit { unit: cw_gun2, parent: matruh }), Err(CommandError::ReorgIllegal)); // >1 gun

        // §19.5 Selby Force (GT 13+): no recce; at most 3 infantry.
        g.game_turn = 20;
        let selby = g.spawn_unit(Nationality::Commonwealth, "b", "Selby Force HQ", Composition::Weapons { groups: vec![] });
        g.unit_mut(selby).unwrap().org_size = OrgSize::StandardBrigade;
        g.unit_mut(selby).unwrap().formation_role = FormationRole::MatruhSelby;
        at_a(&mut g, selby);
        let recce = g.spawn_unit(Nationality::Commonwealth, "hh", "cw recce", Composition::Weapons { groups: vec![] });
        at_a(&mut g, recce);
        assert_eq!(g.apply(Command::AttachUnit { unit: recce, parent: selby }), Err(CommandError::ReorgIllegal)); // no recce
        let inf: Vec<_> = (0..4)
            .map(|_| {
                let u = g.spawn_unit(Nationality::Commonwealth, "i", "gi", Composition::Organic { current_toe: 4 });
                at_a(&mut g, u);
                u
            })
            .collect();
        assert_eq!(g.apply(Command::AttachUnit { unit: inf[0], parent: selby }), Ok(CommandOutcome::UnitAttached));
        assert_eq!(g.apply(Command::AttachUnit { unit: inf[1], parent: selby }), Ok(CommandOutcome::UnitAttached));
        assert_eq!(g.apply(Command::AttachUnit { unit: inf[2], parent: selby }), Ok(CommandOutcome::UnitAttached));
        assert_eq!(g.apply(Command::AttachUnit { unit: inf[3], parent: selby }), Err(CommandError::ReorgIllegal)); // >3 infantry
    }

    #[test]
    fn reorganization_nesting_19_46() {
        use crate::reference::terrain::HexTerrain;
        use crate::reference::units::Side;
        use crate::state::command::{Command, CommandError, CommandOutcome};
        use crate::state::game::{GameState, Phase};
        use crate::state::map::HexCoord;
        use crate::state::unit::{Composition, OrgSize, UnitLocation};

        let mut refs = ReferenceData::new();
        refs.load_terrain(TERRAIN_RON).unwrap();
        refs.load_unit_characteristics(Nationality::German, include_str!("../../../data/reference/unit-characteristics/german.ron")).unwrap();
        let mut g = GameState::new(refs, 1);
        g.phase = Phase::Organization;
        g.active_player = Side::Axis;
        let a = HexCoord::new(0, 0);
        g.map.set_hex(a, HexTerrain::Clear);
        let mk = |g: &mut GameState| {
            let u = g.spawn_unit(Nationality::German, "i", "u", Composition::Organic { current_toe: 4 });
            g.unit_mut(u).unwrap().location = Some(UnitLocation::OnMap(a));
            u
        };
        let bde = g.spawn_unit(Nationality::German, "b", "bde", Composition::Weapons { groups: vec![] });
        g.unit_mut(bde).unwrap().org_size = OrgSize::StandardBrigade; // §19.5 German brigade limit = 1
        g.unit_mut(bde).unwrap().location = Some(UnitLocation::OnMap(a));
        let bn = mk(&mut g);
        let leaf = mk(&mut g);

        // Fill the brigade with one non-assigned battalion (its §19.5 limit is 1).
        assert_eq!(g.apply(Command::AttachUnit { unit: bn, parent: bde }), Ok(CommandOutcome::UnitAttached));
        // §19.46: attaching `leaf` to `bn` also counts against the brigade — which is full — so it is
        // refused, even though `bn` itself has room.
        assert_eq!(g.apply(Command::AttachUnit { unit: leaf, parent: bn }), Err(CommandError::ReorgIllegal));
        // Detach the battalion from the brigade; now `leaf` may attach to it (no full ancestor).
        assert_eq!(g.apply(Command::DetachUnit { unit: bn }), Ok(CommandOutcome::UnitDetached));
        assert_eq!(g.apply(Command::AttachUnit { unit: leaf, parent: bn }), Ok(CommandOutcome::UnitAttached));
    }

    #[test]
    fn barrage_resolver_applies_results() {
        use crate::reference::crt::{BarrageResult, BarrageTargetClass};
        use crate::state::game::GameState;
        use crate::state::unit::{Composition, WeaponGroup};

        // combat_refs has CW + Italian weapons; add the barrage CRT + CW unit chars.
        let mut refs = combat_refs();
        refs.load_barrage_crt(include_str!("../../../data/reference/crt/barrage.ron"))
            .unwrap();
        refs.load_unit_characteristics(
            Nationality::Commonwealth,
            include_str!("../../../data/reference/unit-characteristics/commonwealth.ron"),
        )
        .unwrap();
        let mut g = GameState::new(refs, 1);

        // Attacker: 15× 25-pounder (barrage 8) -> raw 120 -> actual 12 (column 11,12).
        let arty = g.spawn_unit(
            Nationality::Commonwealth,
            "x",
            "arty",
            Composition::Weapons {
                groups: vec![WeaponGroup { system: "25-pounder Gun".into(), count: 15 }],
            },
        );

        // Infantry target (organic): dice 45 -> Infantry 11,12 col -> result 1 ->
        // lose 1 TOE + Pinned.
        let inf = g.spawn_unit(
            Nationality::Commonwealth,
            "h",
            "inf",
            Composition::Organic { current_toe: 5 },
        );
        let out = g.resolve_barrage_with_dice(&[arty], inf, 45);
        assert_eq!(out.actual_points, 12);
        assert_eq!(out.class, BarrageTargetClass::Infantry);
        assert_eq!(out.result, BarrageResult::One);
        assert_eq!(out.toe_removed, 1);
        assert!(out.pinned);
        assert_eq!(g.unit(inf).unwrap().total_toe(), 4);
        assert!(g.unit(inf).unwrap().status.pinned);

        // Armor target (M 13/40 = armor, no barrage): dice 66 -> Armor 11,12 col ->
        // Pinned (no TOE loss).
        let armor = g.spawn_unit(
            Nationality::Italian,
            "tank",
            "tanks",
            Composition::Weapons {
                groups: vec![WeaponGroup { system: "M 13/40".into(), count: 4 }],
            },
        );
        assert_eq!(g.barrage_class(armor), BarrageTargetClass::Armor);
        let out = g.resolve_barrage_with_dice(&[arty], armor, 66);
        assert_eq!(out.result, BarrageResult::Pinned);
        assert_eq!(out.toe_removed, 0);
        assert!(out.pinned);
        assert_eq!(g.unit(armor).unwrap().total_toe(), 4);

        // Gun target (25-pounders): 2× 25-pdr attacker -> actual 2 (col 1,2), dice 62 ->
        // Gun result 1 -> lose 1 TOE, NOT pinned.
        let small = g.spawn_unit(
            Nationality::Commonwealth,
            "x",
            "small arty",
            Composition::Weapons {
                groups: vec![WeaponGroup { system: "25-pounder Gun".into(), count: 2 }],
            },
        );
        let gun = g.spawn_unit(
            Nationality::Commonwealth,
            "x",
            "gun target",
            Composition::Weapons {
                groups: vec![WeaponGroup { system: "25-pounder Gun".into(), count: 3 }],
            },
        );
        assert_eq!(g.barrage_class(gun), BarrageTargetClass::Gun);
        let out = g.resolve_barrage_with_dice(&[small], gun, 62);
        assert_eq!(out.actual_points, 2);
        assert_eq!(out.class, BarrageTargetClass::Gun);
        assert_eq!(out.result, BarrageResult::One);
        assert_eq!(out.toe_removed, 1);
        assert!(!out.pinned);
        assert_eq!(g.unit(gun).unwrap().total_toe(), 2);
        assert!(!g.unit(gun).unwrap().status.pinned);
    }
}
