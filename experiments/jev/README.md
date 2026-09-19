# Jev experiments

First contact with TypeSafe's Jev (`docs/harness/jev.md`). `probe.py` sends scenario files and prints answers, latency and token
usage; the API key comes from `TYPESAFE_API_KEY` or `~/.config/bar_bots/jev.env`, never from this repository.

Scenarios 01-03 are real decision points from the first Opus strategist game (`docs/transcripts/2026-09-19-opus-first.md`),
with the state rewritten as qualitative statements: Jev's documented weaknesses are arithmetic, counting, comparing quantities
and multi-hop reasoning, so the heuristics must digest numbers into judgments ("very low", "a large group") before asking.
Each file records what Opus decided at that moment, for comparison.

Questions to answer with these: does Jev agree with Opus where Opus was right (01, 03) and do better where Opus was wrong
(02)? How do answers move when one fact in the state changes? What is the real latency from this machine?
