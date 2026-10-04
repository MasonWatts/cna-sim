//! What a model player is told.
//!
//! Each decision window is a **fresh** model session (bounded context and cost). Continuity across
//! the game is carried by the harness, not the session:
//!
//! - [`system_prompt`] — fixed per side: commander persona, the CNA mechanics that matter, the tool
//!   contract, and the memory contract. Replaces the client's default system prompt.
//! - [`window_prompt`] — rebuilt each window: scenario objectives, the current situation, the
//!   harness-written journal of earlier windows (fog-of-war-safe), and the commander's notebook the
//!   model itself maintains with `write_notes`.

use cna_engine::reference::units::Side;
use cna_engine::state::game::GameState;

use crate::memory::Memory;
use crate::observe;

/// The scenario's victory conditions for `side`, in brief (from the rulebook).
pub fn objectives(scenario: &str, side: Side) -> String {
    match (scenario, side) {
        ("scenario1", Side::Axis) => "Scenario: Graziani's Offensive (§60.81). You command the Italian invasion of Egypt. \
Victory, best first: STRATEGIC — take and hold Alexandria or Cairo with a combat unit supplied by convoy from map D; \
DECISIVE — take and hold both Mersa Matruh and Siwa, supplied by convoy from Tobruk; TACTICAL — take and hold Sidi Barrani \
and keep Sollum (C4021), Fort Maddalena and Giarabub in supply (traced to Tobruk).".into(),
        ("scenario1", Side::Commonwealth) => "Scenario: Graziani's Offensive (§60.81). You defend Egypt against the Italian \
invasion, then counter-attack. Victory, best first: STRATEGIC — take and hold Tobruk; DECISIVE — take and hold Bardia, \
Fort Maddalena, Sidi Omar, Sollum and Siwa; TACTICAL — take and hold Sollum, Halfaya Pass (C3922) and Siwa (C0127). Every \
objective must be supplied by truck convoy traced back to Alexandria or Cairo.".into(),
        ("campaign", s) => format!("Scenario: the full Campaign Game (§64.7). Automatic wins: the AXIS wins by occupying every \
hex of Alexandria AND Cairo for one full Game-Turn with a supply line (≤90 truck MP) to a dump fed from Tobruk or Tripoli; \
the COMMONWEALTH wins automatically from Game-Turn 35 if no Axis combat unit can trace a 60-MP truck supply line to a dump \
fed from Tobruk or Tripoli. Otherwise victory points: occupation at game end (Axis/CW VP) — Tobruk 200/100, Mersa Matruh \
100/10, Bardia 100/50, Benghazi 75/100, Sidi Barrani 50/10, Derna 25/50, Sollum 25/10, Siwa 20/10, Giarabub 15/10, Jalo \
10/20 — counted only for a combat unit (≥1 TOE) holding a week of Stores and Water plus Fuel/Ammo to fire three times and \
move 20 CP; plus unused Replacement Points. You are the {s:?}."),
        _ => "Scenario: a small skirmish on open ground — a test engagement. Destroy or rout the enemy force while keeping \
your own units intact and cohesive. There are no geographic objectives.".into(),
    }
}

