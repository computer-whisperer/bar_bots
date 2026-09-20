# Unit micro: which policies pay when our soldiers meet theirs (2026-09-20)

Six candidate policies for what a soldier does once the shooting starts, priced in `crates/combatsim`, then checked
in the engine with the duel runner and in the arena. One survived: **fight as a loose block, not as a blob**
(H-MICRO-SPREAD). It is worth about a fifth of the metal traded in the engine's duels. Kiting, focus fire,
not chasing and pulling damaged units out all failed, three of them for reasons worth writing down.

## What the bot can actually order

Before designing policies, what the brain can express (`crates/bot-protocol/src/messages.rs`, one tick per 15
frames = 0.5 s):

| Policy | Expressible? |
|---|---|
| Fight spread out | **Yes** — one `Fight` per unit, at its own point. No new command, no faster tick. |
| Pull a damaged unit back | **Yes** — `Move` away; the unit keeps firing, as a unit under a move order does. |
| Do not chase | **Yes** — withhold the order, or `Stop`. |
| Hold at max range / kite | **Partly** — `Move` back then `Fight` again is a 0.5 s round trip, and the engine already stops an attack-move at `range x 0.9` of whatever it acquires, so the only part that is missing is backing off when the enemy closes. A step of 0.5 s at 46-87 elmos a second is 23-44 elmos; a Grunt closes 40 of them in the same tick. |
| Focus fire | **No.** There is no attack-unit command. `Fight` names a *point*; which enemy a unit shoots is the engine's choice, and the engine's choice is roughly the nearest. Focus fire cannot be ordered without adding a command to the protocol. |

The simulator can express all six, so focus fire was priced anyway — to find out what a new command would be
worth before asking for one. The answer is that it would be worth less than nothing (below).

## The policies, and how the simulator expresses them

`Scenario::micro` per side (`crates/combatsim/src/scenario.rs`), off by default so the baseline is exactly the
plain attack-move the duel tables were made with. `combatsim micro` prices them all:

- **`spread=N`** — while advancing, a unit is pushed away from friends closer than N elmos. This is the behaviour
  a wave given one destination point each, laid out over the ground, produces.
- **`withdraw=F`** — a unit under F of its health turns round and walks away from the enemy, still firing.
- **`kite`** — a unit that out-reaches its target by 40 elmos *and* out-runs it backs off when the target comes
  inside its stopping distance. **`kite-slow`** drops the speed condition.
- **`focus=weakest`** — shoot the enemy with the fewest hit points left, among those already in range (a unit with
  nothing in range still walks at the nearest, or it would never close).
- **`no-chase`** — do not walk after a target that is faster and already out of reach; stand and let it come.

## What the simulator says

`combatsim micro --reps 16`: 135 cells — our nine tier-1 forces against the opponent's five, at three metal
ratios — 16 seeds each, 1200 metal a side at ratio 1, spawn spacing 56. Every policy runs the same seeds as the
baseline, so the gain is a paired difference. "±" is the standard error of that gain over the 135 cells.

| policy | mean margin | gain | ± | metal killed / lost | cells improved |
|---|---|---|---|---|---|
| plain attack-move | +0.101 | — | — | 1.41 | — |
| **spread=100** | **+0.255** | **+0.155** | 0.021 | **1.96** | 83 / 135 |
| spread=160 | +0.244 | +0.144 | 0.026 | 1.75 | 79 / 135 |
| withdraw=0.35 | +0.062 | −0.039 | 0.009 | 2.41 | 27 / 135 |
| kite | +0.084 | −0.017 | 0.006 | 1.36 | 2 / 135 |
| kite-slow | +0.082 | −0.018 | 0.010 | 1.30 | 19 / 135 |
| focus=weakest | −0.064 | −0.165 | 0.023 | 1.03 | 21 / 135 |
| no-chase | +0.061 | −0.040 | 0.016 | 1.26 | 6 / 135 |

Spreading holds at every metal ratio — it is not a policy that only works from ahead or only from behind:

| ours : theirs | 0.70x | 1.00x | 1.40x |
|---|---|---|---|
| gain in margin | +0.176 | +0.171 | +0.117 |
| metal killed per metal lost, plain | 0.66 | 1.43 | 2.87 |
| ... spread out | 0.99 | 2.14 | 4.01 |

And it is an answer to area damage, not a general improvement. Gain by what we are fighting: Mace +0.34,
Thug +0.36, Rocketeer +0.40, Aggravator +0.40 — every one of them an area weapon (36 elmos for the plasma line
units, 48 for the rocket skirmishers) — against Pawn −0.05, Grunt −0.04 and light towers +0.03 to +0.07.

