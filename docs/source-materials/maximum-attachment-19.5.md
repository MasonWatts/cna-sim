# Maximum Attachment Chart (§19.5)

Human-readable transcription (PDF page 101, Charts & Tables Common to Both Players, chart
page 6). Direct read of the vector render at 220 DPI, 2026-08-31. The numeric per-parent
caps are encoded in `GameState::nonassigned_attachment_limit`.

"Maximum Attachment" = the number of **non-assigned** units that may be attached to a Parent
Formation over and above its assigned organization (§19.4). Assigned units attach to their
own parent freely.

## Allied
| Parent | Max non-assigned attachment(s) |
|---|---|
| Armor Division (GT 1–67) | 2 units — 1 Infantry; no Tank |
| Armor Division (GT 68+) | 3 units — 1 Infantry and/or 1 Tank |
| Infantry Division | 2 units — 1 Infantry; no Tank |
| Tank Brigade (GT 1–67) | 2 units — 1 Infantry; no Tank |
| Tank Brigade (GT 68+) | 1 unit — no Tank |
| other Brigade | 1 unit |
| any Battalion | 1 Company |
| Matruh Garrison (GT 1–12) | 6 units — no tank, 1 gun class |
| Selby Force (GT 13+) | 5 units — no tank, no recce, 3 infantry |

## German
| Parent | Max non-assigned attachment(s) |
|---|---|
| Armor Division | 1 Brigade and 1 unit **or** 4 units — no Tank |
| Infantry Division | 1 Brigade **or** 3 units — no Tank |
| Infantry or Armor Regiment | 1 unit |
| Battle Group | 4 units |
| Artillery Brigade HQ | 1 Artillery unit |
| any Battalion | 1 Company |

## Italian
| Parent | Max non-assigned attachment(s) |
|---|---|
| Armor Division / Tank Group | 2 units |
| Infantry Division | 2 units — 1 Infantry or 1 Tank |
| Brigade / Regiment | 1 unit |
| Battle Group | 3 units |
| any Battalion | no units |

Modification: any nation's Division- and Brigade-equivalent may attach two non-shell
company-equivalents at no cost to the maximum; three company-equivalents equal one
battalion-equivalent, and the company-equivalents must respect the type restrictions.

## Engine simplifications (documented)
`nonassigned_attachment_limit` keys the cap on the parent's effective `OrgSize`, nationality,
and Game-Turn only. The engine does not track a formation's **arm**, so where the chart splits
by type (armor vs infantry division, tank vs other brigade) the more permissive column is used.
The type sub-constraints ("1 Infantry; no Tank"), the German "1 Bde + 1 unit / OR" alternatives,
the Matruh-Garrison / Selby-Force named specials, and the finer §19.3 **assignment** maxima
(Formation Organization Chart — not located in the shared Charts & Tables section) are not yet
enforced. The reorganization commands (Attach/Detach/Reassign, §19.41–44) with their CP costs,
segment gating, co-location and one-parent integrity are exact.
