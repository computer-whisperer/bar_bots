# Opponent: BARb (BARbarIAn) — source reading

Source-reading report, 2026-09-19. Nothing here was tested in the arena by this report; the only arena data used is the
per-minute `mex` column of existing `bot.log` files (cited where used).

**What was read.**
- Game-side config and scripts (the authority; the engine log confirms they are what loads):
  `upstream/Beyond-All-Reason/luarules/configs/BARb/stable/{config,script}/`. Paths below are relative to that directory
  and written `config/...` or `script/...`.
- C++ engine of the AI: `upstream/CircuitAI/src/circuit/` (branch `barbarian`, commit e3bebd3, 2026-09-13). Paths below
  written `src/...`.
- **Version caveat.** The source tree says version `1.6.30` (`src/CircuitAI.cpp:90`); the binary we run logs `1.6.28`
  (`run/matches/1789844092-v5-medium/00/engine.log`, first BARb line). Two patch versions apart; the clone is shallow, so
  I could not see what changed. Every C++ citation is therefore "source of a near-identical version". The config/script
  citations are exact: the log shows `Load config: .../config/medium/behaviour.json` etc.
- Scope of all claims unless stated: 1v1, land map (Quicksilver Remake 1.24, 50.6 % land, 44 metal spots), game
  `byar:test`, `cheating` off, no Legion/scav/extra-units mod options.

**Vocabulary used below.** *Power/threat*: BARb's scalar strength per unit,
`sqrt(dps) * dmg^0.25 / 128 * sqrt(health)` (`src/unit/CircuitDef.cpp:32,621-623`). My hand estimates from unit files:
armpw 2.5, corak 2.8, armrock 4.8, corthud 6.4, armllt 7.3 (conjectured — I computed them, BARb does not log them).
*Role*: a label per unit type in `behaviour.json` (raider, skirmish, assault, riot, ...) that decides both which task the
unit gets and how enemy units of that type are counted for counter-building.

---

## Summary (one page)

**How BARb plays.** It is a task-pool AI. Builders pick the cheapest-to-reach open task from a shared list (extractor,
energy, factory, nano turret, defence, reclaim, repair); factories roll dice on a per-factory probability table chosen by
income tier, overridden by a counter ("response") table when the enemy has a lot of metal in some role. Fighters are
sorted by role: scouts walk metal clusters, **raiders** leave home as soon as ~4 of them exist and hunt weakly defended
buildings, everything else gathers at base in a "defend" group and is promoted to an **attack** group once its power
reaches `max(quota.attack, threat of the second-largest known enemy group)`. After the first attack group exists, every
new unit is promoted at once and trickles forward to merge. Units retreat home to be repaired when health falls under a
per-game random threshold, and come back at 98 % health. The commander builds within 1000 elmos of base, hides when
threatened, and never attacks. A threat map (discs stamped from every remembered enemy) prices all pathing and vetoes
targets and build sites.

