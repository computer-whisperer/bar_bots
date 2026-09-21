# Openings and build orders

Seeded 2026-09-19 from public sources plus the local game source. "Local source" means `upstream/Beyond-All-Reason`
(the checkout we run; changelog head "September"); unit-file paths are relative to it. Web claims are `reported` and
unchecked in the arena. Scope unless stated: 1v1, land, Armada/Cortex, default mod options.

### K-open-start-resources
**Claim.** A team starts with 1000 metal and 1000 energy (storage 1000 each); the commander adds +2 metal/s, +30 energy/s
and 300 build power. Nothing else produces until the first building finishes.
**Status.** supported (2026-09-19) — read in local source, not measured in a match
**Evidence.** `modoptions.lua:2405-2449` (startmetal/startenergy/storage defaults 1000); `units/armcom.lua`, `units/corcom.lua`
(metalmake 2, energymake 30, workertime 300).
**Would be wrong if.** The bot's first status line showed a different starting stock with default options.
**Used by.** (candidate: budget the opening against 1000 M / 1000 E instead of a fixed list)

### K-open-energy-binds-first
**Claim.** In the first minute energy, not metal, is the binding resource: an extractor costs 50 M but 500 E, a bot lab
500 M + 950 E (Cortex 470 M + 1050 E), a wind turbine 40 M + 175 E, a solar 155 M + 0 E. Two extractors plus a lab already
cost 1950 E against 1000 E stock and +30 E/s. Solar's zero energy cost is why it is the classic first generator even where
wind is better later.
**Status.** supported (2026-09-19) for the costs; the conclusion is arithmetic, not observed
**Evidence.** `units/ArmBuildings/LandEconomy/armmex.lua`, `armsolar.lua`, `armwin.lua`, `LandFactories/armlab.lua`, Cortex
equivalents. Official economy guide makes the same point qualitatively ("solars … when you have more metal than energy, or
as an emergency source if you are stalling"): https://www.beyondallreason.info/guide/in-depth-look-at-economy (undated).
**Would be wrong if.** bot.log showed energy never below ~200 during the first 90 s with a wind-only opening.
**Used by.** (candidate: opening = 2 mex, then energy chosen by wind rule, watching stored energy before placing the lab)

### K-open-two-mex-first
**Claim.** The standard opening is two extractors before anything else, then energy, then the first factory, then radar
and one or two light laser towers. Priority order in every written guide found: extractors → energy → (constructors) →
factory.
**Status.** reported (2026-09-19)
**Evidence.** https://www.beyondallreason.info/guide/how-to-start-manage-your-economy ("typically build 2 extractors
before building anything else", "one or two Light Laser Towers once you have metal, energy, production and intel"; undated
official guide). https://www.crdhq.com/articles/bar-build-order-metal-solar (2026, low-reliability aggregator).
**Would be wrong if.** A 3-mex or factory-first opening reached a higher army value at minute 5 over 24+ matches.
**Used by.** H-ECO-OPENING already does this (2 mex, 2 solar, lab); this entry is its first outside support.

### K-open-wind-threshold
**Claim.** Choose wind over solar when the map's average wind is above a threshold; sources disagree on the threshold:
official economy guide ">7", official starter guide ">10", a third-party guide "wind above 8, solar below 5, mix between".
From local costs the pure metal break-even is ~5.2 (wind 40 M per [wind speed] E/s vs solar 155 M per 20 E/s = 7.75 M per
E/s); the higher published thresholds price in wind's variance and its 175 E build cost.
**Status.** reported (2026-09-19); break-even arithmetic from local unit files
**Evidence.** https://www.beyondallreason.info/guide/in-depth-look-at-economy (>7);
https://www.beyondallreason.info/guide/how-to-start-manage-your-economy (>10);
https://www.crdhq.com/articles/bar-build-order-metal-solar (8/5). `armwin.lua` (metalcost 40, windgenerator 25 cap),
`armsolar.lua` (metalcost 155, energyupkeep -20).
**Would be wrong if.** On a map with average wind ≥ 10 a solar-only brain matched a wind brain's energy income per metal
spent at minute 8.
**Used by.** (candidate: H-ECO-ENERGY-CHOICE — read map min/max wind at start; wind if (min+max)/2 ≥ 8, else solar)

