# Combat simulator: "x against y on terrain z" from the game's own numbers (2026-09-20)

`crates/combatsim` simulates a land fight frame by frame at the engine's 30 Hz and answers who wins, with what
left, and how long it takes. It exists because the brain's matchup table (`crates/bot/src/brain/combat.rs`,
`crates/bot/data/matchups.csv`) knows exactly one situation — two single-type blobs of equal metal charging each
other on flat ground — and the questions we want to ask are about range, arrival order, turrets, mixed forces and
the shape of the ground. Ground truth is the engine duel tables of 2026-09-19; `combatsim validate` replays every
pairing of both tables and reports the agreement below.

**Nothing here has been tested in the arena, and no new engine duels were run for it.** The simulator agrees with
the engine on three quarters of the decisive pairings it was fitted against. The quarter it gets wrong is not
random — see *Where it is wrong*.

```
cargo build --release -p combatsim
combatsim --a armham:10 --b corak:30 --spacing 56 --reps 200
combatsim --a "armpw:12,armpw:12@15" --b armham:10          # a second wave fifteen seconds late
combatsim --a armrock:10 --b corllt:6 --hold-b              # rockets against a tower line
combatsim --a armpw:24 --b armham:10 --terrain run/matches/<dir>/00/terrain-0.bin:448:448 \
          --from 1200,3600 --at 2300,3600
combatsim validate [--spacing tight|wide|both] [--reps 4] [--no-collide]
combatsim micro [--reps 8] [--policies "none;spread=100;withdraw=0.35"] [--detail]
combatsim speed
```

## What it models

Per unit: a 2D position, health, the collision radius of its collision volume, speed, sight, metal, armour class.
Per weapon: range, reload and salvo on the engine's whole-frame clock, damage by the target's armour class, area
of effect with the engine's falloff, projectile flight time (including a missile's acceleration from
`startvelocity`), aim scatter, lead prediction, and energy per shot.

Behaviour, deliberately thin: walk at the enemy's centre until a target is inside `range × 0.9` — the engine's own
stop distance (`MobileCAI.cpp` `ExecuteAttack`) — then stand and shoot the nearest visible enemy. Sight is shared
across a side, as the engine's LOS is; a unit that nobody can see cannot be shot at, which is why artillery with
364 sight does not get to use its 710 range. Groups can carry an arrival delay and a hold order; buildings always
hold. Terrain is the bot's own grid (`docs/harness/record-format.md`), with passability per movement class and one
Dijkstra walking field per side, so a choke limits how many get through without any pathfinding in the hot loop.

Three mechanisms were added because the validation asked for them, in order of what they were worth:

**Energy per shot.** The Sentry and Guard have the best damage per metal in the tier-1 table — 161 a second for
85 metal — and lose to Maces anyway. The reason is in the duel rows, not in the model: in `armham` against
`armllt` the towers took 34.5 seconds to deal 3904 damage, which is 52 shots, which at 20 energy a shot is exactly
the 30 energy a second a lone commander makes. In `armpw` against `armllt` they got 118 shots — the same income
plus a full 1500-energy store left over from the previous duel. A side therefore has an energy budget, and a
laser tower holds its shot when the store is empty (`Scenario::energy`, default 500 stored and 30 a second).
Agreement against the tight table, sweeping the starting store:

| stored energy | 0 | 400 | 500 | 1000 | 1500 | unlimited |
|---|---|---|---|---|---|---|
| sign agreement | 75 % | 76 % | 76 % | 75 % | 70 % | 71 % |
| mean abs. error | 0.381 | 0.299 | 0.298 | 0.314 | 0.354 | 0.398 |

**Lead prediction is mostly guesswork.** `predictBoost` defaults to 0 in the engine (`WeaponDef.cpp`), and the
engine's own description of that is "it will over- or under-estimate target speed by between 0-2x its actual
value", redrawn every 15 frames. Of the weapons in our table only five set it to 1; Mace and Thug set 0.4;
artillery, rockets, Pounder, Janus, Stout and Brute leave it at 0. So an unguided shot at a moving target lands
anywhere between the target's current position and twice its lead. Aim scatter uses the engine's own conversion
from the def's raw number, `sin(x · π / 0xafff)` — a Pawn's spray of 1180 is 4.7°, not the 6.5° a naive
65536-to-a-turn reading gives.

