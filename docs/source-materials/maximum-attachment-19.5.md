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

## Engine encoding
`nonassigned_attachment_limit` gives the numeric cap, keyed on the parent's effective `OrgSize`,
nationality, arm (`is_armor`) and Game-Turn — including the named specials. A formation's **arm**
(`is_armor`) and its special **role** (`formation_role`: Artillery Brigade HQ, Matruh/Selby) are
derived from the OB at scenario setup (a formation is armored if it has tank units assigned; roles
match the HQ designation), the faithful static classification rather than runtime composition.

`attachment_type_ok` applies the type sub-constraints: the "no Tank" / per-type maxima
("1 Infantry; no Tank" etc.), the Italian infantry division's "1 Infantry *or* 1 Tank" (combined
≤1), the Artillery Brigade HQ's "Artillery only", and the Matruh Garrison (no tank, ≤1 gun) /
Selby Force (no tank, no recce, ≤3 infantry) specials — all checked across every ancestor formation
(§19.46). The reorganization commands (Attach/Detach/Reassign, §19.41–44) with their CP costs,
segment gating, co-location and one-parent integrity are exact.

Documented simplifications: the German "1 Brigade and 1 unit *or* 4 units" alternative uses the
more permissive count (4), and the finer §19.3 **assignment** maxima (the Formation Organization
Chart — not located in the shared Charts & Tables section) are approximated by `assigned_capacity`
(the OB starting count).
