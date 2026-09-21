
# What we know (field brief, revised 2026-09-20 night)

This is the project's accumulated knowledge, from some hundred recorded games against this opponent, unit duels and the
opponent's own configuration. Each line names its source entry in `docs/knowledge/`. It is evidence, not orders: say in a
`note` when the game in front of you contradicts it.

**Units (tier-1 bots; metal / health / speed / range).** Armada: Tick `armflea` 21/60/132/140 scout; Pawn `armpw`
54/370/87/180 raider; Rocketeer `armrock` 120/720/51/475; Mace `armham` 130/1000/46/380 line unit; Centurion `armwar`
270/1590/45/325 brawler; Crossbow `armjeth` anti-air ONLY, it cannot hit ground units; Lazarus `armrectr` 130, repairs,
reclaims and resurrects, cannot fight. Cortex: Grunt `corak` 43/280/81/215 raider; Aggravator `corstorm` 110/740/48/475;
Thug `corthud` 140/1100/45/380 line unit; Trasher `corcrash` anti-air only; Graverobber `cornecro` resurrects.
[K-units-t1-bot-roster, K-units-anti-air-is-air-only]
- At equal metal, head on: Mace and Centurion (Cortex: Thug) win most tier-1 matchups, including against everything this
  opponent fields and against light turrets. Pawn beats only Grunt and loses badly to Thug. Rocketeer loses to every
  mobile unit it cannot outrange, and is for turrets and buildings: it outranges the light laser tower (475 against
  430). Artillery does not beat turrets without something spotting for it. [K-units-duel-line-bots-beat-raiders,
  K-units-duel-range-vs-turrets]
- Raiders are for what cannot shoot back: extractors, constructors, lone turrets under construction. A Pawn or two
  standing on an approach also stops a lone enemy raider; chasing raiders with the army does not. [K-units-dont-chase-raiders]
- Fights are decided by value on the spot. When a fight is decisive, the side with more metal of soldiers there, a
  turret counting about three times its metal, loses less nine times in ten; beyond 2:1 almost always. So: never walk
  into a turret line at parity, and arrive together. Our units move at different speeds and a squad sent across the
  map arrives strung out, fast ones first, and dies in pieces: gather at a point short of the target, then go.
  [K-army-combat-prediction, K-army-waves-die-to-static-defence, K-army-piecemeal-midmap]

**This opponent (BARb, medium).** It has no resource or vision cheats. [K-barb-no-resource-cheats]
- It raids from the first minutes: raiders gather at its base and leave as a squad of about four, then every new raider
  joins the raid at once. They go for buildings where the threat is low, builders and extractors first, outermost
  first, and they avoid defended ones: a raid squad skips a target whose defence is worth more than about three
  quarters of the squad. One turret and two soldiers on an extractor is usually enough to be skipped.
  [K-barb-raid-timing, K-barb-raid-threat-gate]
- It builds a turret beside nearly every extractor, and forward turret nests in our half later on.
  [K-barb-medium-observed-build, K-army-verdicts-v18]
- Its army, from its true positions in 24 recorded games from our north-west start (mean metal value; games vary by
  about a third either way, and it has less when we have been killing its raids and extractors): minute 4 500, minute 6
  1250, minute 8 2000, minute 10 2800, minute 12 3800, minute 15 4700, minute 20 5400. [K-barb-medium-army-curve]
- It hoards that army at home as one block (20-35 soldiers by minute 10, 40-80 by minute 18) and sends it when it judges
  itself stronger than what it has seen of ours; the block then walks to our nearest weakly defended cluster. After a
  wave of ours dies in its half, it resurrects the wrecks and they fight for it. [K-barb-attack-gate,
  K-army-dead-waves-are-resurrected]
- It counter-builds against what it has seen: riot units against raiders, assault units against static defence.
  [K-barb-response]
