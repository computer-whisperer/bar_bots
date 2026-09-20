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
| v5-easy-mirror | 05be9ec | easy, mirror | 24 | 12-6-6 | does the faction split follow our faction or BARb's? | BARb's: vs Armada BARb 7-2-3, vs Cortex BARb 5-4-3; timeouts are the new 40-min cap |
| strategist-refactor-regression | 264423f+ | easy | 12 | 0-1-11 | INVALID: bot never started (socket path too long) | harness bug, fixed; see pitfalls.md |
| refactor-regression | 264423f+ | easy | 12 | 3-4-5 | economy rule refactor for directives, no strategist | no gross regression |
| opus-first | 6b115b5 | easy | 1 | 0-0-1 | first Opus strategist game, 2x, 20-min cap | mechanics work (33 turns, median 5.3 s, $1.27); economy frozen by raids; see transcript digest |
| v6-station-armada | 76a0e83 | easy, as Armada | 24 | 4-14-6 | forward station + outpost defence + constructor floor | looked harmful, but see the next rows |
| v5-baseline-armada (+rerun) | old binary | easy, as Armada | 24+24 | 13-6-5, 10-9-5 | same binary twice | 24-match batches swing by several wins |
| v6-minus-* (3 batches), v6-outpost-defence-only, v6-control-all-off | 71bbfd3..bc4a161 | easy, as Armada | 24 each | 8-7-9, 3-15-6, 7-12-5, 5-12-7, 6-10-8 | one-rule ablations and an all-off control | control is behaviourally identical to the baseline yet scored 6-10-8: the ablations were noise. Stopped measuring rules; went looking for gross mistakes |
| dropped-orders / dropped-orders-2 | 5052826+ / 85b593d | easy, as Armada | 4+4, 20 min | (diagnostic) | log orders whose builder is idle again within two ticks | 560-890 dropped orders per match, mostly the commander told to build advanced solars; after the guard 7-38, minute-20 income +38-44 |
| v7-no-dropped-orders | 85b593d | easy | 24 | 7-8-9 | economy fix alone | more games reach 40 min: the army was not attacking the enemy base |
| v8-targets-wind | bb5e674 | easy | 24 | 12-6-6 | waves target remembered enemy buildings; wind generators; lab cap 8 | wins in 11-22 min; in losses H-ARMY-TARGET fired ~1,390 times per match (orders not executing) |
| move-failures 1-4 | bb5e674+..6416ca3 | easy, as Armada | 6 each, 25 min | (diagnostic) | log UnitMoveFailed | 1,300-6,800 move failures per match: unreachable stations (units piled at the factory), unreachable build sites (constructors), unreachable attack targets (whole army) |
| v9-reachability | 6416ca3 | easy | 24 | 10-5-9 | remember unreachable sites, stations and targets; enemy base from seen buildings | SE start 8-0-4, NW start 2-5-5; win minutes 13-38 |
| nosite-diag | b683245+ | easy | 8 | 6-1-1 | (diagnostic) what stands on a refused build site | SE: our own wind generators on the home spot (5640,5352) every game; a dead extractor's wreck blocks the site search though building is possible; a commander boxed in by its own converters (234 refusals in one game) |
| v10-sites | 0981e2f | easy | 16 | 8-5-3 | extractors exactly on spots, base buildings off spots | SE 7-0-1, NW 1-5-2; no "no site" left; NW attackers parked at home for 7-15 minutes in 6 of 8 games |
| nw-stuck-diag | 0981e2f+ | easy, NW only | 8 | 2-6-0 | (diagnostic) where soldiers' moves fail | ~17,000 failures, all within ~500 of our start: the base is a maze |
| nw-layout | c90d555 | easy, NW only | 8 | 3-4-1 | labs in a yard, generators behind, wider gaps | soldier move failures 149; fight ledgers show the enemy fielding OUR faction's units in every loss (resurrected wrecks) and a timeout with 739 units lost in their half for 104 kills |
| v11-layout | c90d555 | easy | 24 | 13-7-4 | same, both corners | SE 10-0-2, NW 3-7-2: the maze was not the corner gap. In SE games we lose nothing in our own half; NW losses are commander snipes on a healthy economy |
| v12-stage | 52ae15a | easy | 24 | 17-5-2 | H-ARMY-STAGE: waves gather 1500 short of the target | SE 11-0-1, NW 6-5-1; timeouts down from 4-9 to 2. One batch: read as "not worse", with a plausible mechanism |
| v13-recall | 78ab2dc | easy | 24 | 23-1-0 | H-ARMY-RECALL: six enemies at the base call the attackers home | SE 12-0-0, NW 11-1-0, but the rule fired in only 4 of 24 games (3 of them SE), so it cannot explain the jump from v12; same-binary swings this large have been seen before. Pool v12+v13 for the staged bot: 40-6-2, NW 17-6-1 |

