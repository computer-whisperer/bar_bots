# Army

### K-army-crowds-are-never-idle
**Claim.** A large group ordered to a rally point is never all idle at once, so "count idle units at the rally point" is a
broken launch condition; group membership has to be tracked explicitly.
**Status.** supported (2026-09-19)
**Evidence.** v2-easy-0704/00: army grew to 148 with only 7 waves sent in 35 minutes. After membership-based waves
(v3-easy/03): 8 waves by minute 18.
**Would be wrong if.** (mechanical; would need an engine change)
**Used by.** H-ARMY-WAVES.

### K-army-fighters-before-constructors
**Claim.** The first factory units should be fighters; an opening of several constructors loses the early game to raids.
**Status.** narrowed (2026-09-19): true of five constructors and no fighters, false of two. Against medium, two
constructors before the first fighter cost nothing in army by minute 4 and led by minute 8 (v21-early-expand-ab10, see
K-open-sim-constructors-first). Confounded at the time with K-rules-factory-shift-means-five (both changed in v4).
**Evidence.** v3-easy/04 (5 constructors, 0 army at minute 4, lost) vs v4-easy wins spread over both corners and factions.
**Would be wrong if.** Constructor-first with correct single-unit orders did as well over 24+ matches.
**Used by.** H-PROD-BATCH.

### K-army-piecemeal-midmap
**Claim.** Our waves meet BARb's army in the middle of the map and trade there; reinforcements arrive one by one and die.
**Status.** conjectured (2026-09-19)
**Evidence.** v5-probe/00: attacker centroid stays around (3500-3900, 3700-4300) on a 7168-wide map, attacker count
oscillating 37 -> 20 -> 34 with few idle.
**Would be wrong if.** Attacker positions showed them reaching the enemy base in strength and losing there instead.
**Used by.** (none yet; motivates regrouping before contact)

### K-army-home-defence-does-not-protect-outposts
**Claim.** Keeping the army at home (our default until a wave is ready, or a `defend` stance) does not protect extractors: raids
kill the outer ones, the constructor sent to rebuild dies too, and the economy freezes at 2-3 extractors.
**Status.** conjectured (2026-09-19) — one strategist game plus the medium diagnosis
**Evidence.** opus-first/00 (docs/transcripts/2026-09-19-opus-first.md): 2-3 extractors and +6-9 metal for 20 minutes with 6
turrets and 8-17 idle bots at home; Opus identified it at 4:55, 7:10 and 8:55 and asked for a directive to station the army
at a map point as an escort. Same shape as the v5-medium losses (K-opp-medium-wins-by-20).
**Would be wrong if.** Stationing the home group forward, between the outposts and the enemy, did not raise extractor survival.
**Used by.** (candidate: army `station` directive / forward rally near the most exposed extractor cluster)

### K-army-waves-chased-raiders
**Claim.** Targeting "the visible enemy nearest the enemy start" sends waves to wherever an enemy was last seen, which is
usually a raider inside our own half; the army then spends the game near home.
**Status.** supported as a description of the old behaviour (2026-09-19); the replacement is untested
**Evidence.** dropped-orders-2/00: attacker centroid (2000-2900, 1300-2400) with home at (2032,1188) through minute 20; earlier
batches logged waves launched at (1593,1804), (2326,2943). Replacement: remembered enemy buildings, nearest to us first,
else the presumed enemy start (H-ARMY-TARGET rewritten).
**Would be wrong if.** With building targets the attacker centroid still stayed in our half.
**Used by.** H-ARMY-TARGET.

### K-army-base-maze
**Claim.** Base buildings placed around one anchor with a 3-square (24-elmo) gap form a maze that a T1 army cannot leave:
units fail their moves inside the base, never reach the station, and waves "launch" without going anywhere.
**Status.** supported (2026-09-19)
**Evidence.** v10-sites: attackers' average position sat within 350 of our start for 7-15 whole minutes in 6 of 8 NW
games (up to 64 attackers parked). nw-stuck-diag (8 NW games): ~17,000 soldier move failures, all within ~500 of our
start; after the yard layout (nw-layout, 8 NW games) 149, none clustered at home. It did not explain the corner gap
(K-maps-quicksilver-corner-asymmetry). The false alarm it raised: station after station was declared
unreachable (the failing units were in the base, not at the station) until the station fell back to the start point
itself, in the middle of the maze.
**Would be wrong if.** Soldier move failures clustered at home again with the yard layout.
**Used by.** H-ECO-BASE-LAYOUT.