- Its second factory is a tier-2 lab, built when its income covers it or by minute 15-20 at the latest; after that its
  tier-1 lab makes only constructors. We have no tier-2 play yet, so the game should be decided, or well in hand, by
  then. [K-barb-tier2, K-eco-t1-ceiling]
- Its commander builds at home and is the win condition. Its base has turrets on a clock (minutes 2, 4, ...).
  [K-barb-base-defence-clock]

**Economy.** [K-eco-production-is-the-bottleneck, K-eco-expansion-before-conversion, K-open-sim-energy-ratio]
- Metal in the bank is army we do not have. The bot now adds construction turrets and labs when more than 500 is
  banked; if the bank still grows, raise production, not converters.
- Extractors beat converters while free spots remain. Energy only needs to keep storage from emptying; laser turrets
  stop firing in an energy stall. [K-units-laser-towers-need-energy]
- Constructors die in the field: 16-31 a game in our losses, mostly walking alone to far spots. Expand in steps the
  army has already covered, not to the far side of the map at once. [K-army-verdicts-v18]

**This map, from the north start (the lobby's boxes: we start in a strip across the north, the opponent across the
south).** [K-maps-terrain-not-straight-lines, the terrain picture the bot prints at the start]
- Our commander stands at about (3810, 2081), E3, on a high plateau ringed by cliffs (the `O` block at E2-G3 in the
  picture). The plateau's ways down are west, onto the low ground at D3-E3, and south-east through the middle-height
  slope at F4-G5; behind the base (north) is sea. Anything walking to the base comes up one of those two ways.
- The opponent starts on the mirror plateau in the south-west, B6-C7 (its buildings have been seen at (3320-3660,
  5300-5800), D6/D7): about 3,950 on foot from our start, 3,750 in a straight line, over the open middle (C4-F6).
- 38 of the 44 spots can be walked to; six on islets cannot (the `map` tool's `walk_from_home` is null for them).
  The nearest to us are on and just below the plateau (F2, E3, F3, G4); the middle rows (C4-F5) are the contested
  ones and change hands all game. The `map` tool gives every spot's walking distance from home.
- The bot prices every walk over this ground (slopes and cliffs, each unit class its own): when it says a spot is
  near it means on foot. The commander's leash is 24 seconds of its own walking.

**What the bot does for you now (2026-09-20).** Every soldier handles its own footwork under a control lane that
runs ten times a second: it steps out of the reach of a turret or the enemy commander it was not sent against, holds
at the edge until you or the bot order otherwise, leaves a unit fight only when wounded and losing where it stands,
and a group shoots one target at a time. Do not micro units with orders every few seconds: give a squad its post or
its fight and judge it by the `traded` line minutes later. The early Pawn pressure is priced the same way and backs
off the commander and turrets on its own; what it cannot do is crack a turret line with Pawns, and it will not try.

**What earlier commanders did, right and wrong.** [experiments ledger: commander-1 to -8, cmd-opus-1, cmd-sonnet-1]
- cmd-opus-1 (2026-09-20, eight Opus games on this start, 1-7): every game switched `pressure` off at about 4:40
  when the kill window closed on laser towers ("Pawns cannot crack turrets", true), then massed Maces and Rocketeers
  in defensive squads and played `defend` for the rest: by minute 15 it held 3.6 extractors against the bot's own 5.2
  playing alone, with less army, and the opponent's block (eleven Stumpies and four Janus, 4,200 metal, at 22
  minutes) met "4 soldiers and 7 light turrets". The one win (game 06) kept scouting (`scout_at` nine times), kept
  attacking (`attack` 21 times) and read the traded line: "fist is inside their base, their army really is all in our
  half". Turning the pressure off is not wrong; sitting on four extractors afterwards is the loss.
