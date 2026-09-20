# Inbox: SC2 micro engines — what a per-unit control lane looks like elsewhere

Researched 2026-09-20 from public sources; not reviewed, no arena evidence. Nothing here is a claim about BAR yet.
**S:** stated by the cited source. **I:** my inference. Sources are linked inline.

## 1. The ecosystem

**AI Arena** (<https://aiarena.net>) runs the live SC2 bot ladder. Supported stacks, from the wiki
(<https://aiarena.net/wiki/bot-development/getting-started/>): Python — `python-sc2` ("most popular/supported interface
for scripted and machine learning bots alike") and `sharpy-sc2`; C++ — `cpp-sc2`, CommandCenter; Java — `ocraft-s2client`;
.NET — Tyr, Schmidt, Sharky; Go — `go-sc2ai`; Rust — `rust-sc2`; NodeJS — `node-sc2/core`. Bots upload as a ≤50 MB zip. **S**

Layers, from the curated list (<https://github.com/aiarena/awesome-sc2-ai>): **S**
- `s2client-proto` / `s2client-api` — Blizzard's protobuf API; the game is a separate process, the bot a client.
- `python-sc2` (<https://github.com/BurnySc2/python-sc2>) — the mainstream scripted-bot client.
- `ares-sc2` (<https://github.com/AresSC2/ares-sc2>) — extension over python-sc2: behaviour system, influence grids, squads.
- `sharpy-sc2` (<https://github.com/DrInfy/sharpy-sc2>) — older rapid-prototyping framework; "solid macro and micro out of the box".
- `cython-extensions-sc2` (<https://github.com/AresSC2/cython-extensions-sc2>) — hot-path helpers pulled out of ares.
- `SC2MapAnalysis` (<https://github.com/eladyaniv01/SC2MapAnalysis>) — influence maps + A* over numpy grids.
- `PySC2` — DeepMind's ML interface. Not used by the scripted-micro scene. **I**

Notable bots: **MicroMachine** (C++ on CommandCenter, <https://github.com/RaphaelRoyerRivard/MicroMachine>) — described as
"the strongest bot amongst community made bots in the SC2AI community"; **CommandCenter** (Dave Churchill,
<https://github.com/davechurchill/commandcenter>), the C++ base most C++ bots fork; **BlinkerBot**
(<https://github.com/AMontgomerie/BlinkerBot>) — Protoss, "builds a proxy pylon, warps in stalkers and tries to attack
aggressively using blink and kiting micro"; **Eris** (Zerg, reactive, top-of-ladder); **Sharpened Edge** (sharpy's own bot). **S**

AI Arena also runs a **Micro Ladder**: "non-seasonal ladder played on a custom made micro map", one division, ELO as on the
melee ladder (<https://aiarena.net/wiki/ladders/>) — i.e. the scene separates micro measurement from full games. **S**

Two operational constraints worth noting. Rules forbid deliberate slowdown and any network access beyond the local sc2api
process (<https://aiarena.net/wiki/rules/>). And there is an order-rate ceiling: the wiki's "APM Bug" note says a per-player
action queue "eventually grows to an unsustainable size and takes a long time to resolve" and advises keeping APM
"below the 120k mark" (<https://aiarena.net/wiki/bot-development/>). **S**

## 2. How the micro engines are built

**Loop rate.** SC2 at "Faster" runs 22.4 game loops per second; game time = `game_loop / 22.4`
(<https://github.com/Dentosal/python-sc2/issues/302>). python-sc2 steps the game in chunks: `game_step` defaults to **8**
loops, i.e. `on_step` about **2.8×/s**; bots that want micro set `self.client.game_step = 4` (5.6 Hz) or `2` (11.2 Hz) in
`on_start` (<https://github.com/Dentosal/python-sc2/wiki/The-BotAI-class>). Ladder games run `realtime=False`, where "the
game runs turn-based" with no wall-clock constraint on `on_step`; only `realtime=True` gives the bot limited time
(<https://burnysc2.github.io/python-sc2/text_files/introduction.html>). So SC2 micro bots buy decision rate with wall-clock
game time, not with a latency budget. **S** The practical band the scene uses is 2.8–11.2 Hz. **I**

**Per-unit decisions: an ordered behaviour stack.** ares composes a `CombatManeuver` from behaviours, each returning
`True` if it acted; "maneuvers should be set up so that higher priority tasks are added first… if this returns False for a
tank, then the AMove behavior will try to execute an action instead" (<https://aressc2.github.io/ares-sc2/tutorials/custom_behaviors.html>).
`AMove` always returns `True`, so it is the terminal fallback. The catalogue
(<https://aressc2.github.io/ares-sc2/api_reference/behaviors/combat_behaviors.html>) includes `ShootTargetInRange`,
`AttackTarget`, `KeepUnitSafe`, `StutterUnitBack/Forward`, `PathUnitToTarget`, `MoveToSafeTarget`, `ShootAndMoveToTarget`,
`WorkerKiteBack`, `UseAbility`, plus ability-specific ones. **S**

Kiting is two lines. `StutterUnitBack` (<https://github.com/AresSC2/ares-sc2/blob/main/src/ares/behaviors/combat/individual/stutter_unit_back.py>): **S**

```python
if cy_attack_ready(ai, unit, target):
    return AttackTarget(unit=unit, target=target).execute(...)
elif self.kite_via_pathing and self.grid is not None:
    return KeepUnitSafe(unit=unit, grid=self.grid).execute(...)
```

`KeepUnitSafe` returns `False` if the unit's cell is already safe on the influence grid, else finds the closest safe spot and
paths there. "Retreat" is therefore *gradient descent on a threat grid*, not a fixed rally point. **S** Safety is a scalar
test: `cy_point_below_value(grid, position, weight_safety_limit)` with default limit **1.0**
(<https://github.com/AresSC2/ares-sc2/blob/main/src/ares/managers/path_manager.py>). `PathUnitToTarget` defaults:
`sensitivity=5`, `sense_danger=True`, `danger_distance=20.0`, `danger_threshold=5.0`, `smoothing=False`. **S**

**State computed per unit**, from python-sc2's `Unit` (<https://github.com/BurnySc2/python-sc2/blob/develop/sc2/unit.py>): **S**
`weapon_cooldown` ("time until the unit can fire again", `-1` for units that can't attack; `weapon_ready` is `== 0`),
`ground_range`, `ground_dps` (`damage * attacks / speed`), `movement_speed` (normal speed; ×1.4 for "Faster"),
`shield_percentage`, `is_facing(other, angle_error=0.05)`, `target_in_range(target, bonus_distance)`,
`calculate_dps_vs_target(target, ignore_armor, include_overkill_damage)`. The units of `weapon_cooldown` are undocumented in
the proto and the docstring; the community treats it as game loops. **I**

A worked per-unit rule set, python-sc2's own `mass_reaper.py` example
(<https://github.com/BurnySc2/python-sc2/blob/develop/examples/terran/mass_reaper.py>): retreat if
`health_percentage < 2/5` with threats within **15**; attack ground enemies within **5**; kite when
`weapon_cooldown != 0` and threats within **4.5**; retreat candidates are the 8 neighbours at distance **2** and **4**,
filtered by `in_pathing_grid`. **S**

**Grouping.** ares' `SquadManager` groups by proximity with `squad_radius` default **6.0**; squads merge when they overlap,
stray units drop out, each squad keeps a `main_squad` flag and a `squad_position` = `cy_center(squad_units)`; squads are
computed on demand, not every frame (<https://github.com/AresSC2/ares-sc2/blob/main/src/ares/managers/squad_manager.py>). **S**
sharpy splits the decision explicitly: `MicroStep.group_solve_combat(units, current_command)` and
`MicroStep.unit_solve_combat(unit, current_command)`, with `MicroRules` mapping `UnitTypeId → MicroStep` plus a generic
fallback and a replaceable `focus_fire_func` (<https://github.com/DrInfy/sharpy-sc2/wiki/Extending-Micro>). **S** So: group
decides the intent, the unit decides the action — the same split we already have between brain and party. **I**

**Expensive work off the loop.** ares rebuilds the clean pathing grid only every 8th iteration
(`if iteration % 8 == 0: self._cached_clean_ground_grid = self.map_data.get_pyastar_grid()`), then each step resets the
working grids from that cached copy and re-stamps enemy influence; danger-tile caches clear every 4 iterations; nydus path
caches live 44 game loops (~2 s) (<https://github.com/AresSC2/ares-sc2/blob/main/src/ares/managers/grid_manager.py>,
`path_manager.py`). `mass_reaper` redistributes workers on `iteration % 25`. **S** The hot path is moved to compiled code
rather than threads: `cython-extensions-sc2` reports 2–14× over the Python equivalents (`cy_closest_to` 6.85–13×,
`cy_towards` 14.29×, `cy_is_facing` 9.1×, `cy_in_attack_range` 2.05×) (<https://github.com/AresSC2/cython-extensions-sc2>). **S**
I found no published per-tick CPU budget in ms for any ladder bot. **I**

## 3. Blink stalker micro

Mechanics (multiplayer): Stalker weapon range 6, Blink range 8, researched at the Twilight Council. Sources disagree on the
Blink cooldown — one gives 7 s, another ~10 s; Liquipedia and the Fandom wiki both refused fetches (403/402), so
**I could not verify the cooldown**. Relevant either way: a Stalker's shields (80) regenerate after disengaging, so a unit
pulled out of a fight comes back at full effective HP. **S/unverified**

Two implementations, both concrete:

*h3nnn4n's stalker micro* (<https://h3nnn4n.github.io/post/sc2-stalker-micro/>) — "the unit tries to stay far away as
possible from the enemy units while staying in shooting range"; if faster than the enemy, hold at max range; if outranged,
"walk back without stop while allied units shoot"; "when blink is available, the unit will be able to blink away from the
fight after losing its shields, which allows for the damage to be distributed among all units"; retreat direction is
"directly behind it, when facing the closest enemy unit", which emergently produces an arc around the enemy. Reported
result: win rate vs Elite AI went from "less than 10%" to "about 60% over 250 games" after adding micro. Best matchup is
vs Roach (range 4 vs 6); worst vs Terran bio + tanks. **S**

*BlinkerBot's `ArmyManager`* (<https://github.com/AMontgomerie/BlinkerBot/blob/master/ArmyManager.cpp>): **S**
- Defensive blink: for each enemy, `inRange(enemy, unit) && shieldsCritical(unit, enemy)` where the threshold is
  `nextAttackDamage >= unit->shield` — i.e. blink out the tick *before* the shot that would break through the shield.
- Offensive blink: blink onto a target only when
  `calculateSupplyInRadius(blinkTarget->pos, enemyArmy) * 1.5 < calculateSupplyInRadius(blinkTarget->pos, army)` — a local
  1.5× supply advantage at the destination, not a global army comparison.
- Kiting: `if (canKite && armyUnit.unit->weapon_cooldown > 0)` retreat by `RETREATDIST = 5`; kite conditions are outranging
  the enemy, being in weapon range, being able to target it, and weapon on cooldown.

Known failure mode: finding a *valid* blink destination is awkward; `find_placement` often fails
(<https://github.com/Dentosal/python-sc2/issues/64>). **S**

What makes blink work, reduced: the unit has a cheap, cooldown-gated escape that resets the damage it has taken (shields
regenerate), so the correct policy is to spend HP down to a computed threshold and then leave — the *threshold* is where
the value is, and the threshold is predictive ("next shot breaks my shield"), not reactive ("I am at 40%"). **I**
The a-move comparison: the only concrete number I found is h3nnn4n's <10% → ~60%. **S**

## 4. Influence / threat maps

`SC2MapAnalysis` is the common library (<https://eladyaniv01.github.io/SC2MapAnalysis/source/MapAnalyzer/>): **S**
- `get_pyastar_grid(default_weight=1, include_destructables=True)` — numpy grid, non-pathable cells `numpy.inf`. The docs
  say it "can, and **should** be reused in the duration of the frame, and should be regenerated (**once**) on each frame".
- `add_cost(position, radius, grid, weight=100, safe=True, initial_default_weights=0)` — stamps a *circle* of cost.
- `pathfind(start, goal, grid, large=False, smoothing=False, sensitivity=1)` — `sensitivity` slices the result `path[::sensitivity]`,
  so a bot follows every Nth waypoint; `large` accounts for big unit footprints.
- `find_lowest_cost_points(from_pos, radius, grid)` — the retreat primitive.
- `get_climber_grid`, `get_air_vs_ground_grid(default_weight=100)` — separate grids per movement class.
A* is a C extension. Grid resolution is one cell per map pathing tile; the docs I reached never state it. **I**

How ares populates it (`grid_manager.py`): grids reset each step from a cached clean copy; per enemy unit,
`_add_unit_influence` stamps `weight = int(weight) * config[PATHING][COST_MULTIPLIER]` over radius
`unit.ground_range + config[PATHING][RANGE_BUFFER]`; melee units use `unit.ground_dps` as the cost; special cases carry
hand-set weights (Disruptor `weight=1000`, radius `8 + EFFECTS_RANGE_BUFFER`; storms and other effects have their own
buffer). Only `self.ai.enemy_units` is iterated — **no memory/decay layer for enemies in fog**. **S**

So the technique is: one grid per movement class, regenerated once per tick, cost = threat magnitude (dps), radius = weapon
range + a buffer, and then two uses — pathing (A* over the cost grid routes around danger) and decisions (a scalar threshold
on the cell under a unit answers "am I safe here?"). **S/I**

## 5. What transfers to BAR/Recoil, what does not

**Transfers.**
- The 5–11 Hz band. SC2 micro bots run 4 or 2 game loops per decision = 5.6/11.2 Hz against a 22.4 Hz sim; BAR at 30 fps
  gives 6 Hz at every 5th frame, 10 Hz at every 3rd. The target in the brief sits exactly where the mature scene sits. **I**
- The ordered behaviour stack with a terminal fallback. It is a flat list of predicates, trivially testable, and it makes
  "which rule fired" loggable per unit — which the docs loop wants anyway. **I**
- Kiting as `weapon_ready ? shoot : move_to_lowest_threat_cell`. Requires only reload-remaining per unit. **S→I**
- The threat grid as the single shared per-tick artefact: one rebuild, every unit reads it. Turrets are *exactly* ares'
  static-influence case (radius = weapon range + buffer, weight = dps), which the repo already prices in the brain.
- Squad by proximity radius (ares: 6.0) with group intent / per-unit action split (sharpy). Our party is already this. **I**
- Predictive retreat thresholds (BlinkerBot: "next attack would break my shield") rather than percentage triggers. **I**
- Order-rate discipline. AI Arena's 120k-APM queue warning is a different mechanism from Recoil's command handling, but the
  lesson generalises: at 10 Hz × N units, issuing an order every tick is the default bug. Re-issue only on decision change. **I**

**Does not transfer.**
- Blink itself. BAR raiders have no reset; damage is permanent and there is no shield regen to wait out, so the retreat
  threshold cannot be "spend shields, then leave" — it must be "this unit's remaining future value exceeds what staying buys". **I**
- The step/observation model. SC2 bots choose a decision rate because each decision costs a protobuf round trip to a
  separate process; our shim is in-engine with orders acting next frame, so there is no observation latency to amortise and
  no step negotiation. The real constraint is our own shim↔bot link and the fight simulator's cost. **I**
- `realtime=False`. Ladder bots can think without a wall clock; we cannot — the arena runs at `--speed 50`. **I**
- SC2's collision/footprint model and automatic target acquisition differ enough that emergent arc formation
  (h3nnn4n's "retreat directly away from the closest enemy") should be treated as untested here. **I**

## 6. Recommendations for Within Reason

**Control loop shape.** Two lanes, not one faster lane. Keep the 2 Hz brain owning *intent* (which party, which target,
commit/abort). Add a micro lane at every 3rd frame (10 Hz) or 5th (6 Hz) that owns only units tagged into a raider squad
and never re-chooses the target — it executes the standing intent and may only *abort* it upward to the brain. Snapshot
once per micro tick; all units read the same snapshot and the same threat grid.

**State each micro tick** (cheap, per tick): per-unit reload-remaining in frames; a threat grid rebuilt once (cost = dps,
radius = weapon range + buffer, one layer for mobile enemies from the current snapshot, one decaying layer for turrets and
last-seen enemies — ares has no fog layer and that is a gap we should not copy); per-squad centre of mass; and, per
(our unit type, their unit type) pair, two booleans precomputed at startup: `outranges` and `outruns`. Rebuild the static
terrain/pathing part on a modulo (ares: every 8th iteration), never per tick.

**First three behaviours for raider squads**, in stack order:
1. **Reload-gated stutter.** `if reload_ready && target in range: attack; else: step to the lowest-threat cell within a
   short radius`. This is `StutterUnitBack` and it is the whole of kiting.
2. **Break off on resistance, per unit.** If the threat value under my cell exceeds a threshold, or a newly-seen turret
   bears on me, path out along the grid gradient rather than straight back along the approach. Gate the *engagement* on
   `outranges || outruns` — that differential is our blink: the only reason a raider survives probing is that it can leave.
3. **Squad focus fire** on one target chosen by shortest kill-time × value, with overkill capped (python-sc2 exposes
   `include_overkill_damage` for exactly this reason); replaceable as one function, as sharpy's `focus_fire_func` is.

**Measurement.** Win rate is too noisy to see micro (the docs README already puts 12 matches at ±14 points). Measure the
lane directly, per raid, logged from the match record: raider value lost per enemy value destroyed; unit-frames spent
inside an enemy weapon radius (time-under-fire); and raids that ended by our choice vs raids that ended in deaths. A/B the
same brain with the micro lane on and off over 24+ matches. Register each of the three behaviours as its own heuristic with
a `conjectured` claim so a batch can kill one without killing the lane.

**What I could not find:** AI Arena's per-step or per-match time limits (the rules page is ethics only, the wiki has no
technical page, the local-play-bootstrap README does not show the config); any published per-tick CPU budget in ms for a
ladder bot; the authoritative Blink cooldown (Liquipedia 403, Fandom 402; snippets disagree between 7 s and ~10 s);
SC2MapAnalysis' stated grid resolution; the source of `cy_attack_ready` (the `.pyx` path 404s on raw.githubusercontent);
and any micro-ladder result table showing the gain of a micro engine over a-move beyond h3nnn4n's <10% → ~60%.
