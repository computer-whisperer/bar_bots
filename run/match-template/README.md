Copied into every arena match directory before the engine starts (see `docs/harness/arena.md`, "Efficiency").
- `springsettings.cfg` — one worker thread (the default pool of 12 is a net loss and burns system CPU), smaller memory pools.
- `LuaUI/Config/BYAR.lua` — disables every stock interface widget; regenerate the list when the game adds widgets.
- `LuaUI/Widgets/arena_quit.lua` — quits the client at game over instead of waiting for the game's 12 s autoquit.
The archive cache is machine-specific and lives untracked in `run/cache-template/`; the arena refreshes it after each batch.
