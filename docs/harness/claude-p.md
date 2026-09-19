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
