# Pitfalls that cost time once already

- **Never overwrite a loaded `.so` in place.** `cp` onto `libSkirmishAI.so` while a game had it mapped crashed the user's GUI
  game 24 s later (SIGSEGV, PC=0 on the main thread; coredumpctl). Installs replace by rename (`run/install_ai.sh`), and the
  BAR install holds its own copy, never a symlink to the development build.
- **`pkill -f <pattern>` / `pgrep -f` match the invoking shell's own command line** and kill it. Use PIDs or `pkill -x`.
- **The engine's process name is `recoil-main`,** not `spring-headless`: `pgrep -x spring-headless` finds nothing (it made a
  memory check report "0 engines" while 24 were running). Use `pgrep -x recoil-main`.
- **Do not end the engine with a signal.** SIGTERM/SIGINT leave a 0-byte replay and a truncated `infolog.txt`; end matches with
  `/kill` on the autohost channel. (Retired 2026-09-19: the earlier claim that the stdout log is block-buffered. It is
  line-flushed; the first smoke test looked cut off at frame 31 because the engine prints almost nothing during play.)
- **Factory build orders: SHIFT means "build five".** Enforced in the shim; see K-rules-factory-shift-means-five.
- **The arena rebuilds the workspace at start.** Do not leave `crates/` half-edited while a batch is starting.
- **Requested game speed changes outcomes** (see arena.md, OPEN). Evaluate at `--speed 50`.
- **12 matches is noise-level.** ±14 points at 50%. Confirm with 24+ before believing a gain.
- After a bot restart the brain takes the commander's current position as home; brain state is not persisted.
- **Unix socket paths max out at ~108 bytes.** A long batch label once pushed `bot.sock` past it: the bot died at start-up and
  BARb beat an idle team 11 times (batch strategist-refactor-regression, 0-1-11 — not a brain result). Arena sockets now live in
  `$XDG_RUNTIME_DIR`, and the arena aborts a match whose bot process exits.
- **Look for gross mistakes before measuring small ones** (user, 2026-09-19). BARb easy is trivial for a human; while we lose to
  it, the bot is doing something absurd, and a 24-match batch cannot resolve rule-sized effects anyway (the same binary scored
  13-6-5 and 10-9-5; a behaviourally identical build 6-10-8). The per-minute `rules:` line is the detector: a rule firing
  hundreds of times per match means its orders are not executing. `DROPPED order` lines in `bot.log` and
  `WITHIN_REASON_TRACE_BUILDS` placements in `engine.log` say which and where.

## A replay that names the game by rapid tag will not open in the lobby

The engine accepts `GameType=byar:test` and resolves the tag itself, but the name goes into the replay's header as
written. The BAR lobby reads the header, cannot resolve a tag, and sits at "byar:test: 0%". The arena now resolves the
tag to the full name (`resolve_game`, from `run/data/rapid/*/byar/versions.gz`) before writing the start script.
Replays recorded before that: `run/fix_replay.py <match dir>` writes a `.fixed.sdfz` copy with the header corrected
(checked: the copy loads and plays in the headless engine; the game stream is untouched).


## Engine calls that re-enter the AI

`COMMAND_CHEATS_GIVE_ME_NEW_UNIT` creates the unit before it returns, and the engine delivers the unit's events to
`handleEvent` from inside that call. The shim's exports hold the instance table's mutex, so issuing it from inside an
export hung the engine at the first spawn (watchdog stack trace through `Mutex::lock_contended`). The shim now queues
spawns and issues them after unlocking (`engine::Spawner`). Ordinary unit orders travel through the network layer and
do not re-enter. Any future synchronous engine call (other cheats, Lua calls) needs the same treatment.

## Mass self-destruct is cancelled twice per team

See `duels.md`, "Clearing": BAR cancels self-destruct orders covering 95% of a team's units, two times per team per game.

## Start boxes are the lobby's, per map

Until 2026-09-20 the arena's default boxes were 30 % corner squares of its own, which is not any map's layout in the
lobby: Comet Catcher is played W against E in full-height 20 % strips, Quicksilver N against S in full-width strips.
Results on the corners are balanced (BARb had the same handicap) but not the players' game: the corners leave out most
of a strip's extractor clusters. `--boxes standard` (the default) reads the lobby's saved boxes from
`crates/arena/startboxes.dat`; a map missing there fails at argument parsing, so copy its line from BYAR-Chobby's
`savedBoxes.dat` (or `mapDetails.lua`'s `StartboxesSet`, zlib and base64) rather than falling back to `corners`.

- The bot protocol changed on 2026-09-22 (`UnitDamaged` gained `from` and `weapon`): the arena builds both sides, but
  GUI play needs `run/install_to_bar.sh` re-run so the installed shim matches the bot.
