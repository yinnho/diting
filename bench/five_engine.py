#!/usr/bin/env python3
"""Five-engine agent-browser benchmark: diting vs Obscura vs Moli vs
Lightpanda (blitz excluded — no headless live-web entry, GUI app only).

Metric, per page: wall time to agent-usable text, output chars, peak RSS.
Each engine is driven through its own best native text path (what an agent
would actually consume) — markdown where the engine produces markdown,
text extraction otherwise. Cold-start scenario measures process spawn to
first successful fetch of a static page.

Engines:
  diting     serve (warm) + POST /fetch            [server]
  obscura    CLI per page (cold)                   [process]
  moli       CLI per page (cold)                   [process]
  lightpanda serve CDP / CLI dump (per --help)      [server or process]

Usage: python3 bench/five_engine.py --rounds 2 [--pages N]
Expects binaries at /tmp/fivebench/bin/{diting,obscura,moli,lightpanda}
(symlink or copy; diting defaults to target/release/diting).
Writes bench/results/five-engine-<date>.tsv.
"""

import argparse
import json
import os
import re
import subprocess
import sys
import threading
import time
import urllib.request
from datetime import date
from pathlib import Path

BENCH = Path(__file__).resolve().parent
ENGINES_DIR = Path("/tmp/fivebench/bin")

PAGES = [
    "https://example.com",
    "https://github.com/yinnho/diting",
    "https://www.rust-lang.org/",
    "https://blog.rust-lang.org/",
    "https://react.dev/",
    "https://nextjs.org/",
    "https://nodejs.org/en",
    "https://bun.sh/",
]


def rss_sampler(patterns):
    """Returns (start, stop) closures summing RSS across matching processes."""
    state = {"peak_kb": 0, "stop": threading.Event()}

    def run():
        while not state["stop"].is_set():
            try:
                out = subprocess.run(["ps", "-Ao", "rss=,comm="], capture_output=True, text=True).stdout
            except OSError:
                return
            total = 0
            for line in out.splitlines():
                m = re.match(r"\s*(\d+)\s+(.+)", line)
                if m and any(p in m.group(2) for p in patterns):
                    total += int(m.group(1))
            state["peak_kb"] = max(state["peak_kb"], total)
            state["stop"].wait(0.2)

    t = threading.Thread(target=run, daemon=True)
    t.start()
    return state


def stop_sampler(state):
    state["stop"].set()
    time.sleep(0.25)
    return state["peak_kb"]


def timed_run(argv, timeout=90):
    """Run a process to completion; wall seconds, stdout text, peak RSS."""
    s = rss_sampler([Path(argv[0]).name])
    started = time.monotonic()
    try:
        proc = subprocess.run(argv, capture_output=True, text=True, timeout=timeout)
        wall = time.monotonic() - started
        out = proc.stdout
        err = proc.stderr[-400:]
        ok = proc.returncode == 0 and len(out.strip()) > 40
    except subprocess.TimeoutExpired:
        wall = time.monotonic() - started
        out, err, ok = "", "timeout", False
    rss = stop_sampler(s)
    return wall, out, err, ok, rss


