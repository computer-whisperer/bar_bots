# Units: roles, ranges and counters

Seeded 2026-09-19. Numbers are from the local unit files (`upstream/Beyond-All-Reason/units/…`, paths relative to `units/`)
and are `supported` as numbers in our game version; role and counter statements come from the web and are `reported`.
Per-shot damage and reload are as listed in the weapon def — burst/salvo size and the laser range falloff were not
checked, so treat derived DPS as rough. Internal names are given because the bot uses them.

### K-units-t1-bot-roster
**Claim.** Tier-1 bot lab roster (metal / health / speed / range):
Armada — Tick `armflea` 21/60/132/140 scout (sight 600); Pawn `armpw` 54/370/87/180 raider; Rocketeer `armrock`
120/720/51/475 skirmisher (157 dmg per 3.8 s, no anti-air); Mace `armham` 130/1000/46/380 plasma line unit; Centurion
`armwar` 270/1590/45/325 heavy brawler; Crossbow `armjeth` 125/630/56/760 anti-air only; Lazarus `armrectr` 130/220/78
resurrect/reclaim/repair, 200 build power; Constructor `armck` 110/760/36, 80 build power.
Cortex — Grunt `corak` 43/280/81/215 raider (sight 520); Aggravator `corstorm` 110/740/48/475 skirmisher; Thug `corthud`
140/1100/45/380 plasma line unit; Trasher `corcrash` 125/640/53/760 anti-air only; Graverobber `cornecro` 130/220/78
resurrect; Constructor `corck` 120/820/34.5, 85 build power. Cortex T1 bots have no scout and no heavy brawler.
**Status.** supported (2026-09-19) — local source
**Evidence.** `ArmBots/*.lua`, `CorBots/*.lua`; `ArmBuildings/LandFactories/armlab.lua:33-42` (build list); names from
`language/en/units.json`.
**Would be wrong if.** The AI interface's UnitDef values differed (defs are post-processed in `gamedata/alldefs_post.lua`).
**Used by.** H-PROD-BATCH (raider / skirmisher / artillery slots should map to these names)

### K-units-role-triangle
**Claim.** BAR counters are soft, by role: raiders (fast, cheap) kill undefended extractors and builders; skirmishers and
static defence beat raiders; artillery and long range beat static defence; raiders and fast units beat unescorted
artillery. Speed buys the choice of where to fight but loses the straight fight at equal cost; range buys free damage on
approach but costs health; health matters most when the enemy shoots first.
**Status.** reported (2026-09-19)
**Evidence.** https://masterbel2.wordpress.com/beyond-all-reason-units-guide-uses-tactics-and-counters/ (community coach,
work in progress, undated); https://www.crdhq.com/articles/bar-unit-guide (2026).
**Would be wrong if.** Arena logs showed equal-cost raider groups beating skirmisher groups head-on, or raiders failing to
kill unguarded extractors.
**Used by.** (candidate: H-ARMY-COMPOSITION — pick next unit by what the enemy was last seen fielding)

### K-units-rockets-outrange-llt
**Claim.** Rocketeer / Aggravator range 475 exceeds the light laser tower (Sentry 430, Guard `corllt` 435) and the
commander's laser (300) and D-gun (250), so a rocket-bot group held at ~450-470 kills light towers and extractors without
taking return fire. It does not outrange Beamer / Twin Guard (490) or the heavy laser tower Overwatch / Warden (620).
Mace / Thug (380) are inside LLT range.
**Status.** supported (2026-09-19) for ranges (local source); the tactic is reported and untested
**Evidence.** `ArmBots/armrock.lua`, `CorBots/corstorm.lua`, `ArmBuildings/LandDefenceOffence/armllt.lua` (430), `corllt.lua`
(435), `armbeamer.lua` (490), `corhllt.lua` (490/400), `armhlt.lua`, `corhlt.lua` (620), `units/armcom.lua` (300, 250).
**Would be wrong if.** Rocket bots ordered to attack an LLT from max range still took laser damage (terrain height changes
ballistic range; lasers are unaffected), or missed it persistently.
**Used by.** (candidate: H-ARMY-SIEGE — rocket bots target towers first and stop at 0.95 × range instead of walking in)

