#!/bin/sh
# Install a copy of the AI into a local BAR install, making it selectable in the lobby.
# A copy, not a symlink: development builds and arena runs must never change what a game
# the user is playing has loaded. Re-run this to update the install.
# usage: install_to_bar.sh [BAR_DATA_DIR]
set -eu
run=$(cd "$(dirname "$0")" && pwd)
bar=${1:-"$HOME/.local/state/Beyond All Reason"}
# The AI was called BarBots until 2026-09-19; remove that install (a symlink in its earliest form).
rm -rf "$bar/AI/Skirmish/BarBots"
"$run/install_ai.sh" "$bar"
