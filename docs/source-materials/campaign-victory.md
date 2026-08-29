# Campaign Victory Conditions (Rules §64.7)

Human-readable transcription. Authoritative machine values: `data/reference/campaign-victory.ron`.
City hexes verified by the project owner (2026-07-14).

## §64.71 — Axis automatic win
Occupy **all hexes of Alexandria and Cairo** for one full Game-Turn with units that can trace
a convoy supply line (≤ 90 truck MP) back to a Supply Dump ultimately supplied from Tobruk or
Tripoli → Axis wins outright.

## §64.72 — Commonwealth automatic win
From the first OpStage of **Game-Turn 35** onward, if **no** Axis combat unit can trace a
supply line (≤ 60 truck MP) to a Supply Dump → Tobruk/Tripoli, the Commonwealth wins.

## §64.73 — Geographical Occupation Points
Otherwise, each side scores VP for occupying these cities. Occupation = a **combat unit**
(non-parenthesized Close Assault, §60.82) of **≥1 TOE** in the hex that, at game end, has
stores/water for a week and fuel/ammo to fire 3× and move 20 CP (the supply-adequacy test).

| City | Hex | Axis VP | C'wlth VP |
|---|---|---|---|
| Tobruk | C4807 | 200 | 100 |
| Bardia | C4321 | 100 | 50 |
| Mersa Matruh | D3714 | 100 | 10 |
| Benghazi | A4827 | 75 | 100 |
| Sidi Barrani | C4131 | 50 | 10 |
| Derna | B5925 | 25 | 50 |
| Sollum | C4021 | 25 | 10 |
| Siwa | C0127 | 20 | 10 |
| Giarabub | C1014 | 15 | 10 |
| Jalo | B0513 | 10 | 20 |

## §64.74 / §64.75 — other points
- **Replacement VP:** 1 per unused Replacement Point (excl. planes/trucks; excl. infantry for CW).
- **Withdrawal VP (CW only):** voluntary battalion withdrawals from Alexandria/Cairo.

## §64.76 — Victory levels (ratio of most : least points)
| Ratio | Result |
|---|---|
| 1 : 1 | Draw |
| > 1:1, up to 1½:1 | Marginal Victory |
| > 1½:1, up to 2½:1 | Decisive Victory |
| > 2½:1 | Smashing Victory |

Confirmed from the §64.76 chart (project owner, 2026-07-14). Encoded in the RON `levels`
(max_ratio_pct 100 / 150 / 250; above 250 → Smashing).

## §64.74 Replacement Victory Points (implemented)
1 VP per **unused** Replacement Point allotted in a side's Production Charts — excluding planes,
trucks, and (Commonwealth only) infantry. Modeled as `replacement_vp(side)` = the sum of that
side's §20.66/§20.78C production-chart `total`s (trucks / CW-infantry live in separate tables,
so they are naturally excluded; planes are not modeled) minus `replacements_used[side]` (points
consumed via `absorb_replacement`). Added to each side's total in `evaluate_campaign_victory`.
## §64.75 Commonwealth Withdrawal Points (implemented)
`withdraw_unit(id)` withdraws an eligible CW combat **battalion** (not company) of
infantry/armour/artillery/anti-tank (not AA) at ≥75% TOE, currently in Alexandria/Cairo (§64.71
objective hexes) and not locked out — it leaves the map. `withdrawal_vp()` = banked cycle points +
1 per week each still-withdrawn battalion is gone (max 3). `return_unit(id, hex)` banks
(earned − 2 penalty, §64.75 B) and locks re-withdrawal for six months (~26 GT). Added (clamped ≥0)
to the Commonwealth total in `evaluate_campaign_victory`.

## §64.71 / §64.72 automatic victory (supply-line)
- **§64.71 (Axis):** the Axis occupies every hex of Alexandria (E3613, E3714) and Cairo
  (E1430) for one full Game-Turn, and each occupying combat unit can trace a line of supply
  of **≤90 truck MP** to a Supply Dump that "can be supplied from Tobruk or Tripoli in any
  way" → Axis wins regardless of date.
- **§64.72 (Commonwealth):** from the first OpStage of **Game-Turn 35**, if there are **no
  Axis combat units** that can trace a **≤60 truck-MP** line to such a dump → Commonwealth
  wins automatically (excludes air / coastal shipping).
- **Strategic sources:** Tobruk (C4807) and the Tripoli off-map road's on-map entry
  (A2802, the western coast-road junction of the §8.8 complex).
- **Implemented:** `unit_strategic_supply_line(id, max_mp, sources)` (two legs: unit →≤MP truck→
  a friendly non-dummy dump →any truck route→ a source) and `campaign_auto_winner()` in
  game.rs, with the parameters in `campaign-victory.ron` (sources / axis_objective /
  axis_supply_mp 90 / cw_check_mp 60 / cw_check_from_turn 35).
- **§64.73 supply-adequacy (implemented):** occupation requires the combat unit to pass the real
  commodity-adequacy test `unit_supply_adequate` — at its hex it must have **Stores and Water for
  a week and Fuel/Ammo to move 20 CP and fire 3×**, counting its on-hand supply plus co-located
  friendly (non-dummy) Supply Dumps, with Water also satisfied by a well/oasis/major-city/pipeline
  source at the hex. `city_holder` enforces it, so an under-supplied unit does not occupy for VP.
  (This replaced the earlier coarse §32.16 half-CPA dump-reachability stand-in once §49/§50/§51/§52
  commodities were modeled.) Interpretation notes: "fire 3×" = 3× the unit's Close-Assault ammo
  cost (0 for plain rifle infantry per §50.2); leg infantry needs no Fuel.
- **§64.71 full-turn hold (implemented):** the Axis auto-win requires the objective+supply
  condition to hold continuously for one full Game-Turn. `axis_objective_held_since` records
  the Game-Turn it was first satisfied (via `update_victory_hold`, called once per turn at
  Initiative Determination) and clears the moment it lapses; `campaign_auto_winner` awards the
  Axis win only when the condition still holds and `game_turn > held_since`.

## Implementation status
Implemented: §64.73 occupation-point scoring (a combat unit ≥1 TOE that passes the
`unit_supply_adequate` commodity-adequacy test — *supplied* presence, not bare presence), the
§64.76 level from the point ratio, the §64.71/§64.72 auto-wins (via the ≤90/≤60-MP strategic
convoy line), and the §64.74/§64.75 replacement/withdrawal VP — all in
`GameState::evaluate_campaign_victory` / `campaign_auto_winner`. Nothing here is deferred.
