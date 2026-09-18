//! Records a deterministic CNA-Sim game as a JSON event log for the spectator website.
//!
//! See `docs/visualization-architecture.md`. The static map is serialized **once**; each event
//! carries the acting side, a human-readable action + outcome, and the dynamic unit state *after*
//! the action, so a dumb (no-engine) frontend can render the board at every step. Commands are
//! rendered as strings for this v0 — a round-trippable `Command` schema (shared with the future
//! MCP interface) and per-action `§` rule citations are follow-on passes.

use cna_engine::agent::Agent;
use cna_engine::reference::units::Side;
use cna_engine::state::game::{GameState, Phase};
use serde::Serialize;
use ts_rs::TS;

/// A complete recorded game: header (scenario, seed, agents, static map) + the ordered event log.
///
/// The `#[derive(TS)]` on this and the types below generates the frontend's TypeScript types into
/// `crates/replay/bindings/` when `cargo test` runs — the `game.json` schema is single-sourced here.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct GameRecord {
    pub scenario: String,
    pub seed: u64,
    pub commonwealth_agent: String,
    pub axis_agent: String,
    pub map: MapView,
    /// Every distinct counter-glyph primitive-set present across the full Order of Battle (all
    /// nationalities), for a complete, self-updating Unit Key legend — independent of what the
    /// recorded game happens to field. Each entry is a set like `["Infantry","Airborne","Heavy"]`.
    #[serde(default)]
    pub unit_types: Vec<Vec<String>>,
    pub events: Vec<Event>,
}

/// Distinct `unit_type` glyph primitive-sets across every unit in every nationality's Order of Battle
/// (§4.44), deterministically ordered — the authoritative legend source (includes units that never
/// deploy in a given game).
fn ob_unit_type_glyphs(g: &GameState) -> Vec<Vec<String>> {
    let mut set: std::collections::BTreeSet<Vec<String>> = std::collections::BTreeSet::new();
    for ob in g.reference.orders_of_battle.values() {
        for f in &ob.formations {
            for u in &f.units {
                if let Some(t) = u.unit_type {
                    set.insert(t.glyph().iter().map(|p| format!("{p:?}")).collect());
                }
            }
        }
    }
    set.into_iter().collect()
}

/// The static terrain grid, serialized once in the header.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct MapView {
    pub hexes: Vec<HexView>,
    /// Edge features (Road/Track/Railroad/River/Wadi/Ridge/Escarpment/Slope, …) as endpoint pairs.
    #[serde(default)]
    pub hexsides: Vec<HexsideView>,
}

#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct HexsideView {
    pub a: [i16; 2],
    pub b: [i16; 2],
    pub kind: String,
    /// The "up" hex for directional kinds (Slope/Escarpment); `None` otherwise.
    #[serde(default)]
    pub high: Option<[i16; 2]>,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct HexView {
    pub col: i16,
    pub row: i16,
    pub terrain: String,
    /// Static point features on the hex (Oasis / Village / Bir / Port), for the viewer to glyph.
    #[serde(default)]
    pub features: Vec<String>,
    /// The paper-map place name (Village / Major City), for a cosmetic label; `None` if unnamed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// One recorded step: the state *after* an accepted command (or the initial frame).
#[derive(Serialize, TS)]
#[ts(export)]
pub struct Event {
    pub step: u32,
    pub game_turn: u32,
    pub operations_stage: u8,
    pub phase: String,
    /// Acting side (`"-"` for the initial frame).
    pub side: String,
    /// Human-readable command (v0: `Debug`).
    pub action: String,
    /// Human-readable command outcome (v0: `Debug`).
    pub outcome: String,
    /// `§` rule citations for this action — populated in a later pass.
    pub rules: Vec<String>,
    /// **Delta**: only the units whose view changed since the previous event (step 0 lists them
    /// all). To render the board at step N, fold `units` from step 0 through N. Keeps the log small
    /// on large maps — most steps touch one or two units.
    pub units: Vec<UnitView>,
    /// Unit ids removed (eliminated / left the map) since the previous event.
    pub removed: Vec<u32>,
    /// Per-section weather (§29.1) `[A, B, C, D, E]` — each `"Normal"`/`"Hot"`/`"Sandstorm"`/
    /// `"Rainstorm"`. **Delta**: `Some` only when it changed since the previous event (step 0
    /// carries the initial state); fold forward to render the weather at step N.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weather: Option<Vec<String>>,
    /// Replacement-Point pools (§20). **Delta**: `Some` only when the set changed since the previous
    /// event (step 0 carries the initial state); fold forward to render the pools at step N. An empty
    /// `Some([])` means "all pools cleared". Lets the viewer show staging → arrival → travel →
    /// absorption; a pool vanishing while its assigned unit's TOE jumps is an absorption (§20.49).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacements: Option<Vec<ReplacementView>>,
    /// On-map Supply Dumps (§24.9). **Delta**: `Some` only when the set changed since the previous
    /// event (step 0 carries the initial state); fold forward to render dumps at step N.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supply_dumps: Option<Vec<SupplyDumpView>>,
    /// Free truck convoys (§53). **Delta**: `Some` only when the set changed since the previous event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truck_convoys: Option<Vec<TruckConvoyView>>,
    /// Desert raiders in play (§27). **Delta**: `Some` only when the set changed. Fold forward.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raiders: Option<Vec<RaiderView>>,
    /// The Rommel counter (§31), a 0-or-1 list. **Delta**: `Some` only when it changed. Fold forward.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rommel: Option<Vec<RommelView>>,
    /// Map overlays — minefields / fortifications / pipelines. **Delta**: `Some` only when any changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlays: Option<MapOverlaysView>,
    /// Map hexside features (routes + edge terrain). **Delta**: `Some` only when they change (rail/road
    /// built or destroyed); the header carries the initial set. Fold forward to render edges at step N.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hexsides: Option<Vec<HexsideView>>,
    /// The air layer — facilities / squadrons / CAP patrols (§35/§36/§40). **Delta**: `Some` only when
    /// it changes. Fold forward.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub air: Option<AirView>,
    /// The naval layer — on-map ships + port state + Axis convoys (§30/§55/§56). **Delta**: `Some` only
    /// when it changes. Fold forward.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub naval: Option<NavalView>,
}

#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct UnitView {
    pub id: u32,
    pub name: String,
    pub nationality: String,
    pub side: String,
    /// `[col, row]` of the unit's effective hex (attached units ride their parent), or `None`.
    pub hex: Option<[i16; 2]>,
    pub org_size: String,
    /// The counter-symbol primitives (a set of branch + modifier names, e.g. `["Infantry"]`,
    /// `["Infantry","Motorized"]`, `["Armor","Artillery"]`, `["Hq"]`). The viewer composes the glyph
    /// from these. Empty when unclassified.
    pub unit_type: Vec<String>,
    pub toe: u32,
    pub fuel: u32,
    pub ammo: u32,
    pub cohesion: i32,
    /// §4.44/§17 current Basic Morale. Green Commonwealth units arrive below `morale_reference` and
    /// train up toward it (§17.34); for non-green units the two are equal.
    pub basic_morale: i32,
    /// The unit's trained/target Basic Morale (§17.31).
    pub morale_reference: i32,
    /// §17.3 whether the unit is designated to Train this Operations Stage.
    pub in_training: bool,
    /// True if the unit has broken-down vehicles / weapons or lost TOE to breakdown (§21) — i.e. it
    /// is (partly) immobilized until repaired (§22). Drives the "broken" marker in the viewer.
    pub broken: bool,
    /// True if this is an HQ counter. A 0-TOE HQ persists as its formation's **cadre** (§19.62/§19.67
    /// — new battalions can be assigned to it), so the viewer keeps and marks it; a 0-TOE *non-HQ*
    /// unit is combat-eliminated and gone for good (§19.62), so the recorder omits it entirely.
    pub is_hq: bool,
}

