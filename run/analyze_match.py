#!/usr/bin/env python3
"""Post-game analysis of one recorded match, for a reader (human or model) who has to say why it was lost.

Inputs, all in the match directory: record-<ai>.jsonl (our side, every second: docs/harness/record-format.md),
truth-<ai>.jsonl (the opponent's every unit, every two seconds; written when the match ran with
WITHIN_REASON_OBSERVE=1), terrain-<ai>.bin. Without a truth file the opponent is only what our units saw.

usage:
  run/analyze_match.py MATCH                     the report: curves, turning points, engagements, candidate causes
  run/analyze_match.py MATCH --scene MM:SS X Z   a scene report: who stood where around (X, Z) at that time
  run/analyze_match.py MATCH --engagement N      scene reports before, during and after engagement N of the report
  run/analyze_match.py MATCH --json              the report's numbers as JSON
"""
import glob, json, math, os, struct, sys
from collections import Counter, defaultdict

FPS = 30
ENGAGEMENT_GAP_S = 20      # deaths further apart in time than this belong to different engagements
ENGAGEMENT_REACH = 900     # ... or further apart in space than this
ENGAGEMENT_MIN_VALUE = 250  # metal lost by both sides together for an engagement to be worth listing
PRESENT_RADIUS = 1100      # units this close to an engagement's centre when it starts took part


def clock(frame):
    seconds = frame // FPS
    return f"{seconds // 60}:{seconds % 60:02d}"


class Match:
    def __init__(self, directory):
        self.dir = directory
        record = sorted(glob.glob(os.path.join(directory, "record-*.jsonl")))
        if not record:
            sys.exit(f"{directory}: no record-*.jsonl (matches before the recorder cannot be analysed)")
        self.header, self.samples, self.events, self.intents, self.decisions, self.result = None, [], [], [], [], None
        for line in open(record[0], errors="replace"):
            try:
                r = json.loads(line)
            except ValueError:
                continue  # a killed match may end mid-line
            kind = r.get("t")
            if kind == "header" and self.header is None:
                self.header = r
            elif kind == "s":
                self.samples.append(r)
            elif kind == "ev":
                self.events.append(r)
            elif kind == "intent":
                self.intents.append(r)
            elif kind == "d":
                self.decisions.append(r)
            elif kind == "result":
                self.result = r
        self.defs = self.header["unit_defs"]
        self.by_name = {d["name"]: d for d in self.defs}
        self.width, self.height = self.header["map"]["width"], self.header["map"]["height"]
        self.truth = []
        for path in sorted(glob.glob(os.path.join(directory, "truth-*.jsonl")))[:1]:
            for line in open(path, errors="replace"):
                try:
                    self.truth.append(json.loads(line))
                except ValueError:
                    pass
        self.terrain = self.load_terrain()
        self.home = next((i["home"] for i in self.intents if i.get("home")), None)
        self.enemy_home = self.find_enemy_home()

    def load_terrain(self):
        t = self.header.get("terrain")
        if not t:
            return None
        try:
            raw = open(os.path.join(self.dir, t["file"]), "rb").read()
        except OSError:
            return None
        cells = t["width"] * t["height"]
        if len(raw) < cells * 3:
            return None
        classes = [c for c in t.get("move_classes", []) if c["kind"] == "bot" and c["depth"] < 1000 and c["max_slope"] < 0.99]
        usual = max(classes, key=lambda c: c.get("units", 0)) if classes else {"max_slope": 0.41, "depth": 20}
        return {"w": t["width"], "h": t["height"], "cell": t["cell"], "heights": struct.unpack(f"<{cells}h", raw[:cells * 2]),
                "slopes": raw[cells * 2:cells * 3], "max_slope": usual["max_slope"] * 255, "depth": usual["depth"]}

    def find_enemy_home(self):
        for t in self.truth[:30]:
            for u in t["enemy"]:
                if self.cls(u[1]) == "commander":
                    return [u[2], u[3]]
        return next((i["enemy_start"] for i in self.intents if i.get("enemy_start")), None)

    # unit facts
    def cls(self, name):
        return self.by_name.get(name, {}).get("class", "other")

    def metal(self, name):
        return self.by_name.get(name, {}).get("metal", 0)

    def grid(self, x, z):
        g = self.header["grid"]
        column = min(g["columns"] - 1, max(0, int(x / self.width * g["columns"])))
        row = min(g["rows"] - 1, max(0, int(z / self.height * g["rows"])))
        return f"{chr(ord('A') + column)}{row + 1}"

    def side_of_map(self, x, z):
        if not self.home or not self.enemy_home:
            return "?"
        ours, theirs = math.dist((x, z), self.home), math.dist((x, z), self.enemy_home)
        if ours < 1400:
            return "at our base"
        if theirs < 1400:
            return "at their base"
        return "in our half" if ours < theirs else "in their half"

    # state at a time
    def ours_at(self, frame):
        """[(id, name, x, z, health %, flags)] from the sample nearest `frame`."""
        if not self.samples:
            return []
        s = min(self.samples, key=lambda s: abs(s["f"] - frame))
        return [(u[0], self.defs[u[1]]["name"], u[2], u[3], u[4], u[5]) for u in s["own"]]

    def theirs_at(self, frame):
        """[(id, name, x, z, health %, being built)]; from truth when there is one, else what we saw."""
        if self.truth:
            t = min(self.truth, key=lambda t: abs(t["f"] - frame))
            return [tuple(u) for u in t["enemy"]]
        s = min(self.samples, key=lambda s: abs(s["f"] - frame)) if self.samples else {"en": []}
        return [(u[0], self.defs[u[1]]["name"] if u[1] >= 0 else "?", u[2], u[3], 100, 0) for u in s.get("en", [])]

    def deaths(self):
        """Every unit death on both sides: (frame, side, name, x, z, metal, killer name or None)."""
        out = []
        for e in self.events:
            if e["k"] == "destroyed" and e.get("d") is not None and e["d"] >= 0:
                name = self.defs[e["d"]]["name"]
                killer = self.defs[e["by_d"]]["name"] if e.get("by_d") is not None and e["by_d"] >= 0 else None
                out.append((e["f"], "ours", name, e["x"], e["z"], self.metal(name), killer))
        if self.truth:
            previous = {}
            for t in self.truth:
                current = {u[0]: u for u in t["enemy"]}
                for uid, u in previous.items():
                    # Gone from a complete list: dead (a unit still under construction that vanishes was cancelled or killed).
                    if uid not in current and not u[5]:
                        out.append((t["f"], "theirs", u[1], u[2], u[3], self.metal(u[1]), None))
                previous = current
        else:
            for e in self.events:
                if e["k"] == "enemy_destroyed" and e.get("d") is not None and e["d"] >= 0:
                    name = self.defs[e["d"]]["name"]
                    out.append((e["f"], "theirs", name, e["x"], e["z"], self.metal(name), None))
        return sorted(out)