### K-units-rockets-weak-vs-fast
**Claim.** Rocket bots fire one slow, unguided rocket every 3.8 s: strong against buildings and slow units, poor against
raiders (Pawn 87, Grunt 81, Blitz 101 speed) which close inside their range. A rocket group needs a screen of Pawns/Grunts
or Maces/Thugs in front of it.
**Status.** reported (2026-09-19); reload/speed numbers supported from local source
**Evidence.** https://masterbel2.wordpress.com/beyond-all-reason-units-guide-uses-tactics-and-counters/ ("protect long-range
units with short-range high-damage units"); `armrock.lua` (reloadtime 3.8), `armpw.lua`, `corak.lua`, `ArmVehicles/armflash.lua`.
**Would be wrong if.** Pure rocket groups traded evenly or better against BARb raider groups in the open.
**Used by.** (candidate: H-PROD-BATCH — keep ≥1 front-line unit per 2 rocket bots)

### K-units-grunt-vs-pawn
**Claim.** The two T1 raiders are not mirror images: Grunt is cheaper (43 vs 54 M), out-ranges Pawn (215 vs 180) and hits
harder per shot (37 per 0.5 s vs 9 per 0.3 s, laser); Pawn has more health (370 vs 280) and is slightly faster (87 vs 81).
Both are inside D-gun range (250) and LLT range when they engage. A web guide says Grunts even beat Thugs head-on but die
to area damage.
**Status.** supported (2026-09-19) for the numbers; the Grunt-beats-Thug statement is reported and doubtful
**Evidence.** `ArmBots/armpw.lua`, `CorBots/corak.lua`. https://www.crdhq.com/articles/units-vs-static-defense-counter-system
(2026; uses unit names loosely — see summary).
**Would be wrong if.** Equal-metal Pawn vs Grunt skirmishes in our logs favoured Pawns.
**Used by.** (none; relevant to K-opp-faction-asymmetry — part of "Cortex does better" may be the raider)

### K-units-dont-chase-raiders
**Claim.** Against scout/raider harassment the advice is not to chase: put the defenders (a raider or two, or a light
laser tower) directly beside the extractors, keep slow units grouped, and close open lanes with a line of towers. Static
defence is preferred for this job because it has more range and damage per metal than mobile units.
**Status.** reported (2026-09-19)
**Evidence.** https://masterbel2.wordpress.com/beyond-all-reason-units-guide-uses-tactics-and-counters/;
https://www.crdhq.com/articles/bar-new-player-guide-getting-started (towers should cover metal spots and generators, not
just the commander).
**Would be wrong if.** Extractor losses per match did not fall when H-ECO-OUTPOST-TURRET was active versus disabled.
**Used by.** H-ECO-OUTPOST-TURRET (first outside support), H-ARMY-CONTACT (candidate: a guard answer, defenders holding at the threatened
extractor instead of pursuing)

### K-units-static-defence-value
**Claim.** Static defence out-trades equal-cost units where the fight location is known (chokes, extractor clusters,
expansion points) because it gets free damage during the approach; it is wasted where the enemy can go around. Light
towers: Sentry/Guard 85-90 M, 620-650 hp, range 430-435, 20 E per shot. Beamer / Twin Guard 190-195 M, range 490. Heavy laser
Overwatch/Warden 440-480 M, 2600-2750 hp, range 620, 50-75 E per shot. Plasma battery Gauntlet/Agitator 1250-1300 M, range
1220-1245.
**Status.** reported (2026-09-19) for the value judgement; numbers supported from local source
**Evidence.** https://www.crdhq.com/articles/units-vs-static-defense-counter-system (2026).
`ArmBuildings/LandDefenceOffence/armllt.lua`, `armbeamer.lua`, `armhlt.lua`, `armguard.lua`; `CorBuildings/LandDefenceOffence/corllt.lua`,
`corhllt.lua`, `corhlt.lua`, `corpun.lua`.
**Would be wrong if.** Matches with more metal in towers showed no fewer extractor/builder losses, or BARb simply walked
around H-ECO-BASE-TURRETS.
**Used by.** H-ECO-BASE-TURRETS, H-ECO-OUTPOST-TURRET (candidate: one heavy laser tower at the main approach by minute ~8)

