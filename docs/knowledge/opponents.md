# Opponents

### K-opp-barb-tiers
**Claim.** BARb has profiles `easy`, `medium`, `hard`, `hard_aggressive` (plus `dev`), selected with `[OPTIONS]{profile=...}`;
the default is `hard`. It loads its AngelScript and config from the GAME archive, not the engine's AI directory.
**Status.** supported (2026-09-19)
**Evidence.** `AI/Skirmish/BARb/stable/AIOptions.lua`; engine log "Load script: LuaRules/Configs/BARb/stable/script/hard/init.as".
**Used by.** arena `--profile`.

### K-opp-faction-asymmetry
**Claim.** Against BARb easy our results follow the OPPONENT's faction: easy BARb is much harder as Cortex (it raids) than as
Armada (it does not). It is not our Armada roster.
**Status.** supported (2026-09-19)
**Evidence.** v5-easy-confirm: Cortex 10-2, Armada 4-8 (also SE 9-3, NW 5-7). The arena always gives BARb the other faction,
so "our Armada roster is weak" and "BARb plays Cortex better" cannot be told apart.
**Likely cause (2026-09-19, see K-barb entries in opponents-barb.md).** In `config/easy/behaviour.json` armpw is relabelled
`skirmish` (line 411) but corak is still `raider` + `scout` (line 750): easy BARb raids only when it plays Cortex, which is
exactly when we play Armada. Config fact verified; that it explains the split is inference.
**Mirror test (v5-easy-mirror, 24 matches).** vs Armada BARb: 7-2-3 as Armada (and 10-2 as Cortex in v5-easy-confirm); vs Cortex
BARb: 5-4-3 as Cortex (and 4-8 as Armada). The split tracks BARb's faction. In that batch losses averaged 66 base-defence
firings and 10 commander retreats per match against 5 and 0 in wins.
**Would be wrong if.** A batch with BARb's `disabledunits=corak` did not close the gap.
**Used by.** (none; needs a mirror-matchup option in the arena)

### K-opp-medium-wins-by-20
**Claim.** BARb medium beats the v5 brain every time, between minute 16 and 20.
**Status.** supported (2026-09-19)
**Evidence.** v5-medium 0-12. Diagnosis (2026-09-19): the loss is decided around minute 8-10, not 16-20 — our extractor count
peaks at 4-9 near minute 8 and collapses to 1-3 by minute 10-12 (checked in v5-medium/00, /03, /07, /10). Medium makes its
basic bots raiders, and every BARb fighter carries ANTI_STAT (targets buildings, skips mobile units), so raids walk past our
army to the outermost extractors. In v5-easy-confirm, extractors at minute 10: wins 8-15, losses 1-12 (overlapping, so
extractor survival is a strong factor on easy, not the whole story).
**Used by.** (none)
