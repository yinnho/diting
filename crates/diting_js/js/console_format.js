(function () { // @@SCOPE
// god-file split (刀59, console argument formatter): _fmtVal — the
// DevTools-shaped preview formatter (depth/width clamps, cycle marks,
// per-property getter-throw degradation) with its _peek own-property
// reader (page accessors are never invoked: console must not run page
// code — the xhs mnsv2 tripwire) and the _FMT_* clamps. Pure helpers,
// zero installs, zero native marking. bootstrap's _consoleFn calls
// _fmtVal/_FMT_MAX at runtime through the published hidden globals;
// first slot after bootstrap so any load-time console use resolves too.
// Console argument formatter: DevTools-shaped previews instead of
// "[object Object]". Depth/width clamped, cycles marked, getters that
// throw degrade per-property — a log line must never cost the page its
// console call (issue #99: tmall's fileLoader.loadTimeout logged its
// options object, which collapsed to "[object Object]" and hid which
// resource timed out).
const _FMT_DEPTH = 3, _FMT_WIDTH = 20, _FMT_KEYS = 15, _FMT_MAX = 8192;
// A console call must never run page code: Chrome without DevTools open
// formats nothing, and anti-bot scripts plant accessors on the arguments
// (Error.stack, element id, plain props) as a "DevTools attached" tripwire.
// xhs's mnsv2 loader does exactly that — console.log(err) firing its stack
// getter made it conclude "inspected" and silently skip installing mnsv2.
// Own accessors are therefore never invoked; prototype-chain reads (native
// getters like Element.tagName) stay as they were.
const _peek = (v, k) => {
  const d = Object.getOwnPropertyDescriptor(v, k);
  if (d) return "value" in d ? d.value : _PEEK_GETTER;
  return v[k];
};
const _PEEK_GETTER = Symbol("getter");
// #72: V8's own Error `stack` is an own ACCESSOR too (one native getter
// shared by every error), so the rule above dropped every stack and an
// unattended serve logged 1737 "reading 'kind'" TypeErrors with no clue
// where. The native getter runs no page code unless the page installed
// Error.prepareStackTrace, so it is safe to invoke exactly then; a getter
// the page planted is still never called.
const _NATIVE_STACK_GET = (() => {
  try { const d = Object.getOwnPropertyDescriptor(new Error(""), "stack"); return d && d.get; } catch (e) { return undefined; }
})();
const _errStack = (v) => {
  const d = Object.getOwnPropertyDescriptor(v, "stack");
  if (!d) return undefined;
  if ("value" in d) return d.value;
  if (d.get && d.get === _NATIVE_STACK_GET && !Object.getOwnPropertyDescriptor(Error, "prepareStackTrace")) {
    try { return d.get.call(v); } catch (e) { return undefined; }
  }
  return undefined;
};
const _fmtVal = (v, depth, seen) => {
  if (v === null) return "null";
  if (v === undefined) return "undefined";
  const t = typeof v;
  if (t === "string") return depth === 0 ? v : JSON.stringify(v);
  if (t === "number" || t === "boolean" || t === "bigint" || t === "symbol") return String(v);
  if (t === "function") return "[Function" + (v.name ? ": " + v.name : "") + "]";
  try {
    if (v instanceof Error) {
      const st = _errStack(v);
      if (typeof st === "string" && st) return st;
      const nm = _peek(v, "name"), msg = _peek(v, "message");
      return (typeof nm === "string" ? nm : "Error") + (typeof msg === "string" && msg ? ": " + msg : "");
    }
    if (typeof Element !== "undefined" && v instanceof Element) {
      let s = "<" + String(_peek(v, "tagName")).toLowerCase();
      const eid = _peek(v, "id");
      if (typeof eid === "string" && eid) s += "#" + eid;
      const ecl = _peek(v, "className");
      const cl = typeof ecl === "string" ? ecl.trim() : "";
      if (cl) s += "." + cl.split(/\s+/).join(".");
      return s + ">";
    }
    if (typeof Map !== "undefined" && v instanceof Map) {
      if (seen.has(v)) return "[Circular]";
      if (depth >= _FMT_DEPTH) return "Map(" + v.size + ")";
      seen.add(v);
      const items = [];
      let n = 0;
      for (const [k, val] of v) {
        if (n++ >= _FMT_WIDTH) { items.push("\u2026 +" + (v.size - _FMT_WIDTH) + " more"); break; }
        items.push(_fmtVal(k, depth + 1, seen) + " => " + _fmtVal(val, depth + 1, seen));
      }
      seen.delete(v);
      return "Map(" + v.size + ") {" + items.join(", ") + "}";
    }
    if (typeof Set !== "undefined" && v instanceof Set) {
      if (seen.has(v)) return "[Circular]";
      if (depth >= _FMT_DEPTH) return "Set(" + v.size + ")";
      seen.add(v);
      const items = [];
      let n = 0;
      for (const val of v) {
        if (n++ >= _FMT_WIDTH) { items.push("\u2026 +" + (v.size - _FMT_WIDTH) + " more"); break; }
        items.push(_fmtVal(val, depth + 1, seen));
      }
      seen.delete(v);
      return "Set(" + v.size + ") {" + items.join(", ") + "}";
    }
    if (Array.isArray(v)) {
      if (seen.has(v)) return "[Circular]";
      if (depth >= _FMT_DEPTH) return "Array(" + v.length + ")";
      seen.add(v);
      const items = v.slice(0, _FMT_WIDTH).map(x => _fmtVal(x, depth + 1, seen));
      if (v.length > _FMT_WIDTH) items.push("\u2026 +" + (v.length - _FMT_WIDTH) + " more");
      seen.delete(v);
      return "[" + items.join(", ") + "]";
    }
    if (seen.has(v)) return "[Circular]";
    if (depth >= _FMT_DEPTH) {
      let n = 0; try { n = Object.keys(v).length; } catch {}
      return "Object(" + n + " keys)";
    }
    seen.add(v);
    let keys; try { keys = Object.keys(v); } catch { seen.delete(v); return String(v); }
    const parts = [];
    for (const k of keys.slice(0, _FMT_KEYS)) {
      let val;
      try {
        const pv = _peek(v, k);
        val = pv === _PEEK_GETTER ? "[Getter]" : _fmtVal(pv, depth + 1, seen);
      } catch { val = "[Getter threw]"; }
      parts.push(k + ": " + val);
    }
    if (keys.length > _FMT_KEYS) parts.push("\u2026 +" + (keys.length - _FMT_KEYS) + " more");
    seen.delete(v);
    return "{" + parts.join(", ") + "}";
  } catch { return String(v); }
};

// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('_FMT_MAX', _FMT_MAX);
  d('_fmtVal', _fmtVal);
  d('_peek', _peek);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
