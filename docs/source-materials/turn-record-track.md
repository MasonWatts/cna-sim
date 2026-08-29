# Axis Game-Turn Record Track (Scenario 5 campaign calendar)

Transcribed from the Turn Record Track on Map D. Each Game-Turn = one week (§1). The campaign
opens **GT1 = September week III, 1940** (Italian invasion of Egypt), so September 1940 has only
weeks III–IV (GT1–2); every subsequent month is four one-week turns. Track runs GT1 → GT110
(December week IV, 1942).

| GT | Date | | GT | Date | | GT | Date |
|----|------|-|----|------|-|----|------|
| 1  | Sep III 1940 | | 38 | Jun IV 1941 | | 75 | Apr I 1942 |
| 2  | Sep IV 1940  | | 39 | Jul I 1941  | | 76 | Apr II 1942 |
| 3–6   | Oct I–IV 1940 | | 40–42 | Jul II–IV 1941 | | 77–78 | Apr III–IV 1942 |
| 7–10  | Nov I–IV 1940 | | 43 | Aug I 1941 | | 79–82 | May I–IV 1942 |
| 11–14 | Dec I–IV 1940 | | 44 | Aug II 1941 | | 83–86 | Jun I–IV 1942 |
| 15–18 | Jan I–IV 1941 | | 45 | Aug III 1941 | | 87–90 | Jul I–IV 1942 |
| 19–22 | Feb I–IV 1941 | | 46 | Aug IV 1941 | | 91–94 | Aug I–IV 1942 |
| 23–26 | Mar I–IV 1941 | | 47–50 | Sep I–IV 1941 | | 95–98 | Sep I–IV 1942 |
| 27–30 | Apr I–IV 1941 | | 51–54 | Oct I–IV 1941 | | 99–102 | Oct I–IV 1942 |
| 31–34 | May I–IV 1941 | | 55–58 | Nov I–IV 1941 | | 103–106 | Nov I–IV 1942 |
| 35 | Jun I 1941 | | 59–62 | Dec I–IV 1941 | | 107–110 | Dec I–IV 1942 |
| 36–37 | Jun II–III 1941 | | 63–66 | Jan I–IV 1942 | | | |
| | | | 67–70 | Feb I–IV 1942 | | | |
| | | | 71–74 | Mar I–IV 1942 | | | |

Formula (`state::game::date_of`): GT1→(1940,9,3), GT2→(1940,9,4); for GT≥3, `n = gt-3` weeks since
Oct I 1940, `week = n%4+1`, month/year from October 1940 + `n/4` months.

## Anchors that resolve prior calendar discrepancies
- **GT35 = Jun I 1941** — Bizerta opens (§55.17 "June 1 1941" and the §55.3 chart's "GT 35" now
  agree; the earlier uniform-4/month guess of GT37 was wrong).
- **GT32 = May II 1941** — the "second Game-Turn of May 1941" for the Greece shipping lanes (§56.14);
  the earlier derived GT34 was wrong.
- **GT46 = Aug IV 1941** — the last Game-Turn of August 1941, ending the Commonwealth 9%
  evaporation window (§49.3/§52.44); the earlier constant of GT48 was wrong.

NB: `season_of` is intentionally **not** derived from this calendar — the Weather Table (§29.61)
uses its own 12-GT-block Game-Turn column, which the code follows.
