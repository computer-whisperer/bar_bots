# Target design: team games, territory, reclaim and resurrect, Opus as commander

Written 2026-09-20, before the first line of code, at the user's direction: four changes pushed in together and judged
by looking (does it do the right thing in a watched game), not by A/B win counts. This file is the ground truth for the
arc; `DESIGN.md` gets the decisions once they are built. Status lines at the end of each part say what exists.

## What is true today (read from the code, 2026-09-20)
- One bot process listens on a socket and runs one independent `session` (own `Brain`, own recorder, own commander
  session if asked) per AI that connects. Several of our seats in one game therefore already run; they know nothing of
  each other.
- The shim sends only our own team's units (`getTeamUnits`) and the enemies in sight. Allied units are invisible to the
  brain. `Hello` carries `team` and `ally_team` and nothing about who else is playing.
- The brain assumes one opponent: `enemy_start` is one point (mirror guess, then the mean of every enemy factory seen),
  "our half" is nearer-to-us-than-to-`enemy_start`, the wave target is the nearest known enemy building.
- The arena writes a fixed 1v1 script (two AIs, two ally teams, north-west and south-east start boxes) and its referee
  knows two sides.
- Wrecks: the brain remembers where our units died (`wreck_sites`) and sends an idle constructor to area-reclaim there
  when metal is under 150. It never sees a feature, never resurrects, builds no resurrection bots.
- Ground: H-ECO-HOT-SPOTS (a raided spot is closed four minutes unless covered), H-ECO-FRONTIER (grow from what we
  hold), H-ARMY-STATION (the home group stands by the most exposed extractor). No notion of which ground is ours.

## 1. Team games

### Requirements
- N seats of ours, M allied seats that are not ours (a human, BARb), K enemy seats on one or more enemy ally teams.
- Correct with any of those at 1 or more; in particular 1 human + our AIs against AIs, and two of our seats together.
- A seat whose commander dies is out; the team plays on. An enemy seat that dies stops being a target.

### Protocol and shim
- `Hello` gains `teams: Vec<TeamInfo { team, ally_team, is_ours_to_command: bool (this shim instance's team), side,
  start: Option<Vec3> }>`: every team in the game with its ally team, and its start position where the engine tells
  (`Game_getTeam*`, start boxes or positions; `None` when it does not).
- `Snapshot` gains `allies: Vec<AllyUnit { id, def, pos, team, being_built }>`: every unit of our ally team that is not
  this seat's (`getFriendlyUnits` minus `getTeamUnits`). `EnemyUnit` gains `team: Option<i32>` when the engine tells.
- New event `TeamDied { team }` if the interface offers it; else the brain infers it (no unit of that team seen for
  long and its known buildings razed) and the arena's referee reads it from the engine.

### Brain: several enemies
- `enemy_start: Vec3` becomes `enemy_bases: Vec<EnemyBase { team: Option<i32>, at: Vec3, found: bool, dead: bool }>`:
  one per enemy seat, guessed from `Hello.teams` starts (or the mirror guess when none is told), moved to the cluster of
  that seat's factories once seen (clustered by distance, never the mean over seats). The "enemy start" every rule
  uses today becomes `nearest_live_enemy_base(from)`; "our half" becomes nearer to our start than to ANY live enemy base.
- Wave target: unchanged rule (nearest known enemy building, else the nearest live base).
- The commander's report names each enemy seat: base found or guessed, commander last seen, dead or alive.

### Brain: allies that are not ours
- Allied extractors hold their spots (`spot_taken`, the free-spot counts, the frontier).
- Allied soldiers and turrets count as cover (hot spots, territory) and are shown to the commander as "allied"; we
  never order them.
- Ground nearer to an ally's start than to ours is the ally's to expand into first: we take a spot there only when it
  has stood free for three minutes (a human ally expands where they like; we do not race them at their own door).

