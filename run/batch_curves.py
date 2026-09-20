#!/usr/bin/env python3
"""Mean curves of a batch, by A/B arm and start corner: extractors, builders, army value and turrets, ours/theirs.

usage: run/batch_curves.py run/matches/<batch> [--by-opening] [minute ...]     (default minutes 2 3 4 6 8 10 12 15)
`--by-opening` also splits by the opponent's first factory (bot lab or vehicle plant; needs results written with
WITHIN_REASON_OBSERVE=1 by an arena that records `opponent_first_factory`).
Reads results.jsonl and each match's `analyze_match.py --json`.
"""
import collections, json, os, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    batch = sys.argv[1]
    by_opening = "--by-opening" in sys.argv
    minutes = [int(m) for m in sys.argv[2:] if m != "--by-opening"] or [2, 3, 4, 6, 8, 10, 12, 15]
    points = collections.defaultdict(list)
    for line in open(os.path.join(batch, "results.jsonl")):
        result = json.loads(line)
        match = os.path.join(batch, f"{result['index']:02d}")
        report = json.loads(subprocess.run([sys.executable, os.path.join(HERE, "analyze_match.py"), match, "--json"], capture_output=True, text=True, check=True).stdout)
        for point in report["curve"]:
            opening = {"lab": "bots", "vp": "veh"}.get(result.get("opponent_first_factory", "")[3:], "?") if by_opening else ""
            points[(result.get("arm") or "-", result["our_corner"] + (" " + opening if by_opening else ""), point["minute"])].append(point)
    print("arm corner min games | extractors | builders | army value | turrets   (ours/theirs)")
    for arm, corner in sorted({(a, c) for a, c, _ in points}):
        for minute in minutes:
            games = points.get((arm, corner, minute))
            if not games:
                continue
            def mean(kind, index):
                return "/".join(f"{sum((g[side].get(kind) or [0, 0])[index] for g in games) / len(games):.1f}" for side in ("ours", "theirs"))
            print(f"{arm:>3} {corner:>9} {minute:>3} {len(games):>5} | {mean('extractor', 0):>10} | {mean('builder', 0):>8} | {mean('army', 1):>13} | {mean('turret', 0)}")


if __name__ == "__main__":
    main()
