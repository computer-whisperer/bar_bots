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
**Used by.** H-ECO-OUTPOST-TURRET (first outside support), H-ARMY-DEFEND (candidate: defenders hold at the threatened
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
