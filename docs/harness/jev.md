# Jev (TypeSafe AI) — notes for when we slot it in

Access granted 2026-09-19 (waitlist). Not integrated; the order is Opus observe-and-direct first, Jev after (see roadmap in
`docs/README.md` once written there, and DESIGN.md). Credentials come from the environment only — this repository is public.

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
