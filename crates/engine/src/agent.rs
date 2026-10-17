//! Programmatic players (Phase 2 kickoff).
//!
//! An [`Agent`] observes a [`GameState`] and issues legal [`Command`]s for the side it plays. It
//! chooses from the engine's own legal-action enumerator (`legal_commands`), so an agent can only
//! ever attempt legal moves — the engine remains the single source of truth for legality.
//!
//! Two reference agents are provided: [`RandomAgent`] (uniform over the legal action set) and
//! [`GreedyAgent`] (attack if able, else advance toward the nearest enemy). Both are deterministic
//! from a seed, and their own PRNG is separate from the game's RNG so agent choices do not perturb
//! game determinism. [`self_play`] wires two agents against each other through the Sequence of Play.

use crate::reference::units::Side;
use crate::state::command::{Command, CommandError, CommandOutcome};
use crate::state::game::{GameState, Phase};
use crate::state::view::EvalWeights;

/// The phases in which agents may act (the Sequence-of-Play decision points).
pub fn is_decision_phase(phase: Phase) -> bool {
    matches!(
        phase,
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

/// A programmatic player. `choose` returns the next command the agent wants to issue for `side` in
/// the current state, or `None` to yield (take no further action in this phase occurrence).
pub trait Agent {
    fn choose(&mut self, game: &GameState, side: Side) -> Option<Command>;

    /// Pick the *index* of the command to issue from a precomputed `legal` snapshot. This lets the
    /// driver compute `legal_commands` once per phase-occurrence (and drain it) instead of once per
    /// action — the O(cap × enumeration) cost that dominates large-army turns. The default recomputes
    /// via [`Agent::choose`] and locates the result in `legal`, so agents that don't override it
    /// (Lookahead, MCTS) keep their exact behaviour; [`RandomAgent`]/[`GreedyAgent`] override it to
    /// score `legal` directly. `None` = yield.
    fn choose_index(&mut self, game: &GameState, side: Side, legal: &[Command]) -> Option<usize> {
        let cmd = self.choose(game, side)?;
        legal.iter().position(|c| *c == cmd)
    }

    /// A **free-form** agent issues arbitrary commands via [`Agent::choose`] (not restricted to the
    /// precomputed `legal_commands` snapshot) — e.g. an external/LLM player submitting parameterized
    /// commands. The driver then calls `choose` → `apply` → [`Agent::on_result`] until `choose`
    /// yields. Default `false`: index-based agents are unchanged.
    fn free_form(&self) -> bool {
        false
    }

    /// Told the engine's verdict on the command a free-form agent just issued.
    fn on_result(&mut self, _cmd: &Command, _result: &Result<CommandOutcome, CommandError>) {}
}

/// A small deterministic PRNG so agents are reproducible from a seed, independent of the game RNG.
struct Lcg(u64);
impl Lcg {
    fn new(seed: u64) -> Self {
        Lcg(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { ((self.next_u64() >> 33) as usize) % n }
    }
}

/// Picks a uniformly random legal Movement & Combat command; yields when none remain.
pub struct RandomAgent {
    rng: Lcg,
}
impl RandomAgent {
    pub fn new(seed: u64) -> Self {
        RandomAgent { rng: Lcg::new(seed) }
    }
}
impl RandomAgent {
    fn pick(&mut self, cmds: &[Command]) -> Option<usize> {
        if cmds.is_empty() { None } else { Some(self.rng.below(cmds.len())) }
    }
}
impl Agent for RandomAgent {
    fn choose(&mut self, game: &GameState, side: Side) -> Option<Command> {
        if !is_decision_phase(game.phase) {
            return None;
        }
        let cmds = game.legal_commands(side);
        self.pick(&cmds).map(|i| cmds[i].clone())
    }
    fn choose_index(&mut self, game: &GameState, _side: Side, legal: &[Command]) -> Option<usize> {
        if !is_decision_phase(game.phase) {
            return None;
        }
        self.pick(legal)
    }
}

/// A simple objective-driven player: assault an adjacent enemy if it can, otherwise advance the
/// piece that can get closest to the nearest enemy. Ties broken by its PRNG for variety.
pub struct GreedyAgent {
    rng: Lcg,
}
impl GreedyAgent {
    pub fn new(seed: u64) -> Self {
        GreedyAgent { rng: Lcg::new(seed) }
    }
}
impl GreedyAgent {
    /// The index into `cmds` of the command the greedy agent wants: close assault, then anti-armor,
    /// then upkeep repairs, then the highest-priority build-up/logistics command, then the move whose
    /// destination is closest to an enemy (random tie-break), else a random legal command.
    fn pick(&mut self, game: &GameState, side: Side, cmds: &[Command]) -> Option<usize> {
        if cmds.is_empty() {
            return None;
        }
        if let Some(i) = cmds.iter().position(|c| matches!(c, Command::CloseAssault { .. })) {
            return Some(i);
        }
        if let Some(i) = cmds.iter().position(|c| matches!(c, Command::AntiArmor { .. })) {
            return Some(i);
        }
        // Basic upkeep: fix broken-down units and recover captured gear (§21/§22/§28.3).
        if let Some(i) = cmds.iter().position(|c| {
            matches!(
                c,
                Command::Repair { .. } | Command::RepairCapturedEquipment { .. } | Command::RepairShip { .. }
            )
        }) {
            return Some(i);
        }
        // Constructive build-up / logistics (see `buildup_priority`): highest priority wins.
        if let Some((i, _)) = cmds
            .iter()
            .enumerate()
            .filter_map(|(i, c)| buildup_priority(c).map(|p| (i, p)))
            .max_by_key(|(_, p)| *p)
        {
            return Some(i);
        }
        // Otherwise advance: the legal move whose destination is closest to an enemy unit.
        let enemy = side.opponent();
        let enemy_hexes: Vec<_> = game
            .units
            .values()
            .filter(|u| u.nationality.side() == enemy && u.total_toe() > 0)
            .filter_map(|u| u.hex())
            .collect();
        let mut best: Option<(u32, usize)> = None;
        let mut ties = 0usize;
        for (i, c) in cmds.iter().enumerate() {
            let Command::MoveStep { to, .. } = c else { continue };
            let d = enemy_hexes.iter().map(|e| e.distance(*to)).min().unwrap_or(u32::MAX);
            match &best {
                Some((bd, _)) if d > *bd => {}
                Some((bd, _)) if d == *bd => {
                    ties += 1;
                    if self.rng.below(ties + 1) == 0 {
                        best = Some((d, i));
                    }
                }
                _ => {
                    best = Some((d, i));
                    ties = 0;
                }
            }
        }
        best.map(|(_, i)| i).or_else(|| Some(self.rng.below(cmds.len())))
    }
}
impl Agent for GreedyAgent {
    fn choose(&mut self, game: &GameState, side: Side) -> Option<Command> {
        if !is_decision_phase(game.phase) {
            return None;
        }
        let cmds = game.legal_commands(side);
        self.pick(game, side, &cmds).map(|i| cmds[i].clone())
    }
    fn choose_index(&mut self, game: &GameState, side: Side, legal: &[Command]) -> Option<usize> {
        if !is_decision_phase(game.phase) {
            return None;
        }
        self.pick(game, side, legal)
    }
}

/// A preference score for the non-combat "build-up / upkeep" commands a [`GreedyAgent`] favours over
/// a random pick, higher first: a mandatory squadron withdrawal, then air-force build-up (deploy an
/// SGSU, base it, assign planes/pilots), then air maintenance (refit/ready/arm/refuel/rearm), then
/// keeping supply and production flowing (convoys, trucks, production). Offensive air (bombing,
/// strafing, raids) is deliberately excluded — it stays a random-fallback choice so the greedy agent
/// does not fling planes at targets it cannot evaluate. `None` for commands outside this set.
fn buildup_priority(cmd: &Command) -> Option<u8> {
    use Command::*;
    Some(match cmd {
        WithdrawSquadron { .. } => 90,
        DeploySgsu { .. } => 80,
        AssignSquadronPlanes { .. } => 78,
        AssignPilot { .. } => 76,
        RefitSquadron { .. } => 60,
        ReadySquadron { .. } => 58,
        ArmSquadron { .. } => 56,
        RefuelUnit { .. } => 54,
        RearmUnit { .. } => 52,
        DeliverConvoy { .. } => 48,
        PlanConvoy { .. } => 46,
        LoadTrucks { .. } => 44,
        AssignTrucks { .. } => 42,
        PlanProduction { .. } => 40,
        ProduceCommonwealthTrucks { .. } | ProduceCommonwealthInfantry { .. } | ProduceAxisTrucks { .. } => 38,
        // Off-map basing is a low-priority rear action (mostly the §34.82C relief valve): only after
        // the agent has nothing more useful to build up or supply.
        BaseSquadronOffMap { .. } => 20,
        _ => return None,
    })
}

/// A one-ply lookahead player: for each legal command it clones the state, applies the command, and
/// scores the result with [`GameState::heuristic_value`], choosing the command that most improves
/// its position. It acts only while it can strictly improve (guaranteeing the per-phase loop
/// terminates) and yields otherwise — so it advances toward the enemy and attacks when profitable.
/// The O(1) `GameState` clone (`Rc`-backed map/reference) makes evaluating the whole action set per
/// decision cheap. Combat outcomes are evaluated on one sampled roll from the probe clone (its own
/// RNG copy), an estimate — averaging rollouts is the natural next step.
pub struct LookaheadAgent {
    rng: Lcg,
    weights: crate::state::view::EvalWeights,
}
impl LookaheadAgent {
    pub fn new(seed: u64) -> Self {
        LookaheadAgent { rng: Lcg::new(seed), weights: crate::state::view::EvalWeights::default() }
    }
    /// A lookahead agent evaluating positions with a custom weight blend (for self-play tuning).
    pub fn with_weights(seed: u64, weights: crate::state::view::EvalWeights) -> Self {
        LookaheadAgent { rng: Lcg::new(seed), weights }
    }
}
impl Agent for LookaheadAgent {
    fn choose(&mut self, game: &GameState, side: Side) -> Option<Command> {
        if !matches!(game.phase, Phase::MovementAndCombat | Phase::Organization | Phase::LandSupportAir | Phase::TruckConvoy | Phase::Repair | Phase::CommonwealthFleet | Phase::NavalConvoyArrival | Phase::NavalConvoySchedule | Phase::AircraftMaintenance) {
            return None;
        }
        let cmds = game.legal_commands(side);
        if cmds.is_empty() {
            return None;
        }
        let base = game.heuristic_value_with(side, &self.weights);
        let mut best: Option<(i64, Command)> = None;
        let mut ties = 0usize;
        for c in &cmds {
            let mut probe = game.clone();
            if probe.apply(c.clone()).is_err() {
                continue;
            }
            let score = probe.heuristic_value_with(side, &self.weights);
            match &best {
                Some((bs, _)) if score < *bs => {}
                Some((bs, _)) if score == *bs => {
                    ties += 1;
                    if self.rng.below(ties + 1) == 0 {
                        best = Some((score, c.clone()));
                    }
                }
                _ => {
                    best = Some((score, c.clone()));
                    ties = 0;
                }
            }
        }
        match best {
            Some((score, c)) if score > base => Some(c),
            _ => None, // nothing strictly improves the position — yield
        }
    }
}

/// A flat Monte-Carlo rollout player. For each legal command it runs `samples` independent
/// rollouts — clone the state, **reseed the clone's RNG** (so combat/breakdown dice differ per
/// sample, unlike a bare clone which would replay identically), apply the command, play up to
/// `depth` further random legal actions for its own side, then score the result with
/// [`GameState::heuristic_value`]. It picks the command with the best *average* score, acting only
/// while that beats the current position (so the per-phase loop terminates). Averaging de-noises the
/// single-sample weakness of [`LookaheadAgent`]: e.g. it won't be spooked out of a good attack by
/// one unlucky roll, nor lured by one lucky one. `depth = 0` is pure immediate-outcome averaging
/// (the cleanest de-noiser); larger `depth` adds shallow lookahead. Cheap thanks to the O(1) clone.
pub struct RolloutAgent {
    rng: Lcg,
    samples: u32,
    depth: u32,
}
impl RolloutAgent {
    pub fn new(seed: u64, samples: u32, depth: u32) -> Self {
        RolloutAgent { rng: Lcg::new(seed), samples: samples.max(1), depth }
    }
    /// A sensible default: 8 samples of the immediate outcome, no deeper lookahead.
    pub fn flat(seed: u64) -> Self {
        Self::new(seed, 8, 0)
    }

    /// Mean `heuristic_value` for `side` after issuing `cmd`, averaged over `samples` reseeded
    /// rollouts (each playing `depth` extra random plies). Returns `None` if the command is illegal.
    fn average_value(&mut self, game: &GameState, side: Side, cmd: &Command) -> Option<f64> {
        let mut total: i64 = 0;
        let mut ok = 0u32;
        for _ in 0..self.samples {
            let mut probe = game.clone();
            probe.reseed_rng(self.rng.next_u64());
            if probe.apply(cmd.clone()).is_err() {
                return None; // came from legal_commands, so this should not happen
            }
            for _ in 0..self.depth {
                let cs = probe.legal_commands(side);
                if cs.is_empty() {
                    break;
                }
                let pick = cs[self.rng.below(cs.len())].clone();
                let _ = probe.apply(pick);
            }
            total += probe.heuristic_value(side);
            ok += 1;
        }
        (ok > 0).then(|| total as f64 / ok as f64)
    }
}
impl Agent for RolloutAgent {
    fn choose(&mut self, game: &GameState, side: Side) -> Option<Command> {
        if !matches!(game.phase, Phase::MovementAndCombat | Phase::Organization | Phase::LandSupportAir | Phase::TruckConvoy | Phase::Repair | Phase::CommonwealthFleet | Phase::NavalConvoyArrival | Phase::NavalConvoySchedule | Phase::AircraftMaintenance) {
            return None;
        }
        let cmds = game.legal_commands(side);
        if cmds.is_empty() {
            return None;
        }
        let base = game.heuristic_value(side) as f64;
        let mut best: Option<(f64, Command)> = None;
        let mut ties = 0usize;
        for c in &cmds {
            let Some(avg) = self.average_value(game, side, c) else { continue };
            match &best {
                Some((bv, _)) if avg < *bv => {}
                Some((bv, _)) if (avg - *bv).abs() < f64::EPSILON => {
                    ties += 1;
                    if self.rng.below(ties + 1) == 0 {
                        best = Some((avg, c.clone()));
                    }
                }
                _ => {
                    best = Some((avg, c.clone()));
                    ties = 0;
                }
            }
        }
        match best {
            Some((v, c)) if v > base => Some(c),
            _ => None,
        }
    }
}

/// A UCT (Monte-Carlo Tree Search) player that plans a whole Movement & Combat phase.
///
/// Within its own Movement & Combat phase the opponent does not act, so planning reduces to a
/// single-agent search over sequences of the side's own commands. Tree edges are legal commands
/// plus a **Pass** (end the turn); a node is terminal when reached via Pass or at the `max_actions`
/// depth cap. Transitions are stochastic (combat/breakdown), so every state produced during
/// selection/expansion/rollout is generated on a **reseeded clone** — the tree therefore samples the
/// game's chance events rather than assuming determinism. Rewards are `heuristic_value(side)` at the
/// rollout leaf, min/max-normalized to `[0,1]` for the UCB1 exploration term (the raw eval is
/// unbounded). `choose` returns the first action of the most-visited root child (the robust choice),
/// or `None` if Pass wins. Each `choose` re-plans from the current true state (open-loop), so it
/// naturally adapts to whatever the stochastic outcomes actually were.
pub struct MctsAgent {
    rng: Lcg,
    iterations: u32,
    rollout_depth: u32,
    max_actions: u32,
    c: f64,
}

struct MctsNode {
    state: GameState,
    action: Option<Command>, // the edge from the parent (None at root, or a Pass child)
    is_pass: bool,           // reached via Pass → terminal
    depth: u32,
    untried: Vec<Option<Command>>, // remaining edges to expand (Some(cmd) / None = Pass)
    children: Vec<usize>,
    parent: Option<usize>,
    visits: u32,
    total: f64,
}

impl MctsAgent {
    pub fn new(seed: u64, iterations: u32, rollout_depth: u32, max_actions: u32) -> Self {
        MctsAgent { rng: Lcg::new(seed), iterations: iterations.max(1), rollout_depth, max_actions: max_actions.max(1), c: std::f64::consts::SQRT_2 }
    }
    /// A sensible default: 100 iterations, depth-6 rollouts, up to 8 actions planned per phase.
    pub fn standard(seed: u64) -> Self {
        Self::new(seed, 100, 6, 8)
    }

    /// The edges (legal commands + Pass) available at a node's state for `side`.
    fn edges(state: &GameState, side: Side, depth: u32, max_actions: u32) -> Vec<Option<Command>> {
        if depth >= max_actions {
            return Vec::new();
        }
        let mut e: Vec<Option<Command>> = state.legal_commands(side).into_iter().map(Some).collect();
        e.push(None); // Pass
        e
    }

    /// A random playout from `state`: apply up to `rollout_depth` random legal commands (reseeded
    /// so combat/breakdown vary), then return `heuristic_value(side)`.
    fn rollout(&mut self, mut state: GameState, side: Side) -> f64 {
        state.reseed_rng(self.rng.next_u64());
        for _ in 0..self.rollout_depth {
            let cs = state.legal_commands(side);
            if cs.is_empty() {
                break;
            }
            let pick = cs[self.rng.below(cs.len())].clone();
            let _ = state.apply(pick);
        }
        state.heuristic_value(side) as f64
    }
}

impl Agent for MctsAgent {
    fn choose(&mut self, game: &GameState, side: Side) -> Option<Command> {
        if !matches!(game.phase, Phase::MovementAndCombat | Phase::Organization | Phase::LandSupportAir | Phase::TruckConvoy | Phase::Repair | Phase::CommonwealthFleet | Phase::NavalConvoyArrival | Phase::NavalConvoySchedule | Phase::AircraftMaintenance) {
            return None;
        }
        let root_edges = Self::edges(game, side, 0, self.max_actions);
        // Only "Pass" available → nothing to do.
        if root_edges.iter().all(|e| e.is_none()) {
            return None;
        }
        let mut arena: Vec<MctsNode> = vec![MctsNode {
            state: game.clone(),
            action: None,
            is_pass: false,
            depth: 0,
            untried: root_edges,
            children: Vec::new(),
            parent: None,
            visits: 0,
            total: 0.0,
        }];
        let (mut min_r, mut max_r) = (f64::INFINITY, f64::NEG_INFINITY);

        for _ in 0..self.iterations {
            // SELECTION — descend fully-expanded, non-terminal nodes by UCB1.
            let mut node = 0usize;
            loop {
                let n = &arena[node];
                let terminal = n.is_pass || n.depth >= self.max_actions;
                if terminal || !n.untried.is_empty() {
                    break;
                }
                // pick best child by UCB1 (normalized exploitation + exploration).
                let parent_visits = n.visits.max(1) as f64;
                let denom = (max_r - min_r).max(1e-9);
                let mut best_child = n.children[0];
                let mut best_ucb = f64::NEG_INFINITY;
                for &ci in &n.children {
                    let c = &arena[ci];
                    let exploit = if c.visits == 0 { 0.5 } else { ((c.total / c.visits as f64) - min_r) / denom };
                    let explore = self.c * (parent_visits.ln() / (c.visits.max(1) as f64)).sqrt();
                    let ucb = exploit + explore;
                    if ucb > best_ucb {
                        best_ucb = ucb;
                        best_child = ci;
                    }
                }
                node = best_child;
            }

            // EXPANSION — if the node has an untried edge and is not terminal, expand one.
            let terminal = arena[node].is_pass || arena[node].depth >= self.max_actions;
            if !terminal && !arena[node].untried.is_empty() {
                // pop a random untried edge
                let k = self.rng.below(arena[node].untried.len());
                let edge = arena[node].untried.swap_remove(k);
                let depth = arena[node].depth + 1;
                let mut child_state = arena[node].state.clone();
                let is_pass = edge.is_none();
                if let Some(cmd) = &edge {
                    child_state.reseed_rng(self.rng.next_u64());
                    let _ = child_state.apply(cmd.clone());
                }
                let untried = if is_pass { Vec::new() } else { Self::edges(&child_state, side, depth, self.max_actions) };
                let child = MctsNode {
                    state: child_state,
                    action: edge,
                    is_pass,
                    depth,
                    untried,
                    children: Vec::new(),
                    parent: Some(node),
                    visits: 0,
                    total: 0.0,
                };
                let ci = arena.len();
                arena.push(child);
                arena[node].children.push(ci);
                node = ci;
            }

            // SIMULATION — random rollout from the (possibly just-expanded) node.
            let leaf_state = arena[node].state.clone();
            let value = self.rollout(leaf_state, side);
            min_r = min_r.min(value);
            max_r = max_r.max(value);

            // BACKPROP — up to the root.
            let mut cur = Some(node);
            while let Some(i) = cur {
                arena[i].visits += 1;
                arena[i].total += value;
                cur = arena[i].parent;
            }
        }

        // Robust child: the most-visited root child; return its action (None = Pass → yield).
        let root = &arena[0];
        root.children
            .iter()
            .max_by_key(|&&ci| arena[ci].visits)
            .and_then(|&ci| arena[ci].action.clone())
    }
}

/// A cross-phase, adversarial UCT player. Unlike [`MctsAgent`] (which plans a single Movement &
/// Combat phase in isolation), this search spans **multiple phases across the Sequence of Play** and
/// models the **opponent's response**. A node is a decision point (a Movement & Combat phase) tagged
/// with whose decision it is. Edges are that side's legal commands plus **Pass**; a command edge
/// keeps the same side issuing orders, while Pass advances the SoP (`advance_to_movement_combat`) to
/// the next decision point — which may be the opponent's. All rewards are stored from the *root*
/// side's perspective; at a node whose decision side is the root, UCB exploits high values, and at an
/// opponent node it exploits *low* root values (minimax via player-aware exploitation). Rollouts play
/// random self-play across phases to the turn horizon, then score `heuristic_value(root)`. Every
/// state is a reseeded clone, so chance is sampled and the true game is untouched. Bounded by a
/// `turn_horizon`, a `max_depth` on tree edges, and `rollout_steps`.
pub struct CrossPhaseMctsAgent {
    rng: Lcg,
    iterations: u32,
    turn_horizon: u32,
    max_depth: u32,
    rollout_steps: u32,
    c: f64,
}

struct XNode {
    state: GameState,
    action: Option<Command>, // the edge from the parent (None at root, or a Pass edge)
    decision_side: Side,
    terminal: bool,
    depth: u32,
    untried: Vec<Option<Command>>,
    children: Vec<usize>,
    parent: Option<usize>,
    visits: u32,
    total: f64, // sum of root-perspective rewards
}

impl CrossPhaseMctsAgent {
    pub fn new(seed: u64, iterations: u32, turn_horizon: u32, max_depth: u32, rollout_steps: u32) -> Self {
        CrossPhaseMctsAgent {
            rng: Lcg::new(seed),
            iterations: iterations.max(1),
            turn_horizon,
            max_depth: max_depth.max(1),
            rollout_steps,
            c: std::f64::consts::SQRT_2,
        }
    }
    /// Default: 120 iterations, look one Game-Turn ahead, up to 10 tree edges, depth-24 rollouts.
    pub fn standard(seed: u64) -> Self {
        Self::new(seed, 120, 1, 10, 24)
    }

    fn edges(state: &GameState, side: Side) -> Vec<Option<Command>> {
        let mut e: Vec<Option<Command>> = state.legal_commands(side).into_iter().map(Some).collect();
        e.push(None); // Pass
        e
    }

    /// Random self-play from `state` across phases to the turn horizon, then the root-side value.
    fn rollout(&mut self, mut state: GameState, root: Side, turn_limit: u32) -> f64 {
        state.reseed_rng(self.rng.next_u64());
        let mut steps = 0;
        while state.game_turn <= turn_limit && steps < self.rollout_steps {
            steps += 1;
            if state.phase != Phase::MovementAndCombat {
                state.advance_phase();
                continue;
            }
            let active = state.active_player;
            let cs = state.legal_commands(active);
            if cs.is_empty() || self.rng.below(4) == 0 {
                state.advance_phase(); // sometimes end the turn to keep play moving
            } else {
                let pick = cs[self.rng.below(cs.len())].clone();
                let _ = state.apply(pick);
            }
        }
        state.heuristic_value(root) as f64
    }
}

impl Agent for CrossPhaseMctsAgent {
    fn choose(&mut self, game: &GameState, side: Side) -> Option<Command> {
        if !matches!(game.phase, Phase::MovementAndCombat | Phase::Organization | Phase::LandSupportAir | Phase::TruckConvoy | Phase::Repair | Phase::CommonwealthFleet | Phase::NavalConvoyArrival | Phase::NavalConvoySchedule | Phase::AircraftMaintenance) {
            return None;
        }
        let root = side;
        let turn_limit = game.game_turn + self.turn_horizon;
        let root_edges = Self::edges(game, root);
        if root_edges.iter().all(|e| e.is_none()) {
            return None;
        }
        let mut arena: Vec<XNode> = vec![XNode {
            state: game.clone(),
            action: None,
            decision_side: root,
            terminal: false,
            depth: 0,
            untried: root_edges,
            children: Vec::new(),
            parent: None,
            visits: 0,
            total: 0.0,
        }];
        let (mut min_r, mut max_r) = (f64::INFINITY, f64::NEG_INFINITY);

        for _ in 0..self.iterations {
            // SELECTION
            let mut node = 0usize;
            loop {
                let n = &arena[node];
                if n.terminal || !n.untried.is_empty() || n.children.is_empty() {
                    break;
                }
                let parent_visits = n.visits.max(1) as f64;
                let denom = (max_r - min_r).max(1e-9);
                let decision_is_root = n.decision_side == root;
                let mut best = n.children[0];
                let mut best_ucb = f64::NEG_INFINITY;
                for &ci in &n.children {
                    let c = &arena[ci];
                    let norm = if c.visits == 0 { 0.5 } else { ((c.total / c.visits as f64) - min_r) / denom };
                    let exploit = if decision_is_root { norm } else { 1.0 - norm };
                    let explore = self.c * (parent_visits.ln() / c.visits.max(1) as f64).sqrt();
                    let ucb = exploit + explore;
                    if ucb > best_ucb {
                        best_ucb = ucb;
                        best = ci;
                    }
                }
                node = best;
            }

            // EXPANSION
            if !arena[node].terminal && !arena[node].untried.is_empty() {
                let k = self.rng.below(arena[node].untried.len());
                let edge = arena[node].untried.swap_remove(k);
                let depth = arena[node].depth + 1;
                let mut state = arena[node].state.clone();
                let (decision_side, mut terminal);
                match &edge {
                    Some(cmd) => {
                        state.reseed_rng(self.rng.next_u64());
                        let _ = state.apply(cmd.clone());
                        decision_side = arena[node].decision_side; // same side keeps acting
                        terminal = false;
                    }
                    None => {
                        let reached = state.advance_to_movement_combat(turn_limit);
                        terminal = !reached;
                        decision_side = state.active_player;
                    }
                }
                if depth >= self.max_depth {
                    terminal = true;
                }
                let untried = if terminal { Vec::new() } else { Self::edges(&state, decision_side) };
                let ci = arena.len();
                arena.push(XNode { state, action: edge, decision_side, terminal, depth, untried, children: Vec::new(), parent: Some(node), visits: 0, total: 0.0 });
                arena[node].children.push(ci);
                node = ci;
            }

            // SIMULATION + BACKPROP (root-perspective value everywhere)
            let leaf = arena[node].state.clone();
            let value = self.rollout(leaf, root, turn_limit);
            min_r = min_r.min(value);
            max_r = max_r.max(value);
            let mut cur = Some(node);
            while let Some(i) = cur {
                arena[i].visits += 1;
                arena[i].total += value;
                cur = arena[i].parent;
            }
        }

        arena[0]
            .children
            .iter()
            .max_by_key(|&&ci| arena[ci].visits)
            .and_then(|&ci| arena[ci].action.clone())
    }
}

/// Run two agents against each other from `game`'s current state through the Sequence of Play until
/// `game.game_turn` exceeds `max_turns`. During each Movement & Combat phase the active side's agent
/// issues commands (each validated by `apply`) until it yields or a per-phase action cap is hit
/// (`3 × its on-map units + 8`, so a random agent cannot act forever); every other phase is driven
/// by `advance_phase`. Returns the number of commands the agents successfully applied.
/// Drive one decision-phase occurrence: compute the legal set once, let the agent drain it (each
/// chosen command is removed so it is not re-picked; a command that has become illegal is skipped by
/// `apply`), and recompute only when the agent yields *after* making progress. A large-army phase
/// therefore costs O(waves × enumeration) rather than O(actions × enumeration) — the recompute-per-
/// action cost that dominates post-DAK turns. Bounded by `cap` attempted actions.
pub fn drain_phase(game: &mut GameState, agent: &mut dyn Agent, side: Side, cap: u32, applied: &mut u32) {
    if agent.free_form() {
        // Free-form agents propose commands directly; the engine validates each one. They get a larger
        // action budget (multi-command plans and macro expansions).
        for _ in 0..cap.saturating_mul(10).max(200) {
            let Some(cmd) = agent.choose(game, side) else { break };
            let result = game.apply_as(side, cmd.clone());
            if result.is_ok() {
                *applied += 1;
            }
            agent.on_result(&cmd, &result);
        }
        return;
    }
    let mut legal = game.legal_commands(side);
    let mut acted = 0u32;
    let mut progress = true;
    while acted < cap {
        match agent.choose_index(game, side, &legal) {
            Some(i) if i < legal.len() => {
                let cmd = legal.swap_remove(i);
                if game.apply_as(side, cmd).is_ok() {
                    *applied += 1;
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

pub fn self_play(
    game: &mut GameState,
    commonwealth: &mut dyn Agent,
    axis: &mut dyn Agent,
    max_turns: u32,
) -> u32 {
    let mut applied = 0u32;
    let mut guard = 0u64;
    let mut checked_stage = None;
    while game.game_turn <= max_turns {
        // Stop early on a §64.71/§64.72 automatic campaign win (no-op for scenarios without campaign
        // victory conditions loaded, where `campaign_auto_winner` is always `None`). §64.72 is judged
        // per Operations Stage, and the check traces every Axis unit's supply line, so it runs once
        // per stage rather than once per phase.
        let stage = (game.game_turn, game.operations_stage);
        if checked_stage != Some(stage) {
            checked_stage = Some(stage);
            if game.campaign_auto_winner().is_some() {
                break;
            }
        }
        if is_decision_phase(game.phase) {
            let side = game.active_player;
            let on_map = game.units.values().filter(|u| u.nationality.side() == side && u.hex().is_some()).count();
            let cap = 3 * on_map as u32 + 8;
            drain_phase(game, if side == Side::Commonwealth { commonwealth } else { axis }, side, cap, &mut applied);
        }
        game.advance_phase();
        guard += 1;
        assert!(guard < 5_000_000, "self-play loop failed to progress");
    }
    applied
}

/// A fixed, tuning-independent score of the position from `side`'s view — Victory Points dominate
/// (the actual win condition), with material as the tie-break. Because it does not depend on the
/// weights being searched, it gives `tune_weights` a stable objective to hill-climb against.
pub fn outcome_score(game: &GameState, side: Side) -> i64 {
    game.heuristic_value_with(
        side,
        &EvalWeights { material: 1, vp: 200, cohesion: 0, supply_penalty: 0, closeness: 0 },
    )
}

/// Play one self-play game built by `make_game` with `LookaheadAgent`s — `weights_a` for side `a`,
/// `weights_b` for its opponent — for `turns` Game-Turns, and return `outcome_score` for side `a`.
fn play_scored(
    make_game: &impl Fn() -> GameState,
    a: Side,
    weights_a: EvalWeights,
    weights_b: EvalWeights,
    seed: u64,
    turns: u32,
) -> i64 {
    let mut game = make_game();
    let mut agent_a = LookaheadAgent::with_weights(seed, weights_a);
    let mut agent_b = LookaheadAgent::with_weights(seed ^ 0xABCD, weights_b);
    let (cw, ax): (&mut dyn Agent, &mut dyn Agent) = if a == Side::Commonwealth {
        (&mut agent_a, &mut agent_b)
    } else {
        (&mut agent_b, &mut agent_a)
    };
    self_play(&mut game, cw, ax, turns);
    outcome_score(&game, a)
}

/// Evaluate `weights` by pitting a `LookaheadAgent` using them against one using `opponent` weights,
/// averaged over `seeds` seeds and both side assignments (to cancel side/first-move bias). Higher is
/// better for `weights`. The objective for [`tune_weights`]; deterministic for fixed inputs.
pub fn evaluate_weights(
    make_game: &impl Fn() -> GameState,
    weights: EvalWeights,
    opponent: EvalWeights,
    seeds: u32,
    turns: u32,
) -> f64 {
    let mut total = 0i64;
    let mut n = 0i64;
    for s in 0..seeds.max(1) {
        for &side in &[Side::Commonwealth, Side::Axis] {
            total += play_scored(make_game, side, weights, opponent, 1000 + s as u64, turns);
            n += 1;
        }
    }
    total as f64 / n as f64
}

/// **Co-evolutionary** steepest-ascent tuning of the eval weights via self-play. Starting from
/// [`EvalWeights::default`], each round holds the current champion fixed as the opponent, evaluates
/// every ±`step` neighbour against it, and promotes the neighbour that beats the champion by the
/// largest average margin (steepest ascent). The search stops when **no** neighbour beats the
/// champion — i.e. the champion is a self-play local optimum, a stronger convergence test than the
/// old "beat the fixed default" (which capped the agent at exploiting the default's weaknesses).
/// `iters` bounds the rounds; `seeds` sets the games per evaluation (higher = less noise, the main
/// lever for the objective to have signal). Deterministic for fixed inputs.
pub fn tune_weights(
    make_game: &impl Fn() -> GameState,
    iters: u32,
    seeds: u32,
    turns: u32,
    step: i64,
) -> EvalWeights {
    let mut best = EvalWeights::default();
    for _ in 0..iters {
        // The champion is this round's fixed opponent. A neighbour is stronger only if it beats the
        // champion by more than the champion beats *itself* (~0, but measured for consistency), so
        // that is the acceptance bar — filtering neutral/noise candidates and never regressing.
        let champion = best;
        let mut best_gain = evaluate_weights(make_game, champion, champion, seeds, turns);
        let mut round_best = best;
        for field in 0..5 {
            for &delta in &[step, -step] {
                let mut cand = best;
                match field {
                    0 => cand.material += delta,
                    1 => cand.vp += delta,
                    2 => cand.cohesion += delta,
                    3 => cand.supply_penalty += delta,
                    _ => cand.closeness += delta,
                }
                // Keep weights sane: material strictly positive, the rest non-negative.
                if cand.material <= 0 || cand.vp < 0 || cand.cohesion < 0 || cand.supply_penalty < 0 || cand.closeness < 0 {
                    continue;
                }
                let gain = evaluate_weights(make_game, cand, champion, seeds, turns);
                if gain > best_gain {
                    best_gain = gain;
                    round_best = cand;
                }
            }
        }
        if round_best == best {
            break; // no neighbour beat the champion → self-play local optimum
        }
        best = round_best;
    }
    best
}

// ---------------------------------------------------------------------------------------------
// Tournament harness — agent strength evaluation.
//
// A round-robin plays every distinct pair of agents over a set of seeds and *both* side
// assignments (to cancel the side / first-initiative bias), scoring each game by the final
// `outcome_score` margin. It answers the questions the Phase-2 toolkit could not on its own:
// does MCTS actually beat Greedy, do tuned weights beat the default, is an agent worth its cost?
// ---------------------------------------------------------------------------------------------

/// A named participant: a label plus a factory that builds a **fresh** agent for each game, seeded
/// per game so a tournament is deterministic yet varied across seeds.
pub struct Contender {
    pub name: &'static str,
    pub make: Box<dyn Fn(u64) -> Box<dyn Agent>>,
}

impl Contender {
    pub fn new(name: &'static str, make: impl Fn(u64) -> Box<dyn Agent> + 'static) -> Self {
        Contender { name, make: Box::new(make) }
    }
}

/// A contender's aggregate record across a tournament.
#[derive(Clone, Debug, Default)]
pub struct Standing {
    pub name: String,
    pub games: u32,
    pub wins: u32,
    pub draws: u32,
    pub losses: u32,
    /// Sum over all this agent's games of (its `outcome_score` − the opponent's).
    pub margin_sum: i64,
}

impl Standing {
    /// Match points: 1 per win, ½ per draw.
    pub fn points(&self) -> f64 {
        self.wins as f64 + 0.5 * self.draws as f64
    }
    /// Win rate in `[0,1]`, counting draws as ½ (0 if it played no games).
    pub fn win_rate(&self) -> f64 {
        if self.games == 0 { 0.0 } else { self.points() / self.games as f64 }
    }
    /// Average per-game score margin (this agent − opponent).
    pub fn avg_margin(&self) -> f64 {
        if self.games == 0 { 0.0 } else { self.margin_sum as f64 / self.games as f64 }
    }
    /// Approximate 95% confidence interval for the true win rate (in `[0,1]`), from the normal
    /// approximation to the mean of the per-game score (win = 1, draw = ½, loss = 0). Wide when
    /// `games` is small — a reminder that a few games can't separate close agents.
    pub fn win_rate_ci95(&self) -> (f64, f64) {
        let n = self.games as f64;
        if n < 2.0 {
            return (0.0, 1.0);
        }
        let mean = self.win_rate();
        // Sample variance of the {1, ½, 0} outcome vector (Bessel-corrected).
        let (w, d, l) = (self.wins as f64, self.draws as f64, self.losses as f64);
        let ss = w * (1.0 - mean).powi(2) + d * (0.5 - mean).powi(2) + l * mean.powi(2);
        let se = (ss / (n - 1.0) / n).sqrt();
        let half = 1.96 * se;
        ((mean - half).max(0.0), (mean + half).min(1.0))
    }
}

/// Play one game between `a` (assigned `a_side`) and `b` (the other side) on a fresh board from
/// `make_game`, running `turns` Game-Turns. Returns `(a_score, b_score)` — each side's
/// [`outcome_score`] at the end. Deterministic in `seed`.
pub fn play_match(
    make_game: &impl Fn() -> GameState,
    a: &Contender,
    a_side: Side,
    b: &Contender,
    seed: u64,
    turns: u32,
) -> (i64, i64) {
    let b_side = match a_side {
        Side::Commonwealth => Side::Axis,
        Side::Axis => Side::Commonwealth,
    };
    let mut game = make_game();
    let mut agent_a = (a.make)(seed);
    let mut agent_b = (b.make)(seed ^ 0x9E37_79B9_7F4A_7C15);
    let (cw, ax): (&mut dyn Agent, &mut dyn Agent) = if a_side == Side::Commonwealth {
        (agent_a.as_mut(), agent_b.as_mut())
    } else {
        (agent_b.as_mut(), agent_a.as_mut())
    };
    self_play(&mut game, cw, ax, turns);
    (outcome_score(&game, a_side), outcome_score(&game, b_side))
}

/// Round-robin tournament: every distinct pair plays `seeds` seeds × both side assignments, each
/// game running `turns` Game-Turns. A game is a win for the side whose `outcome_score` leads by
/// more than `draw_margin`, else a draw. Returns standings sorted best-first (points, then average
/// margin). Each contender plays `(n-1) * seeds * 2` games. Reproducible **within a process**;
/// results vary slightly across processes because combat resolution iterates units in `HashMap`
/// order (randomized per process) — full cross-process determinism is a separate follow-up.
pub fn round_robin(
    make_game: &impl Fn() -> GameState,
    contenders: &[Contender],
    seeds: u32,
    turns: u32,
    draw_margin: i64,
) -> Tournament {
    let n = contenders.len();
    let mut st: Vec<Standing> = contenders
        .iter()
        .map(|c| Standing { name: c.name.to_string(), ..Default::default() })
        .collect();
    let mut grid = vec![vec![HeadToHead::default(); n]; n];
    for i in 0..n {
        for j in (i + 1)..n {
            for s in 0..seeds.max(1) {
                for &i_side in &[Side::Commonwealth, Side::Axis] {
                    let seed = 0x5EED_0000 ^ ((i as u64) << 40) ^ ((j as u64) << 24) ^ (s as u64);
                    let (si, sj) = play_match(make_game, &contenders[i], i_side, &contenders[j], seed, turns);
                    let diff = si - sj;
                    st[i].games += 1;
                    st[j].games += 1;
                    st[i].margin_sum += diff;
                    st[j].margin_sum -= diff;
                    if diff > draw_margin {
                        st[i].wins += 1;
                        st[j].losses += 1;
                        grid[i][j].wins += 1;
                        grid[j][i].losses += 1;
                    } else if diff < -draw_margin {
                        st[j].wins += 1;
                        st[i].losses += 1;
                        grid[j][i].wins += 1;
                        grid[i][j].losses += 1;
                    } else {
                        st[i].draws += 1;
                        st[j].draws += 1;
                        grid[i][j].draws += 1;
                        grid[j][i].draws += 1;
                    }
                }
            }
        }
    }
    let names: Vec<String> = contenders.iter().map(|c| c.name.to_string()).collect();
    st.sort_by(|a, b| {
        b.points()
            .partial_cmp(&a.points())
            .unwrap()
            .then(b.avg_margin().partial_cmp(&a.avg_margin()).unwrap())
    });
    Tournament { standings: st, names, grid }
}

/// One contender's record against one specific opponent (a cell of the head-to-head grid).
#[derive(Clone, Copy, Debug, Default)]
pub struct HeadToHead {
    pub wins: u32,
    pub draws: u32,
    pub losses: u32,
}

/// A completed round-robin: overall standings (best-first) plus the full head-to-head grid, which
/// exposes *who beats whom* — including non-transitive cycles the aggregate win-rate hides.
pub struct Tournament {
    /// Overall standings, sorted best-first.
    pub standings: Vec<Standing>,
    /// Contender names by their **original** (input) index — the row/column order of `grid`.
    pub names: Vec<String>,
    /// `grid[i][j]` = contender `i`'s record versus contender `j` (both by original index; the
    /// diagonal `grid[i][i]` is all-zero).
    pub grid: Vec<Vec<HeadToHead>>,
}

/// Render the head-to-head grid as a matrix of `W–D–L` cells (row = that agent's record vs the
/// column agent), rows/columns in standings order (best-first).
pub fn format_head_to_head(t: &Tournament) -> String {
    use std::fmt::Write;
    // Present in standings order rather than input order, so the strongest agents read top-left.
    let order: Vec<usize> = t
        .standings
        .iter()
        .map(|s| t.names.iter().position(|n| n == &s.name).unwrap())
        .collect();
    let short = |n: &str| n.chars().take(8).collect::<String>();
    let mut out = String::new();
    write!(out, "{:<12}", "vs →").unwrap();
    for &j in &order {
        write!(out, "{:>10}", short(&t.names[j])).unwrap();
    }
    out.push('\n');
    for &i in &order {
        write!(out, "{:<12}", short(&t.names[i])).unwrap();
        for &j in &order {
            if i == j {
                write!(out, "{:>10}", "—").unwrap();
            } else {
                let h = t.grid[i][j];
                write!(out, "{:>10}", format!("{}-{}-{}", h.wins, h.draws, h.losses)).unwrap();
            }
        }
        out.push('\n');
    }
    out
}

/// Render standings as a fixed-width table, best-first, with a 95% win-rate confidence interval per
/// agent and a note when the top two agents' intervals overlap (their ranking is not statistically
/// separable at this game count).
pub fn format_standings(standings: &[Standing]) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    writeln!(out, "{:<16}{:>6}{:>5}{:>5}{:>5}{:>8}{:>18}{:>10}", "agent", "games", "W", "D", "L", "win%", "95% CI", "avg margin").unwrap();
    for s in standings {
        let (lo, hi) = s.win_rate_ci95();
        writeln!(
            out,
            "{:<16}{:>6}{:>5}{:>5}{:>5}{:>7.1}%{:>17}{:>10.1}",
            s.name,
            s.games,
            s.wins,
            s.draws,
            s.losses,
            100.0 * s.win_rate(),
            format!("[{:.0}–{:.0}%]", 100.0 * lo, 100.0 * hi),
            s.avg_margin()
        )
        .unwrap();
    }
    if standings.len() >= 2 {
        let (a, b) = (&standings[0], &standings[1]);
        let (_, a_hi) = a.win_rate_ci95();
        let (b_lo, _) = b.win_rate_ci95();
        if a_hi.min(1.0) >= b_lo {
            writeln!(
                out,
                "note: {} and {} overlap at 95% — not statistically separable at {} games each (run more seeds).",
                a.name, b.name, a.games
            )
            .unwrap();
        }
    }
    out
}
