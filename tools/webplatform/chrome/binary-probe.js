// Binary-fidelity probe: do the BYTES survive every path a page can push them through?
// Runs in real Chrome AND in the diting engine; the two JSON results are compared.
// (Motivated by the Pinduoduo editor's `Error: invalid wire type 7 at offset 2` - a protobuf decoder
// fed bytes that had been through a text round-trip: 0xEF 0xBF 0xBD, the UTF-8 form of U+FFFD,
// reads back as "wire type 7".)
// Each case yields a fingerprint "len:fnv1a32" of the bytes (or code units) it produced, or "THROW ...".
(async function () {
  var R = {};
  function fnv(len, at) { var h = 0x811c9dc5; for (var i = 0; i < len; i++) { h ^= at(i); h = Math.imul(h, 0x01000193) >>> 0; } return len + ':' + h.toString(16); }
  function fb(u8) { u8 = u8 instanceof Uint8Array ? u8 : new Uint8Array(u8); return fnv(u8.length, function (i) { return u8[i]; }); }
  function fs(s) { return fnv(s.length, function (i) { return s.charCodeAt(i); }); }
  async function t(name, f) { try { R[name] = await f(); } catch (e) { R[name] = 'THROW ' + (e && e.constructor && e.constructor.name) + ': ' + String(e && e.message).slice(0, 80); } }

  // 256 byte values + sequences that are INVALID UTF-8 (a text round-trip would replace them)
  var B = new Uint8Array(256 + 20);
  for (var i = 0; i < 256; i++) B[i] = i;
  [0xEF, 0xBF, 0xBD, 0xC0, 0xC1, 0xF5, 0xFF, 0xFE, 0x80, 0x8F, 0x90, 0xE0, 0x80, 0x80, 0xED, 0xA0, 0x80, 0xF4, 0x90, 0x80].forEach(function (b, k) { B[256 + k] = b; });
  R['input'] = fb(B);
  // a realistic protobuf-ish message: field1 varint, field2 bytes (high bytes), field3 fixed32
  var PB = new Uint8Array([0x08, 0x96, 0x01, 0x12, 0x05, 0xFF, 0xFE, 0xFD, 0x80, 0x81, 0x1D, 0xEF, 0xBE, 0xAD, 0xDE]);
  R['pb.input'] = fb(PB);

  // ---- Blob
  await t('blob.arrayBuffer', async function () { return fb(await new Blob([B]).arrayBuffer()); });
  await t('blob.bytes()', async function () { return typeof Blob.prototype.bytes === 'function' ? fb(await new Blob([B]).bytes()) : 'absent'; });
  await t('blob.text.len', async function () { return fs(await new Blob([B]).text()); });
  await t('blob.slice.arrayBuffer', async function () { return fb(await new Blob([B]).slice(10, 200).arrayBuffer()); });
  await t('blob.of.ab', async function () { return fb(await new Blob([B.buffer]).arrayBuffer()); });
  await t('blob.of.dataview', async function () { return fb(await new Blob([new DataView(B.buffer)]).arrayBuffer()); });
  await t('blob.of.blob+u8', async function () { return fb(await new Blob([new Blob([PB]), PB]).arrayBuffer()); });
  // ---- Response
  await t('response.arrayBuffer', async function () { return fb(await new Response(B).arrayBuffer()); });
  await t('response.blob', async function () { return fb(await (await new Response(B).blob()).arrayBuffer()); });
  await t('response.bytes()', async function () { return typeof Response.prototype.bytes === 'function' ? fb(await new Response(B).bytes()) : 'absent'; });
  await t('response.text.len', async function () { return fs(await new Response(B).text()); });
  await t('response.clone.arrayBuffer', async function () { var r = new Response(PB); return fb(await r.clone().arrayBuffer()); });
  await t('response.of.ab', async function () { return fb(await new Response(PB.buffer).arrayBuffer()); });
  // ---- Request (the known text-round-trip suspect)
  await t('request.u8.arrayBuffer', async function () { return fb(await new Request('http://x.test/', { method: 'POST', body: B }).arrayBuffer()); });
  await t('request.pb.arrayBuffer', async function () { return fb(await new Request('http://x.test/', { method: 'POST', body: PB }).arrayBuffer()); });
  await t('request.ab.arrayBuffer', async function () { return fb(await new Request('http://x.test/', { method: 'POST', body: PB.buffer }).arrayBuffer()); });
  await t('request.dv.arrayBuffer', async function () { return fb(await new Request('http://x.test/', { method: 'POST', body: new DataView(PB.buffer) }).arrayBuffer()); });
  await t('request.blob.arrayBuffer', async function () { return fb(await new Request('http://x.test/', { method: 'POST', body: new Blob([PB]) }).arrayBuffer()); });
  await t('request.string.arrayBuffer', async function () { return fb(await new Request('http://x.test/', { method: 'POST', body: 'héllo 你好' }).arrayBuffer()); });
  await t('request.u8.text.len', async function () { return fs(await new Request('http://x.test/', { method: 'POST', body: PB }).text()); });
  await t('request.u8.blob', async function () { return fb(await (await new Request('http://x.test/', { method: 'POST', body: PB }).blob()).arrayBuffer()); });
  await t('request.clone.arrayBuffer', async function () { return fb(await new Request('http://x.test/', { method: 'POST', body: PB }).clone().arrayBuffer()); });
  await t('request.bytes()', async function () { var r = new Request('http://x.test/', { method: 'POST', body: PB }); return typeof r.bytes === 'function' ? fb(await r.bytes()) : 'absent'; });
  // ---- FileReader
  function rd(m) { return new Promise(function (res, rej) { var fr = new FileReader(); fr.onload = function () { res(fr.result); }; fr.onerror = function () { rej(fr.error || new Error('fr error')); }; fr[m](new Blob([B])); }); }
  await t('filereader.arrayBuffer', async function () { return fb(await rd('readAsArrayBuffer')); });
  await t('filereader.binaryString', async function () { return fs(await rd('readAsBinaryString')); });
  await t('filereader.dataURL', async function () { return fs(await rd('readAsDataURL')); });
  await t('filereader.text.len', async function () { return fs(await rd('readAsText')); });
  // ---- fetch of blob:/data: URLs
  await t('fetch.blobURL.arrayBuffer', async function () { var u = URL.createObjectURL(new Blob([B])); try { return fb(await (await fetch(u)).arrayBuffer()); } finally { URL.revokeObjectURL(u); } });
  await t('fetch.dataURL.arrayBuffer', async function () { var s = ''; for (var i = 0; i < B.length; i++) s += String.fromCharCode(B[i]); return fb(await (await fetch('data:application/octet-stream;base64,' + btoa(s))).arrayBuffer()); });
  // ---- XHR over blob:
  function xhr(rt, sync) { return new Promise(function (res, rej) { var u = URL.createObjectURL(new Blob([B])); var x = new XMLHttpRequest(); x.open('GET', u, !sync); x.responseType = rt; x.onload = function () { res(x); }; x.onerror = function () { rej(new Error('xhr error')); }; x.send(); if (sync) res(x); }); }
  await t('xhr.arraybuffer', async function () { var x = await xhr('arraybuffer'); return fb(x.response); });
  await t('xhr.blob', async function () { var x = await xhr('blob'); return fb(await x.response.arrayBuffer()); });
  await t('xhr.text.len', async function () { var x = await xhr('text'); return fs(x.responseText); });
  // ---- messaging / cloning
  await t('structuredClone.u8', async function () { return fb(structuredClone(B)); });
  await t('structuredClone.ab', async function () { return fb(structuredClone(B.buffer)); });
  await t('structuredClone.transfer', async function () { var c = B.slice(); var o = structuredClone(c.buffer, { transfer: [c.buffer] }); return fb(o) + '|detached=' + (c.buffer.byteLength === 0); });
  async function viaPort(msg, transfer) { return await new Promise(function (res, rej) { var ch = new MessageChannel(); ch.port1.onmessage = function (e) { res(e.data); }; ch.port2.postMessage(msg, transfer || []); setTimeout(function () { rej(new Error('timeout')); }, 3000); }); }
  await t('port.u8', async function () { return fb(await viaPort(B)); });
  await t('port.ab', async function () { return fb(await viaPort(B.buffer.slice(0))); });
  await t('port.transfer', async function () { var c = B.slice().buffer; var o = await viaPort(c, [c]); return fb(o) + '|detached=' + (c.byteLength === 0); });
  await t('port.nested', async function () { var o = await viaPort({ a: PB, b: [PB.buffer.slice(0)], c: 'x' }); return fb(o.a) + ',' + fb(o.b[0]); });
  // ---- text codecs / base64
  await t('btoa.atob', async function () { var s = ''; for (var i = 0; i < B.length; i++) s += String.fromCharCode(B[i]); var r = atob(btoa(s)); return fs(r); });
  await t('textencoder.ascii', async function () { return fb(new TextEncoder().encode('héllo 你好 😀')); });
  await t('textdecoder.invalid.len', async function () { return fs(new TextDecoder().decode(B)); });
  await t('textdecoder.fatal', async function () { try { new TextDecoder('utf-8', { fatal: true }).decode(B); return 'no-throw'; } catch (e) { return 'throws ' + e.constructor.name; } });
  await t('textdecoder.latin1', async function () { try { return fs(new TextDecoder('latin1').decode(B)); } catch (e) { return 'THROW ' + e.constructor.name; } });
  await t('textdecoder.stream', async function () { var d = new TextDecoder(); var a = d.decode(PB.subarray(0, 7), { stream: true }); var b = d.decode(PB.subarray(7)); return fs(a + b); });
  // ---- typed array / buffer semantics protobuf readers rely on
  await t('ta.slice.subarray', async function () { var s = B.subarray(5, 25); return fb(s) + '|off=' + s.byteOffset + '|' + fb(B.slice(5, 25).buffer); });
  await t('ta.set.copyWithin', async function () { var c = new Uint8Array(32); c.set(B.subarray(250, 270), 4); c.copyWithin(0, 4, 12); return fb(c); });
  await t('dataview.reads', async function () { var d = new DataView(B.buffer, 3); return [d.getUint32(0, true), d.getFloat64(8, true), d.getUint16(2), d.getInt32(4, true)].join(','); });
  await t('arraybuffer.slice.resize', async function () { return fb(B.buffer.slice(1, 9)) + '|resizable=' + ('resizable' in ArrayBuffer.prototype); });
  await t('u8.from.iter', async function () { return fb(Uint8Array.from(B)) + fb(Uint8Array.of(1, 2, 255)); });
  // ---- natively computed over the bytes (proves the platform saw the same bytes)
  await t('subtle.sha256', async function () { var h = new Uint8Array(await crypto.subtle.digest('SHA-256', B)); return Array.from(h.slice(0, 8)).map(function (x) { return x.toString(16).padStart(2, '0'); }).join(''); });
  await t('compress.gzip.roundtrip', async function () {
    if (typeof CompressionStream !== 'function') return 'absent';
    var cs = new Blob([B]).stream().pipeThrough(new CompressionStream('gzip')); var gz = new Uint8Array(await new Response(cs).arrayBuffer());
    var ds = new Blob([gz]).stream().pipeThrough(new DecompressionStream('gzip')); return fb(await new Response(ds).arrayBuffer());
  });

  // ---- WebSocket (the Pinduoduo SharedWorker reads protobuf frames from `event.data` with binaryType='arraybuffer')
  function wsRound(binaryType, payload) {
    return new Promise(function (res, rej) {
      var ws = new WebSocket('ws://127.0.0.1:__WS_PORT__/'); if (binaryType) ws.binaryType = binaryType;
      var to = setTimeout(function () { rej(new Error('ws timeout')); }, 1500);
      ws.onopen = function () { ws.send(payload); };
      ws.onmessage = function (e) { clearTimeout(to); var d = e.data; res({ d: d, bt: ws.binaryType }); ws.close(); };
      ws.onerror = function () { clearTimeout(to); rej(new Error('ws error')); };
    });
  }
  function kind(d) { return Object.prototype.toString.call(d); }
  await t('ws.default.binaryType', async function () { return (await wsRound(null, PB)).bt; });
  await t('ws.arraybuffer.kind', async function () { return kind((await wsRound('arraybuffer', PB)).d); });
  await t('ws.arraybuffer.bytes', async function () { return fb((await wsRound('arraybuffer', B)).d); });
  await t('ws.arraybuffer.pb', async function () { return fb((await wsRound('arraybuffer', PB)).d); });
  await t('ws.arraybuffer.isAB', async function () { var d = (await wsRound('arraybuffer', PB)).d; return [d instanceof ArrayBuffer, ArrayBuffer.isView(d), typeof d.byteLength, typeof d.length].join(','); });
  await t('ws.arraybuffer.slice.kind', async function () { var d = (await wsRound('arraybuffer', PB)).d; return kind(d.slice(0, 4)); });
  await t('ws.arraybuffer.u8view', async function () { var d = (await wsRound('arraybuffer', PB)).d; var u = new Uint8Array(d); return fb(u) + '|' + u.byteOffset + '|' + u.buffer.byteLength; });
  await t('ws.arraybuffer.dataview', async function () { var d = (await wsRound('arraybuffer', PB)).d; var v = new DataView(d); return v.getUint8(0) + ',' + v.getUint16(1, true) + ',' + v.getUint32(11, true); });
  await t('ws.blob.kind', async function () { return kind((await wsRound('blob', PB)).d); });
  await t('ws.blob.bytes', async function () { var d = (await wsRound('blob', B)).d; return fb(await d.arrayBuffer()); });
  await t('ws.default.kind', async function () { return kind((await wsRound(null, PB)).d); });
  await t('ws.text', async function () { var d = (await wsRound('arraybuffer', 'héllo 你好 😀')).d; return kind(d) + '|' + fs(d); });
  await t('ws.send.u8.subarray', async function () { return fb((await wsRound('arraybuffer', B.subarray(5, 25))).d); });
  await t('ws.send.ab', async function () { return fb((await wsRound('arraybuffer', B.buffer.slice(3, 40))).d); });
  await t('ws.send.blob', async function () { return fb((await wsRound('arraybuffer', new Blob([PB]))).d); });
  return JSON.stringify(R);
})()
