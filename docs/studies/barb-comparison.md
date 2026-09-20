# BARb (CircuitAI) against our bot, subsystem by subsystem (2026-09-20)

> **Reviewer's note (main session, 2026-09-20).** Spot-checked against the source: the threat term in path cost
> (`terrain/path/PathFinder.cpp:481`, `2.f * threatArray[index]`), the contact arc with range rows and AoE spacing
> (`task/fighter/SquadTask.cpp:390-445`) and the explosion-radius spacing all hold. One claim below is **wrong**: that
> resurrection is dead under the BAR config because "no stock BAR con has it". BAR has resurrection bots (`armrectr`,
> `cornecro`), BARb builds them, and our ground-truth logs show a Cortex BARb fielding `armpw` (our dead units) in
> commander-3 (K-army-dead-waves-are-resurrected). The rest of the dead-code list is unverified.


Source-reading study; nothing here was tested in the arena. Companion to `../knowledge/opponents-barb.md`, which covers the
**config** level (roles, quotas, response table, tier-2 rules, base-defence clock, retreat draw). This one goes into the **C++**:
where BARb's authors spent engineering attention on something we do not have.

**Read.** `upstream/CircuitAI/src/circuit/` (cited `src/…`; branch `barbarian`, source 1.6.30, our binary logs 1.6.28 — the same
version caveat as `opponents-barb.md` applies to every C++ citation) and, for what `medium` actually enables,
`…/configs/BARb/stable/{config,script}/` (cited `config/…`, `script/…`). Ours: `crates/bot/src/brain/`, `terrain.rs`,
`crates/ai-shim/src/engine.rs`, `docs/heuristics.md`. Sizes: CircuitAI ~54k lines (military ~10.8k, terrain+maps ~12.4k,
economy modules ~6.8k); our brain ~3.1k.

## Summary

BARb's engineering is concentrated not in tactics but in **spatial infrastructure**, four layers we have no analogue of: a threat
map kept *per unit role* and rebuilt three times a second, a signed influence map (ally − enemy control) on top of it, its own
threaded A\* whose **edge cost includes threat**, and a Delaunay graph over metal clusters used as the expansion planner. Three
consequences map straight onto our recorded losses: squads travel on **one shared path at the slowest member's speed**, regroup
when stretched, and on contact **spread onto an arc at 0.8× weapon range** around one focused target; expansion is a **Dijkstra
over the cluster graph with unsafe clusters deleted from the graph**, not "nearest free spot"; and the base is laid out in a
direction-to-enemy frame that **reserves a corridor from each factory exit toward the front** as un-buildable. Where the user
already suspected weakness, it is confirmed: static defence sits on the metal spots on an income budget (the chokepoint machinery
is computed and never queried), there is no kiting, no cross-squad focus fire, and nothing anywhere asks "am I winning".

## Subsystem table

Rule IDs in "ours" are from `docs/heuristics.md`.

