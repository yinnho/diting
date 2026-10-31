// 刀65: Document's 24 missing members (the missing-83 ledger's biggest row)
// plus the body setter and the FeaturePolicy instance machinery. Every
// expectation carries its Chrome 145 probe receipt (chrome 145.0.7632.117,
// probe legs in /tmp/diting-shape/k65-*) or a GUESS marker. Loads after
// web_api_gaps (wraps its startViewTransition) and BEFORE stub_members, so
// the FeaturePolicy method takeover below wins the fill-if-missing pass.
(() => {
  if (typeof Document === 'undefined' || !Document.prototype) return;
  const DP = Document.prototype;
  const mark = (fn) => { if (typeof fn === 'function') _markNative(fn); return fn; };
  const def = (obj, name, d) => Object.defineProperty(obj, name, d);
  // Accessor pair with Chrome's getter/setter naming and ec flags. Setters
  // get length 1 (gen_iface's installOn convention: WebIDL setter arity).
  const acc = (name, get, set) => {
    def(get, 'name', { value: 'get ' + name, configurable: true });
    if (set) {
      def(set, 'name', { value: 'set ' + name, configurable: true });
      def(set, 'length', { value: 1, configurable: true });
    }
    mark(get); if (set) mark(set);
    const d = { enumerable: true, configurable: true, get };
    if (set) d.set = set; // setter omitted = getter-only; null would be an invalid descriptor
    def(DP, name, d);
  };
  // Method with ecw flags; `f` must be a named function expression whose
  // signature already carries the right arity.
  const meth = (name, f) => {
    mark(f);
    def(DP, name, { value: f, writable: true, enumerable: true, configurable: true });
  };

  // ---- deprecated XML trio (probe: k65-chrome.js) ----------------------
  // xmlEncoding is a getter-only null on HTML documents; the setter
  // probe ('utf-16' assignment) never threw and never stuck.
  // xmlVersion: read null; '1.0' stores and reads back; anything else
  // (including null) throws NotSupportedError naming the attempted
  // version - null stringifies as '' in Chrome's message. xmlStandalone
  // is a boolean store ('nope' coerced to true).
  let _xmlVersion = null, _xmlStandalone = false;
  acc('xmlEncoding', function () { return null; }, null);
  acc('xmlVersion',
    function () { return _xmlVersion; },
    function (v) {
      const s = v === null ? '' : String(v);
      if (s !== '1.0') {
        throw new DOMException("Failed to set the 'xmlVersion' property on 'Document': This document does not support the XML version '" + s + "'.", 'NotSupportedError');
      }
      _xmlVersion = '1.0';
    });
  acc('xmlStandalone',
    function () { return _xmlStandalone; },
    function (v) { _xmlStandalone = !!v; });

  // ---- seven on* handlers (probe: null default, fn round trip) ---------
  // Same store semantics as the generated installOn: per-instance
  // __onHandlers map, non-function assignments coerce to null. No inline
  // content-attribute fallback (Document has no attributes).
  const installOn = (name) => {
    const get = function () {
      const store = this.__onHandlers;
      if (store && Object.prototype.hasOwnProperty.call(store, name)) return store[name];
      return null;
    };
    const set = function (fn) {
      if (typeof fn !== 'function') fn = null;
      const store = this.__onHandlers || (this.__onHandlers = {});
      store[name] = fn;
    };
    def(get, 'name', { value: 'get ' + name, configurable: true });
    def(set, 'name', { value: 'set ' + name, configurable: true });
    def(set, 'length', { value: 1, configurable: true });
    mark(get); mark(set);
    def(DP, name, { enumerable: true, configurable: true, get, set });
  };
  for (const n of ['onreadystatechange', 'onpointerlockchange', 'onpointerlockerror',
    'onfreeze', 'onprerenderingchange', 'onresume', 'onvisibilitychange']) installOn(n);

  // ---- webkit visibility + fullscreen mirrors --------------------------
  // webkitVisibilityState/webkitHidden track the live pair; the four
  // fullscreen faces mirror their standard twins (engine never enters
  // fullscreen: IsFullScreen false, CurrentFullScreenElement null).
  acc('webkitVisibilityState', function () { return this.visibilityState; }, null);
  acc('webkitHidden', function () { return this.hidden; }, null);
  acc('webkitIsFullScreen', function () { return false; }, null);
  acc('webkitCurrentFullScreenElement', function () { return null; }, null);
  acc('webkitFullscreenEnabled', function () { return this.fullscreenEnabled; }, null);
  acc('webkitFullscreenElement', function () { return this.fullscreenElement; }, null);

  // ---- FeaturePolicy: instance + working methods ------------------------
  // FeaturePolicy itself is a pure-stub face; before stub_members runs we
  // take the four prototype methods over (its install is fill-if-missing)
  // so document.featurePolicy answers with Chrome's default allowlist.
  // Lists captured from Chrome 145 on a plain http page (80 allowed,
  // 81 known - the extra feature is 'unload'). getAllowlistForFeature
  // returns [origin] for allowed features, [] otherwise; fresh array per
  // allowedFeatures() call; plain 'Illegal invocation' on foreign receivers.
  const FP_ALLOWED = [
    'geolocation', 'ch-ua-full-version-list', 'cross-origin-isolated',
    'screen-wake-lock', 'on-device-speech-recognition', 'translator',
    'publickey-credentials-get', 'shared-storage-select-url', 'ch-ua-arch',
    'bluetooth', 'compute-pressure', 'ch-prefers-reduced-transparency',
    'deferred-fetch', 'usb', 'ch-save-data',
    'publickey-credentials-create', 'shared-storage', 'deferred-fetch-minimal',
    'run-ad-auction', 'ch-downlink', 'ch-ua-form-factors',
    'otp-credentials', 'payment', 'ch-ua',
    'ch-ua-model', 'ch-ect', 'autoplay',
    'camera', 'language-detector', 'private-state-token-issuance',
    'digital-credentials-get', 'accelerometer', 'ch-ua-platform-version',
    'idle-detection', 'private-aggregation', 'interest-cohort',
    'ch-viewport-height', 'captured-surface-control', 'local-fonts',
    'ch-ua-platform', 'midi', 'ch-ua-full-version',
    'xr-spatial-tracking', 'clipboard-read', 'gamepad',
    'display-capture', 'keyboard-map', 'join-ad-interest-group',
    'aria-notify', 'local-network', 'ch-ua-high-entropy-values',
    'ch-width', 'ch-prefers-reduced-motion', 'browsing-topics',
    'encrypted-media', 'local-network-access', 'gyroscope',
    'serial', 'ch-rtt', 'ch-ua-mobile',
    'window-management', 'ch-dpr', 'ch-prefers-color-scheme',
    'ch-ua-wow64', 'attribution-reporting', 'fullscreen',
    'identity-credentials-get', 'private-state-token-redemption', 'hid',
    'summarizer', 'ch-ua-bitness', 'storage-access',
    'sync-xhr', 'ch-device-memory', 'ch-viewport-width',
    'picture-in-picture', 'magnetometer', 'loopback-network',
    'clipboard-write', 'microphone',
  ];
  const FP_FEATURES = FP_ALLOWED.concat(['unload']);
  if (typeof FeaturePolicy === 'function' && FeaturePolicy.prototype) {
    const FPP = FeaturePolicy.prototype;
    const fpOK = (t) => t !== null && typeof t === 'object' && t !== FPP && Object.prototype.isPrototypeOf.call(FPP, t);
    const fpMeth = (name, f) => {
      mark(f);
      def(FPP, name, { value: f, writable: true, enumerable: true, configurable: true });
    };
    fpMeth('allowedFeatures', function allowedFeatures() {
      if (!fpOK(this)) throw new TypeError('Illegal invocation');
      return FP_ALLOWED.slice();
    });
    fpMeth('allowsFeature', function allowsFeature(feature) {
      if (!fpOK(this)) throw new TypeError('Illegal invocation');
      if (arguments.length < 1) throw new TypeError("Failed to execute 'allowsFeature' on 'FeaturePolicy': 1 argument required, but only 0 present.");
      return FP_ALLOWED.indexOf(String(feature)) >= 0;
    });
    fpMeth('features', function features() {
      if (!fpOK(this)) throw new TypeError('Illegal invocation');
      return FP_FEATURES.slice();
    });
    fpMeth('getAllowlistForFeature', function getAllowlistForFeature(feature) {
      if (!fpOK(this)) throw new TypeError('Illegal invocation');
      if (arguments.length < 1) throw new TypeError("Failed to execute 'getAllowlistForFeature' on 'FeaturePolicy': 1 argument required, but only 0 present.");
      if (FP_ALLOWED.indexOf(String(feature)) < 0) return [];
      const o = globalThis.location && globalThis.location.origin;
      return [String(o)];
    });
  }
  // The member itself: one cached object per Chrome probe (featurePolicy ===
  // featurePolicy), bare FeaturePolicy.prototype instance, no own props.
  let _fp = null;
  acc('featurePolicy', function () {
    if (_fp === null && typeof FeaturePolicy === 'function') _fp = Object.create(FeaturePolicy.prototype);
    return _fp;
  }, null);

  // ---- activeViewTransition + startViewTransition wrap ------------------
  // null at rest; during a transition it IS the object startViewTransition
  // returned; cleared once finished settles (probe k65-chrome3).
  let _avt = null;
  acc('activeViewTransition', function () { return _avt; }, null);
  const _svtOrig = DP.startViewTransition;
  if (typeof _svtOrig === 'function') {
    meth('startViewTransition', function startViewTransition(update) {
      const vt = _svtOrig.call(this, update);
      _avt = vt;
      if (vt && vt.finished && typeof vt.finished.then === 'function') {
        const clear = () => { if (_avt === vt) _avt = null; };
        vt.finished.then(clear, clear);
      }
      return vt;
    });
  }

  // ---- caretRangeFromPoint ----------------------------------------------
  // Chrome 145 anchors the collapsed Range on the ELEMENT at the point
  // (startContainer nodeType 1, offset 0); out-of-range returns null.
  // Arity 0 (truth function:0) - params ride arguments. The text-node
  // interior case is a GUESS (probed legs only hit elements).
  meth('caretRangeFromPoint', function caretRangeFromPoint() {
    const el = this.elementFromPoint(Number(arguments[0]), Number(arguments[1]));
    if (!el) return null;
    const r = this.createRange();
    r.setStart(el, 0);
    r.setEnd(el, 0);
    return r;
  });

  // ---- requestStorageAccessFor ------------------------------------------
  // Missing arg TypeError; non-URL/number arg sync TypeError 'Invalid
  // origin'; a valid http(s) origin returns a promise REJECTED
  // NotAllowedError 'requestStorageAccessFor not allowed' (headless probe).
  meth('requestStorageAccessFor', function requestStorageAccessFor(requestedOrigin) {
    if (arguments.length < 1) throw new TypeError("Failed to execute 'requestStorageAccessFor' on 'Document': 1 argument required, but only 0 present.");
    let u = null;
    try { u = new URL(String(requestedOrigin)); } catch (e) {}
    if (!u || (u.protocol !== 'http:' && u.protocol !== 'https:')) throw new TypeError('Invalid origin');
    return Promise.reject(new DOMException('requestStorageAccessFor not allowed', 'NotAllowedError'));
  });

  // ---- webkit fullscreen verbs (undefined return; GUESS: forward) -------
  meth('webkitCancelFullScreen', function webkitCancelFullScreen() {
    if (typeof this.exitFullscreen === 'function') this.exitFullscreen();
  });
  meth('webkitExitFullscreen', function webkitExitFullscreen() {
    if (typeof this.exitFullscreen === 'function') this.exitFullscreen();
  });

  // ---- browsingTopics ----------------------------------------------------
  // Returns a promise resolving [] on a plain page (no topics context).
  meth('browsingTopics', function browsingTopics() {
    return Promise.resolve([]);
  });

  // ---- ariaNotify ---------------------------------------------------------
  // (DOMString message, optional AriaNotificationOptions options) -> void;
  // truth arity 1 - options rides arguments[1]. 0 args TypeError; second
  // arg present and non-object TypeError.
  meth('ariaNotify', function ariaNotify(message) {
    if (arguments.length < 1) throw new TypeError("Failed to execute 'ariaNotify' on 'Document': 1 argument required, but only 0 present.");
    const options = arguments[1];
    if (arguments.length > 1 && options !== undefined && options !== null && typeof options !== 'object') {
      throw new TypeError("Failed to execute 'ariaNotify' on 'Document': The provided value is not of type 'AriaNotificationOptions'.");
    }
  });

  // ---- body setter (kind fix ro -> rw) ------------------------------------
  // Chrome: null -> HierarchyRequestError "1st argument" message; a
  // non-object -> TypeError convert message; a BODY/FRAMESET replaces the
  // current body through documentElement.replaceChild; any other element
  // type -> HierarchyRequestError naming the tag. The getter is re-pointed
  // with a frameset fallback so the swap reads straight through (after
  // body = <frameset>, Chrome's document.body IS the frameset; the
  // bootstrap getter only matched <body>, leaving the setter's restore
  // path holding null).
  const _bodyDesc = Object.getOwnPropertyDescriptor(DP, 'body');
  if (_bodyDesc && _bodyDesc.get && !_bodyDesc.set) {
    const _bodyGetOrig = _bodyDesc.get;
    const _bodyGet = function () {
      return _bodyGetOrig.call(this) || this.querySelector('frameset');
    };
    def(_bodyGet, 'name', { value: 'get body', configurable: true });
    const _bodySet = function (v) {
      if (v === null) {
        throw new DOMException("Failed to set the 'body' property on 'Document': The 1st argument provided is either null, or an invalid HTMLElement object.", 'HierarchyRequestError');
      }
      if (typeof v !== 'object' || v === null || v.nodeType !== 1) {
        throw new TypeError("Failed to set the 'body' property on 'Document': Failed to convert value to 'HTMLElement'.");
      }
      const tag = v.nodeName;
      if (tag !== 'BODY' && tag !== 'FRAMESET') {
        throw new DOMException("Failed to set the 'body' property on 'Document': The new body element is of type '" + tag + "'. It must be either a 'BODY' or 'FRAMESET' element.", 'HierarchyRequestError');
      }
      const cur = _bodyGet.call(this);
      if (cur === v) return;
      this.documentElement.replaceChild(v, cur);
    };
    def(_bodySet, 'name', { value: 'set body', configurable: true });
    def(_bodySet, 'length', { value: 1, configurable: true });
    mark(_bodyGet); mark(_bodySet);
    def(DP, 'body', { enumerable: true, configurable: true, get: _bodyGet, set: _bodySet });
  }
})();
