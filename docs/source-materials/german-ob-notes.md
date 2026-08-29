# German OB (§4.44c) — modeling notes

Reconstructed by the pipeline into `german-ob-4.44c.DRAFT.txt` (11 formations, pp.162-166).
Most are ordinary formations (Panzer/Light divisions, Ramcke Brigade, Sonderverband 288,
300th Oasis Bn, non-divisional artillery, unassigned AA/infantry). Two need special handling.

## Kampfgruppen HQ's (p165) — NOT a fixed OB table
A Kampfgruppe is an ad-hoc, brigade-level formation formed *dynamically* during play, so
the chart is a **rule + a pool of reusable HQ counters**, not a composition table. The
`ob_chart.py` auto-parse of this block is unreliable — model it manually:

- **Reusable HQ counters** (a random historical selection; ~14): Gruppe Badde, Gruppe Bach,
  Gruppe Dedeking, Kampfgruppe Fischer, Kampfgruppe Graf, Kampfgruppe Kaiser, Kampfgruppe
  Knabe, Kampfgruppe Kost, Gruppe Marcks, Kampfgruppe Menny, Kampfgruppe Mitte, Kampfgruppe
  Pfeiffer, Gruppe Schwerin, Gruppe Voss.
- **ID Code: b** (a brigade-HQ characteristics code); **Basic Morale: "that of the attached
  units"** (derive at runtime, not a fixed value).
- **Rules:** (1) a Kampfgruppe is formed by attaching a German unit to an HQ counter;
  (2) it disbands when its last German unit is detached (elimination in combat does not
  disband it — remaining Italian units detach, remaining anti-tank TOE becomes replacement
  points); (3) may not attach/assign more than two [limit — verify exact number]; the Axis
  player is limited to the number of battlegroup counters available.
- **Engine mapping:** a set of `is_hq` brigade-level counters with **no fixed composition**;
  units attach at runtime (our `attached_parent` model). Needs a future "morale derived from
  attached units" rule and a disband-on-empty rule. For now: seed the counters as empty HQs.

## Unassigned Infantry Units (p164) — DAK HQ + independent companies
Ordinary table (now parses after the note/unit column boundary fix), headed by the
**Deutsches Afrika Korps HQ** plus independent companies (707/708 SchwInf Koy, 13th
Koy/Brandenburg, 778 Naval Engineer Koy). This table carries a **"Rommel" column** — the
Rommel presence marker (§17.28 morale bonus), which we already model via
`UnitStatus.with_rommel`. Notes track re-attachment history (e.g., first to 90th Light, later
to 164th Light).

## Deferred
- Exact Kampfgruppe attach/assign limit (rule 3 cut off in OCR — verify against the page).
- The German charts (like the Italian) give arrival turns (e.g. `2/20`) for the campaign
  game; none of the German formations are in Scenario 1 (Sept 1940 — the DAK arrives 1941).
