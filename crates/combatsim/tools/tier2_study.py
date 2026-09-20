#!/usr/bin/env python3
"""The tier-2 army study: what BARb fields, and what beats it at equal metal.

    tools/tier2_study.py army  run/matches/<batch>/<NN> ...      > docs/studies/data/tier2-barb-army.csv
    tools/tier2_study.py fights [--seeds 24] [--jobs 8] [--out docs/studies/data]
    tools/tier2_study.py energy [--seeds 24] [--jobs 8]          > docs/studies/data/tier2-energy.csv
    tools/tier2_study.py sustain [--bar upstream/Beyond-All-Reason]

`army` reads the per-match truth logs (`docs/harness/record-format.md`: one line every two seconds, every enemy
unit that really exists) and tabulates BARb's mean composition at minutes 15, 20, 25 and 30, per faction it
plays and separated into mobile and static. Its faction comes from `script.txt`; it builds a small minority of
the other faction's units and those are kept as they are. Units still under construction are not counted.

`fights` builds those armies at a metal budget, builds our candidate mixes at the same budget, and asks
`target/release/combatsim` who wins, one process per seed so the spread across seeds is real and reportable.
Both sides are laid out front to back, shortest weapon first, which is what the CLI's group order means.

`energy` reruns the mixes worth watching while sweeping our own energy income, because every beam and
lightning weapon in the tier-2 line charges energy a shot. `sustain` is arithmetic, not simulation: what a
recommended mix costs a second and how much factory build power it takes to keep up with it.
"""
import collections, csv, glob, json, os, pathlib, re, statistics, subprocess, sys
from concurrent.futures import ThreadPoolExecutor

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(HERE + "/../../..")
UNITS = json.load(open(ROOT + "/crates/combatsim/data/units.json"))["units"]
SIM = ROOT + "/target/release/combatsim"
MINUTES = [15, 20, 25, 30]
FPS = 30


def metal(name):
    return UNITS[name]["metal"]


def reach(name):
    """Longest range against ground; 0 for anti-air, radar, builders and anything else that cannot fight one."""
    u = UNITS.get(name)
    if u is None:
        return 0.0
    live = [w for w in u["weapons"]
            if w["only_targets"] != "VTOL" and w["damage"].get("default", 0) > 0
            and not (w["paralyzer"] or w["stockpile"] or w["water_only"] or w["command_fire"])]
    return max([w["range"] for w in live], default=0.0)


# ----------------------------------------------------------------------------------------- what BARb fields

def faction_of(match_dir):
    text = open(match_dir + "/script.txt").read()
    teams = dict(re.findall(r"\[TEAM(\d)\] \{ TeamLeader=\d+; AllyTeam=\d+; Side=(\w+);", text))
    ais = re.findall(r"\[AI(\d)\] \{ Name=\w+; Team=(\d); Host=\d; ShortName=(\w+)", text)
    barb = [team for _, team, name in ais if name == "BARb"]
    return teams[barb[0]] if barb else None


def army(dirs, out):
    rows = collections.defaultdict(collections.Counter)
    matches = collections.Counter()
    for match_dir in dirs:
        truth = glob.glob(match_dir + "/truth-*.jsonl")
        if not truth:
            continue
        faction = faction_of(match_dir)
        snapshots = {m: None for m in MINUTES}
        with open(truth[0]) as handle:
            for line in handle:
                record = json.loads(line)
                for minute in MINUTES:
                    if snapshots[minute] is None and record["f"] >= minute * 60 * FPS:
                        snapshots[minute] = record
        for minute, record in snapshots.items():
            if record is None:
                continue
            matches[(faction, minute)] += 1
            for unit in record["enemy"]:
                if len(unit) < 6 or unit[5] == 0:  # finished, not still being built
                    rows[(faction, minute)][unit[1]] += 1
    writer = csv.writer(out, lineterminator="\n")
    writer.writerow(["faction", "minute", "matches", "group", "unit", "mean_count", "metal_each", "tech"])
    for (faction, minute), counts in sorted(rows.items()):
        n = matches[(faction, minute)]
        for name, count in sorted(counts.items(), key=lambda kv: -kv[1] * metal(kv[0]) if kv[0] in UNITS else 0):
            if name not in UNITS:
                continue  # economy buildings and the like are not in the combat table
            group = "static" if UNITS[name]["speed"] == 0 else "mobile"
            writer.writerow([faction, minute, n, group, name, round(count / n, 3),
                             round(metal(name)), UNITS[name]["tech"]])


