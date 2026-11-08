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
  // prototype. spec is DATA (generated per member from measured Chrome
  // fingerprints - chrome/htmlreflect-truth-145.json - or the earlier knives'
  // truth tables):
  //   attr      content attribute name (already lowercase)
  //   iface     interface name for setter error prefixes (default: the
  //             prototype's constructor name)
  //   kind      string | string-nte (LegacyNullToEmptyString) |
  //             boolean-presence | boolean-truefalse |
  //             enum (measured: map/missing/invalid/empty) |
  //             string-known / known-fallback (legacy spellings of enum) |
  //             unsigned-long (parse digits-prefix, > 2^31-1 -> 0; set
  //               ToUint32, > 2^31-1 -> 0) |
  //             unsigned-long-dim (as above but the GETTER accepts up to
  //               2^32-1: width/height of img/video) |
  //             ulong-def (limited unsigned long with `default`, optional `min`:
  //               invalid/negative/overflow/below-min read the default; set > 2^31-1
  //               or < min writes the default) |
  //             long (signed; out of int32 -> 0; set ToInt32) |
  //             crossorigin (nullable enum: absent null, 'use-credentials',
  //               every other present value 'anonymous'; set null/undefined
  //               removes the attribute) |
  //             url (absent -> default, else resolved against the document
  //               base; '' resolves to the base itself)
  //   fallbackDoc   url: an ABSENT attribute reads document.URL (formAction)
  //   known / default / fallback   legacy enum spellings
  //   nullRemoves   null/undefined set REMOVES the attribute (enum/string)
  //   map / missing / invalid / empty   measured enum (ASCII-lowercased
  //               attribute value -> map entry; '' -> empty when given;
  //               anything else -> invalid; no attribute -> missing)
  // Every setter converts through `conv` so Symbols throw Chrome's
  // "Failed to set the 'x' property on 'I': Cannot convert a Symbol value
  // to a string" TypeError; DOMString conversion is String(v) - null reads
  // back 'null', undefined 'undefined' (htmlreflect-truth-145 extra.*).
  const HTML_WS_INT = /^[\t\n\f\r ]*([+-]?)([0-9]+)/;
  function parseIntAttr(v) {
    const m = HTML_WS_INT.exec(v);
    return m === null ? null : Number((m[1] === '-' ? '-' : '') + m[2]) + 0;
  }
  function asciiLower(v) { return v.replace(/[A-Z]/g, function (c) { return c.toLowerCase(); }); }
  function toInt32(n) {
    if (!isFinite(n)) return 0;
    const t = ((Math.trunc(n) % 4294967296) + 4294967296) % 4294967296;
    return t > 2147483647 ? t - 4294967296 : t;
  }
  function reflector(proto, name, spec) {
    const attr = spec.attr, kind = spec.kind;
    const known = spec.known || null;
    const dflt = 'default' in spec ? spec.default : '';
    const iface = spec.iface || (proto.constructor && proto.constructor.name) || '';
    const prefix = "Failed to set the '" + name + "' property on '" + iface + "'";
    // enum tables: legacy spellings fold into the measured form
    let emap = null, emissing = dflt, einvalid = dflt, eempty;
    if (kind === 'enum') {
      emap = spec.map; emissing = spec.missing; einvalid = spec.invalid;
      if ('empty' in spec) eempty = spec.empty;
    } else if (kind === 'string-known' || kind === 'known-fallback') {
      emap = {};
      for (const k of (known || [])) emap[k] = k;
      if (kind === 'known-fallback') einvalid = spec.fallback;
      if ('empty' in spec) eempty = spec.empty;
    }
    const get = function () {
      const el = this;
      const v = el.getAttribute(attr);
      switch (kind) {
        case 'string': case 'string-nte': return v === null ? dflt : String(v);
        case 'boolean-presence': return el.hasAttribute(attr);
        case 'boolean-truefalse': return v === 'true';
        case 'enum': case 'string-known': case 'known-fallback': {
          if (v === null) return emissing;
          const low = asciiLower(String(v));
          if (low === '' && eempty !== undefined) return eempty;
          return Object.prototype.hasOwnProperty.call(emap, low) ? emap[low] : einvalid;
        }
        case 'unsigned-long': case 'unsigned-long-dim': {
          if (v === null) return 0;
          const n = parseIntAttr(String(v));
          const max = kind === 'unsigned-long' ? 2147483647 : 4294967295;
          return n === null || n < 0 || n > max ? 0 : n;
        }
        case 'ulong-def': {
          // limited unsigned long with a default (textarea cols/rows,
          // marquee scrollAmount/scrollDelay): invalid, negative, overflow
          // or below `min` all read the default
          if (v === null) return spec.default;
          const n = parseIntAttr(String(v));
          return n === null || n < (spec.min || 0) || n > 2147483647 ? spec.default : n;
        }
        case 'long': {
          if (v === null) return 0;
          const n = parseIntAttr(String(v));
          return n === null || n < -2147483648 || n > 2147483647 ? 0 : n;
        }
        case 'crossorigin': {
          if (v === null) return null;
          return asciiLower(String(v)) === 'use-credentials' ? 'use-credentials' : 'anonymous';
        }
        case 'url': {
          if (v === null) return spec.fallbackDoc ? document.URL : dflt;
          const base = (typeof document !== 'undefined' && document && document.baseURI)
            || (typeof location !== 'undefined' ? location.href : undefined);
          try { return new URL(v, base).href; } catch (e) { return String(v); }
        }
      }
      return undefined;
    };
    const set = function (v) {
      const el = this;
      switch (kind) {
        case 'boolean-presence': if (v) el.setAttribute(attr, ''); else el.removeAttribute(attr); return;
        case 'boolean-truefalse': el.setAttribute(attr, v ? 'true' : 'false'); return;
        case 'unsigned-long': case 'unsigned-long-dim': {
          const u = conv.unsignedLong(v, prefix, set);
          el.setAttribute(attr, String(u > 2147483647 ? 0 : u));
          return;
        }
        case 'ulong-def': {
          const u = conv.unsignedLong(v, prefix, set);
          el.setAttribute(attr, String(u > 2147483647 || u < (spec.min || 0) ? spec.default : u));
          return;
        }
        case 'long': {
          if (typeof v === 'symbol') fail(set, 'Cannot convert a Symbol value to a number', prefix);
          el.setAttribute(attr, String(toInt32(Number(v))));
          return;
        }
        case 'crossorigin':
          if (v === null || v === undefined) { el.removeAttribute(attr); return; }
          el.setAttribute(attr, conv.DOMString(v, prefix, set));
          return;
        case 'string-nte':
          el.setAttribute(attr, v === null ? '' : conv.DOMString(v, prefix, set));
          return;
        default:
          // string / url / every enum spelling: the value lands AS GIVEN
          // (Chrome writes the attribute verbatim, getters normalize).
          // nullRemoves: the interface's setter treats null/undefined as
          // "no value" (script.referrerPolicy) instead of the strings.
          if (spec.nullRemoves && (v === null || v === undefined)) { el.removeAttribute(attr); return; }
          el.setAttribute(attr, conv.DOMString(v, prefix, set));
      }
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

  const P = { define, boot, conv, fail, args, illegal, iface, seal, method, getter, accessor, stat, reflector, expose, exposeValue, expect, verify, parseIntAttr, asciiLower };
  return P;
})();