| Subsystem | BARb | Ours | Gap | Worth learning from? |
|---|---|---|---|---|
| Threat map | Per combat role × {air, surf, amph, swim} float grids at 64 elmos, rebuilt from scratch every 10 frames on workers; value `dps^0.5·dmg^0.25·sqrt(hp+1.5·shield)` over a disc of `range + slack`, falloff `1−0.5d/r`; positions **lead-predicted** by velocity; radar blips stamp a flat 0.1; **no decay** (`src/map/ThreatMap.cpp:212,294-349,603-650`) | none; `known_enemy_force(place, radius)` sums remembered armed buildings + visible soldiers on demand (`brain/army.rs:219`) | absent | Yes, the biggest single lever: it prices pathing, targets, build sites and retreat from one structure |
| Influence map | 256-elmo signed grid `ally − enemy`; armed units add power, *un*armed buildings add a flat 2 to a separate `allyDefendInfl`. Gates retreat endpoints, squad merges, build safety, "front" choice, base relocation (`src/map/InfluenceMap.cpp:37-153`) | none | absent | Yes — "whose ground is this" is the question our NW games fail |
| Pathfinding | Own threaded A\* on the 64-elmo grid, separate from the engine's; node cost `terrain + move + 2×threat`; query kinds SINGLE / MULTI (nearest-of-many) / WIDE / COST (Dijkstra cost map) / LINE (Bresenham safety test); one pather per worker (`src/terrain/path/PathFinder.cpp:392-518,696-796`) | engine pathing; our own Dijkstra fields from home and enemy, no threat (`terrain.rs`, `brain/routes.rs`) | absent | Partly — threat-priced routing is the idea; a full A\* is unnecessary if we keep issuing engine orders |
| Move execution | Waypoint by waypoint: re-issues `CmdMoveTo`/`CmdFightTo` at 1 Hz / 0.5 Hz, queues at most one extra waypoint, sets `CmdWantedSpeed`, early-outs inside the same sector (`src/unit/action/TravelAction.cpp:73-108`) | one Move/Fight order to the destination, re-issued when idle | crude | Yes for speed control: `bot-protocol` has no speed command at all |
| Squad cohesion | Squad shares one `CPathInfo` and travels at `lowestSpeed`; leader = member with the most restrictive movement area. Every 16th update a spread test against `max(32 elmos × n, highestRange)`; if spread, halt and gather — **suppressed while engaged or under threat**. Stragglers still out after 60 s are stopped and written off "stuck". Squads merge into stronger ones when a Bresenham line over the threat grid is safe (`src/task/fighter/SquadTask.cpp:131-331`) | H-ARMY-STAGE gathers 1500 short of the target at 70 % quorum or 150 s; no leader, no speed matching, no regroup after that | crude | Yes — shared path plus slowest-member speed is our "arrives strung out", cheaply |
| Combat formation | On contact, members are bucketed by weapon range and placed on an **arc at 0.8× that range** around the target, spacing `3(radius+aoe)/range` capped at `0.9π/n`, near side chosen to minimise travel; the first unit of the first row is pulled to LOS range to spot. Re-issued every 3 s (`src/task/fighter/SquadTask.cpp:385-472`) | none | absent | Yes — K-barb-we-lose-the-fights-not-the-build says the exchange ratio is where we lose |
| Target choice in a fight | One target per task, all members ordered onto it (`unit->Attack` + engine `CmdSetTarget`); melee/jump per unit; `slow_target` toggles on/off weapons. Across squads an anti-piling cap (`enemy->GetTasks().size() > 1`) deliberately **spreads** fire (`src/unit/CircuitUnit.cpp:475-535`) | Fight orders at a position; the engine picks targets | crude | Per-squad focus yes; their cross-squad spreading is a bug to avoid |
| Retreat / repair | Per unit: below `retreat.fighter` (medium: one draw in [0.55, 1.0]), or `threat×2 > own power`, or target out of range. Destination = assigned repairer, else nearest factory "haven", else base — and if base influence `< INFL_SAFE` it **relocates the base**; enqueues a HIGH-priority repair on itself; released at 98 % (`src/task/RetreatTask.cpp:120-189`, `FighterTask.cpp:107-159`) | H-ARMY-RETREAT sends *every attacker* home when odds < 0.6; H-ECO-REPAIR mends buildings and the commander | crude | Yes — per-unit retreat at a fixed ~0.45 with nano-turret havens is low effort |
| Enemy memory | Full last-known state per enemy (pos, vel, health, shield, cost, per-layer range, role mask). Memory ends when the AI **has LOS on the remembered position and the unit is absent**, else 20 min after leaving radar. Damage from an unseen source creates a **fake enemy** at the estimated origin. First sight of a new threat forces every friendly within 1000 elmos to re-plan (`src/map/MapManager.cpp:74-205`, `src/CircuitAI.cpp:1167,1256-1280`) | `enemy_buildings` + `forget_razed_buildings` (a building we stand beside and cannot see is gone); `enemy_army_seen` = biggest force in one look in 2 min (`brain/briefing.rs:113`, `army.rs:249,285`) | crude | Yes — generalise "seen empty ⇒ forget" from buildings to units; fake enemies from damage events |
| Enemy clusters | k-means, `k = min(32, 1+√N)`, one iteration per update on a worker, seeded from the last result; per group cost, role costs, influence and `vagueMetric = (influence+1)/(cost+ε)`. Attack picks the group minimising `vagueMetric × dist²` — **prefers groups that are cheap for their threat** (`src/unit/enemy/EnemyManager.cpp:466-584`, `AttackTask.cpp:238-333`) | coarse grid cells in the briefing, for the LLM only | absent | Yes — the soft-target metric is a one-liner, and its gate already reads "second-largest group" |
| Military tasks | RALLY, GUARD, DEFEND, SCOUT, RAID, ATTACK, BOMB, ARTY, AA, AH, SUPPORT, SUPER + RETREAT. Most units enter via DEFEND and are promoted in bulk. Each task is a fallback ladder of async path queries (AntiHeavy 9, AntiAir 8, Attack 6), not a state machine (`src/task/fighter/*`, `MilitaryManager.cpp:1636-1718`) | home group / attackers / commander squads; H-ARMY-DEFEND, -WAVES, -RESPONDERS, -SCOUT, -SWEEP | crude | The taxonomy, not the code: artillery, anti-heavy and support are roles we have no concept of |
| Scouting | Destinations are metal clusters, scored by a value **diffused over the cluster graph** from sightings (+3, −1 per hop) and kills (−3); tie-break least-scouted; initial order farthest-first (`src/module/MilitaryManager.cpp:987-1054,1774-1798`) | H-ARMY-SCOUT: one raider every 90 s to the attack target and on to the enemy start | crude | Yes — cheap, and it makes scouting follow the enemy's expansion front |
| Commander | Role `builder`, leashed to 1000 elmos after 180 s, hides on influence; **d-gun** is a blocking per-unit action that picks max enemy power (max cost with `DG_COST`), traces terrain for low-trajectory shots, and never d-guns an enemy commander (`src/unit/action/DGunAction.cpp`) | H-COM-LEASH 900, H-COM-RETREAT below 70 %, D-COMMANDER-STATION | crude | The d-gun action; ours never fires it |
| Static defence placement | Points precomputed once: every BWEM chokepoint centre **plus**, per metal cluster, a second hierarchical clustering of its spots at 600 elmos, each sub-cluster's *minimum enclosing circle centre* becoming a point with accumulating `cost`. `maxCost = amountFactor × min(metal, energy income)`; the defender ladder is walked while `totalCost ≤ point.cost`, so accumulated cost *is* the tier. Emplacement is a formation: short range in front, long range middle, AA/radar behind, oriented on `enemyPos` (`src/setup/DefenceData.cpp:61-110`, `MilitaryManager.cpp:705-925`) | H-ECO-OUTPOST-TURRET (every extractor > 500 from home), H-ECO-BASE-TURRETS, H-ECO-HOT-SPOTS | crude | The income-budgeted ladder per cluster and the front/middle/back orientation, yes. The point geometry, no — see §4 |
| Metal clustering & expansion | Spots from BAR's Lua; **complete-link hierarchical** clustering at 950 elmos (k-means is dead code); cluster centre = min enclosing circle; pruned **Delaunay** graph between clusters, edge cost `dist/(incomeA+incomeB)×(spotsA+spotsB)`. Next spot = **lemon Dijkstra over that graph with unsafe clusters filtered out of the graph**, goal = first cluster with an open spot (`src/resource/MetalData.cpp:120-128,605-649`, `MetalManager.cpp:374-424`, `MetalManager.h:102`) | H-ECO-EXPAND: nearest free spot on foot; H-ECO-OWN-HALF, H-ECO-REACH, hot spots | absent | **Yes, high value.** Contiguous expansion that cannot plan through a dangerous cluster is the NW failure |
| Economy balancing | Empty/full/stalling predicates live in **AngelScript** (`script/medium/manager/economy.as:22-37`). Energy def by `make²/(cost·footprint)` gated on per-def income conditions with a factor ramping 1→15 over 5–60 min; factory/nano only when income exceeds what existing factories consume; builders while `buildPower < 0.6 × min(metal, energy income)` (`src/module/EconomyManager.cpp:1283-1416,1511-1542`, `FactoryManager.cpp:1038-1072`) | H-ECO-ENERGY-BY-STORAGE, -ADV-SOLAR, -CONVERT-SURPLUS, -MORE-LABS, H-ECO-SPEND, H-ECO-NANO, `wanted_constructors` | crude | Already ranked in `opponents-barb.md`, "Worth borrowing" 4 |
| Builder task choice | Shared pool; winner minimises `distCost / (priority+1)²` where `distCost` is an async **Dijkstra cost map through the threat map**, not distance; repairs get a "can it finish before the target dies" test; failures register a terrain blocker at the site (`src/module/BuilderManager.cpp:1275-1400`) | `Brain::jobs` counts in-progress jobs; choice is nearest | crude | Yes — travel cost not straight line; we already compute the field |
| Factory & unit choice | Factory type sorted **primarily by how few of that type exist** (round-robin), score `rand + importance × (map reach % + speed %)` only as tie-break; unit choice is a roulette over a probability row by income tier, with the response weight 30 swamping it (`src/unit/FactoryData.cpp:86-146`) | `roster.rs` fixed names per faction; H-PROD-MIX, H-PROD-BATCH | absent (no response) | Low value until we field more than a few types |
| Build placement | Three blocking grids (16 / 128 elmos, plus an ally-zone grid); per class a rectangle (`size + yard`) or a circle whose default radius is the building's **death-explosion AoE/2**, with ignore masks per structure type; search is a distance-sorted spiral behind a coarse pre-filter. Base zones from a frame: `metalBase`/`energyBase` 400 back ±200 across, `energyBase2` 800 back, cheap energy offset *away from map centre*. **A wide path from each factory exit to the front is stamped as blockers** and stitched into a shared road network (`config/block_map.json`, `src/terrain/TerrainManager.cpp:95-290,473-546,1480-1546`, `SetupManager.cpp:225-252,531-599`) | H-ECO-BASE-LAYOUT (lab yard 350 ahead, 8-square gaps; generators 150 behind) + the shim's ring search (`ai-shim/src/engine.rs:219`) | crude | **Yes** — explosion-radius spacing and the reserved factory lane are our "base is a maze" arc |
| Reclaim / repair | Feature reclaim when not metal-full, radius `speed × (60 s stalling / 20 s not)`, nearest feature with metal ≥ 1 (value not maximised); a 60 s watchdog repairs abandoned buildings if affordable, else reclaims them (`src/module/EconomyManager.cpp:1194-1281`) | H-ECO-RECLAIM (area-reclaim where we lost units), H-ECO-REPAIR | crude | The finish-or-reclaim watchdog is a small win |
| Terrain analysis | Per movement class its own sector array and flood-filled areas; two points are reachable iff the same `SArea*`; rebuilt at runtime only when a changed sector flips passability. Separately a **full BWEM port** (areas, altitude, chokepoints, continents) run once at init on `armcom`'s move type (`src/terrain/TerrainData.cpp:573-600`, `src/map/GridAnalyzer.*`) | `terrain.rs`: one movement class for the whole army, Dijkstra fields from home/enemy | crude | Per-class areas matter once we field vehicles or air; chokepoints — see §4 |
| Engine plumbing | Worker pool `clamp(cores−1, 2, 8)`, results marshalled back as main-thread jobs; **no frame budget** — spreading is ad hoc (`size/rate + 1` per frame). AngelScript decides factory choice and switch timing, per-unit task creation, defence composition and the economy predicates; C++ keeps maps, pathing, defence geometry, task execution. Medium's scripts are near-pure pass-throughs (`src/scheduler/Scheduler.cpp`, `script/medium/manager/*.as`) | one tick per 15 frames, one thread per AI; policy in LLM directives | different | No for the scheduler; the policy/mechanism split is a useful framing |
| Team play / map config | One AI is authority for the shared threat/influence/metal/defence state; `lanePos` gives each ally a slot on a line across the map and sets its base-defence radius; ally zones block building in an ally's territory. **No map-specific config at all** — `GetMapName()` is exposed to script and unused; adaptation is by map *area* only (`src/unit/ally/AllyTeam.cpp:233-454`, `SetupManager.cpp:531-599`) | none (1v1) | absent / none | Not now; `lanePos` is interesting as "where my front is" even in 1v1 |

