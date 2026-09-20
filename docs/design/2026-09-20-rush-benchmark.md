# Target design: the 1v1 benchmark — beat BARb medium the way experienced players do

Written 2026-09-20 at the user's direction ("1v1 is still a very valid bench; iterate this until we can beat BARb medium
in a similar manner and escalate to harder tiers; we need to be capable of more than one type of game"), before any
code. Two players' replays (`run/replay_match.py`, K-open-early-pawn-pressure-is-standard) show the manner: a lab by
0:50, five to twelve Pawns at BARb's base before 2:30, its commander dead at 4:45 and 6:08, from Quicksilver's north
start. Ours from that start: 1 win in 36 games today, at minute 39; lab at 63 s, first constructor at 88 s, first
wave at 20 soldiers (minute 8 or later), the harass party at minute 6.

## Judged by
Minutes to BARb medium's commander from Quicksilver's north start (the players: 4:45, 6:08), then from the other
starts and maps of the day's batches, then against the next BARb tier. Beside it the day's mechanism measures, so the
rush does not cost the macro game: extractors and income at minutes 3 and 5, extractors lost, trade.

## 1. The opening search wants the raiders (`crates/buildorder`, `Objective::Tempo`)
- The army term counts a soldier by when it can stand at the opponent's base: finished at t with speed v, it is
  there at t + walk / v; at or before the first-contact time it counts `contact` times its metal on top of `army`
  times its metal; later, that bonus falls to nothing over a window. Walk is the bot's walking distance from home to
  the nearest enemy base; the contact time is 150 s (the replays) until measured otherwise.
- Checked against the two replays' openings minute by minute (lab time, soldiers at 2, 3, 4 minutes, extractors,
  income), with `buildorder optimize` on a north-start record.

## 2. Early pressure (`army.rs` or `raid.rs`)
- The first handful of raiders (five) goes at the opponent's nearest extractors as soon as it exists, priced by the
  chase simulator against what is known to stand there (a turret, the commander), not by a clock: H-ARMY-HARASS's
  minute-6 clock and 20-soldier first wave stay for the main army.
- It keeps going while the simulator says the fight is won; it comes home when it is not.

## 3. Taking the kill
- Raiders at an opponent's base that find no army there go for the commander and the lab, and the home group is
  committed after them: the wave gate's odds, triggered by what the raiders see instead of by wave size.

## Order of work
1, then 2 and 3 together; each a commit with its A/B (minutes to the commander from the north start, 12 seeds an arm)
and its rows in `docs/heuristics.md`, `docs/knowledge/` and `docs/experiments.md`. Then army parts 3-4
(`2026-09-20-army-response.md`), judged in 2v2 as well.

