# Fortifications (§24.4 / §25.1)

## Sources
1. **Static** (§25.12): a Major City hex is Level 2; Cairo (E1430) and Alexandria (E3613) are
   Level 3. Derived from map terrain in `fort_level`.
2. **Scenario-placed** (§61.6 / §63.7x): a `fortifications: [(at, level)]` block in the scenario
   RON (loaded into `GameState.fortifications`).
3. **Player-constructed** (§24.4): Engineer unit + infantry Bn (3+ TOE), 3 Construction Segments,
   30 Stores; max Level 2 (Cairo/Alex excepted). **Deferred to the construction system (§24).**

`fort_level(hex)` = max(placed, static). `fort_shift(hex)` = −level (favouring the defender).

## Combat effect (§25.11 — column shift = level; L2 confirmed = 2 columns by §12.34 & §15.35 examples)
The cumulative rule differs by combat type:
- **Barrage (§12.34)** and **Anti-Armor (§14.33)**: fort and hex terrain are **NOT cumulative** —
  the defender takes the single best (anti-armor hexsides still add on top).
- **Close Assault (§15.35)**: fort is **cumulative** with hex terrain and hexsides (example:
  Fort L2 + Rough + Ridge = 6 columns).

## Deferred
- Construction / reduction / rebuild of forts (§24.4 / §25.14) — needs the construction system.
- Level 1 = 1 and Level 3 = 3 follow the linear pattern (L2 = 2 is confirmed); reconfirm L1/L3 off
  the printed TEC if a discrepancy ever shows.
