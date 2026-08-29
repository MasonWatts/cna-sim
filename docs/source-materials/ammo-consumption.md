# Ammunition Consumption Rates (§50.2)

Transcribed from the §50.2 table (2026-07-14). "Ammo" = the number of Ammunition Points that
must be expended for **one TOE Strength Point** to perform the action (§50.14). A unit may fire
fewer TOE SP to conserve ammo.

## Logistics Game Played (the mode we implement)
| Action | Ammo per TOE SP |
|---|---|
| Barrage | 4 |
| Anti-armor | 3 |
| Close Assault — Armor-class, Gun-class, MG-inf, HvyWpn-inf | 2 |
| Anti-air at a single target group | 2 |
| (Air) Rearm any portion of a squadron's planes' tacair rating | 1 |
| (Air) Rearm one plane with bombs/torpedoes/mines | 1 |
| (Air) Engage in air-to-air and/or strafe | TacAir Bombload |

Notes:
- **Close Assault** ammo (rate 2) applies only to Armor-class, Gun-class, MG-infantry and
  Heavy-Weapon-infantry — plain rifle infantry consumes no Ammo Points for close assault (rifle
  ammo is below the ~4-ton Ammo-Point granularity, §50.11).
- Air-action rows belong to the Air Game (deferred).

## Logistics Game Abstracted (§32 — NOT used; we dropped the abstract mode per D15)
Barrage phasing Bn-Eq 4 / non-phasing 2; Assault phasing 1 / non-phasing 1; Anti-air Gun-class
1; rearm non-fighter squadron 1; non-fighter flight = Bombload. Retained here for reference only.

## Implementation
`data/reference/ammo-consumption.ron` → `AmmoConsumptionRates { barrage, anti_armor,
close_assault, anti_air }`. Consumption = rate × TOE Strength Points, drawn from the firing
unit's onboard reserve (§50.17 "fire once") and Supply Dumps in its hex (§50.15). A unit that
cannot pay may not fire (§50.12). Deferred: defensive-assault consumption and the no-ammo
defender penalties (no ZOC, surrender when fired on in an enemy ZOC — §50.12), the ⅓ captured-
ammo rule (§50.16), and the MG-inf/HvyWpn-inf close-assault classification (needs a unit-type
flag; currently only weapon-holding armor/gun units pay for close assault).
