# Initiative Ratings Chart (Rules §7.2)

Human-readable transcription. Machine data: `data/reference/initiative-ratings.ron`.

Used in §7.14: each player rolls one die and adds their Initiative Rating; higher total
wins Initiative for the Game-Turn (ties re-roll).

| Player | Situation | Initiative Rating |
|---|---|---|
| Commonwealth | Game-Turns 1 thru 42 incl. | 3 |
| Commonwealth | Game-Turns 43 thru 90 incl. | 4 |
| Commonwealth | Game-Turns 91 thru 111 incl. | 5 |
| Axis | Rommel counter on the Game-Maps\* | 6 |
| Axis | German land combat units but no Rommel counter on the Game-Maps\* | 3 |
| Axis | No German combat units or Rommel counter on the Game-Maps\* | 1 |

\* The Holding Boxes on Game-Map A for the Tripoli/Tunisia region are **not** part of the
Game-Maps for this purpose.

## Notes for the engine
- Commonwealth rating is a pure Game-Turn lookup.
- Axis rating is derived from game state: Rommel on the maps → 6; else any German land
  combat unit on the maps → 3; else → 1. (The holding-box exclusion is deferred until
  those boxes are modeled; any on-map German unit currently counts.)

## Provenance
Screenshot of the §7.2 chart, direct read, 2026-07-07.
