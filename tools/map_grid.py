#!/usr/bin/env python3
"""Map hex-grid tooling for CNA per-section map images (flat-top hexes).

Calibration (section image coords, y measured from the TOP):
  hex (col,row) center:
    x = X0 + (col-1)*DCOL
    y_from_bottom = Y0_BOTTOM + (row-1)*DROW + (HALF if column is offset else 0)
    y = image_height - y_from_bottom
Origin hex 0101 is bottom-left; col grows right, row grows up.

Subcommands:
  overlay <image> <out.jpg> <cropx> <cropy> <cropw> <croph> [parity]
      Draw hex centers + CCRR labels over a crop window so alignment can be
      visually confirmed. parity in {evenup, oddup, none} (default evenup).
"""
import sys
import numpy as np
from PIL import Image, ImageDraw, ImageFont

Image.MAX_IMAGE_PIXELS = None

# Section-C calibration (na_map-002.jpg), from manual measurement.
X0 = 88.0
Y0_BOTTOM = 322.0
DCOL = 94.0
DROW = 324.0
HALF = DROW / 2.0


def cna_label(col, row):
    """CNA hex label 'RRCC' from internal (col,row). Columns pair into strips (CC), and
    RR counts every half-row level (odd cols low, even cols half up), so hexes sharing a
    pixel-Y share RR."""
    cc = (col + 1) // 2
    rr = 2 * row - 1 if col % 2 == 1 else 2 * row
    return f"{rr:02d}{cc:02d}"


def sample_hex_color(arr, x, y, radius=28):
    """Median RGB of a central disk at (x,y), ignoring pixels far from the local median
    (labels/lines) — a robust estimate of the hex's fill colour."""
    h, w, _ = arr.shape
    x0, x1 = max(0, int(x - radius)), min(w, int(x + radius))
    y0, y1 = max(0, int(y - radius)), min(h, int(y + radius))
    if x1 <= x0 or y1 <= y0:
        return None
    patch = arr[y0:y1, x0:x1].reshape(-1, 3)
    med = np.median(patch, axis=0)
    # keep pixels within 40 of the median (drop label text / grid lines)
    d = np.abs(patch.astype(int) - med).sum(axis=1)
    keep = patch[d < 60]
    if len(keep) == 0:
        keep = patch
    return tuple(int(v) for v in np.median(keep, axis=0))


def sample(image, cx, cy, cw, ch, parity):
    im = Image.open(image).convert("RGB")
    W, H = im.size
    arr = np.asarray(im)
    rows = []
    for col in range(1, 140):
        for row in range(1, 140):
            x, y = hex_center(col, row, H, parity)
            if cx <= x < cx + cw and cy <= y < cy + ch:
                c = sample_hex_color(arr, x, y)
                if c:
                    rows.append((cna_label(col, row), c))
    # Cluster by rounding to a coarse grid to reveal the palette.
    from collections import Counter
    buckets = Counter()
    for _, c in rows:
        buckets[tuple((v // 16) * 16 for v in c)] += 1
    print(f"sampled {len(rows)} hexes in crop ({cx},{cy}) {cw}x{ch}")
    print("top colour clusters (RGB ~/16, count):")
    for c, n in buckets.most_common(12):
        print(f"  rgb{c}  x{n}")


def hex_center(col, row, height, parity):
    x = X0 + (col - 1) * DCOL
    yb = Y0_BOTTOM + (row - 1) * DROW
    if parity == "evenup" and col % 2 == 0:
        yb += HALF
    elif parity == "oddup" and col % 2 == 1:
        yb += HALF
    return x, height - yb


def load_font(size):
    for p in ("/System/Library/Fonts/Supplemental/Arial Bold.ttf",
              "/System/Library/Fonts/Helvetica.ttc"):
        try:
            return ImageFont.truetype(p, size)
        except Exception:
            pass
    return ImageFont.load_default()


def overlay(image, out, cx, cy, cw, ch, parity):
    im = Image.open(image).convert("RGB")
    W, H = im.size
    crop = im.crop((cx, cy, cx + cw, cy + ch))
    draw = ImageDraw.Draw(crop)
    font = load_font(22)
    n = 0
    for col in range(1, 60):
        for row in range(1, 60):
            x, y = hex_center(col, row, H, parity)
            if cx - DCOL <= x <= cx + cw + DCOL and cy - DROW <= y <= cy + ch + DROW:
                lx, ly = x - cx, y - cy
                r = 6
                draw.ellipse((lx - r, ly - r, lx + r, ly + r), fill=(255, 0, 0))
                draw.text((lx + 8, ly - 12), cna_label(col, row), fill=(255, 0, 0), font=font)
                n += 1
    crop.save(out, quality=80)
    print(f"image {W}x{H}; drew {n} hex labels; crop ({cx},{cy}) {cw}x{ch} -> {out}")


if __name__ == "__main__":
    if len(sys.argv) >= 8 and sys.argv[1] == "overlay":
        img, out = sys.argv[2], sys.argv[3]
        cx, cy, cw, ch = (int(a) for a in sys.argv[4:8])
        parity = sys.argv[8] if len(sys.argv) > 8 else "evenup"
        overlay(img, out, cx, cy, cw, ch, parity)
    elif len(sys.argv) >= 7 and sys.argv[1] == "sample":
        img = sys.argv[2]
        cx, cy, cw, ch = (int(a) for a in sys.argv[3:7])
        parity = sys.argv[7] if len(sys.argv) > 7 else "evenup"
        sample(img, cx, cy, cw, ch, parity)
    else:
        print(__doc__)
        sys.exit(1)
