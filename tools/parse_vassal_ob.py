#!/usr/bin/env python3
"""Extract the order-of-battle hierarchy from a decoded Vassal setup blob.

The CNA Vassal module names each counter image `<NAT> <designation> - <parent>.svg`,
where `<parent>` is the formation the unit is assigned to (`none` = independent). The
per-formation org-chart therefore lists a formation's *full complement* of sub-units —
which is both the attachment hierarchy and the OA "maximum assignable" count used by the
shell rule (§9.26).

This tool does NOT recover TOE/weapon compositions (those live on the OA sheets).

Usage:
    parse_vassal_ob.py <decoded_setup.txt> [--parent SUBSTR] [--nat IT|CW|GE]
"""
import argparse
import re
import sys
from collections import defaultdict


def extract_pairs(text):
    """Return list of (nationality, designation, parent) from counter image names."""
    # e.g. "IT 240Lgn - 4 CCNN.svg", "IT Maletti - none.svg"
    pat = re.compile(r'\b(IT|CW|GE) ([A-Za-z0-9./]+(?: [A-Za-z0-9./]+)*?) - ([A-Za-z0-9][A-Za-z0-9 ]*?)\.svg')
    seen = set()
    out = []
    for nat, desig, parent in pat.findall(text):
        key = (nat, desig.strip(), parent.strip())
        if key not in seen:
            seen.add(key)
            out.append(key)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("path")
    ap.add_argument("--parent", default=None, help="only show parents containing this substring")
    ap.add_argument("--nat", default=None, help="filter by nationality (IT/CW/GE)")
    args = ap.parse_args()

    text = open(args.path, "rb").read().decode("latin-1")
    pairs = extract_pairs(text)

    tree = defaultdict(list)
    for nat, desig, parent in pairs:
        if args.nat and nat != args.nat:
            continue
        tree[(nat, parent)].append(desig)

    for (nat, parent) in sorted(tree):
        if parent == "none":
            continue
        if args.parent and args.parent.lower() not in parent.lower():
            continue
        kids = sorted(tree[(nat, parent)])
        print(f"{nat} {parent}  (complement: {len(kids)})")
        for k in kids:
            print(f"    - {k}")


if __name__ == "__main__":
    main()