### K-units-laser-towers-need-energy
**Claim.** Laser towers pay energy per shot (LLT 20, Twin Guard 15, Beamer 6 per 0.1 s tick, heavy laser 50-75); in an energy
stall they stop firing. Rocket and plasma weapons of T1 bots have no per-shot energy cost.
**Status.** supported (2026-09-19) for the per-shot costs (local source); "stop firing when stalled" is engine behaviour, reported
**Evidence.** `energypershot` in `armllt.lua`, `corllt.lua`, `corhllt.lua`, `armbeamer.lua`, `armhlt.lua`, `corhlt.lua`; none in
`armrock.lua`, `armham.lua`. https://www.crdhq.com/articles/bar-economy-guide ("energy stalls … shut down ALL production").
**Would be wrong if.** Towers kept firing with stored energy at 0.
**Used by.** H-ECO-ENERGY-BY-STORAGE (adds a defensive reason for the 40% floor)

### K-units-vehicles-vs-bots
**Claim.** Vehicles are faster on flat ground, tougher and costlier per unit; bots are cheaper, climb steeper slopes and
are better for early expansion and rough maps. Top players favour vehicles and fast scouts in 1v1 and on large, open maps.
Reference points: Blitz `armflash` 110 M/730 hp/speed 101; Stout `armstump` 225 M/1800 hp/speed 75/range 350; Brute
`corraid` 235 M/2000 hp; Incisor `corgator` 120 M/820 hp/speed 85; the only T1 land artillery are vehicles — Shellshocker
`armart` 135 M range 710, Wolverine `corwolv` 170 M range 710.
**Status.** reported (2026-09-19); numbers supported from local source
**Evidence.** https://masterbel2.wordpress.com/beyond-all-reason-units-guide-uses-tactics-and-counters/;
https://www.crdhq.com/articles/bar-unit-guide; https://www.beyondallreason.info/guide/important-knowledge-on-advanced-mechanics
(slope tolerance). `ArmVehicles/*.lua`, `CorVehicles/*.lua`.
**Would be wrong if.** On Quicksilver (flat island) a vehicle-plant brain did no better than the bot-lab brain at equal economy.
**Used by.** (candidate: second factory = vehicle plant on flat maps; note H-PROD-BATCH's "artillery" slot has no bot-lab
unit at T1 — check what it actually builds)

### K-units-artillery-outranges-everything-t1
**Claim.** T1 artillery vehicles (range 710) outrange every T1 tower including the heavy laser (620) and are the intended
T1 answer to static defence; they are fragile (620-750 hp), slow-firing (4.3-7.1 s) and inaccurate against moving units, so
they need a screen and a spotter with line of sight.
**Status.** reported (2026-09-19); ranges supported from local source
**Evidence.** https://www.crdhq.com/articles/bar-unit-guide ("artillery counters static defenses");
`ArmVehicles/armart.lua`, `CorVehicles/corwolv.lua`, `armhlt.lua`.
**Would be wrong if.** Artillery groups with a screen failed to kill heavy laser towers without losses.
**Used by.** (none)

### K-units-rez-bots-with-army
**Claim.** Resurrection bots (Lazarus / Graverobber: 130 M, speed 78, 200 build power — 2.5x a constructor bot) should
follow the army to repair, reclaim wrecks after a fight and resurrect valuable ones. Resurrection costs no metal and 50% of
the unit's energy cost; repair is free. They cannot build.
**Status.** reported (2026-09-19); unit numbers supported from local source
**Evidence.** https://www.beyondallreason.info/guide/reclaim-resurrect-repair (official, undated);
`ArmBots/armrectr.lua:10,40` (canresurrect, workertime 200), `CorBots/cornecro.lua`.
**Would be wrong if.** Rez bots trailing the waves died before reclaiming enough to repay 130 M each.
**Used by.** (candidate: H-ARMY-RECLAIM — 2 rez bots per wave with an area-reclaim order on the last battle site)

