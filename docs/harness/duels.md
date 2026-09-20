# Duels — unit against unit

`target/release/duel` spawns N units of one type against M of another on flat ground, sends them at each other and
records who is left. It exists to fill the matchup table the brain and the LLM commander choose a unit mix from
(`docs/knowledge/units.md`, K-units-duel-*).

```
duel (--units a,b,c | --ours a,b --theirs c,d | --pairs a:b,c:d)
     [--reps 4] [--budget 1200 | --count N] [--parallel 2] [--sites 3] [--duels-per-match 45] [--time-limit 240]
     [--sweep-waves 3] [--speed 50] [--map NAME] [--label TEXT] [--base-port 9500]
duel --report DIR [duels.csv ...]      rebuild the tables in DIR (from its own duels.csv, or merge the files named)
```
`--units` runs every pair from the list, each unit against itself included; `--ours/--theirs` the cross product;
`--pairs` exactly those. Unit names are the game's internal ones (`armpw`). Two buildings are never paired.

Output, in `run/matches/<stamp>-duel-<label>/`: `duels.csv` (one row per duel, appended as they finish), `pairs.csv`
(per ordered pairing: counts, wins / losses / draws, mean margin, mean seconds), `matrix.csv` and `matrix.md` (row unit
against column unit, mean margin x 100), `batch.json`, and one directory per engine start with `engine.log` and the replay.

## How it works
- **Both teams are our AI**; the `duel` process is the bot for both (`crates/arena/src/bin/duel/`): the two shims connect
  to its socket, and one director sees both sides whole. The heuristic brain is not involved. The shim runs in lockstep
  (`WITHIN_REASON_LOCKSTEP`), so orders land on the frame they were decided for at any game speed; results at speed 50
  and 200 agree (below).
- **Spawning** is the AI interface's cheat `COMMAND_CHEATS_GIVE_ME_NEW_UNIT` (`Command::GiveUnit`). The engine honours
  it without `/cheat` because the game is hosted locally with one player (`CAICheats::OnlyPassiveCheats`,
  `rts/ExternalAI/AICheats.cpp:34`); it would silently do nothing in a multiplayer game. The engine delivers the new
  unit's events from inside that call, so the shim issues it after letting go of its instance table
  (`engine::Spawner`) — issued under the lock, the first spawn deadlocks the engine.
- **Sites** are chosen from `Hello.terrain`: 1440 x 640 elmo rectangles, every cell dry and within the least agile
  listed unit's slope limit, at most 16 elmos of relief, 900 from each other and from both commanders
  (`sites.rs`). Quicksilver gives two. Each site runs its own sequence of duels, so a match fights two at a time.
- **A duel.** The armies appear 1100 elmos apart (beyond every tier-1 weapon; artillery reaches 710) in ranks of eight,
  56 elmos apart, further ranks behind. The axis runs west-east because spawned units always face south: both sides
  start side-on. After one second both get a Fight order (attack-move) at the enemy's centre; idle units are sent again
  at wherever the enemy is now. Buildings just stand; the mobile side comes to them.
- **Sizes.** By default equal metal: about `--budget` a side, with the whole-unit counts whose totals differ least
  (26 Pawns against 10 Thugs). `--count N` gives N against N instead. Energy cost and build time are ignored.
- **Order of play.** Repetition `r` of a pairing puts the first unit at the west end when `r` is even and gives it
  team 0 when `r / 2` is even, so four repetitions cover every combination. The plan is shuffled with a fixed seed.
- **Scoring.** Decided when one army has nobody left (`wiped`), after `--time-limit` game seconds (`timeout`), or after
  60 s without damage to anyone (`stalemate`: anti-air against anti-air). `value_left` is the surviving share of an
  army's metal, each survivor weighted by its health. **Margin** = own `value_left` minus the enemy's: +1 is a flawless
  win, -1 a wipe without scratching them. `damage_taken` sums the engine's damage events (hit points, overkill
  included). `contact_seconds` is the time to the first damage.
- **Clearing.** Survivors self-destruct; then crawling bombs (`corroach`) are spawned on a 200-elmo grid over where
  units died and set off, `--sweep-waves` times, to destroy the wrecks. Without this, results drift within a match:
  80 duels on one site took 27 s -> 41 s each as wrecks piled up, and Rocketeer-against-Incisor read -0.10 instead of
  -0.36 (wrecks stop rockets). With three waves both stay flat over 80 duels (batches `wrecks`, `wrecks-swept`).

## Checks made (2026-09-19)
| Check | Batch | Result |
|---|---|---|
| Position and team slot | `bias`: Pawn, Blitz, Thug each against itself, 16 times | west won 25, east 22, 1 draw; team 0 won 21, team 1 27: no bias visible at this n |
| Wrecks | `wrecks` / `wrecks-swept` | drift without sweeping, none with (above) |
| Site and speed | `speed200`: 3 pairings x 30 at speed 200 on two sites | margins per site -0.46 / -0.43, -0.35 / -0.37, -0.21 / -0.22; same as speed 50 (-0.43, -0.36) |

## Cost
An engine start is ~30 s; after that two sites together finish about two duels a second of wall time at `--speed 200`
(a duel is 20-60 game seconds plus ~10 of clearing). One engine uses ~3 GB and about two cores.

## What a duel is not
No micro (no kiting, no retreat, no focus fire beyond the engine's own targeting), no terrain, no mixed armies, no
support (radar, repair, turrets behind the line), one army size. Both sides charge: a unit that would normally hold at
its range and be approached is tested as an attacker too. See the caveats in K-units-duel-caveats.
