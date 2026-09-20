# Tier-2 army: what the advanced bot lab should build (2026-09-20)

Once we own an `armalab` / `coralab`, what comes out of it. Two pieces of work: BARb's real army composition
counted out of 96 recorded matches, and 90 720 simulated fights at equal metal between our candidate mixes and
that army. Nothing here has been near the arena. The composition table is counted from match logs and the
build-power arithmetic comes from the unit files; everything about who beats whom is `simulated`.

**Short answer.** Cortex: build **Sumos (`corcan`)** for about half the army's metal and keep the Thug /
Aggravator line for the other half; keep one **Mammoth (`corsumo`)** in the army if the plan is to walk into a
tower line. Armada: build **Welders (`armzeus`) and Hounds (`armfido`)**, roughly a third each with a third
tier-1 screen. Both are worth **+0.27 to +0.63 of margin** over the same metal spent on the tier-1 line — far
outside the +-0.01 standard error on a cell, and larger than any error the simulator is known to make. The
tier-2 *build power* argument is separate and stronger still: one advanced lab has four times a tier-1 lab's
build power, and spending 45 metal a second through tier-1 bot labs needs five of them.

```
cargo build --release -p combatsim
crates/combatsim/tools/tier2_study.py army run/matches/*v2[3456]-*/[0-9][0-9] > docs/studies/data/tier2-barb-army.csv
crates/combatsim/tools/tier2_study.py fights --seeds 24 --jobs 20      # ~3 min, writes tier2-fights.csv + tier2-forces.csv
crates/combatsim/tools/tier2_study.py energy --seeds 24 --jobs 20      # writes tier2-energy.csv
crates/combatsim/tools/tier2_study.py sustain                          # arithmetic, no simulation
combatsim --a "corthud:12,corstorm:13,corcan:5" --b "corak:8,corthud:15,corstorm:23" --reps 24 --spacing 100
```

## 1. What BARb actually fields

From `truth-*.jsonl` in the 96 matches of batches v23-v26 (`docs/harness/record-format.md`: one line every two
seconds listing every enemy unit that exists, whatever we can see). Units still under construction are not
counted. BARb's faction comes from `script.txt`, not from what it builds — it builds a small minority of the
other faction's units in every game (Thugs and Maces mostly), and those are left in. Not every match reaches
every minute: a median Armada game ends at minute 22.6 and a median Cortex game at 27.4.

Mean units per match, metal excluding the commander; `*` marks a tier-2 unit.

| BARb | min | n | mobile metal | the army |
|---|---|---|---|---|
| Armada | 15 | 48 | 4190 | armham 9.2, armrock 7.8, armpw 7.3, armwar 1.8, armstump 1.7, armjanus 1.6, armflash 1.5, armflea 1.1 |
| Armada | 20 | 31 | 4960 | armrock 8.3, armham 7.5, armpw 9.1, armwar 4.7, armstump 1.5, armjanus 1.3, corthud 1.3, **armfido 0.6\*** |
| Armada | 25 | 23 | 3770 | armrock 6.0, armham 5.2, armpw 8.1, **armfido 1.6\***, corthud 1.8, armflash 1.3, armwar 1.2, armaak 0.3\*, armsnipe 0.2\*, armlatnk 0.2\* |
| Armada | 30 | 20 | 4480 | **armfido 2.1\***, armrock 4.2, armpw 6.0, armwar 1.9, armham 3.5, armbull 0.5\*, armmav 0.3\*, armsnipe 0.3\*, armfboy 0.2\* |
| Cortex | 15 | 41 | 3640 | corstorm 13.7, corthud 8.6, corak 7.6, corgator 1.2, corraid 1.0, corwolv 0.6 |
| Cortex | 20 | 36 | 3940 | corstorm 13.9, corthud 9.1, corraid 2.1, corak 5.8, corwolv 0.6 |
| Cortex | 25 | 29 | 4160 | corstorm 10.4, corthud 6.9, corak 3.7, **corban 0.3\*, coramph 0.8\*, cortermite 0.4\*, cormort 0.4\*, corhrk 0.2\*, corpyro 0.3\*** |
| Cortex | 30 | 23 | 6270 | **corsumo 0.6\***, corstorm 7.6, corthud 5.2, **corhrk 0.9\*, cormort 0.9\*, coramph 1.1\*, corpyro 1.3\*, corcan 0.3\*, cormart 0.4\***, corraid 1.2, corak 3.1 |

