# Hexside (edge) features — §8.35 / §8.37

Edges carry terrain overlays: Wadi, Ridge, Slope, Escarpment, Major/Minor River, Road, Track,
Railroad, Lake, plus UnfinishedRoad/UnfinishedRailroad (§24.5/§24.6). An edge may carry several
features at once (a Road alongside a Railroad, a Road bridging a River).

## Directional kinds: Slope & Escarpment (§8.35)
Slopes and Escarpments have an **"up"** side and a **"down"** side. On the printed map the
**splash contours (the "teeth"/stem) are drawn on the DOWN side** — they point away from the high
ground; the plain edge line is the up (high) side. A single-hex plateau is therefore ringed by
escarpments whose contour teeth point **outward, away from the plateau**.

Ridges have no direction (a two-sided slope, §8.35) — same effect from either side; no "up".

Movement/combat use direction:
- §8.42 a vehicle may never cross an escarpment hexside in the **up** direction (a Track lets it
  cross **down** only).
- Combat: assaulting **down** a slope/escarpment vs **up** applies different shifts (§15.36).

## Data model
`hexsides: [(a, b, kind, up?)]` in the section RON. `up` (`Some("RRCC")`) is the **high** hex,
required only for Slope/Escarpment. The engine stores `Hexside.high_side = up`; `resolve(from,to)`
yields Up/DownSlope or Up/DownEscarpment by comparing the travel direction to `high_side`.

## Editor convention (tools/map_editor.html)
To mirror the printed map: in Edge mode, **click on the contour (down) side** of the border — the
editor draws the stem there and records `up` as the opposite (high) hex.

## The Cyrenaican railway is historical-only (§8.71)
Only ONE rail line is functional: the Alexandria–Mersa Matruh (→Tobruk) coastal line (Commonwealth,
extendable by construction). The **Soluch–Benghazi–Barce line** (both the Benghazi–Soluch and
Benghazi–Barce arms, on Maps A/B) "is on the game-map solely for historical purposes; it may not be
used for anything and has no effect whatsoever on terrain or movement." → **Do not mark it as any
hexside** (not even UnfinishedRailroad). §54.41: the Barce railroad east of Benghazi is never usable.