## Surprises

1. **A reserved road from every factory to the front.** Each factory runs a wide path query toward its `lanePos` and stamps that
   corridor into the blocking map as `TERRA` blockers, stitched onto other factories' corridors
   (`TerrainManager.cpp:473-546,1480`). Units leaving the factory never squeeze through their own base. *We have exactly this
   problem*: 17,000 soldier move failures within 500 elmos of the NW start (`experiments.md` nw-stuck-diag), only partly fixed by
   H-ECO-BASE-LAYOUT's hand-placed yard.
2. **Buildings are spaced by their own death explosion.** The default blocker radius in `block_map.json` is `explosion` (AoE/2),
   shrinking with ally count. Solves chain detonation in a packed base. We use fixed 5/8-square gaps and have never looked for
   chain losses in our records.
3. **Expansion is a graph search that cannot see dangerous ground.** Unsafe clusters are removed by a node filter, not penalised,
   so BARb literally cannot plan an expansion path through a contested cluster. Our nearest-free-spot rule produces the opposite:
   extractors 3000 elmos out that cannot be held (`experiments.md` commander-1).
4. **Real combat formation** (surprise 4 in the table): the arc at 0.8× range, one row per range bucket, spaced by radius + enemy
   AoE. We assumed BARb a-moved. This is a candidate explanation for our exchange ratios (10-25 lost for 2-14) *independent of
   unit mix*, which is the explanation we have been testing.