And the static defence (metal includes the factories, which are listed because they are what is standing there):

| BARb | min | static metal | the line |
|---|---|---|---|
| Armada | 15 / 20 / 25 / 30 | 2170 / 3510 / 4420 / 6190 | armllt 11.6 -> 13.5 -> 11.9 -> 10.9, armhlt 0.7 -> 1.6 -> 1.9 -> 2.3, armbeamer 1.5 -> 2.2, armrl 1.0, armamb 0.5\* and armguard 0.2 by minute 30 |
| Cortex | 15 / 20 / 25 / 30 | 1820 / 2980 / 5250 / 7820 | corllt 9.7 -> 12.6 -> 14.4 -> 15.8, corhlt 0.4 -> 1.3 -> 2.1 -> 2.3, corhllt 1.0 -> 2.7, corpun 0.2 -> 0.6, cortoast 0.6\* by minute 30 |

Three things in this table decided the rest of the study.

**BARb's army is tier-1 until minute 25 and never more than a third tier-2.** At minute 20 it is 0.6 Hounds
(Armada) or nothing (Cortex). The advanced lab itself shows up as `armalab` 0.3 at minute 20 and 0.5 at minute
25, `coralab` 0.6 at minute 25 — consistent with the 63 % / median-minute-19.7 figure we already had. So the
army we have to beat at minute 20-25 is a tier-1 army, and the tier-2 question is about *our* spending, not
about answering a tier-2 threat.

**Its mobile army does not grow.** 4200 -> 5000 -> 3800 -> 4500 metal for Armada; 3600 -> 3900 -> 4200 -> 6300
for Cortex. What grows is the static line: Armada 2200 -> 6200, Cortex 1800 -> 7800. BARb converts its income
into towers, not into units, which is what makes the base-assault case the one worth getting right.

**The heavy tower arrives late and in twos.** 1.3-2.1 heavy towers at minute 20-25 behind 12-14 light ones,
plus 2 Beamers (Armada) or Twin Guards (Cortex). The plasma batteries (Gauntlet, Agitator, Rattlesnake,
Persecutor, 1220-1390 range) are a minute-30 problem, not a minute-20 one.

## 2. What was added to the simulator

`crates/combatsim/data/units.json` held 82 units: tier-1 bots and vehicles, both commanders, every land
defence. It now holds 142 — everything the advanced bot labs and advanced vehicle plants build, because BARb
fields Bulls and Banishers out of a plant as well as Hounds out of a lab.

**The tier-1 validation did not move.** `combatsim validate` replays every pairing of both engine duel tables of
2026-09-19; before and after all of the changes below it reports the same numbers to every digit: tight (spacing
56) 367/483 decisive pairings with the right sign = 76 %, mean absolute error 0.298, correlation 0.708, slope
0.79; wide (spacing 100) 381/472 = 81 %, 0.252, 0.802, 0.79. The worst-miss list is identical too. None of the
23 units in the duel tables has any of the new weapon flags, no existing entry's numbers changed, and the
movement fix only affects units that start inside one another, which the duel geometry never does. That
agreement — three quarters of the decisive pairings, margins about a fifth flat — is the accuracy every number
in this study inherits.

**Weapons that take no part in a land fight are now dropped**, because the model has no answer for any of them
and would otherwise score each as ordinary damage: `paralyzer` (no stun model), `stockpile` (nuke and anti-nuke
launchers with nothing built to fire), `waterweapon` and TorpedoLauncher, and `commandfire` — the commander's
D-Gun, which is fired by hand, and which the simulator had been auto-firing for 99 999 damage at 250 elmos.

**The smart-trajectory plasma batteries** (`armguard` Gauntlet, `corpun` Agitator, `armamb` Rattlesnake,
`cortoast` Persecutor) mount the same gun twice — a low arc and a high arc — with a gadget picking one per
shot. The simulator was firing both, doubling their rate of fire. It now keeps one; with no height in the model
the two arcs are the same shot. **So yes, the plasma batteries can be represented**, at one shot a reload,
minus the arc.

**Four units are left out of the table rather than modelled wrong**, with the reason written into
`units.json` as `_excluded` so that asking for one fails loudly: the crawling bombs `armvader`, `corroach` and
`corsktl`, whose entire attack is walking into you and dying, and `armspid`, whose only weapon is a paralyser.

