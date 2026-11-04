(function () { // @@SCOPE
// god-file split: bootstrap's real native marker + prototype reorder, both
// reached through hidden/d()-published globals (house style — serializer.js /
// the 刀5 alias device; exports-extra.json pins the reorder publish to
// bootstrap so the alias cannot flip gen-exports' attribution).
const _markNative = globalThis.__diting_markNative;
const __ditingReorderProto = globalThis.__ditingReorderProto;

// languages is a frozen array and Chrome hands back the same object while
// the locale is unchanged (moli ca22988 same face). Module-scope cache —
// never on __navStore, whose own names feed the prototype setup walk.
let __navLangsKey = null, __navLangsVal = null;
const __navStore = {
  vendorSub: "",
  productSub: "20030107",
  vendor: "Google Inc.",
  maxTouchPoints: 0,
  scheduling: { isInputPending() { return false; } },
  get userActivation() { return __diting_userActivation; },
  geolocation: {
    getCurrentPosition(success, error) {
      const pos = globalThis.__diting_makeGeolocationPosition(
        50.1109 + (_fpRand(500) - 0.5) * 0.1,
        8.6821 + (_fpRand(501) - 0.5) * 0.1,
        10 + _fpRand(502) * 40,
      );
      if (typeof success === 'function') success(pos);
    },
    watchPosition(success, error) {
      if (typeof success === 'function') {
        success(globalThis.__diting_makeGeolocationPosition(
          50.1109 + (_fpRand(503) - 0.5) * 0.1,
          8.6821 + (_fpRand(504) - 0.5) * 0.1,
          10 + _fpRand(505) * 40,
        ));
      }
      return 0;
    },
    clearWatch() {},
  },
  doNotTrack: null,
  webkitTemporaryStorage: { queryUsageAndQuota(cb) { if (cb) cb(0, 5000000000); }, requestQuota() {} },
  webkitPersistentStorage: { queryUsageAndQuota(cb) { if (cb) cb(0, 500000000); }, requestQuota() {} },
  windowControlsOverlay: { visible: false, ontitlebarchange: null, addEventListener(){}, removeEventListener(){}, dispatchEvent(){ return false; } },
  hardwareConcurrency: 8,
  cookieEnabled: true,
  appCodeName: "Mozilla",
  appName: "Netscape",
  get appVersion() { return this.userAgent.replace('Mozilla/', ''); },
  get platform() { return globalThis.__diting_platform_override || __ditingPlatformFromUA(); },
  product: "Gecko",
  get userAgent() { return globalThis.__diting_ua || "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36"; },
  get language() { return __ditingLangList()[0]; },
  get languages() {
    const list = __ditingLangList();
    const key = list.join(',');
    if (__navLangsKey !== key) {
      __navLangsKey = key;
      __navLangsVal = Object.freeze(list.slice());
    }
    return __navLangsVal;
  },
  onLine: true,
  // (#78) Chrome's NavigatorAutomationInformation always defines webdriver
  // and reads false on a normal session — `undefined` is the odd one out and
  // a jsvmp scan tell.
  get webdriver() { return false; },
  get plugins() {
    __ditingBuildPdfPersona();
    return _pluginsInst;
  },
  get mimeTypes() {
    __ditingBuildPdfPersona();
    return _mimeTypesInst;
  },
  pdfViewerEnabled: true,
  connection: Object.create(globalThis.NetworkInformation.prototype),
  getGamepads() { return []; },
  javaEnabled() { return false; },
  sendBeacon(url, data) {
    let abs;
    try {
      abs = new URL(String(url), _docBase()).href;
    } catch (e) {
      return false;
    }
    let body = data == null ? "" : data;
    let hdrs;
    if (typeof data === "string") {
      hdrs = { "content-type": "text/plain;charset=UTF-8" };
    } else if (typeof Blob === "function" && data instanceof Blob) {
      // fetch already rides blob.type as the content-type.
    } else if (typeof URLSearchParams === "function" && data instanceof URLSearchParams) {
      // fetch already applies the urlencoded content-type.
    } else if (data instanceof ArrayBuffer || ArrayBuffer.isView(data)) {
      hdrs = { "content-type": "application/octet-stream" };
    } else if (typeof FormData === "function" && data instanceof FormData) {
      // fetch serializes multipart with a generated boundary.
    } else {
      body = String(data);
      hdrs = { "content-type": "text/plain;charset=UTF-8" };
    }
    // Fire-and-forget: a beacon outlives the page that queued it, so the
    // response is never observed — the promise is discarded and its rejection
    // swallowed (a failing endpoint must not surface as an unhandled
    // rejection). true means the transfer was queued, not delivered.
    const init = { method: "POST", body, mode: "no-cors", credentials: "include", keepalive: true };
    if (hdrs) init.headers = hdrs;
    Promise.resolve(globalThis.fetch(abs, init)).catch(() => {});
    return true;
  },
  vibrate() { return false; },
  deprecatedRunAdAuctionEnforcesKAnonymity: false,
  protectedAudience: {},
  bluetooth: {
    getAvailability() { return Promise.resolve(false); },
    requestDevice() { return Promise.reject(new DOMException("Permission denied.", "NotAllowedError")); },
  },
  clipboard: { writeText(){return Promise.resolve();}, readText(){return Promise.resolve("");} },
  credentials: { get(){return Promise.resolve(null);}, create(){return Promise.resolve(null);}, store(){return Promise.resolve();}, preventSilentAccess(){return Promise.resolve();} },
  get keyboard() { return __diting_keyboard; },
  managed: { getManagedConfiguration() { return Promise.resolve({}); } },
  mediaDevices: {
    enumerateDevices() {
      return Promise.resolve([
        {deviceId:"default",kind:"audioinput",label:"",groupId:"default"},
        {deviceId:"comms",kind:"audioinput",label:"",groupId:"comms"},
        {deviceId:"default",kind:"audiooutput",label:"",groupId:"default"},
        {deviceId:"",kind:"videoinput",label:"",groupId:""},
      ]);
    },
    getUserMedia() { return Promise.reject(new DOMException("NotAllowedError")); },
    getDisplayMedia() { return Promise.reject(new DOMException("NotAllowedError")); },
  },
  serviceWorker: (function () {
    // Chrome's register() resolves a ServiceWorkerRegistration; pages chain
    // reg.scope / reg.unregister() straight off the promise (#83) — the
    // previous stub resolved undefined and the .scope read threw. ready
    // carries the same shape. No worker ever runs; controller stays null,
    // getRegistration keeps resolving undefined so "is one registered?"
    // checks still take the no-SW branch.
    // (300015 P7) register must FOLLOW the script fetch: Chrome only
    // resolves when the worker script answers ok with a JavaScript MIME
    // type. ark registers a script that 404s on the CDN, so real Chrome
    // REJECTS and the app swallows it in .catch() — an unconditional
    // resolve is a one-line bot check (.then() on a dead script = fake
    // browser). Chrome truth (CDP 155, 10-15):
    //   cross-origin -> SecurityError "Failed to register a ServiceWorker:
    //     The origin of the provided scriptURL ('{origin}') does not match
    //     the current origin ('{origin}')." (checked before any fetch)
    //   !ok status  -> TypeError "Failed to register a ServiceWorker for
    //     scope ('{scope}') with script ('{url}'): A bad HTTP response code
    //     ({status}) was received when fetching the script."
    //   non-JS MIME -> SecurityError, same prefix, "The script has an
    //     unsupported MIME type ('{mime}')."
    // Net-failure tail uncaptured (a same-origin dead host can't be staged);
    // Chromium-string best effort. The ok path keeps the stub registration —
    // no worker runs either way.
    function registration(scope) {
      return {
        scope: scope,
        updateViaCache: 'imports',
        active: null, installing: null, waiting: null,
        update: function () { return Promise.resolve(undefined); },
        unregister: function () { return Promise.resolve(true); },
        addEventListener: function () {}, removeEventListener: function () {},
        dispatchEvent: function () { return false; },
      };
    }
    function dirScope(url) {
      return url.slice(0, url.lastIndexOf('/') + 1);
    }
    function swFail(ctor, tail, name) {
      return name ? new ctor(tail, name) : new ctor(tail);
    }
    var JS_MIME = /^(application|text)\/(x-)?(javascript|ecmascript)$/;
    return {
      get ready() { return Promise.resolve(registration(dirScope(location.href))); },
      register: function (scriptUrl, options) {
        var su, scope, suHref;
        try {
          su = new URL(scriptUrl, location.href);
        } catch (e) {
          return Promise.reject(new TypeError("Failed to register a ServiceWorker: The URL is not valid."));
        }
        suHref = su.href;
        try {
          scope = (options && options.scope)
            ? new URL(options.scope, location.href).href
            : dirScope(suHref);
        } catch (e) { scope = dirScope(location.href); }
        if (su.origin !== location.origin) {
          return Promise.reject(swFail(DOMException,
            "Failed to register a ServiceWorker: The origin of the provided scriptURL ('" +
            su.origin + "') does not match the current origin ('" + location.origin + "').", "SecurityError"));
        }
        return fetch(suHref).then(function (r) {
          if (!r.ok) {
            throw swFail(TypeError,
              "Failed to register a ServiceWorker for scope ('" + scope + "') with script ('" +
              suHref + "'): A bad HTTP response code (" + r.status + ") was received when fetching the script.");
          }
          var ct = (r.headers.get('content-type') || '').split(';')[0].trim().toLowerCase();
          if (!JS_MIME.test(ct)) {
            throw swFail(DOMException,
              "Failed to register a ServiceWorker for scope ('" + scope + "') with script ('" +
              suHref + "'): The script has an unsupported MIME type ('" + ct + "').", "SecurityError");
          }
          return registration(scope);
        }, function () {
          throw swFail(TypeError,
            "Failed to register a ServiceWorker for scope ('" + scope + "') with script ('" +
            suHref + "'): Registration failed - the worker script fetch failed.");
        });
      },
      getRegistrations(){ return Promise.resolve([]); },
      getRegistration(){ return Promise.resolve(undefined); },
      controller: null,
    };
  })(),
  virtualKeyboard: {
    boundingRect: { x: 0, y: 0, width: 0, height: 0 },
    overlaysContent: false,
    show(){}, hide(){}, setBoundingRect(){},
    addEventListener(){}, removeEventListener(){},
  },
  wakeLock: { request() { return Promise.reject(new DOMException("Permission denied.", "NotAllowedError")); } },
  deviceMemory: 8,
  userAgentData: Object.create(globalThis.NavigatorUAData.prototype),
  locks: { request() { return Promise.resolve(); }, query() { return Promise.resolve([]); } },
  storage: {
    estimate() { return Promise.resolve({ quota: 5000000000, usage: Math.floor(_fpRand(640) * 100000000) }); },
    persist() { return Promise.resolve(false); },
    persisted() { return Promise.resolve(false); },
  },
  gpu: {
    requestAdapter() { return Promise.resolve(null); },
    getPreferredCanvasFormat() { return 'bgra8unorm'; },
    wgslLanguageFeatures: { has() { return false; }, size: 0 },
  },
  login: { status: "logged-out", setStatus() { return Promise.resolve(); } },
  ink: { requestPresenter() { return Promise.resolve(null); }, debouncePresenter() { return Promise.resolve(null); } },
  mediaCapabilities: {
    decodingInfo() { return Promise.resolve({ supported: true, smooth: true, powerEfficient: true }); },
    encodingInfo() { return Promise.resolve({ supported: true, smooth: true, powerEfficient: true }); },
  },
  permissions: { query(params){
    if (params?.name === 'notifications') return Promise.resolve({state:"prompt",onchange:null});
    return Promise.resolve({state:"granted"});
  } },
  devicePosture: { type: "continuous" },
  hid: { getDevices() { return Promise.resolve([]); }, requestDevice() { return Promise.reject(new DOMException("Permission denied.", "NotAllowedError")); } },
  mediaSession: {
    metadata: null, playbackState: "none",
    setActionHandler(){}, setPositionState(){}, setCameraActive(){}, setMicrophoneActive(){},
  },
  presentation: { defaultRequest: null, receiver: null },
  serial: { getPorts() { return Promise.resolve([]); }, requestPort() { return Promise.reject(new DOMException("Permission denied.", "NotAllowedError")); } },
  usb: { getDevices() { return Promise.resolve([]); }, requestDevice() { return Promise.reject(new DOMException("Permission denied.", "NotAllowedError")); } },
  xr: { isSessionSupported() { return Promise.resolve(false); }, ondevicechange: null, addEventListener(){}, removeEventListener(){} },
  storageBuckets: { keys() { return Promise.resolve([]); }, open() { return Promise.resolve(null); }, delete() { return Promise.resolve(false); }, persist() { return Promise.resolve(false); } },
  adAuctionComponents() { return Promise.resolve([]); },
  runAdAuction() { return Promise.resolve(null); },
  canLoadAdAuctionFencedFrame() { return false; },
  canShare() { return false; },
  share() { return Promise.reject(new DOMException("Permission denied.", "NotAllowedError")); },
  clearAppBadge() { return Promise.resolve(); },
  getBattery() {
    const bm = Object.create(globalThis.BatteryManager.prototype);
    bm._charging = _fp('batteryCharging');
    bm._chargingTime = _fp('batteryCharging') ? 0 : Infinity;
    bm._dischargingTime = _fp('batteryCharging') ? Infinity : Math.floor(3600 + _fpRand(250) * 7200);
    bm._level = _fp('batteryLevel');
    return Promise.resolve(bm);
  },
  getUserMedia() { return Promise.reject(new DOMException("Permission denied.", "NotAllowedError")); },
  requestMIDIAccess() { return Promise.reject(new DOMException("Permission denied.", "NotAllowedError")); },
  requestMediaKeySystemAccess() { return Promise.reject(new DOMException("Unsupported key system", "NotSupportedError")); },
  setAppBadge() { return Promise.resolve(); },
  webkitGetUserMedia(cb, err) { if (err) err(new DOMException("Permission denied.", "NotAllowedError")); },
  clearOriginJoinedAdInterestGroups() {},
  createAuctionNonce() { return (globalThis.crypto && globalThis.crypto.randomUUID) ? globalThis.crypto.randomUUID() : ""; },
  joinAdInterestGroup() { return Promise.resolve(); },
  leaveAdInterestGroup() { return Promise.resolve(); },
  updateAdInterestGroups() {},
  deprecatedReplaceInURN() { return Promise.resolve(null); },
  deprecatedURNToURL() { return Promise.resolve(null); },
  getInstalledRelatedApps() { return Promise.resolve([]); },
  getInterestGroupAdAuctionData() { return Promise.resolve(null); },
  registerProtocolHandler() {},
  unregisterProtocolHandler() {},
};
// (#104) Chrome exposes the Navigator interface object on window — the
// Douyin security SDK subclasses it (`class t extends Navigator`) and died
// with ReferenceError. Illegal constructor like every Chrome interface
// without [Constructor]. (#117) all attributes live on the prototype now, so
// instanceof/getPrototypeOf/key-parity all match Chrome. Prototype accessors
// survive the V8 snapshot (#37).
globalThis.Navigator = class Navigator {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
};
for (const _nk of Object.getOwnPropertyNames(__navStore)) {
  const _d = Object.getOwnPropertyDescriptor(__navStore, _nk);
  const _k = _nk;
  if (_d.get || typeof _d.value !== 'function') {
    // Attributes stay getter-only prototype accessors — page writes no-op,
    // exactly like Chrome's readonly navigator attributes (#117).
    Object.defineProperty(globalThis.Navigator.prototype, _k, {
      configurable: true, enumerable: true,
      get: _d.get ? function () { return _d.get.call(__navStore); } : function () { return __navStore[_k]; },
    });
  } else {
    // WebIDL operations are writable data properties on the prototype.
    // #203: instrumentation SDKs (抖店 secsdk's createAspectByPath) reassign
    // navigator.sendBeacon to wrap it; a getter-only accessor turned that
    // strict-mode assignment into a TypeError. Assignment shadows via an
    // own property on `navigator` — the store and __diting_navSet stay
    // untouched, like Chrome.
    Object.defineProperty(globalThis.Navigator.prototype, _k, {
      configurable: true, enumerable: true, writable: true,
      value: _d.value,
    });
  }
}
globalThis.__def(globalThis.Navigator.prototype, Symbol.toStringTag, 'Navigator');
globalThis.__def(globalThis, Symbol.toStringTag, 'Window');
globalThis.navigator = Object.create(globalThis.Navigator.prototype);
// The single sanctioned write path for navigator state (persona refresh,
// viewport emulation). Pages writing navigator.* directly hit the
// getter-only prototype accessors and silently no-op — Chrome parity.
globalThis.__diting_navSet = function(k, v) {
  Object.defineProperty(__navStore, k, { value: v, writable: true, configurable: true, enumerable: true });
};

if (typeof navigator.credentials === 'undefined') {
  navigator.credentials = { get(){return Promise.resolve(null);}, create(){return Promise.resolve(null);}, store(){return Promise.resolve();}, preventSilentAccess(){return Promise.resolve();} };
}

try {
  __ditingReorderProto(Navigator.prototype, [
    'vendorSub','productSub','vendor','maxTouchPoints','scheduling','userActivation','geolocation','doNotTrack',
    'webkitTemporaryStorage','webkitPersistentStorage','windowControlsOverlay','hardwareConcurrency','cookieEnabled',
    'appCodeName','appName','appVersion','platform','product','userAgent','language','languages','onLine','webdriver',
    'plugins','mimeTypes','pdfViewerEnabled','connection','getGamepads','javaEnabled','sendBeacon','vibrate','constructor',
    'deprecatedRunAdAuctionEnforcesKAnonymity','protectedAudience','bluetooth','clipboard','credentials',
    'keyboard','managed','mediaDevices','serviceWorker','virtualKeyboard','wakeLock','deviceMemory','userAgentData',
    'locks','storage','gpu','login','ink','mediaCapabilities','permissions','devicePosture','hid','mediaSession',
    'presentation','serial','usb','xr','storageBuckets','adAuctionComponents','runAdAuction','canLoadAdAuctionFencedFrame',
    'canShare','share','clearAppBadge','getBattery','getUserMedia','requestMIDIAccess','requestMediaKeySystemAccess',
    'setAppBadge','webkitGetUserMedia','clearOriginJoinedAdInterestGroups','createAuctionNonce','joinAdInterestGroup',
    'leaveAdInterestGroup','updateAdInterestGroups','deprecatedReplaceInURN','deprecatedURNToURL','getInstalledRelatedApps',
    'getInterestGroupAdAuctionData','registerProtocolHandler','unregisterProtocolHandler',
  ]);
} catch (e) {}

// The navigator cluster of bootstrap's big mark list: the list runs before
// this file loads, so the entries moved with the singleton they read.
[
  navigator.getBattery, navigator.getGamepads, navigator.sendBeacon,
  navigator.javaEnabled, navigator.geolocation?.getCurrentPosition,
  navigator.geolocation?.watchPosition,
  navigator.serviceWorker?.register,
  navigator.serviceWorker?.addEventListener,
  navigator.serviceWorker?.removeEventListener,
  navigator.serviceWorker?.dispatchEvent,
  navigator.serviceWorker?.getRegistration,
  navigator.permissions?.query, navigator.credentials?.get,
  navigator.storage?.estimate, navigator.storage?.persist, navigator.storage?.persisted,
].forEach(fn => { if (typeof fn === 'function') _markNative(fn); });

// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('_markNative', _markNative);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