## Status
- 2026-09-20, piece 1 built: `Objective::Tempo { contact }` (`anneal::Contact`, H-OPEN-CONTACT, weight 6 at 150 s over a
  120 s window, walk from the bot's route to the enemy base: 6758 on Quicksilver from the north). First A/B (rush-1-ab)
  showed the in-bot search using a third of its budget; `anneal_within` now runs rounds until the budget is spent
  and finds 400-560 army metal by minutes 2-3 where it found 54-108. Both players' orders (from their replays'
  engine records: player 2 mex mex win win win lab ..., lab ck then Pawns; Ben mex mex solar solar lab, lab ck ck
  then Pawns) are on the palette and simulate in order (`tests/replays.rs`): that needed radar (`radar_range` in the
  protocol and the record) and resurrection bots on offer, and construction turrets on. Simulated with our executor's
  3.5 s between builds the same orders come out a minute slow (lab at 75 s against the players' 50): that overhead is
  ours to remove (queued orders for mobile builders), not the simulator's.
- 2026-09-20, pieces 2 and 3 built: H-ARMY-PRESSURE (Pawns only, five as soon as they exist, priced by
  `assault_verdict` against remembered turrets and soldiers in sight, re-priced every tick in contact, marching
  together, home when outmatched or too small for the enemy commander's D-gun) and H-ARMY-KILL (at their base with no
  soldier in sight and the commander outnumbered, the home group is committed). rush-smoke2: five units strung out
  2000 elmos met BARb's commander one at a time and died in six seconds, hence the march and the commander rule.
  A/B rush-2-ab running (pressure and kill against neither, north start, 12 an arm).
- 2026-09-20, later rulings (the user): the benchmark moves to Comet Catcher (symmetric; Quicksilver's asymmetry is a
  variable to avoid and its north start a nonstandard layout); the commander's start position inside its box is the
  first genuine game input and belongs to the build-order search and the LLM commander's guidance; the bot is still
  too timid: Matt's first Pawn left for the enemy base as soon as it was built (83 s) and hurt an extractor at 146 s.
  Done for the last: the party is one raider, the rest join as they come. On the start position: in BAR an AI is
  placed by a human through the lobby (`aiPlacedPosition`, `game_initial_spawn.lua`), never by itself, so for games
  with people the bot can only walk to the point it wants at frame 0 (5 s on Quicksilver's north start), while the
  arena can fix positions in the script (StartPosType 0 with StartPosX/Z per team) so the search's choice is the
  spawn. To build: the search over start points inside the box (`Hello` carries the boxes), the walk as the plan's
  first step, the arena writing the chosen point. rush-4-ab (mirrored, north start): 1-11 against 0-12, no pressure
  party before minute 8 (a 60 s window made the term unreachable). Presence is now whole to the contact time, falling
  over 240 s, times the square of speed over a raider's; offline at weight 6 the plan is ck then six Pawns (7 soldiers
  and 2 constructors by minute 3), at 10 twelve Pawns and nothing else. rush-5-comet-ab running.
- 2026-09-20, start position and boxes: `buildorder::start` chooses the start inside our box when the arena may place
  us (`--place`, StartPosType 3); a human's placement is planned from as it stands, no walk added (the user's
  ruling). rush-5-comet-ab: pressure 3-4-5 against 1-8-3, wins at 30-36 min, no kill; a party of one was sent home as
  too few (fixed). rush-6 (placed 0-7-5, unplaced 1-10-1): no better. The user then noticed the arena's boxes were not
  the players': every batch so far ran on 30 % corner squares, while the lobby plays Comet Catcher as two full-height
  20 % strips (W against E) and Quicksilver as full-width strips, so neither side had the strip's extractor clusters
  (7 of a Comet strip's 14 spots lie outside the corner) and the searched start had no pair of spots within reach.
  The arena's default is now the lobby's boxes (`Boxes::Standard`, `crates/arena/startboxes.dat`); in Comet's strips the
  game spawns AIs diagonally, SW against NE. rush-7-comet-std-{place,noplace} running on them.
- 2026-09-20, rush-7 on the lobby's boxes: 0-12 placed, 1-9-2 unplaced. Not the placement: the party walked to the
  presumed enemy base (the box's centre snapped to a spot, `bases.rs`) and stood at an empty spot for five minutes;
  in Comet's strips BARb spawns at an end, 2000 elmos from the centre, and the home group was committed to the same
  empty spot by H-ARMY-KILL. Now the party scouts the enemy box's metal spots nearest first until a base is found
  (`raid.rs` `unscouted_box_spots`), and the kill needs a building of theirs standing there. rush-8-scout-{place,
  noplace} running (`target-scout`).
- 2026-09-20, rush-8 (scouting) timeline, placed arm: the first Pawn leaves at 1:05-2:15 (median 1:43; Matt's at
  1:23) and the first building of theirs is seen at 5.1-6.2 min in every game (Matt's Pawn hurt an extractor at 2:26),
  with an LLT already beside it. The party crawled at a third of a Pawn's speed: H-ARMY-MARCH held the leaders for the
  body of the party, and the body was the joiners trickling out of the lab 2000 elmos behind. Now the party's body is
  the members within 1500 of the front, and it alone is held for, priced and stands at the target (`target-march`).
  Left for later: the first Pawn a minute late (the executor's 3.5 s between builds), and BARb's Fleas taking 3-7
  extractors a minute at home from minute 4 (H-ARMY-CONTACT's floor of two).
- 2026-09-20, the production gap (rush-9 placed games against the replays, Pawns finished per game minute 1-4): Matt
  5 / 7 / 6 / 8 (26 by minute 5), Ben 3 / 4 / 5 / 8; ours 0-2 / 3-6 / 1-5 / 0-2 (9 by minute 5), with 14 extractors by
  minute 5 to Matt's 7 and two more constructors at minute 5. The opening plan's Pawns stop at its horizon (300 s) and
  H-PROD-BATCH's mixed batch takes over. BARb's Fleas take 3-7 of our extractors a minute from minute 3; neither player
  lost one, because BARb was dead or dying by then. The next lever is the production posture behind the pressure:
  Pawns without a gap from the lab's first minute until the kill or the pressure fails (army part 3, posture) - a
  decision for the user, with the LLM's `build_priorities` as the other route.
- 2026-09-20, back on Quicksilver (the user: parity with the replays until the players send Comet games; map size
  changes timings and aggression patterns). On the lobby's boxes the layout reproduces the replays (BARb south at
  (3371, 5556) as in Matt's game; the search places us at (2152, 972), beside both players' starts). The commander's
  walk for the lab (its anchor lay on the first extractor's nanoframe) is fixed: mex, solar, lab at 22 s, solar, mex,
  solar without a step. rush-10: 0-11-1 both arms. The first Pawn is on the players' schedule now (leaves 1:00-1:45,
  sees the base 1.6-3.1 min) and then every sortie comes home "outmatched" on meeting BARb's commander out front,
  four times a game with a minute's rest each. The user: retreating from the commander is right (high power, slow);
  going home without trying the base elsewhere is the flaw, one of a bucket of troop-movement problems. Now the
  party goes for another extractor it is priced to win at, or waits out of reach for reinforcements; home only with
  nobody left (`target-harass`, rush-11). Still open behind it: Pawns per minute 0 / 3 / 5 / 4 / 2 against Matt's
  5 / 7 / 6 / 8.
