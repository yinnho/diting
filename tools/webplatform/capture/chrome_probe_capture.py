#!/usr/bin/env python3
"""Run chrome/storage-probe.js in real headless Chrome (needs an http origin:
localStorage is unavailable on opaque origins) and store the result as truth.
usage: chrome_storage_capture.py [probe.js out.json]
  default: chrome/storage-probe.js -> chrome/chrome-storage-truth.json
"""
import json, os, re, subprocess, sys, tempfile, threading, http.server, socketserver, datetime

HERE = os.path.dirname(os.path.abspath(__file__))
CH = os.path.join(HERE, "..", "chrome")
CHROME = os.environ.get("CHROME_BIN", "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")
PROBE = sys.argv[1] if len(sys.argv) > 1 else "storage-probe.js"
OUTN = sys.argv[2] if len(sys.argv) > 2 else "chrome-storage-truth.json"
probe = open(os.path.join(CH, PROBE)).read()
html = ("<!doctype html><html><body><pre id=o></pre><script>"
        "var __r; try { __r = (function(){ return (\n" + probe + "\n); })(); } catch (e) { __r = 'PROBE-ERROR ' + e + ' @ ' + (e.stack||''); }"
        "document.getElementById('o').textContent = __r;"
        "</script></body></html>")
d = tempfile.mkdtemp()
open(os.path.join(d, "p.html"), "w").write(html)
class H(http.server.SimpleHTTPRequestHandler):
    def __init__(s, *a, **k): super().__init__(*a, directory=d, **k)
    def log_message(s, *a): pass
srv = socketserver.TCPServer(("127.0.0.1", 0), H); port = srv.server_address[1]
threading.Thread(target=srv.serve_forever, daemon=True).start()
prof = tempfile.mkdtemp()
out = subprocess.run([CHROME, "--headless=new", "--disable-gpu", f"--user-data-dir={prof}", "--dump-dom",
                      f"http://127.0.0.1:{port}/p.html"], capture_output=True, text=True, timeout=60).stdout
ver = subprocess.run([CHROME, "--version"], capture_output=True, text=True).stdout.strip()
m = re.search(r'<pre id="o">(.*?)</pre>', out, re.S)
if not m: sys.exit("probe output not found:\n" + out[:500])
import html as H2
raw = H2.unescape(m.group(1))
try:
    res = json.loads(raw)
except Exception:
    sys.exit('probe did not yield JSON; got: ' + repr(raw[:300]) + '\nDOM head: ' + repr(out[:300]))
json.dump({"_meta": {"chrome": ver, "date": datetime.date.today().isoformat(), "probe": "chrome/" + PROBE, "origin": "http://127.0.0.1 (ephemeral)"}, "result": res},
          open(os.path.join(CH, OUTN), "w"), indent=1, sort_keys=True, ensure_ascii=False)
print("captured", len(res), "probes;", ver)
