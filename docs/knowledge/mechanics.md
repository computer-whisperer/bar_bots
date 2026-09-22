# Mechanics a bot must respect

Seeded 2026-09-19. Paths relative to `upstream/Beyond-All-Reason/` (the game source we run). Entries here are mechanics
that change what a rule should do; pure engine-interface facts belong in `game-rules.md` / `harness/`.

### K-mech-converter-threshold
**Claim.** Energy converters only run while stored energy is above a team-wide fraction of energy storage, default 75%
(`mmLevel` 0.75, set for every team including AIs; players move it with a slider). The gadget converts only the energy above
that level, every 15 frames. Consequences: converters never cause an energy stall by themselves; stored energy hovering at
~75% with converters built means "surplus is being converted", not "short"; and a rule that waits for >80% stored energy
before adding converters is looking at a level the existing converters actively pull down to 75%.
**Status.** supported (2026-09-19) — local source, not observed in a match
**Evidence.** `luarules/gadgets/game_energy_conversion.lua:263,273,295-297` (`teamMMLevels[tID] = 0.75`;
`convertAmount = eCur - eStor * mmLevel`).
**Would be wrong if.** bot.log showed converters producing metal with stored energy below 70% of storage.
**Used by.** H-ECO-CONVERT-SURPLUS (its 80% trigger interacts with this: after the first converters, stored energy will sit
near 75% and the rule may never fire again — check in logs), H-ECO-ENERGY-BY-STORAGE (40% floor is safely below 75%)

