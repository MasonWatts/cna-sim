# Italian Tank & Gun Characteristics Chart (Rules §4.48)

Human-readable transcription. Authoritative machine values:
`data/reference/weapons/italian.ron`.

**Verification status:** Transcribed directly from the PDF render (page 139, read in
full — no OCR gaps), 2026-07-07.

## Columns
Type · CPA · AA · Barrage · Anti-Armor · Vulnerability · Armor Protection ·
Close Assault (Off/Def) · Fuel Rate · BAR

## Notation (from the chart Key)
- All ratings in points. `—` = not applicable or zero. `( )` = conditional rating (Case 3.4).
- **BAR** = breakdown-table column shifts to the right (`R`, positive) or none (`0`); `—` = N/A.
- `0+` CPA = motorizable but without organic transport (footnote ****).

## Tank Weapon Systems

| Type | CPA | AA | Barr | A-Arm | Vul | Armor | CA O/D | Fuel | BAR |
|---|---|---|---|---|---|---|---|---|---|
| CV 33 (L3/35) | 25 | 0 | — | 0 | — | 1 | 1/2 | 1 | 2R |
| L 6/40 | 25 | 0 | — | 1 | — | 2 | 2/2 | 2 | 0 |
| M 11/39 | 20 | 1 | — | 2 | — | 2 | 3/3 | 2 | 1R |
| M 13/40 | 20 | 1 | — | 3 | — | 3 | 3/3 | 2 | 1R |
| M 14/41 | 20 | 1 | — | 3 | — | 3 | 3/3 | 2 | 0 |

## Gun Weapon Systems

| Type | CPA | AA | Barr | A-Arm | Vul | Armor | CA O/D | Fuel | BAR |
|---|---|---|---|---|---|---|---|---|---|
| 65/17 Gun | 15 | — | 5 | 0 | 3 | — | 1/1 | 1 | — |
| 75/18 Gun-Howitzer | 15 | — | 6 | 0 | 5 | — | 1/0 | 1 | — |
| 75/18 Gun\* (Semovente SP) | 20 | — | 6 | 6 | 4 | 3 | 3/3 | 2 | 1R |
| 75/27 Gun | 15 | — | 6 | 2\*\* | 4 | — | 1/1 | 1 | — |
| 100/17 Howitzer | 15 | — | 8 | 0 | 5 | — | 1/0 | 1 | — |
| 105/28 Gun | 15 | — | 9 | 1 | 7 | — | 1/1 | 1 | — |
| 149/13 Howitzer | 15 | — | 15 | 0 | 5 | — | 1/0 | 1 | — |
| ParaArt\*\*\* | 15 | — | 2 | 2 | 1 | — | 1/1 | 1 | — |
| 149mm Vichy French | 15 | — | 10 | 1 | 5 | — | 0/1 | 1 | — |
| 155mm Rimhailo (Fr.) | 15 | — | 13 | 0 | 3 | — | 0/0 | 1 | — |

## Anti-tank

| Type | CPA | AA | Barr | A-Arm | Vul | Armor | CA O/D | Fuel | BAR |
|---|---|---|---|---|---|---|---|---|---|
| 47/32 Mod. 37 | 15 | — | — | 4 | 2 | — | 1/1 | 1 | — |

## Anti-air

| Type | CPA | AA | Barr | A-Arm | Vul | Armor | CA O/D | Fuel | BAR |
|---|---|---|---|---|---|---|---|---|---|
| Light (20mm M/35 Breda) | 15 | 1 | — | 2 | 2 | — | (1)/(1) | 1 | — |
| 'T' Light \*\*\*\* | 0+ | 1 | — | 2 | 2 | — | (1)/(1) | 1 | — |
| Heavy (75/46 Mod. 34) | 15 | 2 | — | (5) | 2 | — | (1)/(1) | 1 | — |
| 'T' Heavy | 0+ | 2 | — | (5) | 2 | — | (1)/(1) | 0 | — |
| Heavy (90/53) | 15 | 3 | — | 9 | 2 | — | (1)/(1) | 1 | — |

## Footnotes
- `*` Semovente self-propelled 75/18 gun — one of the best weapons the Italians had.
- `**` The 75/27 guns have an anti-armor rating of zero until the first Operations Stage of Jan 1942 (Game-Turn 63).
- `***` Comprised of 81mm mortars and light (37mm etc.) anti-tank guns; the only Italian gun weapons system that may be airdropped.
- `****` These `0+` CPA anti-aircraft TOE points were without organic transport (not encased in bunkers); movable if truck points are assigned. Those in the Tobruk 1st–5th CD Artillery Groups are part of immobile units and may not move unless the artillery TOE points are eliminated. The only `0+` CPA TOE points in the game start at Tobruk, Benghazi, and Benina.

## Note on schema
The `0+` CPA (for 'T' Light / 'T' Heavy AA) is stored as `cpa: 0` with the `0+`
motorizable meaning captured in `notes`, since `WeaponsSystem.cpa` is a plain integer.

## Provenance
PDF page 139 → `pdftoppm` (300 DPI) → `sips -r 270` (right-hand page, opposite rotation
to p.136) → read directly. 2026-07-07.
