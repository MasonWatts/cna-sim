//! Per-side, fog-of-war-safe JSON views served to external (LLM) players.
//!
//! Everything here is built from the engine's observation layer (`observe_*`), never from raw
//! enemy state: enemy TOE is reported only when the engine says the viewer knows it (own unit, or
//! revealed by reconnaissance), enemy supply dumps show only that a marker exists, hidden raiders
//! are absent, and dummy flags appear only for the viewer's own minefields.

use cna_engine::reference::units::Side;
use cna_engine::state::command::{Command, CommandError};
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
    /// Only these own unit ids (implies full detail for them).
    pub units: Option<Vec<u32>>,
    /// Full per-unit detail (default false = compact one-line-per-unit form).
    pub detail: Option<bool>,
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
        // Legal actions per own unit right now (only meaningful inside this side's window).
        let legal = if game.active_player == side { game.legal_commands(side) } else { Vec::new() };
        let own: Vec<Value> = views
            .iter()
            .filter(|v| v.friendly)
            .filter(|v| f.in_range(game.effective_hex(v.id)))
            .filter(|v| f.units.as_ref().map_or(true, |ids| ids.contains(&v.id.0)))
            .filter_map(|v| {
                let n = legal.iter().filter(|c| command_mentions_unit(c, v.id.0)).count();
                let legal_now = (game.active_player == side).then_some(n);
                if f.detail.unwrap_or(false) || f.units.is_some() {
                    own_unit_json(game, v.id, legal_now)
                } else {
                    own_unit_compact(game, v.id, legal_now)
                }
            })
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

fn is_vehicle_unit(game: &GameState, id: UnitId) -> bool {
    game.units.get(&id).is_some_and(|u| {
        matches!(u.composition, cna_engine::state::unit::Composition::Weapons { .. })
            || u.trucks.light + u.trucks.medium + u.trucks.heavy > 0
    })
}

/// Why an own unit may be unable to act, citing the rule — the facts a player needs to explain an
/// empty or short legal-action list. `legal_now` = its legal-action count (when in its window).
pub fn unit_blockers(game: &GameState, id: UnitId, legal_now: Option<usize>) -> Vec<String> {
    let Some(u) = game.units.get(&id) else { return Vec::new() };
    let mut b = Vec::new();
    if u.status.pinned {
        b.push("Pinned — may not move, fire anti-armor or close-assault this stage (§12.44).".to_string());
    }
    if u.cohesion <= -26 {
        b.push("Disorganized (cohesion ≤ −26) — may not move or attack (§6.26).".into());
    }
    if u.stages_without_water > 0 {
        if is_vehicle_unit(game, id) {
            b.push("OUT OF WATER — vehicles (tanks, trucks, motorized units) may not move or close-assault (§52.51).".into());
        } else {
            b.push("OUT OF WATER — may not exceed CPA or close-assault; loses 1 TOE each further stage without water (§52.5).".into());
        }
    }
    if game.unit_is_motorized(id) && u.fuel == 0 {
        b.push("Out of fuel — no motorized movement or offensive close assault (§49).".into());
    }
    if matches!(u.composition, cna_engine::state::unit::Composition::Weapons { .. }) && u.ammo == 0 {
        b.push("Out of ammunition — cannot fire (§50).".into());
    }
    if let Some(cpa) = game.effective_cpa(id) {
        if u.cp_expended.0 >= cpa * 10 {
            b.push("CPA fully spent this stage — further action costs cohesion.".into());
        }
    }
    if matches!(u.reserve, cna_engine::state::unit::ReserveStatus::ReserveII) {
        b.push("In Reserve II — may not move (§18.22).".into());
    }
    if game.half_rations.contains(&id) {
        b.push("On half rations — may not exceed CPA or enter an enemy ZOC (§51.23).".into());
    }
    if game.no_pasta.contains(&id) {
        b.push("Missed its Pasta Point — may not exceed CPA (§52.6).".into());
    }
    if legal_now == Some(0) && b.is_empty() {
        b.push("No legal action right now (terrain, ZOC, stacking, or no target in range).".into());
    }
    b
}

