# CNA Addenda (September 1979) — Transcription

**Source:** `docs/source-materials/CNA Addenda Sep 79.pdf` (4 pages, scanned image, no text layer — read from
rendered page images at 200 dpi, 2026-09-16).

The addenda's headline OB fix (§[4.44b] item 1): *"The OA sheets for three entire Italian Divisions —
Sirte, Cirene and Marmarica — have been left out. See pages 3 and 4 of this booklet. If you are not
planning on playing any scenario before February 1941 you can ignore these divisions, as they were gone
by then."* Those three sheets are transcribed below; all three are now chart-backed in `data/ob/italian.ron`
(previously template-abstracted with placeholder data).

Column key matches the §4.44 OA sheets: Abbr = counter abbreviation; ID = characteristics code; Arrives
"n/mm" = Ops Stage n, Game-Turn mm; "D" = Deployed.

## 61st "Sirte" Division (Semi-motorized) — Basic Morale: −1 — Addenda p3
| Unit | Abbr | ID | TOE & Weapons | Arrives |
|---|---|---|---|---|
| 61st Sirte Div HQ | 61 Srt | b | N | 3/10 |
| 69th Infantry Regt HQ | 69 | h | 1×65/17 Gun | 3/10 |
| I/69 Infantry Bn | I/69 | u | N | 3/10 |
| II/69 Infantry Bn | II/69 | u | N | 3/10 |
| III/69 Infantry Bn | III/69 | u | N | 3/10 |
| 70th Infantry Regt HQ | 70 | h | 1×65/17 Gun | 3/10 |
| I/70 Infantry Bn | I/70 | u | N | 3/10 |
| II/70 Infantry Bn | II/70 | u | N | 3/10 |
| III/70 Infantry Bn | III/70 | u | N | 3/10 |
| 61st Machinegun Bn | 61 | bb | N | 3/10 |
| 61st Anti-tank Coy | 61 | rr | 2×47/32 Gun | 3/10 |
| 43rd Artillery Regt | 43 | kk | 6×75/27 Gun, 3×100/17 Howitzer, 6×Light AA | 3/10 |
| 61st Engineer Bn | 61 | bbb | N | 3/10 |
| 61st Bersaglieri Mitrg Coy | 61 Brs | aaa | N | 3/10 |

(No attached tank battalion on the Sirte sheet.)

## 62nd "Marmarica" Division (Semi-motorized) — Basic Morale: −1 — Addenda p3
| Unit | Abbr | ID | TOE & Weapons | Arrives |
|---|---|---|---|---|
| 62nd Marmarica Div HQ | 62 Mrm | b | N | D |
| 115th Infantry Regt HQ | 115 | h | 1×65/17 Gun | D |
| I/115 Infantry Bn | I/115 | u | N | D |
| II/115 Infantry Bn | II/115 | u | N | D |
| III/115 Infantry Bn | III/115 | u | N | D |
| 116th Infantry Regt HQ | 116 | h | 1×65/17 Gun | D |
| I/116 Infantry Bn | I/116 | u | N | D |
| II/116 Infantry Bn | II/116 | u | N | D |
| III/116 Infantry Bn | III/116 | u | N | D |
| 62nd Machinegun Bn | 62 | bb | N | D |
| 62nd Anti-tank Coy | 62 | rr | 2×47/32 Gun | D |
| 44th Artillery Regt | 44 | kk | 6×75/27 Gun, 3×105/28 Gun, 3×Light AA | D |
| 62nd Engineer Bn | 62 | bbb | N | D |
| LXII(L) Tank Bn (note a) | LXII(L) | — | | D |

Note a = The LXII(L) tank battalion is officially part of the Libyan Tank Command; it begins the campaign
attached to the 62nd Marmarica. (Modeled under Libyan Tank Command in the OB.)

## 63rd "Cirene" Division (Semi-motorized) — Basic Morale: −1 — Addenda p4
| Unit | Abbr | ID | TOE & Weapons | Arrives |
|---|---|---|---|---|
| 63rd Cirene Div HQ | 63 Cir | b | N | D |
| 157th Infantry Regt HQ | 157 | h | 1×65/17 Gun | D |
| I/157 Infantry Bn | I/157 | u | N | D |
| II/157 Infantry Bn | II/157 | u | N | D |
| III/157 Infantry Bn | III/157 | u | N | D |
| 158th Infantry Regt HQ | 158 | h | 1×65/17 Gun | D |
| I/158 Infantry Bn | I/158 | u | N | D |
| II/158 Infantry Bn | II/158 | u | N | D |
| III/158 Infantry Bn | III/158 | u | N | D |
| 63rd Machinegun Bn | 63 | bb | N | D |
| 63rd Anti-tank Coy | 63 | rr | 2×47/32 Gun | D |
| 45th Artillery Regt | 45 | kk | 6×75/18 Gun-Howitzer, 3×100/17 Howitzer, 3×Light AA | D |
| 63rd Engineer Bn | 63 | bbb | N | D |
| 63rd Bersaglieri Mitrg Coy | 63 Brs | aaa | N | D |
| LXIII(L) Tank Bn (note a) | LXIII(L) | — | | D |

Note a = The LXIII(L) tank battalion is officially part of the Libyan Tank Command; it begins the campaign
attached to the 63rd Cirene. (Modeled under Libyan Tank Command in the OB.)

---

## Other addenda OB/schedule corrections (NOT yet integrated — backlog)

Recorded from addenda p1–p2/p4 for a future pass; several touch units added during the p116–166 OB
completeness work:

- **§[4.43a] reinforcement-schedule omissions (CW):** add `28/2  42d RTR (1 Army); Trucks: 4M` and
  `64/2  WD: 32nd Army Tank Bde (2); Tpt: 20/15`. Also: *"in 76/2 there is a reference to 2/68; ignore it —
  refer instead to the newly added 64/2 WD."* → **Affects units just added:** 42 RTR (1st Army Tank Bde)
  arrival, and the 2nd Armd Div "32 Army" withdrawal (64/2). Reconcile against current OB arrivals.
- **§[4.44a]:** 1st Buffs and 1st Hampshires (CW) start assigned to the Matruh Garrison.
- **§[4.44a]:** Fuel Consumption rating of the Gladiator is "1".
- **§[4.44b] item 3:** French Motor Marine Company arrival should be "D".
- **§[4.44b] item 6:** German Non-Divisional 362 Artillery Battery ID Code should be "x" (not "w", which
  is anti-tank equipment).
- **§[4.44c]:** Bf.109E has no "D" (Deployed) capability; Bf.110 fighter Maneuver = 32 (only on Night
  missions; otherwise 30).
- **CW Air Characteristics legend:** "F" = Reconnaissance should be **"R" = Reconnaissance**.
- **§[61.38]:** unit listed as "ARTR" should be 4RTR. **§[61.41]:** German Mobile Tank Recovery Squadron
  starts in Tripolitania. **§[62.31/63.31]:** deploy the three CW Tank Delivery Squadrons in Cairo.
  **§[62.41/63.41]:** deploy the German Mobile Tank Recovery Squadron in Benghazi. **§[62.33]:** the 142
  "Blenheim IV F's" should be Blenheim IV's (not F's). **§[63.3]:** the Allies get two airfields; Degheila
  gets only 1 SGSU.
- Numerous pure rules-text clarifications/corrections (§3.x, §8.x, §11.33, §12.44, §14.47/48, §15.x,
  §16.11, §17.28, §19.5, §20.3, §56.x, §59.2, §60.x) — not OB data; see the addenda p1–p2/p4 images.
