#!/usr/bin/env python3
"""Make an arena replay recorded before the GameType fix open in the BAR lobby.

Those replays name the game by its rapid tag ("byar:test") in their header script; the lobby cannot resolve a tag and
sits at "byar:test: 0%". This writes a copy whose header names the game in full, as the engine log of the match
reports it. The game stream itself is untouched.

usage: run/fix_replay.py run/matches/<batch>/<NN> [--out DIR]
"""
import glob, gzip, os, re, struct, sys

SCRIPT_SIZE_AT = 304  # DemoFileHeader: magic 16, version 4, headerSize 4, versionString 256, gameID 16, unixTime 8

def main():
    match = sys.argv[1]
    out = sys.argv[sys.argv.index("--out") + 1] if "--out" in sys.argv else os.path.join(match, "demos")
    log = open(os.path.join(match, "engine.log"), errors="replace").read()
    name = re.search(r'using game "([^"]+)"', log)
    if not name:
        sys.exit("engine.log does not say which game was used")
    for path in glob.glob(os.path.join(match, "demos", "*.sdfz")):
        if path.endswith(".fixed.sdfz"):
            continue
        raw = gzip.open(path).read()
        if raw[:15] != b"spring demofile":
            sys.exit(f"{path}: not a demo file")
        header_size = struct.unpack_from("<i", raw, 20)[0]
        script_size = struct.unpack_from("<i", raw, SCRIPT_SIZE_AT)[0]
        script = raw[header_size:header_size + script_size]
        fixed, count = re.subn(rb"(?im)^(\s*gametype\s*=\s*)[^;]*;", lambda m: m[1] + name[1].encode() + b";", script)
        if count != 1:
            sys.exit(f"{path}: expected one gametype line, found {count}")
        header = bytearray(raw[:header_size])
        struct.pack_into("<i", header, SCRIPT_SIZE_AT, len(fixed))
        target = os.path.join(out, os.path.basename(path)[:-5] + ".fixed.sdfz")
        with gzip.open(target, "wb") as f:
            f.write(bytes(header) + fixed + raw[header_size + script_size:])
        print(f"{target}\n  gametype -> {name[1]}")

main()
