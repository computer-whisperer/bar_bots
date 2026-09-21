# Game knowledge base

One file per topic. Each entry:

```
### K-<topic>-<slug>
**Claim.** One or two sentences, checkable.
**Status.** reported | conjectured | supported | refuted | retired  (date)
**Evidence.** batch-label/match (frame or minute), source file:line, or "reading only".
**Would be wrong if.** The observation that falsifies it.
**Used by.** Heuristic IDs from ../heuristics.md.
```

Statuses: `reported` = taken from an outside source (wiki, guide, another AI's code) and not checked by us — cite the source
and its date or game version, since balance changes; `conjectured` = our own inference; `supported` / `refuted` = tested in
the arena or verified in source we run; `retired` = was true, stopped mattering or holding, with the reason.

Scope every claim: opponent and tier, map, game version. Almost everything here was learned against BARb `easy` on
Quicksilver Remake 1.24 with game `byar:test` (test-31357) and is unverified anywhere else.

Topics: [game-rules](game-rules.md) · [economy](economy.md) · [army](army.md) · [opponents](opponents.md) ·
[opponents-barb](opponents-barb.md) · [openings](openings.md) · [tier2](tier2.md) · [units](units.md) · [scouting](scouting.md) ·
[maps](maps.md) · [mechanics](mechanics.md) · [jev](jev.md)

Agent-written files (2026-09-19: opponents-barb, openings, tier2, units, scouting, maps, mechanics, `_inbox/`) were spot-checked,
not fully reviewed. Checked and confirmed: BARb's ANTI_STAT script (`script/common.as`), `AttackTask.cpp:293`, the easy-profile
armpw/corak roles, the converter 0.75 level, the extractor collapse in v5-medium. Checked and wrong: the claim that bot labs
have no artillery unit. Treat any other entry as unverified until someone cites a check here or in the entry.

`_inbox/` holds entries proposed by research agents that have not been reviewed and merged into a topic file yet.