def summarise(units, match, built_only=True):
    """Counts and metal by class for a unit list of either side."""
    out = defaultdict(lambda: [0, 0.0])
    for u in units:
        name, building = u[1], (u[5] & 1) if isinstance(u[5], int) else u[5]
        if built_only and building:
            continue
        c = match.cls(name)
        out[c][0] += 1
        out[c][1] += match.metal(name)
    return out


def curves(match):
    rows = []
    last = match.samples[-1]["f"] if match.samples else 0
    for minute in range(1, last // (60 * FPS) + 1):
        f = minute * 60 * FPS
        ours, theirs = summarise(match.ours_at(f), match), summarise(match.theirs_at(f), match)
        sample = min(match.samples, key=lambda s: abs(s["f"] - f))
        rows.append({
            "minute": minute, "metal_banked": round(sample["m"][0]), "metal_income": round(sample["m"][1], 1),
            "energy_banked": round(sample["e"][0]), "energy_storage": round(sample["e"][3]),
            "ours": {c: [n, round(v)] for c, (n, v) in ours.items()}, "theirs": {c: [n, round(v)] for c, (n, v) in theirs.items()},
        })
    return rows


def engagements(match):
    clusters = []
    for d in match.deaths():
        frame, _, _, x, z, _, _ = d
        if x == 0 and z == 0:
            continue
        home = None
        for c in reversed(clusters):
            if frame - c["end"] > ENGAGEMENT_GAP_S * FPS:
                break
            if math.dist((x, z), c["centre"]) < ENGAGEMENT_REACH:
                home = c
                break
        if home is None:
            home = {"start": frame, "end": frame, "centre": (x, z), "deaths": []}
            clusters.append(home)
        home["deaths"].append(d)
        home["end"] = frame
        n = len(home["deaths"])
        home["centre"] = ((home["centre"][0] * (n - 1) + x) / n, (home["centre"][1] * (n - 1) + z) / n)
    out = []
    for c in clusters:
        lost = {"ours": Counter(), "theirs": Counter()}
        value = {"ours": 0.0, "theirs": 0.0}
        killers = Counter()
        for _, side, name, _, _, metal, killer in c["deaths"]:
            lost[side][name] += 1
            value[side] += metal
            if side == "ours" and killer:
                killers[killer] += 1
        if value["ours"] + value["theirs"] < ENGAGEMENT_MIN_VALUE:
            continue
        before = c["start"] - 5 * FPS
        cx, cz = c["centre"]
        near = lambda units: [u for u in units if math.dist((u[2], u[3]), (cx, cz)) < PRESENT_RADIUS]
        ours, theirs = near(match.ours_at(before)), near(match.theirs_at(before))
        fighters = lambda units: [u for u in units if match.cls(u[1]) in ("army", "commander")]
        our_fighters, their_fighters = fighters(ours), fighters(theirs)
        spread = 0.0
        if len(our_fighters) > 1:
            mx, mz = sum(u[2] for u in our_fighters) / len(our_fighters), sum(u[3] for u in our_fighters) / len(our_fighters)
            spread = math.sqrt(sum((u[2] - mx) ** 2 + (u[3] - mz) ** 2 for u in our_fighters) / len(our_fighters))
        out.append({
            "start": c["start"], "end": c["end"], "centre": [round(cx), round(cz)], "grid": match.grid(cx, cz),
            "where": match.side_of_map(cx, cz),
            "lost": {s: dict(lost[s]) for s in lost}, "value_lost": {s: round(value[s]) for s in value},
            "killers_of_ours": dict(killers.most_common(5)),
            "present_before": {
                "our_fighters": dict(Counter(u[1] for u in our_fighters)), "our_fighter_value": round(sum(match.metal(u[1]) for u in our_fighters)),
                "our_turrets": sum(1 for u in ours if match.cls(u[1]) == "turret"),
                "their_fighters": dict(Counter(u[1] for u in their_fighters)), "their_fighter_value": round(sum(match.metal(u[1]) for u in their_fighters)),
                "their_turrets": sum(1 for u in theirs if match.cls(u[1]) == "turret"),
                "our_fighters_spread": round(spread),
            },
        })
    return out


def causes(match, curve, fights):
    """Candidate explanations, each with its numbers; the reader judges which one decided the game."""
    out = []
    army = lambda row, side: row[side].get("army", [0, 0])[1]
    mex = lambda row, side: row[side].get("extractor", [0, 0])[0]
    if curve:
        lead = [(army(r, "theirs") - army(r, "ours"), r["minute"]) for r in curve]
        first_double = next((r["minute"] for r in curve if army(r, "theirs") > 1.5 * max(army(r, "ours"), 300)), None)
        out.append(f"army value: their lead peaks at {max(lead)[0]:.0f} metal in minute {max(lead)[1]}; "
                   f"they first have 1.5x our army in minute {first_double}")
        peak = max(curve, key=lambda r: mex(r, "ours"))
        after = [r for r in curve if r["minute"] > peak["minute"] and mex(r, "ours") <= mex(peak, "ours") / 2]
        out.append(f"extractors: ours peak at {mex(peak, 'ours')} in minute {peak['minute']}"
                   + (f", halved by minute {after[0]['minute']}" if after else ", never halved")
                   + f"; theirs end at {mex(curve[-1], 'theirs')}")
        floating = [r["minute"] for r in curve if r["metal_banked"] > 500 and r["minute"] > 2]
        stalled = [r["minute"] for r in curve if r["energy_storage"] and r["energy_banked"] < 0.1 * r["energy_storage"]]
        out.append(f"economy use: metal banked above 500 in minutes {floating or 'none'}; energy stalled in minutes {stalled or 'none'}")
    deaths = match.deaths()
    lost_by_place, killed_by_place = Counter(), Counter()
    for _, side, name, x, z, metal, _ in deaths:
        (lost_by_place if side == "ours" else killed_by_place)[match.side_of_map(x, z)] += metal
    out.append(f"metal lost by place: ours {dict((k, round(v)) for k, v in lost_by_place.items())}; "
               f"theirs {dict((k, round(v)) for k, v in killed_by_place.items())}")
    mex_deaths = [(d[0], d[6]) for d in deaths if d[1] == "ours" and match.cls(d[2]) == "extractor"]
    out.append(f"raids: {len(mex_deaths)} of our extractors destroyed, first at {clock(mex_deaths[0][0]) if mex_deaths else '-'}; "
               f"killers {dict(Counter(k for _, k in mex_deaths if k).most_common(4))}")
    if fights:
        worst = max(fights, key=lambda e: e["value_lost"]["ours"] - e["value_lost"]["theirs"])
        i = fights.index(worst)
        out.append(f"worst engagement: #{i} at {clock(worst['start'])} {worst['grid']} ({worst['where']}): we lost "
                   f"{worst['value_lost']['ours']} metal, they lost {worst['value_lost']['theirs']}; before it we had "
                   f"{worst['present_before']['our_fighter_value']} of fighters there against {worst['present_before']['their_fighter_value']}, "
                   f"our fighters spread over {worst['present_before']['our_fighters_spread']} elmos, our turrets there {worst['present_before']['our_turrets']}")
        uphill = [e for e in fights if e["present_before"]["their_fighter_value"] > 1.5 * max(e["present_before"]["our_fighter_value"], 1)]
        out.append(f"{len(uphill)} of {len(fights)} engagements began with them at over 1.5x our fighter value on the spot; "
                   f"{sum(1 for e in fights if e['where'] in ('in their half', 'at their base'))} were on their side of the map")
    commander = [d for d in deaths if d[1] == "ours" and match.cls(d[2]) == "commander"]
    if commander:
        f, _, _, x, z, _, killer = commander[-1]
        out.append(f"our commander died at {clock(f)} at {match.grid(x, z)} ({match.side_of_map(x, z)}) to {killer}")
    return out


def scene(match, frame, x, z, radius=900, size=30):
    """Who stood where: a character map of the square around (x, z), then both sides' units in it."""
    left, top, step = x - radius, z - radius, 2 * radius / size
    rows = [["." for _ in range(size)] for _ in range(size)]
    t = match.terrain
    if t:
        for r in range(size):
            for c in range(size):
                wx, wz = left + (c + 0.5) * step, top + (r + 0.5) * step
                if not (0 <= wx < match.width and 0 <= wz < match.height):
                    rows[r][c] = " "
                    continue
                i = int(wz / t["cell"]) * t["w"] + int(wx / t["cell"])
                rows[r][c] = "~" if t["heights"][i] < -t["depth"] else "#" if t["slopes"][i] > t["max_slope"] else "."
    legend, glyphs = {}, iter("abcdefghijklmnopqrstuvwxyz")

    def glyph(name, ours):
        key = (name, ours)
        if key not in legend:
            letter = next(glyphs, "?")
            legend[key] = letter if ours else letter.upper()
        return legend[key]

    inside = lambda u: abs(u[2] - x) < radius and abs(u[3] - z) < radius
    ours, theirs = [u for u in match.ours_at(frame) if inside(u)], [u for u in match.theirs_at(frame) if inside(u)]
    for units, mine in ((ours, True), (theirs, False)):
        for u in units:
            c, r = int((u[2] - left) / step), int((u[3] - top) / step)
            if 0 <= r < size and 0 <= c < size:
                rows[r][c] = glyph(u[1], mine)
    for f, side, _, dx, dz, _, _ in match.deaths():
        if 0 <= frame - f < 10 * FPS and abs(dx - x) < radius and abs(dz - z) < radius:
            c, r = int((dx - left) / step), int((dz - top) / step)
            if 0 <= r < size and 0 <= c < size:
                rows[r][c] = "*" if side == "ours" else "+"
    lines = [f"scene at {clock(frame)} around {match.grid(x, z)} ({x:.0f}, {z:.0f}), {2 * radius:.0f} elmos across, north up; "
             f"one character is {step:.0f} elmos. ~ water, # cliff, * one of ours died here in the last 10 s, + one of theirs"]
    lines += ["  " + "".join(row) for row in rows]
    for mine, units in ((True, ours), (False, theirs)):
        counts = Counter(u[1] for u in units)
        health = defaultdict(list)
        for u in units:
            health[u[1]].append(u[4])
        side = "ours (lower case)" if mine else "theirs (UPPER CASE)"
        parts = [f"{glyph(n, mine)}={n} x{k} ({match.cls(n)}, {round(sum(health[n]) / len(health[n]))}% health)" for n, k in counts.most_common()]
        value = sum(match.metal(u[1]) for u in units if match.cls(u[1]) in ("army", "commander", "turret"))
        lines.append(f"{side}: {', '.join(parts) or 'nothing'}; fighting value {value:.0f} metal")
    if ours:
        flags = Counter("squad" if u[5] & 8 else "attack wave" if u[5] & 4 else "home group" for u in ours if match.cls(u[1]) == "army")
        lines.append(f"our soldiers by role: {dict(flags)}")
    return "\n".join(lines)


def report(match):
    curve, fights = curves(match), engagements(match)
    result = (match.result or {}).get("result", {})
    out = [f"MATCH {match.dir}",
           f"{match.header['map']['name']}, we are {match.header.get('side')} ({match.header.get('mode')}), opponent {(match.result or {}).get('opponent')}; "
           f"outcome {result.get('outcome')} after {result.get('game_minutes', 0):.1f} min, start corner {result.get('our_corner')}",
           f"opponent data: {'ground truth every 2 s' if match.truth else 'ONLY what our units saw (no truth file): their numbers are floors'}",
           "", "CURVES (ours / theirs; army and turrets in metal value)",
           "min | metal bank  income | extractors | builders | army value | turrets | factories"]
    get = lambda row, side, c, i: row[side].get(c, [0, 0])[i]
    for r in curve:
        out.append(f"{r['minute']:>3} | {r['metal_banked']:>10}  {r['metal_income']:>6} | {get(r,'ours','extractor',0):>4}/{get(r,'theirs','extractor',0):<4} | "
                   f"{get(r,'ours','builder',0):>3}/{get(r,'theirs','builder',0):<3} | {get(r,'ours','army',1):>5}/{get(r,'theirs','army',1):<5} | "
                   f"{get(r,'ours','turret',0):>2}/{get(r,'theirs','turret',0):<2} | {get(r,'ours','factory',0)}/{get(r,'theirs','factory',0)}")
    out += ["", "CANDIDATE CAUSES (numbers, not a verdict)"] + [f"- {c}" for c in causes(match, curve, fights)]
    out += ["", f"ENGAGEMENTS ({len(fights)} with at least {ENGAGEMENT_MIN_VALUE} metal lost; use --engagement N for scenes)"]
    for i, e in enumerate(fights):
        p = e["present_before"]
        out.append(f"#{i} {clock(e['start'])}-{clock(e['end'])} {e['grid']} {e['where']}: we lost {e['value_lost']['ours']} ({e['lost']['ours']}), "
                   f"they lost {e['value_lost']['theirs']} ({e['lost']['theirs']}); on the spot before: ours {p['our_fighter_value']} + {p['our_turrets']} turrets "
                   f"vs theirs {p['their_fighter_value']} + {p['their_turrets']} turrets; our spread {p['our_fighters_spread']}")
    return "\n".join(out), {"curve": curve, "engagements": fights, "causes": causes(match, curve, fights)}


def main():
    args = sys.argv[1:]
    if not args or args[0].startswith("-"):
        sys.exit(__doc__)
    match = Match(args[0].rstrip("/"))
    if "--scene" in args:
        i = args.index("--scene")
        minutes, seconds = args[i + 1].split(":")
        print(scene(match, (int(minutes) * 60 + int(seconds)) * FPS, float(args[i + 2]), float(args[i + 3])))
    elif "--engagement" in args:
        e = engagements(match)[int(args[args.index("--engagement") + 1])]
        for frame in (e["start"] - 15 * FPS, e["start"], (e["start"] + e["end"]) // 2, e["end"] + 5 * FPS):
            print(scene(match, max(frame, 0), *e["centre"]), end="\n\n")
    elif "--json" in args:
        print(json.dumps(report(match)[1]))
    else:
        print(report(match)[0])


main()
