#!/usr/bin/env python3
"""Show Claude subscription usage for one or more Claude Code config dirs.

usage: claude_usage.py [--json] [--snapshot FILE | --since FILE] [CONFIG_DIR ...]
       (default dirs: ~/.claude ~/.claude2, those that exist)

--snapshot FILE saves the current readings; --since FILE prints what changed since that snapshot and exits with status 3
if any account's extra-usage (paid credits) figure went up. We aim to spend weekly allotments only: a rise in extra usage
during a sweep means stop.

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

def extra_usage(usage):
    """(enabled, used, limit) in currency units; used is 0.0 when extra usage was never enabled."""
    extra = usage.get("extra_usage") or {}
    scale = 10 ** (extra.get("decimal_places") or 2)
    used = (extra.get("used_credits") or 0) / scale
    limit = extra.get("monthly_limit")
    return bool(extra.get("is_enabled")), used, (limit / scale if limit else None)

def flag_value(name):
    if name in sys.argv:
        value = sys.argv[sys.argv.index(name) + 1]
        del sys.argv[sys.argv.index(name):sys.argv.index(name) + 2]
        return value
    return None

def main():
    snapshot_to, since = flag_value("--snapshot"), flag_value("--since")
    args = [a for a in sys.argv[1:] if a != "--json"]
    dirs = args or [d for d in (os.path.expanduser("~/.claude"), os.path.expanduser("~/.claude2")) if os.path.isdir(d)]
    results = {d: fetch(d) for d in dirs}
    if snapshot_to:
        with open(snapshot_to, "w") as f:
            json.dump({"taken_at": datetime.now(timezone.utc).isoformat(), "accounts": results}, f)
    before = json.load(open(since))["accounts"] if since else {}
    if "--json" in sys.argv:
        json.dump(results, sys.stdout, indent=2); print(); return
    spiked = False
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
        enabled, used, limit = extra_usage(usage)
        if enabled:
            print(f"  {'extra usage (PAID)':<22} {used:8.2f} of {limit:.2f} {(usage['extra_usage'].get('currency') or '')} this month")
        else:
            print(f"  {'extra usage (PAID)':<22} not enabled: this account cannot be charged beyond the plan")
        old = before.get(d)
        if old and "error" not in old:
            week = lambda u: (u.get("seven_day") or {}).get("utilization") or 0.0
            rise = used - extra_usage(old)[1]
            print(f"  since snapshot: 7-day {week(usage) - week(old):+.1f} points, extra usage {rise:+.2f}")
            if rise > 0:
                spiked = True
                print("  *** EXTRA USAGE ROSE: stop strategist runs on this account and tell the user ***")
    if spiked:
        sys.exit(3)

main()