def read_army(path):
    """faction -> minute -> group -> {unit: mean count}, fighting units only."""
    out = collections.defaultdict(lambda: collections.defaultdict(dict))
    for row in csv.DictReader(open(path)):
        if reach(row["unit"]) <= 0.0:
            continue  # anti-air, radar and builders take no part in a land fight
        out[row["faction"]][int(row["minute"])].setdefault(row["group"], {})[row["unit"]] = float(row["mean_count"])
    return out


# ------------------------------------------------------------------------------------- building a force

def scale(mix, budget, keep_all=False):
    """`mix` is {unit: relative weight in units}; scale it to `budget` metal and round to whole units.

    Rounding down loses metal, and where that metal goes decides what the force is: giving it all to the
    cheapest type turned BARb's one Flea into twenty-one of them. It goes instead to whichever type is
    furthest below its exact share, one unit at a time, so the proportions survive the rounding. `keep_all`
    is for our own mixes, where naming a type means it has to be in the force even at the smallest budget;
    BARb's measured composition instead lets its rare types round away.
    """
    total = sum(weight * metal(name) for name, weight in mix.items())
    if total <= 0:
        return {}
    exact = {name: max(weight * budget / total, 1e-6) for name, weight in mix.items()}
    counts = {name: max(int(value), 1 if keep_all else 0) for name, value in exact.items()}
    spent = lambda: sum(n * metal(name) for name, n in counts.items())
    floor = 1 if keep_all else 0
    while spent() > budget * 1.02:
        over = [name for name in counts if counts[name] > floor]
        if not over:
            break
        counts[max(over, key=lambda name: counts[name] / exact[name])] -= 1
    while spent() < budget * 0.98:
        room = [name for name in counts if spent() + metal(name) <= budget * 1.02]
        if not room:
            break
        counts[min(room, key=lambda name: (counts[name] + 1) / exact[name])] += 1
    return {name: n for name, n in counts.items() if n > 0}


def by_weight(mix, budget):
    """`mix` is {unit: share of the metal}; turn it into counts at `budget`."""
    return scale({name: share / metal(name) for name, share in mix.items()}, budget, keep_all=True)


def spec(counts):
    """A `--a`/`--b` string, shortest-ranged type first so a screen stands in front of what it screens."""
    order = sorted(counts, key=lambda name: (reach(name), name))
    return ",".join(f"{name}:{counts[name]}" for name in order if counts[name] > 0)


def cost(counts):
    return sum(n * metal(name) for name, n in counts.items())


# ------------------------------------------------------------------------------------------- the fights

# Our candidates. Values are shares of the metal budget. The tier-1 line is what the brain builds today
# (docs/knowledge/army.md); everything else is an advanced bot lab unit.
T1_ARM = {"armham": 0.45, "armwar": 0.25, "armrock": 0.30}
T1_COR = {"corthud": 0.55, "corstorm": 0.45}


def blend(base, extra, share):
    out = {name: weight * (1.0 - share) for name, weight in base.items()}
    for name, weight in extra.items():
        out[name] = out.get(name, 0.0) + weight * share
    return out


