# A tempo model: estimating the opponent's economy and army from what we can see (2026-09-20)

Offline study, no arena batch. Tool: `run/tempo_model.py` (extract + fit + evaluate). Data:
`data/tempo-2026-09-20/samples.csv.gz` (26,532 rows, checked in) and `data/tempo-2026-09-20/tempo-params.json` (the
fitted numbers). `run/tempo_model.py` alone re-runs the fit from the checked-in dataset in about a minute;
`--extract` re-reads the 511 match directories (about 4 GB of JSON, 30 s on 24 cores) and needs `run/matches`, which
is git-ignored working data.

## The question

The wave gate (H-ARMY-WAVE-GATE) weighs our wave against `known_enemy_force` at the target plus `enemy_army_seen`,
which is the biggest force we saw in one look in the last two minutes. K-army-verdicts-v18 already recorded what that
costs: "1500 known against a real 3205". The commander's report has the same hole. So: given only what the bot may
legitimately know at game second *t* — enemies seen and when, buildings remembered, what we killed, what we lost, our
own state — how well can we say what the opponent has **right now**, and how wide does the honest error bar have to be?

The output feeds a gate and a sentence in a report, so a calibrated interval matters more than a sharp point estimate.

## The data

Every match run with `WITHIN_REASON_OBSERVE=1` writes `truth-<ai>.jsonl` beside the record: every enemy unit, with
name, position, health and a being-built flag, every two game seconds. 511 matches in `run/matches` have both files.

| set | games | rows | used for |
|---|---|---|---|
| Quicksilver Remake 1.24, heuristic brain | 430 in 17 batches | 22,353 | fitting and cross-validation |
| four other maps (v33-*: Isidis, Comet Catcher, Feast of Hades, Mithril Mountain) | 64 in 4 batches | 3,421 | held out entirely |
| Sonnet/Opus commander games (commander-2 … commander-11) | 17 | 758 | held out entirely |

Quicksilver batches: t2-first-look, truth-check, v17-truth-medium, v18-verdict-fixes, v19-mix-odds-repair,
v20-opening-scout-reach, v21-early-expand-ab10, v22-early-expand, v23-spend-ab, v24-enemy-base-ab, v25-harass-ab,
v26-radar-startguess-ab, v29-frontier-ab, v30-march-ab, v31-tier2-ab, v32-march-ab-48, v34-expand-first-ab. Every
opponent is BARb medium; sides and corners are mixed. Median game 26 minutes; 163 wins, 177 losses, 90 timeouts.

One row per game per 30 game seconds from second 60 to the end of the match. **Held out by batch**, never by sample:
rows 30 s apart in one game are nearly the same data point, and a batch is one bot version, so a game-level split
would still leak the version. Five folds, batches dealt round-robin.

## What is being predicted

From the truth file at that instant, counting only finished units (`class` comes from the record header's `unit_defs`,
derived from the definition's numbers, so it holds for either faction):

- `y_army_m` — metal value of the opponent's mobile armed units (`class` "army": mobile, armed, no build power — the
  same filter `known_enemy_force` applies to what it can see).
- `y_mex` — finished extractors, tier 1 and advanced together.
- `y_def_m` — metal value of finished turrets.
- `y_t2lab` — is a finished advanced lab standing? (a factory costing ≥ 1500 metal; tier-1 labs are 470-570, advanced
  2600). Judged from the **building**, not from tier-2 units seen: BARb's resurrection bots raise our dead, so our own
  tier-2 units turn up in its army without it ever owning a lab (K-army-dead-waves-are-resurrected).
- `y_income` — a **proxy**, see below.

### The income proxy, and why it is a proxy

The truth file lists units, not income. The proxy is
`rate_t1 x tier-1 extractors + 4 x rate_t1 x advanced extractors + 1.0 x converters + 10.34 x advanced converters`.
The 4x is from the unit files (`extractsmetal` 0.004 against 0.001, K-t2-moho-first); the converter rates are
`energyconv_capacity x energyconv_efficiency` from `armmakr.lua` / `armmmkr.lua`, and they are **ceilings** — an idle
converter makes nothing. `rate_t1` is fitted per map on **our own** income against **our own** extractor count, over
samples from minute 3 on where we had no converter and no advanced extractor running:

| map | metal per extractor | same, minutes 3-6 only | samples | R² | median residual |
|---|---|---|---|---|---|
| Quicksilver Remake 1.24 | 2.41 | 2.40 | 1833 | 0.58 | 0.4 |
| Comet Catcher Remake 1.8 | 2.64 | 2.66 | 100 | 0.89 | 0.6 |
| Isidis crack 1.1 | 2.34 | 2.28 | 628 | 0.19 | 1.0 |
| Mithril Mountain v2.0.1 | 2.30 | 2.19 | 150 | 0.55 | 0.6 |
| Feast of Hades 1.0.1 | 2.11 | 2.12 | 141 | 0.30 | 0.3 |

2.41 on Quicksilver agrees with the earlier inference of "about +2 M/s a spot" in K-t2-moho-first. The engine's income
figure includes reclaim, which is ours and not theirs, so the rate is an over-estimate of a bare extractor; the
minutes 3-6 column, where there is far less wreckage about, is within 1 % of the full fit, so that contamination is
small. Read every `y_income` number as extractor-equivalents per second, not as the engine's number.

## What the bot actually has in hand

Before any model: how much of the opponent do we hold at all? Means over rows, Quicksilver, heuristic games.
"Seen alive" is every enemy unit we ever identified, minus the ones we watched die. "Brain now" is
`enemy_army_seen` reconstructed: the largest army metal in one look in the last 120 s (`ENEMY_ARMY_MEMORY_FRAMES`).

| minute | army real | seen alive | brain now | extractors real | extractors seen | rows with no enemy extractor in hand | turret metal real | turret metal seen |
|---|---|---|---|---|---|---|---|---|
| 1-5 | 249 | 25 | 30 | 2.9 | 0.0 | 100 % | 123 | 0 |
| 5-8 | 1127 | 149 | 172 | 5.0 | 0.0 | 99 % | 366 | 1 |
| 8-11 | 2031 | 788 | 826 | 7.1 | 0.1 | 89 % | 670 | 17 |
| 11-15 | 3160 | 1784 | 1465 | 10.0 | 0.5 | 65 % | 1050 | 92 |
| 15-20 | 4249 | 2795 | 2060 | 13.2 | 0.9 | 49 % | 1851 | 204 |
| 20-40 | 4853 | 3435 | 2287 | 14.6 | 1.3 | 39 % | 3805 | 467 |

Two facts fall out of this table and they set everything that follows.

1. **We never see their economy.** Over the whole dataset we hold, on average, 1.3 of their 14.6 extractors at minute
   20 and none at all before minute 8; in 39 % of late-game rows not one enemy extractor has ever been identified.
   Direct observation cannot estimate their economy. Anything we say about it is inference from the clock and from
   what we do *not* hold.
2. **What the brain uses is the worst of the four estimators.** `enemy_army_seen` says 2287 when the truth is 4853 —
   it understates by a factor of two late, and it is worse than simply summing everything we have seen and not watched
   die (3435), because it throws away everything older than two minutes.

## Models, simple to less simple

**M0, time alone.** The mean of the target over training games at that game minute. This is BARb medium's typical
curve, the sharpened version of K-barb-medium-army-curve. Median and 10th-90th percentile over games still alive:

| minute | games | army metal | extractors | income proxy | turret metal | has advanced lab |
|---|---|---|---|---|---|---|
| 4 | 430 | 584 [120-853] | 4 [2-6] | 10 [5-14] | 180 [170-277] | 0 % |
| 8 | 430 | 1838 [780-2647] | 7 [3-9] | 17 [8-23] | 530 [340-735] | 0 % |
| 12 | 407 | 3112 [1612-4436] | 9 [4-15] | 24 [10-39] | 950 [535-1445] | 0 % |
| 16 | 375 | 4121 [1770-6503] | 12 [4-22] | 31 [12-58] | 1645 [545-2891] | 1 % |
| 20 | 300 | 4432 [1550-7719] | 14 [4-26] | 36 [12-69] | 2015 [620-4360] | 24 % |
| 24 | 235 | 4090 [1381-6836] | 14 [5-25] | 39 [12-80] | 2460 [804-4695] | 55 % |
| 28 | 183 | 4556 [1152-8371] | 14 [5-26] | 46 [12-94] | 4060 [848-7365] | 62 % |
| 32 | 137 | 5218 [976-9971] | 14 [5-25] | 47 [14-107] | 4418 [804-8988] | 63 % |
| 36 | 95 | 5747 [1440-11565] | 13 [6-26] | 51 [18-116] | 6965 [1305-11275] | 60 % |