/// A Replacement-Point pool (§20.2), for visualizing the replacement lifecycle. **Staged** pools
/// (`pending: true`) are planned to arrive at a future Naval Convoy Arrival Phase (§20.21) and carry
/// an `arrival` `[game_turn, stage]`; on arrival they become on-map pools (`pending: false`) with a
/// `hex` and a `training_remaining` countdown (§20.43, 0 = trained/absorbable). `assigned_to` is the
/// unit id the pool is earmarked for (§20.41); it must reach that unit's hex to be absorbed (§20.49).
#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct ReplacementView {
    /// `"Infantry"` | `"Gun"` | `"Tank"` | `"Recce"`.
    pub class: String,
    pub count: u32,
    pub side: String,
    /// `[col, row]` of the pool's hex once on the map; `None` while still staged/off-map.
    pub hex: Option<[i16; 2]>,
    /// Operations Stages of Training remaining (§20.43); 0 = trained and absorbable.
    pub training_remaining: u32,
    /// The unit id this pool is assigned to for delivery (§20.41), if any.
    pub assigned_to: Option<u32>,
    /// True while the pool is a planned future arrival (§20.21), not yet on the map.
    pub pending: bool,
    /// For a pending pool, its scheduled `[game_turn, stage]` arrival.
    pub arrival: Option<[u32; 2]>,
    /// §20.44: index of the free truck convoy currently hauling this pool, if any. `Some` means the
    /// pool rides that convoy (drawn as a subordinate tile in its stack); `None` means it stands alone
    /// on its hex (an Infantry pool stranded awaiting pickup, or a self-moving Tank/Gun/Recce pool).
    pub carried_by: Option<u32>,
}

/// An on-map Supply Dump (§24.9/§32.1) — a hex holding commodities (Land Game: Fuel + Ammo; the
/// Logistics Game adds Stores + Water). The counter's ID has no game effect (§4.55), so only the
/// side, location, contents, and whether it is a §32.18 dummy are surfaced.
#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct SupplyDumpView {
    pub hex: [i16; 2],
    pub side: String,
    pub fuel: u32,
    pub ammo: u32,
    pub stores: u32,
    pub water: u32,
    /// A §32.18 dummy dump (no real supplies; removed on discovery).
    pub dummy: bool,
    /// §24.9: `true` for an actual constructed dump (convoy-loadable); `false` for loose supplies
    /// merely off-loaded in the hex (usable/consumable, but a convoy may not re-load them).
    pub constructed: bool,
}

/// A free (2nd/3rd-line) truck convoy (§53) — Truck Points by type, broken-down count, carried cargo,
/// and whether it hauls Replacement Points. Rendered as a lorry glyph at its hex.
#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct TruckConvoyView {
    /// Stable index into `GameState::truck_convoys` — matches a `ReplacementView::carried_by` so the
    /// viewer can group a hauled pool with its convoy.
    pub id: u32,
    pub hex: [i16; 2],
    pub side: String,
    pub light: u32,
    pub medium: u32,
    pub heavy: u32,
    /// Total broken-down Truck Points (§21) awaiting §22.13 field repair.
    pub broken: u32,
    pub fuel: u32,
    pub ammo: u32,
    pub stores: u32,
    pub water: u32,
    /// §20.44/§27.57: whether the convoy is hauling a Replacement-Point pool.
    pub carries_replacements: bool,
}

/// A desert-raider counter in play (§27) — the Commonwealth Long Range Desert Group or the Axis
/// Sonderkommando Almásy. Only raiders currently on the map (not eliminated/awaiting re-form) appear.
#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct RaiderView {
    pub hex: [i16; 2],
    pub side: String,
    /// `"Lrdg"` (Commonwealth) | `"Dsa"` (Axis).
    pub kind: String,
    /// §27.32: whether the *enemy* can currently see it (an owner always sees its own). A concealed
    /// raider is drawn with a dashed border.
    pub revealed: bool,
    pub fuel: u32,
    pub ammo: u32,
}

/// The Rommel command counter (§31), when on the map. Off-map / in-Germany states are simply omitted.
#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct RommelView {
    pub hex: [i16; 2],
    /// The unit id Rommel is currently lending his CPA to (§31.4), if any.
    pub attached_to: Option<u32>,
}

/// A minefield hex (§26) — real or §26.x dummy, with whether the enemy has spotted it (§26.2).
#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct MinefieldView {
    pub hex: [i16; 2],
    pub side: String,
    pub dummy: bool,
    /// §26.2: whether the enemy has observed it. An unspotted field draws faint/dashed.
    pub revealed: bool,
}

/// A fortification hex (§24.4) — its level and any battle/bombing damage (§41 city gates).
#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct FortView {
    pub hex: [i16; 2],
    pub level: u8,
    /// §41.35: accumulated gate/fort damage, if any.
    pub damage: u8,
}

/// Static-ish map overlays that change during play: minefields (§26), fortifications (§24.4), and
/// water-pipeline hexes (§52.2). Bundled into one delta layer so the log carries them together.
#[derive(Serialize, TS, Clone, PartialEq, Default)]
#[ts(export)]
pub struct MapOverlaysView {
    pub minefields: Vec<MinefieldView>,
    pub fortifications: Vec<FortView>,
    /// `[col, row]` of each hex holding a water pipeline (§52.2).
    pub pipelines: Vec<[i16; 2]>,
}

fn map_overlay_views(g: &GameState) -> MapOverlaysView {
    let mut minefields: Vec<MinefieldView> = g
        .minefields
        .iter()
        .map(|(h, m)| MinefieldView {
            hex: [h.col, h.row],
            side: format!("{:?}", m.side),
            dummy: m.dummy,
            revealed: g.revealed_minefields.contains(h),
        })
        .collect();
    minefields.sort_by_key(|m| (m.hex[0], m.hex[1]));
    let mut fortifications: Vec<FortView> = g
        .fortifications
        .iter()
        .map(|(h, &level)| FortView { hex: [h.col, h.row], level, damage: g.city_fort_damage.get(h).copied().unwrap_or(0) })
        .collect();
    fortifications.sort_by_key(|f| (f.hex[0], f.hex[1]));
    let mut pipelines: Vec<[i16; 2]> = g.pipelines.iter().map(|h| [h.col, h.row]).collect();
    pipelines.sort();
    MapOverlaysView { minefields, fortifications, pipelines }
}

/// An on-map air facility (§36) — an airfield / landing strip / flying-boat basin, with its current
/// squadron capacity. Off-map facilities (Tunisia/Canal boxes) are omitted.
#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct AirFacilityView {
    pub hex: [i16; 2],
    /// `"Airfield"` | `"AirLandingStrip"` | `"FlyingBoatBasin"` | `"FlyingBoatAlightingArea"`.
    pub kind: String,
    /// §36.14 current capacity in squadrons (0 = destroyed).
    pub capacity: u32,
}

/// An on-map squadron / SGSU (§35) — its aircraft class, designation, total plane count, and whether
/// it is supplied (§35.14). Off-map / pooled / eliminated squadrons (no hex) are omitted.
#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct SquadronView {
    pub hex: [i16; 2],
    pub side: String,
    pub class: String,
    pub designation: String,
    pub planes: u32,
    pub supplied: bool,
}

/// A Combat Air Patrol (§40.2) flying over a hex, by side and type.
#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct CapView {
    pub hex: [i16; 2],
    pub side: String,
    pub cap_type: String,
}

/// The on-map air layer (§35/§36/§40) — facilities, based squadrons, and CAP patrols. Bundled into
/// one delta layer so the log carries the air picture together.
#[derive(Serialize, TS, Clone, PartialEq, Default)]
#[ts(export)]
pub struct AirView {
    pub facilities: Vec<AirFacilityView>,
    pub squadrons: Vec<SquadronView>,
    pub caps: Vec<CapView>,
}

fn air_views(g: &GameState) -> AirView {
    let mut facilities: Vec<AirFacilityView> = g
        .air_facilities
        .iter()
        .filter(|f| !f.off_map)
        .map(|f| AirFacilityView { hex: [f.hex.col, f.hex.row], kind: format!("{:?}", f.kind), capacity: f.capacity_level })
        .collect();
    facilities.sort_by_key(|f| (f.hex[0], f.hex[1]));
    let mut squadrons: Vec<SquadronView> = g
        .sgsus
        .values()
        .filter_map(|s| {
            let hex = s.location?;
            Some(SquadronView {
                hex: [hex.col, hex.row],
                side: format!("{:?}", s.nationality.side()),
                class: format!("{:?}", s.class),
                designation: s.designation.clone(),
                planes: s.planes.iter().map(|p| p.count).sum(),
                supplied: s.supplied,
            })
        })
        .collect();
    squadrons.sort_by_key(|s| (s.hex[0], s.hex[1], s.designation.clone()));
    let mut caps: Vec<CapView> = g
        .cap_patrols
        .iter()
        .map(|c| CapView { hex: [c.center.col, c.center.row], side: format!("{:?}", c.side), cap_type: format!("{:?}", c.cap_type) })
        .collect();
    caps.sort_by_key(|c| (c.hex[0], c.hex[1]));
    AirView { facilities, squadrons, caps }
}

