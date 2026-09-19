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
