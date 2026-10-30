// NamedNodeMap + live Attr identity probe (M4 knife 44).
(function () {
  var R = {};
  function conv(v) { return v === undefined ? '#undefined' : JSON.parse(JSON.stringify(v, function (k, x) { return x === undefined ? '#undef' : x; })); }
  function t(n, f) { try { R[n] = conv(f()); } catch (e) { R[n] = 'THROW ' + e.name + ': ' + String(e.message).slice(0, 240); } }
  function kind(o, m) { var d = Object.getOwnPropertyDescriptor(o, m); return (d.get || d.set ? (d.set ? 'rw' : 'ro') : typeof d.value + ':' + (typeof d.value === 'function' ? d.value.length : '')); }
  function shape(C) { var P = C.prototype, s = { order: Object.getOwnPropertyNames(P), syms: Object.getOwnPropertySymbols(P).map(String), len: C.length, kinds: {}, tag: Object.prototype.toString.call(Object.create(P)) }; s.order.forEach(function (m) { s.kinds[m] = kind(P, m); }); return s; }
  R['shape.NamedNodeMap'] = shape(NamedNodeMap);
  function mk() { var d = document.createElement('div'); d.setAttribute('id', 'a'); d.setAttribute('class', 'b c'); d.setAttribute('data-x', '1'); return d; }
  t('ctor', function () { try { new NamedNodeMap(); } catch (e) { return e.name + ': ' + e.message; } });
  t('basic', function () { var d = mk(), m = d.attributes; return [m instanceof NamedNodeMap, Object.prototype.toString.call(m), m.length, m === d.attributes, Object.getOwnPropertyNames(m), Object.keys(m), m[0].name, m[2].value, m[3], m.item(1).name, m.item(9), m.item(-1)]; });
  t('named', function () { var d = mk(), m = d.attributes; return [m.id.value, m['data-x'].value, 'id' in m, 'zz' in m, m.zz, m.getNamedItem('class').value, m.getNamedItem('CLASS').value, m.getNamedItem('zz'), m.length]; });
  t('forin', function () { var d = mk(), o = []; for (var k in d.attributes) o.push(k); return o; });
  t('iter', function () { var d = mk(); var o = []; for (var a of d.attributes) o.push(a.name); return [o, Array.from(d.attributes).map(function (a) { return a.value; }), [].map.call(d.attributes, function (a) { return a.name; }), d.attributes[Symbol.iterator] === Array.prototype.values]; });
  t('live', function () { var d = mk(), m = d.attributes; d.setAttribute('title', 't'); var a = m.length; d.removeAttribute('id'); return [a, m.length, m[0].name, m.id, Array.from(m).map(function (x) { return x.name; })]; });
  t('identity', function () { var d = mk(); var a = d.getAttributeNode('id'), b = d.getAttributeNode('id'); return [a === b, a === d.attributes[0], a === d.attributes.getNamedItem('id'), a.ownerElement === d, d.attributes[1] === d.attributes[1]]; });
  t('live.value', function () { var d = mk(); var a = d.getAttributeNode('id'); d.setAttribute('id', 'z'); var v1 = a.value; a.value = 'y'; return [v1, a.value, d.getAttribute('id'), d.id]; });
  t('removed.detached', function () { var d = mk(); var a = d.getAttributeNode('id'); d.removeAttribute('id'); return [a.ownerElement, a.value, d.getAttributeNode('id'), d.hasAttribute('id')]; });
  t('readded.identity', function () { var d = mk(); var a = d.getAttributeNode('id'); d.removeAttribute('id'); d.setAttribute('id', 'q'); var b = d.getAttributeNode('id'); return [a === b, a.ownerElement, b.ownerElement === d, b.value]; });
  t('setNamedItem', function () { var d = mk(), m = d.attributes; var a = document.createAttribute('title'); a.value = 'T'; var r = m.setNamedItem(a); var a2 = document.createAttribute('title'); a2.value = 'U'; var r2 = m.setNamedItem(a2); return [r, r2 === a, d.getAttribute('title'), a.ownerElement, a2.ownerElement === d, d.getAttributeNode('title') === a2, m.length]; });
  t('setAttributeNode', function () { var d = mk(); var a = document.createAttribute('lang'); a.value = 'fr'; var r = d.setAttributeNode(a); return [r, d.getAttributeNode('lang') === a, a.ownerElement === d, d.lang]; });
  t('inuse', function () { var d = mk(), e = mk(); var a = d.getAttributeNode('id'); try { e.setAttributeNode(a); } catch (x) { return x.name + ': ' + x.message; } });
  t('inuse.same', function () { var d = mk(); var a = d.getAttributeNode('id'); return d.setAttributeNode(a) === a; });
  t('setNamedItem.bad', function () { var d = mk(); return [function () { d.attributes.setNamedItem({}); }, function () { d.attributes.setNamedItem(); }, function () { d.setAttributeNode(null); }].map(function (f) { try { f(); return 'ok'; } catch (e) { return e.name + ': ' + e.message; } }); });
  t('removeNamedItem', function () { var d = mk(), m = d.attributes; var a = m.getNamedItem('class'); var r = m.removeNamedItem('class'); var o = [r === a, d.hasAttribute('class'), a.ownerElement, m.length]; try { m.removeNamedItem('zz'); } catch (e) { o.push(e.name + ': ' + e.message); } return o; });
  t('removeAttributeNode', function () { var d = mk(); var a = d.getAttributeNode('id'); var r = d.removeAttributeNode(a); var o = [r === a, d.hasAttribute('id')]; try { d.removeAttributeNode(a); } catch (e) { o.push(e.name + ': ' + e.message); } return o; });
  t('ns', function () { var d = mk(); var a = document.createAttributeNS('urn:x', 'p:k'); a.value = 'v'; d.attributes.setNamedItemNS(a); var g = d.attributes.getNamedItemNS('urn:x', 'k'); return [g === a, g.name, g.localName, g.prefix, g.namespaceURI, d.getAttributeNS('urn:x', 'k'), d.getAttributeNodeNS('urn:x', 'k') === a]; });
  t('ns.remove', function () { var d = mk(); d.setAttributeNS('urn:x', 'p:k', 'v'); var r = d.attributes.removeNamedItemNS('urn:x', 'k'); var o = [r.name, d.hasAttributeNS('urn:x', 'k')]; try { d.attributes.removeNamedItemNS('urn:x', 'zz'); } catch (e) { o.push(e.name + ': ' + e.message); } return o; });
  t('args', function () { var m = mk().attributes; return ['item', 'getNamedItem', 'getNamedItemNS', 'setNamedItem', 'setNamedItemNS', 'removeNamedItem', 'removeNamedItemNS'].map(function (n) { try { m[n](); return 'ok'; } catch (e) { return e.name + ': ' + e.message; } }); });
  t('brand', function () { var o = []; try { Object.getOwnPropertyDescriptor(NamedNodeMap.prototype, 'length').get.call({}); } catch (e) { o.push(e.name + ': ' + e.message); } try { NamedNodeMap.prototype.item.call({}, 0); } catch (e) { o.push(e.name + ': ' + e.message); } return o; });
  t('case.html', function () { var d = document.createElement('div'); d.setAttribute('DATA-Y', '1'); return [d.attributes[0].name, d.getAttributeNode('DATA-Y').name, d.attributes.getNamedItem('data-Y').value, d.attributes['data-y'].value]; });
  t('empty', function () { var d = document.createElement('p'); return [d.attributes.length, Object.getOwnPropertyNames(d.attributes), d.attributes[0], JSON.stringify(Array.from(d.attributes))]; });
  t('clone', function () { var d = mk(); var c = d.cloneNode(); return [c.attributes.length, c.getAttributeNode('id') === d.getAttributeNode('id'), c.getAttributeNode('id').ownerElement === c]; });
  t('equal', function () { var a = mk(), b = mk(); return [a.isEqualNode(b), a.getAttributeNode('id').isEqualNode(b.getAttributeNode('id'))]; });
  t('getAttributeNames', function () { return mk().getAttributeNames(); });
  t('toggle', function () { var d = mk(); d.toggleAttribute('hidden'); return [d.attributes.length, d.attributes.hidden.value, d.getAttributeNode('hidden').specified]; });
  return JSON.stringify(R);
})()
