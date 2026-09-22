# Getting the bot into real games

Target flow (user, 2026-09-19): the user's own GUI client joins a lobby with a cooperative group, adds our local AI, and the
AI runs on his machine (his compute, his inference subscriptions). Own autohost is the alternative.

## Local install — verified in the GUI 2026-09-19
- BAR data dir `~/.local/share/BeyondAllReason` (moved there from `~/.local/state/Beyond All Reason` by 2026-09-22; the
  install script's default), engine `recoil_2026.07.04`, the arena's engine too.
- `run/install_to_bar.sh` COPIES the AI into `<data dir>/AI/Skirmish/WReason/0.1/`. Never symlink a development build:
  see pitfalls.md.
- The lobby hides AIs without a friendly name unless "Simplified AI list" is off. That checkbox is in the Developer settings
  tab, which exists only in dev mode = a `devmode.txt` file in the BAR data dir (Chobby `configuration.lua:313`). The
  install script creates it.
- A game with people: `run/human_game.sh [label] [effort]` starts the bot as the realtime pianist with the player over
  it, recording into `run/matches/<time>-<label>/00`; then add "Within Reason" in the lobby. Re-run
  `run/install_to_bar.sh` after any shim or protocol change (pitfalls.md).
- Start `target/release/bot` with `WITHIN_REASON_SOCKET` unset; shim and bot both default to `$XDG_RUNTIME_DIR/within-reason.sock`.
  The shim retries about once a second, so the bot may start late or be restarted mid-game.
- **Text edits need no rebuild** (`crates/bot/src/texts.rs`): the prompts (`crates/bot/src/strategist/*.md`), the
  briefs (`docs/briefs/*.md`) and the hands' `rules.md` and `default.md` are read from the checkout at every use. The
  hands see a rules edit on their next call; the player gets a fresh session (handed the notes, as at the 40-turn cap)
  at its next turn after the prompt or brief changed on disk. `bot.log` says which files are read from where; a bot
  run away from its checkout uses the compiled copies (`WITHIN_REASON_TEXTS=<checkout>` names it).

## Public lobbies — source reading only, untested against the live server
- Chobby lists AIs from the LOCAL install (`VFS.GetAvailableAIs`, ai_list_window.lua:14); blacklist is only `CircuitAI`.
- teiserver `ADDBOT` (spring_in.ex:1169) stores the AI name as an opaque string, owner = the adding user; no validation.
  `Lobby.allow?(:add_bot)` (lobby.ex:747): SPECTATORS CANNOT ADD BOTS; founder and moderators always can.
  A bot is removed only when its owner LEAVES the lobby (lobby.ex:447), so "join as player, add the AI, switch to spectator"
  should keep it. Tachyon path (`lobby/addBot`, tachyon_handler.ex:884) not read.
- SPADS (spads.pl ~14360) only counts bots against maxBots / maxLocalBots / maxRemoteBots at add time. Autohost-owned
  "local bots" are limited by `allowedLocalAIs` and need `springServerType` headless.
- Unknown: deployed hosts' bot limits; game-side gadgets touching AIs (`ai_namer.lua`) not read.
