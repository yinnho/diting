#!/usr/bin/env python3
"""#84 retest harness: XHS note comment section.

Usage:
  python3 retest.py [cookies.json]     # no arg = anonymous pass

cookies.json: [{"name": ..., "value": ..., "domain": ".xiaohongshu.com", "path": "/"}, ...]

Probes (the original 2026-09-23 report's four observables):
  1. comment area state — stuck "加载中" vs comment items rendered
  2. did the comment API request fire at all (network rows, comment/page)
  3. fe-static chunk requests — done / hung / failed shapes
  4. console ChunkLoadError presence
"""

import json
import os
import subprocess
import sys
import time
import urllib.parse
import urllib.request

BASE = "http://127.0.0.1:18040"


def call_raw(path, data=None, timeout=90):
    req = urllib.request.Request(BASE + path, method="POST" if data is not None else "GET")
    req.add_header("Content-Type", "application/json")
    body = json.dumps(data).encode() if data is not None else None
    with urllib.request.urlopen(req, timeout=timeout, data=body) as r:
        return r.read().decode()


def call(path, data=None, timeout=90):
    req = urllib.request.Request(BASE + path, method="POST" if data is not None else "GET")
    req.add_header("Content-Type", "application/json")
    body = json.dumps(data).encode() if data is not None else None
    with urllib.request.urlopen(req, timeout=timeout, data=body) as r:
        return json.loads(r.read().decode())


def wait_health():
    for _ in range(100):
        try:
            return call("/health", timeout=3)
        except Exception:
            time.sleep(0.1)
    raise SystemExit("engine never came up")


def main():
    cookies = None
    if len(sys.argv) > 1:
        with open(sys.argv[1]) as f:
            cookies = json.load(f)
        print(f"loaded {len(cookies)} cookies: {[c['name'] for c in cookies]}")

    health = wait_health()
    print(f"engine: v{health['version']}/{health['commit']} stealth={health['capabilities']['stealth']} ua={health['ua'][:40]}...")

    create = {"viewport": {"width": 1280, "height": 900}}
    if cookies:
        create["cookies"] = cookies
    sid = call("/session/create", create)["session_id"]
    print("session:", sid)

    print("navigate explore ...")
    t0 = time.time()
    nav = call(f"/session/{sid}/navigate", {"url": "https://www.xiaohongshu.com/explore"})
    print(f"  landed: {nav.get('url', nav)[:80]} ({time.time()-t0:.1f}s)")

    # wait for the feed, then click into the first note card (modal, the report's shape)
    clicked = None
    for _ in range(40):
        # /state is compact text: "[N] <a href=... rect=...>text</a>"
        raw = call_raw(f"/session/{sid}/state", {})
        import re as _re
        m = _re.search(r"\[(\d+)\] <a href=\"([^\"]*\/explore\/[^\"]*)\"", raw)
        if not m:
            m = _re.search(r"\[(\d+)\] <a href=\"([^\"]*\"[^\"]*search_result[^\"]*)\"", raw)
        if m:
            idx, href = int(m.group(1)), m.group(2)
            call(f"/session/{sid}/click", {"index": idx})
            clicked = href
            break
        time.sleep(0.5)
    if not clicked:
        # fallback: probe page text for feed presence
        txt = call(f"/session/{sid}/eval", {"script": "document.body.innerText.slice(0, 300)"})
        print("  no note link found; body head:", str(txt)[:200])
    else:
        print("clicked note:", clicked)

    # let the detail modal settle, then probe the four observables
    print("settling 15s ...")
    time.sleep(15)

    probe = call(
        f"/session/{sid}/eval",
        {"script": """(function(){
            var t = document.body.innerText || '';
            var comments = document.querySelectorAll('.comment-item, [class*="comment"] li, .comments-container .comment').length;
            var loading = /加载中|comment_load|loading/.test(t);
            var note = document.querySelector('.note-detail, .note-content, [class*="note-detail"]');
            return JSON.stringify({
                comment_items: comments,
                loading_text: loading,
                note_modal_open: !!note,
                text_head: t.slice(0, 150)
            });
        })()"""},
    )
    print("comment-area probe:", probe)

    net = call(f"/session/{sid}/network")
    rows = net.get("requests", [])
    comment_api = [r for r in rows if "comment" in str(r.get("url", ""))]
    festatic = [r for r in rows if "fe-static" in str(r.get("url", ""))]
    hung = [r for r in rows if r.get("hung")]

    def shape(r):
        return {k: r.get(k) for k in ("url", "status", "error", "hung", "age_ms") if r.get(k) is not None}

    print(f"\nrows total: {len(rows)}")
    print(f"comment API rows: {len(comment_api)}")
    for r in comment_api[:5]:
        print("  ", shape(r))
    print(f"fe-static rows: {len(festatic)}")
    for r in festatic[:6]:
        print("  ", shape(r))
    print(f"hung rows: {len(hung)}")
    for r in hung[:5]:
        print("  ", shape(r))

    con = call(f"/session/{sid}/console")
    lines = con if isinstance(con, list) else con.get("lines", [])
    chunk_err = [l for l in lines if "ChunkLoadError" in str(l)]
    print(f"\nconsole lines: {len(lines)}, ChunkLoadError: {len(chunk_err)}")
    for l in chunk_err[:3]:
        print("  ", str(l)[:150])

    # dump full receipt
    out = os.path.join(os.path.dirname(os.path.abspath(__file__)), f"retest-{'login' if cookies else 'anon'}.json")
    with open(out, "w", encoding="utf-8") as f:
        json.dump(
            {"health": health, "clicked": clicked, "probe": probe, "network": rows, "console": lines},
            f,
            indent=1,
            ensure_ascii=False,
        )
    print("\nreceipt →", out)

    call(f"/session/{sid}/close", {})
    print("session closed")


if __name__ == "__main__":
    main()
