# Target design: the micro lane — a 10 Hz tick and a unit control engine

Written 2026-09-20 at the user's direction, before any code. The user's diagnosis of why we lose: micro ("BARb is
excellent at micro because it's the easiest thing for a heuristic bot to hyper-optimize"; our move to the players'
early pressure brought us into early fights where our weaker micro hurts), economy after the opening (untrained
heuristics; build power never optimised), and units thrashed around the map without regard to terrain (the cliffs
behind the Quicksilver base; the commander worst of all, slow and most of our build power). Ruling: micro first,
because it is the one that measures cleanly and the one closest to par with BARb; the terrain fix is small and rides
along; economy is third. The SC2 scene's shape is in `docs/knowledge/_inbox/sc2-micro-engines.md` (researched, not
reviewed).

## What is true today (rush-26 records, 2026-09-20; code as of 1cddf0e)
- One tick every 15 frames (`TICK_INTERVAL` in `crates/ai-shim/src/lib.rs`, the only source; `economy.rs` repeats it
  as `TICK_FRAMES`). The whole brain runs every tick. `Brain::decide` takes 1 ms at the median, 4-5 ms at the 90th
  percentile, 15-26 ms at the 99th and 760 ms once (the opening search at the start).
- The arena runs the shim in lockstep (`WITHIN_REASON_LOCKSTEP`): the engine waits for every answer, so bot time is
  game time stood still and no tick is ever skipped. Outside the arena (a human's game) the shim sends a tick only
  when it holds credit, so a slow answer drops ticks rather than the frame rate.
- A Pawn walks 87 elmos a second: 43 between ticks now, 9 at 10 Hz. A light laser tower reaches 430 and deals 160
  damage a second; the commander reaches 300 at about 390 a second; a Pawn has 370 health. Between two of today's
  ticks a Pawn can walk from outside a tower's range to 40 elmos inside it, or take half its health from the commander.
- Orders are given per party (`raid.rs`, `contact.rs`, `army.rs`): Move, Fight, or Attack on a priced-in turret with
  Fight queued; re-issued every 4 s or on a mode change. Target choice within a fight is the engine's own (Fight
  orders auto-target). Nothing is decided per unit; nothing knows a unit's reload or health in a fight.
- The snapshot has each unit's position and health, nothing of its weapon state; the enemy's units in sight or on
  radar with position and health. The unit table (`crates/combatsim/data/units.json`, mapped by `contacts.sim_defs`)
  has every type's range, reload, damage, speed and sight.
- No threat map. Turrets bearing on a target or an approach are found by segment geometry at each pricing
  (`turrets_bearing`); nothing represents where it is dangerous to stand.
- Walking distances exist (`crates/terrain`, `routes.rs`: fields from home and from the enemy bases, 16-elmo cells)
  and are used in 8 places; straight lines (`dist2d`) in 179. Nearest-spot choices, the commander's leash, scouts'
  routes and responders' arrival order are all straight-line.
- The account by minute 10 of a rush-26 game: 19.7 Pawns built, 17.8 lost, 3.0 buildings of BARb's killed. The four
  raid bugs found this afternoon with the debug line were all fights conducted wrongly at the unit level: the dive at
  the commander after sighting it, the fight under a turret with no flee, the leader waiting for joiners under fire.

