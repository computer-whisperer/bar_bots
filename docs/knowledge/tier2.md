# Tier 2: when, how, and what it buys

Seeded 2026-09-19. "Local source" = `upstream/Beyond-All-Reason`; unit paths relative to it. Scope: 1v1 land, bot labs.

### K-t2-entry-cost
**Claim.** Entering tier 2 through bots costs ~3030-3070 metal before any benefit: advanced bot lab 2600 M / 15-16k E
(buildtime 25000-26000) plus one advanced constructor bot 430 M (Armada) / 470 M (Cortex), 6900 E. Each advanced extractor
is a further 620 M / 7700 E (Armada) or 640 M / 8100 E (Cortex). Web guides quote "roughly three thousand metal" (and one
says ~4000 including the first extractors), which matches.
**Status.** supported (2026-09-19) for costs (local source); web figure reported
**Evidence.** `units/ArmBuildings/LandFactories/armalab.lua`, `CorBuildings/LandFactories/coralab.lua`,
`ArmBots/T2/armack.lua`, `CorBots/T2/corack.lua`, `ArmBuildings/LandEconomy/armmoho.lua`, `cormoho.lua`.
https://www.crdhq.com/articles/bar-t2-timing-rules-and-juno-missile-guide; https://www.beyondallreason.info/guide/in-depth-look-at-economy.
**Would be wrong if.** (numbers) the build menu showed other costs in our game version.
**Used by.** (candidate: H-T2-GATE)

### K-t2-who-builds-what
**Claim.** The advanced bot lab is built by a T1 constructor (it is in the constructor bot's build list, not the
commander's). Advanced extractors, fusion and advanced converters are only in the advanced constructor's list. So the
chain is: T1 constructor → advanced lab → advanced constructor → advanced extractors.
**Status.** supported (2026-09-19) — local source
**Evidence.** `units/ArmBots/armck.lua:48` (`armalab` in buildoptions); `units/ArmBots/T2/armack.lua:40,46` (`armfus`,
`armmoho`); `units/armcom.lua` has neither.
**Would be wrong if.** The AI interface refused `armalab` for a constructor bot or offered it to the commander.
**Used by.** (candidate: H-T2-GATE assigns the lab to constructors and has the commander assist)

### K-t2-gate-thresholds
**Claim.** Community rule of thumb for starting the advanced lab in 1v1: income of at least +30 M/s and +500 E/s; or +20 M/s
and +500 E/s with 1000-1500 metal banked; otherwise stay tier 1, because a transition the economy cannot carry idles the
factories and loses to a tier-1 push. Deciding question: "would 3000 more metal of T1 units kill the opponent sooner?"
**Status.** reported (2026-09-19)
**Evidence.** https://www.crdhq.com/articles/bar-t2-timing-rules-and-juno-missile-guide (2026, undated; aggregator of
community advice); the official economy guide gives the softer "+20 metal and +500 energy, 1000-2000 metal stored":
https://www.beyondallreason.info/guide/in-depth-look-at-economy.
**Would be wrong if.** A brain that started the advanced lab at +20 M/s without a bank did as well against BARb medium as
one that waited for +30 M/s / +500 E/s.
**Used by.** (candidate: H-T2-GATE — our v5 brain reaches ~+38 M/s at minute 17, so by this rule it should have gone T2
around minute 10-12 when it crossed +20-30; its energy income at that time is unknown and must be logged)

### K-t2-needs-build-power
**Claim.** The advanced lab is a build-power problem as much as a metal problem: buildtime 25000 means 312 s for one
constructor bot (80), 83 s for the commander alone (300), ~54 s for commander + 2 constructors (460), at which point it
drains ~48 M/s and ~280 E/s. Without banked metal or assisting builders it either takes minutes or stalls everything else.
**Status.** supported (2026-09-19) — arithmetic from local unit files; not observed
**Evidence.** `armalab.lua` (buildtime 25000, metalcost 2600, energycost 15000); workertime of `armcom.lua` 300, `armck.lua` 80.
**Would be wrong if.** A measured advanced-lab build with known builders finished >20% off buildtime/Σworkertime while not stalled.
**Used by.** (candidate: H-T2-GATE — commander plus ≥2 constructors (or 2 construction turrets in range) assigned; require
~1000 M banked or accept a 2-minute build)

