# Within Reason — target design (MVP)

Ratified 2026-09-19. Findings live under `docs/` (start at `docs/README.md`).

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
- `buildorder` — offline study tool, not part of the running system: a tier-1 economy simulator and a simulated-annealing
  build-order search over it (`docs/studies/build-order.md`). Depends on nothing else in the workspace.

## Protocol (credit-based, so the shim never blocks the sim)
1. shim → bot `Hello { ai_id, team, ally_team, frame, map, unit_defs, metal_spots }` once per connection.
2. bot → shim `Commands(vec)` — also serves as "ready for next tick" credit.
3. shim → bot `Tick { frame, events, snapshot }` — only when it holds credit and `frame % tick_interval == 0`. Events that occur
   while waiting are buffered, not dropped. Snapshot = economy, own units, visible enemies.
4. Shim reads the socket non-blocking at each UPDATE and applies any `Commands` on the engine thread.

If the bot is absent or dies, the shim keeps the game running and retries the connection about once a second, re-sending `Hello`.
Socket path: `$WITHIN_REASON_SOCKET`, else `$XDG_RUNTIME_DIR/within-reason.sock`.

## MVP brain (deliberately weak; exists to exercise the loop end to end)
The game places AI teams itself (`game_initial_spawn.lua` guesses a spot in the start box), so no start position is sent.
Commander and constructors build metal extractors on nearest spots, energy, a bot lab (more when metal floats) → labs queue cheap combat units →
idle army is sent with Fight orders at the enemy start / last seen enemy once it reaches a threshold.
Armada and Cortex name tables only; Legion and everything else is out of scope for the MVP.

## Done means
`run/` script plays shim+bot vs BARb headless; log shows buildings completed, units produced, an attack order issued, and
the bot process can be killed and restarted mid-game without stalling the match.

## Match record and viewer (2026-09-19)
The bot process, which sees everything the brain sees, writes a JSON Lines record per match (`crates/bot/src/recorder.rs`,
format in `docs/harness/record-format.md`): state samples once a game second, engine events, commands, and decision
records in one source-agnostic shape (source, frame, inputs, outputs, latency) fed by the brain's journal
(`brain/journal.rs`). `viewer/` is a static page that replays it beside the LLM transcript and the census.
Chose-because: the shim and the protocol stay untouched, and the record is complete for our side by construction.
Rejected: parsing the engine's `.sdfz` (it re-simulates in the engine and holds none of our decisions); scraping `bot.log`
(free text, per-minute granularity).

---

# Strategist: Opus observing and directing through MCP — ratified 2026-09-19

Ratified order (2026-09-19): docs loop → this → Lua runtime designed from what Opus tries to express → Opus programs Lua →
heuristic + Jev. Verified mechanics are in `docs/harness/claude-p.md`.

## Principle
Opus is never in the control loop. A decision takes 4-8 s; the Rust brain keeps playing every tick. Opus reads a summary of
the game and sets **directives** — parameters and targets that the existing heuristics consult. Every directive expires, so a
slow, dead or rate-limited strategist degrades to the plain heuristic bot rather than wedging it.

```
 claude -p (Opus, stream-json, MCP tools only)
      ▲ user turns: periodic + triggered events          ┌──────────── bot process ─────────────┐
      │                                                   │ strategist driver (one per AI)       │
      └── stdin/stdout ──────────────────────────────────►│   spawns claude, sends turns, logs   │
      ── HTTP MCP (127.0.0.1) ───────────────────────────►│ MCP server ── reads ──► Briefing     │
                                                          │              writes ─► Directives    │
                                                          │ brain (every tick): writes Briefing, │
                                                          │   reads Directives                   │
                                                          └──────────────────────────────────────┘
```

## Pieces
- **Briefing** — what the brain publishes each tick for the strategist to read: game time, economy, counts by role,
  home group / attackers (size, composition, centroid), enemy intel (clusters of seen enemies with last-seen time, buildings
  remembered after they leave sight), metal spots by owner (ours / enemy seen / free), recent events digest, directives in
  force, which heuristics fired. Positions are given as map coordinates plus a coarse named grid (e.g. "C4") so they can be
  talked about.
- **Directives** — typed, each with a time-to-live: `army_stance` (defend / gather / attack), `attack_target` (position or
  enemy cluster id), `wave_size`, `economy_focus` (expand / energy / production / defence), `production_mix` (weights by
  role), `expansion_reach`, `defend_outposts`. Each maps onto registered heuristic IDs, which read "directive, else default".
