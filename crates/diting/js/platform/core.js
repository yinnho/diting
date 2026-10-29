// platform/core.js - L0 core of the module system (JS-PLATFORM-ARCH sec 3.3).
// Hand-written. bundle.py (tools/webplatform) puts this FIRST in
// platform_bundle.js, then every module wrapped in __platform.define(), then
// __platform.boot(). `__platform` is a lexical const (no window property).
//
// Module contract: a module body runs as function (exports, require, P).
//   require(name)  -> exports of an already-loaded module (load order is the
//                     bundle's topological order; a miss is a build bug)
//   P              -> this object
// Legacy hubs (_markNative, _OPS, Event, ...) stay free identifiers resolved
// through the global lexical environment; the manifest must declare them as
// `legacy:Name` and tools/webplatform/analyze checks that it does.
const __platform = (function () {
  'use strict';
  const defs = [];
  const loaded = new Map();

  function define(name, fn) { defs.push({ name, fn }); }
  function boot() {
    for (const d of defs) {
      const exports = {};
      loaded.set(d.name, exports);
      d.fn(exports, function require(n) {
        if (!loaded.has(n)) throw new Error('platform: require(' + n + ') before it was loaded (from ' + d.name + ')');
        return loaded.get(n);
      }, P);
    }
  }

  // ---- errors: ONE construction path so every API throws Chrome-shaped errors.
  // Chrome facts (chrome/chrome-error-truth.json): the stack's first line carries the
  // UNDECORATED message, e.message carries "<prefix>: <message>", no shim frames
  // sit above the caller. Error.captureStackTrace(e, fn) drops everything up to
  // and including `fn` (the API function being executed); the header is computed
  // at capture time, so message is decorated AFTER the capture. A decorated
  // message is an enumerable data prop (Blink sets it afterwards); an undecorated
  // one keeps V8's non-enumerable shape.
  function fail(fn, message, prefix) {
    const e = new TypeError(message);
    Error.captureStackTrace(e, fn);
    // prefix: null/undefined = leave V8's message as constructed; '' = decorate with an
    // empty prefix (Chrome still re-sets message, so it turns enumerable: Storage()
    // called without new); non-empty = "<prefix>: <message>".
    if (prefix !== null && prefix !== undefined) {
      Object.defineProperty(e, 'message', { value: prefix ? prefix + ': ' + message : message, writable: true, enumerable: true, configurable: true });
    }
    throw e;
  }
  function args(prefix, got, need, fn) {
    if (got < need) fail(fn, need + ' argument' + (need === 1 ? '' : 's') + ' required, but only ' + got + ' present.', prefix);
  }
  function illegal(fn) { fail(fn, 'Illegal invocation'); }

  // ---- WebIDL conversions (Chrome message shapes) ----
  const conv = {
    DOMString(v, prefix, fn) {
      if (typeof v === 'symbol') fail(fn, 'Cannot convert a Symbol value to a string', prefix);
      return String(v);
    },
    USVString(v, prefix, fn) {
      const s = conv.DOMString(v, prefix, fn);
      return typeof s.toWellFormed === 'function' ? s.toWellFormed() : s;
    },
    boolean(v) { return !!v; },
    unsignedLong(v, prefix, fn) {
      if (typeof v === 'symbol') fail(fn, 'Cannot convert a Symbol value to a number', prefix);
      const n = Number(v);
      if (!isFinite(n)) return 0;
      const t = Math.trunc(n);
      return ((t % 4294967296) + 4294967296) % 4294967296;
    },
  };

  // ---- shape helpers: one place decides descriptors and nativeness ----
  function setFn(fn, name, length) {
    Object.defineProperty(fn, 'name', { value: name, writable: false, enumerable: false, configurable: true });
    Object.defineProperty(fn, 'length', { value: length, writable: false, enumerable: false, configurable: true });
    return _markNative(fn);
  }
  // interface object: prototype {w:f,e:f,c:f}; proto.constructor {w:t,e:f,c:t}; @@toStringTag
  function iface(ctor, name, length, parent) {
    setFn(ctor, name, length);
    const proto = Object.create(parent ? parent.prototype : Object.prototype);
    Object.defineProperty(ctor, 'prototype', { value: proto, writable: false, enumerable: false, configurable: false });
    Object.defineProperty(proto, Symbol.toStringTag, { value: name, writable: false, enumerable: false, configurable: true });
    if (parent) Object.setPrototypeOf(ctor, parent);
    return proto;
  }
  // proto.constructor {w:t,e:f,c:t}, defined AFTER the members: Chrome lists it last.
  function seal(ctor) {
    Object.defineProperty(ctor.prototype, 'constructor', { value: ctor, writable: true, enumerable: false, configurable: true });
  }
  function method(proto, name, length, fn) {
    setFn(fn, name, length);
    Object.defineProperty(proto, name, { value: fn, writable: true, enumerable: true, configurable: true });
  }
  function getter(proto, name, fn) {
    setFn(fn, 'get ' + name, 0);
    Object.defineProperty(proto, name, { get: fn, set: undefined, enumerable: true, configurable: true });
  }
  // read-write IDL attribute: get/set pair on the prototype (e.g. DOMMatrix.a).
  // (param must not be named setFn — it would shadow the helper above and
  // get CALLED as one; caught by the geometry knife, first accessor user.)
  function accessor(proto, name, getFn, setterFn) {
    setFn(getFn, 'get ' + name, 0);
    setFn(setterFn, 'set ' + name, 1);
    Object.defineProperty(proto, name, { get: getFn, set: setterFn, enumerable: true, configurable: true });
  }
  // static IDL operation: own on the interface object, ENUMERABLE (Chrome:
  // DOMPoint.fromMatrix et al. are own+enumerable statics).
  function stat(ctor, name, length, fn) {
    setFn(fn, name, length);
    Object.defineProperty(ctor, name, { value: fn, writable: true, enumerable: true, configurable: true });
  }
  // Reflected IDL attribute, installed on a (possibly pre-existing native)
  // prototype. spec is DATA (generated per member from the truth table):
  //   attr      content attribute name (already lowercase)
  //   kind      string | boolean-presence | boolean-truefalse |
  //             string-known (getter: known-lowercase else default/'') |
  //             known-fallback (getter: known else fallback)
  //   known     keyword list (lowercase), for the string-known kinds
  //   default   getter default when the attribute is absent ('' unless set)
  //   fallback  getter answer for a present-but-invalid value
  // Chrome facts pinned by htmlelement-truth-145.json: setters write the
  // value AS GIVEN (dir 'RTL' keeps 'RTL'), getters normalize; presence
  // booleans setAttribute(name,'') on truthy and remove on falsy;
  // draggable writes literal 'true'/'false'; nonce does not reflect at all.
  function reflector(proto, name, spec) {
    const attr = spec.attr, kind = spec.kind;
    const known = spec.known || null;
    const dflt = 'default' in spec ? spec.default : '';
    const get = function () {
      const el = this;
      const v = el.getAttribute(attr);
      if (kind === 'string') return v === null ? dflt : String(v);
      if (kind === 'boolean-presence') return el.hasAttribute(attr);
      if (kind === 'boolean-truefalse') return v === 'true';
      if (kind === 'string-known' || kind === 'known-fallback') {
        if (v === null) return dflt;
        const low = String(v).toLowerCase();
        if (known && known.indexOf(low) >= 0) return low;
        return kind === 'known-fallback' ? spec.fallback : dflt;
      }
      return undefined;
    };
    const set = function (v) {
      const el = this;
      if (kind === 'string') { el.setAttribute(attr, v === undefined || v === null ? '' : String(v)); return; }
      if (kind === 'boolean-presence') { if (v) el.setAttribute(attr, ''); else el.removeAttribute(attr); return; }
      if (kind === 'boolean-truefalse') { el.setAttribute(attr, v ? 'true' : 'false'); return; }
      // keyword kinds: Chrome writes the value AS GIVEN; invalid keywords
      // still land in the attribute (truth set.autocapitalize.bad /
      // set.enterKeyHint 'GO' -> attr 'GO') — only translate no-ops invalid.
      el.setAttribute(attr, v === undefined || v === null ? '' : String(v));
    };
    setFn(get, 'get ' + name, 0);
    setFn(set, 'set ' + name, 1);
    Object.defineProperty(proto, name, { get, set, enumerable: true, configurable: true });
  }
  // Globals. expose(): interface objects. exposeValue(): instance globals that
  // the legacy surface installed with plain assignment (enumerable data).
  function expose(name, ctor) {
    Object.defineProperty(globalThis, name, { value: ctor, writable: true, enumerable: false, configurable: true });
  }
  function exposeValue(name, v) { globalThis[name] = v; }

  // ---- descriptor conformance: generated modules register expectations;
  // verify() returns the list of violations ([] = conformant).
  const expectations = [];
  function expect(e) { expectations.push(e); }
  function verify() {
    const bad = [];
    const nat = (f) => /\[native code\]/.test(Function.prototype.toString.call(f));
    for (const e of expectations) {
      const C = globalThis[e.name];
      if (typeof C !== 'function') { bad.push(e.name + ': not exposed'); continue; }
      if (C.length !== e.length) bad.push(e.name + '.length ' + C.length + ' != ' + e.length);
      if (!nat(C)) bad.push(e.name + ': constructor not native-looking');
      const pd = Object.getOwnPropertyDescriptor(C, 'prototype');
      if (!pd || pd.writable || pd.enumerable || pd.configurable) bad.push(e.name + '.prototype descriptor');
      const tag = Object.getOwnPropertyDescriptor(C.prototype, Symbol.toStringTag);
      if (!tag || tag.value !== e.name || tag.writable || tag.enumerable || !tag.configurable) bad.push(e.name + ' @@toStringTag');
      for (const m of e.members) {
        const d = Object.getOwnPropertyDescriptor(C.prototype, m.name);
        if (!d) { bad.push(e.name + '.' + m.name + ': missing'); continue; }
        if (!d.enumerable || !d.configurable) bad.push(e.name + '.' + m.name + ': flags');
        if (m.kind === 'method') {
          if (!('value' in d) || !d.writable) bad.push(e.name + '.' + m.name + ': not a writable data prop');
          else if (d.value.length !== m.length || d.value.name !== m.name || !nat(d.value)) bad.push(e.name + '.' + m.name + ': fn shape');
        } else if (m.kind === 'accessor') {
          if (!d.get || !d.set || typeof d.set !== 'function') bad.push(e.name + '.' + m.name + ': accessor pair shape');
          else if (d.get.name !== 'get ' + m.name || d.get.length !== 0 || !nat(d.get)) bad.push(e.name + '.' + m.name + ': getter shape');
          else if (d.set.name !== 'set ' + m.name || d.set.length !== 1 || !nat(d.set)) bad.push(e.name + '.' + m.name + ': setter shape');
        } else {
          if (!d.get || d.set !== undefined) bad.push(e.name + '.' + m.name + ': accessor shape');
          else if (d.get.name !== 'get ' + m.name || d.get.length !== 0 || !nat(d.get)) bad.push(e.name + '.' + m.name + ': getter shape');
        }
      }
      for (const s of e.statics || []) {
        const d = Object.getOwnPropertyDescriptor(C, s.name);
        if (!d) { bad.push(e.name + '.' + s.name + ': static missing'); continue; }
        if (!d.writable || !d.enumerable || !d.configurable) bad.push(e.name + '.' + s.name + ': static flags');
        else if (d.value.length !== s.length || d.value.name !== s.name || !nat(d.value)) bad.push(e.name + '.' + s.name + ': static fn shape');
      }
    }
    return bad;
  }

  const P = { define, boot, conv, fail, args, illegal, iface, seal, method, getter, accessor, stat, reflector, expose, exposeValue, expect, verify };
  return P;
})();
