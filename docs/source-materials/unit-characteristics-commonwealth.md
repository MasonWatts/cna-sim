# Commonwealth Unit Characteristics Chart (Rules §4.46a)

Human-readable transcription. Authoritative machine values:
`data/reference/unit-characteristics/commonwealth.ron`.

**Verification status:** Fully verified by the project owner (2026-07-06), including the
Reconnaissance block (re-aligned from the OCR x-coordinates after a column-shift error in
the first draft).

## Columns
Unit Type · ID · CPA · Anti-Air · Barrage · Anti-Armor · Vulnerability · Armor Protection ·
Close Assault (Off/Def) · Max TOE · Assignment

## Key
- `*` after CPA — retains this CPA regardless of assigned TOE points' CPA.
- `+` after CPA — historically motorized (CW bn/coy-equivalents only); only CPA changes while motorized.
- `**` after Max TOE — if all US-tank TOE points, Max TOE = 9.
- `( )` — parenthesized (conditional) rating (Case 3.4). `-` — not applicable.

## Table

| Unit Type | ID | CPA | AA | Barr | A-Arm | Vuln | Armor | CA O/D | Max | Assignment |
|---|---|---|---|---|---|---|---|---|---|---|
| Headquarters | a | 30 | — | — | — | — | — | — | — | (bare HQ) |
| Headquarters | b | 30* | | | | | | | (1) | 1 tank |
| Headquarters | c | 30 | | | | | | | 3 | ≤3 artillery |
| Headquarters | d | 20* | | | | | | | (1) | 1 tank |
| Headquarters | e | 20 | | | | | | 0/1 | | |
| Headquarters | f | 20 | | | | | | 0/1 | | |
| Tank Bn-Eq | g | 25 | | | | | | | 10** | ≤10 tank |
| Infantry Bn-Eq | h | 15 | | | | | | 2/2 | 5 | non-motorized (note 4) |
| Infantry Bn-Eq | j | 15 | | | | | | 2/2 | 4 | non-motorized (note 4) |
| Infantry Bn-Eq | k | 10 | | | | | | 2/2 | 7 | |
| Infantry Bn-Eq | l | 10+ | | | | | | 2/2 | 6 | |
| Infantry Bn-Eq | m | 10 | | | | | | 2/2 | 6 | |
| Infantry Bn-Eq | n | 10+ | | | | | | 1/2 | 6 | |
| Infantry Bn-Eq | o | 10+ | | | | | | 1/2 | 6 | |
| Infantry Bn-Eq | p | 10 | | | | | | 1/2 | 6 | |
| Infantry Bn-Eq | q | 10 | | | | | | 1/1 | 6 | |
| Infantry Bn-Eq | r | 10 | | | | | | 1/2 | 5 | |
| Infantry Bn-Eq | s | 10 | | | | | | 1/2 | 5 | |
| Infantry Bn-Eq | t | 10 | | | | | | 1/1 | 3 | |
| Infantry Bn-Eq | u | 8 | | | | | | 4/6 | 3 | |
| Infantry Coy-Eq | v | 10+ | | | | | | 1/1 | 1 | |
| Infantry Coy-Eq | w | 8 | | | | | | 4/6 | 2 | |
| Artillery Bn-Eq | x | 20 | | | | | | | 6 | ≤6 artillery |
| Artillery Bn-Eq | y | 20 | | | | | | | 4 | ≤4 artillery |
| Anti-tank Bn-Eq | z | 15 | | | | | | | 11 | ≤8 anti-tank + 3 artillery |
| Anti-tank Bn-Eq | aa | 15 | | | | | | | 8 | ≤8 anti-tank |
| Anti-tank Bn-Eq | bb | 15 | | | | | | | 6 | ≤4 anti-tank + 2 Light AA |
| Anti-tank Coy-Eq | cc | 15 | | | | | | | 4 | ≤4 anti-tank |
| Anti-tank Coy-Eq | dd | 15 | | | | | | | 2 | ≤2 anti-tank |
| Light Anti-air | ee | 15 | | | | | | | 6 | ≤6 Light AA |
| Light Anti-air | ff | 15 | | | | | | | 4 | ≤4 Light AA |
| Heavy Anti-air Bn-Eq | gg | 15 | | | | | | | 2 | ≤2 Heavy AA |
| Reconnaissance Bn-Eq | hh | 45 | | | | — | 2 | 3/3 | 8 | |
| Reconnaissance Bn-Eq | jj | 45 | | | | | 2 | 3/3 | 8 | |
| Reconnaissance Bn-Eq | kk | 45 | | | | | 2 | 2/3 | 8 | |
| Reconnaissance Bn-Eq | ll | 45 | | | | | 1 | 2/3 | 8 | |
| Reconnaissance Bn-Eq | mm | 45 | | | | | 2 | 3/3 | 6 | |
| Reconnaissance Bn-Eq | nn | 45 | | | | | 1 | 3/3 | 6 | |
| Reconnaissance Bn-Eq | pp | 45 | | | | | 2 | 2/3 | 6 | |
| Reconnaissance Bn-Eq | qq | 45 | | | | | 1 | 2/3 | 6 | |
| Reconnaissance Bn-Eq | rr | 45 | | | | | 2 | 3/3 | 4 | |
| Reconnaissance Bn-Eq | ss | 45 | | | | | 1 | 1/2 | 4 | |
| Reconnaissance Bn-Eq | tt | 35 | | | 2 | — | 3 | 4/4 | 3 | |
| Construction (Road/RR) | uu | 25 | | | | | | 0/(1) | (1) | |

## Chart notes (verified)
1. From the 1/39 Game-Turn, certain Reconnaissance units may be upgraded (Case 20.5).
2. From the 1/75 Game-Turn, infantry battalions with ID `k`–`t` may be augmented by
   anti-tank TOE points (Case 19.9).
3. From the 1/87 Game-Turn, infantry battalions with ID `n`, `p`, `r`, `s` may be
   upgraded (Case 20.77).
4. The two units with ID `h` and `j` are non-motorized (though they may be motorized by
   Truck Points; the SAS may be motorized by a CW LRDG, Case 27.88).

## Provenance
Transcribed from a two-page screenshot via Apple Vision OCR (`tools/ocr.swift`),
verified/corrected by the project owner (2026-07-06).
