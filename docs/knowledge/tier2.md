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
