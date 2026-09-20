# Build-order study: the shape of the tier-1 economy curve (2026-09-19)

What a good opening could reach by minute 3, 5, 8 and 10 on Quicksilver Remake 1.24 if nothing went wrong, how far our
heuristic bot and BARb medium are from it, and what the good orders have in common. The tool is `crates/buildorder`:
a crude economy simulator plus a simulated-annealing search over build orders.

**Read the optimum as a ceiling, not a target.** The simulator has no enemy. Nothing is raided, nothing dies, the
commander walks alone across half the map, every metal spot in our half is free and reachable in a nearly straight line.
A real game cannot reach these numbers; the use of the curve is to show which gaps are large enough to matter and which
direction the opening should move in.

## The tool

Rust, in the workspace (`crates/buildorder`, library + CLI, only dependency `serde_json` for reading match records,
which the workspace already has). Chosen over a Python script because the search needs about 10^6 simulations per
study run (1.6 million per optimisation: 200 000 iterations x 8 restarts; the whole grid below takes 9 minutes on 8
threads) and because openings found here are meant to be fed to the Rust bot later. Deterministic for a given `--seed`.

```
cargo build --release -p buildorder
target/release/buildorder optimize --side arm --factory lab --start nw --objective mix --minutes 10 --iterations 200000
target/release/buildorder simulate --plan my-plan.txt --wind 8          # plan text: "com: mex win lab", "fac0: ck pw", "con0: mex"
target/release/buildorder calibrate run/matches/<batch>/<NN>/record-0.jsonl... [--trace]
target/release/buildorder study --out docs/studies/data --iterations 200000 --restarts 8 --seed 1    # everything below
crates/buildorder/tools/census_curves.py --costs <a record> <match dirs>   # BARb's and our curves from the census
crates/buildorder/tools/extract_units.py upstream/Beyond-All-Reason > crates/buildorder/data/units.csv
cargo test --release -p buildorder
```

Unit numbers (cost, build time, build power, reach, speed, energy, storage, converter rates; 60 units: both commanders,
tier-1 land economy, bot lab, vehicle plant and what they build) are derived from the game's own `units/**/*.lua` at
commit `54199a0d` (2026-09-19) into `crates/buildorder/data/units.csv`; all 60 costs and speeds equal the values the
engine reported to our bot in a match record's header. Nothing in the table is invented. Two numbers the unit files do
not give are measured instead: **metal per spot 2.0/s** (every extractor income step in 12 recorded games; the home spots
18 of 18 times exactly 2.0) and **mean wind 12.8 per turbine** (inferred from energy income in the same games; the map
site says 12.7; single games averaged 8.2 to 15.5 over their first ten minutes).

### What the simulator models
- Metal and energy with storage (1000 + what buildings add), income from commander, extractors, wind (one constant
  value, or a per-second trace for replays), solars, constructors' own trickle; extractor upkeep; converters that burn
  only stored energy above 75 % of storage (`mmLevel`); overflow is lost.
- Builders: commander, constructors, factories; construction turrets add their 200 build power to the first factory;
  an `assist` step lets a mobile builder do the same for 20 s. Build time = buildtime / build power; cost is drawn
  evenly over the build; when either resource is short **every** build slows by the same factor.