| v13-medium | 6dafd19 | medium | 24 | 11-13-0 | same bot against medium (was 0-12 at v5) | SE 10-2-0, NW 1-11-0: the corner gap at its starkest. From NW our extractors at minute 10 are 4-9 against 8-14 from SE, and losses come at 17-24 min |
| nw-medium-mexfirst / -2 | 322d10b+ (not kept) | medium, NW only | 12+12 | 0-12-0 / 0-12-0 | extractors before turrets and converters, constructors first from the lab (first try also a longer commander leash) | extractor curve unchanged (2.2 at min 3, ~5 at min 7-12); first try banked 700+ metal while the commander wandered. Reverted: opening priority is not what holds NW back |
| v13-medium-swapped | 322d10b+ | medium | 16 | 6-10-0 | `--swap-corners`: team 0 starts SE, team 1 NW | NW (as team 1) 0-8, SE (as team 0) 6-2: the gap follows the corner, not the team slot |

| nw-medium-raids | 24edcd5+ | medium, NW only | 12 | 0-12-0 | H-ARMY-RAID: squads of 3 raiders hunt enemy extractors | 6.4 squads a game; enemy extractors killed 3.1 -> 4.9, ours lost 22.2 -> 17.4 (12 games each, within noise). Squads die to the turret medium puts by its extractors (corak lost to armllt) and to flash/leveler groups; meanwhile single fleas still kill our extractors unopposed |
| observe-1 | 370ecc7+ | medium, NW | 2 | 0-2 | first games with the opponent census (`WITHIN_REASON_OBSERVE`, `run/compare_census.py`) | BARb: 1 lab + 1 nano turret all game, army grows every minute (31 by minute 10) and is not spent; ours left in eights from minute 5 and stayed at 2-13 |
| observe-2-hold20 | 370ecc7+ | medium, NW | 4 | 1-3 | first wave at 20 (was 8), growth 5 | armies level to minute 12-15 (19-27 each); then our extractors fall 9 -> 3 while BARb's go 8 -> 20 with 10-20 constructors against our 2-4. Also found: raid squads stuck on a ledge were counted as station failures and the station flapped (fixed) |
| observe-3-constructors | 370ecc7+ | medium, NW | 4 | 0-4 | constructors wanted 3 + extractors/2, cap 10 | alive constructors still 2-5: they die as fast. By minute 10 we BUILD as many soldiers as BARb (26-45 vs 21-66) but lose 10-25 for 2-14 kills; 6-10 of the early losses were raid squads in the enemy half |
| observe-4-noraids | 370ecc7+ | medium, NW | 4 | 0-4 | raid squads removed | armies level to minute 9 (13-35 vs 18-26); the first wave of 20 leaves about then and dies, and BARb counter-attacks |
| observe-5-hold40 | 370ecc7+ | medium, NW | 4 | 1-3 | first wave at 40 (not kept) | holding the army at home does not save extractors: 37 soldiers at home at minute 12 while extractors fell 13 -> 6 -> 2, then 21 soldiers lost in one minute to ~14 vehicles (flash, janus, stumpy) |
| v14-easy-sanity | 370ecc7+ | easy | 12 | 9-3-0 | first wave 20, constructor cap 10, no raids | not broken against easy |

| commander-smoke-2 / commander-1 | 76fa362 | medium, NW, Sonnet field commander | 1 (10 min) + 1 | - / 0-1 | lockstep turns, self-set wake conditions, squads with posts, unit mix, turret requests | works end to end: 72 turns, median 10.7 s, 17.5 min wall for a 16.8 min game, +2 points of one 5-hour window. Lost: BARb out-massed us 37 to 18 by minute 9 with 9 extractors to our 5. Sonnet's own diagnosis in docs/transcripts/2026-09-19-sonnet-commander-1.md: raids hit every expansion at once from minute 8, a posted squad chased a single raider out of its radius and the cluster behind it fell, the bot keeps building extractors 3000 from home that cannot be held. Wishes: an order for the commander unit; a lever to stop remote expansion |
| viewer-test, viewer-test-40, viewer-test-40b, viewer-final | 76fa362+ (match-viewer branch) | easy, medium, easy, easy; NW | 1 each | 0-0-1, 0-1, 0-1, 0-0-1 | harness only: match recorder and viewer (14, 40, 40 and 8 minute caps, census on); brain unchanged | records of 1.5 / 1.4 / 5.2 MB, see harness/record-format.md; not a brain result |
