// 刀79: stub defaults. Pure-stub interfaces (stub_members.js) whose getters answered undefined: on a real instance Chrome 145
// reports these primitive/null defaults (default-value reconciliation over every no-arg constructible interface,
// chrome/defaults-probe.js -> defaults-truth-145.json). The stub getter keeps its brand check; only an `undefined` answer is
// replaced. Loaded after stub_members.js (which installs the getters).
(() => {
  const G = globalThis;
  const mark = G.__diting_markNative || ((f) => f);
  const DEF = {"MediaSource":{"onsourceclose":null,"onsourceended":null,"onsourceopen":null},"CaptureController":{"onzoomlevelchange":null,"zoomLevel":null},"CloseWatcher":{"oncancel":null,"onclose":null},"Highlight":{"priority":0,"size":0,"type":"highlight"},"IdentityCredentialError":{"code":"","error":"","url":""},"IdleDetector":{"onchange":null,"screenState":null,"userState":null},"Origin":{"opaque":true},"QuotaExceededError":{"quota":null,"requested":null},"ScrollTimeline":{"axis":"block"},"ViewTimeline":{"endOffset":null,"startOffset":null,"subject":null},"WebSocketError":{"closeCode":null,"reason":""},"WebTransportError":{"source":"stream","streamErrorCode":null}};
  for (const cn of Object.keys(DEF)) {
    const C = G[cn];
    const P = typeof C === 'function' && C.prototype;
    if (!P) continue;
    for (const k of Object.keys(DEF[cn])) {
      const d = Object.getOwnPropertyDescriptor(P, k);
      if (!d || !d.get) continue;
      const g = d.get, dv = DEF[cn][k];
      const ng = function () { const r = g.call(this); return r === undefined ? dv : r; };
      Object.defineProperty(ng, 'name', { value: 'get ' + k, configurable: true });
      Object.defineProperty(P, k, { get: mark(ng), set: d.set, enumerable: d.enumerable, configurable: true });
    }
  }
})();