**Units cannot stand in each other.** A step onto ground another unit occupies is refused, and the unit tries the
same step deflected up to ±97° before giving up for the frame, so a blob flows around its own front rank rather
than jamming or stacking. The radius used is the collision volume's, not the build footprint's: the engine pushes
units apart with `unit->radius` (`CGroundMoveType`), the volume is nearly twice the footprint for a tier-1 bot
(14.5 against 8), and blocking on the footprint let two units' hit volumes overlap — which made an impact-only
laser splash onto the unit behind and cost three points of agreement.

Post-processing: `gamedata/alldefs_post.lua` is applied where it changes combat numbers with the arena's default
mod options — reload and burst rounded down to whole frames, and a BeamLaser with `impactonly` becoming area 11
with edge effectiveness 1. Every `multiplier_*` mod option is 1 by default, so nothing else in `_post` touches
health, damage, range or speed; the rest is graphics, categories and mod options we do not set. Movement slope and
wading depth come from the class named by `movementclass` in `gamedata/movedefs.lua` (bots 54°, vehicles 27°),
which is what the record format's `move_classes` shows; the unit file's own `maxslope` is legacy and much smaller.

## What it leaves out

No height (the engine's range-by-height rule for ballistic weapons is **not** modelled; nothing in the flat duel
tables could have tested it). No wrecks, and so no wrecks blocking rockets — the duel harness sweeps them between
duels anyway. No veterancy, although the engine gives up to 2.5× health and 1.25× rate of fire at full experience
and a duel earns a little of it. No death explosions: the weapondefs the unit files name for them
(`smallexplosiongeneric` and friends) are not defined anywhere in the game data, so they appear to do nothing. No
turret slew or turn-in-place, no acceleration, no unit-unit collision damage, no repair, no radar, no
line-of-sight blocking by terrain (units shoot across a cliff they cannot walk
over). Shots are resolved as a point impact at a computed time rather than as travelling projectiles, so nothing
is intercepted in flight and a shell does not collide with the front rank on its way to the rear one. Air is
excluded by construction: anti-air weapons are dropped when the tables are built.

## Unit micro (added 2026-09-20)

`Scenario::micro`, one policy set per side, every field off by default so the baseline is the plain attack-move the
duel tables were made with: `spread` (a push away from friends closer than N elmos while advancing), `withdraw_below`
(a hurt unit turns round and walks away, still firing), `kite` (a unit that out-reaches and out-runs its target backs
off when the target closes), `focus` (shoot the weakest, or the most damage a second per hit point left, among those
in range) and `no_chase`. `combatsim micro` prices them over 135 tier-1 cells. The study is
`docs/studies/micro-combat.md`; the short version is that spreading out is worth +0.155 of margin here and +0.131 in
the engine, and that nothing else priced positive.

## Validation

`combatsim validate --reps 4`: every ordered pairing of both duel tables, at the harness's own geometry (front
ranks 1100 elmos apart, ranks of eight, the counts the harness used), four seeds each. "Decisive" is a table
margin of at least 0.10, which is where the table itself stops being noise.

| table | pairings | decisive | sign agreement | mean abs. error | correlation | slope |
|---|---|---|---|---|---|---|
| tight (spacing 56) | 525 | 483 | **367 / 483 = 76 %** | 0.298 | 0.708 | 0.79 |
| wide (spacing 100) | 525 | 472 | **381 / 472 = 81 %** | 0.252 | 0.802 | 0.79 |

Slope 0.79 means the simulation is *less* decisive than the engine across the board: it under-states margins by
about a fifth, in both directions. The whole run takes 5.3 s for 2100 simulated duels.