- Walking: straight line x 1.05, minus build reach (+40 elmos for the target's radius), at the unit's speed, plus 1.5 s
  per build and up to 1.5 s more per walk. These four constants were fitted to 565 builder trips in the recorded games
  (time beyond the straight-line walk: median 1.2-1.5 s when the site was in reach, 2.4-3.4 s otherwise; walks over 1000
  elmos took 8 % longer than straight). Extractors go to the nearest free spot in our half of the map (22 of 44 spots,
  real positions); other buildings on a spiral around home.
- The decision space: one queue per builder (commander, up to 2 factories, up to 6 constructors). The annealer swaps,
  inserts, deletes, replaces and moves steps between builders. A constructor whose queue is empty takes the nearest free
  spot. Objectives: `income` (metal/s, mean of the last 30 s), `army` (metal cost of combat units finished), `mix`
  (army + 120 s of final income).

### What it ignores
Enemies, raids, losses, repair, reclaim and wrecks; terrain, slopes, cliffs, pathing jams, units blocking the factory
exit; wind variation inside a game (constant mean); radar, defences and any value of having turrets or scouts;
constructors helping each other or the commander on one building; turrets assisting anything but the first factory;
energy for anything but building (no weapons fire, no cloak); tier 2; metal spots differing in value; unit quality (army
is counted in metal, so the search picks whatever pushes most metal through the lab: Centurions, Hammers, Thugs).

## Calibration against our recorded games

`buildorder calibrate` feeds the simulator the opening our bot actually played (who built what, where, in which order;
not when) with the wind trace inferred from that game, and compares. 12 recorded games of batch `v15-terrain-medium`
(all north-west starts, 6 Armada, 6 Cortex). Full output: `data/calibration.md`, curves `data/calibration-*.csv`.

| | result |
|---|---|
| first factory finished | simulated minus recorded: mean -0.1 s, mean absolute 0.7 s (recorded 59-75 s) |
| metal income, minutes 1-4, all 12 games | mean absolute error 0.0 / 0.2 / 0.1 / 0.7 metal/s (0-10 %) |
| metal income, minute 5 and 6, games with at most 2 units lost so far (8 and 5 games) | 1.4 (15 %) and 1.0 (8 %), simulator high by 0.7 and 1.0 |
| army metal built, minutes 3-6, same games | 2-4 % |
| extractors, minutes 3-6, same games | 0.2-0.6 (4-25 % of a count of 2-4) |
| minutes 7-10, all games | simulator high by 3-9 metal/s (23-60 %): the bot had lost 10-17 units by then, the simulator loses none |

So: where the game is quiet the arithmetic (build times, stalls, walking, factory throughput) is within about 10 %, and
biased optimistic by roughly that much; the one early stall the games contain (a constructor's turret taking 45 s
instead of 30 s at zero metal) is reproduced. After minute 6 the comparison says nothing about the simulator, only that
losses dominate real games. **Not calibrated at all, because our bot never does it:** construction turrets, assisting,
more than ~4 constructors walking far afield, vehicle plants, solars in number, converters at scale. The fit constants
come from the same 12 games they are tested on.

Search noise: the 8 restarts of one optimisation end 3-5 % apart at 10 minutes (worst case: `army` at 8 minutes, 4267 to
7366). Differences below about 5 % between runs are not findings.

## The curve: optimum, our bot, BARb medium

Armada, bot lab, north-west start, mean wind. "Optimum" rows are single best plans from `data/optima.md`.
Our bot: median of the 12 v15 records (income, army built) and of the census of all 24 v15 games (counts, standing army).
BARb medium: median census of 42 observed games (`data/census-medium-all.csv`). The census counts what is alive, the
simulator what was ever built; for BARb, which hoards, the two are close until minute 8.
Build power = 300 + 150 x labs + 80 (85) x constructors + 200 x turrets, from the median counts.

| minute | | extractors | metal/s | energy/s | build power | constructors | army metal |
|---|---|---|---|---|---|---|---|
| 3 | optimum `mix` @10 | 7 | 15.3 | 181 | 850 | 5 | 54 |
| | optimum `army` @3 (all-in, no constructors) | 2 | 6.0 | 94 | 450 | 0 | 1074 |
| | our bot | 2 | 6.0 | 133 | 530 | 1 | 392 built |
| | BARb medium | 4 | - | - | 535 | 1 | 325 standing |
| 5 | optimum `mix` @10 | 16 | 31.6 | 332 | 1130 | 6 | 1512 |
| | optimum `army` @5 (no constructors) | 5 | 12.0 | 146 | 450 | 0 | 2224 |
| | our bot | 3-4 | 8.7 | 159 | 690 | 3 | 724 built, 750 standing |
| | BARb medium | 5 | - | - | 680 | 1-2 | 963 standing |
| 8 | optimum `mix` @10 | 21 | 43.9 | 528 | 1480 | 6 | 6328 |
| | our bot | 6.5-9 | 15.5 | 249 | 1000 | 5 | 1473 built, 1423 standing |
| | BARb medium | 7 | - | - | 820 | 2 | 1817 standing |
| 10 | optimum `mix` @10 | 22 (all) | 46.0 | 566 | 1680 | 6 | 11918 |
| | optimum `income` @10 | 22 + 11 converters | 57.0 | 839 | 1480 | - | 1832 |
| | optimum, every walk 40 % longer (re-optimised) | 22 | 46.0 | 497 | 1530 | 6 | 10168 |
| | our bot | 5-10 | 14.7 | 288 | 1390 | 6-8 | 2266 built, 1736 standing |
| | BARb medium | 7 | - | - | 820 | 3 | 2542 standing |

(BARb's income is not observable. Our extractor range is records vs census: the 12 recorded games did worse late than
the 24-game census median.)

Other starts and factions, `mix` at 10 minutes, same search: south-east 17902, Armada vehicle plant 18698, Cortex bot
lab 17700, Cortex vehicle plant 17623, against 17456 for the main line: all inside search noise. The simulator sees no
reason to prefer a faction, a factory or a corner. (The SE home given to this study, 5136,5980, has no spot within
800 elmos, yet the SE optimum is not behind: the first minute is energy and lab, not extractors.)

## What the good orders have in common

Best plans verbatim in `data/optima.md`. The main line, `mix` at 10 minutes:

```
com:  solar win lab win win, then eleven extractors in a row, walking outwards
fac0: ck ck ck ck pw ck ck, then Centurions/Rocketeers/Hammers without pause
con0-2: extractors first, then wind/solar; the first construction turret at 4:10
con3-5: energy at home, two more construction turrets (7:10, 8:30), the second lab at 5:40
```

1. **Constructors first, and many.** Every optimum with a horizon of 8 minutes or more opens the lab with 5-6
   constructors (the cap of 6 queues binds: allowed 12, the search used 10 and scored 5 % higher, which is inside the
   noise) before the first real fighter, which appears shortly before minute 4. The horizon at which a constructor pays for
   itself in army metal lies between 5 and 8 minutes: `army` at 3 and at 5 minutes builds none, `army` at 8 builds six.
   Our batch is raider, raider, constructor: on the all-in side of that line, and what it buys is 392 army metal at
   minute 3 against 54. Whether that early army is needed is an arena question, not a simulator one.
2. **The gap is expansion pace, and it opens in minutes 1-5.** 7 extractors at minute 3 and 16 at minute 5 against our
   2 and 3-4 (BARb: 4 and 5). With every walk 40 % longer the ceiling is still 6 and 13. By minute 5 the ceiling has
   3.6x our metal income; by minute 10 it has built 5x our army *after* spending the first three minutes on nothing but
   economy. In the optimum the commander itself takes 10-11 spots (ours is leashed to 900 elmos and, in recorded game 00, built four
   turrets in the first five minutes), and all 22 own-half spots are taken by minute 8-10. Our lab is up at 0:59-1:15,
   the optimum's at about 0:40 (two or three generators, then the lab, extractors after it): 20-35 s, which is
   small next to what follows.
3. **Construction turrets before a second lab.** `mix` / `army` at 10 minutes: one lab alone 15381 / 8085; two labs, no
   turrets 16164 / 10874; one lab plus turrets 17810 / 12196; both allowed 17456 / 12178. One lab with 3-4 turrets is
   as good as anything with a second lab in it, and clearly better than two bare labs. The first turret goes up at
   minute 4-4.5, when income passes roughly 25-30 metal/s; a second lab, where the search builds one, follows at
   minute 5-6 (a bare lab drains about 10 on line units), and each further
   ~10 metal/s of income wants another 200 build power at the lab. Without turrets the search parks constructors and
   the commander on `assist` instead: the build power has to come from somewhere. We build a second and third lab at
   15 metal/s and no turret; BARb builds one turret at minute 5 and stops.
4. **Energy: about 11-12 energy/s per metal/s of income, no more.** The optimum runs at 181/15, 332/32, 528/44,
   566/46. We run at 133/6, 159/9, 249/16, 288/15: 16-22 per metal/s, and the four-turbine opening overflows about
   2100 energy in the first three minutes while holding 2 extractors. The good openings place 1-3 generators before the
   lab and add energy in step with extractors. Converters are absent from every `army` and `mix` optimum (0-2 by
   minute 10); they only appear once spots run out (`income` at 8-10 minutes: 7-11 converters for +7 to +11 metal/s).
5. **On this wind map, solars are the robust choice and cost little.** The plan tuned to the mean wind (mostly
   turbines) replayed unchanged at wind 8 loses 34 % of its army (11918 -> 7918) and at wind 5 loses 58 %. The plan
   tuned to wind 8 (mostly solars) builds 11228 at wind 8, 11228 at 12.7 and 10688 at 5: a 6 % premium for immunity.
   One recorded game in twelve averaged 8.2 wind over its first ten minutes.
6. **Where the simulator agrees with what we do:** lab as the third to fifth commander build after one to three
   generators and at most two extractors (ours: four turbines, then the lab); nearest-free-spot expansion; no converters while spots are free.

## Uncertain or unfinished
- The ceiling's expansion pace rests on walking being nearly straight and safe. The 1.05 detour is measured, but on
  trips our bot chose, which are short and near home; far spots across Quicksilver's cliffs may be much worse.
- Construction turrets, `assist` and mass constructors are uncalibrated (no recorded game contains them).
- Constant wind hides the troughs that make wind openings stall; finding 5 is therefore, if anything, understated.
- The search is not converged (3-5 % spread), explores at most 6 constructor queues and 2 factories by default, and a
  second factory starts with an empty queue, which makes it harder for the search to find than turrets are. Finding 3
  could be partly that; the no-turret runs, where a second lab is the only way to add build power besides assisting,
  argue it is not the whole story.
- "Army metal" is not army strength. The arena has shown we lose fights at equal army size
  (K-barb-we-lose-the-fights-not-the-build); a 5x larger army is a different regime, but it is not evidence of winning.
- Not done: feeding an optimum to the bot and playing it; a leashed-commander or raided-outpost variant of the
  simulator; map metal values from the metal map instead of the measured 2.0.
