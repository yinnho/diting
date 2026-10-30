// Request / Response probe (M4 knife 16): shape, construction validation, defaults,
// body-derived content-type, static methods, clone/bodyUsed. Runs in Chrome AND the engine.
(async function () {
  var R = {};
  function conv(v) { return v === undefined ? '#undefined' : JSON.parse(JSON.stringify(v, function (k, x) { return x === undefined ? '#undef' : x; })); }
  function t(n, f) { try { R[n] = conv(f()); } catch (e) { R[n] = 'THROW ' + e.name + ': ' + String(e.message).slice(0, 220); } }
  async function ta(n, f) { try { R[n] = conv(await f()); } catch (e) { R[n] = 'THROW ' + e.name + ': ' + String(e.message).slice(0, 220); } }
  function kind(o, m) { var d = Object.getOwnPropertyDescriptor(o, m); return (d.get || d.set ? (d.set ? 'rw' : 'ro') : typeof d.value + ':' + (typeof d.value === 'function' ? d.value.length : '')); }
  function shape(C) {
    var P = C.prototype, s = { order: Object.getOwnPropertyNames(P), len: C.length, statics: Object.getOwnPropertyNames(C).filter(function (k) { return k !== 'length' && k !== 'name' && k !== 'prototype'; }), kinds: {}, tag: Object.prototype.toString.call(Object.create(P)) };
    s.order.forEach(function (m) { s.kinds[m] = kind(P, m); });
    return s;
  }
  R['shape.Request'] = shape(Request);
  R['shape.Response'] = shape(Response);
  function dump(h) { var o = []; h.forEach(function (v, k) { o.push([k, v]); }); return o; }
  var u = 'http://probe.test/a?b=1#h';
  t('req.call', function () { return Request(u); });
  t('res.call', function () { return Response(); });
  t('req.noargs', function () { return new Request(); });
  t('req.own', function () { return Object.getOwnPropertyNames(new Request(u)); });
  t('res.own', function () { return Object.getOwnPropertyNames(new Response()); });
  t('req.defaults', function () { var r = new Request(u); return { url: r.url, method: r.method, mode: r.mode, credentials: r.credentials, cache: r.cache, redirect: r.redirect, referrer: r.referrer, referrerPolicy: r.referrerPolicy, integrity: r.integrity, keepalive: r.keepalive, destination: r.destination, duplex: r.duplex, isReload: r.isReloadNavigation, isHist: r.isHistoryNavigation, bodyUsed: r.bodyUsed, body: r.body, sig: r.signal.aborted, hdr: dump(r.headers) }; });
  t('res.defaults', function () { var r = new Response(); return { status: r.status, statusText: r.statusText, ok: r.ok, type: r.type, url: r.url, redirected: r.redirected, bodyUsed: r.bodyUsed, body: r.body, hdr: dump(r.headers) }; });
  t('req.url.rel', function () { return new Request('/x/y').url; });
  t('req.url.badurl', function () { return new Request('http://').url; });
  t('req.url.obj', function () { return [new Request(new URL('http://probe.test/o')).url, new Request({ toString: function () { return 'http://probe.test/ts'; } }).url]; });
  t('req.url.cred', function () { return new Request('http://user:pw@probe.test/').url; });
  t('req.method.norm', function () { return ['get', 'post', 'Put', 'delete', 'head', 'options', 'patch', 'PaTch', 'foo'].map(function (m) { try { return new Request(u, { method: m }).method; } catch (e) { return e.name; } }); });
  t('req.method.bad', function () { return ['CONNECT', 'TRACE', 'TRACK', 'a b', ''].map(function (m) { try { return new Request(u, { method: m }).method; } catch (e) { return e.name + ': ' + e.message; } }); });
  t('req.get.body', function () { try { new Request(u, { body: 'x' }); } catch (e) { return e.name + ': ' + e.message; } });
  t('req.head.body', function () { try { new Request(u, { method: 'HEAD', body: 'x' }); } catch (e) { return e.name + ': ' + e.message; } });
  t('req.get.nullbody', function () { return new Request(u, { body: null }).body; });
  t('req.enum.bad', function () { var o = {}; ['mode', 'credentials', 'cache', 'redirect', 'referrerPolicy', 'duplex', 'priority'].forEach(function (k) { try { var i = {}; i[k] = 'zz'; new Request(u, i); o[k] = 'ok'; } catch (e) { o[k] = e.name + ': ' + e.message; } }); return o; });
  t('req.enum.good', function () { var r = new Request(u, { mode: 'no-cors', credentials: 'include', cache: 'no-store', redirect: 'manual', referrerPolicy: 'origin', integrity: 'sha256-x', keepalive: 1 }); return [r.mode, r.credentials, r.cache, r.redirect, r.referrerPolicy, r.integrity, r.keepalive]; });
  t('req.mode.navigate', function () { try { new Request(u, { mode: 'navigate' }); } catch (e) { return e.name + ': ' + e.message; } });
  t('req.referrer', function () { return [new Request(u, { referrer: '' }).referrer, new Request(u, { referrer: 'about:client' }).referrer, new Request(u, { referrer: '/rel' }).referrer, new Request(u, { referrer: 'http://probe.test/r' }).referrer]; });
  t('req.referrer.bad', function () { try { return new Request(u, { referrer: 'http://other.test/r' }).referrer; } catch (e) { return e.name + ': ' + e.message; } });
  t('req.init.type', function () { return [5, 'x', true].map(function (i) { try { new Request(u, i); return 'ok'; } catch (e) { return e.name + ': ' + e.message; } }); });
  t('req.init.null', function () { return new Request(u, null).method; });
  t('req.signal', function () { var c = new AbortController(); var r = new Request(u, { signal: c.signal }); var r2 = new Request(u); c.abort(); return [r.signal === c.signal, r.signal.aborted, r2.signal instanceof AbortSignal, r2.signal.aborted, r.signal === r2.signal]; });
  t('req.signal.bad', function () { try { new Request(u, { signal: {} }); } catch (e) { return e.name + ': ' + e.message; } });
  t('req.from.request', function () { var a = new Request(u, { method: 'POST', body: 'zz', headers: { 'X-A': '1' }, credentials: 'include', cache: 'no-cache' }); var b = new Request(a); return [b.method, b.credentials, b.cache, dump(b.headers), a.bodyUsed, b.bodyUsed, b.url]; });
  t('req.from.request.override', function () { var a = new Request(u, { method: 'POST', body: 'zz', headers: { 'X-A': '1' } }); var b = new Request(a, { method: 'PUT', headers: { 'X-B': '2' } }); return [b.method, dump(b.headers)]; });
  t('req.body.ct', function () { var f = new FormData(); f.append('a', '1'); return [new Request(u, { method: 'POST', body: 'x' }), new Request(u, { method: 'POST', body: new URLSearchParams('a=1') }), new Request(u, { method: 'POST', body: new Blob(['a'], { type: 'text/x' }) }), new Request(u, { method: 'POST', body: new Uint8Array(2) }), new Request(u, { method: 'POST', body: new ArrayBuffer(2) }), new Request(u, { method: 'POST', body: f }), new Request(u, { method: 'POST', body: 'x', headers: { 'Content-Type': 'a/b' } })].map(function (r) { return (r.headers.get('content-type') || '').replace(/boundary=.*/, 'boundary=B'); }); });
  t('req.body.stream', function () { var r = new Request(u, { method: 'POST', body: 'abc' }); return [r.body instanceof ReadableStream, r.bodyUsed]; });
  t('req.tag', function () { return [String(new Request(u)), Object.prototype.toString.call(new Request(u)), new Request(u) instanceof Request]; });
  t('req.json', function () { return JSON.stringify(new Request(u)); });
  await ta('req.text', async function () { var r = new Request(u, { method: 'POST', body: 'héllo' }); var x = await r.text(); return [x, r.bodyUsed]; });
  await ta('req.text.twice', async function () { var r = new Request(u, { method: 'POST', body: 'a' }); await r.text(); try { await r.text(); } catch (e) { return e.name + ': ' + e.message; } return 'ok'; });
  await ta('req.text.nobody', async function () { var r = new Request(u); return [await r.text(), r.bodyUsed]; });
  await ta('req.json.bad', async function () { try { await new Request(u, { method: 'POST', body: 'x' }).json(); } catch (e) { return e.name; } });
  await ta('req.clone', async function () { var r = new Request(u, { method: 'POST', body: 'abc', headers: { a: '1' } }); var c = r.clone(); return [await c.text(), r.bodyUsed, c.bodyUsed, await r.text(), c.method, dump(c.headers), c === r]; });
  await ta('req.clone.used', async function () { var r = new Request(u, { method: 'POST', body: 'abc' }); await r.text(); try { r.clone(); } catch (e) { return e.name + ': ' + e.message; } });
  await ta('req.blob', async function () { var b = await new Request(u, { method: 'POST', body: 'ab', headers: { 'content-type': 'x/y' } }).blob(); return [b.size, b.type]; });
  await ta('req.ab', async function () { var b = await new Request(u, { method: 'POST', body: new Uint8Array([1, 2, 3]) }).arrayBuffer(); return Array.from(new Uint8Array(b)); });
  await ta('req.bytes', async function () { var r = new Request(u, { method: 'POST', body: 'ab' }); return typeof r.bytes === 'function' ? Array.from(await r.bytes()) : 'nobytes'; });
  await ta('req.formData', async function () { var r = new Request(u, { method: 'POST', body: new URLSearchParams('a=1&b=2') }); var f = await r.formData(); return Array.from(f.entries()); });
  await ta('req.readstream', async function () { var r = new Request(u, { method: 'POST', body: 'abc' }); var rd = r.body.getReader(); var a = await rd.read(); return [Array.from(a.value), a.done, r.bodyUsed]; });
  t('req.headers.same', function () { var r = new Request(u); return r.headers === r.headers; });
  t('req.headers.mut', function () { var r = new Request(u, { headers: { a: '1' } }); r.headers.set('b', '2'); return dump(r.headers); });
  t('req.headers.assign', function () { var r = new Request(u); r.headers = 5; return r.headers instanceof Headers; });
  t('req.forbidden', function () { var r = new Request(u, { headers: { 'Host': 'x', 'Cookie': 'a=1', 'X-A': '1' } }); return dump(r.headers); });
  t('req.nocors.headers', function () { var r = new Request(u, { mode: 'no-cors', headers: { 'X-A': '1', 'Accept': 'x', 'Content-Type': 'application/json' } }); return dump(r.headers); });
  // ---- Response
  t('res.status.default', function () { return [new Response().status, new Response(null, { status: 204 }).status, new Response('', {}).statusText]; });
  t('res.status.range', function () { return [99, 100, 199, 200, 299, 300, 599, 600, 0, -1, 1e9].map(function (s) { try { return new Response(null, { status: s }).status; } catch (e) { return e.name + ': ' + e.message; } }); });
  t('res.status.conv', function () { return ['201', 201.9, true, null, undefined].map(function (s) { try { return new Response(null, { status: s }).status; } catch (e) { return e.name; } }); });
  t('res.status.nullbody', function () { return [101, 204, 205, 304].map(function (s) { try { return new Response('x', { status: s }).status; } catch (e) { return e.name + ': ' + e.message; } }); });
  t('res.statusText', function () { return ['OK', 'a\nb', 'é', '中', '', 5, null].map(function (s) { try { return new Response(null, { statusText: s }).statusText; } catch (e) { return e.name + ': ' + e.message; } }); });
  t('res.ok', function () { return [199, 200, 299, 300, 404].map(function (s) { return new Response(null, { status: s }).ok; }); });
  t('res.init.type', function () { return [5, 'x', true].map(function (i) { try { new Response(null, i); return 'ok'; } catch (e) { return e.name + ': ' + e.message; } }); });
  t('res.init.null', function () { return new Response(null, null).status; });
  t('res.ignores', function () { var r = new Response('x', { type: 'cors', url: 'http://z/', redirected: true }); return [r.type, r.url, r.redirected]; });
  t('res.body.ct', function () { var f = new FormData(); f.append('a', '1'); return [new Response('x'), new Response(new Blob(['a'], { type: 'text/x' })), new Response(new Blob(['a'])), new Response(new URLSearchParams('a=1')), new Response(new Uint8Array(2)), new Response(new ArrayBuffer(2)), new Response(f), new Response(null), new Response('x', { headers: { 'Content-Type': 'a/b' } }), new Response(new ReadableStream())].map(function (r) { return (r.headers.get('content-type') || '').replace(/boundary=.*/, 'boundary=B'); }); });
  t('res.body.type', function () { return [new Response('x').body instanceof ReadableStream, new Response(null).body, new Response('').body instanceof ReadableStream, new Response(null, { status: 204 }).body]; });
  t('res.body.bad', function () { var o = []; [5, true, {}, [1, 2], Symbol.iterator && 1].forEach(function (b) { try { o.push(new Response(b).headers.get('content-type')); } catch (e) { o.push(e.name + ': ' + e.message); } }); return o; });
  t('res.headers.same', function () { var r = new Response(); return r.headers === r.headers; });
  t('res.headers.init', function () { return [dump(new Response(null, { headers: [['a', '1'], ['b', '2']] }).headers), dump(new Response(null, { headers: new Headers({ c: '3' }) }).headers)]; });
  t('res.headers.bad', function () { try { new Response(null, { headers: { 'a b': '1' } }); } catch (e) { return e.name + ': ' + e.message; } });
  t('res.tag', function () { return [String(new Response()), Object.prototype.toString.call(new Response()), JSON.stringify(new Response())]; });
  t('res.static.error', function () { var r = Response.error(); return [r.status, r.type, r.ok, r.statusText, dump(r.headers), r.body]; });
  t('res.static.redirect', function () { var r = Response.redirect('http://probe.test/to', 301); return [r.status, r.type, dump(r.headers), r.ok, r.redirected, r.body]; });
  t('res.static.redirect.def', function () { return Response.redirect('/rel').status + ':' + Response.redirect('/rel').headers.get('location'); });
  t('res.static.redirect.bad', function () { return [200, 304, 308, 0].map(function (s) { try { return Response.redirect('http://probe.test/', s).status; } catch (e) { return e.name + ': ' + e.message; } }); });
  t('res.static.redirect.badurl', function () { try { Response.redirect('http://'); } catch (e) { return e.name + ': ' + e.message; } });
  t('res.static.json', function () { var r = Response.json({ a: 1 }); return [r.status, dump(r.headers), r.ok]; });
  t('res.static.json.init', function () { var r = Response.json([1], { status: 201, statusText: 'C', headers: { 'X-A': '1' } }); return [r.status, r.statusText, dump(r.headers)]; });
  t('res.static.json.ct', function () { return dump(Response.json(1, { headers: { 'content-type': 'x/y' } }).headers); });
  t('res.static.json.bad', function () { var o = []; [undefined, function () {}, Symbol('s'), 1n].forEach(function (v) { try { o.push(Response.json(v).status); } catch (e) { o.push(e.name + ': ' + e.message); } }); return o; });
  t('res.static.json.cyc', function () { var a = {}; a.a = a; try { Response.json(a); } catch (e) { return e.name; } });
  await ta('res.text', async function () { var r = new Response('héllo'); var x = await r.text(); return [x, r.bodyUsed]; });
  await ta('res.text.twice', async function () { var r = new Response('a'); await r.text(); try { await r.text(); } catch (e) { return e.name + ': ' + e.message; } return 'ok'; });
  await ta('res.json', async function () { return await new Response('{"a":[1]}').json(); });
  await ta('res.json.bad', async function () { try { await new Response('x').json(); } catch (e) { return e.name; } });
  await ta('res.text.null', async function () { var r = new Response(null); return [await r.text(), r.bodyUsed]; });
  await ta('res.blob', async function () { var b = await new Response('ab').blob(); return [b.size, b.type]; });
  await ta('res.blob.type', async function () { var b = await new Response('ab', { headers: { 'content-type': 'x/y' } }).blob(); return [b.size, b.type]; });
  await ta('res.ab', async function () { return Array.from(new Uint8Array(await new Response(new Uint8Array([1, 2, 3])).arrayBuffer())); });
  await ta('res.bytes', async function () { var r = new Response('ab'); return typeof r.bytes === 'function' ? Array.from(await r.bytes()) : 'nobytes'; });
  await ta('res.formData', async function () { var f = await new Response(new URLSearchParams('a=1&b=2')).formData(); return Array.from(f.entries()); });
  await ta('res.formData.bad', async function () { try { await new Response('x').formData(); } catch (e) { return e.name + ': ' + e.message; } });
  await ta('res.clone', async function () { var r = new Response('abc', { status: 201, statusText: 'C', headers: { a: '1' } }); var c = r.clone(); return [await c.text(), r.bodyUsed, c.bodyUsed, await r.text(), c.status, c.statusText, dump(c.headers), c === r]; });
  await ta('res.clone.used', async function () { var r = new Response('abc'); await r.text(); try { r.clone(); } catch (e) { return e.name + ': ' + e.message; } });
  await ta('res.stream', async function () { var r = new Response('abc'); var rd = r.body.getReader(); var a = await rd.read(); var b = await rd.read(); return [Array.from(a.value), a.done, b.done, r.bodyUsed]; });
  await ta('res.stream.then.text', async function () { var r = new Response('abc'); r.body; try { return await r.text(); } catch (e) { return e.name; } });
  await ta('res.stream.locked', async function () { var r = new Response('abc'); r.body.getReader(); try { await r.text(); } catch (e) { return e.name + ': ' + e.message; } });
  await ta('res.fromstream', async function () { var s = new ReadableStream({ start: function (c) { c.enqueue(new TextEncoder().encode('st')); c.close(); } }); return await new Response(s).text(); });
  t('req.tas', function () { return new Request(u).targetAddressSpace; });
  await ta('res.text.gbk', async function () { return await new Response(new Uint8Array([0xc4, 0xe3, 0xba, 0xc3]), { headers: { 'content-type': 'text/plain;charset=gbk' } }).text(); });
  await ta('res.static.error.text', async function () { return await Response.error().text(); });
  await ta('res.static.json.text', async function () { return await Response.json({ a: [1, 'é'] }).text(); });
  return JSON.stringify(R).split(location.origin).join('ORIGIN');
})()