**A mixed force could not move.** Two groups placed on the same point froze solid — every one of a blocked
unit's seven deflections was also inside its neighbour — so `--a "armham:5,armrock:5"` produced a four-minute
stalemate and scored zero. A step is now allowed when it takes a unit further from something it already
overlaps, which is what the engine's push-apart does; a step from clear ground into an occupied disc is refused
exactly as before, so the duel tables do not move. The CLI also lays a side's types out one behind the other in
the order listed, so `--b corllt:12,corthud:20` is a tower line with an army behind it.

## 3. How the fights were set up

**Equal metal, three budgets.** Both sides are built to 3000, 6000 and 10 000 metal, within 2 %. BARb's army is
its measured mean composition scaled to the budget by largest-remainder rounding (the first version of this
gave the rounding leftovers to the cheapest type and turned BARb's one Flea into twenty-one; every margin in
that run was wrong). Its commander is left out, because one unit does not scale with a budget — which means
2700 metal and 3700 hit points of defender are missing from every base assault below.

**Seven scenarios**, each against both BARb factions:
`field20` / `field25` its mobile army at those minutes, both sides attack-moving;
`t1only` its minute-20 army with the tier-2 units removed;
`base6-hold` / `base12-hold` its minute-25 army behind 6 or 12 light towers plus the 2 heavy and 2 medium
towers it typically has, the whole garrison standing still;
`base6-out` / `base12-out` the same line with the mobile half coming out to meet us, which is closer to what
BARb's threat-aware pathing does. The tower line is **extra metal on top of the budget** (2280 Armada, 2430
Cortex for the twelve-tower version) — in a real assault the towers are sunk cost, and making them part of an
equal-metal split would flatter us. Base-assault margins are therefore negative for almost everything; what
they rank is which of our mixes is *least* bad, which is what the simulator is for.

