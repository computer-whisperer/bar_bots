# Target design: an opening found by search, with the commander as a unit that walks and fights

Written 2026-09-20 at the user's direction, before any code. The opening is composed today of hand-ordered rules
(`economy.rs` `plan_for`: H-ECO-OPENING, H-ECO-EARLY-EXPAND, H-PROD-BUILDERS-FIRST) tuned once on one map for the time of
the first lab. The user's ruling: this will be a recurring problem, some level of order search belongs inside the
heuristic engine, commander movement is a critical property of the problem, and the commander should be used for early
defence and biased forward as people play it.

## What is true today (measured 2026-09-20, gate-1-ab)
- Our opening from Quicksilver's north-west start (match 24): extractors at 0:05 and 1:16, the third started at 2:18 and
  finished at 3:13; two wind generators, lab at 0:45; after the lab the commander built three solars and a wind.
  The start has spots at 192 and 398 from the commander and then nothing nearer than 954. The second spot waited for the
  lab because only spots within 300 on foot count as "near home".
- BARb from the same start (24 games): extractors at 0:12 and 0:28, three wind generators 0:38-0:58, lab at 1:10,
  first constructor 1:40, first turret 1:34, and its THIRD extractor at 4:05 (ours: 2:18). Its commander stays within
  300 of its start all game (median 194-300 at minutes 1-12). BARb is a reference for "both near spots first", not for
  expansion speed, and not for a forward commander.
- Our commander: within 150 of its start for two minutes, 450 at minutes 3-5; it never fights by choice
  (`protect_commander` only walks it home when hurt).
- `crates/buildorder` exists: an economy simulator and a simulated-annealing search over build orders, offline, with
  Quicksilver's spots and starts compiled in, straight-line walking (optionally 40 % longer), no enemy.

## 1. The search inside the bot
- **The model** is `crates/buildorder`'s simulator, made a library the bot links: resources with storage, build power,
  build times, wind from `Hello.map`, unit numbers from `Hello.unit_defs` (cost, build speed, speed; the unit table the
  crate compiles in becomes a test fixture, not an input).
- **The map** is this game's: metal spots from `Hello`, walking distances from the terrain fields the brain already has
  (`routes.rs`). No map is compiled in.
- **Movement is a first-class cost.** A plan step is "builder B builds X at P"; its cost is B's walk from where the
  previous step left it (walking distance over speed), then the build. The commander and each constructor carry a
  position through the plan. Placement choices that save walking (generators beside the builder, the lab on the way to
  the next spot) come out of the search and are not rules.
- **What is searched:** the first N minutes (5 to start with) of the commander's queue, the first factory's queue and
  the first constructors' queues, over a small palette (extractor at spot k, wind/solar, lab, turret, constructor,
  the kit's soldiers). Anytime search from a sensible seed (today's opening), a fixed budget in the first ticks of the
  game (the opening 2 s in which the engine drops orders anyway, then a slice per tick), best plan so far wins.
- **The objective** is not the lab's time: metal income integrated over the horizon, plus army value at the horizon
  weighted by an assumed first-contact time (the opponent's first raid: minute 3-4 against BARb), with a floor on energy
  (no stall longer than a few seconds).
- **Execution.** The plan replaces H-ECO-OPENING / H-ECO-EARLY-EXPAND / H-PROD-BUILDERS-FIRST while it lasts: each builder
  takes its next step when idle. It is abandoned for the ordinary rules when reality leaves it (a builder dies, a spot
  is taken or contested in the territory grid, an enemy is at the base, the horizon is reached).
- **Later, the same machinery mid-game**: "what should the next 2 minutes of constructor time go to" is the same
  search from the current state. Not in the first version.

## 2. The commander as a unit that walks and fights
- **Forward bias.** In the opening the commander's walk is chosen by the search (it may take the far spot while the lab
  builds the first constructor). After the opening it works forward of home, not behind it: it builds at the edge of
  held ground (the territory grid), within a leash that grows with the clock and shrinks with threat.