## 1. The tick
- The shim sends a tick every **3 frames** (10 Hz). `WITHIN_REASON_TICK_FRAMES` overrides it (3, 5 or 15; the
  brain's own interval must be a multiple) for A/B runs and for measuring the lockstep cost. `Hello` carries
  `tick_frames`; nothing in the bot assumes 15 any more (`economy.rs` `TICK_FRAMES` goes; the dropped-order window
  uses the brain's interval).
- The bot runs two passes on one thread. **`think`** is today's whole brain, on frames that are multiples of
  `BRAIN_FRAMES = 15` (2 Hz, unchanged), fed every event since it last ran (events from the ticks in between are
  kept, in order). **`micro`** is the control lane, every tick, over the snapshot alone.
- The snapshot grows by what the lane needs and nothing else: per own unit the frame its first ground weapon can next
  fire (`Unit_getWeapon`, `Unit_Weapon_getReloadFrame`; 0 for the unarmed) and its velocity; per enemy in sight its
  velocity (`Unit_getVel`). About three callback calls more a unit.
- `Tick` carries `late`: frames the tick was sent after its due frame for want of credit (always 0 in lockstep). The
  record's sample row carries the worst `late` of the interval; the per-minute log line counts late ticks. This is
  the measurement of whether the brain keeps up in a human's game.
- The record stays once-a-second for samples. Commands from the lane appear in `cmd` rows like any other; the lane
  issues an order only when its decision for the unit changes (the SC2 lesson: at 10 Hz an order a tick is the
  default bug), so the file grows little.
- Not now: the opening search (760 ms once) and the chase simulator (up to 25 ms) stay on the tick thread. In
  lockstep they cost wall time only; in a human's game they drop a few ticks once at the start and during pricings.
  `late` says how many; moving them to a worker thread is the next step if it says too many.

## 2. The control engine (`brain/micro.rs`, `brain/threat.rs`)
The think pass keeps deciding **intent** (which party, which target, commit or abort) exactly as today and keeps
giving the orders it gives. The lane sits under every intent source (raid, contact, waves, the LLM's squads): it may
override a unit's standing order for as long as a behaviour claims the unit, and gives the standing order back when
none does. It never chooses a party's target and never re-prices; it can only spend a unit's order on staying alive
and shooting well. Disabled as a whole by one ID, `H-MICRO-LANE`, so an A/B run compares the same brain with and
without it.

**Standing orders.** Every Move, Fight or Attack the think pass issues to a soldier is remembered as that unit's
standing order with its frame. The lane's terminal behaviour is "the standing order": a unit the lane has overridden
and then let go gets it re-issued once.

**Commitment.** A unit is *committed* against a threat when its group was priced against it: the turrets in
`raid.turrets` or a response's `turrets`, the soldiers in sight at the last pricing, the commander once the party is
over the commander-party threshold. Everything else in range of it is *uncommitted*: a static defence or the
commander that its group was not priced against, or that it meets on a Move (elsewhere, a wait point, a scout
route). The think pass publishes the committed set per unit with the standing order.

**The threat grid** (`threat.rs`), rebuilt every tick: 32-elmo cells over the map, one layer. Each armed enemy in
sight stamps its damage a second over its reach plus a tail (the distance a Pawn covers in a second, 90 elmos), full
inside the reach, falling to zero over the tail so that the grid has a gradient outside the range. Every remembered
armed building stamps the same whether in sight or not (they do not move). Every armed enemy seen within the last
10 s and now out of sight stamps at its last place, fading with age (the SC2 grids have no fog layer; ours must).
Cells our movement class cannot stand on (the terrain's passable mask) are never a destination. Cost: a few tens of
microseconds a tick.

**Behaviours**, per soldier with an enemy within 700, in this order; the first that acts owns the unit this tick.
1. **H-MICRO-FLEE.** The threat at the unit's *next* place (its position plus its velocity times half a second)
   from sources it is not committed against is above zero: step to the lowest-threat passable cell within 100 elmos,
   preferring, among equals, the one nearest its standing order's goal. Committed against everything there: flee
   only when the threat there times 1.5 s exceeds its remaining health (a Pawn at 100 health leaves the tower's
   range; the rest of the dive stays). This is the predictive threshold, not "I am at 40 %".
2. **H-MICRO-KITE.** The nearest enemy that can hurt it is one it outranges (its reach beyond theirs by 20 or more)
   or outruns (its speed over theirs by 15 % or more), inside its own reach, and its weapon is not ready: step back
   along the gradient so the enemy stays at the edge of its reach; when the weapon is ready, Attack it. Pawns hold
   Fleas off this way (180 against 140) instead of chasing what they cannot catch.
3. **H-MICRO-FOCUS.** Units of one standing order within 300 of each other with enemies inside their reach pick one
   target: the highest metal over the seconds the group needs to kill it, shooters assigned until a second of their
   damage covers its health, then the next target. Attack orders on it with the standing order queued after. Only on
   targets already inside the shooter's reach, so that an Attack order never becomes a chase.
4. The standing order.

The commander is not in the lane (H-COM-RETREAT stays as it is). Constructors and scouts on routes are: a scout that
walks into a tower's range flees it.

**What the lane cannot fix and does not try:** whether to fight at all (the pricing), the party's target, the
answer's size, and the two intents of target priority (Fight against Raid, the user's correction of the turret-first
rule) which belong to the pricing.

## 3. Judged by (mechanism measures; win counts are noise at 12 a batch)
`run/micro_ledger.py <batch>`, from the records, by minute 10 and at the end:
- Army metal lost per enemy metal killed (the exchange), overall and for the pressure party.
- Unit-seconds under fire: sample rows in which a soldier of ours stands inside the reach of an armed enemy in sight
  (the header's unit table gives reaches).
- Deaths inside a turret's or the commander's reach, against deaths elsewhere.
- Damage taken by soldiers that lived to the end of the interval, against by those that died: are the wounded leaving.
- `late` ticks, and the batch's wall time against rush-26's (the lockstep cost of five times the round trips).
A/B: `--ab-disable H-MICRO-LANE`, 24 games on the Quicksilver benchmark. Each behaviour is its own ID under the lane
so a batch can switch one off. One instrumented game before every batch (the afternoon's lesson).

## 4. Walking distances at decision points (the commander's cliff walks)
After the lane, in the same arc. Fields from **destinations**, not origins: one field per metal spot, plus home and
the enemy bases, built once at the survey (44 spots on Quicksilver at 200k cells each, `u16` tenths of a cell; about
17 MB a seat). Then "nearest spot to this builder" and "the leash" mean walking distance for the commander and the
constructors (`economy.rs` `claim_spot`), scouts' routes are ordered by walking distance from the scout, and the
pressure party's alternatives by walking distance from the party. Responders' arrival order stays straight-line
(parties move; a field per party is not worth it). The engine's `Pathing_getApproximateLength` was considered and
rejected: it would need a query-and-answer round trip added to the protocol, and the fields answer the same
question for every unit at once.

## Order of work
1. The tick: shim interval and override, `tick_frames` and `late` in the protocol, the reload frame and velocities in
   the snapshot, the brain split into `think` and `micro` with event carry-over, `TICK_FRAMES` deleted, `late` in the
   record and the log. Smoke game; the batch wall time against rush-26.
2. The threat grid, standing orders and H-MICRO-FLEE; `run/micro_ledger.py`; one instrumented game.
3. H-MICRO-KITE and H-MICRO-FOCUS; the A/B batch; rows in `docs/heuristics.md`, `docs/knowledge/army.md`,
   `docs/experiments.md`.
4. Fields from destinations and the decision points of section 4; a batch against 3.
Each step is a commit.

## Status
- 2026-09-20: written; nothing built.
- 2026-09-20, part 1 built (f54e3d7): ticks every 3 frames, `WITHIN_REASON_TICK_FRAMES` override, late ticks instead
  of dropped ones (`Tick::late`, `Tick::due`; the record's `late`, a per-minute log line), velocities and the reload
  frame in the snapshot, `think` at multiples of 15 with carried events and an empty `micro`. Smoke and timing runs
  (tick-smoke, tick-cost-3/15 in the ledger): nothing the brain does changed, lockstep cost nil (20.1 against 20.3
  game minutes a wall second). Narrowing for part 2: constructors stay out of the lane in its first version (giving a
  Build order back after a flee would cross the economy's order tracking); soldiers only.
- 2026-09-20, part 2 built: `threat.rs` (the grid on the terrain's 16-elmo cells, stamps sloping inside the reach and
  over a 90 tail), `micro.rs` (standing orders, commitments from the raid, the answers, the waves and the squads;
  H-MICRO-FLEE; claims held across identical re-issued orders and for a second at least; faint memories move nobody),
  `run/micro_ledger.py`. Four instrumented games, each death traced (ledger rows micro-flee-debug to -debug4): deaths
  to turrets and the commander by 12:00 went 11, 1, 7, 1 as the traces found and fixed an answer committed to the
  commander by its forced clause, a release the tick after a step, a committed dive fled too late, the commander
  stepped out of sight and re-ordered, a flat grid inside a tower's reach, the raid's re-issue cancelling a hold, and
  faded memories. What remains in the traces: Pawns dying to Pawns, three of them fleeing a fight they could not
  outrun at under 130 health (K-army-withdrawing-a-hurt-soldier-saves-metal-and-loses-the-fight says as much); that is
  part 3's ground (focus fire, kiting) and the contact answer's. Not measured yet beyond single games: the A/B batch
  is next. The record's `dmg` field carries a killing blow's full damage (a D-gun's tens of thousands); the ledger caps
  a unit's damage at its health.
- 2026-09-20, the first A/B (micro-flee-ab, ledger): the flee alone halves deaths and halves kills; wounded units
  left unit fights their side was winning. The lethal branch now fires only when their fire on the unit beats ours
  round it by 1.2 (`ODDS_TO_STAY`).
- 2026-09-20, parts 3 and 4 built: H-MICRO-FOCUS (groups within 300 shoot the dearest target per second of their
  fire, shooters until a second of fire covers it, Attack with the standing order queued, never on a target out of
  reach), H-MICRO-KITE (out-reaching and no slower: step back while reloading, shoot when ready; Rocketeers against
  Hammers in the smoke), and walking fields per metal spot built on a thread at the survey (44 in 0.4 s), used by
  the spot claim, the scouts' routes and the party's spot candidates. Smoke: micro-focus-debug. Measured next by the
  second A/B (micro-ab2).
