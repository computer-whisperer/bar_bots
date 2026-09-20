# Map dependence

Seeded 2026-09-19. "Map archive" = `run/data/maps/quicksilver_remake_1.24.sd7` (read `mapinfo.lua` and three feature defs
out of it). Unit paths relative to `upstream/Beyond-All-Reason/units/`.

### K-maps-quicksilver-facts
**Claim.** Quicksilver Remake 1.24: 14x14 (7168 elmos square), 2 players, an island surrounded by water (water level 0,
heights -80…325), wind 3-17, tidal strength 10, maxMetal 1.30, extractor radius 120. The site classifies it as a flat,
grassy 1v1 island map; average wind 12.7.
**Status.** supported (2026-09-19) — map archive; terrain description and average wind reported
**Evidence.** Map archive `mapinfo.lua:23-33,118-119`. https://www.beyondallreason.info/map/quicksilver (updated 2026-06-26).
**Would be wrong if.** The engine reported different wind limits at match start.
**Used by.** (see K-open-quicksilver-is-a-wind-map)

### K-maps-read-wind-at-start
**Claim.** Wind income per turbine equals current wind speed, which wanders between the map's minWind and maxWind and is
capped at 25 per turbine; the map limits are available to an AI at game start, so the wind/solar choice can be made per
map rather than hard-coded. Rule-of-thumb thresholds are in K-open-wind-threshold.
**Status.** reported (2026-09-19); cap supported from local source
**Evidence.** `ArmBuildings/LandEconomy/armwin.lua` (windgenerator 25); official economy guide
https://www.beyondallreason.info/guide/in-depth-look-at-economy. Whether our shim exposes `Map_getMinWind/MaxWind` is unchecked.
**Would be wrong if.** Logged energy income from N turbines was not ≈ N × current wind.
**Used by.** (candidate: H-ECO-ENERGY-CHOICE; needs min/max wind in bot-protocol's map info)

### K-maps-wind-needs-a-floor
**Claim.** With wind 3-17 the income of a wind-only economy swings almost 6:1; guides say to overbuild energy and keep the
bar above 50%. One energy storage (170 M, +6000 E) buffers ~60 s of a 100 E/s shortfall and is the cheap fix for wind troughs.
**Status.** reported (2026-09-19); storage numbers supported from local source
**Evidence.** https://www.crdhq.com/articles/bar-economy-guide ("energy bar above 50% at all times", "always overbuild
energy"); `ArmBuildings/LandEconomy/armestor.lua` (metalcost 170, energystorage 6000).
**Would be wrong if.** A wind brain with one storage still hit 0 energy for >10 s stretches in most matches.
**Used by.** (candidate: build one energy storage when turbines ≥ 8; note H-ECO-ENERGY-BY-STORAGE's 40% test scales with storage)

### K-maps-tidal-is-poor-here
**Claim.** A tidal generator makes the map's tidal strength in E/s, steadily. On Quicksilver that is 10 E/s for 90 M
(9 M per E/s) — worse than solar (7.75) and far worse than wind (~3.1) — so water energy is not worth it on this map. It only
wins on maps with tidal ≥ ~20 and low wind.
**Status.** supported (2026-09-19) for inputs (map archive tidal 10; `ArmBuildings/SeaEconomy/armtide.lua` 90 M, tidalgenerator 1);
the engine rule "output = tidal strength" is reported
**Evidence.** As above; https://www.beyondallreason.info/map/quicksilver (tidal 10).
**Would be wrong if.** A tidal generator on Quicksilver produced more than 10 E/s.
**Used by.** (none)

### K-maps-features-are-early-energy
**Claim.** Map features hold reclaimable resources that arrive instantly and cost nothing: on Quicksilver the palm trees
checked hold 250 energy each (0 metal) and the rock checked 10 metal. Four trees equal the whole 1000 E starting stock,
which is what the first minute is short of (K-open-energy-binds-first). Guides call reclaim of features "incredibly
important"; any builder can do it and a constructor's idle time near trees is free energy.
**Status.** supported (2026-09-19) for the sampled feature values (map archive `features/ad0_senegal_1.lua`,
`ad0_senegal_1_large.lua`: energy 250; `agorm_rock1.lua`: metal 10); density near start positions not measured; advice reported
**Evidence.** Map archive; https://www.beyondallreason.info/guide/reclaim-resurrect-repair.
**Would be wrong if.** An area-reclaim order by a constructor near the base yielded no energy, or took so long that a solar
would have paid more.
**Used by.** (candidate: H-ECO-RECLAIM-FEATURES — when stored energy < 40% and features are within ~400 of a builder,
area-reclaim before building another generator; needs a reclaim command in bot-protocol)

### K-maps-factory-by-terrain
**Claim.** Factory choice follows terrain: bots for hills and steep approaches, vehicles for flat open ground, hovercraft
or amphibious units where water separates the players, ships only where the enemy is reachable by sea. The official
starter guide says to pick the factory from the start location. Quicksilver's land is flat, which favours vehicles on paper;
both start positions are on the same island, so land units reach the enemy.
**Status.** reported (2026-09-19)
**Evidence.** https://www.beyondallreason.info/guide/how-to-start-manage-your-economy;
https://www.beyondallreason.info/guide/important-knowledge-on-advanced-mechanics (slope tolerance);
https://masterbel2.wordpress.com/beyond-all-reason-units-guide-uses-tactics-and-counters/ (vehicles favoured in 1v1).
That land units path between the starts is our own arena observation (waves meet mid-map, K-army-piecemeal-midmap).
**Would be wrong if.** A vehicle-plant opening did not beat the bot-lab opening on Quicksilver at equal economy rules.
**Used by.** (candidate: factory type chosen by mean slope between the two start positions)

### K-maps-metal-share-wins
**Claim.** Holding more metal spots is the main predictor of winning; one guide puts it at "60%+ of the spots usually wins".
On a symmetric 1v1 map this means every spot in our half plus some contested middle spots, and denying the opponent's
outer spots by raiding.
**Status.** reported (2026-09-19)
**Evidence.** https://www.crdhq.com/articles/bar-economy-guide (2026; the 60% figure has no stated basis).
**Would be wrong if.** Across our match logs, the side with more extractors at minute 10 did not win clearly more often.
**Used by.** H-ECO-OWN-HALF (candidate: relax to "own half + middle band" once an army group holds the middle)

### K-maps-forward-factory
**Claim.** Slow heavy units are worth more the closer they are built to the enemy; a forward factory after expanding
shortens reinforcement paths and forces the opponent into slow units too. This is the community answer to reinforcements
trickling across the map.
**Status.** reported (2026-09-19)
**Evidence.** https://masterbel2.wordpress.com/beyond-all-reason-units-guide-uses-tactics-and-counters/.
**Would be wrong if.** A second lab placed at ~40% of the way to the enemy was lost in most matches before repaying itself.
**Used by.** (candidate: H-ECO-MORE-LABS places lab 2 forward, behind the outpost turrets; relates to K-army-piecemeal-midmap)

### K-maps-quicksilver-corner-asymmetry
**Claim.** Quicksilver Remake 1.24 is an asymmetric island with cliffs and inlets, and the two corner starts the arena
uses are different games: the north-west start sits on a narrow peninsula and has 15 metal spots nearer to it on foot,
the south-east start 19.
**Status.** supported (2026-09-19). An earlier version of this entry called the metal spots mirror-symmetric; that was
wrong (only the two start positions mirror, because the arena's start boxes do): 42 of 44 spots have no mirror partner.
The user saw the asymmetry at once in the replay.
**Evidence.** Terrain survey at game start (`brain/routes.rs` log lines, terrain-check): 38 of 44 spots reachable on
foot; the six cut off are (3224,520) (6184,520) (6632,984) (504,6184) (936,6664) (4632,6664), the first of which is the
spot constructors kept failing to reach. Results by corner: v11 SE 10-0-2 / NW 3-7-2; v13-medium SE 10-2 / NW 1-11;
with the start boxes swapped the gap stayed with the corner. The viewer's terrain layer shows the shape.
**Would be wrong if.** A symmetric map showed the same gap between its two starts.
**Used by.** Evaluation: results are read per corner; a symmetric 1v1 map is wanted in the pool.

### K-maps-terrain-not-straight-lines
**Claim.** Geometry by straight line ("nearer to us than to the enemy", "500 towards the enemy", "is this target
reachable") is wrong on maps with cliffs and water. A walking-distance field over the engine's slope and height maps,
for the soldiers' movement class, agrees with what the engine lets units do.
**Status.** supported (2026-09-19) on one map
**Evidence.** The field's unreachable spots include the one the engine refused all day; with stations, lab yard,
staging points and "our half" taken from the field, a 12-minute game had 3 move failures and no "unreachable" give-ups
(terrain-check-2), against hundreds to thousands before.
**Would be wrong if.** Move failures or unreachable give-ups came back in numbers on another map.
**Used by.** `brain/routes.rs`: `spot_is_ours`, `forward_of_home`, attack-target filter, staging point.

### K-maps-quicksilver-spot-value-and-wind
**Claim.** On Quicksilver Remake 1.24 a tier-1 extractor yields 2.0 metal/s on every spot we have built on, and wind per
turbine averages 12.8 over the first ten minutes (single games 8.2 to 15.5).
**Status.** supported (2026-09-19)
**Evidence.** Income steps at extractor completion in 12 records of `v15-terrain-medium` (home spots 18 of 18 exactly
2.0; outer spots median 1.8-2.7 with converter noise); wind inferred as (energy income - constant producers) / turbines.
The map site lists 12.7.
**Would be wrong if.** A spot's first extractor raised income by something other than 2.0 with no converter running.
**Used by.** `crates/buildorder` (`map::SPOT_METAL`, `map::WIND_MEAN`).

### K-maps-our-half-shrinks
**Claim.** Placing the enemy's base at the mean of every enemy building we remember drags it toward us, because what we
see is mostly its forward turrets and extractors; the walking-distance line between "its" spots and "ours" follows, and
the constructors (H-ECO-OWN-HALF) are left with almost nothing to take.
**Status.** supported for the south-east (2026-09-20, v24: 14.8 v 4.2 extractors at minute 15 with the fix, 9.6 v 9.8 without; 5-0-1 against 4-0-2);
not the north-west's limit, whose extractor curve did not move.
**Evidence.** commander-5-tempo-brief, minute 23: "free spots on our side 2" with 7 extractors held, of 38 reachable
spots (15 are ours at game start); army 11,200 v 4,100 and extractors flat for 19 minutes; the commander's
`expansion_radius` 3000 changed nothing because the own-half rule applied beneath it; constructors fell through to
converters (30). North-west heuristic batches stall at 4-5 extractors from minute 6 (v22, v23).
**Would be wrong if.** The A/B shows the same extractor curve with the base located by factories only.
**Used by.** H-MAP-ENEMY-BASE; the commander's `expansion_radius` now replaces the own-half rule.

### K-maps-mirror-start-is-a-beach
**Claim.** On Quicksilver the point mirror of our north-west start, (5136, 5980), is a beach across an inlet from the
real south-east base at about (5576, 5360): armies ordered "to the enemy start" walk there and stand on the shore, out
of sight of the base. The walkable metal spot nearest the mirror point, (5448, 5240), is 140 from the real start.
**Status.** observed (2026-09-20); the user recognised it as a recurring failure while watching commander-7.
**Evidence.** commander-7-raid-expansion/00, minute 25: 57 soldiers of squad `farguard` at (4750-5250, 5750-6250) on a
`fight` order to (5136, 5980), the report still saying "its commander has never been seen; its factories seen: none".
commander-5: a strike "reached presumed enemy start F7 but found nothing". The heuristic's fallback target and the
scout use the same point.
**Would be wrong if.** On other maps the nearest-spot rule lands farther from the real start than the mirror does;
start positions are not exposed to a skirmish AI (`Map_getStartPos` gives only our own), so this stays a guess until
a factory is seen (H-MAP-ENEMY-BASE).
**Used by.** H-MAP-ENEMY-START.

### K-team-a-mean-of-two-bases-is-nobodys
**Claim.** With several opponents, any single "enemy start" (a mirror point, a mean of factories) is a place where
nobody lives; ground, direction and targets have to be asked of the nearest live base.
**Status.** reasoned, not measured (2026-09-20): the single-base code was replaced before a team game was played.
**Used by.** H-MAP-ENEMY-BASE.

### K-team-allies-are-invisible-by-default
**Claim.** The AI interface's unit list is our own team's; allied units must be asked for (`getFriendlyUnits`), and no
call tells where another team started: an ally's start is where its commander is first seen, an enemy's is somewhere in
its ally team's start box, which only the setup script (`Game_getSetupScript`) gives.
**Status.** supported (2026-09-20), read from `SSkirmishAICallback.h` and seen in team-2v2-smoke.
**Used by.** H-TEAM-ALLIED-SPOTS, H-TEAM-ALLY-GROUND, H-MAP-ENEMY-BASE.

### K-map-ground-is-who-answers-first
**Claim.** Whether a metal spot can be held is decided by who can bring force there sooner, not by its distance from home
or by a clock since the last loss: spots beside our army are safe far from home, and spots near home are not while the
opponent's army stands among them.
**Status.** supported (2026-09-20), terr-2 against the distance-and-clock rules on the same 8 north-west seeds:
extractors 7.0 / 7.4 / 7.8 at minutes 5 / 7 / 11 against 5.0 / 6.2 / 6.9, soldiers 24-28 against 22 at minutes 11-15; on
Mithril Mountain 12.2 / 14.6 extractors at minutes 11 / 15 against 8.4 / 8.1 (v33, 16 games) and 4-2-2 against 5-10-1.
Wins north-west did not move (1-7-0 against 0-6-2; that corner has won 10-25 % for days, as it does for BARb).
**Evidence.** `run/matches/*terr-2-*`, `*terr-1-nw-base`; curves from the per-minute lines of bot.log.
**Used by.** H-MAP-TERRITORY.

### K-map-presence-is-not-memory
**Claim.** "May a constructor go there" and "where do raids come from" need different memories. With every enemy soldier
seen in three minutes counted against a spot, one visit by the opponent's army closed most of our half for minutes and
nothing was rebuilt; an extractor pays for itself in about half a minute, so only who is there now should stop it.
**Status.** observed (2026-09-20), terr-1-nw: 31-41 of 44 spots classed theirs from minute 8, extractors 4-5 from minute
7 against 6-7 without the grid; with the class forgetting in 60 s (terr-2) the curve went above the old rules'.
**Evidence.** `run/matches/1789915396-terr-1-nw/*/bot.log`, the `ground:` lines.
**Used by.** H-MAP-TERRITORY.

### K-maps-quicksilver-corner-decides-both-economies
**Claim.** The Quicksilver corner asymmetry is a property of the ground, not of our play: whoever holds the south-east
start has roughly twice the extractors of whoever holds the north-west one. At minute 20 over 430 games, when we start
north-west we have 3.9 extractors and BARb (south-east) has 20.8; when we start south-east we have 11.6 and BARb
(north-west) has 8.9. Which start we drew is accordingly the single most informative thing we know about the
opponent's economy: used alone beside the clock it removes 30 % of the error in estimating their extractor count and
32 % of the error in the income proxy, more than any observation of them.
**Status.** measured (2026-09-20). Sharpens K-maps-quicksilver-corner-asymmetry, which established the asymmetry from
the terrain (15 spots near the north-west start on foot, 19 near the south-east one) and from our own results.
**Evidence.** `docs/studies/tempo-model.md`, sections "Models, simple to less simple" and "What evidence actually
moves the estimate"; opponent ground truth in every Quicksilver batch from v17 to v34.
**Would be wrong if.** The two corners' extractor counts evened out once our own play stopped plateauing in the
north-west (K-eco-reach-stops-at-the-first-gap): part of BARb's 20.8 is spots we never contest.
**Used by.** (none yet) — candidate: the tempo model conditions on it; expansion and defence rules could too.

### K-maps-early-walks-nearly-straight
**Claim.** In the first six minutes our builders' ways are 4-7 % longer on foot than in a straight line on Quicksilver
and on Mithril Mountain (median 1.04-1.07, longest 1.35): the home plateau is open. The map's ground matters to the
opening where a start sits behind a choke (not measured yet: Great Divide).
**Status.** measured (2026-09-20), 8 games, 247 trips.
**Evidence.** `buildorder walks` on `open-cal2-*`, last column.
**Would be wrong if.** The same report on Great Divide or Comet Catcher gave the same ratio.
**Used by.** (none) — the search uses the map's ground anyway (`buildorder::game::Walked`).