### K-open-quicksilver-is-a-wind-map
**Claim.** Quicksilver Remake 1.24 has wind 3–17 (site lists average 12.7), so wind turbines give ~2.5x the energy per
metal of solars there (12.7 E per 40 M vs 20 E per 155 M). Our opening and generator rule build solars / advanced solars only.
**Status.** supported (2026-09-19) for min/max wind (local map archive); average and the conclusion are reported/arithmetic
**Evidence.** `run/data/maps/quicksilver_remake_1.24.sd7` → `mapinfo.lua:118-119` (minWind 3.0, maxWind 17.0).
https://www.beyondallreason.info/map/quicksilver (wind 3-17, average 12.7, tidal 10; page updated 2026-06-26).
**Would be wrong if.** A wind-first brain on Quicksilver showed lower energy income at minute 5 and 10 than the solar brain
at equal metal spent on generators, or stalled more often in the low-wind troughs.
**Used by.** (candidate: replace solars in H-ECO-OPENING and H-ECO-ENERGY-BY-STORAGE with wind on this map; keep 1-2 solars
or an energy storage as a floor for wind troughs)

### K-open-build-times
**Claim.** Build time in seconds = `buildtime / total workertime` while resources last. Commander alone (300): extractor 6 s,
wind 5.3 s, solar 8.7 s, bot lab 16.7 s, radar 3.8 s, LLT 8 s. A T1 constructor bot (80-85) takes 3.6x as long. So the
2 mex + 2 solar + lab opening is ~46 s of pure build time plus walking.
**Status.** supported (2026-09-19) for the inputs; the formula is standard Spring/Recoil behaviour, not measured by us
**Evidence.** buildtime/workertime in `units/armcom.lua`, `ArmBuildings/LandEconomy/*.lua`, `ArmBots/armck.lua` (80),
`CorBots/corck.lua` (85).
**Would be wrong if.** Logged start/finish frames of commander builds disagreed by more than ~10% when not stalled.
**Used by.** (candidate: detect a stalled opening — lab not finished by ~1:30 means energy or pathing trouble)

### K-open-factory-drain
**Claim.** A bot lab (150 build power) working alone drains roughly 5 M/s on raiders and constructors and 9-10 M/s on
line units: Pawn 11 s (4.9 M/s, 82 E/s), Grunt 8.3 s (5.2 M/s, 98 E/s), Rocketeer 13.4 s (9 M/s, 75 E/s), Thug 14 s
(10 M/s, 82 E/s), Centurion 28 s (9.6 M/s, 111 E/s), constructor bot 23 s (4.8 M/s, 70 E/s). An opening economy of
2 extractors + commander cannot feed continuous line-unit production; ~100 E/s is needed per busy lab.
**Status.** supported (2026-09-19) — arithmetic on local unit files
**Evidence.** metalcost/energycost/buildtime in `units/ArmBots/*.lua`, `units/CorBots/*.lua`; lab workertime 150 in
`armlab.lua`/`corlab.lua`.
**Would be wrong if.** Measured metal usage with one un-assisted lab on repeat differed from these rates by >20%.
**Used by.** (candidate: size lab count and nano count from income: labs ≈ metal income / 9)

### K-open-constructors-early
**Claim.** Guides put constructors ahead of or right behind the first fighters (priority "extractors, energy,
constructors, factory"; "secure three additional mexes within the first five minutes"), while our arena evidence says
fighters first against BARb's early raids. The two reconcile as: one or two raiders/scouts, then a constructor, never
several constructors in a row.
**Status.** reported (2026-09-19)
**Evidence.** https://www.crdhq.com/articles/bar-new-player-guide-getting-started,
https://www.crdhq.com/articles/bar-beginner-guide-getting-started (2026, undated articles). Our K-army-fighters-before-constructors.
**Would be wrong if.** Moving the first constructor from slot 3 to slot 1 or 2 in the first batch changed neither extractor
count at minute 5 nor early losses over 24+ matches.
**Used by.** H-PROD-BATCH (already raider, raider, constructor …)

### K-open-one-factory-plus-assist
**Claim.** Early on, one factory with build assistance beats two factories: guides say "one factory with nano turrets beats
two without" and 2-3 construction turrets double or triple output. A quantitative community analysis narrows it: ~1-3
assisting builders on a T1 lab is a clear win, beyond that a second factory is more efficient.
**Status.** reported (2026-09-19)
**Evidence.** https://www.crdhq.com/articles/bar-economy-guide (2026);
https://roguel1kegaming.com/beyond-all-reason-patch-construction-turrets/ (post-turret-nerf patch analysis, undated; quotes
turret cost 250 M — local `armnanotc.lua` says 230 M, 3200 E, 200 build power, range 400).
**Simulated 2026-09-19.** K-open-sim-turrets-before-second-lab agrees (no arena test yet).
**Would be wrong if.** At equal metal spent, 2 labs out-produced 1 lab + 2 turrets in army value by minute 10.
**Used by.** (candidate: H-ECO-MORE-LABS — add up to 2 construction turrets beside the first lab before a second lab)

