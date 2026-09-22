#!/bin/sh
# Install a copy of the AI into a local BAR install, making it selectable in the lobby.
# A copy, not a symlink: development builds and arena runs must never change what a game
# the user is playing has loaded. Re-run this to update the install.
# usage: install_to_bar.sh [BAR_DATA_DIR]
set -eu
run=$(cd "$(dirname "$0")" && pwd)
# The launcher moved its data from ~/.local/state/Beyond All Reason to ~/.local/share/BeyondAllReason (2026-09-22).
bar=${1:-"$HOME/.local/share/BeyondAllReason"}
"$run/install_ai.sh" "$bar"
# The lobby lists AIs without a friendly name only with "Simplified AI list" off, a checkbox of the Developer settings
# tab, which exists only when this file does (docs/harness/lobby.md).
touch "$bar/devmode.txt"
