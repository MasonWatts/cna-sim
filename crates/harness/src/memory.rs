//! A side's memory across decision windows — what lets a fresh model session each window still play
//! a coherent game.
//!
//! - **Notebook**: free text the model writes with `write_notes` (replace semantics, size-capped).
//! - **Journal**: written by the harness. Per window: what the side could newly *observe* since its
//!   previous window (a diff of its own fog-of-war view — own losses and position changes, enemy
//!   contacts appearing, moving or vanishing) and the actions it took with their results.
//!
//! Everything recorded comes from the side's own observation or its own commands, so the journal
//! cannot leak hidden enemy information.

use std::collections::BTreeMap;

use cna_engine::reference::units::Side;
use cna_engine::state::game::GameState;

use crate::observe::hex_label;

pub const NOTES_MAX_CHARS: usize = 6000;
/// The doctrine (pre-game strategy + mechanics assessment) may be longer and changes rarely.
pub const DOCTRINE_MAX_CHARS: usize = 15000;
const MAX_OBSERVED_LINES: usize = 40;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct OwnSeen {
    name: String,
    hex: Option<String>,
    toe: u32,
    cohesion: i32,
    out_of_water: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct EnemySeen {
    name: String,
    desc: String,
    hex: Option<String>,
}

/// The side's view at a moment (only what it may know).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    own: BTreeMap<u32, OwnSeen>,
    enemy: BTreeMap<u32, EnemySeen>,
}

impl Snapshot {
    pub fn take(game: &GameState, side: Side) -> Self {
        let mut s = Snapshot::default();
        for v in game.observe_units(side) {
            if v.friendly {
                let Some(u) = game.units.get(&v.id) else { continue };
                if u.total_toe() == 0 && game.effective_hex(v.id).is_none() {
                    continue; // not (or no longer) in play
                }
                s.own.insert(
                    v.id.0,
                    OwnSeen {
                        name: u.name.clone(),
                        hex: game.effective_hex(v.id).map(hex_label),
                        toe: u.total_toe(),
                        cohesion: u.cohesion,
                        out_of_water: u.stages_without_water > 0,
                    },
                );
            } else if let Some(h) = v.hex {
                s.enemy.insert(
                    v.id.0,
                    EnemySeen {
                        name: v.name.clone(),
                        desc: format!("{:?} {:?}", v.nationality, v.org_size),
                        hex: Some(hex_label(h)),
                    },
                );
            }
        }
        s
    }

    /// Human-readable changes from `prev` to `self`.
    fn diff(&self, prev: &Snapshot) -> Vec<String> {
        // (priority, line): 0 = losses & water, 1 = new contacts, 2 = strength/cohesion changes,
        // 3 = contacts lost, 4 = position changes. Most important first when capped.
        let mut out: Vec<(u8, String)> = Vec::new();
        let hex = |h: &Option<String>| h.clone().unwrap_or_else(|| "off-map".into());
        for (id, p) in &prev.own {
            match self.own.get(id) {
                None => out.push((0, format!("LOST: your {} (last at {}, {} TOE) is no longer in play.", p.name, hex(&p.hex), p.toe))),
                Some(n) => {
                    let mut parts = Vec::new();
                    if n.toe != p.toe {
                        parts.push(format!("TOE {}→{}", p.toe, n.toe));
                    }
                    if n.cohesion != p.cohesion {
                        parts.push(format!("cohesion {}→{}", p.cohesion, n.cohesion));
                    }
                    if n.hex != p.hex {
                        parts.push(format!("now at {} (was {})", hex(&n.hex), hex(&p.hex)));
                    }
                    if n.out_of_water && !p.out_of_water {
                        parts.push("OUT OF WATER (§52.5)".into());
                    } else if p.out_of_water && !n.out_of_water {
                        parts.push("water restored".into());
                    }
                    if !parts.is_empty() {
                        let pri = if n.out_of_water != p.out_of_water || n.toe < p.toe {
                            0
                        } else if n.toe != p.toe || n.cohesion != p.cohesion {
                            2
                        } else {
                            4
                        };
                        out.push((pri, format!("Your {}: {}.", n.name, parts.join(", "))));
                    }
                }
            }
        }
        for (id, n) in &self.own {
            if !prev.own.contains_key(id) {
                out.push((2, format!("NEW: your {} has entered play at {} ({} TOE).", n.name, hex(&n.hex), n.toe)));
            }
        }
        for (id, n) in &self.enemy {
            match prev.enemy.get(id) {
                None => out.push((1, format!("NEW CONTACT: {} ({}) at {}.", n.name, n.desc, hex(&n.hex)))),
                Some(p) if p.hex != n.hex => out.push((4, format!("Enemy {} moved {} → {}.", n.name, hex(&p.hex), hex(&n.hex)))),
                _ => {}
            }
        }
        for (id, p) in &prev.enemy {
            if !self.enemy.contains_key(id) {
                out.push((3, format!("CONTACT LOST: {} (last seen at {}).", p.name, hex(&p.hex))));
            }
        }
        out.sort_by_key(|(p, _)| *p); // stable: keeps unit order within a priority
        let mut lines: Vec<String> = out.into_iter().map(|(_, l)| l).collect();
        if lines.len() > MAX_OBSERVED_LINES {
            let extra = lines.len() - MAX_OBSERVED_LINES;
            lines.truncate(MAX_OBSERVED_LINES);
            lines.push(format!("(+{extra} less important changes — use get_overview / get_observation)"));
        }
        lines
    }

