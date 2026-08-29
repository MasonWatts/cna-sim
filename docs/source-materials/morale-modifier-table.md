# Morale Modifier Table (Rules §17.4)

Human-readable transcription (PDF page 100). Machine data:
`data/reference/crt/morale-modifier.ron`.

**How to read:** find the unit's **Disorganization Level** row, roll two dice (straight
read, 11…66), and find which column's dice range contains the roll. The column header is
the **Morale Modifier** outcome for that combat. `+8+` = level 8 or higher; `−17−` = level
17 or lower (clamp). `–` = that outcome can't occur at that level.

Columns (Morale Modifier): `+4` · `+3` · `+2` · `+1` · `NO` · `−1` · `−2` · `−3` · `−4` · `SURR`

| Level | +4 | +3 | +2 | +1 | NO | −1 | −2 | −3 | −4 | SURR |
|---|---|---|---|---|---|---|---|---|---|---|
| +8+ | 11…26 | 31…46 | 51…65 | 66 | – | – | – | – | – | – |
| +7 | 11…16 | 21…36 | 41…56 | 61…66 | – | – | – | – | – | – |
| +6 | 11…12 | 13…25 | 26…36 | 41…66 | – | – | – | – | – | – |
| +5 | 11 | 12…21 | 22…33 | 34…65 | 66 | – | – | – | – | – |
| +4 | – | 11…14 | 15…31 | 32…56 | 61…66 | – | – | – | – | – |
| +3 | – | 11…13 | 14…21 | 22…44 | 45…66 | – | – | – | – | – |
| +2 | – | – | 11…12 | 13…36 | 41…66 | – | – | – | – | – |
| +1 | – | – | – | 11…26 | 31…66 | – | – | – | – | – |
| 0 | – | – | – | 11 | 12…65 | 66 | – | – | – | – |
| −1 | – | – | – | – | 11…54 | 55…65 | 66 | – | – | – |
| −2 | – | – | – | – | 11…46 | 51…56 | 61…66 | – | – | – |
| −3 | – | – | – | – | 11…42 | 43…46 | 51…66 | – | – | – |
| −4 | – | – | – | – | 11…33 | 34…41 | 42…55 | 61…66 | – | – |
| −5 | – | – | – | – | 11…23 | 24…32 | 33…46 | 51…66 | – | – |
| −6 | – | – | – | – | 11…12 | 13…25 | 26…36 | 41…63 | 64…66 | – |
| −7 | – | – | – | – | 11 | 12…16 | 21…33 | 34…51 | 52…63 | 64…66 |
| −8 | – | – | – | – | – | 11…13 | 14…31 | 32…44 | 45…61 | 62…66 |
| −9 | – | – | – | – | – | 11 | 12…26 | 31…41 | 42…55 | 56…66 |
| −10 | – | – | – | – | – | – | 11…21 | 22…34 | 35…53 | 54…66 |
| −11 | – | – | – | – | – | – | 11…14 | 15…33 | 34…46 | 51…66 |
| −12 | – | – | – | – | – | – | 11 | 12…26 | 31…44 | 45…66 |
| −13 | – | – | – | – | – | – | 11 | 12…21 | 22…36 | 41…66 |
| −14 | – | – | – | – | – | – | – | 11…16 | 21…33 | 34…66 |
| −15 | – | – | – | – | – | – | – | 11 | 12…26 | 31…66 |
| −16 | – | – | – | – | – | – | – | – | 11…21 | 22…66 |
| −17− | – | – | – | – | – | – | – | – | – | 11…66 |

## Footnotes
- **NO** = no change to the unit's Basic Morale.
- **SURR** = all of the unit's Strength Points Surrender (exception: see case §17.26).
- **Number** = add (or subtract) that number to the unit's Basic Morale for that combat.
  Note limitations on the maximum and minimum adjusted Basic Morale.

## Notes for the engine
- "Disorganization Level" is the unit's current organization stat (the same quantity CP
  over-expenditure and combat push down). Wiring this table to the tracked value, and
  applying the modifier/surrender within morale checks, comes with the Close Assault /
  morale-check flow.

## Provenance
Screenshot of PDF page 100, direct read, 2026-07-07.
