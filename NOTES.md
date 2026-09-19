# bar_bots — harness findings (2026-09-19)

Status: read from source and release archive listing only. Nothing has been run yet.

## Repos (shallow clones in `upstream/`)
- `RecoilEngine` — engine (Spring fork). AI ABI in `rts/ExternalAI/Interface/`.
  Submodules NOT fetched (BARb, CircuitAI-zk, rts/lib/*).
- `Beyond-All-Reason` — the game: Lua content, 5.2G. Lua AIs listed in `luaai.lua`.
- `CircuitAI` @ branch `barbarian` — BARb, the native C++ AI BAR ships. (`master` is an empty stub.)

## Engine release (github beyond-all-reason/RecoilEngine, tag 2026.09.01, 31MB linux)
Ships: `spring`, `spring-headless`, `spring-dedicated`, `pr-downloader`, `libunitsync.so`,
`AI/Interfaces/C/0.1/libAIInterface.so`, `AI/Skirmish/{BARb,CircuitAI,NullAI}`.

## Native AI ABI
- Discovery: `{datadir}/AI/Skirmish/<ShortName>/<Version>/AIInfo.lua` + `libSkirmishAI.so`
  (AILibraryManager.cpp:183).
- Exports: `init(aiId, SSkirmishAICallback*)`, `release(aiId)`, `handleEvent(aiId, topic, data)`.
- 27 event topics (AISEvents.h), ~94 command structs (AISCommands.h), callback table with
  ~596 function pointers (SSkirmishAICallback.h).
- Lua bridge: COMMAND_CALL_LUA_RULES / COMMAND_CALL_LUA_UI out, EVENT_LUA_MESSAGE in.
- Dispatch: `eoh->Update()` from Game.cpp:1748 — in-process, in the game loop.
- AI orders travel as NETMSG_AICOMMAND from the hosting client through the server.

## Start script (`doc/StartScriptFormat.txt`)
`[AI0] { ShortName=; Team=; Host=<player number>; [OPTIONS]{} }` — `Host` selects which
player's client process loads and runs the AI. `FixedRNGSeed`, `MinSpeed/MaxSpeed` exist.
Lua AIs are set per-team with `LuaAI=name`.

## Three integration routes
1. Native skirmish AI (.so via C interface) — full AI API, how BARb works.
2. Lua AI gadget inside the game archive (SimpleAI, Scavengers, Raptors) — synced, needs a game fork/mutator.
3. LuaUI widget driving a player slot — plays as a "human", limited to UI-visible info.

## Smoke test — PASSED (2026-09-19)
`run/smoke_barb_vs_barb.txt` + `run/autohost_probe.py`; engine 2026.09.01, game `byar:test` (test-31357), Quicksilver Remake 1.24.
- Setup: unpack release to `run/engine`; `pr-downloader --filesystem-writepath run/data --download-game byar:test`
  with env `PRD_RAPID_USE_STREAMER=false PRD_RAPID_REPO_MASTER=https://repos-cdn.beyondallreason.dev/repos.gz
  PRD_HTTP_SEARCH_URL=https://files-cdn.beyondallreason.dev/find`.
- Run: `spring-headless --isolation --write-dir run/data <script>`. Load ~12s, then 3240 frames in 108s = 1x realtime
  (MaxSpeed modoption alone does not speed it up; needs a speed request — not yet investigated).
- Autohost UDP (`AutohostIP/AutohostPort` in script): server -> us events (AutohostInterface.cpp enum: STARTED, STARTPLAYING,
  GAMEOVER, PLAYER_*, GAME_LUAMSG, TEAMSTAT); us -> server plain-text server commands (`/kill` verified; also kick,
  setmaxspeed, forcestart, cheat...). No TEAMSTAT packets seen in 108s of play.
- Engine log is block-buffered: only trustworthy after graceful exit (`/kill`), not after SIGTERM.
- BARb loads its AngelScript/config from the GAME archive (`LuaRules/Configs/BARb/stable/...`), not from the engine's AI dir.
- Replay written to `run/data/demos/*.sdfz` on graceful exit.

## Lobby question (2026-09-19) — source reading only, nothing tested against the live server
Extra clones in `upstream/`: BYAR-Chobby (lobby client), teiserver (lobby server), SPADS (autohost), bar-lobby (new client, not read).
- Chobby lists AIs from the LOCAL install: `VFS.GetAvailableAIs` (ai_list_window.lua:14). Blacklist is only `CircuitAI`.
  With the "simple AI list" setting on, AIs without a simple name are hidden (ai_list_window.lua:86-91) — turn it off to see a custom AI.
- teiserver `ADDBOT` (spring_in.ex:1169): stores `ai_dll` as an opaque string, owner = the adding user. No AI-name validation.
  Permission `Lobby.allow?(:add_bot)` (lobby.ex:747): SPECTATORS CANNOT ADD BOTS (`player_command and changer.player == false -> false`);
  founder/moderators always can. Tachyon path (`lobby/addBot`, tachyon_handler.ex:884) not read.
- SPADS (spads.pl ~14360): only counts bots against maxBots / maxLocalBots / maxRemoteBots; no AI-name validation for remote bots.
  "Local bots" (owned by the autohost itself) are restricted by `allowedLocalAIs` and need `springServerType` headless.
- Unknown: the deployed BAR hosts' actual maxRemoteBots etc.; whether a bot survives its owner switching to spectator;
  game-side gadgets touching AIs (ai_namer.lua etc.) not read.
