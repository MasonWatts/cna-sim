# §19.3 Formation Organization Chart — transcription worksheet

**Purpose:** capture the per-formation, **per-type maximum battalion counts** from the §19.3 charts so
the engine can enforce them in `ReassignUnit`/`AttachUnit`. This replaces the flat `assigned_capacity`
approximation with real per-type maxima (deferral-tracker items **D2/D4**).

## Model (how to read the chart into this table)

Each glyph has three parts, and **all three matter** (they are *not* interchangeable — see below):

- **Box interior = unit type** → picks the column.
- **Roman numeral above = echelon** of that box: `I`=company, `II`=battalion, `III`=regiment,
  `X`=brigade, `XX`=division.
- **Arabic number to the left = how many battalions that one box contains.**
- **Number of boxes of a type = how many sub-units of that type** the formation may hold.

**Why echelon is load-bearing (§9.26/§9.27):** a *division*'s cap is a count of **brigade-equivalents
(regiments)** by type; a *brigade/regiment*'s cap is a count of **battalions** by type. So a division
row `(3)⊠[III] ×3` means **two** limits at once: ≤3 infantry *regiments* **and** each regiment ≤3
battalions (≤9 bns total). We must store the structure, not the battalion sum, or we lose the
regiment cap that §9.26 (and deferral item **D1**, the shell test) depends on.

**Capture format** — each box is just its two numbers, `Roman:Arabic` (echelon : battalions inside);
the Roman numeral already *is* the echelon, so there's no separate word to write. List one entry per
box (shorthand `K× Roman:Arabic` for K identical boxes):
`3× III:3` (three infantry regiments of 3 bns each) · `2× II:1` (two tank battalions) ·
`1× II:2` (one 2-bn slot). Echelons: `I`=coy, `II`=bn, `III`=regt, `X`=bde, `XX`=div.

