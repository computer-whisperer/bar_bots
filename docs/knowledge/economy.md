# Economy

### K-eco-judge-energy-by-storage
**Claim.** Energy shortage must be judged by stored energy, not by income against usage: metal converters absorb any
surplus, so usage always rises to meet income.
**Status.** supported (2026-09-19)
**Evidence.** v4-easy/01 and /05: energy income +2000 with extractors stuck at 6-8 for 60 minutes — constructors built
generators forever because `income < usage + 20` never turned false. After the change (v5-easy) extractors reached 18 by
minute 17 (v5-probe/00) and the win rate went from 4-8 to 7-4-1, confirmed 14-10.
**Would be wrong if.** With the storage test, energy still sat near zero for minutes while builders did something else.
(Partly seen: v5-probe/01 energy 0-6 of 2150 from minute 9 with 4 labs — fixed by removing the two-generator cap.)
**Used by.** H-ECO-ENERGY-BY-STORAGE, H-ECO-CONVERT-SURPLUS.

### K-eco-expansion-before-conversion
**Claim.** Extractors on free spots beat converters as a use of constructor time; converters are for energy that would
otherwise overflow storage.
**Status.** conjectured (2026-09-19)
**Evidence.** Indirect: same batches as above; never tested in isolation.
**Would be wrong if.** A converter-first ordering reached higher metal income at minute 15 over 24+ matches.
**Used by.** H-ECO-EXPAND, H-ECO-CONVERT-SURPLUS.

### K-eco-outposts-get-raided
**Claim.** Extractors far from the base are picked off by early raids; BARb easy raids before minute 10.
**Status.** conjectured (2026-09-19)
**Evidence.** v2-easy-0704/00: extractors 12 at minute 10 falling to 4-6 by minute 22. v3-easy/04: extractors 3-5 throughout,
commander retreat triggered 15 times. The turret-per-outpost response has not been evaluated on its own.
**Would be wrong if.** Extractor losses did not drop in matches where outposts had turrets.
**Used by.** H-ECO-OUTPOST-TURRET, H-ECO-OWN-HALF.

### K-eco-t1-ceiling
**Claim.** A tier-1-only economy tops out around +40 metal/s at minute 17 on Quicksilver, which is not enough against BARb medium.
**Status.** conjectured (2026-09-19)
**Evidence.** v5-probe/00 (+38 at minute 17, 18 extractors); v5-medium 0-12, all lost between minute 16 and 20. BARb's own
income was not measured, so "not enough" is inference.
**Reopened 2026-09-19.** The medium losses are decided by extractor raids at minute 8-10 (see K-opp-medium-wins-by-20), long
before a tier-1 ceiling could matter; the ceiling may still be real but is not why we lose today.
**Would be wrong if.** A tier-1 brain with better army handling beat medium, or a tier-2 economy did not change the result.
**Used by.** (motivates the planned tier-2 work)

### K-eco-nano-turrets-and-reclaim-are-normal-play
**Claim.** Two staples of ordinary play are missing from the bot: construction turrets (nano turrets) beside the
factory, which multiply its build power for far less metal than a second lab, and reclaiming wrecks, which returns a
large share of every dead unit's metal to whoever holds the field after a fight.
**Status.** reported (2026-09-19) by the user, watching the replay of the first Sonnet commander game; he judged them
important but possibly not why that game was lost.
**Evidence.** Consistent with the census: BARb medium plays one lab plus one nano turret all game where we build 3-4
labs (K-barb-medium-observed-build), and its resurrection bots feed on our wrecks (K-army-dead-waves-are-resurrected).
Until 2026-09-19 we built no nano turrets and never issued a reclaim order.
**Would be wrong if.** A version with nano turrets and wreck reclaim near home showed no gain in army built by minute 10.
**Used by.** H-ECO-NANO, H-ECO-RECLAIM.

### K-eco-production-is-the-bottleneck
**Claim.** Once early expansion works, one lab cannot spend the income: metal piles up while the opponent, on half the
extractors, builds the bigger army. Build power (construction turrets, labs, later tier 2) has to follow income.
**Status.** observed once (2026-09-19), first noticed by the user watching the game.
**Evidence.** commander-3-play-to-win: extractors 12 v 6 at minute 5, 13 v 6 at minute 8; income 25-30 and spending
11-19 with one lab; 1393-1749 metal banked from minute 6 to 9; army value 1938 v 2286 at minute 9. One construction
turret and 7 converters by minute 10. The rules that add build power sat behind "expand" in every focus order, and with
free spots left the order never got past it; under the expand focus the turret step was not in the list at all. No
build-site refusals: room was not the limit. There is no tier-2 logic in the bot at all.
**Would be wrong if.** With H-ECO-SPEND the bank still sat above 500 for minutes, or army value did not follow income.
**Used by.** H-ECO-SPEND.


### K-eco-raided-ground-is-raided-again
**Claim.** Against BARb medium the unit that killed an extractor leaves within seconds, but the ground does not become
safe: an armed enemy is near the spot again inside a minute about half the time. What we forgo by not expanding is far
larger than what rebuild delays cost: most of it is spots we never walk to, which the opponent then holds.
**Status.** measured in hindsight (2026-09-20), `run/spot_regret.py` over 96 heuristic games with ground truth (v23-v26).
**Evidence.** 1147 extractor losses. Armed enemy within 600 of the spot after the loss: median 16 s, 90th percentile 47 s.
Quiet after that: median 42 s (NW 30 s, SE 65 s); armed enemy back or still there within 60 s in 48 %, within 120 s in
65 %, within 240 s in 76 % (NW 85 %, SE 69 %). We rebuilt 70 %, the spot standing empty a median 135 s, so cover reopens
most hot spots before H-ECO-HOT-SPOTS' four minutes run out. Per game we held a median 134 extractor-minutes (NW 94,
SE 251) and left 466 quiet, usable extractor-minutes untaken (quiet runs over 60 s, less 60 s to get there). From NW the
8 spots 2-3k out: ours 1 minute a game, theirs 70, quiet and usable 78.
**Limits.** Hindsight: an extractor standing there might have drawn the visit that never came, and "quiet" on the
opponent's side is quiet because it is the opponent's side. Straight-line distance, not walking distance. "Armed enemy
within 600" counts an army passing through and ignores whether we had cover there.