### K-units-early-t2-bots
**Claim.** Early T2 bot reference (metal / health / speed / range): Armada — Welder `armzeus` 350/3500/48/280 assault;
Hound `armfido` 285/1200/69/650 skirmisher; Sprinter `armfast` 160/690/111/230 raider; Gunslinger `armmav` 650/1800/50/365;
Sharpshooter `armsnipe` 680/580/33/900 (2500 dmg per 10 s); Fatboy `armfboy` 1400/7800/30/700 (AoE 300).
Cortex — Fiend `corpyro` 200/1060/82.5/230 flamer raider; Sumo `corcan` 560/6000/38/275 assault; Sheldon `cormort`
400/940/50/850 skirmisher; Arbiter `corhrk` 600/610/54/1210 rockets; Termite `cortermite` 540/3100/50/340; Mammoth `corsumo`
2200/15600/23/650. Community pairings: Welder + Sharpshooter, Fiend + Sheldon (tough short-range front, long-range back).
**Status.** supported (2026-09-19) for numbers (local source); pairings reported
**Evidence.** `ArmBots/T2/*.lua`, `CorBots/T2/*.lua`; https://masterbel2.wordpress.com/beyond-all-reason-units-guide-uses-tactics-and-counters/.
**Would be wrong if.** UnitDefs seen through the AI interface differ.
**Used by.** (candidate: T2 production batch)

### K-units-anti-air-is-air-only
**Claim.** Crossbow / Trasher and the T1 missile tower (Nettle `armrl` / `corrl`, 80 M, range 765) only damage aircraft
(default damage 0 or absent); building them does nothing against ground. Conversely most ground weapons do a fraction of
their damage to air (e.g. Pawn 2 vs 9). Anti-air is only worth building after air has been scouted.
**Status.** supported (2026-09-19) — local source; "only after scouting air" is reported
**Evidence.** `damage` tables in `ArmBots/armjeth.lua`, `CorBots/corcrash.lua`, `ArmBuildings/LandDefenceOffence/armrl.lua`;
https://www.crdhq.com/articles/bar-unit-guide.
**Would be wrong if.** A Crossbow was seen damaging a ground unit.
**Used by.** (candidate: build 2-3 AA bots + 2 missile towers only when an enemy aircraft or air plant has been seen)

## Duel results (2026-09-19)

Measured with the duel harness (`docs/harness/duels.md`): equal metal (~1200 a side), flat ground, both sides
attack-move, no micro. **Margin** = own surviving metal share (health-weighted) minus the enemy's, x 100: +100 a flawless
win, -100 a wipe without a scratch. Two full 23-unit tables, in `docs/data/duels-2026-09-19/`: **tight** (ranks 56 elmos
apart, 8 duels a pairing, 2184 duels) and **wide** (100 elmos, 4 a pairing, 1092 duels). Figures below are tight / wide.
Spread within a pairing is small (median sd 0.07), so the tables are repeatable; whether they are *representative* is
the question (K-units-duel-caveats).

### K-units-duel-line-bots-beat-raiders
**Claim.** At equal metal in a head-on fight, Mace `armham` and Centurion `armwar` are our only tier-1 bots that win most
matchups (row means +37 / +38 and +37 / +40); Pawn `armpw` and Rocketeer `armrock` lose most (-1 / +21 and -2 / +4).
Against what BARb fields: Mace beats Grunt +44 / +32, Aggravator +31 / +33, Thug +16 / +13, Incisor +17 / +4, Brute
+26 / +27, Stout +38 / +31, both light turrets +60 or more. Pawn loses to Thug -43 / -20, Brute -45 / -26, Pounder
-93 / -77 and beats only Grunt (+14 / +15). Rocketeer loses to every mobile Cortex unit it cannot outrange: Thug
-46 / -52, Incisor -41 / -50, Blitz -36 / -54, Brute -29 / -32. Cortex mirrors it: Thug +37 / +38, Grunt -3 / +11.
**Status.** supported (2026-09-19) for the duel setting; untested in real matches
**Evidence.** `docs/data/duels-2026-09-19/{tight,wide}-pairs.csv`. All eight tight duels went the same way for each pairing
quoted except Mace-Incisor wide (+4, mixed).
**Would be wrong if.** An arena batch with production shifted from Pawn / Rocketeer to Mace did not improve the fight
ledger's exchange ratio against BARb; or the result vanished at 3-5x the army size.
**Used by.** (candidate: H-PROD-BATCH weights — Mace as the default line unit, Pawn only for raiding; the commander's
`set_production`)