### K-t2-moho-first
**Claim.** The first thing tier 2 buys is advanced extractors on the safest existing spots: 4x yield (extractsmetal 0.004 vs
0.001) for 620-640 M, i.e. +3 extractor-equivalents per upgrade, and 2800-3900 health vs 270. With a T1 extractor at about
+2 M/s on Quicksilver (inferred from v5-probe: +38 M/s with 18 extractors) one upgrade adds ~+6 M/s and repays its metal in
~100 s; five upgrades roughly double our whole T1 income. An advanced constructor (210-220 build power) needs ~70 s each.
**Status.** supported (2026-09-19) for costs/yield ratio (local source); per-spot income and payback are inference
**Evidence.** `armmoho.lua` (extractsmetal 0.004, metalcost 620, energyupkeep 20, health 2800), `cormoho.lua` (640 M, 3900 hp),
`armmex.lua` (0.001, 270 hp). Upgrade-first advice: https://www.beyondallreason.info/guide/in-depth-look-at-economy;
upgrade command: https://www.beyondallreason.info/commands/upgrade-t1-mex. See also K-rules-mex-upgrade-in-place.
**Would be wrong if.** Metal income did not rise by ~3x the old spot's yield when an upgrade finished, or upgrades were
lost to raids faster than they repaid (~100 s).
**Used by.** (candidate: H-T2-MOHO — advanced constructors upgrade spots nearest home first, 2 advanced constructors)

### K-t2-moho-energy
**Claim.** An advanced extractor draws 20 E/s (T1: 3 E/s) and costs 7700-8100 E to build; ten upgrades need +200 E/s upkeep
plus ~110 E/s while building each. The +500 E/s gate in K-t2-gate-thresholds is what pays for this.
**Status.** supported (2026-09-19) — local source
**Evidence.** `armmoho.lua` / `cormoho.lua` (energyupkeep 20, energycost 7700/8100, buildtime 14900/14100).
**Would be wrong if.** Energy usage did not step up by ~20 per finished upgrade.
**Used by.** (candidate: H-T2-MOHO checks stored energy > 50% before each upgrade)

### K-t2-fusion-numbers
**Claim.** Fusion: Armada 3350 M / 18000 E → +750 E/s; Cortex 3600 M / 22000 E → +850 E/s; buildtime 54000-59000 (257-268 s
for one advanced constructor, so it needs assistance). Metal per E/s: fusion ~4.2-4.5, advanced solar 4.4-4.6
(350-370 M → 80 E/s), solar 7.5-7.75, wind 40/avg-wind (3.1 on Quicksilver). Fusion is therefore not cheaper energy per
metal than wind on a windy map or than advanced solar; its value is density, steadiness and builder time. It follows the
extractor upgrades rather than preceding them.
**Status.** supported (2026-09-19) for numbers (local source); the ordering advice is reported
**Evidence.** `armfus.lua`, `corfus.lua`, `armadvsol.lua` (energymake 80; the official guide says 75 — local wins),
`coradvsol.lua`. Ordering: https://www.beyondallreason.info/guide/in-depth-look-at-economy.
**Would be wrong if.** (numbers) unit files change. (ordering) a fusion-before-upgrades brain had higher metal income at minute 15.
**Used by.** (candidate: no fusion before ≥6 advanced extractors; on Quicksilver keep adding wind instead)

### K-t2-advanced-converter
**Claim.** The advanced converter turns 600 E/s into ~10.3 M/s (58 E per metal) for 370-380 M / 21000 E; the T1 converter
turns 70 E/s into 1.0 M/s (70 E per metal) for 1 M / 1150-1250 E. T2 conversion is only ~17% more energy-efficient; it is
a space/cost-per-capacity gain, not a reason to go T2.
**Status.** supported (2026-09-19) — local source
**Evidence.** `armmmkr.lua` (energyconv_capacity 600, efficiency 0.01724), `armmakr.lua` (70, 0.01429), Cortex equivalents.
**Would be wrong if.** Measured metal from converters differed from capacity × efficiency when energy was above the threshold.
**Used by.** (none)

