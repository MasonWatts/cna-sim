# Scenario 1 — Order of Battle hierarchy (DRAFT, needs verification)

Source: the CNA Vassal module setup (`setup italian.vsav`, decoded to
`/tmp/cna_vmod/italian_plain.txt`), parsed with `tools/parse_vassal_ob.py`. Counter image
names encode `<NAT> <designation> - <parent>`, so each formation's org chart lists its
**full complement** of sub-units. That complement doubles as the OA "maximum assignable"
count used by the shell rule (§9.26).

Status: **unverified draft.** Two things this source does NOT provide, flagged below:
1. **Compositions (TOE / weapon groups)** — these live on the OA/TOE sheets, not in Vassal.
   Unit *types* below are inferred from the designation and marked `(?)`.
2. **Commonwealth screen** — the Italian `.vsav` is Axis-only; no CW units are present, so
   the 7th Armoured covering force hierarchy is not recoverable here (needs the CW setup or
   the §60.41 listing).

## Axis (Italian) formations present in Scenario 1

Legend for inferred type `(?)`: Lgn = CCNN Legion (Blackshirt infantry); BRs = Blackshirt
(?); GaF = Guardia alla Frontiera (frontier guard infantry); Lib/Libyan = Libyan infantry;
"Maletti"/LbMt/Sno = Maletti group tank/motorized sub-units (?).

### Maletti Group  (complement 5 = OA max)
- `1 LbMt`     — Libyan motorized bn (?)
- `1Maletti`   — M11/39 tank bn (?)
- `2Maletti`   — M11/39 tank bn (?)
- `5 Libyan`   — Libyan infantry bn (?)
- `Sno`        — ? (Sahariana / motorized company?) (?)

### 1 Libyan Division  (complement 2)
- `1 Libyan`, `2 Libyan` — Libyan infantry bns (?)

### 2 Libyan Division  (complement 2)
- `3 Libyan`, `4 Libyan` — Libyan infantry bns (?)

### 1 CCNN Division  (complement — Vassal shows 1–2, name variants "1 CCNN"/"1CCNN")
- `219Lgn` (Legion), `201BRs`, `233Lgn`  — Blackshirt infantry bns (?)  ⚠️ name-variant merge (?)

### 2 CCNN Division  (complement 3)
- `202BRs`, `231Lgn`, `238Lgn` — Blackshirt infantry bns (?)

### 4 CCNN Division  (complement 2)
- `240Lgn`, `270Lgn` — Blackshirt infantry bns (?)

### 62 Marmarica Division
- ⚠️ No sub-units found under a "Marmarica" parent tag in the setup (?). Its battalions may
  use a different parent abbreviation — needs a screenshot of the Marmarica OA sheet.

### 63 Cirene Division
- ⚠️ Likewise no "Cirene"-tagged sub-units found (?). Needs the Cirene OA sheet.

### 64 Catanzaro Division
- ⚠️ No "Catanzaro"-tagged sub-units found (?). Needs the Catanzaro OA sheet.

### Bardia Garrison  (complement 4 = OA max)
- `30 GaF` (frontier guard), `V CD`, `VEC`, `XVII` — garrison bns (?)

### Tobruk Garrison  (complement 5 = OA max)
- `31 Gaf`, `32 Gaf` (frontier guard), `Marines`, `TbrkLt` (Tobruk light), `XXXILib`
  (Libyan) — garrison bns (?)

## Open questions for verification
1. Are the Maletti sub-unit types right (which are tank vs Libyan infantry)?
2. The line/motorized divisions (Cirene/Marmarica/Catanzaro) show no tagged sub-units in
   the Vassal setup — do you have OA-sheet screenshots for their battalion breakdowns, or
   should these stay as single Division counters for now?
3. Compositions (TOE + weapon groups) per sub-unit: provide OA-sheet screenshots, or accept
   simplified type-based compositions (infantry → Organic TOE, tanks → weapon groups) for a
   first faithful-hierarchy cut?
4. Commonwealth screen: keep the §60.41 simplified battalions, or is a CW Vassal setup
   available to extract its hierarchy?