/// A player-facing explanation of an engine rejection — what blocked it, the rule, and the way
/// out — for the commands where the error name alone isn't actionable.
pub fn explain_rejection(game: &GameState, cmd: &Command, err: &CommandError) -> Option<String> {
    let (unit, dest) = match cmd {
        Command::MoveStep { unit, to, .. } => (Some(*unit), Some(*to)),
        Command::NavalTransferTroops { unit, to_port } => (Some(*unit), game.port_hex(to_port)),
        _ => (None, None),
    };
    explain_error(game, unit, dest, err)
}

/// See [`explain_rejection`]; `unit`/`dest` are the mover and the hex it would end in.
pub fn explain_error(game: &GameState, unit: Option<UnitId>, dest: Option<HexCoord>, err: &CommandError) -> Option<String> {
    use CommandError as E;
    Some(match err {
        E::StackingLimitExceeded => {
            let (u, h) = (unit?, dest?);
            let name = game.units.get(&u).map(|x| x.name.as_str()).unwrap_or("this unit");
            let sp = game.effective_stacking_points(u);
            let here = game.hex_stacking_points(h);
            match game.hex_stacking_limit(h) {
                Some(limit) if here + sp > limit => format!(
                    "{} already holds {here} of its {limit} Stacking Points ({:?} terrain, §8.37) and {name} counts {sp} \
                     (§9.4 — a division with its brigades attached counts 5, §9.13). Units may not end movement over the \
                     limit (§9.31). Options: detach brigades/battalions in the Organization phase and move them separately, \
                     move units out of that hex first, or stop in a neighbouring hex.",
                    hex_label(h),
                    game.map.terrain(h).unwrap_or(cna_engine::reference::terrain::HexTerrain::Clear),
                ),
                _ => "At most five independent 0-Stacking-Point companies/batteries may share a hex outside a Major City \
                      (§9.25). Attach it to a parent or use another hex."
                    .into(),
            }
        }
        E::InsufficientFuel => "Not enough Fuel in the unit's tanks (or a dump in its hex) to pay for this move (§49.13). \
                                Refuel from a dump in its hex in the Organization phase, or move NonMotorized."
            .into(),
        E::NoWater => "Out of water: vehicles may not move (§52.51); infantry may not exceed their CPA (§52.52). Get water \
                       from a well, oasis, major city or water pipeline/railroad hex, or by truck."
            .into(),
        E::ZocToZoc => "May not move directly from one enemy Zone of Control hex into another (§10).".into(),
        E::MustStopInZoc => "The unit entered an enemy Zone of Control this stage and must stop (§8.14).".into(),
        E::Pinned => "Pinned — may not move this stage (§12.44).".into(),
        E::Disorganized => "Disorganized (cohesion ≤ −26) — may not move voluntarily (§6.26).".into(),
        E::InReserve => "In Reserve — release it first (§18.2).".into(),
        E::HalfRation => "On half rations — may not exceed its CPA or enter an enemy ZOC (§51.23).".into(),
        E::NoPasta => "Missed its Pasta Point — may not exceed its CPA (§52.6).".into(),
        E::CpaCeilingExceeded => "Non-motorized units may never spend more than 150% of their CPA in a stage (§8.17).".into(),
        E::ProhibitedTerrain => "The hex or hexside is prohibited for this unit — enemy-occupied (§8.13), prohibited terrain \
                                 for its movement type (§8.37), or a salt marsh off a track for vehicles (§8.44)."
            .into(),
        _ => return None,
    })
}

