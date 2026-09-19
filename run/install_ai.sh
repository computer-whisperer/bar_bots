#!/bin/sh
# Build the AI shim and install it where the engine discovers skirmish AIs:
# {write-dir}/AI/Skirmish/<shortName>/<version>/{AIInfo.lua,libSkirmishAI.so}
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
dest="$root/run/data/AI/Skirmish/BarBots/0.1"
cargo build --release --manifest-path "$root/Cargo.toml"
mkdir -p "$dest"
cp "$root/crates/ai-shim/data/AIInfo.lua" "$dest/"
cp "$root/target/release/libai_shim.so" "$dest/libSkirmishAI.so"
echo "installed to $dest"