MIXES = {
    "Armada": dict(
        [("t1-line", T1_ARM), ("t1-mace", {"armham": 1.0})]
        + [(name[3:], {name: 1.0}) for name in
           ("armzeus", "armmav", "armfido", "armsptk", "armsnipe", "armfboy", "armfast", "armamph")]
        + [("t1+" + name[3:], blend(T1_ARM, {name: 1.0}, 0.5)) for name in
           ("armzeus", "armmav", "armfido", "armsptk", "armsnipe", "armfboy")]
        + [("zeus+fido", {"armzeus": 0.5, "armfido": 0.5}),
           ("zeus+sptk", {"armzeus": 0.5, "armsptk": 0.5}),
           ("zeus+snipe", {"armzeus": 0.6, "armsnipe": 0.4}),
           ("mace+zeus", {"armham": 0.5, "armzeus": 0.5}),
           ("t1+zeus+fido", blend(T1_ARM, {"armzeus": 0.5, "armfido": 0.5}, 0.66)),
           ("t1+zeus(25)", blend(T1_ARM, {"armzeus": 1.0}, 0.25)),
           ("t1+zeus(75)", blend(T1_ARM, {"armzeus": 1.0}, 0.75))]),
    "Cortex": dict(
        [("t1-line", T1_COR), ("t1-thud", {"corthud": 1.0})]
        + [(name[3:], {name: 1.0}) for name in
           ("corcan", "corsumo", "corpyro", "corhrk", "cormort", "cortermite", "coramph")]
        + [("t1+" + name[3:], blend(T1_COR, {name: 1.0}, 0.5)) for name in
           ("corcan", "corsumo", "corpyro", "corhrk", "cormort", "cortermite")]
        + [("can+mort", {"corcan": 0.5, "cormort": 0.5}),
           ("can+hrk", {"corcan": 0.6, "corhrk": 0.4}),
           ("termite+mort", {"cortermite": 0.5, "cormort": 0.5}),
           ("thud+can", {"corthud": 0.5, "corcan": 0.5}),
           ("t1+can+mort", blend(T1_COR, {"corcan": 0.5, "cormort": 0.5}, 0.66)),
           ("t1+can(25)", blend(T1_COR, {"corcan": 1.0}, 0.25)),
           ("t1+can(75)", blend(T1_COR, {"corcan": 1.0}, 0.75))]),
}

# What a tower line is: light towers, and the heavy towers BARb typically has beside them at that minute.
# Counts come from the static rows of the army table, rounded; see the study for the measured means.
TOWERS = {
    "Armada": {"light": "armllt", "heavy": {"armhlt": 2, "armbeamer": 2}},
    "Cortex": {"light": "corllt", "heavy": {"corhlt": 2, "corhllt": 2}},
}

# Both armies at minute 20-25 draw on a real grid: BARb's measured energy income over the four batches is a
# median of 314-406 a second with 7000 stored (the study says how that was counted). A defended base gets the
# storage; in the field both sides get the same, which is the fair comparison.
FIELD_ENERGY = ["--stored", "4000", "--income", "350"]
BASE_ENERGY = ["--stored-a", "4000", "--income-a", "350", "--stored-b", "7000", "--income-b", "350"]


def enemy_force(armies, faction, minute, tier1_only=False):
    mix = dict(armies[faction][minute]["mobile"])
    mix.pop("armcom", None)
    mix.pop("corcom", None)  # the commander is one unit and does not scale with a budget
    if tier1_only:
        mix = {name: n for name, n in mix.items() if UNITS[name]["tech"] == 1}
    return mix


def run(args):
    out = subprocess.run([SIM] + args, capture_output=True, text=True)
    if out.returncode != 0:
        sys.exit("combatsim: " + out.stderr + out.stdout)
    found = re.search(r"margin ([-+][\d.]+)", out.stdout)
    return float(found[1])


BUDGETS = [3000, 6000, 10000]
# `base<N>-hold` is the whole garrison standing still behind N light towers; `base<N>-out` is the same tower
# line with the mobile half coming out to meet us, which is what BARb's threat-aware pathing actually does.
# Towers hold either way: a building cannot move.
SCENARIOS = ["field20", "field25", "t1only", "base6-hold", "base6-out", "base12-hold", "base12-out"]
# The formation the two armies fight in is an input here and an outcome in the engine, and the area-damage
# units live or die by it (`docs/studies/combat-sim.md`, *Where it is wrong*). Everything is therefore run at
# the duel harness's own spacing and at a loose one, and a mix whose ranking moves between them is not a result.
SPACINGS = [56, 160]