/// A warship on the map (§30): a Commonwealth fleet unit assigned to a sea/coastal hex, or the scuttled
/// San Giorgio wreck at Tobruk (`wreck: true`). In-port ships (Alexandria/Valetta) are omitted.
#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct ShipView {
    pub hex: [i16; 2],
    pub side: String,
    pub name: String,
    pub gun: u32,
    pub max_gun: u32,
    pub wreck: bool,
}

/// A port's operational state (§55) for the off-map naval panel.
#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct PortStateView {
    pub name: String,
    pub efficiency: u32,
    pub max_efficiency: u32,
    /// §55.25/§55.27: an efficiency level is held down by harbour mines or a scuttled wreck.
    pub blocked: bool,
}

/// A scheduled/in-transit Axis naval convoy (§56) for the panel; cancelled convoys are omitted.
#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct NavalConvoyView {
    pub lane: u8,
    pub destination: String,
    pub arrival_gt: u32,
    /// Total tonnage carried (fuel + ammo + stores + replacements).
    pub tons: u32,
}

/// The naval layer (§30/§55/§56): on-map ships (tiles) plus port state and Axis convoys (panel).
#[derive(Serialize, TS, Clone, PartialEq, Default)]
#[ts(export)]
pub struct NavalView {
    pub ships: Vec<ShipView>,
    pub ports: Vec<PortStateView>,
    pub convoys: Vec<NavalConvoyView>,
}

fn naval_views(g: &GameState) -> NavalView {
    use cna_engine::state::naval::ShipLocation;
    let mut ships: Vec<ShipView> = g
        .fleet
        .iter()
        .filter_map(|s| match s.location {
            ShipLocation::AtSea(h) => Some(ShipView {
                hex: [h.col, h.row],
                side: "Commonwealth".into(),
                name: s.name.clone(),
                gun: s.gun_rating,
                max_gun: s.max_gun,
                wreck: false,
            }),
            _ => None, // in-port ships live in a port box, not on the map
        })
        .collect();
    if let Some(sg) = &g.san_giorgio {
        ships.push(ShipView {
            hex: [sg.hex.col, sg.hex.row],
            side: "Axis".into(),
            name: "San Giorgio".into(),
            gun: sg.gun_rating,
            max_gun: sg.max_gun,
            wreck: true,
        });
    }
    ships.sort_by_key(|s| (s.hex[0], s.hex[1], s.name.clone()));
    let mut ports: Vec<PortStateView> = g
        .ports
        .values()
        .map(|p| PortStateView { name: p.name.clone(), efficiency: p.efficiency, max_efficiency: p.max_efficiency, blocked: p.mines > 0 || p.wreck_block > 0 })
        .collect();
    ports.sort_by(|a, b| a.name.cmp(&b.name));
    let mut convoys: Vec<NavalConvoyView> = g
        .convoys
        .iter()
        .filter(|c| !c.cancelled)
        .map(|c| NavalConvoyView { lane: c.lane, destination: c.destination.clone(), arrival_gt: c.arrival_gt, tons: c.fuel_tons + c.ammo_tons + c.stores_tons + c.replacement_tons })
        .collect();
    convoys.sort_by_key(|c| (c.arrival_gt, c.lane));
    NavalView { ships, ports, convoys }
}

/// The eight phases in which agents act (mirrors `agent::self_play`).
fn is_decision_phase(p: Phase) -> bool {
    matches!(
        p,
        Phase::MovementAndCombat
            | Phase::Organization
            | Phase::LandSupportAir
            | Phase::TruckConvoy
            | Phase::Repair
            | Phase::CommonwealthFleet
            | Phase::NavalConvoyArrival
            | Phase::NavalConvoySchedule
    )
}

fn map_view(g: &GameState) -> MapView {
    let mut hexes: Vec<HexView> = g
        .map
        .coords()
        .map(|c| HexView {
            col: c.col,
            row: c.row,
            terrain: g.map.terrain(c).map(|t| format!("{t:?}")).unwrap_or_default(),
            features: g
                .map
                .hex(c)
                .map(|h| h.features.iter().map(|f| format!("{f:?}")).collect())
                .unwrap_or_default(),
            label: g.map.hex(c).and_then(|h| h.label.clone()),
        })
        .collect();
    hexes.sort_by_key(|h| (h.col, h.row));
    MapView { hexes, hexsides: hexside_views(g) }
}

/// The map's hexside features (routes + edge terrain), deterministically ordered. Serialized once in
/// the header, and re-emitted as an `Event.hexsides` delta whenever they change (rail/road built or
/// destroyed §24.6/§24.66) so the viewer's linear features aren't frozen at the initial state.
fn hexside_views(g: &GameState) -> Vec<HexsideView> {
    let mut hexsides: Vec<HexsideView> = g
        .map
        .all_hexsides()
        .map(|(a, b, hs)| HexsideView {
            a: [a.col, a.row],
            b: [b.col, b.row],
            kind: format!("{:?}", hs.kind),
            high: hs.high_side.map(|h| [h.col, h.row]),
        })
        .collect();
    hexsides.sort_by_key(|h| (h.a, h.b, h.kind.clone()));
    hexsides
}

fn unit_views(g: &GameState) -> Vec<UnitView> {
    let mut v: Vec<UnitView> = g
        .units
        .values()
        // §19.62: a combat-eliminated (0-TOE) non-HQ unit is removed from play and can never be
        // rebuilt — omit it so the viewer drops it. A 0-TOE HQ stays as its formation's cadre.
        .filter(|u| u.total_toe() > 0 || u.is_hq || u.is_tds)
        .map(|u| UnitView {
            id: u.id.0,
            name: u.name.clone(),
            nationality: format!("{:?}", u.nationality),
            side: format!("{:?}", u.nationality.side()),
            hex: g.effective_hex(u.id).map(|h| [h.col, h.row]),
            org_size: format!("{:?}", u.org_size),
            // The counter-symbol primitives: the unit's explicit OB set, or (for an untagged unit)
            // the classified arm family mapped to its canonical primitives.
            unit_type: {
                let prims = match u.unit_type {
                    Some(t) => t.glyph(),
                    None => g.org_family_of(u.id).map(cna_engine::reference::formation_org::org_family_to_primitives).unwrap_or_default(),
                };
                prims.iter().map(|p| format!("{p:?}")).collect()
            },
            toe: u.total_toe(),
            fuel: u.fuel,
            ammo: u.ammo,
            cohesion: u.cohesion,
            basic_morale: u.basic_morale,
            morale_reference: u.basic_morale_reference,
            in_training: u.status.in_training,
            broken: !u.weapons_broken.is_empty()
                || u.toe_broken > 0
                || u.trucks_broken.light + u.trucks_broken.medium + u.trucks_broken.heavy > 0,
            is_hq: u.is_hq,
        })
        .collect();
    v.sort_by_key(|u| u.id);
    v
}

/// Replacement-Point pools (§20): on-map/training pools (`game.replacement_points`) then staged
/// future arrivals (`game.pending_replacements`). Deterministic order for stable delta comparison.
fn replacement_views(g: &GameState) -> Vec<ReplacementView> {
    let mut v: Vec<ReplacementView> = Vec::new();
    for rp in &g.replacement_points {
        v.push(ReplacementView {
            class: format!("{:?}", rp.class),
            count: rp.count,
            side: format!("{:?}", rp.side),
            hex: rp.location.as_ref().and_then(|l| l.hex()).map(|h| [h.col, h.row]),
            training_remaining: rp.training_remaining,
            assigned_to: rp.assigned_to.map(|u| u.0),
            pending: false,
            arrival: None,
            carried_by: rp.carried_by.map(|i| i as u32),
        });
    }
    for pr in &g.pending_replacements {
        v.push(ReplacementView {
            class: format!("{:?}", pr.class),
            count: pr.count,
            side: format!("{:?}", pr.side),
            hex: pr.location.as_ref().and_then(|l| l.hex()).map(|h| [h.col, h.row]),
            training_remaining: 0,
            assigned_to: pr.assigned_to.map(|u| u.0),
            pending: true,
            arrival: Some([pr.game_turn, pr.stage as u32]),
            carried_by: None,
        });
    }
    v
}

