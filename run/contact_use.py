#!/usr/bin/env python3
"""What H-ARMY-CONTACT decided, from the records' `contact` decisions: answers by party size, soldiers sent against the
party's metal, and how often a party was let go. usage: run/contact_use.py <batch label> ..."""
import collections, glob, json, statistics, sys

for label in sys.argv[1:]:
    batch = glob.glob(f"run/matches/*{label}")[0]
    by_size = collections.defaultdict(lambda: {"sent": [], "called_off": 0, "n": 0, "forced": 0})
    games = 0
    for record in sorted(glob.glob(batch + "/[0-9]*/record-*.jsonl")):
        games += 1
        for line in open(record):
            if '"contact"' not in line:
                continue
            try:
                r = json.loads(line)
            except ValueError:
                continue
            if r.get("t") != "d" or r.get("kind") != "contact":
                continue
            party, sent = r["inputs"]["party"], r["outputs"]["sent"]
            row = by_size["1" if party == 1 else "2-3" if party <= 3 else "4-7" if party <= 7 else "8+"]
            row["n"] += 1
            row["forced"] += r["inputs"]["forced"]
            if sent:
                row["sent"].append(sent)
            else:
                row["called_off"] += 1
    print(f"## {label}: {games} games")
    for size in ("1", "2-3", "4-7", "8+"):
        row = by_size[size]
        if row["n"]:
            sent = row["sent"]
            print(f"   party of {size:>3}: {row['n'] / games:5.1f} decisions a game, answered with median {statistics.median(sent) if sent else 0:4.1f} (90th pct {sorted(sent)[int(len(sent) * 0.9)] if sent else 0}), called off {100 * row['called_off'] // row['n']} %, at the lab or commander {100 * row['forced'] // row['n']} %")
