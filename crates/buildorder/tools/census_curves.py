#!/usr/bin/env python3
"""Per-minute curves of both sides from the once-a-minute census in engine.log (needs WITHIN_REASON_OBSERVE=1), as the
median over many matches: extractors, constructors, labs, nano turrets, army size and army metal value.

usage: tools/census_curves.py --costs <any record-0.jsonl> [--minutes 10] <match dir>... > curves.csv

The census counts what is alive at that minute, so army value here is the standing army, not everything ever built.
Unit costs come from the unit table in a match record's header (the engine's own numbers, every faction and tier).
"""
import json, pathlib, statistics, sys

sys.dont_write_bytecode = True  # importing from run/ must not leave a __pycache__ there
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[3] / "run"))
from compare_census import classify, read

args = sys.argv[1:]
def option(flag, default=None):
    if flag in args:
        i = args.index(flag)
        value = args[i + 1]
        del args[i:i + 2]
        return value
    return default

costs_path = option("--costs")
minutes = int(option("--minutes", "10"))
with open(costs_path) as f:
    cost = {d["name"]: d["metal"] for d in json.loads(f.readline())["unit_defs"]}

series = {}  # (side, minute, column) -> [value per match]
used = 0
for match in args:
    log = pathlib.Path(match) / "engine.log"
    rows = read(str(log)) if log.exists() else {}
    if not rows:
        continue
    used += 1
    for minute in range(1, minutes + 1):
        for side in ("own", "enemy"):
            if minute not in rows or side not in rows[minute]:
                continue  # that side is dead or the match is over: leave it out of the median rather than count zeros
            units = rows[minute][side]
            by_class = {}
            for name, n in units.items():
                by_class[classify(name)] = by_class.get(classify(name), 0) + n
            values = {c: by_class.get(c, 0) for c in ("mex", "cons", "lab", "nano", "turret", "army")}
            values["army_metal"] = sum(n * cost.get(name, 0) for name, n in units.items() if classify(name) == "army")
            for column, value in values.items():
                series.setdefault((side, minute, column), []).append(value)

columns = ["mex", "cons", "lab", "nano", "turret", "army", "army_metal"]
print(f"# median over {used} matches; 'own' is our bot, 'enemy' the opponent; n = matches still alive at that minute")
print("side,minute,n," + ",".join(columns))
for side in ("own", "enemy"):
    for minute in range(1, minutes + 1):
        if (side, minute, "mex") in series:
            cells = [f"{statistics.median(series[(side, minute, c)]):g}" for c in columns]
            print(f"{side},{minute},{len(series[(side, minute, 'mex')])}," + ",".join(cells))
