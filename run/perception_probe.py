#!/usr/bin/env python3
"""What does a model need to be shown to see what is wrong with a position? (docs/studies/perception.md)

usage: run/perception_probe.py MATCH OUT_DIR [--reps N] [--parallel N] [--model ID] [--conditions a,b] [--config-dir DIR]

For each full report the match's commander session received (the transcript's turns that open with the map), the model
is asked one neutral question about the position under several representations of it:
  text         the map and report exactly as the commander got them
  text+image   the same, plus the whole map as a picture (run/render_scene.py)
  text+trails  the same, the picture also showing where our soldiers walked in the last 90 s
  text+zoom    text, the whole-map picture with trails, and a close-up of our part of the map with trails
  image        the two pictures and a legend, no report
  text+svg     text, plus the scene as SVG source to be read as text: land and cliff outlines, every unit, soldiers' paths
Each answer is then graded by a second call against RUBRIC, blind to the condition. Answers and grades go to
OUT_DIR/answers.jsonl; the table is printed. Sessions run with no tools, on the subscription in --config-dir.
"""
import argparse, base64, concurrent.futures, glob, json, os, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
QUESTION = (
    "You are advising the player of our side in this game of Beyond All Reason, mid-game. What are the biggest problems "
    "with how we are playing this position, and what would you change? Most important first, at most five points, and "
    "be specific about places on the map."
)
LEGEND = (
    "The pictures: north is up; relief shaded by height, water blue, near-black is ground walking units cannot cross "
    "(cliffs). Ours is green: diamond = metal extractor, big square = factory, triangle = turret, small square = other "
    "building, ring = commander, cyan dot = constructor, green dot = soldier. Red = enemy units we can see. White rings = "
    "free metal spots. Yellow lines, where drawn = where our soldiers walked in the last 90 seconds. The grid is A-H west "
    "to east, 1-8 north to south, with map coordinates (x, z) of each cell's corner."
)
RUBRIC = {
    "peninsula": "says our base is on a peninsula / dead end / pocket with a single land exit (a neck or choke) to the rest of the map",
    "army_misplaced": "says our army or squads are posted in the wrong place: sitting inside the base or on the home extractors instead of at or beyond the exit, or guarding what is not threatened",
    "traffic": "says our units shuttle back and forth, mill about, or jam (through the choke or inside the base)",
    "no_ground": "says we hold too few extractors / too little of the map and must take and hold ground beyond the exit",
    "base_clutter": "says our buildings crowd the base or block our own units' movement",
}
CONDITIONS = ["text", "text+image", "text+trails", "text+zoom", "image", "text+svg"]


def ask(model, config_dir, system, content, cwd):
    """One turn of `claude -p` with no tools; `content` is a list of API content blocks. Returns the answer's text."""
    line = json.dumps({"type": "user", "message": {"role": "user", "content": content}}) + "\n"
    env = dict(os.environ, CLAUDE_CONFIG_DIR=config_dir)
    out = subprocess.run(
        ["claude", "-p", "--model", model, "--tools", "", "--strict-mcp-config", "--setting-sources", "", "--system-prompt", system,
         "--input-format", "stream-json", "--output-format", "stream-json", "--verbose"],
        input=line, capture_output=True, text=True, env=env, cwd=cwd, timeout=600,
    ).stdout
    text, tokens = "", 0
    for raw in out.splitlines():
        try:
            message = json.loads(raw)
        except ValueError:
            continue
        if '"isUsingOverage":true' in raw.replace(" ", ""):
            sys.exit("the session reports extra usage (paid credits): stopping")
        if message.get("type") == "result":
            text = message.get("result") or text
            usage = message.get("usage") or {}
            tokens = sum(usage.get(k, 0) for k in ("input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"))
    return text, tokens


