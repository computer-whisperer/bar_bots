# Jev (TypeSafe AI) — notes for when we slot it in

Access granted 2026-09-19 (waitlist). Integrated 2026-09-21 as the pianist (`docs/design/2026-09-21-pianist.md`): `crates/jev` is
the client (`jev SCENARIO.json` reproduces the probe below), `crates/bot/src/brain/pianist/` the player of the keyboard.
Credentials come from the environment only — this repository is public.

What it is: a hosted "System One" model. A request carries a **state** (text or JSON) plus typed **questions**; every question is
answered in parallel, in isolation, in one pass, with probabilities. No text generation. Vendor figures: 70-500 ms end to end,
$0.042 per million input tokens, output free, options per Choice capped at 255, no image input.

Primitives: **Choice** (one of a defined set; distribution + confidence), **Noul** (probability that a condition holds; one per
label when several may apply; 0.5 means unsure, not "medium"), **Score** (position on described ordered levels; use comparable
per-item Scores for graded ranking).

Where the real documentation is: index `https://docs.typesafe.ai/llms.txt`; any page as Markdown by appending `.md`
(e.g. `/api.md`, `/primitives/choice.md`, `/confidence.md`, `/concepts/state.md`, `/patterns/fan-out.md`). Python and
JavaScript SDKs exist; from Rust we would use the HTTP API. A copy of TypeSafe's agent skill file (MIT) sits untracked at the
repo root as `JEV_SKILL.md`; it says to read the live docs before writing an integration, which has not been done yet.

Guidance from that file that shapes our design:
- Code owns the workflow; the model supplies judgments. Heuristics generate the legal options, Jev picks or scores them.
- Ask independent questions over the same state together, including speculative ones ("if we attack, where?") and consume
  only the branch that applies — one request per decision point, not a chain.
- Put named JSON fields in the state and reference them by path in the question; question IDs are not sent to the model.
- Always include a no-match option; the model cannot choose a candidate that was not offered.
- Confidence on Choice/Score is distribution concentration, not permission to act; thresholds must be tuned on our own data
  — the Opus game transcripts are the intended source of labelled decisions for that.
- Keep raw judgments reusable: score dimensions once, let code re-weight without re-asking.

## First contact (2026-09-19, `experiments/jev/`, model jev-1.13.0)
API facts confirmed by use: `POST https://api.typesafe.ai/v1/systemone`, `Authorization: Bearer $TYPESAFE_API_KEY`, body
`{model, state, questions:{id:{type: noul|choice|score, instructions, criteria}}}`; answers carry probabilities (and
`confidence` for choice/score). Limits from their docs: 64k context, 1,200 requests/min, 250k tokens/s, text only.
The key lives in `~/.config/within-reason/jev.env` (mode 600), never in this repository.
- **Latency from this machine: median 318-350 ms** for three questions over a ~650-730 token request (5 calls each,
  min 289, max 386). Cost per call about $0.00003.
- Three real decision points from the first Opus game, state rewritten qualitatively:
  | Scenario | Opus | Jev |
  |---|---|---|
  | 01 raiders kill extractors (4:55) | kept `defend`, wished for an escort order | `escort` 0.72 (defend 0.27); raid-is-main-threat 0.78 |
  | 02 launch or hold with 8 idle bots, enemy unscouted (9:39) | attacked mid-map; a 10-unit push hit the base 27 s later | `defend` 0.51, escort 0.37, **attack 0.03**, low confidence 0.34 |
  | 03 push arrives just after the army left (10:06) | recall and defend | `return_and_defend` 0.93; commander-in-danger 0.82; threat level "critical" 0.93 |
  Jev matched Opus where Opus was right, chose the order Opus had wished for in 01, and did not make Opus's mistake in 02.
  Three scenarios are an anecdote, not an evaluation.
- Sensitivity (scenario 02, only the scouting sentence changed): large-attack-likely-soon went 0.09 (enemy scouted tiny) /
  0.23 (unscouted) / 0.69 (scouted massing); `attack` rose to 0.30 and confidence fell to 0.19 when the enemy was tiny;
  `defend` firmed to 0.67 when it was massing. The derived question "would the base stay safe if the army left" barely moved
  (0.65 / 0.60 / 0.53) — consistent with their warning about multi-hop questions: ask about the facts, combine in code.
- Repeated identical requests differ slightly (escort 0.37 vs 0.31), so answers are not bit-stable; thresholds need margin.
- Their documented weaknesses (arithmetic, counting, comparing quantities, dates, multi-hop, distraction by unrelated state)
  mean the heuristics must hand Jev digested judgments ("metal income is very low", "a large group"), not raw numbers.

## The client (2026-09-21, `crates/jev`)
`jev::Client::from_env()` (the key file or `TYPESAFE_API_KEY`; the model `jev-latest` unless `TYPESAFE_DEFAULT_MODEL`),
`ask(&Request { state, questions })` blocking over `ureq` with rustls, 20 s timeout, two retries on 429 or 5xx waiting
`retry-after` (5 s at most), the versioned model and the usage in every `Response`. The key is never written by the crate.
- The three probe scenarios again (`cargo run -p jev -- experiments/jev/0*.json --repeat 3`, model jev-1.13.0): latency
  **median 145-174 ms** (min 127, max 339) against 318-350 two days earlier, on the same request sizes (630-730 tokens).
- **The same state and model do not give the same answers two days apart.** Scenario 01: `escort` 0.72 on 2026-09-19,
  `defend` 0.58 / `escort` 0.39 on 2026-09-21 (the top choice flipped). Scenario 02: `attack` 0.03 then 0.05,
  `sending_army_out_is_safe` 0.71 (was not asked then). Scenario 03: `return_and_defend` 0.93 then 0.96. Between calls
  minutes apart the wobble was a few hundredths (2026-09-19). Whatever the cause (the vendor's serving, or a wider
  spread than the first day showed), a decision layer needs hysteresis: the pianist holds a busy actor's course unless
  the winner beats "continue" by 0.15 (H-HANDS-SWITCH).
