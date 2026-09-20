# Match record — format version 1

One file per AI per match, `record-<ai_id>.jsonl`, written by the bot process (`crates/bot/src/recorder.rs`) into
`$WITHIN_REASON_LOG_DIR` when `WITHIN_REASON_RECORD=1`; the arena always sets it. It holds what the bot saw and what it
decided, so a finished (or killed) match can be replayed in `viewer/` with `run/view_match.py <match dir>`.

It is not the engine's replay. The engine's own `demos/*.sdfz` re-simulates the game inside the engine, cannot be read by
a web page and knows nothing of our decisions; the record names it (`result.replay`) so the real thing can be opened too.

## Rules of the format
- JSON Lines, UTF-8, one record per line, `t` names the record type, `f` is the game frame (30 per second).
- Append-only, written once per tick (every 15 frames) in a single write. A killed match is valid up to its last tick;
  readers must skip a last line that does not parse and must not require a `result` line.
- Records are in frame order, except that a `rules` decision record covers the sample interval that ends at its frame.
- The first line is the header. A second header later in the file means the bot process was restarted mid-match
  (`first_tick_frame` says when); the tables are the same, the brain's memory is not.
- Readers skip record types and fields they do not know. Adding either does not bump `version`; changing the meaning or
  layout of an existing one does.
- Positions are whole elmos `x, z` (west to east, north to south). Unit types are indices into the header's `unit_defs`,
  `-1` for a radar contact never identified.

