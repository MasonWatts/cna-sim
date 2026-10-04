//! Shared scenario builders used by the `record` bin and the `cna-harness` (LLM player harness).

use cna_engine::reference::terrain::HexTerrain;
use cna_engine::reference::units::Nationality;
use cna_engine::reference::ReferenceData;
use cna_engine::state::game::GameState;
use cna_engine::state::map::HexCoord;
use cna_engine::state::unit::{Composition, UnitLocation, WeaponGroup};

/// A compact skirmish: cruisers + infantry per side on a small clear board, stocked to move/fight.
pub fn build_skirmish(seed: u64) -> GameState {
    let mut g = GameState::new(ReferenceData::load_campaign(), seed);
    for col in 0..10 {
        for row in 0..5 {
            g.map.set_hex(HexCoord::new(col, row), HexTerrain::Clear);
        }
    }
    let place = |g: &mut GameState, nat, id: &str, name: &str, comp, col, row| {
        let u = g.spawn_unit(nat, id, name, comp);
        let unit = g.unit_mut(u).unwrap();
        unit.location = Some(UnitLocation::OnMap(HexCoord::new(col, row)));
        unit.fuel = 800;
        unit.ammo = 800;
    };
    let cruisers = || Composition::Weapons { groups: vec![WeaponGroup { system: "A13 Cruiser".into(), count: 6 }] };
    let m11 = || Composition::Weapons { groups: vec![WeaponGroup { system: "M 11/39".into(), count: 6 }] };
    place(&mut g, Nationality::Commonwealth, "g", "1st RTR", cruisers(), 1, 1);
    place(&mut g, Nationality::Commonwealth, "h", "1st Rifle Bde", Composition::Organic { current_toe: 8 }, 2, 2);
    place(&mut g, Nationality::Commonwealth, "h", "2nd Rifle Bde", Composition::Organic { current_toe: 8 }, 2, 3);
    place(&mut g, Nationality::Italian, "k", "Aresca Tank Bn", m11(), 8, 2);
    place(&mut g, Nationality::Italian, "p", "1 Libyan Bn", Composition::Organic { current_toe: 8 }, 8, 1);
    place(&mut g, Nationality::Italian, "p", "2 Libyan Bn", Composition::Organic { current_toe: 8 }, 8, 3);
    g
}

macro_rules! data {
    ($p:literal) => {
        include_str!(concat!("../../../data/", $p))
    };
}

/// The full engine on the real map: reference bootstrap + all five map sections + a scenario RON.
pub fn build_real(seed: u64, scenario_ron: &str) -> GameState {
    let mut g = GameState::new(ReferenceData::load_campaign(), seed);
    for s in [
        data!("maps/section-a.ron"),
        data!("maps/section-b.ron"),
        data!("maps/section-c.ron"),
        data!("maps/section-d.ron"),
        data!("maps/section-e.ron"),
    ] {
        g.map.load_section(s).unwrap();
    }
    g.load_scenario(scenario_ron).expect("scenario loads");
    g.auto_place_dump_choices();
    g
}

/// Build a named scenario (`skirmish` | `scenario1` | `campaign`), returning it with its display label.
pub fn build(name: &str, seed: u64) -> (GameState, &'static str) {
    match name {
        "scenario1" => (build_real(seed, data!("scenarios/italian-offensive.ron")), "The Italian Offensive (§60)"),
        "campaign" => (build_real(seed, data!("scenarios/campaign.ron")), "Campaign Game (§64)"),
        _ => (build_skirmish(seed), "Skirmish"),
    }
}