**The single most important finding.** The game-side script marks *every* mobile land fighter `ANTI_STAT` ("only static
targets") at start-up (`script/common.as:135-161`, called from every profile's `main.as`). In the C++ target selection of
attack, raid, scout, anti-heavy and bomb tasks, an `ANTI_STAT` unit skips all mobile enemies
(`src/task/fighter/AttackTask.cpp:293`, `RaidTask.cpp:319`). So BARb's armies do not seek our army: they walk to our
**buildings** — extractors first, since those are the outermost — and only shoot our units on the way (fire state 3).
Our own logs agree: in all 12 medium losses our extractor count peaks at 4-9 around minute 8 and is back to 1-3 by
minute 10-12 (`run/matches/1789844092-v5-medium/*/bot.log`). We do not lose at minute 16-20; we are strangled from
minute 8 and the corpse falls at 16-20.

**How the tiers differ.**
- `easy` and `medium` run *identical scripts* (one line differs, the fallback opener). All difference is JSON.
- `easy`: no raiders as Armada (armpw is relabelled "skirmish"), scouts disabled, extractors capped at 30 % of the map
  (13 of 44 spots), 3 s pause after every build, no counter-building except anti-air, fighters retreat at a random
  10-100 % health, no builder escort, attack group at 30 power, at most 1-2 nano turrets per factory.
- `medium`: armpw **and** corak are raiders (squad launches at power 9 ≈ 4 units), extractor cap 80 %, 1 s build pause
  fading to 0 by minute 15, full counter table with weight 30 (it swamps the base probabilities), fighters retreat at a
  random 55-100 %, builders escorted, attack group at 40 power, fusion allowed, nano-turret allowance doubled for T2/air/other
  factories (T1 labs stay at 1).
- `hard` / `hard_aggressive`: own scripts (randomised openers with 3-10 raiders and 3-5 constructors, economy-driven
  factory switching, "energizer" base builders), constructor budget doubled (`buildpower` 1.2 vs 0.6), no build pause,
  no extractor cap, 8-12 nano turrets per lab, 5-6 income tiers, commander roams to 2000 elmos until minute 7.
  `hard_aggressive` = `hard` with fewer nano turrets, T1 labs still eligible as later factory choices
  (`importance[1]` 0.2 instead of 0), and closer engagement at high unit counts.
- No profile gets resource bonuses, handicaps or map hacks. `cheating` (off by default, we do not set it) only gives
  full vision.

**Why we split 10-2 / 4-8 by faction on `easy` (answer to the asymmetry question).** In `config/easy/behaviour.json`
armpw was relabelled `skirmish` (line 411-413) but **corak was left `raider` with `scout`** (line 750-753). Easy-Armada
BARb never raids; easy-Cortex BARb sends AK raid squads (76 % of its tier-0 lab output) at our extractors. We are Armada
exactly when BARb is Cortex. The easy readme says raiding was meant to be removed for both
(`config/easy/easy_ai_readme.txt:26`), so this is an upstream oversight, not design. In our logs the easy losses are the
matches where `mex` collapses (v5-easy-confirm/01, /04, /05, all Armada) and the wins are the ones where it climbs to
13-19 (/02, /03, Cortex).

**Top five exploitable weaknesses.**
1. It attacks buildings, not armies, and a raid squad refuses any building where the threat map value is at or above
   ~0.75× its own power. Defended extractors are skipped; undefended ones die. Its army can be intercepted while it
   walks past ours (K-barb-anti-stat, K-barb-raid-threat-gate).
2. Enemy dragon's teeth were deliberately un-ignored by the same script and count as static targets: 8 metal, 2800
   health. They may soak whole attack squads (K-barb-wall-decoy, conjectured).
3. Retreat threshold is one random draw per game in [0.55, 1.0] (medium). In many games its units turn for home after
   the first scratch and stay out until repaired to 98 %. Chip damage from long range removes units from the fight
   without killing them; pursuit kills them (K-barb-retreat-random).
4. Gathering rule: the home group attacks only when its power ≥ max(40, second-largest known enemy group). It is blind
   to anything it has not seen, and raiders/attackers leave home the moment they qualify, so its base is defended mainly
   by LLTs on a fixed clock and by whatever is still gathering (K-barb-attack-gate, K-barb-base-defence-clock).
5. Its commander is leashed to 1000 elmos around its start, which on a mirrored map is a known point, and it is classed
   as a builder that retreats at 89-100 % health toward its factory (K-barb-commander).

**Top five techniques to borrow** (value for effort, details in the last section).
1. Threat-gated target choice: never send a squad at a target whose local threat ≥ squad power × factor.
2. A coarse threat map with enemy memory (discs of `range + slack` per remembered enemy), used for 1, for expansion
   safety and for retreat.
3. Retreat-and-repair with a haven near the factory/nano turrets, return at ~98 %.
4. Economy state flags from storage fractions plus an energy ladder keyed on income, and "constructor count =
   k × metal income".
5. Build chains: LLT behind every new factory, defence per extractor cluster bounded by income, nano turret when income
   exceeds what the factories can spend.

---

## Profile differences

Files under `config/<profile>/`; a missing file falls back to the shared `config/<name>.json`
(`src/setup/SetupManager.cpp:677-687`; confirmed in the medium engine log: `Load config: .../config/block_map.json`,
`.../config/response.json`). So `medium` uses the shared `response.json` and `block_map.json`; `easy` has its own
`response.json` and the shared `block_map.json`; `hard*` have their own of everything.

| Setting | easy | medium | hard | hard_aggressive |
|---|---|---|---|---|
| Scripts | `script/easy/*` | identical to easy except fallback opener (`misc/commander.as:92`: easy `{BUILDER, SKIRM}`, medium `{RAIDER, BUILDER}`) | own scripts | identical to hard except log string (`init.as:9`) |
| Max scouts (`quota.scout`) | 1 (`behaviour.json:4`), but flea/fav `limit: 0` (:433-436, :530-533, :862-865) | 1 (:4) | 2 (:9) | 2 |
| Raid squad `[min, avg]` power | [9, 420] (:5) | [9, 300] (:5) | [6, 150] (:10) | same |
| Attack group min power | 30 (:6) | 40 (:6) | 60 (:11) | 60 |
| `thr_mod.attack` (enemy threat multiplier range) | [0.8, 1.0] (:8) | [1.0, 1.0] (:8) | [0.6, 0.8] (:13) | same |
| `thr_mod.static` / `mobile` | 0.8 / 1.05 (:10-11) | 1.2 / 1.05 (:10-11) | 1.0 / 1.0 (:15-16) | same |
| Commander threat multiplier `comm` | 0.01 (:12) | 0.01 (:12) | 0.003 (:17) | same |
| Air stops when enemy AA threat > | 25 (:15) | 25 (:15) | 500 (:20) | same |
| `num_batch` (repeat a response build) | 5 (:21) | 5 (:21) | 3 (:26) | 3 |
| Anti-capture probability | -1 = off (:22) | 0.25 (:22) | 2.0 = always (:27) | same |
| Threat-range shrink at high unit counts (`end_scale_value`) | 1.0 = off (:37) | 0.9 (:37) | 0.5, end 350 units (:40-42) | 0.4, end 450 units |
| Builder retreat health `[a, b]` | [0.7, 1.0] (:42) | [0.89, 1.0] (:42) | [0.85, 1.0] (:47) | same |
| Fighter retreat health `[a, b]` | [0.1, 1.0] (:43) | [0.55, 1.0] (:43) | [0.50, 1.0] (:48) | same |
| Builder escort `[builders, defenders, until s]` | [0, 0, 600] (:51) | [2, 1, 600] (:51) | [3, 1, 540] (:58) | same |
| Base-defence radius `base_rad` | [600, 1200] (:49) | [600, 1200] (:49) | [800, 1400] (:56) | same |
| armpw role | **skirmish** (:411-413) | raider + scout (:389-392) | (not read line-by-line) | |
| corak role | **raider + scout** (:750-753) | raider + scout (:722-725) | | |
| armrock role | assault (:419-421) | skirmish (:398-400) | | |
| armflash / corgator role | skirmish (:534-536, :866-868) | raider + scout (:511-513, :837-839) | | |
| Metal storage `since/limit` | 120 s / 5 | 300 s / 2 (:344-355) | | |
| T1 constructor `limit` | 25 | 20 (:386) | | |
| Response table | anti-air only, weight 0.1 (`easy/response.json:33-44`) | shared full table, weight 30 (`response.json:9-109`) | own (`hard/response.json`, 150 lines, not analysed) | same as hard |
| Solar count limit (random in range) | 50-100 (`economy.json:10,14`) | 40-80 (:10,14) | 12-14 arm / 9-12 cor (:12,22) | same |
| Adv. solar condition (metal, energy income) | 30, 1000 (:11,15) | 30, 500 (:11,15) | 12, 220 (:13,23) | same |
| Fusion | limit 1-2, no condition given (:12,16) | 2-4; arm 60/1200, cor 50/1200 (:12,16) | 3-4 arm 50/900; cor 4-6 50/600; also afus, gmm, ckfus (:15-27) | same |
| Energy factor ramp | 1.0@300 s → 10@7200 s (:32) | 1.0@300 s → 15@3600 s (:32) | [6@1 s, 20@300 s, ...] (:53; C++ reads only the first two pairs, `src/module/EconomyManager.cpp:430-438`) | same |
| `buildpower` (mobile build power per metal income) | 0.6 (:67) | 0.6 (:67) | 1.2 (:99) | same |
| `excess` | 0.1 (:70) | 0.1 (:70) | -1 = off (:100) | same |
| `ms_pull` (share of metal for mobile builders) | [[0.9,0],[0.9,0.34]] (:74) | [[0.57,0],[0.66,0.34]] (:74) | same as medium (:102) | same |
| Extractor cap `mex_max` | **0.30** of spots, ally-wide (:79) | 0.80 (:79) | 1.0 = none (:111) | same |
| Simultaneous extractor upgrades `mex_up` | 1 (:55) | 1 (:55) | 4 (:94) | same |
| Build pause `build_delay` | 3 s → 1 s at 1200 s (:83) | 1 s → 0 at 900 s (:83) | off (:115) | off |
| `production` multipliers | [0.9, 0.7, 0.8, 0.8] (:89) | same (:89) | [0.8, 0.8, 1.0, 0.8] (:121) | same |
| Preventive defences `prevent` | 0 (`build_chain.json:20`) | 1 (:15) | (not read) | |
| Base defence clock (index, seconds) | LLT 120, LLT 240, beamer/hllt 420, AA 520, HLT 760, energy storage 900 and 1000, AA 1200, HLT 1400, amb/toast 1600, adv. radar 1800 (:31) | LLT 120, LLT 240, [3] 420, AA 520, HLT 760, flak 1200, HLT 1220, targeting 1320, amb/toast 1400, anni/doom 1800 (:26) | | |
| Extra chains | LLT beside every solar (arm always, cor 80 %) (:90-92, :124-130); solar forced near each extractor until +500 E (:198-202); LLT+rocket turret at radars (:206-225) | none of those (commented out, :85-91, :128-134, :202-206) | | |
| T1 lab start/switch importance | [1.0, 1.0] (`factory.json:28,273`) | [0.9, 0.0] (:28, :276) | [1.0, 0.0] (:27) | [1.0, 0.2] |
| T1 lab nano turrets (`caretaker`) | 1 (:51) | 1 (:51); medium raises most others: armalab 2, armavp 2, gantry 4 | 8 (:54), T2 12 (:81) | 3, T2 6 |
| Income tiers for T1 lab | [20, 30, 40] (:34) | [20, 30, 40] (:34) | [2, 25, 35, 50, 100] (:34) | same |
| Factory switch rule | every 900-1200 s (`manager/factory.as:89-114`) | same | `(frames since)^0.9 × metal income + 7 × stored metal > rand(8000,12000)×30` (`hard/manager/factory.as:127-149`) | same |
| Energy-stall test grace | first 2 min (`manager/economy.as:28`) | same | first 3 min; metal "full" at 99 %; nano assist gated on metal > 20 % and no stall (`hard/manager/economy.as:27-38`) | same |
| Commander hide `time / threat / task_rad` | 180 s / 7 / [1000, 900] (`commander.json:19-22`) | same (:19-22) | 420 s / 8 / [2000, 800] (:20-23) | same |
| Commander assists factory until N constructors | 3 (:25) | 3 (:25) | (not read) | |

Unit restrictions: none per profile beyond the `limit` values above. `disabledunits` and `cheating` are user options
(`run/engines/2026.07.04/AI/Skirmish/BARb/stable/AIOptions.lua`); our start script sets only `profile`
(`run/matches/1789844092-v5-medium/00/script.txt:18`). Note that the engine-side `AIOptions.lua` lists only `dev` as a
profile (the others are commented out) — the game supplies the real list.

Differences between the CircuitAI repo copy (`upstream/CircuitAI/data/`) and the game side: the repo ships only a `dev`
profile; its shared `behaviour.json`, `block_map.json`, `build_chain.json`, `economy.json`, `common.as`, `task.as`,
`unit.as` all differ from the game's (`diff -rq`). I did not analyse the repo copies; nothing in this report rests on them.

---

## Entries

### K-barb-config-fallback
**Claim.** BARb loads each of the seven config parts from `config/<profile>/<part>.json` and falls back to the shared
`config/<part>.json` when the profile lacks it; later parts override earlier keys. `medium` therefore runs the shared
`response.json` and `block_map.json`.
**Status.** supported (2026-09-19)
**Evidence.** `src/setup/SetupManager.cpp:662-732`; `run/matches/1789844092-v5-medium/00/engine.log` lines
"Load config: ...config/block_map.json" and "...config/response.json".
**Would be wrong if.** A medium engine log showed `config/medium/response.json` being loaded.
**Used by.** (none)

### K-barb-no-resource-cheats
**Claim.** No profile receives resource multipliers, build-speed bonuses or vision. Difficulty is made only of
restrictions (caps, pauses, relabelled roles, thinner counter tables). `cheating=true` enables engine cheat events so that
every enemy unit is registered at start and never leaves LOS/radar bookkeeping; it changes nothing else.
**Status.** supported (2026-09-19) for the absence in config and for the `cheating` code path; the `build_speed` values
in `behaviour.json` are, by their own comment, planning overrides ("FIXME: Temporary tag to override buildSpeed",
`config/medium/behaviour.json:137-138`) — that they do not alter the real unit is conjectured (I did not trace the setter).
**Evidence.** `src/CircuitAI.cpp:341-369, 541-550, 676-680, 1700-1703`; all four `economy.json`.
**Would be wrong if.** BARb's measured metal income exceeded what its extractor and converter count can produce.
**Used by.** (none)

### K-barb-anti-stat
**Claim.** In every profile, all mobile non-air units with role raider, riot, assault, skirmish, artillery, anti-heavy or
anti-air get attribute `ANTI_STAT` and fire state 3 at start-up. In attack, raid, scout, anti-heavy and bomb tasks an
`ANTI_STAT` leader skips every mobile enemy when choosing the task's target. BARb's offensive squads therefore choose
only buildings as destinations; they engage our units only incidentally (weapons free) or through the separate defend
task, which has no such filter.
**Status.** supported (2026-09-19) for script and 1.6.30 source; binary is 1.6.28 (contains the string `anti_stat`).
**Evidence.** `script/common.as:135-161, 170-216`; `script/easy/main.as:12`, `script/hard/main.as:34`;
`src/task/fighter/AttackTask.cpp:248,293`, `RaidTask.cpp:256,319`, `ScoutTask.cpp:164,205`,
`AntiHeavyTask.cpp:290,319`, `BombTask.cpp:232,282`; `src/unit/CircuitDef.h:84`. Engine log line
"[WallTargets] static pressure tuned defs=84" shows the function ran. Arena: v5-medium, all 12 matches, our `mex` count
peaks 4-9 near minute 8 and falls to 1-3 by minute 10-12.
**Would be wrong if.** A BARb attack squad, with our army and our extractors both known to it, walked to our army's
position rather than to a building.
**Used by.** (candidate: defend by standing between BARb's base and our outermost buildings, not at home; expect its
squads to arrive at extractors, outermost first)

### K-barb-raid-threat-gate
**Claim.** A raid squad considers an enemy building only if the threat-map value at the building is below
`squadPower × 0.75 / mod` (mod = 1 on medium; ×1/0.75 more tolerant if it already had a target; up to ×2 more tolerant
inside its own base radius). Raiders prefer builders/commander-class and low-threat targets within LOS+200 of the leader
and otherwise walk toward remembered targets through low-threat cells.
**Status.** supported (2026-09-19), same version caveat.
**Evidence.** `src/task/fighter/RaidTask.cpp:250-300`; powerMod from `src/module/MilitaryManager.cpp:627-630`;
`thr_mod.attack` `config/medium/behaviour.json:8`.
**Would be wrong if.** Raid squads of ~4 AK/Pawn attacked extractors standing inside the range of two LLTs as readily as
bare ones.
**Used by.** (candidate: H-ECO-OUTPOST-TURRET is the right idea; size it — one LLT (my estimate: threat 7.3 at its
centre, halving toward the edge of range) only turns away the minimum squad of 4 (power ~10-11 × 0.75); two turrets per
outpost cluster, or turret + a few units, covers squads up to ~8)

### K-barb-raid-timing
**Claim.** Raider-role units wait at base in a defend group and are released as a raid squad when group power ≥
`quota.raid[0]` (9 on easy/medium, 6 on hard) — by my power estimates 4 Pawns or 4 AKs. Once any raid task exists, each
new raider is released immediately and merges with a squad when close. Squads merge up to `raid[1]` power (300 medium).
**Status.** supported for the rule; the "4 units" is conjectured (hand-computed power).
**Evidence.** `src/module/MilitaryManager.cpp:1667-1682, 616-630`; `src/task/fighter/DefendTask.cpp:107-118`
(promotion when `attackPower >= maxPower` **or** a task of the promote type already exists); `config/medium/behaviour.json:5`.
**Would be wrong if.** First contact at our extractors came from single units rather than a group of ~4, or later
raiders arrived in fresh groups of 4 rather than as a trickle.
**Used by.** (candidate: first raid arrives roughly when its lab has produced 4 raiders after the opener — expect
minute 4-6 on medium; have outpost defence up by then)

### K-barb-easy-faction-asymmetry
**Claim.** On `easy`, Armada BARb has no raiders at T1 (armpw, armflash relabelled `skirmish`; armflea/armfav `limit 0`)
but Cortex BARb does: corak is still `raider` + `scout`, and the Cortex lab builds corak with probability 0.76 at income
tier 0. corgator is `skirmish` on easy, so the asymmetry is specific to the bot lab — which is the lab it builds first
on Quicksilver (K-barb-first-factory).
**Status.** supported (2026-09-19) for the config; "this causes our 10-2 vs 4-8 split" is conjectured.
**Evidence.** `config/easy/behaviour.json:411-413, 433-436, 534-536, 750-753, 866-868`; `config/easy/factory.json:276-281`;
stated intent `config/easy/easy_ai_readme.txt:26`. Arena: v5-easy-confirm /01 /04 /05 (we Armada, losses) mex collapses
to 2-3; /02 /03 (we Cortex, wins) mex reaches 13-19.
**Would be wrong if.** With `disabledunits=corak` on easy (or in mirror matchups) the faction split persisted.
**Used by.** (candidate: nothing faction-specific is wrong with our Armada roster; fix outpost defence and re-measure)

### K-barb-medium-vs-easy
**Claim.** What medium does that easy does not, in order of likely impact on us: (1) both factions raid from the first
lab (roles, above); (2) extractor cap 80 % instead of 30 % of spots — 35 vs 13 on Quicksilver; (3) full counter table
with weight 30, e.g. riot-vs-raider importance 100 — Armada medium answers our Pawn/AK spam with Warriors; (4) its units
retreat for repair at ≥ 55 % health instead of possibly fighting to 10 %; (5) 1 s→0 build pause instead of 3 s→1 s;
(6) first two constructors escorted; (7) adv. solar at +500 E instead of +1000 E and fusion enabled; (8) attack group 40
instead of 30 and it discounts our static defence less (static modifier 1.2 vs 0.8).
**Status.** supported (2026-09-19) for each config difference (table above); the ranking is conjectured.
**Evidence.** Profile table. For (3): `config/response.json:33-39`, `src/module/FactoryManager.cpp:1694-1702`.
**Would be wrong if.** Medium with raiders disabled (`disabledunits=armpw+corak+armflash+corgator+armfav+corfav+armflea`)
still beat us 12-0 on the same timeline.
**Used by.** (candidate: the medium problem is extractor survival from minute 6, not tier 2; K-eco-t1-ceiling should be
re-examined after outposts hold)

### K-barb-first-factory
**Claim.** The first factory is chosen by `score = random(-20..20) + importance[0] × (percent of map reachable by the
factory's unit type + speed bonus)`, air excluded for the first choice (`no_air: 1`). On Quicksilver bots reach 88 % of
the map and tanks 38.5 %, so with importances 0.9/0.75 (medium) or 1.0/1.0 (easy) the bot lab wins every time.
**Status.** supported for the formula and the terrain percentages; "every time" is conjectured arithmetic
(bot ≥ 0.9×88−20 = 59 vs vehicle ≤ 0.75×(38.5+speed bonus ≤ ~16)+20 ≈ 61 — nearly but not strictly disjoint on medium).
**Evidence.** `src/unit/FactoryData.cpp:86-121, 134-143`; `config/medium/factory.json:5-12, 28, 76`; engine log
"Mobile-Type ... 'vbot6' ... 88.25%" and "'htank4' ... 38.52%".
**Would be wrong if.** We saw corvp/armvp as BARb's first factory on this map in more than a rare match.
**Used by.** (candidate: plan against bots; BARb does not log its choice — see Open questions)

### K-barb-opener
**Claim.** On the first frame a factory exists, the script queues an opener. easy and medium: armlab → skirmisher,
constructor, skirmisher, constructor; corlab → skirmisher, constructor; any vehicle plant → fallback (medium: raider,
constructor; easy: constructor, skirmisher). Fighters in the opener have HIGH priority, constructors NORMAL. hard: one of
three weighted openers per lab with 3-10 raiders, 1-2 scouts and 3-5 constructors. The role is resolved to the unit of
that role with the highest current tier probability (medium Armada "skirmisher" = armrock; Cortex = corstorm).
**Status.** supported (2026-09-19)
**Evidence.** `script/medium/misc/commander.as:44-93`, `script/medium/manager/factory.as:44-72`;
`script/hard/misc/commander.as:48-138`; `src/module/FactoryManager.cpp:1778-1800`.
**Would be wrong if.** BARb medium's first two lab units were not one fighter and one constructor.
**Used by.** (none)

### K-barb-commander-opening
**Claim.** The commander's opening is not scripted: it takes tasks from the same pool (extractor HIGH priority, energy
when stalling or idle, factory once `metal income − 25 % of factory cost rate` allows it). It assists the factory at high
priority until 3 constructors exist. A LLT is chained behind every T1 factory at priority NOW; base-defence LLTs are
due at 120 s and 240 s.
**Status.** supported for the rules; the resulting order (extractors, 1-2 energy, lab, LLT) is conjectured.
**Evidence.** `src/module/EconomyManager.cpp:1049-1169, 1504-1539`; `config/medium/commander.json:25`;
`config/medium/build_chain.json:26, 179-197`.
**Would be wrong if.** A scouting unit at minute 2 found no LLT directly behind BARb's lab.
**Used by.** (candidate: early raids into its base meet 1 LLT at ~1:30-2:00, 2-3 by minute 4)

