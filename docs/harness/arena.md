# Arena — batch evaluation

`target/release/arena [--matches N] [--parallel N] [--speed N] [--profile easy|medium|hard|hard_aggressive] [--map NAME]
[--max-minutes N] [--label TEXT] [--base-port N]`

Per match: a directory `run/matches/<unix-stamp>-<label>/NN/` holding `script.txt`, `engine.log`, `bot.log`, the match record
`record-<ai_id>.jsonl` (`record-format.md`; open it with `run/view_match.py <match dir>`), the replay under `demos/`, and the engine's write-dir litter; `results.jsonl` per batch. The arena rebuilds and reinstalls the AI first
(`run/install_ai.sh`), starts one bot and one engine per match with a private socket and ports `9100 + 2*index`, requests
the speed over the autohost channel, reads the winner from SERVER_GAMEOVER, and ends the engine with `/kill`.
- Two arenas on one machine need separate port ranges: give the second `--base-port 9300` (match i uses N+2i, N+2i+1).
  Without it the second arena's matches fail to bind the autohost port of the first.
- Corner and faction alternate by match index; the opponent gets the other faction, or ours with `--mirror`.
- After a batch the arena prints mean heuristic firings per match for wins against losses (from the `rules:` lines in
  `bot.log`) and a ledger row for `docs/experiments.md`; `batch.json` records label, commit and options.
- `game_minutes` is read from the last `[f=N]` in the engine log after exit.
- Timeout is game time: the shim prints `heartbeat f=N` once per game minute into the engine log and the arena stops the
  match at `--max-minutes` (default 40). A 120 s wall-clock stall allowance is the backstop.
- Refuses to start if `--parallel` x 4 GB exceeds available memory (a 24-way run once took ~100 GB).
- Throughput on the 24-core machine BEFORE the efficiency changes below: a lone match sustains ~35x at requested 50-80; 8 parallel ~10x each
  (8 matches: 3m06s wall, 28 min user + 6.5 min system CPU). ~2.5-3 cores per match.

Reading a match: `grep " min) \|attackers \|wave " NN/bot.log` gives the per-minute economy line, attacker centroid and
wave launches. Comparing those lines between a win and a loss found every bug so far.

## Efficiency (2026-09-19; measured by a research agent, then verified with arena batches)
Every match dir is seeded from `run/match-template/` and, when present, `run/cache-template/` (untracked, refreshed after each batch).
| Change | Where | Effect |
|---|---|---|
| `WorkerThreadCount = 1` | template `springsettings.cfg` | the default 12-thread pool is a net loss even for one match: idle workers spin on futexes (`ThreadPool.cpp:248-266`); per match to 12 game-min, 8 parallel: 25.7 s / 41.8 user / 15.4 sys -> 13.6 s / 16.0 / 0.5 |
| `UseLuaMemPools = 0`, `TextureMemPoolSize = 256` | same | peak RSS per engine 5.3 GB -> 3.3 GB (64 segfaults, 128 ran); ~7% slower play |
| archive cache pre-seeded | `run/cache-template/` | skips ~3 s of checksumming per match |
| `GameStartDelay=0` | start script | skips a 4 s countdown |
| quit widget | template `LuaUI/Widgets/arena_quit.lua` | client exits ~2.6 s after game over instead of waiting for BAR's 12 s autoquit |
| all stock widgets disabled | template `LuaUI/Config/BYAR.lua` | ~5% play time, -170 MB, no GL error noise; regenerate the list when the game adds widgets |
Verified end to end: 12 matches, 8 parallel, speed 50 now cost 16.5 min user + 0.4 min sys CPU (was 28 + 6.5 for 8 matches)
with results in line with earlier batches (eff-speed50: 8-3-1; eff-speed50-par12: 5-6-1).
Did not matter: demo recording, sound, logging, core pinning (`taskset` was slower), `ServerSleepTime`, `PathingThreadCount`.
Remaining load time (~7-10 s) is pathfinder init, LuaRules, LuaUI parsing, atlases; no headless switch found. The static
~1.7 GB memory floor is compiled-in pools.

**OPEN — requested speed changes outcomes.** `--speed 200 --parallel 12` gave 1-5-6 (eff-applied) where `--speed 50` gives the
usual ~58% at both 8 and 12 parallel. Games ran long and both sides developed slowly. Cause unknown; candidates: BARb's
threaded planning runs on wall-clock time, or our tick round-trip falls behind when frames are not paced (at a capped speed
the client sleeps between frames, `Game.cpp:1825-1835`). Until explained, evaluate at `--speed 50` (the default) and never
compare batches run at different requested speeds.

## Watching the opponent