### K-army-dead-waves-are-resurrected
**Claim.** BARb easy builds resurrection bots (armrectr / cornecro) and raises our dead attackers in its half; the units
that finally kill our base are partly our own.
**Status.** supported (2026-09-19) — inferred from unit names, not from watching a resurrection
**Evidence.** nw-layout fight ledgers: in all 4 losses the enemy fielded units of OUR faction (7-42 of them killed by us,
2-49 of our losses caused by them); in the 3 wins and the timeout, none. `killed armrectr`, `killed cornecro` appear.
**Would be wrong if.** BARb could build both factions' units by some other route (capture, a shared lab).
**Used by.** (none yet) — candidate: do not feed waves into a defended base; fight where we can reclaim the field.

### K-army-waves-die-to-static-defence
**Claim.** Our waves arrive strung out and die to static defence they never see. In a 40-minute timeout we lost 739
units in the enemy half for 104 kills; 427 of those to attackers we had no sight of, 90 to the enemy commander, 114 to
LLT/HLT/HLLT turrets.
**Status.** supported (2026-09-19) — one game read closely (nw-layout match 01), others look alike
**Evidence.** `run/matches/1789857672-nw-layout/01/bot.log` fight ledger.
**Would be wrong if.** Waves gathered at a staging point before engaging lost just as badly.
**Used by.** (none yet) — candidates: gather before the assault, skip targets under turret cover while the army is
small (BARb's own 0.75-power rule), artillery against turrets, a scout for sight.

### K-army-commander-sniped-after-wave-leaves
**Claim.** Many of our losses are not collapses: an enemy group of ~15 walks into a healthy base just after a wave has
left and kills the commander in 10-20 seconds, which ends the game.
**Status.** supported (2026-09-19)
**Evidence.** v11-layout match 14: wave 7 (32 units) left at f=34560; at f=36105 the commander had 3535 health, at
f=36495 it was dead to a pack of armwar, with 8 extractors, 4 labs and 28 soldiers alive, 10 of them at home. v10-sites
losses 04, 08, 12: the commander dies at home to corthud / corstorm from under 300 away.
**Would be wrong if.** With H-ARMY-RECALL, losses with a healthy economy (8+ extractors at the end) still happened as often.
**Used by.** H-ARMY-RECALL.

### K-army-we-never-raid
**Claim.** Marching every soldier in one group at one target leaves the enemy's extractors alone all game, while BARb
medium's small fast groups strip ours.
**Status.** supported (2026-09-19)
**Evidence.** v13-medium: per game we killed 3.1 enemy extractors from NW and 7.4 from SE, and lost 22.2 and 18.5.
**Would be wrong if.** With raid squads the enemy extractors killed per game did not rise.
**Outcome.** It did not (3.1 -> 4.9, noise): BARb medium guards its extractors with a turret each, and squads of three light
raiders die to them. The claim stands as a description; raiding as a remedy is retired.
**Used by.** (none; H-ARMY-RAID retired)

### K-army-defence-is-positioning-and-mix
**Claim.** In the first Sonnet commander game the loss was mostly defender positioning and unit mix, with a smaller
part played by units that huddled at a cliff edge they could not cross.
**Status.** reported (2026-09-19) by the user from the engine replay (he has StarCraft 2 experience, not BAR).
**Evidence.** `docs/transcripts/2026-09-19-sonnet-commander-1.md`: posts set reactively, after an extractor cluster was
already raided; a posted squad drawn out of its radius; light armpw/armrock/armham against corthud/corstorm. The
huddling predates the terrain work (K-maps-terrain-not-straight-lines): squad posts and orders were straight-line
points, some on the far side of a cliff.
**Would be wrong if.** With posts snapped to walkable ground and a sound mix, the exchange ratio in our half stayed as bad.
**Used by.** (none yet) — positioning and mix are the levers meant for the LLM ([[llm-levers]] in project memory).

### K-army-verdicts-v17
**Claim.** Against BARb medium our losses have two interlocking shapes. (1) Cheap lone raiders (armfav 31 metal, armflash,
corak) farm extractors and constructors while the home group and the turrets stand in one clump at the lab; the bot
rebuilds the same spot into the same raider, up to five times. (2) A wave takes the entire army at a target it has not
weighed, arrives in instalments, dies, and the raid that follows meets nothing at home. Fight quality is secondary:
in two losses the army metal traded 1:1 and the game was still lost on replacement rate (2 extractors against 26).
**Status.** supported (2026-09-19): 14 of 14 losses of one batch, each read by an analyst from curves and scenes; three
claims spot-checked against the raw records by me.
**Evidence.** `run/tally_verdicts.py run/matches/1789868387-v17-truth-medium`: primary cause expansion_raided_undefended 7,
blind_wave_into_defence 3, defenders_out_of_position 3, economy_never_grew 1; defenders_out_of_position contributes in
9 more. Specifics: from the NW start the third and fourth metal spots, (2144,2144) and (2320,2352), lie 963 and 1199
from home, beyond the base turret line (650) and inside OUTPOST_DISTANCE (1200), so no rule ever gives them a turret;
they are the spots killed 3-5 times a game (match 18: 2:44, 5:11, 8:11, 11:50). Recall needs 6 intruders, so a single
raider never triggers it. The wave rule is a head count (20, 25, 30...) that leaves 0 soldiers at home. The
constructor target (3 + extractors/2, earlier 2 + extractors/4) is lowest exactly when extractors are being lost.
In every NW loss "they first have 1.5x our army" falls in minute 6-8, even on level extractors.
**Would be wrong if.** With spots remembered as hot, cover before expansion, a home guard that stays, and a wave gate
on known enemy value, the same tags still led the tally.
**Used by.** (next changes)



### K-army-combat-prediction
**Claim.** Once a fight is decisive (a side with a real force on the spot loses at least half of it), the side with the
higher fighting power, metal value weighted by the duel table with turrets at 3x (1.5x at first: 92 % right; 95 % at
4x, `run/predict_check.py`), loses the smaller share in 9 cases
of 10. The matchup weighting adds only a point or two over plain metal in our games, because both sides field a narrow
set of units. It says nothing about skirmishes and raids, where who loses less is a coin toss (56 %), and nothing about
what we have not seen.
**Status.** supported (2026-09-19)
**Evidence.** `run/predict_check.py` over v17-truth-medium and v18-verdict-fixes (48 games): 273 decisive engagements,
89 % right (plain metal 88 %), 93 % beyond 1.5:1, 94 % beyond 2:1, 96 % beyond 3:1; correlation of log power ratio with
log loss-share ratio 0.85. The selection is by outcome (decisive fights only), so this is the accuracy given that a
fight goes to the finish, not the accuracy of "should we start it".
**Would be wrong if.** With retreat and launch decided by these odds, our share of metal lost in their half did not fall.
**Used by.** H-ARMY-WAVE-GATE, H-ARMY-RETREAT, H-ARMY-RESPONDERS (`brain/combat.rs`).

### K-army-verdicts-v18
**Claim.** With raided spots closed, turrets on the ring and a wave gate, the lone-raider losses are gone and the bot
reaches minute 10 level; what decides games now is (1) one mobile enemy ball of 700-3000 metal that our split-up
defence meets in packets, (2) a wave gate that weighs only what stands at the target while their army is a roaming
block it has never scouted ("1500 known" against a real 3205), (3) our force arriving in speed order, light raiders
first and rocket infantry 600 elmos behind, so we lose fights we outweigh, (4) constructor attrition: 16-31
constructors lost a game, the bigger bill behind every extractor count, and (5) a far line of spots 2000-2400 from
home bought and swept in every game.
**Status.** supported (2026-09-19): 12 losses of v18-verdict-fixes read by three analysts; the production-stall and
global "turret on its way" bugs they reported were confirmed in the code and logs.
**Evidence.** `run/tally_verdicts.py run/matches/1789870160-v18-verdict-fixes`. Bugs found: the army parked in the lab
yard jams the factory exits (1500 metal banked for five minutes with two labs alive, soldiers' moves failing beside the
base); `unguarded_outpost` treated any turret under construction anywhere as cover for every outpost; the
quiet-at-home clause of the gate never clears under continuous raiding.
**Would be wrong if.** After the fixes (station clear of labs, positional cover test, gate ceiling and remembered
army, scout, expansion reach) the same tags led the next tally.
**Used by.** H-ARMY-STATION, H-ARMY-WAVE-GATE, H-ARMY-SCOUT, H-ECO-REACH, H-ECO-OUTPOST-TURRET.

