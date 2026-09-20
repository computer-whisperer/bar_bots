#!/usr/bin/env python3
"""Adds up the verdicts of a batch (docs/harness/verdicts.md).  usage: run/tally_verdicts.py run/matches/<batch>"""
import glob, json, os, sys
from collections import Counter

batch = sys.argv[1].rstrip("/")
primary, contributing, rules, rows = Counter(), Counter(), [], []
for path in sorted(glob.glob(os.path.join(batch, "*", "verdict.json"))):
    try:
        v = json.load(open(path))
    except ValueError as problem:
        print(f"{path}: unreadable ({problem})")
        continue
    primary[v.get("primary_cause", "?")] += 1
    contributing.update(v.get("contributing", []))
    rules.append((os.path.basename(os.path.dirname(path)), v.get("preventing_rule", "")))
    moment = v.get("decisive_moment", {})
    rows.append(f"{os.path.basename(os.path.dirname(path))}: level to min {v.get('last_level_minute')}, decided by {v.get('decided_by_minute')}; "
                f"{v.get('primary_cause')} ({v.get('confidence')}) at {moment.get('time')} {moment.get('grid')}: {moment.get('what')}")
print(f"{sum(primary.values())} verdicts in {batch}\n\nprimary cause")
for tag, n in primary.most_common():
    print(f"  {n:>2}  {tag}")
print("\ncontributing")
for tag, n in contributing.most_common():
    print(f"  {n:>2}  {tag}")
print("\nper match")
print("\n".join("  " + r for r in rows))
print("\npreventing rules")
print("\n".join(f"  {m}: {r}" for m, r in rules))
