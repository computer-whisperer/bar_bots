# Game rules and engine semantics that shape play

### K-rules-commander-death-ends-game
**Claim.** With default mod options the game ends for a team when its commander dies.
**Status.** supported (2026-09-19)
**Evidence.** `modoptions.lua:136` (`deathmode` default `com`). baseline-easy/00, /05, /06: commander sent to build extractors
36, 55 and 17 times; each match ended right after a commander order. v2-easy-0704/00 f=63390: commander health 2, game over.
**Would be wrong if.** A match continued after our commander's death with default options.
**Used by.** H-COM-LEASH, H-COM-RETREAT.

### K-rules-ai-start-position
**Claim.** The game places AI teams itself inside their start box; an AI does not need to send a start position.
**Status.** supported (2026-09-19)
**Evidence.** `luarules/gadgets/game_initial_spawn.lua` (GuessStartSpot for AI teams, ~line 662); every arena match spawned
at (2032,1188) NW or (5576,5360) SE on Quicksilver.
**Would be wrong if.** A map or lobby setup left our commander unspawned or outside its box.
**Used by.** (none; it is why the protocol has no start-position command)

### K-rules-factory-shift-means-five
**Claim.** For factory build orders the SHIFT option bit means "build five", CTRL "build twenty"; factory orders always append.
**Status.** supported (2026-09-19)
**Evidence.** `rts/Sim/Units/CommandAI/FactoryCAI.cpp:150`. v3-easy/04: 5 constructors and 0 army at minute 4; after the fix
(v4-easy) openings produced fighters first.
**Would be wrong if.** A factory given one unshifted order produced more than one unit.
**Used by.** Enforced in the shim (`engine.rs`, `Command::Build`), so no heuristic can trip on it.

### K-rules-mex-upgrade-in-place
**Claim.** An advanced extractor ordered at the exact position of an existing extractor is allowed, and the old one is
reclaimed when the new one finishes. Advanced extractors yield 4x (extractsmetal 0.004 vs 0.001).
**Status.** conjectured (2026-09-19) — reading only, never tried through the AI interface
**Evidence.** `luarules/gadgets/unit_mex_upgrade_reclaimer.lua`, `cmd_mex_denier.lua:53`; unit files armmex/armmoho.
**Would be wrong if.** The engine rejects the build order because the square is occupied (the AI interface path may differ
from the player UI path).
**Used by.** (planned tier-2 economy; needs an exact-position build in bot-protocol)

### K-rules-impossible-orders-vanish
**Claim.** A build order for something outside the builder's build options is accepted by the AI interface (return code 0) and
then dropped without any event. The builder is idle again on the next tick.
**Status.** supported (2026-09-19)
**Evidence.** dropped-orders batch: 560-890 dropped orders per 20-minute match, 103 of 160 logged ones were the commander told
to build `armadvsol` (constructor-only, see `units/ArmBots/armck.lua` vs `units/armcom.lua`). After guarding by
`UnitDefInfo.build_options` (dropped-orders-2): 7-38 per match, minute-20 metal income +38-44 on 16-19 extractors.
**Would be wrong if.** (mechanical)
**Used by.** Guard in `economy.rs` `run_economy`; H-ECO-ENERGY-BY-STORAGE / H-ECO-FALLBACK-ENERGY pick per builder.

### K-rules-extractor-must-be-exact
**Claim.** The game rejects an extractor order that is not exactly on a metal spot, or where an allied extractor already stands
(`luarules/gadgets/cmd_mex_denier.lua`, `AllowCommand`); the rejection is silent. Letting the engine shift the site to the
"closest buildable" position therefore produces doomed orders when the exact spot is blocked.
**Status.** supported (2026-09-19)
**Evidence.** dropped-orders/01: spot (5640,5352) placed at (5648,5360) was built; three later orders for the same spot were
placed at (5664,5264) and all dropped. Extractor site search radius is now the build-grid snap (16).
**Would be wrong if.** A shifted extractor order ever produced an extractor.
**Used by.** `EXTRACTOR_SNAP` in `economy.rs`.

### K-rules-unreachable-orders-loop
**Claim.** A move, fight or build order to a place the unit cannot path to makes it walk to the nearest reachable point, raise
`UnitMoveFailed`, and go idle. A brain that re-issues the same order to idle units loops forever. Large crowds also raise
move failures by jostling, so failures alone do not prove unreachability.
**Status.** supported (2026-09-19)
**Evidence.** move-failures batches: 1,300-6,800 `UnitMoveFailed` per match. Units stuck at the factory exit (2224,1188) when the
station was 250 ahead of an outpost extractor; constructors re-sent to extractor spots such as (3224,520), (6184,520);
25-57 attackers hovering mid-map for 20 minutes with no enemy in sight.
**Would be wrong if.** (mechanical)
**Used by.** `economy.rs` `note_unreachable_sites`; `army.rs` `note_station_failures`, `note_unreachable_targets` (gives up a
target only after 90 s of failures with no attacker within 600 of it).


### K-rules-wreck-blocks-site-search
**Claim.** `Map_findClosestBuildSite` refuses a position holding a wreck, though `Map_isPossibleToBuildAt` allows it and a
build order there works (the builder reclaims the wreck first). A dead extractor's wreck therefore hides its spot from
the site search for good.
**Status.** supported (2026-09-19)
**Evidence.** nosite-diag: `no site ... wanted=(3336,3256) possible_at_exact=true feature:cormex_dead`.
**Would be wrong if.** An extractor ordered onto a wreck-covered spot were never started.
**Used by.** The shim places extractors with `Map_isPossibleToBuildAt` at the exact spot and searches nowhere.

### K-rules-site-search-ignores-metal-spots
**Claim.** The engine's site search knows nothing of metal spots: base buildings anchored at the start point get put on
the nearest spots, which are then lost to us.
**Status.** supported (2026-09-19)
**Evidence.** nosite-diag: from the SE start every game refused the spot at (5640,5352) from minute 2, and the diagnostic
found three or four of our own wind generators on it. After the keep-out, SE extractors at minute 5 went from 4 to 6.
**Would be wrong if.** "no site" for extractors reappeared with our own buildings listed at the spot.
**Used by.** Shim `SPOT_KEEPOUT` (100 elmos).

