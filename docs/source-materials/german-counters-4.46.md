# German Combat-Unit Counters — §4.46 Unit-Type Transcription

**Source:** *The Campaign for North Africa* (SPI, 1978), PDF **page 183** — the
German counter sheet **"Counter Section Nr. 4 (200 pieces)"** (khaki/tan),
lower block of the page. Rendered at **400 DPI** via `tools/pdf-page.sh`.

**Method / orientation note:** In this render the page is **already upright**
(the earlier-documented "page 183 renders upside-down + rotate 180°" recipe was
NOT needed here — the raw PNG reads correctly). Full-width horizontal strips
were cropped per row and upscaled 2–5× (PIL LANCZOS), then read cell-by-cell.
Ambiguous glyphs were re-zoomed individually to distinguish AT vs Flak
(dot present/absent) and MG vs engineer.

The German ground counters occupy two column-groups on the Nr. 4 sheet:
a **LEFT group** (10 cols × 10 rows = 100 counters) and a **RIGHT group**
(top 4 rows are markers/air, then 5 rows × 10 = 50 combat counters).
Everything below is on **page 183**; the German counters do **not** spill onto an
adjacent page.

## §4.46 glyph legend as observed on this sheet
| Center glyph | Type |
|---|---|
| horizontal OVAL + bar | Armor / Panzer |
| OVAL + diagonal slash | Reconnaissance / Armored-car (Aufklärung) |
| OVAL enclosing bell/△ (605) | Self-propelled Panzerjäger (Anti-tank) |
| box + plain **X** | Infantry |
| box + **X + diagonal slash** | Motorized infantry (Panzergrenadier/Schützen) |
| box + **X + center bar + side dots** | Machine-gun (MG) |
| box + **dot** | Artillery |
| box + **triangle + dot** (△•) | Anti-tank (Panzerjäger / Pak) |
| box + **plain triangle** (filled ▲ or outline △, NO dot) | Flak / AA (▲ heavy, △ light) |
| box + **"E" / box-in-box comb** | Engineer (Pionier) |
| box + **oval + dot** (408) | Artillery-command variant (ArtKo) |
| "BG"/"XXXX" size + X-box | HQ (corps / Kampfgruppe) |

Notes: quantities are the top-left "(N)" (absent ⇒ 1). Sizes: XX=div, X=bde,
BG=brigade-group HQ, III=regt, II=bn, I=coy, XXXX=army/corps.
"Le" = leichte (Light) Division. "?" marks an uncertain reading.

## Combat-unit table

