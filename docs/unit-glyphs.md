# Unit-type glyphs (counter symbols)

Captured from the counter legends in the source materials, to design a compositional glyph model
(a base **branch** symbol + overlaid **modifiers**; some counters overlay two branches). Each entry
records the printed label, the glyph as drawn, and the proposed **primitive decomposition**.

Legend for the "glyph" column: `[ ]` = the counter box; the echelon tick (`I`=coy, `II`=bn, `III`=regt)
sits above the box and is orthogonal to type.

---

## §19.32 Italian (PDF p174, bottom legend)

### Base branch symbols observed
| Primitive | Drawn as |
|---|---|
| `Infantry` | `✕` (X) filling the box |
| `Armor` | horizontal **oval** |
| `Artillery` | a single **dot** (bullseye) |
| `AntiTank` | upward open **chevron/△ with a dot** inside |
| `AntiAir` | upward open **triangle/delta** (no dot), wider than the AT chevron |
| `Cavalry` | a single **diagonal** line (`╱`) |
| `ArmoredCar` | a small **rounded/oval car** shape (flatter than the tank oval) |
| `MachineGun` | the infantry `✕` with a small **vertical (up) arrow** over the centre |
| `Engineer` | the **`m`** / arch symbol |
| `Recon` | (only as the Bersaglieri motorcycle recce coy — see below) |

### Modifiers observed (overlaid on a branch)
| Modifier | Drawn as | Notes |
|---|---|---|
| `Motorized` | a small **dot** (corner) | subtle; distinct on the Bersaglieri motorized counter |
| `Motorcycle` | a **row of small dots/wheels** along the bottom edge | |
| `Airborne` (parachute) | a **gull/arc** (∪) beneath the branch symbol | clearest on the parachute artillery counter |
| `Bersaglieri` | a **"Brs"** text tag below the box | Italian elite infantry marker |
| `Camel` | a **"c"** (camel) mark by the diagonal | on the Saharan cavalry |
| `SelfPropelled` | the branch symbol drawn **inside/with the armor oval** | e.g. SP artillery = oval + dot |

### Enumerated Italian unit types (in legend order)
| # | Label | Glyph (as drawn) | Decomposition |
|---|---|---|---|
| 1 | Tank battalion | box + oval | `Armor` |
| 2 | Infantry battalion — (Leg) | box + ✕ | `Infantry` |
| 3 | Infantry battalion — motorized | box + ✕ *(same glyph as leg in this legend)* | `Infantry` (+`Motorized`) |
| 4 | Infantry battalion — parachute | box + ✕ *(same glyph as leg in this legend)* | `Infantry` (+`Airborne`) |
| 5 | Bersaglieri motorized infantry battalion | box + ✕ + corner dot, "Brs" below | `Infantry` + `Bersaglieri` + `Motorized` |
| 6 | Bersaglieri motorcycle infantry battalion | box + ✕ + bottom wheels, "Brs" below | `Infantry` + `Bersaglieri` + `Motorcycle` |
| 7 | Machinegun infantry battalion | box + ✕ with a small vertical arrow | `Infantry` + `MachineGun` |
| 8 | Motorized machinegun infantry company | box (coy) + ✕/bar + bottom wheels | `Infantry` + `MachineGun` + `Motorized` |
| 9 | Anti-tank battalion / company | box + ∧ chevron + dot | `AntiTank` |
| 10 | Anti-tank unit (bn *or* coy) | box + ∧ chevron + dot | `AntiTank` |
| 11 | Artillery regiment | box + dot | `Artillery` |
| 12 | Artillery battalion | box + dot | `Artillery` |
| 13 | Parachute artillery regiment | box + dot + gull/arc beneath | `Artillery` + `Airborne` |
| 14 | SP (self-propelled) artillery battalion | box + oval + dot | `Armor` + `Artillery` (`SelfPropelled`) |
| 15 | Anti-aircraft unit | box + open delta | `AntiAir` |
| 16 | Cavalry battalion | box + diagonal | `Cavalry` |
| 17 | Camel cavalry battalion | box + diagonal + "c" | `Cavalry` + `Camel` |
| 18 | Armored car regiment | box + car-oval (III) | `ArmoredCar` |
| 19 | Armored car battalion | box + car-oval (II) | `ArmoredCar` |
| 20 | (Bersaglieri) motorcycle reconnaissance company | box + recon mark + wheels | `Recon` + `Bersaglieri` + `Motorcycle` |
| 21 | Engineer battalion | box + `m` symbol | `Engineer` |

### Open questions / to verify
- The **leg vs motorized vs parachute** infantry styles are drawn with the *same* ✕ box in this legend
  (the style is a label). On the actual counters the modifier may be drawn — confirm when scanning tokens.