### K-t2-unlock-army
**Claim.** Besides economy, guides give one military reason to go T2 even with a borderline economy: the opponent is dug in
behind static defence that T1 cannot break. For bots the relevant unlocks (ranges from local files) are Cortex Sheldon (850)
and Arbiter (1210), Armada Hound (650) and Sharpshooter (900), all outranging the heavy laser tower (620); only the
plasma batteries Gauntlet/Agitator (1220/1245) out-range them.
**Status.** reported (2026-09-19) for the advice; ranges supported from local source
**Evidence.** https://www.crdhq.com/articles/bar-t2-timing-rules-and-juno-missile-guide;
https://www.crdhq.com/articles/when-to-transition-to-t2-and-when-to-resign. `units/CorBots/T2/cormort.lua`, `corhrk.lua`,
`ArmBots/T2/armfido.lua`, `armsnipe.lua`, `ArmBuildings/LandDefenceOffence/armhlt.lua`, `armguard.lua`, `corpun.lua`.
**Would be wrong if.** BARb medium's base fell to T1 waves once our economy matched its own (then T2 army is unnecessary).
**Used by.** (candidate: after upgrades, advanced lab batch = Sheldon/Hound-class skirmishers behind a T1 screen)

### K-t2-barb-goes-tier-2
**Claim.** BARb medium starts a tier-2 factory in about two games in three, never before minute 15, median just before
minute 20; a game of ours that is not over by then is tier 1 against tier 2.
**Status.** measured (2026-09-20), ground truth from 96 heuristic games (v23-v26), Quicksilver only.
**Evidence.** 60 of 96 games; first advanced factory at 15.5 / 19.7 / 37.4 minutes (earliest / median / latest), mostly
advanced bot labs. Of games lasting 20+ minutes (67) it had one by minute 20 in 27; of 25+ (52), 39; of 30+ (44), 37.
NW losses 24 of 38 (median loss minute 22); SE timeouts 16 of 18; SE wins 12 of 29. Commander game 8c's attack met
corsumo and corcan at minute 32.
**Would be wrong if.** Other maps or profiles showed a different clock.
**Used by.** H-T2-GATE (why at all), the commander's brief.

### K-t2-upgrades-starve-without-help
**Claim.** With nothing banked, every consumer gets a share of the metal income, and an extractor upgrade built by one
advanced constructor beside two working labs takes minutes instead of its nominal 70 s.
**Status.** observed once (2026-09-20), t2-first-look match 00.
**Evidence.** Lab ordered minute 10, finished 13 (commander and constructors helping); first armmoho created minute 16,
finished minute 22, with metal at 0 banked, +35-40 income, fully spent, two labs and nine constructors drawing on it.
**Would be wrong if.** With two helpers and the advanced lab idle the first upgrades still took over two minutes.
**Used by.** H-T2-ASSIST (upgrade helpers), H-T2-PRODUCTION (no soldiers from the advanced lab before four upgrades).

**Simulated check (2026-09-20).** Partly contradicted; see K-t2-mix-cortex, K-t2-mix-armada and K-t2-tower-line-needs-mass.
Out-ranging the tower is not enough on its own: Sheldon and Arbiter armies are the two worst Cortex mixes in the sweep
(-0.58 and -0.79 over all scenarios) because they have no health, and only the Mammoth — which barely out-ranges the
tower at all — is reliably positive against a tower line. The Sharpshooter could not be judged (K-t2-sim-cannot-judge).

