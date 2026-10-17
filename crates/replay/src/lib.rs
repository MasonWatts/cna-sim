//! Records a deterministic CNA-Sim game as a JSONL event log for the spectator website.
//!
//! See `docs/visualization-architecture.md`. The header line carries the scenario/seed/agents and
//! the static map (serialized **once**); each subsequent line is one `Event` carrying the acting
//! side, the raw command + outcome, a friendly plain-language `summary`, its `§` rule citations
//! (`Command::rule_refs`), a jump-to-activity target, and the *delta* of dynamic state since the
//! previous step, so a dumb (no-engine) frontend folds the deltas to render the board at every step.
//! The `record` bin streams this to a `.jsonl` file (gzipped to `game.jsonl.gz` for the web asset).
//! The raw command/outcome are still `Debug` strings; a round-trippable `Command` schema for the
//! future WASM replay / MCP interface is a follow-on.


pub mod scenarios;
use cna_engine::agent::Agent;
use cna_engine::reference::units::Side;
use cna_engine::state::air::SquadronId;
use cna_engine::state::command::{Command, CommandOutcome};
use cna_engine::state::game::{GameState, Phase};
use cna_engine::state::map::HexCoord;
use cna_engine::state::unit::UnitId;
use serde::Serialize;
use ts_rs::TS;

/// A complete recorded game: header (scenario, seed, agents, static map) + the ordered event log.
///
/// The `#[derive(TS)]` on this and the types below generates the frontend's TypeScript types into
/// `crates/replay/bindings/` when `cargo test` runs — the event-log schema is single-sourced here.
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
    /// Human-readable command (v0: `Debug`) — the raw command, kept for reference.
    pub action: String,
    /// Human-readable command outcome (v0: `Debug`).
    pub outcome: String,
    /// Friendly, plain-language description of the action (see `describe_command`). Empty for the
    /// initial frame; empty on older logs, where the viewer falls back to prettifying `action`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub summary: String,
    /// Where the viewer's "jump to activity" control should center for this action, or `null` when
    /// there is no clear target (production / strategic actions). See [`ActivityTarget`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity: Option<ActivityTarget>,
    /// `§` rule citations governing this action (from `Command::rule_refs`), shown in the viewer's
    /// "Relevant Rules" panel. Every command carries at least one; the friendly `summary` has none.
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
    /// The off-map §8.8 Tripoli/Tunisia layer — boxes + occupants. **Delta**: `Some` only when it
    /// changes. Fold forward.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub off_map: Option<OffMapView>,
    /// Prisoner-of-war stacks and escaped prisoners (§28). **Delta**: `Some` only when they change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prisoners: Option<Vec<PrisonerView>>,
    /// Captured equipment + knocked-out tank markers (§28.3/§14.46). **Delta**: `Some` on change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured: Option<Vec<CapturedView>>,
    /// Dummy (decoy) tank markers (§32). **Delta**: `Some` on change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dummy_tanks: Option<Vec<DummyTankView>>,
    /// Broken-down vehicle markers awaiting repair (§21.5). **Delta**: `Some` on change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub broken_vehicles: Option<Vec<BrokenVehicleView>>,
}

#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct UnitView {
    pub id: u32,
    pub name: String,
    /// §4.44 Counter Abbreviation — the short counter name the viewer shows as the tile title.
    /// Empty when the OB gives none (the viewer falls back to `name`).
    pub abbr: String,
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
    /// §15 Actual offensive close-assault Strength Points (0 for HQ / pure support).
    pub attack: i32,
    /// §15 Actual defensive close-assault Strength Points.
    pub defense: i32,
    /// §14 Actual Anti-Armor Strength Points (anti-tank fire).
    pub anti_armor: i32,
    /// §13 Actual Barrage Strength Points (artillery); 0 for non-gun units.
    pub barrage: i32,
    /// §6 governing Capability Point Allowance this stage (motorized / Rommel adjusted). `null` when
    /// the unit has no CPA (e.g. off-map / not independently on the map).
    pub cpa: Option<i32>,
    /// Capability Points already expended this Operations Stage, in CP (CPA − this = remaining).
    pub cp_expended: f32,
    /// §6.17 whether the unit moves at the motorized rate (vs leg/foot).
    pub motorized: bool,
    /// §23 whether the unit's current supply (fuel/ammo/stores/water) is adequate this stage.
    pub supplied: bool,
    /// §19.1 administrative parent — the higher formation this unit is assigned to (its HQ's name).
    /// `null` for a top-level formation HQ (which *is* the formation) or an unassigned unit.
    pub formation: Option<String>,
    /// Whether the administrative parent is currently co-located (same effective hex) with this unit,
    /// vs. detached elsewhere. `null` when there is no assigned parent, or the parent has no hex.
    pub formation_colocated: Option<bool>,
    /// §19.1 the counter this unit is physically attached to and rides with, if any.
    pub attached_to: Option<String>,
    /// §19.1 names of units physically attached to (riding on) this counter.
    pub attached_units: Vec<String>,
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
    /// §57: an effectively bottomless base (Cairo/Alexandria) — shown as ∞.
    pub unlimited: bool,
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
    /// Fuel carried as cargo (deliverable).
    pub fuel: u32,
    /// §49.14: Fuel in the trucks' own tanks, and the tank capacity.
    pub tank_fuel: u32,
    pub tank_capacity: u32,
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

/// A §28 prisoner stack (POWs) or a group of escaped prisoners, for the viewer.
#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct PrisonerView {
    pub hex: [i16; 2],
    /// `"Pow"` (a guarded prisoner stack) | `"Escapee"` (escaped, heading home, §28.24).
    pub kind: String,
    /// The army the prisoners belong to.
    pub side: String,
    /// The capturing side guarding a POW stack; empty for escapees.
    pub captor: String,
    pub points: u32,
    /// §28.2 Guard Points watching a POW stack; 0 for escapees.
    pub guards: u32,
}

/// A §28.3/§14.46 pile of captured (or knocked-out) equipment on the map, for the viewer.
#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct CapturedView {
    pub hex: [i16; 2],
    /// The side currently holding it.
    pub captor: String,
    /// The equipment's original nationality (its weapon system / repair rules apply).
    pub nationality: String,
    pub system: String,
    pub count: u32,
    /// §28.3: usable only once repaired.
    pub repaired: bool,
    /// §14.46: a knocked-out tank marker (vs surrendered kit) — repairable by either side.
    pub destroyed_tank: bool,
}

/// A §32 dummy (decoy) tank marker on the map. Indistinguishable from a real armour counter to the
/// enemy; the viewer shows it to the spectator.
#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct DummyTankView {
    pub hex: [i16; 2],
    pub side: String,
    pub designation: String,
}

/// A §21.5 broken-down vehicle marker awaiting tow/repair, for the viewer.
#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct BrokenVehicleView {
    pub hex: [i16; 2],
    /// The side currently owning the marker (flips on capture, §21.52).
    pub side: String,
    pub nationality: String,
    /// Total broken points held (weapon TOE + organic TOE + truck points), for the counter value.
    pub points: u32,
    /// A short breakdown for the tooltip (systems, TOE, trucks).
    pub detail: String,
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

/// A poisoned village/bir well (§52.16), for the viewer. The spectator sees the true state; the
/// `*_knows` flags reflect §52.14 secrecy — which side(s) currently know the well is poisoned (the
/// side that salted it, plus everyone once it's been publicly revealed).
#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct PoisonedWellView {
    pub hex: [i16; 2],
    /// Whether the Commonwealth knows this well is poisoned (§52.14).
    pub cw_knows: bool,
    /// Whether the Axis knows this well is poisoned (§52.14).
    pub axis_knows: bool,
}

/// Static-ish map overlays that change during play: minefields (§26), fortifications (§24.4),
/// water-pipeline hexes (§52.2), and poisoned wells (§52.16). Bundled into one delta layer so the
/// log carries them together.
#[derive(Serialize, TS, Clone, PartialEq, Default)]
#[ts(export)]
pub struct MapOverlaysView {
    pub minefields: Vec<MinefieldView>,
    pub fortifications: Vec<FortView>,
    /// `[col, row]` of each hex holding a water pipeline (§52.2).
    pub pipelines: Vec<[i16; 2]>,
    pub poisoned_wells: Vec<PoisonedWellView>,
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
    let mut poisoned_wells: Vec<PoisonedWellView> = g
        .poisoned_wells
        .iter()
        .map(|&h| PoisonedWellView {
            hex: [h.col, h.row],
            cw_knows: g.observe_well_poisoned(Side::Commonwealth, h) == Some(true),
            axis_knows: g.observe_well_poisoned(Side::Axis, h) == Some(true),
        })
        .collect();
    poisoned_wells.sort_by_key(|w| (w.hex[0], w.hex[1]));
    MapOverlaysView { minefields, fortifications, pipelines, poisoned_wells }
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
    /// Squadrons currently stationed here (SGSUs whose location is this hex).
    pub occupancy: u32,
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
    /// §43: the off-map Axis Mediterranean bomber bases (Italy/Sicily and Crete) and the squadrons
    /// based in each. Always both boxes, even when empty. These squadrons have no map hex.
    #[serde(default)]
    pub med_bases: Vec<MedAirBaseView>,
    /// §44: the Malta air facilities — kind, capacity levels, based-plane count, and diverted flak.
    #[serde(default)]
    pub malta: Vec<MaltaBaseView>,
    /// §36.53: the Commonwealth off-map air facilities (the Canal-zone boxes + Ethiopia) and the
    /// squadrons based in each. Always every chart facility, even when empty.
    #[serde(default)]
    pub cw_off_map: Vec<CwOffMapBaseView>,
}

/// A §44 Malta air facility for the viewer.
#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct MaltaBaseView {
    pub name: String,
    /// `"Airfield"` | `"FlyingBoatBasin"` | `"FlyingBoatAlightingArea"`.
    pub kind: String,
    pub levels: u32,
    pub max_levels: u32,
    /// §44.14 planes currently based here (capacity = levels × 18).
    pub planes: u32,
    /// §44.18 diverted Light flak points at this field.
    pub flak_light: u32,
    /// §44.18 diverted Heavy flak points at this field.
    pub flak_heavy: u32,
    /// §44.14: loose planes grouped by aircraft type (Malta uses no SGSUs) — rendered as
    /// squadron-style counters. Sorted by descending plane count so the first is the representative.
    #[serde(default)]
    pub groups: Vec<MaltaPlaneGroupView>,
}

/// A §44.14 group of Malta planes of one aircraft type, shown as a squadron-style counter.
#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct MaltaPlaneGroupView {
    pub aircraft_type: String,
    /// The representative squadron class for the counter glyph (`"Fighter"|"Bomber"|"Transport"`).
    pub class: String,
    pub planes: u32,
}

/// A squadron based off-map in a §43 Mediterranean box (no hex; otherwise like `SquadronView`).
#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct MedSquadronView {
    pub side: String,
    pub class: String,
    pub designation: String,
    pub planes: u32,
    pub supplied: bool,
}

/// A §43 off-map Mediterranean bomber base and its based squadrons.
#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct MedAirBaseView {
    /// `"Italy/Sicily"` or `"Crete"`.
    pub name: String,
    pub squadrons: Vec<MedSquadronView>,
}