- cmd-opus-low-1 (2026-09-20, Opus at low effort, lost at 16 minutes): `expansion_radius` 2,200 at 0:30 and 2,600
  at 4:06 with `economy_focus` expand sent constructors into the middle rows (D4-E5, 2,000-2,600 on foot) while the
  army stood at its station and the Pawn party waited far south; four-Pawn raids through D4/E5 took five extractors
  in three minutes from 5:00 and it never got past three again; the base fell at 14:00. Its own lesson at 14:10:
  "needed line units on the D4/E5 approach from minute 4 and turrets at every forward mex". On this start the safe
  spots are the plateau and its foot (F2, E2, E3, G3, F4, about 1,500 on foot); anything in the middle rows needs a
  squad posted on the D4/E5 approach and a turret first, and the radius set only as far as that squad covers.
- The bot alone against this opponent on this start (the rush series, 2026-09-20): the opening is at the players'
  level (lab at 25 s, first Pawn at 65-70 s, seven extractors by 3:00), the Pawns take two or three of its
  buildings by minute 10 and lose about as much as they kill; the game is decided by what comes behind them.
- Game 8, low effort, thrown at minute 32: banked 10,000 metal of army on 4 extractors, marched on the base with no
  rockets against 15 turrets, arrived with the Centurions 350 ahead of the Hammers and lost 3,000 in 17 seconds while
  killing its tier-2 lab; noted "pressing through", and eleven seconds later ordered everyone home with `move`. At
  that moment the opponent had 17-19 soldiers left, no lab of that tier and its construction turrets dying; our 19
  Hammers walked out through its Pyros without firing and none got home. The retreat was judged on our losses alone.
- Game 7, the lost one: metal traded ran 3.3 to 1 against us from minute 6 (0.8 to 1 in the game won beside it) and
  nobody looked. Extractors stood up to 4,400 from home with turrets only at the base; the `take_first` list sent
  constructors back to the same raided spots, one of them four times. It called a "decisive win" 23 seconds after a
  fight order, with both armies intact on either side of a ridge, and gave up a position because a squad 15 seconds
  old "sat stuck for 90+ seconds": read the clock in the report before judging what has or has not happened.
- Game 6, won in 23 minutes: held the mouth of the peninsula and the near spots, judged from the usual curve and from
  what kept dying at its posts that the opponent's army was below par, scouted its base, then sent the whole main body
  (59 soldiers) at the commander's last sighting while one squad held home. That is the pattern.
- Game 6, the other one, lost: three squads ordered to one rally point arrived one after another and fought alone,
  while the base they had left was raided. Squads of different units walk at different speeds: bring them together
  at a point well short of the enemy and check they are all there before the next order.
- Game 5: by minute 33 our army was worth 19,400 metal and theirs 2,800, and the game was not ended. The commander
  believed the lead was 1.4 to 1 (the report's count of enemy soldiers never forgot the ones that died out of sight;
  fixed), kept 100 soldiers in garrisons on quiet ground, and sent strikes of 8-20 that ground themselves down on
  turrets. One of them walked to the game's guess of the enemy start and found nothing there. When you are ahead, the
  whole army goes, together, to where its factories and commander were actually seen.
- Game 4 also: the bank was spent (three labs by minute 7), but 22 extractors died to raids over the game, mostly to
  Grunts in groups of 3-5; we held 4-6 against 23 from minute 6 on with the army level throughout. Ground taken and
  not held costs the extractor, the constructor's time and the turret beside it.
- Game 4: read "army worth 983 metal seen" as the opponent's army, concluded "we have a 2.3x advantage" and later "well
  ahead (3212 vs unseen enemy)", when the opponent's army was 3032 and 3872. What is in sight is a fragment. The
  report's wording was at fault and has been changed; the habit of reading absence as weakness is yours to watch.
- Game 2: every squad posted on our own lab yard, expansion radius 900, focus "defence": 2-5 extractors against 20 for
  half an hour, and a loss.
- Game 3: expanded well (13 extractors against 6 at minute 8), then lost 8 of them to raids with the expansion radius
  at 6500 and the squads in two places: more ground than the army could answer for.