/// On-map Supply Dumps (§24.9), for the viewer to glyph. Deterministic order for stable delta comparison.
fn supply_dump_views(g: &GameState) -> Vec<SupplyDumpView> {
    g.supply_dumps
        .iter()
        .filter_map(|d| {
            let hex = d.location.hex()?;
            Some(SupplyDumpView {
                hex: [hex.col, hex.row],
                side: format!("{:?}", d.side),
                fuel: d.supply.fuel,
                ammo: d.supply.ammo,
                stores: d.supply.stores,
                water: d.supply.water,
                dummy: d.dummy,
                constructed: d.constructed,
            })
        })
        .collect()
}

/// Free truck convoys (§53), for the viewer to glyph. Deterministic order for stable delta comparison.
fn truck_convoy_views(g: &GameState) -> Vec<TruckConvoyView> {
    g.truck_convoys
        .iter()
        .enumerate()
        .map(|(i, c)| TruckConvoyView {
            id: i as u32,
            hex: [c.hex.col, c.hex.row],
            side: format!("{:?}", c.side),
            light: c.trucks.light,
            medium: c.trucks.medium,
            heavy: c.trucks.heavy,
            broken: c.trucks_broken.light + c.trucks_broken.medium + c.trucks_broken.heavy,
            fuel: c.cargo.fuel,
            ammo: c.cargo.ammo,
            stores: c.cargo.stores,
            water: c.cargo.water,
            // §27.57: derived from the pools' binding, not a stored flag.
            carries_replacements: g.convoy_carries_replacements(i),
        })
        .collect()
}

/// Desert raiders currently in play (§27), for the viewer. Eliminated/re-forming raiders (no hex) are
/// omitted. Deterministic order for stable delta comparison.
fn raider_views(g: &GameState) -> Vec<RaiderView> {
    g.raiders
        .iter()
        .filter_map(|r| {
            let hex = r.hex?;
            Some(RaiderView {
                hex: [hex.col, hex.row],
                side: format!("{:?}", r.side),
                kind: format!("{:?}", r.kind),
                revealed: r.revealed,
                fuel: r.fuel,
                ammo: r.ammo,
            })
        })
        .collect()
}

/// The Rommel counter (§31) when on the map, as a 0-or-1 element list (so it folds like the other
/// delta layers). Off-map / in-Germany yields an empty list.
fn rommel_views(g: &GameState) -> Vec<RommelView> {
    g.rommel
        .iter()
        .filter_map(|r| {
            let hex = r.hex?;
            Some(RommelView { hex: [hex.col, hex.row], attached_to: r.cpa_unit.map(|u| u.0) })
        })
        .collect()
}

/// Per-section weather `[A, B, C, D, E]` (§29.1), read via the public `weather_at` on one column
/// of each section.
fn weather_vec(game: &GameState) -> Vec<String> {
    (0..5)
        .map(|i| format!("{:?}", game.weather_at(cna_engine::state::map::HexCoord::new(i * 33 + 1, 1))))
        .collect()
}

/// Build an event as a **delta** vs `prev` (the units as of the last event): only changed units go
/// in `units`, removed ids in `removed`. `prev` is updated to the current full set. Weather is a
/// delta too — emitted only when it differs from `prev_weather` (which is then updated).
fn delta_frame(
    game: &GameState,
    step: u32,
    side: &str,
    action: String,
    outcome: String,
    rules: Vec<String>,
    prev: &mut std::collections::HashMap<u32, UnitView>,
    prev_weather: &mut Vec<String>,
    prev_replacements: &mut Vec<ReplacementView>,
    prev_dumps: &mut Vec<SupplyDumpView>,
    prev_convoys: &mut Vec<TruckConvoyView>,
    prev_raiders: &mut Vec<RaiderView>,
    prev_rommel: &mut Vec<RommelView>,
    prev_overlays: &mut MapOverlaysView,
    prev_hexsides: &mut Vec<HexsideView>,
    prev_air: &mut AirView,
    prev_naval: &mut NavalView,
) -> Event {
    let cur = unit_views(game);
    let cur_ids: std::collections::HashSet<u32> = cur.iter().map(|u| u.id).collect();
    let units: Vec<UnitView> = cur.iter().filter(|u| prev.get(&u.id) != Some(*u)).cloned().collect();
    let mut removed: Vec<u32> = prev.keys().copied().filter(|id| !cur_ids.contains(id)).collect();
    removed.sort_unstable();
    *prev = cur.into_iter().map(|u| (u.id, u)).collect();
    let w = weather_vec(game);
    let weather = if w != *prev_weather {
        *prev_weather = w.clone();
        Some(w)
    } else {
        None
    };
    let rp = replacement_views(game);
    let replacements = if rp != *prev_replacements {
        *prev_replacements = rp.clone();
        Some(rp)
    } else {
        None
    };
    let dumps = supply_dump_views(game);
    let supply_dumps = if dumps != *prev_dumps {
        *prev_dumps = dumps.clone();
        Some(dumps)
    } else {
        None
    };
    let convoys = truck_convoy_views(game);
    let truck_convoys = if convoys != *prev_convoys {
        *prev_convoys = convoys.clone();
        Some(convoys)
    } else {
        None
    };
    let rd = raider_views(game);
    let raiders = if rd != *prev_raiders {
        *prev_raiders = rd.clone();
        Some(rd)
    } else {
        None
    };
    let rm = rommel_views(game);
    let rommel = if rm != *prev_rommel {
        *prev_rommel = rm.clone();
        Some(rm)
    } else {
        None
    };
    let ov = map_overlay_views(game);
    let overlays = if ov != *prev_overlays {
        *prev_overlays = ov.clone();
        Some(ov)
    } else {
        None
    };
    let hs = hexside_views(game);
    let hexsides = if hs != *prev_hexsides {
        *prev_hexsides = hs.clone();
        Some(hs)
    } else {
        None
    };
    let av = air_views(game);
    let air = if av != *prev_air {
        *prev_air = av.clone();
        Some(av)
    } else {
        None
    };
    let nv = naval_views(game);
    let naval = if nv != *prev_naval {
        *prev_naval = nv.clone();
        Some(nv)
    } else {
        None
    };
    Event {
        step,
        game_turn: game.game_turn,
        operations_stage: game.operations_stage,
        phase: format!("{:?}", game.phase),
        side: side.to_string(),
        action,
        outcome,
        rules,
        units,
        removed,
        weather,
        replacements,
        supply_dumps,
        truck_convoys,
        raiders,
        rommel,
        overlays,
        hexsides,
        air,
        naval,
    }
}

