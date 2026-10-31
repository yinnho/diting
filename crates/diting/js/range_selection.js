(function () { // @@SCOPE
// Range / StaticRange / Selection.
// Split out of bootstrap.js wholesale (god-file ratchet, pure move): top level only
// declares bindings; every bootstrap helper it names is touched at call time.

// Live Range over the real DOM tree. dom/ranges/* tests are pure boundary-point
// algorithms (no layout, no editing engine), so a property-storing Range with
// correct tree-order comparison passes them. Mutating ops (extract/delete/
// insert/surround) are kept minimal: they do not throw, but do not rewrite the
// tree (that is the editing mega-bucket, out of scope).
// (刀90) raw cross-tree base/extent for Selection.toString — kept in a
// module WeakMap, not an own property: Chrome instances carry zero own keys
// and the engine's native slot only takes scalar values (an array leaks
// into Reflect.ownKeys; k73's `own` case catches exactly that).
const SEL_RAW = new WeakMap();
function _rngNodeLength(n) {
  const t = n.nodeType;
  if (t === 3 || t === 4 || t === 8 || t === 7) return (n.data || n.nodeValue || "").length;
  return n.childNodes.length;
}
// Index among siblings, computed in Rust (one op) instead of serializing the
// whole childNodes list per call: the Range matrices call this heavily.
function _rngNodeIndex(n) {
  if (!n.parentNode) return 0;
  return +_dom("node_index", n._nid);
}
function _rngSame(a, b) { return a === b || (!!a && !!b && a._nid === b._nid); }
// Root nid in one op (callers only read ._nid), instead of an O(depth) walk.
function _rngRoot(n) { return { _nid: +_dom("node_root", n._nid) }; }
function _rngAncestors(n) { const a = []; let c = n; while (c) { a.push(c); c = c.parentNode; } return a; }
// document (preorder) tree order: -1 if a precedes b, 1 if a follows b, 0 same.
// Computed in Rust (one op) rather than walking ancestor chains over per-step
// DOM ops, which made the large dom/ranges matrices time out.
function _rngOrder(a, b) {
  if (_rngSame(a, b)) return 0;
  return +_dom("compare_order", a._nid, b._nid) || 0;
}
// Position of (nA,oA) relative to (nB,oB): -1 before, 0 equal, 1 after.
function _rngCmp(nA, oA, nB, oB) {
  if (_rngSame(nA, nB)) return oA < oB ? -1 : (oA > oB ? 1 : 0);
  if (_rngOrder(nA, nB) > 0) return -_rngCmp(nB, oB, nA, oA);
  if (nA.contains && nA.contains(nB)) { // nA is a strict ancestor of nB
    let child = nB;
    while (child && child.parentNode && child.parentNode._nid !== nA._nid) child = child.parentNode;
    if (child && child.parentNode && child.parentNode._nid === nA._nid && _rngNodeIndex(child) < oA) return 1;
    return -1;
  }
  return -1;
}
function _rngCheckOffset(n, o) {
  if (n && n.nodeType === 10) throw new DOMException("Range boundary cannot be a DocumentType", "InvalidNodeTypeError");
  if (o < 0 || o > _rngNodeLength(n)) throw new DOMException("Range offset out of bounds", "IndexSizeError");
}
// (刀90) Selection.toString text extraction — Chrome's Selection uses a
// layout-flavored plain text (block boundaries emit '\n'), unlike
// Range.toString's pure concatenation (k90-text5/6: Range side has NO
// newlines and already matches). Newline counts from Chrome 145 probes
// (k90-text7/8/9): a block with UA vertical margin (p, h1-h6, blockquote,
// ul, ol, dl, dd, figure, pre) emits "\n\n" when the selection leaves it,
// a plain block (div, li, table, section...) emits one '\n', inline nodes
// emit nothing, <br> emits one. The leaving-block set is the INNERMOST
// block on the exit chain. Tail rule: the last segment's innermost block
// emits when its exit position is strictly before the range end, or equal
// to the end while the end container still has later children (n6: p1 at
// end-of-range emits because p2 follows; n1: no sibling -> silent).
function _selIsBlock(n) {
  if (!n || n.nodeType !== 1) return false;
  const t = n.nodeName;
  return t === 'DIV' || t === 'P' || t === 'UL' || t === 'OL' || t === 'LI' || t === 'DL' ||
    t === 'DT' || t === 'DD' || t === 'TABLE' || t === 'SECTION' || t === 'ARTICLE' ||
    t === 'ASIDE' || t === 'HEADER' || t === 'FOOTER' || t === 'MAIN' || t === 'NAV' ||
    t === 'BLOCKQUOTE' || t === 'PRE' || t === 'FIGURE' || t === 'FIGCAPTION' || t === 'FORM' ||
    t === 'FIELDSET' || t === 'ADDRESS' || t === 'DETAILS' || t === 'SUMMARY' || t === 'BODY' ||
    t === 'HTML' || (t >= 'H1' && t <= 'H6');
}
function _selBlockNewlines(n) {
  const t = n.nodeName;
  const margined = t === 'P' || t === 'BLOCKQUOTE' || t === 'PRE' || t === 'FIGURE' ||
    t === 'FIGCAPTION' || t === 'UL' || t === 'OL' || t === 'DL' || t === 'DD' || t === 'DT' ||
    t === 'FORM' || t === 'FIELDSET' || (t >= 'H1' && t <= 'H6');
  return margined ? 2 : 1;
}
// text segments of the range in DFS order: [node, from, to] plus a flag when
// a <br> sits between two segments.
function _selSegments(r) {
  const sc = r._sc, ec = r._ec;
  if (!sc) return [];
  const segs = [];
  if (_rngSame(sc, ec) && (sc.nodeType === 3 || sc.nodeType === 4)) {
    if (r._so < r._eo) segs.push([sc, r._so, r._eo]);
    return segs;
  }
  if (sc.nodeType === 3 || sc.nodeType === 4) segs.push([sc, r._so, -1]);
  const cac = r.commonAncestorContainer;
  if (cac) {
    const walk = (node) => {
      if (node.nodeType === 3 || node.nodeType === 4) {
        if (!_rngSame(node, sc) && !_rngSame(node, ec) &&
            _rngCmp(node, 0, sc, r._so) >= 0 && _rngCmp(node, _rngNodeLength(node), ec, r._eo) <= 0) {
          segs.push([node, 0, -1]);
        }
      } else if (node.nodeName === 'BR') {
        // a BR only separates text the range actually covers (k90-text7 part:
        // an out-of-range <br> in a sibling div must not emit).
        if (_rngCmp(node, 0, sc, r._so) >= 0 && _rngCmp(node, 1, ec, r._eo) <= 0) segs.push([node, -1, -1]);
      }
      const kids = node.childNodes;
      for (let i = 0; i < kids.length; i++) if (kids[i]) walk(kids[i]);
    };
    walk(cac);
  }
  if (!_rngSame(sc, ec) && (ec.nodeType === 3 || ec.nodeType === 4)) segs.push([ec, 0, r._eo]);
  return segs;
}
// innermost block exited when going from node A up to (not including) stop.
function _selExitBlock(a, stopNid) {
  let n = a;
  while (n && n._nid !== stopNid) {
    if (_selIsBlock(n)) return n;
    n = n.parentNode;
  }
  return null;
}
function _selPlainText(r) {
  const segs = _selSegments(r);
  if (!segs.length) return '';
  // LCA nid of two nodes, for the between-segments exit-chain walk below.
  const lcaNid = (a, b) => {
    const set = new Set(_rngAncestors(a).map(n => n._nid));
    let n = b;
    while (n) { if (set.has(n._nid)) return n._nid; n = n.parentNode; }
    return null;
  };
  const isText = (n) => n.nodeType === 3 || n.nodeType === 4;
  let s = '';
  for (let i = 0; i < segs.length; i++) {
    const seg = segs[i];
    if (isText(seg[0])) {
      // newline before this segment: BR -> one, or the innermost block on
      // the previous segment's exit chain toward the LCA.
      if (i > 0 && isText(segs[i - 1][0])) {
        const blk = _selExitBlock(segs[i - 1][0], lcaNid(segs[i - 1][0], seg[0]));
        if (blk) s += '\n'.repeat(_selBlockNewlines(blk));
      }
      s += seg[1] < 0 ? String(seg[0].data || '') : String(seg[0].data || '').slice(seg[1], seg[2] < 0 ? undefined : seg[2]);
    } else if (seg[0].nodeName === 'BR') { // BR between segments: exactly one newline
      if (s.length && i + 1 < segs.length) s += '\n';
    }
  }
  // tail: the last text segment's innermost block on the exit chain toward
  // the root of the range's tree.
  const last = segs[segs.length - 1][0];
  if (isText(last) && !(_rngSame(r._sc, r._ec) && r._so === r._eo)) {
    const rootNid = _rngRoot(r._sc)._nid;
    const blk = _selExitBlock(last, rootNid);
    if (blk) {
      const par = blk.parentNode;
      const exitEqEnd = par && _rngCmp(par, _rngNodeIndex(blk) + 1, r._ec, r._eo) === 0;
      const exitBeforeEnd = par && _rngCmp(par, _rngNodeIndex(blk) + 1, r._ec, r._eo) < 0;
      // equal-to-end emits only when the END container still has children
      // after the end offset (n6/n7: later sibling exists -> emit; n1/n4:
      // end sits at the container's last child -> silent).
      const siblingAfter = r._ec && r._ec.nodeType === 1 && r._eo < _rngNodeLength(r._ec);
      if (exitBeforeEnd || (exitEqEnd && siblingAfter)) s += '\n'.repeat(_selBlockNewlines(blk));
    }
  }
  return s;
}
// (刀90) a cross-tree setBaseAndExtent keeps the raw base/extent for
// String(): Chrome extracts within the BASE's tree only, walking toward the
// tree boundary on the extent's side (k90-str4 fwd: "ight text\nmid text\n"
// stops before the host; rev: from the shadow tree head down to base "sha").
// extend() collapses for real instead (ext:str is ""), and addRange sees a
// Range that setStart/setEnd already collapsed — both stay empty.
function _selCrossTreeRange(self) {
  const raw = SEL_RAW.get(self);
  if (!raw) return null;
  const b = raw[0], bO = raw[1], x = raw[2];
  const tb = b.getRootNode ? b.getRootNode() : null;
  const xr = x.getRootNode ? x.getRootNode() : null;
  if (!tb || !xr || _rngRoot(b)._nid === _rngRoot(x)._nid) return null;
  const tbIsShadow = tb.nodeType === 11 && !!tb.host;
  const xIsShadow = xr.nodeType === 11 && !!xr.host;
  let after;
  if (tbIsShadow && xIsShadow) after = _rngOrder(xr.host, tb.host) > 0;
  else if (tbIsShadow) after = _rngOrder(x, tb.host) > 0;
  else if (xIsShadow) after = _rngOrder(xr.host, b) > 0;
  else return null;
  // The extraction Range lives entirely inside the base's tree: fields are
  // written directly — setStart/setEnd would collapse a cross-tree pair.
  const r = new Range();
  if (after) {
    r._sc = b; r._so = bO;
    if (tbIsShadow) { r._ec = tb; r._eo = _rngNodeLength(tb); }
    else { const h = xr.host; r._ec = h.parentNode; r._eo = _rngNodeIndex(h); }
  } else {
    r._ec = b; r._eo = bO;
    if (tbIsShadow) { r._sc = tb; r._so = 0; }
    else { const h = xr.host; r._sc = h.parentNode; r._so = _rngNodeIndex(h) + 1; }
  }
  return r;
}
globalThis.Range = class Range {
  constructor() {
    const d = globalThis.document || null;
    this._sc = d; this._so = 0; this._ec = d; this._eo = 0;
    __hideOwn(this);
  }
  get startContainer() { return this._sc; }
  get startOffset() { return this._so; }
  get endContainer() { return this._ec; }
  get endOffset() { return this._eo; }
  get collapsed() { return _rngSame(this._sc, this._ec) && this._so === this._eo; }
  get commonAncestorContainer() {
    if (!this._sc || !this._ec) return null;
    const setA = new Set(_rngAncestors(this._sc).map(n => n._nid));
    let c = this._ec;
    while (c) { if (setA.has(c._nid)) return c; c = c.parentNode; }
    return null;
  }
  setStart(n, o) { _rngCheckOffset(n, o); this._sc = n; this._so = o; if (_rngRoot(n)._nid !== _rngRoot(this._ec)._nid || _rngCmp(this._sc, this._so, this._ec, this._eo) > 0) { this._ec = n; this._eo = o; } }
  setEnd(n, o) { _rngCheckOffset(n, o); this._ec = n; this._eo = o; if (_rngRoot(n)._nid !== _rngRoot(this._sc)._nid || _rngCmp(this._sc, this._so, this._ec, this._eo) > 0) { this._sc = n; this._so = o; } }
  setStartBefore(n) { const p = n.parentNode; if (!p) throw new DOMException("node has no parent", "InvalidNodeTypeError"); this.setStart(p, _rngNodeIndex(n)); }
  setStartAfter(n) { const p = n.parentNode; if (!p) throw new DOMException("node has no parent", "InvalidNodeTypeError"); this.setStart(p, _rngNodeIndex(n) + 1); }
  setEndBefore(n) { const p = n.parentNode; if (!p) throw new DOMException("node has no parent", "InvalidNodeTypeError"); this.setEnd(p, _rngNodeIndex(n)); }
  setEndAfter(n) { const p = n.parentNode; if (!p) throw new DOMException("node has no parent", "InvalidNodeTypeError"); this.setEnd(p, _rngNodeIndex(n) + 1); }
  collapse(toStart) { if (toStart) { this._ec = this._sc; this._eo = this._so; } else { this._sc = this._ec; this._so = this._eo; } }
  selectNode(n) { const p = n.parentNode; if (!p) throw new DOMException("node has no parent", "InvalidNodeTypeError"); const i = _rngNodeIndex(n); this._sc = p; this._so = i; this._ec = p; this._eo = i + 1; }
  selectNodeContents(n) { if (n && n.nodeType === 10) throw new DOMException("cannot select a DocumentType", "InvalidNodeTypeError"); const len = _rngNodeLength(n); this._sc = n; this._so = 0; this._ec = n; this._eo = len; }
  comparePoint(n, o) {
    o = o >>> 0; // offset is a WebIDL unsigned long: -1 -> 4294967295 -> IndexSizeError
    if (_rngRoot(n)._nid !== _rngRoot(this._sc)._nid) throw new DOMException("nodes are in different trees", "WrongDocumentError");
    if (n.nodeType === 10) throw new DOMException("node is a DocumentType", "InvalidNodeTypeError");
    if (o > _rngNodeLength(n)) throw new DOMException("offset out of bounds", "IndexSizeError");
    if (_rngCmp(n, o, this._sc, this._so) < 0) return -1;
    if (_rngCmp(n, o, this._ec, this._eo) > 0) return 1;
    return 0;
  }
  isPointInRange(n, o) {
    o = o >>> 0;
    if (!this._sc || _rngRoot(n)._nid !== _rngRoot(this._sc)._nid) return false;
    if (n.nodeType === 10) throw new DOMException("node is a DocumentType", "InvalidNodeTypeError");
    if (o > _rngNodeLength(n)) throw new DOMException("offset out of bounds", "IndexSizeError");
    return _rngCmp(n, o, this._sc, this._so) >= 0 && _rngCmp(n, o, this._ec, this._eo) <= 0;
  }
  compareBoundaryPoints(how, other) {
    // `how` is a WebIDL `unsigned short`: ToUint16-convert before validating,
    // so NaN/Infinity become 0 (START_TO_START) rather than throwing.
    let h = Math.trunc(Number(how));
    if (!Number.isFinite(h)) h = 0;
    h = ((h % 65536) + 65536) % 65536;
    let a, b;
    switch (h) {
      case 0: a = [this._sc, this._so]; b = [other._sc, other._so]; break; // START_TO_START
      case 1: a = [this._ec, this._eo]; b = [other._sc, other._so]; break; // START_TO_END
      case 2: a = [this._ec, this._eo]; b = [other._ec, other._eo]; break; // END_TO_END
      case 3: a = [this._sc, this._so]; b = [other._ec, other._eo]; break; // END_TO_START
      default: throw new DOMException("invalid comparison type", "NotSupportedError");
    }
    // Different roots -> WrongDocumentError. Guard so a null/foreign container
    // raises that DOMException rather than a raw TypeError from _rngRoot.
    let differ;
    try { differ = _rngRoot(a[0])._nid !== _rngRoot(b[0])._nid; }
    catch (e) { differ = true; }
    if (differ) throw new DOMException("The two Ranges are not in the same tree.", "WrongDocumentError");
    return _rngCmp(a[0], a[1], b[0], b[1]);
  }
  intersectsNode(n) {
    if (_rngRoot(n)._nid !== _rngRoot(this._sc)._nid) return false;
    const p = n.parentNode;
    if (!p) return true;
    const o = _rngNodeIndex(n);
    return _rngCmp(p, o, this._ec, this._eo) < 0 && _rngCmp(p, o + 1, this._sc, this._so) > 0;
  }
  cloneRange() { const r = new Range(); r._sc = this._sc; r._so = this._so; r._ec = this._ec; r._eo = this._eo; return r; }
  createContextualFragment(html) {
    if (arguments.length < 1) throw new TypeError("Failed to execute 'createContextualFragment' on 'Range': 1 argument required, but only 0 present.");
    const node = this._sc;
    const ownerDoc = (node && node.ownerDocument) || globalThis.document;
    const frag = ownerDoc.createDocumentFragment();
    // 刀57: plain fragments have no innerHTML setter (Chrome truth) — parse
    // through the op directly.
    _dom("set_inner_html", frag._nid, String(html));
    return frag;
  }
  toString() {
    const sc = this._sc, ec = this._ec;
    if (!sc) return "";
    if (_rngSame(sc, ec) && (sc.nodeType === 3 || sc.nodeType === 4)) return (sc.data || "").slice(this._so, this._eo);
    let s = "";
    if (sc.nodeType === 3 || sc.nodeType === 4) s += (sc.data || "").slice(this._so);
    const cac = this.commonAncestorContainer;
    if (cac) {
      const walk = (node) => {
        if (node.nodeType === 3 || node.nodeType === 4) {
          if (!_rngSame(node, sc) && !_rngSame(node, ec) &&
              _rngCmp(node, 0, this._sc, this._so) >= 0 && _rngCmp(node, _rngNodeLength(node), this._ec, this._eo) <= 0) {
            s += (node.data || "");
          }
        }
        const kids = node.childNodes;
        for (let i = 0; i < kids.length; i++) if (kids[i]) walk(kids[i]);
      };
      walk(cac);
    }
    if (!_rngSame(sc, ec) && (ec.nodeType === 3 || ec.nodeType === 4)) s += (ec.data || "").slice(0, this._eo);
    return s;
  }
  cloneContents() { return (globalThis.document || document).createDocumentFragment(); }
  extractContents() { return (globalThis.document || document).createDocumentFragment(); }
  deleteContents() {}
  insertNode(node) { if (node && this._sc && this._sc.insertBefore) { const kids = this._sc.childNodes; this._sc.insertBefore(node, kids[this._so] || null); } }
  surroundContents(node) { this.insertNode(node); }
  detach() {}
  getBoundingClientRect() { return new DOMRect(); }
  getClientRects() { return []; }
};
// (刀76) the four comparison constants are DATA statics (e--) in Chrome, not static getters
for (const [k, v] of [['START_TO_START', 0], ['START_TO_END', 1], ['END_TO_END', 2], ['END_TO_START', 3]]) Object.defineProperty(globalThis.Range, k, { value: v, enumerable: true });
Object.assign(globalThis.Range.prototype, { START_TO_START: 0, START_TO_END: 1, END_TO_END: 2, END_TO_START: 3 });
globalThis.StaticRange = class StaticRange {
  // (刀87) Chrome 145 dict ladder (k87-ctor-e/f): endContainer is read
  // BEFORE startContainer; Attr (2) joins DocumentType (10) as rejected.
  constructor(init) {
    if (arguments.length < 1) throw new TypeError("Failed to construct 'StaticRange': 1 argument required, but only 0 present.");
    const readNode = (v, name) => {
      if (v === undefined) throw new TypeError("Failed to construct 'StaticRange': Failed to read the '" + name + "' property from 'StaticRangeInit': Required member is undefined.");
      if (!(v instanceof globalThis.Node)) throw new TypeError("Failed to construct 'StaticRange': Failed to read the '" + name + "' property from 'StaticRangeInit': Failed to convert value to 'Node'.");
      return v;
    };
    const ec = readNode(init && init.endContainer, 'endContainer');
    const sc = readNode(init && init.startContainer, 'startContainer');
    if (ec.nodeType === 10 || ec.nodeType === 2 || sc.nodeType === 10 || sc.nodeType === 2)
      throw new DOMException("Failed to construct 'StaticRange': Neither startContainer nor endContainer can be a DocumentType or Attribute node.", 'InvalidNodeTypeError');
    this._sc = sc; this._so = (init && init.startOffset) >>> 0; this._ec = ec; this._eo = (init && init.endOffset) >>> 0;
    __hideOwn(this);
  }
  get startContainer() { return this._sc; }
  get startOffset() { return this._so; }
  get endContainer() { return this._ec; }
  get endOffset() { return this._eo; }
  get collapsed() { return _rngSame(this._sc, this._ec) && this._so === this._eo; }
};
// Live Selection over the real Range: at most one range + a direction, one
// instance per document. Everything except modify() (needs visual line/word
// layout) is layout-free, built on the Range boundary-point helpers above.
// (刀73) brand check for the new accessors: foreign receivers are Illegal invocation.
function _selChk(self) { if (!(self instanceof globalThis.Selection) || !self._doc) throw new TypeError('Illegal invocation'); }
globalThis.Selection = class Selection {
  constructor(doc) { this._doc = doc; this._range = null; this._direction = 'none';  __hideOwn(this);}
  _setRange(r, dir) { this._range = r; this._direction = dir; SEL_RAW.delete(this); }
  // (刀73) shadow-including containment: a node inside an open/closed shadow tree is
  // in the document through its host chain (Chrome lets a selection live there).
  _inDoc(node) {
    if (!node || !this._doc) return false;
    for (let n = node, i = 0; n && i < 256; i++) {
      if (this._doc.contains && this._doc.contains(n)) return true;
      let up = n.parentNode;
      if (!up && n.nodeType === 11 && n.host) up = n.host;
      n = up;
    }
    return false;
  }
  get rangeCount() { return this._range ? 1 : 0; }
  get isCollapsed() { return !this._range || this._range.collapsed; }
  get type() { return !this._range ? 'None' : (this._range.collapsed ? 'Caret' : 'Range'); }
  get _anchor() { const r = this._range; if (!r) return null; return this._direction === 'backwards' ? [r.endContainer, r.endOffset] : [r.startContainer, r.startOffset]; }
  get _focus() { const r = this._range; if (!r) return null; return this._direction === 'backwards' ? [r.startContainer, r.startOffset] : [r.endContainer, r.endOffset]; }
  get anchorNode() { return this._anchor ? this._anchor[0] : null; }
  get anchorOffset() { return this._anchor ? this._anchor[1] : 0; }
  get focusNode() { return this._focus ? this._focus[0] : null; }
  get focusOffset() { return this._focus ? this._focus[1] : 0; }
  // (刀73) Chrome 145 (k73): direction is 'none' (also after addRange / collapse),
  // 'forward' or 'backward'; baseNode/baseOffset/extentNode/extentOffset are the
  // legacy aliases of anchor/focus.
  get direction() { _selChk(this); return this._direction === 'backwards' ? 'backward' : this._direction === 'forwards' ? 'forward' : 'none'; }
  get baseNode() { _selChk(this); return this.anchorNode; }
  get baseOffset() { _selChk(this); return this.anchorOffset; }
  get extentNode() { _selChk(this); return this.focusNode; }
  get extentOffset() { _selChk(this); return this.focusOffset; }
  // getComposedRanges({shadowRoots}) -> StaticRange[]: the range with endpoints inside a
  // shadow tree the caller did not name moved out around its host (start before the host,
  // end after it); undefined/null/arrays/ShadowRoots as the argument are accepted and
  // grant nothing (Chrome 145 treats a non-dictionary object as an empty options dict).
  getComposedRanges(options) {
    _selChk(this);
    let roots = [];
    if (options !== undefined && options !== null) {
      if (typeof options !== 'object' && typeof options !== 'function') throw new TypeError("Failed to execute 'getComposedRanges' on 'Selection': The provided value is not of type 'GetComposedRangesOptions'.");
      const sr = options.shadowRoots;
      if (sr !== undefined) {
        const bad = (m) => new TypeError("Failed to execute 'getComposedRanges' on 'Selection': Failed to read the 'shadowRoots' property from 'GetComposedRangesOptions': " + m);
        if (sr === null || typeof sr !== 'object' || typeof sr[Symbol.iterator] !== 'function') throw bad('The provided value cannot be converted to a sequence.');
        roots = Array.from(sr);
        for (const r of roots) if (!(r instanceof ShadowRoot)) throw bad("Failed to convert value to 'ShadowRoot'.");
      }
    }
    const rg = this._range;
    if (!rg) return [];
    const lift = (node, offset, end) => {
      for (let i = 0; i < 64; i++) {
        const root = node && typeof node.getRootNode === 'function' ? node.getRootNode() : null;
        if (!root || root.nodeType !== 11 || !root.host || roots.indexOf(root) >= 0) break;
        const host = root.host, par = host.parentNode;
        if (!par) break;
        node = par; offset = Array.prototype.indexOf.call(par.childNodes, host) + (end ? 1 : 0);
      }
      return [node, offset];
    };
    const a = lift(rg.startContainer, rg.startOffset, false), b = lift(rg.endContainer, rg.endOffset, true);
    return [new StaticRange({ startContainer: a[0], startOffset: a[1], endContainer: b[0], endOffset: b[1] })];
  }
  getRangeAt(i) { i = +i; if (!this._range || i < 0 || i > 0) throw new DOMException('The index provided is out of range.', 'IndexSizeError'); return this._range; }
  addRange(range) { if (this._range) return; if (!(range instanceof Range)) return; if (!this._inDoc(range.startContainer) || !this._inDoc(range.endContainer)) return; this._setRange(range, 'none'); }
  removeRange(range) { if (!(range instanceof Range)) throw new TypeError("Failed to execute 'removeRange' on 'Selection': parameter 1 is not a Range."); if (this._range === range) this._setRange(null, 'none'); else throw new DOMException('The range was not found.', 'NotFoundError'); }
  removeAllRanges() { this._setRange(null, 'none'); }
  empty() { this.removeAllRanges(); }
  collapse(node, offset) { if (node == null) { this.removeAllRanges(); return; } offset = offset >>> 0; _rngCheckOffset(node, offset); if (!this._inDoc(node)) return; const r = new Range(); r.setStart(node, offset); r.setEnd(node, offset); this._setRange(r, 'none'); }
  setPosition(node, offset) { this.collapse(node, offset); }
  collapseToStart() { if (!this._range) throw new DOMException('There is no selection to collapse.', 'InvalidStateError'); const r = new Range(); r.setStart(this._range.startContainer, this._range.startOffset); r.setEnd(this._range.startContainer, this._range.startOffset); this._setRange(r, 'none'); }
  collapseToEnd() { if (!this._range) throw new DOMException('There is no selection to collapse.', 'InvalidStateError'); const r = new Range(); r.setStart(this._range.endContainer, this._range.endOffset); r.setEnd(this._range.endContainer, this._range.endOffset); this._setRange(r, 'none'); }
  extend(node, offset) { if (!this._range) throw new DOMException('There is no selection to extend.', 'InvalidStateError'); if (!this._inDoc(node)) return; offset = offset >>> 0; _rngCheckOffset(node, offset); const a = this._anchor; const r = new Range(); if (_rngRoot(node)._nid !== _rngRoot(a[0])._nid) { r.setStart(node, offset); r.setEnd(node, offset); this._setRange(r, 'forwards'); return; } if (_rngCmp(a[0], a[1], node, offset) <= 0) { r.setStart(a[0], a[1]); r.setEnd(node, offset); this._setRange(r, 'forwards'); } else { r.setStart(node, offset); r.setEnd(a[0], a[1]); this._setRange(r, 'backwards'); } }
  setBaseAndExtent(aN, aO, fN, fO) { if (arguments.length < 4) throw new TypeError("Failed to execute 'setBaseAndExtent' on 'Selection': 4 arguments required."); if (aN == null || fN == null) throw new TypeError("Failed to execute 'setBaseAndExtent' on 'Selection': nodes must not be null."); aO = +aO; fO = +fO; if (aO < 0 || aO > _rngNodeLength(aN)) throw new DOMException('anchor offset out of range', 'IndexSizeError'); if (fO < 0 || fO > _rngNodeLength(fN)) throw new DOMException('focus offset out of range', 'IndexSizeError'); if (!this._inDoc(aN) || !this._inDoc(fN)) { this.removeAllRanges(); return; } const r = new Range(); if (_rngCmp(aN, aO, fN, fO) <= 0) { r.setStart(aN, aO); r.setEnd(fN, fO); this._setRange(r, 'forwards'); } else { r.setStart(fN, fO); r.setEnd(aN, aO); this._setRange(r, 'backwards'); } if (_rngRoot(aN)._nid !== _rngRoot(fN)._nid) SEL_RAW.set(this, [aN, aO, fN, fO]); }
  selectAllChildren(node) { if (node && node.nodeType === 10) throw new DOMException('cannot selectAllChildren of a DocumentType', 'InvalidNodeTypeError'); if (!this._inDoc(node)) return; const len = _rngNodeLength(node); const r = new Range(); r.setStart(node, 0); r.setEnd(node, len); this._setRange(r, 'forwards'); }
  containsNode(node, allowPartial) { const r = this._range; if (!r || !node) return false; if (_rngRoot(node)._nid !== _rngRoot(r.startContainer)._nid) return false; const len = _rngNodeLength(node); if (allowPartial) return _rngCmp(node, len, r.startContainer, r.startOffset) > 0 && _rngCmp(node, 0, r.endContainer, r.endOffset) < 0; return _rngCmp(node, 0, r.startContainer, r.startOffset) >= 0 && _rngCmp(node, len, r.endContainer, r.endOffset) <= 0; }
  deleteFromDocument() { if (this._range) this._range.deleteContents(); }
  toString() { if (!this._range) return ''; return _selPlainText(_selCrossTreeRange(this) || this._range); }
  modify() {}
};
_markNative(globalThis.Selection);
// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('_rngCmp', _rngCmp);
  d('_rngNodeIndex', _rngNodeIndex);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
