(function () { // @@SCOPE
const _markNative = globalThis.__diting_markNative;

// god-file split (刀22, bchkAPIs faces): the batch166 anti-bot probe
// surface — illegal-constructor interface faces Chrome exposes for
// `'X' in window` checks (FragmentDirective, UserActivation, Keyboard,
// LaunchQueue, BatteryManager, Geolocation pair + factory, MediaDeviceInfo,
// MediaMetadata, PasswordCredential, Scheduling, TextTrackCue,
// ViewTransition, WebTransport, WebSocketStream, CustomStateSet,
// IIRFilterNode, the chrome object, Notification, the 刀69 WebGL stubs...)
// plus the Object.create'd singletons. All installs are globalThis member
// writes; __diting_fragmentDirective is pinned for bootstrap's window-face
// getter, __diting_keyboard/__diting_userActivation publish via their
// navigator_persona consumers, __diting_launchQueue has zero readers and
// stays file-private. The dead #117 navigator-store comment that sat in
// the middle of the zone was dropped here (the store it describes moved to
// navigator_persona.js at 刀13).
// ---- bchkAPIs global face (batch166, datadome-solver generators.go) ------
// Interfaces real Chrome exposes that anti-bot probes check with
// `'X' in window`. Illegal constructors where Chrome forbids `new`; kept
// standalone (no DOM base classes) so definition order stays independent.

globalThis.FragmentDirective = class FragmentDirective {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
};
const __diting_fragmentDirective = Object.create(globalThis.FragmentDirective.prototype);

globalThis.UserActivation = class UserActivation {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get hasBeenActive() { return this._hasBeenActive === true; }
  get isActive() { return this._isActive === true; }
};
const __diting_userActivation = Object.create(globalThis.UserActivation.prototype);

globalThis.Keyboard = class Keyboard {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  getLayoutMap() { return Promise.resolve(new Map()); }
  lock() { return Promise.resolve(); }
  unlock() {}
};
const __diting_keyboard = Object.create(globalThis.Keyboard.prototype);

globalThis.LaunchQueue = class LaunchQueue {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  setConsumer(callback) { this._consumer = callback; }
};
const __diting_launchQueue = Object.create(globalThis.LaunchQueue.prototype);

globalThis.BatteryManager = class BatteryManager {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get charging() { return this._charging; }
  get chargingTime() { return this._chargingTime; }
  get dischargingTime() { return this._dischargingTime; }
  get level() { return this._level; }
};

globalThis.AudioSinkInfo = class AudioSinkInfo {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get type() { return "default"; }
};

globalThis.EyeDropper = class EyeDropper {
  // Chrome's color picker needs real user activation; a headless page gets
  // the NotAllowedError branch of the same promise shape.
  open(options) { return Promise.reject(new DOMException("NotAllowedError")); }
};

globalThis.GPUSupportedLimits = class GPUSupportedLimits {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
};

globalThis.GeolocationCoordinates = class GeolocationCoordinates {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get latitude() { return this._latitude ?? 0; }
  get longitude() { return this._longitude ?? 0; }
  get accuracy() { return this._accuracy ?? 0; }
  get altitude() { return this._altitude ?? null; }
  get altitudeAccuracy() { return this._altitudeAccuracy ?? null; }
  get heading() { return this._heading ?? null; }
  get speed() { return this._speed ?? null; }
};
globalThis.GeolocationPosition = class GeolocationPosition {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get coords() { return this._coords; }
  get timestamp() { return this._timestamp ?? 0; }
};
globalThis.__diting_makeGeolocationPosition = function (lat, lon, acc) {
  const coords = Object.create(globalThis.GeolocationCoordinates.prototype);
  coords._latitude = lat;
  coords._longitude = lon;
  coords._accuracy = acc;
  const pos = Object.create(globalThis.GeolocationPosition.prototype);
  pos._coords = coords;
  pos._timestamp = Date.now();
  return pos;
};

globalThis.MediaDeviceInfo = class MediaDeviceInfo {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get deviceId() { return this._deviceId || ""; }
  get kind() { return this._kind || ""; }
  get label() { return this._label || ""; }
  get groupId() { return this._groupId || ""; }
  toJSON() { return { deviceId: this.deviceId, groupId: this.groupId, kind: this.kind, label: this.label }; }
};

globalThis.MediaMetadata = class MediaMetadata {
  constructor(init = {}) {
    if (init == null) init = {};
    this._title = init.title === undefined ? "" : String(init.title);
    this._artist = init.artist === undefined ? "" : String(init.artist);
    this._album = init.album === undefined ? "" : String(init.album);
    this._artwork = Array.isArray(init.artwork) ? init.artwork : [];
  }
  get title() { return this._title; }
  set title(v) { this._title = String(v); }
  get artist() { return this._artist; }
  set artist(v) { this._artist = String(v); }
  get album() { return this._album; }
  set album(v) { this._album = String(v); }
  get artwork() { return this._artwork; }
  set artwork(v) { this._artwork = Array.isArray(v) ? v : []; }
};

globalThis.MediaSourceHandle = class MediaSourceHandle {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
};

globalThis.PasswordCredential = class PasswordCredential {
  constructor(data) {
    if (arguments.length === 0) throw new TypeError("Failed to construct 'PasswordCredential': 1 argument required, but only 0 present.");
    if (data == null || typeof data !== "object") throw new TypeError("Failed to construct 'PasswordCredential': The provided value is not of type 'PasswordCredentialData'.");
    if (data.id === undefined) throw new TypeError("Failed to construct 'PasswordCredential': 'id' cannot be undefined.");
    if (data.password === undefined) throw new TypeError("Failed to construct 'PasswordCredential': 'password' cannot be undefined.");
    this._id = String(data.id);
    this._name = data.name === undefined ? "" : String(data.name);
    this._password = String(data.password);
    // 刀57: id/type answer through Credential.prototype now (Chrome owns
    // them there; PasswordCredential.prototype lists neither).
    this._type = "password";
  }
  get name() { return this._name; }
  get password() { return this._password; }
  get iconURL() { return ""; }
};

globalThis.PushSubscriptionOptions = class PushSubscriptionOptions {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get userVisibleOnly() { return this._userVisibleOnly === true; }
  get applicationServerKey() { return this._applicationServerKey ?? null; }
};

globalThis.Scheduling = class Scheduling {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  isInputPending() { return false; }
};

globalThis.TextTrackCue = class TextTrackCue {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get track() { return this._track ?? null; }
  get id() { return this._id || ""; }
  set id(v) { this._id = String(v); }
  get startTime() { return this._startTime ?? 0; }
  set startTime(v) { this._startTime = Number(v); }
  get endTime() { return this._endTime ?? 0; }
  set endTime(v) { this._endTime = Number(v); }
  get pauseOnExit() { return this._pauseOnExit === true; }
  set pauseOnExit(v) { this._pauseOnExit = Boolean(v); }
};

globalThis.VideoPlaybackQuality = class VideoPlaybackQuality {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get creationTime() { return this._creationTime ?? 0; }
  get totalVideoFrames() { return this._totalVideoFrames ?? 0; }
  get droppedVideoFrames() { return this._droppedVideoFrames ?? 0; }
  get corruptedVideoFrames() { return this._corruptedVideoFrames ?? 0; }
};

globalThis.ViewTransition = class ViewTransition {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get updateCallbackDone() { if (!this._ucd) this._ucd = Promise.resolve(); return this._ucd; }
  get ready() { if (!this._readyP) this._readyP = Promise.resolve(); return this._readyP; }
  get finished() { if (!this._fin) this._fin = Promise.resolve(); return this._fin; }
  skipTransition() {}
};

globalThis.WebGLObject = class WebGLObject {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
};

globalThis.WebTransport = class WebTransport {
  constructor(url, options) {
    if (arguments.length === 0) throw new TypeError("Failed to construct 'WebTransport': 1 argument required, but only 0 present.");
    try { this._url = new URL(url, _docBase()).href; }
    catch (e) { throw new TypeError("Failed to construct 'WebTransport': The provided value is not a valid URL."); }
  }
  get ready() { if (!this._ready) this._ready = new Promise(() => {}); return this._ready; }
  get closed() { if (!this._closedP) this._closedP = new Promise(() => {}); return this._closedP; }
  get datagrams() {
    if (!this._datagrams) {
      this._datagrams = {
        readable: new ReadableStream(),
        writable: new WritableStream(),
      };
    }
    return this._datagrams;
  }
  close() {}
  createBidirectionalStream() { return Promise.reject(new DOMException("NotAllowedError")); }
  createUnidirectionalStream() { return Promise.reject(new DOMException("NotAllowedError")); }
};

globalThis.WebSocketStream = class WebSocketStream {
  constructor(url, protocols) {
    if (arguments.length === 0) throw new TypeError("Failed to construct 'WebSocketStream': 1 argument required, but only 0 present.");
    const wss = __diting_liftSlot(this);
    wss.url = String(url);
    wss.opened = new Promise(() => {});   // Chrome 145: `opened` (the old `opening` name was the pre-ship draft)
    wss.closed = new Promise(() => {});
  }
  close() {}
};

globalThis.CSSCounterStyleRule = class CSSCounterStyleRule {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get name() { return this._name || ""; }
};

globalThis.CSSFontPaletteValuesRule = class CSSFontPaletteValuesRule {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get name() { return this._name || ""; }
};

globalThis.CropTarget = class CropTarget {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  static fromElement(element) { return Object.create(globalThis.CropTarget.prototype); }
};

globalThis.CustomStateSet = class CustomStateSet {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get size() { return 0; }
  add() { return this; }
  has() { return false; }
  delete() { return false; }
  clear() {}
  forEach() {}
  *entries() {}
  *keys() {}
  *values() {}
  [Symbol.iterator]() { return [][Symbol.iterator](); }
};

if (typeof globalThis.AudioNode === "undefined") {
  globalThis.AudioNode = class AudioNode {
    constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
    connect(dest) { return dest; }
    disconnect() {}
  };
}
globalThis.IIRFilterNode = class IIRFilterNode {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  getFrequencyResponse() {}
};

// Key order matches real Chrome's Object.keys(chrome) = ["loadTimes","csi","app"]
// (DataDome-class fingerprints read key order, not just membership). No
// `runtime` key: fp-diff-1014 #2 caught us exposing chrome.runtime as an
// object — on a normal Chrome page it is absent entirely (it only exists in
// extension content-script contexts), so its presence reads as automation.
// Real Chrome's csi/loadTimes are anonymous, constructible natives
// (name "", own `prototype`, toString "function () { [native code] }");
// the `(0, function () {})` shape keeps name inference from naming them.
// xhs's b1 field x45 (__SEC_CAV__) checks exactly this and read 0 for both.
globalThis.chrome = {
  loadTimes: (0, function () {
    const t = Date.now() / 1000;
    const request = t - 0.5 - _fpRand(611) * 0.5;
    const startLoad = request + 0.05 + _fpRand(612) * 0.02;
    const commit = request + 0.3 + _fpRand(613) * 0.4;
    const finishDoc = commit + 0.1 + _fpRand(614) * 0.2;
    const finish = finishDoc + 0.05 + _fpRand(615) * 0.1;
    const firstPaint = commit + 0.03 + _fpRand(616) * 0.1;
    const navTypes = ["BackForward","Reload","Link","Other"];
    return {
      requestTime: request, startLoadTime: startLoad * 1000, commitLoadTime: commit * 1000,
      finishDocumentLoadTime: finishDoc * 1000, finishLoadTime: finish * 1000,
      firstPaintTime: firstPaint * 1000, firstPaintAfterLoadTime: 0,
      navigationType: navTypes[Math.floor(_fpRand(617) * 4)],
      wasFetchedViaSpdy: false, wasNpnNegotiated: false,
      npnNegotiatedProtocol: "http/1.1",
      wasAlternateProtocolAvailable: false, connectionInfo: "http/1.1",
    };
  }),
  csi: (0, function () {
    const t = Date.now();
    return { startE: t - Math.floor(100 + _fpRand(610) * 200), onloadT: t, pageT: 0, tran: 5 };
  }),
  // Key set AND order captured from a real Chrome 155 (fp probe 10-14):
  // isInstalled|getDetails|getIsInstalled|installState|runningState|InstallState|RunningState
  app: {
    isInstalled: false,
    getDetails() { return null; },
    getIsInstalled() { return false; },
    installState() { return "disabled"; },
    runningState() { return "cannot_run"; },
    InstallState: { DISABLED: "disabled", INSTALLED: "installed", NOT_INSTALLED: "not_installed" },
    RunningState: { CANNOT_RUN: "cannot_run", READY_TO_RUN: "ready_to_run", RUNNING: "running" },
  },
};

globalThis.Notification = class Notification {
  static permission = "default";
  static maxActions = 2;
  static requestPermission() { return Promise.resolve(Notification.permission); }
  constructor() {}
  get image() { return null; }
};

// (刀69) Chrome: `new WebGLRenderingContext()` is an illegal constructor — the
// contexts come out of canvas.getContext (Object.create over the prototype).
globalThis.WebGLRenderingContext = class WebGLRenderingContext { constructor() { throw new TypeError("Failed to construct 'WebGLRenderingContext': Illegal constructor"); } };
globalThis.WebGL2RenderingContext = class WebGL2RenderingContext { constructor() { throw new TypeError("Failed to construct 'WebGL2RenderingContext': Illegal constructor"); } };

// Native marking for the faces this file owns, migrated from bootstrap's
// load-time sweep at the 刀22 carve (bootstrap loads before this file
// installs them — the old sweep entries saw undefined). Member form.
_markNative(globalThis.Notification);
_markNative(globalThis.Notification.requestPermission);
_markNative(globalThis.chrome.csi);
_markNative(globalThis.chrome.loadTimes);

// BatteryManager EventTarget chain relink, migrated from bootstrap's
// DOM-cluster tail at the 刀22 carve: bootstrap loads before this file
// installs the class, so the old bare `globalThis.BatteryManager.prototype`
// line would throw on undefined. Member form both sides.
Object.setPrototypeOf(globalThis.BatteryManager.prototype, globalThis.EventTarget.prototype);

// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('__diting_fragmentDirective', __diting_fragmentDirective);
  d('__diting_keyboard', __diting_keyboard);
  d('__diting_userActivation', __diting_userActivation);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
