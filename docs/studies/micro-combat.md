# Unit micro: which policies pay when our soldiers meet theirs (2026-09-20)

Six candidate policies for what a soldier does once the shooting starts, priced in `crates/combatsim`, then checked
in the engine with the duel runner and in the arena. One survived the first two stages: **fight as a loose block,
not as a blob** (H-MICRO-SPREAD), worth about a fifth of the metal traded over 336 engine duels. **In the arena it does nothing
measurable**, and the measurement of why is the most useful thing here: our waves already fight at 330-360 elmos
of dispersion against the duel harness's blob of 81, so there is no blob left to unpack. Kiting,
focus fire, not chasing and pulling damaged units out all failed earlier, three of them for reasons worth writing
down.

## What the bot can actually order

Before designing policies, what the brain can express (`crates/bot-protocol/src/messages.rs`, one tick per 15
frames = 0.5 s):

| Policy | Expressible? |
|---|---|
| Fight spread out | **Yes** — one `Fight` per unit, at its own point. No new command, no faster tick. |
| Pull a damaged unit back | **Yes** — `Move` away. The simulator assumes it keeps firing on the way, which is what a unit at the engine's default fire state does; **not checked in the engine**, and it would have to be before this policy was built. |
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
- **`withdraw=F`** — a unit under F of its health turns round and walks away from the enemy, still firing (an assumption about the engine, see the table above).
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

Six signs of six — though `armpw v corak` is a sign in name only, +0.01 against +0.09, both of them nothing — and
the simulator overstates the size by about a factor of two in all six, the same direction as
the bias `combat-sim.md` already records for area damage in dense formations.

### 2. Duels with the spread *orders*

Spacing is an input to a duel; orders are what the bot has. `duel --spread N` (new) sends the **first** army of
each pairing to its own point in a loose block around the enemy instead of sending everybody to one point — the
same `loose_block` layout the bot uses, copied into `crates/arena/src/bin/duel/director.rs`. `duels.csv` also
gained `spread_x` / `spread_y`: how far each army's units stood from their own centre, RMS, when the first shot
landed. That is the probe `combat-sim.md` asked for first, and it is what tells us the behaviour happened.

**First probe** (batches `micro-spread-off` / `micro-spread-on`, 3 pairings x 8 duels, a single line 110 apart,
which for 23-26 Pawns is 2500 elmos of front): dispersion at contact 86-100 → 288-303 elmos for us, 93-104
unchanged for them. The behaviour happens. Margins: armpw v armham −0.437 → −0.258, armpw v corthud
−0.455 → −0.321, armpw v corak +0.150 → +0.253. The baseline reproduced the recorded table row for armpw v armham
to three decimals (−0.437 against −0.436).

**The A/B that counts** (batches `micro2-off` / `micro2-on`, 28 pairings x 6 duels per arm, equal metal at 1200,
spawn spacing 56, block gap 110 — the bot's own geometry, both arms re-run after the corrections in *Two faults
found on review* below):

**Metal killed per metal lost over all 336 duels: 1.09 without, 1.30 with.** Mean margin gain +0.146.
Dispersion at contact: ours 81 → 131 elmos (28-152 → 60-200), theirs 94 either way. The behaviour happens, and
the block is deliberately far more compact than the first probe's line.

Gain by our own unit, averaged over the four opponents each met:

| ours | corak | armpw | corstorm | armwar | armrock | armham | corthud |
|---|---|---|---|---|---|---|---|
| mean gain | **+0.47** | **+0.47** | +0.04 | +0.04 | +0.02 | −0.01 | −0.01 |

**The whole gain belongs to the raiders; everything else is inside ±0.05, which at six duels a cell is nothing.**
The largest cells:

| pairing | margin without | margin with | gain | metal killed per metal lost |
|---|---|---|---|---|
| armpw v corllt | −0.182 ±0.035 | +0.570 ±0.031 | **+0.753** | 0.82 → 2.33 |
| corak v armllt | −0.070 ±0.049 | +0.613 ±0.030 | **+0.683** | 0.91 → 2.55 |
| corak v armrock | −0.153 ±0.041 | +0.426 ±0.031 | +0.580 | 0.84 → 1.74 |
| armpw v corthud | −0.477 ±0.020 | +0.094 ±0.040 | +0.571 | 0.52 → 1.10 |
| corak v armham | −0.418 ±0.020 | +0.081 ±0.037 | +0.499 | 0.59 → 1.10 |
| armpw v corstorm | −0.312 ±0.035 | +0.135 ±0.045 | +0.447 | 0.70 → 1.18 |
| armham v corak | +0.425 ±0.034 | +0.355 ±0.043 | −0.071 | 1.73 → 1.54 |

