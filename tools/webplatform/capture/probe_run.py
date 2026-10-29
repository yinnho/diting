#!/usr/bin/env python3
"""Run a behaviour probe in a REAL Chrome (over CDP) and/or in the diting engine, save the JSON.

  probe_run.py chrome <probe.js> <out.json> [chrome-binary]    default: Chrome for Testing 145.0.7632.117
  probe_run.py engine <probe.js> <out.json> [diting-base-url]  default: http://127.0.0.1:18199 (start it with --allow-private-network)

The probe is an expression (sync or async) that evaluates to a JSON string. It runs in a page served
from http://127.0.0.1 (blob:, FileReader, MessageChannel etc. need a real origin). CDP is used because
`--dump-dom` does not return on every Chrome-for-Testing build.
"""
import json, os, sys, tempfile, threading, http.server, socketserver, urllib.request
HERE = os.path.dirname(os.path.abspath(__file__))
# reuse the CDP helper from wire_capture.py (everything before its `meta = {` main section)
_ns = {"__file__": os.path.join(HERE, "wire_capture.py")}
_saved = sys.argv; sys.argv = ["x", "chrome", "x", "x"]
exec(compile(open(os.path.join(HERE, "wire_capture.py")).read().split("meta = {")[0], "wire_capture", "exec"), _ns)
sys.argv = _saved
chrome_dom = _ns["chrome_dom"]

mode, probe_path, out = sys.argv[1], sys.argv[2], sys.argv[3]
probe = open(probe_path, encoding="utf8").read().strip().rstrip(";")
page = ("<!doctype html><meta charset=utf-8><pre id=o></pre><script>(function(){var P=(\n" + probe + "\n);"
        "Promise.resolve(P).then(function(v){document.getElementById('o').textContent=v},"
        "function(e){document.getElementById('o').textContent='PROBE-ERROR '+e});})();</script>")
# Local WebSocket echo server (binary + text), so probes can test real WebSocket frames.
import asyncio, websockets
_loop = asyncio.new_event_loop()
async def _echo(ws):
    async for m in ws:
        await ws.send(m)          # echoes text as text, bytes as a binary frame
async def _serve():
    return await websockets.serve(_echo, "127.0.0.1", 0)
_ws = _loop.run_until_complete(_serve())
WS_PORT = _ws.sockets[0].getsockname()[1]
threading.Thread(target=_loop.run_forever, daemon=True).start()
page = page.replace("__WS_PORT__", str(WS_PORT))
d = tempfile.mkdtemp(); open(os.path.join(d, "p.html"), "w", encoding="utf8").write(page)
class H(http.server.SimpleHTTPRequestHandler):
    def __init__(s, *a, **k): super().__init__(*a, directory=d, **k)
    def log_message(s, *a): pass
srv = socketserver.TCPServer(("127.0.0.1", 0), H); port = srv.server_address[1]
threading.Thread(target=srv.serve_forever, daemon=True).start()
url = "http://127.0.0.1:%d/p.html" % port
if mode == "chrome":
    binary = sys.argv[4] if len(sys.argv) > 4 else os.path.expanduser(
        "~/cft/145.0.7632.117/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing")
    raw = chrome_dom(binary, url)
elif mode == "engine":
    base = sys.argv[4] if len(sys.argv) > 4 else "http://127.0.0.1:18199"
    r = json.load(urllib.request.urlopen(urllib.request.Request(base + "/eval",
        data=json.dumps({"url": url, "script": probe.replace("__WS_PORT__", str(WS_PORT))}).encode(), headers={"Content-Type": "application/json"}), timeout=180))
    raw = r["result"]
else:
    sys.exit(__doc__)
srv.shutdown()
res = json.loads(raw)
json.dump(res, open(out, "w"), indent=1, sort_keys=True, ensure_ascii=False)
print("captured %d probes -> %s" % (len(res), out))
