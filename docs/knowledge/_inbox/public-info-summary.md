# Public-information pass: summary (2026-09-19)

67 entries written: `openings.md` 9, `tier2.md` 9, `units.md` 13, `scouting.md` 6, `maps.md` 8, `mechanics.md` 12,
`_inbox/public-info.md` 10 (for economy / army / game-rules). Nothing was tested in the arena. "supported" in these files
always means "read in the local game source or the local map archive", never "played".

## How good the sources were
Written, specific BAR strategy material is thin. The official site's most useful guides (openings, when to go T2, unit
composition, unit control) are video-only pages with no text. What could be read:
- beyondallreason.info official text guides (economy, starter, radar, advanced mechanics, reclaim, command pages, the
  Quicksilver map page). Undated; the map page says updated 2026-06-26.
- masterbel2.wordpress.com (community coach; principles rather than numbers; marked work in progress).
- crdhq.com "Creed of Champions" articles (2026). Treat as low reliability: generic, internally inconsistent in unit naming
  (it calls a chokepoint static defence "pounders" — Pounder is a Cortex vehicle in the current game — and states "Grunts
  beat Thugs"), and one rule is vacuous (see contradictions). Useful mainly for the T2 gate numbers, which match the
  official guide's.
- roguel1kegaming.com construction-turret analysis (one number disagrees with local files).
- bar-rts.fandom.com refused the fetch (HTTP 402). Reddit/Steam/Discord produced nothing specific through search.
Because of that, most numbers in the entries come from the local unit files, and several of the most useful "claims" are
arithmetic on those files rather than community wisdom. They are labelled as such.

## The ten claims that matter most against BARb medium
1. **K-open-quicksilver-is-a-wind-map / K-eco-energy-per-metal.** Wind is 3-17 (avg 12.7) on Quicksilver; wind gives ~2.5x the
   energy per metal of solar and beats advanced solar and even fusion per metal. Our brain builds only solars. Cheapest
   large economy gain available, and every other item below needs energy.
2. **K-t2-gate-thresholds + K-eco-t2-is-the-public-answer-to-the-ceiling.** Community gate is +20-30 M/s and +500 E/s. We
   reach ~+38 M/s at minute 17 without ever teching; by public standards we are 5+ minutes late, which matches "loses at 16-20".
3. **K-t2-moho-first.** Advanced extractors are 4x per spot for 620-640 M; five upgrades roughly double our T1 income, each
   repaying in ~100 s. This is what T2 is for; the T2 army is secondary.
4. **K-t2-needs-build-power / K-t2-who-builds-what.** The advanced lab is 25000 build time: 5 minutes for one constructor, under
   a minute with commander + 2 constructors. A constructor (not the commander) must start it. A naive "one builder builds the
   T2 lab" rule will look like it never finishes.
5. **K-mech-build-power-ratio.** ~200 build power per +5 M/s: at +38 M/s that is ~1500. Construction turrets (230 M, 200 build
   power, reach 400) are the cheapest way; our brain builds none.
6. **K-mech-reclaim-values.** Wrecks hold 60% of unit metal. Our mid-map trades (K-army-piecemeal-midmap) leave thousands of
   metal on the ground that we never collect; rez bots are 130 M with 200 build power.
7. **K-units-rockets-outrange-llt + K-scout-sight-shorter-than-range.** Rocket bots (475) outrange light towers (430/435) and the
   commander, but see only 380 — they need a spotter, and a front line against raiders. Turns "walk in and trade" into
   "siege from range".
8. **K-mech-converter-threshold / K-mech-converter-rates.** Converters only run above 75% stored energy, and cost ~220-540 M
   per +1 M/s versus ~25 M (free spot) or ~100 M (advanced extractor). Also flags a probable bug-by-design: our 80% trigger
   sits above the level the converters hold energy at.
9. **K-mech-dgun.** D-gun (range 250, 500 E, one-shots anything) never fires unless ordered. A commander that D-guns raiders at
   home is a large free defence; ours presumably never uses it.
10. **K-scout-radar-tower-is-cheap / K-scout-what-to-look-for.** Radar is 60 M for radius 2100. Whether it helps depends on the
    shim exposing radar contacts; if it does, H-ARMY-DEFEND and wave timing get minutes of warning.

Runner-up: **K-maps-features-are-early-energy** — trees on Quicksilver hold 250 E each and the opening is energy-bound.

## Contradictions between sources
- Wind threshold: official economy guide "> 7", official starter guide "> 10", crdhq "> 8 wind, < 5 solar". Pure metal
  break-even from local costs is ~5.2. All agree Quicksilver (12.7) is a wind map.
- Advanced solar output: official guide 75 E/s; local `armadvsol.lua`/`coradvsol.lua` 80.
- Construction turret cost: roguel1kegaming 250 M; local `armnanotc.lua` 230 M.
- Reclaim value: official economy guide "1/3 to 2/3 of metal cost"; local `alldefs_post.lua` wreck 0.6, heap 0.25.
- T2 entry cost: "~3000" (crdhq) vs "~4000 including first extractors" (official) — consistent once itemised (K-t2-entry-cost).
- T2 gate: official "+20 M / +500 E with 1000-2000 banked"; crdhq "+30/+500, or +20/+500 with 1000-1500 banked".
- Assist vs second factory: crdhq "one factory with nano turrets beats two"; roguel1kegaming "beyond ~3 assisting builders a
  second T1 factory is more efficient". Reconciled in K-open-one-factory-plus-assist.
- crdhq "build converters when energy income exceeds 2x metal income" is always true in BAR and therefore not a rule.

## Tension with our existing entries and heuristics
- **H-ECO-OPENING / H-ECO-ADV-SOLAR vs wind.** No existing knowledge entry covers generator choice; the heuristics are marked
  unexamined. Public info says they are wrong for this map.
- **K-eco-t1-ceiling** is not contradicted, but reframed: public advice says the ceiling is expected and the fix is timing T2
  at +20-30 M/s, far earlier than minute 17.
- **K-army-fighters-before-constructors** vs guides that list constructors before the factory's fighters. Not a real
  contradiction (guides assume no early raid pressure); reconciled in K-open-constructors-early.
- **H-ECO-CONVERT-SURPLUS (80%)** vs the 75% converter threshold (K-mech-converter-threshold): after the first converters the
  trigger may rarely fire. Needs a log check, not a belief change.
- **H-ECO-ENERGY-BY-STORAGE (40%)** vs the public "keep energy above 50%". Minor.
- **H-COM-LEASH (900)** is stricter than community practice (commander as early combat/expansion unit). Given
  K-rules-commander-death-ends-game and our retired H-MVP-COM-EXPANDS evidence, ours is the safer reading; proposed only as a
  time-limited experiment.
- **[REVIEW 2026-09-19: WRONG — armlab.lua lists armham and corlab.lua lists corthud, which is what the slot resolves to; whether those units deserve the label "artillery" is a naming question, not a bug.]** **H-PROD-BATCH "artillery" slot:** the T1 bot labs have no artillery unit (K-units-t1-bot-roster); worth checking what that
  slot resolves to in code.
- **K-rules-mex-upgrade-in-place** gains web and gadget-description support but is still untested through the AI interface.

## Looked for and not found
- A written, step-by-step 1v1 build order for either faction (counts and timings). Only videos exist (official "Opening
  Build Guides" playlist, per-map).
- Metal/energy income benchmarks by game minute for 1v1. Only the T2 gate numbers.
- Any numeric commit/retreat rule for armies (force ratios), or concrete raid timings.
- Anything specific to Quicksilver Remake beyond the map page's data: no strategy notes, no metal-spot count, no typical
  factory choice. Geothermal vents on the map: unknown.
- Text of the official guides "When to go to T2", "Unit Composition vs Map size", "How to control units like a Pro" (video only).
- Written unit-by-unit counter tables. The role triangle is as specific as public text gets.
- Exact energy-stall semantics (proportional slowdown, which units switch off) in source — reported from guides only; the
  engine source was not read.
- Whether our shim exposes map wind limits, radar contacts, the manual-fire (D-gun), reclaim, repeat and builder-priority
  commands — out of scope here, but most candidate heuristics depend on it.

## Process notes
No source pushed toward running commands or downloading anything. The only non-web reading outside `upstream/` was the local
map archive `run/data/maps/quicksilver_remake_1.24.sd7`: `mapinfo.lua` and three feature definition files were extracted to
the session scratchpad to read wind, tidal and feature reclaim values.
