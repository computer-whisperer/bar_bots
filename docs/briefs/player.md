

# What we know (the player's brief, written 2026-09-21 evening)

This is the project's accumulated knowledge, from some hundred recorded games against this opponent's medium setting,
unit duels, the opponent's own configuration, and seven games of your hands playing alone from a default packet against
its easy setting. Each line names its source entry in `docs/knowledge/`. It is evidence, not orders: say in a `note`
when the game in front of you contradicts it.

**Units (tier-1 bots; metal / health / speed / range).** Armada: Tick `armflea` 21/60/132/140 scout; Pawn `armpw`
54/370/87/180 raider; Rocketeer `armrock` 120/720/51/475; Mace `armham` 130/1000/46/380 line unit; Centurion `armwar`
270/1590/45/325 brawler; Crossbow `armjeth` anti-air ONLY, it cannot hit ground units; Lazarus `armrectr` 130, repairs,
reclaims and resurrects, cannot fight. Cortex: Grunt `corak` 43/280/81/215 raider; Aggravator `corstorm` 110/740/48/475;
Thug `corthud` 140/1100/45/380 line unit; Trasher `corcrash` anti-air only; Graverobber `cornecro` resurrects.
[K-units-t1-bot-roster, K-units-anti-air-is-air-only]
- At equal metal, head on: Mace and Centurion (Cortex: Thug) win most tier-1 matchups, including against everything this
  opponent fields and against light turrets. Pawn beats only Grunt and loses badly to Thug. Rocketeer loses to every
  mobile unit it cannot outrange, and is for turrets and buildings: it outranges the light laser tower (475 against
  430). [K-units-duel-line-bots-beat-raiders, K-units-duel-range-vs-turrets]
- Raiders are for what cannot shoot back: extractors, constructors, lone turrets under construction. A Pawn or two
  standing on an approach also stops a lone enemy raider; chasing raiders with the army does not. [K-units-dont-chase-raiders]
- Fights are decided by value on the spot: the side with more metal of soldiers there, a turret counting about three
  times its metal, loses less nine times in ten; beyond 2:1 almost always. Our units move at different speeds and a
  group sent across the map arrives strung out unless it is sent with `fight_to`, which marches it together.
  [K-army-combat-prediction, K-army-waves-die-to-static-defence, K-army-piecemeal-midmap]

**This opponent (BARb).** It has no resource or vision cheats. [K-barb-no-resource-cheats]
- It raids from the first minutes: raiders gather at its base and leave as a squad of about four, then every new raider
  joins the raid at once. They go for buildings where the threat is low, builders and extractors first, outermost
  first, and skip a target whose defence is worth more than about three quarters of the squad: one turret and two
  soldiers on an extractor is usually enough. [K-barb-raid-timing, K-barb-raid-threat-gate]
- It builds a turret beside nearly every extractor, and forward turret nests in our half later on. Its base has turrets
  on a clock (minutes 2, 4, ...). Its commander builds at home and is the win condition; in one easy game it walked into
  our half with a party and died to a group of ours that outweighed it. [K-barb-medium-observed-build,
  K-barb-base-defence-clock]
- It hoards its army at home as one block and sends it when it judges itself stronger than what it has seen of ours;
  the block then walks to our nearest weakly defended cluster. On medium the block is about 500 metal at minute 4,
  1,250 at 6, 2,000 at 8, 2,800 at 10, 3,800 at 12 (games vary by a third either way); easy is softer and slower, and
  its column has come at minutes 8 to 16 in your hands' games. After a wave of ours dies in its half, it resurrects the
  wrecks and they fight for it. [K-barb-medium-army-curve, K-barb-attack-gate, K-army-dead-waves-are-resurrected]
- It counter-builds against what it has seen: riot units against raiders, assault units against static defence; its
  second factory is a tier-2 lab by minute 15-20 at the latest. The game should be decided, or well in hand, by then.
  [K-barb-response, K-barb-tier2, K-eco-t1-ceiling]

**Economy.** [K-eco-production-is-the-bottleneck, K-eco-expansion-before-conversion, K-open-sim-energy-ratio]
- Metal in the bank is army we do not have: when metal is banking up, a second lab or a construction turret, not
  converters. Extractors beat converters while free spots remain. Energy only needs to keep storage from emptying;
  laser turrets stop firing in an energy stall, and the picture says STALLING when it happens.
  [K-units-laser-towers-need-energy]
- Constructors die in the field walking alone to far spots. Expand in steps the army has already covered.
  [K-army-verdicts-v18]

