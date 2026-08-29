# Stores (Rules §51) — full Logistics Game

"Stores" = everything that isn't fuel, ammo, or water (food, paper, uniforms…). Unlike the
per-OpStage commodities, Stores are distributed **at the start of each Game-Turn** and a unit
can limp along without them (at reduced effectiveness, with attrition). Transcribed 2026-07-14.

## Usage (§51.1)
- **§51.11** Every TOE Strength Point needs **4 Stores Points / Game-Turn**.
- **§51.13** HQ and Engineer units need only **1 Stores Point / Game-Turn**.
- **§51.12** Prisoners: 1 Stores per 5 Prisoner Points per OpStage; supplied before all else,
  and may draw from the nearest dump at any range (deferred).
- **§51.17** Guard points: 2 Stores each / GT (deferred).
- **§51.15** 1 ton each; must be present in the hex; convoy-truck Stores must be off-loaded first.
- **§51.16** 50% of captured Stores are usable (deferred).

## Lack of Stores (§51.2)
- **§51.21** A unit not receiving its Stores earns **1 Disorganization Point per Game-Turn**
  (−1 cohesion here).
- **§51.22** Every **2 consecutive** Game-Turns without Stores, lose **2% of TOE** (rounded),
  progressive & cumulative (4% at 4 turns, 6% at 6…). Only **infantry-type** TOE is eliminated;
  guns and tanks are unaffected.
- **§51.23** Half-rations (requirement 4→2, no exceeding CPA, no entering enemy ZOC), used only
  when Stores are short (deferred).

## Implementation
`GameState::unit_stores_requirement(id)` (4 × TOE, or 1 for HQ/Engineer by unit_type) and
`expend_stores()` — run once per Game-Turn at Initiative Determination (`advance_phase`). Each
unit draws its requirement from friendly Stores Dumps in its hex (off-map units are supplied
from the unlimited boxes); an unsupplied unit takes −1 cohesion (§51.21) and, every second
consecutive unsupplied turn, loses ⌈TOE × turns%⌉ of its **Organic** (infantry) TOE (§51.22).
`Unit.turns_without_stores` tracks the streak (reset when supplied).

Deferred: prisoners/guards (§51.12/17), half-rations (§51.23), captured-Stores ⅓/½ rules,
construction Stores (§24), and evaporation (that is Fuel/Water, not Stores).
