# Heuristic registry

Every rule in `crates/bot/src/brain/` that embodies a judgment about the game. IDs appear in the code at the rule (`self.fire("H-...")` or a comment for rules that only filter), in the per-minute
`rules:` line of `bot.log`, in the arena's wins-vs-losses firing table, and in knowledge entries. Status: `active`, `retired (date, why)`.

| ID | Rule | Code | Rests on | Status |
|---|---|---|---|---|
| H-COM-LEASH | Commander builds only within 900 elmos of home | `economy.rs` `claim_spot`, `COMMANDER_LEASH` | K-rules-commander-death-ends-game | active |
| H-COM-RETREAT | Commander under 70% health, just damaged and away from home walks home | `mod.rs` `protect_commander` | K-rules-commander-death-ends-game | active |
| H-ECO-OPENING | 2 extractors, 2 solars, 1 lab, in that order | `economy.rs` `plan_for` | (convention; unexamined) | active |
| H-ECO-ENERGY-BY-STORAGE | Build generators when stored energy < 40% of storage; no cap on builders | `economy.rs` `plan_for` | K-eco-judge-energy-by-storage | active |
| H-ECO-ADV-SOLAR | Advanced solar once energy income > 250 | `economy.rs` `ADVANCED_SOLAR_INCOME` | (unexamined) | active |
| H-ECO-EXPAND | Constructors take the nearest free metal spot | `economy.rs` `claim_spot` | K-eco-expansion-before-conversion | active |
| H-ECO-OWN-HALF | Constructors only take spots nearer to home than to the enemy start | `economy.rs` `claim_spot` | K-eco-outposts-get-raided | active |
| H-ECO-OUTPOST-TURRET | An extractor farther than 1200 from home gets a turret within 350 | `economy.rs` `unguarded_outpost` | K-eco-outposts-get-raided | active |
| H-ECO-CONVERT-SURPLUS | Converters when stored energy > 80% and nothing to expand to (max 40) | `economy.rs` `plan_for` | K-eco-judge-energy-by-storage | active |
| H-ECO-MORE-LABS | Another lab (max 4) when stored metal > 500 and energy is not short | `economy.rs` `plan_for` | (unexamined) | active |
| H-ECO-BASE-TURRETS | 2 turrets 450 forward of home after the lab; up to 6 when idle | `economy.rs` `plan_for` | (unexamined) | active |
| H-ECO-FALLBACK-ENERGY | Nothing else to do: build a generator | `economy.rs` `plan_for` (last line) | (unexamined) | active |
| H-ECO-JOBS | Builders count each other's in-progress jobs before choosing | `economy.rs` `plan_for`, `Brain::jobs` | (mechanical) | active |
| H-PROD-BATCH | Factory batch: raider, raider, constructor-or-artillery, skirmisher, skirmisher; constructors wanted = 2 + extractors/4, max 6 | `economy.rs` `production_batch` | K-army-fighters-before-constructors | active |
| H-ARMY-DEFEND | Enemy within 1400 of home: home group fights it, no wave leaves | `army.rs` `run_army` | (unexamined) | active |
| H-ARMY-WAVES | Home group of 8 (+4 per wave, max 40) is committed as attackers | `army.rs` `run_army` | K-army-crowds-are-never-idle | active |
| H-ARMY-TARGET | Attack remembered enemy buildings, the one nearest to us first; else the presumed enemy start. Buildings our soldiers stand next to and cannot see are forgotten | `army.rs` `run_army`, `forget_razed_buildings` | K-army-waves-chased-raiders | active (untested) |
| H-ECO-WIND | Average map wind >= 8: wind generators instead of solars (4 in the opening instead of 2) | `economy.rs` `plan_for`, `WINDY_AVERAGE` | K-maps/K-mechanics wind entries (reported) | active (untested) |
| H-ARMY-STATION | Home group waits 250 ahead of our most exposed outpost extractor | `army.rs` `station` | K-army-home-defence-does-not-protect-outposts | active; effect unknown (ablations were noise) |
| H-ARMY-DEFEND-OUTPOST | Home group fights raiders within 500 of any of our extractors | `army.rs` `run_army` | same | active; effect unknown |
| H-PROD-CONSTRUCTOR-FLOOR | At least 3 constructors wanted | `economy.rs` `production_batch` | K-eco-outposts-get-raided | active; effect unknown |
| H-ECO-BASE-LAYOUT | Labs in a yard 350 ahead of the start (8-square gaps), generators and converters 150 behind it (5-square gaps), base turrets 650 ahead; extractors exactly on spots; nothing else within 100 of a spot | `economy.rs` `plan_for`, `gap_around`; shim `find_build_site` | K-army-base-maze, K-rules-site-search-ignores-metal-spots | active; soldier move failures in base 17,000 -> 149 per 8 games |
| H-ARMY-STAGE | A launched wave, with the survivors of earlier ones, gathers 1500 short of the target; the assault starts at 70 % gathered or after 150 s | `army.rs` `run_army` | K-army-waves-die-to-static-defence, K-army-piecemeal-midmap | active; v12-stage 17-5-2 against 13-7-4 before (one batch each) |
| H-ARMY-RECALL | 6 or more enemies within 1400 of the start: every attacker rejoins the home group and defends | `army.rs` `run_army` | K-army-commander-sniped-after-wave-leaves | active; fired in 4 of 24 games of v13-recall (23-1-0); effect not separable |
| H-ARMY-RAID | (removed) Squads of 3 raiders hunting enemy extractors | was `raid.rs` | K-army-we-never-raid | retired 2026-09-19: no gain in extractor kills, 6-10 raiders lost in the enemy half by minute 10, a quarter of the early army |
| H-ARMY-SWEEP | Idle attackers at an empty target sweep metal spots from the enemy side | `army.rs` `run_army` | (unexamined) | active |

Retired:
| ID | Rule | Retired | Why |
|---|---|---|---|
| H-ARMY-TARGET (v1) | Attack the visible enemy nearest the presumed enemy start | 2026-09-19 | K-army-waves-chased-raiders |
| H-MVP-COM-EXPANDS | Commander takes the nearest free metal spot anywhere | 2026-09-19 | K-rules-commander-death-ends-game |
| H-MVP-IDLE-WAVES | Launch when enough idle units stand near home | 2026-09-19 | K-army-crowds-are-never-idle |
| H-ECO-ENERGY-BY-INCOME | Energy short when income < usage + 20 | 2026-09-19 | K-eco-judge-energy-by-storage |
| H-ECO-GENERATOR-CAP | At most 2 generators under construction | 2026-09-19 | was a patch for the wrong shortage signal; starved 4 labs (v5-probe/01) |