5. **Scouting follows the enemy's economy, not the map** — cluster scores diffuse over the metal graph from sightings and kills.
6. **Stuck units are written off.** After 60 s of failed regrouping a member is stopped and marked `Garbage("stuck")`; if the
   *leader* has not moved the group point, the leader is written off. We blacklist stations and targets (`army.rs:169,192`) but
   never individual units.
7. **No threat decay at all, yet positions are lead-predicted** a full second ahead with speed-proportional radius slack
   (`ThreatMap.cpp:294-349`). Effort on *where an enemy will be*, none on forgetting — the opposite of what we would have written.
8. **Medium barely uses its scripting hooks.** Every `medium` manager script passes through to the C++ default except the economy
   predicates and a gate delaying cluster defence to minute 5 / income 10 / first mobile threat. Medium is close to stock CircuitAI.
9. **Builder task choice is travel cost through threat**, and the same query decides whether a repair can finish in time.
10. **~3k lines are dead or disabled under the BAR config**: the pylon/energy-grid Kruskal MST (`UpdatePylonTasks` opens with
    `return nullptr`), terraform (body commented out), resurrect (no stock BAR con has it), `CKMeansCluster` and `ConvexHull`
    (zero references), `RallyTask` and `FightType::MELEE` (unreachable), the converter-vs-extractor comparison. Their `Init` still runs.

