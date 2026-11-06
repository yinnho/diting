(function () { // @@SCOPE
// god-file split (刀88, per-realm initializer): __diting_init — the whole
// per-realm setup Rust calls after every context restore: fp seed/cache
// re-draw, location URL drop, window on* + viewport accessor re-installs
// (#37/#105 — the snapshot flattens accessors, so they land per context),
// persona apply, performance.now coarsening offset, the SharedArrayBuffer
// intrinsic delete (the restore rebuilds the intrinsic table; only a
// post-restore delete sticks), NavigationTiming seeding, the interface
// hide belt (_isInternal/_hideOn/_patch over gOPN/ownKeys/keys/gOPDS), and
// its own self-delete. Free names are accessor/d()-mediated at call time
// (_fpSeed/_fpCache a(), _fpRand d()) or globalThis members; nothing is
// captured at load. The hide-list capture in bootstrap runs at ITS load
// and the init property self-deletes on first run, so moving the install
// changes no visibility window. Any slot safe.
globalThis.__diting_init = function() {
  _fpSeed = Date.now() ^ (Math.random() * 0xFFFFFFFF >>> 0);
  _fpCache = null;
  // A real navigation just completed (this runs after set_url), so drop any
  // URL a location setter previewed synchronously and let document_url drive
  // location.href again, including any redirect target.
  globalThis.__virtualUrl = null;
  _installWasmStreamingFallback();
  // (#37) V8 snapshot creation flattens accessors on the global object to
  // data:null props, so the window on* family installs here, per context.
  globalThis.__diting_installWindowOnHandlers();
  // (#105) Same flattening: the window size accessors install here too,
  // so page writes to innerWidth/innerHeight are ignored like Chrome's.
  globalThis.__diting_installViewportAccessors();

  globalThis.__diting_setPersona();

  // Per-realm performance.now() coarsening offset (see performance.js): the
  // module-level draw happens once at snapshot build, which would freeze one
  // bucket offset onto EVERY context the process ever serves — cross-session
  // aggregatable. Re-draw per realm onto the performance object's hidden _
  // slot family (_marks/_measures), not a new window global.
  try {
    Object.defineProperty(globalThis.performance, '_noise', {
      value: Math.random() * 0.1, writable: true, configurable: true,
    });
  } catch (e) {}

  // SharedArrayBuffer delete lives HERE, not at snapshot top level: V8's
  // context deserializer rebuilds the intrinsic table on every restore, so
  // the snapshot-time delete never survived (see the FontFace block above
  // for the full story). __diting_init runs per realm post-restore, where
  // the delete actually sticks.
  try { delete globalThis.SharedArrayBuffer; } catch (e) {}

  const t0 = Date.now() + Math.floor(_fpRand(641) * 100) - 50;
  // NavigationTiming offsets: DCL 60–600ms after navigationStart, load
  // DCL+40–800ms after that, with a plausible connect/response chain ahead
  // of domInteractive. All-equal epoch stamps (the old shape) made every
  // derived duration 0 — telemetry reading getEntriesByType('navigation')
  // got a zero-duration page, which is its own automation tell.
  const dcl = 60 + Math.floor(_fpRand(642) * 540);
  const load = dcl + 40 + Math.floor(_fpRand(643) * 760);
  // (#78) Chrome's timeOrigin is a float; navigationStart stays integral.
  globalThis.performance._timeOrigin = t0 + _fpRand(644) * 0.9;
  globalThis.performance._timing = {
    navigationStart: t0,
    fetchStart: t0 + 1, domainLookupStart: t0 + 2, domainLookupEnd: t0 + 12,
    connectStart: t0 + 12, connectEnd: t0 + 48, secureConnectionStart: t0 + 18,
    requestStart: t0 + 50, responseStart: t0 + 85, responseEnd: t0 + 130,
    domInteractive: t0 + Math.floor(dcl * 0.8),
    domContentLoadedEventStart: t0 + dcl - 4, domContentLoadedEventEnd: t0 + dcl,
    domComplete: t0 + load, loadEventStart: t0 + load, loadEventEnd: t0 + load,
  };
  // Derived entries (navigation/paint) are cached per navigation.
  if (globalThis.performance._resetDerived) globalThis.performance._resetDerived();
  globalThis.performance._memory = {
    jsHeapSizeLimit: 2172649472,
    totalJSHeapSize: 15000000 + Math.floor(_fpRand(620) * 85000000),
    usedJSHeapSize: 8000000 + Math.floor(_fpRand(621) * 42000000),
  };
  // Notification.permission stays "default" — the class default. A random
  // granted/default coin flip (old line) read as automation: fresh Chrome
  // profiles are always "default" until the OS prompt answers (fp-diff-1014 #8).

  // Hide internals (_*, obscura, Obscura). The set of keys is static at
  // snapshot-build time, so we precompute it ONCE below (after this
  // function definition) and reuse it on every page init. Was an
  // Object.keys + filter on every navigation, ~5-40ms per page on
  // SPAs that load 1000+ globals.
  const toHide = globalThis.__diting_hide_list || [];
  for (let i = 0; i < toHide.length; i++) {
    try { Object.defineProperty(globalThis, toHide[i], { enumerable: false }); } catch(e) {}
  }

  // Strip the deno runtime globals from the page's window. A `Deno` (or
  // `__bootstrap`) own property on window is the canonical marker of a
  // deno-based runtime — precisely what an AI-agent-blocker looks for — and
  // it survives enumerable:false because collectors enumerate with
  // getOwnPropertyNames, not Object.keys. All engine op calls run through
  // the script-scoped `_OPS` binding, so the globals are dead weight here.
  // Both are configurable in deno_core and delete cleanly; keep the try
  // guards in case a future deno_core makes them non-configurable.
  try { delete globalThis.Deno; } catch(e) {}
  try { delete globalThis.__bootstrap; } catch(e) {}

  // Non-configurable function declarations above (the engine's `_`-prefixed
  // helpers and `__diting*` bookkeeping) cannot be deleted, so hide them at
  // the enumeration boundary instead: Radar's windowFeatures collector
  // hashes Object.getOwnPropertyNames(window), and fingerprinting scripts
  // also enumerate with Reflect.ownKeys / Object.keys /
  // Object.getOwnPropertyDescriptors (upstream 4c33f6d). The wrappers only
  // filter the global object — every other receiver sees the untouched
  // native result — and are marked native so toString lie-detectors report
  // [native code].
  if (!globalThis.__diting_gopn_patched) {
    // Hide engine bookkeeping from window enumeration, but do NOT blanket-
    // strip every `_`-prefixed name. Chrome exposes page-installed props
    // like `_webmsxyw` (ark/itemark AS signer) in getOwnPropertyNames, and
    // xhs 300015 "risk plugin" probes hash that face. A blanket
    // `startsWith('_')` made our winKeys diverge on every page that
    // installs a `_` signer — leading candidate for the ark VM refusing
    // to take over (www-family X-s≈664 / no x-s-common vs Chrome
    // ark-family X-s≈408 + common≈2108).
    // Policy:
    //   - `__*` double-underscore: engine/recipe bookkeeping → hide
    //   - single `_` in the snapshot hide list: engine helpers → hide
    //   - `*diting*` / `__bootstrap`: engine → hide
    //   - page `_webmsxyw` and friends: visible like Chrome
    // `__diting_objects` & friends created by Rust AFTER the snapshot
    // froze the list are still caught by the diting / `__` arms.
    let _hideSet = null;
    // crypto_subtle.js / ed25519.js helper function declarations (block-level declarations hoist to global
    // scope and ed25519.js shares them): not `_`-prefixed, so the membership set above cannot catch them.
    const _extraHide = new Set(['makeKey', 'keyBytes', 'ecdsaUnflat', 'ecMaterial', 'requireEcdsaUsages', 'ecUsagesFor', 'normalizeAlgo',
      'normalizeHash', 'b64urlToBytes', 'bytesToB64url', 'runOp', 'keyAlgorithmFor', 'checkUsageStrings', 'checkKeyUsages', 'jwkRequire',
      'jwkMemberBytes', 'jwkValidateCommon', 'requireHashParam', 'rsaFace', 'rsaUnflat', 'rsaMaterial', 'rsaImportKey', 'rsaGenerateKey',
      'subtleCipher', 'checkUsages']);
    const _isInternal = (n) => {
      if (typeof n !== 'string') return false;
      if (_extraHide.has(n)) return true;
      if (n.includes('diting') || n.includes('Diting') || n === '__bootstrap') return true;
      if (n.startsWith('__')) return true;
      // `_` names and the scope-exported internals (the generated @@EXPORTS block registers them) come from the hide list
      if (_hideSet === null) {
        _hideSet = new Set(globalThis.__diting_hide_list || []);
      }
      return _hideSet.has(n);
    };
    const _gopn = Object.getOwnPropertyNames;
    const _ownKeys = Reflect.ownKeys;
    const _keys = Object.keys;
    const _gopds = Object.getOwnPropertyDescriptors;
    const _patch = (obj, key, impl) => {
      _markNative(impl);
      try { Object.defineProperty(impl, 'name', { value: key }); } catch(e) {}
      try { Object.defineProperty(obj, key, { value: impl, writable: true, enumerable: false, configurable: true }); } catch(e) {}
    };
    // Engine helpers living on PLATFORM interface objects (`_pump`, `_fire`, `_signalAbort` ...) are
    // not Chrome members; Chrome's own-name list for those prototypes never has them. Hide `_`-prefixed
    // names whenever the receiver is a platform interface constructor or its prototype (page classes
    // defined later are not in the set, so a page's own `_x` stays visible).
    const _ifaceObjs = new WeakSet();
    const _jsBuiltin = /^(Object|Function|Array|String|Number|Boolean|Symbol|BigInt|Date|RegExp|Promise|Proxy|Map|Set|WeakMap|WeakSet|WeakRef|FinalizationRegistry|ArrayBuffer|SharedArrayBuffer|DataView|Atomics|Intl|WebAssembly|(Eval|Range|Reference|Syntax|Type|URI|Aggregate)?Error|(Big)?(Int|Uint|Float)(8|16|32|64)(Clamped)?Array|Iterator)$/;
    for (const n of _gopn(globalThis)) {
      if (!/^[A-Z]/.test(n) || _jsBuiltin.test(n)) continue;
      let d; try { d = Object.getOwnPropertyDescriptor(globalThis, n); } catch (e) { continue; }
      const C = d && d.value;
      if (typeof C === 'function') {
        _ifaceObjs.add(C);
        if (C.prototype && typeof C.prototype === 'object') _ifaceObjs.add(C.prototype);
      }
    }
    const _gpo = Object.getPrototypeOf;
    const _hideUnderscore = (names) => names.filter(n => typeof n !== 'string' || n.charCodeAt(0) !== 95);
    // instances: only the names the engine's own scripts use (internal_names.js, generated) - a page's own `_x`
    // expando on document / performance / any platform object stays visible like in Chrome.
    const _hideInternalOnInstance = (names) => {
      const set = globalThis.__diting_internal_names, ap = globalThis.__diting_accPrivs;
      return set ? names.filter(n => typeof n !== 'string' || !(set.has(n) || (ap && ap.has(n)) || /^_on[a-z]+$/.test(n))) : names;
    };
    const _hideOn = (o, names) => {
      if (o === globalThis) return names.filter(n => !_isInternal(n));
      if (o === null || (typeof o !== 'object' && typeof o !== 'function')) return names;
      if (_ifaceObjs.has(o)) return _hideUnderscore(names);
      // an INSTANCE of a platform interface (its direct prototype is the platform prototype): the engine keeps
      // helper state in `_x` own properties that Chrome's instances do not have. Instances of page-defined
      // subclasses have a page prototype and keep their `_x` (Lit/Stencil style fields) visible.
      if (typeof o === 'object') { let pr; try { pr = _gpo(o); } catch (e) { return names; } if (pr !== null && _ifaceObjs.has(pr)) return _hideInternalOnInstance(names); }
      return names;
    };
    _patch(Object, 'getOwnPropertyNames', function(o) { return _hideOn(o, _gopn(o)); });
    _patch(Reflect, 'ownKeys', function(o) { return _hideOn(o, _ownKeys(o)); });
    _patch(Object, 'keys', function(o) { return _hideOn(o, _keys(o)); });
    _patch(Object, 'getOwnPropertyDescriptors', function(o) {
      const all = _gopds(o);
      if (o === globalThis) { for (const n of _gopn(all)) { if (_isInternal(n)) delete all[n]; } }
      else if (o !== null && typeof o === 'object' || typeof o === 'function') {
        let hide = _ifaceObjs.has(o);
        if (!hide && typeof o === 'object') { try { const pr = _gpo(o); hide = pr !== null && _ifaceObjs.has(pr); } catch (e) {} }
        if (hide) { const keep = typeof o === 'object' && !_ifaceObjs.has(o) ? _hideInternalOnInstance : _hideUnderscore; const kept = new Set(keep(_gopn(all))); for (const n of _gopn(all)) { if (!kept.has(n)) delete all[n]; } }
      }
      return all;
    });
    globalThis.__diting_gopn_patched = true;
  }
  if (typeof globalThis.__diting_demote_own === 'function') { try { globalThis.__diting_demote_own(); } catch (e) {} }
  if (typeof globalThis.__diting_window_accessors === 'function') { try { globalThis.__diting_window_accessors(); } catch (e) {} }
  if (typeof globalThis.__diting_reorder_window === 'function') { try { globalThis.__diting_reorder_window(true); } catch (e) {} }
  delete globalThis.__diting_init;
};

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
