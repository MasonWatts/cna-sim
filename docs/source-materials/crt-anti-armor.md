# Anti-Armor Fire Combat Results Table (Rules §14.6)

Human-readable transcription (PDF page 98, top). Machine data:
`data/reference/crt/anti-armor.ron`.

- **Rows** = the two-dice read, paired: `11,12 / 13,14 / 15,16 / 21,22 / … / 65,66` (18
  rows). The read is straight (die1 tens, die2 ones) covering all 36 outcomes.
- **Columns** = Actual Anti-Armor Points: `0*`, `1`..`15`, `16+` (17 columns).
- **Cells** = Damage Points = Armor Protection Points that must be lost. `–` = 0.
- `0*` applies only if Actual is 0 but **Raw** Anti-Armor Points are 1..4 (before terrain).

| DICE | 0* | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16+ |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 11,12 | – | – | – | 1 | 2 | 3 | 4 | 6 | 8 | 10 | 11 | 12 | 14 | 16 | 18 | 20 | 22 |
| 13,14 | – | – | – | 1 | 3 | 4 | 5 | 6 | 8 | 10 | 11 | 13 | 15 | 17 | 19 | 20 | 22 |
| 15,16 | – | – | 1 | 1 | 3 | 4 | 5 | 7 | 9 | 10 | 12 | 13 | 15 | 17 | 19 | 21 | 23 |
| 21,22 | – | – | 1 | 2 | 4 | 5 | 6 | 8 | 9 | 11 | 12 | 14 | 16 | 18 | 20 | 21 | 23 |
| 23,24 | – | – | 1 | 2 | 4 | 5 | 6 | 8 | 10 | 11 | 13 | 14 | 16 | 18 | 20 | 22 | 24 |
| 25,26 | – | – | 2 | 3 | 4 | 6 | 7 | 9 | 10 | 12 | 13 | 15 | 17 | 19 | 20 | 22 | 24 |
| 31,32 | – | 1 | 2 | 3 | 5 | 6 | 8 | 9 | 11 | 12 | 14 | 15 | 17 | 19 | 21 | 23 | 25 |
| 33,34 | – | 1 | 2 | 3 | 5 | 7 | 8 | 10 | 11 | 13 | 14 | 16 | 18 | 20 | 21 | 23 | 25 |
| 35,36 | – | 1 | 3 | 4 | 5 | 7 | 9 | 10 | 12 | 13 | 15 | 16 | 18 | 20 | 22 | 24 | 26 |
| 41,42 | – | 2 | 3 | 4 | 6 | 7 | 9 | 11 | 12 | 14 | 15 | 17 | 19 | 21 | 22 | 24 | 26 |
| 43,44 | – | 2 | 3 | 5 | 6 | 8 | 10 | 11 | 13 | 14 | 16 | 17 | 19 | 21 | 23 | 25 | 27 |
| 45,46 | – | 2 | 4 | 5 | 7 | 8 | 10 | 12 | 13 | 15 | 16 | 18 | 20 | 22 | 23 | 25 | 27 |
| 51,52 | 1 | 3 | 4 | 6 | 7 | 9 | 11 | 12 | 14 | 15 | 17 | 19 | 20 | 22 | 24 | 26 | 28 |
| 53,54 | 1 | 3 | 4 | 6 | 8 | 9 | 11 | 13 | 14 | 16 | 17 | 20 | 21 | 23 | 25 | 27 | 29 |
| 55,56 | 1 | 3 | 5 | 7 | 8 | 10 | 12 | 13 | 15 | 16 | 19 | 21 | 22 | 24 | 26 | 28 | 30 |
| 61,62 | 1 | 4 | 5 | 7 | 8 | 10 | 12 | 14 | 15 | 17 | 19 | 22 | 23 | 25 | 27 | 29 | 31 |
| 63,64 | 1 | 4 | 6 | 8 | 9 | 11 | 13 | 14 | 16 | 18 | 21 | 22 | 24 | 26 | 28 | 29 | 32 |
| 65,66 | 2 | 4 | 6 | 8 | 9 | 11 | 13 | 15 | 17 | 19 | 21 | 23 | 24 | 26 | 28 | 30 | 32 |

## Modifiers (to apply in the resolver)
- **Phasing Player** decreases his dice roll by one row (an 11 or 12 is unaffected).
- **Terrain** (§14.3, deferred): shift left 1 column if the target is in Level-1
  Fortification / Rough / Heavy Vegetation; left 2 for Level-2/3 Fortification or
  Mountain. Additionally left 1 if all attackers cross a slope hexside, left 2 for a
  ridge hexside, or left 2 if attacking down an escarpment (attacker cannot be harmed).
  A shift below zero = zero Anti-Armor Points.

## Provenance
Screenshot crops of PDF page 98 (top) via Apple Vision OCR + direct read, 2026-07-07.