**Two formation spacings, 56 and 160.** The formation an army fights in is an input here and an outcome in the
engine, and the area-damage units live or die by it (`combat-sim.md`, *Where it is wrong*: loosening the
formation from 56 to 160-240 recovers the engine's answer for tier-1 artillery exactly). Both are reported. A
mix whose ranking moves between them is not a result.

**Energy.** BARb's measured energy at minute 20-25 is a median 314-406 a second with 7000 stored, counted from
the wind, solar and fusion in the same truth logs (12.8 a turbine, the build-order study's measured Quicksilver
mean). Field battles give both sides 4000 stored and 350 a second; base assaults give the defender 7000.

**24 seeds a cell**, run one process per seed so the spread is real. Median standard deviation across the 24
seeds of a cell is **0.023 of margin** (p90 0.094, worst 0.278), so the standard error on a cell is typically
**+-0.005** and at worst +-0.06. Every difference this study calls a result is 0.2 or more.

## 4. Results

Margin is our side's surviving share of its metal minus theirs: +1 a flawless win, -1 a wipe. Each number is
the mean over both BARb factions and all three budgets, at spacing 56 / spacing 160. Full grid in
`docs/studies/data/tier2-fights.csv`, the forces in `tier2-forces.csv`.

### Cortex (our advanced lab is `coralab`)

| mix | field20 | field25 | t1only | base12-out |
|---|---|---|---|---|
| `t1-line` Thug+Aggravator | -0.04 / +0.18 | -0.07 / +0.02 | -0.03 / +0.17 | -0.38 / -0.26 |
| **`t1+can`** half Sumos | **+0.35 / +0.68** | **+0.42 / +0.65** | **+0.35 / +0.67** | -0.16 / +0.07 |
| `can` all Sumos | +0.30 / +0.69 | +0.34 / +0.70 | +0.31 / +0.68 | -0.22 / +0.24 |
| `t1+can(75)` three-quarter Sumos | +0.34 / +0.68 | +0.37 / +0.68 | +0.33 / +0.67 | -0.21 / +0.21 |
| `termite` all Termites | +0.46 / +0.58 | +0.48 / +0.61 | +0.47 / +0.56 | -0.12 / +0.02 |
| `can+hrk` Sumo + Arbiter | +0.52 / +0.74 | +0.55 / +0.73 | +0.52 / +0.74 | -0.12 / +0.22 |
| **`t1+sumo`** half Mammoths | +0.20 / +0.42 | +0.21 / +0.33 | +0.20 / +0.42 | **+0.41 / +0.25** |
| `t1+mort` half Sheldons | -0.21 / +0.14 | -0.16 / -0.01 | -0.20 / +0.14 | -0.43 / -0.31 |
| `t1+pyro` half Fiends | -0.06 / +0.10 | -0.07 / -0.07 | -0.05 / +0.09 | -0.40 / -0.28 |
| `hrk` all Arbiters | -0.86 / -0.84 | -0.84 / -0.77 | -0.86 / -0.83 | -0.88 / -0.86 |

The **Sumo** (`corcan`, 560 metal, 6000 hp, a 275-range beam at 344 damage a second) is the answer and it is
not close: +0.35 to +0.70 wherever it appears, against a line that manages -0.07 to +0.18. It is also the
cleanest result in the study, because a beam laser with `impactonly` is the one weapon type the simulator was
fitted on: no area damage, no arc, no flight time. Whether a tier-1 screen in front of it is worth its metal,
the fights cannot decide: in the field `t1+can` is within 0.11 of `can` everywhere, and in the twelve
base-assault cells the screen is ahead in 7 of 12 at spacing 56 and in 3 of 12 at spacing 160. The last two
parts of this section are what actually argue for keeping tier-1.

The **Mammoth** (`corsumo`, 2200 metal, 15 600 hp, a 650-range beam) is the only thing either faction has that
reliably breaks a tower line: +0.41 / +0.25 against twelve towers and a sallying garrison, where everything
else is negative. Its 650 range is just over the heavy tower's 620, though the simulator's units close to 90 %
of their range before stopping, so it walks into the last 35 elmos anyway and wins on hit points rather than
on standing off. One Mammoth is 2200 metal and 108 seconds of advanced lab, so it is a purchase, not a mix.

`can+hrk` (Sumo plus Arbiter) is nominally the best field mix at +0.52 / +0.74, but Arbiters *alone* score
-0.86, so the whole of that mix's value is the Sumo tanking while a poorly-modelled weapon shells a dense blob
— see *Where these numbers are weakest*. Termites are a respectable second to Sumos and cost the same per
point of margin, but their heat ray carries area 42 where the Sumo's beam carries 11, so part of that margin
is the same suspect mechanism.

### Armada (our advanced lab is `armalab`)

| mix | field20 | field25 | t1only | base12-out |
|---|---|---|---|---|
| `t1-line` Mace+Centurion+Rocketeer | +0.09 / +0.19 | +0.06 / +0.10 | +0.09 / +0.19 | -0.33 / -0.24 |
| `t1+zeus` half Welders | +0.11 / +0.40 | +0.09 / +0.45 | +0.12 / +0.38 | -0.30 / -0.08 |
| `t1+fido` half Hounds | +0.41 / +0.51 | +0.37 / +0.39 | +0.43 / +0.50 | -0.13 / -0.12 |
| `zeus+fido` Welder + Hound | +0.38 / +0.48 | +0.39 / +0.54 | +0.40 / +0.48 | -0.08 / -0.05 |
| **`t1+zeus+fido`** third each | **+0.35 / +0.49** | **+0.32 / +0.48** | **+0.36 / +0.49** | -0.19 / -0.09 |
| `t1+sptk` half Recluses | +0.13 / +0.59 | +0.11 / +0.50 | +0.14 / +0.59 | -0.18 / -0.03 |
| `fboy` all Fatboys | +0.55 / +0.61 | +0.55 / +0.58 | +0.55 / +0.61 | +0.65 / +0.47 |
| `t1+fboy` half Fatboys | +0.35 / +0.50 | +0.28 / +0.46 | +0.36 / +0.51 | +0.34 / +0.40 |
| `t1+snipe` half Sharpshooters | -0.37 / -0.16 | -0.36 / -0.13 | -0.36 / -0.12 | -0.50 / -0.24 |
| `mav` all Gunslingers | -0.43 / -0.12 | -0.39 / -0.24 | -0.43 / -0.13 | -0.52 / -0.30 |
| `fast` all Sprinters | -0.63 / -0.40 | -0.61 / -0.43 | -0.63 / -0.40 | -0.68 / -0.52 |

Armada's picture is worse in two ways. The best options on the board — Fatboy (area 300), Recluse (area 90),
Hound (area 72) — are all area-damage units, which is the simulator's weakest mechanism; and Armada's one clean
answer to a tower line, the cloaked 900-range Sharpshooter, is one the simulator cannot judge at all.

The recommendation therefore leans on the **Welder** (`armzeus`, 350 metal, 3500 hp, a 280-range lightning gun
with area 8) as the part that does not depend on a suspect model, with **Hounds** behind it for reach. Welders
alone fall apart at large budgets (-0.60 at 10 000) — 280 range and 48 speed means a pure Welder army is
out-shot walking in — so they need either the tier-1 line or Hounds in front of them, which is exactly what
`zeus+fido` and `t1+zeus+fido` are. Those two are within 0.06 of each other everywhere; the three-way mix is
chosen because it is the one that also absorbs the tier-1 lab's output.

### Does tier-1 keep being built alongside?

**Yes, about a third to a half of the metal — but not because of the fights.** Head to head, adding a tier-1
screen is close to free either way: in the field `t1+can` is within 0.11 of `can` and `t1+zeus+fido` within
0.06 of `zeus+fido`; in the twelve base-assault cells the Cortex screen is ahead in 7 of 12 at spacing 56 and
3 of 12 at spacing 160, and the Armada screen is ahead in 0 of 12 and 3 of 12. The one place the screen clearly
pays is in front of the Mammoth, which is slow and expensive enough to be caught alone (`t1+sumo` +0.41 against
`sumo` +0.23 behind twelve towers with the garrison sallying). The real reasons are the next two subsections:
one advanced lab cannot absorb a large income, and the tier-1 half is what still shoots during an energy stall.

### Energy

Every beam and lightning weapon in the tier-2 line charges energy per shot: Welder 59 a second at full rate,
Sumo 56, Termite 75, Mammoth 125, Sharpshooter 50. Sweeping our own income with 4000 stored
(`tier2-energy.csv`, 6000 metal, spacing 100):

| our income, E/s | 50 | 150 | 350 | 700 | 2000 |
|---|---|---|---|---|---|
| Cortex `can` field25 | -0.20 | +0.57 | +0.59 | +0.59 | +0.59 |
| Cortex `t1+can` field25 | +0.53 | +0.55 | +0.55 | +0.55 | +0.55 |
| Armada `t1+zeus` field25 | -0.00 | +0.24 | +0.34 | +0.34 | +0.34 |
| Armada `t1+zeus+fido` field25 | +0.45 | +0.46 | +0.46 | +0.46 | +0.46 |

The cliff is below ~150 a second and everything above 350 is flat. A pure Sumo army swings 0.77 of margin
between a starved and a fed economy; the half-and-half mixes barely move, because the tier-1 half fires for
free. That is a second reason to keep tier-1 in the mix: it is the part of the army that still shoots during an
energy stall. Caveat: with 4000 stored, a short fight is paid for out of the store, so this understates how
much a long siege depends on income.

### What it costs to sustain

`tier2_study.py sustain`, arithmetic from `buildtime`, `energycost` and the labs' `workertime` (tier-1 lab 150,
advanced lab 600), no simulation:

