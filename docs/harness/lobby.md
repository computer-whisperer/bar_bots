# Getting the bot into real games

Target flow (user, 2026-09-19): the user's own GUI client joins a lobby with a cooperative group, adds our local AI, and the
AI runs on his machine (his compute, his inference subscriptions). Own autohost is the alternative.

## Local install — verified in the GUI 2026-09-19
- BAR data dir `~/.local/state/Beyond All Reason`, engine `recoil_2026.07.04`.
- `run/install_to_bar.sh` COPIES the AI into `<data dir>/AI/Skirmish/BarBots/0.1/`. Never symlink a development build:
  see pitfalls.md.
- The lobby hides AIs without a friendly name unless "Simplified AI list" is off. That checkbox is in the Developer settings
  tab, which exists only in dev mode = a `devmode.txt` file in the BAR data dir (Chobby `configuration.lua:313`). Created.
- Start `target/release/bot` with `BAR_BOTS_SOCKET` unset; shim and bot both default to `$XDG_RUNTIME_DIR/bar_bots.sock`.
  The shim retries about once a second, so the bot may start late or be restarted mid-game.

## Public lobbies — source reading only, untested against the live server
- Chobby lists AIs from the LOCAL install (`VFS.GetAvailableAIs`, ai_list_window.lua:14); blacklist is only `CircuitAI`.
- teiserver `ADDBOT` (spring_in.ex:1169) stores the AI name as an opaque string, owner = the adding user; no validation.
  `Lobby.allow?(:add_bot)` (lobby.ex:747): SPECTATORS CANNOT ADD BOTS; founder and moderators always can.
  A bot is removed only when its owner LEAVES the lobby (lobby.ex:447), so "join as player, add the AI, switch to spectator"
  should keep it. Tachyon path (`lobby/addBot`, tachyon_handler.ex:884) not read.
- SPADS (spads.pl ~14360) only counts bots against maxBots / maxLocalBots / maxRemoteBots at add time. Autohost-owned
  "local bots" are limited by `allowedLocalAIs` and need `springServerType` headless.
- Unknown: deployed hosts' bot limits; game-side gadgets touching AIs (`ai_namer.lua`) not read.
