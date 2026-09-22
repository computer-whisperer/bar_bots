#!/usr/bin/env python3
"""The micro ledger: how the fighting went at the unit level, from a batch's match records
(`docs/design/2026-09-20-micro-lane.md`, "Judged by").

usage: run/micro_ledger.py run/matches/<batch> [more batches...] [--until MINUTES] [--arms H-ID]

Per batch (mean per game, by --until, default 10 game minutes):
  exchange      army metal of ours lost per enemy metal killed (all our soldiers; then the raiders alone)
  under fire    soldier-seconds spent inside the reach of an armed enemy in sight (from the once-a-second samples)
  deaths        soldiers of ours lost inside a turret's or the commander's reach, and elsewhere
  wounded       damage taken by soldiers that lived to the end of the window, against by those that died
  late ticks    the worst `late` seen, and the share of samples with one
  commander     elmos the commander walked in the window, trips over 400 elmos, the longest (routing design)
  milling       the control lane's claims that ended, the path they walked over their net displacement, reversals
                (over ninety degrees) per claim: the milling instrument (H-HANDS-LANE)
With --arms H-ID the batch is split into the arm with the heuristic on and the arm with it off (`--ab-disable`).

Reaches come from the simulator's unit table (crates/combatsim/data/units.json) by unit name.
"""
import collections
import glob
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
TABLE = os.path.join(HERE, "..", "crates", "combatsim", "data", "units.json")


def reaches():
    table = json.load(open(TABLE))["units"]
    out = {}
    for name, unit in table.items():
        ground = [w["range"] for w in unit.get("weapons", []) if w.get("only_targets") != "VTOL" and not w.get("paralyzer") and not w.get("stockpile") and not w.get("command_fire")]
        out[name] = (max(ground) if ground else 0.0, unit["speed"] == 0.0, name.endswith("com"))
    return out


def healths():
    return {name: unit["health"] for name, unit in json.load(open(TABLE))["units"].items()}


REACH = reaches()
HEALTH = healths()


def read(path, until_frame):
    """One record: the per-game figures."""
    defs = None
    own_soldier = {}  # id -> def index, for our finished soldiers
    lost = 0.0
    raider_lost = 0.0
    killed = 0.0
    under_fire = 0
    deaths_in_reach = 0
    deaths_elsewhere = 0
    damage = collections.Counter()  # own unit id -> damage taken
    died = set()
    worst_late = 0
    late_samples = 0
    samples = 0
    milling = [0, 0.0, 0.0, 0]  # claims ended, path, net, reversals
    header = None
    # Which heuristics the game ran without, from the banner the bot says at its first orders.
    disabled = None
    com_last = None
    com_walked = 0.0
    com_trip = 0.0
    com_trips = []
    # A commander's kills wait for the next sample's damage: a D-gun's killing blow is tens of thousands.
    commander_kills = {}   # own unit id -> frame killed by a commander
    dgun_frames = []       # frames of deaths to the D-gun
    for line in open(path):
        try:
            r = json.loads(line)
        except ValueError:
            continue
        t = r.get("t")
        if t == "header":
            header = r
            defs = r["unit_defs"]
            continue
        if defs is None or r.get("f", 0) > until_frame:
            continue
        if t == "cmd" and disabled is None:
            for c in r["c"]:
                if c[0] == "say" and " off: " in c[1]:
                    disabled = c[1].split(" off: ", 1)[1].split(" | ")[0]
            if disabled is None:
                disabled = ""
        if t == "ev":
            d = defs[r["d"]] if r.get("d", -1) >= 0 else None
            if r["k"] == "finished" and d and d["class"] == "army":
                own_soldier[r["u"]] = r["d"]
            elif r["k"] == "destroyed" and r["u"] in own_soldier:
                d = defs[own_soldier[r["u"]]]
                lost += d["metal"]
                if d["name"].endswith("pw") or d["name"].endswith("ak"):
                    raider_lost += d["metal"]
                died.add(r["u"])
                killer = defs[r["by_d"]]["name"] if r.get("by_d") is not None and r["by_d"] >= 0 else ""
                reach, static, commander = REACH.get(killer, (0.0, False, False))
                if static or commander:
                    deaths_in_reach += 1
                else:
                    deaths_elsewhere += 1
                if commander:
                    commander_kills[r["u"]] = r["f"]
            elif r["k"] == "enemy_destroyed" and d:
                killed += d["metal"]
        elif t == "s":
            samples += 1
            for u in r["own"]:
                if defs[u[1]]["class"] == "commander":
                    if com_last is not None:
                        step = ((u[2] - com_last[0]) ** 2 + (u[3] - com_last[1]) ** 2) ** 0.5
                        com_walked += step
                        if step > 20:
                            com_trip += step
                        elif com_trip > 0:
                            com_trips.append(com_trip)
                            com_trip = 0.0
                    com_last = (u[2], u[3])
                    break
            late = r.get("late", 0)
            worst_late = max(worst_late, late)
            for k, v in enumerate(r.get("lane", [])):
                milling[k] += v
            late_samples += late > 0
            for unit, dmg in r.get("dmg", []):
                if unit in own_soldier:
                    damage[unit] += dmg
                if unit in commander_kills and dmg > 3 * HEALTH.get(defs[own_soldier[unit]]["name"], 1e9):
                    dgun_frames.append(commander_kills[unit])
            commander_kills = {u: f for u, f in commander_kills.items() if r["f"] - f < 60}
            threats = []
            for e in r["en"]:
                if e[1] < 0:
                    continue
                name = defs[e[1]]["name"]
                reach, _, _ = REACH.get(name, (0.0, False, False))
                if reach > 0:
                    threats.append((e[2], e[3], reach))
            if threats:
                for u in r["own"]:
                    if u[0] in own_soldier and any((u[2] - x) ** 2 + (u[3] - z) ** 2 < reach * reach for x, z, reach in threats):
                        under_fire += 1
    # The engine reports a killing blow's full damage (a D-gun's tens of thousands), so a unit's damage is capped
    # at its health.
    cap = lambda k, v: min(v, HEALTH.get(defs[own_soldier[k]]["name"], v))
    wounded_lived = sum(cap(k, v) for k, v in damage.items() if k not in died)
    wounded_died = sum(cap(k, v) for k, v in damage.items() if k in died)
    # Shots: D-gun deaths within a second of the last count as one.
    dgun_shots = sum(1 for i, f in enumerate(sorted(dgun_frames)) if i == 0 or f - sorted(dgun_frames)[i - 1] > 30)
    return {
        "dgun_deaths": len(dgun_frames), "dgun_shots": dgun_shots,
        "lost": lost, "raider_lost": raider_lost, "killed": killed, "under_fire": under_fire,
        "deaths_in_reach": deaths_in_reach, "deaths_elsewhere": deaths_elsewhere,
        "wounded_lived": wounded_lived, "wounded_died": wounded_died,
        "worst_late": worst_late, "late_share": late_samples / max(samples, 1),
        "disabled": disabled or "",
        "com_walked": com_walked, "com_trips": sum(1 for x in com_trips if x > 400), "com_longest": max(com_trips, default=0.0),
        "claims": milling[0], "claim_path": milling[1], "claim_net": milling[2], "reversals": milling[3],
    }


