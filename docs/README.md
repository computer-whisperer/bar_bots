# bar_bots documentation

Four kinds of knowledge with different lifetimes. Put a fact where its lifetime says it belongs.

| Where | What | Changes when |
|---|---|---|
| `knowledge/` | Claims about the game and opponents, each with evidence and a status | every batch can support, weaken or retire one |
| `heuristics.md` | Registry of the brain's rules: ID, code location, the claims it rests on, status | a rule is added, changed or retired |
| `experiments.md` | Ledger of arena batches: label, commit, setup, result, what it was testing | every batch |
| `harness/` | Engine, AI interface, lobby, arena and tooling facts | rarely; on engine or game bumps |

`../DESIGN.md` is the architecture (shim, protocol, bot process). It records decisions, not findings.

## The loop
1. **Observe.** Run a batch. Compare wins against losses using the per-minute status lines in `bot.log`
   (and, once rule-firing logs exist, which heuristics fired).
2. **Claim.** Write what you think is true as a knowledge entry with status `conjectured`, citing batch, match and frame.
   A claim must be checkable: say what observation would prove it wrong.
3. **Exploit.** Add or change a heuristic. Give it an ID, register it in `heuristics.md` with the claims it rests on.
4. **Verify.** Run a batch that could show the change did nothing. Record it in `experiments.md`.
   Move the claim to `supported`, or to `refuted` with the evidence. 12 matches is noise-level (±14 points); confirm with 24+.
5. **Retire.** When a claim stops holding (new opponent tier, new map, our own play changed the situation), mark it
   `retired` with the reason and retire or rewrite the heuristics that cite it. Never delete entries: why something stopped
   being true is knowledge.

Rules of the road: a heuristic without a registered claim is a guess — register the guess as `conjectured`. A batch without a
ledger line did not happen. Results that contradict a `supported` claim reopen it; say so in the entry rather than quietly
tuning around it.
