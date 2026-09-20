# Notes from experienced players (unreviewed claims, to be tested before anything rests on them)

## 2026-09-20, a more experienced player watching cmp-opus game 00 (relayed by the user)
- **Mass tier 1 with tier 2 up is a mistake.** Once the production exists to turn resources into tier-2 units, people
  stop making tier-1 soldiers and often reclaim ("eat") the Maces they have to pay for tier 2. Ours: H-T2-PRODUCTION keeps
  the tier-1 labs making soldiers while the advanced lab runs; nothing reclaims our own units. Untested.
- **Maces as the first units of the game are wrong.** Ours: H-PROD-MIX and the opening plan both put a line unit (Mace,
  Thug) first. Untested in games.
- **Well-microed Grunts kill Maces without taking damage early; the numbers shift a few minutes later.** First look in
  `combatsim` (2026-09-20, no dodging micro, 16 seeds): 10 Grunts (430 metal) beat 4 Maces (520) every time, margin
  +0.15, +0.57 when spread at 120; 20 Grunts lose to 8 Maces every time (-0.29). 10 Pawns against 4 Thugs: +0.16. The
  direction agrees: small early numbers favour the raider, bigger blobs favour the plasma unit. "Damageless" needs
  dodging the simulator's policies do not have.
