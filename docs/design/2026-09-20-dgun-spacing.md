# Target design: spacing inside the enemy commander's D-gun reach

Written 2026-09-20, late, at the user's direction ("Build the spacing rule and measure it on the same seeds"), after
the count that motivates it: in matt-plan and matt-raidprice (twelve games each, Matt's order) the D-gun killed 14
and 10 of ours by 10:00, against 22 and 23 to the commander's lasers and about 250 army deaths in all; but all but
one of them came in four shots of three to seven Pawns each, killed within 6 to 18 frames of one another inside a
box 40 by 70 elmos, and two of those shots ended the game's first dive (matt-plan 07 at 3:21, 08 at 4:53).

## What is true today
- The D-gun (`command_fire` weapon, 250 reach for Armada, 262 for Cortex, a 300 elmo/s projectile, one shot every
  0.9 s, 99,999 damage) passes through everything on its line. A party that comes at the commander on one bearing
  loses one unit per unit of its depth per shot.
- The lane (`micro.rs`) has no D-gun: a commander is a source with its lasers' reach and damage rate. H-MICRO-FLEE's
  lethal test (their damage over 1.5 s against the unit's health) never fires inside the commander's reach for a
  Pawn, and the odds clause keeps a party there. A committed party (900 metal, `Commitment::All`, or a raid
  `fights_commander`) is meant to stand there.
- H-MICRO-SPREAD (retired, e0f165c) spread attack orders over a block at the destination for every wave; waves
  already arrived scattered over 300 elmos and it measured nothing. This is not that: it acts only inside the D-gun's
  reach, on the shot's line.

## The change
1. **A source knows its D-gun reach** (`Source.dgun`, from the simulator's table: the longest `command_fire`
   weapon's range; 0 for everything but commanders).
2. **H-MICRO-FAN** (`micro.rs` `fan`, after flee and before focus): for each commander of theirs in sight or
   remembered, every soldier of ours within its D-gun reach plus 40 whose line from the commander passes within
   32 elmos of a friend nearer the commander is in that friend's shadow: it steps 48 elmos across the line, to the
   side with fewer of ours, on reachable ground, and holds the claim as flee does (re-issued after 32 elmos and 6
   frames; released to its order when no longer shadowed and the claim has stood 30 frames). The unit nearest the
   commander on any line never steps: the rule spreads the party into a fan, it does not withdraw it. Commitment
   is not consulted: a sidestep leaves no fight.
3. The ledger (`run/micro_ledger.py`) reports deaths to the D-gun (a commander's kill whose killing blow is more
   than three times the unit's health) and how many shots they came in (kills within a second of each other).

## Judged by
- D-gun deaths and shots by 10:00 on the same twelve seeds as matt-raidprice (10 deaths in 4 shots), and the
  batch's other ledger lines (deaths to the commander, exchange, buildings killed by 5:00) for what the sidestep
  costs.

## Status
- 2026-09-20: written; nothing built.
- 2026-09-20 night, built and measured twice (ledger: matt-fan, matt-fan2). First: the zone was the reach plus 40
  and the rule fired ten times in twelve games, because every D-gun death is a unit closing at full speed, killed at
  240-280 from the commander as it enters the reach. Second, with the zone at the reach plus 260: the rule fired 89
  times a game and one shot still took three, twice. The geometry of every multi-kill shot: victims within 20 elmos
  either side of the line and 30-50 deep along it (a shot is about 40 wide), and lines to one point converge, so
  the shadow is now measured at the edge of the reach (a friend's line passing within 48 there) and the step opens
  64 there (up to 150 here). Third batch: matt-fan3.
- 2026-09-20 night, matt-fan3 (ledger): the angular fan. D-gun deaths 6 in 5 shots, none over two, against 10 in 6
  with a four before it; exchange 0.89 against 1.48; 7-4-1, which is noise at twelve games (matt-fan, with the rule
  firing in one game, was 1-8-3 on the same seeds). Kept. Open: the unit re-claimed every few frames as its order
  walks it back onto the line (a fanned unit could hold its offset while it approaches), and the 33 laser deaths
  the fan does nothing about.