The spread is enormous: at minute 20 the middle 80 % of games runs from 1550 to 7719 metal of army. A curve is a
prior, not an estimate, and the survivor bias is real — the games alive at minute 30 are the ones we did not lose.

**M0b, time and which start we drew.** Adding one bit, whether we started north-west or south-east, is worth a lot
(see the table below). It is the map's asymmetry, not ours: at minute 20 the opponent has 20.8 extractors when we are
in the north-west and 8.9 when we are in the south-east (K-maps-quicksilver-corner-asymmetry, 15 spots near the NW
start against 19 near the SE one — and the corner effect is symmetric, whoever holds it).

**M1, seen alive.** Report what we have seen and not watched die. The honest floor, and the "brain now" column above.

**M2, the model.** Baseline by game minute, tilted by the evidence. For each of 23 observations *x*, take its
per-minute z-score `(x − mean_m(x)) / sd_m(x)`, clipped to ±6; the estimate is
`mean_m(y) + sd_m(y) · Σ β_i z_i`, one pooled coefficient vector (ridge 0.02) across all minutes. Then:

- **shrinkage.** A per-minute weight `trust(m) ∈ [0,1]` blends model and baseline. It is fitted on an inner split by
  batch inside the training set, never on training residuals, so a tilt that only looks good in sample gets a low
  weight. For the army target it comes out 0.0 in minutes 2-3, 0.3 / 0.5 / 0.6 / 0.75 / 0.9 in minutes 4-8 and 1.0
  from minute 9 on: before the first contact the evidence is noise and the model defers to the clock. Without it the
  army model is 10 % *worse* than the clock in minutes 1-5 (111 against 100) instead of 1 % better (99); over the
  whole game it is worth little (640 against 637). The weights for the economy targets are noisier (1.0 to minute
  6, a dip to 0.55-0.75 in minutes 7-10, 1.0 after) — each is a grid search over a few hundred rows, so read them as
  "roughly, follow the evidence less before minute 10", not as a schedule.
- **floor.** The estimate is never below what we have seen alive and not watched die — a logical lower bound.
- an additive and a multiplicative (`log1p`) version are both fitted; the additive one wins every target here
  (army 637 against 673, extractors 2.2 against 2.3).

## Held-out error, by game minute

Mean absolute error, five folds held out by batch, Quicksilver heuristic games.

**`y_army_m`, the opponent's live army in metal.** Truth mean 3055.

| minute | rows | truth | time alone | seen alive | brain now | model | model vs time |
|---|---|---|---|---|---|---|---|
| 1-5 | 3440 | 249 | 100 | 223 | 221 | 99 | 1 % |
| 5-8 | 2580 | 1127 | 377 | 978 | 957 | 340 | 10 % |
| 8-11 | 2562 | 2031 | 633 | 1243 | 1218 | 463 | 27 % |
| 11-15 | 3233 | 3160 | 964 | 1377 | 1718 | 524 | 46 % |
| 15-20 | 3563 | 4249 | 1559 | 1464 | 2250 | 711 | 54 % |
| 20-40 | 6975 | 4870 | 2332 | 1498 | 2725 | 1090 | 53 % |
| all | 22353 | 3055 | 1247 | 1190 | 1741 | **637** | 49 % |

Model variants on the same folds: time alone 1247, time + which start we drew 980, evidence about the opponent only
(dropping the three features about our own state) 652, everything 637.

**`y_mex`, finished enemy extractors.** Truth mean 10.

