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
  for nothing, because pursuers never give a chase up. Validation against recorded raids: not done. Parts 2-4: not started.