**A correction to the first pass.** The pre-correction batches put Rocketeers against Grunts at −0.254 and I
reported it as the policy's one clear cost. Re-run it is −0.056, and the Rocketeer's row mean is +0.02. Six
duels a cell cannot separate a fifth of a margin from noise, and that cell was noise. The claim that a slow
fragile long-ranged unit is hurt by spreading out is **not supported by this harness**; the engine's own spacing
tables still say it (armrock v corak −0.03 tight against −0.23 wide), and those have more duels behind them, so
the question is open rather than settled either way.

So there are two mechanisms, not one, and only the first was designed for:

1. **Dodging area damage.** Predicted by the simulator and by the spacing tables; visible in armpw/corak against
   Mace, Thug, Rocketeer and Aggravator.
2. **Getting more short-range guns into range at once.** The two biggest cells in the whole matrix are raiders
   against a *light tower line*, where there is no area damage to dodge (the towers' beam is impact-only, area 11).
   A blob of 20 Pawns cannot all stand within 180 elmos of one Guard — the crowding the simulator pins as
   `collision_saturates_a_short_range_blob` — while a block can. The simulator has that mechanism and still
   predicts nothing here (below): what it is missing is that a *blob* also walks in as a column and feeds itself
   to a tower a few at a time.

There is no clear cost anywhere in the matrix: the worst cell is −0.071 and the worst row mean −0.01.

### 3. Where the simulator and the engine disagree

Per pairing, the gain the simulator predicts against the gain the engine measured, over the same 28 pairings at
the same counts:

- mean gain: simulated +0.237, engine +0.146 — overstated by about 1.6x, as in check 1;
- sign agreement 11 of the 19 pairings where the engine moved by more than 0.05 (58 %, barely better than a coin);
- **correlation 0.36, slope 0.42**: the simulator is a poor ranker of which matchup the policy pays in.

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
  tier-1 bot's radius is 11-15) with room for the engine's own jostling. Swept in the simulator, the gain is
  +0.093 at a 60-elmo gap, +0.155 at 100 and +0.154 at 140: it saturates once a neighbour is out of one blast,
  and going wider only costs against raiders (spread=160 is worse than spread=100 in the table above).
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

## Arena A/B: still nothing, now with the rule really firing

The first arena batch (`micro-spread`, 48 games) measured an implementation that formed a block only for the units
it happened to be ordering in one tick — a few idle attackers at a time, usually below the six-unit minimum — so
it acted about 9 times a game. After the correction it acts **155 times a game**. This batch is the one that
tests the policy. (The first batch's raw data was lost with the worktree it lived in; its numbers survive only in
this study's history, and nothing about it can be re-examined.)

`arena --matches 48 --parallel 4 --speed 50 --profile medium --ab-disable H-MICRO-SPREAD --label micro2-spread`
(the arena records the opponent's ground truth by itself now). Arm A is the rule on, arm B the same binary with
it off; 24 games each, interleaved in blocks of four on the same seeds.

| | games | W-L-T | army metal killed / lost | NW | SE |
|---|---|---|---|---|---|
| arm A, rule on | 24 | 14-10-0 | **0.98** | 0.86 | 1.04 |
| arm B, rule off | 24 | 12-11-1 | **0.95** | 0.85 | 1.00 |

Everything leans the right way and nothing is resolvable: three points of trade ratio and two wins, on a harness
where two arms of identical code have scored 8-3-1 against 4-4-4. The ablation is clean — H-MICRO-SPREAD fired 0
times in all 24 arm-B games, and in 17 of 24 arm-A games (155 orders a game over the arm, 220 where it fired; the
seven that never fired it are games where no wave launched).

**The mechanism measurement is the result.** `analyze_match.py` reports, for every engagement, how far our
soldiers stood from their own centre — root mean square, the same statistic as the duel harness's `spread_x`.
Over the 930 engagements of this batch with at least 300 metal of ours present:

| | engagements | mean spread | median |
|---|---|---|---|
| arm A (rule on) | 489 | 358 | 327 |
| arm B (rule off) | 441 | 356 | 343 |
| arm A, in their half or at their base | 189 | 303 | 262 |
| arm B, in their half or at their base | 141 | 270 | 221 |

With the rule firing 17 times more often than in the first batch, our army's fighting dispersion moved from 356
to 358 elmos. **A real wave already fights at 330-360, and the duel harness's blob is 81.** Even the harness's
*spread* arm only reaches 131. There is nothing to unpack: by the time one of our waves is in contact it has
walked a thousand elmos through terrain, been marched, regrouped, detached and lost its fastest, and it is
scattered three times as loosely as the formation the policy was designed to break up. The only place the rule
shows at all is in the enemy's half, where the army is most concentrated: 303 against 270 mean, 262 against 221
median.

**And it is not free.** Sending forty units to forty separate points costs walkability: soldier and builder move
failures 225 a game with the rule against 155 without, and attack targets given up as unreachable 0.21 a game
against 0.12. (Stations given up are 19.9 against 20.4 — that is a background rate this rule does not touch.) In
the first batch, where the rule barely fired, this cost was invisible (91 against 90).

So the honest verdict is unchanged and better supported than before: **the policy is real, the problem is not
ours.** It is kept — the trade ratio and the win count both lean its way, the ablation is clean, and it is right
in every stand-up fight the engine will run — but it buys nothing measurable here and it does cost move failures,
so it is a candidate to switch off if walkability ever becomes the binding problem.

## What was not done

- **Withdrawal was not implemented**, although it has the best metal efficiency of any policy. It costs margin in
  the simulator, the gap between the two measures is exactly "does a hurt soldier ever get repaired", and we do
  not repair soldiers. The cheap experiment that would settle it is to repair them first.
- **No terrain in any of the simulator cells.** `Scenario::terrain` exists and the policies are all movement
  rules, so a choke should change every one of them. Every cell here is flat open ground.
- **Uneven metal was only tested in the simulator.** The duel harness sizes both sides to one budget; the engine
  A/B is entirely at equal metal.
- **The first pass's raw data is gone.** Batches `micro-spread-off/on`, `micro-block-off/on` and the 48-game
  `micro-spread` were deleted with the worktree they lived in before they could be archived. Their numbers survive
  in this study's history; nothing about them can be re-examined, which is why the duel A/B and the arena A/B were
  both re-run from scratch rather than patched up.
- **Mixed forces were only tested in the simulator** (`armham:2+armrock:1` and `corthud:2+corstorm:1`), where
  spreading gains +0.10 and +0.13, between the same simulator's Pawn (+0.32) and its Rocketeer (+0.12). The duel harness pairs
  one type against one type, which is precisely the case the engine gain splits on, so the number our real
  mixed waves should expect is unmeasured in the engine.
- **The obvious refinement is untested**: the engine says the gain is entirely the raiders'. Spreading only the
  short-ranged units would keep it and drop the Rocketeer cost. The bot cannot see weapon range
  (`UnitDefInfo` has no range field), so this needs either the roster's role names or a protocol field.

## What to do next, in the order the numbers argue for

1. **Stop pricing formation rules on the duel tables.** Any future spacing, concave or frontage rule should be
   measured against `our_fighters_spread` in real games first (K-army-a-real-wave-is-not-a-blob). The duel harness
   answers "which unit beats which", not "which formation a wave arrives in". The gap is a factor of four.
2. **The wave gate, not the fighting, is what loses these games.** Nine of 24 arm-A games never launched a wave at
   all and went 0-9. No micro policy can reach a game the army spends at home.
3. **Repair soldiers, then re-price withdrawal.** It is the best metal-efficiency policy measured (1.41 to 2.41)
   and the only reason to discount it is that a hurt soldier never heals. That is a change to `H-ECO-REPAIR`, not
   to micro.
4. **If a formation rule is wanted anyway, spread only the raiders.** The engine says the whole gain is Pawn's and
   Grunt's and the one real cost is the Rocketeer's. `UnitDefInfo` carries no weapon range, so this needs the
   roster's role names or a protocol field.
5. **Do not add an attack-unit command for focus fire.** Simulated, naive focus fire is 0.165 of margin worse than
   the engine's own targeting.