| minute | rows | truth | time alone | seen alive | model | model vs time |
|---|---|---|---|---|---|---|
| 1-5 | 3440 | 2.9 | 0.76 | 2.85 | 0.42 | 45 % |
| 5-8 | 2580 | 5.0 | 1.39 | 5.02 | 1.13 | 19 % |
| 8-11 | 2562 | 7.1 | 1.91 | 6.99 | 1.65 | 14 % |
| 11-15 | 3233 | 10.0 | 3.79 | 9.48 | 2.52 | 34 % |
| 15-20 | 3563 | 13.2 | 6.00 | 12.32 | 3.09 | 49 % |
| 20-40 | 6975 | 14.6 | 6.76 | 13.24 | 3.11 | 54 % |
| all | 22353 | 10.0 | 4.06 | 9.29 | **2.20** | 46 % |

Variants: time alone 4.1, + start 2.9, opponent evidence only 2.3, everything 2.2.

**`y_income`, the proxy.** Truth mean 28 metal/s. Time alone 12.9, + start 8.7, opponent evidence only 6.8,
everything **6.7** (48 %). By band: 1-5 0.98 (46 % better than time alone), 5-8 2.73 (19 %), 8-11 4.35 (12 %),
11-15 6.68 (32 %), 15-20 8.12 (48 %), 20-40 10.99 (56 %).

**`y_def_m`, static defence metal.** Truth mean 1782. Time alone 846, + start 739, opponent evidence only 569,
everything **558** (34 %). Weakest of the four, and the weakness is concentrated early: 5 % better than time alone in
minutes 8-11, 36 % in minutes 20-40.

**`y_t2lab`, does an advanced lab stand right now?** 211 of the 430 games reach one, median at minute 21 (10th
percentile 18, 90th 27); before minute 15 it is 0 % in this dataset. Brier score (lower better), held out:

| minute | rows | really has one | time alone | + evidence | AUC |
|---|---|---|---|---|---|
| 15-20 | 3563 | 6 % | 0.053 | 0.057 | 0.83 |
| 20-40 | 6975 | 54 % | 0.239 | 0.142 | 0.87 |
| all | 22353 | 18 % | 0.083 | 0.054 | 0.97 |

(The all-rows AUC of 0.97 is mostly the clock: it is easy to separate minute 5 from minute 25. The 0.87 within minutes
20-40 is the part that is about evidence.) In minutes 15-20 the evidence makes the estimate slightly *worse* than the
base rate — with 6 % positives there is almost nothing to learn and the tilt only adds variance. Of the 211 games with
a lab we saw tier-2 evidence in 183, a median of **3 minutes after** the lab finished (quartiles +2 and +4); in 17
further games we saw tier-2 evidence when the opponent never built a lab at all — its resurrection bots fielding our
own advanced units.

## What evidence actually moves the estimate

Each observation on its own, beside the time-only baseline, as the share of the time-only error it removes. Single
features, because the full model's coefficients suffer sign flips between collinear partners (`f_seen_army_m` and
`f_seen_army_n` are nearly the same number) and cannot be read as importance.

| observation | army | extractors | income | static defence |
|---|---|---|---|---|
| `f_live_army_m` — army metal seen and not watched die | **39 %** | 8 % | 7 % | 6 % |
| `f_own_mex` — our own finished extractors | 27 % | 28 % | 26 % | **14 %** |
| `f_corner_nw` — which start we drew | 22 % | **30 %** | **32 %** | 13 % |
| `f_seen_army_m` — cumulative distinct army seen, ever | 25 % | 17 % | 15 % | 12 % |
| `f_own_army_m` — our own army metal | 24 % | 21 % | 21 % | 13 % |
| `f_max_army_m` — biggest force in one look, ever | 23 % | 7 % | 6 % | 6 % |
| `f_rec_army_m` — biggest force in one look, last 120 s | 22 % | 6 % | 6 % | 5 % |
| `f_lost_m` — metal of ours destroyed, cumulative | 19 % | 9 % | 8 % | 9 % |
| `f_live_mex_n` — enemy extractors seen and alive | 14 % | 7 % | 7 % | 9 % |
| `f_live_turret_m` — enemy turret metal seen and alive | 12 % | 1 % | 1 % | 6 % |
| `f_t2_age` — minutes since first tier-2 sighting | 12 % | 3 % | 6 % | 7 % |
| `f_kills_m` — metal of theirs we destroyed | 12 % | 1 % | 1 % | 2 % |
| `f_radar_n` — contacts never identified | 12 % | 0 % | 0 % | 0 % |

