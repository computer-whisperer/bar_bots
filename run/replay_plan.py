#!/usr/bin/env python3
"""A player's opening as a plan the bot can play (`buildorder::plan::Plan` text, `WITHIN_REASON_OPENING_PLAN`), read
off a replayed game's record (`run/replay_match.py`): what each builder built, in order, with extractor sites, from the
`created` events, which name the builder.

usage: run/replay_plan.py run/matches/<replay>/record-<team>.jsonl [--until SECONDS] [--assist-gap SECONDS]

The commander's queue gets an `assist` step wherever it built nothing for --assist-gap seconds (default 25) while a
factory of ours stood: a player's commander that is not building is assisting the lab. Factories are `fac0`, `fac1`
in the order they were finished; constructors `con0`, `con1`, ... likewise. Check the result by eye: it is a
transcription, not a search.
"""
import json
import sys


def main():
    args = sys.argv[1:]
    until = 300.0
    gap = 25.0
    paths = []
    while args:
        a = args.pop(0)
        if a == "--until":
            until = float(args.pop(0))
        elif a == "--assist-gap":
            gap = float(args.pop(0))
        else:
            paths.append(a)
    if not paths:
        sys.exit(__doc__)
    defs = None
    spots = []
    kind = {}          # unit id -> class
    name = {}          # unit id -> name
    queues = {}        # builder id -> [(seconds, word)]
    finished = {}      # unit id -> seconds finished
    commander = None
    for line in open(paths[0]):
        try:
            r = json.loads(line)
        except ValueError:
            continue
        t = r.get("t")
        if t == "header":
            defs = r["unit_defs"]
            spots = [(s[0], s[1]) for s in r["metal_spots"]]
            continue
        if defs is not None and t == "s" and commander is None:
            for u in r["own"]:
                if defs[u[1]]["class"] == "commander":
                    commander = u[0]
                    kind[u[0]] = "commander"
        if defs is None or t != "ev" or r.get("d", -1) < 0:
            continue
        seconds = r["f"] / 30.0
        if seconds > until:
            break
        d = defs[r["d"]]
        if r["k"] == "created":
            kind[r["u"]] = d["class"]
            name[r["u"]] = d["name"]
            builder = r.get("by")
            if builder is None:
                if d["class"] == "commander":
                    commander = r["u"]
                    kind[r["u"]] = "commander"
                continue
            word = d["name"][3:]
            if d["class"] == "extractor" and spots:
                x, z = min(spots, key=lambda s: (s[0] - r["x"]) ** 2 + (s[1] - r["z"]) ** 2)
                word = f"{word}@{x:.0},{z:.0}"
            queues.setdefault(builder, []).append((seconds, word))
        elif r["k"] == "finished":
            finished[r["u"]] = seconds
    if commander is None:
        sys.exit("no commander in the record")
    factories = sorted((u for u, k in kind.items() if k == "factory" and u in finished), key=lambda u: finished[u])
    constructors = sorted((u for u, k in kind.items() if k == "builder" and u in finished), key=lambda u: finished[u])
    first_factory = finished[factories[0]] if factories else None

    def words(builder, assist):
        steps = queues.get(builder, [])
        out = []
        last = None
        for seconds, word in steps:
            if assist and first_factory is not None and last is not None and seconds - last > gap and last >= first_factory:
                out.extend(["assist"] * int((seconds - last) // gap))
            out.append(word)
            last = seconds if not word.endswith("lab") else seconds
        if assist and first_factory is not None and last is not None and until - last > gap:
            out.extend(["assist"] * int((until - last) // gap))
        return out

    print(f"# from {paths[0]}, the first {until:.0f} s")
    print("com: " + " ".join(words(commander, True)))
    for i, f in enumerate(factories):
        print(f"fac{i}: " + " ".join(words(f, False)))
    for i, c in enumerate(constructors):
        print(f"con{i}: " + " ".join(words(c, False)))
    # For the eye: when each builder's steps happened.
    print("# timings:")
    for label, builder in [("com", commander)] + [(f"fac{i}", f) for i, f in enumerate(factories)] + [(f"con{i}", c) for i, c in enumerate(constructors)]:
        print(f"#   {label}: " + " ".join(f"{w}@{s:.0f}s" for s, w in queues.get(builder, [])))


if __name__ == "__main__":
    main()
