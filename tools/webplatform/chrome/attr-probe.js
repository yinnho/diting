// Attr probe (M4 knife 43): Attr is a Node; members live on the prototype; nodeType/nodeName/nodeValue via Node.prototype.
(function () {
  var R = {};
  function conv(v) { return v === undefined ? '#undefined' : JSON.parse(JSON.stringify(v, function (k, x) { return x === undefined ? '#undef' : x; })); }
  function t(n, f) { try { R[n] = conv(f()); } catch (e) { R[n] = 'THROW ' + e.name + ': ' + String(e.message).slice(0, 200); } }
  var d = document.createElement('div'); d.setAttribute('data-x', 'v1'); d.id = 'q';
  t('inst', function () { var a = d.getAttributeNode('data-x'); return [a instanceof Attr, a instanceof Node, Object.prototype.toString.call(a), Object.getOwnPropertyNames(a), Object.getPrototypeOf(Attr.prototype) === Node.prototype]; });
  t('fields', function () { var a = d.getAttributeNode('data-x'); return [a.name, a.localName, a.value, a.namespaceURI, a.prefix, a.ownerElement === d, a.specified, a.nodeType, a.nodeName, a.nodeValue, a.textContent]; });
  t('node.members', function () { var a = d.getAttributeNode('data-x'); return [a.parentNode, a.firstChild, a.lastChild, a.previousSibling, a.nextSibling, a.childNodes.length, a.hasChildNodes(), a.isConnected, a.ownerDocument === document]; });
  t('value.set', function () { var a = d.getAttributeNode('data-x'); a.value = 'v2'; return [a.value, a.nodeValue, d.getAttribute('data-x')]; });
  t('create', function () { var a = document.createAttribute('foo'); a.value = 'bar'; return [a.name, a.value, a.ownerElement, a instanceof Attr, a.nodeType]; });
  t('create.bad', function () { try { document.createAttribute('1a b'); } catch (e) { return e.name; } });
  t('ctor', function () { try { new Attr(); return 'ok'; } catch (e) { return e.name + ': ' + e.message; } });
  t('proto.kinds', function () { var o = {}; Object.getOwnPropertyNames(Attr.prototype).forEach(function (m) { var x = Object.getOwnPropertyDescriptor(Attr.prototype, m); o[m] = x.get ? (x.set ? 'rw' : 'ro') : typeof x.value; }); return o; });
  t('node.has', function () { var a = d.getAttributeNode('id'); return ['nodeType', 'nodeName', 'nodeValue'].map(function (k) { return Object.prototype.hasOwnProperty.call(Attr.prototype, k); }); });
  t('compare', function () { var a = d.getAttributeNode('id'), b = d.getAttributeNode('id'); return [a === b, a.isSameNode(b), a.isEqualNode(b)]; });
  t('named', function () { var m = d.attributes; return [m.length, m[0].name, m.item(0).name, m.getNamedItem('id').value]; });
  return JSON.stringify(R);
})()
