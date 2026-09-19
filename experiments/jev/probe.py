#!/usr/bin/env python3
"""Send scenario files to Jev and print the answers, latency and token usage.

usage: probe.py [--repeat N] SCENARIO.json [...]

The API key is read from the TYPESAFE_API_KEY environment variable, else from ~/.config/within-reason/jev.env
(never from this repository, which is public). A scenario file is a request body without the model:
{"about": "...", "opus_decided": "...", "state": {...}, "questions": {...}}; "about" and "opus_decided" are for us and
are not sent. API reference: https://docs.typesafe.ai/api.md
"""
import json, os, statistics, sys, time, urllib.request, urllib.error

URL = "https://api.typesafe.ai/v1/systemone"
KEY_FILE = os.path.expanduser("~/.config/within-reason/jev.env")

def api_key():
    key = os.environ.get("TYPESAFE_API_KEY")
    if not key and os.path.exists(KEY_FILE):
        for line in open(KEY_FILE):
            if line.startswith("TYPESAFE_API_KEY="):
                key = line.split("=", 1)[1].strip().strip('"\'')
    if not key:
        sys.exit(f"no API key: put TYPESAFE_API_KEY=... in {KEY_FILE}")
    return key

def ask(key, scenario):
    body = {"model": os.environ.get("TYPESAFE_DEFAULT_MODEL", "jev-latest"),
            "state": scenario["state"], "questions": scenario["questions"]}
    request = urllib.request.Request(URL, data=json.dumps(body).encode(), method="POST", headers={
        "Authorization": f"Bearer {key}", "Content-Type": "application/json", "User-Agent": "within-reason-jev-probe"})
    started = time.time()
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return json.load(response), time.time() - started
    except urllib.error.HTTPError as e:
        sys.exit(f"HTTP {e.code}: {e.read().decode(errors='replace')[:500]}")

def show(answer):
    kind = answer["type"]
    if kind == "noul":
        return f"yes-probability {answer['noul']:.2f}"
    ranked = sorted(answer["probabilities"].items(), key=lambda kv: -kv[1])
    spread = "  ".join(f"{name}={p:.2f}" for name, p in ranked[:4])
    picked = answer["choice"] if kind == "choice" else f"{answer['score']:.2f}"
    return f"{picked}  (confidence {answer['confidence']:.2f};  {spread})"

def main():
    args = sys.argv[1:]
    repeat = int(args.pop(args.index("--repeat") + 1)) if "--repeat" in args else 1
    files = [a for a in args if a != "--repeat"]
    key = api_key()
    for path in files:
        scenario = json.load(open(path))
        print(f"== {os.path.basename(path)}: {scenario.get('about', '')}")
        if scenario.get("opus_decided"):
            print(f"   Opus decided: {scenario['opus_decided']}")
        latencies = []
        for run in range(repeat):
            result, seconds = ask(key, scenario)
            latencies.append(seconds)
            if run == 0:
                for question, answer in result["answers"].items():
                    print(f"   {question:<28} {show(answer)}")
                print(f"   usage: {result['usage']}  model: {result['model']}")
        print(f"   latency: median {statistics.median(latencies)*1000:.0f} ms over {repeat} call(s), "
              f"min {min(latencies)*1000:.0f}, max {max(latencies)*1000:.0f}")

main()
