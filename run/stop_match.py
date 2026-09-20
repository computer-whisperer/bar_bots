#!/usr/bin/env python3
"""End a running arena match by hand, keeping its replay and record.

usage: run/stop_match.py run/matches/<batch>[/<NN>] [loss|win]

Writes `stop` into the match directory (every match directory of a batch when given the batch). The arena sees it within
a second, records the match as the given outcome (undecided, a timeout, when none is given) with `called: true`, and
ends the engine as it ends every match: `/kill` over the autohost channel, which lets it write the replay. Killing the
arena or the engine instead loses the replay and the result line.
"""
import os, sys

if len(sys.argv) < 2 or (len(sys.argv) > 2 and sys.argv[2] not in ("loss", "win")):
    sys.exit(__doc__)
target = sys.argv[1]
dirs = [target] if os.path.exists(os.path.join(target, "engine.log")) else sorted(
    os.path.join(target, d) for d in os.listdir(target) if os.path.exists(os.path.join(target, d, "engine.log")))
if not dirs:
    sys.exit(f"no match directory (with an engine.log) at {target}")
for d in dirs:
    records = [f for f in os.listdir(d) if f.startswith("record-") and f.endswith(".jsonl")]
    # The arena appends the result line when the match is over (the replay file exists, empty, from the first frame).
    if any('"t":"result"' in open(os.path.join(d, f)).readlines()[-1].replace(" ", "") for f in records):
        print(f"{d}: already over")
        continue
    with open(os.path.join(d, "stop"), "w") as f:
        f.write(sys.argv[2] if len(sys.argv) > 2 else "")
    print(f"{d}: stop written")
