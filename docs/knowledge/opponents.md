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
**Would be wrong if.** Mirror matchups (both sides the same faction) showed no difference between our factions.
**Used by.** (none; needs a mirror-matchup option in the arena)

### K-opp-medium-wins-by-20
**Claim.** BARb medium beats the v5 brain every time, between minute 16 and 20.
**Status.** supported (2026-09-19)
**Evidence.** v5-medium 0-12. Cause not diagnosed.
**Used by.** (none)
