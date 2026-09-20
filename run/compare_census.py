#!/usr/bin/env python3
"""Side-by-side timeline of our units and the opponent's from a match's engine.log (needs WITHIN_REASON_OBSERVE=1).

usage: run/compare_census.py run/matches/<batch>/<NN> [--detail MINUTE]
"""
import re, sys, collections

CLASSES = [  # first match wins; suffix after the 3-letter faction prefix
    ("mex", r"mex$|moho$"), ("energy", r"win$|solar$|advsol$|tide$|fus$|geo$"), ("conv", r"makr$"),
    ("store", r"mstor$|estor$"), ("lab", r"lab$|vp$|ap$|sy$|alab$|avp$|hp$"), ("nano", r"nanotc$"),
    ("turret", r"llt$|hllt$|hlt$|beamer$|rl$|guard$|pb$|dl$|ferret$|madsam$|mg$|rad$|jamt$|drag$|fort$"),
    ("com", r"^com$"), ("cons", r"^(ck|cv|ca|cs|ack|acv|aca|rectr|necro|fark|consul)$"),
]

def classify(name):
    suffix = name[3:]
    for label, pattern in CLASSES:
        if re.search(pattern, suffix):
            return label
    return "army"

def read(path):
    rows = collections.defaultdict(dict)
    for line in open(path, errors="replace"):
        m = re.search(r"census f=(\d+) (enemy|own) ?(.*)", line)
        if not m:
            continue
        units = {n: int(c) for n, c in re.findall(r"(\w+?)x(\d+)@", m[3])}
        rows[int(m[1]) // 1800][m[2]] = units
    return rows

def main():
    match = sys.argv[1]
    rows = read(f"{match}/engine.log")
    labels = [c for c, _ in CLASSES if c not in ("com",)] + ["army"]
    print("min | " + "  ".join(f"{l:>6}" for l in labels) + "   (ours / theirs)")
    for minute in sorted(rows):
        cells = []
        for label in labels:
            pair = []
            for side in ("own", "enemy"):
                units = rows[minute].get(side, {})
                pair.append(sum(n for name, n in units.items() if classify(name) == label))
            cells.append(f"{pair[0]:>2}/{pair[1]:<3}")
        print(f"{minute:>3} | " + "  ".join(cells))
    if "--detail" in sys.argv:
        minute = int(sys.argv[sys.argv.index("--detail") + 1])
        for side in ("own", "enemy"):
            print(side, " ".join(f"{n}x{c}" for n, c in sorted(rows[minute].get(side, {}).items())))

if __name__ == "__main__":
    main()