- The artillery `1[⊡]` symbol = **1** single 1-SP artillery unit. The `(2)[⊡]` symbol = the
  "artillery HQ + all its units, **or** any three 1-SP artillery units" group — record it as
  `HQ-grp` (I'll bound it as ≤3 units unless you say otherwise).
- Leave blank / `—` for types not shown.
- `?` = my scan read is uncertain (Arabic count or echelon); **please confirm/correct.**

**Types** (German §19.33 legend): `tank`, `tank-destroyer` (TD), `anti-tank` (AT), `infantry`,
`para-heavy-weapons` (para-hvy), `artillery`, `armored-recce` (recce), `heavy-AA`, `light-AA`,
`engineer-bn`, `engineer-coy`.

---

## §19.33 German (PDF p175) — my reading (please verify the `?`s)

Cells use `K× Roman:Arabic` per box (see format note above). Parenthesized Arabic on the chart =
the counter's printed value; recorded here as the Arabic. `?` = my read is uncertain.

| Formation (OB) | tank | TD | AT | infantry | para-hvy | artillery | recce | heavy-AA | light-AA | eng-bn | eng-coy |
|---|---|---|---|---|---|---|---|---|---|---|---|
| **5 Le** (5th Light Pz, pre-transform) | `II:2` | `II:1` | `II:1` | `2× II:1` | — | `II:1` | `II:1` | `II:1` | `II:1` | `II:1` | — |
| **21 Pz** (post-transform of 5 Le) | `III:2` | `II:1` | `II:1` | `III:2` | — | `2× II:1` | `II:1` | `II:1` | `II:1` | `II:1` | — |
| **15 Pz** | `III:2` | `II:1` | — | `X:3` (15th Bde) | — | `HQ-grp` | `II:1` | — | — | `II:1` | — |
| **90 Le** | — | — | `II:1` | `2× III:2, 2× II:1` | — | `II:1` | `II:1` | — | — | `II:1` | — |
| **164 Le** | — | — | `II:1` | `3× III:2` | — | `HQ-grp` | `II:1` | — | `II:1` | `II:1` | — |
| **Armored Regiment** (generic) | `2× II:1` | — | — | — | — | — | — | — | — | — | — |
| **15th Infantry Brigade** (15 Schutzen) | — | — | — | `3× III:2` (≤6 bn rule) | — | — | — | — | — | — | — |
| **Ramcke Brigade** | — | — | — | - | `5× II:1` | — | — | — | — | — | — |
| **Infantry Regiment** (generic) | — | — | — | `3× II:1` | — | — | — | — | — | — | — |
| **288 Sonderverband** | — | — | `II:1` | `3× II:1` | — | — | — | — | — | — | `I:0` |
| **361 Afrika** | — | — | — | `2× II:1` | — | `II:1` | — | — | — | — | — |

Reading notes: divisions list infantry as **regiment (`III`)** or **brigade (`X`)** boxes — 15 Pz's
infantry is the single `X` "15th Bde" box; 164 Le is three `III` regiments; 90 Le mixes 2 regiments +
2 loose battalions. The `(N)` parenthesized numbers (regiment/brigade counters) are the crux `?`s —
confirm whether e.g. 164 Le's regiments read `(2)` or `(3)`, and Ramcke's split of plain vs
heavy-weapons parachute battalions.

Notes captured (already enforced in code):
- "No Battle Group may be assigned to a division." ✔
- 15th Infantry Brigade "may never contain more than six battalions of infantry." ✔
- 5 Le "becomes the 21st Panzer Division" — OB models this as one formation, dual HQ, time-phased.
- 28 Son / 361 Af "may be freely substituted for any 'normal' infantry regiment" (permissive).

Open questions for German: Ramcke — how many of its 5 battalions are **para-heavy-weapons** vs plain
infantry? 288 Son — is the 4th slot an anti-tank bn (`1[△·]`)? 15 Pz / 164 Le artillery — HQ-group
(store 3) or a specific number?

---

## §19.32 Italian (PDF p174) — to transcribe

Formations (OB names to be matched): Armored division · Tank Group (Libyan Tank Command) · Motorized
infantry division · Semi-motorized infantry division · Libyan infantry & parachute divisions (Gruppo
Maletti +3rd regt) · Tank regiment (4 Light + 1 Medium) · Tank regiment (smaller) · Bersaglieri infantry
regiment · Infantry regiment (54th Territorial + 4th of 10th Army = single counter) · Cavalry regiment ·
Sahara detachment · RECAM.

Italian legend: `tank`, `infantry` (leg/motorized/parachute bn), `bersaglieri` (motorized or
motorcycle inf bn), `MG-inf` (machinegun inf bn/coy), `anti-tank` (AT bn or up to 3 AT coys),
`artillery` (regt = up to 3 bns, or SP arty bns for armored divs), `AA` (any AA unit), `cavalry`
(cavalry or camel-cavalry bn), `armored-car` (regt/bn), `recce` (Bersaglieri motorcycle recce coy),
`engineer`. My reading below — verify `?`s.

| Formation (OB) | tank | infantry | bersaglieri | MG-inf | anti-tank | artillery | AA | cavalry | armored-car | recce | engineer |
|---|---|---|---|---|---|---|---|---|---|---|---|
| **Armored division** | `III:2` | `III:2` | — | — | `II:1` | `III:1` | — | — | `II:1` | — | — |
| **Tank Group** (Libyan Tank Cmd, GT 1–26 only) | `2× III:3` | — | — | — | — | — | — | — | — | — | — |
| **Motorized infantry division** | — | `2× III:2` | `III:2` | `II:1` | `II:1` | `II:1` | `1` | — | — | — | `II:1` |
| **Semi-motorized infantry division** | — | `2× III:2, II:1` | `II:1` | — | `1` | `III:1` | — | (or `I:0` camel) | — | — | `II:1` |
| **Libyan infantry & parachute divisions** | — | `2× III:2` (Maletti +1 → `3× III:2`) | — | — | `I:0` | `III:1` | — | — | — | — | — |
| **Tank regiment** (4 Light + 1 Medium) | `5× II:1` | — | — | — | — | — | — | — | — | — | — |
| **Tank regiment** (smaller) | `3× II:1` | — | — | — | — | — | — | — | — | — | — |
| **Bersaglieri infantry regiment** | — | — | `3× II:1` (or +1) | — | — | — | — | — | — | — | — |
| **Infantry regiment** (generic) | — | `3× II:1` | — | — | — | — | — | — | — | — | — |
| **Cavalry regiment** | — | — | — | — | — | — | — | `3× II:1` | — | — | — |
| **Sahara detachment** | — | `3× II:1` (or 1 → cavalry) | — | `II:1` | — | `III:1?` | — | — | — | `I:0` | — |
| **RECAM** (not "armored" for armored-div assignment) | — | — | — | — | — | `III:1` | — | — | `II:1` | `I:0` | `II:1` |

Reading notes: divisions list infantry/tank/bersaglieri as **regiment (`III`)** boxes; support arms as
`II`/`I` battalions/companies. Text rules to enforce: **Gruppo Maletti** may assign a 3rd infantry
regiment (Libyan div); **54th Territorial** and **4th of 10th Army** infantry regiments are a *single
counter*, not 4 battalions; **armored divisions** may assign SP artillery battalions; **RECAM** counts
as non-armored for armored-division assignment. Open `?`s: Motorized-div AA count; Semi-motorized-div
anti-tank (bn vs coy) and the camel-cavalry alternative; Sahara-detachment trailing artillery regiment;
Bersaglieri-regiment optional 4th battalion.

---

## §19.31 Allied / Commonwealth (PDF p135) — to transcribe (time-phased)

CW formations vary by **Game-Turn window** (Type I = GT 1–18, II = GT 19–70, III = GT 71–91, IV = GT 92+).
Enter one row per (formation, type-window).

Formations: Armored division (Type I/II/III/IV) · Infantry division · Armored brigade (Type I/II/III) ·
Support Group / Motor Brigade Group (Type I/II/III) · Infantry brigade [3] = battle group (18th Aus / 1st
Free French / Polish) · Infantry brigade [2] (3rd & 161st Indian Motor) · Allied infantry brigade [2]A
(2nd Free French / 1st Greek).

CW legend: `tank` bn · `infantry` bn (also motorized/marine/commando) · `MG-inf` bn · `artillery`
(regt `III` / bn `II` / SP-arty regt) · `AT-regt` (`III`: AT / AT-AA / tank-destroyer regiment) ·
`AT-unit` (any of those or an AT company `I:0`) · `light-AA` (regt `III` / bn `II`) · `armored-car`
(regt/bn) · `recce` (armored-recce regt/bn) · `engineer` bn (2/1 Aus Pioneer or Scorpion-tank bn) ·
`unit` = any non-tank-bn counter of stacking value 1 or 0. **Divisions compose from brigades**
(`arm-bde`, `inf-bde` = `X` sub-formations); brigades compose from battalions.

Notation `X:2` = a brigade (stacking 2). Support-arm slots are `II`/`III`/`I`. `?` = verify.

| Formation + GT window | arm-bde | inf-bde | tank | infantry | MG-inf | artillery | AT-regt | AT-unit | light-AA | armored-car | recce | engineer | unit |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| **Armored division** — I (GT 1–18) | `2× X:2` | `X:2` | — | — | — | — | — | — | — | — | — | — | — |
| **Armored division** — II (GT 19–70) | `2× X:2` | `X:2` | — | — | — | — | — | — | — | `II:1` (or recce) | (or AC) | — | — |
| **Armored division** — III (GT 71–91) | `X:2` | `X:2` | — | — | — | — | — | — | — | `II:1` | — | — | — |
| **Armored division** — IV (GT 92+) | `X:2` | `X:2` | — | — | `II:1` | `3× III:1` | `III:1` | — | `III:1` | `II:1` (or recce) | (or AC) | `II:1` | — |
| **Infantry division** | — | `3× X:2` | — | — | `II:1` | `3× III:1` | `III:1` | `1` | — | `II:1` (or recce) | (or AC) | `II:1` | — |
| **Armored brigade** — I (GT 1–18) | — | — | `3× II:1` | — | — | — | — | — | — | — | — | — | — |
| **Armored brigade** — II (GT 19–70; 92+) | — | — | `3× II:1` | `II:1` | — | — | — | — | — | — | — | — | — |
| **Armored brigade** — III (GT 71–91) | — | — | `3× II:1` | `II:1` | — | `II:1` | — | — | — | — | — | — | — |
| **Support Group / Motor Bde Grp** — I (GT 1–18) | — | — | — | `2× II:1` | — | `II:1` | `III:1` | — | — | `II:1` (or recce) | (or AC) | — | — |
| **Support Group / Motor Bde Grp** — II (GT 19–70) | — | — | — | `II:1` | — | `II:1` | `III:1` | — | — | — | — | — | — |
| **Support Group / Motor Bde Grp** — III (GT 71–91) | — | — | — | `3× II:1` | — | `II:1` | — | — | — | — | — | — | — |
| **Infantry brigade [3]** (18th Aus / 1st FF / Polish) | — | — | — | `3× II:1` | `II:1` or Light AA | `II:1` | — | `1` | `III:1` or MG | `III:1` (or recce) | (or AC) | — | `1` |
| **Infantry brigade [2]** (3rd / 161st Indian Motor) | — | — | — | `3× II:1` | — | — | — | — | — | — | — | — | — |
| **Allied infantry brigade [2]A** (2nd FF / 1st Greek) | — | — | — | `3× II:1` | — | `II:1` (or AT-unit) | — | — | — | — | — | — | `1` |

Reading notes: CW armored & infantry divisions are capped by **brigade** count (§9.26): armored div =
2 armored + 1 infantry brigade (Types I–II), dropping to 1+1 by Type III/IV; infantry div = 3 infantry
brigades. Each brigade then has its own battalion-level row. Types are Game-Turn windows (**time-phased,
= deferral item D4**). Open `?`s: division artillery counts (3 regiments vs battalions?); the
armored-car/recce "or" slots; the battle-group [3] brigade's light-AA/MG and AC/recce "or" choices and
its trailing "Unit" slot.
