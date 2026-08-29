# 62 Marmarica & 63 Cirene — template model (abstracted in §4.44b)

These two Italian binary infantry divisions are **deployed in the September 1940 setup**
(§60.31: `63 Cirene Div` at C4120, `62 Marmarica Div` at C3918, each `(I)` = infantry) and
are present at the start of the **Campaign Game** too (both begin Sept 1940). They have **no
dedicated §4.44b OA sheet** — the Italian OB deliberately abstracts/"melds" units the
designers had scarce records for (p147 notes: many were "captured in entirety or completely
destroyed … records are scarce", the Italian OB is "the least accurate historically"). So
this is an OB coverage gap, **not** a timeline issue.

## Model: the standard Italian semi-motorized division template
They were structurally identical to Catanzaro/Sabratha/Pistoia (which share one template).
Model each as (mirroring the §4.44b **64th Catanzaro** sheet):

- Div HQ — ID `b`, TOE `N`
- **2 Infantry Regiments** — ID `h`, `1 x 65/17 Gun` each — with **3 Infantry Battalions**
  each (standard infantry, TOE `N`)
- Machinegun Bn — ID `bb`, `N`
- Anti-tank Coy — `2 x 47/32 Gun`
- Artillery Regiment — ID `kk` (≈ `6 x 75/27 Gun, 3 x 100/17 Howitzer, 3 x Heavy AA`)
- Engineer Bn — ID `bbb`, `N`
- **Attached** (Libyan Tank Command, per §60.31 + p149 note): Marmarica ← `LXII(L)` tank Bn;
  Cirene ← `LXIII(L)` tank Bn.

Morale: no sheet value; use **-1** (matches the other semi-motorized divisions). `[flag]`

## Regiment designations (historical — VERIFY, not from the game chart)
Marked `(?)` — the ruleset does not name these; supplied for readability and open to
correction:
- **62 Marmarica:** 115th & 116th Infantry Regts (?), 44th Artillery Regt (?)
- **63 Cirene:** 157th & 158th Infantry Regts (?), 45th Artillery Regt (?)

(If preferred, use generic designations — "Marmarica 1st/2nd Infantry Regt" — since the game
abstracts them.)

## Encoding flag
In the scenario/OB data, mark both divisions: `abstracted_in_444b: true` — modeled on the
standard semi-motorized template, not a transcribed OA sheet.