- Is `MachineGun` a **base branch** (✕ + bar) or better modeled as `Infantry` + a MG modifier? (Leaning
  base branch, since the bar replaces nothing and reads as its own type.)
- `Motorized` mark is faint — confirm whether plain motorized infantry shows it on the counters.
- `ArmoredCar` vs `Recon`: the armored-car oval and the Bersaglieri recce mark differ — keep both, or is
  `ArmoredCar` = `Armor` + `Recon`?

---

### Verified against physical tokens (Italian Pistoia/Pavia set)
- **Bersaglieri** is drawn as **motorized infantry** (✕ + wheels at the lower corners); the "Brs" on
  the counter is the unit's *name*, not a symbol tag. So `Bersaglieri` renders as motorized wheels
  (kept as a distinct primitive only so §19.3 can use its separate Bersaglieri slot). "57 Brs" ✓.
- **III Monferrato** ("III Monf", Pistoia) is an **armored car** (car body on wheels), not cavalry —
  retagged `ArmoredCar`.
- Division HQ / infantry regiment / infantry battalion / artillery ("30") / engineer ("16") tokens all
  match the current glyphs.

Verified against the full Italian counter sheet (**PDF p182**):
- **Camel** ("Mhrsti"/Meharisti) = the cavalry diagonal **+ a "c"** → `Cavalry` + `Camel` ✓.
- **Marine** ("Marines") = infantry ✕ **+ an anchor** → `Infantry` + `Marine`. Viewer draws a small
  anchor (ring / shaft / stock / fluke arc) over the ✕.
- **GaF** (frontier-guard) battalions = infantry ✕; GaF artillery = dot — plain `Infantry`/`Artillery`.
- **Commando** still unconfirmed here (no commando counters on the Italian sheet — needs a CW token).

## §19.33 German (PDF p175, bottom legend)

### New/changed primitives vs. Italian
| Primitive | Drawn as | Notes |
|---|---|---|
| `TankDestroyer` | a **dome/arch** (rounded hump) in the box | **new base branch** — distinct from `AntiTank` (△) |
| `Recon` | an **oval with a diagonal** slash (armored recce) | German draws it as armor+diagonal |
| `HeavyWeapons` (modifier) | the branch symbol drawn **filled/solid** | heavy-weapons para infantry = filled ✕; heavy AA = filled △ |
| `Airborne` (modifier) | (parachute) — see heavy-weapons parachute infantry | as in Italian |

`AntiAir` is drawn as an upward triangle; **filled = heavy, open = light** (the `HeavyWeapons`/`Heavy`
modifier, not two separate branches). `Artillery` = dot, as Italian.

### Enumerated German unit types (in legend order)
| # | Label | Glyph (as drawn) | Decomposition |
|---|---|---|---|
| 1 | Artillery HQ group `(2)[⊡]` | box + dot (HQ grouping) | `Artillery` — a **composition slot** ("an arty HQ + all its units, *or* any three 1-SP arty units"), not a distinct counter symbol |
| 2 | Tank battalion | box + oval | `Armor` |
| 3 | Tank destroyer battalion | box + dome/arch | `TankDestroyer` |
| 4 | Anti-tank battalion | box + open △ | `AntiTank` |
| 5 | Infantry-type battalion (any except heavy-wpn para) | box + ✕ | `Infantry` |
| 6 | Heavy weapons parachute infantry battalion | box + **filled ✕** | `Infantry` + `Airborne` + `HeavyWeapons` |
| 7 | Artillery unit (1-SP) | box + dot | `Artillery` |
| 8 | Armored reconnaissance battalion | box + oval/diagonal | `Recon` (armored) |
| 9 | Heavy anti-aircraft unit | box + **filled △** | `AntiAir` + `HeavyWeapons` |
| 10 | Light anti-aircraft unit | box + open △ | `AntiAir` |
| 11 | Engineer battalion | box + `m` | `Engineer` |
| 12 | Engineer company | box + `m` (coy echelon) | `Engineer` |

### Verified against German tokens (PDF p183, counter section Nr. 4)
All German branch glyphs confirmed: Armor (oval), Infantry (✕), armored-recce = **oval + diagonal**
("3" Aufklärungs), AntiTank = **open △** ("39"), **TankDestroyer = dome** ("605" SP Panzerjäger — the
only SP one; retagged `[TankDestroyer]`), heavy AA = **filled △** ("606") vs light AA = **open △**
("I/33"), Artillery (dot), Engineer (`m`). German **Machinegewehr** bns ("2"/"8") are drawn as **plain
infantry ✕** (no MG bar) → `[Infantry]`, as tagged.

