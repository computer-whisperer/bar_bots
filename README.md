# Within Reason

*An AI opponent for Beyond All Reason that plays within reason. Lobby name: `WReason`. (Formerly `bar_bots`.)*

AI players for [Beyond All Reason](https://www.beyondallreason.info/), the open-source RTS on the Recoil engine.
The aim is a better sparring partner than the stock BARb AI: heuristic bots first, with slower model-driven strategy
layers on top later. This is an early-stage hobby project and is not affiliated with the BAR team.

## How it works

```
 Recoil engine (spring / spring-headless)                 bot process
 ┌──────────────────────────────────────────┐   Unix     ┌─────────────────────────────┐
 │ libSkirmishAI.so  (crates/ai-shim)       │  socket    │ crates/bot                  │
 │  relays events + state snapshots  ───────┼──────────► │  heuristic brain            │
 │  applies the commands it gets back ◄─────┼─────────── │  one session per AI         │
 └──────────────────────────────────────────┘            └─────────────────────────────┘
```

The engine loads a thin Rust library through its C skirmish-AI interface. The library holds no strategy: twice a second it
sends the bot process what happened and what the world looks like, and applies the orders that come back. The bot can be
written, restarted and debugged like any ordinary program, and slow decision-makers can never stall the game.
[`DESIGN.md`](DESIGN.md) has the details.

| Crate | What it is |
|---|---|
| `recoil-ai-sys` | bindgen bindings over the engine's AI interface headers (vendored) |
| `bot-protocol` | messages and framing between shim and bot |
| `ai-shim` | the library the engine loads |
| `bot` | the bot process and its heuristic brain |
| `arena` | batch evaluation: parallel headless matches against BARb, win rates, logs and replays per match |

## Status

Plays full 1v1 games as Armada or Cortex with tier-1 bots. Against BARb on Quicksilver Remake: about 58% wins versus the
`easy` profile (14-10 over 24 games), 0-12 versus `medium`. Selectable as a local AI in the BAR lobby.
[`docs/experiments.md`](docs/experiments.md) is the running ledger.

## Knowledge base

Most of the work is learning the game, so the documentation is structured as a loop: observe a batch, write down a checkable
claim, build a heuristic on it, verify with another batch, retire it when it stops holding. Claims live in
[`docs/knowledge/`](docs/knowledge/README.md) with their evidence and status, the brain's rules are registered in
[`docs/heuristics.md`](docs/heuristics.md), and [`docs/README.md`](docs/README.md) describes the process. The analysis of how
BARb plays, from its source and configs, is in [`docs/knowledge/opponents-barb.md`](docs/knowledge/opponents-barb.md).

## Running it

Requirements: Linux, a Rust toolchain, libclang (for bindgen), `7z`, and about 3 GB of disk for the engine and game data.

```sh
cargo build --release

# engine (use the version BAR ships; see docs/harness/engine.md) and game data
mkdir -p run/engines && cd run
gh release download 2026.07.04 -R beyond-all-reason/RecoilEngine -p 'recoil_*_amd64-linux.7z'
7z x -oengines/2026.07.04 recoil_*_amd64-linux.7z && ln -s engines/2026.07.04 engine
export PRD_RAPID_USE_STREAMER=false PRD_RAPID_REPO_MASTER=https://repos-cdn.beyondallreason.dev/repos.gz \
       PRD_HTTP_SEARCH_URL=https://files-cdn.beyondallreason.dev/find
./engine/pr-downloader --filesystem-writepath "$PWD/data" --download-game byar:test
./engine/pr-downloader --filesystem-writepath "$PWD/data" --download-map "Quicksilver Remake 1.24"
cd ..

# 24 headless matches against BARb easy, 8 at a time (each engine needs ~4 GB of memory)
target/release/arena --matches 24 --parallel 8 --speed 50 --profile easy --label my-first-batch
```

To play against it in the BAR client: `run/install_to_bar.sh` copies the AI into a local BAR install, then start
`target/release/bot` and add `WReason` to a skirmish. The lobby only lists unknown AIs with "Simplified AI list" turned off
(Developer settings tab; see [`docs/harness/lobby.md`](docs/harness/lobby.md)). Please ask the people you are playing with
before bringing an experimental AI into a shared lobby.

## Licence

GPL-2.0-or-later, the same as the engine whose interface headers are vendored in `crates/recoil-ai-sys/vendor/`.
See [`LICENSE`](LICENSE).
