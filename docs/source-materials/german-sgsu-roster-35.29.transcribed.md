# German SGSU Counter Roster (§35.29)

Transcription of the German SGSU counters (desktop screenshot 2026-09-10). Per §35.29 each German
SGSU represents a specific plane type, encoded in its Geschwader prefix: **JG** = Fighter, **St.G** =
Dive Bomber, **KG** = Bomber, **ZG** = Fighter-Bomber, **H/(F)** = Recon. (Following this is optional —
§35.29: "Players need not follow this representation.")

**Every counter carries CPA 25** (confirms the value used in `squadron::sgsu_cpa`). The two leftmost
counters on the sheet ("INF REPL C / D") are infantry replacements, not SGSUs, and are excluded.

## Roster (38 SGSUs, all CPA 25)

| Designation | Prefix | §35.29 role |
|---|---|---|
| 2/I St.G.1 | St.G | Dive Bomber |
| 1/I St.G.1 | St.G | Dive Bomber |
| 6/II St.G.2 | St.G | Dive Bomber |
| 3/I St.G.1 | St.G | Dive Bomber |
| Stab St.G.3 | St.G | Dive Bomber |
| 1/I St.G.3 | St.G | Dive Bomber |
| 4/II St.G.3 | St.G | Dive Bomber |
| 9/III St.G.3 | St.G | Dive Bomber |
| 1/I JG 27 | JG | Fighter |
| 4/II JG 27 | JG | Fighter |
| 5/II JG 27 | JG | Fighter |
| 6/II JG 27 | JG | Fighter |
| 7/III JG 27 | JG | Fighter |
| 8/III JG 27 | JG | Fighter |
| 9/III JG 27 | JG | Fighter |
| 7/III JG 53 | JG | Fighter |
| 8/III JG 53 | JG | Fighter |
| 9/III JG 53 | JG | Fighter |
| 1/I JG 77 | JG | Fighter |
| 1/III JG 77 | JG | Fighter |
| 8/III JG 26 | JG | Fighter |
| 9/III JG 26 | JG | Fighter |
| 1/I NJG 2 | NJG | Fighter (night) — inference; §35.29 lists JG only |
| 5/II NJG 2 | NJG | Fighter (night) — inference |
| 8/III ZG 26 | ZG | Fighter-Bomber |
| 7/III ZG 1 | ZG | Fighter-Bomber |
| 2.(H) 14 | (H) | Recon |
| 4.(H) 12 | (H) | Recon |
| 1.(F) 121 | (F) | Recon |
| 5/II KLG 1 | KLG | Bomber |
| 7/III KLG 1 | KLG | Bomber |
| 7/ KLG 26 | KLG | Bomber |
| 4/II KG 100 | KG | Bomber |
| 1/I KG 54 | KG | Bomber |
| Stab KLG 1 | KLG | Bomber |
| 3/I KLG 1 | KLG | Bomber |
| 8/III KLG 77 | KLG | Bomber |
| 1/I SG 2 | SG | **Shelved** — confirmed real counter (Schlachtgeschwader 2); §35.29 has no SG category, and the designation is a cosmetic label, so it stays unclassified (`german_sgsu_role` → `None`) until a role is needed. |

## Tally by §35.29 role

- **Fighter (JG):** 14
- **Fighter, night (NJG):** 2
- **Dive Bomber (St.G):** 8
- **Fighter-Bomber (ZG):** 2
- **Recon (H/F):** 3
- **Bomber (KG/KLG):** 8
- **Uncertain (SG 2):** 1
- **Total: 38**

## Notes (reads confirmed by the owner 2026-09-10)
- **KLG** is a genuine, distinct designation (Kampf-Lehrgeschwader), not a read error — there are
  several KLG counters. §35.29 doesn't list it, but the K-prefix bomber role is unambiguous, so
  `german_sgsu_role` maps it to Bomber (via the `LG`/`KG` branch).
- **NJG** (night fighter) is likewise absent from §35.29's list; mapped to Fighter (night).
- **1/I SG 2** is a confirmed real counter (Schlachtgeschwader 2), but §35.29 gives it no role and the
  label is purely cosmetic, so it is **shelved** — `german_sgsu_role` returns `None` for it until a
  role is actually needed.
- This is the captured sheet; further German SGSU counters may sit on other sheets.
