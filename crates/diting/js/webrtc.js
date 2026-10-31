(function () { // @@SCOPE
// WebRTC / MediaStream / CacheStorage / setHTMLUnsafe faces — ratchet split
// out of bootstrap.js. Top level
// only declares bindings; nothing here is read before call time.
globalThis.MediaStream = class MediaStream { constructor(){this.id='';this.active=true;} getTracks(){return [];} getAudioTracks(){return [];} getVideoTracks(){return [];} addTrack(){} removeTrack(){} clone(){return new MediaStream();} };
// 刀62: MediaStreamTrack full Chrome face — 20 own prototype members in
// Chrome's raw order (kind first, constructor LAST; probed), no statics,
// getCapabilities on the PROTOTYPE (statics probed empty), `new` is an
// illegal constructor, and the prototype chain runs through EventTarget.
// Function-minted (not class syntax) so every member, constructor included,
// is configurable — the flag/reorder pass at the bottom can then make the
// raw own-key order equal Chrome's with the audit's ec/e flags. All state
// lives in hidden slots; a minted track answers Chrome's defaults.
let __rtcMintTrackImpl = null; // wired by the PC face below once mintTrack exists
(function () {
  const P = { __proto__: EventTarget.prototype };
  const slot = (o, k, v) => { if (o[k] === undefined) Object.defineProperty(o, k, { value: v, writable: true, configurable: true }); };
  const dg = (n, get, set) => Object.defineProperty(P, n, set ? { get, set, enumerable: true, configurable: true } : { get, enumerable: true, configurable: true });
  const df = (n, f) => { _markNative(f); return Object.defineProperty(P, n, { value: f, writable: true, enumerable: true, configurable: true }); };
  dg('kind', function kind() { return this.__kind || ''; });
  // 刀62: track ids must be UNIQUE per track (Chrome mints a fresh GUID per
  // instance) — __rtcToken is persona-seeded and stable, so roll real entropy.
  const __rtcUid = () => { let s = ''; for (let i = 0; i < 64; i++) s += 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/'[Math.floor(Math.random() * 64)]; return s; };
  dg('id', function id() { slot(this, '__id', __rtcUid()); return this.__id; });
  dg('label', function label() { return this.__label || ''; });
  dg('enabled', function enabled() { return this.__enabled === undefined ? true : this.__enabled; },
    function enabled(v) { Object.defineProperty(this, '__enabled', { value: !!v, writable: true, configurable: true }); });
  const onDef = (n) => dg(n, new Function('n', 'return function () { const o = this.__on; return o && o[n] !== undefined ? o[n] : null; };')(n),
    new Function('n', 'return function (v) { if (!this.__on) Object.defineProperty(this, "__on", { value: {}, writable: true, configurable: true }); this.__on[n] = typeof v === "function" ? v : null; };')(n));
  dg('muted', function muted() { return false; });
  onDef('onmute');
  onDef('onunmute');
  dg('readyState', function readyState() { return this.__rs || 'live'; });
  onDef('onended');
  dg('stats', function stats() { return null; });
  dg('contentHint', function contentHint() { return this.__ch || ''; },
    function contentHint(v) { Object.defineProperty(this, '__ch', { value: String(v), writable: true, configurable: true }); });
  df('applyConstraints', function applyConstraints(constraints) { return Promise.resolve(); });
  df('clone', function clone() {
    const c = __rtcMintTrackImpl ? __rtcMintTrackImpl(this.__kind || '') : Object.create(globalThis.MediaStreamTrack.prototype);
    Object.defineProperty(c, '__label', { value: this.__label || '', writable: true, configurable: true });
    Object.defineProperty(c, '__rs', { value: this.__rs || 'live', writable: true, configurable: true });
    if (this.__enabled !== undefined) Object.defineProperty(c, '__enabled', { value: this.__enabled, writable: true, configurable: true });
    if (this.__ch !== undefined) Object.defineProperty(c, '__ch', { value: this.__ch, writable: true, configurable: true });
    return c;
  });
  // 刀62 divergence (recorded): capabilities/settings/constraints return {}
  // — Chrome answers source-derived dictionaries (canvas track probed);
  // the engine mints carry no source, and {} is exactly Chrome's own answer
  // for getConstraints on that same probed track.
  df('getCapabilities', function getCapabilities() { return {}; });
  df('getConstraints', function getConstraints() { return {}; });
  df('getSettings', function getSettings() { return {}; });
  df('stop', function stop() { Object.defineProperty(this, '__rs', { value: 'ended', writable: true, configurable: true }); });
  onDef('oncapturehandlechange');
  df('getCaptureHandle', function getCaptureHandle() { return null; });
  function MediaStreamTrack() {
    throw new TypeError("Failed to construct 'MediaStreamTrack': Illegal constructor");
  }
  Object.defineProperty(P, 'constructor', { value: MediaStreamTrack, writable: true, enumerable: false, configurable: true });
  MediaStreamTrack.prototype = P;
  Object.defineProperty(globalThis, 'MediaStreamTrack', { value: MediaStreamTrack, writable: true, enumerable: false, configurable: true });
})();
// (#170) RTCPeerConnection is a WebIDL interface in Chrome: a bare call
// throws the DOM TypeError (not V8's class-constructor text), and
// Object.prototype.toString brands instances [object RTCPeerConnection]
// via Symbol.toStringTag. The AWSC collection on the 1688/taobao punish
// page walks both — a plain ES class answers V8's raw message and
// [object Object], a distinguishable JS-env signal. Same pattern family
// as the CSSStyleDeclaration branding (059015a).
// (300015 / fp-depth) Chrome gather fires host ICE candidates after
// setLocalDescription; an empty stub (n=0 candidates, empty SDP) shrinks
// AntiSpam profileData vs Chrome as/webprofile ~2176-2224. Seed cand+ufrag
// from persona _fpRand so the face is stable per device, not random/session.
const __RTCPCImpl = class RTCPeerConnection extends EventTarget {
  constructor(config){
    super();
    // 刀61: all live state into two hidden slots — Chrome's fresh pc answers
    // Object.keys(pc) === [] and every state member is a prototype getter.
    // (Previously these were own enumerable data properties: visible to
    // for-in/JSON.stringify AND shadowing the lift_own accessors, so the
    // on* handlers could never fire — dispatch's isHandlerSlot only honors
    // accessor slots.)
    this.__rtc = { ld: null, rd: null, cld: null, pld: null, crd: null, prd: null,
      sig: 'stable', igs: 'new', ics: 'new', cs: 'new', ctc: null, sctp: null,
      senders: [], receivers: [], transceivers: [], lstreams: [], rstreams: [],
      cfg: __rtcNormConfig(config) };
    this.__on = {};
    this.__closed = false;
    this.__ufrag = __rtcToken(4); this.__pwd = __rtcToken(24);
    const a = 1 + Math.floor(_fpRand(710) * 254);
    const b = 1 + Math.floor(_fpRand(711) * 254);
    this.__host = '192.168.' + a + '.' + b;
    this.__port = 40000 + Math.floor(_fpRand(712) * 20000);
  }
  __setGather(s){
    if (this.__rtc.igs === s) return;
    this.__rtc.igs = s;
    try { this.dispatchEvent(new Event('icegatheringstatechange')); } catch (e) {}
  }
  __emitCand(candStr){
    const cand = candStr == null ? null : new RTCIceCandidate({
      candidate: candStr, sdpMid: '0', sdpMLineIndex: 0,
    });
    const ev = new RTCPeerConnectionIceEvent('icecandidate', { candidate: cand });
    // dispatchEvent alone: onicecandidate is an EventHandler listener.
    try { this.dispatchEvent(ev); } catch (e) {}
  }
  __mkOfferSdp(){
    const fp = [];
    for (let i = 0; i < 32; i++) {
      const n = Math.floor(_fpRand(720 + i) * 256);
      fp.push((n < 16 ? '0' : '') + n.toString(16).toUpperCase());
    }
    return [
      'v=0',
      'o=- ' + String(Math.floor(1e15 + _fpRand(721) * 9e15)) + ' 2 IN IP4 127.0.0.1',
      's=-', 't=0 0',
      'a=group:BUNDLE 0', 'a=extmap-allow-mixed', 'a=msid-semantic: WMS',
      'm=application 9 UDP/DTLS/SCTP webrtc-datachannel',
      'c=IN IP4 0.0.0.0', 'a=ice-ufrag:' + this.__ufrag, 'a=ice-pwd:' + this.__pwd,
      'a=ice-options:trickle',
      'a=fingerprint:sha-256 ' + fp.join(':'),
      'a=setup:actpass', 'a=mid:0', 'a=sctp-port:5000', 'a=max-message-size:262144',
    ].join('\r\n') + '\r\n';
  }
  createOffer(){
    if (this.__closed) return Promise.reject(new DOMException('The RTCPeerConnection\'s signalingState is \'closed\'.', 'InvalidStateError'));
    return Promise.resolve(new RTCSessionDescription({ type: 'offer', sdp: this.__mkOfferSdp() }));
  }
  createAnswer(){
    if (this.__closed) return Promise.reject(new DOMException('The RTCPeerConnection\'s signalingState is \'closed\'.', 'InvalidStateError'));
    return Promise.resolve(new RTCSessionDescription({ type: 'answer', sdp: this.__mkOfferSdp() }));
  }
  setLocalDescription(desc){
    const self = this;
    if (self.__closed) return Promise.reject(new DOMException('The RTCPeerConnection\'s signalingState is \'closed\'.', 'InvalidStateError'));
    const d = desc || { type: 'offer', sdp: self.__mkOfferSdp() };
    self.__rtc.ld = (d instanceof RTCSessionDescription) ? d : new RTCSessionDescription(d);
    self.__rtc.cld = self.__rtc.ld;
    self.__rtc.sig = self.__rtc.ld.type === 'answer' ? 'stable' : 'have-local-offer';
    self.__setGather('gathering');
    return Promise.resolve().then(function () {
      if (self.__closed) return;
      // Host UDP + TCP candidates (Chrome desktop typically exposes both).
      const foundation = String(Math.floor(1e8 + _fpRand(730) * 9e8));
      const pri = 2122260223;
      self.__emitCand('candidate:' + foundation + ' 1 udp ' + pri + ' ' + self.__host + ' ' + self.__port + ' typ host generation 0 ufrag ' + self.__ufrag + ' network-id 1 network-cost 10');
      self.__emitCand('candidate:' + foundation + ' 1 tcp ' + (pri - 1) + ' ' + self.__host + ' ' + (9) + ' typ host tcptype active generation 0 ufrag ' + self.__ufrag + ' network-id 1 network-cost 10');
      self.__emitCand(null);
      self.__setGather('complete');
    });
  }
  setRemoteDescription(desc){
    if (this.__closed) return Promise.reject(new DOMException('The RTCPeerConnection\'s signalingState is \'closed\'.', 'InvalidStateError'));
    this.__rtc.rd = (desc instanceof RTCSessionDescription) ? desc : new RTCSessionDescription(desc || {});
    this.__rtc.crd = this.__rtc.rd;
    return Promise.resolve();
  }
  addIceCandidate(){ return Promise.resolve(); }
  close(){
    this.__closed = true;
    // 刀61 probed: close flips sig/ics/cs to 'closed' but LEAVES
    // iceGatheringState at 'new'.
    this.__rtc.ics = 'closed';
    this.__rtc.cs = 'closed';
    this.__rtc.sig = 'closed';
  }
  createDataChannel(label, opts){
    // 刀62: a branded instance whose whole face is the RTCDataChannel
    // prototype (installed load-time in web_api_gaps) — options parse into
    // the hidden __dc slot: Chrome's fresh channel answers ordered:true,
    // reliable true unless maxRetransmits/maxPacketLifeTime is set, id null
    // unless negotiated carries one, readyState 'connecting' (close() on a
    // connecting channel lands 'closing', probed).
    const o = opts && typeof opts === 'object' ? opts : {};
    const d = { label: label == null ? '' : String(label), ordered: o.ordered !== false,
      maxPacketLifeTime: o.maxPacketLifeTime === undefined ? null : Number(o.maxPacketLifeTime),
      maxRetransmits: o.maxRetransmits === undefined ? null : Number(o.maxRetransmits),
      protocol: o.protocol === undefined ? '' : String(o.protocol),
      negotiated: !!o.negotiated, id: o.id === undefined ? null : Number(o.id),
      rs: 'connecting', bt: 'arraybuffer', ba: 0, batlt: 0 };
    const ch = Object.create(globalThis.RTCDataChannel.prototype);
    Object.defineProperty(ch, '__dc', { value: d, writable: true, configurable: true });
    return ch;
  }
  getStats(){ return Promise.resolve(new Map()); }
};
function __rtcToken(n){
  const alpha = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789+/';
  let s = '';
  for (let i = 0; i < n; i++) s += alpha[Math.floor(_fpRand(740 + i) * alpha.length)];
  return s;
}
// 刀61: Chrome's getConfiguration dictionary — exactly six keys with these
// defaults (probed; note encodedInsertableStreams:false and NO certificates
// key on a fresh pc). Every iceServers entry normalizes with username and
// credential ALWAYS present as strings (7→"7" probed); a scheme-correct but
// hostless url ('stun:') is as dead as a non-url — both throw the same
// SyntaxError text (probed).
function __rtcNormConfig(cfg){
  const d = { bundlePolicy: 'balanced', encodedInsertableStreams: false, iceCandidatePoolSize: 0,
    iceServers: [], iceTransportPolicy: 'all', rtcpMuxPolicy: 'require' };
  if (cfg && typeof cfg === 'object') {
    if (['balanced', 'max-compat', 'max-bundle'].includes(cfg.bundlePolicy)) d.bundlePolicy = cfg.bundlePolicy;
    if (typeof cfg.encodedInsertableStreams === 'boolean') d.encodedInsertableStreams = cfg.encodedInsertableStreams;
    if (Number.isInteger(cfg.iceCandidatePoolSize) && cfg.iceCandidatePoolSize >= 0) d.iceCandidatePoolSize = cfg.iceCandidatePoolSize;
    if (cfg.iceTransportPolicy === 'relay') d.iceTransportPolicy = cfg.iceTransportPolicy;
    if (cfg.rtcpMuxPolicy === 'negotiate') d.rtcpMuxPolicy = cfg.rtcpMuxPolicy;
    if (Array.isArray(cfg.iceServers)) d.iceServers = cfg.iceServers.map((s) => {
      const urls = Array.isArray(s && s.urls) ? s.urls.slice() : (s && s.urls != null ? [s.urls] : []).flatMap((u) => Array.isArray(u) ? u : [u]);
      for (const u of urls) {
        let ok = false;
        try {
          const str = String(u);
          const url = new URL(str, 'http://example.invalid');
          ok = ['stun:', 'stuns:', 'turn:', 'turns:'].includes(url.protocol) && str.split(/[?#]/)[0].length > url.protocol.length;
        } catch (e) {}
        if (!ok) throw new DOMException("Failed to execute 'setConfiguration' on 'RTCPeerConnection': The given configuration has a syntax error.", 'SyntaxError');
      }
      return { credential: s && s.credential !== undefined ? String(s.credential) : '',
        urls,
        username: s && s.username !== undefined ? String(s.username) : '' };
    });
  }
  return d;
}
// 刀61: generateCertificate resolves to a branded RTCCertificate (its own
// audit line owns the rest of that face).
function __rtcMintCert(){
  const c = Object.create(globalThis.RTCCertificate.prototype);
  Object.defineProperty(c, '__expires', { value: Math.floor(Date.now() / 1000) + 2592000, writable: true, configurable: true });
  return c;
}
globalThis.RTCPeerConnection = function RTCPeerConnection() {
  if (!new.target) {
    throw new TypeError("Failed to construct 'RTCPeerConnection': Please use the 'new' operator, this DOM object constructor cannot be called as a function.");
  }
  return Reflect.construct(__RTCPCImpl, arguments, new.target);
};
globalThis.RTCPeerConnection.prototype = __RTCPCImpl.prototype;
// ---- 刀61: RTCPeerConnection's Chrome-true face --------------------------------
// Chrome 145 lists 46 proto members (6 description/state getters, 11 on*
// handler accessors, canTrickleIceCandidates, sctp, 28 methods, constructor
// LAST) plus one static (generateCertificate). The mint carried 19; the six
// state getters were lift_own rw stubs reading nothing, and the four on* the
// ctor wrote as own data props never fired (dispatch's isHandlerSlot honors
// accessor slots only — an own data prop shadows the chain). Everything here
// reads the two hidden slots the impl ctor mints (__rtc/__on). All probed
// against CfT 145 headless (fresh-pc defaults, getConfiguration's 6-key
// dictionary, addTrack's sender/transceiver/legacy-stream effects, the DTMF
// and removeTrack error family, close leaving iceGatheringState at 'new').
(function () {
  const PC = globalThis.RTCPeerConnection;
  const P = PC.prototype;
  const mark = (f) => { _markNative(f); return f; };
  const dg = (obj, name, get) => Object.defineProperty(obj, name, { get: mark(get), enumerable: true, configurable: true });
  // state getters — ro:ec per truth, named after themselves like Chrome's
  dg(P, 'localDescription', function localDescription() { return this.__rtc.ld; });
  dg(P, 'currentLocalDescription', function currentLocalDescription() { return this.__rtc.cld; });
  dg(P, 'pendingLocalDescription', function pendingLocalDescription() { return this.__rtc.pld; });
  dg(P, 'remoteDescription', function remoteDescription() { return this.__rtc.rd; });
  dg(P, 'currentRemoteDescription', function currentRemoteDescription() { return this.__rtc.crd; });
  dg(P, 'pendingRemoteDescription', function pendingRemoteDescription() { return this.__rtc.prd; });
  dg(P, 'signalingState', function signalingState() { return this.__rtc.sig; });
  dg(P, 'iceGatheringState', function iceGatheringState() { return this.__rtc.igs; });
  dg(P, 'iceConnectionState', function iceConnectionState() { return this.__rtc.ics; });
  dg(P, 'connectionState', function connectionState() { return this.__rtc.cs; });
  dg(P, 'canTrickleIceCandidates', function canTrickleIceCandidates() { return this.__rtc.ctc; });
  dg(P, 'sctp', function sctp() { return this.__rtc.sctp; });
  // on* handler accessors — rw:ec; a function assignment now FIRES on dispatch
  for (const name of ['onnegotiationneeded', 'onicecandidate', 'onsignalingstatechange',
    'oniceconnectionstatechange', 'onconnectionstatechange', 'onicegatheringstatechange',
    'onicecandidateerror', 'ontrack', 'ondatachannel', 'onaddstream', 'onremovestream']) {
    const get = mark({ ['get ' + name]: function () { const s = this.__on; return s && typeof s[name] === 'function' ? s[name] : null; } }['get ' + name]);
    const set = mark({ ['set ' + name]: function (v) { (this.__on || (this.__on = {}))[name] = typeof v === 'function' ? v : null; } }['set ' + name]);
    Object.defineProperty(P, name, { get, set, enumerable: true, configurable: true });
  }
  // mints: branded, zero-own-prop instances off the shell prototypes. Those
  // shells (RTCRtpSender/Receiver, RTCDTMFSender, RTCCertificate) are minted
  // by global_surface LATER in the load order, so the per-instance reads are
  // installed lazily on first mint, fill-if-missing.
  let satsFilled = false;
  const fillSats = () => {
    if (satsFilled) return;
    satsFilled = true;
    const RSP = globalThis.RTCRtpSender && globalThis.RTCRtpSender.prototype;
    const RRP = globalThis.RTCRtpReceiver && globalThis.RTCRtpReceiver.prototype;
    const DSP = globalThis.RTCDTMFSender && globalThis.RTCDTMFSender.prototype;
    const CERT = globalThis.RTCCertificate && globalThis.RTCCertificate.prototype;
    // force: these were stub_members' undefined-returning stubs (the audit
    // sees the right kind, the reads were dead) or the satellite's constant
    // getter — the mints' per-instance reads must be REAL.
    const force = (o, name, get) => { try { delete o[name]; } catch (e) {} dg(o, name, get); };
    if (RSP) {
      force(RSP, 'track', function track() { return this.__t === undefined ? undefined : (this.__t || null); });
      force(RSP, 'transport', function transport() { return null; });
    }
    if (RRP) force(RRP, 'track', function track() { return this.__t === undefined ? undefined : (this.__t || null); });
    if (DSP) {
      force(DSP, 'canInsertLetter', function canInsertLetter() { return null; });
      force(DSP, 'duration', function duration() { return null; });
      force(DSP, 'interToneGap', function interToneGap() { return null; });
      force(DSP, 'toneBuffer', function toneBuffer() { return ''; });
    }
    if (CERT) force(CERT, 'expires', function expires() { return this.__expires == null ? 0 : this.__expires; });
    // 刀62: MediaStreamTrack/RTCDataChannel/RTCRtpTransceiver own their whole
    // prototype faces now — MediaStreamTrack load-time above, the two shells
    // load-time in web_api_gaps (which runs after global_surface). Only the
    // sender/receiver/DTMF/certificate reads still need this lazy force.
  };
  const mintTrack = (kind) => {
    fillSats();
    const t = Object.create(globalThis.MediaStreamTrack.prototype);
    Object.defineProperty(t, '__kind', { value: kind, writable: true, configurable: true });
    return t;
  };
  const mintSender = (track) => {
    fillSats();
    const s = Object.create(globalThis.RTCRtpSender.prototype);
    Object.defineProperty(s, '__t', { value: track || null, writable: true, configurable: true });
    return s;
  };
  const mintReceiver = (track) => {
    fillSats();
    const r = Object.create(globalThis.RTCRtpReceiver.prototype);
    Object.defineProperty(r, '__t', { value: track || null, writable: true, configurable: true });
    return r;
  };
  const mintTransceiver = (sender, receiver, kind) => {
    fillSats();
    const t = Object.create(globalThis.RTCRtpTransceiver.prototype);
    Object.defineProperties(t, {
      __sender: { value: sender, writable: true, configurable: true },
      __receiver: { value: receiver, writable: true, configurable: true },
      __stopped: { value: false, writable: true, configurable: true },
      __xkind: { value: kind === 'audio' ? 'audio' : 'video', writable: true, configurable: true },
    });
    return t;
  };
  const dm = (obj, name, fn, len) => {
    if (len !== undefined) Object.defineProperty(fn, 'length', { value: len, configurable: true });
    Object.defineProperty(obj, name, { value: mark(fn), writable: true, enumerable: true, configurable: true });
  };
  const argErr = (m, n) => new TypeError("Failed to execute '" + m + "' on 'RTCPeerConnection': " + n + " argument required, but only 0 present.");
  const typeErr = (m, t) => new TypeError("Failed to execute '" + m + "' on 'RTCPeerConnection': parameter 1 is not of type '" + t + "'.");
  const closedErr = (m) => new DOMException("Failed to execute '" + m + "' on 'RTCPeerConnection': The RTCPeerConnection's signalingState is 'closed'.", 'InvalidStateError');
  dm(P, 'getConfiguration', function getConfiguration() {
    const c = this.__rtc.cfg;
    return { bundlePolicy: c.bundlePolicy, encodedInsertableStreams: c.encodedInsertableStreams,
      iceCandidatePoolSize: c.iceCandidatePoolSize, iceServers: c.iceServers.map((s) => ({
        credential: s.credential, urls: Array.isArray(s.urls) ? s.urls.slice() : s.urls, username: s.username })),
      iceTransportPolicy: c.iceTransportPolicy, rtcpMuxPolicy: c.rtcpMuxPolicy };
  }, 0);
  // 刀61 Chrome truth: bundlePolicy / rtcpMuxPolicy / encodedInsertableStreams
  // are fixed at construction — setConfiguration changing any of them throws
  // InvalidModificationError; iceServers / iceCandidatePoolSize /
  // iceTransportPolicy stay mutable; a closed pc rejects the call outright.
  dm(P, 'setConfiguration', function setConfiguration(cfg) {
    if (this.__closed) throw closedErr('setConfiguration');
    const d = __rtcNormConfig(cfg), o = this.__rtc.cfg;
    if (d.bundlePolicy !== o.bundlePolicy || d.rtcpMuxPolicy !== o.rtcpMuxPolicy || d.encodedInsertableStreams !== o.encodedInsertableStreams)
      throw new DOMException("Failed to execute 'setConfiguration' on 'RTCPeerConnection': Attempted to modify the PeerConnection's configuration in an unsupported way.", 'InvalidModificationError');
    this.__rtc.cfg = d;
  }, 0);
  dm(P, 'getSenders', function getSenders() { return this.__rtc.senders.slice(); }, 0);
  dm(P, 'getReceivers', function getReceivers() { return this.__rtc.receivers.slice(); }, 0);
  dm(P, 'getTransceivers', function getTransceivers() { return this.__rtc.transceivers.slice(); }, 0);
  dm(P, 'getLocalStreams', function getLocalStreams() { return this.__rtc.lstreams.slice(); }, 0);
  dm(P, 'getRemoteStreams', function getRemoteStreams() { return this.__rtc.rstreams.slice(); }, 0);
  dm(P, 'addStream', function addStream(stream) {
    if (stream && !this.__rtc.lstreams.includes(stream)) this.__rtc.lstreams.push(stream);
  }, 1);
  dm(P, 'removeStream', function removeStream(stream) {
    const i = this.__rtc.lstreams.indexOf(stream);
    if (i >= 0) this.__rtc.lstreams.splice(i, 1);
  }, 1);
  dm(P, 'addTrack', function addTrack(track, ...streams) {
    if (arguments.length < 1) throw argErr('addTrack', 1);
    if (!(track instanceof globalThis.MediaStreamTrack)) throw typeErr('addTrack', 'MediaStreamTrack');
    if (this.__closed) throw closedErr('addTrack');
    const kind = track && track.__kind === 'audio' ? 'audio' : 'video';
    const sender = mintSender(track), recv = mintReceiver(mintTrack(kind)), tr = mintTransceiver(sender, recv, kind);
    this.__rtc.senders.push(sender); this.__rtc.receivers.push(recv); this.__rtc.transceivers.push(tr);
    for (const s of streams) if (s && !this.__rtc.lstreams.includes(s)) this.__rtc.lstreams.push(s);
    return sender;
  }, 1);
  dm(P, 'addTransceiver', function addTransceiver(kindOrTrack, init) {
    const isTrack = kindOrTrack instanceof globalThis.MediaStreamTrack;
    if (!isTrack && kindOrTrack !== 'audio' && kindOrTrack !== 'video')
      throw new TypeError("Failed to execute 'addTransceiver' on 'RTCPeerConnection': The argument provided as parameter 1 is not a valid MediaStreamTrack kind ('audio' or 'video').");
    if (this.__closed) throw closedErr('addTransceiver');
    const kind = isTrack ? (kindOrTrack.__kind === 'audio' ? 'audio' : 'video') : kindOrTrack;
    const recv = mintReceiver(mintTrack(kind === 'audio' ? 'audio' : 'video'));
    const sender = mintSender(isTrack ? kindOrTrack : null);
    const tr = mintTransceiver(sender, recv, kind);
    this.__rtc.senders.push(sender); this.__rtc.receivers.push(recv); this.__rtc.transceivers.push(tr);
    return tr;
  }, 1);
  dm(P, 'removeTrack', function removeTrack(sender) {
    if (arguments.length < 1) throw argErr('removeTrack', 1);
    if (!(sender instanceof globalThis.RTCRtpSender)) throw typeErr('removeTrack', 'RTCRtpSender');
    if (this.__closed) throw closedErr('removeTrack');
    const i = this.__rtc.senders.indexOf(sender);
    if (i < 0) throw new DOMException("Failed to execute 'removeTrack' on 'RTCPeerConnection': The RTCRtpSender is not owned by this RTCPeerConnection.", 'InvalidAccessError');
    this.__rtc.senders[i].__t = null;
  }, 1);
  dm(P, 'restartIce', function restartIce() {}, 0);
  dm(P, 'createDTMFSender', function createDTMFSender(track) {
    if (arguments.length < 1) throw argErr('createDTMFSender', 1);
    if (!(track instanceof globalThis.MediaStreamTrack)) throw typeErr('createDTMFSender', 'MediaStreamTrack');
    if (this.__closed) throw closedErr('createDTMFSender');
    if (!track || track.kind !== 'audio') throw new DOMException("Failed to execute 'createDTMFSender' on 'RTCPeerConnection': track.kind is not 'audio'.", 'SyntaxError');
    if (!this.__rtc.senders.some((s) => s.__t === track)) throw new DOMException("Failed to execute 'createDTMFSender' on 'RTCPeerConnection': No RTCRtpSender is available for the track provided.", 'SyntaxError');
    fillSats();
    const d = Object.create(globalThis.RTCDTMFSender.prototype);
    Object.defineProperty(d, '__track', { value: track, writable: true, configurable: true });
    return d;
  }, 1);
  // static: generateCertificate — returns a promise; the no-arg call does not
  // throw synchronously in Chrome (probed).
  Object.defineProperty(PC, 'generateCertificate', { value: mark(function generateCertificate(keygen) {
    return Promise.resolve(__rtcMintCert());
  }), writable: true, enumerable: true, configurable: true });
  // Chrome's prototype order: accessors (truth sequence above), then methods
  // alphabetically, constructor LAST. Reorder in-file so the raw capture
  // order already equals truth — gen_shape_fix then emits no ORDER entry and
  // nothing reorders against this prototype later.
  const ORDER = ['localDescription', 'currentLocalDescription', 'pendingLocalDescription', 'remoteDescription',
    'currentRemoteDescription', 'pendingRemoteDescription', 'signalingState', 'iceGatheringState',
    'iceConnectionState', 'connectionState', 'canTrickleIceCandidates', 'onnegotiationneeded',
    'onicecandidate', 'onsignalingstatechange', 'oniceconnectionstatechange', 'onconnectionstatechange',
    'onicegatheringstatechange', 'onicecandidateerror', 'ontrack', 'sctp', 'ondatachannel', 'onaddstream',
    'onremovestream', 'addIceCandidate', 'addStream', 'addTrack', 'addTransceiver', 'close', 'createAnswer',
    'createDTMFSender', 'createDataChannel', 'createOffer', 'getConfiguration', 'getLocalStreams',
    'getReceivers', 'getRemoteStreams', 'getSenders', 'getStats', 'getTransceivers', 'removeStream',
    'removeTrack', 'restartIce', 'setConfiguration', 'setLocalDescription', 'setRemoteDescription', 'constructor'];
  Object.defineProperty(P, 'constructor', { value: PC, writable: true, enumerable: false, configurable: true });
  __rtcMintTrackImpl = mintTrack;
  for (const n of ORDER) {
    const d = Object.getOwnPropertyDescriptor(P, n);
    if (d && d.configurable) { delete P[n]; Object.defineProperty(P, n, d); }
  }
})();
globalThis.RTCSessionDescription = class RTCSessionDescription { constructor(d){this.type=d?.type;this.sdp=d?.sdp;} };
globalThis.RTCIceCandidate = class RTCIceCandidate { constructor(d){this.candidate=d?.candidate||'';} };
// RTC satellite interfaces — DataDome-class audits walk the whole RTCPeerConnection
// orbit (`'RTCDataChannel' in window`), and a peer with RTCPeerConnection but no
// RTCDataChannel/RTCTrackEvent is a JS-env tell. All are presence-level stubs:
// abstract ones throw Illegal constructor like Chrome, the two events derive from
// Event, RTCRtpTransceiver carries the probed stop() method.
globalThis.RTCDataChannel = class RTCDataChannel {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get label() { return ""; }
  get readyState() { return "connecting"; }
  close() {}
  send() { throw new TypeError("Failed to execute 'send' on 'RTCDataChannel'"); }
};
globalThis.RTCDtlsTransport = class RTCDtlsTransport {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get state() { return "new"; }
};
globalThis.RTCSctpTransport = class RTCSctpTransport {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get state() { return "connecting"; }
  get maxMessageSize() { return 65536; }
};
globalThis.RTCStatsReport = class RTCStatsReport {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  forEach() {}
  get() { return undefined; }
  has() { return false; }
  get size() { return 0; }
};
globalThis.RTCEncodedAudioFrame = class RTCEncodedAudioFrame {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get timestamp() { return 0; }
  // Chrome 145: `data` is a read/write ArrayBuffer attribute (the engine had a method by that name)
  get data() { return this._data || (this._data = new ArrayBuffer(0)); }
  set data(v) { this._data = v; }
};
globalThis.RTCError = class RTCError extends globalThis.DOMException {
  constructor(init, message) {
    super(message !== undefined ? String(message) : "RTCError", "OperationError");
    this.errorDetail = init?.errorDetail ?? null;
    this.sdpLineNumber = init?.sdpLineNumber ?? null;
    this.httpRequestStatusCode = init?.httpRequestStatusCode ?? null;
    this.sctpCauseCode = init?.sctpCauseCode ?? null;
    this.receivedAlert = init?.receivedAlert ?? null;
    this.sentAlert = init?.sentAlert ?? null;
  }
};
globalThis.RTCPeerConnectionIceEvent = class RTCPeerConnectionIceEvent extends globalThis.Event {
  constructor(type, init) {
    super(type || 'icecandidate', init);
    this.candidate = init && 'candidate' in init ? init.candidate : null;
  }
};
globalThis.RTCPeerConnectionIceErrorEvent = class RTCPeerConnectionIceErrorEvent extends globalThis.Event {
  constructor(type, init) { super(type || "icecandidateerror", init); this.errorCode = init?.errorCode ?? 0; this.errorText = init?.errorText ?? ""; }
  get address() { return null; }
  get port() { return null; }
  get url() { return ""; }
};
globalThis.RTCTrackEvent = class RTCTrackEvent extends globalThis.Event {
  constructor(type, init) { super(type || "track", init); this.receiver = init?.receiver ?? null; this.track = init?.track ?? null; this.streams = init?.streams ?? []; this.transceiver = init?.transceiver ?? null; }
};
globalThis.RTCRtpTransceiver = class RTCRtpTransceiver {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get mid() { return null; }
  get direction() { return "sendrecv"; }
  stop() {}
};
globalThis.webkitRTCPeerConnection = globalThis.RTCPeerConnection;
// (#170) The same brand across the whole RTC orbit — Chrome answers
// [object RTCDataChannel] / [object RTCSessionDescription] / … for every
// interface here; a class-without-tag reads as [object Object].
for (const __n of ['RTCPeerConnection','RTCSessionDescription','RTCIceCandidate','RTCDataChannel','RTCDtlsTransport','RTCSctpTransport','RTCStatsReport','RTCEncodedAudioFrame','RTCError','RTCPeerConnectionIceEvent','RTCPeerConnectionIceErrorEvent','RTCTrackEvent','RTCRtpTransceiver']) {
  const __k = globalThis[__n];
  if (__k && __k.prototype && !__k.prototype[Symbol.toStringTag]) {
    Object.defineProperty(__k.prototype, Symbol.toStringTag, { value: __n, enumerable: false, writable: false, configurable: true });
  }
}

_markNative(MediaStream); _markNative(MediaStreamTrack);
_markNative(RTCPeerConnection); _markNative(RTCSessionDescription); _markNative(RTCIceCandidate);

// setHTMLUnsafe / getHTML: shims over innerHTML. setHTMLUnsafe parses markup
// like innerHTML (the parse fixup expands declarative shadow roots the same
// way document parsing does); getHTML serializes like innerHTML.
Element.prototype.setHTMLUnsafe = function setHTMLUnsafe(html) { this.innerHTML = String(html == null ? "" : html); };
Element.prototype.getHTML = function getHTML() { return this.innerHTML; };
_markNative(Element.prototype.setHTMLUnsafe);
_markNative(Element.prototype.getHTML);
// Document.parseHTMLUnsafe(html): static that parses into a new HTML document.
if (typeof Document !== 'undefined' && typeof Document.parseHTMLUnsafe !== 'function') {
  Document.parseHTMLUnsafe = function parseHTMLUnsafe(html) {
    return new DOMParser().parseFromString(String(html == null ? "" : html), "text/html");
  };
  _markNative(Document.parseHTMLUnsafe);
}

// (M4 刀70) SpeechSynthesisUtterance: an EventTarget whose instances carry NO own
// state (WeakMap slots), attributes are prototype accessors with pinned names;
// text/lang ToString, volume/rate/pitch ToNumber -> finite check -> clamp
// ([0,1]/[0.1,10]/[0,2]) -> float32; voice only takes a SpeechSynthesisVoice or
// null; on* handlers keep functions only. Defaults are Chrome's (lang '').
const __ssuState = new WeakMap();
function SpeechSynthesisUtterance(text) {
  if (!new.target) throw new TypeError("Failed to construct 'SpeechSynthesisUtterance': Please use the 'new' operator, this DOM object constructor cannot be called as a function.");
  const self = Reflect.construct(EventTarget, [], new.target);
  let t = '';
  if (arguments.length > 0 && text !== undefined) {
    try { t = `${text}`; } catch (e) { throw new TypeError("Failed to construct 'SpeechSynthesisUtterance': " + e.message); }
  }
  __ssuState.set(self, { text: t, lang: '', voice: null, volume: 1, rate: 1, pitch: 1, h: Object.create(null) });
  return self;
}
{
  const P = Object.create(EventTarget.prototype);
  Object.defineProperty(P, 'constructor', { value: SpeechSynthesisUtterance, writable: true, configurable: true });
  SpeechSynthesisUtterance.prototype = P;
  Object.defineProperty(SpeechSynthesisUtterance, 'length', { value: 0 });
  const st = (self) => { const s = (self !== null && typeof self === 'object') ? __ssuState.get(self) : undefined; if (!s) throw new TypeError('Illegal invocation'); return s; };
  const pin = (f, n) => { Object.defineProperty(f, 'name', { value: n }); return f; };
  const acc = (name, get, set) => Object.defineProperty(P, name, { get: pin(get, 'get ' + name), set: pin(set, 'set ' + name), enumerable: true, configurable: true });
  const setErr = (name, msg) => new TypeError("Failed to set the '" + name + "' property on 'SpeechSynthesisUtterance': " + msg);
  for (const k of ['text', 'lang']) {
    acc(k, function () { return st(this)[k]; }, function (v) {
      const s = st(this);
      try { s[k] = `${v}`; } catch (e) { throw setErr(k, e.message); }
    });
  }
  for (const [k, lo, hi] of [['volume', 0, 1], ['rate', 0.1, 10], ['pitch', 0, 2]]) {
    acc(k, function () { return st(this)[k]; }, function (v) {
      const s = st(this);
      let n;
      try { n = Number(v); } catch (e) { throw setErr(k, e.message); }
      if (!Number.isFinite(n)) throw setErr(k, 'The provided float value is non-finite.');
      s[k] = Math.fround(Math.min(hi, Math.max(lo, n)));
    });
  }
  acc('voice', function () { return st(this).voice; }, function (v) {
    const s = st(this);
    if (v === null || v === undefined) { s.voice = null; return; }
    const isVoice = globalThis.__diting_isSpeechVoice;
    if (typeof isVoice !== 'function' || !isVoice(v)) throw setErr('voice', "Failed to convert value to 'SpeechSynthesisVoice'.");
    s.voice = v;
  });
  for (const type of ['start', 'end', 'error', 'pause', 'resume', 'mark', 'boundary']) {
    acc('on' + type, function () { return st(this).h[type] || null; }, function (v) { st(this).h[type] = (typeof v === 'function' || (v !== null && typeof v === 'object')) ? v : null; });
  }
  Object.defineProperty(P, Symbol.toStringTag, { value: 'SpeechSynthesisUtterance', configurable: true });
  Object.defineProperty(globalThis, '__diting_isUtterance', { value: (o) => o !== null && typeof o === 'object' && __ssuState.has(o), enumerable: false, configurable: true, writable: true });
}
globalThis.SpeechSynthesisUtterance = SpeechSynthesisUtterance;


globalThis.caches = {
  open() { return Promise.resolve({ match(){return Promise.resolve(undefined);}, put(){return Promise.resolve();}, delete(){return Promise.resolve(false);}, keys(){return Promise.resolve([]);} }); },
  match() { return Promise.resolve(undefined); },
  has() { return Promise.resolve(false); },
  delete() { return Promise.resolve(false); },
  keys() { return Promise.resolve([]); },
};
// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('SpeechSynthesisUtterance', SpeechSynthesisUtterance);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
