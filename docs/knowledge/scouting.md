# Scouting and information

Seeded 2026-09-19. Numbers from local unit files (`upstream/Beyond-All-Reason/units/…`) are `supported`; advice is `reported`.
Scope: 1v1 land. Our bot currently has no scouting and targets "the mirrored home position" (H-ARMY-TARGET).

### K-scout-radar-tower-is-cheap
**Claim.** A radar tower costs 60 M / 630 E, builds in 3.8 s by the commander, and covers radius 2100 (sight 680); the
commander itself has radar 700. On a 7168-wide map, one tower at the base and one at each forward outpost covers most of
our half. Official opening advice lists radar as step 4 of 4 (metal, energy, factory, intel). Terrain blocks radar, so
towers go on high ground or edges.
**Status.** supported (2026-09-19) for numbers (local source); placement advice reported
**Evidence.** `ArmBuildings/LandUtil/armrad.lua`, `CorBuildings/LandUtil/corrad.lua` (radardistance 2100, metalcost 60),
`units/armcom.lua` (radardistance 700). https://www.beyondallreason.info/guide/how-to-start-manage-your-economy;
https://www.beyondallreason.info/guide/radar (official, undated).
**Would be wrong if.** The AI interface did not report radar contacts to the bot at all (then radar only helps unit
auto-targeting, not decisions).
**Used by.** (candidate: H-INTEL-RADAR — radar right after the first lab, one more with each outpost turret)

### K-scout-radar-blips-are-inaccurate
**Claim.** Units firing at a radar-only contact aim at a wobbling position and miss most shots; line of sight on the target
is required for accurate fire. Identified buildings stay as ghosts once seen. Targeting facilities (T2) halve the wobble
each (50% / 75% / 87.5%).
**Status.** reported (2026-09-19)
**Evidence.** https://www.beyondallreason.info/guide/radar;
https://www.beyondallreason.info/guide/important-knowledge-on-advanced-mechanics (official, undated).
**Would be wrong if.** Rocket bots hit a tower at max range equally often with and without a spotter in sight range.
**Used by.** (candidate: H-ARMY-SIEGE — long-range groups need a cheap spotter within its sight radius of the target:
rocket bots see 380 but shoot 475)

### K-scout-sight-shorter-than-range
**Claim.** Several units shoot farther than they see, so alone they fire at blips: Rocketeer/Aggravator sight 380 vs
range 475; Shellshocker 364 vs 710; Wolverine 330 vs 710; Sheldon 380 vs 850; Arbiter 380 vs 1210. Cheap eyes: Tick sight
600 (21 M), Grunt 520 (43 M), Pawn 429, Rover `armfav` 635 (31 M), Rascal `corfav` 600 (26 M), LLT 494, radar tower 680,
Beholder camera `armeyes` 560 (32 M, cloaked).
**Status.** supported (2026-09-19) — local source
**Evidence.** `sightdistance` / `range` in `ArmBots/armrock.lua`, `CorBots/corstorm.lua`, `ArmVehicles/armart.lua`,
`CorVehicles/corwolv.lua`, `CorBots/T2/cormort.lua`, `corhrk.lua`, `ArmBots/armflea.lua`, `CorBots/corak.lua`,
`ArmVehicles/armfav.lua`, `CorVehicles/corfav.lua`, `ArmBuildings/LandUtil/armeyes.lua`.
**Would be wrong if.** UnitDefs through the AI interface show different sight values (alldefs_post may scale them).
**Used by.** (candidate: every attack group includes ≥1 raider ordered to stay ~100 ahead of the skirmishers)

### K-scout-what-to-look-for
**Claim.** One early scout pays for itself by revealing the opponent's factory type and first units (raiders vs line
units vs air), which decides anti-air, tower placement and army composition. Things a bot should extract from a scouting
pass: factory types and count; advanced lab under construction; defence positions and types on the approach;
extractor count; commander position.
**Status.** reported (2026-09-19)
**Evidence.** https://www.crdhq.com/articles/bar-beginner-guide-getting-started (2026);
https://masterbel2.wordpress.com/beyond-all-reason-units-guide-uses-tactics-and-counters/ (scouts give composition, radar
gives area information).
**Would be wrong if.** Against BARb (whose build is near-deterministic per profile) a scouting brain made no different
decisions than a blind one — then hard-code what opponents-barb.md says instead.
**Used by.** (candidate: H-INTEL-SCOUT — first factory unit for Armada is a Tick sent to the mirrored start; Cortex sends
the first Grunt; repeat every ~3 minutes)

### K-scout-air-scout
**Claim.** The T1 air scouts (Blink `armpeep` 52 M, sight 865, radar 1140, speed 375; Finch `corfink` 51 M, sight 835,
radar 1120, speed 360) cross Quicksilver in ~20 s and see far more than ground scouts, but need an aircraft plant
(Armada 650 M / Cortex 630 M).
**Status.** supported (2026-09-19) for numbers — local source; not evaluated as a plan
**Evidence.** `ArmAircraft/armpeep.lua`, `CorAircraft/corfink.lua`, `ArmBuildings/LandFactories/armap.lua`, `corap.lua`.
**Would be wrong if.** (numbers only)
**Used by.** (none)

### K-scout-target-the-economy
**Claim.** When the enemy army is stronger, the advice is to avoid it and hit several economic targets faster than they can
be defended; that needs knowledge of where the enemy's extractors are. On a symmetric 1v1 map the enemy's likely spots are
the mirror images of ours, and any extractor once seen persists as a ghost.
**Status.** reported (2026-09-19)
**Evidence.** https://masterbel2.wordpress.com/beyond-all-reason-units-guide-uses-tactics-and-counters/ ("raid multiple
enemy bases faster than they can defend them"); ghosts: https://www.beyondallreason.info/guide/radar.
**Would be wrong if.** Raider groups sent to mirrored spots on Quicksilver found them empty or towered most of the time.
**Used by.** H-ARMY-SWEEP (candidate: run the sweep with raiders from minute ~4, not only when the main target is empty)

### K-scout-we-never-see-their-extractors
**Claim.** Our bot has essentially no sight of the opponent's economy. Averaged over 430 games on Quicksilver we have
identified and not watched die 0.0 of their extractors in minutes 1-8, 0.5 in minutes 11-15 and 1.3 of their 14.6 in
minutes 20-40; in 39 % of late-game rows not one enemy extractor has ever been seen. Static defence is barely better:
467 metal of turrets in hand against 3805 standing. Their army is the one thing we do see — 3435 metal of the 4853
they have by minute 20 — because it comes to us.
**Status.** measured (2026-09-20) over every match that has a ground-truth file.
**Evidence.** `docs/studies/tempo-model.md` table "What the bot actually has in hand", from `run/tempo_model.py`
(`f_live_mex_n`, `f_live_turret_m`, `f_live_army_m` against `truth-*.jsonl`).
**Would be wrong if.** With a scouting rule that visits their half, the number of enemy extractors ever identified per
game rose materially. That is the test of any scouting change, and it now has a baseline to beat.
**Used by.** (none yet) — it is the reason an estimate of their economy has to be inferred rather than observed
(K-barb-tempo-is-half-clock-half-sighting), and the strongest argument yet for H-ARMY-SCOUT / radar coverage.
