#!/usr/bin/env python3
"""Derive data/units.csv from the game's unit definition files (no game file is copied, only the numbers we use).

usage: tools/extract_units.py <path to Beyond-All-Reason checkout> > data/units.csv

Reads the top-level numeric fields of each unit's Lua table with a line regex (the files are machine-formatted: two tabs
for unit fields, three for customparams). Units: both commanders, the tier-1 land economy, the bot lab and vehicle plant
and everything those two factories can build.
"""
import re, subprocess, sys, pathlib

root = pathlib.Path(sys.argv[1])
FIELDS = ["metalcost", "energycost", "buildtime", "workertime", "builddistance", "speed", "metalmake", "energymake",
          "energyupkeep", "extractsmetal", "windgenerator", "metalstorage", "energystorage", "health"]
CUSTOM = ["energyconv_capacity", "energyconv_efficiency"]
FIXED = ["com", "mex", "win", "solar", "advsol", "makr", "estor", "mstor", "lab", "vp", "nanotc", "llt", "rad"]

files = {p.stem: p for p in (root / "units").rglob("*.lua")}

def parse(name):
    text = files[name].read_text(errors="replace")
    row = {}
    for key, value in re.findall(r"^\t\t(\w+) = ([-\d.]+),\s*$", text, re.M):
        if key in FIELDS:
            row[key] = value
    for key, value in re.findall(r"^\t\t\t(\w+) = ([-\d.]+),\s*$", text, re.M):
        if key in CUSTOM:
            row[key] = value
    block = re.search(r"^\t\tbuildoptions = \{(.*?)^\t\t\},", text, re.M | re.S)
    options = re.findall(r'"(\w+)"', block[1]) if block else []
    weapons = len(re.findall(r"^\t\t\t\[\d+\] = \{\s*$", text, re.M)) if "weapons = {" in text else 0
    return row, options, weapons

commit = subprocess.run(["git", "-C", str(root), "log", "-1", "--format=%H %ad", "--date=short"],
                        capture_output=True, text=True).stdout.strip()
print(f"# Derived from Beyond-All-Reason units/**/*.lua at commit {commit} by crates/buildorder/tools/extract_units.py.")
print("# Raw definition values before gamedata/alldefs_post.lua (default mod options leave these fields unchanged).")
print("# role: com, eco building, factory, builder (mobile), nano, turret, army. Empty cell = field absent in the file.")
print("name,role,factory," + ",".join(FIELDS + CUSTOM))
for side in ("arm", "cor"):
    rows = [(side + s, None) for s in FIXED]
    for fac in ("lab", "vp"):
        _, options, _ = parse(side + fac)
        rows += [(o, fac) for o in options]
    for name, fac in rows:
        row, options, weapons = parse(name)
        suffix = name[3:]
        if suffix == "com": role = "com"
        elif suffix in ("lab", "vp"): role = "factory"
        elif suffix == "nanotc": role = "nano"
        elif suffix in ("llt", "rad"): role = "turret"
        elif fac is None: role = "eco"
        elif "workertime" in row: role = "builder"
        else: role = "army"
        print(",".join([name, role, fac or ""] + [row.get(f, "") for f in FIELDS + CUSTOM]))
