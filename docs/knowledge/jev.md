# Jev (TypeSafe's System One model) as a player of the keyboard

What the pianist games (`docs/design/2026-09-21-pianist.md`, `docs/harness/jev.md`) found about how Jev answers a menu
over a picture of the game. Scope: model jev-1.13.0, the picture and menus of `crates/bot/src/brain/pianist/`, BARb easy
on Quicksilver Remake 1.24.

### K-jev-words-not-numbers
**Claim.** Jev does not count what it has against a plan and does not read a stall off numbers; a count or a stall changes
its answer only when the words beside the option say what the number means ("we have 20 constructors already: far too
many", "energy: STALLING").
**Status.** supported (2026-09-21)
**Evidence.** pianist-smoke-1: the lab chose a constructor at 0.5-0.9 thirty times running against "two constructors first,
then raiders", with `ours.constructors` in the picture rising from 0 to 30 and `soldiers: 0`; the probability drifted from
0.93 to 0.5 with the count and flipped only when metal read "full". Energy stood at 0 from 0:50 to the end with the line
reading "77 coming in, 77 going out: in balance" (the engine caps spending at income once the store is empty). pianist-smoke-2,
with the count in words in the option and the question: the lab switched to raiders at 3:10 with three constructors and to
line units at 6:03. pianist-smoke-3, with the energy state in the generator option's words: energy at 1293 of 1306 by 5:00.
**Would be wrong if.** A picture with the counts as numbers only produced the plan's switch at the right count.
**Used by.** H-HANDS-MENU.

### K-jev-split-vote
**Claim.** A Choice with several near-equivalent options (one per free spot) spreads its probability over them, and a
single alternative wins with a fraction of the total; the remedy is one option for the kind of action and a parallel
question for its parameter (the spot in `where`).
**Status.** supported (2026-09-21)
**Evidence.** pianist-smoke-2: with six `extractor_at_spot_N` options, constructors chose `assist_lab` at 0.20-0.47 over
extractors at 0.14-0.25 each (0.6 between them), 4 extractors at 5:00; pianist-smoke-3 with one `extractor` option and the
spot in `where`: 8 / 14 extractors at 4:00 / 6:00.
**Would be wrong if.** The one-option form chose extractors no more often on the same pictures.
**Used by.** H-HANDS-MENU.

### K-jev-answers-drift
**Claim.** The same state and questions on the same model version do not give the same answers two days apart; the top
choice can flip. Between calls minutes apart the wobble is a few hundredths.
**Status.** supported (2026-09-21)
**Evidence.** `experiments/jev/01`: `escort` 0.72 on 2026-09-19, `defend` 0.58 / `escort` 0.39 on 2026-09-21, jev-1.13.0
both days; scenarios 02 and 03 moved by 0.02-0.03.
**Would be wrong if.** A re-run reproduced the first day's numbers.
**Used by.** H-HANDS-SWITCH.