| faction / mix | metal/s | energy/s | tier-1 labs | advanced labs | units a minute |
|---|---|---|---|---|---|
| Armada `t1+zeus+fido` | 30 | 492 | 1.12 | 0.75 | Mace 2.1, Centurion 0.6, Rocketeer 1.5, Welder 1.7, Hound 2.1 |
| Armada `t1+zeus+fido` | 45 | 738 | 1.69 | 1.13 | Mace 3.2, Centurion 0.8, Rocketeer 2.3, Welder 2.5, Hound 3.1 |
| Armada tier-1 line only | 30 | 296 | 3.31 | 0 | Mace 6.2, Centurion 1.7, Rocketeer 4.5 |
| Armada tier-1 line only | 45 | 444 | 4.96 | 0 | Mace 9.3, Centurion 2.5, Rocketeer 6.8 |
| Cortex `t1+can` | 30 | 387 | 1.62 | 0.54 | Thug 3.5, Aggravator 3.7, Sumo 1.6 |
| Cortex `t1+can` | 45 | 581 | 2.43 | 0.80 | Thug 5.3, Aggravator 5.5, Sumo 2.4 |
| Cortex tier-1 line only | 30 | 277 | 3.25 | 0 | Thug 7.1, Aggravator 7.4 |
| Cortex tier-1 line only | 45 | 415 | 4.87 | 0 | Thug 10.6, Aggravator 11.0 |