### K-mech-converter-rates
**Claim.** T1 converter: 70 E/s → 1.0 M/s, costs 1 M + 1150-1250 E, 167 hp. Feeding one takes 3.5 solars (542 M) or, at
Quicksilver's average wind, 5.5 turbines (220 M): +1 M/s from conversion costs ~220-540 M versus 50 M for a free extractor
spot at roughly +2 M/s and ~100 M per +1 M/s for an advanced-extractor upgrade. Converters are the last resort for metal, and
the right sink for surplus energy.
**Status.** supported (2026-09-19) for rates/costs (local source); ranking is arithmetic using an inferred ~2 M/s per spot
**Evidence.** `units/ArmBuildings/LandEconomy/armmakr.lua` (energyconv_capacity 70, energyconv_efficiency 0.01429),
`cormakr.lua`; generator costs as in openings.md. Agrees with official guide (converters "used primarily if expansion becomes
too difficult"): https://www.beyondallreason.info/guide/in-depth-look-at-economy. A third-party rule says build converters
"when energy income exceeds 2x metal income" (https://www.crdhq.com/articles/bar-economy-guide) — that ratio is always true
in BAR (E income is ~10-20x M income), so the rule is useless as written.
**Would be wrong if.** Measured converter output differed from 1 M/s per 70 E/s consumed.
**Used by.** H-ECO-CONVERT-SURPLUS, K-eco-expansion-before-conversion (supports it)

### K-mech-stall-slows-everything
**Claim.** When a resource runs out, every builder and factory drawing on it slows in proportion (construction gets only the
fraction of resources available), energy-per-shot weapons stop firing, and units with energy upkeep (extractors 3 E/s,
advanced extractors 20 E/s, radar, cloak) can switch off. An energy stall is therefore worse than a metal stall: it also cuts
metal income and defence. Guide targets: energy bar above 50% at all times, metal bar between 20% and 80% (above 80% means
too little build power, not wealth).
**Status.** reported (2026-09-19); upkeep and per-shot numbers supported from local source
**Evidence.** https://www.crdhq.com/articles/bar-economy-guide (2026); https://www.beyondallreason.info/guide/in-depth-look-at-economy.
`units/ArmBuildings/LandEconomy/armmex.lua` (energyupkeep 3), `armmoho.lua` (20), `LandDefenceOffence/armllt.lua` (energypershot 20).
**Would be wrong if.** Metal income stayed flat through a logged period of zero stored energy.
**Used by.** H-ECO-ENERGY-BY-STORAGE; (candidate: H-ECO-BUILD-POWER — stored metal > 80% for 30 s ⇒ add a construction turret
or lab rather than wait for the 500-metal test in H-ECO-MORE-LABS)

### K-mech-build-power-ratio
**Claim.** Official rule of thumb: about 200 build power (one construction turret, or 2+ constructor bots) for every +5 M/s
and +100 E/s of income. Reference build power: commander 300, bot lab 150, advanced lab 600, constructor bot 80-85,
constructor vehicle 90, advanced constructor bot 210-220, rez bot 200, construction turret 200 (230 M, 3200 E, reach 400,
immobile). Per metal, the turret (1.15 M per build power) is cheaper than a constructor bot (1.4-1.5) and cannot wander off.
At our +38 M/s the rule asks for ~1500 build power in total.
**Status.** reported (2026-09-19) for the ratio; build-power numbers supported from local source
**Evidence.** https://www.beyondallreason.info/guide/in-depth-look-at-economy (official, undated).
`workertime` in `units/armcom.lua`, `ArmBuildings/LandFactories/armlab.lua`, `armalab.lua`, `ArmBots/armck.lua`,
`CorBots/corck.lua`, `ArmVehicles/armcv.lua`, `ArmBots/T2/armack.lua`, `ArmBots/armrectr.lua`, `ArmBuildings/LandUtil/armnanotc.lua`.
**Would be wrong if.** Our logs showed metal never pooling (stored metal < 20%) — then build power is not our bottleneck and
more of it only deepens stalls.
**Used by.** (candidate: H-ECO-BUILD-POWER — target build power = 40 × metal income; add turrets next to labs)

### K-mech-turrets-assist-in-radius
**Claim.** A construction turret helps anything within 400 elmos: it assists factories, builds/repairs, and reclaims. Placed
between two labs or beside lab + base buildings it raises production without pathing. It can be given a guard/assist order
on a factory like any builder. Low-priority ("passive") builders only draw resources left over after normal builders, which
lets assistants yield during a stall instead of slowing everything.
**Status.** supported (2026-09-19) for reach and the passive-priority gadget (local source); usage advice reported
**Evidence.** `units/ArmBuildings/LandUtil/armnanotc.lua` (builddistance 400, workertime 200);
`luarules/gadgets/unit_builder_priority.lua:2-23,59-62` (command `priority`; "low priority cons build either at their full
speed or not at all"). https://www.crdhq.com/articles/bar-build-order-metal-solar ("1-2 nano turrets next to factory").
**Would be wrong if.** A turret ordered to guard a lab 300 away did not raise that lab's output.
**Used by.** (candidate: turrets guard the lab; consider setting turrets passive — needs the custom `priority` command id in
the shim)

### K-mech-factory-repeat
**Claim.** Factories have a repeat state: with repeat on, each finished unit's order is re-appended, so a short composition
queue runs forever without the bot re-issuing orders. Guides treat "factory always on repeat, never idle" as a basic rule.
**Status.** reported (2026-09-19)
**Evidence.** https://www.crdhq.com/articles/bar-build-order-metal-solar; https://www.crdhq.com/articles/bar-beginner-guide-getting-started.
Engine command `CMD_REPEAT` (standard Spring/Recoil; not checked in our shim).
**Would be wrong if.** A lab with repeat on and a 5-unit queue went idle after 5 units.
**Used by.** H-PROD-BATCH (candidate: use repeat for the steady-state batch and only rewrite the queue when the composition
changes; interacts with K-rules-factory-shift-means-five)

### K-mech-reclaim-values
**Claim.** A dead unit leaves a wreck worth 60% of its metal cost; a wreck that takes enough damage becomes a heap worth 25%.
Reclaiming wrecks and features costs no energy and is gradual (metal arrives as you go); several builders can work one wreck.
Reclaiming your own live unit returns 100% of its metal, all at the end. Resurrecting costs 0 metal and 50% of the unit's
energy cost. Repair is free. The commander's wreck holds 1250 M. After an even mid-map trade of 40 units the field holds on
the order of 2000-3000 M — whoever holds the ground afterwards collects it.
**Status.** supported (2026-09-19) — local source; the mid-map estimate is arithmetic
**Evidence.** `gamedata/alldefs_post.lua:556-569` (wreck_metal_ratio 0.6, heap_metal_ratio 0.25); `gamedata/modrules.lua:21-39`
(reclaimMethod 0, unitMethod 1, unitEfficiency 1, featureEnergyCostFactor 0, repair energyCostFactor 0, resurrect 0.5);
`units/armcom.lua:114` (dead metal 1250). Web agrees: https://www.beyondallreason.info/guide/reclaim-resurrect-repair.
**Would be wrong if.** A logged area-reclaim over a battle site yielded far less than 0.6 × the metal of units lost there.
**Used by.** (candidate: H-ARMY-RECLAIM — after a fight within our half or the middle, send rez bots/constructors to
area-reclaim; BARb gets that metal otherwise. Relates to K-army-piecemeal-midmap: our mid-map trades feed whoever reclaims)

### K-mech-dgun
**Claim.** The commander's D-gun: range 250, 500 energy per shot, 0.9 s reload, damage 99999 to everything except commanders
(0), small area (36); it is a manual-fire weapon (`commandfire`), so it never fires unless explicitly ordered. The commander's
automatic laser has range 300, 75 damage per 0.4 s. With 3700 hp, 5 hp/s self-repair and the D-gun, a commander with ≥500
stored energy beats any small T1 group that comes within 250 — raiders (range 180-215) must — but not rocket bots (475) that
keep their distance.
**Status.** supported (2026-09-19) — local source; tactical reading is ours, untested
**Evidence.** `units/armcom.lua:4,187-205,258-287` (autoheal 5; laser range 300; disintegrator commandfire, energypershot 500,
range 250, reloadtime 0.9, damage default 99999 / commanders 0). Web agrees on 500 E:
https://www.beyondallreason.info/commands/dgun.
**Would be wrong if.** The AI interface cannot issue the manual-fire command, or a D-gun order with <500 stored energy still fired.
**Used by.** H-COM-RETREAT (candidate: H-COM-DGUN — enemy ground unit within 240 and stored energy ≥ 500 ⇒ D-gun it, highest
metal cost first; keep 500 E reserved while enemies are near home)

### K-mech-commander-blast
**Claim.** A dying commander explodes for 5000 damage at the centre falling to 0 at the edge (edge effectiveness 0) of an
area of effect of 700. Spring weapon defs give area of effect as a diameter, so the radius is 350 and T1 units (≤1600 hp)
die within roughly 240. In a 1v1 the opposing commander is protected: a gadget caps
the blast damage it takes at 33% of its current health, so the game cannot end in a double kill. Our own units and buildings
get no such protection from our own commander's blast.
**Status.** supported (2026-09-19) — local source
**Evidence.** `weapons/Unit_Explosions.lua:841-856` (AreaOfEffect 700, edgeeffectiveness 0, default 5000; the diameter convention is
engine knowledge, not checked in Recoil source), `units/armcom.lua:24,48`; `luarules/gadgets/game_preventcombomb.lua:100-130`.
**Would be wrong if.** A logged enemy-commander death next to our army killed units more than ~350 away.
**Used by.** (candidate: H-ARMY-COM-SNIPE — when the enemy commander is below ~30% health, finish it with ranged units and
pull melee units back beyond 400; never fight next to our own low-health commander)

### K-mech-height-and-lasers
**Claim.** Ballistic weapons (plasma, unguided rockets) gain range firing downhill and lose it uphill; lasers lose damage
with distance (100% at point blank to 50% at max range); units take bonus damage (up to 2x) when hit from directions other
than the one they have been absorbing fire from (flanking bonus, engine default mode 1).
**Status.** reported (2026-09-19); flanking mode 1 default confirmed in `gamedata/modrules.lua:46-47`, magnitudes not checked
**Evidence.** https://www.beyondallreason.info/guide/important-knowledge-on-advanced-mechanics (official, undated).
**Would be wrong if.** Logged damage per Grunt shot was constant with distance.
**Used by.** (candidate: attack from two directions when the group is ≥ 16; place towers on high ground)

### K-mech-experience
**Claim.** Units gain experience from damage dealt relative to target value; experience raises max health (healthScale 2.5) and rate of fire (reloadScale 1.25) along a saturating `xp/(xp+1)` curve, not damage.
Only one unit type (Gunslinger `armmav`) gains range from experience. Veteran units are worth preserving, but the effect is
small for T1 units that die in one fight; it is not a reason to change rules now.
**Status.** supported (2026-09-19) for the settings (local source); the exact bonus formula is the engine's and was not verified
**Evidence.** `gamedata/modrules.lua:5-10,123-127` (experienceMult 0.3, powerScale 0, healthScale 2.5, reloadScale 1.25);
`luarules/gadgets/unit_xp_range_bonus.lua`; `rangexpscale` appears only in `units/ArmBots/T2/armmav.lua`.
**Would be wrong if.** (low value; no test proposed)
**Used by.** (none)

### K-mech-buildings-explode
**Claim.** Buildings explode when killed and can chain: energy storage and fusion blasts are large, converters and wind
turbines are fragile (167-220 hp). Advice: spread generators and converters rather than packing them, and keep converters
away from labs.
**Status.** reported (2026-09-19); health values supported from local source
**Evidence.** https://www.beyondallreason.info/guide/important-knowledge-on-advanced-mechanics (energy storage self-destruct
1280 damage in 260 AoE); https://www.crdhq.com/articles/bar-new-player-guide-getting-started. `armmakr.lua` (health 167),
`armwin.lua` (196).
**Would be wrong if.** Raids on our generator field in logs killed only what they shot at.
**Used by.** (candidate: building placement keeps ≥ 1 footprint gap between converters/turbines)

### K-mech-builder-trip-overhead
**Claim.** A mobile builder's time from finishing one building to laying the next frame is the straight-line walk (distance
minus build reach, at the unit's `speed` in elmos/s) plus a fixed loss: median 1.2-1.5 s when the next site is already in
reach, 2.4-3.4 s when it has to walk; on walks over 1000 elmos the total is 8 % above straight-line time. Build reach is
measured to the target's edge, roughly `builddistance` + 40. With these, `buildtime / workertime` predicts our first
factory's finish within 0.7 s (mean absolute, 12 games), which also checks K-open-build-times.
**Status.** supported (2026-09-19) on Quicksilver, near home; our bot's orders (0.5 s tick) are part of the fixed loss
**Evidence.** 565 commander and constructor trips in the 12 records of `v15-terrain-medium`; `buildorder calibrate`
(`../studies/data/calibration.md`). Far trips across cliffs are barely sampled.
**Would be wrong if.** On another map or for far outposts the recorded trip times exceeded this by more than ~20 %.
**Used by.** `crates/buildorder` (`Scenario::detour`, `mobile_overhead`, `walk_overhead`, `reach_bonus`).

### K-mech-upkeep-has-no-priority
**Claim.** An extractor's energy upkeep stands in the same queue as construction: with stored energy at zero extractors
stop (income 22 to 14 with 11 extractors standing), while a build that costs no energy (a solar collector) goes on at
full speed through the stall.
**Status.** measured (2026-09-20) for the extractors (8 games); the solar half is the engine's rule as we read it
(each builder is refused only the resources its own target needs) and fits the recorded recoveries, not isolated.
**Evidence.** `open-cal2-*` records around 4:30-5:00; `crates/buildorder/src/sim.rs` step 3 models both, and the
simulator's metal-income bias at minute 5 went from +2.0 / +2.4 metal/s to +1.1 / -1.1 (Quicksilver / Mithril).
**Would be wrong if.** A cheat-spawned test (extractors, zero energy, no builders) kept paying metal.
**Used by.** `crates/buildorder` (the opening search's model).


### K-engine-damage-direction-from-unseen-attackers
**Claim.** The AI interface lists no projectiles, but its damage event carries the weapon definition and a unit vector
from the hit unit toward the attacker's (radar-error) position whether or not the attacker is in our sight; only the
attacker's id is withheld when it is not. With the weapon's range from the callback, one hit gives a bearing and a
reach, and two hits on units standing apart give a point.
**Status.** read from the engine source (2026-09-22); the bearings' accuracy is pianist-player-9's to show.
**Evidence.** `rts/ExternalAI/EngineOutHandler.cpp` `UnitDamaged`: `attackeeDir` is set whenever `attacker` is not
null, and only `visibleAttackerUnitId` depends on LOS or radar; `AISEvents.h` `SUnitDamagedEvent` has `dir_posF3`
and `weaponDefId`.
**Would be wrong if.** The engine zeroed the direction for an unseen attacker (it does not: the check is on the
pointer, not on visibility) or the radar error put the bearing far off (it is the attacker's error position).
**Used by.** H-HANDS-SHELLED.
