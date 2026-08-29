# Commonwealth Rail Movement (§8.72–8.76 / §54.3)

The Commonwealth may use Rail Movement once per direction each Operations Stage (§8.74): move one
stack any distance along the finished rail line.

## Implemented (unit movement)
`rail_move(unit, to)`:
- Commonwealth only (§8.72; Axis use of captured rail §54.4 is deferred).
- The unit must **start on a rail hex**, have expended **no CP** this stage, and not be in an enemy
  ZOC (§8.73); carry **≤ 2 Stacking Points** (§8.75).
- `to` must be a rail hex **reachable along built (Railroad) track** avoiding any enemy ZOC or
  enemy-occupied hex (`rail_reachable`, §8.72/8.73).
- One stack **west** and one **east** per stage (§8.74; `rail_west_used`/`rail_east_used`, reset
  each Operations Stage). Destroyed/unbuilt track is impassable (not a `Railroad` hexside, §8.76).

## Implemented (supply)
`rail_move_supply(from, to, cargo, points)`: move one commodity along the line, capped at 1500 tons
per direction (§54.32) — Stores 1 t, Ammo 4 t, Fuel ⅛ t per point, so **1500 / 375 / 12000** points
respectively; Water is never railed (§54.33). One commodity type per shipment, sharing the
once-per-direction allowance with unit movement (§54.32). Delivers into (or creates) a Commonwealth
dump at the destination (§54.35).

## Implemented (§54.34)
`rail_maintenance_this_stage`: while set, no Rail Movement or rail supply is possible (the RR
carries water forward for its own use). The Commonwealth designates one such Operations Stage per
calendar month; the flag is reset at the start of each stage.
- **Axis use of the Commonwealth railroad (§54.4):** control 5+ contiguous rail hexes + rolling
  stock; the Barce line east of Benghazi is never usable.
