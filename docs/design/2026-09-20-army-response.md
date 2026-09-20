# Target design: the default army — contact response, posture, what to let go, and what to build

Written 2026-09-20 at the user's direction, before any code. The user watched the pre-search heuristic games: "80 % of
the army chasing single units behind the base while small detachments of enemy troops burn unguarded mexes", "maces
chasing grunts is usually wasting time", and expects the heuristic to lose to this more and more. The heuristic "needs
to start considering defensive posture, contact response allocations, and when it is acceptable to leave things
undefended"; the fight simulator should answer chase questions ("if x can catch and kill y within time, or if it
shouldn't bother").

## What is true today (measured 2026-09-20, the five `now-*` batches, 16 games a map, medium BARb)
- `army.rs` H-ARMY-DEFEND: any enemy unit within 1500 of home, a lone scout or a radar contact included, gets the
  WHOLE home group, re-ordered every 5 s at whichever intruder is nearest. While one exists nothing else is decided:
  no outpost response (H-ARMY-DEFEND-OUTPOST sizes its answer, but runs only with the base clear), no wave.
- That order is given in 41 % (Quicksilver), 48 % (Isidis), 57 % (Comet), 64 % (Feast), 73 % (Mithril) of all game
  minutes after 4:00, and every 5 s for the whole minute in 6-22 %.
- At minute 18 we hold 50-60 soldiers, 52-83 % of them within 1500 of home, under 10 % committed attackers; half the
  games never launch a wave. 28-57 extractors are lost per game after minute 4, 21-35 % of them with no soldier of ours
  within 900.
- Responders are "the nearest soldiers", whatever they are. Nothing asks whether they can catch the target: a Mace
  walks at about 45, a Pawn at 87, a Grunt at 80-85. The odds (`combat.rs`, the duel table) assume the fight happens.
- Unit mix: one fixed batch (H-PROD-MIX: line, raider, line-or-constructor, line, second), chosen from equal-metal
  stand-up duels, blind to the opponent and to what the army has to do; the commander's `set_production` overrides it.
- Opus as commander has 43 % / 28 % of its soldiers near home at minutes 14 / 18 with squads and no waves; Sonnet 72-99 %.
- `crates/combatsim` has speeds, ranges, reload, health, arrival order, the walking field, turrets, mixed forces and
  three target choices. Both sides always close and fight to the end; buildings that are not turrets are not in a
  scenario; the brain does not link it.

## 1. Chase scenarios in the fight simulator
- **Intent per group**: `Fight` (today's behaviour), `Raid(targets)` (go for a list of buildings, ignore soldiers until
  shot at, then by a stated rule fight back or go on), `Flee(to)`, `Guard(point, radius)` (stand, engage what comes in).
- **Assets**: buildings with health and a value (extractor, constructor, generator, lab), killable, not shooting.
- **The question it answers**: pursuers at A, a party at B with an intent, our assets around, T seconds: do we catch
  them and when, what do we kill, what do we lose, what do they burn meanwhile. Asked two or three ways for one
  contact (chase with these units; guard where the party is heading; send nothing), the difference is the decision.
- **Judged by** recorded raids: every record holds raids with known responders, unit types, distances and outcomes
  (raiders killed or not, assets lost first). Predicted against played over a few thousand episodes from existing
  batches; the duel harness stages what the records do not cover. And by its cost: well under a millisecond a
  question, measured, because it runs inside the tick.

## 2. Contact response in the brain (deletes H-ARMY-DEFEND, -DEFEND-OUTPOST, -RESPONDERS as they stand)
- Contacts in sight are grouped into **parties** (near each other, moving together); each party has a guessed intent
  from what it is near and heading for (the territory grid's threat memory says where such parties have gone before).
- Every party is answered in the same tick, not the nearest first. For each: the candidate answers (nearest free
  soldiers that the simulator says can catch it, up to odds of about 1.5; a guard at the asset it is heading for;
  nothing) are priced by the simulator, and the best positive one is taken. A scout gets two fast units or nothing,
  never the army.
- **Commitment**: responders keep their party until it is dead, gone out of reach, or the price turns; no re-targeting
  every 5 s. One soldier answers one party.
- **What to let go**: an asset's price is its rebuild metal plus the income lost while it is down; an answer is sent
  when price x chance of saving it beats the responders' expected loss plus what leaving their post costs. The lab and
  the commander are always answered. The opening search's exposure figure (`Objective::Tempo`'s `exposed`) comes from
  the same prices once they are measured.

