#!/usr/bin/env python3
# Portable Chrome 154.0.8037.98 (CDP 9223) 一次采齐：
#   gl 表       → 对齐 tools/webplatform/chrome/chrome-gl-const-tables.json (155)
#   glvals      → 对齐 tools/webplatform/chrome/chrome-gl-accessor-values.json (g1 800x600 / g2 1024x768)
#   blank       → 对齐 tools/webplatform/chrome/xhs-chrome-blank-consts.json (10 尺寸 × png/jpeg/webp)
#   audio       → 对齐 tools/webplatform/chrome/chrome-audio-truth.json (oac/ac/base 三面)
#   uad         → 对齐 tools/webplatform/chrome/conn-uad-truth-1015.json 的 uad 部分 + 高熵值
import asyncio, json, urllib.request
import websockets

PORT = 9223

JS = r"""
(async () => {
  const out = {};
  // --- GL const/method tables ---
  for (const [key, C] of [['1', WebGLRenderingContext], ['2', WebGL2RenderingContext]]) {
    if (!C) { out['consts'+key] = null; continue; }
    const P = C.prototype;
    const names = Object.getOwnPropertyNames(P);
    const consts = {}, methods = [], left = [];
    for (const n of names) {
      const d = Object.getOwnPropertyDescriptor(P, n);
      if (typeof d.value === 'number') consts[n] = d.value;
      else if (typeof d.value === 'function') methods.push(n);
      else left.push(n);
    }
    out['consts'+key] = consts;
    out['methods'+key] = methods;
    out['left'+key] = left;
    out['count'+key] = { names: names.length,
      consts: Object.keys(consts).length, methods: methods.length, left: left.length };
  }
  // --- GL accessor values (same canvas sizes as 155 capture) ---
  const c1 = document.createElement('canvas'); c1.width = 800; c1.height = 600;
  const g1 = c1.getContext('webgl');
  const c2 = document.createElement('canvas'); c2.width = 1024; c2.height = 768;
  const g2 = c2.getContext('webgl2');
  out.glvals = {
    g1: g1 ? [g1.drawingBufferWidth, g1.drawingBufferHeight, g1.drawingBufferColorSpace, g1.unpackColorSpace, g1.drawingBufferFormat] : null,
    g2: g2 ? [g2.drawingBufferWidth, g2.drawingBufferHeight, g2.drawingBufferColorSpace, g2.unpackColorSpace, g2.drawingBufferFormat] : null,
  };
  // --- blank canvas consts grid ---
  const sizes = ['300x150','220x30','240x60','120x20','640x480','64x64','200x100','280x60','500x200','1024x768'];
  out.blank = {};
  for (const s of sizes) {
    const [w, h] = s.split('x').map(Number);
    const c = document.createElement('canvas'); c.width = w; c.height = h;
    const row = {};
    for (const fmt of ['png','jpeg','webp']) row[fmt] = c.toDataURL('image/' + fmt);
    out.blank[s] = row;
  }
  // --- audio prototype faces ---
  const face = (C) => {
    const m = {};
    for (const n of Object.getOwnPropertyNames(C.prototype)) {
      const d = Object.getOwnPropertyDescriptor(C.prototype, n);
      if (d.get) m[n] = 'get';
      else if (typeof d.value === 'function') m[n] = 'fn';
      else m[n] = 'data';
    }
    return m;
  };
  out.audio = {
    oac: face(OfflineAudioContext),
    ac: face(AudioContext),
    base: face(BaseAudioContext),
  };
  // --- UAD ---
  const u = navigator.userAgentData;
  if (u) {
    out.uad = {
      present: true,
      json: u.toJSON(),
      protoOwn: Object.getOwnPropertyNames(Object.getPrototypeOf(u)),
      tag: Object.prototype.toString.call(u),
      gevType: typeof u.getHighEntropyValues,
    };
    try {
      out.uad.highEntropy = await u.getHighEntropyValues(
        ['architecture','bitness','model','platformVersion','uaFullVersion','fullVersionList','wow64','formFactors']);
    } catch (e) { out.uad.highEntropy = String(e); }
  } else out.uad = { present: false };
  out.ua = navigator.userAgent;
  return JSON.stringify(out);
})()
"""

async def main():
    target = json.loads(urllib.request.urlopen(urllib.request.Request(
        f"http://127.0.0.1:{PORT}/json/new", method="PUT")).read())
    ws_url = target["webSocketDebuggerUrl"]
    import time; time.sleep(0.3)
    mid = 0
    async with websockets.connect(ws_url, max_size=256 * 1024 * 1024) as ws:
        async def cmd(method, params=None):
            nonlocal mid
            mid += 1
            await ws.send(json.dumps({"id": mid, "method": method, "params": params or {}}))
            while True:
                m = json.loads(await ws.recv())
                if m.get("id") == mid:
                    if "error" in m: raise RuntimeError(m["error"])
                    return m.get("result", {})
        # /json/new 不再吃 URL 参数；正经导航到 https 页并等 load
        #（userAgentData 只在安全上下文暴露，about:blank 拿不到）
        await cmd("Page.enable")
        nav_mid = mid + 1000
        await ws.send(json.dumps({"id": nav_mid, "method": "Page.navigate",
                                  "params": {"url": "https://www.baidu.com"}}))
        while True:
            m = json.loads(await ws.recv())
            if m.get("method") == "Page.loadEventFired": break
            if m.get("id") == nav_mid and "error" in m: raise RuntimeError(m["error"])
        loc = await cmd("Runtime.evaluate", {"expression": "location.href", "returnByValue": True})
        print("capture page:", loc["result"]["value"])
        r = await cmd("Runtime.evaluate", {"expression": JS, "returnByValue": True, "awaitPromise": True})
        d = json.loads(r["result"]["value"])
        json.dump(d, open("chrome154-face-truth.json", "w"), indent=1)
        print("UA:", d["ua"])
        print("count1:", d["count1"]); print("count2:", d["count2"])
        print("glvals:", d["glvals"])
        print("uad brands:", json.dumps(d["uad"]["json"]["brands"]))
        print("uad highEntropy keys:", list(d["uad"].get("highEntropy", {}).keys()) if isinstance(d.get("uad",{}).get("highEntropy"), dict) else d["uad"].get("highEntropy"))
        print("audio oac/ac/base:", len(d["audio"]["oac"]), len(d["audio"]["ac"]), len(d["audio"]["base"]))
        print("blank sizes:", len(d["blank"]))

asyncio.run(main())
