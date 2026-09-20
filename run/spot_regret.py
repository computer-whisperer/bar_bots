#!/usr/bin/env python3
"""What did not taking a metal spot cost us, and how long is raided ground really dangerous?

usage: run/spot_regret.py BATCH_OR_MATCH_DIR... [--threat-radius 600] [--overhead 60]

Reads each match's record (what we held) beside its truth file (where every enemy unit really was; matches run with
WITHIN_REASON_OBSERVE=1), and classes every metal spot, second by second, as ours, theirs, threatened (free, with an
armed enemy unit or turret inside the threat radius) or quiet (free, nothing armed near). Two questions:

  regret     Quiet time is time an extractor there would have stood unmolested. A quiet run longer than the overhead
             (walking there and building, 60 s by default) counts, less the overhead, as extractor-minutes forgone.
  hot spots  After each extractor we lost: how long until the ground was quiet again, how long that quiet then lasted
             before the next armed visit (capped by the end of the game), and how long we left the spot empty.
             H-ECO-HOT-SPOTS closes such a spot for four minutes; this is the evidence for or against that number.

Hindsight, not a policy: an extractor standing there might itself have drawn the visit that never came.
"""
import argparse, glob, json, os, statistics

SPOT_RADIUS = 120  # an extractor this close to a spot is on it