### K-units-duel-pounder-dominates
**Claim.** Pounder `corlevlr` (220 M riot tank, range 315, speed 40) beats every other tier-1 land unit at equal metal,
at every formation spacing tried: row mean +77 / +70; against Pawn -93 / -77 from our side, Mace -68 / -57, Centurion
-65 / -59, Rocketeer -75 / -67, Stout -73 / -67. Its closest matchups are Janus (+24 / +45 for Pounder) and the light
turrets (+57 / +29). Pawn against Pounder stays lost at spacing 56, 100, 120 and 160 (-0.89, -0.75, -0.74, -0.56).
**Status.** supported (2026-09-19) for the duel setting
**Evidence.** Tables above; batches `sp56`..`sp160` (8 duels each).
**Would be wrong if.** Kiting decides it: the Pounder is the slowest unit in the table and out-ranged by rockets (475) and
artillery (710); a duel with the rockets held at range, or with retreat-while-firing, may reverse Rocketeer-Pounder.
Untested here because both sides charge.
**Used by.** (candidate: never send Pawn / Mace blobs into scouted Pounders; rockets or Janus with stand-off instead)

### K-units-duel-spacing-decides-area-damage
**Claim.** How tightly an army is packed changes who wins when one side does area damage, so a single table is not enough.
Janus `armjanus` has row mean +37 tight and +8 wide, and eight of its matchups change sign (Pawn +34 -> -29, Thug
+15 -> -34, Centurion +37 -> -20). Pawn against Mace goes -0.45, -0.02, +0.09, +0.19 at spacing 56, 100, 120, 160.
Pawn's row mean rises from -1 to +21. 19 of 252 pairings change sign between the tables with both margins beyond 0.10;
the median shift is 0.12.
**Status.** supported (2026-09-19)
**Evidence.** `tight-` against `wide-pairs.csv`; batches `sp56`..`sp160`; the first probe, `spacing120`.
**Would be wrong if.** The shift came from the wide formation's rear ranks standing outside the flat rectangle (they do,
by up to ~500 elmos for the largest armies) rather than from spacing. Not separated.
**Used by.** (candidate: the brain's blob attack-move is the tight case; spreading a Pawn wave is worth more than its unit
stats suggest)

### K-units-duel-range-vs-turrets
**Claim.** Against light laser towers at equal metal: Lasher `cormist` (range 575) wins untouched, +96 / +100; Rocketeer
+43 / +37 and Aggravator +24 / +28 (range 475 against 430-435) win but pay for it; Mace +60 / +52 and Thug +63 / +53
simply overpower them; Pawn loses -20 / -18. Artillery does **not** beat turrets here: Shellshocker `armart` -5 / -17
against Sentry, -20 / -54 against Guard, despite range 710.
**Status.** supported (2026-09-19) for the numbers; the explanation below is conjectured
**Evidence.** Tables above. Conjecture for the artillery result: its sight (364) is shorter than the turret's range
(430-435), so with no spotter or radar an attack-moving Shellshocker first sees the turret from inside its range.
Not checked in a replay.
**Would be wrong if.** A Shellshocker group given a scout or radar coverage still lost to equal-metal turrets.
**Used by.** K-units-rockets-outrange-llt (supports it, with the cost: rockets take return fire unless held at range);
K-units-artillery-outranges-everything-t1 (qualifies it: range without sight is not stand-off)