Forward greedy selection (adding the feature that most reduces held-out error, stopping at half a point of gain):

- army: `f_live_army_m` (39 %), then `f_own_mex` (47 %). Two features score 658 against 637 for all 23.
- extractors: `f_corner_nw`, `f_live_mex_n`, `f_own_mex`, `f_seen_army_m`, `f_seen_army_n` (44 %). Five features
  score 2.3 against 2.2 for all 23.
- income: `f_corner_nw`, `f_live_mex_n`, `f_own_mex`, `f_t2_flag`, `f_seen_army_m` (45 %); 7.1 against 6.7.
- static defence: `f_own_mex`, `f_live_turret_m`, `f_t2_age`, `f_seen_army_m`, `f_seen_army_n` (31 %); 586 against 558.

Held-out MAE of the army estimate for the sets worth wiring, all on the same folds:

| features | all | 8-11 | 11-15 | 15-20 | 20-40 |
|---|---|---|---|---|---|
| `f_live_army_m` alone | 754 | 518 | 685 | 922 | 1257 |
| `+ f_corner_nw` | 700 | 479 | 593 | 809 | 1204 |
| `+ f_own_mex` instead | 658 | 476 | 571 | 766 | 1098 |
| the 20 features that are about the opponent, not us | **652** | 470 | 537 | 732 | 1118 |
| all 23 | 637 | 463 | 524 | 711 | 1090 |

The same for extractors: `f_corner_nw` + `f_live_mex_n` 2.63, plus `f_seen_army_m` 2.50, the 20 opponent features
2.29, all 23 2.21.

Readings:

1. **For the army, direct observation is the model.** `f_live_army_m` alone removes 39 % of the error and is 80 % of
   the total gain. The thing the brain already has is the right thing; it is the two-minute window that ruins it
   (`f_rec_army_m` 22 % against `f_live_army_m` 39 %, and as a raw estimate 1741 MAE against 1190).
2. **For the economy, our own extractor count is better than anything we see of theirs.** It is not a coincidence and
   it is not endogenous noise: the map has a fixed 44 metal spots, and the correlation between our extractor count and
   theirs is **−0.39 at minute 5, −0.54 at 10, −0.72 at 20, −0.78 at 25**. The spots we do not hold, they hold. Our
   own count is a *complement* measurement of their economy, and a far better one than the 1.3 extractors of theirs we
   have ever laid eyes on.
3. **Time alone is not nearly enough, and it is also most of what we have.** Halving the error against the clock is a
   real gain, but the model's minute-20 error of about 1100 metal of army sits against a truth of 4870: a ±25 % band.
   No feature set here gets below that.
4. **Kills and damage carry almost nothing** (`f_kills_m` 12 % and `f_dmg` 14 % for the army, 1-3 % for the economy).
   What we destroyed is a poor guide to what they still have, because BARb replaces it.
5. **The three features about ourselves are individually strong and jointly redundant.** Alone, `f_own_mex` is the
   best or second-best feature for every target. But dropping all three from the full model costs 15 metal of army
   MAE (652 → 637) and 0.1 extractors (2.3 → 2.2): once the start corner and what we have seen of them are in, our
   own state adds almost nothing. That matters for integration, because a gate that reads our own economy and then
   changes it is a feedback loop — and here it can simply be left out.

## The error bar

The half-width is the 80th percentile of the absolute error, fitted **on the training folds only**, so coverage
measured on the held-out fold is an honest test. A width fitted by minute alone is calibrated on average and badly
miscalibrated conditionally — for the army target, minutes 8 on:

