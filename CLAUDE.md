# bar_bots

AI players for Beyond All Reason (Recoil engine): a Rust AI shim loaded by the engine, a Rust bot process that decides,
and an arena that runs headless batches against the stock BARb AI.

- `DESIGN.md` — architecture and protocol. Decisions, not findings.
- `docs/README.md` — how knowledge is organised and the observe -> claim -> exploit -> verify -> retire loop. **Follow it:**
  every brain change registers its heuristic in `docs/heuristics.md` and the claim it rests on in `docs/knowledge/`;
  every arena batch gets a line in `docs/experiments.md`.
- `docs/harness/pitfalls.md` — read before running engines or installing the AI.
- `upstream/`, `run/engines/`, `run/data/`, `run/matches/` are git-ignored working data.

Build: `cargo build --release`. Evaluate: `target/release/arena --matches 24 --parallel 8 --speed 50 --label <what-you-test>`.
Never touch `~/.local/state/Beyond All Reason` except through `run/install_to_bar.sh`, and only when the user asks.
