# Scenario 1 — Initial Supply, Abstract Land Game (§60.92 B/C)

> **Update (D15 — full Logistics Game):** the engine now loads the **full** §60.34 (Axis) /
> §60.44 (Commonwealth) four-commodity initial supply (Ammo/Fuel/Stores/Water), not the §60.92
> abstract Supply Units below. Fixed dumps: Tobruk 200/2000/500, Bardia 100/1000/200, Benghazi
> (A4827) 100/250/100, Derna 0/250/50, Tripoli box 250/5000/250, C0716 100/50/50; Mersa Matruh
> 1000/3000/4000, Sidi Barrani 250/500/100; Cairo & Alexandria unlimited. Player-choice: Axis
> Dump 1 (1000/1500/1500/200) + Dump 2 (1000/1500/1000/200) + 2 dummies (section C, ≥5 from any
> CW unit); CW Dump I (500/750/500) + 1 dummy (Egypt, map C or D). The §60.92 tables below are
> retained for reference only.


For the Land Game played with abstract logistics (§32), **§60.92** replaces the full-game
supply tables (§60.34 Axis / §60.44 CW) with these **Supply Units** (each ≤40 Ammo / ≤60 Fuel).
Transcribed from the rulebook (2026-07-14).

## §60.92 B — Axis Initial Supply
| Location | Supply Units | Ammo (each) | Fuel (each) |
|---|---|---|---|
| Tobruk (C4807) | 3 | 15 | 50 |
| Bardia (C4321) | 2 | 10 | 40 |
| Benghazi (A4827)† | 1 | 10 | 10 |
| Derna (B5925) | 1 | 0 | 10 |
| Tripoli (box, off-map) | 8 | 5 | 60 |
| Dump 1* | 3 | 40 | 50 |
| Dump 2* | 3 | 40 | 50 |
| C0716 | 1 | 20 | 6 |

Plus **two groups of three dummy Supply Units** (placed per the Dump 1/2 rule).
\*Dump 1/2: any one hex on map-section C in Libya, not within four hexes of a Commonwealth unit.
†The rulebook prints "B4827"; **Benghazi is actually A4827** (map A) — confirmed by the user;
treated as an OCR error and kept consistent with the §64.73 victory-city data.

## §60.92 C — Commonwealth Initial Supply
| Location | Supply Units | Ammo (each) | Fuel (each) |
|---|---|---|---|
| Mersa Matruh (D3714) | 5 | 40 | 60 |
| Sidi Barrani (C4131) | 1 | 40 | 60 |
| Dump I* | 3 | 40 | 30 |
| Alexandria (E3613) | 3 | 40 | 60 |
| Cairo (E1430) | 3 | 40 | 60 |

Plus **one group of three dummy Supply Units** (placed per the Dump I rule).
\*Dump I: all three in any one hex on map-section D west of Mersa Matruh.

## Player-choice dumps are modeled, not hardcoded
The Dump 1/2/I locations (and their dummies) are **player choice**, so the scenario expresses
the *constraint* rather than a fixed hex: `supply_dump_choices` entries carry a `DumpConstraint`
(`section`, `west_of`/`east_of` a reference hex, `not_within_of_enemy: N`). These load as
`GameState::pending_dump_choices` — unplaced until a decision-maker resolves them:
- **`place_dump_choice(index, hex)`** validates `hex` against the constraint and places the
  units (where a player / AI agent decides).
- **`auto_place_dump_choices()`** picks the first legal hex (arbitrary but valid) — used by
  demos/tests.

Constraints used: Axis Dump 1/2 → `section: 'C', not_within_of_enemy: 4` ("in Libya" is
approximated by the 4-hex Commonwealth standoff, which keeps the dump west toward Cyrenaica —
a precise Libya/Egypt region boundary is not yet modeled). CW Dump I → `section: 'D',
west_of: "D3714"`. NOTE: west = lower column in this coordinate system; an earlier hardcoded
D3316 was actually *east* of Mersa Matruh and thus illegal — exactly the error the modeled
constraint now catches (locked by a test).

## Implementation
Fixed dumps load via the scenario `supply_dumps` list (`SupplyDumpSpec { side, at, ammo, fuel,
count, dummy }`); `at` is a hex label or off-map node resolved through the same `resolve_entry`
used for reinforcements. Player-choice dumps load via `supply_dump_choices` (→
`pending_dump_choices`). Scenario 1: 28 fixed Supply Units placed at named locations + 6
player-choice groups (18 units) resolved to legal hexes = 46 total.
