# Arena — batch evaluation

`target/release/arena [--matches N] [--parallel N] [--speed N] [--profile easy|medium|hard|hard_aggressive] [--map NAME]
[--max-minutes N] [--label TEXT]`

Per match: a directory `run/matches/<unix-stamp>-<label>/NN/` holding `script.txt`, `engine.log`, `bot.log`, the replay under
`demos/`, and the engine's write-dir litter; `results.jsonl` per batch. The arena rebuilds and reinstalls the AI first
(`run/install_ai.sh`), starts one bot and one engine per match with a private socket and ports `9100 + 2*index`, requests
the speed over the autohost channel, reads the winner from SERVER_GAMEOVER, and ends the engine with `/kill`.
- Corner and faction alternate by match index; the opponent always gets the other faction (see K-opp-faction-asymmetry).
- `game_minutes` is read from the last `[f=N]` in the engine log after exit.
- Timeout is wall-clock: `--max-minutes` of game time at an assumed floor of 5x, so faster matches get more game time.
- Throughput on the 24-core machine: a lone match sustains ~35x at requested 50-80; 8 parallel ~10x each
  (8 matches: 3m06s wall, 28 min user + 6.5 min system CPU). ~2.5-3 cores per match.

Reading a match: `grep " min) \|attackers \|wave " NN/bot.log` gives the per-minute economy line, attacker centroid and
wave launches. Comparing those lines between a win and a loss found every bug so far.