### K-t2-barb-composition
**Claim.** BARb medium's army at minutes 20 and 25 is a tier-1 army. Mean per match over 96 recorded games (v23-v26),
metal excluding its commander: as Armada 4960 M at minute 20 (armrock 8.3, armham 7.5, armpw 9.1, armwar 4.7, armstump 1.5,
armjanus 1.3) and 3770 M at minute 25, of which the only tier-2 unit worth counting is armfido (0.6 then 1.6); as Cortex
3940 M at minute 20 (corstorm 13.9, corthud 9.1, corak 5.8, corraid 2.1) with no tier-2 at all, and 4160 M at minute 25
with about 500 M of it tier-2 (coramph, cortermite, cormort, corban, corhrk, corpyro, all under one unit a game).
Its **mobile army does not grow** across minutes 15-30 (Armada 4190 -> 4960 -> 3770 -> 4480, Cortex 3640 -> 3940 -> 4160 ->
6270) while its **static line more than triples** (Armada 2170 -> 6190, Cortex 1820 -> 7820 including its factories).
Light towers 12-14 at minute 20-25; heavy towers only 1.3-2.1, plus 2 Beamers or Twin Guards; the 1220+ range plasma
batteries (armguard, corpun, armamb, cortoast) are a minute-30 thing, 0.2-0.6 a game.
**Status.** supported (2026-09-20) — counted from match logs, not arena-tested against a changed brain
**Evidence.** `docs/studies/data/tier2-barb-army.csv`, from `truth-*.jsonl` in `run/matches/*v2[3456]-*/NN`
(96 matches, 48 a faction; 31/36 reach minute 20, 23/29 minute 25). `crates/combatsim/tools/tier2_study.py army`.
Faction from `script.txt`; BARb builds a small minority of the other faction's units and those are counted as they are.
**Would be wrong if.** A later batch on another map or against a stronger brain showed its mobile army growing with its
income, or more than a third of its minute-25 metal in tier-2 units.
**Used by.** K-t2-mix-cortex, K-t2-mix-armada, K-t2-tower-line-needs-mass (all use this as the opponent).

### K-t2-lab-build-power
**Claim.** The advanced bot lab is 600 build power against the tier-1 lab's 150, and that — not the units — is the
first thing it buys. A tier-1 bot lab turns its 150 into about 10 metal a second of army (Thug 140 M / 2100 buildtime,
Mace 130 M / 2200). Sustaining +30 metal a second of the tier-1 line therefore needs 3.3 tier-1 labs' worth of build
power and +45 needs 5.0; the same spend with half the metal in tier-2 units needs 1.6 tier-1 labs plus 0.5-0.8 of the
advanced lab (+30) or 2.4 plus 0.8-1.1 (+45). A nanotower adds 200.
**Status.** supported (2026-09-20) — arithmetic from local source, not observed
**Evidence.** `armlab.lua`/`corlab.lua` workertime 150, `armalab.lua`/`coralab.lua` workertime 600; buildtimes
armzeus 8000, armfido 6500, corcan 12000, corsumo 65000, corthud 2100, corstorm 1950, armham 2200, armrock 2010.
`crates/combatsim/tools/tier2_study.py sustain`.
**Would be wrong if.** A recorded game showed a single tier-1 lab sustaining much more than ~10 M/s of bots without
assistance, or the advanced lab finishing units far off buildtime/600.
**Used by.** (candidate: H-T2-GATE and the production heuristic — at +38 M/s our brain cannot spend through two tier-1 labs)