## Records
| `t` | When | Fields |
|---|---|---|
| `header` | first tick | `format` ("within-reason-record"), `version`, `ai_id`, `team`, `ally_team`, `side` (first three letters of our first unit's name: `arm`, `cor`), `mode` (`heuristic`, `strategist`, `commander`), `start_frame`, `first_tick_frame`, `frames_per_second`, `sample_frames`, `wall_start` (unix s), `map` {`name`, `width`, `height`, `wind_min`, `wind_max`}, `grid` {`columns`, `rows`} (the A-H / 1-8 cells of `World::grid`), `metal_spots` [[x, z, amount]] (amount as the engine reports the spot; records before 2026-09-20 lack it), `unit_defs` [{`id` (engine's), `name`, `class`, `metal`, `energy`, `speed`, `weapons`; and, in records from 2026-09-20 on, the numbers the build-order simulator reads: `build_time`, `build_speed`, `build_distance`, `builds` [engine ids], `extracts_metal`, `metal_make`, `energy_make`, `energy_upkeep` (negative for solar collectors), `wind_cap`, `metal_storage`, `energy_storage`, `radar_range` (records from 2026-09-20 evening on), `converter` [energy/s taken, metal per energy] or null, `move` [kind as in `move_classes`, max_slope, depth] or null}], `siblings` {`bot_log`, `engine_log`, `replay` (glob), `decision_logs` [file names; the LLM transcript when a session runs]} |
| `s` | every `sample_frames` (30) | state sample. `m`, `e`: [current, income, usage, storage]. `own`: [[id, def, x, z, health %, flags]], flags 1 being built, 2 idle, 4 attacker (committed wave), 8 in a commander's squad. `en`: enemies in sight or radar [[id, def, x, z, health]] (absolute health; the protocol has no maximum for enemies). `al`: allied units (other seats of ours, other players on our side) [[id, def, x, z, team, being built]]; each seat of ours writes its own record, and any one of them shows the whole side through this. `dmg`: [[own unit id, damage taken since the last sample]]. `ms`: slowest `Brain::decide` since the last sample, wall milliseconds (in lockstep mode this includes the commander's thinking time) |
| `ev` | as they happen | engine event. `k`: `created` (+`by` builder id), `finished`, `destroyed` (+`by` attacker id, `by_d` its type), `move_failed`, `enemy_seen`, `enemy_lost`, `enemy_destroyed`, `no_site` (+`what` def), `rejected` (+`code`). `u` unit id, `d` its type, `x`, `z` its last known place (0, 0 when never seen). Not recorded singly: `UnitIdle` (it is a flag) and `UnitDamaged` (summed into `dmg`) |
| `cmd` | ticks with commands | `c`: [["build", unit, def, x, z] or ["build", unit, def] for a factory, ["move", unit, x, z], ["fight", unit, x, z], ["stop", unit], ["repeat", unit, 0 or 1], ["guard", unit, target unit], ["repair", unit, target unit], ["reclaim", unit, x, z, radius] (an area; records before 2026-09-20 only), ["reclaim_feature", unit, feature], ["resurrect", unit, feature]; from the duel harness only: ["give", def, x, z], ["selfdestruct", unit]] |
| `intent` | when it changes | where the army stands in the brain's mind: `home`, `enemy_start` (presumed), `station`, `target`, `staging`; each [x, z] or null |
| `d` | as they happen | decision record, see below |
| `result` | appended by the arena after the match | `result` (the arena's `MatchResult`: outcome, side, corner, game minutes, wall seconds), `opponent`, `replay` (path relative to the match dir, or null) |

`class` is derived from the definition's numbers only, so it holds for every faction and for enemy types:
`commander`, `extractor`, `factory`, `turret`, `building`, `builder`, `army`, `other`.

## Decision records
One shape for every decision source, so a new layer slots in without a format change:

`{"t":"d", "f":<frame>, "source":<who>, "kind":<what>, "inputs":<what it was decided on>, "outputs":<what was decided>, "latency_ms":<optional>}`

| `source` | `kind` | `inputs` / `outputs` |
|---|---|---|
| `heuristic` | `rules` | outputs: {heuristic ID (docs/heuristics.md): firings} over the sample interval ending at `f`. Same counts as the `rules:` line of `bot.log`, per second instead of per minute |
| `heuristic` | `wave` | inputs {`home_group`}; outputs {`wave`, `target` {grid, x, z}, `first_stop`} |
| `heuristic` | `contact` | H-ARMY-CONTACT, when the number answering a party changes. inputs {`party` (units), `metal`, `at` [x, z], `forced` (at the lab or the commander), `within_reach` (soldiers asked about), `burned_if_nobody` (metal, the simulator's)}; outputs {`sent`, `worth` (metal gained over sending nobody), `caught` (share of runs)} |
| `heuristic` | `scout` | H-SCOUT-ROUTE, a raider sent on a route. inputs {`unit`, `route` ([x, z] per spot)}; outputs {`squares` (grid names)} |
| `heuristic` | `recall` | inputs {`intruders`}; outputs {`attackers_called_home`} |
| `heuristic` | `assault` | inputs {`gathered`, `attackers`} |
| `heuristic` | `event` | outputs: the text the brain notes for the strategist ("lost armmex at C3", trigger texts) |
| `llm:strategist`, `llm:commander` | `turn` | not in the record file: the viewer builds these from the sibling `strategist-<ai_id>.jsonl` (inputs {`wake`, `prompt`}; outputs {`calls` [{tool, arguments, result}], `said`, `thinking`}; `latency_ms` from `turn_end`) |
| `jev` (planned) | per decision head | inputs: a summary of the features; outputs: the typed decision; `latency_ms` |

The brain's side is `crates/bot/src/brain/journal.rs`: `self.journal.note(frame, kind, inputs, outputs)` at the decision,
`self.fire(...)` for rule counts. `main` drains the journal every tick whether or not a record is written. A Jev layer
should write through the same journal with its own `source` (`Note::source`; `Journal::note` is the heuristic brain's).

## Size (measured 2026-09-19, Quicksilver, speed 50)
| Match | Game minutes | Units at the end (ours + seen) | File | Per game minute |
|---|---|---|---|---|
| viewer-test, easy | 14.2 | 88 | 1.5 MB | 108 KB |
| viewer-test-40, medium (loss) | 17.9 | 59 | 1.4 MB | 80 KB |
| viewer-test-40b, easy (loss) | 26.6 | 152 | 5.2 MB | 35 KB in minutes 0-5, 118 in 5-10, ~290 from minute 15 |

Samples are about 85 % of the file and grow with the unit count (about 20 bytes per unit per game second); events about
10 %, half of them `move_failed`. At the late-game rate a 40-minute match is 10-12 MB; gzip takes it to about a tenth.
The header is 60 KB (the unit table). Wall-clock cost was not measurable: 14 game minutes in 49 s with recording on.

## Ground truth about the opponent
The record holds the bot's fair view only. With `WITHIN_REASON_OBSERVE=1` the shim writes the once-a-minute census of
both sides into `engine.log` (see `arena.md`); the viewer reads those lines next to the record and draws them as faint
markers and as the "theirs" series. Census positions are the mean of all units of a type, so a marker for scattered
extractors sits between them.

## Viewer
`run/view_match.py run/matches/<batch>/<NN>` serves `viewer/` and the match directory and opens the browser. It listens on `::` (every interface, IPv6 and IPv4) so another machine on the network can open it; `--bind 127.0.0.1` keeps it to this machine.
Plain HTML, CSS and JS, no build step, nothing fetched from outside. `viewer/record.js` is the parser and model (also
runs under node), `viewer/app.js` the page. URL parameters: `t=<seconds>` start position, `record=<file>`, `bg=<image URL>`.
Terrain: an image at `viewer/maps/<map name>.png` (whole map, north up) is drawn under the map when present; none ship.
Live: a record with no `result` line is a match still being played. The page then asks the server every 3 s for each
file's new bytes (`<file>?from=<byte offset>`, answered by `run/view_match.py` with an `X-From` header; any other server
sends the whole file and the page copes), parses up to the last complete line, and with "follow live" ticked stays on the
newest sample. Scrubbing, stepping or playing unticks it; ticking it again jumps to the newest sample. Opened details and
the scroll position of the decision list survive each refresh. Polling stops when the result line arrives.
Tests: `node viewer/test/smoke.js <match dir>` (model, truncated file) and `node viewer/test/browser.js <url>` (the real
page in headless Chromium: load, follow a live match if it is one, scrub, play, hover, toggles; fails on any page error).

## Terrain

The header's `terrain` object names a sibling binary file (`terrain-<ai_id>.bin`) and how to read it: `width` x `height`
cells of `cell` elmos (the engine's slope-map resolution, 16), row-major from the north-west; first every height as a
little-endian i16 (elmos; water level is 0, negative is under water), then every slope as a u8 (the engine's slope
value, 1 - the ground normal's y, times 255). `move_classes` lists the distinct movement classes among the unit types
(`kind` tank/bot/hover/ship, `max_slope` in the same slope units, `depth`: deepest water waded, or for ships the
shallowest floated in; `units`: how many unit types use it), so a reader can work out where each kind cannot go. The
viewer renders relief with water and the "bots cannot go" / "vehicles cannot go" layers from it.

## Opponent ground truth

`truth-<ai_id>.jsonl`, written by the shim (not the bot) when the match runs with `WITHIN_REASON_OBSERVE=1`: one line
every two seconds, `{"f": frame, "enemy": [[id, name, x, z, health %, being built 0/1], ...]}`, every enemy unit
wherever it is. The viewer draws it as the faint "opponent (truth)" layer under what our units could see, and uses it
for the opponent's curves; `run/analyze_match.py` derives the opponent's deaths from units leaving the list.

