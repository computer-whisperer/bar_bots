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
**Would be wrong if.** A tier-1 brain with better army handling beat medium, or a tier-2 economy did not change the result.
**Used by.** (motivates the planned tier-2 work)
