# Opponents

### K-opp-barb-tiers
**Claim.** BARb has profiles `easy`, `medium`, `hard`, `hard_aggressive` (plus `dev`), selected with `[OPTIONS]{profile=...}`;
the default is `hard`. It loads its AngelScript and config from the GAME archive, not the engine's AI directory.
**Status.** supported (2026-09-19)
**Evidence.** `AI/Skirmish/BARb/stable/AIOptions.lua`; engine log "Load script: LuaRules/Configs/BARb/stable/script/hard/init.as".
**Used by.** arena `--profile`.

### K-opp-faction-asymmetry
**Claim.** We do much better as Cortex than as Armada against BARb easy.
**Status.** conjectured (2026-09-19) — confounded
**Evidence.** v5-easy-confirm: Cortex 10-2, Armada 4-8 (also SE 9-3, NW 5-7). The arena always gives BARb the other faction,
so "our Armada roster is weak" and "BARb plays Cortex better" cannot be told apart.
**Likely cause (2026-09-19, see K-barb entries in opponents-barb.md).** In `config/easy/behaviour.json` armpw is relabelled
`skirmish` (line 411) but corak is still `raider` + `scout` (line 750): easy BARb raids only when it plays Cortex, which is
exactly when we play Armada. Config fact verified; that it explains the split is inference.
**Would be wrong if.** A batch with BARb's `disabledunits=corak` (or mirror matchups) left the split unchanged.
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
