# Openings and build orders

Seeded 2026-09-19 from public sources plus the local game source. "Local source" means `upstream/Beyond-All-Reason`
(the checkout we run; changelog head "September"); unit-file paths are relative to it. Web claims are `reported` and
unchecked in the arena. Scope unless stated: 1v1, land, Armada/Cortex, default mod options.

### K-open-start-resources
**Claim.** A team starts with 1000 metal and 1000 energy (storage 1000 each); the commander adds +2 metal/s, +30 energy/s
and 300 build power. Nothing else produces until the first building finishes.
**Status.** supported (2026-09-19) — read in local source, not measured in a match
**Evidence.** `modoptions.lua:2405-2449` (startmetal/startenergy/storage defaults 1000); `units/armcom.lua`, `units/corcom.lua`
(metalmake 2, energymake 30, workertime 300).
**Would be wrong if.** The bot's first status line showed a different starting stock with default options.
**Used by.** (candidate: budget the opening against 1000 M / 1000 E instead of a fixed list)

### K-open-energy-binds-first
**Claim.** In the first minute energy, not metal, is the binding resource: an extractor costs 50 M but 500 E, a bot lab
500 M + 950 E (Cortex 470 M + 1050 E), a wind turbine 40 M + 175 E, a solar 155 M + 0 E. Two extractors plus a lab already
cost 1950 E against 1000 E stock and +30 E/s. Solar's zero energy cost is why it is the classic first generator even where
wind is better later.
**Status.** supported (2026-09-19) for the costs; the conclusion is arithmetic, not observed
**Evidence.** `units/ArmBuildings/LandEconomy/armmex.lua`, `armsolar.lua`, `armwin.lua`, `LandFactories/armlab.lua`, Cortex
equivalents. Official economy guide makes the same point qualitatively ("solars … when you have more metal than energy, or
as an emergency source if you are stalling"): https://www.beyondallreason.info/guide/in-depth-look-at-economy (undated).
**Would be wrong if.** bot.log showed energy never below ~200 during the first 90 s with a wind-only opening.
**Used by.** (candidate: opening = 2 mex, then energy chosen by wind rule, watching stored energy before placing the lab)

### K-open-two-mex-first
**Claim.** The standard opening is two extractors before anything else, then energy, then the first factory, then radar
and one or two light laser towers. Priority order in every written guide found: extractors → energy → (constructors) →
factory.
**Status.** reported (2026-09-19)
**Evidence.** https://www.beyondallreason.info/guide/how-to-start-manage-your-economy ("typically build 2 extractors
before building anything else", "one or two Light Laser Towers once you have metal, energy, production and intel"; undated
official guide). https://www.crdhq.com/articles/bar-build-order-metal-solar (2026, low-reliability aggregator).
**Would be wrong if.** A 3-mex or factory-first opening reached a higher army value at minute 5 over 24+ matches.
**Used by.** H-ECO-OPENING already does this (2 mex, 2 solar, lab); this entry is its first outside support.

### K-open-wind-threshold
**Claim.** Choose wind over solar when the map's average wind is above a threshold; sources disagree on the threshold:
official economy guide ">7", official starter guide ">10", a third-party guide "wind above 8, solar below 5, mix between".
From local costs the pure metal break-even is ~5.2 (wind 40 M per [wind speed] E/s vs solar 155 M per 20 E/s = 7.75 M per
E/s); the higher published thresholds price in wind's variance and its 175 E build cost.
**Status.** reported (2026-09-19); break-even arithmetic from local unit files
**Evidence.** https://www.beyondallreason.info/guide/in-depth-look-at-economy (>7);
https://www.beyondallreason.info/guide/how-to-start-manage-your-economy (>10);
https://www.crdhq.com/articles/bar-build-order-metal-solar (8/5). `armwin.lua` (metalcost 40, windgenerator 25 cap),
`armsolar.lua` (metalcost 155, energyupkeep -20).
**Would be wrong if.** On a map with average wind ≥ 10 a solar-only brain matched a wind brain's energy income per metal
spent at minute 8.
**Used by.** (candidate: H-ECO-ENERGY-CHOICE — read map min/max wind at start; wind if (min+max)/2 ≥ 8, else solar)

