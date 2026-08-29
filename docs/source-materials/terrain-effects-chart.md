# Terrain Effects Chart (Rules §8.37)

Human-readable transcription of the CNA Terrain Effects Chart. Authoritative machine
values live in `data/reference/terrain.ron`. This file is the readable companion and
provenance record (see design.md §9).

**Column-shift notation:** `L#` = shift # columns left (favors defender); `R#` = shift #
columns right (favors attacker); `P` = prohibited; `—` = no effect / blank.
**Movement modes:** non-Mot = non-motorized; Mot = motorized. Costs are Capability Points.

## Area (hex) terrain

| Terrain | non-Mot | Mot | Breakdown | Barrage | Anti-Armor | Close Assault | Stacking |
|---|---|---|---|---|---|---|---|
| Clear | 2 | 2 | 4 | — | — | — | 6 |
| Sand/Gravel | 2 | 2 | 6 | — | — | — | 6 |
| Salt Marsh ² | 3 | 2 | 6 | — | — | R1 | 6 |
| Heavy Vegetation | 3 | 3 | 3 | — | L1 | L1 | 6 |
| Rough | 3 | 4 | 8 | L1 | L1 | L2 | 6 |
| Mountain | 4 | 6 | 12 | L2 | L2 | L3 | 3 |
| Delta | 2 | 4 | 2 | — | — | — | 6 |
| Desert | 3 | 4 ³ | 24 | — | — | — | 6 |
| Major City | 1 | 0.5 | 0.5 | *see Fortifications* | | | 8 |
| Swamp | *enter only on road/railroad* | | | — | — | — | 6 |
| Village / Bir / Oasis | *same as terrain in hex for all purposes* | | | | | | |
| Railroads | *same as terrain in hex for all purposes* | | | | | | |
| Road | 1 ⁶ | 0.5 ⁶ | 0.5 ⁶ | — | — | — | 5 ⁷ |
| Track | 1 ⁸ | 1 ⁸ | — ⁸ | — | — | — | 5 ⁷ |

All cells confirmed by the project owner (2026-07-06). Major City combat adjustment is
"See Fortifications": it uses the city's fortification level (Alexandria/Cairo = L3,
others = L2; footnotes 4 & 12).

## Hexsides (cost is *added* to crossing)

| Hexside | non-Mot | Mot | Breakdown | Anti-Armor | Close Assault |
|---|---|---|---|---|---|
| Ridge | +2 | +4 | — | L2 | L2 |
| Up Slope | +2 | +4 | — | L1 | L2 |
| Down Slope | +1 | +2 | 2 | L1 | R1 |
| Up Escarpment | +6 | P | — | P | L3 |
| Down Escarpment | +4 | +8 ⁹ | +6 | L2 | R1 |
| Wadi | +1 ¹⁰ | +4 ¹⁰ | +8 | — | L1 |
| Major River | +8 | P | — | — | L6 ¹¹ |
| Minor River | +3 | +6 | +1 | — | L2 |

Road/Track (listed under area terrain above) are movement *overlays*: their cost
replaces the hex terrain cost when moving along them (a code rule), and per footnotes 6
& 8 they negate/halve hexside crossing and breakdown costs.

## Fortifications & Minefields (movement same as hex; add combat shifts)

| Feature | non-Mot | Mot | Breakdown | Barrage | Anti-Armor | Close Assault |
|---|---|---|---|---|---|---|
| Fortification L1 | *same as hex* | | | L1 ¹² | L1 | L2 |
| Fortification L2 | *same as hex* | | | L2 ¹² | L2 | L3 |
| Fortification L3 | *same as hex* | | | L2 ¹² | L2 | L4 |
| Friendly Minefield ¹³ | +1 | +4 | 0 | — | L1 | L1 |
| Enemy Minefield ¹³ | +4 | +CPA | +2 | — | — | — |

> Fortifications and minefields are modeled as *placed map features* (design tier 2),
> not base terrain. Their parameters are recorded here now; their Rust reference
> structs will be added when we build the map-feature model.

## Footnotes

1. A garrison unit in its assigned hex occupies zero stacking space. An unlimited number
   of anti-aircraft units may occupy a Major City hex at zero stacking; three AA units
   at an Airfield hex at zero; one AA unit at an Air Landing Strip hex at zero.
2. Vehicles other than Light Trucks, Recce-type units, and motorcycle infantry may only
   enter/leave the hex on a track. Motorized units may not engage in combat into/out of
   a Salt Marsh (Case 8.44).
3. Light Trucks, motorcycle reconnaissance units, and motorcycle infantry units may not
   enter Desert, even via a track.
4. Alexandria and Cairo hexes are Level Three Fortifications; all other Major Cities are
   Level Two.
5. Commonwealth units may be able to use Rail Movement (Case 8.7).
6. Road negates all hexside terrain feature entry costs and Breakdown Point values.
7. Stacking limit applies only to the units/truck points in that hex at that time.
8. Track halves all terrain feature entry costs and all hex/hexside Breakdown Point
   values, except the Capability Points expended and the Breakdown acquired moving a
   vehicle down an escarpment.
9. Motorized units and trucks may cross a Down Escarpment only on a track.
10. Wadi may be crossed during a Rainstorm only if the hexside is crossed by a road or
    railroad (treated as a road), at a cost of two additional Capability Points.
11. Major River may be crossed only if the hexside is crossed by a road or railroad; a
    motorized unit may so cross at no additional Capability or Breakdown cost.
12. Armor targets receive the fortification defensive benefit only if the hex is a Major
    City hex.
13. Engineer units reduce the cost to enter a minefield (Case 26.2). If assaulting
    forces are in an Enemy minefield, the non-Phasing forces receive L1 shifts for
    Anti-Armor and Close Assault if not already receiving them for occupying a Friendly
    minefield.

## Provenance
Transcribed from a photo of the printed chart via Apple Vision OCR (`tools/ocr.swift`),
with unreadable cells confirmed by the project owner (2026-07-06).
