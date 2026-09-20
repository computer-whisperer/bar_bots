#!/usr/bin/env python3
"""What a batch's fighting cost and bought, per A/B arm: the mechanism measure an arena batch is read by.

    run/batch_trade.py <batch dir> [--by-corner]

Army metal killed per metal lost, from the match records (`run/analyze_match.py`'s `Match`): every soldier and
turret death on both sides, ours from our own events, theirs from the truth file a match run with
WITHIN_REASON_OBSERVE=1 writes. Matches without a truth file are counted for our losses only and named, because
a ratio computed without one is our losses over what our units happened to watch die.
"""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from analyze_match import Match  # noqa: E402

FIGHTING = ("army", "commander", "turret")


def main(argv):
    args = [a for a in argv if not a.startswith("--")]
    if len(args) != 1:
        print(__doc__)
        return 1
    batch = Path(args[0])
    by_corner = "--by-corner" in argv
    results = {}
    with open(batch / "results.jsonl") as handle:
        for line in handle:
            r = json.loads(line)
            results[r["index"]] = r

    rows, aborted = [], []
    for index, result in sorted(results.items()):
        directory = batch / f"{index:02d}"
        if not directory.is_dir():
            continue
        try:
            match = Match(str(directory))
        except SystemExit:
            continue
        killed = lost = 0.0
        for _, side, name, _, _, metal, _ in match.deaths():
            if match.cls(name) not in FIGHTING:
                continue
            if side == "ours":
                lost += metal
            else:
                killed += metal
        if result["outcome"] == "Aborted":
            # An engine run that never finished has partial deaths and no game time: it would drag the per-game
            # means and the median down while not appearing in the W-L-T count.
            aborted.append(index)
            continue
        rows.append({
            "index": index,
            "arm": result.get("arm") or "-",
            "corner": result["our_corner"],
            "outcome": result["outcome"],
            "minutes": result["game_minutes"],
            "killed": killed,
            "lost": lost,
            "truth": bool(match.truth),
        })

    if aborted:
        print(f"matches {aborted} were aborted and are left out\n")
    missing = [r["index"] for r in rows if not r["truth"]]
    if missing:
        print(f"no truth file in matches {missing}: their 'killed' is only what our units saw\n")

    def show(name, chosen):
        if not chosen:
            return
        killed = sum(r["killed"] for r in chosen)
        lost = sum(r["lost"] for r in chosen)
        wins = sum(1 for r in chosen if r["outcome"] == "Win")
        losses = sum(1 for r in chosen if r["outcome"] == "Loss")
        timeouts = sum(1 for r in chosen if r["outcome"] == "Timeout")
        print(
            f"{name:<16}{len(chosen):>4} games {wins:>3}-{losses:<3}-{timeouts:<3}"
            f"  killed {killed / len(chosen):>7.0f}  lost {lost / len(chosen):>7.0f}"
            f"  per game, ratio {killed / max(lost, 1):>5.2f}   median {sorted(r['minutes'] for r in chosen)[len(chosen) // 2]:>5.1f} min"
        )

    print(f"{'arm':<16}{'n':>4}{'':>7}{'W-L-T':<9}{'army metal':>16}{'':>14}")
    for arm in sorted({r["arm"] for r in rows}):
        show(f"arm {arm}", [r for r in rows if r["arm"] == arm])
        if by_corner:
            for corner in sorted({r["corner"] for r in rows}):
                show(f"  {corner}", [r for r in rows if r["arm"] == arm and r["corner"] == corner])
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
