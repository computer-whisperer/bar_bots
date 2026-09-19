# bar_bots — target design (MVP)

Ratified 2026-09-19. Findings and test results are in `NOTES.md`.

## Shape

```
 Recoil engine client (spring / spring-headless)          bot process (Rust binary)
 ┌───────────────────────────────────────────┐            ┌──────────────────────────────┐
 │ libSkirmishAI.so  = crates/ai-shim        │   Unix     │ crates/bot                   │
 │  engine thread only:                      │  socket    │  one thread per AI instance  │
 │   events ──► buffer                       │ ─────────► │  Hello, then Tick → Commands │
 │   every N frames: snapshot via callbacks  │ ◄───────── │  heuristic brain (MVP)       │
 │   apply Commands via Engine_handleCommand │            │  later: Jev / LLM layers     │
 └───────────────────────────────────────────┘            └──────────────────────────────┘
```

The shim is the only code that touches the engine. It is dumb on purpose: no strategy, no blocking.
The bot process never calls into the engine; everything it knows arrives in `Hello` and `Tick`.

## Crates
- `recoil-ai-sys` — bindgen over vendored engine headers (`rts/ExternalAI/Interface/*.h`, `System/{Export,Main}Defines.h`,
  GPL-2.0-or-later, so this crate and the shim are too). Header version is pinned by the vendored copy; re-vendor on engine bumps.
- `bot-protocol` — serde message types + length-prefixed postcard framing. Shared by shim and bot. No engine types leak into it.
- `ai-shim` — cdylib. `engine.rs` is the safe wrapper over the callback table, exposing only what the snapshot and commands need.
- `bot` — the bot binary. Listens on the socket; MVP heuristic brain.

## Protocol (credit-based, so the shim never blocks the sim)
1. shim → bot `Hello { ai_id, team, ally_team, frame, map, unit_defs, metal_spots }` once per connection.
2. bot → shim `Commands(vec)` — also serves as "ready for next tick" credit.
3. shim → bot `Tick { frame, events, snapshot }` — only when it holds credit and `frame % tick_interval == 0`. Events that occur
   while waiting are buffered, not dropped. Snapshot = economy, own units, visible enemies.
4. Shim reads the socket non-blocking at each UPDATE and applies any `Commands` on the engine thread.

If the bot is absent or dies, the shim keeps the game running and retries the connection about once a second, re-sending `Hello`.
Socket path: `$BAR_BOTS_SOCKET`, else `$XDG_RUNTIME_DIR/bar_bots.sock`.

## MVP brain (deliberately weak; exists to exercise the loop end to end)
The game places AI teams itself (`game_initial_spawn.lua` guesses a spot in the start box), so no start position is sent.
Commander and constructors build metal extractors on nearest spots, energy, a bot lab (more when metal floats) → labs queue cheap combat units →
idle army is sent with Fight orders at the enemy start / last seen enemy once it reaches a threshold.
Armada and Cortex name tables only; Legion and everything else is out of scope for the MVP.

## Done means
`run/` script plays shim+bot vs BARb headless; log shows buildings completed, units produced, an attack order issued, and
the bot process can be killed and restarted mid-game without stalling the match.
