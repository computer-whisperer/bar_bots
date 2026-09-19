# `claude -p` as an in-match strategist — verified facts (2026-09-19, Claude Code 2.1.278)

Verified with a toy MCP server and live runs on the user's subscription login. Probe scripts were scratch files; the shapes
below are what worked.

## Command line that works
```
claude -p --model claude-opus-5 \
  --tools "" \                         # no built-in tools at all (no Bash, files, web)
  --strict-mcp-config --mcp-config '{"mcpServers":{"bar":{"type":"http","url":"http://127.0.0.1:PORT/mcp"}}}' \
  --allowedTools "mcp__bar__*" --permission-mode dontAsk \
  --setting-sources "" \               # no user/project settings, hooks, CLAUDE.md-driven config
  --system-prompt "<strategist prompt>" \   # replaces the default prompt
  --input-format stream-json --output-format stream-json --verbose
```
Run it from an empty directory. The init message confirmed: `tools = [mcp__bar__observe, mcp__bar__set_directive]` and nothing else.
- **Do NOT use `--bare`.** Its help text: auth is strictly `ANTHROPIC_API_KEY`; OAuth and keychain are never read — so it
  cannot run on the subscription. (A documentation agent recommended it; wrong for us.)
- The Agent SDK needs an API key; the CLI under subscription login does not. CLI it is.

## Warm sessions
One process, stdin kept open. Each turn is one line: `{"type":"user","message":{"role":"user","content":"..."}}`.
Output is JSON lines: `system/init` (session id, tools, MCP status), `assistant` (text and `tool_use` blocks), `user`
(tool results), `rate_limit_event`, and `result` closing each turn (`duration_ms`, `duration_api_ms`, `usage` with
`cache_read_input_tokens` / `cache_creation_input_tokens`, cumulative `total_cost_usd`, `session_id`).
Measured over four turns in one session: 8.3 s, 5.6 s, 7.2 s (after a 20 s idle), 3.5 s (no tool call); cache reads of
~2.7-3.2k tokens every turn; ~$0.01 per decision at list price; context overhead with a one-line system prompt ~3.4k tokens.
Closing stdin ends the process with exit 0. Not tested: behaviour near the context limit, behaviour when a usage limit is hit
(the stream carries `rate_limit_event` messages with a status and reset time — log them).

## What the MCP server must implement (streamable HTTP, plain JSON responses are accepted)
Requests seen, in order: `server/discover` (newer-protocol probe; an empty result was tolerated — answer with a JSON-RPC
method-not-found error), `initialize` (client sent protocolVersion `2025-11-25`; echo it back with `capabilities.tools` and
`serverInfo`), `notifications/initialized` (no id; reply 202), `GET /mcp` (reply 405 = no server-initiated stream),
`tools/list`, then `tools/call` with `params.name`, `params.arguments`, and `_meta` (tool-use id, progress token).
Tool results: `{"content":[{"type":"text","text":"..."}]}`. Tool names surface as `mcp__<server>__<tool>`.
An MCP server cannot push into the session; new information arrives as a tool result or as a user turn on stdin.
Limits (from documentation, not tested): `MCP_TIMEOUT` startup (30 s default), `MCP_TOOL_TIMEOUT`, `MAX_MCP_OUTPUT_TOKENS` (25k default).

## Subscription budget and how to check it
`run/claude_usage.py [--json] [CONFIG_DIR ...]` prints, per Claude Code config dir (default `~/.claude` and `~/.claude2`), the
5-hour window, the 7-day window and the per-model 7-day caps (e.g. Fable), with reset times. Method borrowed from
`~/workspace/prism-widgets`: OAuth access token from `<dir>/.credentials.json`, `GET https://api.anthropic.com/api/oauth/usage`
with `anthropic-beta: oauth-2025-04-20`. The token goes only to that endpoint and is never printed. A stale token comes back as
HTTP 429, not 401 — running that account's CLI once refreshes it.

Budget (user, 2026-09-19): two personal Max 20x subscriptions, `claude` (`~/.claude`) and `claude2` (`CLAUDE_CONFIG_DIR=~/.claude2`).
The user's main projects run Fable, which has its own cap of up to half the weekly allotment; the other half of each week is
free for Opus, Sonnet and Haiku here. The 5-hour window is rarely a constraint — watch it, but spend where it makes sense.
Check usage before and after any batch of strategist games and put the delta in the ledger. Prefer the account with more
7-day headroom (on 2026-09-19: `claude` at 31%, `claude2` at 0%).

### Extra usage (paid credits) — we do not spend it
Aim (user, 2026-09-19): weekly allotments only. Anthropic has been known to move heavy `claude -p` users onto extra-usage
credits; we do not know the heuristics, so watch the numbers rather than guess.
- `claude` has extra usage ENABLED ($200 monthly limit; $13.16 used when first read on 2026-09-19, origin not attributable —
  the one Opus game before that reading reported `isUsingOverage: false` on every rate-limit event).
- `claude2` has extra usage NOT enabled and never had: it cannot be charged beyond the plan, only blocked. **Run strategist
  sweeps on `claude2`.**
- Before a strategist batch: `run/claude_usage.py --snapshot run/usage-<label>.json`. After (and during long sweeps):
  `run/claude_usage.py --since run/usage-<label>.json` — it prints the 7-day and extra-usage deltas and exits 3 if extra
  usage rose on any account. A rise means stop and tell the user.
- Live tripwire (to build): every `rate_limit_event` in the stream carries `isUsingOverage`, `overageStatus` and `status`;
  the strategist driver must stop sending turns the moment `isUsingOverage` is true or `status` is not `allowed`.