## From the build-order study (simulator, no enemy)

Entries below come from `../studies/build-order.md` (tool: `crates/buildorder`). The simulator reproduces our recorded
openings to about 10 % while nothing is lost and knows no enemy, terrain or losses; its optimum is a ceiling. Nothing here
has been played in the arena, hence `conjectured` throughout. Scope: Quicksilver Remake 1.24, tier 1, game commit 54199a0d.

### K-open-sim-expansion-gap
**Claim.** The largest gap between our opening and a good one is expansion pace in minutes 1-5: a no-enemy ceiling holds
7 extractors at minute 3 and 16 at minute 5 (6 and 13 if every walk is 40 % longer) where we hold 2 and 3-4 and BARb
medium 4 and 5; metal income at minute 5 is 31.6 against our 8.7. The first minute (lab at 0:40 against our 0:59-1:15)
is a small part of it.
**Status.** conjectured (2026-09-19); our and BARb's numbers are measured (12 records, census of 24 and 42 games)
**Evidence.** `../studies/build-order.md` table "The curve"; `../studies/data/optima.md`, `census-medium-*.csv`, `calibration.md`.
**Would be wrong if.** A brain that reached 6+ extractors by minute 3 and 10+ by minute 5 in the arena was no richer at
minute 8 than today's (raids eating the difference), or did not win more.
**Used by.** (candidates: H-COM-LEASH, H-PROD-BATCH constructor share, H-ECO-BASE-TURRETS timing)

### K-open-sim-constructors-first
**Claim.** For army value at minute 8 or later, the lab's first 5-6 units (up to 9-10 when allowed) should be
constructors, with the first fighter shortly before minute 4; for army value at minute 5 or earlier, none. The
break-even horizon of a constructor lies between 5 and 8 minutes.
**Status.** supported for two constructors first (2026-09-19, v21-early-expand-ab10); more than two untested. The
simulator has no raids, so its 5-6 is an upper bound, not a recommendation.
**Evidence.** `../studies/data/optima.md`: `army` at 3 and 5 minutes build 0 constructors, `army` at 8 and 10 and every
`mix` from 5 minutes on build 6 (the cap); side run with 12 allowed used 10, +5 % (inside search noise).
Arena, 10-minute A/B against medium, 4 games per arm and corner, means ours/theirs. With the fighter-first batch
and constructors building generators before extractors, v20 sat on 2.0 extractors until minute 4 with one constructor,
all metal spent as it came (north-west 0-12). Two constructors first, constructors to spots before anything else, and a
1500 commander leash for five minutes: north-west extractors at minute 4 4.0/4.5 against 2.8/5.0, army value at
minute 10 2992/2696 against 1798/3361; south-east extractors at minute 4 5.2/2.5 against 2.5/2.8. Army value at minute 4
was level between the arms: the early fighters bought nothing.
**Would be wrong if.** In the arena, batches opening constructor-heavy (say ck x4 before the second fighter) had no more
extractors at minute 5, or lost them all to raids before minute 8.
**Used by.** H-PROD-BUILDERS-FIRST, H-ECO-EARLY-EXPAND.

### K-open-sim-turrets-before-second-lab
**Claim.** One lab plus 3-4 construction turrets builds as much army by minute 10 as any plan containing a second lab,
and about 12 % more than two bare labs; one bare lab is 20-35 % behind. The first turret belongs at about minute 4-4.5,
when metal income passes 25-30/s; roughly one more per further 10 metal/s.
**Status.** conjectured (2026-09-19); agrees with the reported K-open-one-factory-plus-assist and with BARb medium's
one lab + one turret (K-barb-medium-observed-build). Turrets are uncalibrated in the simulator (no recorded game has one).
**Evidence.** `../studies/data/optima.md`, `mix` / `army` at 10 min: 1 lab 15381 / 8085; 2 labs 16164 / 10874; 1 lab +
turrets 17810 / 12196; both 17456 / 12178.
**Would be wrong if.** At equal metal income, a brain with lab + 2 turrets had not built more army by minute 10 than the
current 2-3 lab brain over 24+ matches.
**Used by.** (candidate: H-ECO-MORE-LABS)

