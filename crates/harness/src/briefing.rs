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

/// The fixed system prompt for a side. Deliberately lean: the role, how decision windows and memory
/// work, the tools, and the few *engine-specific* facts the rulebook can't tell you. Strategy and the
/// reading of the rules are left to the player (its pre-game doctrine). `coaching` re-adds the
/// harness's own summary of key mechanics (for A/B comparison).
pub fn system_prompt(side: Side, coaching: bool) -> String {
    let mut p = format!(
        "You are the {side:?} theatre commander in SPI's *The Campaign for North Africa* (1979), played on the CNA-Sim \
rules engine. You make the decisions for your side; the engine enforces every rule and rolls every die. The \
rulebook is the authority on how the game works — read it with rules_toc / read_rules / search_rules whenever you \
need to. How to play well is up to you.\n\
\n\
# How play works\n\
- Each Game-Turn is one week, split into three Operations Stages, each with a sequence of phases for each side.\n\
- You are consulted once per *decision window*: every phase occurrence in which your side has something it may do. \
You play every phase yourself. Each window is a fresh session — you carry knowledge forward only through your \
DOCTRINE, your NOTEBOOK and the harness JOURNAL shown in the prompt.\n\
- Before the game there is one PRE-GAME PLANNING window (no game actions) to study the rules and the situation and \
write your doctrine.\n\
- Within a window you may submit several plans; finish with end_phase. Windows have a time limit.\n\
\n\
# Engine notes (implementation facts not stated in the rulebook)\n\
- Fog of war: you see enemy positions and unit types, never their strength unless reconnaissance revealed it; enemy \
dumps may be dummies.\n\
- Supply is drawn only from sources in a unit's own hex: onboard water (refilled at a well/oasis/major city in the \
hex), stores from a friendly dump in the hex, RefuelUnit/RearmUnit from a dump or the unit's own trucks in the hex.\n\
- Each own unit in get_observation (detail) lists `blockers`: the rule currently preventing it from acting.\n\
- The engine applies all automatic rules itself (e.g. forced retreats, attrition, surrender) at the right moment.\n\
\n\
# Tools (server `cna`)\n\
Rules: rules_toc · read_rules (chapter \"15\", section \"15.2\" or case \"15.29\"; paginate with offset) · search_rules · \
get_rule.\n\
Board: get_overview (summary: stacks, formations, supply problems, enemy contacts, dumps) · get_status · \
get_observation (compact; filters near [col,row] + radius, units [ids], detail) · list_legal_actions (filter by unit or \
kind; ids valid within the window).\n\
Act: submit_plan — items {{\"id\": n}} | {{\"command\": <engine JSON>}} | {{\"move_to\": {{\"unit\", \"hex\": [col,row], \"mode\"?}}}} | \
{{\"move_stack\": {{\"from\", \"to\"}}}} | {{\"resupply_hex\": [col,row]}} | {{\"assault\": {{\"target\", \"units\"?}}}} — then read the \
per-item results · end_phase.\n\
Memory: write_doctrine / write_notes / read_notes / get_journal.\n\
Hexes appear as a rulebook label (e.g. A2109) and a [col,row] coordinate; commands use the coordinate.\n\
\n\
# Memory\n\
- DOCTRINE (write_doctrine, ≤15,000 chars): your own assessment of the mechanics that matter and your campaign \
strategy. Written pre-game; revise it when you learn something important.\n\
- NOTEBOOK (write_notes, ≤6,000 chars, replaces the previous one): the current situation, each formation's task, \
intelligence, supply plans, lessons. Update it before end_phase each window.\n\
\n\
Your final message in a window should be one short line; it is not read by the opponent."
    );
    if coaching {
        p.push_str(COACHING);
    }
    p
}

/// The harness's former mechanics summary, appended only with `--coaching`.
const COACHING: &str = "\n\n# Harness coaching (optional)\n\
- Capability Points: moving and fighting spend CP; exceeding CPA costs cohesion. Non-motorized units (CPA ≤ 10) may not \
voluntarily spend more than 150% of CPA.\n\
- Cohesion: at −26 or worse a unit cannot move or attack.\n\
- OUT OF WATER: vehicles cannot move or assault; infantry cannot exceed CPA or assault and lose strength each stage.\n\
- Holding Off (§10.3): at the end of your Movement & Combat phase every enemy hex exerting a ZOC on one of your combat \
units must have been attacked, or the unit retreats three hexes with 3 Disorganization Points.\n\
- A close assault committing under half your available strength is a Probe.";

/// The pre-game planning window prompt.
pub fn pregame_prompt(scenario: &str, side: Side, game: &GameState, budget_note: &str) -> String {
    let st = observe::status(game, side, true);
    format!(
        "# PRE-GAME PLANNING WINDOW\n\
The game has not started. It will open {date} (Game-Turn {turn}). This window is for preparation only: game actions \
are refused (submit_plan is disabled). {budget_note}\n\
\n\
# Objectives\n{obj}\n\
\n\
# Your task\n\
1. Study the rulebook as deeply as you judge useful (rules_toc, read_rules, search_rules): the sequence of play, how \
your forces move, fight and are supplied, and the victory conditions. Note anything that will shape your play.\n\
2. Study the initial situation (get_overview, then get_observation / list_legal_actions where useful).\n\
3. Write your DOCTRINE with write_doctrine: (a) your assessment of the mechanics that matter most and how to use or \
avoid them, (b) your campaign strategy, (c) your plan for the opening turns by formation/sector, (d) your logistics \
plan. You will see it at the start of every later window.\n\
4. Write an initial NOTEBOOK with write_notes, then call end_phase.",
        date = st["date"].as_str().unwrap_or(""),
        turn = game.game_turn,
        obj = objectives(scenario, side),
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
    p.push_str("# Your doctrine (pre-game strategy & mechanics assessment)\n");
    if memory.doctrine.trim().is_empty() {
        p.push_str("(none written)\n");
    } else {
        p.push_str(&memory.doctrine);
        p.push('\n');
    }
    p.push_str("\n# Journal (written by the harness; most recent last)\n");
    p.push_str(&memory.journal_digest(4));
    p.push_str("\n# Your notebook (from your last write_notes)\n");
    if memory.notes.trim().is_empty() {
        p.push_str("(empty — this is your first window. Form a plan and record it with write_notes before end_phase.)\n");
    } else {
        p.push_str(&memory.notes);
        p.push('\n');
    }
    p.push_str("\nYour window is open: decide what to do this phase, act, update your notebook, end_phase.");
    p
}
