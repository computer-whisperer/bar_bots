#!/usr/bin/env python3
"""Show Claude subscription usage for one or more Claude Code config dirs.

usage: claude_usage.py [--json] [CONFIG_DIR ...]      (default: ~/.claude ~/.claude2, those that exist)

Same method as ~/workspace/prism-widgets: read the OAuth access token from <dir>/.credentials.json and
GET https://api.anthropic.com/api/oauth/usage. The token is sent only to that endpoint and never printed.
"""
import json, os, sys, urllib.request, urllib.error
from datetime import datetime, timezone

URL = "https://api.anthropic.com/api/oauth/usage"

def fetch(config_dir):
    try:
        with open(os.path.join(config_dir, ".credentials.json")) as f:
            oauth = json.load(f).get("claudeAiOauth", {})
    except FileNotFoundError:
        return {"error": "not logged in"}
    token = oauth.get("accessToken")
    if not token:
        return {"error": "not logged in"}
    request = urllib.request.Request(URL, headers={
        "Authorization": f"Bearer {token}", "anthropic-beta": "oauth-2025-04-20",
        "Content-Type": "application/json", "User-Agent": "bar_bots-claude-usage"})
    try:
        with urllib.request.urlopen(request, timeout=15) as response:
            body = json.load(response)
    except urllib.error.HTTPError as e:
        if e.code == 401:
            return {"error": "token expired (run that account's CLI once to refresh it)"}
        if e.code == 429:
            # Seen for an EXPIRED token as well as for real rate limiting (the endpoint is also polled by prism-widgets).
            return {"error": f"HTTP 429 (retry-after {e.headers.get('retry-after', '?')} s): rate limited, or the token is stale — "
                             "run that account's CLI once to refresh it"}
        return {"error": f"HTTP {e.code}"}
    except OSError as e:
        return {"error": str(e)}
    body["plan"] = oauth.get("rateLimitTier")
    return body

def resets_in(iso):
    if not iso:
        return ""
    left = datetime.fromisoformat(iso.replace("Z", "+00:00")) - datetime.now(timezone.utc)
    hours = max(left.total_seconds(), 0) / 3600
    return f"resets in {hours / 24:.1f} d" if hours >= 48 else f"resets in {hours:.1f} h"

def main():
    args = [a for a in sys.argv[1:] if a != "--json"]
    dirs = args or [d for d in (os.path.expanduser("~/.claude"), os.path.expanduser("~/.claude2")) if os.path.isdir(d)]
    results = {d: fetch(d) for d in dirs}
    if "--json" in sys.argv:
        json.dump(results, sys.stdout, indent=2); print(); return
    for d, usage in results.items():
        print(f"{d}  [{usage.get('plan') or '?'}]")
        if "error" in usage:
            print(f"  {usage['error']}"); continue
        for key, label in (("five_hour", "5-hour window"), ("seven_day", "7-day window")):
            window = usage.get(key) or {}
            if window.get("utilization") is not None:
                print(f"  {label:<22} {window['utilization']:5.1f}%  {resets_in(window.get('resets_at'))}")
        for limit in usage.get("limits") or []:
            name = ((limit.get("scope") or {}).get("model") or {}).get("display_name")
            if name and limit.get("percent") is not None:
                print(f"  {'7-day, ' + name:<22} {limit['percent']:5.1f}%  {resets_in(limit.get('resets_at'))}")

main()