/// One-line form of an own unit (the default): enough to plan with; ask for `detail` or `units`
/// for the full record.
fn own_unit_compact(game: &GameState, id: UnitId, legal_now: Option<usize>) -> Option<Value> {
    let u = game.units.get(&id)?;
    let hex = game.effective_hex(id)?;
    let flags: Vec<&str> = [
        (u.stages_without_water > 0, "NO_WATER"),
        (game.unit_is_motorized(id) && u.fuel == 0, "NO_FUEL"),
        (matches!(u.composition, cna_engine::state::unit::Composition::Weapons { .. }) && u.ammo == 0, "NO_AMMO"),
        (u.status.pinned, "PINNED"),
        (u.cohesion <= -26, "DISORGANIZED"),
        (u.status.in_contact, "IN_CONTACT"),
        (u.status.engaged, "ENGAGED"),
        (u.attached_parent.is_some(), "ATTACHED"),
    ]
    .into_iter()
    .filter_map(|(on, f)| on.then_some(f))
    .collect();
    Some(json!({
        "id": id.0,
        "name": u.name,
        "hex": [hex.col, hex.row],
        "toe": u.total_toe(),
        "coh": u.cohesion,
        "cp": format!("{}/{}", u.cp_expended.0 / 10, game.effective_cpa(id).unwrap_or(0)),
        "fuel": u.fuel,
        "ammo": u.ammo,
        "flags": flags,
        "legal": legal_now,
    }))
}

/// Root of a unit's organizational chain (its top HQ), for formation grouping.
fn formation_root(game: &GameState, id: UnitId) -> UnitId {
    let mut cur = id;
    for _ in 0..16 {
        match game.units.get(&cur).and_then(|u| u.assigned_parent.or(u.attached_parent)) {
            Some(p) => cur = p,
            None => break,
        }
    }
    cur
}

