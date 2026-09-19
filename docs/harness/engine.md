# Engine, game and the native AI interface

## Sources (shallow clones in `upstream/`, git-ignored)
- `RecoilEngine` — engine (Spring fork). AI interface in `rts/ExternalAI/Interface/`. Submodules not fetched.
- `Beyond-All-Reason` — the game: Lua content, 5.2G. Lua AIs listed in `luaai.lua`.
- `CircuitAI` @ `barbarian` — BARb, the C++ AI BAR ships (`master` is an empty stub).
- `BYAR-Chobby`, `teiserver`, `SPADS`, `bar-lobby` — lobby client, lobby server, autohost, new lobby (not read).

## Engine releases (github beyond-all-reason/RecoilEngine)
A 31MB Linux archive ships `spring`, `spring-headless`, `spring-dedicated`, `pr-downloader`, `libunitsync.so`,
`AI/Interfaces/C/0.1/libAIInterface.so`, `AI/Skirmish/{BARb,CircuitAI,NullAI}`.
- `run/engine` -> `run/engines/2026.07.04` — the version the user's BAR install runs. Use this.
- `run/engines/2026.09.01` — DO NOT USE for batches: SIGSEGV in a pathfinder worker thread in 3 of 18 headless matches,
  `QTPFS::IPath::SetPoint` (Path.h:239) via `PathSearch::Finalize` / `LoadPartialPath` <- `PathManager::ExecuteSearch`
  (symbolized with the release's `-dbgsym` archive). 0 crashes in 100+ matches on 2026.07.04. Not reported upstream.
- AI interface headers are byte-identical between the two tags, so one shim build serves both.

## Game data
`pr-downloader --filesystem-writepath run/data --download-game byar:test` (and `--download-map "<name>"`) with env
`PRD_RAPID_USE_STREAMER=false PRD_RAPID_REPO_MASTER=https://repos-cdn.beyondallreason.dev/repos.gz
PRD_HTTP_SEARCH_URL=https://files-cdn.beyondallreason.dev/find`. Currently: test-31357, Quicksilver Remake 1.24.

## Native AI interface
- Discovery: `<data dir>/AI/Skirmish/<ShortName>/<Version>/{AIInfo.lua,libSkirmishAI.so}` (AILibraryManager.cpp:183); any data
  dir counts, including the write dir.
- Exports: `init(aiId, SSkirmishAICallback*)`, `release(aiId)`, `handleEvent(aiId, topic, data)`.
- 27 event topics (AISEvents.h), ~94 command structs (AISCommands.h), ~596 callbacks (SSkirmishAICallback.h). Array
  callbacks return the size when passed a null buffer; metal spot arrays are counted in floats, `y` holds the metal value.
- One UPDATE event per sim frame (30/s). Dispatch is in-process in the game loop (`eoh->Update()`, Game.cpp:1748): a
  blocking AI stalls its host client.
- Lua bridge: COMMAND_CALL_LUA_RULES / COMMAND_CALL_LUA_UI out, EVENT_LUA_MESSAGE in (unused so far).
- AI orders travel as NETMSG_AICOMMAND from the hosting client through the server; other clients do not need the AI's files.
- The AI's stderr lands in the engine's stdout log.

## Start script (`doc/StartScriptFormat.txt`)
`[AI0] { ShortName=; Version=; Team=; Host=<player number>; [OPTIONS]{} }` — `Host` selects which player's client loads
and runs the AI. `FixedRNGSeed` exists. Lua AIs are set per team with `LuaAI=name`. `GameType` accepts a rapid tag (`byar:test`).

## Three ways to be a bot
1. Native skirmish AI (.so via the C interface) — full AI API, fog of war respected. What we do; how BARb works.
2. Lua AI gadget inside the game archive (SimpleAI, Scavengers, Raptors) — synced code, needs a game fork or mutator.
3. LuaUI widget driving a player slot — plays as a human; the only route that needs no AI slot.

## Running headless
`spring-headless --isolation --write-dir <dir> <script>`; extra read-only data via `SPRING_DATADIR` (honoured in isolation
mode, DataDirLocater.cpp:432). Load takes ~12 s. The engine log is block-buffered: complete only after a graceful exit.
Autohost channel (`AutohostIP`/`AutohostPort` in the script): server -> us UDP events (AutohostInterface.cpp: SERVER_STARTED 0,
QUIT 1, STARTPLAYING 2, GAMEOVER 3 `[id, size, player, winning ally teams...]`, MESSAGE 4, PLAYER_* 10-14, GAME_LUAMSG 20,
TEAMSTAT 60); us -> server plain-text commands (`/kill`, `/setminspeed`, `/setmaxspeed`, `/kick`, `/forcestart`, `/cheat`...).
Speed: `/setmaxspeed N` then `/setminspeed N` at STARTPLAYING forces the speed up; the `MaxSpeed` mod option alone does nothing.
