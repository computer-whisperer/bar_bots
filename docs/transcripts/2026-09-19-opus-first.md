# Strategist transcript digest — batch opus-first, match 00 (2026-09-19)

Opus (`claude-opus-5`) via `claude -p`, commit 6b115b5, BARb easy (Cortex) vs us (Armada, NW), Quicksilver Remake 1.24, 2x speed,
20 game-minute cap. Result: timeout, economy stuck at 2-3 extractors. 33 turns, median 5.3 s, $1.27 at list price.
Digest of turns, directives and notes; tool reads and unchanged routine checks are omitted. Raw file: the match directory's
`strategist-0.jsonl` (not kept in git).


**Game time 0:00. Routine check.**
- directives: `{"army_stance": "defend", "economy_focus": "expand", "ttl_seconds": 300, "wave_size": 14}`
- note: Opening: we start C2 (NW), enemy presumed F7 (SE), long diagonal. Prioritise metal expansion early, keep army home to guard extractors, wave threshold 14 so we don't dribble units away.

**Game time 2:15. Routine check.**
- note: 2:18 and still only 2 extractors / 1 constructor, metal income 6 — that's the real bottleneck. Keeping expand; if extractor count doesn't rise by ~3:00 I'll try production focus to get more constructors.

**Game time 3:00. Routine check.**
- directives: `{"army_stance": "defend", "economy_focus": "expand", "ttl_seconds": 180}`
- note: 3:03 metal at zero, income 6 from 2 extractors, while energy overflows (1350 stored, 117 income, 0 converters). Renewing expand — metal spots are the fix. Wish I could order 'build metal makers/converters' or 'more constructors' directly.

**Game time 4:37. Routine check.**
- directives: `{"army_stance": "defend", "ttl_seconds": 180, "wave_size": 16}`

**Game time 4:52. Our base is under attack: 2 enemies near C3.**
- directives: `{"army_stance": "defend", "economy_focus": "defence", "ttl_seconds": 100}`
- note: 4:55 raiders killed 2 extractors and a constructor (income 10.7 -> 6.9). Switching to defence focus for ~100s to get turrets covering the southern expansion spots, then back to expand; pure chasing with bots is losing the extractor trade.

**Game time 5:37. Routine check.**
- directives: `{"economy_focus": "expand", "ttl_seconds": 180}`

**Game time 6:22. Our base is under attack: 1 enemies near C3.**
- directives: `{"army_stance": "defend", "ttl_seconds": 180}`

**Game time 7:07. Routine check.**
- directives: `{"economy_focus": "energy", "ttl_seconds": 120}`
- note: 7:10 still only 2 extractors; outer spots keep dying to raids and our single constructor can't hold them. Energy income (137) is fully consumed by 2 converters, so switching to energy focus to stack generators+converters inside the base — a raid-proof metal source — then back to expand.

**Game time 7:52. Routine check.**
- directives: `{"ttl_seconds": 180, "wave_size": 16}`

**Game time 8:52. Routine check.**
- directives: `{"army_stance": "defend", "economy_focus": "expand", "ttl_seconds": 180}`
- note: 8:55 energy focus added generators but no new converters, so metal is still ~8/s on 2 extractors. Going back to expand; the only real fix is holding metal spots. Would value a directive to force converter count or to station the army on a specific spot as escort.

**Game time 9:37. Routine check.**
- directives: `{"army_stance": "attack", "attack_target": {"x": 4056, "z": 3624}, "ttl_seconds": 120}`
- note: 9:39 economy frozen at 2 extractors/8 metal because the lab consumes every scrap of metal; the 8 idle bots do nothing at home while 6 turrets already stop the small raids. Sending them to the midmap metal cluster at E5 (4056,3624) to kill enemy extractors — trading units for their economy is better than idling.

**Game time 10:06. Our base is under attack: 10 enemies near C3.**
- directives: `{"army_stance": "defend", "attack_target": null, "ttl_seconds": 180}`
- note: 10:09 a 10-unit enemy push hit C3 just as I launched the raid — recalling everything to defend; losing the commander/base outweighs any economic trade.

**Game time 10:51. Routine check.**
- directives: `{"ttl_seconds": 180, "wave_size": 20}`

**Game time 12:00. Routine check.**
- directives: `{"army_stance": "defend", "economy_focus": "expand", "ttl_seconds": 180}`

**Game time 13:00. Routine check.**
- directives: `{"economy_focus": "production", "ttl_seconds": 150}`
- note: 13:03 converters give us a metal surplus (201 banked, usage only 5) while outer extractors keep dying. Switching to production focus to turn that idle metal into another lab/constructors and more units.

**Game time 14:19. Routine check.**
- directives: `{"army_stance": "defend", "ttl_seconds": 180, "wave_size": 20}`

**Game time 15:21. Routine check.**
- directives: `{"economy_focus": "energy", "ttl_seconds": 150}`
- note: 15:24 10 converters now consume all 257 energy, so energy is the binding constraint on metal. Switching to energy focus to add generators that feed the converters.

**Game time 16:44. Routine check.**
- directives: `{"army_stance": "defend", "ttl_seconds": 180, "wave_size": 20}`

**Game time 17:47. Routine check.**
- directives: `{"economy_focus": "expand", "ttl_seconds": 180}`

**Game time 18:32. Routine check.**
- directives: `{"army_stance": "attack", "attack_target": {"x": 2328, "z": 4200}, "ttl_seconds": 120}`
- note: 18:35 army at 19 and economy stagnant at ~9 metal; turtling only loses slower. Committing the group to the enemy's near-side expansion at C5 (2328,4200) — short path home, hits the extractors feeding their raids. Base keeps 6 turrets.

**Game time 19:01. Our base is under attack: 3 enemies near C3.**
- directives: `{"army_stance": "defend", "attack_target": null, "ttl_seconds": 150}`
- note: 19:04 a 15-unit thud/storm force is at C3 directly in our wave's path. Recalling to defend so we fight it inside our turret umbrella rather than trading in the open.

**Game time 19:46. Routine check.**
- directives: `{"ttl_seconds": 180, "wave_size": 24}`
