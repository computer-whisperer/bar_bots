#!/bin/sh
# Make the AI selectable in a local BAR install by symlinking it into the install's data dir
# (survives engine updates; rebuilds via install_ai.sh are picked up automatically).
# usage: link_install.sh [BAR_DATA_DIR]
set -eu
run=$(cd "$(dirname "$0")" && pwd)
bar=${1:-"$HOME/.local/state/Beyond All Reason"}
"$run/install_ai.sh"
mkdir -p "$bar/AI/Skirmish"
ln -sfn "$run/data/AI/Skirmish/BarBots" "$bar/AI/Skirmish/BarBots"
echo "linked $bar/AI/Skirmish/BarBots"
