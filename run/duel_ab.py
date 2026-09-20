#!/usr/bin/env python3
"""Compare two duel batches run on the same pairings: what a change to the first army's orders was worth.

    run/duel_ab.py <without dir> <with dir> [--by-them] [--min-duels N]

Prints, per pairing seen from the first unit's side: mean margin in each arm with its standard error, the
metal each side lost summed over the arm's duels (the ledger's "metal killed per metal lost"), and how spread
out each army stood when the first shot landed (`spread_x` in `duels.csv`, the probe added for this).
"""

import csv
import statistics
import sys
from pathlib import Path


def rows(directory):
    with open(Path(directory) / "duels.csv") as handle:
        return [r for r in csv.DictReader(handle) if r["reason"] != "spawn_failed"]


def tally(rs):
    margin = [float(r["value_left_x"]) - float(r["value_left_y"]) for r in rs]
    killed = sum((1 - float(r["value_left_y"])) * float(r["metal_y"]) for r in rs)
    lost = sum((1 - float(r["value_left_x"])) * float(r["metal_x"]) for r in rs)
    # Batches recorded before the dispersion probe have no spread columns; report them as unknown, not as zero.
    spread = [statistics.mean(float(r[c]) for r in rs) if all(r.get(c) for r in rs) else float("nan") for c in ("spread_x", "spread_y")]
    error = statistics.stdev(margin) / len(margin) ** 0.5 if len(margin) > 1 else 0.0
    return len(margin), statistics.mean(margin), error, killed, lost, spread


def main(argv):
    by_them = "--by-them" in argv
    floor, skip = 1, set()
    if "--min-duels" in argv:
        i = argv.index("--min-duels")
        floor, skip = int(argv[i + 1]), {i + 1}
    elif any(a.startswith("--min-duels=") for a in argv):
        floor = int(next(a for a in argv if a.startswith("--min-duels=")).split("=")[1])
    args = [a for i, a in enumerate(argv) if not a.startswith("--") and i not in skip]
    if len(args) != 2:
        print(__doc__)
        return 1
    without, with_it = rows(args[0]), rows(args[1])
    keys = sorted({(r["x"], r["y"]) for r in without} | {(r["x"], r["y"]) for r in with_it})
    if by_them:
        groups = {y: [k for k in keys if k[1] == y] for _, y in keys}
    else:
        groups = {f"{x} v {y}": [(x, y)] for x, y in keys}
    print(f"{'pairing':<22}{'n':>4}{'margin off':>12}{'margin on':>12}{'gain':>9}{'k/l off':>9}{'k/l on':>8}{'spread off':>12}{'spread on':>11}")
    overall = [0.0, 0.0, 0.0, 0.0]
    for name, members in groups.items():
        a = [r for r in without if (r["x"], r["y"]) in members]
        b = [r for r in with_it if (r["x"], r["y"]) in members]
        if len(a) < floor or len(b) < floor:
            continue
        na, ma, ea, ka, la, sa = tally(a)
        nb, mb, eb, kb, lb, sb = tally(b)
        overall = [overall[0] + ka, overall[1] + la, overall[2] + kb, overall[3] + lb]
        print(
            f"{name:<22}{na:>4}{ma:>+9.3f}+-{ea:<4.3f}{mb:>+9.3f}+-{eb:<4.3f}{mb - ma:>+9.3f}"
            f"{ka / max(la, 1):>9.2f}{kb / max(lb, 1):>8.2f}{sa[0]:>9.0f}/{sa[1]:<3.0f}{sb[0]:>8.0f}/{sb[1]:<3.0f}"
        )
    print(
        f"\nover everything: metal killed per metal lost {overall[0] / max(overall[1], 1):.2f} without,"
        f" {overall[2] / max(overall[3], 1):.2f} with"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
