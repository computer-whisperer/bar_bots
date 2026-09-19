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
**Claim.** On Quicksilver Remake 1.24 our bot does far better from the south-east start than from the north-west one,
and the difference lies in what BARb does, not in what we do: BARb starting north-west hardly ever reaches our half.
**Status.** conjectured (2026-09-19). The base-maze explanation (K-army-base-maze) was real but is refuted as the cause:
fixing it left the gap unchanged.
**Evidence.** v9-reachability SE 8-0-4, NW 2-5-5; v10-sites SE 7-0-1, NW 1-5-2; v11-layout (after the maze fix) SE 10-0-2,
NW 3-7-2. v11 fight ledgers: in 9 of 12 SE games we lost nothing at all in our own half; in NW games we lost 6-256 units
there, and the NW losses end with an enemy group killing a healthy commander in 10-20 s (match 14: 3535 health to dead in
13 s to a pack of armwar, with 8 extractors, 4 labs and 28 soldiers on the books).
Against medium (v13-medium) SE 10-2, NW 1-11. NW and SE games build identically through minute 6 (5.7 against 6.0
extractors); in the first ten minutes we then lose 8 extractors a game from NW against 3.4 from SE. With the start boxes
swapped (v13-medium-swapped) the gap stayed with the corner: NW as team 1 0-8, SE as team 0 6-2, so it is not the team slot.
**Would be wrong if.** BARb vs BARb on this map showed no corner difference, or a second map showed the same gap for us.
**Used by.** (none). Until settled, read the NW record as the honest one; the SE record flatters us.