## 3. Posture (deletes the single station, H-ARMY-DETACH, the all-or-nothing wave gate)
- Soldiers not answering a party stand where parties arrive: **posts** at the passages between us and the enemy
  (`terrain::passages`) and at the contested edge of held ground (`territory.rs`), sized by what the threat memory
  says comes through each, slow line units first (that is where stand-up fights happen and the duel table is right).
- A **response pool** of fast units stands central to the held extractors, sized to the raiding seen.
- A **push** is posts moving the edge forward where the simulator gives local odds, fed by what posture leaves free;
  there is no global "whole army against everything ever seen" test. The commander's levers (stance, station, squads,
  attack target) stay and outrank posture as they outrank today's rules.

## 4. Unit mix by role
- Production fills whichever of the pools is short of what sections 2-3 want: fast responders, line units for posts,
  plus a counter term from the opponent units seen so far (`enemy_soldiers`) through the simulator rather than the
  equal-metal duel table. `set_production` stays on top as the commander's override. The opening search's army term
  takes the same role split instead of metal alone.

## Judged by (mechanism measures first; win counts are noise at our batch sizes)
- Extractors lost per game after 4:00, and the share lost with no answer within reach.
- Soldiers answering a contact against the contact's value (today: the whole home group against one scout).
- Share of game minutes in which the army is ordered about at home; share of soldiers near home at minutes 10 / 18.
- Army metal killed per metal lost; then results by map and start.

## Order of work
1. Chase scenarios and assets in `combatsim`; validation against recorded raids. 2. The brain links `combatsim`;
contact response replaces the defend rules (deleted first). 3. Posture replaces station, detachments and the wave gate
(deleted first). 4. Unit mix by role. Each step is a commit with its measurement and its rows in `docs/heuristics.md`,
`docs/knowledge/` and `docs/experiments.md`.

