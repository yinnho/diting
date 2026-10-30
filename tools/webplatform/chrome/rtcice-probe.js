// RTCIceCandidate probe (M4 knife 13): corpus-driven candidate-string parsing
// plus constructor validation, toJSON and shape. Runs in Chrome AND the engine.
(function () {
  var R = {};
  function t(n, f) { try { var v = f(); R[n] = v === undefined ? '#undefined' : JSON.parse(JSON.stringify(v, function (k, x) { return x === undefined ? '#undef' : x; })); } catch (e) { R[n] = 'THROW ' + e.name + ': ' + String(e.message).slice(0, 240); } }
  var C = RTCIceCandidate, P = C.prototype;
  function kind(o, m) { var d = Object.getOwnPropertyDescriptor(o, m); return (d.get || d.set ? (d.set ? 'rw' : 'ro') : typeof d.value + ':' + (typeof d.value === 'function' ? d.value.length : '')) + ':' + d.enumerable; }
  R.shape = { order: Object.getOwnPropertyNames(P), statics: Object.getOwnPropertyNames(C), len: C.length, tag: Object.prototype.toString.call(Object.create(P)), parent: Object.getPrototypeOf(C).name, kinds: {} };
  R.shape.order.forEach(function (m) { R.shape.kinds[m] = kind(P, m); });
  var FIELDS = ['candidate', 'sdpMid', 'sdpMLineIndex', 'usernameFragment', 'foundation', 'component', 'priority', 'address', 'protocol', 'port', 'type', 'tcpType', 'relatedAddress', 'relatedPort', 'relayProtocol', 'url'];
  function dump(c) { var o = {}; FIELDS.forEach(function (f) { var v = c[f]; o[f] = v === undefined ? '#undef' : v; }); return o; }
  t('call', function () { return RTCIceCandidate({}); });
  t('brand', function () { return Object.getOwnPropertyDescriptor(RTCIceCandidate.prototype, 'candidate').get.call({}); });
  t('brand.toJSON', function () { return RTCIceCandidate.prototype.toJSON.call({}); });
  t('noargs', function () { return dump(new RTCIceCandidate()); });
  t('empty', function () { return dump(new RTCIceCandidate({})); });
  t('undef', function () { return dump(new RTCIceCandidate(undefined)); });
  t('null', function () { return dump(new RTCIceCandidate(null)); });
  t('num', function () { return dump(new RTCIceCandidate(5)); });
  t('str', function () { return dump(new RTCIceCandidate('x')); });
  t('fn', function () { return dump(new RTCIceCandidate(function () {})); });
  // candidate-string corpus (the "candidate:" prefix, a=, spacing and every type)
  var CANDS = [
    'candidate:842163049 1 udp 1677729535 203.0.113.5 46154 typ srflx raddr 10.0.0.2 rport 46154 generation 0 ufrag abcd network-cost 999',
    'candidate:1 1 UDP 2122260223 192.168.1.10 54321 typ host generation 0 ufrag Xy12 network-id 1',
    'candidate:1 2 udp 2122260222 192.168.1.10 54322 typ host',
    'candidate:2 1 tcp 1518280447 192.168.1.10 9 typ host tcptype active generation 0',
    'candidate:2 1 tcp 1518280447 192.168.1.10 9 typ host tcptype passive',
    'candidate:2 1 tcp 1518280447 192.168.1.10 9 typ host tcptype so',
    'candidate:3 1 udp 41885439 198.51.100.7 3478 typ relay raddr 203.0.113.5 rport 54000 generation 0',
    'candidate:4 1 udp 1686052607 203.0.113.9 51000 typ prflx raddr 10.0.0.1 rport 51000',
    'candidate:5 1 udp 2122194687 2001:db8::1 40000 typ host',
    'candidate:5 1 udp 2122194687 [2001:db8::1] 40000 typ host',
    'candidate:6 1 udp 2122194687 abcd.local 40000 typ host',
    'a=candidate:7 1 udp 100 1.2.3.4 5 typ host',
    'candidate: 8 1 udp 100 1.2.3.4 5 typ host',
    'candidate:8  1   udp 100 1.2.3.4 5 typ host',
    'candidate:9 1 udp 100 1.2.3.4 5 typ host extra stuff here',
    'candidate:10 1 udp 100 1.2.3.4 5 typ srflx',
    'candidate:11 1 udp 100 1.2.3.4 5 typ srflx raddr 5.6.7.8',
    'candidate:12 1 udp 100 1.2.3.4 5 typ srflx rport 9',
    'candidate:13 1 udp 100 1.2.3.4 70000 typ host',
    'candidate:14 1 udp 4294967295 1.2.3.4 5 typ host',
    'candidate:15 1 udp 4294967296 1.2.3.4 5 typ host',
    'candidate:16 3 udp 100 1.2.3.4 5 typ host',
    'candidate:17 1 sctp 100 1.2.3.4 5 typ host',
    'candidate:18 1 udp 100 1.2.3.4 5 typ unknown',
    'candidate:19 1 udp 100 1.2.3.4 abc typ host',
    'candidate:20 1 udp abc 1.2.3.4 5 typ host',
    'candidate:21 x udp 100 1.2.3.4 5 typ host',
    'candidate:22 1 udp 100 1.2.3.4 5 host',
    'candidate:23 1 udp 100 1.2.3.4',
    'candidate:24',
    'candidate:',
    'candidate',
    'foo bar',
    '',
    ' ',
    'candidate:25 1 udp 100 1.2.3.4 5 typ host\r\n',
    'candidate:26 1 udp 100 1.2.3.4 5 typ host relay-protocol tls',
    'candidate:27 1 udp 100 1.2.3.4 5 typ relay raddr 0.0.0.0 rport 0 relayprotocol tcp',
    'candidate:28 1 udp 100 1.2.3.4 5 typ host url stun:stun.example.com',
    'candidate:29 1 UDP 100 1.2.3.4 5 TYP HOST',
    'end-of-candidates',
    'candidate:30 1 udp 100 1.2.3.4 5 typ host ufrag a b',
  ];
  CANDS.forEach(function (c, i) {
    t('cand.' + i, function () { return dump(new RTCIceCandidate({ candidate: c, sdpMid: 'm' })); });
  });
  // constructor argument validation
  t('v.noids', function () { return new RTCIceCandidate({ candidate: 'candidate:1 1 udp 100 1.2.3.4 5 typ host' }); });
  t('v.mid.only', function () { return dump(new RTCIceCandidate({ sdpMid: 'a' })); });
  t('v.idx.only', function () { return dump(new RTCIceCandidate({ sdpMLineIndex: 0 })); });
  t('v.cand.empty.mid', function () { return dump(new RTCIceCandidate({ candidate: '', sdpMid: 'a' })); });
  t('v.cand.empty.noids', function () { return dump(new RTCIceCandidate({ candidate: '' })); });
  t('v.ids', function () { return dump(new RTCIceCandidate({ candidate: 'candidate:1 1 udp 100 1.2.3.4 5 typ host', sdpMid: '0', sdpMLineIndex: 3, usernameFragment: 'uf' })); });
  t('v.idx.conv', function () { var o = {}; [['str', '7'], ['neg', -1], ['big', 70000], ['float', 1.9], ['nan', NaN], ['null', null], ['undef', undefined], ['obj', {}]].forEach(function (c) { try { var x = new RTCIceCandidate({ candidate: 'candidate:1 1 udp 100 1.2.3.4 5 typ host', sdpMLineIndex: c[1] }); o[c[0]] = x.sdpMLineIndex; } catch (e) { o[c[0]] = e.name + ': ' + e.message; } }); return o; });
  t('v.mid.conv', function () { var o = {}; [['num', 5], ['null', null], ['undef', undefined], ['obj', {}], ['empty', '']].forEach(function (c) { try { var x = new RTCIceCandidate({ candidate: 'candidate:1 1 udp 100 1.2.3.4 5 typ host', sdpMid: c[1] }); o[c[0]] = x.sdpMid; } catch (e) { o[c[0]] = e.name + ': ' + e.message; } }); return o; });
  t('v.uf.conv', function () { var o = {}; [['num', 5], ['null', null], ['undef', undefined], ['empty', '']].forEach(function (c) { try { var x = new RTCIceCandidate({ candidate: 'candidate:1 1 udp 100 1.2.3.4 5 typ host ufrag zz', sdpMid: 'a', usernameFragment: c[1] }); o[c[0]] = x.usernameFragment; } catch (e) { o[c[0]] = e.name + ': ' + e.message; } }); return o; });
  t('v.cand.conv', function () { var o = {}; [['num', 5], ['null', null], ['undef', undefined], ['obj', {}], ['sym', Symbol()]].forEach(function (c) { try { var x = new RTCIceCandidate({ candidate: c[1], sdpMid: 'a' }); o[c[0]] = x.candidate; } catch (e) { o[c[0]] = e.name + ': ' + e.message; } }); return o; });
  t('v.init.unknown', function () { return dump(new RTCIceCandidate({ foo: 1, candidate: 'candidate:1 1 udp 100 1.2.3.4 5 typ host', sdpMid: 'a' })); });
  t('toJSON', function () { var c = new RTCIceCandidate({ candidate: 'candidate:1 1 udp 100 1.2.3.4 5 typ host ufrag q', sdpMid: 'a', sdpMLineIndex: 2 }); var j = c.toJSON(); return [j, Object.keys(j), Object.getPrototypeOf(j) === Object.prototype]; });
  t('toJSON.partial', function () { var j = new RTCIceCandidate({ candidate: 'candidate:1 1 udp 100 1.2.3.4 5 typ host', sdpMid: 'a' }).toJSON(); return [j, Object.keys(j)]; });
  t('toJSON.noufrag', function () { var j = new RTCIceCandidate({ candidate: 'candidate:1 1 udp 100 1.2.3.4 5 typ host', sdpMLineIndex: 0 }).toJSON(); return [j, Object.keys(j)]; });
  t('JSON.stringify', function () { return JSON.stringify(new RTCIceCandidate({ candidate: 'candidate:1 1 udp 100 1.2.3.4 5 typ host', sdpMid: '0', sdpMLineIndex: 0 })); });
  t('props.ro', function () { var c = new RTCIceCandidate({ candidate: 'candidate:1 1 udp 100 1.2.3.4 5 typ host', sdpMid: 'a' }); c.candidate = 'x'; c.port = 9; return [c.candidate, c.port]; });
  t('own', function () { var c = new RTCIceCandidate({ candidate: 'candidate:1 1 udp 100 1.2.3.4 5 typ host', sdpMid: 'a' }); return Object.getOwnPropertyNames(c); });
  t('identity', function () { var c = new RTCIceCandidate({ candidate: 'candidate:1 1 udp 100 1.2.3.4 5 typ host', sdpMid: 'a' }); return [c instanceof RTCIceCandidate, c.address === c.address, typeof c.priority, typeof c.port, typeof c.component]; });
  t('exist', function () { return ['RTCPeerConnection', 'RTCSessionDescription', 'RTCIceCandidate', 'RTCIceCandidatePair', 'RTCIceTransport'].map(function (n) { return typeof window[n]; }); });
  return JSON.stringify(R);
})()