def image_block(path):
    return {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": base64.b64encode(open(path, "rb").read()).decode()}}


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("match")
    parser.add_argument("out")
    parser.add_argument("--reps", type=int, default=3)
    parser.add_argument("--parallel", type=int, default=4)
    parser.add_argument("--model", default="claude-sonnet-5")
    parser.add_argument("--conditions", default=",".join(CONDITIONS))
    parser.add_argument("--zoom", type=float, nargs=4, default=[900, 300, 3500, 2900], help="map rectangle of the close-up")
    parser.add_argument("--config-dir", default=os.path.expanduser("~/.claude2"))
    args = parser.parse_args()
    os.makedirs(args.out, exist_ok=True)

    transcript = glob.glob(os.path.join(args.match, "strategist-*.jsonl"))[0]
    moments = []
    for raw in open(transcript):
        if raw.startswith('{"frame"'):
            turn = json.loads(raw)
            # A session that takes over mid-game is given its predecessor's notes first; the probe leaves them out, so
            # that the answer rests on the position and not on what the commander already believed.
            if turn.get("kind") == "turn" and "Map: {" in turn["prompt"] and turn["frame"] > 5 * 60 * 30:
                turn["prompt"] = turn["prompt"][turn["prompt"].index("Map: {") :]
                moments.append(turn)
    jobs = []
    for turn in moments:
        seconds = turn["frame"] // 30
        stamp = f"{seconds // 60}:{seconds % 60:02d}"
        pictures = {}
        for name, extra in (("plain", []), ("trails", ["--trails", "90"]), ("zoom", ["--trails", "90", "--crop", *map(str, args.zoom)])):
            pictures[name] = os.path.join(args.out, f"{stamp.replace(':', '-')}-{name}.png")
            subprocess.run([sys.executable, os.path.join(HERE, "render_scene.py"), args.match, stamp, "--out", pictures[name], *extra], check=True, capture_output=True)
        svg = os.path.join(args.out, f"{stamp.replace(':', '-')}.svg")
        subprocess.run([sys.executable, os.path.join(HERE, "render_scene.py"), args.match, stamp, "--svg", svg, "--trails", "90"], check=True, capture_output=True)
        for condition in args.conditions.split(","):
            content = []
            if condition in ("text+image",):
                content.append(image_block(pictures["plain"]))
            if condition in ("text+trails", "text+zoom", "image"):
                content.append(image_block(pictures["trails"]))
            if condition in ("text+zoom", "image"):
                content.append(image_block(pictures["zoom"]))
            words = []
            if condition == "text+svg":
                words.append("The position as SVG source (read it as geometry; it is not drawn for you):\n" + open(svg).read())
            elif condition != "text":
                words.append(LEGEND + (" The second picture is a close-up of our part of the map." if condition in ("text+zoom", "image") else ""))
            if condition != "image":
                words.append(turn["prompt"])
            else:
                words.append(f"Game time {stamp}.")
            content.append({"type": "text", "text": "\n\n".join(words + [QUESTION])})
            for rep in range(args.reps):
                jobs.append((stamp, condition, rep, content))

    system = "You are an experienced real-time-strategy player reviewing a game in progress. Answer plainly."
    grader = (
        "You grade an advisor's answer about a strategy-game position. For each numbered statement, say whether the answer "
        "makes that point (clearly, not by a stretch). Reply with one JSON object only, keys as given, values true or false.\n"
        + "\n".join(f"{key}: the answer {text}" for key, text in RUBRIC.items())
    )

    def run(job):
        stamp, condition, rep, content = job
        answer, tokens = ask(args.model, args.config_dir, system, content, args.out)
        verdict, _ = ask(args.model, args.config_dir, grader, [{"type": "text", "text": "The answer to grade:\n\n" + answer}], args.out)
        try:
            grades = json.loads(verdict[verdict.index("{") : verdict.rindex("}") + 1])
        except ValueError:
            grades = {}
        return {"moment": stamp, "condition": condition, "rep": rep, "input_tokens": tokens, "answer": answer, "grades": grades}

    with concurrent.futures.ThreadPoolExecutor(args.parallel) as pool, open(os.path.join(args.out, "answers.jsonl"), "a") as log:
        results = []
        for result in pool.map(run, jobs):
            log.write(json.dumps(result) + "\n")
            log.flush()
            results.append(result)

    print(f"{'moment':>6} {'condition':<12} {'n':>2} {'tokens':>7} | " + " ".join(f"{k:>14}" for k in RUBRIC))
    for stamp in sorted({r["moment"] for r in results}):
        for condition in args.conditions.split(","):
            rows = [r for r in results if r["moment"] == stamp and r["condition"] == condition]
            if rows:
                hits = " ".join(f"{sum(bool(r['grades'].get(k)) for r in rows):>12}/{len(rows)}" for k in RUBRIC)
                print(f"{stamp:>6} {condition:<12} {len(rows):>2} {sum(r['input_tokens'] for r in rows) // len(rows):>7} | {hits}")


if __name__ == "__main__":
    main()