**This map, Quicksilver Remake, from the north-west corner start (the games so far: `--corner nw`, mirrored).**
[K-maps-terrain-not-straight-lines, the terrain picture in the `map` tool]
- 38 of the 44 spots can be walked to; six on islets cannot (the map's `walk_from_home` is null for them). The
  nearest spots are on and just below our plateau; the middle rows are the contested ones and change hands all game.
  The opponent starts on the mirror plateau across the open middle, about 3,900 on foot. The map's `passages` list
  names the narrow ways; the picture names the three narrowest `passage_1` to `passage_3`.

**What your hands did alone (pianist-smoke-1 to -6, pianist-easy-1, pianist-audit-1: four wins, four losses and a
timeout against easy from the default packet, `docs/experiments.md`).** [docs/knowledge/jev.md]
- The economy from the default packet is strong: 8 extractors by 4 minutes, 14 by 6, income 30 by 7, several labs and
  a hundred soldiers by 15. What lost games was the army: it holds where it stands unless told where to stand, and it
  engages only when the menu's odds words say it outweighs the party; it never went for the enemy base from the default
  packet (the base reads "not found" until a scout has stood there, and one scout at a time goes there and mostly
  dies on the way). A timeout at 40 minutes with 206 soldiers at home is the shape of a packet with no attack plan.
- The commander died twice walking to far spots for extractors beside the enemy; it retreats when the picture says a
  party outweighs it alone, and the packet should keep it near home after the opening.
- Words the hands act on: counts as words beside the option ("far too many constructors"), stalls as STALLING, the
  odds as "we outweigh it"; numbers alone change nothing. One kind of action per option; the place in a parallel
  question, which is why instructions name places. [K-jev-words-not-numbers, K-jev-split-vote,
  K-jev-where-needs-its-premise]

**What earlier players did, right and wrong.** [experiments ledger: pianist-player-1]
- pianist-player-1 (2026-09-21, Opus at low effort, lost at 14 minutes): the opening packet was sound and the hands
  played it (five extractors by 2:00). At 2:13 it wrote "commander: go home now and build a lab at home"; the hands
  read the packet afresh every ten seconds and alternated going home with starting the lab, abandoning the frame each
  time, and the report of the day called the decayed frames losses to something unseen. The player read that as an
  air raider, spent the next four minutes on Crossbows (anti-air only, six of them) and generators that were abandoned
  in turn, and met the first real raid (ten Flashes at 7:48) with three Maces. Both harness faults are fixed
  (H-HANDS-STARTED; abandoned frames are reported as abandoned). Its lesson: write the packet as states that hold
  ("the commander stays at home and builds the lab there"), never as a sequence of commands to be done once.
- pianist-player-2 (2026-09-21, Opus at low effort, lost at 27 minutes): the best economy and army of the series (14
  extractors by 6:00, a hundred soldiers by 18:00) and a loss all the same. Three waves went at enemy_base one after
  another and were ground down piecemeal; the whole ball of 82 stood short of the base for five minutes under
  artillery while the hands answered "continue": the hands of that day would not walk a group into a turret's reach
  (fixed: `fight_to` now fights its way through turrets, so it is the attack, and the odds words on the option say
  what it faces). Meanwhile the raids took the middle spots behind the attack (16 extractors to 8) and every field
  constructor died: an attack needs a home guard on the passage the raids use, and constructors told to rebuild
  behind it. The ball also chased single Flashes when told to engage parties it outweighs: name a small raider-hunting
  group for that and keep the ball's instruction to holding and advancing.
- pianist-player-3 (2026-09-21, Opus at low effort, WON at 11 minutes, the series' first win): the same opening,
  and the packet rewritten four times against what the hands did (the commander looping generators into an energy
  stall, the commander walking 2,700 to a far spot, the ball chasing a lone Flash 2,000 away, lone Maces sent as
  scouts and lost); at 8:51, with the army at 3,200 against 870 seen and 17 extractors, "committing the whole block to
  attack via spot_36 toward the presumed enemy base", and the ball of 25 Maces walked through fourteen artillery
  pieces and seven towers to the commander. The pattern: expand hard, one ball, read the army lead off the score line,
  and one `fight_to` with everything.
- pianist-player-4 (2026-09-21, Opus at low effort, WON at 13 minutes): the same shape as game 3 (10 extractors by 4:13,
  15 by 5:43, the ball committed at 8:27 with the army at 3,250 against 370 seen), and the first game in which the report
  named the hands' own failures: the player read "held back by their own footwork" at 11:33 and kept the ball moving
  through named spots inside their half instead of holding at enemy_base, where a holding group steps out of turret reach.
  Its other lessons: a scouting instruction on the ball peels one soldier per ask until it names a small group for the
  job; energy needs a named builder of its own or it stalls twice; the enemy commander leaves its base to raid our spots
  once its factories are dead, and the ball must come home to hunt it, gathering first so stragglers are not eaten.
- pianist-player-5 (2026-09-21, Opus at low effort, the first game against MEDIUM, lost at 20 minutes): the same
  economy as the wins (15 extractors by 6:38, a 4:1 army lead at 8:03) and the attack went nowhere: the enemy base
  guess had been pulled to the middle of the map by a forward extractor of theirs, the ball stood on the empty guess,
  and the southern spots the player then named were never on the menu (both fixed: a guess found empty goes back to
  the start, and every spot named in the packet is a place). Medium's raids are twelve Flashes by minute 10, and they
  took the extractors from 15 to 5 while the ball hunted; its block came at 16 with Janus rocket trucks and Stumpy
  tanks behind two artillery pieces. Its lessons: on medium, turrets and a home guard on the raids' passage come before
  the hunt; labs and the commander never forward of the army; a ball never stands under unseen artillery: advance
  onto it or leave; and when the report says the base guess is at a place our units have stood on and seen nothing,
  the guess is wrong.
- pianist-player-6 (2026-09-21, Opus at low effort, WON at 29 minutes, the first win against MEDIUM): a win with the
  economy destroyed. Ten extractors at 6:00, then twelve Flashes held us at 0 to 3 for fifteen minutes; turrets on
  every spot and a ball of Maces parked at the passage both failed, because Maces cannot catch Flashes and the raids
  went round the ball. The ball gutted the enemy base from 11:01, but the enemy commander had left it to raid our half,
  and the game was won at 29:42 when it died at C3 in our half with its army still alive. Named far spots and marks
  (`mark`) were followed by the hands. Its lessons: against medium's raids, raiders of our own (Pawns) on the raids'
  passages, not turrets alone and not Maces; the enemy commander leaves its base once pressed and dies to whatever
  stands at home; a ball under unseen artillery advances onto it or leaves, never stands.
