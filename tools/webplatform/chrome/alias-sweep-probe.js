// (刀96) LegacyAlias identity sweep: every window key with a
// webkit/WebKit prefix vs its prefix-stripped twin. Chrome 145 truth
// (alias-sweep-truth-145.json): exactly 8 SAME aliases — the sweep CANNOT
// see WebKitCSSMatrix (twin name "CSSMatrix" does not exist), so known
// LegacyAlias pairs are also probed directly alongside.
(function () {
  const r = { aliases: {}, dmx: {}, url: {} };
  for (const k of Object.getOwnPropertyNames(globalThis)) {
    if (!/^webkit/i.test(k)) continue;
    const twin = k.replace(/^webkit/i, '');
    if (!twin) continue;
    const A = globalThis[k], B = globalThis[twin];
    if (A === undefined || B === undefined) continue;
    r.aliases[k] = (A === B) ? 'SAME' : 'diff:' + typeof A + '/' + typeof B;
  }
  const both = (C) => ({
    own: Object.getOwnPropertyNames(C).filter((n) => n !== 'name' && n !== 'length' && n !== 'prototype').sort().join(','),
    len: C.length, name: C.name,
    call: (() => { try { C(); } catch (e) { return e.name + ':' + String(e.message).slice(0, 100); } })(),
    protoCtor: (C.prototype && C.prototype.constructor) === C ? 'self' : String(C.prototype && C.prototype.constructor && C.prototype.constructor.name),
  });
  r.dmx.DOMMatrix = both(DOMMatrix);
  r.dmx.DMR = Object.getOwnPropertyNames(DOMMatrixReadOnly).filter((n) => n !== 'name' && n !== 'length' && n !== 'prototype').sort().join(',');
  r.url.URL = both(URL);
  r.url.urlStatics = Object.getOwnPropertyNames(URL).filter((n) => n !== 'name' && n !== 'length' && n !== 'prototype').sort().join(',');
  // direct pairs the prefix sweep is blind to (twin name is not the strip)
  const pairs = [['webkitURL', 'URL'], ['WebKitCSSMatrix', 'DOMMatrix']];
  for (const [a, b] of pairs) r.aliases[a + '===' + b] = (globalThis[a] === globalThis[b]) ? 'SAME' : 'diff';
  return JSON.stringify(r);
})()