/// Run `game` to `max_turns` with the two agents, recording an [`Event`] per accepted command.
/// Deterministic given the game seed and the agents' seeds.
#[allow(clippy::too_many_arguments)]
pub fn record_game(
    mut game: GameState,
    scenario: &str,
    seed: u64,
    commonwealth_name: &str,
    commonwealth: &mut dyn Agent,
    axis_name: &str,
    axis: &mut dyn Agent,
    max_turns: u32,
) -> GameRecord {
    let map = map_view(&game);
    let mut prev: std::collections::HashMap<u32, UnitView> = std::collections::HashMap::new();
    let mut prev_weather: Vec<String> = Vec::new();
    let mut prev_replacements: Vec<ReplacementView> = Vec::new();
    let mut prev_dumps: Vec<SupplyDumpView> = Vec::new();
    let mut prev_convoys: Vec<TruckConvoyView> = Vec::new();
    let mut prev_raiders: Vec<RaiderView> = Vec::new();
    let mut prev_rommel: Vec<RommelView> = Vec::new();
    let mut prev_overlays: MapOverlaysView = MapOverlaysView::default();
    let mut prev_hexsides: Vec<HexsideView> = hexside_views(&game);
    let mut prev_air: AirView = AirView::default();
    let mut prev_naval: NavalView = NavalView::default();
    let mut events = vec![delta_frame(&game, 0, "-", "start".into(), String::new(), Vec::new(), &mut prev, &mut prev_weather, &mut prev_replacements, &mut prev_dumps, &mut prev_convoys, &mut prev_raiders, &mut prev_rommel, &mut prev_overlays, &mut prev_hexsides, &mut prev_air, &mut prev_naval)];
    let mut step = 0u32;
    let mut guard = 0u64;
    while game.game_turn <= max_turns {
        // Stop early on a §64.71/§64.72 automatic campaign win (None for non-campaign scenarios).
        if game.campaign_auto_winner().is_some() {
            break;
        }
        if is_decision_phase(game.phase) {
            let side = game.active_player;
            let on_map = game
                .units
                .values()
                .filter(|u| u.nationality.side() == side && u.hex().is_some())
                .count();
            let cap = 3 * on_map as u32 + 8;
            // Snapshot the legal set once and drain it (see agent::drain_phase); recompute only when
            // the agent yields after progress. Records a frame per successfully applied command.
            let mut legal = game.legal_commands(side);
            let mut acted = 0u32;
            let mut progress = true;
            while acted < cap {
                let agent: &mut dyn Agent = if side == Side::Commonwealth { commonwealth } else { axis };
                match agent.choose_index(&game, side, &legal) {
                    Some(i) if i < legal.len() => {
                        let cmd = legal.swap_remove(i);
                        let action = format!("{cmd:?}");
                        let rules: Vec<String> = cmd.rule_refs().iter().map(|s| s.to_string()).collect();
                        if let Ok(outcome) = game.apply(cmd) {
                            step += 1;
                            events.push(delta_frame(&game, step, &format!("{side:?}"), action, format!("{outcome:?}"), rules, &mut prev, &mut prev_weather, &mut prev_replacements, &mut prev_dumps, &mut prev_convoys, &mut prev_raiders, &mut prev_rommel, &mut prev_overlays, &mut prev_hexsides, &mut prev_air, &mut prev_naval));
                            progress = true;
                        }
                        acted += 1;
                    }
                    _ => {
                        if !progress {
                            break;
                        }
                        legal = game.legal_commands(side);
                        progress = false;
                    }
                }
            }
        }
        game.advance_phase();
        guard += 1;
        assert!(guard < 5_000_000, "record loop failed to progress");
    }
    GameRecord {
        scenario: scenario.to_string(),
        seed,
        commonwealth_agent: commonwealth_name.to_string(),
        axis_agent: axis_name.to_string(),
        map,
        unit_types: ob_unit_type_glyphs(&game),
        events,
    }
}

/// Like [`record_game`] but streams the log as **JSONL** to `out` instead of buffering the whole
/// `GameRecord` in memory: the first line is a header object (scenario/seed/agents/map), then one
/// [`Event`] per line, flushed at each Game-Turn boundary. This gives live progress (via `on_turn`,
/// called with `(game_turn, events_so_far)` whenever the turn advances) and crash/kill-safety, and
/// keeps memory bounded for a full-campaign run. Returns the number of event lines written (excluding
/// the header). The viewer reconstructs a `GameRecord` from `{...header, events: <rest of lines>}`.
#[allow(clippy::too_many_arguments)]
pub fn record_game_streaming(
    mut game: GameState,
    scenario: &str,
    seed: u64,
    commonwealth_name: &str,
    commonwealth: &mut dyn Agent,
    axis_name: &str,
    axis: &mut dyn Agent,
    max_turns: u32,
    out: &mut dyn std::io::Write,
    on_turn: &mut dyn FnMut(u32, u32),
) -> std::io::Result<u32> {
    let header = serde_json::json!({
        "scenario": scenario,
        "seed": seed,
        "commonwealth_agent": commonwealth_name,
        "axis_agent": axis_name,
        "map": map_view(&game),
        "unit_types": ob_unit_type_glyphs(&game),
    });
    writeln!(out, "{}", serde_json::to_string(&header)?)?;

    let mut prev: std::collections::HashMap<u32, UnitView> = std::collections::HashMap::new();
    let mut prev_weather: Vec<String> = Vec::new();
    let mut prev_replacements: Vec<ReplacementView> = Vec::new();
    let mut prev_dumps: Vec<SupplyDumpView> = Vec::new();
    let mut prev_convoys: Vec<TruckConvoyView> = Vec::new();
    let mut prev_raiders: Vec<RaiderView> = Vec::new();
    let mut prev_rommel: Vec<RommelView> = Vec::new();
    let mut prev_overlays: MapOverlaysView = MapOverlaysView::default();
    let mut prev_hexsides: Vec<HexsideView> = hexside_views(&game);
    let mut prev_air: AirView = AirView::default();
    let mut prev_naval: NavalView = NavalView::default();
    let start = delta_frame(&game, 0, "-", "start".into(), String::new(), Vec::new(), &mut prev, &mut prev_weather, &mut prev_replacements, &mut prev_dumps, &mut prev_convoys, &mut prev_raiders, &mut prev_rommel, &mut prev_overlays, &mut prev_hexsides, &mut prev_air, &mut prev_naval);
    writeln!(out, "{}", serde_json::to_string(&start)?)?;

    let mut step = 0u32;
    let mut guard = 0u64;
    let mut last_turn = game.game_turn;
    on_turn(game.game_turn, step);
    while game.game_turn <= max_turns {
        if game.campaign_auto_winner().is_some() {
            break;
        }
        if is_decision_phase(game.phase) {
            let side = game.active_player;
            let on_map = game
                .units
                .values()
                .filter(|u| u.nationality.side() == side && u.hex().is_some())
                .count();
            let cap = 3 * on_map as u32 + 8;
            // Snapshot-and-drain (see agent::drain_phase): compute the legal set once per phase and
            // recompute only after the agent yields with progress, instead of per action.
            let mut legal = game.legal_commands(side);
            let mut acted = 0u32;
            let mut progress = true;
            while acted < cap {
                let agent: &mut dyn Agent = if side == Side::Commonwealth { commonwealth } else { axis };
                match agent.choose_index(&game, side, &legal) {
                    Some(i) if i < legal.len() => {
                        let cmd = legal.swap_remove(i);
                        let action = format!("{cmd:?}");
                        let rules: Vec<String> = cmd.rule_refs().iter().map(|s| s.to_string()).collect();
                        if let Ok(outcome) = game.apply(cmd) {
                            step += 1;
                            let ev = delta_frame(&game, step, &format!("{side:?}"), action, format!("{outcome:?}"), rules, &mut prev, &mut prev_weather, &mut prev_replacements, &mut prev_dumps, &mut prev_convoys, &mut prev_raiders, &mut prev_rommel, &mut prev_overlays, &mut prev_hexsides, &mut prev_air, &mut prev_naval);
                            writeln!(out, "{}", serde_json::to_string(&ev)?)?;
                            progress = true;
                        }
                        acted += 1;
                    }
                    _ => {
                        if !progress {
                            break;
                        }
                        legal = game.legal_commands(side);
                        progress = false;
                    }
                }
            }
        }
        game.advance_phase();
        if game.game_turn != last_turn {
            last_turn = game.game_turn;
            out.flush()?;
            on_turn(game.game_turn, step);
        }
        guard += 1;
        assert!(guard < 5_000_000, "record loop failed to progress");
    }
    out.flush()?;
    Ok(step)
}