### Brain: several seats of ours (same process)
- A `TeamBoard` shared by the sessions of one game (keyed by the game's socket, `Arc<Mutex<..>>`): spot claims, each
  seat's start and station, known enemy buildings and sightings (pooled: what one seat sees all know), wave intents.
- Spots: seats of ours see each other as allies, so the ally-ground rule above already splits the ground between them
  (built that way: one mechanism, not two); the board adds the builders' claims in flight. Waves: a seat launching a wave posts its target and time; another seat with a ready wave joins
  the same target instead of choosing its own (two half-armies at two targets is how team games are thrown).
- The commander (user's ruling, 2026-09-20): both configurations are supported. One commander for all our seats is
  the common one: the LLM session attaches to the team, sees every seat's units, and its squads may take soldiers from
  any seat. One commander per seat is the other (`--commander-each`). Rejected as the only mode: one each, because two
  sessions that cannot talk split the army and cost twice as much.

### Arena
- `--ours N --allies M --enemies K` (defaults 1, 0, 1; allies and enemies are BARb), `--ffa` (every enemy seat its own
  ally team), `--boxes corners|north-south|west-east`; allies share a box, each enemy ally team gets its own.
  The team-game test ground is Great Divide V1, north against south (user's choice, 2026-09-20).
  Results carry per-seat outcome; the referee declares a win when no enemy team lives and a loss when no seat on our
  ally team does.
- A human seat cannot be scripted headless; that case is tested by an allied BARb, which is the same code path for us.

### Judged by
A watched 2 (ours) v 2 BARb game and a 1 ours + 1 BARb v 2 BARb game on a larger map: both seats expand without
fighting over spots, attack the same base, turn to the second enemy when the first dies, and the record shows no build
order refused for "occupied by ally".

## 2. Reclaim and resurrect

### Facts to establish first (cheap experiments, before design hardens)
- Through the AI interface: `CMD_RESURRECT` on a feature id or an area; which units have it (`armrectr`, `cornecro`);
  what a wreck is worth (`FeatureDef_getContainedResource`), and whether a resurrected unit arrives at full health.
- How much metal lies on a typical battlefield of ours at minute 15-25 (from features, in a recorded game).

### Protocol and shim
- `Snapshot.wrecks: Vec<Wreck { id, pos, metal, resurrects_into: Option<UnitDefId> }>` for features with metal within
  sight of our units (capped, nearest our units first, refreshed every few seconds, not every tick).
- Commands `Resurrect { unit, feature | centre+radius }` and `ReclaimFeature { unit, feature }`.

### Brain
- H-REC-FIELDS: wreck fields (clusters of wrecks with their metal), ranked by metal over walking distance, skipping
  fields with enemies in sight or outside our territory (part 3).
- H-REC-CREW: resurrection bots built in proportion to metal lying in safe fields (one per ~600 metal, at most 6, none
  before the first field exists); they follow the army's rear rather than wait at home.
- Resurrect what is worth more than its reclaim value and that we would build anyway (our line units, enemy tier 2);
  reclaim the rest. When metal is short (income spent, nothing banked) reclaim first: metal now beats a unit later.
- The commander sees a `wrecks:` line (fields, metal, safe or not) and one lever (`reclaim: fields to work, in order;
  resurrect yes/no`).

### Judged by
A watched game: after the first big fight the crew walks out behind the army, the field's metal shows up in income,
and resurrected units join the home group.

## 3. Territory

### The idea
Ground is ours when we can answer for it sooner than the opponent can reach it. Everything that today asks a local
question with a clock (hot spots: "was something lost here within four minutes"; reach: "how far from home";
station: "the most exposed extractor") asks the territory instead.

### Model
- A coarse grid over the map (the terrain grid's cells, ~256 elmos). For each cell two numbers, updated every few seconds:
  `ours`: the combat value we can bring there within T seconds (soldiers and turrets, each discounted by its walking
  time to the cell; turrets count in range only), allies included; `theirs`: the same for enemies, from what is in sight
  now, what was seen lately (decaying over ~3 minutes, moved toward us at its speed: a raider seen two minutes ago
  can be anywhere within two minutes' walk) and a standing prior around each live enemy base.
- A cell is **held** (ours clearly larger), **contested**, or **theirs**. The front is the held cells that border
  contested ones.
- This is the one new structure; it replaces, not joins: H-ECO-HOT-SPOTS, H-ECO-REACH, H-ECO-FRONTIER's distance
  test, H-ECO-OWN-HALF, and the station rule's "most exposed extractor".

### What reads it
- Expansion: spots in held cells, nearest first; spots in contested cells only with an escort (below); never theirs.
- Station and posts: the home group stands on the front where the opponent's approaches are (the cells its recent
  raids crossed), split into at most two or three groups when the front is wider than one group can answer in time
  (the north-west collapse: 23 of 24 soldiers on one post, raiders arriving on a 2000-wide front).
- Escort: taking a contested spot moves part of the home group there first, then the constructor, then a turret; the
  cell becomes held or the attempt is abandoned.
- Turrets: outpost turrets go where they turn the most contested cells with extractors into held ones.
- Reclaim crews and constructors do not walk through cells that are theirs.
- The commander: the text map gains held / contested / theirs shading and the report a `ground:` line (spots held,
  contested, the front's width, where raids came from lately); its posts and expansion orders override as today.

### Judged by
Watched north-west games and Mithril Mountain: extractors beyond the first ring survive longer than four minutes on
average, constructors stop dying alone (deaths with nothing of ours within 600 fall from 12 of 19), the army stands
where the raids come in.

## 4. Opus as commander
- `--commander-model <id>` on the arena (today Sonnet is fixed); same prompt, tools and penalty. Compared against a
  stable Sonnet set on the same build: at least 4 games a corner for each, judged on the same measures as before
  (result, extractors and trade over time, turns and thought per game second) and on reading its notes.
- Runs on the second account, which has no paid overage; usage snapshot before, `--since` after, stop at exit 3.

## Order of work
1. Team games: protocol and shim (allies, teams), brain (enemy bases, allied spots), arena (N v M), then the board.
2. Territory: the grid and the readers, replacing the four rules named above.
3. Reclaim and resurrect: experiments, protocol, crew.
4. Sonnet baseline set on the result, then Opus.
Territory before reclaim because the crew needs to know where it is safe to walk.

## Status
- 2026-09-20, part 1 without the commander: built and watched in 2v2 and 1+BARb v 2 on Great Divide (`docs/experiments.md`,
  team-* rows). Protocol and shim (teams, start boxes from the setup script, allied units, enemy team ids, game id), enemy
  bases per seat, allied spots / ground / cover, the team board (claims, pooled buildings, one target, combined odds,
  joining a launch), H-TEAM-DEFEND, arena N v M. Differences from the text above: no `TeamDied` event (the interface has
  none; a base is called dead when it is found and then razed); enemy starts are start boxes, not positions (no call
  gives positions); H-TEAM-DEFEND was not planned and came from a lost game.
- Not built: the commander over several seats; parts 2-4.
