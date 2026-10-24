#!/usr/bin/env python3
"""P3-9 release-profile tip micro-bench (2026-10-25).

Same shape as bench/scoreboard-20261019 (debug run): cold start → first
/health, warm /health, /fetch markdown shortset (5 URLs, cache off), peak
RSS during the shortset, session snapshot, curl baseline. New this round:
the AginxBrowser column is a --release stealth+screenshot binary, and
Lightpanda 1.0.1 / Obscura 0.2.5 run the same shortset through their
`fetch` CLIs (per-invocation, like the curl/Chrome columns of the 20261019
matrix) plus a serve-mode cold-start probe.

Receipts land next to this script; INDEX.json summarizes.
"""

import json
import os
import signal
import subprocess
import threading
import time
import urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
REPO = os.path.abspath(ROOT)
OUT = os.path.join(REPO, "bench", "scoreboard-20261025")
TMP = os.path.join(OUT, "tmp")
PORT = 18025
BASE = f"http://127.0.0.1:{PORT}"
URLS = [
    "https://example.com",
    "https://www.rust-lang.org/",
    "https://www.python.org/",
    "https://nodejs.org/en",
    "https://doc.rust-lang.org/book/title-page.html",
]
AGINX = os.path.join(REPO, "target", "release", "aginxbrowser")
LIGHTPANDA = os.path.join(TMP, "lightpanda-aarch64-macos")
OBSCURA = os.path.join(TMP, "obscura")


def dump(name, obj):
    path = os.path.join(OUT, name)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(obj, f, indent=1, ensure_ascii=False)
    print(f"  → {name}")


def now_ms():
    return time.time() * 1000.0


def http_json(url, timeout=90, data=None):
    req = urllib.request.Request(url, method="POST" if data else "GET")
    req.add_header("Content-Type", "application/json")
    with urllib.request.urlopen(req, timeout=timeout, data=data.encode() if data else None) as r:
        return json.loads(r.read().decode())


class Sampler(threading.Thread):
    """Sum RSS (KB) across processes matching a pattern, ~10 Hz."""

    def __init__(self, pattern):
        super().__init__(daemon=True)
        self.pattern = pattern
        self.peak_kb = 0
        self.stop = threading.Event()

    def run(self):
        while not self.stop.is_set():
            try:
                out = subprocess.run(
                    ["pgrep", "-f", self.pattern], capture_output=True, text=True
                ).stdout.split()
                if out:
                    ps = subprocess.run(
                        ["ps", "-o", "rss=", "-p", ",".join(out)],
                        capture_output=True,
                        text=True,
                    ).stdout.split()
                    total = sum(int(v) for v in ps)
                    self.peak_kb = max(self.peak_kb, total)
            except Exception:
                pass
            time.sleep(0.1)


def wait_health(timeout_s=60):
    t0 = now_ms()
    while now_ms() - t0 < timeout_s * 1000:
        try:
            return now_ms() - t0, http_json(f"{BASE}/health", timeout=3)
        except Exception:
            time.sleep(0.02)
    raise TimeoutError("health never came up")


