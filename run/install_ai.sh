#!/bin/sh
# Build the AI shim and install it where an engine discovers skirmish AIs:
# <data dir>/AI/Skirmish/<shortName>/<version>/{AIInfo.lua,libSkirmishAI.so}
# usage: install_ai.sh [DATA_DIR]   (default: the arena's run/data)
# Files are replaced by rename, never rewritten in place: a running game keeps the library it
# loaded. Overwriting a mapped .so crashes the process that has it loaded.
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
dest="${1:-$root/run/data}/AI/Skirmish/BarBots/0.1"
cargo build --release --manifest-path "$root/Cargo.toml"
mkdir -p "$dest"
for pair in "crates/ai-shim/data/AIInfo.lua:AIInfo.lua" "target/release/libai_shim.so:libSkirmishAI.so"; do
    cp "$root/${pair%%:*}" "$dest/.${pair##*:}.new"
    mv -f "$dest/.${pair##*:}.new" "$dest/${pair##*:}"
done
echo "installed to $dest"