`WITHIN_REASON_OBSERVE=1` makes the shim write a census of both sides to `engine.log` once a game minute (unit types,
counts, mean positions). It switches the engine's cheat callbacks on for the length of that one query only; the bot's own
view stays fair. `run/compare_census.py run/matches/<batch>/<NN> [--detail MINUTE]` prints the two sides beside each other.
`--corner nw|se` fixes our start (`nw` is the first start box of the layout); `--swap-corners` puts ally team 0 in the second box.

Team games: `--ours N --allies N --enemies N` (seats of ours, BARb seats on our side, BARb seats against us; default 1 0 1),
`--ffa` (every enemy seat its own ally team; corners only, at most 3), `--boxes corners|north-south|west-east` (default
corners; Great Divide V1 is played north against south). Allies share a start box and the game places them in it. An
allied BARb plays the other faction. One bot process serves all our seats of a match, one session each, joined by the
team board (`crates/bot/src/team.rs`); each seat writes its own `record-<ai>.jsonl`. A human ally cannot be scripted
headless; an allied BARb is the same code path for us. The referee's balance line counts our whole ally team against every enemy.

## Post-game analysis

With `WITHIN_REASON_OBSERVE=1` the shim also writes `truth-<ai>.jsonl` into the match directory: every enemy unit
(`[id, name, x, z, health %, being built]`) every two seconds, read through the engine's cheat callbacks switched on for
that one query. It is for analysis only; the bot never sees it.

Controlling for the opponent: BARb's profile is the only choice the lobby offers, but within a profile its first
factory varies (bot lab or vehicle plant) and with it the whole game (K-barb-opening-varies). `--opponent-opening
bots|vehicles` pins it; with `WITHIN_REASON_OBSERVE=1` every result carries `opponent_first_factory`, and
`run/batch_curves.py --by-opening` splits the curves by it. Match i plays seed `--seed-base` + i (engine and BARb), and the
two arms of an A/B batch meet the same seeds; the seed does not make BARb repeatable.

Stopping a match by hand: `run/stop_match.py <match or batch dir> [loss|win]` writes a `stop` file the arena's referee
sees within a second; the match is recorded with that outcome (a timeout when none is given, `called: true`) and the
engine is ended with `/kill` like any other, so the replay is written. Do not kill the arena or the engine: the replay
stays at 0 bytes and the record gets no result line (commander-3 and -4 were lost that way).

`run/batch_curves.py <batch dir> [minute ...]` prints a batch's mean curves (extractors, builders, army value, turrets,
ours/theirs) by A/B arm and start corner: the first thing to read after a batch, before the win count.

`run/spot_regret.py <batch or match dir>...` (matches run with `WITHIN_REASON_OBSERVE=1`) classes every metal spot second by
second as ours, theirs, threatened or quiet: extractor-minutes forgone on quiet free ground, and how soon raided ground
is visited again (K-eco-raided-ground-is-raided-again).

`run/analyze_match.py <match dir>` turns a recorded match into what a reader needs to say why it was lost:
- curves for both sides per minute (extractors, builders, army and turret value, factories, our bank and income);
- candidate causes with their numbers (army lead and when it opened, extractor peak and collapse, idle metal and energy
  stalls, metal lost by place, raids, the worst engagement, how many engagements began outnumbered, the commander's death);
- engagements: deaths on both sides clustered in space and time (900 elmos, 20 s), with losses by type and value, where,
  and what each side had on the spot five seconds before, including how spread out our fighters were and turrets present;
- `--engagement N` or `--scene MM:SS X Z`: scene reports, a character map of the ground (water, cliffs) with both sides'
  units as letters (ours lower case, theirs upper case), recent deaths marked, and a legend with counts, classes, health,
  fighting value and our soldiers' roles (home group, attack wave, squad).
Without a truth file the opponent is only what our units saw, and the report says so. The verdict itself is left to
the reader: the tool gives numbers and scenes, not conclusions.

## Calling settled games

The shim logs `balance f=N ours=ARMY/EXTRACTORS theirs=ARMY/EXTRACTORS` every 30 game seconds (soldiers' metal value and
finished extractors, the opponent's read through the cheat callbacks; the bot never sees it). The arena's referee ends
a game as a **called loss** once they have had 3x our army and 3x our extractors for 2 minutes (not before minute 6),
and as a **called win** once we have had 5x their army and 3x their extractors for 5 minutes (not before minute 12):
BARb comes back from early deficits, we do not. Replayed over v17-truth-medium's 24 games the loss rule called 10 of
14 losses 1-16 minutes early and nothing else; the win rule would have called one 40-minute timeout at about minute 29
(67,000 metal of army against 5,900: a game the bot could not finish, which is its own finding). `results.jsonl` marks
such games `"called": true` and the batch summary counts them; `--play-out` disables calling.