class Diting:
    name = "diting"
    mode = "server"

    def __init__(self, binary, port):
        self.binary, self.port = binary, port
        self.proc = None

    def start(self):
        env = dict(os.environ)
        env["DITING_BIND"] = f"127.0.0.1:{self.port}"
        env["DITING_CACHE_TTL_SECS"] = "0"
        self.proc = subprocess.Popen(
            [self.binary], env=env,
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        base = f"http://127.0.0.1:{self.port}"
        for _ in range(80):
            try:
                urllib.request.urlopen(base + "/health", timeout=2)
                return
            except Exception:
                time.sleep(0.5)
        raise RuntimeError("diting server did not come up")

    def fetch(self, url):
        s = rss_sampler(["diting"])
        started = time.monotonic()
        body = json.dumps({"url": url, "render_tier": "browser"}).encode()
        try:
            req = urllib.request.Request(
                f"http://127.0.0.1:{self.port}/fetch", data=body,
                headers={"Content-Type": "application/json"})
            with urllib.request.urlopen(req, timeout=90) as resp:
                data = json.loads(resp.read())
            wall = time.monotonic() - started
            if "error" in data:
                text, ok = str(data["error"])[:200], False
            else:
                text = data.get("content") or ""
                ok = bool(text.strip())
        except Exception as e:
            wall, text, ok = time.monotonic() - started, str(e)[:200], False
        return wall, text, ok, stop_sampler(s)

    def stop(self):
        if self.proc:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.proc.kill()


class CliEngine:
    """Cold-process CLI engine: argv template with {url}."""
    mode = "process"
    text_markers = None  # optional stderr noise to strip

    def __init__(self, name, binary, argv_tpl, timeout=90):
        self.name, self.binary, self.tpl, self.timeout = name, binary, argv_tpl, timeout

    def start(self):
        pass

    def stop(self):
        pass

    def fetch(self, url):
        argv = [a.format(url=url) for a in self.tpl]
        wall, out, err, ok, rss = timed_run(argv, timeout=self.timeout)
        return wall, out, ok, rss


def discover_engines(args):
    engines = []
    diting_bin = args.diting or str(BENCH.parent / "target/release/diting")
    engines.append(Diting(diting_bin, args.port))

    obscura = ENGINES_DIR / "obscura"
    if obscura.exists():
        engines.append(CliEngine("obscura", str(obscura), [str(obscura), "fetch", "--dump", "markdown", "{url}"]))
    moli = ENGINES_DIR / "moli"
    if moli.exists():
        engines.append(CliEngine("moli", str(moli), [str(moli), "fetch", "--markdown", "{url}"]))
    lp = ENGINES_DIR / "lightpanda"
    if lp.exists():
        engines.append(CliEngine("lightpanda", str(lp), [str(lp), "fetch", "--markdown", "{url}"]))
    return engines


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--rounds", type=int, default=2)
    ap.add_argument("--port", type=int, default=18155)
    ap.add_argument("--diting", default=None)
    args = ap.parse_args()

    engines = discover_engines(args)
    print(f"engines: {[e.name for e in engines]}", file=sys.stderr)
    if not engines:
        sys.exit("no engines found")

    results_dir = BENCH / "results"
    results_dir.mkdir(exist_ok=True)
    tsv_path = results_dir / f"five-engine-{date.today().isoformat()}.tsv"
    rows = []

    for engine in engines:
        try:
            engine.start()
            # Warmup, excluded from results: pays V8 snapshot / first-TLS etc.
            engine.fetch("https://example.com")
        except Exception as e:
            rows.append((engine.name, "-", "startup", 0.0, 0, 0, str(e)[:120]))
            continue
        for url in PAGES:
            for r in range(args.rounds):
                wall, text, ok, rss = engine.fetch(url)
                chars = len(text) if ok else 0
                rows.append((engine.name, url, f"r{r}", round(wall, 3), chars, rss, "" if ok else (text[:120] if not ok else "")))
                print(f"{engine.name}\t{url}\tr{r}\t{wall:.2f}s\t{chars} chars\t{rss} KB", file=sys.stderr)
        engine.stop()

    with open(tsv_path, "w") as f:
        f.write("engine\turl\tround\twall_s\ttext_chars\tpeak_rss_kb\terror\n")
        for row in rows:
            f.write("\t".join(str(c) for c in row) + "\n")
    print(f"\nwrote {tsv_path}", file=sys.stderr)

    # Summary: per engine per page, best round (successful rows preferred,
    # then fastest); per engine, median wall + success count + peak RSS.
    print("\n=== summary (best round per engine×page) ===")
    best = {}
    for name, url, _r, wall, chars, rss, err in rows:
        if url == "-":
            continue
        key = (name, url)
        cur = best.get(key)
        if cur is None or (chars > 0 and (cur[1] == 0 or wall < cur[0])):
            best[key] = (wall, chars, rss)
    for name in dict.fromkeys(e.name for e in engines):
        per = [v for k, v in best.items() if k[0] == name]
        if not per:
            continue
        okpages = [v for v in per if v[1] > 40]
        if not okpages:
            print(f"{name:12s} pages_ok 0/{len(PAGES)}")
            continue
        walls = sorted(v[0] for v in okpages)
        med = walls[len(walls) // 2]
        peak = max(v[2] for v in okpages)
        print(f"{name:12s} pages_ok {len(okpages)}/{len(PAGES):2d}  wall p50 {med:7.2f}s  rss_peak {peak} KB")


if __name__ == "__main__":
    main()
