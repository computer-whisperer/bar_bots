# Jev (TypeSafe's System One model) as a player of the keyboard

What the pianist games (`docs/design/2026-09-21-pianist.md`, `docs/harness/jev.md`) found about how Jev answers a menu
over a picture of the game. Scope: model jev-1.13.0, the picture and menus of `crates/bot/src/brain/pianist/`, BARb easy
on Quicksilver Remake 1.24.

### K-jev-words-not-numbers
**Claim.** Jev does not count what it has against a plan and does not read a stall off numbers; a count or a stall changes
its answer only when the words beside the option say what the number means ("we have 20 constructors already: far too
many", "energy: STALLING").
**Status.** supported (2026-09-21)
**Evidence.** pianist-smoke-1: the lab chose a constructor at 0.5-0.9 thirty times running against "two constructors first,
then raiders", with `ours.constructors` in the picture rising from 0 to 30 and `soldiers: 0`; the probability drifted from
0.93 to 0.5 with the count and flipped only when metal read "full". Energy stood at 0 from 0:50 to the end with the line
reading "77 coming in, 77 going out: in balance" (the engine caps spending at income once the store is empty). pianist-smoke-2,
with the count in words in the option and the question: the lab switched to raiders at 3:10 with three constructors and to
line units at 6:03. pianist-smoke-3, with the energy state in the generator option's words: energy at 1293 of 1306 by 5:00.
**Would be wrong if.** A picture with the counts as numbers only produced the plan's switch at the right count.
**Used by.** H-HANDS-MENU.

### K-jev-split-vote
**Claim.** A Choice with several near-equivalent options (one per free spot) spreads its probability over them, and a
single alternative wins with a fraction of the total; the remedy is one option for the kind of action and a parallel
question for its parameter (the spot in `where`).
**Status.** supported (2026-09-21)
**Evidence.** pianist-smoke-2: with six `extractor_at_spot_N` options, constructors chose `assist_lab` at 0.20-0.47 over
extractors at 0.14-0.25 each (0.6 between them), 4 extractors at 5:00; pianist-smoke-3 with one `extractor` option and the
spot in `where`: 8 / 14 extractors at 4:00 / 6:00.
**Would be wrong if.** The one-option form chose extractors no more often on the same pictures.
**Used by.** H-HANDS-MENU.

### K-jev-answers-drift
**Claim.** The same state and questions on the same model version do not give the same answers two days apart; the top
choice can flip. Between calls minutes apart the wobble is a few hundredths.
**Status.** supported (2026-09-21)
**Evidence.** `experiments/jev/01`: `escort` 0.72 on 2026-09-19, `defend` 0.58 / `escort` 0.39 on 2026-09-21, jev-1.13.0
both days; scenarios 02 and 03 moved by 0.02-0.03.
**Would be wrong if.** A re-run reproduced the first day's numbers.
**Used by.** H-HANDS-SWITCH.

### K-jev-where-needs-its-premise
**Claim.** A parallel parameter question ("where, if the action needs a place; otherwise answer home") is answered with
the fallback nearly every time, since it cannot see which action the other question chose; a speculative question works
only with its premise stated ("suppose the group sends a scout: where should it look?"), one per kind of action.
**Status.** conjectured (2026-09-21; the per-action form is smoke-6's test)
**Evidence.** pianist-smoke-5: `where` answered `home` on all but a handful of 1,300 asks; scouts sent home, advances to
home, forty one-unit groups. TypeSafe's own fan-out pattern says to state each speculative premise explicitly.
**Would be wrong if.** The per-premise questions still answered the same fallback whatever the chosen action.
**Used by.** H-HANDS-MENU.

### K-jev-instructions-are-standing
**Claim.** The hands read the whole packet afresh at every ask with no memory of the last: a time-bound command in it
("go home now, then build a lab") is matched again each time, so two steps that both fit the moment alternate, and an
instruction written as a state ("the commander stays at home and builds the lab there") holds.
**Status.** supported (2026-09-21)
**Evidence.** pianist-player-1: under "commander: go home now and build a lab at home" the commander, asked every ten
seconds, chose `retreat_home` at 0.76 and 0.81 and `lab` at 0.52 and 0.65 in turn (2:20, 2:30, 2:40, 2:50), abandoning
the started lab each time; the decayed frames were reported as losses and the player diagnosed aircraft.
**Would be wrong if.** The same packet, re-asked, kept the commander on the started lab once it stood at home.
**Used by.** H-HANDS-STARTED; the player's role prompt (`strategist/player.md`: write states, not commands).

### K-hands-abandoned-frames-read-as-losses
**Claim.** A nanoframe its builder walks away from decays and arrives as `UnitDestroyed` with no attacker; counted as a
loss it misleads every reader (the traded line, the fights line, the territory's raided memory, the picture's notes).
**Status.** demonstrated (2026-09-21)
**Evidence.** pianist-player-1: "lost armlab to unseen at home x2", "lost armwin to unseen at home x7" and 500 metal
traded with no enemy within 1,800 of home before 6:04 (the truth file); the player's turns 4 and 5 diagnosed an air
raider and ordered anti-air. Fixed the same day: `Brain::abandoned` (a destroyed unit that was being built at the
last look, with no attacker) is accounted as abandoned everywhere.
**Would be wrong if.** The engine reported an attacker for decayed frames, or never destroyed them.
**Used by.** `brain/briefing.rs` `track_losses`, `territory.rs`, `pianist/mod.rs`.

### K-hands-advance-stopped-at-turret-reach
**Claim.** Under the pianist a group advancing with `fight_to` was committed to no turret, so the control lane stepped
every soldier out of the first turret's reach and held it at the edge while its way to the goal crossed one: an attack
on a base could not be carried out whatever Jev or the player said. Committed to everything (as the heuristic waves
are) the advance walks in; whether it should is the odds question the menu words put to Jev.
**Status.** demonstrated (2026-09-21; the fix's effect is pianist-player-3's to show)
**Evidence.** pianist-player-2: the ball of 82 stood 2,255 short of enemy_base from 21:00 to 22:40 with `continue`
(advance) answered on every ask; the record shows 275 `move` and 2 `fight` commands to its units in the stall minute;
the truth file puts a Guardian 1,002 from it and the nearest laser tower at 1,584.
**Would be wrong if.** The ball had walked in under the same commitment, or the flee steps had another source.
**Used by.** H-HANDS-GROUPS (`micro.rs` `note_commitments`).

### K-hands-hold-in-the-base-stepped-out
**Claim.** A pianist group that arrived at its `fight_to` goal was set to Hold, and a hold was priced against no
turret, so the ball that had fought its way into the enemy base stood at `enemy_base` stepping out of the towers'
reach; likewise an engaging group was never committed to the commander, so a ball outweighing the lone enemy
commander many times stepped back from it. Both are the commitment mapping, not Jev and not the player.
**Status.** demonstrated (2026-09-21; the fix's effect is pianist-player-5's to show)
**Evidence.** pianist-player-4: the picture's footwork line read "35 of its 35 soldiers are being held back by
their own footwork" on a group "holding" at enemy_base at 10:33-11:33, and "28 of its 34" on the same group
"attacking party_1 (1 armcom) at spot_19" at 12:09-12:30; the player named the first as the game-2 shape and
worked round it with a sweep of named spots.
**Would be wrong if.** The held-back counts came from the lethal-fight test rather than the unpriced-threat test
(the ball was at full strength and outgunned nothing there: the towers were the only threats).
**Used by.** H-HANDS-GROUPS (`micro.rs` `note_commitments`).

### K-hands-lane-untested-under-the-pianist
**Claim.** The control lane's rules were measured on the heuristic bot's Pawn raids (H-MICRO-LANE: micro-ab2, 3-7-2
against 0-12; H-MICRO-FAN: matt-fan3) and never apart from each other for focus and kite, and never at all under the
pianist, where the groups are Mace balls with different commitments. The lane models a unit as a point that moves at
full speed in any direction at once, and the protocol carries no turn rate or acceleration, so a unit re-stepped every
six frames turns rather than walks (the user, watching: "rapidly changing direction causes pawns to mill about rather
than decisively moving towards or away from a threat").
**Status.** conjecture (2026-09-21); the milling counters (H-HANDS-LANE) are the instrument, a raw-against-laned
batch the test.
**Evidence.** The heuristics ledger's status column for the four lane rules; the user's observation.
**Would be wrong if.** A raw ball traded worse than a laned one in a batch, or the milling counters showed path
close to net displacement under the flee.
**Used by.** H-HANDS-LANE.

### K-hands-far-places-never-on-the-menu
**Claim.** The picture's places were home, enemy_base, our spots, the ten nearest free spots, the six nearest of
theirs and three passages, so a place in the far half of the map was not on any `where` question and an instruction
naming it did nothing; a deep attack could not be ordered except through `enemy_base`. Named spots and marks fix
the reach; whether Jev picks a far place when told to is the next thing to see.
**Status.** demonstrated (2026-09-21)
**Evidence.** pianist-player-5: the player named spot_36 in four packets from 9:37 and wrote at 10:42 "Only nearby
places appear in the picture, so spot_36 was never on the menu and the ball kept re-picking the empty enemy_base",
and at 13:21 "I wish I could name an unexplored map cell as a destination"; the jev log's places carry no spot_36.
**Would be wrong if.** spot_36 had been unreachable on foot (it is listed as walkable in the map tool).
**Used by.** H-HANDS-NAMED-PLACES.

### K-hands-frame-placed-off-the-ordered-point
**Claim.** The engine places a building's frame where the site search settles, up to about a building's width from
the point ordered (200 elmos for a windmill beside a metal spot), so a started-build test that looks for the frame
within 200 of the ordered point misses it; the builder is then asked again, Jev answers the packet's word
("generator") over `continue`, and the hands order a second frame at the same point, which the engine places beside
the first. Each such answer abandons the last frame.
**Status.** demonstrated (2026-09-22)
**Evidence.** pianist-player-6: build orders to the commander at (3855, 2135) at 0:40, 0:50 and 1:00 created frames
at (4056, 2136), (4056, 2072) and (3992, 2136); the first two decayed at 1:20 and 1:29; the jev log has the
commander "building a armwin at spot_10" and answering `generator` at 0:50 and 1:00 with `continue` on offer. The
user, watching: "we are still leaving wind turbines 80% built and dying in the very early game".
**Would be wrong if.** The decayed frames had been abandoned for a threat (no party was within 800) or the engine
had refused the first site (it created a frame each time).
**Used by.** H-HANDS-STARTED.

### K-hands-spot-read-as-taken-by-its-neighbour
**Claim.** The picture described a metal spot by every building of ours within 350 of it, so a spot 300 from a taken
spot read "our extractor, our lab, our wind generators", the extractor menu (which offers only spots whose words
begin "free") never offered it, and it was never taken: spot_10, 150 from our start, in every player game.
**Status.** demonstrated (2026-09-22; the user, from the replay: "one of its starting mexes is never taken")
**Evidence.** Games 9, 11, 12, 13: spot_10 at (3864, 2088) never had an extractor; spot_12's extractor stands 300
from it; the picture at 1:00 of game 13 reads spot_10 as "our armmex (extractor), our armlab (lab), our armwin ...".
**Would be wrong if.** The engine had refused the site (no refusal was logged, and no order was ever given).
**Used by.** the picture's spot words: the extractor within the spot's radius takes it, the buildings beside it are
said beside it.

### K-hands-ball-chases-lone-raiders
**Claim.** With `engage` the only answer that goes at an enemy party, the hands send the whole group after any raider
at one of our extractors, and a group of ten to twenty-five never catches a lone Fav or Flash while a second raider
kills what the group left; nothing local reacts to a raider at a structure.
**Status.** demonstrated (2026-09-22, the user watching realtime-2: "a fair amount of poor control")
**Evidence.** realtime-2 (`run/matches/1790042330-realtime-2`): 18 engage picks, 11 of them the whole ball (10 to
25 units) after a single Fav, Stump or Beaver, one 2,362 elmos away; 10 units chased one Fav from 4:56 to 5:34
through four re-picks; at 8:27 to 8:38 a Fav sat on our extractor at B3 killing a windmill and an extractor while
the ball of 24 was 2,700 away after three Flashes; the game's fight ledger: a lab, 3 extractors and 4 windmills lost
to Favs, 12 windmills and 3 constructors to raiders in all.
**Would be wrong if.** The chases had been the player's instruction (the packets of realtime-2 said "engage raiders
in sight", with no size).
**Used by.** H-HANDS-DETACH (`send_against`); the player's prompt, "Defence is yours".

