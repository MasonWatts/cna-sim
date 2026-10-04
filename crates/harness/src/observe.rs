//! Per-side, fog-of-war-safe JSON views served to external (LLM) players.
//!
//! Everything here is built from the engine's observation layer (`observe_*`), never from raw
//! enemy state: enemy TOE is reported only when the engine says the viewer knows it (own unit, or
//! revealed by reconnaissance), enemy supply dumps show only that a marker exists, hidden raiders
//! are absent, and dummy flags appear only for the viewer's own minefields.

use cna_engine::reference::units::Side;
use cna_engine::state::command::Command;
use cna_engine::state::game::{date_of, GameState};
use cna_engine::state::map::{section_char_of_col, HexCoord};
use cna_engine::state::unit::{UnitId, UnitLocation};
use serde_json::{json, Value};

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// The rulebook hex label (e.g. `A2109`) for an internal coordinate.
pub fn hex_label(h: HexCoord) -> String {
    let section = section_char_of_col(h.col);
    let cc = h.col as i32 - (section as u8 - b'A') as i32 * 33;
    format!("{section}{:02}{:02}", h.row, cc)
}

/// A hex as both the rulebook label and the `[col, row]` coordinate used in command JSON.
pub fn hex_json(h: HexCoord) -> Value {
    json!({ "label": hex_label(h), "coord": [h.col, h.row] })
}

fn location_json(loc: &UnitLocation) -> Value {
    match loc {
        UnitLocation::OnMap(h) => hex_json(*h),
        UnitLocation::OffMap { node, .. } => json!({ "off_map": node }),
    }
}

/// Where the game is and whose turn it is.
pub fn status(game: &GameState, side: Side, in_window: bool) -> Value {
    let d = date_of(game.game_turn);
    json!({
        "you_are": format!("{side:?}"),
        "game_turn": game.game_turn,
        "date": format!("Week {} of {} {}", d.week, MONTHS[(d.month.clamp(1, 12) - 1) as usize], d.year),
        "operations_stage": game.operations_stage,
        "phase": format!("{:?}", game.phase),
        "acting_side": format!("{:?}", game.active_player),
        "your_window_is_open": in_window,
        "weather": format!("{:?}", game.weather),
        "window": game.decision_window_key(),
    })
}

/// Observation filter (all optional).
#[derive(Debug, Clone, Default, serde::Deserialize, schemars::JsonSchema)]
pub struct ObserveFilter {
    /// Only include things within `radius` hexes of this `[col, row]` coordinate.
    pub near: Option<[i16; 2]>,
    /// Radius for `near` (default 6).
    pub radius: Option<u32>,
    /// Sections to include: any of `own_units`, `enemy_units`, `supply_dumps`, `minefields`,
    /// `raiders`. Default: all.
    pub include: Option<Vec<String>>,
}

impl ObserveFilter {
    fn wants(&self, section: &str) -> bool {
        self.include.as_ref().map_or(true, |v| v.iter().any(|s| s == section))
    }
    fn in_range(&self, h: Option<HexCoord>) -> bool {
        match (self.near, h) {
            (None, _) => true,
            (Some(_), None) => false,
            (Some([c, r]), Some(h)) => HexCoord::new(c, r).distance(h) <= self.radius.unwrap_or(6),
        }
    }
}

/// The side's observation of the board.
pub fn observation(game: &GameState, side: Side, f: &ObserveFilter) -> Value {
    let mut out = serde_json::Map::new();
    out.insert("status".into(), status(game, side, true));
    let views = game.observe_units(side);
    if f.wants("own_units") {
        let own: Vec<Value> = views
            .iter()
            .filter(|v| v.friendly)
            .filter(|v| f.in_range(game.effective_hex(v.id)))
            .filter_map(|v| own_unit_json(game, v.id))
            .collect();
        out.insert("own_units".into(), Value::Array(own));
    }
    if f.wants("enemy_units") {
        let enemy: Vec<Value> = views
            .iter()
            .filter(|v| !v.friendly && v.hex.is_some())
            .filter(|v| f.in_range(v.hex))
            .map(|v| {
                json!({
                    "id": v.id.0,
                    "name": v.name,
                    "nationality": format!("{:?}", v.nationality),
                    "size": format!("{:?}", v.org_size),
                    "is_hq": v.is_hq,
                    "hex": v.hex.map(hex_json),
                    // Only present when known to the viewer (§42.27 recon) — otherwise hidden.
                    "toe_strength": v.strength,
                })
            })
            .collect();
        out.insert("enemy_units".into(), Value::Array(enemy));
    }
    if f.wants("supply_dumps") {
        let dumps: Vec<Value> = game
            .observe_supply_dumps(side)
            .into_iter()
            .filter(|d| f.in_range(d.location.hex()))
            .map(|d| {
                json!({
                    "friendly": d.friendly,
                    "location": location_json(&d.location),
                    "contents": d.contents.map(|c| json!({"fuel": c.fuel, "ammo": c.ammo, "stores": c.stores, "water": c.water})),
                    "dummy": d.dummy,
                })
            })
            .collect();
        out.insert("supply_dumps".into(), Value::Array(dumps));
    }
    if f.wants("minefields") {
        let mf: Vec<Value> = game
            .observe_minefields(side)
            .into_iter()
            .filter(|m| f.in_range(Some(m.hex)))
            .map(|m| json!({"hex": hex_json(m.hex), "friendly": m.friendly, "dummy": m.dummy}))
            .collect();
        out.insert("minefields".into(), Value::Array(mf));
    }
    if f.wants("raiders") {
        let r: Vec<Value> = game
            .observe_raiders(side)
            .into_iter()
            .filter(|r| r.hex.is_some() && f.in_range(r.hex))
            .map(|r| json!({"kind": format!("{:?}", r.kind), "friendly": r.friendly, "hex": r.hex.map(hex_json)}))
            .collect();
        out.insert("raiders".into(), Value::Array(r));
    }
    Value::Object(out)
}