/// A §36.53 Commonwealth off-map air facility (a Canal-zone box or Ethiopia) and its based squadrons.
#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct CwOffMapBaseView {
    pub name: String,
    /// The on-map entry/exit hex (e.g. `"E1833"`).
    pub entry_hex: String,
    /// Printed squadron capacity; `null` = unlimited (Ethiopia).
    pub max_squadrons: Option<u32>,
    /// A flying-boat basin (§36.3 — Port Said).
    pub flying_boat: bool,
    pub squadrons: Vec<MedSquadronView>,
}

fn air_views(g: &GameState) -> AirView {
    let mut facilities: Vec<AirFacilityView> = g
        .air_facilities
        .iter()
        .filter(|f| !f.off_map)
        .map(|f| AirFacilityView {
            hex: [f.hex.col, f.hex.row],
            kind: format!("{:?}", f.kind),
            capacity: f.capacity_level,
            occupancy: g.sgsus.values().filter(|s| s.location == Some(f.hex)).count() as u32,
        })
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
    // §43 off-map Mediterranean bomber bases — always both boxes, populated from med-based SGSUs.
    use cna_engine::state::air::MedBase;
    let med_squadrons = |base: MedBase| -> Vec<MedSquadronView> {
        let mut v: Vec<MedSquadronView> = g
            .sgsus
            .values()
            .filter(|s| s.med_base == Some(base))
            .map(|s| MedSquadronView {
                side: format!("{:?}", s.nationality.side()),
                class: format!("{:?}", s.class),
                designation: s.designation.clone(),
                planes: s.planes.iter().map(|p| p.count).sum(),
                supplied: s.supplied,
            })
            .collect();
        v.sort_by(|a, b| a.designation.cmp(&b.designation));
        v
    };
    let med_bases = vec![
        MedAirBaseView { name: "Italy/Sicily".into(), squadrons: med_squadrons(MedBase::ItalySicily) },
        MedAirBaseView { name: "Crete".into(), squadrons: med_squadrons(MedBase::Crete) },
    ];
    use cna_engine::reference::squadron::SquadronClass;
    let cw_air = g.reference.aircraft_charts.get(&cna_engine::reference::units::Nationality::Commonwealth);
    let malta: Vec<MaltaBaseView> = g
        .malta_bases
        .iter()
        .map(|b| {
            let mut groups: Vec<MaltaPlaneGroupView> = b
                .planes
                .iter()
                .map(|(ty, count)| {
                    // §44.14: Malta stores loose planes by type; derive the display class from the
                    // aircraft chart (default Bomber if the type is unknown to the chart).
                    let class = cw_air
                        .and_then(|c| c.get(ty))
                        .map(|a| SquadronClass::for_aircraft(a.class))
                        .unwrap_or(SquadronClass::Bomber);
                    MaltaPlaneGroupView { aircraft_type: ty.clone(), class: format!("{class:?}"), planes: *count }
                })
                .collect();
            groups.sort_by(|a, b| b.planes.cmp(&a.planes).then(a.aircraft_type.cmp(&b.aircraft_type)));
            MaltaBaseView {
                name: b.name.clone(),
                kind: format!("{:?}", b.kind),
                levels: b.levels,
                max_levels: b.max_levels,
                planes: b.plane_count(),
                flak_light: b.flak_light,
                flak_heavy: b.flak_heavy,
                groups,
            }
        })
        .collect();
    // §36.53 Commonwealth off-map facilities — always every chart row, populated from CW SGSUs whose
    // `off_map_box` names that facility.
    let cw_off_map: Vec<CwOffMapBaseView> = g
        .reference
        .off_map_air_facilities
        .as_ref()
        .map(|chart| {
            chart
                .facilities
                .iter()
                .map(|f| {
                    let mut squadrons: Vec<MedSquadronView> = g
                        .sgsus
                        .values()
                        .filter(|s| s.off_map_box.as_deref() == Some(f.name.as_str()))
                        .map(|s| MedSquadronView {
                            side: format!("{:?}", s.nationality.side()),
                            class: format!("{:?}", s.class),
                            designation: s.designation.clone(),
                            planes: s.planes.iter().map(|p| p.count).sum(),
                            supplied: s.supplied,
                        })
                        .collect();
                    squadrons.sort_by(|a, b| a.designation.cmp(&b.designation));
                    CwOffMapBaseView {
                        name: f.name.clone(),
                        entry_hex: f.entry_hex.clone(),
                        max_squadrons: f.max_squadrons,
                        flying_boat: f.flying_boat,
                        squadrons,
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    AirView { facilities, squadrons, caps, med_bases, malta, cw_off_map }
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
    /// §55.3: supply tonnage/Operations Stage at full efficiency (the base capacity).
    pub max_tonnage: u32,
    /// Current tonnage/Operations Stage at the present efficiency (base × efficiency ÷ max_efficiency).
    pub throughput: u32,
    /// §55.25/§55.27: an efficiency level is held down by harbour mines or a scuttled wreck.
    pub blocked: bool,
    /// §55.13: personnel (TOE Stacking Point) capacity in / out per Operations Stage. `None` where
    /// the chart leaves it unspecified.
    pub stacking_in: Option<u32>,
    pub stacking_out: Option<u32>,
    /// §30.5/§55.13: whether a naval troop transfer has already used this port this Operations Stage
    /// (one transfer per port per stage).
    #[serde(default)]
    pub transferred_this_stage: bool,
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
    /// Fleet ship names, index-aligned with the engine's `ship_index` (all ships, in port or at sea),
    /// so the viewer can name a ship referenced by a command's `ship_index`.
    pub fleet_names: Vec<String>,
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
        .map(|p| PortStateView {
            name: p.name.clone(),
            efficiency: p.efficiency,
            max_efficiency: p.max_efficiency,
            max_tonnage: p.max_tonnage,
            throughput: if p.max_efficiency > 0 { p.max_tonnage * p.efficiency / p.max_efficiency } else { 0 },
            blocked: p.mines > 0 || p.wreck_block > 0,
            stacking_in: p.stacking_in,
            stacking_out: p.stacking_out,
            transferred_this_stage: g.ports_transferred_this_stage.contains(&p.name),
        })
        .collect();
    ports.sort_by(|a, b| a.name.cmp(&b.name));
    let mut convoys: Vec<NavalConvoyView> = g
        .convoys
        .iter()
        .filter(|c| !c.cancelled)
        .map(|c| NavalConvoyView { lane: c.lane, destination: c.destination.clone(), arrival_gt: c.arrival_gt, tons: c.fuel_tons + c.ammo_tons + c.stores_tons + c.replacement_tons })
        .collect();
    convoys.sort_by_key(|c| (c.arrival_gt, c.lane));
    let fleet_names: Vec<String> = g.fleet.iter().map(|s| s.name.clone()).collect();
    NavalView { ships, ports, convoys, fleet_names }
}

/// A unit sitting in or transiting an off-map §8.8 box (Tripoli/Tunisia complex).
#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct OffMapUnitView {
    /// The full unit counter (TOE, type glyph, status) — rendered like an on-map tile.
    pub unit: UnitView,
    /// Operations Stages already spent on the current leg (§8.84); 0 = resting in the box.
    pub transit: u8,
    /// Heading toward the map (Nofilia) rather than inland.
    pub toward_map: bool,
    /// Total Operations Stages of the current leg for this unit's CPA (§8.89); 0 = resting in the
    /// box. Lets the viewer place the unit on a track of the right length at step `transit`.
    #[serde(default)]
    pub leg_total: u8,
}

/// One off-map node (box) and its occupants.
#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct OffMapNodeView {
    pub name: String,
    pub units: Vec<OffMapUnitView>,
    /// §8.8 box AA value.
    #[serde(default)]
    pub aa_points: u32,
    /// The debarkation port for this box, if any (Tripoli → "Tripoli", Tunis → "Bizerta").
    #[serde(default)]
    pub port: Option<String>,
    /// The toward-map leg's cost in Operations Stages at the fast (CPA ≥25), medium (CPA 15–24), and
    /// slow (CPA <15) tiers (§8.89) — the transit-track lengths from this box to the next box closer
    /// to the map.
    #[serde(default)]
    pub leg_fast: u8,
    #[serde(default)]
    pub leg_mid: u8,
    #[serde(default)]
    pub leg_slow: u8,
}

/// The off-map Tripoli/Tunisia complex (§8.8): the Axis-only chain of boxes and their occupants,
/// in MAP ORDER (nearest the map/Nofilia first). Every node appears even when empty so the panel
/// is stable.
#[derive(Serialize, TS, Clone, PartialEq, Default)]
#[ts(export)]
pub struct OffMapView {
    pub nodes: Vec<OffMapNodeView>,
}

fn off_map_views(g: &GameState) -> OffMapView {
    use cna_engine::state::unit::UnitLocation;
    // Map order: nearest the map (Nofilia) first, extending inland/west.
    const NODES: [&str; 4] = ["Tripolitania", "Tripoli", "Gabes", "Tunis"];
    let net = g.reference.off_map.as_ref();
    let nodes = NODES
        .iter()
        .map(|&node| {
            let mut units: Vec<OffMapUnitView> = g
                .units
                .iter()
                .filter_map(|(&id, u)| match &u.location {
                    Some(UnitLocation::OffMap { node: n, transit, toward_map }) if n == node => {
                        // §8.89: the current leg's length for this unit's CPA — the toward-map leg
                        // lives on this node, the inland leg on the next box out. 0 while resting.
                        let leg_total = if *transit == 0 {
                            0
                        } else {
                            net.and_then(|net| {
                                let i = net.index_of(node)?;
                                let cpa = g.formation_cpa(id).or_else(|| g.unit_cpa(id)).unwrap_or(0);
                                let leg = if *toward_map { &net.nodes[i].leg } else { &net.nodes.get(i + 1)?.leg };
                                Some(leg.stages_for_cpa(cpa) as u8)
                            })
                            .unwrap_or(0)
                        };
                        Some(OffMapUnitView {
                            unit: unit_view(g, u),
                            transit: *transit,
                            toward_map: *toward_map,
                            leg_total,
                        })
                    }
                    _ => None,
                })
                .collect();
            units.sort_by(|a, b| a.unit.name.cmp(&b.unit.name));
            let n = net.and_then(|net| net.node(node));
            OffMapNodeView {
                name: node.to_string(),
                units,
                aa_points: n.map(|n| n.aa_points as u32).unwrap_or(0),
                port: n.and_then(|n| n.port.clone()),
                leg_fast: n.map(|n| n.leg.cpa_high).unwrap_or(0),
                leg_mid: n.map(|n| n.leg.cpa_mid).unwrap_or(0),
                leg_slow: n.map(|n| n.leg.cpa_low).unwrap_or(0),
            }
        })
        .collect();
    OffMapView { nodes }
}

/// The phases in which agents act (mirrors `agent::self_play`).
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
            | Phase::AircraftMaintenance
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

/// Build the viewer `UnitView` for a single unit — shared by the on-map unit layer and the off-map
/// complex so both render identical counters (TOE, glyph, status).
// ── Friendly action descriptions ────────────────────────────────────────────────────────────────
// Turn an accepted (Command, CommandOutcome) into one plain-language sentence for the viewer's
// Current Action panel, resolving unit/ship/squadron ids to names and hexes as "(col,row)". The
// match is exhaustive (no wildcard) so the compiler guarantees every Command variant has a phrasing.
fn hx_str(h: HexCoord) -> String {
    format!("({},{})", h.col, h.row)
}
fn un_bare(g: &GameState, id: UnitId) -> String {
    match g.units.get(&id) {
        Some(u) => u.name.clone(),
        None => format!("unit #{}", id.0),
    }
}
fn un_name(g: &GameState, id: UnitId) -> String {
    match g.effective_hex(id) {
        Some(h) => format!("{} {}", un_bare(g, id), hx_str(h)),
        None => un_bare(g, id),
    }
}
fn unit_list(g: &GameState, ids: &[UnitId]) -> String {
    if ids.is_empty() {
        return "no units".into();
    }
    ids.iter().map(|&i| un_bare(g, i)).collect::<Vec<_>>().join(", ")
}
fn sq_name(g: &GameState, id: SquadronId) -> String {
    g.sgsus.get(&id).map(|s| s.designation.clone()).unwrap_or_else(|| format!("squadron #{}", id.0))
}
/// "a"/"an" for a following word (rough — good enough for the nationality/class words we use).
fn article(word: &str) -> &'static str {
    match word.chars().next() {
        Some(c) if "AEIOUaeiou".contains(c) => "an",
        _ => "a",
    }
}
fn ship_name(g: &GameState, idx: usize) -> String {
    g.fleet.get(idx).map(|s| s.name.clone()).unwrap_or_else(|| format!("ship #{idx}"))
}
fn last_hex(path: &[HexCoord]) -> Option<HexCoord> {
    path.last().copied()
}

/// A friendly description of a command *before* it is applied (no outcome yet) — used to label
/// legal actions offered to external players.
pub fn describe_action(g: &GameState, cmd: &Command) -> String {
    describe_command(g, cmd, &CommandOutcome::Moved)
}

/// The friendly one-line description of an applied command and its outcome (the Action-Log text).
pub fn describe_command(g: &GameState, cmd: &Command, outcome: &CommandOutcome) -> String {
    use cna_engine::state::naval::ShipLocation;
    let mv = |m: &str| if m.contains("Motor") { "motorized" } else { "on foot" };
    match cmd {
        // ---- Movement & Rommel ----
        Command::MoveStep { unit, to, mode } => format!("Moved {} to {} ({}).", un_bare(g, *unit), hx_str(*to), mv(&format!("{mode:?}"))),
        Command::FormationMove { formation, to, mode } => format!("Advanced formation {} to {} ({}).", un_bare(g, *formation), hx_str(*to), mv(&format!("{mode:?}"))),
        Command::MoveRommel { to } => format!("Rommel advanced to {}.", hx_str(*to)),
        Command::RetreatRommel { to } => format!("Rommel fell back to {}.", hx_str(*to)),
        Command::DesignateRommelUnit { unit } => format!("Placed {} under Rommel's command (+5 CPA this stage).", un_name(g, *unit)),
        Command::RailMove { unit, to } => format!("Moved {} by rail to {}.", un_bare(g, *unit), hx_str(*to)),
        Command::MoveOffMap { unit, direction } => {
            let dir = if format!("{direction:?}").contains("Toward") { "toward the map" } else { "further off-map" };
            let base = format!("Marched {} {} along the off-map track", un_bare(g, *unit), dir);
            match outcome {
                CommandOutcome::OffMapMoved(m) => {
                    let s = format!("{m:?}");
                    if let Some(box_name) = s.split('"').nth(1) { format!("{base} — arrived in the {box_name} box.") } else { format!("{base}.") }
                }
                _ => format!("{base}."),
            }
        }
        // ---- Combat ----
        Command::CloseAssault { attackers, target } => format!("Close assault: {} attacked {}.", unit_list(g, attackers), hx_str(*target)),
        Command::AntiArmor { attackers, target_hex, .. } => format!("Anti-armor fire: {} engaged the tanks in {}.", unit_list(g, attackers), hx_str(*target_hex)),
        Command::Barrage { attackers, target, target_class, .. } => format!("Barrage: {} shelled the {:?} in {}.", unit_list(g, attackers), target_class, hx_str(*target)),
        Command::BarrageFacility { barragers, target, facility } => format!("Barrage: {} shelled the {:?} at {}.", unit_list(g, barragers), facility, hx_str(*target)),
        Command::Retreat { unit, path } => match last_hex(path) {
            Some(h) => format!("{} retreated to {}.", un_bare(g, *unit), hx_str(h)),
            None => format!("{} stood its ground under a retreat order.", un_name(g, *unit)),
        },
        Command::React { unit, path } => match last_hex(path) {
            Some(h) => format!("{} reacted, pulling back to {}.", un_bare(g, *unit), hx_str(h)),
            None => format!("{} held in place.", un_name(g, *unit)),
        },
        Command::RetreatBeforeAssault { unit, path } => match last_hex(path) {
            Some(h) => format!("{} withdrew before the assault to {}.", un_bare(g, *unit), hx_str(h)),
            None => format!("{} chose not to withdraw.", un_name(g, *unit)),
        },
        Command::DeclineDefense { hex } => format!("Declined to defend {} — the garrison falls back three hexes (§15.29).", hx_str(*hex)),
        Command::Patrol { patrol_points, target, .. } => {
            let pts: u32 = patrol_points.iter().map(|(_, p)| *p).sum();
            format!("Patrolled {} with {pts} reconnaissance point(s).", hx_str(*target))
        }
        Command::SetGunPosition { unit, forward } => format!("Set {}'s guns to the {} position.", un_name(g, *unit), if *forward { "forward" } else { "back" }),
        // ---- Reorganization ----
        Command::AttachUnit { unit, parent } => format!("Attached {} to formation {}.", un_bare(g, *unit), un_bare(g, *parent)),
        Command::DetachUnit { unit } => format!("Detached {} to operate independently.", un_name(g, *unit)),
        Command::ReassignUnit { unit, parent } => format!("Reassigned {} to formation {}.", un_bare(g, *unit), un_bare(g, *parent)),
        Command::TrainUnit { unit } => format!("Set {} to train up toward its target morale.", un_name(g, *unit)),
        Command::DesignateReserve { unit, status } => format!("Set {} to {:?} reserve status.", un_name(g, *unit), status),
        Command::WithdrawUnit { unit } => format!("Withdrew {} from play.", un_name(g, *unit)),
        Command::ReturnUnit { unit, hex } => format!("Returned {} to play at {}.", un_bare(g, *unit), hx_str(*hex)),
        Command::UpgradeRecce { unit } => format!("Upgraded {} with a light tank.", un_name(g, *unit)),
        Command::AugmentAntiTank { hq, gun_system, count } => format!("Augmented HQ {} with {count} TOE of {gun_system} anti-tank guns.", un_bare(g, *hq)),
        Command::AugmentBattalionAntiTank { battalion, gun_system, count } => format!("Gave {} {count} TOE of {gun_system} anti-tank guns.", un_bare(g, *battalion)),
        Command::SatisfyWithdrawalChoice { units, .. } => format!("Withdrew {} to satisfy a mandatory withdrawal.", unit_list(g, units)),
        // ---- Supply / logistics on units ----
        Command::RefuelUnit { unit } => match outcome {
            CommandOutcome::UnitRefueled(n) => format!("Refuelled {} — fuel now {n}.", un_name(g, *unit)),
            _ => format!("Refuelled {}.", un_name(g, *unit)),
        },
        Command::RearmUnit { unit } => match outcome {
            CommandOutcome::UnitRearmed(n) => format!("Rearmed {} — ammo now {n}.", un_name(g, *unit)),
            _ => format!("Rearmed {}.", un_name(g, *unit)),
        },
        Command::SiphonFuel { donor, receiver, amount } => format!("Siphoned {amount} fuel from {} to {}.", un_bare(g, *donor), un_bare(g, *receiver)),
        Command::LoadTrucks { unit, commodity, amount } => format!("Loaded {amount} {commodity:?} onto {}'s trucks.", un_name(g, *unit)),
        Command::TrucksDrawWater { unit } => match outcome {
            CommandOutcome::WaterDrawn(n) => format!("{}'s trucks drew {n} Water at the well.", un_name(g, *unit)),
            _ => format!("{}'s trucks drew water at the well.", un_name(g, *unit)),
        },
        Command::ConvoyDrawWater { .. } => match outcome {
            CommandOutcome::WaterDrawn(n) => format!("A truck convoy drew {n} Water at the well."),
            _ => "A truck convoy drew water at the well.".into(),
        },
        Command::UnloadTrucks { unit, commodity, amount } => format!("Unloaded {amount} {commodity:?} from {}'s trucks into the hex.", un_name(g, *unit)),
        Command::AssignTrucks { unit, light, medium, heavy } => format!("Assigned trucks ({light}L/{medium}M/{heavy}H) to {}.", un_name(g, *unit)),
        Command::ReleaseTrucks { unit, light, medium, heavy } => format!("Released trucks ({light}L/{medium}M/{heavy}H) from {} to the pool.", un_name(g, *unit)),
        Command::Repair { unit, kind } => match outcome {
            CommandOutcome::Repaired(n) => format!("Repaired {}'s {:?} — {n} point(s) recovered.", un_name(g, *unit), kind),
            _ => format!("Repaired {}'s {:?}.", un_name(g, *unit), kind),
        },
        // ---- Supply dumps / convoys / replacements ----
        Command::ConstructSupplyDump { unit, hex, dummy } => format!("{} built a {} at {}.", un_bare(g, *unit), if *dummy { "dummy (decoy) dump" } else { "supply dump" }, hx_str(*hex)),
        Command::FormTruckConvoy { from_unit, light, medium, heavy } => format!("Formed a truck convoy ({light}L/{medium}M/{heavy}H) from {}.", un_name(g, *from_unit)),
        Command::MoveTruckConvoy { to, .. } => format!("Moved a free truck convoy to {}.", hx_str(*to)),
        Command::LoadConvoy { commodity, amount, .. } => format!("Loaded {amount} {commodity:?} from a dump onto a truck convoy.", ),
        Command::UnloadConvoy { commodity, amount, .. } => format!("Off-loaded {amount} {commodity:?} from a truck convoy into a dump.", ),
        Command::RefuelConvoy { .. } => match outcome {
            CommandOutcome::ConvoyRefueled(n) => format!("Refuelled a truck convoy's tanks (+{n} Fuel)."),
            _ => "Refuelled a truck convoy's tanks.".into(),
        },
        Command::RepairConvoyTrucks { .. } => match outcome {
            CommandOutcome::Repaired(n) => format!("Repaired a convoy's broken-down trucks — {n} recovered."),
            _ => "Repaired a convoy's broken-down trucks.".into(),
        },
        Command::MoveReplacement { to, .. } => format!("Moved a replacement pool toward the front, to {}.", hx_str(*to)),
        Command::LoadReplacement { .. } => "Loaded a replacement pool onto a truck convoy.".into(),
        Command::UnloadReplacement { .. } => "Unloaded a replacement pool from its convoy.".into(),
        Command::AbsorbReplacement { unit, .. } => match outcome {
            CommandOutcome::ReplacementAbsorbed(n) => format!("{} absorbed replacements (+{n} TOE).", un_name(g, *unit)),
            _ => format!("{} absorbed a replacement pool.", un_name(g, *unit)),
        },
        Command::RefitDeacons { unit } => match outcome {
            CommandOutcome::DeaconsRefitted(n) => format!("{} re-equipped {n} anti-tank guns with Deacons (SP 6-pounders) at Cairo.", un_name(g, *unit)),
            _ => format!("{} refit with Deacons at Cairo.", un_name(g, *unit)),
        },
        // ---- Construction ----
        Command::LayMinefield { builder, hex, dummy } => format!("{} began laying a {}minefield at {}.", un_bare(g, *builder), if *dummy { "dummy " } else { "" }, hx_str(*hex)),
        Command::ClearMinefield { builder, hex } => format!("{} began clearing the minefield at {}.", un_bare(g, *builder), hx_str(*hex)),
        Command::BuildPipeline { builder, hex } => format!("{} began building a water pipeline into {}.", un_bare(g, *builder), hx_str(*hex)),
        Command::BuildFortification { hex, level, .. } => format!("Began building a level-{level} fortification at {}.", hx_str(*hex)),
        Command::BuildRailroad { a, b, .. } => format!("Began building railroad between {} and {}.", hx_str(*a), hx_str(*b)),
        Command::DemolishRailroad { a, b, .. } => format!("Began demolishing railroad between {} and {}.", hx_str(*a), hx_str(*b)),
        Command::BuildRoad { builder, a, b } => format!("{} began building a road between {} and {}.", un_bare(g, *builder), hx_str(*a), hx_str(*b)),
        Command::RailMoveSupply { from, to, cargo, points } => format!("Moved {points} {cargo:?} supply by rail from {} to {}.", hx_str(*from), hx_str(*to)),
        Command::ImportRollingStock { hex } => format!("Activated imported Axis rolling stock at {} (+300 tons haulage).", hx_str(*hex)),
        // ---- Wells ----
        Command::PoisonWell { hex } => format!("Poisoned the well at {} — {}.", hx_str(*hex), if matches!(outcome, CommandOutcome::WellPoisoned(true)) { "the well is now poisoned" } else { "the attempt failed" }),
        Command::SweetenWell { hex } => format!("Sweetened the well at {} — {}.", hx_str(*hex), if matches!(outcome, CommandOutcome::WellSweetened(true)) { "it is drinkable again" } else { "the attempt failed" }),
        // ---- Prisoners / captured / surrender ----
        Command::FormGuard { group_hex } => format!("Formed a prisoner guard at {}.", hx_str(*group_hex)),
        Command::ReturnGuard { group_hex } => format!("Returned a prisoner guard to its unit at {}.", hx_str(*group_hex)),
        Command::MovePrisoners { from, to } => {
            let base = format!("Marched prisoners from {} to {}", hx_str(*from), hx_str(*to));
            match outcome {
                CommandOutcome::PrisonersMoved { escaped } if *escaped > 0 => format!("{base} — {escaped} point(s) escaped for want of escort."),
                _ => format!("{base}."),
            }
        }
        Command::RepairCapturedEquipment { hex, system } => format!("Repaired captured {system} at {} — now usable.", hx_str(*hex)),
        Command::VoluntarySurrender { hex, captor, .. } => {
            let base = format!("The stack at {} surrendered to {}", hx_str(*hex), un_bare(g, *captor));
            match outcome {
                CommandOutcome::VoluntarilySurrendered { toe_lost, .. } => format!("{base} — {toe_lost} TOE taken prisoner."),
                _ => format!("{base}."),
            }
        }
        Command::PlaceDummyTank { hex, designation } => format!("Placed a dummy tank formation \"{designation}\" at {}.", hx_str(*hex)),
        // ---- Raiders / commandos ----
        Command::FormRaider { kind, from_unit } => format!("Formed a {:?} desert raider from {}.", kind, un_bare(g, *from_unit)),
        Command::MoveRaider { to, .. } => format!("A desert raider slipped to {}.", hx_str(*to)),
        Command::SpotRaider { .. } => format!("Attempted to spot a desert raider — {}.", if matches!(outcome, CommandOutcome::RaiderSpotChecked { eliminated: true }) { "spotted and destroyed" } else { "it evaded" }),
        Command::ResupplyRaider { .. } => "A desert raider resupplied on fuel, ammo and water.".into(),
        Command::RaidRommel { .. } => "An LRDG patrol raided Rommel.".into(),
        Command::RaidPipeline { .. } => "A raider struck the water pipeline.".into(),
        Command::RaidAirfield { .. } => "A raider struck an enemy airfield.".into(),
        Command::RaidPlanes { .. } => "A raider struck grounded enemy planes.".into(),
        Command::RaidSupplyDump { .. } => "A raider struck an enemy supply dump.".into(),
        Command::RaidTruckConvoy { .. } => "A raider struck an enemy truck convoy.".into(),
        Command::InterceptTruckConvoy { .. } => "A raider intercepted a passing enemy convoy.".into(),
        Command::LandLayforce { unit, to, .. } => format!("Landed {} (Layforce commando) at {} by sea.", un_bare(g, *unit), hx_str(*to)),
        Command::EmbarkLayforce { unit } => format!("Re-embarked {} (Layforce) for Alexandria.", un_bare(g, *unit)),
        Command::LandSas { unit, to, .. } => format!("Landed {} (SAS Brigade) at {} by sea.", un_bare(g, *unit), hx_str(*to)),
        Command::RaidAirfieldSas { unit } => match outcome {
            CommandOutcome::SasRaided { planes_pct } => format!("{} (SAS) raided the airfield — {planes_pct}% of grounded planes destroyed.", un_bare(g, *unit)),
            _ => format!("{} (SAS) raided the airfield.", un_bare(g, *unit)),
        },
        // ---- Naval ----
        Command::AssignShip { ship_index, location } => {
            let where_to = match location { ShipLocation::AtSea(h) => format!("to sea at {}", hx_str(*h)), other => format!("to {other:?}") };
            format!("Positioned {} {}.", ship_name(g, *ship_index), where_to)
        }
        Command::RepairShip { ship_index } => match outcome {
            CommandOutcome::ShipRepaired(n) => format!("Repaired {} — gun rating now {n}.", ship_name(g, *ship_index)),
            _ => format!("Repaired {}.", ship_name(g, *ship_index)),
        },
        Command::NavalTransferTroops { unit, to_port } => format!("Shipped {} by sea to {to_port}.", un_bare(g, *unit)),
        Command::OffShoreBombard { ship_index, target, target_class } => format!("{} bombarded the {:?} in {}.", ship_name(g, *ship_index), target_class, hx_str(*target)),
        Command::ArtilleryBombardShip { attackers, ship_index } => format!("{} fired on {}.", unit_list(g, attackers), ship_name(g, *ship_index)),
        Command::SanGiorgioAttackShip { ship_index } => format!("The San Giorgio fired on {}.", ship_name(g, *ship_index)),
        Command::BlockHarbor { port, .. } => format!("Blocked the harbour at {port} by scuttling."),
        Command::UnblockHarbor { port, .. } => format!("Cleared harbour blocking at {port}."),
        Command::LayHarborMine { port } => format!("Air-laid a mine in {port}'s harbour."),
        Command::RemoveHarborMine { port } => format!("Attempted to clear a harbour mine at {port} — {}.", if matches!(outcome, CommandOutcome::HarborMineRemoved(true)) { "cleared" } else { "still mined" }),
        Command::PlanChariotRaid => "Planned the Italian Chariot raid on Alexandria.".into(),
        Command::ExecuteChariotRaid => match outcome {
            CommandOutcome::ChariotRaidExecuted { damage } => format!("Executed the Chariot raid — {damage} gun-damage inflicted at Alexandria."),
            _ => "Executed the Chariot raid.".into(),
        },
        // ---- Axis convoys / production ----
        Command::PlanConvoy { destination, arrival_gt, fuel_tons, ammo_tons, stores_tons, replacement_tons, .. } => {
            let tons = fuel_tons + ammo_tons + stores_tons + replacement_tons;
            format!("Scheduled an Axis convoy to {destination}, ~{tons} tons, arriving GT {arrival_gt}.")
        }
        Command::BombConvoy { .. } => match outcome {
            CommandOutcome::ConvoyBombed { percent, attackers_lost } => {
                let flak = if *attackers_lost > 0 { format!(" ({attackers_lost} bomber(s) lost to convoy flak, §56.16)") } else { String::new() };
                format!("Bombed a planned Axis convoy — {percent}% of its cargo lost{flak}.")
            }
            _ => "Bombed a planned Axis convoy.".into(),
        },
        Command::DeliverConvoy { .. } => "Delivered an arrived convoy's cargo into supply.".into(),
        Command::ReconConvoyLane { lane, .. } => format!("Flew naval reconnaissance over shipping lane {lane}."),
        Command::PlanProduction { nationality, item, count } => format!("Planned production of {count} {item} for the {nationality:?}."),
        Command::ProduceCommonwealthTrucks {} => "Rolled Commonwealth truck production.".into(),
        Command::ProduceCommonwealthInfantry {} => match outcome {
            CommandOutcome::InfantryProduced { points, .. } => format!("Rolled Commonwealth infantry production — {points} replacement point(s)."),
            _ => "Rolled Commonwealth infantry production.".into(),
        },
        Command::ProduceAxisTrucks { kind, count } => format!("Planned {count} {kind:?} Axis truck point(s)."),
        // ---- Air: squadrons ----
        Command::DeploySgsu { nationality, class, at } => {
            let nat = format!("{nationality:?}");
            format!("Deployed {} {nat} {class:?} squadron at {}.", article(&nat), hx_str(*at))
        }
        Command::WithdrawSquadron { squadron } => format!("Withdrew squadron {} to meet a withdrawal requirement.", sq_name(g, *squadron)),
        Command::BaseSquadronOffMap { squadron, facility } => format!("Based squadron {} at the off-map field {facility}.", sq_name(g, *squadron)),
        Command::MoveSgsu { squadron, to } => format!("Moved squadron {}'s ground echelon (SGSU) to {}.", sq_name(g, *squadron), hx_str(*to)),
        Command::AssignSquadronPlanes { squadron, aircraft_type, class, count } => format!("Assigned {count} × {aircraft_type} ({class:?}) to squadron {}.", sq_name(g, *squadron)),
        Command::TransferPlanes { from, to, aircraft_type, count } => format!("Transferred {count} × {aircraft_type} from squadron {} to under-strength squadron {} (§35.22).", sq_name(g, *from), sq_name(g, *to)),
        Command::RefitSquadron { squadron, .. } => match outcome {
            CommandOutcome::SquadronRefitted(n) => format!("Refit squadron {} — {n} planes returned to service.", sq_name(g, *squadron)),
            _ => format!("Refit squadron {}.", sq_name(g, *squadron)),
        },
        Command::ReadySquadron { squadron, count } => format!("Readied {count} planes of squadron {}.", sq_name(g, *squadron)),
        Command::ArmSquadron { squadron, count } => format!("Armed {count} planes of squadron {} for a strike.", sq_name(g, *squadron)),
        Command::FlySquadronMission { squadron, count } => format!("Flew {count} planes of squadron {} on a mission.", sq_name(g, *squadron)),
        Command::AssignPilot { rating, squadron } => format!("Assigned a rating-{rating} pilot to squadron {}.", sq_name(g, *squadron)),
        Command::TransferSquadron { squadron, to } => format!("Began transferring squadron {} to {to:?}.", sq_name(g, *squadron)),
        Command::DeclareInitiativeOrder { initiative_moves_first } => format!("Declared to move {} this stage.", if *initiative_moves_first { "first" } else { "second" }),
        Command::DetermineMaltaAvailability { level } => format!("Determined Malta raid availability at level {level}."),
        // ---- Air: missions ----
        Command::DesignateCap { squadron, center, cap_type, planes } => format!("Flew {planes} fighters of squadron {} on a {cap_type:?} Combat Air Patrol over {}.", sq_name(g, *squadron), hx_str(*center)),
        Command::DesignateScramble { squadron, planes } => format!("Put {planes} fighters of squadron {} on standby to scramble (§40.31).", sq_name(g, *squadron)),
        Command::ReconHex { squadron, target, planes } => format!("Flew {planes} planes of squadron {} on reconnaissance over {}.", sq_name(g, *squadron), hx_str(*target)),
        Command::Strafe { squadron, target, strafe_target, planes } => format!("Strafed the {strafe_target:?} in {} with {planes} planes of squadron {}.", hx_str(*target), sq_name(g, *squadron)),
        Command::FlyBombingMission { squadron, planes, .. } => format!("Flew a bombing mission: {planes} bombers of squadron {}.", sq_name(g, *squadron)),
        Command::FlyBombingMissionAt { squadron, target, planes, .. } => format!("Flew {planes} bombers of squadron {} to bomb {}.", sq_name(g, *squadron), hx_str(*target)),
        Command::FlyBombingMissionAtBox { squadron, box_name, mission, planes, night, .. } => {
            use cna_engine::reference::aircraft::BombingMission as BM;
            let what = match mission { BM::Port => "the port", BM::AirFacility => "grounded aircraft", _ => "targets" };
            format!("Flew {planes} bombers of squadron {} to bomb {what} in the {box_name} box{}.", sq_name(g, *squadron), if *night { " at night" } else { "" })
        }
        Command::BombAirFacility { hex, levels, .. } => format!("Bombed the air facility at {}, reducing it {levels} level(s).", hx_str(*hex)),
        Command::BombShip { squadron, planes, ship_index, .. } => format!("Bombed {} with {planes} planes of squadron {}.", ship_name(g, *ship_index), sq_name(g, *squadron)),
        Command::AirdropSupplies { target, commodity, amount, .. } => format!("Airdropped {amount} {commodity:?} into {}.", hx_str(*target)),
        Command::TransportUnit { unit, target, .. } => format!("Air-transported {} to {}.", un_bare(g, *unit), hx_str(*target)),
        Command::AirdropUnit { unit, target, .. } => format!("Airdropped {} into {}.", un_bare(g, *unit), hx_str(*target)),
        // ---- Malta ----
        Command::BombMaltaBase { base_index, planes, .. } => match outcome {
            CommandOutcome::MaltaBaseBombed { levels_lost, .. } => format!("Axis raided Malta base #{} with {planes} planes — {levels_lost} capacity level(s) lost.", base_index + 1),
            _ => format!("Axis raided Malta base #{} with {planes} planes.", base_index + 1),
        },
        Command::ConstructMaltaFacility { base_index } => match outcome {
            CommandOutcome::MaltaFacilityConstructed(n) => format!("Built up Malta base #{} by {n} level(s).", base_index + 1),
            _ => format!("Built up Malta base #{}.", base_index + 1),
        },
        Command::DivertMaltaAA { base_index, heavy } => format!("Diverted a {} AA point to Malta base #{}.", if *heavy { "heavy" } else { "light" }, base_index + 1),
        Command::MoveMaltaFlak { from_base, to_base, light, heavy } => format!("Redistributed Malta flak ({light} light / {heavy} heavy) from base #{} to base #{}.", from_base + 1, to_base + 1),
        Command::BaseMaltaPlanes { base_index, aircraft_type, count } => format!("Based {count} × {aircraft_type} on Malta base #{}.", base_index + 1),
        Command::MoveMaltaPlanes { from_base, to_base, aircraft_type, count } => format!("Moved {count} × {aircraft_type} from Malta base #{} to #{}.", from_base + 1, to_base + 1),
        Command::InterdictConvoy { aircraft_type, planes, torpedo, .. } => {
            let base = format!("Flew {planes} × {aircraft_type} from Malta to {} an Axis convoy", if *torpedo { "torpedo" } else { "bomb" });
            match outcome {
                CommandOutcome::ConvoyInterdicted { percent, attackers_lost } => format!("{base} — {percent}% of its tonnage sunk, {attackers_lost} plane(s) lost to flak."),
                _ => format!("{base}."),
            }
        }
        // ---- Towing ----
        Command::TowBrokenVehicle { to, .. } => format!("Towed a broken-down vehicle to {}.", hx_str(*to)),
        Command::TowDestroyedTank { to, .. } => format!("Towed a knocked-out tank to {} for recovery.", hx_str(*to)),
    }
}

/// Where the viewer's "jump to activity" control centers. `kind` is `"hex"` (an on-map hex, `hex`
/// set), `"box"` (an off-map panel named by `name`: `"axis_land"` / `"malta"` / `"axis_convoys"`),
/// or `"cwair"` (a Commonwealth off-map air-field box named by `name`).
#[derive(Serialize, TS, Clone, PartialEq)]
#[ts(export)]
pub struct ActivityTarget {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hex: Option<[i16; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
fn t_hex(h: HexCoord) -> ActivityTarget {
    ActivityTarget { kind: "hex".into(), hex: Some([h.col, h.row]), name: None }
}
fn t_box(id: &str) -> ActivityTarget {
    ActivityTarget { kind: "box".into(), hex: None, name: Some(id.into()) }
}
fn t_air(name: &str) -> ActivityTarget {
    ActivityTarget { kind: "cwair".into(), hex: None, name: Some(name.into()) }
}

/// The jump-to-activity target for an action. Layers off-map/box/port targets over the on-map hex
/// classifier `activity_hex`. `None` → the viewer hides the jump control (production/strategic).
fn activity_target(g: &GameState, cmd: &Command) -> Option<ActivityTarget> {
    use cna_engine::state::naval::ShipLocation;
    match cmd {
        Command::MoveOffMap { .. } => return Some(t_box("axis_land")),
        Command::FlyBombingMissionAtBox { box_name, .. } => return Some(t_box(box_name)),
        Command::BombMaltaBase { .. }
        | Command::ConstructMaltaFacility { .. }
        | Command::DivertMaltaAA { .. }
        | Command::MoveMaltaFlak { .. }
        | Command::BaseMaltaPlanes { .. }
        | Command::MoveMaltaPlanes { .. }
        | Command::InterdictConvoy { .. }
        | Command::DetermineMaltaAvailability { .. } => return Some(t_box("malta")),
        Command::PlanConvoy { .. } | Command::BombConvoy { .. } | Command::DeliverConvoy { .. } | Command::ReconConvoyLane { .. } => {
            return Some(t_box("axis_convoys"))
        }
        Command::BaseSquadronOffMap { facility, .. } => return Some(t_air(facility)),
        Command::NavalTransferTroops { to_port, .. } => return g.port_hex(to_port).map(t_hex),
        Command::BlockHarbor { port, .. } | Command::UnblockHarbor { port, .. } | Command::LayHarborMine { port } | Command::RemoveHarborMine { port } => {
            return g.port_hex(port).map(t_hex)
        }
        Command::RepairShip { ship_index } => {
            return g.fleet.get(*ship_index).and_then(|s| match &s.location {
                ShipLocation::AtSea(h) => Some(*h),
                ShipLocation::Alexandria => g.port_hex("Alexandria"),
                ShipLocation::Valetta => g.port_hex("Valetta"),
            }).map(t_hex)
        }
        Command::UnloadReplacement { pool } => {
            return g.replacement_points.get(*pool).and_then(|rp| rp.location.as_ref().and_then(|l| l.hex())).map(t_hex)
        }
        _ => {}
    }
    activity_hex(g, cmd).map(t_hex)
}

// The on-map hex a "jump to activity" control should center for an action, or `None` when the
// action has no clear on-map hex (off-map movement, Malta, shipping-lane/production/strategic
// actions, in-port ship/harbour actions). `None` → the viewer hides the jump control.
fn activity_hex(g: &GameState, cmd: &Command) -> Option<HexCoord> {
    use cna_engine::state::naval::ShipLocation;
    let uh = |id: UnitId| g.effective_hex(id);
    let rh = |i: usize| g.raiders.get(i).and_then(|r| r.hex);
    let ch = |i: usize| g.truck_convoys.get(i).map(|c| c.hex);
    let shh = |i: usize| g.fleet.get(i).and_then(|s| match s.location {
        ShipLocation::AtSea(h) => Some(h),
        _ => None,
    });
    let sqh = |id: SquadronId| g.sgsus.get(&id).and_then(|s| s.location);
    match cmd {
        Command::MoveSgsu { to, .. } => Some(*to),
        // Destination-hex actions.
        Command::MoveStep { to, .. }
        | Command::FormationMove { to, .. }
        | Command::MoveRommel { to }
        | Command::RetreatRommel { to }
        | Command::RailMove { to, .. }
        | Command::MoveTruckConvoy { to, .. }
        | Command::MoveReplacement { to, .. }
        | Command::RailMoveSupply { to, .. }
        | Command::MovePrisoners { to, .. }
        | Command::MoveRaider { to, .. }
        | Command::LandLayforce { to, .. }
        | Command::LandSas { to, .. }
        | Command::TowBrokenVehicle { to, .. }
        | Command::TowDestroyedTank { to, .. } => Some(*to),
        Command::ReturnUnit { hex, .. } => Some(*hex),
        // Target-hex actions (combat, air missions, patrol).
        Command::CloseAssault { target, .. }
        | Command::Barrage { target, .. }
        | Command::BarrageFacility { target, .. }
        | Command::OffShoreBombard { target, .. }
        | Command::Patrol { target, .. }
        | Command::Strafe { target, .. }
        | Command::ReconHex { target, .. }
        | Command::AirdropSupplies { target, .. }
        | Command::TransportUnit { target, .. }
        | Command::AirdropUnit { target, .. }
        | Command::FlyBombingMissionAt { target, .. } => Some(*target),
        Command::AntiArmor { target_hex, .. } => Some(*target_hex),
        Command::DeclineDefense { hex } => Some(*hex),
        Command::DesignateCap { center, .. } => Some(*center),
        Command::DesignateScramble { squadron, .. } => g.sgsus.get(squadron).and_then(|s| s.location),
        // Explicit-hex actions (construction, wells, dumps, captures).
        Command::ConstructSupplyDump { hex, .. }
        | Command::RepairCapturedEquipment { hex, .. }
        | Command::PlaceDummyTank { hex, .. }
        | Command::LayMinefield { hex, .. }
        | Command::ClearMinefield { hex, .. }
        | Command::BuildPipeline { hex, .. }
        | Command::BuildFortification { hex, .. }
        | Command::ImportRollingStock { hex }
        | Command::PoisonWell { hex }
        | Command::SweetenWell { hex }
        | Command::VoluntarySurrender { hex, .. }
        | Command::BombAirFacility { hex, .. } => Some(*hex),
        Command::FormGuard { group_hex } | Command::ReturnGuard { group_hex } => Some(*group_hex),
        Command::DeploySgsu { at, .. } => Some(*at),
        // Hexside-project actions — anchor on the first endpoint.
        Command::BuildRailroad { a, .. } | Command::DemolishRailroad { a, .. } | Command::BuildRoad { a, .. } => Some(*a),
        // The acting unit's own hex.
        Command::DesignateRommelUnit { unit }
        | Command::TrainUnit { unit }
        | Command::AttachUnit { unit, .. }
        | Command::DetachUnit { unit }
        | Command::ReassignUnit { unit, .. }
        | Command::SetGunPosition { unit, .. }
        | Command::Repair { unit, .. }
        | Command::RefuelUnit { unit }
        | Command::RearmUnit { unit }
        | Command::LoadTrucks { unit, .. }
        | Command::TrucksDrawWater { unit }
        | Command::UnloadTrucks { unit, .. }
        | Command::AssignTrucks { unit, .. }
        | Command::ReleaseTrucks { unit, .. }
        | Command::UpgradeRecce { unit }
        | Command::DesignateReserve { unit, .. }
        | Command::WithdrawUnit { unit }
        | Command::AbsorbReplacement { unit, .. }
        | Command::RefitDeacons { unit }
        | Command::EmbarkLayforce { unit }
        | Command::RaidAirfieldSas { unit } => uh(*unit),
        Command::FormTruckConvoy { from_unit, .. } | Command::FormRaider { from_unit, .. } => uh(*from_unit),
        Command::AugmentAntiTank { hq, .. } => uh(*hq),
        Command::AugmentBattalionAntiTank { battalion, .. } => uh(*battalion),
        Command::SiphonFuel { receiver, .. } => uh(*receiver),
        Command::SatisfyWithdrawalChoice { units, .. } => units.first().and_then(|u| uh(*u)),
        Command::ArtilleryBombardShip { attackers, .. } => attackers.first().and_then(|u| uh(*u)),
        // Retreat/react — the hex actually reached (or the unit's hex if it stood).
        Command::Retreat { unit, path } | Command::React { unit, path } | Command::RetreatBeforeAssault { unit, path } => {
            path.last().copied().or_else(|| uh(*unit))
        }
        // Index-referenced raiders / convoys / ships.
        Command::SpotRaider { raider }
        | Command::ResupplyRaider { raider }
        | Command::RaidRommel { raider }
        | Command::RaidPipeline { raider }
        | Command::RaidAirfield { raider }
        | Command::RaidPlanes { raider }
        | Command::RaidSupplyDump { raider }
        | Command::RaidTruckConvoy { raider, .. }
        | Command::InterceptTruckConvoy { raider, .. } => rh(*raider),
        Command::LoadReplacement { convoy, .. }
        | Command::LoadConvoy { convoy, .. }
        | Command::UnloadConvoy { convoy, .. }
        | Command::RepairConvoyTrucks { convoy }
        | Command::RefuelConvoy { convoy }
        | Command::ConvoyDrawWater { convoy } => ch(*convoy),
        Command::AssignShip { ship_index, .. }
        | Command::RepairShip { ship_index }
        | Command::BombShip { ship_index, .. }
        | Command::SanGiorgioAttackShip { ship_index } => shh(*ship_index),
        // Squadron-base actions — the squadron's on-map base hex (None if off-map).
        Command::TransferPlanes { to, .. } => sqh(*to),
        Command::ReadySquadron { squadron, .. }
        | Command::ArmSquadron { squadron, .. }
        | Command::RefitSquadron { squadron, .. }
        | Command::AssignSquadronPlanes { squadron, .. }
        | Command::AssignPilot { squadron, .. }
        | Command::FlySquadronMission { squadron, .. }
        | Command::FlyBombingMission { squadron, .. }
        | Command::WithdrawSquadron { squadron } => sqh(*squadron),
        // No clear on-map hex — off-map / Malta / shipping-lane / production / strategic / port.
        Command::MoveOffMap { .. }
        | Command::UnloadReplacement { .. }
        | Command::BaseSquadronOffMap { .. }
        | Command::TransferSquadron { .. }
        | Command::DetermineMaltaAvailability { .. }
        | Command::DeclareInitiativeOrder { .. }
        | Command::NavalTransferTroops { .. }
        | Command::BlockHarbor { .. }
        | Command::UnblockHarbor { .. }
        | Command::LayHarborMine { .. }
        | Command::RemoveHarborMine { .. }
        | Command::PlanChariotRaid
        | Command::ExecuteChariotRaid
        | Command::PlanConvoy { .. }
        | Command::BombConvoy { .. }
        | Command::DeliverConvoy { .. }
        | Command::ReconConvoyLane { .. }
        | Command::PlanProduction { .. }
        | Command::ProduceCommonwealthTrucks { .. }
        | Command::ProduceCommonwealthInfantry { .. }
        | Command::ProduceAxisTrucks { .. }
        | Command::BombMaltaBase { .. }
        | Command::ConstructMaltaFacility { .. }
        | Command::DivertMaltaAA { .. }
        | Command::MoveMaltaFlak { .. }
        | Command::BaseMaltaPlanes { .. }
        | Command::MoveMaltaPlanes { .. }
        | Command::InterdictConvoy { .. }
        | Command::FlyBombingMissionAtBox { .. } => None, // handled by the box early-return above
    }
}

fn unit_view(g: &GameState, u: &cna_engine::state::unit::Unit) -> UnitView {
    use cna_engine::state::unit::RatingKind;
    UnitView {
        id: u.id.0,
        name: u.name.clone(),
        abbr: u.abbr.clone(),
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
        attack: u.actual_strength(&g.reference, RatingKind::CloseAssaultOff),
        defense: u.actual_strength(&g.reference, RatingKind::CloseAssaultDef),
        anti_armor: u.actual_strength(&g.reference, RatingKind::AntiArmor),
        barrage: u.actual_strength(&g.reference, RatingKind::Barrage),
        cpa: g.effective_cpa(u.id),
        cp_expended: u.cp_expended.0 as f32 / 10.0,
        motorized: g.unit_is_motorized(u.id),
        supplied: g.unit_supply_adequate(u.id),
        formation: u.assigned_parent.and_then(|p| g.units.get(&p)).map(|p| p.name.clone()),
        formation_colocated: u.assigned_parent.and_then(|p| {
            let ph = g.effective_hex(p)?;
            Some(g.effective_hex(u.id) == Some(ph))
        }),
        attached_to: u.attached_parent.and_then(|p| g.units.get(&p)).map(|p| p.name.clone()),
        attached_units: {
            let mut v: Vec<String> = g.units.values().filter(|o| o.attached_parent == Some(u.id)).map(|o| o.name.clone()).collect();
            v.sort();
            v
        },
    }
}

fn unit_views(g: &GameState) -> Vec<UnitView> {
    let mut v: Vec<UnitView> = g
        .units
        .values()
        // §19.62: a combat-eliminated (0-TOE) non-HQ unit is removed from play and can never be
        // rebuilt — omit it so the viewer drops it. A 0-TOE HQ stays as its formation's cadre.
        .filter(|u| u.total_toe() > 0 || u.is_hq || u.is_tds)
        .map(|u| unit_view(g, u))
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
                unlimited: d.unlimited,
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
            tank_fuel: c.tank_fuel,
            tank_capacity: g.convoy_tank_capacity(c),
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

/// §28 prisoner stacks (POWs) and escaped prisoners on the map, for the viewer. Deterministic order.
fn prisoner_views(g: &GameState) -> Vec<PrisonerView> {
    let mut v: Vec<PrisonerView> = Vec::new();
    for p in &g.prisoner_groups {
        v.push(PrisonerView {
            hex: [p.hex.col, p.hex.row],
            kind: "Pow".into(),
            side: format!("{:?}", p.original),
            captor: format!("{:?}", p.captor),
            points: p.points,
            guards: p.guards,
        });
    }
    for e in &g.escapees {
        v.push(PrisonerView {
            hex: [e.hex.col, e.hex.row],
            kind: "Escapee".into(),
            side: format!("{:?}", e.side),
            captor: String::new(),
            points: e.points,
            guards: 0,
        });
    }
    v.sort_by(|a, b| (a.hex[0], a.hex[1], &a.kind).cmp(&(b.hex[0], b.hex[1], &b.kind)));
    v
}

/// §28.3/§14.46 captured equipment and knocked-out tank markers on the map, for the viewer.
fn captured_views(g: &GameState) -> Vec<CapturedView> {
    let mut v: Vec<CapturedView> = g
        .captured_equipment
        .iter()
        .map(|c| CapturedView {
            hex: [c.hex.col, c.hex.row],
            captor: format!("{:?}", c.captor),
            nationality: format!("{:?}", c.nationality),
            system: c.system.clone(),
            count: c.count,
            repaired: c.repaired,
            destroyed_tank: c.destroyed_tank,
        })
        .collect();
    v.sort_by(|a, b| (a.hex[0], a.hex[1], &a.system).cmp(&(b.hex[0], b.hex[1], &b.system)));
    v
}

/// §32 dummy (decoy) tank markers on the map, for the viewer.
fn dummy_tank_views(g: &GameState) -> Vec<DummyTankView> {
    let mut v: Vec<DummyTankView> = g
        .dummy_tanks
        .iter()
        .map(|d| DummyTankView { hex: [d.hex.col, d.hex.row], side: format!("{:?}", d.side), designation: d.designation.clone() })
        .collect();
    v.sort_by(|a, b| (a.hex[0], a.hex[1], &a.designation).cmp(&(b.hex[0], b.hex[1], &b.designation)));
    v
}

/// §21.5 broken-down vehicle markers awaiting tow/repair, for the viewer.
fn broken_vehicle_views(g: &GameState) -> Vec<BrokenVehicleView> {
    let mut v: Vec<BrokenVehicleView> = g
        .broken_vehicles
        .iter()
        .map(|b| {
            let wsum: u32 = b.weapons.iter().map(|w| w.count).sum();
            let trucks = b.trucks.light + b.trucks.medium + b.trucks.heavy;
            let mut parts: Vec<String> = b.weapons.iter().filter(|w| w.count > 0).map(|w| format!("{}× {}", w.count, w.system)).collect();
            if b.toe > 0 { parts.push(format!("{} TOE", b.toe)); }
            if trucks > 0 { parts.push(format!("{trucks} truck pt")); }
            BrokenVehicleView {
                hex: [b.hex.col, b.hex.row],
                side: format!("{:?}", b.side),
                nationality: format!("{:?}", b.nationality),
                points: wsum + b.toe + trucks,
                detail: if parts.is_empty() { "empty".into() } else { parts.join(", ") },
            }
        })
        .collect();
    v.sort_by(|a, b| (a.hex[0], a.hex[1]).cmp(&(b.hex[0], b.hex[1])));
    v
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
    prev_off_map: &mut OffMapView,
    prev_prisoners: &mut Vec<PrisonerView>,
    prev_captured: &mut Vec<CapturedView>,
    prev_dummy: &mut Vec<DummyTankView>,
    prev_broken: &mut Vec<BrokenVehicleView>,
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
    let omv = off_map_views(game);
    let off_map = if omv != *prev_off_map {
        *prev_off_map = omv.clone();
        Some(omv)
    } else {
        None
    };
    let pv = prisoner_views(game);
    let prisoners = if pv != *prev_prisoners {
        *prev_prisoners = pv.clone();
        Some(pv)
    } else {
        None
    };
    let cpv = captured_views(game);
    let captured = if cpv != *prev_captured {
        *prev_captured = cpv.clone();
        Some(cpv)
    } else {
        None
    };
    let dtv = dummy_tank_views(game);
    let dummy_tanks = if dtv != *prev_dummy {
        *prev_dummy = dtv.clone();
        Some(dtv)
    } else {
        None
    };
    let bvv = broken_vehicle_views(game);
    let broken_vehicles = if bvv != *prev_broken {
        *prev_broken = bvv.clone();
        Some(bvv)
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
        summary: String::new(),
        activity: None,
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
        off_map,
        prisoners,
        captured,
        dummy_tanks,
        broken_vehicles,
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
    let mut prev_off_map: OffMapView = OffMapView::default();
    let mut prev_prisoners: Vec<PrisonerView> = Vec::new();
    let mut prev_captured: Vec<CapturedView> = Vec::new();
    let mut prev_dummy: Vec<DummyTankView> = Vec::new();
    let mut prev_broken: Vec<BrokenVehicleView> = Vec::new();
    let mut events = vec![delta_frame(&game, 0, "-", "start".into(), String::new(), Vec::new(), &mut prev, &mut prev_weather, &mut prev_replacements, &mut prev_dumps, &mut prev_convoys, &mut prev_raiders, &mut prev_rommel, &mut prev_overlays, &mut prev_hexsides, &mut prev_air, &mut prev_naval, &mut prev_off_map, &mut prev_prisoners, &mut prev_captured, &mut prev_dummy, &mut prev_broken)];
    let mut step = 0u32;
    let mut guard = 0u64;
    let mut checked_stage = None;
    while game.game_turn <= max_turns {
        // Stop early on a §64.71/§64.72 automatic campaign win (None for non-campaign scenarios) —
        // checked once per Operations Stage (the supply-line trace is costly; see `self_play`).
        let stage = (game.game_turn, game.operations_stage);
        if checked_stage != Some(stage) {
            checked_stage = Some(stage);
            if game.campaign_auto_winner().is_some() {
                break;
            }
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
            // Free-form agents (e.g. an LLM player via the harness) propose commands directly rather
            // than indexing the legal snapshot; they get a larger action budget (plans/macros).
            let free = {
                let a: &dyn Agent = if side == Side::Commonwealth { &*commonwealth } else { &*axis };
                a.free_form()
            };
            let cap = if free { cap.saturating_mul(10).max(200) } else { cap };
            let mut legal = if free { Vec::new() } else { game.legal_commands(side) };
            let mut acted = 0u32;
            let mut progress = true;
            while acted < cap {
                let agent: &mut dyn Agent = if side == Side::Commonwealth { commonwealth } else { axis };
                let pick = if free {
                    agent.choose(&game, side).map(Ok)
                } else {
                    agent.choose_index(&game, side, &legal).filter(|&i| i < legal.len()).map(Err)
                };
                match pick {
                    Some(p) => {
                        let cmd = match p {
                            Ok(c) => c,
                            Err(i) => legal.swap_remove(i),
                        };
                        let action = format!("{cmd:?}");
                        let rules: Vec<String> = cmd.rule_refs().iter().map(|s| s.to_string()).collect();
                        let cmd_desc = cmd.clone();
                        let result = game.apply_as(side, cmd);
                        if free {
                            agent.on_result(&cmd_desc, &result);
                        }
                        if let Ok(outcome) = result {
                            step += 1;
                            let summary = describe_command(&game, &cmd_desc, &outcome);
                            let act = activity_target(&game, &cmd_desc);
                            let mut ev = delta_frame(&game, step, &format!("{side:?}"), action, format!("{outcome:?}"), rules, &mut prev, &mut prev_weather, &mut prev_replacements, &mut prev_dumps, &mut prev_convoys, &mut prev_raiders, &mut prev_rommel, &mut prev_overlays, &mut prev_hexsides, &mut prev_air, &mut prev_naval, &mut prev_off_map, &mut prev_prisoners, &mut prev_captured, &mut prev_dummy, &mut prev_broken);
                            ev.summary = summary;
                            ev.activity = act;
                            events.push(ev);
                            progress = true;
                        }
                        acted += 1;
                    }
                    None => {
                        if free || !progress {
                            break;
                        }
                        legal = game.legal_commands(side);
                        progress = false;
                    }
                }
            }
        }
        game.advance_phase();
        // §4.44a and other system-initiated events surface as their own Action-Log entries.
        for entry in std::mem::take(&mut game.system_log) {
            step += 1;
            let side = entry.unit.and_then(|u| game.units.get(&u)).map(|u| format!("{:?}", u.nationality.side())).unwrap_or_else(|| "-".into());
            let act = entry.unit.and_then(|u| game.effective_hex(u)).map(t_hex);
            let mut ev = delta_frame(&game, step, &side, entry.label.clone(), String::new(), entry.rule_refs.clone(), &mut prev, &mut prev_weather, &mut prev_replacements, &mut prev_dumps, &mut prev_convoys, &mut prev_raiders, &mut prev_rommel, &mut prev_overlays, &mut prev_hexsides, &mut prev_air, &mut prev_naval, &mut prev_off_map, &mut prev_prisoners, &mut prev_captured, &mut prev_dummy, &mut prev_broken);
            ev.summary = entry.summary;
            ev.activity = act;
            events.push(ev);
        }
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
    game: GameState,
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
    record_game_streaming_ext(game, scenario, seed, commonwealth_name, commonwealth, axis_name, axis, max_turns, out, on_turn, &mut |_, _, _, _| {})
        .map(|(steps, _)| steps)
}

/// A command the driver attempted to apply, with the engine's verdict (see
/// [`record_game_streaming_ext`]).
pub type AttemptHook<'a> = dyn FnMut(&GameState, Side, &Command, &Result<CommandOutcome, cna_engine::state::command::CommandError>) + 'a;

/// [`record_game_streaming`], additionally reporting **every attempted** command (accepted or
/// rejected, in order) to `on_attempt` — enough to replay the game deterministically, since some
/// commands roll dice before validating — and returning the final [`GameState`].
#[allow(clippy::too_many_arguments)]
pub fn record_game_streaming_ext(
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
    on_attempt: &mut AttemptHook<'_>,
) -> std::io::Result<(u32, GameState)> {
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
    let mut prev_off_map: OffMapView = OffMapView::default();
    let mut prev_prisoners: Vec<PrisonerView> = Vec::new();
    let mut prev_captured: Vec<CapturedView> = Vec::new();
    let mut prev_dummy: Vec<DummyTankView> = Vec::new();
    let mut prev_broken: Vec<BrokenVehicleView> = Vec::new();
    let start = delta_frame(&game, 0, "-", "start".into(), String::new(), Vec::new(), &mut prev, &mut prev_weather, &mut prev_replacements, &mut prev_dumps, &mut prev_convoys, &mut prev_raiders, &mut prev_rommel, &mut prev_overlays, &mut prev_hexsides, &mut prev_air, &mut prev_naval, &mut prev_off_map, &mut prev_prisoners, &mut prev_captured, &mut prev_dummy, &mut prev_broken);
    writeln!(out, "{}", serde_json::to_string(&start)?)?;

    let mut step = 0u32;
    let mut guard = 0u64;
    let mut last_turn = game.game_turn;
    on_turn(game.game_turn, step);
    let mut checked_stage = None;
    while game.game_turn <= max_turns {
        let stage = (game.game_turn, game.operations_stage);
        if checked_stage != Some(stage) {
            checked_stage = Some(stage);
            if game.campaign_auto_winner().is_some() {
                break;
            }
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
            // Free-form agents (e.g. an LLM player via the harness) propose commands directly rather
            // than indexing the legal snapshot; they get a larger action budget (plans/macros).
            let free = {
                let a: &dyn Agent = if side == Side::Commonwealth { &*commonwealth } else { &*axis };
                a.free_form()
            };
            let cap = if free { cap.saturating_mul(10).max(200) } else { cap };
            let mut legal = if free { Vec::new() } else { game.legal_commands(side) };
            let mut acted = 0u32;
            let mut progress = true;
            while acted < cap {
                let agent: &mut dyn Agent = if side == Side::Commonwealth { commonwealth } else { axis };
                let pick = if free {
                    agent.choose(&game, side).map(Ok)
                } else {
                    agent.choose_index(&game, side, &legal).filter(|&i| i < legal.len()).map(Err)
                };
                match pick {
                    Some(p) => {
                        let cmd = match p {
                            Ok(c) => c,
                            Err(i) => legal.swap_remove(i),
                        };
                        let action = format!("{cmd:?}");
                        let rules: Vec<String> = cmd.rule_refs().iter().map(|s| s.to_string()).collect();
                        let cmd_desc = cmd.clone();
                        let result = game.apply_as(side, cmd);
                        on_attempt(&game, side, &cmd_desc, &result);
                        if free {
                            agent.on_result(&cmd_desc, &result);
                        }
                        if let Ok(outcome) = result {
                            step += 1;
                            let summary = describe_command(&game, &cmd_desc, &outcome);
                            let act = activity_target(&game, &cmd_desc);
                            let mut ev = delta_frame(&game, step, &format!("{side:?}"), action, format!("{outcome:?}"), rules, &mut prev, &mut prev_weather, &mut prev_replacements, &mut prev_dumps, &mut prev_convoys, &mut prev_raiders, &mut prev_rommel, &mut prev_overlays, &mut prev_hexsides, &mut prev_air, &mut prev_naval, &mut prev_off_map, &mut prev_prisoners, &mut prev_captured, &mut prev_dummy, &mut prev_broken);
                            ev.summary = summary;
                            ev.activity = act;
                            writeln!(out, "{}", serde_json::to_string(&ev)?)?;
                            progress = true;
                        }
                        acted += 1;
                    }
                    None => {
                        if free || !progress {
                            break;
                        }
                        legal = game.legal_commands(side);
                        progress = false;
                    }
                }
            }
        }
        game.advance_phase();
        // §4.44a and other system-initiated events surface as their own Action-Log entries.
        for entry in std::mem::take(&mut game.system_log) {
            step += 1;
            let side = entry.unit.and_then(|u| game.units.get(&u)).map(|u| format!("{:?}", u.nationality.side())).unwrap_or_else(|| "-".into());
            let act = entry.unit.and_then(|u| game.effective_hex(u)).map(t_hex);
            let mut ev = delta_frame(&game, step, &side, entry.label.clone(), String::new(), entry.rule_refs.clone(), &mut prev, &mut prev_weather, &mut prev_replacements, &mut prev_dumps, &mut prev_convoys, &mut prev_raiders, &mut prev_rommel, &mut prev_overlays, &mut prev_hexsides, &mut prev_air, &mut prev_naval, &mut prev_off_map, &mut prev_prisoners, &mut prev_captured, &mut prev_dummy, &mut prev_broken);
            ev.summary = entry.summary;
            ev.activity = act;
            writeln!(out, "{}", serde_json::to_string(&ev)?)?;
        }
        if game.game_turn != last_turn {
            last_turn = game.game_turn;
            out.flush()?;
            on_turn(game.game_turn, step);
        }
        guard += 1;
        assert!(guard < 5_000_000, "record loop failed to progress");
    }
    out.flush()?;
    Ok((step, game))
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
        constructed: true, unlimited: false,
    });
    // A Medium truck convoy parked at the depot. It starts idle/unbound — it is only committed to the
    // RP haul once the pool is loaded onto it (step 5, `LoadReplacement`, §20.44/§54.2), after which the
    // pool's `carried_by` binding makes the convoy an escort (§27.58) and it travels with the pool.
    game.truck_convoys.push(TruckConvoy {
        side: Side::Commonwealth,
        hex: HexCoord::new(0, 1),
        trucks: TruckPoints { light: 0, medium: 5, heavy: 0 },
        cargo: Commodities { fuel: 0, ammo: 0, stores: 0, water: 0 }, tank_fuel: 0,
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
    let mut prev_off_map: OffMapView = OffMapView::default();
    let mut prev_prisoners: Vec<PrisonerView> = Vec::new();
    let mut prev_captured: Vec<CapturedView> = Vec::new();
    let mut prev_dummy: Vec<DummyTankView> = Vec::new();
    let mut prev_broken: Vec<BrokenVehicleView> = Vec::new();
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
                     prev_off_map: &mut OffMapView,
                     prev_prisoners: &mut Vec<PrisonerView>,
                     prev_captured: &mut Vec<CapturedView>,
                     prev_dummy: &mut Vec<DummyTankView>,
                     prev_broken: &mut Vec<BrokenVehicleView>,
                     events: &mut Vec<Event>| {
        events.push(delta_frame(game, *step, "Commonwealth", action.into(), outcome, Vec::new(), prev, prev_weather, prev_rp, prev_dumps, prev_convoys, prev_raiders, prev_rommel, prev_overlays, prev_hexsides, prev_air, prev_naval, prev_off_map, prev_prisoners, prev_captured, prev_dummy, prev_broken));
        *step += 1;
    };

    // 0 — STAGE: a pool is planned to arrive at a future Naval Convoy Arrival Phase (§20.21).
    game.pending_replacements.push(PendingReplacement {
        class: ReplacementClass::Infantry, sub: None, count: 4, side: Side::Commonwealth,
        game_turn: 3, stage: 1, location: None, assigned_to: Some(unit), tonnage: 0,
    });
    frame(&game, &mut step, "Staged: Infantry replacement pool planned (§20.21)", String::new(), &mut prev, &mut prev_weather, &mut prev_rp, &mut prev_dumps, &mut prev_convoys, &mut prev_raiders, &mut prev_rommel, &mut prev_overlays, &mut prev_hexsides, &mut prev_air, &mut prev_naval, &mut prev_off_map, &mut prev_prisoners, &mut prev_captured, &mut prev_dummy, &mut prev_broken, &mut events);

    // 1 — ARRIVE: it lands on-map at a port hex and begins Training (§20.21 → §20.43).
    game.pending_replacements.clear();
    game.replacement_points.push(ReplacementPoints {
        class: ReplacementClass::Infantry, sub: None, count: 4, side: Side::Commonwealth,
        location: Some(UnitLocation::OnMap(HexCoord::new(0, 1))), training_remaining: 3, assigned_to: Some(unit), cpa_used: 0, carried_by: None,
    });
    frame(&game, &mut step, "Arrived: pool on-map at the port, training begins (§20.21/§20.43)", "ReplacementArrived".into(), &mut prev, &mut prev_weather, &mut prev_rp, &mut prev_dumps, &mut prev_convoys, &mut prev_raiders, &mut prev_rommel, &mut prev_overlays, &mut prev_hexsides, &mut prev_air, &mut prev_naval, &mut prev_off_map, &mut prev_prisoners, &mut prev_captured, &mut prev_dummy, &mut prev_broken, &mut events);

    // 2-4 — TRAIN: the pool trains down (§20.43) at the training area.
    for remaining in [2u32, 1, 0] {
        game.replacement_points[0].training_remaining = remaining;
        frame(&game, &mut step, "Training replacement pool (§20.43)", format!("Training: {remaining} stage(s) left"), &mut prev, &mut prev_weather, &mut prev_rp, &mut prev_dumps, &mut prev_convoys, &mut prev_raiders, &mut prev_rommel, &mut prev_overlays, &mut prev_hexsides, &mut prev_air, &mut prev_naval, &mut prev_off_map, &mut prev_prisoners, &mut prev_captured, &mut prev_dummy, &mut prev_broken, &mut events);
    }

    // 5 — LOAD: in the Truck Convoy phase, the convoy commits to the haul — the trained pool is loaded
    // onto it (§20.44 `LoadReplacement`, binding the two) and it picks up the depot's Fuel (§53.14).
    game.phase = Phase::TruckConvoy;
    game.active_player = Side::Commonwealth;
    let picked = game.apply(Command::LoadReplacement { convoy: 0, pool: 0 });
    let loaded = game.apply(Command::LoadConvoy { convoy: 0, commodity: Commodity::Fuel, amount: 30 });
    frame(&game, &mut step, "Convoy loads the pool + Fuel at the depot (§20.44/§53.14)", format!("{picked:?}; {loaded:?}"), &mut prev, &mut prev_weather, &mut prev_rp, &mut prev_dumps, &mut prev_convoys, &mut prev_raiders, &mut prev_rommel, &mut prev_overlays, &mut prev_hexsides, &mut prev_air, &mut prev_naval, &mut prev_off_map, &mut prev_prisoners, &mut prev_captured, &mut prev_dummy, &mut prev_broken, &mut events);

    // 6-9 — TRAVEL: the pool moves one hex per step toward its unit via §20.46 MoveReplacement; the
    // carrying convoy burns its own carried Fuel (§49.18), so no en-route dumps are needed. Each step
    // is a fresh Operations Stage, so the pool's §20.42d CPA resets between steps.
    for hex in [1i16, 2, 3, 4] {
        game.replacement_points[0].cpa_used = 0; // new stage's Truck Convoy Phase
        let outcome = game.apply(Command::MoveReplacement { index: 0, to: HexCoord::new(hex, 1) });
        frame(&game, &mut step, "Delivering replacement pool toward its unit (§20.46)", format!("{outcome:?}"), &mut prev, &mut prev_weather, &mut prev_rp, &mut prev_dumps, &mut prev_convoys, &mut prev_raiders, &mut prev_rommel, &mut prev_overlays, &mut prev_hexsides, &mut prev_air, &mut prev_naval, &mut prev_off_map, &mut prev_prisoners, &mut prev_captured, &mut prev_dummy, &mut prev_broken, &mut events);
    }

    // 7 — ABSORB: co-located and trained, the pool is absorbed and the unit's TOE jumps (§20.49).
    let added = game.absorb_replacement(0, unit).unwrap_or(0);
    frame(&game, &mut step, "Absorbed into the unit (§20.49)", format!("ReplacementAbsorbed({added})"), &mut prev, &mut prev_weather, &mut prev_rp, &mut prev_dumps, &mut prev_convoys, &mut prev_raiders, &mut prev_rommel, &mut prev_overlays, &mut prev_hexsides, &mut prev_air, &mut prev_naval, &mut prev_off_map, &mut prev_prisoners, &mut prev_captured, &mut prev_dummy, &mut prev_broken, &mut events);

    // 8 — DEMOLISH: the NZ Rail Coy at (3,1) demolishes its own railroad hex (§24.66 scorched earth) via
    // a real `DemolishRailroad` command; completing the one-stage project flips both rail edges of the
    // hex to DestroyedRailroad — a genuine on-board action, surfaced as a hexside delta.
    game.phase = Phase::Organization;
    game.active_player = Side::Commonwealth;
    let started = game.apply(Command::DemolishRailroad { builder: sapper, a: HexCoord::new(3, 1), b: HexCoord::new(4, 1) });
    game.advance_construction();
    frame(&game, &mut step, "NZ Rail Coy demolishes the railroad at (3,1) (§24.66)", format!("{started:?}"), &mut prev, &mut prev_weather, &mut prev_rp, &mut prev_dumps, &mut prev_convoys, &mut prev_raiders, &mut prev_rommel, &mut prev_overlays, &mut prev_hexsides, &mut prev_air, &mut prev_naval, &mut prev_off_map, &mut prev_prisoners, &mut prev_captured, &mut prev_dummy, &mut prev_broken, &mut events);

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
        // Every starting unit is accounted for: reconstructed by folding, or explicitly removed
        // (eliminated) by a later frame — whatever the agents' play did to it.
        let removed: std::collections::HashSet<u32> = rec.events.iter().flat_map(|e| e.removed.iter().copied()).collect();
        for uv in &rec.events[0].units {
            assert!(folded.contains_key(&uv.id) || removed.contains(&uv.id), "unit {} vanished without a removal", uv.id);
        }
        assert_eq!(folded.len() + removed.len(), 2, "folding the deltas reconstructs every unit not removed");
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
        g.ports.insert("Tobruk".into(), Port { name: "Tobruk".into(), max_efficiency: 3, efficiency: 1, max_tonnage: 100, mines: 0, wreck_block: 2, stacking_in: Some(1), stacking_out: Some(3), tonnage_used_this_turn: 0, troops_landed_sp_this_turn: 0 });
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
        assert_eq!(av.facilities[0].occupancy, 0, "no squadrons stationed → occupancy 0");
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
            class: ReplacementClass::Infantry, sub: None, count: 3, side: Side::Commonwealth,
            location: Some(UnitLocation::OnMap(HexCoord::new(1, 1))), training_remaining: 0, assigned_to: Some(u), cpa_used: 0, carried_by: None,
        });
        // Pool still in training at a training area (§20.43).
        g.replacement_points.push(ReplacementPoints {
            class: ReplacementClass::Tank, sub: None, count: 2, side: Side::Commonwealth,
            location: Some(UnitLocation::OnMap(HexCoord::new(0, 1))), training_remaining: 4, assigned_to: None, cpa_used: 0, carried_by: None,
        });
        // Staged future arrival, not yet on the map (§20.21).
        g.pending_replacements.push(PendingReplacement {
            class: ReplacementClass::Gun, sub: None, count: 1, side: Side::Commonwealth,
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