def summarise(label, games):
    n = len(games)
    if n == 0:
        print(f"{label}: no records")
        return
    mean = lambda key: sum(g[key] for g in games) / n
    exchange = mean("lost") / max(mean("killed"), 1.0)
    print(f"{label}: {n} games")
    print(f"  exchange     {mean('lost'):6.0f} lost / {mean('killed'):6.0f} killed = {exchange:.2f}   (raiders lost {mean('raider_lost'):.0f})")
    print(f"  under fire   {mean('under_fire'):6.0f} soldier-seconds")
    print(f"  deaths       {mean('deaths_in_reach'):5.1f} to turrets or the commander, {mean('deaths_elsewhere'):5.1f} elsewhere")
    print(f"  D-gun        {sum(g['dgun_deaths'] for g in games):3d} deaths in {sum(g['dgun_shots'] for g in games)} shots over the {n} games")
    print(f"  wounded      {mean('wounded_lived'):6.0f} damage on soldiers that lived, {mean('wounded_died'):6.0f} on those that died")
    print(f"  late ticks   worst {max(g['worst_late'] for g in games)} frames, {100 * mean('late_share'):.1f} % of samples")
    print(f"  commander    {mean('com_walked'):6.0f} elmos walked, {mean('com_trips'):4.1f} trips over 400, longest {mean('com_longest'):5.0f}")
    claims = mean("claims")
    if claims > 0:
        print(f"  milling      {claims:6.0f} claims, path/net {mean('claim_path') / max(mean('claim_net'), 1.0):.2f}, {mean('reversals') / claims:.2f} reversals per claim")


def main():
    args = sys.argv[1:]
    until = 10.0
    arms = None
    batches = []
    while args:
        a = args.pop(0)
        if a == "--until":
            until = float(args.pop(0))
        elif a == "--arms":
            arms = args.pop(0)
        else:
            batches.append(a)
    if not batches:
        print(__doc__)
        sys.exit(2)
    until_frame = int(until * 60 * 30)
    for batch in batches:
        by_arm = collections.defaultdict(list)
        for match in sorted(glob.glob(os.path.join(batch, "[0-9]*"))):
            for rec in glob.glob(os.path.join(match, "record-*.jsonl")):
                game = read(rec, until_frame)
                arm = "off " + arms if arms and arms in game["disabled"] else ("on " + arms if arms else "")
                by_arm[arm].append(game)
        name = os.path.basename(batch.rstrip("/"))
        for arm in sorted(by_arm):
            summarise(f"{name} {arm}".strip(), by_arm[arm])


if __name__ == "__main__":
    main()