### Reconciliation notes
- **`TankDestroyer`** (dome) is a distinct branch (German 605th SP Panzerjäger; CW tank-destroyer regt);
  towed Panzerjäger/Pak stay `AntiTank` (△). `TankDestroyer` still counts as anti-tank for §19.3.
- The **filled vs open** convention (`HeavyWeapons`/heavy) is a general modifier: it distinguishes
  heavy vs light AA and marks heavy-weapons parachute infantry. Consider a `Heavy` modifier bit.
- German **`Recon`** (armored recce) reads as armor-oval + diagonal — same family as the Italian
  `ArmoredCar`/`Recon`; decide whether `ArmoredCar` = `Armor` + `Recon`.
- The `(2)[⊡]` "artillery HQ group" is a **§19.3 slot**, not a token glyph — it belongs in the
  organization chart, not the per-unit type set.

## §19.31 Allied / Commonwealth (PDF p135, bottom legend)

The division/brigade/support-group symbols in the left column (`(5)[oval XX]` armored division,
`(2)[oval X]` armored brigade, `(5)[⊠ XX]` infantry division, `(2)[⊠ X]` support group / motor
brigade group, `(3)[⊠ X]`/`(2)[⊠ X]`/`(2)[⊠ X]A` infantry brigades) are **formation types** for the
§19.3 chart, not per-token unit symbols — they're already in the org chart. The per-unit glyphs:

### New/changed primitives vs. IT/DE
| Primitive | Drawn as | Notes |
|---|---|---|
| `ArmoredCar` | a **car-oval** (rounded, flat) | **distinct** from `Recon` — CW draws both |
| `Recon` | an **oval with a diagonal** (armored recce) | separate symbol from `ArmoredCar` → keep both as branches |
| `AnyUnit` | an **empty box** (no interior symbol) | "any non-tank-bn counter of stacking value 1 or 0" |

CW has only **light** AA in the legend (`AntiAir`); heavy AA appears only via the combined AT/AA
regiment. `MachineGun` = ✕ + a small vertical arrow (as Italian). `SelfPropelled` (Armor+branch) reappears
(SP artillery regiment).

### Enumerated Commonwealth per-unit types
| # | Label | Glyph (as drawn) | Decomposition |
|---|---|---|---|
| 1 | Tank battalion | box + oval | `Armor` |
| 2a | Infantry battalion (leg) | box + ✕ | `Infantry` |
| 2b | Motorized infantry battalion | box + ✕ + wheels at lower **corners/sides** | `Infantry` + `Motorized` |
| 2c | Marine infantry battalion | box + ✕ + a bottom mark (anchor/bar — exact form unclear) | `Infantry` + `Marine` |
| 2d | Commando infantry battalion | box + ✕ + a corner mark (exact form unclear) | `Infantry` + `Commando` |
| 3 | Machinegun infantry battalion | box + ✕ with a small vertical arrow | `Infantry` + `MachineGun` |
| 4 | Artillery — regiment / battalion | box + dot | `Artillery` |
| 5 | Self-propelled artillery regiment | box + oval + dot | `Armor` + `Artillery` (`SelfPropelled`) |
| 6 | Anti-tank regiment / unit | box + △ | `AntiTank` |
| 7 | Anti-tank / anti-aircraft regiment | box + △ (combined AT+AA mark) | `AntiTank` + `AntiAir` |
| 8 | Tank destroyer regiment | box + dome/arch | `TankDestroyer` |
| 9 | Light anti-aircraft unit (regt / bn) | box + open △ | `AntiAir` |
| 10 | Armored car unit (regt / bn) | box + car-oval | `ArmoredCar` |
| 11 | Armored reconnaissance unit (regt / bn) | box + oval/diagonal | `Recon` |
| 12 | Engineer battalion (Pioneer or Scorpion-tank) | box + `m` | `Engineer` |
| 13 | "Unit" (any 1/0-SP non-tank counter) | **empty box** | `AnyUnit` |

### Verified against CW tokens (PDF p184, NZ + Misc Allies)
- **MachineGun** = infantry ✕ **+ a small vertical (up) arrow** over the centre ("27 NZ" MG bn).
- **2nd NZ Cavalry** ("2NZ Cv") = a plain **armor oval** (no diagonal) → retagged `[Armor]` (was Recon).
- Anti-tank ("7 NZ") = triangle **+ dot**; light AA ("14NZ Lt") = **open △**; artillery ("4/5/6 NZ Fld") = dot;
  Maori/infantry = ✕; Foreign Legion / Marche (French) = plain ✕. All match.
- **Commando** not present on this Allies sheet (Free French / Greek / Polish / Czech only) — still needs a
  British counter to confirm.