/// The fixed system prompt for a side.
pub fn system_prompt(side: Side) -> String {
    format!(
        "You are the {side:?} theatre commander in SPI's *The Campaign for North Africa* (1979), played on the CNA-Sim \
rules engine. You make operational decisions for your side; the engine enforces every rule and rolls every die.\n\
\n\
# How play works\n\
- Each Game-Turn is one week, split into three Operations Stages. Within a stage each side gets phases \
(Organization, Air, Reserve, Movement & Combat, Truck Convoy, Repair, ...).\n\
- You are consulted once per *decision window*: one phase occurrence in which your side may act. Each window is a fresh \
session — you remember earlier windows only through the JOURNAL and your NOTEBOOK in the prompt. Phases you are not \
consulted on are handled automatically by your staff.\n\
- Within a window you may submit several plans, then you MUST call end_phase. The game waits for you; a window also has \
a time limit.\n\
\n\
# Mechanics that matter most\n\
- Capability Points (CP): every unit has a CPA per stage. Moving and fighting spend CP; exceeding CPA costs cohesion \
(Disorganization). Non-motorized units (CPA ≤ 10) may not voluntarily spend more than 150% of CPA.\n\
- Cohesion: low cohesion weakens units; at −26 or worse a unit cannot move or attack. Rest and success restore it.\n\
- Supply: motorized units need Fuel to move and assault; guns and tanks need Ammo to fire; everyone needs Stores and \
Water. Units out of fuel cannot assault. Keep supply dumps and trucks close behind the front.\n\
- Zones of Control: large stacks project a ZOC into adjacent hexes. Entering one stops movement. HOLDING OFF (§10.3): at \
the end of YOUR Movement & Combat phase, every enemy hex exerting a ZOC on one of your combat units must have been \
attacked (close assault or barrage) that phase, or the unit is forced to retreat three hexes with 3 Disorganization \
Points — or surrenders if it cannot. Do not leave combat units in an enemy ZOC without attacking.\n\
- Combat: Barrage (artillery), Anti-Armor fire (tanks/AT guns vs armor), Close Assault (the decisive melee). An assault \
committing under half your available strength is a Probe (no Engaged/Contact results). Terrain, escarpments, and \
combined arms matter.\n\
- Fog of war: you see enemy unit positions and types, never their strength unless revealed by reconnaissance; enemy \
supply dumps may be dummies.\n\
\n\
# Tools (server `cna`)\n\
get_status · get_observation (filters: near [col,row] + radius, include sections) · list_legal_actions (filter by unit id \
or command kind; ids are valid within the window) · submit_plan (items: {{\"id\": n}} | {{\"command\": <engine JSON>}} | \
{{\"move_to\": {{\"unit\": id, \"hex\": [col,row], \"mode\"?: \"Motorized\"|\"NonMotorized\"}}}}; read the per-item results) \
· get_rule (rules case, e.g. \"10.31\") · read_notes / write_notes (your notebook) · get_journal (full history) · end_phase.\n\
Hexes appear as a rulebook label (e.g. A2109) and a [col,row] coordinate; commands use the coordinate.\n\
\n\
# Memory contract\n\
Before end_phase, call write_notes with your updated NOTEBOOK (it REPLACES the old one; keep it under ~3000 \
characters): your current strategy, each formation's task, what you know and suspect about the enemy, supply concerns, \
and lessons from failed actions. Your next window starts from it. Rewrite and condense — do not just append.\n\
\n\
# Style\n\
Be decisive and economical with tool calls: observe what you need, batch actions into few plans, record notes, end the \
phase. Your final message should be one short line; it is not read by the opponent."
    )
}

/// The per-window prompt: objectives, situation, journal, notebook.
pub fn window_prompt(scenario: &str, side: Side, game: &GameState, memory: &Memory) -> String {
    let st = observe::status(game, side, true);
    let mut p = String::new();
    p.push_str(&format!(
        "# Decision window {n}\n{date} — Game-Turn {turn}, Operations Stage {stage}, phase {phase}. Weather: {weather}. \
Your window is OPEN.\n\n# Objectives\n{obj}\n\n",
        n = memory.windows + 1,
        date = st["date"].as_str().unwrap_or(""),
        turn = game.game_turn,
        stage = game.operations_stage,
        phase = st["phase"].as_str().unwrap_or(""),
        weather = st["weather"].as_str().unwrap_or(""),
        obj = objectives(scenario, side),
    ));
    p.push_str("# Journal (written by the harness; most recent last)\n");
    p.push_str(&memory.journal_digest(4));
    p.push_str("\n# Your notebook (from your last write_notes)\n");
    if memory.notes.trim().is_empty() {
        p.push_str("(empty — this is your first window. Form a plan and record it with write_notes before end_phase.)\n");
    } else {
        p.push_str(&memory.notes);
        p.push('\n');
    }
    p.push_str("\nAct now: observe, decide, submit_plan, write_notes, end_phase.");
    p
}