/// A short **scripted** demo of the Replacement-Point lifecycle (§20) for the viewer — a maintained
/// showcase, not agent-driven play. It walks one Infantry pool through: **staged** future arrival
/// (§20.21) → **arrival** on-map at a port and start of **training** (§20.43) → training countdown →
/// **delivery/travel** toward its assigned unit (§20.41) → **absorption** (§20.49), where the unit's
/// TOE jumps and the pool is consumed. State is set between frames to make each stage deterministic.
pub fn record_rp_demo(seed: u64) -> GameRecord {
    use cna_engine::reference::terrain::HexTerrain;
    use cna_engine::reference::units::{Nationality, Side};
    use cna_engine::reference::ReferenceData;
    use cna_engine::state::command::Command;
    use cna_engine::state::game::{Commodities, Commodity, Minefield, PendingReplacement, Phase, Raider, RaiderKind, ReplacementClass, ReplacementPoints, RommelState, SupplyDump, TruckConvoy};
    use cna_engine::state::map::{HexCoord, HexsideKind};
    use cna_engine::state::unit::{Composition, TruckPoints, UnitLocation};

    let mut game = GameState::new(ReferenceData::load_campaign(), seed);
    for col in 0..6 {
        game.map.set_hex(HexCoord::new(col, 1), HexTerrain::Clear);
    }
    // A badly-reduced Commonwealth infantry battalion (TOE 1) that will be rebuilt by the replacements.
    let unit = game.spawn_unit(Nationality::Commonwealth, "h", "1st Rifle Bde", Composition::Organic { current_toe: 1 });
    game.unit_mut(unit).unwrap().location = Some(UnitLocation::OnMap(HexCoord::new(4, 1)));
    // A depot Supply Dump at the start hex (0,1). The convoy picks up its Fuel and then hauls the pool
    // forward burning that carried Fuel (§49.18) — no en-route dumps are needed.
    game.add_supply_dump_full(UnitLocation::OnMap(HexCoord::new(0, 1)), Side::Commonwealth, Commodities { fuel: 30, ammo: 5, stores: 5, water: 5 });
    // A §32.18 dummy dump at (5,1) to show the "D"-labelled glyph variant.
    game.supply_dumps.push(SupplyDump {
        location: UnitLocation::OnMap(HexCoord::new(5, 1)),
        side: Side::Commonwealth,
        supply: Commodities { fuel: 0, ammo: 0, stores: 0, water: 0 },
        dummy: true,
        constructed: true,
    });
    // A Medium truck convoy parked at the depot. It starts idle/unbound — it is only committed to the
    // RP haul once the pool is loaded onto it (step 5, `LoadReplacement`, §20.44/§54.2), after which the
    // pool's `carried_by` binding makes the convoy an escort (§27.58) and it travels with the pool.
    game.truck_convoys.push(TruckConvoy {
        side: Side::Commonwealth,
        hex: HexCoord::new(0, 1),
        trucks: TruckPoints { light: 0, medium: 5, heavy: 0 },
        cargo: Commodities { fuel: 0, ammo: 0, stores: 0, water: 0 },
        cpa_used: 0,
        trucks_broken: TruckPoints { light: 0, medium: 0, heavy: 0 }, breakdown_points: 0, breakdown_checked_col: -1,
    });

    // Demo showcase of the other on-map entities, spread along the (0..5, 1) row:
    //  • pipelines (§52.2) at the depot end;
    //  • a real (spotted) Axis minefield and a Commonwealth dummy (unspotted) minefield (§26);
    //  • the LRDG raider (revealed) and the Axis Sonderkommando raider (concealed) (§27);
    //  • the Rommel counter (§31); and a level-2 fortification (§24.4) on the rebuilt unit's hex.
    // The Commonwealth haul runs (0,1)→(4,1); keep all *enemy* pieces off that route (Rommel has no
    // ZOC and would react-and-retreat anyway, §31, but an enemy counter mid-route reads confusingly),
    // clustering the Axis presence at (5,1) beyond the pool's destination.
    game.pipelines.insert(HexCoord::new(0, 1));
    game.pipelines.insert(HexCoord::new(1, 1));
    // Friendly (Commonwealth) minefields along the route — a real spotted field and a dummy.
    game.minefields.insert(HexCoord::new(1, 1), Minefield { side: Side::Commonwealth, dummy: false });
    game.revealed_minefields.insert(HexCoord::new(1, 1));
    game.minefields.insert(HexCoord::new(2, 1), Minefield { side: Side::Commonwealth, dummy: true });
    // An Axis minefield in the enemy pocket at (5,1).
    game.minefields.insert(HexCoord::new(5, 1), Minefield { side: Side::Axis, dummy: false });
    game.revealed_minefields.insert(HexCoord::new(5, 1));
    game.fortifications.insert(HexCoord::new(4, 1), 2);
    // The LRDG (Commonwealth) raider scouts along the route; the concealed Axis raider + Rommel sit at (5,1).
    game.raiders.push(Raider {
        kind: RaiderKind::Lrdg, side: Side::Commonwealth, hex: Some(HexCoord::new(2, 1)),
        cpa_used: 0, fuel: 3, ammo: 1, drew_water_this_gt: false, reform_game_turn: 0, revealed: true,
    });
    game.raiders.push(Raider {
        kind: RaiderKind::Dsa, side: Side::Axis, hex: Some(HexCoord::new(5, 1)),
        cpa_used: 0, fuel: 2, ammo: 1, drew_water_this_gt: false, reform_game_turn: 0, revealed: false,
    });
    game.rommel = Some(RommelState {
        arrival_game_turn: 0, start_hex: Some(HexCoord::new(5, 1)), hex: Some(HexCoord::new(5, 1)),
        in_germany: false, mcp_left: 0, stage_start_hex: None, cpa_unit: None,
    });
    // A short live railroad running (2,1)→(3,1)→(4,1), and a Commonwealth infantry unit sitting on the
    // (3,1) rail hex. At the end it demolishes that hex itself (§24.66 scorched earth) — a real command
    // by an on-board unit — which flips both rail edges to DestroyedRailroad via a hexside delta.
    game.map.set_hexside(HexCoord::new(2, 1), HexCoord::new(3, 1), HexsideKind::Railroad, None);
    game.map.set_hexside(HexCoord::new(3, 1), HexCoord::new(4, 1), HexsideKind::Railroad, None);
    let sapper = game.spawn_unit(Nationality::Commonwealth, "h", "NZ Rail Coy", Composition::Organic { current_toe: 5 });
    game.unit_mut(sapper).unwrap().location = Some(UnitLocation::OnMap(HexCoord::new(3, 1)));

    let map = map_view(&game);
    let mut prev: std::collections::HashMap<u32, UnitView> = std::collections::HashMap::new();
    let mut prev_weather: Vec<String> = Vec::new();
    let mut prev_rp: Vec<ReplacementView> = Vec::new();
    let mut prev_dumps: Vec<SupplyDumpView> = Vec::new();
    let mut prev_convoys: Vec<TruckConvoyView> = Vec::new();
    let mut prev_raiders: Vec<RaiderView> = Vec::new();
    let mut prev_rommel: Vec<RommelView> = Vec::new();
    let mut prev_overlays: MapOverlaysView = MapOverlaysView::default();
    let mut prev_hexsides: Vec<HexsideView> = hexside_views(&game);
    let mut prev_air: AirView = AirView::default();
    let mut prev_naval: NavalView = NavalView::default();
    let mut events: Vec<Event> = Vec::new();
    let mut step = 0u32;
    let frame = |game: &GameState,
                     step: &mut u32,
                     action: &str,
                     outcome: String,
                     prev: &mut std::collections::HashMap<u32, UnitView>,
                     prev_weather: &mut Vec<String>,
                     prev_rp: &mut Vec<ReplacementView>,
                     prev_dumps: &mut Vec<SupplyDumpView>,
                     prev_convoys: &mut Vec<TruckConvoyView>,
                     prev_raiders: &mut Vec<RaiderView>,
                     prev_rommel: &mut Vec<RommelView>,
                     prev_overlays: &mut MapOverlaysView,
                     prev_hexsides: &mut Vec<HexsideView>,
                     prev_air: &mut AirView,
                     prev_naval: &mut NavalView,
                     events: &mut Vec<Event>| {
        events.push(delta_frame(game, *step, "Commonwealth", action.into(), outcome, Vec::new(), prev, prev_weather, prev_rp, prev_dumps, prev_convoys, prev_raiders, prev_rommel, prev_overlays, prev_hexsides, prev_air, prev_naval));
        *step += 1;
    };

    // 0 — STAGE: a pool is planned to arrive at a future Naval Convoy Arrival Phase (§20.21).
    game.pending_replacements.push(PendingReplacement {
        class: ReplacementClass::Infantry, count: 4, side: Side::Commonwealth,
        game_turn: 3, stage: 1, location: None, assigned_to: Some(unit), tonnage: 0,
    });
    frame(&game, &mut step, "Staged: Infantry replacement pool planned (§20.21)", String::new(), &mut prev, &mut prev_weather, &mut prev_rp, &mut prev_dumps, &mut prev_convoys, &mut prev_raiders, &mut prev_rommel, &mut prev_overlays, &mut prev_hexsides, &mut prev_air, &mut prev_naval, &mut events);

    // 1 — ARRIVE: it lands on-map at a port hex and begins Training (§20.21 → §20.43).
    game.pending_replacements.clear();
    game.replacement_points.push(ReplacementPoints {
        class: ReplacementClass::Infantry, count: 4, side: Side::Commonwealth,
        location: Some(UnitLocation::OnMap(HexCoord::new(0, 1))), training_remaining: 3, assigned_to: Some(unit), cpa_used: 0, carried_by: None,
    });
    frame(&game, &mut step, "Arrived: pool on-map at the port, training begins (§20.21/§20.43)", "ReplacementArrived".into(), &mut prev, &mut prev_weather, &mut prev_rp, &mut prev_dumps, &mut prev_convoys, &mut prev_raiders, &mut prev_rommel, &mut prev_overlays, &mut prev_hexsides, &mut prev_air, &mut prev_naval, &mut events);

    // 2-4 — TRAIN: the pool trains down (§20.43) at the training area.
    for remaining in [2u32, 1, 0] {
        game.replacement_points[0].training_remaining = remaining;
        frame(&game, &mut step, "Training replacement pool (§20.43)", format!("Training: {remaining} stage(s) left"), &mut prev, &mut prev_weather, &mut prev_rp, &mut prev_dumps, &mut prev_convoys, &mut prev_raiders, &mut prev_rommel, &mut prev_overlays, &mut prev_hexsides, &mut prev_air, &mut prev_naval, &mut events);
    }

    // 5 — LOAD: in the Truck Convoy phase, the convoy commits to the haul — the trained pool is loaded
    // onto it (§20.44 `LoadReplacement`, binding the two) and it picks up the depot's Fuel (§53.14).
    game.phase = Phase::TruckConvoy;
    game.active_player = Side::Commonwealth;
    let picked = game.apply(Command::LoadReplacement { convoy: 0, pool: 0 });
    let loaded = game.apply(Command::LoadConvoy { convoy: 0, commodity: Commodity::Fuel, amount: 30 });
    frame(&game, &mut step, "Convoy loads the pool + Fuel at the depot (§20.44/§53.14)", format!("{picked:?}; {loaded:?}"), &mut prev, &mut prev_weather, &mut prev_rp, &mut prev_dumps, &mut prev_convoys, &mut prev_raiders, &mut prev_rommel, &mut prev_overlays, &mut prev_hexsides, &mut prev_air, &mut prev_naval, &mut events);

    // 6-9 — TRAVEL: the pool moves one hex per step toward its unit via §20.46 MoveReplacement; the
    // carrying convoy burns its own carried Fuel (§49.18), so no en-route dumps are needed. Each step
    // is a fresh Operations Stage, so the pool's §20.42d CPA resets between steps.
    for hex in [1i16, 2, 3, 4] {
        game.replacement_points[0].cpa_used = 0; // new stage's Truck Convoy Phase
        let outcome = game.apply(Command::MoveReplacement { index: 0, to: HexCoord::new(hex, 1) });
        frame(&game, &mut step, "Delivering replacement pool toward its unit (§20.46)", format!("{outcome:?}"), &mut prev, &mut prev_weather, &mut prev_rp, &mut prev_dumps, &mut prev_convoys, &mut prev_raiders, &mut prev_rommel, &mut prev_overlays, &mut prev_hexsides, &mut prev_air, &mut prev_naval, &mut events);
    }

    // 7 — ABSORB: co-located and trained, the pool is absorbed and the unit's TOE jumps (§20.49).
    let added = game.absorb_replacement(0, unit).unwrap_or(0);
    frame(&game, &mut step, "Absorbed into the unit (§20.49)", format!("ReplacementAbsorbed({added})"), &mut prev, &mut prev_weather, &mut prev_rp, &mut prev_dumps, &mut prev_convoys, &mut prev_raiders, &mut prev_rommel, &mut prev_overlays, &mut prev_hexsides, &mut prev_air, &mut prev_naval, &mut events);

    // 8 — DEMOLISH: the NZ Rail Coy at (3,1) demolishes its own railroad hex (§24.66 scorched earth) via
    // a real `DemolishRailroad` command; completing the one-stage project flips both rail edges of the
    // hex to DestroyedRailroad — a genuine on-board action, surfaced as a hexside delta.
    game.phase = Phase::Organization;
    game.active_player = Side::Commonwealth;
    let started = game.apply(Command::DemolishRailroad { builder: sapper, a: HexCoord::new(3, 1), b: HexCoord::new(4, 1) });
    game.advance_construction();
    frame(&game, &mut step, "NZ Rail Coy demolishes the railroad at (3,1) (§24.66)", format!("{started:?}"), &mut prev, &mut prev_weather, &mut prev_rp, &mut prev_dumps, &mut prev_convoys, &mut prev_raiders, &mut prev_rommel, &mut prev_overlays, &mut prev_hexsides, &mut prev_air, &mut prev_naval, &mut events);

    GameRecord {
        scenario: "Replacement-Point Lifecycle Demo (§20)".into(),
        seed,
        commonwealth_agent: "scripted".into(),
        axis_agent: "scripted".into(),
        map,
        unit_types: ob_unit_type_glyphs(&game),
        events,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cna_engine::agent::{GreedyAgent, RandomAgent};
    use cna_engine::reference::terrain::HexTerrain;
    use cna_engine::reference::units::Nationality;
    use cna_engine::reference::ReferenceData;
    use cna_engine::state::map::HexCoord;
    use cna_engine::state::unit::{Composition, UnitLocation};

    #[test]
    fn records_a_game_to_a_wellformed_log() {
        let mut g = GameState::new(ReferenceData::load_campaign(), 3);
        for col in 0..6 {
            g.map.set_hex(HexCoord::new(col, 1), HexTerrain::Clear);
        }
        for (nat, id, col) in [
            (Nationality::Commonwealth, "h", 1),
            (Nationality::Italian, "p", 4),
        ] {
            let u = g.spawn_unit(nat, id, "u", Composition::Organic { current_toe: 8 });
            let unit = g.unit_mut(u).unwrap();
            unit.location = Some(UnitLocation::OnMap(HexCoord::new(col, 1)));
            unit.fuel = 400;
            unit.ammo = 400;
        }
        let mut cw = GreedyAgent::new(1);
        let mut ax = RandomAgent::new(2);
        let rec = record_game(g, "T", 3, "Greedy", &mut cw, "Random", &mut ax, 2);

        // Header: static map serialized once, both agent names present.
        assert!(!rec.map.hexes.is_empty(), "map is captured in the header");
        assert_eq!((rec.commonwealth_agent.as_str(), rec.axis_agent.as_str()), ("Greedy", "Random"));
        // Events: an initial frame, then per-command frames with monotonically increasing steps.
        assert!(rec.events.len() >= 2, "at least a start frame and some play");
        assert_eq!(rec.events[0].step, 0);
        assert_eq!(rec.events[0].action, "start");
        for w in rec.events.windows(2) {
            assert_eq!(w[1].step, w[0].step + 1, "steps are contiguous");
        }
        // Step 0 lists all units; later events are deltas (≤ 2 changed). Folding reconstructs both.
        assert_eq!(rec.events[0].units.len(), 2, "step 0 lists all units");
        assert!(rec.events.iter().all(|e| e.units.len() <= 2), "later events are deltas");
        let mut folded: std::collections::HashMap<u32, UnitView> = std::collections::HashMap::new();
        for e in &rec.events {
            for uv in &e.units {
                folded.insert(uv.id, uv.clone());
            }
            for r in &e.removed {
                folded.remove(r);
            }
        }
        assert_eq!(folded.len(), 2, "folding the deltas reconstructs both units");
        assert!(rec.events.last().unwrap().game_turn >= 1);
        // The whole record round-trips through serde_json.
        let json = serde_json::to_string(&rec).unwrap();
        assert!(json.contains("\"scenario\":\"T\"") && json.contains("\"map\""));
    }

    #[test]
    fn surfaces_raiders_and_rommel() {
        use cna_engine::reference::units::Side;
        use cna_engine::state::game::{Raider, RaiderKind, RommelState};

        let mut g = GameState::new(ReferenceData::load_campaign(), 1);
        let hx = HexCoord::new(2, 3);
        // An in-play raider is surfaced; an eliminated/re-forming one (no hex) is omitted.
        g.raiders.push(Raider {
            kind: RaiderKind::Lrdg, side: Side::Commonwealth, hex: Some(hx),
            cpa_used: 0, fuel: 3, ammo: 1, drew_water_this_gt: false, reform_game_turn: 0, revealed: true,
        });
        g.raiders.push(Raider {
            kind: RaiderKind::Dsa, side: Side::Axis, hex: None,
            cpa_used: 0, fuel: 0, ammo: 0, drew_water_this_gt: false, reform_game_turn: 9, revealed: false,
        });
        let rv = raider_views(&g);
        assert_eq!(rv.len(), 1, "only the on-map raider is surfaced");
        assert_eq!((rv[0].kind.as_str(), rv[0].revealed, rv[0].fuel, rv[0].hex), ("Lrdg", true, 3, [2, 3]));

        // Rommel: none off-map, one on-map.
        assert!(rommel_views(&g).is_empty(), "no Rommel counter while off-map");
        g.rommel = Some(RommelState {
            arrival_game_turn: 1, start_hex: Some(hx), hex: Some(hx),
            in_germany: false, mcp_left: 0, stage_start_hex: None, cpa_unit: None,
        });
        let mv = rommel_views(&g);
        assert_eq!(mv.len(), 1);
        assert_eq!(mv[0].hex, [2, 3]);
        // In Germany / off-map → omitted again.
        g.rommel.as_mut().unwrap().hex = None;
        assert!(rommel_views(&g).is_empty());
    }

    #[test]
    fn surfaces_naval() {
        use cna_engine::state::naval::{Convoy, Port, Ship, ShipLocation};
        let mut g = GameState::new(ReferenceData::new(), 1);
        g.fleet.push(Ship { name: "HMS Test".into(), max_gun: 5, gun_rating: 4, max_aa: 2, location: ShipLocation::AtSea(HexCoord::new(3, 3)), at_sea_stages: 0, repair_stages: 0, fired_this_stage: false, rest_owed: 0 });
        // An in-port ship is omitted from the map tiles.
        g.fleet.push(Ship { name: "HMS Port".into(), max_gun: 5, gun_rating: 5, max_aa: 2, location: ShipLocation::Alexandria, at_sea_stages: 0, repair_stages: 0, fired_this_stage: false, rest_owed: 0 });
        g.place_san_giorgio(HexCoord::new(43, 21), "Tobruk", 3, 2);
        g.ports.insert("Tobruk".into(), Port { name: "Tobruk".into(), max_efficiency: 3, efficiency: 1, max_tonnage: 100, mines: 0, wreck_block: 2 });
        g.convoys.push(Convoy { lane: 3, destination: "Benghazi".into(), arrival_gt: 3, fuel_tons: 20, ammo_tons: 20, stores_tons: 20, replacement_tons: 0, cancelled: false });
        g.convoys.push(Convoy { lane: 1, destination: "Tripoli".into(), arrival_gt: 2, fuel_tons: 10, ammo_tons: 0, stores_tons: 0, replacement_tons: 0, cancelled: true });

        let nv = naval_views(&g);
        // AtSea ship + San Giorgio wreck; the in-port ship is omitted.
        assert_eq!(nv.ships.len(), 2);
        assert!(nv.ships.iter().any(|s| s.name == "HMS Test" && !s.wreck && s.hex == [3, 3]));
        assert!(nv.ships.iter().any(|s| s.wreck && s.side == "Axis"));
        // Port state (blocked by the wreck).
        assert_eq!(nv.ports.len(), 1);
        assert_eq!((nv.ports[0].efficiency, nv.ports[0].max_efficiency, nv.ports[0].blocked), (1, 3, true));
        // Cancelled convoy omitted; tonnage summed.
        assert_eq!(nv.convoys.len(), 1);
        assert_eq!((nv.convoys[0].destination.as_str(), nv.convoys[0].tons), ("Benghazi", 60));
    }

    #[test]
    fn surfaces_air_facilities() {
        use cna_engine::state::air::{AirFacility, AirFacilityKind};
        let mut g = GameState::new(ReferenceData::new(), 1);
        g.air_facilities.push(AirFacility { kind: AirFacilityKind::Airfield, hex: HexCoord::new(4, 5), capacity_level: 6, off_map: false, off_map_sgsu_limit: None });
        // An off-map facility is omitted from the viewer.
        g.air_facilities.push(AirFacility { kind: AirFacilityKind::Airfield, hex: HexCoord::new(0, 0), capacity_level: 6, off_map: true, off_map_sgsu_limit: Some(12) });
        let av = air_views(&g);
        assert_eq!(av.facilities.len(), 1, "off-map facility omitted");
        assert_eq!((av.facilities[0].hex, av.facilities[0].kind.as_str(), av.facilities[0].capacity), ([4, 5], "Airfield", 6));
    }

    #[test]
    fn surfaces_map_overlays() {
        use cna_engine::reference::units::Side;
        use cna_engine::state::game::Minefield;

        let mut g = GameState::new(ReferenceData::load_campaign(), 1);
        let a = HexCoord::new(1, 1);
        let b = HexCoord::new(2, 1);
        g.minefields.insert(a, Minefield { side: Side::Axis, dummy: false });
        g.revealed_minefields.insert(a);
        g.minefields.insert(b, Minefield { side: Side::Commonwealth, dummy: true }); // unspotted dummy
        g.fortifications.insert(a, 2);
        g.city_fort_damage.insert(a, 1);
        g.pipelines.insert(b);

        let ov = map_overlay_views(&g);
        assert_eq!(ov.minefields.len(), 2);
        let ma = ov.minefields.iter().find(|m| m.hex == [1, 1]).unwrap();
        assert_eq!((ma.side.as_str(), ma.dummy, ma.revealed), ("Axis", false, true));
        let mb = ov.minefields.iter().find(|m| m.hex == [2, 1]).unwrap();
        assert_eq!((mb.dummy, mb.revealed), (true, false));
        assert_eq!(ov.fortifications.len(), 1);
        assert_eq!((ov.fortifications[0].hex, ov.fortifications[0].level, ov.fortifications[0].damage), ([1, 1], 2, 1));
        assert_eq!(ov.pipelines, vec![[2, 1]]);
    }

    #[test]
    fn records_replacement_point_pools() {
        use cna_engine::reference::units::Side;
        use cna_engine::state::game::{PendingReplacement, ReplacementClass, ReplacementPoints};
        let mut g = GameState::new(ReferenceData::load_campaign(), 7);
        g.map.set_hex(HexCoord::new(0, 1), HexTerrain::Clear);
        g.map.set_hex(HexCoord::new(1, 1), HexTerrain::Clear);
        let u = g.spawn_unit(Nationality::Commonwealth, "h", "1 RB", Composition::Organic { current_toe: 4 });
        g.unit_mut(u).unwrap().location = Some(UnitLocation::OnMap(HexCoord::new(1, 1)));
        // Trained pool, on-map at the unit's hex, assigned for delivery (§20.41/§20.49).
        g.replacement_points.push(ReplacementPoints {
            class: ReplacementClass::Infantry, count: 3, side: Side::Commonwealth,
            location: Some(UnitLocation::OnMap(HexCoord::new(1, 1))), training_remaining: 0, assigned_to: Some(u), cpa_used: 0, carried_by: None,
        });
        // Pool still in training at a training area (§20.43).
        g.replacement_points.push(ReplacementPoints {
            class: ReplacementClass::Tank, count: 2, side: Side::Commonwealth,
            location: Some(UnitLocation::OnMap(HexCoord::new(0, 1))), training_remaining: 4, assigned_to: None, cpa_used: 0, carried_by: None,
        });
        // Staged future arrival, not yet on the map (§20.21).
        g.pending_replacements.push(PendingReplacement {
            class: ReplacementClass::Gun, count: 1, side: Side::Commonwealth,
            game_turn: 5, stage: 2, location: None, assigned_to: None, tonnage: 0,
        });
        let mut cw = GreedyAgent::new(1);
        let mut ax = RandomAgent::new(2);
        let rec = record_game(g, "RP", 7, "Greedy", &mut cw, "Random", &mut ax, 1);

        // Step 0 (before any command) carries all three seeded pools.
        let rp0 = rec.events[0].replacements.as_ref().expect("step 0 carries the seeded pools");
        assert_eq!(rp0.len(), 3);
        let trained = rp0.iter().find(|r| r.class == "Infantry").unwrap();
        assert_eq!((trained.count, trained.training_remaining, trained.pending), (3, 0, false));
        assert_eq!(trained.hex, Some([1, 1]));
        assert_eq!(trained.assigned_to, Some(u.0));
        assert_eq!(rp0.iter().find(|r| r.class == "Tank").unwrap().training_remaining, 4);
        let staged = rp0.iter().find(|r| r.pending).unwrap();
        assert_eq!((staged.class.as_str(), staged.arrival, staged.hex), ("Gun", Some([5, 2]), None));
        assert!(serde_json::to_string(&rec).unwrap().contains("\"replacements\""));
    }
}
