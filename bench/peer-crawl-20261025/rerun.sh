#!/bin/zsh
# P3-9 follow-up: same-machine rerun of the Amiibo crawl at current tip
# (the optimization campaign — float series, on-demand layout, memory
# slimming, timer perf — landed AFTER the last recorded crawl pair).
set -e
set +e
R=/Users/sophiehe/Documents/aginx/aginxbrowser
PORT=18033
BIN=$R/target/release/aginxbrowser
OUT=$R/bench/peer-crawl-20261025

run_one () {
  local tag=$1
  env AGINXBROWSER_BIND=127.0.0.1:$PORT \
      AGINXBROWSER_DOMAIN_RATE_PER_MIN=0 \
      AGINXBROWSER_SESSION_PAGE_LIMIT=0 \
      AGINXBROWSER_STORE=0 \
      AGINXBROWSER_CACHE_TTL_SECS=0 \
      $BIN > $OUT/server-$tag.log 2>&1 &
  local srv=$!
  sleep 1
  curl -sS -m 5 --retry 3 http://127.0.0.1:$PORT/health > $OUT/health-$tag.json
  python3 $R/bench/peer-crawl/sample.py $srv $OUT/crawl-$tag.sample.json &
  local sampler=$!
  local t0=$(python3 -c 'import time; print(time.time())')
  python3 $R/bench/peer-crawl/crawl_http.py --base http://127.0.0.1:$PORT --pool 25 --out $OUT/crawl-$tag.json
  local t1=$(python3 -c 'import time; print(time.time())')
  kill $sampler 2>/dev/null || true
  wait $sampler 2>/dev/null || true
  kill $srv; wait $srv 2>/dev/null || true
  python3 - "$tag" "$t0" "$t1" <<'PY'
import json, sys
tag, t0, t1 = sys.argv[1], float(sys.argv[2]), float(sys.argv[3])
crawl = json.load(open(f"/Users/sophiehe/Documents/aginx/aginxbrowser/bench/peer-crawl-20261025/crawl-{tag}.json"))
samples = json.load(open(f"/Users/sophiehe/Documents/aginx/aginxbrowser/bench/peer-crawl-20261025/crawl-{tag}.sample.json"))
peak = max(s["rss_kb"] for s in samples) if samples else 0
cpu = samples[-1]["cpu_s"] if samples else None
print(json.dumps({"tag": tag, "pages": crawl.get("pages"), "known": crawl.get("known"),
                  "wall_s": round(t1 - t0, 3), "peak_rss_kb": peak, "cpu_seconds": cpu,
                  "error": crawl.get("error")}, indent=1))
PY
}

run_one f