| Designation | Parent | Size | Type | Qty | Page | Notes |
|---|---|---|---|---|---|---|
| **5. leichte Division** ||||||
| 5 Le | — | XX | Armor (Div HQ) | 5 | 183 | oval |
| 5 | 5 Le | III | Armor (Pz Regt) | 2 | 183 | Pz-Regt 5 |
| I/5 | 5 Le | II | Armor | 1 | 183 | |
| II/5 | 5 Le | II | Armor | 1 | 183 | |
| 2 | 5 Le | II | **Machine-gun** | 1 | 183 | MG-Btl 2 |
| 8 | 5 Le | II | **Machine-gun** | 1 | 183 | MG-Btl 8 |
| 3 | 5 Le | II | **Recce/Armored-car** | 1 | 183 | Aufkl 3 |
| 39 | 5 Le | II | **Anti-tank (Pzjg)** | 1 | 183 | △• PzJg-Abt 39 |
| I/75 | 5 Le | II | **Artillery** | 1 | 183 | |
| 605 | 5 Le | II | **Anti-tank (SP Pzjg)** | 1 | 183 | oval+△ (SP) |
| 606 | 5 Le | II | **Flak/AA** | 1 | 183 | ▲ |
| I/33 | 5 Le | II | **Flak/AA** | 1 | 183 | △ |
| 200 | 5 Le | II | Engineer (Pionier) | 1 | 183 | E-box; Pi-Btl 200 |
| **21. Panzer-Division** ||||||
| 21 | — | XX | Armor (Div HQ) | 5 | 183 | oval |
| **15. Panzer-Division** ||||||
| 15 | — | XX | Armor (Div HQ) | 5 | 183 | oval |
| 8 | 15 | III | Armor (Pz Regt) | 2 | 183 | Pz-Regt 8 |
| I/8 | 15 | II | Armor | 1 | 183 | |
| II/8 | 15 | II | Armor | 1 | 183 | |
| 15 | 15 | X | **Motorized infantry** (Bde) | 3 | 183 | X+slash; Schtz-Bde 15 |
| 200 | 15 | III | **Motorized infantry** | 2 | 183 | X+slash |
| 104 | 15 | III | **Motorized infantry** | 2 | 183 | X+slash |
| 115 | 15 | III | **Motorized infantry** | 2 | 183 | X+slash |
| I/104 | 15 | II | **Motorized infantry** | 1 | 183 | |
| II/104 | 15 | II | **Motorized infantry** | 1 | 183 | |
| I/115 | 15 | II | **Motorized infantry** | 1 | 183 | X+slash |
| II/115 | 15 | II | **Motorized infantry** | 1 | 183 | |
| 15/200 | 15 | II | **Machine-gun (?)** | 1 | 183 | X+dots; poss. Kradschützen |
| 33 | 15 | II | **Recce/Armored-car** | 1 | 183 | oval+slash; Aufkl 33 |
| 33 | 15 | III | **Artillery** | 1 | 183 | dot; Art-Regt 33 |
| 140 | 15 | III | **Artillery** | 1 | 183 | dot |
| I/115 | 15 | II | **Artillery** | 1 | 183 | dot; NOTE dup. desig. w/ mot. I/115 |
| 33 | 15 | II | **Anti-tank (Pzjg)** | 1 | 183 | △• PzJg-Abt 33 |
| 33 | 15 | II | Engineer (Pionier) | 1 | 183 | E-box; Pi-Btl 33 |
| **90. leichte Division** ||||||
| 90 Le | — | XX | Infantry (Div HQ) | 5 | 183 | X-box |
| 361 Af | 90 Le | III | Infantry | 2 | 183 | Inf-Regt 361 (Afrika) |
| 155 | 90 Le | III | Infantry | 2 | 183 | Inf-Regt 155 |
| I/155 | 90 Le | II | Infantry | 1 | 183 | |
| II/155 | 90 Le | II | Infantry | 1 | 183 | |
| III/155 | 90 Le | II | Infantry | 1 | 183 | |
| I/361 | 90 Le | II | Infantry | 1 | 183 | |
| II/361 | 90 Le | II | Infantry | 1 | 183 | |
| III/255 | 90 Le | II | Infantry | 1 | 183 | |
| III/347 | 90 Le | II | Infantry | 1 | 183 | |
| 361/361 | 90 Le | II | **Artillery** | 1 | 183 | dot |
| 190 | 90 Le | III | **Artillery** | 1 | 183 | dot; Art-Regt 190 |
| 580 | 90 Le | II | **Recce/Armored-car** | 1 | 183 | oval+slash; Aufkl 580 |
| 900 | 90 Le | II | Engineer (Pionier) | 1 | 183 | E-box; Pi-Btl 900 |
| 613 | 90 Le | II | **Flak/AA** | 1 | 183 | △ |
| 190 | 90 Le | II | **Flak/AA** | 1 | 183 | ▲; dup. desig. w/ Art 190 |
| **164. leichte Division** ||||||
| 164 Le | — | XX | Infantry (Div HQ) | 5 | 183 | X-box |
| 125 | 164 Le | III | **Motorized infantry** | 2 | 183 | X+slash |
| 382 | 164 Le | III | **Motorized infantry** | 2 | 183 | X+slash |
| 433 | 164 Le | III | **Motorized infantry** | 2 | 183 | X+slash |
| I/125 | 164 Le | II | Infantry | 1 | 183 | |
| II/125 | 164 Le | II | Infantry | 1 | 183 | |
| III/125 | 164 Le | II | Infantry | 1 | 183 | |
| I/382 | 164 Le | II | Infantry | 1 | 183 | |
| II/382 | 164 Le | II | Infantry | 1 | 183 | |
| III/382 | 164 Le | II | Infantry | 1 | 183 | |
| I/433 | 164 Le | II | **Motorized infantry** | 1 | 183 | X+slash |
| II/433 | 164 Le | II | **Motorized infantry** | 1 | 183 | X+slash |
| III/433 | 164 Le | II | **Motorized infantry** | 1 | 183 | X+slash |
| 164 | 164 Le | II | **Recce/Armored-car** | 1 | 183 | oval+slash; Aufkl 164 |
| 220 | 164 Le | III | **Artillery** | 2 | 183 | dot; Art-Regt 220 |
| I/220 | 164 Le | II | **Artillery** | 1 | 183 | dot |
| II/220 | 164 Le | II | **Artillery** | 1 | 183 | dot |
| III/220 | 164 Le | II | **Artillery** | 1 | 183 | dot |
| IV/220 | 164 Le | II | **Artillery** | 1 | 183 | dot |
| 220 | 164 Le | II | Engineer (Pionier) | 1 | 183 | E-box; Pi-Btl 220 |
| 609 | 164 Le | II | **Flak/AA** | 1 | 183 | △ |
| **Sonderverband 288** ||||||
| 288 Son | — | III | Infantry | 2 | 183 | X-box |
| 2 | 288 Son III | II | **Motorized infantry** | 1 | 183 | X+slash |
| 3 | 288 Son III | II | **Motorized infantry** | 1 | 183 | X+slash |
| 4 | 288 Son III | II | **Motorized infantry** | 1 | 183 | X+slash |
| 288 | 288 Son III | II | **Anti-tank (Pzjg)** | 1 | 183 | △• |
| 288 | 288 Son III | I | Engineer (Pionier) | 0 | 183 | E-box; Pi 288 |
| **Ramcke Brigade** ||||||
| Ramcke | — | X | Infantry (Bde HQ) | 3 | 183 | X-box |
| I Kroh | Ramcke | II | Infantry | 1 | 183 | |
| II vdH | Ramcke | II | Infantry | 1 | 183 | |
| III Hub | Ramcke | II | Infantry | 1 | 183 | |
| IV Sch | Ramcke | II | Infantry | 1 | 183 | |
| V Brc | Ramcke | II | Infantry | 1 | 183 | |
| **Oasen (300 oas) companies** ||||||
| 1 | 300 oas | I | Infantry | 0 | 183 | |
| 2 | 300 oas | I | Infantry | 0 | 183 | |
| 3 | 300 oas | I | Infantry | 0 | 183 | |
| 4 | 300 oas | I | Infantry | 0 | 183 | |
| 5 | 300 oas | I | Infantry | 0 | 183 | |
| 6 | 300 oas | I | Infantry | 0 | 183 | |
| 7 | 300 oas | I | Infantry | 0 | 183 | |
| 8 | 300 oas | I | Infantry | 0 | 183 | |
| 9 | 300 oas | I | Infantry | 0 | 183 | |
| 10 | 300 oas | I | Infantry | 0 | 183 | |
| 11 | 300 oas | I | Infantry | 0 | 183 | |
| 12 | 300 oas | I | Infantry | 0 | 183 | |
| 13 | 300 oas | I | Infantry | 0 | 183 | |
| **Artillerie-Kommandeur (ArtKo) — Artillery Brigade/Command HQs** ||||||
| ArKo104 | — | III | **Artillery-Bde-HQ (ArtKo)** | 2 | 183 | box+dot |
| 408 | ArKo | III | **Artillery-Bde-HQ (ArtKo)** | 1 | 183 | oval+dot variant |
| 362 | ArKo | II | **Artillery-Bde-HQ (ArtKo)** | 0 | 183 | box+dot |
| 149 | ArKo | II | **Artillery-Bde-HQ (ArtKo)** | 1 | 183 | box+dot |
| 528 | ArKo | II | **Artillery-Bde-HQ (ArtKo)** | 1 | 183 | box+dot |
| **DAK HQ & Kampfgruppen (right group, row 5)** ||||||
| Rommel | DAK | XXXX | Infantry (Corps HQ) | 0 | 183 | X-box; Deutsches Afrikakorps |
| Pfeiffer | — | BG | Infantry (KG HQ) | 2 | 183 | X-box |
| Schwrn | — | BG | Infantry (KG HQ) | 2 | 183 | Schwerin |
| Voss | — | BG | Infantry (KG HQ) | 2 | 183 | |
| Marcks | — | BG | Infantry (KG HQ) | 2 | 183 | |
| Knabe | — | BG | Infantry (KG HQ) | 2 | 183 | |
| Bach | — | BG | Infantry (KG HQ) | 2 | 183 | |
| Graf | — | BG | Infantry (KG HQ) | 2 | 183 | |
| 707 | — | I | Infantry | 0 | 183 | |
| 708 | — | I | Infantry | 0 | 183 | |
| 13/Bdbg | — | I | Infantry | 0 | 183 | Brandenburg |
| **Independent engineer** ||||||
| 778/Nvl(?) | — | II | Engineer (Pionier) | 0 | 183 | E-box; suffix unclear |
| **Independent / Army artillery** ||||||
| 155 | — | III | **Artillery** | 2 | 183 | dot |
| 1 Afr | — | III | **Artillery** | 2 | 183 | dot |
| 2 Afr | — | III | **Artillery** | 2 | 183 | dot |
| I | 155 III | II | **Artillery** | 1 | 183 | dot |
| II | 155 III | II | **Artillery** | 1 | 183 | dot |
| III | 155 III | II | **Artillery** | 1 | 183 | dot |
| 557 | — | II | **Artillery** | 1 | 183 | dot |
| 1/902 | — | II | **Artillery** | 0 | 183 | dot |
| I/772 | — | I | **Artillery** | 0 | 183 | dot |
| 364 | — | I | **Artillery** | 0 | 183 | dot |
| 523 | — | II | **Artillery** | 1 | 183 | dot |
| 529 | — | II | **Artillery** | 1 | 183 | dot |
| 532 | — | II | **Artillery** | 1 | 183 | dot |
| 533 | — | II | **Artillery** | 1 | 183 | dot |
| 526 | — | II | **Artillery** | 1 | 183 | dot |
| **Independent Flak/AA (right group rows 7–9)** ||||||
| I/18 | — | II | **Flak/AA** | 1 | 183 | ▲ |
| I/33 | — | II | **Flak/AA** | 1 | 183 | △ |
| II/25 | — | II | **Flak/AA** | 1 | 183 | ▲ |
| 612 | — | II | **Flak/AA** | 1 | 183 | ▲ |
| I/53 | — | II | **Flak/AA** | 1 | 183 | △ |
| I/6 | — | II | **Flak/AA** | 1 | 183 | ▲ |
| I/43 | — | II | **Flak/AA** | 1 | 183 | △ |
| 114 | — | II | **Flak/AA** | 0 | 183 | ▲ |
| 192 | — | II | **Flak/AA** | 1 | 183 | △ |
| 368 | — | II | **Flak/AA** | 1 | 183 | △ |
| 442 | — | II | **Flak/AA** | 1 | 183 | △ |
| 617 | — | II | **Flak/AA** | 1 | 183 | △ |
| 841Lt | — | II | **Flak/AA (light)** | 1 | 183 | △ |
| II/5 | — | II | **Flak/AA** | 1 | 183 | ▲ |
| II/12 | — | II | **Flak/AA** | 1 | 183 | △ |
| II/46 | — | II | **Flak/AA** | 1 | 183 | △ |
| I/54 | — | II | **Flak/AA** | 1 | 183 | △ |
| 30 | — | II | **Flak/AA** | 0 | 183 | ▲ |
| 211 | — | II | **Flak/AA** | 1 | 183 | △ |
| 243 | — | II | **Flak/AA** | 1 | 183 | △ |
| 329 | — | II | **Flak/AA** | 1 | 183 | △ |
| 358 | — | II | **Flak/AA** | 1 | 183 | △ |
| 860 | — | II | **Flak/AA** | 1 | 183 | △ |

## Non-combat counters on the same sheet (NOT transcribed above)
- **Markers (right group, row 1):** TRANSPORT ×2, DSKA, TANK RECOVERY,
  tank-ID markers A/B/C/D, INF REPL A/B (+ INF REPL C/D in row 2). ~10–12 pieces.
- **SGSU air-unit strength markers (right group rows 2–4, value 25):** Luftwaffe
  squadron step markers — JG 27/53/77/26, NJG 2, ZG 26/1, StG.1/2/3, SG 2,
  KG 100/54, KLG 1/26/77, recon staffeln 2.(H)/4.(H)/1(F), etc. ~28 pieces.
  These carry no §4.46 ground unit-type glyph.
- **German AIRCRAFT counters** appear in the adjacent **Counter Section Nr. 3**
  (upper block of page 183, labelled "GERMAN"): generic FIGHTER, DIVE BOMBER,
  BOMBER, FLYING BOAT type counters (no unit designations). Air units, outside
  the §4.46 ground-symbol scope.

## Totals
- **150 German ground combat-unit counters** transcribed, all on **page 183**
  (100 in the LEFT column-group, 50 in the RIGHT column-group of Section Nr. 4).
- No German counters continue onto an adjacent page.
