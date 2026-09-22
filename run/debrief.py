#!/usr/bin/env python3
"""The player's own debrief of a match, for the next player (docs/design/2026-09-21-pianist.md, "The player's
learnings"): one `claude -p` over the match's notes, closing sentences, last packet and result, no game held, asked
for at most five lines for the next player on this map and opponent, what it wished it could order, and what decided
the game. The lines land in docs/briefs/inbox/<match>.md for the review pass that integrates them into the brief's
layers.

usage: run/debrief.py <match dir> [--model ID] [--effort LEVEL] [--claude-config-dir DIR] [--dry-run]
  <match dir> is a match (run/matches/<batch>/00) or a batch (its 00 is taken).
  --dry-run prints the prompt and writes nothing.
The session runs on ~/.claude2 (weekly allotment only; a usage snapshot is taken before and reported after).
"""
import json, os, re, subprocess, sys, time

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
INBOX = os.path.join(REPO, "docs", "briefs", "inbox")
MAX_NOTES = 40
MAX_CLOSING = 30
MAX_CHARS = 14000


def clock(frame):
    seconds = frame // 30
    return f"{seconds // 60}:{seconds % 60:02d}"


def load_match(match_dir):
    if not any(f.startswith("record-") for f in os.listdir(match_dir)) and os.path.isdir(os.path.join(match_dir, "00")):
        match_dir = os.path.join(match_dir, "00")
    batch = {}
    batch_path = os.path.join(os.path.dirname(match_dir), "batch.json")
    if os.path.isfile(batch_path):
        batch = json.load(open(batch_path))
    record = next((f for f in os.listdir(match_dir) if f.startswith("record-") and f.endswith(".jsonl")), None)
    if not record:
        sys.exit(f"no record-*.jsonl in {match_dir}")
    result = header = None
    for line in open(os.path.join(match_dir, record)):
        try:
            r = json.loads(line)
        except ValueError:
            continue
        if r.get("t") == "header" and header is None:
            header = r
        if "result" in r and isinstance(r["result"], dict) and "outcome" in r["result"]:
            result = r
    transcript = next((f for f in os.listdir(match_dir) if f.startswith("strategist-") and f.endswith(".jsonl")), None)
    if not transcript:
        sys.exit(f"no strategist-*.jsonl in {match_dir}: not a player match")
    turns = []
    turn = None
    for line in open(os.path.join(match_dir, transcript)):
        try:
            r = json.loads(line)
        except ValueError:
            continue
        kind = r.get("kind")
        if kind == "turn":
            turn = {"frame": int(r.get("frame", 0)), "notes": [], "closing": "", "instruct": None}
            turns.append(turn)
        elif kind == "tool_call" and turn is not None:
            calls = r.get("arguments", {}).get("calls") or [{"tool": r.get("tool"), "arguments": r.get("arguments", {})}]
            for c in calls:
                if c.get("tool") == "note":
                    turn["notes"].append(c.get("arguments", {}).get("text", ""))
                elif c.get("tool") == "instruct":
                    turn["instruct"] = c.get("arguments", {}).get("text", "")
        elif kind == "result" and turn is not None:
            text = r.get("message", {}).get("result")
            if isinstance(text, str) and text.strip():
                turn["closing"] = text.strip()
    return match_dir, batch, header, result, turns


