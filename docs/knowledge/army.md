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
**Status.** supported (2026-09-19), confounded with K-rules-factory-shift-means-five (both changed in v4)
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

