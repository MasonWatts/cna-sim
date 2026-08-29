# Commonwealth OB (§4.44a) — modeling notes

Reconstructed into `cw-ob-4.44a.DRAFT.txt` (35 formations, pp.117-133; p116 is the intro
legend). Most are ordinary formations (armored/infantry divisions and brigades across
British, Australian, Indian, New Zealand, South African, Free French, Greek, and Polish
contingents, plus the "Unassigned <type>" collections). Two on **p121** are special-rule
formations laid out **side by side in two columns** (Selby left, Guards right), so the
stacked-formation parser skips them — model them manually like the German Kampfgruppen.

Both are **empty brigade HQ counters** — ID Code `a`, Basic Morale **"None (that of the
attached units)"**, no fixed battalions — formed by attaching units at runtime (our
`attached_parent` model + a future "morale from attached / disband when empty" rule).

## Selby Force / The Matruh Garrison  (p121, left column)
- Starts as the Matruh Garrison; at **Operations Stage 2 of Game-Turn 12** the unit becomes
  **Selby Force** — described as "the only Commonwealth Battle Group."
- Historically comprised the 3rd Coldstream Guards, 2nd Scots Guards, 1st Durham Light
  Infantry, 1st Buffs, 9th Rifle Brigade, and various anti-tank units (attach at play).

## 22nd Guards Brigade → 200th Guards Brigade  (p121, right column)
- An infantry brigade HQ with **no battalions assigned**; units attach during play.
- Assigned to the **70th Infantry Division**.
- Renamed over time: Jan 1942 → 200th Guards Brigade; Apr 1942 → 200th Guards Motor Brigade
  Group; May 1942 → 201st Guards Motor Brigade Group.

## Other CW-chart notes
- **Parenthesized morale** (e.g. 1st Armoured `+2 (+1)`, Polish Carpathian `+1 (-1)`):
  the p116 legend + rules glossary (Morale) define it as the *arrival/deploy* morale — the
  lower value a green Commonwealth unit **begins with** and may be **trained up** to its
  Basic Morale (the main number). Basic Morale can even be exceeded via battlefield success
  (§6.25/§17.2, Cohesion). Encoding: current `basic_morale` = the parenthetical (arrival),
  a new reference/target = the main number, plus a future train-up rule (raise current
  toward reference). A bare number means the unit arrives already at its Basic Morale.
- **Footnotes can carry the arrival morale**, not just history: when a formation's only
  parenthetical is a letter (e.g. 9th Australian `+1 (a)`), read the footnote — it may state
  the arrival value in prose ("arrives untrained at 0, may be trained to +1" → treat as
  `+1 (0)`). So check the footnote whenever the parenthetical is a bare letter.
- None of these are in Scenario 1 (Sept 1940) except the 7th Armoured Division's covering
  force; the 7th Armoured sheet (p117) is the source for the Scenario-1 CW screen.

## Deferred
- Encode the p121 specials once we add the dynamic-formation (attach/disband, morale-from-
  attached) mechanic shared with the German Kampfgruppen.
