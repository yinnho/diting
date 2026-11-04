(function () { // @@SCOPE
// god-file split (刀26, structuredClone family): the HTML structured-clone
// serializer (#389 turnstile) — buffers/typed arrays/DataView, maps/sets,
// dates, regexps, errors with cause, plain objects with descriptor shape,
// platform clone-hook routing — plus the globalThis.structuredClone install
// (only when the realm hasn't one). All eight in-file references are
// self-contained; native_clone.js / crypto_subtle.js mention it in comments
// only, page/split callers go through the installed global (member form).
// Real structured clone (not JSON). JSON.parse(JSON.stringify) silently drops
// ArrayBuffer/TypedArray (they serialize to {}), so Cloudflare's turnstile
// orchestrate loses every byte it tries to round-trip through postMessage and
// the challenge never completes (issue #389). Clone buffers, typed arrays,
// maps/sets, dates, errors, and plain objects recursively; platform objects
// that register a clone hook (see crypto.subtle, registered in the WebCrypto
// group) are routed there.
function _structuredClone(value, seen) {
  // Functions and symbols are not structured-cloneable (HTML structured clone,
  // DataCloneError). This must run before the primitive early-return below,
  // which would otherwise pass them through by reference.
  if (typeof value === "function" || typeof value === "symbol") {
    throw new DOMException("Failed to execute 'structuredClone': value could not be cloned.", "DataCloneError");
  }
  if (value === null || typeof value !== "object") return value;
  if (seen.has(value)) return seen.get(value);
  // Typed arrays: copy the underlying buffer slice. DataView has no .slice(),
  // so slice its buffer over the view's range and wrap a fresh view.
  if (ArrayBuffer.isView(value)) {
    if (value instanceof DataView) {
      const buf = value.buffer.slice(value.byteOffset, value.byteOffset + value.byteLength);
      const copy = new DataView(buf);
      seen.set(value, copy);
      return copy;
    }
    const Ctor = value.constructor;
    const copy = new Ctor(value.slice());
    seen.set(value, copy);
    return copy;
  }
  if (value instanceof ArrayBuffer) {
    const copy = value.slice(0);
    seen.set(value, copy);
    return copy;
  }
  if (typeof SharedArrayBuffer !== "undefined" && value instanceof SharedArrayBuffer) {
    return value; // transferable, not copyable
  }
  if (value instanceof Date) return new Date(value.getTime());
  if (value instanceof RegExp) return new RegExp(value.source, value.flags);
  if (value instanceof Map) {
    const m = new Map();
    seen.set(value, m);
    for (const [k, v] of value) m.set(_structuredClone(k, seen), _structuredClone(v, seen));
    return m;
  }
  if (value instanceof Set) {
    const s = new Set();
    seen.set(value, s);
    for (const v of value) s.add(_structuredClone(v, seen));
    return s;
  }
  // Platform objects (consulted BEFORE the generic Error path: a native DOMException is an Error by prototype but keeps its state in slots) that carry internal slots opt into cloning via a hook
  // (CryptoKey re-registers its key material so the clone stays usable by
  // crypto.subtle). Anything else with a registered hook takes that path.
  if (typeof value[Symbol.toStringTag] === "string" && globalThis.__diting_clone_hooks) {
    const hook = globalThis.__diting_clone_hooks[value[Symbol.toStringTag]];
    if (typeof hook === "function") return hook(value, seen);
  }
  if (value instanceof Error) {
    const Ctor = value.constructor || Error;
    const e = new Ctor(value.message);
    // Record the clone before recursing into `cause`, otherwise a cycle
    // through the error (e.cause === e) recurses until the stack overflows.
    seen.set(value, e);
    if (value.name) e.name = value.name;
    if (value.stack) e.stack = value.stack;
    if (value.cause !== undefined) e.cause = _structuredClone(value.cause, seen);
    return e;
  }
  // Plain objects clone onto Object.prototype (like Chrome), not the source's
  // prototype. Define each property instead of assigning it: a source with an
  // own enumerable `__proto__` data prop (what JSON.parse('{"__proto__":…}')
  // yields) would otherwise hit the inherited __proto__ setter and reparent
  // the clone instead of copying the property.
  const out = Array.isArray(value) ? [] : {};
  seen.set(value, out);
  for (const k in value) {
    if (Object.prototype.hasOwnProperty.call(value, k)) {
      const cloned = _structuredClone(value[k], seen);
      // Only `__proto__` needs defineProperty: plain assignment would hit the
      // inherited prototype setter and reparent the clone instead of adding an
      // own data property. Every other key takes the fast assignment path.
      if (k === "__proto__") {
        Object.defineProperty(out, k, {
          value: cloned,
          writable: true,
          enumerable: true,
          configurable: true,
        });
      } else {
        out[k] = cloned;
      }
    }
  }
  // Symbols are not enumerable via for-in; copy own symbol-keyed properties.
  const syms = Object.getOwnPropertySymbols(value);
  for (const s of syms) {
    const d = Object.getOwnPropertyDescriptor(value, s);
    if (d && "value" in d) out[s] = _structuredClone(d.value, seen);
  }
  return out;
}
globalThis.structuredClone = globalThis.structuredClone || ((v) => _structuredClone(v, new Map()));

// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