**With collision off** (`--no-collide`): tight 74 % / 0.336 / r 0.667, wide 82 % / 0.227 / r 0.840. So on the duel
tables collision is worth three points of agreement in the tight table and costs one in the wide one. That is
about what it should be: the duel is open flat ground with no choke and armies well under the width the site
allows, so crowding only bites where the formation is already dense. The duel tables cannot test the case
collision exists for, which is why the two checks below were run instead.

**Firepower saturation.** N units killing eight weaponless factories (23 200 hp), effective damage a second
delivered, with collision on and off:

| | 8 | 32 | 64 | 8 → 64 |
|---|---|---|---|---|
| Pawn (range 180), colliding | 663 | 1311 | 1459 | ×2.2 |
| Pawn, passing through each other | 693 | 2231 | 3135 | ×4.5 |
| Rocketeer (range 475), colliding | 552 | 1950 | 2468 | ×4.5 |
| Rocketeer, passing through | 554 | 1933 | 2829 | ×5.1 |

Eight times the Pawns deliver a bit over twice the damage: past about 32 the extra ones are standing behind
someone. Rocketeers, which stop 475 elmos out on a much longer arc, barely notice. Pinned by
`collision_saturates_a_short_range_blob` in `tests/mechanics.rs`.

**Chokes.** 24 Pawns attacking 10 Maces score −0.26 on open ground and −0.87 when they have to come through a
48-elmo gap in a cliff wall: they arrive three abreast and are killed in the order they arrive. A solid wall ends
the fight as a stalemate with neither side able to reach the other
(`a_choke_costs_the_side_that_has_to_come_through_it`, `terrain_can_cut_a_fight_off_entirely`).

**Frontage and the tight/wide difference are not the same thing**, and this study could not separate them with
what it has. Turning collision off changes the tight table by three points and the wide table by one, while the
tight-to-wide difference in the tables themselves is much larger than either. The spacing effect in the duel
tables is dominated by area damage, not by frontage, at these army sizes.

## Where it is wrong

**Artillery.** Every one of the ten worst tight-table misses involves `armart` or `corwolv`. The engine says
tier-1 artillery loses to everything mobile (row means −37 and −28, K-units-duel-range-vs-turrets); the simulator
has it winning. The cause is measurable and it is not the range or the sight: it is damage per shell. In the
engine, `armart` against `armflash` dealt 4473 damage with about 22 shells — 203 a shell, one unit's worth. The
simulator's shells land in a formation that is still in rank at 56 elmos and take four neighbours with them, about
450 a shell. Loosening the formation recovers the engine's answer exactly: `armart` against `armflash` goes +0.63,
+0.15, −0.24, −0.42 at spacing 56, 100, 160, 240, and the engine's spacing-56 answer is −0.67. So **real armies do
not arrive in the formation they were spawned in**, and the simulator's do. The same error makes Janus and Pounder
too strong in the tight table. The honest summary is that the simulator's formation density is an input where the
engine's is an outcome.

**Spreading out against a tower line.** Measured 2026-09-20 with the duel runner's new `--spread` (28 tier-1
pairings, 6 duels an arm): the two largest engine gains from spreading an army are raiders against a light tower
line — Grunt against Sentry −0.193 to +0.652, Pawn against Guard −0.150 to +0.560 — and the simulator predicts
nothing at all for either. It has the mechanism (`collision_saturates_a_short_range_blob`) and still misses the
case, because its blob does not also *arrive* as a column and feed itself to the tower a few at a time. Over those
28 pairings the simulator's gain correlates with the engine's at only 0.27 and is 1.8x too large on average, while
agreeing in sign on 10 of the 15 pairings the engine moved by more than 0.05. **Read the policy numbers as a
shortlist of what to test in the engine, not as a ranking.**

Secondary, in the wide table: light towers against the fast scout cars (`armfav`, `corfav`) are the worst misses
there, and they are the same shape — the tower's 20-energy shots plus a 0-lead prediction against a 150-speed
target, where a small error in either swings the whole result.