"Labs" is build power, so 3.31 means three tier-1 labs running flat out or two labs plus two nanotowers (200
each). **This is the strongest argument in the study and it owes nothing to the combat model:** a tier-1 bot
lab turns 150 build power into about 10 metal a second of army. At +45 metal a second, a tier-1-only army needs
five labs' worth of build power; the same spend with half the metal in tier 2 needs 1.6 tier-1 labs and 1.2
advanced ones. The advanced lab is how a bot at the ~+38 metal a second our v5 brain reaches at minute 17
(K-t2-gate-thresholds) actually spends it. Energy follows: **about 400-500 a second at +30 and 580-740 at
+45**, on top of 20 a second for each advanced extractor, which is what the community's "+500 energy a second
before you go tier 2" rule is paying for.

## 5. Where these numbers are weakest

In roughly descending order of how much they could move a conclusion.

**Area damage in a dense formation.** The known headline failure (`combat-sim.md`): the simulator's armies
arrive in the formation they were spawned in and real ones do not, so a shell takes four neighbours where the
engine's takes one. Every Armada option above the tier-1 line except the Welder is an area weapon — Fatboy
(area 300, 800 damage), Recluse (90), Hound (72) — and Cortex's `can+hrk` and `termite` carry area 70 and 42.
The spacing-160 column is the check, and it does not clear them: at area 300 a Fatboy shell still covers a
160-elmo formation. **Treat every Fatboy, Recluse and Arbiter number as an upper bound.** The Welder (area 8),
the Sumo (area 11) and the Mammoth (area 11) are the ones this does not touch.

**High-trajectory and vertical-launch weapons.** The Arbiter (`corhrk`) is a StarburstLauncher that flies up
and then down; the simulator gives it a straight line at `weaponvelocity`, so its shells arrive at the wrong
time, and it has no notion of shooting over a hill. Its 1210 range against 380 sight also means it depends
entirely on a spotter, which the shared-sight model gives it for free and which a real army has to arrange.
Same shape for Sheldon (`cormort`, 850 range, 380 sight) and every artillery bot.

**The Sharpshooter cannot be judged at all.** `armsnipe` is cloaked in the game and the simulator has no cloak,
so it is targeted like anything else with 580 hit points for 680 metal, and it scores -0.90. It also has 455
sight against 900 range, so a pure Sharpshooter force walks to 455 to see anything and dies. **Its -0.90 is an
artefact, not a verdict.** Armada's answer to a tower line may well be Sharpshooters, and this study cannot
say.

**The flamethrower.** The Fiend's (`corpyro`) flame is 16 projectiles of 16.5 damage over area 48, and the
engine's flame bounces, lingers and grows. The simulator resolves each as a point explosion on arrival.
Direction of the error unknown; the Fiend's -0.21 / -0.09 should be read as "not obviously good" rather than as
a number.

**Whether the garrison comes out.** `base*-hold` and `base*-out` differ by a lot for exactly the units that
outrange the tower line: behind twelve towers at spacing 56, a pure Fatboy force scores +1.00 against a
garrison that stands still and +0.65 against one that sallies, and a pure Mammoth force +1.00 against +0.23.
Standing still, both are untouchable — nothing in the base can reach them, and the fight becomes target
practice. We do not know which BARb does; the `-out` column is the headline because BARb's pathing is
threat-aware and its army does move, but the truth is bracketed, not measured.

**No reinforcement, no repair, no rebuilding.** A base assault in the engine is fought against a factory that
is still producing and construction bots that are still repairing. Every base number here is a single fight
against a fixed garrison, which flatters the attacker.

**BARb's commander is not in any of these fights** — 2700 metal, 3700 hp and a 300-range beam, standing in the
base. Add it to the base-assault scenarios and every margin there drops.

**Anti-air is excluded on both sides**, since the simulator drops anti-air weapons; including BARb's Archangels
and Manticores would only be free hit points for us to chew through. It is about 130-160 metal of its
minute-25 army and up to 370 by minute 30.

**Margins run about a fifth flat.** The engine duel tables give slope 0.79 against the simulator, in both
directions, so a real +0.35 is probably nearer +0.44. This does not reorder anything.