### K-open-sim-energy-ratio
**Claim.** A well-run tier-1 economy needs about 11-12 energy/s per metal/s of income through minute 10 (181/15, 332/32,
528/44, 566/46 at minutes 3/5/8/10). We run at 16-22 and our four-turbine opening overflows about 2100 energy in the
first three minutes while holding two extractors. Converters do not belong in the first 10 minutes while spots are free.
**Status.** conjectured (2026-09-19); our ratios are measured (12 records)
**Evidence.** `../studies/build-order.md`, finding 4; `calibration.md` median table; `optima.md` (0-2 converters in every
`army`/`mix` optimum, 7-11 only in `income` at 8-10 minutes after all 22 spots are taken).
**Would be wrong if.** A brain holding energy income near 12x metal income stalled on energy (stored energy near zero
for more than ~10 % of the first 10 minutes) in the arena.
**Used by.** (candidates: H-ECO-WIND opening count, H-ECO-ENERGY-BY-STORAGE, H-ECO-CONVERT-SURPLUS)

### K-open-sim-solar-is-the-robust-generator
**Claim.** On Quicksilver (wind 3-17) an opening tuned to mean wind, mostly turbines, loses 34 % of its minute-10 army
if the game's wind averages 8 and 58 % at 5; an opening built on solars loses 6 % against it at mean wind and nothing
in poor wind. Game-average wind over the first 10 minutes ranged 8.2-15.5 in 12 recorded games (mean 12.8).
**Status.** conjectured (2026-09-19); qualifies K-open-quicksilver-is-a-wind-map, whose per-metal arithmetic stands. The
simulator's constant wind hides in-game troughs, so the effect is more likely understated than overstated.
**Evidence.** `../studies/data/optima.md`, last table, and `../studies/build-order.md` finding 5.
**Would be wrong if.** Arena batches with a solar opening built measurably less by minute 10 than the wind opening in
games whose recorded wind average was at or above the mean.
**Used by.** (candidate: H-ECO-WIND)

### K-open-energy-stall-minute-5
**Claim.** Today's opening runs out of energy at 4:30-5:00 with 600-740 metal unspent, and the extractors stop with it:
metal income falls from 20-22 to 14-16 for 20-40 s. The spending that does it is the first construction turret, light
turrets and Hammers started together (230-270 energy/s asked of 130-190 coming in, wind falling).
**Status.** measured (2026-09-20): 8 of 8 games, Quicksilver and Mithril Mountain, both factions, BARb easy.
**Evidence.** `open-cal2-quicksilver`, `open-cal2-mithril`: stored energy reaches 0 between 4:30 and 5:00 in every
record; `buildorder calibrate` shows the income dip the simulator lacked until it modelled it.
**Would be wrong if.** A batch against BARb medium (more early losses, so less to spend on) showed no stall.
**Used by.** (none yet) — the opening search's energy floor (`docs/design/2026-09-20-opening-search.md`).

