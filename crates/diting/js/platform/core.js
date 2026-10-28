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

  // ---- WebIDL conversions (Chrome message shapes) ----
  const conv = {
    DOMString(v, ctx) {
      if (typeof v === 'symbol') throw new TypeError(ctx + ': Cannot convert a Symbol value to a string');
      return String(v);
    },
    USVString(v, ctx) {
      const s = conv.DOMString(v, ctx);
      return typeof s.toWellFormed === 'function' ? s.toWellFormed() : s;
    },
    boolean(v) { return !!v; },
    unsignedLong(v, ctx) {
      if (typeof v === 'symbol') throw new TypeError(ctx + ': Cannot convert a Symbol value to a number');
      const n = Number(v);
      if (!isFinite(n)) return 0;
      const t = Math.trunc(n);
      return ((t % 4294967296) + 4294967296) % 4294967296;
    },
  };

  function args(ctx, got, need) {
    if (got < need) {
      throw new TypeError(ctx + ': ' + need + ' argument' + (need === 1 ? '' : 's') + ' required, but only ' + got + ' present.');
    }
  }
  function illegal() { throw new TypeError('Illegal invocation'); }

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
        } else {
          if (!d.get || d.set !== undefined) bad.push(e.name + '.' + m.name + ': accessor shape');
          else if (d.get.name !== 'get ' + m.name || d.get.length !== 0 || !nat(d.get)) bad.push(e.name + '.' + m.name + ': getter shape');
        }
      }
    }
    return bad;
  }

  const P = { define, boot, conv, args, illegal, iface, seal, method, getter, expose, exposeValue, expect, verify };
  return P;
})();
