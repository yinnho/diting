#!/usr/bin/env python3
"""Capture what a client looks like ON THE WIRE and in `navigator` (JS-PLATFORM-ARCH,
Chrome-version coherence audit). One output file per (client, version).

  wire_capture.py chrome <chrome-binary> <out.json>   real Chrome (Chrome for Testing recommended)
  wire_capture.py engine <diting-base-url> <out.json> the engine via its product HTTP API
                                                      (start it with --allow-private-network)

Captures: tls.peet.ws/api/all (JA3/JA4, cipher/extension/sigalg lists, H2 settings and
HEADERS frame incl. request headers) and chrome/ua-probe.js (navigator + userAgentData).
The echo service only sees our own handshake/headers; no user data is sent. Volatile
per-connection material (ip, client_random, session_id, key-share bytes) is stripped.
"""
import html, json, os, re, subprocess, sys, tempfile, threading, urllib.request
import http.server, socketserver

HERE = os.path.dirname(os.path.abspath(__file__))
PROBE = open(os.path.join(HERE, "..", "chrome", "ua-probe.js")).read().strip().rstrip(";")
mode, target, out_path = sys.argv[1], sys.argv[2], sys.argv[3]


def scrub_tls(d):
    for k in ("ip", "donate"):
        d.pop(k, None)
    d.pop("tcpip", None)
    t = d.get("tls", {})
    for k in ("client_random", "session_id"):
        t.pop(k, None)
    for e in t.get("extensions", []):
        for k in ("data", "public_key", "public_key_length"):
            e.pop(k, None)
        if "shared_keys" in e:
            e["shared_keys"] = [{list(x)[0]: "<scrubbed>"} for x in e["shared_keys"]]
    return d


def page_server():
    d = tempfile.mkdtemp()
    open(os.path.join(d, "p.html"), "w").write(
        "<!doctype html><pre id=o></pre><script>(function(){var P=(\n" + PROBE +
        "\n);Promise.resolve(P).then(function(v){document.getElementById('o').textContent=v});})();</script>")
    class H(http.server.SimpleHTTPRequestHandler):
        def __init__(s, *a, **k): super().__init__(*a, directory=d, **k)
        def log_message(s, *a): pass
    srv = socketserver.TCPServer(("127.0.0.1", 0), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv, srv.server_address[1]


def chrome_dom(binary, url, pre_selector="pre"):
    import os
    """Drive a headless Chrome over CDP (--dump-dom does not return reliably on every
    Chrome-for-Testing build): navigate, wait for the <pre> to be filled, read it, quit."""
    import asyncio, socket, time, urllib.request
    import websockets
    s = socket.socket(); s.bind(("127.0.0.1", 0)); port = s.getsockname()[1]; s.close()
    prof = tempfile.mkdtemp()
    # DITING_CHROME_WEBGL=1: software WebGL (SwiftShader) instead of --disable-gpu, so probes can read
    # real WebGLRenderingContext instances (M4 刀69); the default capture stays GPU-less and unchanged.
    # =metal (刀92): real GPU (no gpu flags at all) — exposes GPU-dependent extensions SwiftShader lacks.
    # DITING_CHROME_EXTRA: extra launch flags, whitespace-separated, appended after the gpu block.
    gpu = (["--disable-gpu"] if os.environ.get("DITING_CHROME_WEBGL") is None
           else [] if os.environ.get("DITING_CHROME_WEBGL") == "metal"
           else ["--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"])
    extra = os.environ.get("DITING_CHROME_EXTRA", "").split()
    proc = subprocess.Popen([binary, "--headless=new"] + gpu + extra + ["--no-first-run", "--no-default-browser-check",
                             "--use-mock-keychain", "--password-store=basic", "--disable-background-networking",
                             "--disable-component-update", "--disable-sync", "--remote-debugging-port=%d" % port,
                             "--user-data-dir=" + prof, "about:blank"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        ws_url = None
        for _ in range(60):
            try:
                tabs = json.load(urllib.request.urlopen("http://127.0.0.1:%d/json" % port, timeout=2))
                ws_url = next(t["webSocketDebuggerUrl"] for t in tabs if t["type"] == "page"); break
            except Exception:
                time.sleep(0.5)
        if not ws_url:
            raise SystemExit("chrome did not expose a page target")

        async def run():
            async with websockets.connect(ws_url, max_size=64 * 1024 * 1024) as ws:
                n = 0
                async def call(method, params=None):
                    nonlocal n
                    n += 1; me = n
                    await ws.send(json.dumps({"id": me, "method": method, "params": params or {}}))
                    while True:
                        m = json.loads(await ws.recv())
                        if m.get("id") == me:
                            return m
                await call("Page.enable")
                await call("Page.navigate", {"url": url})
                for _ in range(80):
                    await asyncio.sleep(0.5)
                    r = await call("Runtime.evaluate", {"expression": "(document.querySelector('%s')||{}).textContent||''" % pre_selector, "returnByValue": True})
                    v = r.get("result", {}).get("result", {}).get("value", "")
                    if v and v.strip():
                        return v
                raise SystemExit("timed out waiting for page content at " + url)
        return asyncio.run(run())
    finally:
        proc.terminate()
        try: proc.wait(timeout=10)
        except Exception: proc.kill()


meta = {"mode": mode, "target": target}
if mode == "chrome":
    ver = subprocess.run([target, "--version"], capture_output=True, text=True).stdout.strip()
    meta["version"] = ver
    tls = json.loads(chrome_dom(target, "https://tls.peet.ws/api/all"))
    srv, port = page_server()
    ua = json.loads(chrome_dom(target, "http://127.0.0.1:%d/p.html" % port))
    srv.shutdown()
elif mode == "engine":
    health = json.load(urllib.request.urlopen(target + "/health", timeout=20))
    meta["version"] = "diting %s (%s) tls=%s" % (health.get("version"), health.get("commit"), health.get("tls"))
    r = json.load(urllib.request.urlopen(urllib.request.Request(
        target + "/fetch", data=json.dumps({"url": "https://tls.peet.ws/api/all", "format": "html"}).encode(),
        headers={"Content-Type": "application/json"}), timeout=90))
    tls = json.loads(html.unescape(re.search(r"<pre>(.*)</pre>", r["content"], re.S).group(1)))
    srv, port = page_server()
    r = json.load(urllib.request.urlopen(urllib.request.Request(
        target + "/eval", data=json.dumps({"url": "http://127.0.0.1:%d/p.html" % port, "script": PROBE}).encode(),
        headers={"Content-Type": "application/json"}), timeout=90))
    ua = json.loads(r["result"])
    srv.shutdown()
else:
    sys.exit(__doc__)
json.dump({"meta": meta, "tls": scrub_tls(tls), "ua": ua}, open(out_path, "w"), indent=1, sort_keys=True, ensure_ascii=False)
print("captured", mode, meta["version"], "->", out_path)
