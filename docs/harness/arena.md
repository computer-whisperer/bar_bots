# Arena — batch evaluation

`target/release/arena [--matches N] [--parallel N] [--speed N] [--profile easy|medium|hard|hard_aggressive] [--map NAME]
[--max-minutes N] [--label TEXT]`

Per match: a directory `run/matches/<unix-stamp>-<label>/NN/` holding `script.txt`, `engine.log`, `bot.log`, the replay under
`demos/`, and the engine's write-dir litter; `results.jsonl` per batch. The arena rebuilds and reinstalls the AI first
(`run/install_ai.sh`), starts one bot and one engine per match with a private socket and ports `9100 + 2*index`, requests
the speed over the autohost channel, reads the winner from SERVER_GAMEOVER, and ends the engine with `/kill`.
- Corner and faction alternate by match index; the opponent always gets the other faction (see K-opp-faction-asymmetry).
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
