#!/usr/bin/env python3
"""Smoke-test probe: listen on the engine's autohost UDP port, print events, /kill after N seconds."""
import socket, struct, sys, time

NAMES = {0: "SERVER_STARTED", 1: "SERVER_QUIT", 2: "SERVER_STARTPLAYING", 3: "SERVER_GAMEOVER",
         4: "SERVER_MESSAGE", 5: "SERVER_WARNING", 10: "PLAYER_JOINED", 11: "PLAYER_LEFT",
         12: "PLAYER_READY", 13: "PLAYER_CHAT", 14: "PLAYER_DEFEATED", 20: "GAME_LUAMSG",
         60: "GAME_TEAMSTAT"}
port, run_secs = int(sys.argv[1]), float(sys.argv[2])
sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
sock.bind(("127.0.0.1", port))
sock.settimeout(1.0)
t0, engine, killed, luamsgs = time.time(), None, False, 0
while time.time() - t0 < run_secs + 20:
    if engine and not killed and time.time() - t0 > run_secs:
        sock.sendto(b"/kill", engine); killed = True
        print(f"{time.time()-t0:7.1f}s  -> sent /kill", flush=True)
    try:
        data, engine = sock.recvfrom(65536)
    except socket.timeout:
        continue
    ev, now = data[0], time.time() - t0
    if ev == 20:
        luamsgs += 1; continue
    detail = ""
    if ev == 60:
        team, frame = data[1], struct.unpack_from("<i", data, 2)[0]
        detail = f"team={team} frame={frame} ({frame/30/60:.1f} game-min)"
    elif ev in (4, 5, 13):
        detail = data[1:].decode(errors="replace")[:160]
    else:
        detail = data[1:].hex()[:40]
    print(f"{now:7.1f}s  {NAMES.get(ev, ev)}  {detail}", flush=True)
    if ev == 1:
        break
print(f"lua messages ignored: {luamsgs}")
