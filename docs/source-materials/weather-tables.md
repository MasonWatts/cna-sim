# Weather Tables (Rules §29.61, §29.7)

Human-readable transcription. Authoritative machine values: `data/reference/weather-table.ron`.
Verified from the rulebook screenshots by the project owner (2026-07-14).

## §29.61 Weather Table
Roll two dice read sequentially as **larger×10 + smaller** (11..66, §12.42), and read the row
for the current season. The season is by **Game-Turn block** (this column is authoritative and
does *not* match the calendar-date seasons in §29.1's prose):

| Season | Game-Turns | Normal | Hot | Sandstorm* | Rainstorm* |
|---|---|---|---|---|---|
| Spring | 1–12, 49–60, 97–108 | 11–42 | 43–55 | 56–64 | 65–66 |
| Summer | 13–24, 61–72, 109–110 | 11–23 | 24–55 | 56–66 | — |
| Fall | 25–36, 73–84 | 11–35 | 36–54 | 55–61 | 62–66 |
| Winter | 37–48, 85–96 | 11–52 | — | — | 53–66 |

`*` → proceed to the Foul Weather Location Table (§29.7).

Worked example (§29.1): a roll of **53 in Summer** → Hot Weather.

## §29.7 Foul Weather Location Table
Roll **one die**; the storm hits these map sections (others get Normal weather):

| Die | 1 | 2 | 3 | 4 | 5 | 6 |
|---|---|---|---|---|---|---|
| Sections | A,B | C,D | D,E* | B,C | B,D | B,C,D |

- `*` A sandstorm on a (Nile) Delta hex (Map E) is treated as Normal weather.
- Sandstorms do not extend north of the coastal hexes (never over open Mediterranean).
- Rainstorms *do* extend to the northern (Mediterranean) edge.

## Effects (from §29 text — implementation status)
- **Normal (§29.2):** no effect.
- **Hot (§29.3):** all sections; breakdown column +1 (§21.37); fuel & water −5%; water
  requirements doubled. *(Touch the deferred breakdown/supply systems.)*
- **Sandstorm (§29.4):** affected sections only (never Delta); **all movement costs doubled**
  (§29.44); breakdown +1 if ≥50% CP spent in sandstorm; no air; stops at coast.
- **Rainstorm (§29.5):** roads move/break like **tracks** (§29.56); **wadi hexsides**
  uncrossable except by road (§29.55); **river hexsides** not crossed by road/rail impassable
  (§29.57); Delta motorized restrictions (§29.58); fills wells; no air/construction.

Determination + per-section weather state + the §29.44 sandstorm movement penalty (×2, with the §29.41 no-delta & §29.46 stops-at-coast exceptions) are
implemented. The movement/breakdown effects are a follow-on (sandstorm movement-doubling is
the most directly implementable; rainstorm road/wadi/river effects need hexside road data,
which the digitized maps don't yet carry).
