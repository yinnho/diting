// TextTrackCue behavior takeover. VTTCue itself is generated (issue #67:
// platform/gen/vttcue.gen.js + platform/vttcue.impl.js); its instances are the
// only TextTrackCues a page can make, and they keep their state in the impl's
// WeakMap, published as the hidden __diting_cueState — read at CALL time here
// (this file loads after the bundle, and after global_surface minted
// TextTrackCue). The minted prototype stays (shape/brand tables survive), only
// the member bodies are replaced. startTime is a restricted double, endTime
// unrestricted (Infinity ok, NaN rejected). Loaded after
// stub_members.js/stub_defaults.js (which install the accessors being
// replaced) — defineProperty on an existing key keeps its order.
(() => {
  const G = globalThis;
  const TTC = G.TextTrackCue;
  if (typeof TTC !== 'function' || !TTC.prototype) return;
  const mark = G.__diting_markNative || ((f) => f);
  const pin = (f, n) => { Object.defineProperty(f, 'name', { value: n, configurable: true }); return mark(f); };
  const S = (self) => {
    const st = G.__diting_cueState;
    const s = (st && self !== null && typeof self === 'object') ? st.get(self) : undefined;
    if (!s) throw new TypeError('Illegal invocation');
    return s;
  };
  const num = (v, fail) => { const n = Number(v); if (!Number.isFinite(n)) throw new TypeError(fail + 'The provided double value is non-finite.'); return n; };
  const setMsg = (prop) => "Failed to set the '" + prop + "' property on 'TextTrackCue': ";

  // accessor (re)definition keeping each key's current flags and position
  const acc = (obj, k, get, set) => {
    const d = Object.getOwnPropertyDescriptor(obj, k) || { enumerable: true, configurable: true };
    Object.defineProperty(obj, k, { get: pin(get, 'get ' + k), set: set ? pin(set, 'set ' + k) : undefined, enumerable: d.enumerable, configurable: d.configurable });
  };
  const TP = TTC.prototype;
  acc(TP, 'track', function () { S(this); return null; });
  acc(TP, 'id', function () { return S(this).id; }, function (v) { S(this).id = String(v); });
  acc(TP, 'startTime', function () { return S(this).startTime; }, function (v) { const s = S(this); s.startTime = num(v, setMsg('startTime')); });
  acc(TP, 'endTime', function () { return S(this).endTime; }, function (v) {
    const s = S(this), n = Number(v);
    if (Number.isNaN(n)) throw new TypeError(setMsg('endTime') + 'The provided double value is non-finite.');
    s.endTime = n;
  });
  acc(TP, 'pauseOnExit', function () { return S(this).pauseOnExit; }, function (v) { S(this).pauseOnExit = Boolean(v); });
})();
