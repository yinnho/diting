#!/usr/bin/env python3
"""RSS + cputime sampler for one pid, ~10 Hz, JSON-lines out."""
import json, subprocess, sys, time

pid, out = sys.argv[1], sys.argv[2]
rows = []
while True:
    try:
        r = subprocess.run(["ps", "-o", "rss=,time=", "-p", pid], capture_output=True, text=True).stdout.split()
        if len(r) == 2:
            rss_kb = int(r[0])
            parts = r[1].split(":")
            cpu_s = sum(float(x) * 60 ** i for i, x in enumerate(reversed(parts)))
            rows.append({"t": time.time(), "rss_kb": rss_kb, "cpu_s": cpu_s})
    except Exception:
        pass
    with open(out, "w") as f:
        json.dump(rows, f)
    time.sleep(0.1)
