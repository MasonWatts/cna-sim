#!/usr/bin/env python3
"""Validate a section terrain export from tools/map_editor.html.

Usage: check_map.py <export.ron> <section_image>

Reports: total in-bounds hexes vs classified, any UNCLASSIFIED (missing) hexes, any
exported ids that are out of bounds, and a count per terrain type. Uses the locked
section-C calibration to enumerate the true set of hexes.
"""
import sys
import re
from collections import Counter
from PIL import Image

Image.MAX_IMAGE_PIXELS = None

# Locked section-C calibration (must match tools/map_editor.html).
X0, Y0B, DCOL, DROW = 84.0, 326.0, 94.5, 323.5
HALF = DROW / 2.0


def center(col, row, H):
    x = X0 + (col - 1) * DCOL
    yb = Y0B + (row - 1) * DROW + (HALF if col % 2 == 0 else 0)
    return x, H - yb


def label(col, row):
    cc = (col + 1) // 2
    rr = 2 * row - 1 if col % 2 == 1 else 2 * row
    return f"{rr:02d}{cc:02d}"


def main(ron, image):
    W, H = Image.open(image).size
    valid = set()
    for c in range(1, 90):
        for r in range(1, 40):
            x, y = center(c, r, H)
            if 0 <= x < W and 0 <= y < H:
                valid.add(label(c, r))
    txt = open(ron).read()
    cls = {m.group(1): m.group(2)
           for m in re.finditer(r'"(\d{4})":\s*(?:\(terrain:\s*)?([A-Za-z]+)', txt)}
    missing = sorted(valid - set(cls))
    extra = sorted(set(cls) - valid)
    counts = Counter(cls.values())

    print(f"image {W}x{H}")
    print(f"in-bounds hexes: {len(valid)}")
    print(f"classified:      {len(cls)}")
    print(f"UNCLASSIFIED:    {len(missing)}")
    if missing:
        print("  missing ids:", ", ".join(missing[:60]), ("…" if len(missing) > 60 else ""))
    if extra:
        print(f"out-of-bounds ids in file: {len(extra)} -> {', '.join(extra[:30])}")
    print("terrain counts:")
    for name, n in counts.most_common():
        print(f"  {name:16} {n}")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        print(__doc__)
        sys.exit(1)
    main(sys.argv[1], sys.argv[2])
