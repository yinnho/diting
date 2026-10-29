// Element staged-family probe (M4 knife 4): the nine members the knife-2
// ledger staged — attributeStyleMap/computedStyleMap (Typed OM), part
// (DOMTokenList), requestFullscreen/webkitRequestFullScreen/
// webkitRequestFullscreen/requestPointerLock (fullscreen family), moveBefore,
// ariaNotify. Runs in real Chrome AND in the diting engine; diffed. Promises
// (fullscreen/pointer-lock) are observed with a 3s race so an unsettled
// promise is recorded as ['pending'] instead of hanging the probe.
(function () {
  var R = {};
  function t(name, f) { try { var v = f(); R[name] = v === undefined ? '#undefined' : JSON.parse(JSON.stringify(v)); } catch (e) { R[name] = 'THROW ' + (e && e.constructor && e.constructor.name) + (e && e.name && e.name !== (e.constructor && e.constructor.name) ? '(' + e.name + ')' : '') + ': ' + (e && e.message); } }
  var PEND = [];
  function settle(key, pr) {
    PEND.push(Promise.race([
      pr.then(function (v) { return ['resolve', v === undefined ? '#undefined' : v]; },
              function (e) { return ['reject', e && e.name, String(e && e.message).slice(0, 220)]; }),
      new Promise(function (res) { setTimeout(function () { res(['pending']); }, 3000); })
    ]).then(function (v) { R[key] = v; }));
  }
  var div = document.createElement('div');
  document.body.appendChild(div);
  var svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  document.body.appendChild(svg);

  // ---- descriptor faces (explicit; no staged filter)
  t('EL.kinds', function () {
    var o = {};
    ['requestFullscreen', 'webkitRequestFullScreen', 'webkitRequestFullscreen', 'requestPointerLock', 'moveBefore', 'part', 'computedStyleMap', 'ariaNotify'].forEach(function (n) {
      var d = Object.getOwnPropertyDescriptor(Element.prototype, n);
      if (!d) { o[n] = 'MISSING'; return; }
      o[n] = d.get || d.set ? (d.set ? 'rw' : 'ro') : { fn: true, len: d.value.length, name: d.value.name };
    });
    return o;
  });
  t('HT.kinds', function () {
    var o = {};
    ['attributeStyleMap', 'computedStyleMap', 'ariaNotify'].forEach(function (n) {
      var d = Object.getOwnPropertyDescriptor(HTMLElement.prototype, n);
      if (!d) { o[n] = 'MISSING'; return; }
      o[n] = d.get || d.set ? (d.set ? 'rw' : 'ro') : { fn: true, len: d.value.length, name: d.value.name };
    });
    return o;
  });
  t('owner.ariaNotify', function () {
    return [
      Object.prototype.hasOwnProperty.call(Element.prototype, 'ariaNotify'),
      Object.prototype.hasOwnProperty.call(HTMLElement.prototype, 'ariaNotify'),
      Object.prototype.hasOwnProperty.call(SVGElement.prototype, 'ariaNotify')
    ];
  });
  t('DOC.moveBefore', function () { var d = Object.getOwnPropertyDescriptor(Document.prototype, 'moveBefore'); return d ? { len: d.value.length } : 'MISSING'; });
  t('FRAG.moveBefore', function () { return typeof DocumentFragment.prototype.moveBefore; });

  // ---- part: DOMTokenList face
  t('part.default', function () { var p = div.part; return [typeof p, p instanceof DOMTokenList, p.length, p.value, String(p)]; });
  t('part.same', function () { return div.part === div.part; });
  t('part.set', function () { div.part = 'a b'; return [div.getAttribute('part'), div.part.length, div.part.value]; });
  t('part.add', function () { div.part.add('c'); return [div.getAttribute('part'), div.part.contains('c'), div.part.length]; });
  t('part.setAttr', function () { div.setAttribute('part', 'x y'); return [div.part.length, div.part.value, div.part.item(1)]; });
  t('part.svg', function () { return [typeof svg.part, svg.part instanceof DOMTokenList, svg.part.length]; });

  // ---- attributeStyleMap / computedStyleMap
  t('asm.face', function () { var m = div.attributeStyleMap; return [typeof m, m && m.constructor && m.constructor.name, m && m.size, m && String(m)]; });
  t('asm.identity', function () { return div.attributeStyleMap === div.attributeStyleMap; });
  t('asm.assign', function () { div.attributeStyleMap = 5; return 'assigned'; });
  t('asm.get', function () { return [div.attributeStyleMap.get('color'), div.attributeStyleMap.has('color')]; });
  t('csm.face', function () { var m = div.computedStyleMap(); return [typeof m, m && m.constructor && m.constructor.name, m && m.size]; });
  t('csm.identity', function () { return div.computedStyleMap() === div.computedStyleMap(); });
  t('spm.classes', function () {
    return [Object.getOwnPropertyNames(StylePropertyMapReadOnly.prototype),
            Object.getOwnPropertyNames(StylePropertyMap.prototype)];
  });
  t('spm.ctor', function () { try { new StylePropertyMap(); } catch (e) { return 'THROW ' + (e && e.name) + ': ' + (e && e.message); } });
  t('spm.symbols', function () {
    return [Object.getOwnPropertySymbols(StylePropertyMapReadOnly.prototype).map(String),
            Object.getOwnPropertySymbols(StylePropertyMap.prototype).map(String)];
  });
  t('part.desc', function () { var d = Object.getOwnPropertyDescriptor(Element.prototype, 'part'); return [d.get.name, d.set.name, d.get.length, d.set.length]; });
  t('asm.withstyle', function () { var d = document.createElement('div'); d.setAttribute('style', 'color: red'); var v = d.attributeStyleMap.get('color'); return [d.attributeStyleMap.size, v && v.constructor && v.constructor.name, v && String(v)]; });
  t('gcs.len', function () { return getComputedStyle(div).length; });

  // ---- fullscreen family
  t('rfs.face', function () { var p = div.requestFullscreen(); settle('rfs.plain', p); return [typeof p, p instanceof Promise]; });
  t('wrfs.face', function () { var v = div.webkitRequestFullScreen(); settle('wrfs.plain', v); return [typeof v]; });
  t('wrfsu.face', function () { var v = div.webkitRequestFullscreen(); settle('wrfsu.plain', v); return [typeof v]; });
  t('rpl.face', function () { var p = div.requestPointerLock(); settle('rpl.plain', p); return [typeof p, p instanceof Promise, p && p.constructor && p.constructor.name]; });
  t('rfs.args', function () { return [Element.prototype.requestFullscreen.length, Element.prototype.webkitRequestFullScreen.length, Element.prototype.webkitRequestFullscreen.length, Element.prototype.requestPointerLock.length]; });

  // ---- moveBefore
  t('mb.basic', function () {
    var p = document.createElement('div');
    var a = document.createElement('span'); var b = document.createElement('b');
    p.appendChild(a); p.appendChild(b);
    p.moveBefore(b, a);
    return [p.childNodes.length, p.firstChild.tagName];
  });
  t('mb.null', function () { var p = document.createElement('div'); p.moveBefore(null, null); return 'no-throw'; });
  t('mb.nonchild', function () { var p = document.createElement('div'); var x = document.createElement('i'); p.moveBefore(x, null); return 'no-throw'; });
  t('mb.badref', function () { var p = document.createElement('div'); var a = document.createElement('span'); p.appendChild(a); var other = document.createElement('b'); p.moveBefore(a, other); return 'no-throw'; });
  t('mb.selfmove', function () { var p = document.createElement('div'); var a = document.createElement('span'); p.appendChild(a); p.moveBefore(a, null); return [p.childNodes.length, p.firstChild.tagName]; });

  // ---- ariaNotify
  t('ariaNotify.face', function () { return div.ariaNotify('hi'); });

  // ---- moveBefore on Document / DocumentFragment (same op, other hosts)
  t('mb.doc', function () {
    try { document.moveBefore(document.createElement('i'), null); return 'no-throw'; }
    catch (e) { return 'THROW ' + (e && e.name) + ': ' + (e && e.message); }
  });
  t('mb.frag', function () {
    var f = document.createDocumentFragment();
    var a = document.createElement('span'); var b = document.createElement('b');
    f.appendChild(a); f.appendChild(b);
    f.moveBefore(b, a);
    return [f.childNodes.length, f.firstChild.tagName];
  });

  return Promise.all(PEND).then(function () { return JSON.stringify(R); });
})()
