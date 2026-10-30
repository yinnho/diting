// Window members Chrome 145 has that no engine script installed. `sharedStorage` (the Shared Storage API entry point) is an
// accessor on the window returning one SharedStorage object (the interface itself is minted by stub_members.js).
(function () {
  const SS = globalThis.SharedStorage;
  if (typeof SS !== 'function' || Object.prototype.hasOwnProperty.call(globalThis, 'sharedStorage')) return;
  let inst = null;
  const get = function () { return inst || (inst = Object.create(SS.prototype)); };
  const mark = globalThis.__diting_markNative || ((f) => f);
  Object.defineProperty(get, 'name', { value: 'get sharedStorage', configurable: true });
  Object.defineProperty(globalThis, 'sharedStorage', { get: mark(get), set: undefined, enumerable: true, configurable: true });
})();

// NOTE (tried, reverted): Chrome's window has no OWN Symbol.toStringTag - it is inherited from Window.prototype, which is
// window's IMMEDIATE prototype. In the engine the global's immediate prototype is a different object than Window.prototype
// (Object.getPrototypeOf(window) !== Window.prototype, EventTarget is not in the chain: `window instanceof EventTarget` is false
// where Chrome's is true), so deleting the own tag turns Object.prototype.toString.call(window) into "[object Object]".
// Fixing that is a V8 global-template change (Rust side), not a script-layer one.

// Window prototype chain (shape-audit finding, M4 knife 37). The main window's chain was window -> gate(WindowProperties, named
// access) -> a bare `{constructor}` layer -> Object.prototype: no Window.prototype, no EventTarget.prototype, so
// `window instanceof EventTarget` was false (Chrome: true). Re-point the gate's raw target (bootstrap's `_namedHolder`, exposed as the hidden `__diting_namedHolder`) at the real
// Window.prototype, which chains to EventTarget.prototype (iframe_realm.js). The gate still sits ahead of everything, so named
// access resolution and the trio the holder carries are unchanged; the immediate prototype is still the gate, not Window.prototype
// (Chrome's order is window -> Window.prototype -> WindowProperties -> EventTarget), which needs a per-realm Window.prototype.
(function () {
  try {
    const holder = globalThis.__diting_namedHolder;
    if (typeof holder !== 'object' || holder === null || holder === globalThis) return;
    const W = globalThis.Window;
    if (typeof W !== 'function' || !W.prototype || W.prototype === holder) return;
    Object.setPrototypeOf(holder, W.prototype);
  } catch (e) {}
})();

// `location` own layout (M4 knife 48). Chrome's Location is [LegacyUnforgeable]: every member is an OWN property of the instance, in
// the order valueOf, ancestorOrigins, href, origin, protocol, host, hostname, port, pathname, search, hash, assign, reload,
// replace, toString, plus an own Symbol.toPrimitive (undefined, frozen); the instance inherits from Location.prototype (which
// carries the toStringTag) and has no fragmentDirective (that is a Document member). The bootstrap literal had an Object.prototype
// parent, an own toStringTag and fragmentDirective, and lacked valueOf/ancestorOrigins.
(function () {
  try {
    const L = globalThis.location, LP = globalThis.Location && globalThis.Location.prototype;
    if (!L || !LP || Object.getPrototypeOf(L) === LP) return;
    const ORDER = ['valueOf', 'ancestorOrigins', 'href', 'origin', 'protocol', 'host', 'hostname', 'port', 'pathname', 'search', 'hash', 'assign', 'reload', 'replace', 'toString'];
    const D = Object.create(null);
    for (const k of Object.getOwnPropertyNames(L)) D[k] = Object.getOwnPropertyDescriptor(L, k);
    const mark = globalThis.__diting_markNative || ((f) => f);
    if (!D.valueOf) D.valueOf = { value: Object.prototype.valueOf, writable: false, enumerable: false, configurable: false };
    if (!D.ancestorOrigins) {
      const empty = typeof globalThis.DOMStringList === 'function' ? Object.create(globalThis.DOMStringList.prototype) : Object.freeze([]);
      D.ancestorOrigins = { get: mark({ get ancestorOrigins() { return empty; } }.__lookupGetter__('ancestorOrigins')), set: undefined, enumerable: true, configurable: false };
    }
    if (Object.getOwnPropertyDescriptor(L, Symbol.toStringTag)) delete L[Symbol.toStringTag];
    delete D.fragmentDirective;
    for (const k of Object.getOwnPropertyNames(L)) { try { delete L[k]; } catch (e) { return; } }
    Object.setPrototypeOf(L, LP);
    for (const k of ORDER) if (D[k]) Object.defineProperty(L, k, D[k]);
    Object.defineProperty(L, Symbol.toPrimitive, { value: undefined, writable: false, enumerable: false, configurable: false });
    if (!Object.getOwnPropertyDescriptor(LP, Symbol.toStringTag)) Object.defineProperty(LP, Symbol.toStringTag, { value: 'Location', writable: false, enumerable: false, configurable: true });
  } catch (e) {}
})();

// Chrome's window carries no OWN @@toStringTag (it comes from Window.prototype now that the chain reaches it).
(function () {
  try {
    if (Object.getOwnPropertyDescriptor(globalThis, Symbol.toStringTag) && Object.getOwnPropertyDescriptor(globalThis.Window.prototype, Symbol.toStringTag)) delete globalThis[Symbol.toStringTag];
  } catch (e) {}
})();