def build_cells(armies):
    cells = []
    for ours in MIXES:
        for theirs in ("Armada", "Cortex"):
            for budget in BUDGETS:
                for scenario in SCENARIOS:
                    minute = 20 if scenario in ("field20", "t1only") else 25
                    enemy = scale(enemy_force(armies, theirs, minute, scenario == "t1only"), budget)
                    extra, hold, energy = {}, [], FIELD_ENERGY
                    if scenario.startswith("base"):
                        line = TOWERS[theirs]
                        extra = {line["light"]: int(scenario[4:].split("-")[0]), **line["heavy"]}
                        hold = ["--hold-b"] if scenario.endswith("-hold") else []
                        energy = BASE_ENERGY
                    for name, n in extra.items():
                        enemy[name] = enemy.get(name, 0) + n
                    for spacing in SPACINGS:
                        for mix_name, mix in MIXES[ours].items():
                            mine = by_weight(mix, budget)
                            cells.append(dict(ours=ours, theirs=theirs, budget=budget, scenario=scenario,
                                              spacing=spacing, mix=mix_name, a=spec(mine), b=spec(enemy),
                                              a_metal=cost(mine), b_metal=cost(enemy), tower_metal=cost(extra),
                                              args=energy + hold + ["--spacing", str(spacing)]))
    return cells


def fights(seeds, jobs, out_dir, army_csv):
    cells = build_cells(read_army(army_csv))
    tasks = [(cell, seed) for cell in cells for seed in range(seeds)]
    print(f"{len(cells)} cells x {seeds} seeds = {len(tasks)} fights", file=sys.stderr)
    with ThreadPoolExecutor(max_workers=jobs) as pool:
        results = list(pool.map(lambda t: run(["--a", t[0]["a"], "--b", t[0]["b"], "--seed", str(t[1])] + t[0]["args"]),
                                tasks))
    for cell in cells:
        cell["margins"] = []
    for (cell, _), margin in zip(tasks, results):
        cell["margins"].append(margin)
    for cell in cells:
        cell["margin"] = statistics.mean(cell["margins"])
        cell["sd"] = statistics.stdev(cell["margins"]) if seeds > 1 else 0.0
        cell["sem"] = cell["sd"] / seeds ** 0.5

    path = out_dir + "/tier2-fights.csv"
    with open(path, "w", newline="") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(["our_faction", "barb_faction", "budget", "spacing", "scenario", "mix", "margin", "sd",
                         "sem", "wins_of"])
        for cell in cells:
            writer.writerow([cell["ours"], cell["theirs"], cell["budget"], cell["spacing"], cell["scenario"],
                             cell["mix"], round(cell["margin"], 3), round(cell["sd"], 3), round(cell["sem"], 3),
                             f"{sum(1 for m in cell['margins'] if m > 0)}/{seeds}"])
    # The forces themselves go in their own file: each one appears in dozens of cells, and written out beside
    # every margin they were four fifths of the bytes.
    forces = out_dir + "/tier2-forces.csv"
    with open(forces, "w", newline="") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(["side", "faction", "budget", "key", "metal", "tower_metal", "force"])
        seen = set()
        for cell in cells:
            for side, faction, key, force, spent, towers in (
                    ("ours", cell["ours"], cell["mix"], cell["a"], cell["a_metal"], 0),
                    ("barb", cell["theirs"], cell["scenario"], cell["b"], cell["b_metal"], cell["tower_metal"])):
                row = (side, faction, cell["budget"], key)
                if row not in seen:
                    seen.add(row)
                    writer.writerow([*row, round(spent), round(towers), force])
    print(f"wrote {path} and {forces}", file=sys.stderr)
    summarise(cells)


def pooled(cells):
    """Mean and standard error over every seed of every cell in the group."""
    seeds = [m for cell in cells for m in cell["margins"]]
    sd = statistics.stdev(seeds) if len(seeds) > 1 else 0.0
    return statistics.mean(seeds), sd / len(seeds) ** 0.5


