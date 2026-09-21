# Target design: a rolling-horizon economy planner, with the commander's requirements as its objective

Written 2026-09-21 at the user's direction, after commander game 7: "the next lever is more general economy and
build order control. The current set of heuristics is not going to scale for more complex games, and opus needs to be
able to dictate additional requests and requirements as it runs." The user proposed iterating the build-order search
into "a central system on a rolling horizon dictating economy construction", and asked whether a MAX-SAT solver would
serve better. Decided: extend the annealer; not MAX-SAT (the problem is continuous resource flow with storage, shared
build power, stall factors and walking, and the calibrated simulator is the asset; a SAT encoding needs a time grid and
pseudo-Boolean flow encodings and loses the calibration; hard constraints and a declarative requirement language come
as penalty terms in the annealer's objective for free).

## What is true today
- `crates/buildorder` is an economy simulator (`sim.rs`: fixed 0.5 s step, per-builder queues, resources with
  storage, build power sharing, stalls, converters, walking over the map's own ground; calibrated to 13 % of a
  played game's metal income to minute 6, `docs/design/2026-09-20-opening-search.md`) and a simulated-annealing
  search over `Plan`s (`anneal.rs`: one queue for the commander, one per factory, one per constructor, indexed in the
  order builders appear; moves swap, insert, remove, replace, move across queues, and name an extractor's spot;
  warm start from `Search::start`; anytime `anneal_within`).
- The simulator only starts from an empty game: the commander at home, 1000 of each resource, no buildings.
- One search per game (`crates/bot/src/brain/opening.rs`, H-OPEN-SEARCH): 500 ms on 2 threads **on the tick thread**
  at the first order, horizon 5 minutes, objective `Tempo` (metal made + 90 s of the income at the horizon + army
  metal + the first-contact term at 2:30 - stalls). The plan is played by H-OPEN-PLAN until 5:00, an armed enemy
  within 1000 of home, or the commander's death; then every builder goes to the rule ladder in `economy.rs`
  (`plan_for`: opening, early expand, spend, T2 gate, energy, converters, then a focus-ordered list of repair,
  reclaim, radar, nano, turrets, labs, outpost turret, expand, convert, with the commander's caps and floors as
  directives: `min_constructors`, `min/max_converters`, `expansion_radius`, `base_turrets`, `outpost_turrets`,
  `economy_focus`).
- Cost of one simulated plan on game 7's record (measured 2026-09-21): 0.21 ms at a 5-minute horizon, 0.27 ms at
  10, 0.37 ms at 20. The in-game budget buys about 5,000 evaluations; the offline experiments that found good plans
  used 40,000 x 8 restarts (about 10 s of one core). Speed is not the obstacle.
- The arena's `--place` (`crates/arena/src/place.rs`) runs the same search from six candidate starts in the box
  (4 s, 4 threads, horizon 300) and spawns us where the score is highest. Game 7's finding: at (4448, 1544) the
  search predicts 6 extractors at 5:00 for 1,960 army metal (score 10,141) against 11 extractors for 1,428 at
  (4000, 2152) (score 9,993); games 6 and 7 started at the former and had the worst economies of the series. A
  5-minute horizon with 90 s of terminal income undervalues the economy.
- The plan executor keys builders by order of appearance (as the simulator recruits them), which only works from an
  empty start.
- The bot cannot tell whether it runs in lockstep (the arena) or against a live engine; `Tick::late` is all it sees.

## The change
1. **Snapshot simulation.** `sim::State`: the game as it stands at `t0`: metal, energy and both storages; standing
   modelled units (extractors with what they pay, generators, converters, storage, turrets, nanos); builders
   (commander, constructors, factories) with their positions and their current job (unit, site, progress, what it
   pays) if any; spots already taken. `simulate` takes a state; the empty start is `State::start(scenario)`. Time is
   absolute (`Sample::t` from `t0`; the wind trace, the contact term and the horizon read absolute time). A
   nanoframe's progress is its health over its maximum (the engine builds health with progress).
   Check: `buildorder calibrate --from SECONDS` starts a record's replay from the record's own state at that second
   with the steps the record shows after it, and must match the record's curves to minute 6 as well as a replay
   from 0:00 does (within the 13 %).
2. **Queues keyed by builder.** The state lists its builders in a fixed order (commander, factories, constructors,
   each by unit id); a plan's queues follow that order, then the queues of factories and constructors not yet
   built, in the order they finish. The executor maps unit ids to queues from the snapshot, not from appearance.
3. **The planner on its own thread** (`brain/planner.rs`, replacing `opening.rs`): the brain takes a snapshot,
   sends it with the scenario, the palette, the objective and the current plan (re-based on the snapshot: the steps
   still ahead) as the warm start, and polls a channel each tick. When the plan arrives it is re-based again on
   what each builder has started since the snapshot (leading steps whose type matches builds begun since then are
   skipped), then played. Budget 400 ms of wall on 2 threads; at speed 50 in the arena that is 20 game-seconds of
   staleness, the same order as the interval, and the re-basing absorbs it.
4. **Rolling horizon.** Horizon 10 minutes from the snapshot. Re-plan every 30 game-seconds, and at once when a
   builder or factory of ours is finished or lost, or a directive changes. The plan no longer ends at 5:00. It is
   suspended while an armed enemy is within 1000 of home (the rules run, as today) and re-planned when they leave.
   It ends only with the commander.
5. **Placement** uses the 10-minute horizon; nothing else changes there. The two starts of game 7's finding are
   re-scored.
6. **Palette and objective** stay as they are in step 1 (extractor, generators, factory, constructor, nano, the kit's
   turret at a spot, assist; `Tempo` with the contact term). What the rule ladder builds that the palette lacks
   (radar, converters, more labs, T2) is step 2's.

Then, in later steps of the arc (each a design status line and a measured commit):
- **Step 2: the planner replaces the ladder.** The palette gains radar, converters, further labs, the T2 lab and
  its upgrades; the objective gains what the directives say (each cap or floor a term); the mid-game rules in
  `plan_for` are deleted down to the reactions (repair, reclaim, the energy-short rule, turret requests). The
  threat grid prices a spot's expected life in place of the static exposed rule.
- **Step 3: the commander's requirements.** A `requirements` tool: a list of (what, how many, where, by when,
  hard or weighted), each a term of the objective; and a what-if query that runs the planner from the live state
  with a candidate requirement and returns the predicted plan and its cost, so the commander can ask before it
  commits.

## Judged by
- Step 1: the `--from` calibration on the open-cal2 records (error to minute 6 from 3:00 no worse than from 0:00);
  the placement's choice on Quicksilver's north box and its predicted extractors at 5:00; a 24-game heuristic batch
  (medium, mirror, `--place`, `--corner nw`) against the previous commit on extractors and income at 5:00 and
  10:00, and the ledger. The plan now covers ten minutes, so this batch is the first measure of the planner against
  the ladder in minutes 5 to 10.
- Step 2: the same batch, extractors lost and rebuilt by minute 10.
- Step 3: commander games, the usual way.

## Status
- 2026-09-21: written; nothing built.
- 2026-09-21, step 1 built: `sim::State` (the empty start a special case; time absolute), the search and the placement
  over it, `buildorder calibrate --from S` (K-open-sim-snapshot-replays: from 3:00 as good as from the start, or
  better); `brain/planner.rs` in place of `opening.rs` (snapshot from the tick, the search on its own thread, the
  first blocking; re-plan every 30 s and, at least 10 s apart, when a lab or constructor comes or goes; queues by
  unit id; the plan re-based on what began while it was searched; labs given two units at a time; the plan ends only
  with the commander; spots by an armed enemy in sight left out); placement horizon 600 s
  (K-open-five-minute-horizon-undervalues-extractors: the ranking of the two starts flips). Smoke game
  (planner-smoke-1): re-plans arrive in 400-800 ms, the economy to minute 6 as game 5's, lost at 10:00 to the raid
  shape; the first re-plan's warm start scored a quarter of the found plan: the snapshot had missed the
  commander's lab nanoframe as its job (smoke-2); a builder's job is now the nanoframe the engine says it made
  (smoke-3: warm starts within 10-30 % everywhere). A/B planner-base (the previous commit's bot) against planner-1, 24 games each: the economy
  measure met (extractors 10 / 9 at 5:00 / 10:00 against 8 / 5, income 20.5 / 19.3 against 17.9 / 13.3) at the
  cost of the early army (720 against 1,327 army metal by 5:00, 8 against 15 soldiers), 2-19-3 against 5-15-4
  (K-plan-ten-minute-horizon-trades-early-army). The user: "10 minutes sounds much too long -- try it with 3 minutes and use the utility
  function to incentivise expected extractor numbers etc."
- 2026-09-21, horizon 180 s with `Objective::Expect` (expectation curves by the clock for extractors, constructors
  and army metal, `Expectations::STANDARD`; each unit up to the expectation worth a fixed metal value, beyond it
  less, a constructor beyond it its cost; the bot and the placement alike). planner-smoke-4: to the 40-minute cap,
  but constructors without limit (fixed). planner-2 against planner-base: 9-13-2 against 5-15-4, extractors
  9 / 9.5 at 5:00 / 10:00 against 8 / 5, army metal 4,014 against 3,346 at 10:00, still 870 against 1,327 at 5:00
  (K-plan-three-minute-expectations). The curves are the first thing step 3's requirements should set.