    pub fn enemy_count(&self) -> usize {
        self.enemy.len()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Block {
    header: String,
    observed: Vec<String>,
    /// The player's own stated rationale for this window's plans (and its end-of-window summary).
    #[serde(default)]
    intent: Vec<String>,
    actions: Vec<String>,
    ok: u32,
    failed: u32,
}

impl Block {
    fn render(&self, current: bool) -> String {
        let mut s = format!("## {}{}\n", self.header, if current { " — THIS WINDOW" } else { "" });
        if !self.observed.is_empty() {
            s.push_str("Observed since your previous window:\n");
            for l in &self.observed {
                s.push_str(&format!("- {l}\n"));
            }
        }
        if !current && !self.intent.is_empty() {
            s.push_str("Your rationale:\n");
            for l in &self.intent {
                s.push_str(&format!("- {l}\n"));
            }
        }
        if !current {
            if self.actions.is_empty() {
                s.push_str("Actions: none.\n");
            } else {
                s.push_str(&format!("Actions ({} ok, {} failed):\n", self.ok, self.failed));
                for l in &self.actions {
                    s.push_str(&format!("- {l}\n"));
                }
            }
        }
        s
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Memory {
    pub notes: String,
    /// The side's own doctrine: mechanics assessment + campaign strategy, written in the pre-game
    /// planning window (and refinable later). Shown in every window prompt.
    #[serde(default)]
    pub doctrine: String,
    /// The pre-game planning window has run.
    #[serde(default)]
    pub pregame_done: bool,
    /// Windows completed.
    pub windows: u32,
    blocks: Vec<Block>,
    last: Option<Snapshot>,
    /// A window is in progress (its block is "THIS WINDOW").
    open: bool,
}

impl Memory {
    /// Start a window: record what changed in the side's view since its previous window.
    pub fn begin_window(&mut self, game: &GameState, side: Side) {
        let now = Snapshot::take(game, side);
        let observed = match &self.last {
            Some(prev) => now.diff(prev),
            None => vec![format!(
                "Opening situation: {} of your units in play; {} enemy units visible.",
                now.own.len(),
                now.enemy_count()
            )],
        };
        self.open = true;
        self.blocks.push(Block {
            header: format!(
                "Window {} — Game-Turn {}, Stage {}, {:?}",
                self.windows + 1,
                game.game_turn,
                game.operations_stage,
                game.phase
            ),
            observed,
            intent: Vec::new(),
            actions: Vec::new(),
            ok: 0,
            failed: 0,
        });
    }

    /// Record the player's stated rationale (a plan's, or the window's closing summary).
    pub fn record_intent(&mut self, text: &str) {
        let t = text.trim();
        if t.is_empty() {
            return;
        }
        if let Some(b) = self.blocks.last_mut() {
            b.intent.push(t.chars().take(600).collect());
        }
    }

    /// Record one action result in the current window.
    pub fn record_action(&mut self, ok: bool, line: String) {
        if let Some(b) = self.blocks.last_mut() {
            if ok {
                b.ok += 1;
            } else {
                b.failed += 1;
            }
            b.actions.push(format!("{} {line}", if ok { "OK" } else { "FAILED" }));
        }
    }

    /// Close the current window, remembering the side's view at its end.
    pub fn end_window(&mut self, game: &GameState, side: Side) {
        self.windows += 1;
        self.open = false;
        self.last = Some(Snapshot::take(game, side));
    }

    /// Replace the doctrine; returns whether it had to be truncated.
    pub fn write_doctrine(&mut self, text: &str) -> bool {
        let truncated = text.chars().count() > DOCTRINE_MAX_CHARS;
        self.doctrine = text.chars().take(DOCTRINE_MAX_CHARS).collect();
        truncated
    }

    /// Replace the notebook; returns whether it had to be truncated.
    pub fn write_notes(&mut self, text: &str) -> bool {
        let truncated = text.chars().count() > NOTES_MAX_CHARS;
        self.notes = text.chars().take(NOTES_MAX_CHARS).collect();
        truncated
    }

    /// Older windows as one-line ledger entries, the last `full` (incl. the current) in detail.
    pub fn journal_digest(&self, full: usize) -> String {
        if self.blocks.is_empty() {
            return "(no earlier windows)\n".into();
        }
        let n = self.blocks.len();
        let split = n.saturating_sub(full);
        let mut s = String::new();
        for b in &self.blocks[..split] {
            s.push_str(&format!("- {}: {} ok, {} failed actions\n", b.header, b.ok, b.failed));
        }
        for (i, b) in self.blocks[split..].iter().enumerate() {
            s.push_str(&b.render(self.open && split + i + 1 == n));
        }
        s
    }

    /// Persist to / restore from JSON (checkpointing across harness restarts).
    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        std::fs::write(path, serde_json::to_string(self)?)
    }
    pub fn load(path: &std::path::Path) -> Option<Memory> {
        serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
    }

    /// The whole journal (for `get_journal`), newest last.
    pub fn journal_full(&self, last: Option<usize>) -> String {
        let n = self.blocks.len();
        let from = last.map_or(0, |k| n.saturating_sub(k));
        self.blocks[from..].iter().enumerate().map(|(i, b)| b.render(self.open && from + i + 1 == n)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn journal_diffs_the_sides_own_view() {
        let (mut g, _) = cna_replay::scenarios::build("skirmish", 1);
        let mut m = Memory::default();
        m.begin_window(&g, Side::Commonwealth);
        assert!(m.journal_digest(4).contains("Opening situation"));
        m.record_action(true, "moved 1st RTR".into());
        m.record_action(false, "assault — out of range".into());
        m.end_window(&g, Side::Commonwealth);

        // Between windows: a Commonwealth unit loses strength, an enemy unit moves.
        use cna_engine::state::unit::Composition;
        let own = g
            .units
            .values()
            .find(|u| u.nationality.side() == Side::Commonwealth && matches!(u.composition, Composition::Organic { .. }))
            .unwrap()
            .id;
        let enemy = g.units.values().find(|u| u.nationality.side() == Side::Axis).unwrap().id;
        if let Composition::Organic { current_toe } = &mut g.units.get_mut(&own).unwrap().composition {
            *current_toe -= 2; // 8 → 6
        }
        let eh = g.units[&enemy].hex().unwrap();
        g.units.get_mut(&enemy).unwrap().location =
            Some(cna_engine::state::unit::UnitLocation::OnMap(cna_engine::state::map::HexCoord::new(eh.col - 1, eh.row)));

        m.begin_window(&g, Side::Commonwealth);
        let d = m.journal_digest(4);
        assert!(d.contains("Actions (1 ok, 1 failed)"), "{d}");
        assert!(d.contains("TOE 8→6"), "own loss journaled: {d}");
        assert!(d.contains("moved"), "enemy movement journaled: {d}");
        // Fog of war: the journal never states enemy strength.
        assert!(!d.contains("TOE") || d.lines().filter(|l| l.contains("Enemy")).all(|l| !l.contains("TOE")));
        assert!(m.write_notes(&"x".repeat(NOTES_MAX_CHARS + 10)));
        assert_eq!(m.notes.chars().count(), NOTES_MAX_CHARS);
    }
}
