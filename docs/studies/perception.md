# What a model has to be shown to see a bad position (2026-09-19)

The user, watching the Sonnet commander's second game (`run/matches/1789869187-commander-2`, north-west start on
Quicksilver), saw at a glance what Sonnet never acted on: the start is a peninsula with two extractors and one neck,
70-80 % of the army stood within 700 elmos of the start point for the whole game, and the rest shuttled through the neck.
`run/perception_probe.py` asks a fresh Sonnet one neutral question ("what are the biggest problems with how we are
playing this position?") about the two moments where the commander got a full report (14:12, 23:53), under six
representations, three answers each; a second call grades each answer blind against five points. Pictures and SVG come
from `run/render_scene.py`. Data: `../data/perception-2026-09-19/`.

| shown | input tokens | peninsula with one exit | army in the wrong place | units shuttle or jam | we hold no ground | base cluttered |
|---|---|---|---|---|---|---|
| text report + text map (what the commander got) | 4.1k | 0/6 | 3/6 | 0/6 | 6/6 | 0/6 |
| + whole-map picture | 5.2k | 0/6 | 5/6 | 0/6 | 5/6 | 0/6 |
| + picture with 90 s trails | 5.2k | 0/6 | 3/6 | 1/6 | 6/6 | 0/6 |
| + trails picture and close-up | 6.1k | 2/6 | 4/6 | 0/6 | 5/6 | 0/6 |
| + SVG source (outlines, units, paths) | 13k | 1/6 | 4/6 | 0/6 | 6/6 | 0/6 |
| two pictures and a legend, no report | 2.6k | 6/6 | 1/6 | 4/6 | 6/6 | 2/6 |

Readings (n = 3 per cell, one game; treat as direction, not rates):
- **The report crowds the picture out.** With the report present the answer is written from the report's numbers
  (metal balance, idle soldiers, squads stacked on one post) whatever else is attached. Only without the report does
  the model describe the land: peninsula and neck 6/6, shuttling 4/6. Pictures do carry the spatial facts, at half the
  tokens of the text.
- **SVG source read as text does not work**: the dearest input and no better than the text map.
- "Army in the wrong place" comes from the text (every squad's post printed at the same coordinates), not the pictures.
- Asked cold, Sonnet sees much of what it missed in play even from text alone (no ground held 6/6, squads stacked on one
  post). In the game it was working from change-only reports and its own earlier notes. Part of the fault is the role,
  not the eyes.
- At 23:53 the raiders were amphibious (`coramph`) and came by water: the peninsula stopped being safe once the opponent
  built them. A rule that calls the home extractors safe has to watch for that.

Not yet tried: asking for a description of the picture first and the advice second; a picture in the running
commander's turns; a bigger model; other games.
