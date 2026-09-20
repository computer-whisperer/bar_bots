# Sonnet field commander, first full game (2026-09-19)

Medium BARb, north-west start, Armada, lockstep turns at arena speed 20. Lost at 16.8 game minutes; 72 turns, median 10.7 s,
17.5 minutes of wall time, +2 points of the 5-hour window on the second Max account, no paid usage.
Source: `run/matches/*commander-1/00/strategist-0.jsonl` (git-ignored). Below: every turn's wake reason, tool calls and closing sentence;
reports and tool results are omitted.


**[1:09] Woken because: 30 s have passed; our first factory is up**
- note: Plan: raids start ~min4, target outermost extractors first (bot AI). Our extractors cluster near C1/C2 (2000,1008)/(2160,816), start C2(2032,1188). Will build a defense squad from first raiders/skirmishers, post near C1 (most exposed, northeast of our cluster) once we have ~4-6 units, plus request a turret there. Keep economy_focus=expand until then.
- `set_directives` {"economy_focus": "expand", "min_constructors": 4, "ttl_seconds": 300}
- says: Set economy to expand with 4 constructors minimum, and will be woken once we have 4 raiders (armpw/armflea) to form the first defense squad near our C1/C2 extractor cluster, or sooner if anything happens.

**[2:09] Woken because: 60 s have passed**
- says: Nothing urgent yet, just building up economy and a couple raiders; extending the quiet window to 90s while we approach the raid window.

**[3:03] Woken because: enemies within 600 of our extractors at C3, C3**
- `squad` {"name": "southguard", "near": {"x": 2230, "z": 2250}, "post": {"radius": 500, "x": 2230, "z": 2250}, "take": {"armpw": 4, "armrock": 2}}
- `request_turret` {"x": 2230, "z": 2250}
- says: Formed "southguard" (4 armpw + 2 armrock) posted at the threatened C3 extractor cluster and requested a turret there to cover both undefended spots.

**[3:08] Woken because: squad engaged: southguard; an extractor was destroyed**
- `request_turret` {"x": 2144, "z": 2144}
- says: Requested a second turret at the remaining threatened C3 extractor and staying alert for the outcome of southguard's engagement with corak.

**[3:13] Woken because: enemies within 600 of our extractors at C3; squad engaged: southguard**
- `squad` {"name": "southguard", "post": {"radius": 700, "x": 2260, "z": 2500}}
- `request_turret` {"x": 2296, "z": 2936}
- says: Repositioned southguard's post further south/wider to cover the remaining C3 extractor and the C4 spot the scout is now probing, plus requested a turret there.

**[3:38] Woken because: 25 s have passed**
- says: Quiet for now, southguard holding at full health covering the southern extractor cluster; extending the wait.

**[3:45] Woken because: enemies within 600 of our extractors at C3**
- says: Turret now covers that extractor; letting it handle the lone scout and checking back shortly.

**[3:51] Woken because: enemies within 600 of our extractors at C2**
- `request_turret` {"x": 2000, "z": 1008}
- says: Requested a turret at our home C2 extractor since the scout is now probing it undefended; home group should also be close enough to react.

**[4:16] Woken because: 25 s have passed**
- `squad` {"name": "southguard", "near": {"x": 2260, "z": 2500}, "take": {"armpw": 1, "armrock": 2}}
- says: Reinforced southguard with the nearby stray units and settling back to a longer watch since the scout has moved off.

**[5:16] Woken because: 60 s have passed**
- `set_directives` {"economy_focus": "expand", "min_constructors": 4, "ttl_seconds": 300}
- `squad` {"name": "northguard", "near": {"x": 2080, "z": 912}, "post": {"radius": 500, "x": 2080, "z": 912}, "take": {"armpw": 1, "armrock": 2}}
- `request_turret` {"x": 2160, "z": 816}
- `request_turret` {"x": 2000, "z": 1008}
- `request_turret` {"x": 2320, "z": 2352}
- says: Created "northguard" to cover our undefended home extractors (C1/C2) and requested turrets there plus for the last uncovered C3 spot; south is already solid with southguard + the bot's home group.

**[5:52] Woken because: enemies within 600 of our extractors at B4**
- `request_turret` {"x": 1360, "z": 2816}
- says: Requested a turret for the new B4 extractor the scout is probing; too far for existing squads to cover, so turret is the efficient answer here.