def build_prompt(match_dir, batch, header, result, turns):
    label = batch.get("label") or os.path.basename(os.path.dirname(match_dir))
    outcome = result["result"] if result else {}
    lines = [
        "You are debriefing a game of Beyond All Reason that you played as the player over a pair of fast hands (Jev), "
        "a game now over. Below are your notes and closing sentences from each turn, the last packet of standing "
        "instructions you wrote, and the result. Write for the next player who will sit where you sat, on this map "
        "against this opponent, and who has never seen this game.",
        "",
        f"Game: {label}, map {batch.get('map', header['map']['name'] if header else '?')}, opponent {batch.get('opponent', '?')}, "
        f"our side {header.get('side', '?') if header else '?'}, corner {outcome.get('our_corner', '?')}.",
        f"Result: {outcome.get('outcome', 'unknown')} after {outcome.get('game_minutes', 0):.1f} minutes"
        + (" (called by the referee)" if outcome.get("called") else "") + f"; {len(turns)} turns.",
        "",
        "Your notes, by game time:",
    ]
    notes = [(t["frame"], n) for t in turns for n in t["notes"]]
    if len(notes) > MAX_NOTES:
        notes = notes[: MAX_NOTES // 2] + notes[-MAX_NOTES // 2 :]
    lines += [f"  {clock(f)} {n}" for f, n in notes]
    closings = [(t["frame"], t["closing"]) for t in turns if t["closing"]]
    if len(closings) > MAX_CLOSING:
        closings = closings[: MAX_CLOSING // 2] + closings[-MAX_CLOSING // 2 :]
    lines += ["", "Your closing sentences, by game time:"] + [f"  {clock(f)} {c[:300]}" for f, c in closings]
    last_packet = next((t["instruct"] for t in reversed(turns) if t["instruct"]), None)
    if last_packet:
        lines += ["", "The last packet of standing instructions you wrote:", last_packet[:3000]]
    lines += [
        "",
        "Answer in exactly this shape, plain text, nothing else:",
        "DECIDED: one sentence on what decided the game.",
        "NEXT PLAYER: up to five lines, one lesson each, concrete and in the words of the packet where possible "
        "(places, groups, units, timings); only lessons this game supports.",
        "WISHED: up to three lines, each one thing you wished you could order or see and could not; 'none' if none.",
    ]
    text = "\n".join(lines)
    if len(text) > MAX_CHARS:
        text = text[:MAX_CHARS] + "\n[cut]"
    return label, text


def main():
    args = sys.argv[1:]
    if not args or args[0].startswith("--"):
        sys.exit(__doc__)
    match_dir = os.path.abspath(args[0])

    def flag(name, default):
        return args[args.index(name) + 1] if name in args else default

    model = flag("--model", "claude-opus-5")
    effort = flag("--effort", "low")
    config_dir = flag("--claude-config-dir", os.path.join(os.path.expanduser("~"), ".claude2"))
    match_dir, batch, header, result, turns = load_match(match_dir)
    label, prompt = build_prompt(match_dir, batch, header, result, turns)
    if "--dry-run" in args:
        print(prompt)
        return
    os.makedirs(INBOX, exist_ok=True)
    out_path = os.path.join(INBOX, f"{os.path.basename(os.path.dirname(match_dir))}.md")
    snapshot = os.path.join(REPO, "run", f"usage-before-debrief-{int(time.time())}.json")
    usage_script = os.path.join(REPO, "run", "claude_usage.py")
    subprocess.run([sys.executable, usage_script, "--snapshot", snapshot, config_dir], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    env = {**os.environ, "CLAUDE_CONFIG_DIR": config_dir}
    started = time.time()
    proc = subprocess.run(
        ["claude", "-p", "--model", model, "--effort", effort, "--tools", "", "--strict-mcp-config", "--output-format", "text"],
        input=prompt, capture_output=True, text=True, env=env, timeout=300,
    )
    answer = proc.stdout.strip()
    if proc.returncode != 0 or not answer:
        sys.exit(f"claude -p failed ({proc.returncode}): {proc.stderr.strip()[:500]}")
    usage = subprocess.run([sys.executable, usage_script, "--since", snapshot, config_dir], capture_output=True, text=True)
    since = next((l.strip() for l in usage.stdout.splitlines() if "since snapshot" in l), usage.stdout.strip()[-200:])
    outcome = result["result"] if result else {}
    with open(out_path, "w") as f:
        f.write(f"# {label}: the player's debrief\n\n")
        f.write(f"Match `{os.path.relpath(match_dir, REPO)}`, commit {batch.get('commit', '?')}, {batch.get('opponent', '?')} on "
                f"{batch.get('map', '?')}: {outcome.get('outcome', '?')} after {outcome.get('game_minutes', 0):.1f} min, {len(turns)} turns. "
                f"Debriefed by {model} (effort {effort}) in {time.time() - started:.0f} s; {since}.\n\n")
        f.write(answer.rstrip() + "\n")
    print(f"wrote {os.path.relpath(out_path, REPO)}\n{since}")
    if usage.returncode == 3:
        print("STOP: the usage check reports a limit or overage", file=sys.stderr)
        sys.exit(3)
    print(answer)


if __name__ == "__main__":
    main()
