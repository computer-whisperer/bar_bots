#!/usr/bin/env python3
"""Checks the duel-table combat predictor against engagements of recorded matches (with opponent ground truth).

usage: run/predict_check.py run/matches/<batch> [...]
For every engagement where both sides had soldiers on the spot: predicted power ratio against what happened.
"""
import csv, glob, json, math, os, subprocess, sys

# How much a turret's metal counts for in a fight; set TURRET_WORTH in the environment to try others.
TURRET_WORTH = float(os.environ.get("TURRET_WORTH", "1.5"))

def table():
    out = {}
    for name in ("tight", "wide"):
        for r in csv.DictReader(open(f"docs/data/duels-2026-09-19/{name}-pairs.csv")):
            out.setdefault((r["unit"], r["against"]), []).append(float(r["mean_margin"]))
    return {k: sum(v) / len(v) for k, v in out.items()}

MARGIN = table()

def effectiveness(u, v):
    """How much one metal of u is worth against one metal of v (Lanchester square law inverted from the duel margin)."""
    m = max(-0.95, min(0.95, MARGIN.get((u, v), 0.0)))
    return 1 / (1 - m * m) if m >= 0 else (1 - m * m)

def power(side, other, metal):
    total_other = sum(metal(n) * k for n, k in other.items()) or 1
    p = 0.0
    for u, count in side.items():
        e = sum(metal(v) * k / total_other * effectiveness(u, v) for v, k in other.items()) if other else 1.0
        p += metal(u) * count * math.sqrt(e)
    return p

def load_analyzer():
    source = open("run/analyze_match.py").read().replace("\nmain()\n", "\n")
    scope = {}
    exec(compile(source, "analyze_match", "exec"), scope)
    return scope

def participants(match, e, side):
    """Everyone of `side` who came within 900 of the engagement's centre while it lasted: {name: count}, turret metal."""
    seen = {}
    cx, cz = e["centre"]
    for frame in range(e["start"] - 150, e["end"] + 1, 120):
        units = match.ours_at(frame) if side == "ours" else match.theirs_at(frame)
        for u in units:
            building = (u[5] & 1) if side == "ours" else u[5]
            if not building and math.dist((u[2], u[3]), (cx, cz)) < 900:
                seen[u[0]] = u[1]
    soldiers, turrets = {}, 0.0
    for name in seen.values():
        if match.cls(name) == "army":
            soldiers[name] = soldiers.get(name, 0) + 1
        elif match.cls(name) == "turret":
            turrets += match.metal(name)
    return soldiers, turrets

def main():
    am = load_analyzer()
    rows = []
    for batch in sys.argv[1:]:
        for directory in sorted(glob.glob(os.path.join(batch, "[0-9][0-9]"))):
            if not glob.glob(directory + "/truth-*.jsonl"):
                continue
            match = am["Match"](directory)
            for e in am["engagements"](match):
                # Fighting losses only: a raider that kills three extractors and dies has not won a fight.
                fighting = lambda side: sum(match.metal(n) * k for n, k in e["lost"][side].items() if match.cls(n) in ("army", "turret"))
                lost_o, lost_t = fighting("ours"), fighting("theirs")
                if lost_o + lost_t < 400:
                    continue
                (ours, our_turrets), (theirs, their_turrets) = participants(match, e, "ours"), participants(match, e, "theirs")
                if not ours or not theirs:
                    continue
                po = power(ours, theirs, match.metal) + TURRET_WORTH * our_turrets
                pt = power(theirs, ours, match.metal) + TURRET_WORTH * their_turrets
                plain_o = sum(match.metal(n) * k for n, k in ours.items()) + TURRET_WORTH * our_turrets
                plain_t = sum(match.metal(n) * k for n, k in theirs.items()) + TURRET_WORTH * their_turrets
                # A decisive fight: somebody with a real force on the spot lost at least half of it. The outcome is who
                # lost the larger share of what they brought.
                share_o, share_t = lost_o / max(plain_o, 1), lost_t / max(plain_t, 1)
                if min(plain_o, plain_t) < 400 or max(share_o, share_t) < 0.5:
                    continue
                rows.append((2 * math.log(po / max(pt, 1)), 2 * math.log(plain_o / max(plain_t, 1)), math.log((share_t + 0.05) / (share_o + 0.05))))
    n = len(rows)
    agree = lambda i, subset=None: sum(1 for r in (subset or rows) if (r[i] > 0) == (r[2] > 0)) / len(subset or rows)
    def corr(i):
        xs, ys = [r[i] for r in rows], [r[2] for r in rows]
        mx, my = sum(xs) / n, sum(ys) / n
        return sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / math.sqrt(sum((x - mx) ** 2 for x in xs) * sum((y - my) ** 2 for y in ys))
    print(f"{n} engagements with soldiers of both sides taking part and 400+ metal of soldiers and turrets lost")
    print(f"who lost the smaller share of its force, predicted right: matchup-weighted {agree(0):.2f}, plain metal {agree(1):.2f}")
    print(f"correlation of log power ratio with log loss ratio: matchup-weighted {corr(0):.2f}, plain metal {corr(1):.2f}")
    for label, bound in (("1.5:1", 1.5), ("2:1", 2.0), ("3:1", 3.0)):
        for i, name in ((0, "matchup-weighted"), (1, "plain metal")):
            confident = [r for r in rows if abs(r[i]) > math.log(bound)]
            if confident:
                print(f"  {name} ratio beyond {label}: {len(confident)} cases, right {agree(i, confident):.2f}")

main()