def load(match):
    record = glob.glob(os.path.join(match, "record-*.jsonl"))
    truth = glob.glob(os.path.join(match, "truth-*.jsonl"))
    if not record or not truth:
        return None
    header, own_by_second, last = None, {}, 0
    for raw in open(record[0]):
        try:
            line = json.loads(raw)
        except ValueError:
            continue
        if line.get("t") == "header" and header is None:
            header = line
        elif line.get("t") == "s":
            own_by_second[line["f"] // 30] = line["own"]
            last = max(last, line["f"] // 30)
    enemy_by_second = {}
    for raw in open(truth[0]):
        try:
            line = json.loads(raw)
        except ValueError:
            continue
        enemy_by_second[line["f"] // 30] = line["enemy"]
    return header, own_by_second, enemy_by_second, last


def near(units, spot, radius):
    r2 = radius * radius
    return any((x - spot[0]) ** 2 + (z - spot[1]) ** 2 < r2 for x, z in units)


def runs(states, wanted):
    """(start, length) of each unbroken run of `wanted` in a list of per-second states."""
    out, start = [], None
    for second, state in enumerate(states + [None]):
        if state == wanted and start is None:
            start = second
        elif state != wanted and start is not None:
            out.append((start, second - start))
            start = None
    return out


def study(match, threat_radius):
    loaded = load(match)
    if loaded is None:
        return None
    header, own_by_second, enemy_by_second, last = loaded
    defs = header["unit_defs"]
    klass = {d["name"]: d["class"] for d in defs}
    extractor_defs = {i for i, d in enumerate(defs) if d["class"] == "extractor"}
    armed = lambda name: klass.get(name) in ("army", "turret", "commander")
    spots = header["metal_spots"]
    states = [[] for _ in spots]
    own, enemy = [], []
    for second in range(last + 1):
        own = own_by_second.get(second, own)
        enemy = enemy_by_second.get(second, enemy)
        ours = [(u[2], u[3]) for u in own if u[1] in extractor_defs and not u[5] & 1]
        theirs = [(e[2], e[3]) for e in enemy if klass.get(e[1]) == "extractor"]
        threats = [(e[2], e[3]) for e in enemy if armed(e[1])]
        for index, spot in enumerate(spots):
            if near(ours, spot, SPOT_RADIUS):
                states[index].append("ours")
            elif near(theirs, spot, SPOT_RADIUS):
                states[index].append("theirs")
            elif near(threats, spot, threat_radius):
                states[index].append("threatened")
            else:
                states[index].append("quiet")
    home = next(((u[2], u[3]) for u in own_by_second.get(min(own_by_second), []) if defs[u[1]]["class"] == "commander"), (0, 0))
    return spots, states, home, last


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("dirs", nargs="+")
    parser.add_argument("--threat-radius", type=float, default=600)
    parser.add_argument("--overhead", type=int, default=60, help="seconds to walk to a spot and build on it")
    args = parser.parse_args()

    matches = []
    for d in args.dirs:
        matches += [d] if glob.glob(os.path.join(d, "record-*.jsonl")) else sorted(glob.glob(os.path.join(d, "[0-9][0-9]")))
    games, forgone, held, after_loss, by_distance = 0, [], [], [], {}
    for match in matches:
        result = study(match, args.threat_radius)
        if result is None:
            continue
        spots, states, home, last = result
        games += 1
        game_forgone = 0
        for spot, line in zip(spots, states):
            distance = ((spot[0] - home[0]) ** 2 + (spot[1] - home[1]) ** 2) ** 0.5
            usable = sum(length - args.overhead for _, length in runs(line, "quiet") if length > args.overhead)
            ours = line.count("ours")
            game_forgone += usable
            band = by_distance.setdefault(int(distance // 1000), [0, 0, 0, 0, 0])
            band[0] += usable
            band[1] += ours
            band[2] += line.count("theirs")
            band[3] += line.count("threatened")
            band[4] += 1
            # Every loss: an `ours` run that ends before the game does.
            for start, length in runs(line, "ours"):
                end = start + length
                if end >= last:
                    continue
                rest = line[end:]
                danger = next((i for i, s in enumerate(rest) if s != "threatened"), len(rest))
                after = rest[danger:]
                quiet = next((i for i, s in enumerate(after) if s == "threatened"), len(after))
                retaken = next((i for i, s in enumerate(rest) if s == "ours"), None)
                after_loss.append((danger, quiet, quiet == len(after), retaken))
        forgone.append(game_forgone / 60)
        held.append(sum(line.count("ours") for line in states) / 60)

    if not games:
        raise SystemExit("no match here has both a record and a truth file")
    print(f"{games} games, threat radius {args.threat_radius:.0f}, overhead {args.overhead} s")
    print(f"extractor-minutes held per game: median {statistics.median(held):.0f}; forgone on quiet free spots: median {statistics.median(forgone):.0f}")
    print("\nby straight distance from our start (per game; minutes summed over the band's spots):")
    print(f"{'band':>10} {'spots':>6} {'ours':>7} {'theirs':>7} {'threatened':>11} {'quiet, usable':>14}")
    for band in sorted(by_distance):
        usable, ours, theirs, threatened, count = by_distance[band]
        print(f"{band}-{band + 1}k elmos {count / games:>6.1f} {ours / 60 / games:>7.1f} {theirs / 60 / games:>7.1f} {threatened / 60 / games:>11.1f} {usable / 60 / games:>14.1f}")

    print(f"\nafter losing an extractor ({len(after_loss)} losses):")
    danger = sorted(a[0] for a in after_loss)
    quiet = sorted(a[1] for a in after_loss)
    pick = lambda values, q: values[min(len(values) - 1, int(q * len(values)))]
    print(f"  armed enemy stays within {args.threat_radius:.0f} for: median {pick(danger, .5)} s, 75th {pick(danger, .75)} s, 90th {pick(danger, .9)} s")
    print(f"  then the ground stays quiet for: median {pick(quiet, .5)} s, 25th {pick(quiet, .25)} s, 10th {pick(quiet, .1)} s"
          f" ({sum(a[2] for a in after_loss)} of them quiet to the end of the game)")
    for window in (60, 120, 240):
        back = sum(1 for a in after_loss if a[0] >= window or (a[0] + a[1] < window and not a[2]))
        print(f"  an armed enemy is back (or still there) within {window:>3} s of the loss: {back / len(after_loss):.0%}")
    retaken = sorted(a[3] for a in after_loss if a[3] is not None)
    print(f"  we rebuilt {len(retaken)} of {len(after_loss)}; time empty before the rebuild stood: median {pick(retaken, .5)} s, 25th {pick(retaken, .25)} s" if retaken else "  none rebuilt")


if __name__ == "__main__":
    main()