### K-t2-mix-cortex
**Claim.** Out of a `coralab`, build **Sumos (`corcan`)** for about half the army's metal and keep Thug and Aggravator
for the other half (50 % corcan / 27.5 % corthud / 22.5 % corstorm). At equal metal against BARb's measured minute-20
and minute-25 armies this scores +0.35 (dense formation) to +0.68 (loose) of margin, where the tier-1 line alone scores
-0.07 to +0.18. It holds at 3000, 6000 and 10 000 metal, against both of BARb's factions, and against a pure tier-1
BARb army. The Sumo is 560 M for 6000 health and 344 damage a second at 275 range — 10.7 health a metal against the
Thug's 7.9 — and its beam is the one weapon type the simulator was fitted on. The tier-1 half is kept for reasons
outside the fights (K-t2-lab-build-power and K-t2-army-energy); head to head a pure Sumo army is within 0.11 of the
mix in the field and ahead of it in 9 of 12 base-assault cells at the loose formation spacing.
**Status.** simulated (2026-09-20); never run in the arena
**Evidence.** `docs/studies/tier2-army.md` section 4, `docs/studies/data/tier2-fights.csv` (3780 cells, 24 seeds each,
median seed sd 0.023, typical standard error 0.005). Termite is a close second (+0.46/+0.58) and `can+hrk` scores higher
still (+0.52/+0.74) but leans on a weapon the simulator models badly (K-t2-sim-cannot-judge).
**Would be wrong if.** An engine duel of `corthud:12,corstorm:13,corcan:5` against the same metal of pure
corthud/corstorm, both against a scaled BARb army, failed to favour the Sumo mix by at least a fifth of the simulated
margin; or an arena batch with the mix did no better than one without.
**Used by.** H-T2-PRODUCTION's Cortex line. Note for that rule: its placeholder pairs corcan with **cormort**
(Sheldon), and cormort is one of the two worst Cortex options in the sweep — `t1+mort` -0.21/+0.14 and `mort` alone
-0.58 over all scenarios, against `t1+can` +0.35/+0.68. Pair the Sumo with the tier-1 line instead, and with a
Mammoth when a tower line has to be broken.

### K-t2-mix-armada
**Claim.** Out of an `armalab`, build **Welders (`armzeus`) and Hounds (`armfido`)** for about a third of the army's
metal each and keep the tier-1 line for the last third. +0.32 to +0.49 of margin against BARb's minute-20 and
minute-25 armies, where the tier-1 line alone scores +0.06 to +0.19. The Welder is the part that does not depend on a
weapon the simulator models badly (280-range lightning, area 8, 3500 health for 350 M) but it cannot go alone: a pure
Welder army scores -0.60 at 10 000 metal because 280 range and 48 speed is out-shot on the way in. Gunslinger,
Sprinter and Platypus never beat the tier-1 line in any of the 54 field cells (closest: the Platypus at 3000 metal and
loose spacing, -0.01 behind). The tier-1 third is kept for the reasons in K-t2-lab-build-power and K-t2-army-energy,
not because the screen wins fights: `t1+zeus+fido` is within 0.06 of `zeus+fido` in the field and behind it in 21 of
the 24 base-assault cells.
**Status.** simulated (2026-09-20); never run in the arena
**Evidence.** `docs/studies/tier2-army.md` section 4, `docs/studies/data/tier2-fights.csv`. The Hound's half of this
rests on area damage (area 72), which the simulator over-rates in a dense formation, so the mix is reported with the
spacing-160 column beside the spacing-56 one; the ranking is stable between them.
**Would be wrong if.** An engine duel put a Welder-and-Hound mix at or below the tier-1 line at equal metal; or the
measured fighting spacing of a real army turned out to be tighter than 56, which would mean the Hound number is worse
than the study's own upper bound.
**Used by.** H-T2-PRODUCTION's Armada line, whose armzeus/armfido placeholder this supports at roughly one Welder to
one Hound by metal (1.7 Welders to 2.1 Hounds a minute at +30 M/s), behind a tier-1 third.