| how much of the estimate we had seen | rows | coverage, width by minute | coverage, width by minute and share |
|---|---|---|---|
| under a quarter | 1648 | 66 % | 77 % |
| a quarter to 60 % | 6628 | 78 % | 79 % |
| over 60 % | 8057 | 83 % | 79 % |

So the shipped width is fitted in cells of (game minute × how much of the estimate we have laid eyes on, split at 0.25
and 0.6). Mean width is the same; the coverage is flat. Resulting 80 % half-widths for the army, by minute band:
±164, ±566, ±749, ±814, ±1076, ±1594 — that is ±66 % of the truth in minutes 1-5 falling to ±25-33 % from minute 15.
Extractors: ±0.8, ±1.9, ±2.8, ±3.8, ±4.9, ±4.8 (±26 % to ±39 % of the truth). Income proxy: ±1.8, ±4.7, ±7.0, ±10.1,
±12.7, ±16.3. Static defence: ±37, ±118, ±215, ±380, ±777, ±1866 — ±49 % of the truth at the end, the worst of the four.

**The estimate is unbiased on average and biased conditionally, in the dangerous direction.** Mean signed error over
all rows is +12 metal of army. Split by how the game ended (minutes 8 on): **+337 in games we won, −249 in games we
lost, −17 in timeouts**. It is ordinary regression to the mean — a shrunk estimator over-states a small opponent and
under-states a big one — but the consequence for a wave gate is that it is *too cautious when we are ahead and too
bold when we are behind*, which is exactly backwards. The same shape appears for extractors (+1.1 / −0.8).

## Transfer

The model is fitted on Quicksilver only. Applied unchanged to data it never saw (MAE):

| held-out set | rows | target | time alone | model | both scaled by the map's spot count |
|---|---|---|---|---|---|
| four other maps | 3421 | army metal | 2944 | 1629 | — |
| four other maps | 3421 | extractors | 7.92 | 7.13 | time 6.55, model 6.79 |
| four other maps | 3421 | income proxy | 21.4 | 17.9 | time 18.9, model 22.0 |
| four other maps | 3421 | static defence | 2019 | 1614 | — |
| commander games | 758 | army metal | 1076 | 628 | — |
| commander games | 758 | extractors | 3.44 | 2.57 | — |

The army model transfers in shape — it halves the error on maps it has never seen — but the absolute error (1629)
is two and a half times its in-map error (637), so the numbers are not usable off Quicksilver without refitting. The
extractor model does **not** transfer: the other four maps have 53-92 metal spots against Quicksilver's 44, so the
baseline curve is the wrong height, and scaling the estimate by the map's spot count closes only an eighth of the gap
(7.13 → 6.79 against 2.20 in-map). Worse, once scaled the *time-only* baseline (6.55) beats the model, and for the
income proxy scaling makes the model worse (17.9 → 22.0): the evidence coefficients are calibrated to Quicksilver's
spread and do not survive a change of map. Refit per map; there are 16 games per map here, which is not enough. The
commander games — same map, a very different player — transfer fine.

## What this does not do, and what I did not check

- **One opponent, one difficulty.** Every game is BARb medium. Nothing here is known to hold against another profile,
  another AI or a human, and the whole thing is a model of *one opponent's habits*, not of RTS economies.
- **Our own play is inside the fit.** 430 games spread over bot versions v17 to v34 and 17 batches. Holding out by
  batch means the reported error is the error a *new* bot version would see, which is the right test, but the model
  has learned what BARb does against a bot that plays roughly like ours. If our play changes a lot, refit.
- **The income figure is a proxy** built from extractor counts. It cannot see an energy stall idling their converters,
  reclaim, or an extractor sitting on a rich or poor spot. Its error bar is the extractor error bar in disguise.
- **`forget_razed_buildings` is not replicated.** The brain drops a remembered building when our soldiers stand beside
  its place and cannot see it; the extraction here only removes enemy units on an `enemy_destroyed` event. So
  `f_live_*` for buildings drifts slightly high relative to what the brain would compute. Given how few enemy
  buildings we ever see (0.1-1.3 extractors), the effect is small, but it was not measured.
