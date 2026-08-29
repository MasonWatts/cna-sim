# Commonwealth Tank & Gun Characteristics Chart (Rules §4.47)

Human-readable transcription. Authoritative machine values:
`data/reference/weapons/commonwealth.ron`.

**Verification status:** Fully verified by the project owner (2026-07-07).

## Columns
Type · CPA · AA · Barrage · Anti-Armor · Vulnerability · Armor Protection ·
Close Assault (Off/Def) · Fuel Rate · BAR

## Notation (from the chart Key)
- All ratings are "in points". `—` = not applicable or zero. `( )` = conditional rating (Case 3.4).
- **BAR** (Breakdown Adjustment Rating) = column shifts on the breakdown table: `R` = right, `L` = left, `0` = none, `—` = N/A (towed guns aren't subject to breakdown). Encoded as signed int (R=+, L=−, `0`, `None`).
- `*` US tank · `†` mineclearing (Valentine-based, engineer-treated) · `‡` self-propelled ("Bishop"/"Priest"/"Deacon").

## Tank Weapon Systems

| Type | CPA | AA | Barr | A-Arm | Vul | Armor | CA O/D | Fuel | BAR |
|---|---|---|---|---|---|---|---|---|---|
| A9 Cruiser | 25 | 1 | — | 3 | — | 1 | 3/3 | 2 | 1R |
| A10 Cruiser | 15 | 1 | — | 3 | — | 2 | 3/3 | 2 | 1R |
| A13 Cruiser | 30 | 0 | — | 3 | — | 2 | 2/3 | 2 | 1R |
| Churchill II | 15 | — | — | 6 | — | 7 | 5/6 | 7 | 0 |
| Crusader Mk.I | 25 | — | — | 3 | — | 3 | 3/4 | 3 | 1R |
| Crusader Mk.II | 25 | — | — | 3 | — | 4 | 4/4 | 3 | 1R |
| Crusader Mk.III | 25 | — | — | 6 | — | 4 | 4/5 | 3 | 1R |
| Grant M3* | 20 | — | — | 7 | — | 4 | 6/5 | 7 | 0 |
| Mark VI Light | 35 | 1 | — | 0 | — | 1 | 2/2 | 1 | 0 |
| Matilda Mk.II | 15 | — | — | 3 | — | 6 | 3/4 | 3 | 1R |
| Sherman* | 20 | 1 | — | 6 | — | 6 | 5/5 | 6 | 0 |
| Stuart M3* | 35 | 1 | — | 4 | — | 3 | 4/4 | 4 | 0 |
| Valentine Mk.II | 15 | 0 | — | 3 | — | 5 | 3/4 | 3 | 0 |
| Scorpion† | 25 | 0 | — | — | — | 7 | 0/(2) | 3 | 1L |

## Artillery

| Type | CPA | AA | Barr | A-Arm | Vul | Armor | CA O/D | Fuel | BAR |
|---|---|---|---|---|---|---|---|---|---|
| 18-pounder Gun | 15 | — | 7 | (2) | 4 | — | 0/1 | 1 | — |
| 18/25-pounder Gun | 15 | — | 8 | 2 | 5 | — | 1/1 | 1 | — |
| 25-pounder Gun | 15 | — | 8 | 5 | 6 | — | 1/1 | 1 | — |
| 4.5" Gun | 15 | — | 11 | 1 | 9 | — | 1/0 | 1 | — |
| 5.5" Gun/Howitzer | 15 | — | 15 | 1 | 7 | — | 1/0 | 1 | — |
| 60-pounder Gun | 15 | — | 12 | 1 | 7 | — | 1/0 | 1 | — |
| SP 25-pounder Gun‡ | 15 | — | 8 | 4 | 3 | 5 | 2/1 | 3 | 2R |
| 3.7" Howitzer | 15 | — | 7 | 0 | 3 | — | 0/0 | 1 | — |
| 4.5" Howitzer | 15 | — | 9 | 0 | 3 | — | 1/0 | 1 | — |
| 6" Howitzer | 15 | — | 15 | 1 | 5 | — | 1/0 | 1 | — |
| 155mm Howitzer | 15 | — | 15 | 1 | 6 | — | 1/0 | 1 | — |
| 105mm SP Howitzer‡ | 20 | 1 | 9 | 2 | 4 | 4 | 2/3 | 3 | 0 |

## Anti-tank

| Type | CPA | AA | Barr | A-Arm | Vul | Armor | CA O/D | Fuel | BAR |
|---|---|---|---|---|---|---|---|---|---|
| 2-pounder | 15 | — | — | 4 | 2 | — | 1/1 | 1 | — |
| 6-pounder | 15 | — | — | 7 | 2 | — | 1/1 | 1 | — |
| 17-pounder | 15 | — | — | 13 | 2 | — | 1/1 | 1 | — |
| SP 6-pounder‡ | 20 | — | — | 7 | 2 | 1 | 2/2 | 1 | 1L |

## Anti-air

| Type | CPA | AA | Barr | A-Arm | Vul | Armor | CA O/D | Fuel | BAR |
|---|---|---|---|---|---|---|---|---|---|
| Light AA (Bofors 40mm) | 15 | 1 | — | (1) | 2 | — | (1)/(1) | 1 | — |
| Heavy AA (3.7") | 15 | 4 | — | (7) | 2 | — | (1)/(1) | 1 | — |

## Footnotes (from the chart Key)
- `*` = US tank (affects the number of tanks a Commonwealth tank battalion-equivalent may contain).
- `†` = mineclearing tanks (based on a modified Valentine chassis); treated as engineer
  units for entering/clearing minefields (§26.0). The Scorpion listed here is the flail
  variant. (Historically also Baron on a Matilda chassis, Crab on a Sherman chassis.)
- `‡` = self-propelled. SP 25-pounder = "Bishop" (later "Sexton"); 105mm SP = "Priest";
  SP 6-pounder = "Deacon".

## Provenance
Two-pass Apple Vision OCR (`tools/ocr.swift`): original + 4-part higher-zoom recapture,
merged and verified/corrected by the project owner (2026-07-07).
