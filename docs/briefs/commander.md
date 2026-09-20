
# What we know (field brief, revised 2026-09-19)

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

**This map, from the north-west start.** [K-maps-terrain-not-straight-lines, K-maps-quicksilver-corner-asymmetry,
`docs/studies/perception.md`]
- Our start is the head of a peninsula (x 1500-2500, z under 1200) with two metal spots. The head joins the rest through
  a corridor only about 250 wide at (1650-1900, 1220-1350); below it is a lobe (x 1500-2000, z 1400-1800), which opens
  onto the mainland between z 1900 and 2050, with the west coast at x 1500 and cliffs at x 2300. Nothing that walks
  reaches the base except through there. The place to stand is at or beyond that mouth, around (1900, 2150), not in
  the lobe or the corridor. Later in the game the opponent may build amphibious bots (`coramph`) that come out of the
  water anywhere: watch the fights list for them.
- Beyond the neck the near spots are at C3/C4 (2136,2136), (2312,2344), (2296,2936), B4 (1352,2808), D3/D4. The line of
  spots far to the east (x above 4000 in rows 2-3) has been bought and swept in every game we recorded.
- Six spots cannot be reached on foot at all (`walk_from_home` null).
- Keep the army out of the base: 40 soldiers among the buildings on the peninsula jam, and everything they do starts
  with a walk through the neck.

**What earlier commanders did, right and wrong.** [experiments ledger: commander-1 to -6]
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