- **Early defence.** The commander has the best gun and the most health we own for the first five minutes. When raiders
  come within its reach of something of ours and the territory grid says the fight is ours with it counted, it fights
  (`Fight` to the raiders), and its value counts in `ours` on the grid. It breaks off by health (today's 70 % retreat
  rule, plus never below a health floor far from home) and never goes where the grid says theirs.
- **The limit.** The game ends with the commander. From about minute 6, when the opponent's first real army is out
  (BARb: 8-12 soldiers at minute 6-7), it goes back to today's behaviour. The clock and the health floor are the knobs;
  both come from measurement (commander deaths by minute with and without the rule).

## 3. The commander (the LLM) drives the search (user's ruling, 2026-09-20)
- The search takes **priorities** as input, and the heuristic-only game is the search with default priorities. Both are
  kept and iterated: defaults, search inputs and commander-unit behaviour are tuned in heuristic games; the LLM is given
  the same inputs as levers.
- **Priorities** (one tool, `build_priorities`, valid before the game and at any turn): weights of the objective (metal
  income, army at a named time, static defence), constraints ("a turret at P by 2:30", "lab by 1:00", spots to take first
  or leave alone: today's `expansion` lever becomes a constraint of the search), the commander unit's role (build
  forward / hold home / fight raiders: section 2's knobs), the horizon. The answer to the tool is the plan the search
  found under them with its predicted curve (extractors, income, army by minute), so the LLM sees what its priorities
  buy before the game has paid for them, and may change them and ask again within the turn.
- **A pre-game turn.** Human games open with a countdown of several seconds (its length to be checked in the game's
  settings); the LLM gets a turn there, or at frame 0 with the game held as its turns are today, with the map, the
  start positions, the passages and the default plan, and sets priorities before the first order. Today its first turn
  comes when the first factory is up, after the opening has been decided without it.
- **Mid-game** the same tool re-plans from the current state ("the next two minutes of builder time"), once the
  mid-game search exists.

## Where the commander unit's rules come from (user's ruling)
Not from off-the-cuff rules: from games against BARb. How far forward, until when, when to leave a building site to
defend, when to pull back are fitted to measurement: commander deaths, extractor and constructor losses, and build
time lost to walking, by minute, across settings of the knobs in arena batches; the defaults are what that shows.

## Judged by
- Opening: extractors and metal income at minutes 2, 3 and 5 against today's rules on the same seeds, on at least three
  maps; first soldier's time no later than today's; no energy stall. Predicted-against-played: the plan's predicted income
  curve beside the record's, so the simulator's faults show.
- Commander: extractor and constructor losses in minutes 2-6, commander deaths before minute 8, with and without.

## Order of work
1. `buildorder` as a library over `Hello` data with walking distances; calibrate its prediction against recorded games
   on two maps (it has a `calibrate` command for Quicksilver records already).
2. The plan executor in the brain, replaying today's opening as a plan (no behaviour change: the check that the
   executor is faithful).
3. The search, the objective, the budget. 4. The commander rules. Each step is a commit with its measurement.

## Status
- 2026-09-20, step 1, library half: `Hello` and the record's header carry the simulator's numbers for every unit type
  (build time, reach, generator output, wind cap, storage, converter rates, movement class) and each spot's amount;
  `crates/buildorder` reads a game from either (`game::Game`), chooses what to offer the search by what units do and
  not by name, enforces build menus, and walks over the map's own ground (`game::Walked` on the new `crates/terrain`,
  moved out of the bot). Its compiled-in unit table, Quicksilver constants and `study` command are deleted.
- 2026-09-20, step 1, calibration half (`open-cal2-*`, 4 games each on Quicksilver and Mithril Mountain): metal income
  within 1.6 metal/s (13 %) of the played game to minute 6 on both maps after three corrections the records forced:
  3.5 s lost between builds (ours, not the engine's walk), extractors stopping at zero energy with upkeep given no
  priority, and each build held back only by the resources it costs. Builders' walks predicted within 2 s (median) at
  every length. Found on the way: today's opening energy-stalls at 4:30-5:00 in 8 of 8 games with 600-740 metal
  unspent. Beyond minute 6 a replay by order drifts (the played bot reacts to its stall, the replay cannot).
  Steps 2-4: not started.
