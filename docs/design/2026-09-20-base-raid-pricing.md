# Target design: pricing the raid on their base as a raid, not a fight

Written 2026-09-20, late, at the user's direction: "Let's focus on that outmatched problem then. There most likely
is an opening there that we are just not finding." Matt's transcribed order gives the bot Matt's Pawns (12 by 3:00,
21 by 5:00) and the party stands "outmatched" 1,200 to 1,450 out from 2:00 on (matt-plan: first kill at 3:55 median,
Matt's at 2:28; his leading Pawn was under the commander's guns at 2:34 and everything but the commander was dead
by 4:00, for seven Pawns).

## What is true today
- The pressure party prices its target with `assault_verdict` (`contact.rs`): the chase simulator runs our party as
  the pursuers, with `Intent::Fight`, against every soldier of theirs in sight within 1,000 of the target and every
  turret bearing on it or the approach, for 45 s, and the gain is what they lose minus 1.7 times what we lose.
  Nothing of theirs that does not shoot is in the scene, so the only way the party can "win" is to kill their
  soldiers and turrets outright in a stand-up fight. Against two towers, the commander and three Pawns, twelve Pawns
  lose that fight, and the rule waits for more.
- What Matt's Pawns did is a raid: burn the outer extractor, the winds, then the towers one at a time with twelve
  guns on each, then the lab, and leave the commander for the sixteen that came. The simulator has that intent
  (`Intent::Raid { then }`: assets first, fight only when provoked, leave when nothing is left to burn) and has
  assets, but only on our side of a chase (our buildings a raiding party burns). No question is ever asked with our
  party as the raider and their buildings as the assets.
- The simulator has no D-gun: `command_fire` weapons are left out ("nobody presses the button"). BARb's commander
  presses it (K-barb-commander-dgun-beats-a-pawn-party): 99,999 damage, 250 reach, one shot every 0.9 s. So the
  commander is under-priced as a defender and over-priced as a kill.

## The change
1. **`Chase` gains `pursuer_buildings`** (turrets holding on the pursuers' side), so a chase can be asked the other
   way round: their soldiers pursue, their towers hold, their unarmed buildings are the assets, and our party is the
   party with `Intent::Raid { then: the way it came }`. `Tuning.dgun` makes a commander's D-gun a weapon.
2. **`raid_verdict`** (`contact.rs`): for a target of theirs, the scene above within 600 of it (assets: every unarmed
   building of theirs remembered there; defenders: turrets bearing on the target or the approach, soldiers in
   sight within 1,000, the commander as a soldier with its D-gun), 45 s. Gain = their metal burned plus their
   soldiers and towers killed, minus 1.7 times ours lost. Outmatched means no target of theirs within the party's
   reach has a positive raid gain.
3. **H-ARMY-PRESSURE prices by `raid_verdict`**: its target, its alternatives and its re-pricing. The party goes for
   the dearest positive target, turrets in the scene attacked first as now (`Command::Attack`), the lane keeping
   the units alive. The contact answer keeps `assault_verdict` (that question is a fight).
4. Calibration before any game: the command-line tool with the scene from Matt's replay at 3:00 (twelve Pawns;
   two towers, the commander, four extractors, seven winds, a lab, no soldier) must price the raid as a go and lose
   about seven Pawns; with five Pawns of theirs added (our usual BARb at 4:00) it says what it says, and the batch
   judges it.

## Judged by
- First kill and first time under fire in the matt-plan opening (today medians 3:55 and 3:16; Matt 2:28 and 2:34),
  BARb's buildings dead by 4:00 (Matt: all but the commander), Pawns lost by 5:00 (Matt 15), and wins, on the same
  seeds as matt-plan (3-9).

## Status
- 2026-09-20: written; nothing built.
- 2026-09-20 night, built: `Chase.pursuer_buildings`, `Tuning.dgun` (the brain's rules arm it; the tool's `--dgun`,
  `--defences`), `raid_verdict` in `contact.rs`, H-ARMY-PRESSURE priced by it. Calibration on the tool: twelve Pawns
  against two towers and everything behind them lose six and burn it all (Matt paid seven); against three towers and
  five Pawns twelve die for 220 and twenty lose eleven for everything. The simulator's commander wins every scene it
  is in (its lasers alone), so it is left out and the lane keeps the party off it (a raid party is committed to it
  only at 900 metal). Three instrumented games on Matt's order found: a party priced only against what it had seen
  walked into three unseen towers (fixed: the unseen base and its towers by the clock are in the scene), and the
  lane's focus put Pawns on winds under a tower (fixed: shooters first, ordered targets first of all). Third game:
  first kills at 2:54 and 2:59 (an extractor and a tower; Matt's 2:28), then the stream lost twelve Pawns in minute
  4 to three towers, five Pawns and the commander. Batch matt-raidprice against matt-plan next.
- 2026-09-20 night, matt-raidprice (ledger): 5-6-1 against matt-plan's 3-9 on the same seeds; first kill 3:16
  against 3:55, BARb's buildings killed by 5:00 1.8 against 1.2, at 17 Pawns lost by 10:00 against 15.8. The party
  now spends its Pawns on the base; it does not yet arrive with twelve at 3:00 as Matt did (the stream goes in as it
  comes, from four up), and the commander is a hole in the scene. Open: the party gathering to Matt's size before
  the first dive when the base is the usual variant; a D-gun model that lets the commander into the scene; the
  simulator's optimism for parties of six to eight (raid-price-debug2).
