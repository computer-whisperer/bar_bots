# Heuristic registry

Every rule in `crates/bot/src/brain/` that embodies a judgment about the game. IDs appear in code comments next to the rule
(to be added) and in knowledge entries. Status: `active`, `retired (date, why)`.

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
| H-ECO-JOBS | Builders count each other's in-progress jobs before choosing | `economy.rs` `plan_for`, `Brain::jobs` | (mechanical) | active |
| H-PROD-BATCH | Factory batch: raider, raider, constructor-or-artillery, skirmisher, skirmisher; constructors wanted = 2 + extractors/4, max 6 | `economy.rs` `production_batch` | K-army-fighters-before-constructors | active |
| H-ARMY-DEFEND | Enemy within 1400 of home: home group fights it, no wave leaves | `army.rs` `run_army` | (unexamined) | active |
| H-ARMY-WAVES | Home group of 8 (+4 per wave, max 40) is committed as attackers | `army.rs` `run_army` | K-army-crowds-are-never-idle | active |
| H-ARMY-TARGET | Attack the visible enemy nearest the presumed enemy start, else the mirrored home position | `army.rs` `run_army` | (unexamined) | active |
| H-ARMY-SWEEP | Idle attackers at an empty target sweep metal spots from the enemy side | `army.rs` `run_army` | (unexamined) | active |

Retired:
| ID | Rule | Retired | Why |
|---|---|---|---|
| H-MVP-COM-EXPANDS | Commander takes the nearest free metal spot anywhere | 2026-09-19 | K-rules-commander-death-ends-game |
| H-MVP-IDLE-WAVES | Launch when enough idle units stand near home | 2026-09-19 | K-army-crowds-are-never-idle |
| H-ECO-ENERGY-BY-INCOME | Energy short when income < usage + 20 | 2026-09-19 | K-eco-judge-energy-by-storage |
| H-ECO-GENERATOR-CAP | At most 2 generators under construction | 2026-09-19 | was a patch for the wrong shortage signal; starved 4 labs (v5-probe/01) |