### K-units-towers-fire-at-the-rate-energy-arrives
**Claim.** A light laser tower's 20 energy a shot, not its damage, is what decides a tower line's output where
there is no economy behind it. A duel team is one idle commander: 30 energy a second, 1500 of storage. Fourteen
Sentries want 600 a second. In the duel rows the towers fire at exactly the rate energy arrives, plus whatever the
previous duel left in the store: `armham` against `armllt` dealt 3904 damage in 34.5 s (52 shots = 20 x 52 =
1040 energy = the income alone, an empty store), `armpw` against `armllt` 8840 in 24.5 s (118 shots = income plus a
full 1500). This is the quantitative form of K-units-laser-towers-need-energy, and it is why the highest damage
per metal in the tier-1 table has a row mean of -10. In a real match the same line behind a working economy is a
different unit.
**Status.** supported (2026-09-20) — arithmetic on the existing duel rows, no new engine run
**Evidence.** `docs/data/duels-2026-09-19/tight-duels.csv`, rows `armham,armllt` and `armpw,armllt`
(`damage_taken_x` against `seconds`); `energypershot = 20` in `ArmBuildings/LandDefenceOffence/armllt.lua`,
`energymake = 30` and `energystorage = 500` in `units/armcom.lua`, `startenergy`/`startenergystorage` 1000 in
`modoptions.lua`. Modelling it moves `crates/combatsim`'s agreement with the tight table from 71 % to 76 % and its
mean error from 0.40 to 0.30 (`docs/studies/combat-sim.md`).
**Would be wrong if.** A duel run with the tower team's energy storage deliberately filled (or drained) before the
duel gave the same result either way.
**Used by.** (candidate: value a scouted tower line by the enemy's energy income, not only its metal; our own
H-ECO-BASE-TURRETS is worth less during an energy stall)

### K-units-lead-prediction-is-mostly-guesswork
**Claim.** Most BAR ground weapons do not lead a moving target properly. The engine's `predictBoost` defaults to
0, which its own tag description defines as "over- or under-estimate target speed by between 0-2x its actual
value", redrawn every 15 frames; the lead is `target speed x flight time x that multiplier`. Of the weapons in the
duel table only five set it to 1 (Lasher's missiles among them); Mace and Thug set 0.4; artillery
(`armart`, `corwolv`), rockets (`armrock`, `corstorm`), Pounder, Janus, Stout and Brute leave it at 0. So against
anything fast, a slow unguided shot lands anywhere from the target's current position to twice its lead — which is
the mechanism behind K-units-rockets-weak-vs-fast, and it is a property of the weapon, not of the unit's speed.
Separately, the def's raw aim-error numbers are angles through `sin(x * pi / 0xafff)`, so a Pawn's `sprayangle`
of 1180 is 4.7 degrees.
**Status.** supported (2026-09-20) — engine source; the size of the effect in a real fight is conjectured
**Evidence.** `upstream/RecoilEngine/rts/Sim/Weapons/WeaponDef.cpp` (`predictBoost` default 0, `AccuracyToSin`),
`Weapon.cpp:784` (`predictSpeedMod` redrawn each SlowUpdate) and `GetLeadVec`. `predictboost` is absent from
`armart.lua`, `corwolv.lua`, `armrock.lua`, `corstorm.lua`, `corlevlr.lua`, `armjanus.lua`, `armstump.lua`,
`corraid.lua`. Modelling it took `crates/combatsim`'s wide-table correlation from 0.74 to 0.81 when it was added.
**Would be wrong if.** A duel of rockets or artillery against a stationary target of the same metal and health
traded no better than against a moving one.
**Used by.** (candidate: prefer tracking or hitscan weapons against raiders; do not count artillery damage against
anything that is moving)

### K-units-duel-caveats
**Claim.** The duel tables rank units for one situation — two single-type blobs of ~1200 metal charging each other on
flat ground — and are wrong to the extent a real fight differs:
(1) no micro: nothing kites, retreats or holds at range, so slow short-range units (Pounder, Mace, Thug) are flattered and
fast or long-range ones (Rocketeer, artillery, scouts) are not; (2) formation density matters as much as unit choice for
area damage (K-units-duel-spacing-decides-area-damage); (3) no sight support: artillery and rockets out-range their own
vision; (4) metal only — energy cost and build time are ignored, which favours energy-hungry units; (5) one army size —
Lanchester effects at 3-5x are untested; (6) no mixed armies, no terrain, no turrets or repair behind the line, no
veterancy; (7) anti-air (`armjeth`, `corcrash`) cannot hit ground and scores about -76 by construction; (8) units spawn
facing south on a west-east axis, so every army begins with a 90-degree turn.
**Status.** supported (2026-09-19) as a description of the method
**Evidence.** `docs/harness/duels.md`; harness checks there (no position, team-slot, site or game-speed bias found; wrecks
bias results unless cleared).
**Would be wrong if.** n/a (scope statement). Retire items as the harness gains stand-off orders, mixed armies or sizes.
**Used by.** Every K-units-duel-* claim.