### Verified against CW tokens (PDF p185, counter sections Nr. 7 & 8 — British / Indian / South African / Australian)
- **`ArmoredCar` vs `Recon` confirmed distinct**: the three **South African "Armored Car"** units
  ("3 SA" bn, "4 SA"/"6 SA" regt) are drawn as the **car-body-on-wheels** (rounded body over three
  wheels) → retagged `[Recon]` → **`[ArmoredCar]`**. The **"7 SA" Recce Bn** and every British/Indian/
  Aus recce unit (`11 Hus`, `12Lancr`, Royals, Dragoons, Guides, Lancers, DCO, Aus Cav, 44/50/51 Recce)
  are drawn as the **oval + diagonal** → `[Recon]` (correct, unchanged).
- **Commando** = a **slightly smaller ✕** than infantry (arms stop short of the box corners) with a
  small **arrowhead atop each upper arm**. The **SAS Bde** and **"A" Bn, Special Service Bde** →
  retagged `[Infantry]` → **`[Infantry, Commando]`**. Viewer draws the smaller ✕ + upper arrowheads.
- Counters printed on p185 but **not in our (campaign) OB** — noted, not tag issues: `KDGd` (King's
  Dragoon Guards, drawn as armored-car), `12Lancr` (recon), `2 Bays`/`10 Hus`/`9 Lancr` (armor),
  `76`, `2 RHA`. These belong to non-campaign scenarios.

### Reconciliation notes
- **`ArmoredCar` and `Recon` are distinct branches** (CW draws two different symbols) — resolves the
  earlier open question: do **not** model `ArmoredCar` as `Armor`+`Recon`.
- **Combined AT/AA regiment** = `AntiTank` + `AntiAir` — another genuine two-branch combination.
- **`TankDestroyer`** (dome) confirmed in CW as well as German — keep it a distinct branch.
- **`SelfPropelled`** (Armor + branch) confirmed across IT + CW.
- Infantry **styles** each carry a subtle drawn mark (motorized = side wheels; marine = bottom mark;
  commando = corner mark) — not plain ✕, though the marine/commando marks are hard to read at scan
  resolution (to verify on the tokens). Marine = the 3 French naval-infantry units; Commando = the
  "A" Bn Special Service Bde + SAS Bde.

---

## Data model: `UnitType` leaves → primitives

Units are tagged in the OB/scenario data with a single **leaf** `UnitType` (the most-specific counter
type, e.g. `MarineInfantry`, `BersaglieriMotorcycleInfantry`, `SelfPropelledArtillery`,
`RoadConstruction`). Two helpers generalize it:
- `UnitType::arm() -> OrgFamily` — the §19.3 formation-chart family (the only rules-facing use).
- `UnitType::glyph() -> Vec<Primitive>` — the render primitives the viewer composes (below).

`arm()` is defined independently of `glyph()`, so a leaf can roll up one way and draw another — e.g.
`BersaglieriMotorcycleInfantry` has arm `Bersaglieri` but draws `[Infantry, Motorcycle]` (center
wheels, no Bersaglieri side-wheels). The `Primitive`s below are now purely the **rendering
vocabulary**; the leaf is the authoritative tag.

## Combined primitive vocabulary (IT + DE + CW)

**Base branches:** `Infantry` · `Armor` · `Artillery` · `AntiTank` · `TankDestroyer` · `AntiAir` ·
`Cavalry` · `ArmoredCar` · `Recon` · `MachineGun` · `Engineer` · `AnyUnit` (empty box) ·
`Hq` · `Tds` (roles).

**Modifiers — all drawn (each adds a mark):**
- `Motorized` — two wheels at the lower **corners/sides** of the symbol
- `Motorcycle` — wheels clustered **under the center**
- `Airborne` — gull/arc beneath (or the symbol drawn **filled** for heavy-weapons parachute)
- `SelfPropelled` — the branch symbol drawn inside the **armor oval**
- `Heavy` — the symbol drawn **filled/solid** (heavy AA, heavy-weapons)
- `Bersaglieri` — a **"Brs"** tag below the box
- `Camel` — a **"c"** by the diagonal
- `Marine` — an **anchor** overlaid on the infantry ✕ (confirmed on IT p182)
- `Commando` — a **smaller ✕** (short of the box corners) with an **arrowhead atop each upper arm**
  (confirmed on p185: SAS Bde, "A" Bn Special Service Bde)

(Leg infantry / plain artillery etc. are the un-modified base symbols.)

**Confirmed combinations:** SP artillery = `Armor`+`Artillery`; AT/AA regiment = `AntiTank`+`AntiAir`;
heavy-weapons parachute infantry = `Infantry`+`Airborne`+`Heavy`; Bersaglieri motorized = `Infantry`+
`Bersaglieri`+`Motorized`; camel cavalry = `Cavalry`+`Camel`. → A **set/mask of primitives** fits.