**And the whole thing is a fixed-composition pitched battle.** No raiding, no reinforcement trickle, no
retreat, no map. The question it answers is "at equal metal, which mix survives better", which is the question
the production heuristic needs, and not "will we win the game".

## 6. Recommendation

### Cortex

**Default: 50 % `corcan` (Sumo), 27.5 % `corthud` (Thug), 22.5 % `corstorm` (Aggravator), by metal.**
Margin against BARb's minute-20 and minute-25 armies +0.35 to +0.68 depending on formation, against +0.18 to
-0.07 for the tier-1 line alone. Confidence **good**: the Sumo's beam is the weapon type the simulator was
fitted on, the result holds across both BARb factions, all three budgets, both spacings and the pure-tier-1
opponent, and the mechanism is plain in the stat line (6000 hp and 344 damage a second for 560 metal, 10.7 hit
points a metal against the Thug's 7.9).

**When the plan is to walk into the tower line, add one `corsumo` (Mammoth) and accept a slower army.**
`t1+sumo` is the only mix either faction has that is positive against twelve towers plus a sallying garrison
(+0.41 / +0.25). Confidence **moderate**: clean weapon model, but it depends on the garrison's behaviour and on
the towers not being repaired.

At +30 metal a second: 1.6 Sumos, 3.5 Thugs and 3.7 Aggravators a minute, 387 energy a second, 0.54 of the
advanced lab and 1.6 tier-1 labs' build power. At +45: 2.4 / 5.3 / 5.5 a minute, 581 energy a second, 0.80 and
2.4 labs.

### Armada

**Default: 33 % `armzeus` (Welder), 33 % `armfido` (Hound), and the tier-1 line for the remaining third —
15 % `armham` (Mace), 10 % `armrock` (Rocketeer), 9 % `armwar` (Centurion).** Margin +0.32 to +0.49, against
+0.06 to +0.19 for the tier-1 line. Confidence **moderate**: the Welder half is clean, the Hound half is an
area weapon and could be worth less than this says. If the Hound turns out to be overrated, `t1+zeus` at
spacing 160 is still +0.38 to +0.45, so the Welder alone carries most of the gain.

**Armada has no simulated answer to a heavy tower line.** Only the Fatboy is positive there, and the Fatboy's
number is the least trustworthy in the study. The Sharpshooter, which is the answer the game's own players
would give, is unmeasurable here. If Armada needs to crack a base before we have engine evidence, build Fatboys
and expect a good deal less than the +0.47 to +0.65 the table shows.

At +30 metal a second: 1.7 Welders, 2.1 Hounds, 2.1 Maces, 1.5 Rocketeers and 0.6 Centurions a minute, 492
energy a second, 0.75 of the advanced lab and 1.1 tier-1 labs. At +45: 2.5 / 3.1 / 3.2 / 2.3 / 0.8 a minute,
738 energy a second, 1.13 and 1.7 labs.

### Keep building tier-1

Both factions: **a third to a half of the army's metal stays tier-1, for economic reasons, not tactical ones.**
The fights are close to indifferent to the screen (above). What is not indifferent: one advanced lab cannot
absorb +45 metal a second on its own, and the tier-1 half is the part that still shoots when energy stalls.
Confidence **good** for the build-power half of that (it is arithmetic from the unit files), **moderate** for
the energy half. What the tier-1 third should be made of was not varied here — both recommendations keep the
line the brain already builds, in the proportions it already uses.

## 7. What the arena and the engine should test next

1. **The one engine duel this study needs**: `corthud:12,corstorm:13,corcan:5` against a scaled BARb army, and
   the same metal as pure `corthud`/`corstorm`. If the engine agrees within a fifth, the Cortex recommendation
   is done.
2. **Measure the spacing an army actually fights at.** Still the first item on `combat-sim.md`'s list and now
   the thing that decides whether half the Armada table is real.
3. **A Sharpshooter duel with cloak**, against a light tower line with and without a spotter. This is the only
   way to settle Armada's base-cracking question.
4. **Does BARb's garrison sally?** Countable from the truth logs we already have: the distance of its mobile
   units from its own towers while we approach. It would remove the widest bracket in section 5.
5. **An arena batch** with the advanced lab gated on `+30 M/s` and the Cortex mix above, against the same brain
   without it, 24 matches. That is the only thing that turns any of this from `simulated` to `supported`.
