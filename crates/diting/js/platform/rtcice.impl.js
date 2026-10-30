//! module: rtcice
//! layer: L2
//! exposes: RTCIceCandidate
//! requires-load: -
//! requires-call: -

// RTCIceCandidate (M4 knife 13). The bootstrap class only stored `candidate`;
// Chrome parses the candidate-attribute string into foundation / component /
// priority / address / protocol / port / type / tcpType / relatedAddress /
// relatedPort, and the parse is ALL-OR-NOTHING: any malformed token leaves
// every parsed field null. Pinned by chrome/rtcice-truth-145.json
// (rtcice-probe.js, 72 keys: a 43-candidate corpus, constructor conversions
// and validation, toJSON) and diffed by rtcice_matches_chrome_truth.

const slots = new WeakMap();

const TYPES = new Set(['host', 'srflx', 'prflx', 'relay']);
const TCP_TYPES = new Set(['active', 'passive', 'so']);

// candidate-attribute: [a=]candidate:<foundation> <component> <transport>
// <priority> <address> <port> typ <type> [raddr <a>] [rport <n>] [tcptype <t>]
// followed by extension attributes that are ignored. Single spaces only,
// lowercase `typ` and type keywords, transport case-insensitive (udp/tcp).
function parseCandidate(str) {
  let s = str;
  if (s.startsWith('a=')) s = s.slice(2);
  s = s.replace(/\r?\n$/, '');
  if (!s.startsWith('candidate:')) return null;
  const t = s.slice('candidate:'.length).split(' ');
  if (t.length < 8) return null;
  const [foundation, comp, transport, prio, addr, port, typ, type] = t;
  if (foundation === '' || !/^[0-9]+$/.test(comp) || !/^[0-9]+$/.test(prio) || !/^[0-9]+$/.test(port)) return null;
  const proto = transport.toLowerCase();
  if (proto !== 'udp' && proto !== 'tcp') return null;
  const priority = Number(prio), portN = Number(port);
  if (priority > 4294967295 || portN > 65535) return null;
  if (addr === '' || typ !== 'typ' || !TYPES.has(type)) return null;
  const r = { foundation, component: comp === '1' ? 'rtp' : comp === '2' ? 'rtcp' : null, priority, protocol: proto,
    address: addr.includes(':') && !addr.startsWith('[') ? '[' + addr + ']' : addr, port: portN, type,
    tcpType: null, relatedAddress: null, relatedPort: null };
  let raddr = null, rport = null;
  for (let i = 8; i + 1 < t.length; i++) {
    if (t[i] === 'raddr') { raddr = t[i + 1]; i++; }
    else if (t[i] === 'rport') {
      if (!/^[0-9]+$/.test(t[i + 1]) || Number(t[i + 1]) > 65535) return null;
      rport = Number(t[i + 1]); i++;
    } else if (t[i] === 'tcptype') {
      if (!TCP_TYPES.has(t[i + 1])) return null;
      r.tcpType = t[i + 1]; i++;
    }
  }
  if (raddr !== null || rport !== null) {
    r.relatedAddress = raddr === null ? '' : raddr;
    r.relatedPort = rport === null ? 0 : rport;
  }
  return r;
}

const PRE = "Failed to construct 'RTCIceCandidate'";
function ushort(v) {
  const n = Number(v);
  return isFinite(n) ? ((Math.trunc(n) % 65536) + 65536) % 65536 : 0;
}
function RTCIceCandidate() {
  if (new.target === undefined) P.fail(RTCIceCandidate, "Please use the 'new' operator, this DOM object constructor cannot be called as a function.", PRE);
  let init = arguments[0];
  if (init === null) init = undefined;
  if (init !== undefined && typeof init !== 'object' && typeof init !== 'function') {
    P.fail(RTCIceCandidate, "The provided value is not of type 'RTCIceCandidateInit'.", PRE);
  }
  const o = init === undefined ? {} : init;
  const rd = (k) => PRE + ": Failed to read the '" + k + "' property from 'RTCIceCandidateInit'";
  // dictionary members in WebIDL (lexicographic) order
  const candidate = o.candidate === undefined ? '' : P.conv.DOMString(o.candidate, rd('candidate'), RTCIceCandidate);
  let sdpMLineIndex = null;
  if (o.sdpMLineIndex !== undefined && o.sdpMLineIndex !== null) {
    if (typeof o.sdpMLineIndex === 'symbol') P.fail(RTCIceCandidate, 'Cannot convert a Symbol value to a number', rd('sdpMLineIndex'));
    sdpMLineIndex = ushort(o.sdpMLineIndex);
  }
  const nstr = (k) => (o[k] === undefined || o[k] === null ? null : P.conv.DOMString(o[k], rd(k), RTCIceCandidate));
  const sdpMid = nstr('sdpMid');
  const usernameFragment = nstr('usernameFragment');
  if (sdpMid === null && sdpMLineIndex === null) P.fail(RTCIceCandidate, 'sdpMid and sdpMLineIndex are both null.', PRE);
  const self = Reflect.construct(Object, [], new.target);
  slots.set(self, { candidate, sdpMid, sdpMLineIndex, usernameFragment, parsed: parseCandidate(candidate) });
  return self;
}
const RP = P.iface(RTCIceCandidate, 'RTCIceCandidate', 0);
function slot(o, fn) { const s = o !== null && typeof o === 'object' ? slots.get(o) : undefined; if (!s) P.illegal(fn); return s; }
// Chrome's own order: candidate, sdpMid, sdpMLineIndex, then the parsed fields,
// usernameFragment after relatedPort, relayProtocol and url last
const PARSED = ['foundation', 'component', 'priority', 'address', 'protocol', 'port', 'type', 'tcpType', 'relatedAddress', 'relatedPort'];
for (const m of ['candidate', 'sdpMid', 'sdpMLineIndex']) P.getter(RP, m, function __f() { return slot(this, __f)[m]; });
for (const m of PARSED) P.getter(RP, m, function __f() { const s = slot(this, __f); return s.parsed === null ? null : s.parsed[m]; });
P.getter(RP, 'usernameFragment', function __f() { return slot(this, __f).usernameFragment; });
P.getter(RP, 'relayProtocol', function __f() { slot(this, __f); return null; });
P.getter(RP, 'url', function __f() { slot(this, __f); return null; });
P.method(RP, 'toJSON', 0, function __f() {
  const s = slot(this, __f);
  const j = { candidate: s.candidate, sdpMid: s.sdpMid === null ? '' : s.sdpMid };
  if (s.sdpMLineIndex !== null) j.sdpMLineIndex = s.sdpMLineIndex;
  j.usernameFragment = s.usernameFragment === null ? '' : s.usernameFragment;
  return j;
});
P.seal(RTCIceCandidate);

P.expose('RTCIceCandidate', RTCIceCandidate);