**Withdrawal is a trade, not a gain.** It has the best metal efficiency of any policy (2.41 against 1.41) and it
*costs* margin (−0.039). Fewer of our units die; the ones that leave stop shooting, so fewer of theirs die too,
and what walks away walks away hurt. The two measures disagree because `metal_lost` counts a unit that got home
on 10 % health as fully saved while `margin` counts it as a tenth of itself. The truth is in between and depends
on something outside the fight: whether a hurt soldier is ever repaired. Ours are not (`H-ECO-REPAIR` is for the
commander), so the honest reading is closer to `margin`, and it was not taken further. When we are outnumbered
(0.70x) it is the one case where withdrawal is not negative: +0.005 of margin and 0.66 → 1.02 metal killed per
metal lost.

**Kiting is worth nothing at these ranges.** In most cells it never triggers: the pairs where we out-reach them
are the pairs where they out-run us (Rocketeer 475 range at 51 speed against Grunt 215 at 81). Forced anyway
(`kite-slow`), the Rocketeer's gain is −0.12 — it backs away from something faster and arrives at the same fight
with less of it left. Where speed and reach do line up the cells are already won.

**Focus fire is worse than the engine's own targeting**, by a lot (−0.165). Nothing stops a salvo already in the
air, so a side that all shoots the lowest-health enemy overkills it; with Rocketeers on a 3.8 s reload the waste
is most of the volley. This is the one policy the bot could not order anyway, and the number says not to ask for
the command.

**Not chasing** is neutral where it does nothing and bad where it fires: the Centurion (45 speed, slower than
everything) loses 0.22 because every target is "faster" and it stands still for the whole fight. As written it is
a defensive rule, and it is the home group and the squads that defend, not the attackers this study is about.

## What the engine says

Three checks, in order of what they cost.

### 1. The simulator's spacing against the engine's

The duel tables of 2026-09-19 were run at spawn spacing 56 and 100 (`docs/data/duels-2026-09-19/`), which is the
same lever from the other end — both sides loose instead of ours. Simulated at the same two spacings:

| pairing | engine tight → wide | simulated tight → wide |
|---|---|---|
| armpw v armham | −0.44 → −0.10 (+0.34) | −0.36 → +0.12 (+0.48) |
| armpw v corthud | −0.43 → −0.21 (+0.22) | −0.33 → +0.19 (+0.52) |
| corak v armham | −0.44 → −0.32 (+0.12) | −0.25 → +0.23 (+0.48) |
| armpw v corak | +0.14 → +0.15 (+0.01) | −0.48 → −0.39 (+0.09) |
| armrock v corak | −0.03 → −0.23 (−0.20) | +0.08 → −0.34 (−0.42) |
| armrock v corthud | −0.46 → −0.52 (−0.06) | −0.24 → −0.40 (−0.16) |

Six signs of six, and the simulator overstates the size by about a factor of two in all six — the same direction
as the bias `combat-sim.md` already records for area damage in dense formations.

### 2. Duels with the spread *orders*

Spacing is an input to a duel; orders are what the bot has. `duel --spread N` (new) sends the **first** army of
each pairing to its own point in a loose block around the enemy instead of sending everybody to one point — the
same `loose_block` layout the bot uses, copied into `crates/arena/src/bin/duel/director.rs`. `duels.csv` also
gained `spread_x` / `spread_y`: how far each army's units stood from their own centre, RMS, when the first shot
landed. That is the probe `combat-sim.md` asked for first, and it is what tells us the behaviour happened.

**First probe** (batches `micro-spread-off` / `micro-spread-on`, 3 pairings x 8 duels, a single line 110 apart):
dispersion at contact 86-100 → 288-303 elmos for us, 93-104 unchanged for them. The behaviour happens.
Margins: armpw v armham −0.437 → −0.258, armpw v corthud −0.455 → −0.321, armpw v corak +0.150 → +0.253. The
baseline reproduced the recorded table row for armpw v armham to three decimals (−0.437 against −0.436).