/// Theatre-level summary for large scenarios: own stacks, formations, supply problems, enemy
/// contacts by hex, and own dumps — a few KB instead of the full unit list.
pub fn overview(game: &GameState, side: Side) -> Value {
    use std::collections::BTreeMap;
    let views = game.observe_units(side);
    let own: Vec<UnitId> = views
        .iter()
        .filter(|v| v.friendly)
        .map(|v| v.id)
        .filter(|id| game.effective_hex(*id).is_some())
        .collect();
    // Supply / status problems.
    let mut problems: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for &id in &own {
        let u = &game.units[&id];
        let mut push = |k: &'static str| problems.entry(k).or_default().push(format!("{} ({})", u.name, id.0));
        if u.stages_without_water > 0 { push("out_of_water"); }
        if game.unit_is_motorized(id) && u.fuel == 0 { push("out_of_fuel"); }
        if matches!(u.composition, cna_engine::state::unit::Composition::Weapons { .. }) && u.ammo == 0 { push("out_of_ammo"); }
        if u.cohesion <= -26 { push("disorganized"); }
        if u.status.pinned { push("pinned"); }
        if u.turns_without_stores > 0 { push("short_of_stores"); }
    }
    let problems: BTreeMap<&str, Value> = problems
        .into_iter()
        .map(|(k, v)| (k, json!({"count": v.len(), "units": v.iter().take(12).collect::<Vec<_>>()})))
        .collect();
    // Own stacks by hex.
    let mut stacks: BTreeMap<(i16, i16), Vec<UnitId>> = BTreeMap::new();
    for &id in &own {
        let h = game.effective_hex(id).unwrap();
        stacks.entry((h.col, h.row)).or_default().push(id);
    }
    let mut stack_list: Vec<Value> = stacks
        .iter()
        .map(|(&(c, r), ids)| {
            let toe: u32 = ids.iter().map(|i| game.units[i].total_toe()).sum();
            let contact = ids.iter().any(|i| game.units[i].status.in_contact || game.units[i].status.engaged);
            let names: Vec<&str> = ids.iter().take(4).map(|i| game.units[i].name.as_str()).collect();
            json!({"hex": hex_label(HexCoord::new(c, r)), "coord": [c, r], "units": ids.len(), "toe": toe,
                   "in_contact": contact, "names": names, "more": ids.len().saturating_sub(4)})
        })
        .collect();
    stack_list.sort_by_key(|v| (!v["in_contact"].as_bool().unwrap_or(false), std::cmp::Reverse(v["toe"].as_u64().unwrap_or(0))));
    let stacks_total = stack_list.len();
    stack_list.truncate(60);
    // Formations (top HQ → members).
    let mut forms: BTreeMap<u32, Vec<UnitId>> = BTreeMap::new();
    for &id in &own {
        forms.entry(formation_root(game, id).0).or_default().push(id);
    }
    let mut form_list: Vec<Value> = forms
        .iter()
        .filter(|(_, m)| m.len() > 1)
        .map(|(&root, m)| {
            let toe: u32 = m.iter().map(|i| game.units[i].total_toe()).sum();
            let mut hexes: Vec<String> = m.iter().filter_map(|i| game.effective_hex(*i)).map(hex_label).collect();
            hexes.sort();
            hexes.dedup();
            json!({"hq_id": root, "hq": game.units.get(&UnitId(root)).map(|u| u.name.clone()), "units": m.len(),
                   "toe": toe, "hexes": hexes.iter().take(6).collect::<Vec<_>>(), "hex_count": hexes.len()})
        })
        .collect();
    form_list.sort_by_key(|v| std::cmp::Reverse(v["toe"].as_u64().unwrap_or(0)));
    form_list.truncate(40);
    // Enemy contacts by hex.
    let mut enemy: BTreeMap<(i16, i16), Vec<&cna_engine::state::view::UnitView>> = BTreeMap::new();
    for v in views.iter().filter(|v| !v.friendly) {
        if let Some(h) = v.hex {
            enemy.entry((h.col, h.row)).or_default().push(v);
        }
    }
    let enemy_total: usize = enemy.values().map(|v| v.len()).sum();
    let mut enemy_list: Vec<Value> = enemy
        .iter()
        .map(|(&(c, r), vs)| {
            let names: Vec<&str> = vs.iter().take(3).map(|v| v.name.as_str()).collect();
            let sizes: Vec<String> = vs.iter().map(|v| format!("{:?}", v.org_size)).collect();
            json!({"hex": hex_label(HexCoord::new(c, r)), "coord": [c, r], "units": vs.len(), "names": names, "sizes": sizes})
        })
        .collect();
    enemy_list.sort_by_key(|v| std::cmp::Reverse(v["units"].as_u64().unwrap_or(0)));
    enemy_list.truncate(60);
    // Own real dumps.
    let mut dumps: Vec<Value> = game
        .observe_supply_dumps(side)
        .into_iter()
        .filter(|d| d.friendly && d.dummy != Some(true))
        .filter_map(|d| {
            let c = d.contents?;
            Some(json!({"at": location_json(&d.location), "fuel": c.fuel, "ammo": c.ammo, "stores": c.stores, "water": c.water}))
        })
        .collect();
    dumps.sort_by_key(|v| std::cmp::Reverse(["fuel", "ammo", "stores", "water"].iter().map(|k| v[*k].as_u64().unwrap_or(0)).sum::<u64>()));
    let dumps_total = dumps.len();
    dumps.truncate(30);
    let total_toe: u32 = own.iter().map(|i| game.units[i].total_toe()).sum();
    json!({
        "status": status(game, side, true),
        "own": {"units_on_map": own.len(), "total_toe": total_toe, "stacks": stacks_total, "formations": forms.len()},
        "problems": problems,
        "stacks": stack_list,
        "formations": form_list,
        "enemy_contacts": {"units_visible": enemy_total, "by_hex": enemy_list},
        "own_dumps": {"count": dumps_total, "largest": dumps},
        "note": "Lists are capped (contact stacks and largest first). Drill down with get_observation (near / units / detail) and list_legal_actions (unit / kind).",
    })
}

