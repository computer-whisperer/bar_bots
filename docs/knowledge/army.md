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
