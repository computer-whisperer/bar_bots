# Experiment ledger

Arena batches, oldest first. Opponent BARb, map Quicksilver Remake 1.24, game `byar:test` (test-31357) unless stated.
The arena prints a ready-made row (label, commit with `+` if the tree was dirty, opponent, n, result) at the end of each
batch and stores the same data in the batch's `batch.json`; paste the row here and fill in the last two columns.
W-L-T = wins, losses, timeouts; (n) = aborted by engine crash.

| Batch label | Commit | Opponent | n | W-L-T | Testing | Outcome |
|---|---|---|---|---|---|---|
| baseline-easy | 6362953 | easy | 8 | 1-5-1 (1) | MVP brain | commander dies expanding; energy stalls |
| v2-easy | (uncommitted) | easy | 8 | 2-4-0 (2) | leashed commander, jobs, turrets, unit mix | 2 engine crashes on 2026.09.01 -> moved to 2026.07.04 |
| v2-easy-0704 | (uncommitted) | easy | 12 | 2-5-5 | same, engine 2026.07.04 | timeouts were the arena's wall-clock deadline, fixed |
| v3-easy | (uncommitted) | easy | 12 | 3-9-0 | membership-based waves, outpost turrets, 40 converters | found factory SHIFT bug |
| v4-easy | 5855d95+ | easy | 12 | 4-8-0 | factory order fix, fighters first | losses now long games; found energy/converter loop |
| v5-probe | (uncommitted) | easy | 2 | 1-1-0 | energy by storage; attacker diagnostics | expansion works (18 extractors by min 17); energy cap starves labs |
| v5-easy | ce87634 | easy | 12 | 7-4-1 | cap removed, labs gated on energy | |
| v5-easy-confirm | same | easy | 24 | 14-10-0 | confirmation | Cortex 10-2, Armada 4-8; SE 9-3, NW 5-7 |
| v5-medium | same | medium | 12 | 0-12-0 | reference against next tier | all lost at minute 16-20 |
| eff-base / eff-cfg | be2fbee | easy | 8+8 | (throughput only) | research agent's config A/B at speed 80 | 12.1 -> 5.8 CPU-s per game-min |
| eff-applied | (this commit) | easy | 12 | 1-5-6 | all efficiency changes, speed 200, 12 parallel, 40-min game-time limit | outcomes distorted by requested speed; see arena.md OPEN |
| eff-speed50 | (this commit) | easy | 12 | 8-3-1 | same settings at speed 50, 8 parallel | normal play at ~40% of the old CPU cost |
| eff-speed50-par12 | (this commit) | easy | 12 | 5-6-1 | speed 50, 12 parallel | within noise of normal; no wall-time gain over 8 parallel |
