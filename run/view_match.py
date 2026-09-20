#!/usr/bin/env python3
"""Open a recorded match in the web viewer (viewer/, format in docs/harness/record-format.md).

usage: run/view_match.py run/matches/<batch>/<NN> [--port N] [--bind ADDRESS] [--no-browser]

Serves viewer/ at / and the match directory at /match/, read-only, until interrupted. It listens on `::` by default:
every interface, IPv6 and IPv4, so the viewer can be opened from another machine on the network. That exposes the
match directory (logs, records, transcripts) to that network; `--bind 127.0.0.1` keeps it to this machine.
A batch directory is accepted too and means its match 00.
"""
import argparse, http.server, json, os, socket, sys, webbrowser

VIEWER = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "viewer")


class Handler(http.server.SimpleHTTPRequestHandler):
    match_dir = None

    def translate_path(self, path):
        # The base class resolves against `directory` and drops "..", so neither root can be escaped.
        if path.startswith("/match/"):
            self.directory = self.match_dir
            path = path[len("/match"):]
        else:
            self.directory = VIEWER
        return super().translate_path(path)

    def do_GET(self):
        if self.path.split("?")[0] == "/match/index.json":
            files = sorted(f for f in os.listdir(self.match_dir) if os.path.isfile(os.path.join(self.match_dir, f)))
            body = json.dumps({"dir": self.match_dir, "files": files}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        super().do_GET()

    def end_headers(self):
        self.send_header("Cache-Control", "no-store")  # a match still being played grows between reloads
        super().end_headers()

    def log_message(self, *args):
        pass


class Server(http.server.ThreadingHTTPServer):
    """Listens on an IPv6 address when given one, and then on IPv4 too (dual stack) where the system allows."""

    def __init__(self, address, handler):
        self.address_family = socket.AF_INET6 if ":" in address[0] else socket.AF_INET
        super().__init__(address, handler)

    def server_bind(self):
        if self.address_family == socket.AF_INET6:
            try:
                self.socket.setsockopt(socket.IPPROTO_IPV6, socket.IPV6_V6ONLY, 0)
            except OSError:
                pass  # IPv6 only, then
        super().server_bind()


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("match_dir")
    parser.add_argument("--port", type=int, default=8137, help="first port to try (default 8137)")
    parser.add_argument("--bind", default="::", help="address to listen on (default ::, every interface; 127.0.0.1 for this machine only)")
    parser.add_argument("--no-browser", action="store_true")
    args = parser.parse_args()
    match_dir = os.path.abspath(args.match_dir)
    if not any(f.startswith("record-") for f in os.listdir(match_dir)) and os.path.isdir(os.path.join(match_dir, "00")):
        match_dir = os.path.join(match_dir, "00")
    if not any(f.startswith("record-") and f.endswith(".jsonl") for f in os.listdir(match_dir)):
        sys.exit(f"{match_dir} holds no record-*.jsonl; records are written when the bot runs with WITHIN_REASON_RECORD=1 (the arena sets it)")
    Handler.match_dir = match_dir
    for port in range(args.port, args.port + 20):
        try:
            server = Server((args.bind, port), Handler)
            break
        except OSError:
            continue
    else:
        sys.exit(f"no free port in {args.port}-{args.port + 19}")
    everywhere = args.bind in ("::", "0.0.0.0")
    local = "127.0.0.1" if everywhere else args.bind
    url = f"http://[{local}]:{port}/" if ":" in local else f"http://{local}:{port}/"
    print(f"{match_dir}\n{url}   (ctrl-c to stop)")
    if everywhere:
        print(f"listening on every interface: from another machine, http://{socket.gethostname()}:{port}/")
    if not args.no_browser:
        webbrowser.open(url)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
