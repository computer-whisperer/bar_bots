#!/usr/bin/env python3
"""What an LLM commander did, turn by turn, from its transcript (`strategist-<ai>.jsonl`, written by
`crates/bot/src/strategist/transcript.rs`): the game clock, what it called and with what, and its notes, deduplicated
(the prompt carries notes forward, so a note appears on every later turn until it is superseded).

usage: run/commander_turns.py run/matches/<batch>/<NN> [--calls] [--notes]
  default: one line a turn (clock, calls made, the note's first words); --calls prints every call's arguments;
  --notes prints every distinct note in full.
"""
import glob
import json
import os
import sys


def clock(frame):
    seconds = frame // 30
    return f"{seconds // 60}:{seconds % 60:02d}"


def calls_in(value):
    """Every tool call (name, arguments) inside a transcript record, however nested."""
    if isinstance(value, dict):
        name = value.get("name") or value.get("tool")
        args = value.get("input") if "input" in value else value.get("arguments")
        if isinstance(name, str) and isinstance(args, dict):
            if name == "orders" and isinstance(args.get("calls"), list):
                for entry in args["calls"]:
                    yield from calls_in(entry)
            else:
                yield name, args
            return
        for child in value.values():
            yield from calls_in(child)
    elif isinstance(value, list):
        for child in value:
            yield from calls_in(child)


def main():
    args = sys.argv[1:]
    show_calls = "--calls" in args
    show_notes = "--notes" in args
    dirs = [a for a in args if not a.startswith("--")]
    if not dirs:
        print(__doc__)
        sys.exit(2)
    for match in dirs:
        for path in sorted(glob.glob(os.path.join(match, "strategist-*.jsonl"))):
            print(f"== {path}")
            seen_notes = []
            frame = 0
            calls = []
            turns = 0
            wall = 0.0
            output_tokens = 0
            # A turn is a `turn` record (the prompt), its `tool_call` records, then `turn_end` and `result`.
            for line in open(path):
                try:
                    record = json.loads(line)
                except ValueError:
                    continue
                kind = record.get("kind")
                if kind == "turn":
                    frame = record.get("frame", 0)
                    calls = []
                elif kind == "tool_call":
                    calls.extend(calls_in(record))
                elif kind == "result":
                    usage = record.get("message", {}).get("modelUsage", {})
                    output_tokens += sum(m.get("outputTokens", 0) for m in usage.values() if isinstance(m, dict))
                elif kind == "turn_end":
                    turns += 1
                    wall += record.get("wall_seconds", 0.0)
                    names = [n for n, _ in calls if n not in ("orders", "note")]
                    new_notes = []
                    for name, a in calls:
                        if name == "note" and isinstance(a.get("text"), str) and a["text"] not in seen_notes:
                            seen_notes.append(a["text"])
                            new_notes.append(a["text"])
                    summary = ", ".join(names) or "(nothing)"
                    first_note = new_notes[0][:110] if new_notes else ""
                    print(f"{clock(frame):>6}  {summary:<52} {first_note}")
                    if show_calls:
                        for name, a in calls:
                            if name not in ("note", "orders"):
                                print(f"          {name} {json.dumps(a)[:300]}")
                    if show_notes:
                        for text in new_notes:
                            print(f"          note: {text}")
            print(f"  {turns} turns, {wall:.0f} s of wall time, {output_tokens} output tokens, {len(seen_notes)} distinct notes" + ("" if show_notes else " (--notes prints them)"))


if __name__ == "__main__":
    main()