### K-barb-unit-mix
**Claim.** Each factory rolls on a probability row chosen by `min(metal income, energy income)` against `income_tier`
([20, 30, 40] for T1 labs on easy/medium; a separate "air" table applies when enemy air metal exceeds its AA metal).
Medium/easy T1 rows (constructor, raider/pawn, rez bot, rocket, hammer/thud, AA, warrior, flea):
Armada tier0 `.04 .52 .10 .00 .09 .00 .00 .25`, tier1 `.03 .27 .10 .35 .15 .00 .06 .04`, tier3 `… .80 warrior`;
Cortex (corck, corak, cornecro, corstorm, corthud, corcrash) tier0 `.04 .76 .10 .01 .09 .00`, tier1 `.03 .35 .10 .35 .17`,
tier3 `.01 .05 .10 .40 .43 .01`. Constructors are requested separately whenever mobile build power <
`buildpower × min(metal, energy income)`.
**Status.** supported (2026-09-19)
**Evidence.** `config/medium/factory.json:34-49, 278-291` (easy identical: `config/easy/factory.json:34-49, 275-288`);
`src/module/FactoryManager.cpp:1038-1072, 1744-1776`.
**Would be wrong if.** Below +20 metal, Cortex BARb's army were not mostly AKs.
**Used by.** (candidate: below +20 M/s expect AK/Pawn swarms → riot units and LLTs are the efficient answer; above +40
Armada medium is 80 % Warriors, Cortex is Thud/Storm)

