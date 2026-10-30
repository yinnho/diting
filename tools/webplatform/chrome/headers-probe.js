// Headers probe (M4 knife 15): corpus-driven Fetch-standard Headers.
// Runs in Chrome AND the engine; diffed key by key.
(function () {
  var R = {};
  function t(n, f) { try { var v = f(); R[n] = v === undefined ? '#undefined' : JSON.parse(JSON.stringify(v, function (k, x) { return x === undefined ? '#undef' : x; })); } catch (e) { R[n] = 'THROW ' + e.name + ': ' + String(e.message).slice(0, 220); } }
  function kind(o, m) { var d = Object.getOwnPropertyDescriptor(o, m); return (d.get || d.set ? (d.set ? 'rw' : 'ro') : typeof d.value + ':' + (typeof d.value === 'function' ? d.value.length : '')) + ':' + d.enumerable; }
  var C = Headers, P = C.prototype;
  R.shape = { order: Object.getOwnPropertyNames(P), len: C.length, tag: Object.prototype.toString.call(Object.create(P)), kinds: {}, iterIsEntries: P[Symbol.iterator] === P.entries, symbols: Object.getOwnPropertySymbols(P).map(String), statics: Object.getOwnPropertyNames(C) };
  R.shape.order.forEach(function (m) { R.shape.kinds[m] = kind(P, m); });
  function dump(h) { var o = []; h.forEach(function (v, k) { o.push([k, v]); }); return o; }
  function ents(h) { return Array.from(h.entries()); }
  t('call', function () { return Headers(); });
  t('brand.get', function () { return Headers.prototype.get.call({}, 'a'); });
  t('brand.entries', function () { return Headers.prototype.entries.call({}); });
  // ---- construction
  var CTOR = [['empty', ''], ['undef', 'undefined'], ['null', 'null'], ['obj', "{ A: '1', b: '2' }"], ['pairs', "[['a', '1'], ['B', '2']]"], ['dupe.pairs', "[['a', '1'], ['A', '2']]"],
    ['headers', "new Headers({ x: '1', y: '2' })"], ['num', '5'], ['str', "'abc'"], ['bad.pair1', "[['a']]"], ['bad.pair3', "[['a', 'b', 'c']]"], ['bad.pairstr', "['ab']"],
    ['map', "new Map([['m', '1']])"], ['iterable', "(function* () { yield ['g', '1']; })()"], ['symkey', "{ [Symbol('s')]: 'v', k: 'v' }"], ['proto', "Object.create({ inherited: '1' })"],
    ['numval', "{ a: 5, b: null, c: undefined, d: true }"], ['arrval', "{ a: [1, 2] }"], ['fn', 'function () {}'], ['bool', 'true'], ['arr.empty', '[]'],
    ['badname', "{ 'a b': '1' }"], ['badname.empty', "{ '': '1' }"], ['badname.colon', "{ 'a:b': '1' }"], ['badvalue.nl', "{ a: 'x\\ny' }"], ['badvalue.nul', "{ a: 'x\\u0000y' }"],
    ['value.trim', "{ a: '  x  ' }"], ['value.tab', "{ a: '\\tx\\t' }"], ['value.inner', "{ a: 'x  y' }"], ['value.empty', "{ a: '' }"], ['value.latin1', "{ a: '\\u00e9' }"], ['value.wide', "{ a: '\\u4e2d' }"], ['name.wide', "{ '\\u4e2d': 'v' }"],
    ['cookie', "[['Set-Cookie', 'a=1'], ['set-cookie', 'b=2']]"]];
  CTOR.forEach(function (c) { t('c.' + c[0], function () { var h = eval('new Headers(' + c[1] + ')'); return [dump(h), ents(h)]; }); });
  t('c.set-cookie.obj', function () { var h = new Headers({ 'Set-Cookie': 'a=1' }); return [dump(h), h.getSetCookie()]; });
  // ---- operations
  t('op.append.combine', function () { var h = new Headers(); h.append('A', '1'); h.append('a', '2'); h.append('a', '3'); return [h.get('a'), h.get('A'), dump(h)]; });
  t('op.set.replace', function () { var h = new Headers(); h.append('a', '1'); h.append('a', '2'); h.set('A', '9'); return [h.get('a'), dump(h)]; });
  t('op.set.order', function () { var h = new Headers(); h.set('b', '1'); h.set('a', '2'); h.set('c', '3'); h.set('b', '4'); return [dump(h), Array.from(h.keys()), Array.from(h.values())]; });
  t('op.sort', function () { var h = new Headers([['Zeta', '1'], ['alpha', '2'], ['Beta', '3'], ['_u', '4'], ['9n', '5'], ['a-b', '6'], ['a_b', '7'], ['A.b', '8']]); return Array.from(h.keys()); });
  t('op.get.missing', function () { return [new Headers().get('x'), new Headers().has('x')]; });
  t('op.has.case', function () { var h = new Headers({ 'X-A': '1' }); return [h.has('x-a'), h.has('X-A'), h.has('x-b')]; });
  t('op.delete', function () { var h = new Headers({ a: '1', b: '2' }); var r = h.delete('A'); return [r, dump(h), h.delete('zz')]; });
  t('op.args', function () { var o = {}; var h = new Headers(); ['append', 'set', 'get', 'has', 'delete', 'forEach', 'getSetCookie'].forEach(function (m) { try { h[m](); o[m] = 'ok'; } catch (e) { o[m] = e.message; } }); o.append1 = (function () { try { h.append('a'); } catch (e) { return e.message; } })(); o.set1 = (function () { try { h.set('a'); } catch (e) { return e.message; } })(); return o; });
  t('op.lens', function () { var o = {}; ['append', 'set', 'get', 'has', 'delete', 'forEach', 'getSetCookie', 'entries', 'keys', 'values'].forEach(function (m) { o[m] = Headers.prototype[m].length; }); return o; });
  t('op.badname', function () { var h = new Headers(), o = {}; ['append', 'set', 'get', 'has', 'delete'].forEach(function (m) { try { h[m]('a b', 'v'); o[m] = 'ok'; } catch (e) { o[m] = e.name + ': ' + e.message; } }); return o; });
  t('op.badvalue', function () { var h = new Headers(), o = {}; ['append', 'set'].forEach(function (m) { try { h[m]('a', 'x\ny'); o[m] = 'ok'; } catch (e) { o[m] = e.name + ': ' + e.message; } }); return o; });
  t('op.conv', function () { var h = new Headers(); h.append(5, 6); h.set(null, undefined); return dump(h); });
  t('op.sym', function () { var h = new Headers(); try { h.append(Symbol(), 'v'); } catch (e) { return e.name + ': ' + e.message; } });
  t('op.value.norm', function () { var h = new Headers(); h.append('a', '\n x \r'); h.append('b', ' \t y\t '); return dump(h); });
  t('op.wide', function () { var h = new Headers(); try { h.append('a', '中'); } catch (e) { return e.name + ': ' + e.message; } });
  // ---- set-cookie
  t('sc.basic', function () { var h = new Headers(); h.append('Set-Cookie', 'a=1'); h.append('set-cookie', 'b=2; Path=/'); h.append('X-A', '1'); return [h.getSetCookie(), h.get('set-cookie'), ents(h), dump(h)]; });
  t('sc.empty', function () { return new Headers().getSetCookie(); });
  t('sc.set', function () { var h = new Headers(); h.append('set-cookie', 'a=1'); h.set('Set-Cookie', 'z=9'); return [h.getSetCookie(), h.has('SET-COOKIE')]; });
  t('sc.delete', function () { var h = new Headers(); h.append('set-cookie', 'a=1'); h.delete('set-cookie'); return h.getSetCookie(); });
  t('sc.copy', function () { var h = new Headers(); h.append('set-cookie', 'a=1'); h.append('set-cookie', 'b=2'); var c = new Headers(h); return [c.getSetCookie(), ents(c)]; });
  t('sc.order', function () { var h = new Headers([['b', '1'], ['Set-Cookie', 'x'], ['a', '2'], ['set-cookie', 'y']]); return [ents(h), h.getSetCookie(), Array.from(h.keys())]; });
  t('sc.response', function () { var r = new Response('', { headers: [['Set-Cookie', 'q=1'], ['set-cookie', 'r=2']] }); return [r.headers.getSetCookie(), r.headers.get('set-cookie')]; });
  t('sc.brand', function () { return Headers.prototype.getSetCookie.call({}); });
  // ---- iteration protocol
  t('iter.shape', function () { var h = new Headers({ a: '1' }); var it = h.entries(); return [Object.prototype.toString.call(it), typeof it.next, it[Symbol.iterator]() === it, Object.getPrototypeOf(Object.getPrototypeOf(it)) === Object.getPrototypeOf(Object.getPrototypeOf([][Symbol.iterator]())), Object.getOwnPropertyNames(Object.getPrototypeOf(it)), it.next(), it.next(), it.next()]; });
  t('iter.kinds', function () { var h = new Headers({ a: '1', b: '2' }); return [Array.from(h.keys()), Array.from(h.values()), Array.from(h.entries()), Array.from(h), Object.prototype.toString.call(h.keys()), Object.prototype.toString.call(h.values())]; });
  t('iter.live', function () { var h = new Headers({ a: '1', c: '3' }); var it = h.keys(); var o = [it.next().value]; h.set('b', '2'); o.push(it.next().value, it.next().value); return o; });
  t('iter.live.delete', function () { var h = new Headers({ a: '1', b: '2', c: '3' }); var it = h.keys(); var o = [it.next().value]; h.delete('b'); o.push(it.next().value, it.next().done); return o; });
  t('iter.forEach', function () { var h = new Headers({ a: '1', B: '2' }); var log = [], self = {}; h.forEach(function (v, k, hh) { log.push([v, k, hh === h, this === self]); }, self); return log; });
  t('iter.forEach.bad', function () { var o = []; [undefined, 5, {}].forEach(function (x) { try { new Headers().forEach(x); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); return o; });
  t('iter.forEach.mutate', function () { var h = new Headers({ a: '1', b: '2' }); var seen = []; h.forEach(function (v, k) { seen.push(k); if (k === 'a') h.append('c', '3'); }); return seen; });
  t('iter.spread', function () { var h = new Headers({ a: '1' }); var [[k, v]] = h; return [k, v, [...h.keys()], Object.fromEntries(h)]; });
  t('json', function () { return [JSON.stringify(new Headers({ a: '1' })), Object.keys(new Headers({ a: '1' })), Object.getOwnPropertyNames(new Headers({ a: '1' })), String(new Headers())]; });
  t('own', function () { var h = new Headers({ a: '1' }); return [Object.getOwnPropertyNames(h), Object.getOwnPropertySymbols(h).length, h instanceof Headers]; });
  t('headers.from.request', function () { var r = new Request('http://x.test/', { headers: { 'X-A': '1' } }); return [r.headers instanceof Headers, dump(r.headers), Object.getOwnPropertyNames(r).includes('headers')]; });
  t('headers.from.response', function () { var r = new Response('x', { headers: { 'X-B': '2' } }); return [r.headers instanceof Headers, dump(r.headers), Object.getOwnPropertyNames(r).includes('headers')]; });
  t('headers.response.ct', function () { var r = new Response('x'); var r2 = new Response(new Blob(['a'], { type: 'text/x' })); var r3 = new Response(new URLSearchParams('a=1')); var r4 = new Response('{}', { headers: { 'content-type': 'application/json' } }); return [dump(r.headers), dump(r2.headers), dump(r3.headers), dump(r4.headers)]; });
  return JSON.stringify(R);
})()