def summarise(cells):
    pick = lambda **want: [c for c in cells if all(c[k] == v for k, v in want.items())]
    for ours in MIXES:
        for spacing in SPACINGS:
            for scenario in SCENARIOS:
                print(f"\n== our {ours}, {scenario}, spacing {spacing} "
                      f"(both BARb factions pooled; +1 is a flawless win) ==")
                print(f"{'mix':16s}" + "".join(f"{b:>14d}" for b in BUDGETS) + "     mean")
                rows = []
                for mix in MIXES[ours]:
                    per = [pooled(pick(ours=ours, scenario=scenario, spacing=spacing, budget=b, mix=mix))
                           for b in BUDGETS]
                    rows.append((statistics.mean(m for m, _ in per), mix, per))
                for mean, mix, per in sorted(rows, reverse=True):
                    print(f"{mix:16s}" + "".join(f"  {m:+.2f}+-{e:.2f}" for m, e in per) + f"   {mean:+.2f}")


# ------------------------------------------------- how much our own energy income decides it, and what it costs

# Every beam and lightning weapon in the tier-2 line charges energy per shot, so a mix can be starved the same
# way a laser tower is (K-units-laser-towers-need-energy). These are the mixes the study ends up recommending
# plus the tier-1 line to compare against.
WATCHED = {
    "Armada": ["t1-line", "t1+zeus", "t1+fido", "zeus+fido", "t1+zeus+fido"],
    "Cortex": ["t1-line", "t1+can", "can", "t1+sumo", "t1+termite"],
}


def energy_sweep(seeds, jobs, out_dir, army_csv):
    armies = read_army(army_csv)
    incomes = [50, 150, 350, 700, 2000]
    cells = []
    for ours in MIXES:
        for theirs in ("Armada", "Cortex"):
            for scenario in ("field25", "base12-out"):
                enemy = scale(enemy_force(armies, theirs, 25), 6000)
                energy, hold = FIELD_ENERGY, []
                if scenario.startswith("base"):
                    line = TOWERS[theirs]
                    for name, n in {line["light"]: 12, **line["heavy"]}.items():
                        enemy[name] = enemy.get(name, 0) + n
                    energy = BASE_ENERGY
                for mix_name in WATCHED[ours]:
                    mine = by_weight(MIXES[ours][mix_name], 6000)
                    for income in incomes:
                        cells.append(dict(ours=ours, theirs=theirs, scenario=scenario, mix=mix_name,
                                          income=income, a=spec(mine), b=spec(enemy),
                                          args=energy + hold + ["--spacing", "100",
                                                                "--stored-a", "4000", "--income-a", str(income)]))
    tasks = [(cell, seed) for cell in cells for seed in range(seeds)]
    print(f"energy sweep: {len(tasks)} fights", file=sys.stderr)
    with ThreadPoolExecutor(max_workers=jobs) as pool:
        results = list(pool.map(lambda t: run(["--a", t[0]["a"], "--b", t[0]["b"], "--seed", str(t[1])] + t[0]["args"]),
                                tasks))
    for cell in cells:
        cell["margins"] = []
    for (cell, _), margin in zip(tasks, results):
        cell["margins"].append(margin)
    path = out_dir + "/tier2-energy.csv"
    with open(path, "w", newline="") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(["our_faction", "barb_faction", "scenario", "mix", "our_income", "margin", "sem"])
        for cell in cells:
            mean, sem = pooled([cell])
            writer.writerow([cell["ours"], cell["theirs"], cell["scenario"], cell["mix"], cell["income"],
                             round(mean, 3), round(sem, 3)])
    print("wrote " + path, file=sys.stderr)
    for ours in MIXES:
        for scenario in ("field25", "base12-out"):
            print(f"\n== our {ours}, {scenario}, 6000 metal, spacing 100: our energy income ==")
            print(f"{'mix':16s}" + "".join(f"{i:>10d}" for i in incomes))
            for mix_name in WATCHED[ours]:
                row = [pooled([c for c in cells if c["ours"] == ours and c["scenario"] == scenario
                               and c["mix"] == mix_name and c["income"] == i])[0] for i in incomes]
                print(f"{mix_name:16s}" + "".join(f"  {m:+7.2f}" for m in row))