- **Team games are excluded.** The team batches have no truth file. The design carries over per enemy ally team, but
  nothing here tests it.
- **Not tried:** a state-space filter (the target is a smooth growing curve and the model treats each instant
  independently, so successive estimates can jump); conditioning on BARb's opening (`opponent_first_factory` is in
  `results.jsonl`, and K-barb-opening-varies says bots and vehicles are different games — but the bot only knows it if
  it scouts, and `f_live_fact_n` is a weak feature); a per-minute rather than pooled coefficient vector; gradient
  boosting (the pooled linear model is almost certainly leaving something on the table, and would cost interpretability
  and a Rust port).

## Integration proposal

Not wired in. Concretely, when it is:

1. **The features are already in the brain.** `enemy_soldiers` (`brain/mod.rs:90`) holds every armed mobile enemy ever
   seen minus the ones we watched die — the same filter as the record's `army` class — and is never pruned by time;
   summing its metal costs **without** the `recent` filter that `squads.rs:327` applies gives `f_live_army_m` exactly.
   `f_live_mex_n`, `f_live_turret_m`, `f_live_fact_n`, `f_live_bld_m` and `f_live_builder_n` are `enemy_buildings`
   (`mod.rs:86`) split by class. `f_kills_m` and `f_lost_m` are the two columns of `trade_log` (`briefing.rs:69,82`).
   `f_corner_nw` is one comparison of `self.home` against the map's midpoint. New counters needed: the cumulative
   `f_seen_army_n` / `f_seen_army_m` (a running total beside `enemy_soldiers`), `f_max_army_m`, `f_dmg`, and the first
   tier-2 sighting time behind `f_t2_flag` / `f_t2_age`.
2. **A `tempo.rs` in `crates/bot/src/brain/`** holding the `enemy_only` tables from `tempo-params.json` as `const`
   arrays — about 1900 numbers per target (20 features × 40 minutes × mean and sd, plus the baseline, the trust
   weights, the coefficients and the interval table) — and one function `estimate(minute, features) ->
   (value, half_width)`. No file loading; paste the constants. Ship the `enemy_only` set rather than all 23: it costs
   15 metal of MAE and keeps our own state out of the loop, so a rule that reads the estimate cannot move its own
   inputs. If even that is too much plumbing, `f_live_army_m` + `f_corner_nw` alone scores 700 against 652.
3. **The wave gate** (`army.rs:465-478`). Today `defenders` is `known_enemy_force` at the target, raised to
   `enemy_army_seen` if that is bigger. Replace the `enemy_army_seen` clause with the tempo estimate. Use the
   **lower end of the interval** (`estimate − half_width`), not the point estimate: the gate's failure mode is walking
   into an army it did not count, and the model's conditional bias runs the wrong way (−249 metal in games we are
   losing). Even the lower end is a large correction — the current input understates by about 2100 metal at minute 20.
   This changes launch behaviour, so it is an A/B batch of its own (`--ab-disable H-ARMY-WAVE-GATE-TEMPO`).
4. **The commander's briefing** (`briefing.rs` `publish_briefing`): one line beside `enemies_visible`, e.g.
   *"their tempo (estimate): army ~4400 metal ±1100, 14 extractors ±5, ~36 metal/s, advanced lab likely (0.7)"* —
   with the interval printed, never a bare number, and with the estimate's own honesty stated (it is a model, not a
   sighting). K-barb-medium-army-curve's note applies: this belongs in what the *player* knows, so it goes in the
   briefing the commander reads, not in the harness asserting a fact.
5. **Register the claims before the code.** The entries this study leaves behind are
   K-barb-tempo-is-half-clock-half-sighting, K-army-enemy-army-seen-is-the-worst-estimate-we-have,
   K-scout-we-never-see-their-extractors, K-eco-their-extractors-are-the-spots-we-do-not-hold,
   K-maps-quicksilver-corner-decides-both-economies and
   K-t2-barb-labs-at-minute-21-and-we-notice-three-minutes-late. A rule that reads the estimate rests on the first
   two; the arena batch that tests it decides whether they move to `supported`.