**The A/B that counts** (batches `micro-block-off` / `micro-block-on`, 28 pairings x 6 duels per arm, equal metal
at 1200, spawn spacing 56, block gap 110 — the bot's own geometry):

**Metal killed per metal lost over all 336 duels: 1.11 without, 1.31 with.** Mean margin gain +0.131.

Gain by our own unit, averaged over the four opponents each met:

| ours | armpw | corak | armwar | corthud | armham | corstorm | armrock |
|---|---|---|---|---|---|---|---|
| mean gain | **+0.41** | **+0.46** | +0.05 | +0.02 | +0.02 | −0.00 | −0.03 |

The whole gain belongs to the raiders, and the rest is inside the noise of six duels. The largest cells:

| pairing | margin without | margin with | gain | metal killed per metal lost |
|---|---|---|---|---|
| corak v armllt | −0.193 ±0.028 | +0.652 ±0.048 | **+0.845** | 0.80 → 2.84 |
| armpw v corllt | −0.150 ±0.093 | +0.560 ±0.019 | **+0.710** | 0.84 → 2.27 |
| armpw v corstorm | −0.234 ±0.066 | +0.253 ±0.016 | +0.487 | 0.78 → 1.36 |
| corak v armrock | −0.021 ±0.082 | +0.381 ±0.033 | +0.402 | 0.97 → 1.61 |
| armpw v corthud | −0.423 ±0.033 | −0.052 ±0.064 | +0.372 | 0.58 → 0.94 |
| corak v armham | −0.419 ±0.022 | −0.056 ±0.062 | +0.363 | 0.59 → 0.95 |
| **armrock v corak** | +0.111 ±0.069 | −0.143 ±0.036 | **−0.254** | 1.13 → 0.86 |

So there are two mechanisms, not one, and only the first was designed for:

1. **Dodging area damage.** Predicted by the simulator and by the spacing tables; visible in armpw/corak against
   Mace, Thug, Rocketeer and Aggravator.
2. **Getting more short-range guns into range at once.** The two biggest cells in the whole matrix are raiders
   against a *light tower line*, where there is no area damage to dodge (the towers' beam is impact-only, area 11).
   A blob of 26 Pawns cannot all stand within 180 elmos of one tower — the crowding the simulator pins as
   `collision_saturates_a_short_range_blob` — while a block can. The simulator has that mechanism and still
   predicts nothing here (below): what it is missing is that a *blob* also walks in as a column and feeds itself
   to a tower a few at a time.

The one clear cost, Rocketeers against Grunts (−0.254), is the same shape as the tables' own −0.20 from spacing:
a slow, fragile, long-ranged unit needs its neighbours, and spread out it is run down one at a time.

### 3. Where the simulator and the engine disagree

Per pairing, the gain the simulator predicts against the gain the engine measured, over the same 28 pairings at
the same counts:

- mean gain: simulated +0.237, engine +0.131 — overstated by about 1.8x, as in check 1;
- sign agreement 10 of the 15 pairings where the engine moved by more than 0.05;
- **correlation 0.27, slope 0.31**: the simulator is a poor ranker of which matchup the policy pays in.

The two biggest engine gains are the two the simulator misses completely: raiders against a tower line, where it
predicts nothing at all. **The engine is right; the simulator's answer was used to choose what to test, not what
to believe.** Recorded in `combat-sim.md` under *Where it is wrong*.

## The heuristic

`crates/bot/src/brain/micro.rs`, one call in `brain/mod.rs`:

> **H-MICRO-SPREAD.** An attack order given to a committed attacker is aimed at that unit's own place in a block
> around the destination — files across the approach, ranks behind, 110 elmos apart, at most 8 files wide —
> instead of at the destination itself.

Details and why:
- **110 elmos** clears the blasts being dodged (36 and 48, measured to the collision volume's surface, and a
  tier-1 bot's radius is 11-15) with room for the engine's own jostling.
- **A block, not a line.** The first probe used a line, which for 24 units is 2500 elmos of front. That is fine on
  a duel site and absurd on a map. The block is `ceil(sqrt(2n))` files, so twice as wide as deep, capped at 8
  files: 40 attackers stand 770 x 440.
- **Units keep their left-to-right order** when lanes are handed out, so nobody is sent across anybody's path.
- **Points off walkable ground are snapped** onto it (`snap_to_reachable`), or the wave would file move-failures
  into the rule that gives targets up as unreachable.
- **The march to the staging point is left alone.** H-ARMY-STAGE counts an attacker as gathered by its distance
  to the staging point (500), and a wave told to stand in a block around it would never reach quorum and would
  wait out all 150 s of its patience. Spreading is for where the shooting is.
- **The hook is a rewrite of the orders the army already decided on**, at the end of `Brain::decide`, not a hook
  in each of the six places `army.rs` issues a `Fight`. What to attack and when to go are not this rule's
  business, and `army.rs` is being rewritten elsewhere.
- Squad orders and the home group's defence orders are untouched: only committed attackers.

## Arena A/B

See the row in `docs/experiments.md` (`micro-spread`).

## What was not done

- **Withdrawal was not implemented**, although it has the best metal efficiency of any policy. It costs margin in
  the simulator, the gap between the two measures is exactly "does a hurt soldier ever get repaired", and we do
  not repair soldiers. The cheap experiment that would settle it is to repair them first.
- **No terrain in any of the simulator cells.** `Scenario::terrain` exists and the policies are all movement
  rules, so a choke should change every one of them. Every cell here is flat open ground.
- **Uneven metal was only tested in the simulator.** The duel harness sizes both sides to one budget; the engine
  A/B is entirely at equal metal.
- **Mixed forces were only tested in the simulator** (`armham:2+armrock:1` and `corthud:2+corstorm:1`), where
  spreading gains +0.10 and +0.13 — between the raider's +0.32 and the skirmisher's −0.00. The duel harness pairs
  one type against one type, which is precisely the case the engine gain splits on, so the number our real
  mixed waves should expect is unmeasured in the engine.
- **The obvious refinement is untested**: the engine says the gain is entirely the raiders'. Spreading only the
  short-ranged units would keep it and drop the Rocketeer cost. The bot cannot see weapon range
  (`UnitDefInfo` has no range field), so this needs either the roster's role names or a protocol field.