- **MCP tools** — observe: `overview`, `army`, `enemy_intel`, `map`, `events_since`. Direct: `set_directives` (any subset,
  with ttl), `clear_directives`. Record: `note` (the strategist's reasoning in a sentence or two, stored with the game time).
  Hand-rolled minimal JSON-RPC over HTTP in the bot (the verified surface is five methods); no async runtime needed.
- **Driver** — spawns one `claude -p` per AI session at game start when `--strategist opus` is given. Sends a turn every
  ~45 s of game time and immediately on triggers (base or outpost under attack, commander damaged, wave destroyed, first
  sight of a new enemy unit class). One turn in flight; triggers that arrive meanwhile are coalesced into the next turn.
- **Transcript** — `strategist.jsonl` per match: every turn's prompt, tool calls and results, directives set, notes, usage.
  This is the raw material for the knowledge base and, later, labelled decisions for Jev.

## Evaluation
Strategist matches run at 1-3x real time (a decision at arena speed 50 would span minutes of game time), a few at a time,
headless or in the user's GUI client. Compare against the same brain without a strategist on the same seeds; expect to need
more games per comparison because the strategist adds variance. Subscription rate limits are read from the stream's
`rate_limit_event` messages; on a limit the driver stops sending turns and the match continues on heuristics.

## Safety
The session gets no built-in tools (`--tools ""`), only our MCP server, no user or project settings, an empty working
directory and a replaced system prompt. The MCP server binds to 127.0.0.1 and exposes game state and directives only.
No credentials in the repository or in transcripts.

## Decisions (user, 2026-09-19)
1. Directives start small: `army_stance`, `attack_target`, `wave_size`, `economy_focus`. Chose-because: the transcripts
   should show what is missing before we add levers; too fine a set is the Lua layer by another name.
2. First strategist games run in the arena against BARb at 2x speed (repeatable), GUI games later.
The first tool set is correspondingly small: `overview`, `map`, `set_directives`, `note`.

## What the first strategist game asked for (opus-first, 2026-09-19)
Digest: `docs/transcripts/2026-09-19-opus-first.md`. Opus's stated wishes, verbatim themes:
1. **Station the army at a map point as an escort** (asked three times) — the home group idles at base while outposts die.
2. **Force converter count / order more constructors directly** — `economy_focus` was too indirect: `energy` added generators
   but no converters, `expand` could not overcome a single surviving constructor.
Observed problems on our side: the base-attack trigger fired for lone raiders (11 of 33 turns), spending turns on noise; the
constructor count rule (2 + extractors/4) collapses exactly when extractors are being lost, which is when more are needed.
Candidates for the next directive set: `army_station {x, z}`, `min_constructors`, `min_converters`; trigger only on 3+
intruders or a building lost.

## Field commander (experiment, 2026-09-19)

A second way to run the `claude -p` session, `bot --commander`: Sonnet taking turns with the game held still,
with lower-level levers than the strategist's directives. The purpose is to learn what good defender management and
unit mix look like (the heuristic home group is "F2 a-move": one blob charging every raider), and to find where the
LLM levers belong. BARb is the reference for economy and army hoarding, not for defence.

- **Division of labour.** The heuristic brain keeps the economy, the opening and every soldier the commander has not
  claimed (home group, waves). The commander owns squads, the unit mix and turret requests. A silent commander costs
  nothing: squads keep their standing posts, everything else is heuristic.
- **Squads, not unit ids.** `squad {name, take: {type: count}, near?, post?, order?, release?}`. `take` draws from the
  unassigned pool (nearest to `near`, else to the post). A **post** `{x, z, radius}` is the defender primitive: stand
  there, engage enemies that come inside the radius, go back. An **order** `{kind: move|fight, x, z}` is a one-off.
- **Unit mix.** `set_production {weights: {unit name: n}}`: factories pick the type furthest below its share. The
  constructor floor stays heuristic.
- **Turrets.** `request_turret {x, z}`: the next free constructor builds one there.
- **Turns (revised the same day after watching a real-time run: most turns were "no change", and the repeated
  identical reports are poor context).** Lockstep: with `WITHIN_REASON_LOCKSTEP` the shim waits for the bot's answer
  to every tick, so the bot pauses the game by holding its reply while the commander thinks, and the game runs at
  full arena speed between turns. The commander chooses when it is woken (`wait`: a maximum quiet time plus events:
  enemies near an extractor, a squad engaged, an extractor lost, awaited soldiers ready); the brain watches for the
  rising edge of each. Reports are terse text, full at the start of a session and changes only afterwards. The
  session is restarted every 40 turns with the commander's notes carried over. Rejected: real-time turns every 5-10 s
  (tried; slow to watch and mostly idle); engine `/pause` (the AI gets no callbacks while paused, so it could not
  unpause itself).
- **Accounts and limits** as for the strategist: `claude2`, overage tripwire, usage snapshot before and after.
- **Reading a run.** Fight ledger exchange ratio in our half, extractors alive at minute 12 against the observed
  heuristic games, and the transcript: where it posts defenders, with what, and what it changes after a loss.

## Terrain (2026-09-19)

The shim sends the ground once, in `Hello`: heights and slopes at the engine's slope-map resolution (16 elmos), and
each unit type's movement class (kind, steepest slope, water depth). The bot builds walking-distance fields over it
(`crates/bot/src/terrain.rs`, Dijkstra on the passable cells of our soldiers' class) from home and from where the enemy
is believed to live, and the brain's geometry goes through them (`brain/routes.rs`): which metal spots are ours, what
"forward of home" means (along the route, not the straight line), which attack targets can be walked to, where a wave
stages. Without terrain data everything falls back to straight lines. Known simplifications: one movement class stands
for the whole army; buildings and wrecks are not obstacles; the commander uses the soldiers' field. The recorder writes
the grid beside the match record and the viewer draws it (`docs/harness/record-format.md`, "Terrain").


## Duels (2026-09-19)

Unit-against-unit tests for the matchup table (`docs/harness/duels.md`): a headless match in which both teams are our AI
and the `duel` binary (in the arena crate) is the bot for both, so one director spawns both armies, orders them and
scores the fight. The protocol gained two commands at the end of `Command`, `GiveUnit` (the AI interface's spawn cheat)
and `SelfDestruct`; normal matches never send them. The arena crate became a library plus two binaries so that `duel`
shares the autohost channel and the engine chores (`crates/arena/src/harness.rs`).
Chose-because: the director needs both sides' state on the same frame (to start both armies together and to know when
a fight is over), which two independent brain sessions in `bot` would have had to share through a side channel.
Rejected: `/give` over the autohost channel (needs `/cheat`, and the runner would not learn unit ids); a Lua gadget
(needs a game fork or mutator).