def sustain(times):
    """What a mix costs to keep building: units a minute, energy a second, and how many labs' build power it
    needs. Tier-1 units come out of a tier-1 lab (build power 150) and tier-2 units out of the advanced lab
    (600), so the two are counted separately; a nanotower adds 200 to whichever it stands beside."""
    labs = {"Armada": ("armlab", "armalab"), "Cortex": ("corlab", "coralab")}
    print(f"{'faction / mix':26s}{'M/s':>6s}{'E/s':>7s}{'T1 labs':>9s}{'T2 labs':>9s}   units a minute")
    for faction, mixes in RECOMMENDED.items():
        power = {tier: times[name][2] for tier, name in zip((1, 2), labs[faction])}
        for mix_name, mix in mixes.items():
            for spend in (30.0, 45.0):
                rate = {n: share * spend / metal(n) for n, share in mix.items()}  # units a second
                energy = sum(rate[n] * times[n][1] for n in mix)
                duty = {tier: sum(rate[n] * times[n][0] / power[tier]
                                  for n in mix if UNITS[n]["tech"] == tier) for tier in (1, 2)}
                per_minute = " ".join(f"{n} x{rate[n] * 60:.1f}" for n in mix)
                print(f"{faction + ' ' + mix_name:26s}{spend:6.0f}{energy:7.0f}{duty[1]:9.2f}{duty[2]:9.2f}   "
                      + per_minute)


# Exactly the mixes `fights` ran, so the costs below belong to the margins above and not to a nearby mix.
RECOMMENDED = {
    "Armada": {"t1+zeus+fido": MIXES["Armada"]["t1+zeus+fido"], "t1 line only": T1_ARM},
    "Cortex": {"t1+can": MIXES["Cortex"]["t1+can"], "t1 line only": T1_COR},
}


def build_times(root):
    """buildtime, energycost and workertime straight out of the unit files, for the sustain arithmetic."""
    files = {p.stem: p for p in (pathlib.Path(root) / "units").rglob("*.lua")}
    wanted = sorted({n for mixes in RECOMMENDED.values() for mix in mixes.values() for n in mix}
                    | {"armlab", "armalab", "corlab", "coralab"})
    listed = ",".join('"%s"' % files[n] for n in wanted)
    body = ("local out = {} for _, path in ipairs({%s}) do for name, def in pairs(dofile(path)) do "
            "out[#out+1] = name .. '\\t' .. (def.buildtime or 0) .. '\\t' .. (def.energycost or 0) .. '\\t' "
            ".. (def.workertime or 0) end end io.write(table.concat(out, '\\n'))") % listed
    done = subprocess.run(["lua", "-e", body], capture_output=True, text=True)
    if done.returncode != 0:
        sys.exit("lua: " + done.stderr)
    out = {}
    for line in done.stdout.splitlines():
        name, time, energy, worker = line.split("\t")
        out[name] = (float(time), float(energy), float(worker))
    return out


def main():
    argv = sys.argv[1:]
    flag = lambda name, default: (argv[argv.index(name) + 1] if name in argv else default)
    out_dir = flag("--out", ROOT + "/docs/studies/data")
    seeds, jobs = int(flag("--seeds", 24)), int(flag("--jobs", 8))
    if argv and argv[0] == "army":
        army([a for a in argv[1:] if not a.startswith("--")], sys.stdout)
    elif argv and argv[0] == "fights":
        fights(seeds, jobs, out_dir, flag("--army", out_dir + "/tier2-barb-army.csv"))
    elif argv and argv[0] == "energy":
        energy_sweep(seeds, jobs, out_dir, flag("--army", out_dir + "/tier2-barb-army.csv"))
    elif argv and argv[0] == "sustain":
        sustain(build_times(flag("--bar", ROOT + "/upstream/Beyond-All-Reason")))
    else:
        print(__doc__)


main()