### K-open-quicksilver-is-a-wind-map
**Claim.** Quicksilver Remake 1.24 has wind 3–17 (site lists average 12.7), so wind turbines give ~2.5x the energy per
metal of solars there (12.7 E per 40 M vs 20 E per 155 M). Our opening and generator rule build solars / advanced solars only.
**Status.** supported (2026-09-19) for min/max wind (local map archive); average and the conclusion are reported/arithmetic
**Evidence.** `run/data/maps/quicksilver_remake_1.24.sd7` → `mapinfo.lua:118-119` (minWind 3.0, maxWind 17.0).
https://www.beyondallreason.info/map/quicksilver (wind 3-17, average 12.7, tidal 10; page updated 2026-06-26).
**Would be wrong if.** A wind-first brain on Quicksilver showed lower energy income at minute 5 and 10 than the solar brain
at equal metal spent on generators, or stalled more often in the low-wind troughs.
**Used by.** (candidate: replace solars in H-ECO-OPENING and H-ECO-ENERGY-BY-STORAGE with wind on this map; keep 1-2 solars
or an energy storage as a floor for wind troughs)

### K-open-build-times
**Claim.** Build time in seconds = `buildtime / total workertime` while resources last. Commander alone (300): extractor 6 s,
wind 5.3 s, solar 8.7 s, bot lab 16.7 s, radar 3.8 s, LLT 8 s. A T1 constructor bot (80-85) takes 3.6x as long. So the
2 mex + 2 solar + lab opening is ~46 s of pure build time plus walking.
**Status.** supported (2026-09-19) for the inputs; the formula is standard Spring/Recoil behaviour, not measured by us
**Evidence.** buildtime/workertime in `units/armcom.lua`, `ArmBuildings/LandEconomy/*.lua`, `ArmBots/armck.lua` (80),
`CorBots/corck.lua` (85).
**Would be wrong if.** Logged start/finish frames of commander builds disagreed by more than ~10% when not stalled.
**Used by.** (candidate: detect a stalled opening — lab not finished by ~1:30 means energy or pathing trouble)

### K-open-factory-drain
**Claim.** A bot lab (150 build power) working alone drains roughly 5 M/s on raiders and constructors and 9-10 M/s on
line units: Pawn 11 s (4.9 M/s, 82 E/s), Grunt 8.3 s (5.2 M/s, 98 E/s), Rocketeer 13.4 s (9 M/s, 75 E/s), Thug 14 s
(10 M/s, 82 E/s), Centurion 28 s (9.6 M/s, 111 E/s), constructor bot 23 s (4.8 M/s, 70 E/s). An opening economy of
2 extractors + commander cannot feed continuous line-unit production; ~100 E/s is needed per busy lab.
**Status.** supported (2026-09-19) — arithmetic on local unit files
**Evidence.** metalcost/energycost/buildtime in `units/ArmBots/*.lua`, `units/CorBots/*.lua`; lab workertime 150 in
`armlab.lua`/`corlab.lua`.
**Would be wrong if.** Measured metal usage with one un-assisted lab on repeat differed from these rates by >20%.
**Used by.** (candidate: size lab count and nano count from income: labs ≈ metal income / 9)

### K-open-constructors-early
**Claim.** Guides put constructors ahead of or right behind the first fighters (priority "extractors, energy,
constructors, factory"; "secure three additional mexes within the first five minutes"), while our arena evidence says
fighters first against BARb's early raids. The two reconcile as: one or two raiders/scouts, then a constructor, never
several constructors in a row.
**Status.** reported (2026-09-19)
**Evidence.** https://www.crdhq.com/articles/bar-new-player-guide-getting-started,
https://www.crdhq.com/articles/bar-beginner-guide-getting-started (2026, undated articles). Our K-army-fighters-before-constructors.
**Would be wrong if.** Moving the first constructor from slot 3 to slot 1 or 2 in the first batch changed neither extractor
count at minute 5 nor early losses over 24+ matches.
**Used by.** H-PROD-BATCH (already raider, raider, constructor …)

### K-open-one-factory-plus-assist
**Claim.** Early on, one factory with build assistance beats two factories: guides say "one factory with nano turrets beats
two without" and 2-3 construction turrets double or triple output. A quantitative community analysis narrows it: ~1-3
assisting builders on a T1 lab is a clear win, beyond that a second factory is more efficient.
**Status.** reported (2026-09-19)
**Evidence.** https://www.crdhq.com/articles/bar-economy-guide (2026);
https://roguel1kegaming.com/beyond-all-reason-patch-construction-turrets/ (post-turret-nerf patch analysis, undated; quotes
turret cost 250 M — local `armnanotc.lua` says 230 M, 3200 E, 200 build power, range 400).
**Would be wrong if.** At equal metal spent, 2 labs out-produced 1 lab + 2 turrets in army value by minute 10.
**Used by.** (candidate: H-ECO-MORE-LABS — add up to 2 construction turrets beside the first lab before a second lab)
