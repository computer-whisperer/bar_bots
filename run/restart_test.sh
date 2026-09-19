#!/bin/sh
# Kill the bot mid-match and start a new one; the match must keep running and the shim must reconnect.
# Run alongside match.sh: restart_test.sh KILL_AFTER_SECONDS DOWN_SECONDS BOT_LIFETIME_SECONDS
set -eu
run=$(cd "$(dirname "$0")" && pwd)
sleep "$1"
kill "$(cat "$run/logs/bot.pid")" && echo "bot killed"
sleep "$2"
BAR_BOTS_SOCKET="$run/logs/bar_bots.sock" timeout "$3" "$run/../target/release/bot" 2>"$run/logs/bot2.log" || true
echo "second bot exited"