/// Full detail for one of the viewer's own units.
fn own_unit_json(game: &GameState, id: UnitId, legal_now: Option<usize>) -> Option<Value> {
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
        "water": u.water,
        "out_of_water": u.stages_without_water > 0,
        "stages_without_water": u.stages_without_water,
        "turns_without_stores": u.turns_without_stores,
        "motorized": game.unit_is_motorized(id),
        "reserve": format!("{:?}", u.reserve),
        "pinned": u.status.pinned,
        "in_contact": u.status.in_contact,
        "engaged": u.status.engaged,
        "legal_actions_now": legal_now,
        "blockers": unit_blockers(game, id, legal_now),
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
    fn blockers_explain_why_a_unit_cannot_act() {
        let (mut g, _) = cna_replay::scenarios::build("skirmish", 1);
        let tank = g.units.values().find(|u| u.name == "1st RTR").unwrap().id;
        let inf = g.units.values().find(|u| u.name == "1st Rifle Bde").unwrap().id;
        assert!(unit_blockers(&g, tank, None).is_empty(), "a supplied unit has no blockers");
        g.units.get_mut(&tank).unwrap().stages_without_water = 1;
        g.units.get_mut(&inf).unwrap().stages_without_water = 1;
        g.units.get_mut(&inf).unwrap().status.pinned = true;
        let t = unit_blockers(&g, tank, None).join(" ");
        assert!(t.contains("OUT OF WATER") && t.contains("vehicles") && t.contains("§52.51"), "{t}");
        let i = unit_blockers(&g, inf, None).join(" ");
        assert!(i.contains("Pinned") && i.contains("loses 1 TOE"), "{i}");
        // An unexplained empty legal list is still called out.
        g.units.get_mut(&tank).unwrap().stages_without_water = 0;
        assert!(unit_blockers(&g, tank, Some(0))[0].contains("No legal action"));
        let obs = observation(&g, cna_engine::reference::units::Side::Commonwealth, &ObserveFilter { detail: Some(true), ..Default::default() });
        let u0 = &obs["own_units"][0];
        assert!(u0.get("water").is_some() && u0.get("blockers").is_some() && u0.get("out_of_water").is_some());
    }

    #[test]
    fn rejections_are_explained_with_the_rule() {
        use cna_engine::reference::terrain::HexTerrain;
        use cna_engine::rules::movement::MoveMode;
        let (mut g, _) = cna_replay::scenarios::build("skirmish", 1);
        g.phase = cna_engine::state::game::Phase::MovementAndCombat;
        g.active_player = Side::Commonwealth;
        let tank = g.units.values().find(|u| u.name == "1st RTR").unwrap().id;
        let at = g.units[&tank].hex().unwrap();
        let n = at.neighbors().into_iter().find(|n| g.map.hex(*n).is_some() && g.units_in_hex(*n).is_empty()).unwrap();
        g.map.set_hex(n, HexTerrain::Mountain); // Stacking Limit 3
        // Fill it: one other own counter, counted as a full division (5 SP > 3).
        let filler = g
            .units
            .values()
            .find(|u| u.id != tank && u.nationality.side() == Side::Commonwealth && u.hex().is_some())
            .unwrap()
            .id;
        let fu = g.units.get_mut(&filler).unwrap();
        fu.location = Some(UnitLocation::OnMap(n));
        fu.org_size = cna_engine::state::unit::OrgSize::Division;
        let err = g.move_step_error(tank, n, MoveMode::Motorized).expect("full hex rejects the step");
        assert_eq!(err, CommandError::StackingLimitExceeded);
        let cmd = Command::MoveStep { unit: tank, to: n, mode: MoveMode::Motorized };
        let why = explain_rejection(&g, &cmd, &err).unwrap();
        assert!(why.contains("of its 3 Stacking Points") && why.contains("§9.31") && why.contains("detach"), "{why}");
        assert!(explain_rejection(&g, &cmd, &CommandError::InsufficientFuel).unwrap().contains("§49"));
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