The fight is also **too short**: `armart` against `armflash` reaches first damage at 5.5 s and ends at 10-14 s,
where the engine takes 8.0 s and 18-20 s. Some of that is acceleration and the 90° turn every spawned army starts
with, which the simulator does not model; the rest is unexplained and is a lead worth pulling on, because a fight
that is a third too short flatters whoever shoots first.

## Speed

Release build, one core, including the 1100-elmo approach (so these are pessimistic for the short-range queries a
bot would ask):

| fight | queries a second | per query |
|---|---|---|
| 5 v 5 | 5100 | 0.20 ms |
| 20 v 20 | 1000 | 1.0 ms |
| 40 v 40 | 390 | 2.6 ms |

The target was thousands a second for small fights and well under 10 ms at 40 v 40; both are met. A 200-seed Monte
Carlo of a 40 v 40 takes half a second, which is a decision the commander can afford and the heuristic brain
cannot — see below.

## Engine duels this study would most like

In priority order, all runnable with the existing harness except the first:

1. ~~**Measure the spacing an army actually fights at.**~~ Done 2026-09-20: `spread_x` / `spread_y` in
   `duels.csv` are each army's root-mean-square distance from its own centre at the first damage. A spacing-56
   army of tier-1 bots fights at a mean of 79-93 of them and the spread is wide: Centurion 49 (29-63), Rocketeer 79
   (37-122), Aggravator 81, Mace 85, Pawn 86, Thud 91, Grunt 93 (76-136), and a light tower line stands at 114-119
   simply because that is how it was placed. So spawn spacing is **not** what an army fights at, the number varies
   by a factor of four within one unit type, and the formation density the artillery error turns on can now be
   measured per unit instead of assumed (`micro-block-off`, 168 duels).
2. **Artillery with a spotter.** `armart` plus two `armflea` against `armllt`, against `armart` alone. Tests
   K-units-duel-range-vs-turrets' conjecture that sight, not range, is what loses it, which the simulator says is
   only part of the story.
3. **Mixed forces.** A line unit screening a skirmisher: `armham:5,armrock:5` against `corthud:9`, and against
   `corak:28`. The simulator's whole reason for existing is mixed forces and nothing in the tables tests one.
4. **Turrets at a stand-off.** `armrock` ordered to hold at 460 against `corllt`, against the same charging. The
   tables only have both sides charging, and K-units-rockets-outrange-llt's tactic is untested.
5. **A choke.** The same pairing on a site with a wall and a gap, at two gap widths. Nothing in the tables has
   terrain at all.
6. **Energy.** The same tower pairing with the tower team's energy storage deliberately full and deliberately
   empty. That would turn the energy model from an inference off the duel rows into a measurement.

## How the bot could use it

`Brain::odds` returns one number — a power ratio from metal and the matchup table — and that is what the wave
gate, the recall rule and the commander's `attack` tool all read. Two ways in, in increasing order of risk:

- **Complement it.** Keep `odds` as the cheap gate and call `combatsim::odds` only at the decisions that are
  expensive to get wrong: committing a wave, and answering the commander's "should I attack here?". At 200 seeds
  of a 20 v 20 that is 200 ms, which is nothing beside the commander's thinking time and too much for every tick.
  The scenario would come straight out of the brain's world model: our wave as one group at its centroid, the
  enemy as one group per seen type at theirs, the seen turrets as held groups, `Scenario::terrain` from the
  terrain the bot already has, and `Scenario::energy` from our actual stored energy and income.
- **Replace it.** Only once a batch has shown the simulator predicting recorded engagements at least as well as
  the table does. The table's own record is 89 % of 273 decisive recorded engagements (`run/predict_check.py`);
  the simulator has not been measured against that at all, and the honest first step is to run
  `predict_check.py`'s engagements through it and compare.

Either way the simulator's known bias — artillery and area damage too strong in a dense formation, margins a fifth
too flat — argues for using it as a *comparison between options* (is A better than B for this fight?) rather than
as an absolute "we win". The comparisons are what the brain needs anyway.