### K-open-seconds-lost-between-builds
**Claim.** Our mobile builders lose 3.5-4.0 s between one build's last frame and the next one's first when the next site
is already in reach (53 trips), and 4-5 s before the game's first build. At some thirty builds in the first five
minutes that is about two minutes of builder time. The orders are given one at a time when the builder reports idle;
nothing is queued.
**Status.** measured (2026-09-20), two maps, 8 games; the cause (idle detection on a half-second tick, the site query,
the engine's own start-up of a build) is not split.
**Evidence.** `buildorder walks` on `open-cal2-*` (recorded against predicted, by length of walk).
**Would be wrong if.** Queued orders (the plan executor's) left the gap where it is: then it is the engine's and not ours.
**Used by.** `Scenario::mobile_overhead` (3.5 s) in `crates/buildorder`.

### K-open-sim-snapshot-replays
**Claim.** The economy simulator started from a played game's own state at 3:00 (standing buildings, each builder's
job with its nanoframe's health as its progress, the stock) and given what the builders started after that, matches
the record to minute 6 as well as a replay from the empty start does, or better: extractors within 0-27 % (from the
start: 27-33 %), metal income within 6-19 % (19-26 %), army metal built from there within 8 % at minute 6.
**Status.** measured (2026-09-21), the four open-cal2 records with terrain (two a map), `buildorder calibrate --from 180`.
**Evidence.** `buildorder calibrate <records> --minutes 6 --from 180`; `docs/design/2026-09-21-rolling-planner.md`.
**Would be wrong if.** A snapshot mid-raid (builders dead, jobs abandoned) drifted at once: the check has no losses
in it (0-8 units lost by minute 6). What the snapshot leaves out: builders helping another's nanoframe (their power
is not counted), tier-2 constructors and resurrectors (not the plan's), and the army (nothing of it is modelled).
**Used by.** H-OPEN-SEARCH, H-OPEN-PLAN (the rolling planner re-plans from such a snapshot).

### K-open-five-minute-horizon-undervalues-extractors
**Claim.** With the opening search's horizon at five minutes and 90 s of terminal income, a start whose plan
predicts six extractors at 5:00 for one more Pawn party (4448, 1544 on Quicksilver's north box: 6 extractors, 1,960
army metal, score 10,141) outscores one with eleven (4000, 2152: 11 extractors, 1,428 army metal, 9,993); at ten
minutes the ranking flips (25,934 for the latter, 23,252 for the former; 16 against 12 extractors predicted at 5:00,
24 against 22 at 10:00). Commander games 6 and 7 started at the former and had the worst economies of the series
(five and six extractors at 6:00 against nine); game 5 started at the latter and won.
**Status.** measured (2026-09-21): the arena's `--place` candidate scores at both horizons on the same record.
**Evidence.** `docs/briefs/commander.md` cmd-opus-low-7; `planner-smoke-1`'s arena output (candidate lines).
**Would be wrong if.** A batch from the ten-minute choice held fewer extractors at 5:00 than one from the five-minute
choice; or the Pawn-heavy opening from the former start won more (the opening series found it standing outmatched).
**Used by.** H-OPEN-SEARCH (horizon 600 s), the arena's placement (`crates/arena/src/place.rs`).

### K-plan-ten-minute-horizon-trades-early-army
**Claim.** The rolling planner with the `Tempo` objective ten minutes ahead (metal made + 90 s of terminal income
+ army metal x 1 + the 2:30 contact term) spends the first five minutes on the economy: 5 constructors and 8
soldiers alive at 5:00 against the five-minute one-shot opening's 2 and 15, army metal built 720 against 1,327.
It holds 10 extractors at 5:00 and 9 at 10:00 against 8 and 5, income 20.5 and 19.3 against 17.9 and 13.3, and
loses more games (2-19-3 against 5-15-4): the raids of minutes 5-10 take the larger economy from the smaller army.
The simulator prices every extractor at full worth to the horizon and knows no enemy, so the plan's economy is
overbuilt for what stands against it.
**Status.** measured (2026-09-21), 24 games an arm, Quicksilver north box, BARb medium, mirrored.
**Evidence.** `planner-base`, `planner-1` (ledger); `docs/design/2026-09-21-rolling-planner.md` step 1.
**Would be wrong if.** The army share were the objective's and not the horizon's: an army weight or a contact term
that keeps 15 soldiers by 5:00 at the ten-minute horizon and holds the extractors too would show it. The
commander's requirements (step 3) are the intended lever for the share.
**Used by.** H-OPEN-SEARCH.

### K-plan-three-minute-expectations
**Claim.** The rolling planner three minutes ahead with `Objective::Expect` (what stands at the horizon paid against
the clock's expectations: extractors 2 / 4 / 6 / 8 / 10 at minutes 1-5, constructors 1-5 from 1:30 to 10:00, army
metal 200 / 650 / 1,400 / 3,200 at 2 / 3 / 5 / 10 min; a constructor beyond the expectation costs its metal) holds
the economy the ten-minute form gained (9 / 9.5 / 9 extractors at 5 / 10 / 15 min against the old opening's
8 / 5 / 5, income 18 / 18 / 21 against 18 / 13 / 13) and builds the army from it (army metal 4,014 / 7,297 at
10 / 15 min against 3,346 / 5,958; 25.5 / 39 soldiers against 23.5 / 31), for 9-13-2 against 5-15-4 (the ten-minute
form: 2-19-3). The first five minutes are still army-poor: 870 army metal and 9.5 soldiers at 5:00 against the
expectation's 1,400 and the old opening's 15.
**Status.** measured (2026-09-21), 24 games an arm on the same setup as planner-base; 24 games is within noise of
the base's wins (docs/README: 12 games ±14 points), the economy and army curves are not.
**Evidence.** `planner-2` against `planner-base` and `planner-1`; `docs/design/2026-09-21-rolling-planner.md`.
**Would be wrong if.** A 48-game rerun lost the win margin, or the early-army shortfall came from the executor
(labs idle) rather than the objective (the plan's factory queues). Open: the expectation curves are mine, by eye
from the players' and the searched openings; the commander's requirements (step 3) are meant to set them.
**Used by.** H-OPEN-SEARCH.

### K-open-search-beats-rules
**Claim.** An opening found by half a second of search over a simulator of this game's economy beats our hand-ordered
opening on every opening measure at once, on the same seeds: extractors 5.2 / 8.2 / 11.4 at minutes 3 / 4 / 5 against
4.6 / 6.8 / 9.1, metal income 20.2 against 16.0 at minute 5, army metal built by minute 5 1116 against 922, six
constructors against four, and 1.6 s against 15 s with stored energy at zero. What it does differently: four
constructors before the first soldier, generators interleaved with the commander's extractors so the stall at 4:30
never comes, and solars where the rules took wind.
**Status.** measured (2026-09-20), Quicksilver, 8 games an arm, against the same opening played through the same executor.
The simulator's prediction holds to minute 3 (extractors 5.2 predicted, 5.2 played; income 12.5 against 11.5) and is
optimistic at minute 5 (14 extractors and 30 metal/s predicted, 11.4 and 20 played): it knows no enemy and no losses.
**Evidence.** `open-search-1-ab`; `run/opening_ab.py <batch dir>`.
**Would be wrong if.** Full games lost the gain. **They did** (open-search-2-ab, 2026-09-20: 3-7-2 against 7-5-0 on
Quicksilver, 1-7 against 2-5-1 on Mithril): with no turret to minute 6 the extractors it takes are raided away from
minute 6 on. The claim stands for the opening measures only; an opening objective has to price exposure
(`Objective::Tempo`'s `exposed`, open-search-3-ab).
**Used by.** H-OPEN-SEARCH.


### K-open-early-pawn-pressure-is-standard
**Claim.** Early raider pressure (a lab by about 0:50, then a handful of Pawns or Grunts before anything else, arriving
at the opponent's base before 2:30) is standard 1v1 play on maps like Quicksilver, in basically every game; 1v1 games
rarely reach 20 minutes. Against BARb medium the handful is enough to win outright: it has two LLTs and no soldier at
2:30 and offers the kill. Long macro games, the kind our bot is meant to be capable of, come from 2v2 or bigger on
larger maps, where the game stabilises long enough.
**Status.** stated by the user 2026-09-20 from watching an experienced player's replay and his own knowledge of the
game; the replay's record agrees (below). Not measured over many games.
**Evidence.** `run/matches/*-replay-player2-vs-medium` (run/replay_match.py on the player's demo): lab by 0:50, 5
Pawns at minute 2 and 12 at minute 3 with metal income held at 6, BARb's first extractor dead at 2:28, its lab at
3:30, its commander at 4:45; the player lost 15 Pawns; BARb built no soldier. Pawns finished per game minute 1-4: 5 / 7 / 6 / 8 (26 by minute 5) beside 2 constructors and 7 extractors; neither player lost an extractor. The second replay (Ben vs medium, `*-replay-ben-vs-medium`) is the
same shape with expansion beside it: 3 / 7 / 10 / 15 Pawns at minutes 2-5 while extractors go 2 to 9 by minute 6
(income 6 to 16); BARb, raided from 2:25, never left 2 extractors and income 6, had 2-5 Pawns and 3 LLTs, and its
commander died at 6:08; Ben lost 22 Pawns and one resurrection bot. Neither player's commander left its base.
**Update 2026-09-20 night (matt-plan, matt-search).** Matt's order transcribed and played by the bot gives his Pawn
count (21 by 5:00) and the same 3-9 as the searched opening: BARb in our arena games has 1-4 soldiers and 1-3 towers at
3:00 and 5-9 soldiers with 3 towers at 4:00 in ten games of twelve (in Matt's replay it had no soldier and two towers,
the variant two of our twelve met), and our pressure rule waits where Matt went in with twelve and paid seven for the
lab and every tower. "The handful is enough to win outright" holds for the soft variant; against the usual one the
question is the party's commitment, not its size.
**Would be wrong if.** Experienced players opened with constructors and a turret against each other on this map and
raided only from minute 4; or the same rush lost to BARb medium in half of a batch.
**Used by.** (candidates) the opening search's army term: raiders early, weighted by a first-contact time near 2:30
rather than metal alone (the design's leftover); H-ARMY-CONTACT: the first parties to answer are 5-12 Pawns at 2:30
with two turrets and no army of ours, which is a turret and commander question; the commander-unit rules of the
opening design, step 4 (BARb's commander does not fight either).