### K-barb-response
**Claim.** Counter-building: for each of its roles BARb compares enemy metal in listed roles against its own metal in
the responding role; if `enemyMetal × ratio ≥ ownMetal` and the role is under `max_percent` of army cost, candidates of
that role get weight `enemyMetal/(ownMetal+1) × importance × (tableProb + 30)`, which dwarfs the ≤ 1.0 table weights.
A chosen response is repeated `num_batch` times. Enemy roles are whatever BARb's own `behaviour.json` assigns to our unit
types, counted from units it has seen. Medium highlights: riot vs raider 0.85/100; raider vs raider 1.0/85; skirmish vs
riot 1.0/35; assault vs static 5.0/25; anti-air vs air 0.75/150. Easy: anti-air only.
**Status.** supported (2026-09-19)
**Evidence.** `config/response.json:9-109`, `config/easy/response.json:33-44`;
`src/module/MilitaryManager.cpp:1257-1273`, `src/module/FactoryManager.cpp:1694-1702`.
**Would be wrong if.** Against a pure Pawn/AK army, medium Armada built no Warriors before income tier 3.
**Used by.** (candidate: our H-PROD-BATCH raiders are read as "raider" → Armada medium answers with Warriors, Cortex
(no T1 bot riot) with more AKs; a rocket/skirmisher-heavy mix is read as "skirmish" and triggers nothing on medium)

