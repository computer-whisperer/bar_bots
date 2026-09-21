# Target design: the commander dictates how many light turrets go up, and where

Written 2026-09-21 at the user's direction after commander game 6 ("Agreed that we need to find a way to let opus
dictate how many llts to bring up where"), with the user's ruling on what turrets are for: Ticks are repelled by one
light turret per extractor in the expansions where no army stands; Hammers and Pawns have to be countered by the army.

## What is true today
- The bot builds turrets on its own by two rules the commander cannot touch: H-ECO-BASE-TURRETS (two 450 forward of
  home after the lab, up to six when constructors are idle) and H-ECO-OUTPOST-TURRET (every extractor more than 500
  from home gets one within 350). In game 6 the bot had six turrets up by 6:00 with the commander asking for none.
- The commander's only lever is `request_turret {x, z}`: one turret near a point, built by the next free constructor.
- Metal spots are numbered `n` in the map the commander is given; `expansion` takes and leaves spots by number.

## The change
Two fields on `set_directives`, timed like the rest:
1. **`base_turrets`** (integer, 0 to 6): the cap on H-ECO-BASE-TURRETS. 0 stops the bot's own base turrets. Unset,
   the rule's own numbers (2, then 6) stand.
2. **`outpost_turrets`**: `"all"` (the rule as it is: one per extractor beyond 500), `"none"` (the rule off; turrets
   only where `request_turret` asks), or a list of spot numbers (one turret at each of those extractors as they
   stand, none elsewhere). Unset means `"all"`.
`request_turret` stays as the point lever. The directives line in every report shows both; the prompt's directive
paragraph and the brief say what turrets are for, in the user's words.

## Judged by
- The next commander game: the turret count and metal by minute 10 against what the commander asked for (game 5:
  nine turrets, 765 metal, all the bot's own; game 6: six by 6:00, 680), and whether the extractors it chose to
  guard survive the Ticks.

## Status
- 2026-09-21: written; nothing built.
- 2026-09-21, built: `Directives.base_turrets`, `Directives.outpost_turrets` (`OutpostTurrets`: all, none, spots), the schema and parser, the report line, H-ECO-BASE-TURRETS capped and H-ECO-OUTPOST-TURRET filtered by them, the prompt's paragraph, the brief's ruling. First game with it: cmd-opus-low-7 (ledger): the lever works (no turret before minute 6 on `base_turrets` 0; outpost turrets on at 4:58), the game lost on the placed start's opening (one constructor to 7:00).