### K-t2-tower-line-needs-mass
**Claim.** At equal army metal, an assault on BARb's minute-25 base — 12 light towers, 2 heavy, 2 medium, its mobile
army behind them, all of it extra metal on top of ours — loses for almost every mix we could build. The one reliably
positive answer either faction has is the Cortex **Mammoth (`corsumo`)** behind a tier-1 screen: +0.41 (dense) / +0.25
(loose) against a garrison that comes out, where the tier-1 line is -0.38 / -0.26 and the Sumo mix -0.16 / +0.07.
Armada has no simulated answer that does not rest on the Fatboy's area-300 shell. Margins improve steeply with budget:
the same mixes are strongly negative at 3000 metal and positive at 10 000, so **the tower line is a mass problem
before it is a unit-choice problem**.
**Status.** simulated (2026-09-20); never run in the arena
**Evidence.** `docs/studies/data/tier2-fights.csv`, scenarios `base6-*` and `base12-*`. Tower line 2280 M (Armada) /
2430 M (Cortex) on top of the budget; defender energy 7000 stored and 350 a second, from BARb's own measured wind,
solar and fusion at minute 20-25 (median 314-406 a second, 7000 stored).
**Would be wrong if.** The garrison turns out to stand still rather than sally (the two brackets differ by 0.3-0.8 for
anything that out-ranges the towers), or repair and rebuilding during the assault change the answer, or BARb's
commander — 2700 M and 3700 health, left out of every one of these fights — is what actually holds the line.
**Used by.** (candidate: a wave gate that refuses a base assault below some multiple of the seen tower metal)

### K-t2-army-energy
**Claim.** A tier-2 bot army is an energy purchase as much as a metal one. Every beam and lightning weapon charges
energy a shot: Welder 59 a second at full rate, Sumo 56, Termite 75, Mammoth 125, Sharpshooter 50. Building the
recommended mixes costs about **390-500 energy a second at +30 metal a second and 580-740 at +45**, on top of 20 a
second for each advanced extractor. In the fights, performance falls off a cliff below roughly 150 energy a second of
spare income (a pure Sumo army swings 0.77 of margin between 50 and 150) and is flat above 350. A half tier-1 mix
barely moves, because the tier-1 half fires for free.
**Status.** simulated (2026-09-20) for the cliff; supported (local source) for the per-shot and build costs
**Evidence.** `docs/studies/data/tier2-energy.csv` (4800 fights, our income swept 50-2000 with 4000 stored);
`crates/combatsim/tools/tier2_study.py sustain`; `energypershot` in the unit files. This is the same mechanism as
K-units-laser-towers-need-energy and it supports the +500 E/s of K-t2-gate-thresholds from a second direction.
**Would be wrong if.** A recorded game showed a tier-2 army's damage output not dropping while energy was stalled.
Note the sweep gives 4000 stored, so a short fight is paid out of the store and this understates a long siege.
**Used by.** (candidate: H-T2-GATE's energy threshold; a production rule that falls back to tier-1 during an energy stall)

### K-t2-sim-cannot-judge
**Claim.** Four kinds of tier-2 unit the combat simulator cannot be trusted about, so their numbers in
`tier2-army.md` are not evidence. (1) **Area-damage units** — Fatboy (area 300), Recluse (90), Arbiter (70), Hound
(72), Termite (42): the simulator's armies fight in the formation they were spawned in, so a shell takes four
neighbours where the engine's takes one; their margins are upper bounds. (2) **Vertical-launch and high-trajectory
weapons** — Arbiter, Sheldon: flight time is computed as a straight line and there is no arc over terrain. (3) **The
Sharpshooter** — cloaked in the game, not in the simulator, and 580 health for 680 metal once it can be shot at; its
-0.90 is an artefact, and Armada's real answer to a tower line may well be Sharpshooters. (4) **The Fiend's
flamethrower** — the engine's flame bounces, lingers and grows; the simulator resolves each of its 16 projectiles as a
point explosion, error of unknown sign. Three crawling bombs and the EMP spider are excluded from the unit table
outright (`_excluded` in `crates/combatsim/data/units.json`).
**Status.** supported (2026-09-20) — property of the model, from `docs/studies/combat-sim.md` and the unit files
**Evidence.** `combat-sim.md` *Where it is wrong* (artillery is the worst miss in the tight duel table, and loosening
the formation to 160-240 recovers the engine's answer exactly); `crates/combatsim/tools/extract_units.py` for what is
excluded and why; unit files for cloak, `weapontype` and `areaofeffect`.
**Would be wrong if.** An engine duel reproduced a simulated area-damage margin within a fifth, or a measurement of
the spacing a real army fights at came out near 56.
**Used by.** K-t2-mix-armada, K-t2-tower-line-needs-mass (both report the caveat rather than the number alone).
