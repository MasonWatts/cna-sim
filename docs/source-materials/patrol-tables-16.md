# Patrol & Reconnaissance Tables (§16)

Human-readable transcription (PDF pages 96 & 98, Charts & Tables Common to Both Players,
chart pages 1 and 3). Direct read of the vector render at 220 DPI, 2026-08-31. Machine
values live in `crates/engine/src/reference/patrol.rs` (encoded in code, like the §9.4
Stacking Point Values); this file is the provenance record.

## [16.6] Patrol Survival Table (PDF p96)

Losses to the **patrolling** force, in TOE Strength Points, by die roll. `*` If all
Patrolling TOE Strength Points are from Recce units, subtract one from the die roll
(§16.33). A roll of 0 (from the −1) reads as the `0…3` row.

| DIE* | Killed / Captured |
|------|-------------------|
| 0…3  | 0 / 0             |
| 4    | 0 / 1             |
| 5    | 1 / 0             |
| 6    | 1 / 1             |

Captured Patrol Points become Prisoner Points (§16.33). If no Patrol Points survive, the
patroller gets no information (§16.33) and any Objective Loss "Captured" becomes "Killed"
(§16.34).

## [16.7] Patrol Reconnaissance Table (PDF p98)

Result = the number of battalion-size-equivalent enemy units about which the non-Phasing
player must reveal information (§16.5). Cross-reference the die roll (row) with the
**net** (surviving) Patrol Points (column). `All` = every unit in the hex.

| DIE | 1 | 2 | 3   |
|-----|---|---|-----|
| 1   | 0 | 0 | 1   |
| 2   | 0 | 1 | 1   |
| 3   | 1 | 1 | 2   |
| 4   | 1 | 2 | 3   |
| 5   | 2 | 3 | 4   |
| 6   | 3 | 4 | All |

(Net Patrol Points is capped at 3, §16.14. A dummy tank unit counts as TOE 3 for
reveal purposes, §16.45 / `DUMMY_TANK_RECON_TOE`.)

Revealed information per unit (§16.51): historical designation, unit type, motorized-or-not,
and the exact TOE Strength **within two** of the actual strength (defender's choice within
±2, §16.51). Defender chooses which units (§16.52), restricted to battalion-size units
(§16.53), never HQ/AA unless no combat units are present (§16.54); the patroller may name
units to exclude (§16.55).

## [16.8] Objective Loss Table (PDF p98)

Losses inflicted on the **patrollee** (units in the reconnoitered hex), by die roll. The
non-Phasing player chooses which TOE to lose, from a combat unit if any (§16.34). If the
patrolling force was completely eliminated, a `Captured` result becomes `Killed` (§16.34).

| DIE | Killed / Captured |
|-----|-------------------|
| 1…4 | 0 / 0             |
| 5    | 0 / 1            |
| 6    | 1 / 0            |
