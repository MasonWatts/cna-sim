# Unit Basic Stacking Point Values (Rules §9.4)

Human-readable transcription (PDF page 97). Encoded in code as
`OrgSize::stacking_points()` in `crates/engine/src/state/unit.rs`.

| Unit Equivalent (Full) | (Shell counts as) | Stacking Point Value |
|---|---|---|
| Division | — | 5 |
| Super Brigade\* | Division | 3 |
| (Standard) Brigade, Battle Group | — | 2 |
| Battalion | Any Brigade, Battle-Group | 1 |
| Company | Battalion, HQ, any attached units | 0 |
| Truck Points in Convoy | — | ½ for each 5 (or fraction) † |
| Replacement Points | — | 1 for each 5 (or fraction) |

The **Shell** column encodes §9.28: a shell unit counts one organizational level down
(a shell Division = Super-Brigade value 3; a shell Brigade/Battle-Group = Battalion
value 1; a shell Battalion = Company value 0).

## Footnotes
- `*` "Super Brigade" is a term coined to distinguish brigades that were effectively
  infantry-light divisions from 'standard' brigades.
- `†` The Truck-Points-in-Convoy value only matters when determining Road/Track CP
  movement-cost applicability.

## Notes on modeling
- `OrgSize` = { Division, SuperBrigade, StandardBrigade, BattleGroup, Battalion, Company }.
- Truck/Replacement point stacking (fractional) will be handled with the transport model.
- HQ counters have parenthesized stacking (§9.12): 0 when bare, the formation's value
  when representing attached combat units — wired when formation aggregation lands.

## Provenance
Screenshot of the printed chart (PDF page 97) via Apple Vision OCR, 2026-07-07.
