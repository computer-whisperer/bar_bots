# Inbox: public-information entries for existing topics

Proposed 2026-09-19 by the public-info research pass; not reviewed. Entries are grouped by the topic file they would merge
into. Unit paths are relative to `upstream/Beyond-All-Reason/units/`. New-topic entries from the same pass are already in
`openings.md`, `tier2.md`, `units.md`, `scouting.md`, `maps.md`, `mechanics.md`.

## For economy.md

### K-eco-energy-per-metal
**Claim.** Metal cost per +1 E/s, from the unit files: wind 40 / wind-speed (3.1 at Quicksilver's 12.7 average, 13 at its
minimum of 3), geothermal 1.9 (560 M → 300 E/s, vents only), fusion 4.2-4.5, advanced solar 4.4-4.6 (but 4000-5000 E up
front), solar 7.5-7.75 (0 E up front), tidal 90 / tidal-strength. On Quicksilver wind is the cheapest generator at every
stage; our brain builds only solars and advanced solars.
**Status.** supported (2026-09-19) — arithmetic on local unit files and the local map archive; not tested in the arena
**Evidence.** `ArmBuildings/LandEconomy/armwin.lua`, `armsolar.lua`, `armadvsol.lua` (350 M, 5000 E, +80), `armfus.lua`,
`armgeo.lua`, `SeaEconomy/armtide.lua`, Cortex equivalents; `run/data/maps/quicksilver_remake_1.24.sd7` mapinfo (wind 3-17).
Official guide gives advanced solar as +75 (https://www.beyondallreason.info/guide/in-depth-look-at-economy); local file says 80.
**Would be wrong if.** Energy income per metal spent on generators was not higher for a wind brain than for the solar brain
on Quicksilver over 24+ matches.
**Used by.** H-ECO-OPENING, H-ECO-ENERGY-BY-STORAGE, H-ECO-ADV-SOLAR (candidate: all three switch to wind when map average ≥ 8)

### K-eco-adv-solar-not-when-short
**Claim.** Advanced solar is metal-efficient but costs 4000-5000 energy to build, so guides say not to build it when energy is
urgently short — use plain solars (0 E) or wind (175 E) to get out of a stall, advanced solar to grow an already-healthy
energy economy. Our H-ECO-ADV-SOLAR switches on income > 250 regardless of stored energy.
**Status.** reported (2026-09-19); costs supported from local source
**Evidence.** https://www.beyondallreason.info/guide/in-depth-look-at-economy; `armadvsol.lua` (energycost 5000), `coradvsol.lua` (4000).
**Would be wrong if.** Logs showed no deepening of energy troughs while advanced solars were under construction.
**Used by.** H-ECO-ADV-SOLAR (candidate: require stored energy > 50% as well)

### K-eco-metal-bar-band
**Claim.** Healthy play keeps stored metal between ~20% and ~80% of storage and stored energy above 50%. Metal pinned near full
means too little build power or too few factories; metal pinned at 0 with energy fine means too much build power for the
income (stop adding builders, add extractors). Our energy floor is 40% and our spend trigger is an absolute 500 metal.
**Status.** reported (2026-09-19)
**Evidence.** https://www.crdhq.com/articles/bar-economy-guide (2026, third-party).
**Would be wrong if.** Win rate did not change when the spend rules were expressed as fractions of storage.
**Used by.** H-ECO-MORE-LABS, H-ECO-ENERGY-BY-STORAGE

### K-eco-storage-before-t2
**Claim.** Banking for the advanced lab needs room: default storage is 1000 metal plus small amounts per building (extractor
50, lab 100, commander 500). A metal storage adds 3000 for 330 M; an energy storage adds 6000 for 170 M. The community T2 gate
with a bank ("+20 M/s and 1000-1500 banked") fits in default storage by mid-game without a metal storage; anything above
is wasted as overflow.
**Status.** supported (2026-09-19) for the numbers (local source); whether the commander's 500 adds to the modoption's 1000 is unchecked
**Evidence.** `modoptions.lua:2418-2449`; `armmstor.lua` (330 M, +3000), `armestor.lua` (170 M, +6000), `armmex.lua`
(metalstorage 50), `armlab.lua` (100), `units/armcom.lua` (500/500).
**Would be wrong if.** bot.log showed metal storage capacity different from 1000 + 500 + per-building amounts.
**Used by.** (candidate: H-T2-GATE; log metal overflow as a wasted-resource counter)

### K-eco-t2-is-the-public-answer-to-the-ceiling
**Claim.** Public advice agrees with K-eco-t1-ceiling's diagnosis: once free spots run out, the next metal comes from advanced
extractors (4x per spot), then converters; the community gate for starting is +20-30 M/s and +500 E/s. Our brain passes +20
M/s well before minute 17, so by public standards it is 5+ minutes late to tier 2 rather than merely tier-1-limited.
**Status.** reported (2026-09-19)
**Evidence.** See tier2.md K-t2-gate-thresholds, K-t2-moho-first. When our brain crosses +20 M/s and what its energy income is
then have not been read out of bot.log.
**Would be wrong if.** A tier-2 economy brain reaching ≥ +60 M/s by minute 15 still lost to BARb medium at the same time.
**Used by.** (planned tier-2 work)

## For army.md

### K-army-mass-at-a-forward-rally
**Claim.** The public remedies for piecemeal mid-map trading are (a) group and move slow units together, (b) reinforce from a
forward factory or forward rally point so new units join the group instead of walking into the enemy alone, and (c) do not
fight under enemy towers with T1 line units — towers get free damage during the approach. None of the sources gives a numeric
commit rule (army-value ratio); that has to come from our own logs.
**Status.** reported (2026-09-19)
**Evidence.** https://masterbel2.wordpress.com/beyond-all-reason-units-guide-uses-tactics-and-counters/;
https://www.crdhq.com/articles/units-vs-static-defense-counter-system.
**Would be wrong if.** A brain whose reinforcements gather at a rally point ~1500 short of the last contact and join only in
groups of ≥ 8 did not improve the kill/loss metal ratio over H-ARMY-WAVES.
**Used by.** H-ARMY-WAVES (candidate: H-ARMY-RALLY-FORWARD)

### K-army-mex-sniping-numbers
**Claim.** A T1 extractor has 270-275 health and leaves 50 M to rebuild plus lost income; a single Grunt (37 damage per 0.5 s)
kills one in ~4 s, a Pawn (9 per 0.3 s) in ~9 s, a Tick in ~7 s. Three raiders kill an unguarded extractor faster than
any defender outside ~300 elmos can arrive; an LLT beside it (range 430, 75 per 0.47 s) kills a Grunt in ~2 s. This is the
arithmetic behind both raiding and H-ECO-OUTPOST-TURRET.
**Status.** supported (2026-09-19) — arithmetic on local unit files (laser range falloff ignored)
**Evidence.** `ArmBuildings/LandEconomy/armmex.lua`, `CorBots/corak.lua`, `ArmBots/armpw.lua`, `armflea.lua`,
`ArmBuildings/LandDefenceOffence/armllt.lua`.
**Would be wrong if.** Logged raids took much longer per extractor.
**Used by.** H-ECO-OUTPOST-TURRET, H-ARMY-SWEEP (candidate: a standing 4-6 raider group that only targets extractors farther
than 500 from any known tower)

### K-army-commander-is-the-best-early-unit
**Claim.** For the first ~5 minutes the commander is the strongest thing on the field (3700 hp, D-gun, 300 build power) and
public guides describe it as a unit that "builds, fights". With game-ending commander death the accepted compromise in 1v1
is: commander expands and fights near home early, never crosses the middle, and retires behind towers once rocket bots or
artillery (which out-range its 300/250 weapons) appear. Our H-COM-LEASH of 900 is stricter than this.
**Status.** reported (2026-09-19); weak sourcing — general community stance, the only written sources found are
https://www.crdhq.com/articles/bar-unit-guide and https://www.beyondallreason.info/commands/dgun
**Evidence.** As above; commander numbers in `units/armcom.lua`.
**Would be wrong if.** Widening the leash to ~1500 before minute 5 produced any commander death in 24 matches (then keep 900).
**Used by.** H-COM-LEASH, H-COM-RETREAT (candidate: time-dependent leash; retreat trigger when enemy units with range > 300 are in sight)

## For game-rules.md

### K-rules-mex-upgrade-web-support (additional evidence for K-rules-mex-upgrade-in-place)
**Claim.** For human players the upgrade is "select a T2 constructor, right-click a T1 extractor" (or an area-upgrade
command); the local gadget describes itself as insta-reclaiming/refunding the old extractor "when another mex on top has
finished", so the T1 extractor keeps producing during the upgrade and its metal is refunded at the end. Only advanced
constructors have the advanced extractor in their build list.
**Status.** reported (2026-09-19) for the UI path; gadget description read locally, behaviour through the AI interface still untested
**Evidence.** https://www.beyondallreason.info/commands/upgrade-t1-mex; `luarules/gadgets/unit_mex_upgrade_reclaimer.lua` (desc);
`units/ArmBots/T2/armack.lua:46`.
**Would be wrong if.** Income from the spot dropped to zero as soon as the advanced extractor's frame was placed.
**Used by.** (planned tier-2 economy)

### K-rules-no-commander-draw
**Claim.** In 1v1 an exploding commander cannot kill the other commander: blast damage to a commander is capped at 33% of its
current health and it is briefly immune to D-gun. Walking our commander into theirs to force a draw or a trade does not work,
and theirs cannot do it to us.
**Status.** supported (2026-09-19) — local source
**Evidence.** `luarules/gadgets/game_preventcombomb.lua:100-130`.
**Would be wrong if.** A match ended with both commanders dead in the same second.
**Used by.** (none)