## Status
- 2026-09-20, part 1, the model half: `combatsim` has economy buildings in its unit table, group intents (`Fight`,
  `Raid { then }`, `Flee`, `Guard`), escapes, and `chase::Chase` / `Verdict` (caught and when, party metal killed,
  pursuers' lost, buildings burned, `gain_over` another answer); `combatsim chase` asks it from the command line; the
  duel regression is unchanged. First answers: three Grunts burn four extractors in about 15 s, so pursuers 1500 away
  are worth nothing whatever they are, and from 600 away Maces and Pawns both catch them at their work; a fleeing
  Grunt is never caught by a Mace. 0.4-1.2 ms a run. Known fault: a fleeing party out-ranging its pursuers kills them
  for nothing, because pursuers never give a chase up. 
- 2026-09-20, part 1, validation: 1106 recorded raids (K-army-distance-decides-a-raid-response). "Was any raider
  killed" by where the answer came from: played 79 / 52 / 28 / 23 % (within 600, 600-1500, beyond 1500, nobody),
  predicted 89 / 47 / 23 / 19 %; 71 % agreement episode by episode; our answer's losses under-predicted by 40-80 %.
  Good enough to rank answers; the loss side needs a safety factor or the units following the party. 1.2 ms a
  question at 4 seeds. Parts 2-4: not started.
- 2026-09-20, sight against range (the user's intel: a unit with the range may not see what it chases). The
  simulator already let nobody shoot what its side does not see (sight shared across a side, buildings' sight
  included, no radar). What it had wrong: a side walked at the true middle of the other, seen or not, so pursuers
  homed in on a party out of everybody's sight. Now each side walks at where it last saw the other. On the 1106
  raids: answers from within 600 predicted 83 % "any raider killed" (was 89, played 79) and 632 metal killed (was 681,
  played 464); answers from 600-1500 39 % (was 47, played 52): in play those pursuers are re-ordered from what the
  whole team and its radar see, which a scenario holding only the party and the answer cannot know. Not modelled:
  radar (units shoot at radar contacts, with a wobble), and the brain's re-orders.
- 2026-09-20, part 2, first version: H-ARMY-DEFEND, -DEFEND-OUTPOST and -RESPONDERS are deleted; `contact.rs`
  (H-ARMY-CONTACT) groups enemies on our ground into parties, prices 2-32 of the soldiers who would arrive first
  against nobody with `combatsim::chase` every 4 s, sends the best answer worth 20 metal or more (always for a party
  at the lab or the commander), keeps responders on their party until it is dead, unseen for 6 s or priced out twice
  running; everyone else carries on (station, detachments, waves). Costs up to 100 ms a game minute. Not in it yet:
  the guard answer, walking distances (chases are priced over straight lines), the threat memory for intent, asset
  prices from measurement (1.7, 3 and 20 are guesses). First A/B ran against BARb easy by mistake (contact-1): no
  clear difference; re-run against medium as contact-2.
- 2026-09-20, part 2 measured (contact-2, 56 games an arm on three maps against BARb medium, results within noise):
  the response cuts what dies at home (army metal lost within 1500 of home per game-minute 88 / 219 / 54 against
  109 / 287 / 150 on Quicksilver / Mithril / Isidis) and barely the extractors (0.97 / 1.55 / 1.30 lost a minute
  against 0.99 / 1.63 / 1.51), and pays for it 1500-3000 out (303 / 288 / 329 against 308 / 117 / 189): answers
  chased into what follows the party, the loss the simulator under-predicts. 80 decisions a game on lone units. What
  to change before part 3: price the answer against the party plus the enemy soldiers remembered within reach of it
  (`enemy_soldiers` with places, or the threat memory), no chase beyond held ground, and a floor of two on an answer.
- 2026-09-20, the user, from the rush-7 games on Comet: the army is dragged back and forth as one group along our
  front while the enemy picks at it. Suggested: a subagent's review of BARb's own movement logic (`upstream/CircuitAI`)
  and a complete overhaul on our side. Not the near-term priority: the flaw may not show on Quicksilver, where the
  benchmark returns until the players' Comet replays arrive. Belongs to part 3 (posture).
- 2026-09-20, target priority (the user's correction of the "kill the turret first" rule in 8ffc510): the real
  decision is between ignoring combatants to kill a key target with expected losses, and focusing combatants before
  non-combatants; nothing about it is specific to turrets, it is standard micro priority, which we do not model yet.
  The simulator already has the two halves: `Intent::Fight` (combatants first) and `Intent::Raid` (assets first,
  fighting only when provoked, then leaving). The rule to build: price the party under both intents against the same
  scene and take the better gain, and let the chosen intent set the orders (attack-unit on combatants first, or
  attack-unit on the key target with a move out queued). The interim rule in `raid.rs` and `contact.rs` (a priced-in
  turret is attacked first by everybody) is one branch of that hard-coded; the user judged it may suffice for now.
  Belongs to part 3 with the army-movement overhaul.
- 2026-09-20, the user, on the update rate: the 2 Hz tick (`TICK_INTERVAL = 15` in the shim, the only source of it)
  will not do for micro; "small squads of raiders have a lot of room to probe defences, run within milliseconds of
  resistance, and take advantage of various unit differentials". Not the full arc now. Sequence when it comes:
  research the SC2 bot scene's micro engines (report in `docs/knowledge/_inbox/sc2-micro-engines.md`), prepare the
  engine (interval in the hello, a micro pass on every tick and the full brain every k-th, the search and the
  simulator off the tick thread, tick-keyed constants in frames), then a Fable subagent to optimise the control
  engine for raider squads.