/// Full detail for one of the viewer's own units.
fn own_unit_json(game: &GameState, id: UnitId) -> Option<Value> {
    let u = game.units.get(&id)?;
    let hex = game.effective_hex(id);
    Some(json!({
        "id": id.0,
        "name": u.name,
        "abbr": u.abbr,
        "nationality": format!("{:?}", u.nationality),
        "size": format!("{:?}", u.org_size),
        "type": u.unit_type.as_ref().map(|t| format!("{t:?}")),
        "is_hq": u.is_hq,
        "hex": hex.map(hex_json),
        "attached_to": u.attached_parent.map(|p| p.0),
        "toe_strength": u.total_toe(),
        "cohesion": u.cohesion,
        "cpa": game.effective_cpa(id),
        "cp_spent": u.cp_expended.0 as f64 / 10.0,
        "fuel": u.fuel,
        "ammo": u.ammo,
        "motorized": game.unit_is_motorized(id),
        "reserve": format!("{:?}", u.reserve),
        "pinned": u.status.pinned,
        "in_contact": u.status.in_contact,
        "engaged": u.status.engaged,
    }))
}

/// The top-level variant name of a command (`MoveStep`, `CloseAssault`, …).
pub fn command_kind(cmd: &Command) -> String {
    match serde_json::to_value(cmd) {
        Ok(Value::Object(m)) => m.keys().next().cloned().unwrap_or_default(),
        Ok(Value::String(s)) => s,
        _ => String::new(),
    }
}

/// Whether a command's JSON references unit `id` in one of the usual unit-bearing fields.
pub fn command_mentions_unit(cmd: &Command, id: u32) -> bool {
    const KEYS: [&str; 12] = [
        "unit", "attackers", "barragers", "from_unit", "donor", "receiver", "builder", "parent", "hq",
        "formation", "units", "defenders",
    ];
    fn walk(v: &Value, id: u32, hit: &mut bool) {
        match v {
            Value::Object(m) => {
                for (k, x) in m {
                    if KEYS.contains(&k.as_str()) {
                        let found = match x {
                            Value::Number(n) => n.as_u64() == Some(id as u64),
                            Value::Array(a) => a.iter().any(|e| e.as_u64() == Some(id as u64)),
                            _ => false,
                        };
                        if found {
                            *hit = true;
                        }
                    }
                    walk(x, id, hit);
                }
            }
            Value::Array(a) => a.iter().for_each(|x| walk(x, id, hit)),
            _ => {}
        }
    }
    let mut hit = false;
    if let Ok(v) = serde_json::to_value(cmd) {
        walk(&v, id, &mut hit);
    }
    hit
}

/// Legal-action listing filter / pagination.
#[derive(Debug, Clone, Default, serde::Deserialize, schemars::JsonSchema)]
pub struct LegalFilter {
    /// Only actions involving this unit id.
    pub unit: Option<u32>,
    /// Only actions of this command kind (e.g. `MoveStep`, `CloseAssault`, `RefuelUnit`).
    pub kind: Option<String>,
    /// Pagination offset (default 0).
    pub offset: Option<usize>,
    /// Page size (default 40, max 200).
    pub limit: Option<usize>,
}

/// One page of legal actions. `id`s index the full (unfiltered) `legal` snapshot and stay valid
/// until the next listing or the next applied command.
pub fn legal_page(game: &GameState, legal: &[Command], f: &LegalFilter) -> Value {
    let mut kinds: std::collections::BTreeMap<String, usize> = Default::default();
    for c in legal {
        *kinds.entry(command_kind(c)).or_default() += 1;
    }
    let matching: Vec<(usize, &Command)> = legal
        .iter()
        .enumerate()
        .filter(|(_, c)| f.kind.as_ref().map_or(true, |k| &command_kind(c) == k))
        .filter(|(_, c)| f.unit.map_or(true, |u| command_mentions_unit(c, u)))
        .collect();
    let offset = f.offset.unwrap_or(0);
    let limit = f.limit.unwrap_or(40).min(200);
    let page: Vec<Value> = matching
        .iter()
        .skip(offset)
        .take(limit)
        .map(|(i, c)| {
            json!({
                "id": i,
                "kind": command_kind(c),
                "text": cna_replay::describe_action(game, c),
                "command": serde_json::to_value(c).unwrap_or(Value::Null),
            })
        })
        .collect();
    json!({
        "total_legal": legal.len(),
        "counts_by_kind": kinds,
        "matching": matching.len(),
        "offset": offset,
        "actions": page,
        "note": "Submit actions with submit_plan using {\"id\": n} (from this listing) or {\"command\": <command JSON>}.",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cna_engine::state::map::HexId;

    #[test]
    fn hex_label_round_trips_rulebook_ids() {
        for id in ["A2109", "C4807", "E1430", "B0101", "D3714"] {
            let h = HexId::parse(id).unwrap().to_hex_coord();
            assert_eq!(hex_label(h), id);
        }
    }

    #[test]
    fn command_kind_and_unit_mentions() {
        let c = Command::MoveStep { unit: UnitId(7), to: HexCoord::new(3, 4), mode: cna_engine::rules::movement::MoveMode::Motorized };
        assert_eq!(command_kind(&c), "MoveStep");
        assert!(command_mentions_unit(&c, 7));
        assert!(!command_mentions_unit(&c, 8));
        let a = Command::CloseAssault { attackers: vec![UnitId(1), UnitId(2)], target: HexCoord::new(0, 0) };
        assert!(command_mentions_unit(&a, 2));
    }
}