def main():
    os.makedirs(TMP, exist_ok=True)

    # --- 1. cold start → first /health --------------------------------
    print("[1/7] cold start")
    env = dict(os.environ)
    env.update(
        AGINXBROWSER_BIND=f"127.0.0.1:{PORT}",
        AGINXBROWSER_CACHE_TTL_SECS="0",
        AGINXBROWSER_RESTORE_SESSIONS="0",
        AGINXBROWSER_STORE="0",
    )
    spawn_t0 = now_ms()
    server = subprocess.Popen([AGINX], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        first_ms, health = wait_health()
        dump(
            "cold-start.summary.json",
            {
                "metric": "cold_start_first_health",
                "profile": "release",
                "spawn_ms": round(now_ms() - spawn_t0 - first_ms, 1),
                "first_health_ms": round(first_ms, 1),
                "ok": True,
                "health": health,
            },
        )

        # --- 2. warm /health ------------------------------------------
        print("[2/7] warm health")
        warm = []
        for _ in range(3):
            t0 = now_ms()
            http_json(f"{BASE}/health", timeout=5)
            warm.append(round(now_ms() - t0, 3))
        warm.sort()
        dump("health-warm.summary.json", {"metric": "health_warm_ms_n3", "samples": warm, "median": warm[1]})

        # --- 3. aginx /fetch shortset + RSS ---------------------------
        print("[3/7] aginx fetch shortset")
        sampler = Sampler(f"aginxbrowser.*127.0.0.1:{PORT}|{AGINX}")
        sampler.start()
        rows = []
        for u in URLS:
            t0 = now_ms()
            try:
                r = http_json(
                    f"{BASE}/fetch",
                    timeout=120,
                    data=json.dumps({"url": u, "format": "markdown", "max_chars": 0, "render_tier": "auto"}),
                )
                rows.append(
                    {
                        "url": u,
                        "ms": round(now_ms() - t0, 1),
                        "tier": r.get("tier"),
                        "chars": len(r.get("content") or ""),
                        "ok": True,
                    }
                )
            except Exception as e:  # noqa: BLE001
                rows.append({"url": u, "ms": round(now_ms() - t0, 1), "ok": False, "error": str(e)[:120]})
        sampler.stop.set()
        sampler.join()
        ok_ms = sorted(r["ms"] for r in rows if r.get("ok"))
        dump(
            "aginx-fetch-shortset.summary.json",
            {
                "metric": "fetch_shortset_markdown",
                "profile": "release",
                "rows": rows,
                "median_ms": ok_ms[len(ok_ms) // 2] if ok_ms else None,
                "ok": sum(1 for r in rows if r.get("ok")),
                "peak_rss_kb": sampler.peak_kb,
            },
        )

        # --- 4. session snapshot --------------------------------------
        print("[4/7] session snapshot")
        t0 = now_ms()
        sid = http_json(f"{BASE}/session/create", data=json.dumps({"viewport": {"width": 1280, "height": 720}}))["session_id"]
        create_ms = round(now_ms() - t0, 1)
        t0 = now_ms()
        http_json(f"{BASE}/session/{sid}/navigate", timeout=90, data=json.dumps({"url": "https://example.com"}))
        nav_ms = round(now_ms() - t0, 1)
        t0 = now_ms()
        snap = http_json(f"{BASE}/session/{sid}/snapshot", timeout=30)
        snap_ms = round(now_ms() - t0, 1)
        dump(
            "aginx-snapshot.summary.json",
            {
                "metric": "session_snapshot_example_com",
                "create_ms": create_ms,
                "navigate_ms": nav_ms,
                "snapshot_ms": snap_ms,
                "snapshot_nodes": len(snap.get("nodes", snap if isinstance(snap, list) else [])) if snap else None,
            },
        )
        http_json(f"{BASE}/session/{sid}/close", timeout=10, data="{}")
    finally:
        server.send_signal(signal.SIGTERM)
        server.wait(timeout=10)

    # --- 5. curl baseline -----------------------------------------------
    print("[5/7] curl shortset")
    rows = []
    for u in URLS:
        t0 = now_ms()
        p = subprocess.run(["curl", "-sS", "-m", "60", "-o", "/dev/null", "-w", "%{http_code}", u], capture_output=True, text=True)
        rows.append({"url": u, "ms": round(now_ms() - t0, 1), "http": p.stdout.strip()})
    dump("curl-fetch-shortset.summary.json", {"metric": "curl_raw_get", "rows": rows})

    # --- 6. lightpanda shortset (per-invocation, like the curl column) --
    print("[6/7] lightpanda shortset")
    rows = []
    for u in URLS:
        t0 = now_ms()
        p = subprocess.run([LIGHTPANDA, "fetch", u, "--dump", "html"], capture_output=True, text=True, timeout=120)
        rows.append({"url": u, "ms": round(now_ms() - t0, 1), "chars": len(p.stdout), "rc": p.returncode})
    lp_cold_t0 = now_ms()
    lp = subprocess.Popen([LIGHTPANDA, "serve", "--port", "19225"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    lp_cold_ms = None
    while now_ms() - lp_cold_t0 < 30000:
        try:
            import socket
            s = socket.create_connection(("127.0.0.1", 19225), timeout=0.3)
            s.close()
            lp_cold_ms = round(now_ms() - lp_cold_t0, 1)
            break
        except Exception:
            time.sleep(0.02)
    lp.send_signal(signal.SIGTERM)
    lp.wait(timeout=10)
    dump(
        "lightpanda-fetch-shortset.summary.json",
        {
            "metric": "lightpanda_fetch_cli_per_invocation",
            "version": "1.0.1",
            "rows": rows,
            "serve_cold_connect_ms": lp_cold_ms,
            "note": "CLI fetch = process spawn + page load + dump, per URL (same shape as the curl/Chrome columns). Not comparable to a warm HTTP API call.",
        },
    )

    # --- 7. obscura shortset --------------------------------------------
    print("[7/7] obscura shortset")
    rows = []
    for u in URLS:
        t0 = now_ms()
        p = subprocess.run([OBSCURA, "fetch", u, "--dump", "markdown"], capture_output=True, text=True, timeout=120)
        rows.append({"url": u, "ms": round(now_ms() - t0, 1), "chars": len(p.stdout), "rc": p.returncode})
    ob_cold_t0 = now_ms()
    ob = subprocess.Popen([OBSCURA, "serve", "--port", "19226"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    ob_cold_ms = None
    while now_ms() - ob_cold_t0 < 30000:
        try:
            import socket
            s = socket.create_connection(("127.0.0.1", 19226), timeout=0.3)
            s.close()
            ob_cold_ms = round(now_ms() - ob_cold_t0, 1)
            break
        except Exception:
            time.sleep(0.02)
    ob.send_signal(signal.SIGTERM)
    ob.wait(timeout=10)
    dump(
        "obscura-fetch-shortset.summary.json",
        {
            "metric": "obscura_fetch_cli_per_invocation",
            "version": "0.2.5",
            "rows": rows,
            "serve_cold_connect_ms": ob_cold_ms,
            "note": "CLI fetch = process spawn + page load + markdown dump, per URL. obscura-worker spawns alongside; RSS sampling of CLI runs not taken (short-lived processes).",
        },
    )

    print("done")


if __name__ == "__main__":
    main()
