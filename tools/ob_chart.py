#!/usr/bin/env python3
"""Reconstruct §4.44 "Organization at Arrival" OB charts from OCR output.

Pipe the Vision OCR output (one `y<TAB>x<TAB>text` line per observation, from
`tools/ocr.swift`) into this tool. A page may hold several formations stacked vertically;
each is delimited by its `Basic Morale: <value>` line (the most reliable marker). For each
formation it:
  * finds the title (line just above the morale line) and the table body (column header
    down to the "Notes:" footnotes or the next formation),
  * assigns tokens to columns by x, groups rows by y,
  * clusters the Unit-column x into indentation levels and resolves each unit's parent as
    the nearest row above with a smaller indent (§4.44b: indent = parent formation),
  * merges multi-line TOE cells.

Output per formation: a nested tree (for verification) + a TSV
(level/unit/abbr/id/toe/arrives/note/parent) for encoding. Compositions come from this
chart; ratings come from the Unit-Characteristics chart keyed by the ID column.

Usage:  swift tools/ocr.swift page.png | python3 tools/ob_chart.py
"""
import re
import sys

COLS = [
    ("note", 0.00, 0.16),
    ("unit", 0.16, 0.35),
    ("abbr", 0.35, 0.48),
    ("id", 0.48, 0.55),
    ("toe", 0.55, 0.80),
    ("arrives", 0.80, 1.01),
]
ROW_EPS = 0.009
INDENT_GAP = 0.008
HEADER_KEYS = {"Unit", "Arrives", "Abbreviation", "Counter", "Code"}
# A formation's morale line is "Basic Morale: <value>" (OCR may mangle "Basic" -> "Basie"
# etc., so anchor on the reliable "Morale:"), where <value> is a signed number (OCR
# sometimes renders 1->I/l, 0->O), "As listed in the Notes Column", or "That of the
# attached units" (Kampfgruppen). The legend's "...The rating for all of the units" must
# NOT match.
MORALE_RE = re.compile(r"(?i)\bmorale:\s*(?:[+\-]*\s*[\dIlOoZS]\b|as\s*listed|that\s*of)")
HEADER_WORDS = {"counter", "abbreviation", "notes", "unit", "arrives", "id", "code", "note"}


def column_of(x):
    for name, lo, hi in COLS:
        if lo <= x < hi:
            return name
    return None