### K-barb-attack-gate
**Claim.** Non-raider fighters gather in a defend group at base. The group is promoted to an attack task when its power
≥ `max(quota.attack, threat of the second-strongest k-means cluster of known enemies)` or when an attack task already
exists (then promotion is immediate — reinforcements trickle). An attack task aborts when its power falls below
`quota.attack`. It targets the nearest reachable enemy cluster (distance scaled so clusters near BARb's base look closer)
whose threat is below `attackPower × 0.8/mod`, and within it the nearest non-mobile unit. With no valid target it walks
to a "front" position, else home.
**Status.** supported (2026-09-19), version caveat.
**Evidence.** `src/module/MilitaryManager.cpp:1688-1714, 1380-1397, 632-635`;
`src/task/fighter/DefendTask.cpp:107-118`; `src/task/fighter/AttackTask.cpp:99-107, 238-333, 346-400`;
`src/unit/enemy/EnemyManager.cpp:575-581`.
**Would be wrong if.** BARb launched its first non-raider attack with fewer than ~8 Rockos / 6 Thuds worth of units
(power 40) on medium.
**Used by.** (candidate: after we destroy an attack group below power 40 it dissolves and BARb regathers at base — that
is the window to push; candidate: keep our army in ≥ 2 visible blobs to raise its gate — weak, see Open questions)

### K-barb-retreat-random
**Claim.** The fighter retreat threshold is drawn once per game, uniformly between the two numbers of
`retreat.fighter`: medium [0.55, 1.0], easy [0.1, 1.0], hard [0.5, 1.0] (the JSON comment calls them "default, modifier",
the code uses them as min, max). A damaged unit under the threshold retreats if it has no visible target, the target is
out of range, or local threat × 2 > its power; otherwise it finishes the shot and retreats when idle. Under 20 % it
always retreats. It goes to the nearest "haven" by its factories (or a mobile repairer), requests a HIGH-priority repair
task, and returns to duty only at > 98 % health.
**Status.** supported (2026-09-19), version caveat; RNG is seeded from wall-clock time (`src/CircuitAI.cpp:1716-1718`).
**Evidence.** `src/module/MilitaryManager.cpp:246-251, 284`; `src/task/fighter/FighterTask.cpp:107-159`;
`src/task/RetreatTask.cpp:90-92, 132-151, 165-188`; `config/*/behaviour.json` retreat lines in the table.
**Would be wrong if.** Across 24 medium matches, the health at which BARb units turn back were the same in every match.
**Used by.** (candidate: outranging chip damage (rockets, LLT, artillery) removes its units from the field for a long
walk + repair; chase retreating units — they path home through low-threat cells and do not fight back unless attribute
`ret_fight`; expect large match-to-match variance in how "brave" BARb looks)

### K-barb-base-defence-clock
**Claim.** Static base defence is built on a fixed clock from `porcupine.base` (medium: LLT 120 s, LLT 240 s,
beamer/twin-LLT 420 s, AA rocket 520 s, HLT 760 s, flak 1200 s, ...), plus one LLT chained behind each T1 factory, plus
per-extractor-cluster defence capped at `amountFactor × income` metal (≈ 43 × income on a 14×14 map) which starts only
after minute 5, or income > 10, or any enemy mobile threat seen. Medium also puts a beamer/twin-LLT in front of 25 % of
adv. solars.
**Status.** supported (2026-09-19)
**Evidence.** `config/medium/build_chain.json:13-27, 76-84, 136-143, 179-208`; `script/medium/manager/military.as:36-44`;
`src/module/MilitaryManager.cpp:373-387, 705-780`.
**Would be wrong if.** A scout at minute 6 found BARb medium's base with more than ~3 light turrets and no raids had
reached it.
**Used by.** (candidate: before 12:40 its base holds no HLT; its outer extractors get defence only in proportion to income)

### K-barb-commander
**Claim.** The commander is role `builder`. After 180 s (420 s hard) and once more than 2 builders exist, it only takes
tasks within 1000 elmos of base (900 when enemy influence at its position ≥ 7, with hysteresis, or when the enemy has any
air). It retreats to a haven below its builder threshold (medium: random 0.89-1.0 of health) and resumes when the
threshold is regained and no enemy influence remains. It takes a "combat" task only when a target exists for which its
power × 1.5 suffices, and D-guns via a separate action. It never joins attacks; no morph/upgrade modules are configured.
**Status.** supported (2026-09-19), version caveat.
**Evidence.** `config/medium/commander.json:12-25`; `src/module/BuilderManager.cpp:938-989, 265-268`;
`src/task/RetreatTask.cpp:180-186`; `src/task/fighter/FighterTask.cpp:61-64`.
**Would be wrong if.** BARb's commander was seen more than ~1200 elmos from its start point after minute 3 (easy/medium).
**Used by.** (candidate: a commander snipe has a known address — mirrored start ± 1000; it will be among LLTs and
retreating/repairing units, and D-gun makes small-unit rushes poor; heavier, outranging units are the tool)

### K-barb-economy-rules
**Claim.** Economy flags come from storage fractions (script): metal empty < 20 %, full > 80 % (hard 99 %); energy empty
< 20 %; energy stalling = empty or (income < pull and stored < 60 %) (first 2 min: < 30 %); energy full > 88 %.
Builders: if stalling → energy at HIGH/NOW priority; else extractor upgrade (≤ `mex_up` at once, only when metal income
> 10), else new extractor (while under `mex_max` and not metal-full at > 100 income), else **converter** when energy full
and metal not full (≤ 2 converter tasks), else reclaim. An idle builder with no task builds energy first. Energy type =
highest-tech generator whose (metal, energy) income conditions hold and whose count is under its random limit; the
metal-income side is multiplied by a factor ramping 1→15 over 5-60 min (medium). New factory / nano turret when metal
income exceeds what existing factories consume (× `production` multipliers), otherwise "energy required" is raised first.
Fighters are only queued when metal is "excessed" and not metal-stalling.
**Status.** supported (2026-09-19), version caveat for C++.
**Evidence.** `script/medium/manager/economy.as:22-37`; `src/module/EconomyManager.cpp:1059-1191, 1283-1375,
1527-1539, 1992-2001`; `src/module/BuilderManager.cpp:1402-1414`; `src/module/FactoryManager.cpp:1480-1486`;
`config/medium/economy.json:10-16, 32, 55, 79, 89`.
**Would be wrong if.** BARb medium was seen with converters while its energy storage was under half.
**Used by.** (candidate: same ordering we converged on in K-eco-judge-energy-by-storage; adopt its 0.2/0.6/0.88 storage
fractions as tested defaults)

### K-barb-constructors
**Claim.** Constructor count is not fixed: a factory builds another constructor whenever total mobile build power <
`buildpower × min(avg metal, avg energy income)` (0.6 easy/medium, 1.2 hard), using the planning build speeds in
`behaviour.json` (T1 con 5, commander 5, T2 con 10), an active factory skips the request half the time; hard caps per type
(`limit` 20 medium, 25 easy for T1 cons). At +10 M/s medium wants 6 build power = commander (5) + 1 con; at +20, commander
+ 2 cons. Medium is therefore constructor-poor; hard has twice the budget.
**Status.** supported for the formula; the counts are conjectured arithmetic.
**Evidence.** `src/module/FactoryManager.cpp:1038-1072`; `config/medium/economy.json:67`;
`config/medium/behaviour.json:141-152, 384-388`.
**Would be wrong if.** BARb medium fielded 5+ T1 constructors at +15 M/s.
**Used by.** (candidate: killing one or two medium constructors is a large fraction of its expansion capacity; they
retreat at ≥ 89 % health, so even a scratch sends them home)

### K-barb-tier2
**Claim.** After the first factory, easy/medium choose the next by `importance[1]`; T1 labs have 0.0 on medium, so its
second factory is a T2 lab (armalab/coralab 0.7-0.8 × 88 % reach beats armavp/coravp × 38 %), built when spare metal
income covers it, or forced at the "switch time" every 900-1200 s. Once a non-T1 factory exists, T1 factories are
"inactive": they build only constructors (when the build-power budget asks) and units marked `rare`. T2 Armada medium rolls 44 % T2 constructors
at tier 0 (Cortex 1 %, relying on its opener's three). T2 units need `require_energy` (fallback to tier 0 when energy
empty). Extractor upgrades start once a builder that can build a better extractor exists, one at a time on easy/medium.
**Status.** supported for each rule; "second factory is the T2 bot lab around minute 10-20 on Quicksilver" conjectured.
**Evidence.** `config/medium/factory.json:54-73, 295-309`; `src/unit/FactoryData.cpp:86-121`;
`src/module/FactoryManager.cpp:1472, 1636-1641`; `script/medium/manager/factory.as:89-114`;
`src/module/EconomyManager.cpp:1067-1124, 1489-1542`.
**Would be wrong if.** BARb medium was seen running two T1 labs, or still producing Pawns/AKs in volume after its T2
lab completed.
**Used by.** (candidate: the minutes after its T2 lab finishes are a production trough — T1 output stops, T2 opener
starts with a constructor)

### K-barb-information
**Claim.** Without `cheating`, BARb knows only what entered its LOS or radar. A seen enemy is remembered at its last
position (and keeps stamping threat there) until BARb has LOS on that spot and the unit is absent, or 20 min pass after
it left radar. Shots from unseen sources create a "fake" enemy at the estimated origin (1 min for mobile, 20 min for
static weapon types). The threat map is rebuilt from scratch several times a second (no time decay): each remembered
armed enemy adds `threat × (1 − 0.5·d/r)` over a disc of radius weapon range + slack; threat scales with √health.
Scouts cycle metal clusters, farthest first, skipping clusters with threat ≥ 1. It has no start-position assumption:
"enemy position" is the centroid of known enemy clusters, initially the map centre.
**Status.** supported (2026-09-19), version caveat.
**Evidence.** `src/map/MapManager.cpp:75-205`; `src/unit/enemy/EnemyManager.cpp:52-53, 129-146, 190-214, 560-613`;
`src/CircuitAI.cpp:1609-1635`; `src/map/ThreatMap.cpp:374-408`; `src/module/MilitaryManager.cpp:505-517, 987-1054`.
**Would be wrong if.** BARb's first raid went straight to our base with no scout or prior contact.
**Used by.** (candidate: killing its single scout (medium: one Pawn/AK with the scout job, or a Flea) delays every
raid, because raid targets must be known; candidate: static defence it has seen keeps deterring even if we reclaim it,
until it re-scouts)

### K-barb-wall-decoy
**Claim.** The start-up script un-ignores dragon's teeth and fortification walls and gives them threat 0.01, so they are
legal targets for `ANTI_STAT` squads; with fire state 3 its units also shoot them in passing. armdrag costs 8 metal and
has 2800 health. A line of enemy dragon's teeth nearer to BARb's squad than our real buildings should be chosen as the
squad's target (nearest non-mobile unit in a permissible cluster) and absorb its fire.
**Status.** conjectured (2026-09-19) — the un-ignore and the targeting rule are read (`script/common.as:184-208`,
`src/task/fighter/AttackTask.cpp:276-325`); that the combination really makes squads stop to chew walls is my inference.
**Evidence.** As above; `upstream/Beyond-All-Reason/units/` armdrag: metalcost 8, health 2800.
**Would be wrong if.** BARb squads walked past a row of our dragon's teeth to reach extractors behind it.
**Used by.** (candidate: 3-5 dragon's teeth on BARb's side of each outpost, under LLT cover)

### K-barb-builder-safety
**Claim.** A builder rejects any non-urgent task whose site has threat above the builder's own power while enemy
influence dominates there, and any task it cannot path to at acceptable threat cost. Builders pick the task minimising
`pathCost / (priority+1)²`.
**Status.** supported (2026-09-19), version caveat.
**Evidence.** `src/module/BuilderManager.cpp:1306-1391`.
**Would be wrong if.** BARb constructors kept walking into the range of a turret they had already seen.
**Used by.** (candidate: one visible turret/unit group near a contested cluster denies that cluster to BARb's builders
for as long as it is remembered)

---

## Worth borrowing (ranked by value for effort on our data)

Our inputs: snapshot twice a second (economy, own units with position/health/idle, visible enemies), static unit defs,
metal spots.

1. **Threat-gated targeting and expansion.** *What:* never assign a squad to a target whose local threat ≥ squad power ×
   factor; never send a constructor to a spot whose threat exceeds its own strength. *Where:*
   `src/task/fighter/RaidTask.cpp:283-300`, `AttackTask.cpp:262-274`, `BuilderManager.cpp:1306-1345`. *Effort:* low once
   (2) exists. Directly addresses K-army-piecemeal-midmap and outposts walking into raids.
2. **Threat map with enemy memory.** *What:* grid of 64-128 elmo cells; each remembered armed enemy stamps
   `power × (1 − 0.5 d/r)` over radius range + slack; remember unseen enemies at last position until we see the spot
   empty. Power formula is one line from unit defs (`CircuitDef.cpp:621-623`). *Where:* `src/map/ThreatMap.cpp`,
   `src/map/MapManager.cpp:75-150`. *Effort:* low-medium; a 112×112 float grid rebuilt twice a second is trivial in Rust.
   We lack LOS maps, so "seen empty" needs an approximation (own unit within its sight radius of the cell).
3. **Retreat and repair.** *What:* unit under health threshold with no target in range, or outnumbered locally, walks to
   a haven beside the lab/nano turrets; returns at ~98 %. *Where:* `src/task/fighter/FighterTask.cpp:107-159`,
   `src/task/RetreatTask.cpp`. *Effort:* low (health is in the snapshot; nano turrets repair automatically). Use a fixed
   threshold (~0.4-0.5), not BARb's random one.
4. **Economy flags and the energy ladder.** *What:* storage-fraction flags (0.2 / 0.6 / 0.88), converters only when
   energy full and metal not full, generator tier chosen by (metal, energy) income conditions, constructor count =
   k × min(metal, energy income), factory/nano when income exceeds factory consumption. *Where:*
   `script/medium/manager/economy.as:22-37`, `config/hard/economy.json:9-28, 99`,
   `src/module/EconomyManager.cpp:1283-1375, 1949-2001`. *Effort:* low; replaces several unexamined constants
   (H-ECO-ADV-SOLAR, H-ECO-MORE-LABS, constructor formula in H-PROD-BATCH).
5. **Build chains.** *What:* "when X finishes, queue Y at offset": LLT behind each factory, defence per extractor
   cluster with a metal budget proportional to income, AA next to fusion when enemy air was seen. *Where:*
   `config/hard/build_chain.json`, `src/task/builder/BuilderTask.cpp:215-220`. *Effort:* low.
6. **Gather-then-attack with a power gate and merge.** *What:* squads form at home until power ≥ max(floor, known enemy
   group threat); squads within 1000 elmos and similar speed merge; regroup on the unit nearest the leader when
   stretched. *Where:* `DefendTask.cpp:100-130`, `AttackTask.cpp:48-75`, `SquadTask.cpp:228-275`. *Effort:* medium;
   needs (2) for enemy group threat. Addresses K-army-piecemeal-midmap. Do **not** copy the "promote immediately once an
   attack exists" rule — that is BARb's own trickle bug.
7. **Shared task pool with cost = path / (priority+1)².** *Where:* `BuilderManager.cpp:1319-1391`. *Effort:* medium; we
   already have `Brain::jobs`. Worth it when we add repair, reclaim and assist jobs.
8. **Response table.** *What:* counter-composition from enemy metal per role. *Where:* `config/response.json`,
   `MilitaryManager.cpp:1257-1273`. *Effort:* medium (needs a role label per unit def — can be lifted from
   `config/hard/behaviour.json`). Value is low until we field more than three unit types.
9. **`block_map.json` building blockers.** *What:* per-building-class keep-out rectangles/circles (factory exit lane
   `yard [8, 20]` ×16 elmos, explosion-radius spacing for fusions, which classes may overlap which). *Where:*
   `config/block_map.json:17-75`, `src/terrain/BlockingMap.*`. *Effort:* high for the general system. Cheap subset worth
   taking now: keep a 320-elmo lane in front of every lab free, and space wind/solar by one footprint.

---

## Open questions

1. **Does the 1.6.28 binary behave like the 1.6.30 source for `ANTI_STAT`, the retreat draw and the promote rule?**
   Settle by replay/observation: does a BARb squad path to our army or to our buildings when both are known; or obtain
   the CircuitAI tag for 1.6.28 (needs network — not done).
2. **Which factory does it actually build, and when does T2 arrive?** BARb logs nothing after
   "BARbarIAn 1.6.28 (1) Initialized!" (130 lines total per match, all start-up). The `FACTORY_CHOICE` log is compiled
   out (`FactoryManager.cpp`, `#ifdef FACTORY_CHOICE`). Settle from our side: log the def names of visible enemy
   buildings per minute in `bot.log` (first sighting of `*lab`, `*vp`, `*alab`, `*nanotc`, `*fus`).
3. **Exact power values.** Mine are hand-computed from unit files with default-armour damage; the code may use a
   different damage class or include area-damage custom params. Settle: no log available (`THREAT` log is `#if 0`,
   `CircuitDef.cpp:627`); replicate the formula in our bot against the same unit defs and check the predicted "4 raiders
   = first squad" against first-contact group sizes.
4. **Is the easy faction split really corak?** Settle: 24 easy matches with `[OPTIONS]{profile=easy; disabledunits=corak;}`
   as Armada, or a mirror-matchup arena option (already noted in K-opp-faction-asymmetry).
5. **Does `random_seed` work?** The C++ reads an AI option `random_seed` (`src/CircuitAI.cpp:1716-1718`) although
   `AIOptions.lua` has it commented out. If the engine passes unknown option keys through, `[OPTIONS]{random_seed=N}`
   would fix the retreat-threshold draw, solar limits and factory dice — valuable for variance reduction. Settle: run
   two matches with the same seed and compare BARb's build order.
6. **Dragon's-teeth decoy (K-barb-wall-decoy).** Settle: one probe match with 5 armdrag in front of an outpost; watch
   whether raid squads stop at them.
7. **How much of medium's win is the raid economy damage versus its main army?** Settle: medium batch with raiders
   disabled via `disabledunits` (list in K-barb-medium-vs-easy).
8. **`hard` was read only where it differs structurally** (scripts, economy, behaviour header, armlab table). Its
   `response.json`, `build_chain.json`, `block_map.json` and per-unit roles were not analysed; do that before the first
   hard batch.
9. **Second-largest-group gate (K-barb-attack-gate).** Whether splitting our army into two visible blobs measurably
   delays its first attack is untested and may be swamped by the "attack task already exists" shortcut. Low priority.
