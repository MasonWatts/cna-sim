# Off-Map Tripoli/Tunisia Land-Movement Complex (Rules §8.8)

Human-readable transcription. Authoritative machine values: `data/reference/off-map-nodes.ron`.

**Verification status:** §8.89 chart numbers confirmed by the project owner (2026-07-14);
node attributes (AA points, ports, garrison) read from Map A.

## What it is (§8.81)
The western edge of game-map section A holds four large **boxes** — regions — plus small
**In-Transit** boxes. As a group they are the "Tripoli/Tunisia boxes." Chain, map-side → inland:

```
[map: A2802 / Nofilia A2703] — Tripolitania — Tripoli — Gabes — Tunis
```

## Key rules
- **§8.82 — Axis only.** Only Axis land units (and Commonwealth *aircraft*) may enter. No
  Commonwealth land/sea unit ever enters.
- **§8.83–8.84 — movement is abstracted into Operations Stages.** Distance is the number of
  Operations Stages a unit must spend expending its **full CPA** moving. Stages-per-leg depend
  on the unit's CPA (see §8.89). Cover a leg in one stage → placed directly in the next box;
  otherwise advance **one In-Transit box per stage** until done. An In-Transit unit need not
  move every stage and **may reverse** back toward its start. Motorized units consume fuel but
  **skip breakdown checks** here.
- **§8.85 — map junction = A2802.** A unit must *start* the stage in A2802 to move off-map,
  and enters the map at the road hex closest to the Tripolitania box. On entry, forces string
  out along the coast road as each hex fills past its stacking limit. The verified on-map road
  order (project owner, from Map A) is:

  `A2802 → A2703 (Nofilia) → A2704 → A2604 → A2505 → A2405`

  Note: §8.85's illustrative example strings "5 pts in 2802, 5 in 2803, 3 in 2804", but in the
  digitized Map A **A2803/A2804 are Sea** — the rulebook's example hex numbers don't match our
  RRCC coordinates, so the actual coast-road hexes above are authoritative over the "e.g.".
- **§8.86 — arrival lock.** Reinforcements/replacements arriving in Tunis or Tripoli may not
  leave that box the stage they arrive.
- **§8.87–8.88 — logistics.** No stacking limits, unlimited Water; the boxes hold air
  facilities + supplies + maintenance for all Axis planes and are unlimited-capacity Supply
  Dumps (a truck may not move the stage it loads/unloads).
- **§20.15 — Axis reinforcements arrive at Tripoli** (Benghazi A4827 diversion under conditions).

## §8.89 Off-Map Land Unit Movement Distance Chart
Each cell = Operations Stages of continuous movement, for **CPA ≥25 / CPA 15–20 / CPA <15**.
A unit may not exceed its Basic CPA during off-map movement.

| from ↓ \ to → | Tunis | Gabes | Tripoli | Tripolitania |
|---|---|---|---|---|
| Gabes        | 1/2/5  | –     |        |     |
| Tripoli      | 2/3/8  | 1/1/3 | –      |     |
| Tripolitania | 3/4/11 | 2/2/6 | 1/1/3  | –   |
| Nofilia (A2703) | 4/5/14 | 3/3/9 | 2/2/6 | 1/1/3 |

The matrix is cumulative sums of the **adjacent legs**, so the model only needs the four legs:

| leg | CPA ≥25 | CPA 15–20 | CPA <15 |
|---|---|---|---|
| Tripolitania ↔ Nofilia (map) | 1 | 1 | 3 |
| Tripoli ↔ Tripolitania | 1 | 1 | 3 |
| Gabes ↔ Tripoli | 1 | 1 | 3 |
| Tunis ↔ Gabes | 1 | 2 | 5 |

**CPA-tier note:** the chart states tiers as "25 or more / 15 to 20 / less than 15"; CPA
21–24 is not addressed. We interpret the middle tier as CPA 15–24 (i.e. `≥25` → high, `≥15` →
mid, else low). Revisit if a 21–24 CPA unit appears.

## Node attributes (from Map A)
| node | AA points | port | garrison |
|---|---|---|---|
| Tunis | 15 | Bizerta | — |
| Gabes | 10 | — | — |
| Tripoli | 26 | (unnamed) | Axis |
| Tripolitania | 10 | — | — |

AA points are the air-defense value (used by the §33+ Air Game — recorded now, not yet used).