**[5:58] Woken because: an extractor was destroyed**
- `squad` {"name": "southguard", "post": {"radius": 950, "x": 1810, "z": 2650}}
- says: Widened and recentred southguard's post to cover the C3/C4/B4 cluster in one radius, since a solo scout is sniping newly-built extractors faster than turrets complete.

**[6:03] Woken because: squad engaged: southguard**
- says: Southguard is now engaged with corak; letting the fight play out and will check the result shortly.
- says: Ending the turn now to let the engagement resolve.

**[6:33] Woken because: 30 s have passed**
- note: Killed the early lone corak scout with southguard (posted wide radius over C3/C4/B4 cluster). Enemy AI raids new/undefended extractors as they complete, often faster than turret can finish - so wide-radius mobile squads posted near expansion clusters beat reactive single-point turrets. Economy now healthy (6 extractors, +43 metal/s). Watching for the real ~min4+ raid wave next.
- says: Threat cleared, economy strong; noted the lesson and extending the quiet window while we watch for the next, bigger raid wave.

**[7:05] Woken because: enemies within 600 of our extractors at D4**
- `request_turret` {"x": 2944, "z": 2816}
- `squad` {"name": "eastguard", "near": {"x": 2944, "z": 2816}, "post": {"radius": 600, "x": 2944, "z": 2816}, "take": {"armpw": 4, "armrock": 2}}
- says: Split off a new "eastguard" squad for the D4 expansion (too far east for southguard's radius) plus requested a turret there.

**[7:10] Woken because: squad engaged: eastguard; an extractor was destroyed**
- `squad` {"name": "southguard", "order": {"kind": "fight", "x": 2999, "z": 3043}}
- `set_directives` {"army_stance": "attack", "attack_target": {"x": 2999, "z": 3043}, "ttl_seconds": 60}
- says: A real 5-unit raid group hit D4; sent southguard to reinforce eastguard and committed the 18-strong home group to attack the same spot.

**[7:30] Woken because: 20 s have passed**
- `squad` {"name": "southguard", "post": {"radius": 700, "x": 3150, "z": 3040}}
- `request_turret` {"x": 3344, "z": 3264}
- `set_directives` {"army_stance": "defend", "attack_target": null, "ttl_seconds": 300}
- says: Raid repelled with one extractor lost; repositioned southguard to cover the newest D4 spot, requested its turret, and pulled the home group back to defend rather than chase into empty ground.

**[7:35] Woken because: enemies within 600 of our extractors at D4; squad engaged: southguard; We lost 2 extractors in the last minute.; an extractor was destroyed**
- `squad` {"name": "eastguard", "order": {"kind": "fight", "x": 3361, "z": 3386}}
- `set_directives` {"army_stance": "attack", "attack_target": {"x": 3361, "z": 3386}, "ttl_seconds": 60}
- says: Sending eastguard to reinforce southguard against the 4-strong raid and committing the 19-unit home group to converge on the same spot.

**[7:55] Woken because: 20 s have passed**
- `set_directives` {"army_stance": "defend", "attack_target": null, "ttl_seconds": 300}
- `squad` {"name": "eastguard", "post": {"radius": 600, "x": 3354, "z": 3370}}
- `squad` {"name": "southguard", "near": {"x": 2260, "z": 2500}, "take": {"armpw": 3, "armrock": 2}}
- says: Raid quieted down; pulled home group back to defend, gave eastguard a standing post at its current forward position, and folded stray units into southguard.

**[8:00] Woken because: enemies within 600 of our extractors at C5; squad engaged: eastguard, southguard; an extractor was destroyed**
- `squad` {"name": "eastguard", "order": {"kind": "move", "x": 2700, "z": 2700}}
- `squad` {"name": "southguard", "order": {"kind": "move", "x": 2700, "z": 2700}}
- `set_directives` {"army_stance": "attack", "attack_target": {"x": 2700, "z": 2700}, "ttl_seconds": 60}
- says: A major combined enemy push (heavy corthud/corstorm group plus a second corak pack, ~19 units total) hit two fronts at once and was overpowering eastguard; pulling both squads back to rally with the home group at one point to fight as a consolidated force rather than piecemeal.

**[8:12] Woken because: enemies within 600 of our extractors at C4**
- `squad` {"name": "eastguard", "order": {"kind": "move", "x": 2032, "z": 1188}}
- says: Pulling eastguard's survivor home to safety; southguard holds near the rally point where the 19-strong home group is converging to merge into one force.

**[8:17] Woken because: enemies within 600 of our extractors at C4, B4**
- says: Turrets should handle the scattered corak packs at B4/C4; letting southguard and the merging home group continue while eastguard's last unit is lost.

**[8:26] Woken because: an extractor was destroyed**
- `squad` {"name": "southguard", "order": {"kind": "move", "x": 2300, "z": 2000}}
- `set_production` {"weights": {"armham": 30, "armjeth": 15, "armpw": 15, "armrock": 20, "armwar": 20}}
- `set_directives` {"army_stance": "gather", "army_station": {"x": 2300, "z": 2000}, "attack_target": null, "economy_focus": "defence", "ttl_seconds": 180}
- says: A heavy corthud/corstorm stack (10 units) is overrunning our southern extractor ring; falling back southguard and the home group to consolidate near our defended C1-C3 core, shifting production toward heavier units (armham/armjeth/armwar) and defence-focused economy to rebuild before pushing back out.

**[8:32] Woken because: We lost 3 extractors in the last minute.**
- says: Southguard is retreating toward our turreted core where it can fight alongside fixed defenses; monitoring closely as metal has bottomed out.

**[8:37] Woken because: an extractor was destroyed**
- `squad` {"name": "southguard", "post": {"radius": 500, "x": 2300, "z": 2150}}
- says: Southguard has reached our turret cluster; giving it a standing post there so it holds the line with fixed defense support instead of continuing to retreat.

**[8:43] Woken because: enemies within 600 of our extractors at E3**
- `request_turret` {"x": 3863, "z": 2172}
- says: Southguard is stable and holding the core; requested a cheap turret for the exposed new eastern extractor rather than risk overextending our recovering squads.

**[8:49] Woken because: an extractor was destroyed**
- says: Letting the economy stabilize around our defended core while the enemy scout mops up the last exposed eastern extractor.

**[8:54] Woken because: enemies within 600 of our extractors at E3; an extractor was destroyed**
- says: Not worth chasing a lone scout 1.9k out with our recovering main force; letting that outlying spot go for now while we consolidate.

**[8:59] Woken because: enemies within 600 of our extractors at C3, C3; squad engaged: eastguard, southguard**
- `squad` {"name": "eastguard", "release": true}
- `squad` {"name": "southguard", "near": {"x": 2300, "z": 2150}, "take": {"armrock": 3}}
- `request_turret` {"x": 2000, "z": 1008}
- `request_turret` {"x": 1248, "z": 3776}
- says: Cleaned up the dead eastguard squad, reinforced southguard, and requested turrets for the two remaining uncovered extractors (home C2 and B5) while southguard fights off the corstorm/corthud pair at our core.

**[9:04] Woken because: enemies within 600 of our extractors at C3, C3, B5; an extractor was destroyed; an extractor was destroyed**
- `set_directives` {"army_stance": "defend", "army_station": {"x": 2300, "z": 2150}, "ttl_seconds": 300}
- says: Switched from gather (which suppresses reacting to raiders) back to defend and moved the home group's station right onto our core cluster so it engages the remaining corthud alongside southguard.

**[9:09] Woken because: enemies within 600 of our extractors at E3**
- says: Core is holding (corthud killed, southguard healthy); not worth spending scarce metal chasing the far-off E3 skirmish right now — letting it ride while we rebuild.

**[9:14] Woken because: an extractor was destroyed**
- `squad` {"name": "huntsquad", "near": {"x": 2256, "z": 1945}, "order": {"kind": "fight", "x": 3971, "z": 2361}, "take": {"armpw": 2, "armrock": 4}}
- says: Pulling 6 units from the idle home group to hunt down the persistent lone corak at E3 that's been repeatedly wiping our rebuild attempts there — cheaper to kill it once than keep losing constructors.

**[9:23] Woken because: enemies within 600 of our extractors at E3**
- says: Huntsquad is still assembling from fresh production; letting it build up before it moves out to deal with the E3 nuisance.

**[9:32] Woken because: We lost 6 extractors in the last minute.**
- `squad` {"name": "huntsquad", "release": true}
- `set_directives` {"economy_focus": "production", "min_constructors": 3, "ttl_seconds": 300}
- says: We keep bleeding constructors/mexes on suicidal rebuild attempts at exposed spots; switching economy focus to production (stop chasing risky expansion) so metal goes into a stronger army instead, while our turreted core holds.

**[9:39] Woken because: an extractor was destroyed**
- `squad` {"name": "northguard", "near": {"x": 2080, "z": 912}, "take": {"armham": 2, "armrock": 1}}
- says: Reinforced northguard with the freshly-unassigned armham/armrock as a scattered corstorm probes toward our western approach.

**[9:44] Woken because: enemies within 600 of our extractors at C3; Our base is under attack: 3 enemies near B3.**
- `squad` {"name": "southguard", "order": {"kind": "fight", "x": 1619, "z": 1876}}
- `set_directives` {"army_stance": "attack", "attack_target": {"x": 1619, "z": 1876}, "ttl_seconds": 45}
- says: Base is under direct attack; sending southguard to reinforce northguard and committing the 20-strong home group to converge on the intruders at B3.

**[9:59] Woken because: 15 s have passed**
- `squad` {"name": "southguard", "post": {"radius": 700, "x": 2100, "z": 1600}}
- `set_directives` {"army_stance": "defend", "attack_target": null, "ttl_seconds": 300}
- says: Base threat cleared (both corstorm killed); repositioned southguard to a central post between our two remaining clusters and stood the home group back down to defend.

**[10:54] Woken because: enemies within 600 of our extractors at C3; squad engaged: southguard**
- `squad` {"name": "southguard", "near": {"x": 2100, "z": 1600}, "take": {"armham": 1, "armwar": 1}}
- says: Adding the newly available armham/armwar into southguard for extra punch as it handles the lone corstorm.

**[10:59] Woken because: Our base is under attack: 3 enemies near B3.; enemies within 600 of our extractors at C3**
- says: Southguard is strong enough (16 units, 94% health) to handle this 3-unit push alone; watching for the outcome.

**[11:09] Woken because: enemies within 600 of our extractors at C3, C3**
- `set_directives` {"army_stance": "attack", "attack_target": {"x": 1718, "z": 1896}, "ttl_seconds": 45}
- says: A new 3-corthud wave is hitting near our base while southguard is already worn down; committing the 22-strong home group to smash it now.

**[11:14] Woken because: an extractor was destroyed**
- `squad` {"name": "southguard", "order": {"kind": "fight", "x": 2144, "z": 2144}}
- `set_directives` {"army_stance": "defend", "army_station": {"x": 2200, "z": 2000}, "attack_target": null, "ttl_seconds": 300}
- note: 11:14 - Extractor lost at/near C3 despite 4 turrets there: southguard had chased a lone corthud out to B3, outside its post radius, leaving the C3 cluster (hit by 11 corak) uncovered. Lesson: mobile squads must not chase singles far from their post while a bigger raid is inbound elsewhere - consider tighter radius or splitting a small quick-reaction unit for stragglers. Redirected southguard back to fight at C3, switched home group to defend/station at C3 cluster (2200,2000) instead of the stale attack_target, to avoid the bot's late/strung-out charge failure mode.
- says: Redirected southguard to intercept the 11-corak raid at C3 and set the home group to defend/station there instead of chasing a stale attack target, then set a short wake to watch the fight resolve.

**[11:34] Woken because: 20 s have passed**
- `squad` {"name": "southguard", "post": {"radius": 600, "x": 2144, "z": 2144}}
- `request_turret` {"x": 2000, "z": 1008}
- `set_directives` {"army_stance": "defend", "army_station": {"x": 2080, "z": 1300}, "economy_focus": "defence", "ttl_seconds": 300}
- note: 11:34 - We've lost every outlying extractor (B3/B4/B5/C4/C5/E3) to sustained raids since ~8:00; down to the original core 3 (C1/C2/C3), which are currently safe. This bot AI raids all expansions simultaneously/continuously from min ~8, not just one at a time - single reactive squads can't cover far-flung expansions. New plan: stop wide expansion for now (economy_focus=defence), consolidate defenders (northguard, southguard) tightly around the core C1/C2/C3 cluster with posts/turrets, let the home group rebuild as a real reserve at the cluster centre, then re-expand only outward in escorted/turreted steps once the core + army is solid, not into open ground.
- says: We've lost all outlying expansions to continuous raids; consolidating defence tightly around the remaining core C1/C2/C3 cluster (posted southguard, requested the missing C2 turret, switched economy to defence-focus) before any further expansion.

**[11:50] Woken because: enemies within 600 of our extractors at C5**
- `request_turret` {"x": 2336, "z": 4208}
- note: 11:50 - Bot built a new remote extractor at C5 (2336,4208), ~3000 from home, immediately hit by 2 corak with no turret. Consistent with plan: not diverting core squads (northguard/southguard) 3000 away to save one remote, undefended mex - accepting likely loss. Requested a turret there anyway in case it survives/we reclaim the spot later, but real fix is economy_focus=defence should stop these lone remote builds; will watch if it keeps happening.
- says: Not diverting core defenders for one remote, undefended mex 3000 out — requested a turret there as a cheap hedge and holding the consolidated core.

**[11:55] Woken because: We lost 2 extractors in the last minute.; an extractor was destroyed**
- says: Expected loss at C5, core cluster untouched — extending the wait since things are stable.

**[12:06] Woken because: enemies within 600 of our extractors at C3; squad engaged: southguard**
- says: Southguard (13 units) easily handles the lone corstorm at C3 — no action needed, continuing to watch.

**[12:18] Woken because: enemies within 600 of our extractors at C3; squad engaged: southguard**
- says: Southguard (9 units) should handle the 3 corthud fine; watching closely with a short wait given it's already worn to 67%.

**[12:27] Woken because: Our base is under attack: 5 enemies near C3.**
- `squad` {"name": "southguard", "order": {"kind": "move", "x": 2020, "z": 1050}}
- `set_production` {"weights": {"armham": 20, "armjeth": 10, "armpw": 10, "armrock": 15, "armwar": 45}}
- note: 12:27 - C3 overrun by a mixed wave (4 corthud, 2 corstorm, plus a captured armrock) - southguard wiped from 9 units to 12% hp, likely losing C3. Pulling southguard back to regroup with northguard at the C1/C2 core rather than feed it in piecemeal to a losing fight. Shifted production toward heavier armwar (was too much cheap armham/armrock, which loses straight-up to corthud/corstorm). Accepting C3 may fall; priority is holding C1/C2 and rebuilding a real combined force before pushing back out.
- says: Pulling southguard's survivors back to regroup at the core with northguard rather than lose them piecemeal, and shifting production toward heavier armwar to actually stand up to the corthud/corstorm wave.

**[12:40] Woken because: an extractor was destroyed**
- `squad` {"name": "southguard", "release": true}
- `set_production` {"weights": {"armham": 35, "armjeth": 10, "armpw": 35, "armrock": 20}}
- `set_directives` {"army_stance": "defend", "army_station": {"x": 2060, "z": 1000}, "ttl_seconds": 300}
- `request_turret` {"x": 2032, "z": 1188}
- note: 12:40 - southguard wiped, C3 extractor lost; down to 3 extractors (C1,C2 core + distant unguarded B6) and only 7 army units total. Metal income (~9.4/s) too low to afford 270m armwar reinforcements fast enough, so reverted production to cheap armpw/armham/armrock for quick numbers; will shift back to armwar once economy/army recover. Losses "at home" (even an armmex at home) mean raiders are reaching the actual base, not just outer extractors - pulled army_station tighter to (2060,1000) right at core, and put a turret request at home itself. Priority now: survive and rebuild around C1/C2 before any further expansion.
- says: Southguard is gone, economy is critical; switched to cheap unit production for fast numbers, pulled the army station tight onto the core, and put a turret request right at home base since raiders are reaching it directly.

**[12:51] Woken because: We lost 2 extractors in the last minute.**
- `set_production` {"weights": {"armflea": 35, "armham": 20, "armpw": 30, "armrock": 15}}
- `request_turret` {"x": 1900, "z": 1400}
- says: A large 14-unit force is closing on our last core extractors (C1/C2); shifted production to cheap fast armflea/armpw for immediate numbers and added a turret on the approach, watching closely now.

**[12:56] Woken because: enemies within 600 of our extractors at C2**
- `squad` {"name": "northguard", "order": {"kind": "fight", "x": 2000, "z": 1008}}
- says: Multi-pronged raid closing on C2 from several sides — pulling northguard down from C1 to intercept the 6 corak right at C2 since that's the most immediate threat to our last real income.

**[13:11] Woken because: 15 s have passed**
- `squad` {"name": "northguard", "post": {"radius": 400, "x": 2020, "z": 1100}}
- `set_directives` {"army_stance": "defend", "army_station": {"x": 2020, "z": 1100}, "economy_focus": "production", "ttl_seconds": 300}
- note: 13:11 - CRITICAL: both labs destroyed (13:00-13:10), armlab/armmakr/armck lost repeatedly at home to raids that reached the base itself while northguard was pulled forward. We're down to 6 army units (northguard), 3 constructors, no factory. Switched economy_focus=production to get constructors rebuilding a lab ASAP, pulled northguard into a tight defensive knot right on top of the C2 turret cluster (2020,1100 r400) instead of sallying out, to turtle under turret cover until a factory exists again. Enemy has 5-9 units (corstorm/corthud heavy) circling within 700 of home - avoid engaging that group in the open with only 6 light units; let turrets do the work.
- says: Critical: both factories destroyed. Turtling northguard tight under our turret cluster and switching to production-focus economy to get a lab rebuilt before we do anything offensive.

**[13:17] Woken because: enemies within 600 of our extractors at C2**
- `squad` {"name": "northguard", "post": {"radius": 700, "x": 2020, "z": 1100}}
- says: Buildings keep dying at home without northguard ever engaging — likely corstorm sniping from beyond the 400 radius. Widening the post to 700 to actually reach them before they finish sniping.

**[13:22] Woken because: squad engaged: northguard**
- says: Northguard is now engaged with the corstorm/corthud group threatening C2 — watching closely to see if it holds.

**[13:31] Woken because: Our base is under attack: 5 enemies near C2.**
- says: Northguard trading well (killed a corstorm and corthud, down to 86%) — letting it keep fighting under the current post while metal stockpiles for the rebuilt lab.

**[13:46] Woken because: 15 s have passed**
- says: A lab is rebuilt and army production resumed, the attack has broken off — extending the wait now that we've stabilized.

**[13:51] Woken because: enemies within 600 of our extractors at C5; an extractor was destroyed; squad engaged: northguard**
- says: Remote C5 lost again as expected (too far to defend); northguard handling a lone corstorm near home — short wait to confirm it holds.

**[13:56] Woken because: enemies within 600 of our extractors at C2**
- `set_directives` {"economy_focus": "production", "min_constructors": 3, "ttl_seconds": 300}
- says: Constructors down to 1 — renewed min_constructors=3 so the bot rebuilds them alongside production focus.

**[14:16] Woken because: 20 s have passed**
- says: No immediate threat to our remaining extractors; enemy group massing at lost C3 territory but not near us — continuing to watch.

**[14:30] Woken because: squad engaged: northguard**
- says: Northguard engaging a small corak group near home — should handle it, watching closely.

**[14:35] Woken because: Our base is under attack: 7 enemies near B2.; enemies within 600 of our extractors at C2**
- says: Second lab back online, northguard holding at 87% against the corak swarm — tight watch continues.

**[14:50] Woken because: 15 s have passed**
- says: Stable now — 2 labs, 4 constructors, core extractors safe, northguard healthy. Current cheap-unit mix is working well against the corak/corstorm raids, so leaving it as is and extending the wait.

**[15:35] Woken because: enemies within 600 of our extractors at B6**
- `set_directives` {"army_stance": "defend", "army_station": {"x": 2020, "z": 1100}, "economy_focus": "expand", "ttl_seconds": 300}
- note: 15:35 - Recovered: 2 labs, 5 constructors, 4 extractors (C1/C2 core + C3 rebuilt with its own turret already + far B6), northguard holding core at 98%. Cautiously switching economy_focus back to expand now that core defence (northguard+turrets) is solid, to rebuild income beyond the ~10/s we've been stuck at. Will watch for the bot re-expanding into undefended far corners (like B6/C5 before) and accept those as low-priority losses rather than diverting core defenders again.
- says: Core defence is solid — cautiously re-enabling expansion to rebuild income while keeping northguard anchored on C1/C2/C3.

**[15:41] Woken because: an extractor was destroyed**
- says: Lost the far, undefended B6 as expected but gained a new one near the core cluster at C3 — net neutral, continuing to watch.

**[15:49] Woken because: We lost 2 extractors in the last minute.; an extractor was destroyed**
- `squad` {"name": "strays", "near": {"x": 1750, "z": 1950}, "order": {"kind": "move", "x": 2020, "z": 1100}, "take": {"armflea": 3, "armpw": 1}}
- note: 15:49 - Enemy has a cornecro (capture-type unit) roaming with corak at B4 - explains earlier sightings of our own unit types (armrock) showing up in enemy compositions, i.e. they capture rather than just kill. Keep squads consolidated, don't let small groups linger isolated near enemy contact. Pulled 4 stray unassigned home-group units back from B3 to the core rather than let them wander into capture range.
- says: Identified a capture-unit (cornecro) explaining earlier "friendly units in enemy composition" sightings, and pulled stray idle units back from B3 to the safety of the core.

**[15:54] Woken because: enemies within 600 of our extractors at C3**
- `squad` {"name": "strays", "release": true}
- says: Strays are safe home, releasing them back to the bot's heuristics; C3's lone corstorm should be handled by its turret.

**[15:59] Woken because: enemies within 600 of our extractors at C3, B4, C4; an extractor was destroyed**
- says: Single corstorm at the turreted C3 extractor should be handled; B4 is another far, undefended outpost we're accepting as expendable — continuing to watch.

**[16:05] Woken because: an extractor was destroyed**
- `squad` {"name": "eastguard", "near": {"x": 2290, "z": 2261}, "post": {"radius": 600, "x": 2250, "z": 2300}, "take": {"armflea": 4, "armpw": 3}}
- says: 3 corstorm converging on the C3 extractor could overwhelm the lone turret — formed eastguard from the idle units already sitting there and posted them to intercept.

**[16:10] Woken because: squad engaged: eastguard; enemies within 600 of our extractors at C3**
- `squad` {"name": "eastguard", "order": {"kind": "move", "x": 2020, "z": 1100}}
- `request_turret` {"x": 2020, "z": 1100}
- note: 16:10 - Major enemy push spotted: ~29 units (9+2 corstorm, 3+9 corthud, plus corak/cornecro) massing at C3/C4/D4/B4, 1600-1900 from home. This dwarfs anything seen before and is mostly heavy corthud/corstorm - our light army (armflea/armpw/armham/armrock, ~24 total) cannot win in the open. Retreating eastguard from C3 back to core rather than let it be wiped for one outpost; requesting another turret right on the core. Plan: turtle everything tight around C1/C2 turret cluster and let them come to us, do not chase or hold forward outposts through this wave.
- says: A large ~29-unit heavy assault force is massing well beyond anything seen so far — pulling eastguard back to the core and reinforcing turrets there rather than fight it forward.

**[16:15] Woken because: Our base is under attack: 8 enemies near C3.**
- `squad` {"name": "eastguard", "order": {"kind": "move", "x": 2020, "z": 1100}}
- `set_production` {"weights": {"armham": 35, "armjeth": 10, "armpw": 20, "armrock": 35}}
- says: Facing heavier corstorm/corthud, shifting production toward sturdier armham/armrock and reissuing eastguard's retreat order.

**[16:20] Woken because: an extractor was destroyed**
- `request_turret` {"x": 2100, "z": 1450}
- note: 16:20 - Enemy assault force has grown to 17 (11 corstorm, 6 corthud) approaching from C3 direction, 1308 from home; we're down to 2 extractors (C1,C2). Our total force ~20 units but mostly light/medium (armpw/armham/armrock) - a hard fight. Consolidating everything onto the C2 turret cluster, requested another turret at (2100,1450) to add a firing line on the southern approach before they reach the core. If this line breaks, fall back further north toward C1 as last-ditch.
- says: A serious 17-unit heavy force is bearing down on our last two extractors — adding a forward turret and consolidating both squads onto the core cluster, watching very closely now.

**[16:35] Woken because: 15 s have passed**
- says: An 18-unit heavy force (mostly corthud) is now only 850 out from home with our defenses already worn down — holding the co-located squads at the core and watching every tick.

**[16:40] Woken because: Our commander is under fire away from home (69% health).**
- note: 16:40 - Commander under fire away from home (69% hp) with no way for me to give it a direct retreat order (no commander/unit-id lever exists, only named squads built from unassigned soldiers, and turret/squad tools). Wish: a way to select/move the commander specifically. Meanwhile an 18-strong corthud/corstorm horde is 765 from home; holding northguard+eastguard at the C2 post, nothing more to spend (0 metal, only 2 turrets left). This may be the fight that decides the game.
- says: Commander is under fire with no direct lever available to redirect it — flagged that gap. Holding both squads at the core against the incoming 18-unit horde with everything we have left.
