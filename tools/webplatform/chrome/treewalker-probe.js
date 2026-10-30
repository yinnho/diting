// TreeWalker / NodeIterator probe (M4 knife 38): shape, construction validation, traversal corpus.
(function () {
  var R = {};
  function conv(v) { return v === undefined ? '#undefined' : JSON.parse(JSON.stringify(v, function (k, x) { return x === undefined ? '#undef' : x; })); }
  function t(n, f) { try { R[n] = conv(f()); } catch (e) { R[n] = 'THROW ' + e.name + ': ' + String(e.message).slice(0, 220); } }
  function kind(o, m) { var d = Object.getOwnPropertyDescriptor(o, m); return (d.get || d.set ? (d.set ? 'rw' : 'ro') : typeof d.value + ':' + (typeof d.value === 'function' ? d.value.length : '')); }
  function shape(C) { var P = C.prototype, s = { order: Object.getOwnPropertyNames(P), len: C.length, kinds: {}, tag: Object.prototype.toString.call(Object.create(P)) }; s.order.forEach(function (m) { s.kinds[m] = kind(P, m); }); return s; }
  R['shape.TreeWalker'] = shape(TreeWalker); R['shape.NodeIterator'] = shape(NodeIterator);
  Array.prototype.slice.call(document.querySelectorAll('#tw')).forEach(function (e) { e.remove(); });
  var root = document.createElement('div'); root.id = 'tw';
  root.innerHTML = '<ul id="u"><li id="a">A<b id="ab">x</b></li><li id="b"><!--c-->B</li><li id="c"><i id="ci"></i></li></ul><p id="p">P<span id="s">S</span></p>';
  document.body.appendChild(root);
  function nm(n) { return n === null ? null : n.nodeType === 1 ? n.nodeName + (n.id ? '#' + n.id : '') : n.nodeType === 3 ? '"' + n.data + '"' : n.nodeType === 8 ? '<!--' + n.data + '-->' : 'n' + n.nodeType; }
  t('ctor', function () { try { new TreeWalker(); } catch (e) { return e.name + ': ' + e.message; } });
  t('ctor.ni', function () { try { new NodeIterator(); } catch (e) { return e.name + ': ' + e.message; } });
  t('noargs', function () { try { document.createTreeWalker(); } catch (e) { return e.name + ': ' + e.message; } });
  t('noargs.ni', function () { try { document.createNodeIterator(); } catch (e) { return e.name + ': ' + e.message; } });
  t('badroot', function () { return [{}, null, 5, 'x'].map(function (r) { try { document.createTreeWalker(r); return 'ok'; } catch (e) { return e.name + ': ' + e.message; } }); });
  t('badroot.ni', function () { return [{}, null, 5].map(function (r) { try { document.createNodeIterator(r); return 'ok'; } catch (e) { return e.name + ': ' + e.message; } }); });
  t('defaults', function () { var w = document.createTreeWalker(root); return [w.root === root, w.currentNode === root, w.whatToShow, w.filter, Object.getOwnPropertyNames(w), w instanceof TreeWalker, Object.prototype.toString.call(w), Object.keys(w)]; });
  t('defaults.ni', function () { var w = document.createNodeIterator(root); return [w.root === root, w.referenceNode === root, w.pointerBeforeReferenceNode, w.whatToShow, w.filter, Object.getOwnPropertyNames(w), w instanceof NodeIterator, Object.prototype.toString.call(w)]; });
  t('whatToShow.conv', function () { return [0, 1, 4294967295, -1, 4294967296, 'abc', '5', 1.9, null, undefined, NaN].map(function (v) { return document.createTreeWalker(root, v).whatToShow; }); });
  t('filter.forms', function () { var f = function () { return 1; }; var o = { acceptNode: function () { return 1; } }; return [document.createTreeWalker(root, 1, f).filter === f, document.createTreeWalker(root, 1, o).filter === o, document.createTreeWalker(root, 1, null).filter, document.createTreeWalker(root, 1, undefined).filter]; });
  t('filter.bad', function () { return [5, 'x', true].map(function (f) { try { document.createTreeWalker(root, 1, f); return 'ok'; } catch (e) { return e.name + ': ' + e.message; } }); });
  t('currentNode.set', function () { var w = document.createTreeWalker(root); var o = []; var el = document.getElementById('a'); w.currentNode = el; o.push(w.currentNode === el); [null, 5, {}].forEach(function (v) { try { w.currentNode = v; o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); return o; });
  t('readonly', function () { var w = document.createTreeWalker(root); w.root = 5; w.whatToShow = 7; w.filter = 3; return [w.root === root, w.whatToShow, w.filter]; });
  function walk(w, steps) { var o = []; for (var i = 0; i < steps; i++) { var n = w.nextNode(); if (!n) break; o.push(nm(n)); } return o; }
  t('next.all', function () { return walk(document.createTreeWalker(root), 40); });
  t('next.elements', function () { return walk(document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT), 40); });
  t('next.text', function () { return walk(document.createTreeWalker(root, NodeFilter.SHOW_TEXT), 40); });
  t('next.comment', function () { return walk(document.createTreeWalker(root, NodeFilter.SHOW_COMMENT), 40); });
  t('next.zero', function () { return walk(document.createTreeWalker(root, 0), 40); });
  t('filter.skip', function () { return walk(document.createTreeWalker(root, 1, function (n) { return n.id === 'u' ? 3 : 1; }), 40); });
  t('filter.reject', function () { return walk(document.createTreeWalker(root, 1, function (n) { return n.id === 'u' ? 2 : 1; }), 40); });
  t('filter.reject.li', function () { return walk(document.createTreeWalker(root, 1, { acceptNode: function (n) { return n.nodeName === 'LI' ? 2 : 1; } }), 40); });
  t('filter.odd', function () { return [0, 4, 'x', null, undefined, true, 1.5].map(function (v) { return walk(document.createTreeWalker(root, 1, function (n) { return n.id === 'a' ? v : 1; }), 3); }); });
  t('filter.throw', function () { try { document.createTreeWalker(root, 1, function () { throw new RangeError('boom'); }).nextNode(); } catch (e) { return e.name + ': ' + e.message; } });
  t('filter.this', function () { var th; var f = function () { th = this; return 1; }; document.createTreeWalker(root, 1, f).nextNode(); var o = { acceptNode: function () { th = [this === o]; return 1; } }; document.createTreeWalker(root, 1, o).nextNode(); return [typeof th, th]; });
  t('filter.args', function () { var args; document.createTreeWalker(root, 1, function () { args = arguments.length; return 1; }).nextNode(); return args; });
  t('prev', function () { var w = document.createTreeWalker(root, 1); w.currentNode = document.getElementById('s'); var o = []; for (var i = 0; i < 40; i++) { var n = w.previousNode(); if (!n) break; o.push(nm(n)); } return o; });
  t('children', function () { var w = document.createTreeWalker(root, 1); var u = w.firstChild(); var o = [nm(u)]; o.push(nm(w.firstChild()), nm(w.nextSibling()), nm(w.nextSibling()), nm(w.nextSibling()), nm(w.lastChild()), nm(w.previousSibling()), nm(w.parentNode()), nm(w.parentNode()), nm(w.parentNode()), nm(w.parentNode())); return o; });
  t('children.skip', function () { var w = document.createTreeWalker(root, 1, function (n) { return n.id === 'u' ? 3 : 1; }); return [nm(w.firstChild()), nm(w.firstChild()), nm(w.lastChild()), nm(w.parentNode())]; });
  t('children.reject', function () { var w = document.createTreeWalker(root, 1, function (n) { return n.id === 'u' ? 2 : 1; }); return [nm(w.firstChild()), nm(w.lastChild())]; });
  t('root.bounds', function () { var w = document.createTreeWalker(document.getElementById('u'), 1); w.currentNode = document.getElementById('u'); return [nm(w.parentNode()), nm(w.nextSibling()), nm(w.previousSibling()), nm(w.previousNode())]; });
  t('currentNode.outside', function () { var w = document.createTreeWalker(document.getElementById('u'), 1); w.currentNode = document.getElementById('p'); return [nm(w.parentNode()), nm(w.nextNode()), nm(w.currentNode)]; });
  t('methods.len', function () { var P = TreeWalker.prototype; return ['nextNode', 'previousNode', 'firstChild', 'lastChild', 'nextSibling', 'previousSibling', 'parentNode'].map(function (m) { return P[m].length; }); });
  t('create.lens', function () { return [document.createTreeWalker.length, document.createNodeIterator.length]; });
  t('brand', function () { var o = []; ['root', 'currentNode', 'whatToShow', 'filter'].forEach(function (m) { try { Object.getOwnPropertyDescriptor(TreeWalker.prototype, m).get.call({}); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); ['nextNode', 'firstChild'].forEach(function (m) { try { TreeWalker.prototype[m].call({}); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); ['root', 'referenceNode', 'pointerBeforeReferenceNode'].forEach(function (m) { try { Object.getOwnPropertyDescriptor(NodeIterator.prototype, m).get.call({}); o.push('ok'); } catch (e) { o.push(e.name + ': ' + e.message); } }); try { NodeIterator.prototype.nextNode.call({}); } catch (e) { o.push(e.name + ': ' + e.message); } return o; });
  // ---- NodeIterator
  function nwalk(it, dir, steps) { var o = []; for (var i = 0; i < steps; i++) { var n = dir > 0 ? it.nextNode() : it.previousNode(); if (!n) { o.push(null); break; } o.push(nm(n)); } return o; }
  t('ni.fwd', function () { return nwalk(document.createNodeIterator(root), 1, 40); });
  t('ni.elements', function () { return nwalk(document.createNodeIterator(root, 1), 1, 40); });
  t('ni.text', function () { return nwalk(document.createNodeIterator(root, 4), 1, 40); });
  t('ni.back.first', function () { var it = document.createNodeIterator(root); return [nm(it.previousNode()), it.pointerBeforeReferenceNode, nm(it.referenceNode)]; });
  t('ni.reverse', function () { var it = document.createNodeIterator(root, 1); var o = [nm(it.nextNode()), nm(it.nextNode()), nm(it.previousNode()), it.pointerBeforeReferenceNode, nm(it.previousNode()), nm(it.nextNode()), it.pointerBeforeReferenceNode]; return o; });
  t('ni.state', function () { var it = document.createNodeIterator(root, 1); var o = [it.pointerBeforeReferenceNode, nm(it.referenceNode)]; it.nextNode(); o.push(it.pointerBeforeReferenceNode, nm(it.referenceNode)); it.nextNode(); o.push(it.pointerBeforeReferenceNode, nm(it.referenceNode)); return o; });
  t('ni.end', function () { var it = document.createNodeIterator(document.getElementById('s'), 1); return [nm(it.nextNode()), nm(it.nextNode()), nm(it.nextNode()), nm(it.previousNode()), nm(it.previousNode())]; });
  t('ni.reject', function () { return nwalk(document.createNodeIterator(root, 1, function (n) { return n.id === 'u' ? 2 : 1; }), 1, 40); });
  t('ni.skip', function () { return nwalk(document.createNodeIterator(root, 1, function (n) { return n.id === 'u' ? 3 : 1; }), 1, 40); });
  t('ni.filter.obj', function () { return nwalk(document.createNodeIterator(root, 1, { acceptNode: function (n) { return n.nodeName === 'LI' ? 1 : 3; } }), 1, 40); });
  t('ni.detach', function () { var it = document.createNodeIterator(root); return [it.detach(), typeof it.nextNode()]; });
  t('ni.readonly', function () { var it = document.createNodeIterator(root); it.root = 1; it.referenceNode = 2; it.pointerBeforeReferenceNode = false; return [it.root === root, it.referenceNode === root, it.pointerBeforeReferenceNode]; });
  t('ni.methods.len', function () { var P = NodeIterator.prototype; return ['nextNode', 'previousNode', 'detach'].map(function (m) { return P[m].length; }); });
  t('nodefilter', function () { var o = {}; Object.getOwnPropertyNames(NodeFilter).forEach(function (k) { o[k] = typeof NodeFilter[k] === 'number' ? NodeFilter[k] : typeof NodeFilter[k]; }); return [o, Object.getOwnPropertyNames(NodeFilter.prototype || {}), typeof NodeFilter]; });
  root.remove();
  return JSON.stringify(R);
})()