## What BARb does badly or not at all

- **Static defence is its weakest part, and the good version is commented out.** Def points are the enclosing-circle centres of
  groups of metal spots — *on top of the extractors*, never where an attack comes from. BWEM chokepoint centres are inserted into
  `defPoints` but the only accessor returns per-cluster indices, so they are never selected; the nanoflann tree over all points is
  built and its query commented out; the choke-wall code (teeth across small chokes, a turret at large ones) is commented out
  entirely (`MilitaryManager.cpp:718-766`); and `GetDefPoint` has an inverted comparison (`tmp = dist`, `DefenceData.cpp:139-141`).
  What remains is a clock plus an income budget. The user's stance holds: do not model defence on this.
- **No kiting or retreat-while-shooting anywhere.** The arc standoff is the only ranged discipline.
- **No cross-squad focus fire**; the anti-piling caps deliberately spread damage.
- **Regroup is disabled under fire** — a squad strung out while engaged stays strung out, exactly when it matters.
- **Nearest-target logic in three task types**: anti-air and anti-heavy score only distance; the bomber picks the lowest *absolute*
  health enemy, with cost/AoE scoring left as a commented-out "FIXME: Finish" (`BombTask.cpp:303-317`).
- **Threat is per-tile from an aggregate map**, so one turret vetoes a target wholesale instead of prompting another approach.
- **No sense of the game state.** `GetArmyCost()` has one consumer: the response table's army-share cap. Nothing escalates or closes
  out — the mechanism behind K-barb-medium-observed-build (its army grows every minute and is never spent).
- **Enemy cost accounting never decays** and is removed only on confirmed kills, so its estimate of our army inflates over a game.
- **Target selection is duplicated eight times**, acknowledged by upstream's own TODO (`MilitaryManager.h:177-178`).

## Open questions

1. **Does threat-priced pathing actually bend BARb's routes on Quicksilver**, or is the map too open for the 2× threat term to
   matter? Settle from our match records: plot its approach paths past a known turret line.
2. **Is the arc formation visible in play?** Our records carry enemy positions; measure the spread of a BARb group at contact
   against a uniform-blob null. If it is real it explains our exchange ratios better than the unit mix does.
3. **What would a threat map cost us?** `UnitDefInfo` (`bot-protocol/src/messages.rs:87`) has no weapon range, no dps and no
   def-level health, and the snapshot has no LOS map — both the power formula and "seen empty ⇒ forget" need new protocol fields.
   What the AI interface exposes was not checked.
4. **Squad speed matching needs a command we lack.** `Command` has no `WantedSpeed`; the engine supports it (BARb sets it on every
   travel order). Check the interface before designing around it.
5. **Does `isPorc` ever fire on Quicksilver?** It needs cluster income stddev > 0.3 × average and a cluster >1000 from base, or two
   threatened unfinished neighbours. If it never fires, BARb's outer extractors only ever get `prevent: 1` defence.
6. **`amountFactor` on Quicksilver** — interpolated from map area between `[48, 32]` for 10×10 → 20×20. It sets the real metal
   budget per cluster, which is what our raids have to beat. Not computed here.
7. Carried over: the 1.6.28 binary against 1.6.30 source applies to every C++ claim above.
