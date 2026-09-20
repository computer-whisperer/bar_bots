# Verdict pass: why was this game lost?

One analyst (a subagent, or you) per lost match, working only from `run/analyze_match.py` (see `arena.md`,
"Post-game analysis"). The match must have been played with `WITHIN_REASON_OBSERVE=1` so the opponent's side is ground
truth. The analyst writes `verdict.json` into the match directory; `run/tally_verdicts.py <batch dir>` adds them up, so
that we fix the commonest cause rather than the latest one seen.

## Procedure for the analyst
1. `run/analyze_match.py <match>`: read the curves first. Find the last minute the game was still level (extractors and
   army value within ~25 %) and the first minute it clearly was not. The decisive moment lies between them; what
   happens after it is consequence, not cause. A game that was never level was decided by the opening or economy.
2. Read the engagements in that window, and the candidate causes. Pull scenes (`--engagement N`, `--scene MM:SS X Z`)
   for the one or two engagements that moved the curves most, and look at what each side had on the spot, where it
   was, whether our fighters were together, whether turrets were involved, and what role our soldiers had (home group,
   attack wave, squad).
3. Decide. Prefer the earliest sufficient cause. Do not blame the final base fight for a game that was lost ten
   minutes earlier. Say what evidence would change your mind if the call is close.

## `verdict.json`
```json
{
  "match": "run/matches/<batch>/<NN>",
  "last_level_minute": 10,
  "decided_by_minute": 13,
  "decisive_moment": {"time": "11:54", "grid": "G6", "what": "one sentence"},
  "primary_cause": "<one tag>",
  "contributing": ["<tag>", "..."],
  "evidence": ["short statements with numbers from the report or scenes"],
  "preventing_rule": "the rule or lever that would have prevented it, concretely",
  "confidence": "high | medium | low",
  "notes": "anything that does not fit the tags, or looks like a bug in the bot or in the analysis tool"
}
```

Tags (use `other` with a note rather than stretching one):
- `blind_wave_into_defence`: an attack wave walked into a force or turret line worth clearly more than itself.
- `expansion_raided_undefended`: extractors and constructors lost to raiders with no defenders or turrets in place.
- `defenders_out_of_position`: defenders existed but were elsewhere, arrived late or arrived strung out.
- `lost_fight_at_equal_or_better_value`: we had the value on the spot and still lost: unit mix or target handling.
- `out_massed_at_level_economy`: economies level, but their army grew faster with no big engagement to explain it.
- `economy_never_grew`: behind on extractors and builders from the opening, without raids to explain it.
- `idle_resources`: metal banked or energy stalled for minutes while behind.
- `commander_exposed`: the commander died while the game was otherwise still contestable.
- `tech_gap`: tier-2 or heavier units we had no answer to.
- `other`
