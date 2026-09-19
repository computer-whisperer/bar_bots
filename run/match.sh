#!/bin/sh
# Run one headless match with the bot process attached.
# usage: match.sh START_SCRIPT RUN_SECONDS [GAME_SPEED]
# Logs land in run/logs/: engine.log, bot.log, probe.log.
set -eu
run=$(cd "$(dirname "$0")" && pwd)
script=$(realpath "$1"); secs=$2; speed=${3:-}
"$run/install_ai.sh" >/dev/null
mkdir -p "$run/logs"
export BAR_BOTS_SOCKET="$run/logs/bar_bots.sock"
"$run/../target/release/bot" 2>"$run/logs/bot.log" &
bot=$!
echo $bot >"$run/logs/bot.pid"
python3 "$run/autohost_probe.py" 8453 "$secs" $speed >"$run/logs/probe.log" 2>&1 &
probe=$!
trap 'kill $bot $probe 2>/dev/null || true' EXIT
timeout $((secs + 90)) "$run/engine/spring-headless" --isolation --write-dir "$run/data" "$script" >"$run/logs/engine.log" 2>&1 || echo "engine exit=$?"
wait $probe || true