def reconstruct(seg, formation, morale):
    """seg: list of (y,x,text) tokens for one formation's table body."""
    seg = sorted(seg)
    rows = []
    for y, x, t in seg:
        if rows and abs(y - rows[-1][0]) <= ROW_EPS:
            rows[-1][1].append((x, t))
        else:
            rows.append((y, [(x, t)]))

    records = []
    for _y, cells in rows:
        rec = {"unit": "", "unit_x": None, "abbr": "", "id": "", "toe": "", "arrives": "", "note": ""}
        for x, t in sorted(cells):
            col = column_of(x)
            if col == "unit":
                rec["unit"] = (rec["unit"] + " " + t).strip()
                rec["unit_x"] = x if rec["unit_x"] is None else min(rec["unit_x"], x)
            elif col == "toe":
                rec["toe"] = (rec["toe"] + " " + t).strip()
            elif col == "note":
                rec["note"] = (rec["note"] + t).strip()
            elif col:
                rec[col] = (rec[col] + " " + t).strip()
        if rec["unit"]:
            # A bare name fragment (a unit token with no other columns) is a wrapped
            # unit-name continuation, e.g. "136th Giovani Fascisti" / "Div HQE".
            bare = not (rec["abbr"] or rec["id"] or rec["toe"] or rec["arrives"] or rec["note"])
            if records and bare:
                records[-1]["unit"] = (records[-1]["unit"] + " " + rec["unit"]).strip()
            else:
                records.append(rec)
        elif rec["toe"] and records:
            records[-1]["toe"] = (records[-1]["toe"] + " " + rec["toe"]).strip()
        elif records:
            for k in ("id", "arrives", "abbr", "note"):
                if rec[k] and not records[-1][k]:
                    records[-1][k] = rec[k]

    if not records:
        return

    # Drop any column-header band that leaked in (e.g. when "Unit" mis-OCR'd as "Umit.").
    def is_header_remnant(r):
        u = r["unit"].lower()
        return (
            u.startswith("unit") or u.startswith("umit")
            or "abbreviation" in r["abbr"].lower()
            or "weapon system" in r["toe"].lower()
            or r["id"].strip() == "Code"
        )

    records = [r for r in records if not is_header_remnant(r)]
    if not records:
        return

    xs = sorted({round(r["unit_x"], 4) for r in records if r["unit_x"] is not None})
    levels = []
    for x in xs:
        if levels and x - levels[-1][-1] <= INDENT_GAP:
            levels[-1].append(x)
        else:
            levels.append([x])
    centers = [sum(c) / len(c) for c in levels]

    def level_of(x):
        return min(range(len(centers)), key=lambda i: abs(centers[i] - x)) if centers else 0

    for r in records:
        r["level"] = level_of(r["unit_x"]) if r["unit_x"] is not None else 0

    stack = []
    for r in records:
        while stack and stack[-1][0] >= r["level"]:
            stack.pop()
        r["parent"] = stack[-1][1] if stack else ""
        stack.append((r["level"], r["unit"]))

    print(f"# Formation: {formation or '(?)'}")
    print(f"# {morale or 'Basic Morale: (?)'}")
    print(f"# indent levels (unit-x centers): {[round(c, 3) for c in centers]}")
    print("## Tree")
    for r in records:
        pad = "    " * r["level"]
        extra = []
        if r["id"]:
            extra.append(f"ID={r['id']}")
        if r["toe"]:
            extra.append(r["toe"])
        if r["arrives"]:
            extra.append(f"arr {r['arrives']}")
        note = f" [{r['note']}]" if r["note"] else ""
        print(f"{pad}{r['unit']}  ({r['abbr']}){note}  " + " | ".join(extra))
    print("## TSV\tlevel\tunit\tabbr\tid\ttoe\tarrives\tnote\tparent")
    for r in records:
        print("\t".join([str(r["level"]), r["unit"], r["abbr"], r["id"], r["toe"], r["arrives"], r["note"], r["parent"]]))
    print()


def main():
    toks = []
    for line in sys.stdin:
        parts = line.rstrip("\n").split("\t")
        if len(parts) != 3:
            continue
        try:
            y = float(parts[0]); x = float(parts[1])
        except ValueError:
            continue
        toks.append((y, x, parts[2].strip()))
    if not toks:
        print("no OCR tokens on stdin", file=sys.stderr)
        return

    morales = sorted((y, t) for y, x, t in toks if MORALE_RE.search(t))
    header_ys = sorted(y for y, x, t in toks if t in HEADER_KEYS)
    notes_ys = sorted(y for y, x, t in toks if t.startswith("Notes:"))
    if not morales:
        print("no 'Basic Morale' lines found — is this a §4.44 chart page?", file=sys.stderr)
        return

    for i, (my, mtext) in enumerate(morales):
        next_m = morales[i + 1][0] if i + 1 < len(morales) else 1.0
        # Title: nearest text line above the morale line (ignore column-header words and
        # footnote lines like "a = Renamed..." / "b - The full name...").
        above = [
            (y, t) for y, x, t in toks
            if my - 0.06 < y < my and x < 0.5 and len(t) > 6
            and t.strip().lower() not in HEADER_WORDS
            and not re.match(r"^[a-zA-Z]\s*[-=]\s", t.strip())
        ]
        title = max(above, key=lambda p: p[0])[1] if above else None
        # Header row within this formation's span (fall back to just below morale).
        seg_headers = [h for h in header_ys if my < h < next_m]
        header_y = seg_headers[0] if seg_headers else my + 0.015
        # Body ends at this formation's footnotes, else just before the next formation's
        # title (which sits ~0.02 above the next morale line).
        seg_notes = [n for n in notes_ys if header_y < n < next_m]
        if seg_notes:
            end_y = seg_notes[0]
        elif i + 1 < len(morales):
            end_y = next_m - 0.025
        else:
            end_y = next_m
        body = [(y, x, t) for (y, x, t) in toks if header_y + ROW_EPS < y < end_y]
        reconstruct(body, title, mtext)


if __name__ == "__main__":
    main()
