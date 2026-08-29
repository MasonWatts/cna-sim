#!/usr/bin/env python3
"""Convert an OB-chart reconstruction draft (from tools/ob_chart.py, saved per nation in
docs/source-materials/*-ob-*.DRAFT.txt) into an Order-of-Battle RON file for the engine
(reference::ob::OrderOfBattle).

Best-effort: maps each formation's morale + each unit's TSV row (level/unit/abbr/id/toe/
arrives/note/parent) into ObUnit. Fields the OCR couldn't resolve are emitted with a
`// TODO` marker for verification. Output is a DRAFT — verify id codes and weapon-name
strings against the §4.46/§4.47-49 charts before trusting combat values.

Usage:  ob_tsv_to_ron.py <draft.txt> > data/ob/<nation>.ron
"""
import re
import sys


def ron_str(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def org_size(name):
    n = name.lower()
    if "div" in n and "hq" in n or "division" in n:
        return "Division"
    if re.search(r"\b(regt|regiment|bde|brigade|gruppo|command|group)\b", n):
        return "StandardBrigade"
    if re.search(r"\b(coy|koy|company|bty|battery|squadron|squadrone|detachment)\b", n):
        return "Company"
    return "Battalion"  # bn / default


def type_sig(name):
    """A coarse unit-type signature for ID inference (§4.46 codes are per type)."""
    n = name.lower()
    if re.search(r"\b(div|division)\b.*\bhq", n) or re.search(r"\bhqe?\b", n) and "div" in n:
        return "div-hq"
    if re.search(r"machinegun|mitrg|\bmg\b", n):
        return "mg"
    if re.search(r"anti-?air|anti-?aircraft|flak|\baa\b", n):
        return "aa"
    if re.search(r"anti-?tank", n):
        return "at-coy" if re.search(r"coy|company", n) else "at-bn"
    if re.search(r"artiller|artiglier|\barty\b", n):
        return "arty-regt" if re.search(r"regt|regiment", n) else "arty-bn"
    if re.search(r"engineer|pioneer", n):
        return "engineer"
    if re.search(r"bersaglieri", n):
        return "bersaglieri"
    if re.search(r"recce|recon|esplorante|aufkl", n):
        return "recce"
    if re.search(r"tank|\(m\)|\(l\)|corazzat|panzer", n):
        return "tank"
    if re.search(r"legion|\blgn\b", n) and re.search(r"hq|ho\b", n):
        return "legion-hq"
    if re.search(r"regt|regiment|\bregt hq\b|\bregt ho\b", n) and re.search(r"hq|ho\b", n):
        return "inf-regt-hq"
    if re.search(r"\bbn\b|battalion|bataillon|coy|koy|company|infantry|fanteria", n):
        return "inf-bn"
    return "other:" + re.sub(r"[^a-z]+", "", n)[:12]


def parse_comp(toe):
    """Return (ron_composition, todo) from the TOE cell."""
    t = toe.strip()
    if not t or t.upper() == "N":
        return "Organic(Normal)", not t  # empty -> Normal but flag
    m = re.match(r"[UО]@?\s*(\d+)", t, re.I)  # understrength U@x
    if t[:1] in "Uu" and m:
        return f"Organic(Under({m.group(1)}))", False
    m = re.match(r"[OО]@?\s*(\d+)", t)  # overstrength O@x
    if t[:1] in "Oo" and m and "@" in t:
        return f"Organic(Over({m.group(1)}))", False
    # Weapons: comma-separated "<count> x <system>"
    groups = []
    todo = False
    for part in t.split(","):
        p = part.strip()
        if not p:
            continue
        wm = re.match(r"(\d+)\s*[x×X]\s*(.+)", p)
        if not wm:
            wm = re.match(r"(\d+)\s+([A-Za-z].+)", p)  # "8 Light AA" (missing separator)
        if wm:
            count = int(wm.group(1))
            system = re.sub(r"\s+", " ", wm.group(2)).strip()
            groups.append((system, count))
        else:
            todo = True  # couldn't parse this fragment
    if groups:
        inner = ", ".join(f"({ron_str(s)}, {c})" for s, c in groups)
        return f"Weapons([{inner}])", todo
    return "Organic(Normal)", True  # unrecognized -> default + flag


def parse_arrival(arr):
    a = arr.strip()
    if not a:
        return "Deployed", True  # empty -> assume deployed but flag
    if a.upper() == "D":
        return "Deployed", False
    m = re.match(r"(\d+)\s*/\s*(\d+)", a)  # stage/game-turn
    if m:
        return f"Turn(game_turn: {int(m.group(2))}, stage: {int(m.group(1))})", False
    return "Deployed", True


def parse_morale(text):
    """Return (basic, arrival_or_None, dynamic, note)."""
    t = text
    low = t.lower()
    if "that of" in low:
        return 0, None, True, "morale = that of attached units"
    if "as listed" in low:
        return 0, None, False, "morale as listed per unit"
    # main signed number
    mains = re.findall(r"[+\-]\s*\d+|\b\d+\b", t)
    basic = int(re.sub(r"\s+", "", mains[0])) if mains else 0
    # numeric parenthetical = arrival ('=' is an OCR '-')
    pm = re.search(r"\(\s*([+\-=]?\s*\d+)\s*\)", t)
    arrival = None
    if pm:
        v = re.sub(r"\s+", "", pm.group(1)).replace("=", "-")
        arrival = int(v)
    return basic, arrival, False, ""


# Cyrillic homoglyphs the OCR emits for the (Latin, lowercase) §4.46 id codes.
CYRILLIC = str.maketrans({
    "а": "a", "в": "b", "с": "c", "д": "d", "е": "e", "н": "h", "і": "i", "к": "k",
    "м": "m", "н": "n", "о": "o", "р": "p", "т": "t", "у": "y", "х": "x", "ц": "u",
})


def normalize_id(raw, valid):
    """Lowercase + de-homoglyph an OCR'd id; return it only if it's a real chart key."""
    s = re.sub(r"[^a-z]", "", raw.strip().translate(CYRILLIC).lower())
    if not valid:
        return s
    return s if s in valid else ""


def load_char_types(path):
    """Map each id code -> its unit_type string from a §4.46 characteristics file."""
    if not path:
        return {}
    text = open(path, encoding="utf-8").read()
    return dict(re.findall(r'"([a-z]{1,3})":\s*\(\s*unit_type:\s*"([^"]+)"', text))


def load_valid_ids(path):
    return set(load_char_types(path).keys())


def org_size_from_type(unit_type):
    """Derive OrgSize from a characteristics unit_type suffix. Returns None for
    'Headquarters' (no size in the chart) or types with no equivalent suffix."""
    if not unit_type:
        return None
    t = unit_type.lower()
    if "coy-equivalent" in t or "company-equivalent" in t:
        return "Company"
    if "bde-equivalent" in t or "brigade-equivalent" in t:
        return "StandardBrigade"
    if "bn-equivalent" in t:
        return "Battalion"
    return None  # Headquarters / sizeless types -> caller uses hierarchy


def formation_org(name):
    n = name.lower()
    if "division" in n:
        return "Division"
    if "battle" in n and "group" in n:
        return "BattleGroup"
    return "StandardBrigade"


def main():
    lines = open(sys.argv[1], encoding="utf-8").read().splitlines()
    chars_path = sys.argv[sys.argv.index("--chars") + 1] if "--chars" in sys.argv else None
    char_types = load_char_types(chars_path)
    valid_ids = set(char_types.keys())
    formations = []
    cur = None
    in_tsv = False
    for line in lines:
        if line.startswith("# Formation:"):
            cur = {"name": line[len("# Formation:"):].strip(), "morale": "", "units": []}
            formations.append(cur)
            in_tsv = False
        elif cur is not None and re.search(r"(?i)morale:", line) and line.startswith("#"):
            cur["morale"] = line.lstrip("# ").strip()
        elif line.startswith("## TSV"):
            in_tsv = True
        elif in_tsv and cur is not None and re.match(r"^\d\t", line):
            f = line.split("\t")
            while len(f) < 8:
                f.append("")
            _lvl, name, abbr, uid, toe, arr, note, parent = f[:8]
            cur["units"].append({
                "name": name.strip(), "abbr": abbr.strip(),
                "id_raw": uid.strip(), "id": normalize_id(uid, valid_ids),
                "toe": toe, "arr": arr, "note": note.strip(), "parent": parent.strip(),
            })

    # ---- ID inference (conservative — accuracy over coverage). Fill blanks from a
    # same-formation sibling; else the majority id for that unit *type*, but only when that
    # type-mode is corroborated (>=2 instances). Every proposed id is a real chart key. ----
    from collections import Counter, defaultdict
    type_counts = defaultdict(Counter)
    for f in formations:
        for u in f["units"]:
            if u["id"]:
                type_counts[type_sig(u["name"])][u["id"]] += 1
    type_mode = {}
    for sig, c in type_counts.items():
        uid, cnt = c.most_common(1)[0]
        # Only infer from a well-supported, dominant mode (kills cross-type OCR pollution).
        if cnt >= 3 and cnt >= 0.6 * sum(c.values()):
            type_mode[sig] = (uid, cnt)

    filled_sib = filled_type = still_blank = 0
    for f in formations:
        sib = {}  # type -> Counter(id): same-type units share a code within a formation
        for u in f["units"]:
            if u["id"]:
                sib.setdefault(type_sig(u["name"]), Counter())[u["id"]] += 1
        for u in f["units"]:
            u["id_src"] = "ocr" if u["id"] else None
            if u["id"]:
                continue
            cand = sib.get(type_sig(u["name"]))
            if cand:
                u["id"], u["id_src"] = cand.most_common(1)[0][0], "sibling"
                filled_sib += 1
            elif type_sig(u["name"]) in type_mode:
                u["id"], u["id_src"] = type_mode[type_sig(u["name"])][0], "type"
                filled_type += 1
            else:
                still_blank += 1  # left for manual entry (esp. HQs, rare types)
    print(f"// ID inference: {filled_sib} sibling, {filled_type} type, {still_blank} unknown "
          f"(valid-id set: {len(valid_ids)}).", file=sys.stderr)

    def unit_org(u, form_org):
        # An HQ's org size is its command echelon (hierarchy), NOT its id — some HQ counters
        # carry a combat id (e.g. a division HQ with 1 tank), which only gives combat stats.
        n = u["name"].lower()
        is_hq = bool(re.search(r"hq|headquarters|\bho\b|\bcommand\b", n)) \
            or char_types.get(u["id"]) == "Headquarters"
        if is_hq:
            return form_org if u["parent"] == "" else "StandardBrigade"
        # Combat units: size is authoritative from the id's characteristics unit_type.
        size = org_size_from_type(char_types.get(u["id"]))
        if size:
            return size
        # Unknown id, no size: fall back (top heads the formation, else battalion).
        return form_org if u["parent"] == "" else "Battalion"

    print("// Generated by tools/ob_tsv_to_ron.py — DRAFT. Ids validated against the §4.46")
    print("// characteristics; org_size derived from id unit_type (HQ via hierarchy).")
    print("// 'id inferred' = filled by sibling/type, verify. 'TODO id?' = supply.")
    print("(")
    print("    formations: [")
    for f in formations:
        basic, arrival, dynamic, mnote = parse_morale(f["morale"])
        top_org = formation_org(f["name"])
        print("        (")
        print(f"            name: {ron_str(f['name'])},")
        print(f"            org_size: {top_org},")
        print(f"            basic_morale: {basic},")
        if arrival is not None:
            print(f"            arrival_morale: Some({arrival}),")
        if dynamic:
            print("            dynamic: true,")
        if mnote:
            print(f"            // {mnote}")
        print("            units: [")
        for u in f["units"]:
            comp, comp_todo = parse_comp(u["toe"])
            arrv, _ = parse_arrival(u["arr"])
            todo = []
            if not u["id"]:
                raw = f" (was {u['id_raw']!r})" if u["id_raw"] else ""
                todo.append("id?" + raw)
            if comp_todo:
                todo.append(f"toe? {u['toe'].strip()!r}")
            notes = []
            if todo:
                notes.append("TODO " + "; ".join(todo))
            if u["id_src"] in ("sibling", "type"):
                notes.append(f"id {u['id_src']}")
            flag = ("  // " + " | ".join(notes)) if notes else ""
            print(
                f"                (name: {ron_str(u['name'])}, abbr: {ron_str(u['abbr'])}, "
                f"id: {ron_str(u['id'])}, comp: {comp}, org_size: {unit_org(u, top_org)}, "
                f"arrival: {arrv}, parent: {ron_str(u['parent'])}),{flag}"
            )
        print("            ],")
        print("        ),")
    print("    ],")
    print(")")


if __name__ == "__main__":
    main()
