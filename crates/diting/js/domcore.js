(function () { // @@SCOPE
// god-file split: bootstrap's real native marker, reached through its
// hidden prop (house style — see ctor_ladders.js) so no cross-file load
// edge appears in the dependency ratchet.
const _markNative = globalThis.__diting_markNative;

class Node {
  static ELEMENT_NODE = 1;
  static ATTRIBUTE_NODE = 2;
  static TEXT_NODE = 3;
  static CDATA_SECTION_NODE = 4;
  static ENTITY_REFERENCE_NODE = 5;
  static ENTITY_NODE = 6;
  static PROCESSING_INSTRUCTION_NODE = 7;
  static COMMENT_NODE = 8;
  static DOCUMENT_NODE = 9;
  static DOCUMENT_TYPE_NODE = 10;
  static DOCUMENT_FRAGMENT_NODE = 11;
  static NOTATION_NODE = 12;
  static DOCUMENT_POSITION_DISCONNECTED = 1;
  static DOCUMENT_POSITION_PRECEDING = 2;
  static DOCUMENT_POSITION_FOLLOWING = 4;
  static DOCUMENT_POSITION_CONTAINS = 8;
  static DOCUMENT_POSITION_CONTAINED_BY = 16;
  static DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC = 32;

  constructor(nid) { this._nid = nid;  __hideOwn(this);}
  get nodeType() { return +_dom("node_type", this._nid); }
  get nodeName() { return _domParse("node_name", this._nid) || ""; }
  get ownerDocument() { return globalThis.document; }
  // https://dom.spec.whatwg.org/#dom-node-baseuri
  get baseURI() {
    try {
      const doc = globalThis.document;
      const docUrl = (doc && doc.URL) || "";
      const baseEl = (doc && doc.querySelector) ? doc.querySelector("base[href]") : null;
      if (baseEl) {
        const href = baseEl.getAttribute("href");
        if (href) {
          return docUrl ? new URL(href, docUrl).href : href;
        }
      }
      return docUrl;
    } catch (e) {
      return "";
    }
  }
  get textContent() { return _domParse("text_content", this._nid) ?? ""; }
  set textContent(v) {
    const oldChildren = _domParse("child_nodes", this._nid) || [];
    for (const c of oldChildren) _dom("remove_child", c);
    let added = [];
    if (v != null && v !== "") {
      const tn = +_dom("create_text_node", String(v));
      _dom("append_child", this._nid, tn);
      added = [tn];
    }
    // Real MutationObserver fires childList for the children swap.
    // Without this React 18+ hydration mismatch detection and many polling
    // libs (intersection-driven lazy load, content sync) silently stall.
    if (globalThis.__mutationObservers?.length) {
      globalThis.__notifyMutation('childList', this._nid, added, oldChildren);
    }
  }
  get nodeValue() {
    const t = this.nodeType;
    if (t === 3 || t === 8) return _domParse("text_content", this._nid) ?? "";
    return null;
  }
  set nodeValue(v) {
    const t = this.nodeType;
    if (t === 3 || t === 8) _dom("set_text_content", this._nid, String(v ?? ""));
  }
  get parentNode() {
    // #225: memoized edge — see _TREE_EDGE_MUTATION_COMMANDS. Form walks
    // (getFormItemNodeByElement up-chains) hit this millions of times per
    // boot; the memo turns each hit past the first into a Map lookup.
    let p = _edgeMemo.get(this._nid);
    if (p === undefined) {
      p = +_dom("parent_node", this._nid);
      _edgeMemo.set(this._nid, p);
    }
    return p >= 0 ? _wrap(p) : null;
  }
  get parentElement() { const p = this.parentNode; return p && p.nodeType === 1 ? p : null; }
  get childNodes() {
    const ids = _domParse("child_nodes", this._nid) || [];
    return _nodeList(ids.map(_wrap).filter(Boolean));
  }
  get firstChild() { return _wrap(+_dom("first_child", this._nid)); }
  get lastChild() { return _wrap(+_dom("last_child", this._nid)); }
  get nextSibling() { return _wrap(+_dom("next_sibling", this._nid)); }
  get previousSibling() { return _wrap(+_dom("prev_sibling", this._nid)); }
  // The trailing boolean is an internal flag: Element.append/prepend/
  // replaceChildren batch their per-node inserts and emit ONE combined
  // childList record (Chrome truth: append(i, b) is a single record with
  // both nodes in addedNodes), so the per-call notify is suppressed there.
  appendChild(c, _batchInsert) {
    if (!c) return c;
    // Per DOM spec, inserting a DocumentFragment inserts its CHILDREN — the
    // fragment itself never enters the tree (a `#document-fragment` visible
    // in body.childNodes is an instant bot tell).
    // A ShadowRoot has nodeName '#document-fragment' too but must NOT be
    // unwrapped — splicing its children out would destructively move the
    // whole shadow tree into the light parent. The guard falls it through
    // to the plain append path, which the tree side rejects.
    if (c.nodeType === 11 && c.nodeName === '#document-fragment' && !(c instanceof globalThis.ShadowRoot)) {
      const kids = Array.from(c.childNodes);
      for (const k of kids) this.appendChild(k);
      if (globalThis.__mutationObservers?.length) {
        const k0 = kids[0], k1 = kids[kids.length - 1];
        globalThis.__notifyMutation('childList', this._nid, kids.map(k => k._nid), [],
          null, null, k0.previousSibling ? k0.previousSibling._nid : null, k1.nextSibling ? k1.nextSibling._nid : null);
      }
      if (__inDocShadowIncl(this)) for (const k of kids) _registerIframesIn(k);
      // (#48) fragment children entering the document become window-named.
      if (_nodeInDocument(this)) for (const k of kids) _namedScanIn(k);
      return c;
    }
    // A Document takes at most one element child (DOM standard's document
    // child constraints; Chrome message pinned by the #272 Chrome probe).
    if (this.nodeType === 9 && c.nodeType === 1) {
      for (const k of this.childNodes) {
        if (k !== c && k.nodeType === 1) {
          throw new DOMException("Failed to execute 'appendChild' on 'Node': Only one element on document allowed.", "HierarchyRequestError");
        }
      }
    }
    _dom("append_child", this._nid, c._nid);
    if (!_batchInsert && globalThis.__mutationObservers?.length) {
      globalThis.__notifyMutation('childList', this._nid, [c._nid], [],
        null, null, c.previousSibling ? c.previousSibling._nid : null, c.nextSibling ? c.nextSibling._nid : null);
    }
    // Insertion into the document instantiates browsing contexts for any
    // iframes in the inserted subtree (window[N] / window.length).
    if (__inDocShadowIncl(this)) _registerIframesIn(c);
    _namedScanIn(c);
    __prepareInsertedSubtree(c);
    return c;
  }
  removeChild(c, _batchRemove) {
    if (!c) return c;
    // The removed node's old neighbours ride the record (DOM standard: the
    // remove steps capture them BEFORE the node leaves the tree).
    const prevNid = c.previousSibling ? c.previousSibling._nid : null;
    const nextNid = c.nextSibling ? c.nextSibling._nid : null;
    _dom("remove_child", c._nid);
    __disconnectCustomElementsIn(c);
    // (#48) the removed subtree loses its window-named properties.
    _namedUnscanIn(c);
    // A tracked animation on a removed element must fire animationcancel.
    _cancelAnimationsInSubtree(c);
    _cancelTransitionsInSubtree(c);
    if (!_batchRemove && globalThis.__mutationObservers?.length) {
      globalThis.__notifyMutation('childList', this._nid, [], [c._nid], null, null, prevNid, nextNid);
    }
    return c;
  }
  replaceChild(newChild, oldChild) {
    if (!oldChild || !newChild) return oldChild;
    // A DocumentFragment is replaced by its children, in order (DOM spec).
    // A ShadowRoot (same nodeType/nodeName) must not be unwrapped — see
    // appendChild; the plain path below is rejected by the tree.
    if (newChild.nodeType === 11 && newChild.nodeName === '#document-fragment' && !(newChild instanceof globalThis.ShadowRoot)) {
      const kids = Array.from(newChild.childNodes);
      for (const k of kids) this.insertBefore(k, oldChild);
      this.removeChild(oldChild);
      return oldChild;
    }
    _dom("insert_before", newChild._nid, oldChild._nid);
    _dom("remove_child", oldChild._nid);
    __disconnectCustomElementsIn(oldChild);
    _namedUnscanIn(oldChild);
    // A replacement is a single combined record in Chrome (added+removed in
    // one childList entry, siblings from the replacement point), not the
    // spec's remove+insert pair — #272 Chrome probe.
    if (globalThis.__mutationObservers?.length) {
      globalThis.__notifyMutation('childList', this._nid, [newChild._nid], [oldChild._nid],
        null, null, newChild.previousSibling ? newChild.previousSibling._nid : null, newChild.nextSibling ? newChild.nextSibling._nid : null);
    }
    if (__inDocShadowIncl(this)) _registerIframesIn(newChild);
    _namedScanIn(newChild);
    __prepareInsertedSubtree(newChild);
    return oldChild;
  }
  insertBefore(n, ref, _batchInsert) {
    if (!n) return n;
    if (!ref) { this.appendChild(n, _batchInsert); return n; }
    if (n.nodeType === 11 && n.nodeName === '#document-fragment' && !(n instanceof globalThis.ShadowRoot)) {
      const kids = Array.from(n.childNodes);
      for (const k of kids) this.insertBefore(k, ref);
      return n;
    }
    // Same document-child constraint as appendChild (one element child).
    if (this.nodeType === 9 && n.nodeType === 1) {
      for (const k of this.childNodes) {
        if (k !== n && k.nodeType === 1) {
          throw new DOMException("Failed to execute 'insertBefore' on 'Node': Only one element on document allowed.", "HierarchyRequestError");
        }
      }
    }
    _dom("insert_before", n._nid, ref._nid);
    // Same reporting as appendChild: where a node is inserted does not decide
    // whether an observer sees it.
    if (!_batchInsert && globalThis.__mutationObservers?.length) {
      globalThis.__notifyMutation('childList', this._nid, [n._nid], [],
        null, null, n.previousSibling ? n.previousSibling._nid : null, n.nextSibling ? n.nextSibling._nid : null);
    }
    if (__inDocShadowIncl(this)) _registerIframesIn(n);
    _namedScanIn(n);
    __prepareInsertedSubtree(n);
    return n;
  }
  // Self-containment short-circuits to true (spec: "a node contains
  // itself") — the Rust op's descendants walk excludes the node itself.
  contains(o) {
    if (o === this) return true;
    return o ? _dom("contains", this._nid, o._nid) === "true" : false;
  }
  hasChildNodes() { return _dom("has_child_nodes", this._nid) === "true"; }
  cloneNode(deep) {
    const t = this.nodeType;
    if (t === 1) {
      const c = _wrap(+_dom("clone_node", this._nid, deep ? "true" : "false"));
      // Cloned custom elements are born upgraded (constructor + attribute
      // reactions; detached, so no connectedCallback).
      if (c) { globalThis.__diting_cloneAttrNs?.(this, c); __upgradeCustomElementsIn(c, true); }
      return c;
    }
    // Clone structurally via real DOM nodes rather than round-tripping through a
    // throwaway <div>.innerHTML: the fragment parser discards elements that are
    // not valid children of <div> (<tr>, <td>, <option>, …), so the old path
    // returned null for them and lost JS-set inline styles. Building each node
    // directly with createElement(NS) + attribute copy avoids any parsing
    // context, and an explicit stack keeps a deep subtree from overflowing the
    // JS stack.
    const root = _shallowCloneNode(this);
    if (!deep || !root) return root;
    const stack = [[this, root]];
    while (stack.length) {
      const [src, dst] = stack.pop();
      // A <template>'s children hang off its content fragment, not childNodes,
      // so clone them into the clone's fragment. Gated on the tag name because
      // .content means something else on other elements (e.g. <meta>).
      if (src.localName === 'template' && dst.localName === 'template') {
        const sc = src.content, dc = dst.content;
        if (sc && dc && sc.childNodes) {
          const tk = sc.childNodes;
          for (let i = 0; i < tk.length; i++) {
            const c = _shallowCloneNode(tk[i]);
            if (c) { dc.appendChild(c); stack.push([tk[i], c]); }
          }
        }
      }
      const kids = src.childNodes;
      for (let i = 0; i < kids.length; i++) {
        const c = _shallowCloneNode(kids[i]);
        if (c) { dst.appendChild(c); stack.push([kids[i], c]); }
      }
    }
    return root;
  }
  compareDocumentPosition(other) {
    if (!other) return 0;
    if (this._nid === other._nid) return 0;
    // Different roots: DISCONNECTED | IMPLEMENTATION_SPECIFIC plus a stable
    // (consistent across calls) PRECEDING/FOLLOWING bit, chosen by node-id order.
    if (+_dom("node_root", this._nid) !== +_dom("node_root", other._nid)) {
      return 1 | 32 | ((this._nid < other._nid) ? 4 : 2);
    }
    if (this.contains(other)) return 16 | 4;          // CONTAINED_BY | FOLLOWING
    if (other.contains && other.contains(this)) return 8 | 2; // CONTAINS | PRECEDING
    // Same root, neither contains the other: real tree order (compare_order op:
    // -1 => this precedes other => other FOLLOWS this(4); +1 => this PRECEDING(2)).
    return (+_dom("compare_order", this._nid, other._nid) < 0) ? 4 : 2;
  }
  getRootNode(opts) {
    const composed = !!(opts && opts.composed);
    const nid = +(composed ? _dom("shadow_including_root", this._nid) : _dom("tree_scope_root", this._nid));
    return (nid >= 0 ? _wrap(nid) : null) || this;
  }
  normalize() {
    // Merge adjacent exclusive Text nodes, drop empty ones, recurse. Detached
    // removed nodes keep their own data (read from the backing node by nid).
    let child = this.firstChild;
    while (child) {
      const next = child.nextSibling;
      if (child.nodeType === 3) {
        let data = child.data, sib = child.nextSibling;
        while (sib && sib.nodeType === 3) { const after = sib.nextSibling; data += sib.data; this.removeChild(sib); sib = after; }
        if (data.length === 0) { this.removeChild(child); child = sib; continue; }
        if (data !== child.data) child.data = data;
        child = sib; continue;
      } else if (child.nodeType === 1 || child.nodeType === 11) {
        child.normalize();
      }
      child = next;
    }
  }
  isEqualNode(other) {
    if (!other) return false;
    if (this._nid === other._nid) return true;
    if (this.nodeType !== other.nodeType) return false;
    if (this.nodeName !== other.nodeName) return false;
    if (this.nodeValue !== other.nodeValue) return false;
    const a = this.attributes ? this.attributes : null;
    const b = other.attributes ? other.attributes : null;
    if ((a && a.length) || (b && b.length)) {
      if (!a || !b || a.length !== b.length) return false;
      for (let i = 0; i < a.length; i++) {
        if (other.getAttribute(a[i].name) !== a[i].value) return false;
      }
    }
    const cA = this.childNodes || [];
    const cB = other.childNodes || [];
    if (cA.length !== cB.length) return false;
    for (let i = 0; i < cA.length; i++) {
      if (!cA[i].isEqualNode(cB[i])) return false;
    }
    return true;
  }
  isSameNode(other) { return !!other && this._nid === other._nid; }
  addEventListener() {} removeEventListener() {} dispatchEvent() { return true; }
}
class CharacterData extends Node {
  get data() {
    return _domParse("text_content", this._nid) ?? "";
  }
  set data(v) {
    const oldValue = _domParse("text_content", this._nid) ?? "";
    _dom("set_text_content", this._nid, String(v ?? ""));
    if (globalThis.__mutationObservers?.length) {
      globalThis.__notifyMutation('characterData', this._nid, [], [], null, oldValue);
    }
  }
  get length() { return this.data.length; }
  substringData(offset, count) {
    return this.data.substring(offset, offset + count);
  }
  appendData(s) { this.data += s; }
  insertData(offset, s) {
    const d = this.data;
    this.data = d.slice(0, offset) + s + d.slice(offset);
  }
  deleteData(offset, count) {
    const d = this.data;
    this.data = d.slice(0, offset) + d.slice(offset + count);
  }
  replaceData(offset, count, s) {
    const d = this.data;
    this.data = d.slice(0, offset) + s + d.slice(offset + count);
  }
}

class Text extends CharacterData {
  get nodeName() { return "#text"; }
  get nodeType() { return 3; }
  get wholeText() { return this.data; }
  splitText(offset) {
    const d = this.data;
    const tail = d.substring(offset);
    this.data = d.substring(0, offset);
    const newNid = +_dom("create_text_node", tail);
    const parent = this.parentNode;
    if (parent) {
      const ref = this.nextSibling;
      parent.insertBefore(_wrap(newNid), ref);
    }
    return _wrap(newNid);
  }
  cloneNode() { return document.createTextNode(this.data); }
}

class Comment extends CharacterData {
  get nodeName() { return "#comment"; }
  get nodeType() { return 8; }
  cloneNode() { return document.createComment(this.data); }
}

// Supported-token sets per HTML spec. Without them supports() must throw —
// classList/sizes have no supported tokens — but relList on <link> and
// iframe.sandbox DO, and resource loaders feature-probe via
// relList.supports("preload"); throwing there aborts their load chains.
// a/area relList stays unsupported (supports() throws), matching Chrome.
const LINK_REL_SUPPORTED = ["alternate","author","bookmark","canonical","dns-prefetch","external","help","icon","license","manifest","me","modulepreload","next","nofollow","noopener","noreferrer","opener","pingback","preconnect","prefetch","preload","prev","privacy-policy","search","stylesheet","tag","terms-of-service"];
// Fetch-standard RequestInit referrerPolicy tokens; '' = spec default
// (strict-origin-when-cross-origin).
const REFERRER_POLICIES = ["", "no-referrer", "no-referrer-when-downgrade", "origin", "origin-when-cross-origin", "strict-origin", "strict-origin-when-cross-origin", "unsafe-url"];
const SANDBOX_SUPPORTED = ["allow-downloads","allow-forms","allow-modals","allow-orientation-lock","allow-pointer-lock","allow-popups","allow-popups-to-escape-sandbox","allow-presentation","allow-same-origin","allow-scripts","allow-storage-access-by-user-activation","allow-top-navigation","allow-top-navigation-by-user-activation","allow-top-navigation-to-custom-protocols"];

// DOMTokenList backs class/rel/sandbox/etc. attribute reflection. It parses the
// associated content attribute as an ordered set of tokens and writes changes
// straight back, so reads and writes stay live with the element. A Proxy is
// layered on top so numeric indexing (list[0]) hits item().
class DOMTokenList {
  constructor(el, attr, supportedTokens) {
    // Non-enumerable so the element <-> token-list cycle is not visible to
    // enumeration/serialization (JSON.stringify(classList) would otherwise
    // throw "circular structure").
    Object.defineProperty(this, "_el", { value: el, writable: true, enumerable: false });
    Object.defineProperty(this, "_attr", { value: attr, writable: true, enumerable: false });
    Object.defineProperty(this, "_supported", { value: supportedTokens || null, writable: true, enumerable: false });
    return new Proxy(this, {
      get(t, k, r) {
        if (typeof k === "string" && /^\d+$/.test(k)) return t.item(+k);
        return Reflect.get(t, k, r);
      },
      has(t, k) {
        if (typeof k === "string" && /^\d+$/.test(k)) return +k < t.length;
        return Reflect.has(t, k);
      },
    });
  }
  get [Symbol.toStringTag]() { return "DOMTokenList"; }
  _tokens() {
    const v = this._el.getAttribute(this._attr);
    if (!v) return [];
    const seen = new Set();
    const out = [];
    for (const tok of v.split(/[ \t\n\f\r]+/)) {
      if (tok && !seen.has(tok)) { seen.add(tok); out.push(tok); }
    }
    return out;
  }
  _write(tokens) {
    this._el.setAttribute(this._attr, tokens.join(" "));
  }
  get length() { return this._tokens().length; }
  get value() { return this._el.getAttribute(this._attr) || ""; }
  set value(v) { this._el.setAttribute(this._attr, String(v)); }
  item(i) { const t = this._tokens(); return (i >= 0 && i < t.length) ? t[i] : null; }
  contains(token) { return this._tokens().includes(String(token)); }
  add(...tokens) {
    const t = this._tokens();
    for (const raw of tokens) {
      const tok = String(raw);
      if (tok === "") throw new DOMException("The token provided must not be empty.", "SyntaxError");
      if (/[ \t\n\f\r]/.test(tok)) throw new DOMException("The token provided contains HTML space characters, which are not valid in tokens.", "InvalidCharacterError");
      if (!t.includes(tok)) t.push(tok);
    }
    this._write(t);
  }
  remove(...tokens) {
    let t = this._tokens();
    for (const raw of tokens) {
      const tok = String(raw);
      if (tok === "") throw new DOMException("The token provided must not be empty.", "SyntaxError");
      if (/[ \t\n\f\r]/.test(tok)) throw new DOMException("The token provided contains HTML space characters, which are not valid in tokens.", "InvalidCharacterError");
      t = t.filter((x) => x !== tok);
    }
    this._write(t);
  }
  toggle(token, force) {
    const tok = String(token);
    if (tok === "") throw new DOMException("The token provided must not be empty.", "SyntaxError");
    if (/[ \t\n\f\r]/.test(tok)) throw new DOMException("The token provided contains HTML space characters, which are not valid in tokens.", "InvalidCharacterError");
    const t = this._tokens();
    const has = t.includes(tok);
    if (has) {
      if (force === true) return true;
      this._write(t.filter((x) => x !== tok));
      return false;
    }
    if (force === false) return false;
    t.push(tok);
    this._write(t);
    return true;
  }
  replace(token, newToken) {
    const a = String(token), b = String(newToken);
    if (a === "" || b === "") throw new DOMException("The token provided must not be empty.", "SyntaxError");
    if (/[ \t\n\f\r]/.test(a) || /[ \t\n\f\r]/.test(b)) throw new DOMException("The token provided contains HTML space characters, which are not valid in tokens.", "InvalidCharacterError");
    const t = this._tokens();
    const i = t.indexOf(a);
    if (i === -1) return false;
    if (t.includes(b) && b !== a) { t.splice(i, 1); } else { t[i] = b; }
    this._write(t);
    return true;
  }
  supports(token) {
    if (!this._supported) throw new TypeError("DOMTokenList has no supported tokens.");
    return this._supported.includes(String(token).toLowerCase());
  }
  forEach(cb, thisArg) {
    // (刀95) Chrome throws on a missing callback BEFORE iterating (even an
    // empty list) — the natural V8 call shape, "undefined is not a function",
    // not a WebIDL coercion message.
    if (typeof cb !== "function") throw new TypeError(String(cb) + " is not a function");
    const t = this._tokens();
    for (let i = 0; i < t.length; i++) cb.call(thisArg, t[i], i, this);
  }
  *values() { yield* this._tokens(); }
  *keys() { const t = this._tokens(); for (let i = 0; i < t.length; i++) yield i; }
  *entries() { const t = this._tokens(); for (let i = 0; i < t.length; i++) yield [i, t[i]]; }
  [Symbol.iterator]() { return this._tokens()[Symbol.iterator](); }
  toString() { return this.value; }
}

// CDATASection: a Text-derived node (nodeType 4) used only in XML documents.
// Extends Text so data/length/textContent/childNodes reuse the working text
// node machinery; only the type-identifying getters differ.
class CDATASection extends Text {
  get nodeName() { return "#cdata-section"; }
  get nodeType() { return 4; }
  get nodeValue() { return this.data; }
  set nodeValue(v) { this.data = v; }
  cloneNode() { return new CDATASection(+_dom("create_text_node", this.data)); }
}

// ProcessingInstruction: nodeType 7, nodeName === target. Extends CharacterData
// and carries a separate target. Backed by a text node so data/nodeValue/
// textContent/length work without native PI support.
class ProcessingInstruction extends CharacterData {
  constructor(nid, target) { super(nid); this._target = target;  __hideOwn(this);}
  get target() { return this._target; }
  get nodeName() { return this._target; }
  get nodeType() { return 7; }
  get nodeValue() { return this.data; }
  set nodeValue(v) { this.data = v; }
  cloneNode() { return new ProcessingInstruction(+_dom("create_text_node", this.data), this._target); }
}

// Document character encoding (WHATWG canonical name, e.g. "UTF-8", "EUC-JP").
// Cached per runtime: the encoding is fixed for a document's lifetime and this
// is read on every <a>/<area> URL-component access, so the UTF-8 common case
// must reduce to a single cached-boolean read with no op call and no allocation.
let __docEncoding;
let __docIsUtf8;
function _docEncoding() {
  if (__docEncoding === undefined) {
    const e = _domParse("document_encoding");
    __docEncoding = (typeof e === 'string' && e) ? e : 'UTF-8';
    __docIsUtf8 = __docEncoding.toLowerCase() === 'utf-8';
  }
  return __docEncoding;
}
function _docIsUtf8() { if (__docIsUtf8 === undefined) _docEncoding(); return __docIsUtf8; }
// Main response's MIME type (lowercased, no parameters), e.g. "text/plain".
// Empty when the response carried no Content-Type — callers then fall back
// to URL sniffing. Cached per runtime like the encoding above.
let __docContentType;
function _docContentType() {
  if (__docContentType === undefined) {
    const ct = _domParse("document_content_type");
    __docContentType = (typeof ct === 'string') ? ct : '';
  }
  return __docContentType;
}
// WHATWG "special scheme" check (these get the special-query percent-encode set).
function _isSpecialScheme(protocol) {
  const s = (protocol || '').replace(/:$/, '').toLowerCase();
  return s === 'http' || s === 'https' || s === 'ws' || s === 'wss' || s === 'ftp' || s === 'file';
}
// Apply the WHATWG URL "encoding override": in a legacy (non-UTF-8) document
// the query of an <a>/<area> href is percent-encoded in the document charset,
// not UTF-8. The url op already produced a UTF-8-encoded query; recover the
// original characters (percent-decode + UTF-8) and re-encode them through the
// document charset. Pure-ASCII queries round-trip unchanged.
function _applyDocQueryEncoding(u) {
  if (!u || !u.search || u.search.length < 2) return u;
  let decoded;
  try { decoded = decodeURIComponent(u.search.slice(1)); } catch (e) { return u; }
  let reencoded;
  try { reencoded = _OPS.op_url_encode_query(decoded, _docEncoding(), _isSpecialScheme(u.protocol)); }
  catch (e) { return u; }
  const newSearch = '?' + reencoded;
  if (newSearch === u.search) return u;
  const hashIdx = u.href.indexOf('#');
  const frag = hashIdx >= 0 ? u.href.slice(hashIdx) : '';
  const beforeHash = hashIdx >= 0 ? u.href.slice(0, hashIdx) : u.href;
  const qIdx = beforeHash.indexOf('?');
  u.href = (qIdx >= 0 ? beforeHash.slice(0, qIdx) : beforeHash) + newSearch + frag;
  u.search = newSearch;
  return u;
}

// HTMLHyperlinkElementUtils helpers (the <a>/<area> URL-decomposition members).
// The element's href attribute is parsed against the document base URL via the
// WHATWG url op; component getters read it, setters rewrite the href attribute.
// Base = document URL with <base href> folded in (upstream #658); identity and
// origin checks keep using the plain document URL.
function _docBase() { return _domParse("document_base_url") || _domParse("document_url") || "about:blank"; }
function _anchorBase() { return _docBase(); }
function _elemHrefURL(el) {
  const raw = el.getAttribute('href');
  if (raw === null || raw === undefined) return null;
  const u = _urlParseOp(raw, _anchorBase());
  if (u && !_docIsUtf8()) return _applyDocQueryEncoding(u);
  return u;
}
function _setElemHrefPart(el, part, value) {
  const u = _elemHrefURL(el);
  if (!u) return;
  const c = _urlSetOp(u.href, part, value);
  if (c) el.setAttribute('href', c.href);
}

// --- <input> number/date conversion (valueAsNumber/valueAsDate/stepUp/Down) ---
// Applicable types and their step scale factor + default step (HTML spec).
const _INPUT_NUM_TYPES = { date: 1, month: 1, week: 1, time: 1, 'datetime-local': 1, number: 1, range: 1 };
const _INPUT_DATE_TYPES = { date: 1, month: 1, week: 1, time: 1, 'datetime-local': 1 };
const _INPUT_STEP_SCALE = { date: 86400000, 'datetime-local': 1000, month: 1, number: 1, range: 1, time: 1000, week: 604800000 };
const _INPUT_STEP_DEFAULT = { date: 1, 'datetime-local': 60, month: 1, number: 1, range: 1, time: 60, week: 1 };
function _pad(n, w) { n = String(Math.abs(n | 0)); while (n.length < w) n = '0' + n; return n; }
function _daysInMonth(y, m) { return [31, ((y % 4 === 0 && y % 100 !== 0) || y % 400 === 0) ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][m - 1]; }
function _isoWeek1Monday(y) { const jan4 = Date.UTC(y, 0, 4); const dow = (new Date(jan4).getUTCDay() + 6) % 7; return jan4 - dow * 86400000; }
// Parse an <input> value string to its numeric form per type; NaN if invalid.
function _inputParseNumber(type, v) {
  v = String(v == null ? '' : v);
  let m;
  switch (type) {
    case 'number': case 'range': { if (v === '') return NaN; const n = Number(v); return isFinite(n) ? n : NaN; }
    case 'date': if ((m = /^(\d{4,})-(\d{2})-(\d{2})$/.exec(v))) { const y = +m[1], mo = +m[2], d = +m[3]; if (mo >= 1 && mo <= 12 && d >= 1 && d <= _daysInMonth(y, mo)) return Date.UTC(y, mo - 1, d); } return NaN;
    case 'month': if ((m = /^(\d{4,})-(\d{2})$/.exec(v))) { const y = +m[1], mo = +m[2]; if (mo >= 1 && mo <= 12) return (y - 1970) * 12 + (mo - 1); } return NaN;
    case 'week': if ((m = /^(\d{4,})-W(\d{2})$/.exec(v))) { const y = +m[1], w = +m[2]; if (w >= 1 && w <= 53) return _isoWeek1Monday(y) + (w - 1) * 604800000; } return NaN;
    case 'time': if ((m = /^(\d{2}):(\d{2})(?::(\d{2})(?:\.(\d{1,3}))?)?$/.exec(v))) { const h = +m[1], mi = +m[2], s = m[3] ? +m[3] : 0, ms = m[4] ? +((m[4] + '00').slice(0, 3)) : 0; if (h <= 23 && mi <= 59 && s <= 59) return ((h * 60 + mi) * 60 + s) * 1000 + ms; } return NaN;
    case 'datetime-local': if ((m = /^(\d{4,})-(\d{2})-(\d{2})T(\d{2}):(\d{2})(?::(\d{2})(?:\.(\d{1,3}))?)?$/.exec(v))) { const y = +m[1], mo = +m[2], d = +m[3], h = +m[4], mi = +m[5], s = m[6] ? +m[6] : 0, ms = m[7] ? +((m[7] + '00').slice(0, 3)) : 0; if (mo >= 1 && mo <= 12 && d >= 1 && d <= _daysInMonth(y, mo) && h <= 23 && mi <= 59 && s <= 59) return Date.UTC(y, mo - 1, d, h, mi, s, ms); } return NaN;
  }
  return NaN;
}
// Format a numeric value back to an <input> value string per type.
function _inputFormatNumber(type, n) {
  switch (type) {
    case 'number': case 'range': return String(n);
    case 'date': { const dt = new Date(n); return _pad(dt.getUTCFullYear(), 4) + '-' + _pad(dt.getUTCMonth() + 1, 2) + '-' + _pad(dt.getUTCDate(), 2); }
    case 'month': { const y = 1970 + Math.floor(n / 12); const mo = ((n % 12) + 12) % 12 + 1; return _pad(y, 4) + '-' + _pad(mo, 2); }
    case 'week': { const d = new Date(n); const dow = (d.getUTCDay() + 6) % 7; const thu = n - dow * 86400000 + 3 * 86400000; const ty = new Date(thu).getUTCFullYear(); const w = Math.round((n - dow * 86400000 - _isoWeek1Monday(ty)) / 604800000) + 1; return _pad(ty, 4) + '-W' + _pad(w, 2); }
    case 'time': { n = ((n % 86400000) + 86400000) % 86400000; const ms = n % 1000; n = Math.floor(n / 1000); const s = n % 60; n = Math.floor(n / 60); const mi = n % 60; const h = Math.floor(n / 60); let str = _pad(h, 2) + ':' + _pad(mi, 2); if (s || ms) { str += ':' + _pad(s, 2); if (ms) str += '.' + _pad(ms, 3); } return str; }
    case 'datetime-local': { const dt = new Date(n); let str = _pad(dt.getUTCFullYear(), 4) + '-' + _pad(dt.getUTCMonth() + 1, 2) + '-' + _pad(dt.getUTCDate(), 2) + 'T' + _pad(dt.getUTCHours(), 2) + ':' + _pad(dt.getUTCMinutes(), 2); const s = dt.getUTCSeconds(), ms = dt.getUTCMilliseconds(); if (s || ms) { str += ':' + _pad(s, 2); if (ms) str += '.' + _pad(ms, 3); } return str; }
  }
  return String(n);
}

// WebIDL interface constants live on both the interface object and the interface
// prototype object (instances inherit; idlharness checks Node.prototype).
Object.assign(Node.prototype, {
  ELEMENT_NODE: 1, ATTRIBUTE_NODE: 2, TEXT_NODE: 3, CDATA_SECTION_NODE: 4,
  ENTITY_REFERENCE_NODE: 5, ENTITY_NODE: 6, PROCESSING_INSTRUCTION_NODE: 7,
  COMMENT_NODE: 8, DOCUMENT_NODE: 9, DOCUMENT_TYPE_NODE: 10, DOCUMENT_FRAGMENT_NODE: 11,
  NOTATION_NODE: 12, DOCUMENT_POSITION_DISCONNECTED: 1, DOCUMENT_POSITION_PRECEDING: 2,
  DOCUMENT_POSITION_FOLLOWING: 4, DOCUMENT_POSITION_CONTAINS: 8,
  DOCUMENT_POSITION_CONTAINED_BY: 16, DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC: 32,
});

// HTML elements ASCII-lowercase attribute names (setAttribute('accessKey') is
// stored as 'accesskey'). The toLowerCase is gated behind a cheap uppercase
// charCode scan so the all-lowercase common case (href, class, id, data-*)
// allocates nothing and never consults the namespace; only when an uppercase
// ASCII letter is present do we check the element is HTML before folding.
function _htmlAttrName(el, n) {
  n = typeof n === "string" ? n : String(n);
  for (let i = 0; i < n.length; i++) {
    const c = n.charCodeAt(i);
    if (c >= 65 && c <= 90) {
      return el.namespaceURI === "http://www.w3.org/1999/xhtml" ? n.toLowerCase() : n;
    }
  }
  return n;
}
// Shared memo behind the dataset getter's attrFor (see there for why).
const _attrForMemo = new Map();

// innerText machinery (see the getter on Element). Tags whose UA default is
// display:none (never rendered) and block-level (line-breaking) respectively.
const _IT_SKIP_TAGS = new Set(['script', 'style', 'template', 'noscript']);
const _IT_BLOCK_TAGS = new Set(('address article aside blockquote center dd details dialog dir div dl dt ' +
  'fieldset figcaption figure footer form h1 h2 h3 h4 h5 h6 header hgroup hr li listing main menu nav ol p pre ' +
  'section summary table tfoot tr ul').split(' '));
const _IT_BLOCK_DISPLAY = /^(block|flow-root|list-item|flex|inline-flex|grid|inline-grid|table|table-row|table-row-group|table-header-group|table-footer-group|table-caption)$/;
const _IT_PRE_TAGS = new Set(['pre', 'textarea', 'listing']);
// One cascade read (whole computed-style table) — same op getComputedStyle
// uses, just without building the proxy. Null when the style engine hasn't
// answered for this element yet.
const _itCascade = (el) => {
  try {
    if (el && el._nid != null) {
      const raw = _domRaw('computed_style', String(el._nid | 0), '');
      if (raw && raw !== 'null') {
        const t = JSON.parse(raw);
        if (t && typeof t === 'object') return t;
      }
    }
  } catch (e) {}
  return null;
};
const _innerTextDisplay = (el) => {
  // Inline style first, then the hidden attribute, then the cascade table —
  // the cascade answers with defaults ('block') for any element it has seen,
  // so asking it first would shadow [hidden] entirely whenever a style pass
  // has run (feature-dependent behavior otherwise).
  const s = el && el.style && el.style.display;
  if (s) return s;
  if (el && el.getAttribute && el.getAttribute('hidden') != null) return 'none';
  const cs = _itCascade(el);
  if (cs && cs.display != null) return String(cs.display);
  return null;
};
const _itPushSep = (out, sep) => {
  // Push a structural separator (block '\n', table-cell '\t'). Consecutive
  // cell tabs stack (Chrome emits one per empty cell); a row break eats a
  // trailing tab; a text run before the break loses its trailing spaces.
  while (out.length) {
    const last = out[out.length - 1];
    if (last.s.endsWith('\n')) return;
    if (last.sep && sep === '\n') { out.pop(); continue; }
    if (!last.sep && !last.pre) {
      last.s = last.s.replace(/[ \t]+$/, '');
      if (last.s === '') { out.pop(); continue; }
    }
    break;
  }
  if (!out.length) return;
  out.push({ s: sep, pre: false, sep: true });
};
// 批240 / blitz#924: innerText REFLECTS text-transform (Chrome: uppercase
// renders "ABC DEF", innerText reads it back transformed while textContent
// keeps the source). The computed-style layer is declared-only for this
// inherited prop, so the walk threads `st.tt` — a DECLARED value (including
// 'none', which resets) overrides, unset keeps the ancestor's.
const _itWordJoiner = (c) => {
  const cp = c.codePointAt(0);
  if (cp >= 0x30 && cp <= 0x39) return true; // ASCII digits join (`2abc` stays lowercase)
  if (c === '_' || c === "'" || c === '\u2019' || c === ':' || c === '\u00b7') return true;
  // Han ideographs + kana own their words — they break (`中文abc` caps the a).
  if ((cp >= 0x3040 && cp <= 0x30FF) || (cp >= 0x31F0 && cp <= 0x31FF) ||
      (cp >= 0x3400 && cp <= 0x4DBF) || (cp >= 0x4E00 && cp <= 0x9FFF) ||
      (cp >= 0xF900 && cp <= 0xFAFF) || (cp >= 0x20000 && cp <= 0x2A6DF)) return false;
  return /[\p{L}\p{Nd}]/u.test(c); // letters (Hangul/Latin/Greek/…) and Nd digits join
};
const _itCapitalize = (s) => {
  let out = '', joins = false;
  for (const c of s) {
    out += !joins && /\p{Ll}/u.test(c) ? c.toUpperCase() : c;
    joins = _itWordJoiner(c);
  }
  return out;
};
const _itApplyTransform = (s, tt) => {
  if (tt === 'uppercase') return s.toUpperCase();
  if (tt === 'lowercase') return s.toLowerCase();
  if (tt === 'capitalize') return _itCapitalize(s);
  return s;
};
const _innerTextCollect = (node, out, st) => {
  if (!node) return;
  const nt = node.nodeType;
  if (nt === 3 || nt === 4) {
    if (st.hidden) return;
    let s = String(node.data ?? '');
    if (!s) return;
    s = _itApplyTransform(s, st.tt);
    if (st.pre) { out.push({ s, pre: true }); return; }
    s = s.replace(/[ \t\r\n\f]+/g, ' ');
    if (out.length && out[out.length - 1].s.endsWith('\n')) s = s.replace(/^ +/, '');
    if (s) out.push({ s, pre: false });
    return;
  }
  if (nt !== 1) return;
  const el = node;
  const tag = el.localName;
  if (_IT_SKIP_TAGS.has(tag)) return;
  if (tag === 'br') { _itPushSep(out, '\n'); return; }
  const cs = _itCascade(el);
  let display = (el.style && el.style.display) || null;
  if (display == null && el.getAttribute && el.getAttribute('hidden') != null) display = 'none';
  if (display == null && cs && cs.display != null) display = String(cs.display);
  if (display === 'none') return;
  // Cells break with a tab (Chrome's cell separator), not a newline. Tag
  // shape covers the no-cascade walk; an explicit display (including
  // non-cell overrides on td/th) wins.
  const cell = display === 'table-cell' || (display == null && (tag === 'td' || tag === 'th'));
  const blockish = cell || (display != null
    ? _IT_BLOCK_DISPLAY.test(display)
    : _IT_BLOCK_TAGS.has(tag));
  const sep = cell ? '\t' : '\n';
  let visibility = cs && cs.visibility != null ? String(cs.visibility) : null;
  // Whether the visibility value is DECLARED (cascade answered, or inline
  // style carries one) rather than the walk's 'visible' default — only a
  // declared 'visible' restores text under a hidden ancestor (obscura#1106);
  // an undeclared child inherits whatever state it walked into.
  const visDeclared = visibility != null || !!(el.style && el.style.visibility);
  if (visibility == null) visibility = (el.style && el.style.visibility) || 'visible';
  const ws = cs && typeof cs['white-space'] === 'string' ? cs['white-space'] : '';
  // Cascade first (it already folds the inline style); the inline read only
  // serves elements the style engine hasn't answered for yet.
  let tt = cs && typeof cs['text-transform'] === 'string' ? cs['text-transform'] : null;
  if (tt == null) tt = (el.style && el.style.textTransform) || null;
  const prevHidden = st.hidden;
  const prevPre = st.pre;
  const prevTt = st.tt;
  if (visibility === 'hidden' || visibility === 'collapse') st.hidden = true;
  else if (visDeclared && visibility === 'visible') st.hidden = false;
  if (_IT_PRE_TAGS.has(tag) || ws.startsWith('pre')) st.pre = true;
  if (tt) st.tt = tt;
  // Cells separate on CLOSE only — the previous cell's close already laid
  // the tab, an open-push would double it, and an empty cell still
  // contributes its own tab via its close (Chrome emits one per boundary).
  if (blockish && !cell) _itPushSep(out, sep);
  const kids = el.childNodes;
  for (let i = 0; i < kids.length; i++) _innerTextCollect(kids[i], out, st);
  if (blockish) _itPushSep(out, sep);
  st.hidden = prevHidden;
  st.pre = prevPre;
  st.tt = prevTt;
};

class Element extends Node {
  constructor(nid) {
    super(nid);
    this._style = _styleProxy(_newCssDecl(this));
    __hideOwn(this);
  }
  get tagName() { return _domParse("tag_name", this._nid) || ""; }
  get localName() {
    // tagName is an op call and the tag never changes, so cache the localName.
    // This keeps the new <a>/<area> href getters (which read localName) and
    // every other localName consumer off the op path. HTML lowercases; the op
    // reports ns=html tags ALWAYS uppercased and XML-namespace tags in source
    // case, so a lowercase first letter already means non-HTML (svg
    // linearGradient must not fold, #28).
    if (this._lname !== undefined) return this._lname;
    const t = this.tagName || "";
    const ln = t && t[0] === t[0].toLowerCase() ? t : t.toLowerCase();
    if (ln) this._lname = ln;
    return ln;
  }
  get id() { return this.getAttribute("id") || ""; }
  set id(v) { this.setAttribute("id", v); }
  get className() { return this.getAttribute("class") || ""; }
  set className(v) { this.setAttribute("class", v); }
  get namespaceURI() {
    // The tree carries the true per-node namespace (namespace_uri): parsed
    // SVG children inherit theirs, create_element records what was asked.
    // Legacy mirror: DOMParser's XML builder and old createElementNS set _ns
    // as an own prop, where "" maps to the null namespace per spec — keep
    // honoring it so those paths stay exact, then fall back to the tree.
    if (this._ns !== undefined) return this._ns === "" ? null : this._ns;
    const ns = _domParse("namespace_uri", this._nid);
    if (typeof ns === "string") return ns === "" ? null : ns;
    return "http://www.w3.org/1999/xhtml";
  }
  get innerHTML() { return _domParse("inner_html", this._nid) ?? ""; }
  set innerHTML(v) {
    if (this.localName === 'template') {
      // 刀57: the fragment prototype no longer carries an innerHTML setter
      // (Chrome truth), so parse straight through the op instead of writing
      // `content.innerHTML` — that would now be a plain no-parse expando.
      _dom("set_inner_html", this.content._nid, String(v ?? ""));
      return;
    }
    // Capture the children that are about to be replaced so we can deliver
    // them as `removedNodes` in the MutationObserver record. Without this,
    // libraries that mutate via `innerHTML =` (jQuery's `.html(s)`, React
    // `dangerouslySetInnerHTML`, vue-style content swaps) silently bypass
    // every MutationObserver subscriber and downstream hydration / polling
    // logic stalls.
    let oldChildren = [];
    let newChildren = [];
    if (globalThis.__mutationObservers?.length) {
      oldChildren = _domParse("child_nodes", this._nid) || [];
    }
    // (#48 + #16) The replaced subtree's nids die at set_inner_html and may
    // be recycled; collect BOTH registries' names (window + document) before
    // the op so each can drop them (a stale nid would alias a future element).
    let oldNamed = null;
    if (_namedEls.size || _docEls.size) {
      const kids0 = _domParse("child_nodes", this._nid) || [];
      for (const nid of kids0) {
        const el = _wrapEl(+nid);
        if (el && el.nodeType === 1) {
          const ns = _namedNamesAll(el);
          if (ns.length) { (oldNamed = oldNamed || []).push([el._nid, ns]); }
        }
      }
    }
    _dom("set_inner_html", this._nid, String(v ?? ""));
    if (oldNamed) for (const pair of oldNamed) _namedUntrackBy(pair[0], pair[1]);
    if (_nodeInDocument(this)) _namedScanIn(this);
    // Detached markup upgrades too (Chrome: constructor + attribute
    // reactions at innerHTML time, connectedCallback waits for insertion).
    else __upgradeCustomElementsIn(this, false);
    if (_nodeInDocument(this)) {
      _registerIframesIn(this);
      // Custom elements instanced by the markup upgrade as it enters the
      // document — scripts among the new children stay inert (the
      // already-started flag), elements do not (Chrome upgrades innerHTML
      // custom elements synchronously).
      __upgradeCustomElementsIn(this, false);
      // Stylesheets arriving through innerHTML-connected markup load like
      // any other (the replaced children are new nodes — scripts among them
      // stay inert per the already-started flag, sheets do not).
      __prepareInsertedStylesheetLinksIn(this);
      __prepareInsertedStylesIn(this);
      // Markup-level animations start when the markup enters the document.
      _scheduleAnimationCheck(this);
    }
    if (globalThis.__mutationObservers?.length) {
      newChildren = _domParse("child_nodes", this._nid) || [];
      globalThis.__notifyMutation('childList', this._nid, newChildren, oldChildren);
    }
  }
  get outerHTML() { return _domParse("outer_html", this._nid) ?? ""; }
  // innerText approximates rendered-text semantics (obscura #828 lineage):
  // <script>/<style>/<template>/<noscript> and display:none subtrees are not
  // rendered so contribute nothing; visibility:hidden suppresses text but a
  // declared visibility:visible descendant restores it (obscura#1106);
  // block-level boxes break lines, table cells separate with tabs;
  // collapsible whitespace collapses as rendered; text-transform reflects
  // (批240). A root that is NOT being rendered — display:none/[hidden], or
  // detached from the document — falls back to textContent exactly like
  // Chrome; the rendered-text contract governs descendants of a rendered
  // root. Known approximations: no list markers. The cascade is read per
  // element (stylesheet rules + inline style); when the style engine
  // hasn't run, the inline style attribute and the UA-default tag table
  // below stand in.
  get innerText() {
    try {
      // A visibility:hidden root deliberately keeps the walk (the WeChat
      // fetch contract reads #js_content's innerText while the container is
      // still visibility:hidden SSR-pending — v0.3.1 P1-3); only display:none
      // / detachment are "not being rendered".
      const rootDisp = _innerTextDisplay(this);
      if (rootDisp === 'none') return this.textContent;
      if (!_nodeInDocument(this)) return this.textContent;
      // Seed the walk's context from the element's OWN declarations (批240):
      // reading innerText on the element carrying text-transform must reflect
      // it — a children-only walk never saw the declaration on the very
      // element innerText was read from. Deliberately NOT walking the
      // element itself as a node: the WeChat fetch contract reads
      // #js_content's innerText while the container is still
      // visibility:hidden (SSR-pending), and the container's own hiddenness
      // must not suppress that read (v0.3.1 P1-3).
      const cs = _itCascade(this);
      let tt0 = cs && typeof cs['text-transform'] === 'string' ? cs['text-transform'] : null;
      if (tt0 == null) tt0 = (this.style && this.style.textTransform) || null;
      const ws = cs && typeof cs['white-space'] === 'string' ? cs['white-space'] : '';
      const st = { hidden: false, pre: false };
      if (tt0) st.tt = tt0;
      if (_IT_PRE_TAGS.has(this.localName) || ws.startsWith('pre')) st.pre = true;
      const out = [];
      const kids = this.childNodes;
      for (let i = 0; i < kids.length; i++) {
        _innerTextCollect(kids[i], out, st);
      }
      if (!out.length) return '';
      let s = '';
      for (const p of out) s += p.s;
      // Outer trim, except where a pre-formatted piece owns the edge —
      // Chromium keeps pre spacing even at the element boundary.
      if (!out[0].pre) s = s.replace(/^[ \t\r\n]+/, '');
      if (!out[out.length - 1].pre) s = s.replace(/[ \t\r\n]+$/, '');
      return s;
    } catch (e) {
      return this.textContent;
    }
  }
  set innerText(v) { this.textContent = v; }
  get children() {
    const ids = _domParse("element_children", this._nid) || [];
    return __diting_mkCollection(ids.map(_wrapEl).filter(Boolean));
  }
  get content() {
    // <template>.content is a DocumentFragment; <meta>.content reflects
    // the content attribute (read/write per spec). Next.js' next/head
    // iterates <meta> tags and sets .content during hydration, which
    // threw with the previous getter-only stub and put React into an
    // infinite retry loop (issue #210).
    const tag = this.localName;
    if (tag === 'template') {
      // Back the fragment with the node's real template contents.
      // The parser stores template children in a separate contents document
      // instead of under the element, so without this the getter handed back a
      // fabricated empty fragment and the parsed markup was unreachable.
      // `template_contents` allocates one on demand for created templates.
      const nid = +_dom("template_contents", this._nid);
      if (nid >= 0) {
        // Cache by node id so `.content` keeps a stable identity across reads —
        // frameworks stash the fragment and compare it later.
        if (!_cache.has(nid)) _cache.set(nid, new DocumentFragment(nid));
        const content = _cache.get(nid);
        content._fragmentContext = 'template';
        return content;
      }
      if (!this._templateContent) __def(this, '_templateContent', document.createDocumentFragment());
      return this._templateContent;
    }
    if (tag === 'meta') return this.getAttribute('content') || '';
    return undefined;
  }
  set content(v) {
    if (this.localName === 'meta') {
      this.setAttribute('content', v == null ? '' : String(v));
    }
  }
  get childElementCount() { return this.children.length; }
  get firstElementChild() { return this.children[0] || null; }
  get lastElementChild() { const ch = this.children; return ch[ch.length-1] || null; }
  get nextElementSibling() { let s = this.nextSibling; while(s && s.nodeType !== 1) s = s.nextSibling; return s; }
  get previousElementSibling() { let s = this.previousSibling; while(s && s.nodeType !== 1) s = s.previousSibling; return s; }
  get classList() {
    if (!this._classList) __def(this, '_classList', new DOMTokenList(this, "class"));
    return this._classList;
  }
  get relList() {
    const ns = this.namespaceURI, ln = this.localName;
    const ok = (ns === "http://www.w3.org/2000/svg" && ln === "a") ||
               (ns === "http://www.w3.org/1999/xhtml" && (ln === "a" || ln === "area" || ln === "link"));
    if (!ok) return undefined;
    if (!this._relList) {
      __def(this, '_relList', new DOMTokenList(this, "rel",
        (ns === "http://www.w3.org/1999/xhtml" && ln === "link") ? LINK_REL_SUPPORTED : undefined));
    }
    return this._relList;
  }
  // `rel` reflection — the plain-string twin of relList, same element set as
  // Chrome. webpack's CSS chunk runtime (d.f.miniCss) builds its <link> with
  // `link.rel = "stylesheet"` through this property: without reflection the
  // attribute never lands, the sheet never loads, and the chunk promise the
  // runtime parks on that link's load event never settles (tmall pc-shop
  // lazy-route white screen).
  get rel() {
    const ns = this.namespaceURI, ln = this.localName;
    const ok = (ns === "http://www.w3.org/2000/svg" && ln === "a") ||
               (ns === "http://www.w3.org/1999/xhtml" && (ln === "a" || ln === "area" || ln === "link"));
    if (!ok) return undefined;
    return this.getAttribute("rel") || "";
  }
  set rel(v) {
    const ns = this.namespaceURI, ln = this.localName;
    const ok = (ns === "http://www.w3.org/2000/svg" && ln === "a") ||
               (ns === "http://www.w3.org/1999/xhtml" && (ln === "a" || ln === "area" || ln === "link"));
    if (ok) this.setAttribute("rel", v);
  }
  // `rev` reflection (#177) — the obsolete-but-reflected twin of rel, same
  // element set and same plain-DOMString shape as Chrome. Legacy SEO markup
  // readers walk link.rev; undefined (the pre-fix answer) breaks them.
  get rev() {
    const ns = this.namespaceURI, ln = this.localName;
    const ok = (ns === "http://www.w3.org/2000/svg" && ln === "a") ||
               (ns === "http://www.w3.org/1999/xhtml" && (ln === "a" || ln === "area" || ln === "link"));
    if (!ok) return undefined;
    return this.getAttribute("rev") || "";
  }
  set rev(v) {
    const ns = this.namespaceURI, ln = this.localName;
    const ok = (ns === "http://www.w3.org/2000/svg" && ln === "a") ||
               (ns === "http://www.w3.org/1999/xhtml" && (ln === "a" || ln === "area" || ln === "link"));
    if (ok) this.setAttribute("rev", v);
  }
  // `scheme` reflection (#177, HTMLMetaElement.scheme) — same obsolete-
  // reflected-DOMString family as rev, mirroring the meta arm of `content`
  // above.
  get scheme() {
    if (this.localName !== 'meta') return undefined;
    return this.getAttribute('scheme') || '';
  }
  set scheme(v) {
    if (this.localName === 'meta') {
      this.setAttribute('scheme', v == null ? '' : String(v));
    }
  }
  get sandbox() {
    if (this.namespaceURI !== "http://www.w3.org/1999/xhtml" || this.localName !== "iframe") return undefined;
    if (!this._sandboxList) __def(this, '_sandboxList', new DOMTokenList(this, "sandbox", SANDBOX_SUPPORTED));
    return this._sandboxList;
  }
  get sizes() {
    if (this.namespaceURI !== "http://www.w3.org/1999/xhtml" || this.localName !== "link") return undefined;
    if (!this._sizesList) __def(this, '_sizesList', new DOMTokenList(this, "sizes"));
    return this._sizesList;
  }
  get htmlFor() {
    if (this.namespaceURI !== "http://www.w3.org/1999/xhtml") return undefined;
    const ln = this.localName;
    if (ln === "output") {
      if (!this._htmlForList) __def(this, '_htmlForList', new DOMTokenList(this, "for"));
      return this._htmlForList;
    }
    if (ln === "label") return this.getAttribute("for") || "";
    return undefined;
  }
  set htmlFor(v) {
    if (this.namespaceURI === "http://www.w3.org/1999/xhtml" && this.localName === "label") {
      this.setAttribute("for", String(v));
    }
  }
  get style() { return this._style; }
  set style(v) { if (typeof v === "string") this._style.cssText = v; }
  getAttribute(n) {
    // Fast path: HTML attributes are stored lowercase, so a direct hit needs no
    // case folding. Only on a miss do we lowercase (gated) and retry, so the hot
    // case (reading an existing lowercase attribute) pays zero scan.
    let v = _domParse("get_attribute", this._nid, n);
    if (v === null) { const ln = _htmlAttrName(this, n); if (ln !== n) v = _domParse("get_attribute", this._nid, ln); }
    return v;
  }
  setAttribute(n, v) {
    n = _htmlAttrName(this, n);
    const popoverPrev = (n === "popover") ? this.popover : undefined;
    // (#48 + #16) id/name writes re-key BOTH named registries; snapshot the
    // old names before the op drops them.
    let oldNamed = null;
    if ((n === "id" || n === "name") && (_namedEls.size || _docEls.size) && this.nodeType === 1) {
      const ns = _namedNamesAll(this);
      if (ns.length) oldNamed = [this._nid, ns];
    }
    const __moOld = globalThis.__mutationObservers?.length ? this.getAttribute(n) : undefined;
    const __ceOld = this.__customUpgraded ? this.getAttribute(n) : undefined;
    if (n === "open") globalThis.__detailsAttr?.(this);
    _dom("set_attribute", this._nid, n + "\0" + String(v));
    if (this.__customUpgraded) globalThis.__diting_ceAttrChanged?.(this, n, __ceOld, String(v), null);
    if (oldNamed) _namedUntrackBy(oldNamed[0], oldNamed[1]);
    if ((n === "id" || n === "name") && this.isConnected) _namedTrack(this);
    if (popoverPrev !== undefined) this._popoverTypeMaybeChanged(popoverPrev);
    // A stylesheet link's href/rel changing while connected restarts its
    // fetch in a browser (HTML §the-link-element: "whenever an href is
    // set, changed... run these steps"). Covers both setAttribute and the
    // IDL `link.href = …` reflection, which funnels through here.
    if ((n === "href" || n === "rel") &&
        (this.tagName || '').toUpperCase() === 'LINK' && this.isConnected) {
      __prepareInsertedStylesheetLink(this);
    }
    // An iframe's src changing while connected starts a navigation in a
    // browser. Framework DOM writers (React et al.) only touch the
    // attribute, never the IDL property — the tmall sucai-selector iframe
    // sat registered-but-never-fetched this way (#121). The IDL
    // `iframe.src = …` setter funnels through here as well; the loading
    // flag inside _loadIframeSrc collapses the repeat.
    if (n === "src" &&
        (this.tagName || '').toUpperCase() === 'IFRAME' && this.isConnected) {
      const s = String(v);
      if (s && s !== 'about:blank') this._loadIframeSrc(s);
    }
    // srcdoc present wins over src (HTML spec): setting it navigates the
    // frame to about:srcdoc regardless of any src attribute.
    if (n === "srcdoc" &&
        (this.tagName || '').toUpperCase() === 'IFRAME' && this.isConnected) {
      this._loadIframeSrc('about:srcdoc');
    }
    if (globalThis.__mutationObservers?.length) globalThis.__notifyMutation('attributes', this._nid, [], [], n, __moOld, null, null);
    // class/style writes can start or stop a CSS animation on this element,
    // and may start a transition (before/after computed-value diff).
    if (n === "class" || n === "style") { _scheduleAnimationCheck(this); _scheduleTransitionCheck(this); }
  }
  setAttributeNS(ns, n, v) {
    n = String(n);
    const __moOld = globalThis.__mutationObservers?.length ? this.getAttributeNS(ns, n) : undefined;
    const __ceOld = this.__customUpgraded ? this.getAttributeNS(ns, n) : undefined;
    _dom("set_attribute", this._nid, n + "\0" + String(v)); // exact name, no HTML folding
    if (this.__customUpgraded) globalThis.__diting_ceAttrChanged?.(this, n.slice(n.indexOf(":") + 1), __ceOld, String(v), ns);
    if (globalThis.__mutationObservers?.length) globalThis.__notifyMutation('attributes', this._nid, [], [], n, __moOld, null, null);
    const ln = n.toLowerCase();
    if (ln === "class" || ln === "style") { _scheduleAnimationCheck(this); _scheduleTransitionCheck(this); }
  }
  removeAttribute(n) {
    n = _htmlAttrName(this, n);
    // No attribute present → no record (Chrome fires nothing when removing
    // an absent attribute).
    const __moHad = globalThis.__mutationObservers?.length ? this.hasAttribute(n) : false;
    const __moOld = __moHad ? this.getAttribute(n) : undefined;
    const popoverPrev = (n === "popover") ? this.popover : undefined;
    if ((n === "id" || n === "name") && _namedEls.size && this.nodeType === 1) _namedUntrack(this);
    if (n === "open") globalThis.__detailsAttr?.(this);
    const __ceOld = this.__customUpgraded ? this.getAttribute(n) : null;
    _dom("remove_attribute", this._nid, n);
    if (__ceOld !== null) globalThis.__diting_ceAttrChanged?.(this, n, __ceOld, null, null);
    if (__moHad && globalThis.__mutationObservers?.length) globalThis.__notifyMutation('attributes', this._nid, [], [], n, __moOld, null, null);
    if (popoverPrev !== undefined) this._popoverTypeMaybeChanged(popoverPrev);
    if (n === "class" || n === "style") { _scheduleAnimationCheck(this); _scheduleTransitionCheck(this); }
  }
  removeAttributeNS(ns, n) {
    n = String(n);
    const __moHad = globalThis.__mutationObservers?.length ? this.getAttributeNS(ns, n) !== null : false;
    const __moOld = __moHad ? this.getAttributeNS(ns, n) : undefined;
    const __ceOld = this.__customUpgraded ? this.getAttributeNS(ns, n) : null;
    _dom("remove_attribute", this._nid, n);
    if (__ceOld !== null) globalThis.__diting_ceAttrChanged?.(this, n, __ceOld, null, ns);
    if (__moHad && globalThis.__mutationObservers?.length) globalThis.__notifyMutation('attributes', this._nid, [], [], n, __moOld, null, null);
    const ln = n.toLowerCase();
    if (ln === "class" || ln === "style") { _scheduleAnimationCheck(this); _scheduleTransitionCheck(this); }
  }
  getAttributeNames() {
    // Chrome 61+ API; the xhs creator publish page's mount path calls it
    // unguarded, so its absence rejects the whole component tree (#51).
    return _domParse("attribute_names", this._nid) || [];
  }
  hasAttribute(n) { return this.getAttribute(n) !== null; }
  hasAttributes() { return (_domParse("attribute_names", this._nid) || []).length > 0; }
  get attributes() {
    const el = this;
    const names = _domParse("attribute_names", el._nid) || [];
    const list = names.map((name) => ({
      name,
      localName: name,
      value: el.getAttribute(name) ?? "",
      namespaceURI: null,
      prefix: null,
      specified: true,
      ownerElement: el,
      nodeName: name,
      nodeValue: el.getAttribute(name) ?? "",
      nodeType: 2,
    }));
    list.length = names.length;
    list.getNamedItem = (n) => names.includes(n) ? list[names.indexOf(n)] : null;
    list.setNamedItem = (a) => { if (a && a.name) el.setAttribute(a.name, a.value); return a; };
    list.removeNamedItem = (n) => { const a = list.getNamedItem(n); if (a) el.removeAttribute(n); return a; };
    list.item = (i) => list[i] || null;
    for (let i = 0; i < names.length; i++) {
      Object.defineProperty(list, names[i], { value: list[i], configurable: true, enumerable: false });
    }
    return list;
  }
  getAttributeNS(ns, n) { return _domParse("get_attribute", this._nid, String(n)); }
  querySelector(s) {
    const id = +_dom("query_selector_scoped", this._nid, s);
    // Parser-order visibility (#120): if the engine's first match is a script
    // the parser has not reached yet, fall back to the filtered list so the
    // caller gets the first *visible* match, not null-with-phantoms.
    if (_parserScriptHidden(id)) return this.querySelectorAll(s)[0] || null;
    return _wrapEl(id);
  }
  querySelectorAll(s) {
    const ids = _parserVisibleIds(_domParse("query_selector_all_scoped", this._nid, s) || []);
    return _nodeList(ids.map(_wrapEl).filter(Boolean));
  }
  getElementsByTagName(t) { return __diting_mkCollection(this.querySelectorAll(t)); }
  getElementsByClassName(c) { return _getElementsByClassName(this, c); }
  matches(s) {
    // :popover-open is a JS-observable popover state, not understood by the
    // native selector engine. Handle it here (and strip it from compound
    // selectors so the rest can still be matched natively).
    if (typeof s === "string" && s.indexOf(":popover-open") !== -1) {
      if (this._popoverState !== "showing") return false;
      const rest = s.replace(/:popover-open/g, "").trim();
      if (rest === "") return true;
      return this.matches(rest);
    }
    // :modal is a JS-observable dialog state (a dialog opened via showModal()),
    // not understood by the native selector engine; handle it like :popover-open.
    if (typeof s === "string" && s.indexOf(":modal") !== -1) {
      if (this._dialogModal !== true) return false;
      const rest = s.replace(/:modal/g, "").trim();
      if (rest === "") return true;
      return this.matches(rest);
    }
    // Native single-element match. The Rust side binds `:scope` to the element
    // being tested (Element.matches semantics) and handles detached subjects,
    // so no :scope special case and no parent-scoped rescan are needed here.
    return _dom("matches_selector", this._nid, s) === "1";
  }
  closest(s) {
    let el = this;
    while (el) {
      if (el.nodeType === 1 && el.matches && el.matches(s)) return el;
      el = el.parentNode;
    }
    return null;
  }
  insertAdjacentHTML(position, html) {
    // Parse in the insertion element's context so table/select content
    // survives — a fixed <div> context makes the fragment parser drop
    // <tr>/<td>/<option> (set_inner_html parses with the target as context).
    const pos = String(position).toLowerCase();
    const parent = this.parentNode;
    const context = (pos === 'beforebegin' || pos === 'afterend') ? parent : this;
    const tag = context && context.nodeType === 1 ? String(context.tagName || 'body').toLowerCase() : 'body';
    const tmp = document.createElement(tag);
    tmp.innerHTML = html;
    // Pop firstChild repeatedly: tmp.childNodes is LIVE, so indexing it while
    // moving nodes out skips every other node.
    const nodes = [];
    let child;
    while ((child = tmp.firstChild)) nodes.push(tmp.removeChild(child));
    switch (pos) {
      case 'beforebegin':
        if (parent) { for (const n of nodes) parent.insertBefore(n, this); }
        break;
      case 'afterbegin':
        { const first = this.firstChild; for (const n of nodes) this.insertBefore(n, first); }
        break;
      case 'beforeend':
        for (const n of nodes) this.appendChild(n);
        break;
      case 'afterend':
        if (parent) { const next = this.nextSibling; for (const n of nodes) parent.insertBefore(n, next); }
        break;
      default:
        // Unknown position throws SyntaxError (a silent no-op before).
        throw new DOMException(
          "Failed to execute 'insertAdjacentHTML' on 'Element': The value provided ('" + position + "') is not one of 'beforeBegin', 'afterBegin', 'beforeEnd', or 'afterEnd'.",
          "SyntaxError"
        );
    }
  }
  // insertAdjacentElement: like insertAdjacentHTML but inserts an existing
  // element node. Some site frameworks (Zhihu's column.app.js) call this and
  // crash when it's missing, aborting the page's JS (including fingerprint
  // generation that sets __zse_ck). Implemented per DOM spec.
  insertAdjacentElement(position, element) {
    const parent = this.parentNode;
    switch (position) {
      case 'beforebegin':
        if (parent) { parent.insertBefore(element, this); return element; }
        break;
      case 'afterbegin':
        { const first = this.firstChild; this.insertBefore(element, first); return element; }
        break;
      case 'beforeend':
        this.appendChild(element);
        return element;
      case 'afterend':
        if (parent) { const next = this.nextSibling; parent.insertBefore(element, next); return element; }
        break;
    }
    return element;
  }
  // insertAdjacentText: inserts a text node relative to this element. DDoS-Guard
  // challenge view.js (annas-archive.gl) calls this inside NodeList.forEach —
  // missing here, the TypeError aborted the whole challenge bootstrap and
  // window.DDG was never defined.
  insertAdjacentText(position, text) {
    const pos = String(position).toLowerCase();
    const node = document.createTextNode(String(text));
    const parent = this.parentNode;
    switch (pos) {
      case 'beforebegin':
        if (parent) parent.insertBefore(node, this);
        break;
      case 'afterbegin':
        this.insertBefore(node, this.firstChild);
        break;
      case 'beforeend':
        this.appendChild(node);
        break;
      case 'afterend':
        if (parent) parent.insertBefore(node, this.nextSibling);
        break;
      default:
        throw new DOMException(
          "Failed to execute 'insertAdjacentText' on 'Element': The value provided ('" + position + "') is not one of 'beforeBegin', 'afterBegin', 'beforeEnd', or 'afterEnd'.",
          "SyntaxError"
        );
    }
  }
  // addEventListener/removeEventListener/dispatchEvent resolve through the
  // prototype chain to the NATIVE EventTarget core (M3 B4): the trio here is
  // gone; everything Element-specific about a dispatch lives in host hooks
  // (js/event_hooks.js) — parent edge (incl. document → window), inline
  // content-attribute handlers, offset resolution (origin hook), click
  // activation (activationPre/Post), the animation check (registered hook).
  _resolveInlineHandler(name) {
    // name = 'onclick' / 'onsubmit' / etc. Compile the content attribute
    // as a function body on first read and cache it on the instance.
    const cache = this.__inlineHandlerCache || __def(this, '__inlineHandlerCache', {});
    if (Object.prototype.hasOwnProperty.call(cache, name)) return cache[name];
    const src = this.getAttribute && this.getAttribute(name);
    if (!src) { cache[name] = null; return null; }
    try {
      cache[name] = new Function('event', src);
    } catch (e) {
      cache[name] = null;
    }
    return cache[name];
  }
  click() {
    // Disabled form controls have no activation behaviour and dispatch no
    // click event at all — own disabled attribute or disabled by an ancestor
    // <fieldset disabled> (first <legend> exempt).
    if (_isFormControlDisabled(this)) return;
    // File chooser interception (#239): a real browser opens the native
    // picker here and the page waits for the user's selection. The engine
    // has no picker UI — instead the input becomes the session's PENDING
    // CHOOSER: selector-less set_files feeds it (el.files + input/change
    // dispatch), so off-DOM inputs whose listeners attach only on widget
    // click (doudian's new upload bundle) become drivable. Latest click()
    // wins; the record is page-global state, so a navigation clears it.
    if (this.tagName === 'INPUT' && (this.getAttribute('type') || '').toLowerCase() === 'file') {
      const prev = globalThis.__ditingFileChooser;
      globalThis.__ditingFileChooser = { el: this, at: Date.now(), gen: prev ? prev.gen + 1 : 1 };
    }
    // "Click in progress" flag per spec, checked BEFORE any pre-activation
    // step: a nested .click() on an element whose click is still running must
    // be a full no-op, not a second state flip. This also stops a control's
    // handler clicking its own label from bouncing the click back.
    if (this._ditingClickInProgress) return;
    __def(this, '_ditingClickInProgress', true);
    try {
      // Pre-click activation steps (HTML spec): a checkbox/radio flips BEFORE
      // the click event dispatches, so listeners observe the new state, and
      // the flip reverts if the event is cancelled. Radio groups uncheck
      // same-name peers up front; a cancel restores every prior state.
      // Checkbox activation also clears `indeterminate`; a cancel restores it
      // along with `checked`. The rest of activation (label forwarding,
      // anchors, submit) runs after an uncanceled dispatch.
      const flip = _preClickFlip(this);
      const cancelled = !this.dispatchEvent(new MouseEvent("click", {bubbles: true, cancelable: true}));
      if (cancelled) {
        _cancelPreClickFlip(flip);
        return;
      }
      _commitPreClickFlip(flip);
      if (flip && flip.el.checked !== flip.oldChecked) return;
      _postClickActivation(this);
    } finally {
      this._ditingClickInProgress = false;
    }
  }
  // Focus/blur are real now (blitz#839): the tree-level focused node drives
  // :focus/:focus-within/:focus-visible matching and repaint, and the focus
  // event family fires. Chrome order for a switch old→new is blur+focusout
  // on the old target, then focus (non-bubbling) + focusin (bubbling) on the
  // new one; listeners observe activeElement already moved. A disabled form
  // control is a full no-op (no events, focus unchanged). Chrome also gates
  // focus() on "focusable area" (tabindex/controls/links); we keep the old
  // engine permissiveness — anything can hold focus — because typing and
  // click targeting flow through activeElement and page scripts rely on it.
  focus(options) {
    if (_isFormControlDisabled(this)) return;
    if (globalThis.__diting_focused === this) return;
    if (globalThis.__diting_focused) _fireBlurFamily(globalThis.__diting_focused);
    globalThis.__diting_focused = this;
    globalThis.__diting_click_target = this;
    _domRaw("set_focused", String(this._nid), "1");
    // Chrome: script focus() puts the caret at the end of the value unless
    // the control already has a recorded selection (refocus restores it).
    if (_ns_isTextEntry(this) && _ns_selectionStart.get(this) == null) {
      const len = String(this.value ?? '').length;
      _ns_selectionStart.set(this, len);
      _ns_selectionEnd.set(this, len);
      _ns_mirrorSelection(this);
    }
    this.dispatchEvent(new FocusEvent("focus"));
    this.dispatchEvent(new FocusEvent("focusin", {bubbles: true}));
    // #215: focus() implies scrollIntoView nearest/nearest (CSSOM View
    // §3) — the login/form flows that focus an off-screen control expect
    // the viewport to follow. preventScroll opts out.
    if (!(options && options.preventScroll)) {
      try { this.scrollIntoView({block: 'nearest', inline: 'nearest'}); } catch (e) {}
    }
  }
  blur() {
    if (globalThis.__diting_focused !== this) return;
    _fireBlurFamily(this);
  }

  // --- Popover API (HTML "popover") ---------------------------------------
  // Read the popover content attribute case-insensitively. The HTML parser
  // lowercases attribute names, but runtime setAttribute("PoPoVeR", ...)
  // preserves case, and the IDL reflection matches the name ASCII-case-
  // insensitively. Returns the raw stored string, or null if absent.
  _popoverAttrValue() {
    const v = this.getAttribute("popover");
    if (v !== null) return v;
    const names = _domParse("attribute_names", this._nid) || [];
    for (let i = 0; i < names.length; i++) {
      if (names[i].toLowerCase() === "popover") return this.getAttribute(names[i]);
    }
    return null;
  }
  // The reflected (effective) popover type: null (No Popover), "auto",
  // "hint", or "manual". Empty string maps to "auto"; any non-keyword value
  // (invalid) maps to "manual".
  get popover() {
    const raw = this._popoverAttrValue();
    if (raw === null) return null;
    const v = String(raw).toLowerCase();
    if (v === "auto" || v === "hint" || v === "manual") return v;
    if (v === "") return "auto";
    return "manual";
  }
  set popover(value) {
    if (value === null || value === undefined) { this._popoverRemoveAttr(); return; }
    this.setAttribute("popover", String(value));
  }
  _popoverRemoveAttr() {
    if (this.getAttribute("popover") !== null) { this.removeAttribute("popover"); return; }
    const names = _domParse("attribute_names", this._nid) || [];
    for (let i = 0; i < names.length; i++) {
      if (names[i].toLowerCase() === "popover") { this.removeAttribute(names[i]); return; }
    }
  }
  // "check popover validity". expectedToBeShowing is true for hide, false for
  // show. Throws NotSupportedError when there is no valid popover type, and
  // InvalidStateError when the element is not connected; returns false (no
  // throw) when the current state does not match expectedToBeShowing.
  _checkPopoverValidity(expectedToBeShowing) {
    if (this.popover === null) throw new DOMException("Not supported on elements that don't have a valid value for the popover attribute", "NotSupportedError");
    const showing = this._popoverState === "showing";
    if ((expectedToBeShowing && !showing) || (!expectedToBeShowing && showing)) return false;
    if (!this.isConnected) throw new DOMException("Invalid on popover elements which aren't connected", "InvalidStateError");
    return true;
  }
  showPopover() {
    if (!this._checkPopoverValidity(/*expectedToBeShowing*/false)) return;
    const beforeEvent = new ToggleEvent("beforetoggle", { cancelable: true, oldState: "closed", newState: "open" });
    if (!this.dispatchEvent(beforeEvent)) return;
    // The beforetoggle handler may have changed our type or shown us; re-check.
    if (!this._checkPopoverValidity(/*expectedToBeShowing*/false)) return;
    __def(this, '_popoverState', "showing");
    const target = this;
    setTimeout(() => { try { target.dispatchEvent(new ToggleEvent("toggle", { oldState: "closed", newState: "open" })); } catch (e) {} }, 0);
  }
  hidePopover() {
    if (!this._checkPopoverValidity(/*expectedToBeShowing*/true)) return;
    this.dispatchEvent(new ToggleEvent("beforetoggle", { oldState: "open", newState: "closed" }));
    __def(this, '_popoverState', "hidden");
    const target = this;
    setTimeout(() => { try { target.dispatchEvent(new ToggleEvent("toggle", { oldState: "open", newState: "closed" })); } catch (e) {} }, 0);
  }
  togglePopover(force) {
    let options = force;
    if (options && typeof options === "object") force = options.force;
    const showing = this._popoverState === "showing";
    if (showing && (force === undefined || force === null || force === false)) {
      this.hidePopover();
    } else if (force === undefined || force === null || force === true) {
      this.showPopover();
    }
    return this._popoverState === "showing";
  }
  // Called from setAttribute/removeAttribute/IDL setter when the popover
  // attribute may have changed. If the effective type changed while showing,
  // hide the popover (firing the hide events) per the HTML spec.
  _popoverTypeMaybeChanged(prevType) {
    const newType = this.popover;
    if (this._popoverState === "showing" && prevType !== newType) {
      // Hide directly. Do not call hidePopover(): it re-validates against the
      // popover attribute, which may now be removed (No Popover), and would
      // throw NotSupportedError. This mirrors the spec hide with throw=false.
      this.dispatchEvent(new ToggleEvent("beforetoggle", { oldState: "open", newState: "closed" }));
      __def(this, '_popoverState', "hidden");
      const target = this;
      setTimeout(() => { try { target.dispatchEvent(new ToggleEvent("toggle", { oldState: "open", newState: "closed" })); } catch (e) {} }, 0);
    }
  }
  // HTMLDialogElement members (live on Element.prototype like popover/input;
  // meaningful only when localName === 'dialog'). Modal top-layer/focus/render
  // is layout (out of scope); the open state, returnValue, and beforetoggle/
  // toggle/close/cancel events are JS-observable and implemented here.
  get open() { return this.hasAttribute('open'); }
  set open(v) { if (v) { if (!this.hasAttribute('open')) this.setAttribute('open', ''); } else if (this.hasAttribute('open')) { this.removeAttribute('open'); this._dialogModal = false; } }
  get returnValue() { return this._returnValue != null ? this._returnValue : ''; }
  set returnValue(v) { __def(this, '_returnValue', String(v)); }
  get oncancel() { return this._oncancel || null; }
  set oncancel(f) { __def(this, '_oncancel', typeof f === 'function' ? f : null); }
  get onclose() { return this._onclose || null; }
  set onclose(f) { __def(this, '_onclose', typeof f === 'function' ? f : null); }
  get closedBy() { const v = (this.getAttribute('closedby') || '').toLowerCase(); return (v === 'any' || v === 'closerequest' || v === 'none') ? v : 'auto'; }
  set closedBy(v) { this.setAttribute('closedby', String(v)); }
  show() {
    if (this.hasAttribute('open')) { if (this._dialogModal) throw new DOMException("The dialog is already open as a modal dialog.", "InvalidStateError"); return; }
    const before = new ToggleEvent("beforetoggle", { cancelable: true, oldState: "closed", newState: "open" });
    if (!this.dispatchEvent(before)) return;
    if (this.hasAttribute('open')) return;
    this.setAttribute('open', ''); __def(this, '_dialogModal', false);
    const self = this; setTimeout(() => { try { self.dispatchEvent(new ToggleEvent("toggle", { oldState: "closed", newState: "open" })); } catch (e) {} }, 0);
  }
  showModal() {
    if (this.hasAttribute('open')) throw new DOMException("The dialog is already open.", "InvalidStateError");
    if (!this.isConnected) throw new DOMException("The dialog is not connected to a document.", "InvalidStateError");
    const before = new ToggleEvent("beforetoggle", { cancelable: true, oldState: "closed", newState: "open" });
    if (!this.dispatchEvent(before)) return;
    if (this.hasAttribute('open')) return;
    this.setAttribute('open', ''); __def(this, '_dialogModal', true);
    const self = this; setTimeout(() => { try { self.dispatchEvent(new ToggleEvent("toggle", { oldState: "closed", newState: "open" })); } catch (e) {} }, 0);
  }
  _dialogClose(result, fireClose) {
    if (!this.hasAttribute('open')) return;
    this.dispatchEvent(new ToggleEvent("beforetoggle", { oldState: "open", newState: "closed" }));
    this.removeAttribute('open'); __def(this, '_dialogModal', false);
    if (result !== undefined) this._returnValue = String(result);
    const self = this;
    setTimeout(() => { try { self.dispatchEvent(new ToggleEvent("toggle", { oldState: "open", newState: "closed" })); } catch (e) {} }, 0);
    if (fireClose) setTimeout(() => { try { self.dispatchEvent(new Event('close', { bubbles: false, cancelable: false })); } catch (e) {} }, 0);
  }
  close(result) { this._dialogClose(result, true); }
  requestClose(result) {
    if (!this.hasAttribute('open')) return;
    if (this._dialogCancelFiring) return; // no re-entrant cancel
    __def(this, '_dialogCancelFiring', true);
    let canceled = false;
    try { const ev = new Event('cancel', { bubbles: false, cancelable: true }); this.dispatchEvent(ev); canceled = ev.defaultPrevented; }
    finally { this._dialogCancelFiring = false; }
    if (canceled) return;
    this._dialogClose(result, true);
  }
  attachInternals() {
    const reg = (typeof customElements !== 'undefined' && customElements._registry) ? customElements._registry : null;
    if (!reg || !reg.get(this.localName)) throw new DOMException("Failed to execute 'attachInternals' on 'HTMLElement': Unable to attach ElementInternals to non-custom elements.", "NotSupportedError");
    if (this.getAttribute('is')) throw new DOMException("Failed to execute 'attachInternals' on 'HTMLElement': Unable to attach ElementInternals to a customized built-in element.", "NotSupportedError");
    if (this._internalsAttached) throw new DOMException("Failed to execute 'attachInternals' on 'HTMLElement': ElementInternals for the specified element was already attached.", "NotSupportedError");
    this._internalsAttached = true;
    return new ElementInternals(this);
  }
  get value() {
    const tag = this.localName;
    if (tag === 'select') {
      // Selected option wins; otherwise first option (HTML default).
      const opts = this.querySelectorAll('option');
      for (let i = 0; i < opts.length; i++) {
        if (opts[i].selected) {
          return opts[i].getAttribute('value') !== null ? opts[i].getAttribute('value') : opts[i].textContent;
        }
      }
      if (opts.length) return opts[0].getAttribute('value') !== null ? opts[0].getAttribute('value') : opts[0].textContent;
      return '';
    }
    // File inputs never read the dirty-value store: their DOM value is the
    // fakepath string Chrome synthesizes from the selected list.
    if (tag === 'input' && (this.getAttribute('type') || '').toLowerCase() === 'file') {
      const fs = _fileInputs[this._nid];
      return fs && fs.length ? 'C:\\fakepath\\' + fs[0].name : '';
    }
    if (_formValues[this._nid] !== undefined) return _formValues[this._nid];
    if (tag === 'textarea') return this.textContent;
    if (tag === 'option') {
      const attr = this.getAttribute('value');
      return attr !== null ? attr : this.textContent;
    }
    if (tag === 'input') {
      const itype = (this.getAttribute('type') || '').toLowerCase();
      if (itype === 'checkbox' || itype === 'radio') {
        // A checkbox/radio with no value attribute defaults to "on" in a real
        // browser, not the empty string.
        const attr = this.getAttribute('value');
        return attr !== null ? attr : 'on';
      }
      if (_NS_DATE_VALUE_TYPES.indexOf(itype) >= 0) {
        // The attribute default runs the same value sanitization: markup
        // like <input type=date value="not-a-date"> reads '' in Chrome.
        return _ns_sanitizeDateValue(itype, this.getAttribute("value") || "");
      }
    }
    return this.getAttribute("value") || "";
  }
  set value(v) {
    const tag = this.localName;
    // Spec: a file input's value may only be cleared — setting a non-empty
    // string throws InvalidStateError in Chrome. The fakepath string is a
    // read-side synthesis, never stored.
    if (tag === 'input' && (this.getAttribute('type') || '').toLowerCase() === 'file') {
      if (String(v) === '') delete _fileInputs[this._nid];
      else throw new DOMException("Failed to set the 'value' property on 'HTMLInputElement': This input element accepts a filename, which may only be programmatically set to the empty string.", "InvalidStateError");
      return;
    }
    if (tag === 'option') {
      this.setAttribute('value', String(v));
      return;
    }
    if (tag === 'select') {
      // Set selected on matching option, clear on others. Puppeteer's
      // page.select(selector, value) round-trips through this setter and
      // dispatches its own input/change events in-page afterwards, like a
      // real browser: a programmatic value assignment never fires change
      // itself. Dispatching here fed pages that assign inside a change
      // handler back into that handler in an infinite loop.
      const wanted = String(v);
      const opts = this.querySelectorAll('option');
      for (let i = 0; i < opts.length; i++) {
        const attrV = opts[i].getAttribute('value');
        const optVal = attrV !== null ? attrV : opts[i].textContent;
        opts[i].selected = optVal === wanted;
      }
      // Each opts[i].selected write mirrors through the property setter;
      // this final walk pins the select's own state once (and covers an
      // empty option list, where no per-option write happens at all).
      _mirrorSelectLabel(this);
      return;
    }
    if (tag === 'input' && _NS_DATE_VALUE_TYPES.indexOf((this.getAttribute('type') || '').toLowerCase()) >= 0) {
      // Value sanitization: only a complete valid string is stored, everything
      // else stores '' (Chrome parity — and unlike Chrome's segmented editor,
      // the rejected partial keeps composing in the draft so typing works).
      const ty = (this.getAttribute('type') || '').toLowerCase();
      const clean = _ns_sanitizeDateValue(ty, String(v));
      if (clean === '') _ns_dateDraft.set(this, String(v)); else _ns_dateDraft.delete(this);
      _formValues[this._nid] = clean;
      // The control paints the sanitized text; the dirty store above means
      // the attribute default stops applying, same as Chrome's value flag.
      _domRaw("set_live_value", String(this._nid), clean);
      // Caret follows the RAW (draft) length so the typing funnels keep
      // appending at the composition end — this branch returns before the
      // generic text-entry reset below.
      const rawLen = String(v).length;
      _ns_selectionStart.set(this, rawLen);
      _ns_selectionEnd.set(this, rawLen);
      _ns_mirrorSelection(this);
      return;
    }
    _formValues[this._nid] = String(v);
    // Mirror the dirty value into the Rust tree (NodeData::Element::
    // live_value) so paint can draw what the control shows. Kept out of the
    // value ATTRIBUTE on purpose — Chrome's el.value=x never touches
    // getAttribute('value')/outerHTML, and this preserves that.
    _domRaw("set_live_value", String(this._nid), String(v));
    // Chrome: assigning value moves the selection to the end (any prior
    // range is dropped). insertText/Backspace write through here first and
    // then setSelectionRange their own caret, so the reset ordering works
    // out naturally.
    if (_ns_isTextEntry(this)) {
      const len = String(v).length;
      _ns_selectionStart.set(this, len);
      _ns_selectionEnd.set(this, len);
      _ns_mirrorSelection(this);
    }
    // NOTE: no textarea textContent write here. Chrome's el.value=x never
    // touches the child text (defaultValue = child text; the dirty value
    // stays out of outerHTML/cloneNode), and paint now reads live_value.
    // Writing textContent also poisoned form.reset(): the "default" it
    // restored had become the dirty value itself.
  }
  // File upload state lives in _fileInputs keyed by the stable node id — the
  // same outlives-the-wrapper pattern as _formValues. `files` is null on
  // non-file inputs and a FileList-like (length/item/indexed/iterator) on
  // file inputs.
  get files() {
    if (this.localName !== 'input' || this._inputType() !== 'file') return null;
    return _makeFileList(_fileInputs[this._nid] || []);
  }
  set files(v) {
    if (this.localName !== 'input' || this._inputType() !== 'file') return;
    // Chrome-shaped tolerance: the setter only accepts sequences of File and
    // silently ignores anything else (polyfills that reassign a DataTransfer
    // fileList must not explode). null/undefined clears the selection.
    if (v == null) { delete _fileInputs[this._nid]; return; }
    // (#57) A bare File is the most common page-level injection typo
    // (`input.files = file`) — accept it as a one-entry selection instead
    // of a silent no-op, which cost the xhs send batch a debug cycle.
    if (typeof File === 'function' && v instanceof File) { _fileInputs[this._nid] = [v]; return; }
    if (typeof v !== 'object' || typeof v.length !== 'number') return;
    const out = [];
    for (let i = 0; i < v.length; i++) {
      if (typeof File === 'function' && v[i] instanceof File) out.push(v[i]);
    }
    _fileInputs[this._nid] = out;
  }
  get min() { return this.getAttribute('min') || ''; }
  set min(v) { this.setAttribute('min', v); }
  get max() { return this.getAttribute('max') || ''; }
  set max(v) { this.setAttribute('max', v); }
  get step() { return this.getAttribute('step') || ''; }
  set step(v) { this.setAttribute('step', v); }
  _inputType() { return this.localName === 'input' ? (this.getAttribute('type') || 'text').toLowerCase() : ''; }
  get valueAsNumber() {
    const t = this._inputType();
    if (!_INPUT_NUM_TYPES[t]) return NaN;
    if (t === 'range') {
      let minN = _inputParseNumber('range', this.getAttribute('min')); if (isNaN(minN)) minN = 0;
      let maxN = _inputParseNumber('range', this.getAttribute('max')); if (isNaN(maxN)) maxN = 100;
      if (maxN < minN) maxN = minN;
      const v = _inputParseNumber('range', this.value);
      let n = isNaN(v) ? (minN + (maxN - minN) / 2) : v;
      if (n < minN) n = minN; if (n > maxN) n = maxN;
      return n;
    }
    return _inputParseNumber(t, this.value);
  }
  set valueAsNumber(n) {
    const t = this._inputType();
    if (!_INPUT_NUM_TYPES[t]) throw new DOMException("Failed to set the 'valueAsNumber' property on 'HTMLInputElement': This input element does not support Number values.", 'InvalidStateError');
    n = Number(n);
    if (isNaN(n)) { this.value = ''; return; }
    if (!isFinite(n)) throw new TypeError("Failed to set the 'valueAsNumber' property on 'HTMLInputElement': The value provided is infinite.");
    this.value = _inputFormatNumber(t, n);
  }
  get valueAsDate() {
    const t = this._inputType();
    if (!_INPUT_DATE_TYPES[t]) return null;
    const n = _inputParseNumber(t, this.value);
    if (isNaN(n)) return null;
    if (t === 'month') { const y = 1970 + Math.floor(n / 12); const mo = ((n % 12) + 12) % 12; return new Date(Date.UTC(y, mo, 1)); }
    return new Date(n);
  }
  set valueAsDate(d) {
    const t = this._inputType();
    if (!_INPUT_DATE_TYPES[t]) throw new DOMException("Failed to set the 'valueAsDate' property on 'HTMLInputElement': This input element does not support Date values.", 'InvalidStateError');
    if (d === null) { this.value = ''; return; }
    if (!(d instanceof Date)) throw new TypeError("Failed to set the 'valueAsDate' property on 'HTMLInputElement': The provided value is not a Date.");
    const ms = d.getTime();
    if (isNaN(ms)) { this.value = ''; return; }
    if (t === 'month') { this.value = _inputFormatNumber('month', (d.getUTCFullYear() - 1970) * 12 + d.getUTCMonth()); return; }
    this.value = _inputFormatNumber(t, ms);
  }
  stepUp(n) { this._stepBy(n === undefined ? 1 : (n | 0)); }
  stepDown(n) { this._stepBy(-(n === undefined ? 1 : (n | 0))); }
  _stepBy(delta) {
    const t = this._inputType();
    const stepAttr = this.getAttribute('step');
    if (!_INPUT_STEP_SCALE[t] || (stepAttr && stepAttr.trim().toLowerCase() === 'any')) {
      throw new DOMException("Failed to execute 'stepUp' on 'HTMLInputElement': This form element does not have allowed value steps.", 'InvalidStateError');
    }
    const scale = _INPUT_STEP_SCALE[t];
    let stepN = _INPUT_STEP_DEFAULT[t];
    if (stepAttr) { const s = Number(stepAttr); if (isFinite(s) && s > 0) stepN = s; }
    const allowed = stepN * scale;
    const minN = _inputParseNumber(t, this.getAttribute('min'));
    const maxN = _inputParseNumber(t, this.getAttribute('max'));
    const stepBase = isNaN(minN) ? 0 : minN;
    let value = this.valueAsNumber;
    if (isNaN(value)) value = isNaN(minN) ? 0 : minN;
    value += delta * allowed;
    value = stepBase + Math.round((value - stepBase) / allowed) * allowed;
    const effMin = (t === 'range' && isNaN(minN)) ? 0 : minN;
    const effMax = (t === 'range' && isNaN(maxN)) ? 100 : maxN;
    if (!isNaN(effMin) && value < effMin) value = effMin;
    if (!isNaN(effMax) && value > effMax) value = effMax;
    this.value = _inputFormatNumber(t, value);
  }
  get checked() {
    if (_formChecked[this._nid] !== undefined) return _formChecked[this._nid];
    return this.hasAttribute("checked");
  }
  set checked(v) {
    _formChecked[this._nid] = !!v;
    // Mirror the dirty checkedness into the Rust tree (NodeData::Element::
    // live_checked) so the widget paint draws the state. Same posture as
    // set_live_value: getAttribute/outerHTML keep showing only the parsed
    // `checked` attribute, like Chrome.
    _domRaw("set_live_checked", String(this._nid), v ? "1" : "0");
  }
  // Real IDL property (not an expando): `in` checks, Object.keys enumeration,
  // and prototype introspection must all see it, and the click activation
  // steps clear/restore it as real state.
  get indeterminate() { return !!_formIndeterminate[this._nid]; }
  set indeterminate(v) { _formIndeterminate[this._nid] = !!v; _domRaw("set_live_indeterminate", String(this._nid), v ? "1" : "0"); }
  get selected() {
    if (this._selected !== undefined) return this._selected;
    return this.hasAttribute("selected");
  }
  set selected(v) {
    // Selecting an option deselects its siblings in the same select (Chrome:
    // option.selected = true moves select.value). glama.ai's admin form sets
    // selected on a freshly appended <option>; without sibling clearing the
    // old default-selected option kept winning every value/selectedIndex read.
    // Chrome's property setter never touches the `selected` ATTRIBUTE: the
    // attribute stays the parsed default (outerHTML keeps it, and reset()
    // restores from it); dirtiness lives only in _selected, which the getter
    // prefers. Stripping the attribute made reset() irreversible — the parsed
    // default was destroyed by the first assignment.
    v = !!v;
    let p = this.parentNode;
    while (p && p.localName !== 'select') p = p.parentNode;
    if (v && p) {
      const siblings = p.querySelectorAll('option');
      for (let i = 0; i < siblings.length; i++) {
        if (siblings[i] !== this) siblings[i]._selected = false;
      }
    }
    __def(this, '_selected', v);
    // The displayed label may have moved — recompute the owning select's
    // mirror once, covering both this write and the sibling clears above.
    if (p) _mirrorSelectLabel(p);
  }
  get disabled() { return this.hasAttribute("disabled"); }
  set disabled(v) { if (v) this.setAttribute("disabled", ""); else this.removeAttribute("disabled"); }
  get type() {
    // select and textarea report fixed IDL types, not the content attribute.
    // jQuery's select valHook branches on type === "select-one" to decide
    // scalar vs array .val(); "" here made every single select read as an
    // array, so value comparisons against strings never matched.
    if (this.localName === "select") return this.hasAttribute("multiple") ? "select-multiple" : "select-one";
    if (this.localName === "textarea") return "textarea";
    return this.getAttribute("type") || (this.localName === "input" ? "text" : "");
  }
  set type(v) { this.setAttribute("type", v); }
  get name() { return this.getAttribute("name") || ""; }
  set name(v) { this.setAttribute("name", v); }
  get placeholder() { return this.getAttribute("placeholder") || ""; }
  set placeholder(v) { this.setAttribute("placeholder", v); }
  // For <a>/<area>, href returns the resolved absolute URL (the spec behavior,
  // and what scrapers want). It uses op_url_resolve, which returns just the
  // resolved string, rather than the full-component op the decomposition
  // members use. Other elements reflect the raw attribute.
  get href() {
    const ln = this.localName;
    if (ln === 'a' || ln === 'area' || ln === 'link' || ln === 'base') {
      const raw = this.getAttribute('href');
      if (raw === null) return '';
      // Legacy-charset document: href must reflect the encoding-override query.
      if (!_docIsUtf8()) { const u = _elemHrefURL(this); return u ? u.href : raw; }
      const r = _urlResolveOp(raw, _anchorBase());
      return r !== null ? r : raw;
    }
    return this.getAttribute("href") || "";
  }
  set href(v) { this.setAttribute("href", v); }
  // HTMLHyperlinkElementUtils URL-decomposition members, live on <a>/<area>.
  get protocol() { const u = (this.localName === 'a' || this.localName === 'area') ? _elemHrefURL(this) : null; return u ? u.protocol : ''; }
  set protocol(v) { if (this.localName === 'a' || this.localName === 'area') _setElemHrefPart(this, 'protocol', v); }
  get username() { const u = (this.localName === 'a' || this.localName === 'area') ? _elemHrefURL(this) : null; return u ? u.username : ''; }
  set username(v) { if (this.localName === 'a' || this.localName === 'area') _setElemHrefPart(this, 'username', v); }
  get password() { const u = (this.localName === 'a' || this.localName === 'area') ? _elemHrefURL(this) : null; return u ? u.password : ''; }
  set password(v) { if (this.localName === 'a' || this.localName === 'area') _setElemHrefPart(this, 'password', v); }
  get host() { const u = (this.localName === 'a' || this.localName === 'area') ? _elemHrefURL(this) : null; return u ? u.host : ''; }
  set host(v) { if (this.localName === 'a' || this.localName === 'area') _setElemHrefPart(this, 'host', v); }
  get hostname() { const u = (this.localName === 'a' || this.localName === 'area') ? _elemHrefURL(this) : null; return u ? u.hostname : ''; }
  set hostname(v) { if (this.localName === 'a' || this.localName === 'area') _setElemHrefPart(this, 'hostname', v); }
  get port() { const u = (this.localName === 'a' || this.localName === 'area') ? _elemHrefURL(this) : null; return u ? u.port : ''; }
  set port(v) { if (this.localName === 'a' || this.localName === 'area') _setElemHrefPart(this, 'port', v); }
  get pathname() { const u = (this.localName === 'a' || this.localName === 'area') ? _elemHrefURL(this) : null; return u ? u.pathname : ''; }
  set pathname(v) { if (this.localName === 'a' || this.localName === 'area') _setElemHrefPart(this, 'pathname', v); }
  get search() { const u = (this.localName === 'a' || this.localName === 'area') ? _elemHrefURL(this) : null; return u ? u.search : ''; }
  set search(v) { if (this.localName === 'a' || this.localName === 'area') _setElemHrefPart(this, 'search', v); }
  get hash() { const u = (this.localName === 'a' || this.localName === 'area') ? _elemHrefURL(this) : null; return u ? u.hash : ''; }
  set hash(v) { if (this.localName === 'a' || this.localName === 'area') _setElemHrefPart(this, 'hash', v); }
  get origin() { const u = (this.localName === 'a' || this.localName === 'area') ? _elemHrefURL(this) : null; return u ? u.origin : ''; }
  get src() {
    const raw = this.getAttribute("src");
    // Spec: missing attribute reflects as "". An empty attribute resolves
    // against the document base (so `<img src="">.src` returns the document
    // URL) — don't special-case '' away before resolution.
    if (raw === null) return '';
    // URL-reflection attribute (HTML spec): return the resolved absolute URL,
    // not the raw attribute string. Real browsers resolve `img.src`,
    // `script.src`, `iframe.src` etc. against the document base. Next.js /
    // Turbopack's webpack runtime derives its chunk base from
    // `document.currentScript.src` via `new URL(x, base)`; a relative base
    // throws "TypeError: Invalid scheme" and React never hydrates (all
    // delegated event listeners are dead). Resolve like real browsers.
    const r = _urlResolveOp(raw, _anchorBase());
    return r !== null ? r : raw;
  }
  set src(v) {
    this.setAttribute("src", v);
    if (this.localName === 'script') {
      // A connected script gaining its src NOW becomes ready — preparation
      // runs at that moment (the empty-insert path above left the start
      // slot untouched for exactly this arrival).
      if (v && this.isConnected) {
        try { __prepareInsertedScript(this); } catch (e) {}
      }
      return;
    }
    if (this.localName === 'iframe' && v && v !== 'about:blank') {
      this._loadIframeSrc(v);
    }
    // MSE attach: setting a media element's src to a MediaSource blob URL
    // flips the source 'open' and fires 'sourceopen' asynchronously (see
    // the MediaSource stub near canPlayType).
    if ((this.localName === 'video' || this.localName === 'audio')
        && typeof v === 'string' && v.startsWith('blob:')
        && globalThis.__mediaSources && globalThis.__mediaSources[v]) {
      globalThis.__mediaSources[v]._attachToElement(this);
    }
  }
  get srcdoc() {
    // Reflected attribute: absent reflects as "" (HTML spec).
    return this.getAttribute("srcdoc") ?? '';
  }
  set srcdoc(v) {
    // Null/undefined removes the attribute (nullable reflected string) —
    // the frame then falls back to src navigation like browsers.
    if (v === null || v === undefined) { this.removeAttribute("srcdoc"); return; }
    this.setAttribute("srcdoc", v);
  }
  _loadIframeSrc(url) {
    // Loads can be kicked off from three places — the src property setter,
    // the post-navigation scan, and the document.write insertion scan — and
    // more than one can see the same element. The flag collapses them to
    // one fetch.
    if (this._iframeLoading) return;
    this._iframeLoading = true;
    // srcdoc wins over src (HTML §the-iframe-element): the child document
    // comes from the attribute, the browsing context URL is about:srcdoc,
    // and the document inherits the parent's origin (same-origin).
    const srcdoc = this.getAttribute('srcdoc');
    if (srcdoc !== null) {
      const el = this;
      try {
        el._iframeDoc = new _IframeDocument(srcdoc, 'about:srcdoc', el);
        el._iframeWin = new _IframeWindow(el._iframeDoc, 'about:srcdoc');
        _registerIframe(el);
        _runSrcdocScripts(el);
      } catch (e) {
        globalThis.__diting_reportUncaughtError?.(e);
      }
      return;
    }
    let fullUrl = url;
    if (!url.includes('://')) {
      try { fullUrl = new URL(url, _docBase()).href; } catch(e) {}
    }
    const el = this;
    // credentials:'include' — an iframe load is a NAVIGATION of the child
    // frame, not a subresource fetch: the frame's own origin's cookies ride
    // (same-site frames like taobao's detailDescPreview get the login jar).
    // The fetch() default 'same-origin' sends nothing cross-origin, so the
    // server answers as a guest and its Set-Cookie mint lands on the shared
    // domain over the login state (#203: editor page's preview iframe was
    // silently logging the session out; forcing include — what a real
    // browser does by navigation — held the login for hours).
    fetch(fullUrl, {mode: 'no-cors', credentials: 'include'}).then(async resp => {
      if (resp.ok || resp.type === 'opaque') {
        const html = await resp.text();
        el._iframeDoc = new _IframeDocument(html, fullUrl, el);
        el._iframeWin = new _IframeWindow(el._iframeDoc, fullUrl);
      } else {
        el._iframeDoc = new _IframeDocument('<!DOCTYPE html><html><head></head><body></body></html>', fullUrl, el);
        el._iframeWin = new _IframeWindow(el._iframeDoc, fullUrl);
      }
      _registerIframe(el);
      // Dispatch through the element so the onload property/attribute and any
      // addEventListener('load', ...) listeners all run. Calling el.onload()
      // directly bypasses listeners registered via addEventListener (#478).
      el.dispatchEvent(new Event('load'));
    }).catch(() => {
      el._iframeDoc = new _IframeDocument('<!DOCTYPE html><html><head></head><body></body></html>', fullUrl, el);
      el._iframeWin = new _IframeWindow(el._iframeDoc, fullUrl);
      _registerIframe(el);
      el.dispatchEvent(new Event('load'));
    });
  }
  get contentDocument() {
    if (this.localName !== 'iframe') return undefined;
    // A browsing context only exists while the iframe is connected —
    // never-connected and removed elements both read null (Chrome truth
    // 10-18; moli ba5cd6f deleted the same synthetic surfaces).
    if (!this.isConnected) return null;
    if (this._iframeDoc) {
      // A srcdoc document inherits the parent's origin: always same-origin.
      const pageOrigin = (function(){ try { return new URL(_domParse("document_url")).origin; } catch(e) { return ''; } })();
      const iframeOrigin = (function(url){ try { return new URL(url).origin; } catch(e) { return ''; } })(this.src);
      if (this.hasAttribute('srcdoc') || pageOrigin === iframeOrigin || this.src === '' || this.src === 'about:blank' || !this.src.includes('://')) {
        return this._iframeDoc;
      }
      return null; // Cross-origin: blocked
    }
    if (!this._iframeDoc) {
      // Lazy creation before any load: derive the browsing context's URL
      // from the src attribute when present (browsers do this at context
      // creation), so contentWindow.location.origin and postMessage
      // targetOrigin checks see the authored origin, not about:blank.
      const lazyUrl = this.hasAttribute('srcdoc') ? 'about:srcdoc'
        : ((this.src && this.src.includes('://')) ? this.src : 'about:blank');
      __def(this, '_iframeDoc', new _IframeDocument('<!DOCTYPE html><html><head></head><body></body></html>', lazyUrl, this));
      __def(this, '_iframeWin', new _IframeWindow(this._iframeDoc, lazyUrl));
    }
    return this._iframeDoc;
  }
  get contentWindow() {
    if (this.localName !== 'iframe') return undefined;
    if (!this.isConnected) return null; // same gate as contentDocument
    if (!this._iframeWin) {
      this.contentDocument; // side effect: creates _iframeDoc + _iframeWin
    }
    return this._iframeWin;
  }
  get action() {
    const action = this.getAttribute("action") || _docBase();
    try { return new URL(action, _docBase()).href; } catch(e) { return action; }
  }
  set action(v) { this.setAttribute("action", v); }
  get method() { return this.getAttribute("method") || "get"; }
  set method(v) { this.setAttribute("method", v); }
  get form() {
    let p = this.parentNode;
    while (p && p.localName !== 'form') p = p.parentNode;
    return p;
  }
  get options() {
    if (this.localName !== 'select') return [];
    // HTMLOptionsCollection (platform/collections.impl.js): add/remove/selectedIndex/length on the prototype.
    return __diting_mkCollection(this.querySelectorAll('option'), 'options', this);
  }
  // `options` only exists on HTMLSelectElement in Chrome, so assigning it on
  // any other element is a plain own-property expando there. bilibili's
  // player binds component state as `el.options = ...` during mount; with
  // only the shared-prototype getter (see the labels note at _htmlInterface)
  // that assignment threw and killed the whole player init. A select keeps
  // the readonly accessor (silent no-op assignment, spec behavior).
  set options(v) {
    if (this.localName === 'select') return;
    Object.defineProperty(this, 'options', {
      value: v, writable: true, configurable: true, enumerable: true,
    });
  }
  add(item, before = null) {
    if (this.localName !== 'select') {
      throw new TypeError("Illegal invocation");
    }
    if (!item || item.nodeType !== 1
        || (item.localName !== 'option' && item.localName !== 'optgroup')) {
      throw new TypeError("Failed to execute 'add' on 'HTMLSelectElement': parameter 1 is not of type 'HTMLOptionElement' or 'HTMLOptGroupElement'.");
    }
    if (typeof before === 'number') {
      const reference = this.options[before] || null;
      this.insertBefore(item, reference);
    } else if (before == null) {
      this.appendChild(item);
    } else {
      this.insertBefore(item, before);
    }
  }
  get selectedIndex() {
    const opts = this.options;
    for (let i = 0; i < opts.length; i++) {
      // `.selected` already falls back to the parsed attribute when the option
      // was never dirtied; an explicit `_selected = false` must WIN over the
      // attribute (Chrome: dirtiness beats the parsed default), so no `||`
      // hasAttribute resurrection here — that arm undid deselection and kept
      // the old default winning selectedIndex after select.value moved it.
      if (opts[i].selected) return i;
    }
    // Only a single select implicitly selects its first option; a multiple
    // select with nothing chosen idles at -1 like a real browser.
    return opts.length && !this.hasAttribute('multiple') ? 0 : -1;
  }
  set selectedIndex(v) {
    const opts = this.options;
    for (let i = 0; i < opts.length; i++) {
      // Direct _selected writes (not the property setter): the setter's
      // sibling-clearing walk is O(n) per option, and this loop IS the
      // batch form of that walk. The explicit mirror below covers the
      // whole assignment.
      opts[i]._selected = (i === v);
    }
    _mirrorSelectLabel(this);
  }
  // selectedOptions: the multiple-select filtered view (spec §4.10.8).
  get selectedOptions() {
    if (this.localName !== 'select') return undefined;
    return __diting_mkCollection(Array.from(this.options).filter((o) => o.selected));
  }
  get multiple() {
    if (this.localName !== 'select') return undefined;
    return this.hasAttribute('multiple');
  }
  set multiple(v) {
    if (this.localName !== 'select') return;
    if (v) this.setAttribute('multiple', '');
    else this.removeAttribute('multiple');
  }
  get size() {
    // input.size: reflected unsigned long limited to only positive numbers,
    // default 20 (Chrome 152 probed). Parse is the HTML non-negative
    // integer grammar (leading ws/+ tolerated, stops at the first
    // non-digit); 0, parse-fail and >= 2^31 all fall back to the default.
    if (this.localName === 'input') {
      const v = this.getAttribute('size');
      if (v !== null) {
        const m = /^\s*\+?(\d+)/.exec(v);
        if (m !== null) {
          const p = +m[1];
          if (p >= 1 && p <= 2147483647) return p;
        }
      }
      return 20;
    }
    if (this.localName !== 'select') return undefined;
    const n = parseInt(this.getAttribute('size'), 10);
    return Number.isFinite(n) && n > 0 ? n : (this.hasAttribute('multiple') ? 4 : 1);
  }
  set size(v) {
    // Chrome's input.size setter THROWS IndexSizeError on a ToUint32 of 0
    // ("which is an invalid size") instead of writing; values above
    // 2^31-1 after the mod-2^32 wrap write the default string, smaller
    // wrapped values write verbatim (1e10 -> "1410065408").
    if (this.localName === 'input') {
      const n = Number(v) >>> 0;
      if (n === 0) {
        throw new DOMException(
          "Failed to set the 'size' property on 'HTMLInputElement': The value provided is 0, which is an invalid size.",
          'IndexSizeError');
      }
      this.setAttribute('size', String(n > 2147483647 ? 20 : n));
      return;
    }
    if (this.localName !== 'select') return;
    if (v != null && +v > 0) this.setAttribute('size', String(+v));
  }
  // (#36) maxLength/minLength: [ReflectNonNegative] long, missing-value
  // default -1 on both input and textarea (Chrome 152 probed — MDN's 0 is
  // stale). Getter takes the HTML signed-integer parse and falls back on
  // anything negative or above 2^31-1; the setter is ToInt32 and THROWS
  // IndexSizeError on negatives — including -1, the very value that is the
  // getter's default.
  get maxLength() {
    const t = this.localName;
    if (t !== 'input' && t !== 'textarea') return undefined;
    const v = this.getAttribute('maxlength');
    if (v !== null) {
      const m = /^\s*([+-]?\d+)/.exec(v);
      if (m !== null) {
        const p = +m[1];
        if (p >= 0 && p <= 2147483647) return p;
      }
    }
    return -1;
  }
  set maxLength(v) {
    const t = this.localName;
    if (t !== 'input' && t !== 'textarea') return;
    const n = Number(v) | 0;
    if (n < 0) {
      throw new DOMException(
        "Failed to set the 'maxLength' property on '" + this.constructor.name + "': The value provided (" + n + ") is not positive or 0.",
        'IndexSizeError');
    }
    this.setAttribute('maxlength', String(n));
  }
  get minLength() {
    const t = this.localName;
    if (t !== 'input' && t !== 'textarea') return undefined;
    const v = this.getAttribute('minlength');
    if (v !== null) {
      const m = /^\s*([+-]?\d+)/.exec(v);
      if (m !== null) {
        const p = +m[1];
        if (p >= 0 && p <= 2147483647) return p;
      }
    }
    return -1;
  }
  set minLength(v) {
    const t = this.localName;
    if (t !== 'input' && t !== 'textarea') return;
    const n = Number(v) | 0;
    if (n < 0) {
      throw new DOMException(
        "Failed to set the 'minLength' property on '" + this.constructor.name + "': The value provided (" + n + ") is not positive or 0.",
        'IndexSizeError');
    }
    this.setAttribute('minlength', String(n));
  }
  // (#36) textarea.rows/cols: [ReflectPositiveWithFallback] unsigned long —
  // same positive-parse getter as input.size (defaults 2 / 20), but the
  // setter never throws: a ToUint32 of 0 or anything above 2^31-1 writes
  // the DEFAULT string ("2"/"20"), not the number. The rows GETTER lives
  // with the table family below (duplicate object-literal keys would
  // silently overwrite each other); this is just the setter.
  set rows(v) {
    if (this.localName !== 'textarea') return;
    const n = Number(v) >>> 0;
    this.setAttribute('rows', String(n === 0 || n > 2147483647 ? 2 : n));
  }
  get cols() {
    if (this.localName !== 'textarea') return undefined;
    const v = this.getAttribute('cols');
    if (v !== null) {
      const m = /^\s*\+?(\d+)/.exec(v);
      if (m !== null) {
        const p = +m[1];
        if (p >= 1 && p <= 2147483647) return p;
      }
    }
    return 20;
  }
  set cols(v) {
    if (this.localName !== 'textarea') return;
    const n = Number(v) >>> 0;
    this.setAttribute('cols', String(n === 0 || n > 2147483647 ? 20 : n));
  }
  // (#36) textarea.wrap is a PLAIN [Reflect] DOMString — no keyword
  // folding: the getter returns the attribute verbatim ("" when absent,
  // "HARD" stays "HARD") and the setter is String(v) with no validation.
  get wrap() {
    if (this.localName !== 'textarea') return undefined;
    return this.getAttribute('wrap') || '';
  }
  set wrap(v) {
    if (this.localName !== 'textarea') return;
    this.setAttribute('wrap', String(v));
  }
  // ---- Table DOM API family (WHATWG §4.9) — the classes were bare interface
  // aliases, so script-built tables (Sina's TabSwitchController: insertRow /
  // insertCell; anything calling new-section accessors) threw "is not a
  // function". Shared-prototype accessors with localName guards, same shape
  // as the select family above. `rows` is spec order (thead, then each tbody
  // in tree order, then tfoot), NOT tree order.
  get rows() {
    // (#36) textarea.rows is a reflected positive long (default 2) — it
    // shares this key with the table rows collection, so it dispatches
    // first; the setter next to the cols family above writes the default
    // string instead of throwing.
    if (this.localName === 'textarea') {
      const v = this.getAttribute('rows');
      if (v !== null) {
        const m = /^\s*\+?(\d+)/.exec(v);
        if (m !== null) {
          const p = +m[1];
          if (p >= 1 && p <= 2147483647) return p;
        }
      }
      return 2;
    }
    const t = this.localName;
    if (t === 'table') {
      const out = [];
      for (const name of ['thead', 'tbody', 'tfoot']) {
        for (const s of Array.from(this.children)) {
          if (s.localName === name) out.push(...Array.from(s.children));
        }
      }
      return __diting_mkCollection(out);
    }
    if (t === 'thead' || t === 'tbody' || t === 'tfoot') {
      return __diting_mkCollection(Array.from(this.children).filter((c) => c.localName === 'tr'));
    }
    return undefined;
  }
  get tBodies() {
    if (this.localName !== 'table') return undefined;
    return __diting_mkCollection(Array.from(this.children).filter((c) => c.localName === 'tbody'));
  }
  get tHead() {
    if (this.localName !== 'table') return undefined;
    return Array.from(this.children).find((c) => c.localName === 'thead') || null;
  }
  get tFoot() {
    if (this.localName !== 'table') return undefined;
    return Array.from(this.children).find((c) => c.localName === 'tfoot') || null;
  }
  get caption() {
    if (this.localName !== 'table') return undefined;
    return Array.from(this.children).find((c) => c.localName === 'caption') || null;
  }
  // (刀76) caption/tHead/tFoot are rw in Chrome: the setter swaps the element in (null removes it);
  // a wrong element type is a TypeError (caption) / HierarchyRequestError (tHead, tFoot)
  set caption(v) {
    if (this.localName !== 'table') return;
    if (v !== null && !(v && v.localName === 'caption')) throw new TypeError("Failed to set the 'caption' property on 'HTMLTableElement': The provided value is not of type 'HTMLTableCaptionElement'.");
    const old = this.caption; if (old) old.remove();
    if (v) this.insertBefore(v, this.firstChild);
  }
  set tHead(v) {
    if (this.localName !== 'table') return;
    if (v !== null && !(v && v.localName === 'thead')) throw new DOMException("Failed to set the 'tHead' property on 'HTMLTableElement': Not a thead element.", 'HierarchyRequestError');
    const old = this.tHead; if (old) old.remove();
    if (v) { const ref = Array.from(this.children).find((c) => c.localName !== 'caption' && c.localName !== 'colgroup'); this.insertBefore(v, ref || null); }
  }
  set tFoot(v) {
    if (this.localName !== 'table') return;
    if (v !== null && !(v && v.localName === 'tfoot')) throw new DOMException("Failed to set the 'tFoot' property on 'HTMLTableElement': Not a tfoot element.", 'HierarchyRequestError');
    const old = this.tFoot; if (old) old.remove();
    if (v) this.appendChild(v);
  }
  createTHead() {
    if (this.localName !== 'table') return undefined;
    const head = this.tHead;
    if (head) return head;
    const created = document.createElement('thead');
    // caption/colgroup stay ahead of the thead, matching Chrome placement.
    const firstRowish = Array.from(this.children).find(
      (c) => c.localName !== 'caption' && c.localName !== 'colgroup'
    );
    this.insertBefore(created, firstRowish || null);
    return created;
  }
  createTBody() {
    if (this.localName !== 'table') return undefined;
    const body = document.createElement('tbody');
    this.appendChild(body);
    return body;
  }
  createTFoot() {
    if (this.localName !== 'table') return undefined;
    const foot = this.tFoot;
    if (foot) return foot;
    const created = document.createElement('tfoot');
    this.appendChild(created);
    return created;
  }
  createCaption() {
    if (this.localName !== 'table') return undefined;
    const cap = this.caption;
    if (cap) return cap;
    const created = document.createElement('caption');
    this.insertBefore(created, this.firstChild);
    return created;
  }
  deleteTHead() { if (this.localName === 'table') { const h = this.tHead; if (h) h.remove(); } }
  deleteTFoot() { if (this.localName === 'table') { const f = this.tFoot; if (f) f.remove(); } }
  deleteCaption() { if (this.localName === 'table') { const c = this.caption; if (c) c.remove(); } }
  insertRow(index) {
    const t = this.localName;
    const rows = t === 'table' || t === 'thead' || t === 'tbody' || t === 'tfoot'
      ? Array.from(this.rows) : null;
    if (!rows) return undefined;
    index = index == null || Number.isNaN(+index) ? -1 : Math.trunc(+index);
    const owner = t === 'table' ? 'HTMLTableElement' : 'HTMLTableSectionElement';
    if (index < -1 || index > rows.length) {
      throw new DOMException(
        "Failed to execute 'insertRow' on '" + owner + "': The index provided (" + index + ") is out of range.",
        'IndexSizeError'
      );
    }
    const tr = document.createElement('tr');
    if (index === -1 || index === rows.length) {
      if (t === 'table') {
        // Append into the last tbody, creating one when the table has none.
        let body = null;
        for (const c of this.children) { if (c.localName === 'tbody') body = c; }
        if (!body) { body = document.createElement('tbody'); this.appendChild(body); }
        body.appendChild(tr);
      } else {
        this.appendChild(tr);
      }
    } else {
      const ref = rows[index];
      // Intermediate index: the new row belongs to the section OWNING that
      // row, never as a direct child of the table.
      ref.parentNode.insertBefore(tr, ref);
    }
    return tr;
  }
  deleteRow(index) {
    const t = this.localName;
    if (t !== 'table' && t !== 'thead' && t !== 'tbody' && t !== 'tfoot') return;
    index = index == null || Number.isNaN(+index) ? -1 : Math.trunc(+index);
    const rows = Array.from(this.rows);
    const row = index === -1 ? rows[rows.length - 1] : rows[index];
    const owner = t === 'table' ? 'HTMLTableElement' : 'HTMLTableSectionElement';
    if (!row) {
      throw new DOMException(
        "Failed to execute 'deleteRow' on '" + owner + "': The index provided (" + index + ") is out of range.",
        'IndexSizeError'
      );
    }
    row.remove();
  }
  get cells() {
    if (this.localName !== 'tr') return undefined;
    return __diting_mkCollection(Array.from(this.children).filter(
      (c) => c.localName === 'td' || c.localName === 'th'
    ));
  }
  get rowIndex() {
    if (this.localName !== 'tr') return -1;
    const table = typeof this.closest === 'function' ? this.closest('table') : null;
    if (!table) return -1;
    return Array.from(table.rows).indexOf(this);
  }
  get sectionRowIndex() {
    if (this.localName !== 'tr') return -1;
    if (!this.parentNode || !this.parentNode.children) return -1;
    return Array.from(this.parentNode.children)
      .filter((c) => c.localName === 'tr')
      .indexOf(this);
  }
  insertCell(index) {
    if (this.localName !== 'tr') return undefined;
    index = index == null || Number.isNaN(+index) ? -1 : Math.trunc(+index);
    const cells = Array.from(this.cells);
    if (index < -1 || index > cells.length) {
      throw new DOMException(
        "Failed to execute 'insertCell' on 'HTMLTableRowElement': The index provided (" + index + ") is out of range.",
        'IndexSizeError'
      );
    }
    const cell = document.createElement('td');
    if (index === -1 || index === cells.length) this.appendChild(cell);
    else this.insertBefore(cell, cells[index]);
    return cell;
  }
  deleteCell(index) {
    if (this.localName !== 'tr') return;
    index = index == null || Number.isNaN(+index) ? -1 : Math.trunc(+index);
    const cells = Array.from(this.cells);
    const cell = index === -1 ? cells[cells.length - 1] : cells[index];
    if (!cell) {
      throw new DOMException(
        "Failed to execute 'deleteCell' on 'HTMLTableRowElement': The index provided (" + index + ") is out of range.",
        'IndexSizeError'
      );
    }
    cell.remove();
  }
  get cellIndex() {
    if (this.localName !== 'td' && this.localName !== 'th') return -1;
    if (!this.parentNode || !this.parentNode.cells) return -1;
    return Array.from(this.parentNode.cells).indexOf(this);
  }
  // Per the HTML spec, the submit() METHOD submits the form WITHOUT firing a
  // cancelable `submit` event — a page's submit listener cannot veto it. Only
  // requestSubmit() and user-initiated submits fire the cancelable event.
  // Conflating the two broke sites whose submit listener preventDefault()s the
  // native submit and then calls form.submit() from a callback (e.g. an
  // invisible-reCAPTCHA data-callback) to actually send the form.
  submit(submitter) {
    this._navigateSubmit(submitter);
  }
  requestSubmit(submitter) {
    // Per spec, a given submitter must be a submit button owned by this form;
    // both checks run before the submit event fires. A missing/null submitter
    // means "submit from the form itself".
    if (submitter !== undefined && submitter !== null) {
      if (!_isSubmitButton(submitter)) {
        throw new TypeError(
          "Failed to execute 'requestSubmit' on 'HTMLFormElement': The specified element is not a submit button."
        );
      }
      if (submitter.form !== this) {
        throw new DOMException(
          "Failed to execute 'requestSubmit' on 'HTMLFormElement': The specified element is not owned by this form element.",
          'NotFoundError'
        );
      }
    }
    if (!globalThis.__interactiveValidate(this, submitter)) return;
    // the platform SubmitEvent carries the submitter; UA-originated, so trusted
    const sev = new SubmitEvent('submit', { bubbles: true, cancelable: true, submitter: submitter === undefined ? null : submitter });
    _eventState(sev, 'isTrusted', true);
    const cancelled = !this.dispatchEvent(sev);
    if (cancelled) return;
    this._navigateSubmit(submitter);
  }
  _navigateSubmit(submitter) {
    const pairs = [];
    const fields = this.querySelectorAll('input, select, textarea');
    for (let i = 0; i < fields.length; i++) {
      const f = fields[i];
      const name = f.getAttribute('name');
      if (!name) continue;
      if (f.getAttribute('disabled') !== null) continue;
      const tag = f.localName;
      const type = (f.getAttribute('type') || '').toLowerCase();
      if ((type === 'checkbox' || type === 'radio') && !f.checked) continue;
      if (type === 'file' || type === 'reset') continue;
      if (type === 'button') continue;
      if (type === 'submit' || tag === 'button') {
        if (submitter && f !== submitter) continue;
        if (!submitter) continue; // default submit: don't include submit button value
      }

      let val;
      if (tag === 'select') {
        const opt = f.querySelector('option[selected]') || f.querySelector('option');
        val = opt ? (opt.getAttribute('value') !== null ? opt.getAttribute('value') : opt.textContent) : '';
      } else if (tag === 'textarea') {
        val = f.value || f.textContent || '';
      } else {
        val = f.value !== undefined ? f.value : (f.getAttribute('value') || '');
      }
      const enc = (s) => encodeURIComponent(s).replace(/%20/g, '+').replace(/!/g, '%21');
      pairs.push(enc(name) + '=' + enc(val));
    }

    const action = this.getAttribute('action') || '';
    const method = (this.getAttribute('method') || 'GET').toUpperCase();
    const baseUrl = globalThis.location?.href || 'about:blank';
    let targetUrl;
    try { targetUrl = new URL(action, baseUrl).href; } catch(e) { targetUrl = action; }

    const encoded = pairs.join('&');
    if (method === 'POST') {
      _OPS.op_navigate(targetUrl, 'POST', encoded);
    } else {
      const sep = targetUrl.includes('?') ? '&' : '?';
      _OPS.op_navigate(targetUrl + (encoded ? sep + encoded : ''), 'GET', '');
    }
  }
  reset() {
    this.dispatchEvent(new Event('reset', { bubbles: true }));
  }
  get dataset() {
    const el = this;
    if (el._dataset) return el._dataset;
    // Pure function over a small key universe (form frameworks reuse the
    // same camelCase keys across a write storm) — the regex chain was 80%+
    // of dataset's per-call cost, so memo it module-level.
    const attrFor = (k) => {
      let a = _attrForMemo.get(k);
      if (a === undefined) {
        a = "data-" + String(k).replace(/([A-Z])/g, "-$1").toLowerCase();
        if (_attrForMemo.size < 4096) _attrForMemo.set(k, a);
      }
      return a;
    };
    const camel = (n) => n.slice(5).replace(/-([a-z])/g, (_, c) => c.toUpperCase());
    const dataKeys = () => (_domParse("attribute_names", el._nid) || [])
      .filter((n) => n.startsWith("data-"))
      .map(camel);
    // Proxy target is a real DOMStringMap so `dataset instanceof DOMStringMap`
    // and the [object DOMStringMap] tag hold; data-* reflection stays dynamic
    // (upstream ec05ed0).
    el._dataset = new Proxy(new DOMStringMap(_domStringMapKey), {
      // getAttribute returning null IS the miss signal — pairing
      // hasAttribute with getAttribute doubled the op count on every read,
      // and dataset reads ride the attribute storm.
      get(target, k, receiver) {
        if (typeof k === "string") {
          const v = el.getAttribute(attrFor(k));
          if (v !== null) return v;
        }
        return Reflect.get(target, k, receiver);
      },
      set(target, k, v, receiver) {
        if (typeof k !== "string") return Reflect.set(target, k, v, receiver);
        el.setAttribute(attrFor(k), String(v));
        return true;
      },
      has(target, k) {
        return (typeof k === "string" && el.getAttribute(attrFor(k)) !== null) || Reflect.has(target, k);
      },
      deleteProperty(target, k) {
        if (typeof k !== "string") return Reflect.deleteProperty(target, k);
        el.removeAttribute(attrFor(k));
        return true;
      },
      ownKeys() { return dataKeys(); },
      getOwnPropertyDescriptor(target, k) {
        if (typeof k === "string") {
          const v = el.getAttribute(attrFor(k));
          if (v !== null) return { value: v, writable: true, enumerable: true, configurable: true };
        }
        return Reflect.getOwnPropertyDescriptor(target, k);
      },
    });
    return el._dataset;
  }
  get offsetWidth() { return this._ditingExtent("w", 100); }
  get offsetHeight() { return this._ditingExtent("h", 20); }
  // Local-first (blitz #663 family): the pre-map box, so a transformed
  // element's offset* stay layout-true instead of tracking its mapped gBCR.
  get offsetTop() { const b = _ditingLocalBox(this); return b ? Math.round(b.y) : 0; }
  get offsetLeft() { const b = _ditingLocalBox(this); return b ? Math.round(b.x) : 0; }
  get offsetParent() { return _ditingOffsetParent(this); }
  // documentElement / body / window expose VIEWPORT geometry, not their own content box.
  // Puppeteer's #clickableBox clips boxes to document.documentElement.clientWidth/Height;
  // returning 100x20 there made every element appear off-screen and broke .click().
  get clientWidth() { return this._ditingExtent("w", 100); }
  get clientHeight() { return this._ditingExtent("h", 20); }
  // CSSOM scroll* = own box + descendant overflow extent (blitz #444): a
  // scroll range that ignores overflowing content reports "nothing to
  // scroll" and whoever reads it stops at the first viewport. html/body
  // clamp up to the viewport natively (Chrome: the scrolling area is never
  // smaller than the window) while client* keep the viewport contract.
  // No layout data (no screenshot feature, display:none, …) → old path.
  get scrollWidth() { return this._ditingScrollExtent("w") ?? this._ditingExtent("w", 100); }
  get scrollHeight() { return this._ditingScrollExtent("h") ?? this._ditingExtent("h", 20); }
  _ditingScrollExtent(axis) {
    try {
      if (this._nid == null) return null;
      const raw = _domRaw("scroll_extent", String(this._nid | 0), "");
      const arr = typeof raw === "string" ? JSON.parse(raw) : raw;
      if (Array.isArray(arr) && arr.length === 2 && Number.isFinite(arr[0])) {
        return Math.max(0, Math.round(axis === "w" ? arr[0] : arr[1]));
      }
    } catch { /* no layout data */ }
    return null;
  }
  // Same taffy-layout source as getBoundingClientRect (without its
  // click-target side effect), so a script measuring a container via
  // offsetWidth (map-lib init) sees the real box instead of the 100x20 stub
  // gBCR contradicted. Deviations from CSSOM, both document-safe:
  // offsetTop/Left are document coordinates (layout is scroll-blind), not
  // offsetParent-relative; client*/scroll* read the border box, not
  // padding/content extents.
  _ditingExtent(axis, fallback) {
    if (this._isViewportRoot()) {
      // A fabricated iframe document's html/body span the IFRAME's viewport
      // (obscura #976), never the host page's — the element's laid-out box,
      // not the 300x150 stub, once host layout exists.
      if (_ditingIframeDoc(this)) {
        const ivp = _ditingIframeViewport(this);
        return axis === "w" ? ivp.w : ivp.h;
      }
      return axis === "w" ? (globalThis.innerWidth || 1280) : (globalThis.innerHeight || 720);
    }
    if (_ditingDisplayNone(this)) return 0;
    // Prefer font-box for inline-styled leaf text. Layout wins for everything
    // else, but taffy ignores named font-family — FingerprintJS/Castle probes
    // set style.fontFamily = "'Menlo',monospace" and compare offsetWidth to the
    // bare generic; if layout always wins, every named face matches baseline
    // and x29/fonts stays []. _ditingFontBox only fires when style._props has
    // font + leaf textContent (the probe shape), so page layout is untouched.
    const m = _ditingFontBox(this);
    if (m) return m[axis];
    const box = _ditingLayoutBox(this);
    if (box) return Math.round(box[axis]);
    return fallback;
  }
  _isViewportRoot() {
    const t = this.tagName;
    return t === 'HTML' || t === 'BODY';
  }
  // Max scrollable offset of the viewport root from the REAL extents, so the
  // root scroller clamps like a real browser: scrollTo(0, 1e9) lands on the
  // last screenful, and a viewport whose overflow propagated to hidden
  // (css-overflow-3 §3.3) pins the range at 0 because the root scrollHeight
  // collapses to the viewport. Only the viewport root clamps — element
  // scrollers keep wrapper-local offsets. Null extent (no layout data,
  // no screenshot feature) keeps the old unclamped path: any synthetic max
  // there would deadlock scroll-driven lazy loaders.
  _rootScrollMax(axis) {
    if (!this._isViewportRoot()) return null;
    const scroller = this._rootScroller();
    const ext = scroller._ditingScrollExtent(axis);
    if (ext == null) return null;
    const win = axis === 'w' ? (globalThis.innerWidth || 1280) : (globalThis.innerHeight || 720);
    return Math.max(0, ext - win);
  }
  // Element-scroller counterparts (sticky v2): a real scroll container
  // clamps to its own scrollable range (scroll extent minus the client
  // box) and mirrors the offset into the native tree, where the shift
  // walks read it — the sticky scrollport, the paint band, and the gBCR
  // shift lookups all key off the same stored offset. Non-containers
  // (and extent-less elements) keep the wrapper-local legacy path.
  _isNativeScrollContainer() {
    try {
      if (this._nid == null) return false;
      return _domRaw('is_scroll_container', String(this._nid | 0), '') === '1';
    } catch (e) { return false; }
  }
  _elemScrollMax(axis) {
    const ext = this._ditingScrollExtent(axis);
    if (ext == null) return null;
    const client = this._ditingExtent(axis, 0);
    return Math.max(0, ext - client);
  }
  _elemScrollMirror(left, top) {
    try {
      if (this._nid == null) return;
      if (_domRaw('is_scroll_container', String(this._nid | 0), '') !== '1') return;
      _domRaw('set_node_scroll', String(this._nid | 0), left + '\0' + top);
    } catch (e) { /* not a container here */ }
  }
  // The offset used to be deliberately unclamped (pre-layout-engine era:
  // any synthetic max was a guess and pinned lazy loaders dead). With real
  // extents the viewport root clamps like a real browser via
  // _rootScrollMax; element scrollers and extent-less docs stay unclamped.
  // We track the offset so scrollTop/scrollLeft round-trip, and fire a
  // scroll event on direct assignment — lazy loaders that set
  // `el.scrollTop = N` rely on that event, and scrollTo/scrollBy below
  // would otherwise be its only source.
  // CSSOM View: in standards mode the body element proxies root scrolling to
  // the document's scrolling element, so body.scrollTop/Left read and write
  // the SAME scroll box as documentElement (two wrappers, one offset).
  _rootScroller() {
    if (this.tagName === 'BODY') {
      const root = globalThis.document && globalThis.document.documentElement;
      if (root && root !== this) return root;
    }
    return this;
  }
  get scrollTop() { return this._rootScroller()._scrollTop || 0; }
  set scrollTop(v) {
    v = +v;
    let nv = Number.isFinite(v) && v > 0 ? v : 0;
    const target = this._rootScroller();
    if (target !== this) { target.scrollTop = nv; return; }
    const max = this._rootScrollMax('h');
    if (max != null) nv = Math.min(nv, max);
    else if (this._isNativeScrollContainer()) {
      // Clamp only REAL element scrollers (the same native predicate the
      // shift walks filter with). A plain div's extent reads 0, so an
      // ungated clamp would pin every legacy wrapper-local write to 0
      // and kill the scroll events lazy loaders key on.
      const emax = this._elemScrollMax('h');
      if (emax != null) nv = Math.min(nv, emax);
    }
    const changed = nv !== (this._scrollTop || 0);
    __def(this, '_scrollTop', nv);
    // Viewport-root mirror: publish the root scroller offset to the native
    // layout state so the CDP frame pump paints the right viewport band.
    // The engine has ONE root scroll box (html/body share the viewport
    // scroll), so both roots write the same offset. Element scrollers
    // (sticky v2) mirror through _elemScrollMirror instead — gated on the
    // same native predicate the shift walks filter with, so a wrapper can
    // never write an offset paint would ignore.
    if (changed && this._isViewportRoot()) {
      try { _domRaw('set_scroll_offset', String(this._scrollLeft || 0), String(nv)); } catch (e) {}
    }
    if (changed && !this._isViewportRoot()) {
      this._elemScrollMirror(String(this._scrollLeft || 0), String(nv));
    }
    if (changed && !this._scrollSuppress) this._fireScroll();
  }
  get scrollLeft() { return this._rootScroller()._scrollLeft || 0; }
  set scrollLeft(v) {
    v = +v;
    let nv = Number.isFinite(v) && v > 0 ? v : 0;
    const target = this._rootScroller();
    if (target !== this) { target.scrollLeft = nv; return; }
    const max = this._rootScrollMax('w');
    if (max != null) nv = Math.min(nv, max);
    else if (this._isNativeScrollContainer()) {
      const emax = this._elemScrollMax('w');
      if (emax != null) nv = Math.min(nv, emax);
    }
    const changed = nv !== (this._scrollLeft || 0);
    __def(this, '_scrollLeft', nv);
    if (changed && this._isViewportRoot()) {
      try { _domRaw('set_scroll_offset', String(nv), String(this._scrollTop || 0)); } catch (e) {}
    }
    if (changed && !this._isViewportRoot()) {
      this._elemScrollMirror(String(nv), String(this._scrollTop || 0));
    }
    if (changed && !this._scrollSuppress) this._fireScroll();
  }
  // Chrome hands back a DOMRect (instanceof, [object DOMRect], accessors on the prototype, no own keys); the layout paths below
  // still build plain literals, which this converts at the single exit.
  getBoundingClientRect() {
    const r = this._boundingRectLiteral();
    return new DOMRect(r.x, r.y, r.width, r.height);
  }
  _boundingRectLiteral() {
    globalThis.__diting_click_target = this;
    // Fabricated iframe documents (obscura #976): elements of a sync-created
    // contentDocument are an orphan subtree — measured by a dedicated
    // sub-run in the IFRAME'S OWN viewport (Chrome semantics, no coordinate
    // stitching into the host page). Must precede the viewport-root check:
    // the iframe doc's own html/body span the iframe's 300x150, not the
    // host page's viewport.
    if (this._nid != null && _ditingIframeDoc(this)) {
      // The iframe doc's own html/body span the IFRAME's viewport — the
      // iframe element's laid-out box on the host page (the 300x150 default
      // stands until the host has laid it out) — the same contract the host
      // page's viewport roots serve, one world down.
      const ivp = _ditingIframeViewport(this);
      if (this._isViewportRoot()) {
        return {
          x: 0, y: 0, width: ivp.w, height: ivp.h,
          top: 0, right: ivp.w, bottom: ivp.h, left: 0,
          toJSON() { return this; },
        };
      }
      try {
        const raw = _domRaw("iframe_layout_rect", String(this._nid | 0), ivp.arg);
        const arr = typeof raw === "string" ? JSON.parse(raw) : raw;
        if (Array.isArray(arr) && arr.length === 4 && Number.isFinite(arr[0])) {
          const [x, y, w, h] = arr;
          return {
            x, y, width: w, height: h,
            top: y, right: x + w, bottom: y + h, left: x,
            toJSON() { return this; },
          };
        }
      } catch (e) { /* fall through */ }
    }
    // documentElement and body span the full viewport. Without this every
    // hit test against them clips down to a 100x20 synthetic cell and
    // Document.elementFromPoint can never recurse into their children.
    if (this._isViewportRoot()) {
      const vw = globalThis.innerWidth || 1280;
      const vh = globalThis.innerHeight || 720;
      return {
        x: 0, y: 0, width: vw, height: vh,
        top: 0, right: vw, bottom: vh, left: 0,
        toJSON() { return this; },
      };
    }
    // Real layout first: the diting_css + diting_layout pipeline computes
    // actual geometry for this tree (memoized per tree epoch in the op
    // layer). Falls back to the synthetic grid when the layout feature is
    // compiled out or the element has no box (display:none subtree, etc.).
    if (this._nid != null) {
      try {
        const raw = _domRaw("layout_rect", String(this._nid | 0), "");
        const arr = typeof raw === "string" ? JSON.parse(raw) : raw;
        if (Array.isArray(arr) && arr.length === 4 && Number.isFinite(arr[0])) {
          // layout_rect is document-space; CSSOM View getBoundingClientRect
          // is viewport-relative. Subtract the root scroll so gBCR and
          // elementFromPoint (client coords) share one world after any
          // scroll — the click recheck feeds gBCR straight into
          // elementFromPoint and mis-verdicts "outside the viewport" the
          // moment scrollY != 0 (tmall publish: SKU button below the fold).
          // Viewport roots above already answer client-space; iframe
          // sub-worlds return before this shift.
          let [x, y, w, h] = arr;
          const root = globalThis.document && globalThis.document.documentElement;
          x -= (root && root._scrollLeft) || 0;
          y -= (root && root._scrollTop) || 0;
          return {
            x, y, width: w, height: h,
            top: y, right: x + w, bottom: y + h, left: x,
            toJSON() { return this; },
          };
        }
      } catch (e) { /* fall through to synthetic */ }
    }
    // No layout engine, but Playwright's actionability polling needs each
    // element to occupy a stable, distinct rect so hit-testing can pick the
    // right one (issue #45). Synthesize a deterministic position from the
    // node id: every nid maps to a unique cell in a 12-column grid, sized
    // to fit a 1280x720 viewport. Stable across reads, different per node.
    const VW = 1280, VH = 720, COLS = 12, CW = 100, CH = 20, GX = 110, GY = 30;
    const rowsPerScreen = Math.max(1, Math.floor((VH - 10) / GY));
    const cell = this._nid | 0;
    const col = ((cell * 7) | 0) % COLS;
    const row = (((cell * 13) | 0) >> 0) % rowsPerScreen;
    const x = 10 + col * GX;
    const y = 10 + row * GY;
    return {
      x, y, width: CW, height: CH,
      top: y, right: x + CW, bottom: y + CH, left: x,
      toJSON() { return this; },
    };
  }
  getClientRects() { return [this.getBoundingClientRect()]; }
  // cssom-view checkVisibility (spec): an element with no box is not
  // visible — detached, or display:none on itself or any ancestor. The
  // visibility and opacity checks are opt-in flags (Chrome answers true for
  // visibility:hidden unless checkVisibilityCSS is set; same for opacity:0
  // without checkOpacity, and the opacity check multiplies down the ancestor
  // chain). Geometry (rect > 0) is deliberately NOT part of the answer —
  // Chrome's return value carries no geometry, and snapshot layers that need
  // it filter on rects themselves (issue #45).
  checkVisibility(opts) {
    opts = opts || {};
    // The spec names are checkVisibilityCSS/checkOpacity; the IDL shipped
    // visibilityProperty/opacityProperty first and Chrome answers to both
    // (obscura#1067 / #160 — Playwright's expect(visible) passes the
    // deprecated spellings).
    var visOn = !!(opts.checkVisibilityCSS || opts.visibilityProperty);
    var opOn = !!(opts.checkOpacity || opts.opacityProperty);
    if (!this.isConnected) return false;
    var opacity = 1;
    for (var node = this; node && node.nodeType === 1; node = node.parentElement) {
      var cs;
      try { cs = getComputedStyle(node); } catch (e) { return false; }
      if (!cs) return false;
      var display = String(cs.getPropertyValue('display'));
      if (display === 'none') return false;
      if (visOn) {
        var vis = String(cs.getPropertyValue('visibility'));
        if (vis !== 'visible') return false;
      }
      // contentVisibilityAuto: the closest analog our cascade supports is
      // content-visibility: hidden (contents skipped outright). 'auto'
      // skip-tracking is a layout-feature we don't have; it reads as visible.
      if (opts.contentVisibilityAuto &&
          String(cs.getPropertyValue('content-visibility')) === 'hidden') return false;
      var op = parseFloat(String(cs.getPropertyValue('opacity')));
      if (Number.isFinite(op)) opacity *= op;
    }
    if (opOn && opacity === 0) return false;
    return true;
  }
  // ARIA reflection properties. Without an accessibility tree we expose the
  // raw aria-* attributes so Playwright's getByRole / getByLabel locators can
  // at least find elements that author them explicitly.
  get role() { return this.getAttribute('role'); }
  set role(v) { if (v == null) this.removeAttribute('role'); else this.setAttribute('role', String(v)); }
  get ariaLabel() { return this.getAttribute('aria-label'); }
  set ariaLabel(v) { if (v == null) this.removeAttribute('aria-label'); else this.setAttribute('aria-label', String(v)); }
  get ariaRoleDescription() { return this.getAttribute('aria-roledescription'); }
  set ariaRoleDescription(v) { if (v == null) this.removeAttribute('aria-roledescription'); else this.setAttribute('aria-roledescription', String(v)); }
  get ariaChecked() { return this.getAttribute('aria-checked'); }
  set ariaChecked(v) { if (v == null) this.removeAttribute('aria-checked'); else this.setAttribute('aria-checked', String(v)); }
  get ariaDisabled() { return this.getAttribute('aria-disabled'); }
  set ariaDisabled(v) { if (v == null) this.removeAttribute('aria-disabled'); else this.setAttribute('aria-disabled', String(v)); }
  get ariaExpanded() { return this.getAttribute('aria-expanded'); }
  set ariaExpanded(v) { if (v == null) this.removeAttribute('aria-expanded'); else this.setAttribute('aria-expanded', String(v)); }
  get ariaHidden() { return this.getAttribute('aria-hidden'); }
  set ariaHidden(v) { if (v == null) this.removeAttribute('aria-hidden'); else this.setAttribute('aria-hidden', String(v)); }
  get ariaSelected() { return this.getAttribute('aria-selected'); }
  set ariaSelected(v) { if (v == null) this.removeAttribute('aria-selected'); else this.setAttribute('aria-selected', String(v)); }
  // scrollIntoView (CSSOM View §): scroll the element's scroll containers —
  // each real element scroller up the ancestor chain, then the viewport
  // root — so the element lands at the block/inline alignment. Used to be a
  // click-target-only stub, which left anything below the fold permanently
  // unclickable: the click recheck hit-tested a client point outside the
  // viewport and verdicted "covered_by: nothing" (tmall publish SKU button).
  // Options default per spec: block 'start', inline 'nearest'.
  scrollIntoView(arg) {
    globalThis.__diting_click_target = this;
    var opts = (arg && typeof arg === 'object') ? arg : {};
    // scrollIntoView(false) is {block: 'end', inline: 'nearest'} per CSSOM
    // View; anything else non-object behaves like () = start/nearest.
    var block = opts.block || (arg === false ? 'end' : 'start');
    var inline = opts.inline || 'nearest';
    var doc = globalThis.document;
    var root = doc && doc.documentElement;
    if (!root) return;
    // Alignment delta for one axis: where the element's edge should sit
    // inside the container's client box. 'nearest' scrolls the smaller
    // amount that reveals it (0 when already visible).
    var delta = function (eStart, eSize, cStart, cSize, align) {
      var eEnd = eStart + eSize, cEnd = cStart + cSize;
      if (align === 'center') return eStart + eSize / 2 - (cStart + cSize / 2);
      if (align === 'end') return eEnd - cEnd;
      if (align === 'nearest') {
        if (eStart < cStart) return eStart - cStart;
        if (eEnd > cEnd) return eEnd - cEnd;
        return 0;
      }
      return eStart - cStart; // 'start'
    };
    // Element scrollers, innermost first. Both rects are client-space, so a
    // container's client box is its own gBCR (its border box is fixed in
    // the viewport; scrolling moves the content inside it, not the box).
    var node = this.parentElement;
    while (node && node !== root) {
      if (node._isNativeScrollContainer && node._isNativeScrollContainer()) {
        var cr = node.getBoundingClientRect();
        if (cr.width > 0 || cr.height > 0) {
          var er = this.getBoundingClientRect();
          var dt = delta(er.top, er.height, cr.top, cr.height, block);
          if (dt) node.scrollTop = (node.scrollTop || 0) + dt;
          var dl = delta(er.left, er.width, cr.left, cr.width, inline);
          if (dl) node.scrollLeft = (node.scrollLeft || 0) + dl;
        }
      }
      node = node.parentElement;
    }
    // Viewport root: the container box is [0, 0, innerWidth, innerHeight].
    var er2 = this.getBoundingClientRect();
    if (er2.width <= 0 && er2.height <= 0) return;
    var vw = globalThis.innerWidth || 0, vh = globalThis.innerHeight || 0;
    var dt2 = delta(er2.top, er2.height, 0, vh, block);
    if (dt2) root.scrollTop = (root.scrollTop || 0) + dt2;
    var dl2 = delta(er2.left, er2.width, 0, vw, inline);
    if (dl2) root.scrollLeft = (root.scrollLeft || 0) + dl2;
  }
  // scrollTo/scrollBy/scroll accept either (x, y) or a ScrollToOptions
  // object. They write through the scrollTop/scrollLeft setters, so the
  // viewport root inherits the real-extent clamp and element scrollers keep
  // wrapper-local offsets; firing a scroll event lets scroll-driven lazy
  // loaders advance (#429). The setters fire a scroll event of their own, so
  // suppress the per-axis ones here and emit a single event for the whole
  // movement, the way a real browser coalesces one scroll per scroll
  // operation rather than one per axis.
  scrollTo(x, y) {
    let left, top;
    if (x !== null && typeof x === 'object') { left = x.left; top = x.top; }
    else { left = x; top = y; }
    const root = this._rootScroller();
    const beforeLeft = root.scrollLeft || 0;
    const beforeTop = root.scrollTop || 0;
    __def(this, '_scrollSuppress', true);
    if (left !== undefined) this.scrollLeft = +left || 0;
    if (top !== undefined) this.scrollTop = +top || 0;
    this._scrollSuppress = false;
    // A scroll op that translated nothing fires neither scroll nor
    // scrollend (blitz#354's negative clause).
    if ((root.scrollLeft || 0) !== beforeLeft || (root.scrollTop || 0) !== beforeTop) this._fireScroll();
  }
  scroll(x, y) { this.scrollTo(x, y); }
  scrollBy(x, y) {
    let dl, dt;
    if (x !== null && typeof x === 'object') { dl = x.left; dt = x.top; }
    else { dl = x; dt = y; }
    const root = this._rootScroller();
    const beforeLeft = root.scrollLeft || 0;
    const beforeTop = root.scrollTop || 0;
    __def(this, '_scrollSuppress', true);
    this.scrollLeft = (this.scrollLeft || 0) + (+dl || 0);
    this.scrollTop = (this.scrollTop || 0) + (+dt || 0);
    this._scrollSuppress = false;
    if ((root.scrollLeft || 0) !== beforeLeft || (root.scrollTop || 0) !== beforeTop) this._fireScroll();
  }
  _fireScroll() {
    const self = this;
    // scrollend (blitz#354) trails the scroll in the same tick: our scrolls
    // are instant, so "scrolling finished" is the moment after the scroll
    // event. Fires with bubbles: true, as the spec has it (unlike scroll,
    // scrollend bubbles) — since the M3 native core the propagation is real,
    // so this reaches document and window listeners up the chain.
    setTimeout(() => {
      try { self.dispatchEvent(new Event('scroll', { bubbles: false })); } catch (e) {}
      try { self.dispatchEvent(new Event('scrollend', { bubbles: true })); } catch (e) {}
    }, 0);
  }
  animate(keyframes, options) {
    const duration = typeof options === 'number' ? options : (options?.duration || 0);
    // WAAPI animations fire the same lifecycle events as CSS animations
    // (with an empty animationName, like Chrome — keyframe names are not
    // part of the API). Detached elements never start.
    if (_nodeInDocument(this)) {
      const durSec = Math.max(0, (Number(duration) || 0) / 1000);
      _dispatchAnimationEvent(this, "animationstart", "", 0);
      setTimeout(() => {
        if (_nodeInDocument(this)) _dispatchAnimationEvent(this, "animationend", "", durSec);
      }, durSec * 1000);
    }
    return {
      finished: Promise.resolve(), currentTime: 0, playState: 'finished',
      effect: { getComputedTiming() { return { duration }; } },
      cancel(){}, finish(){}, play(){}, pause(){}, reverse(){},
      addEventListener(){}, removeEventListener(){},
      onfinish: null, oncancel: null,
    };
  }
  getAnimations() { return []; }
  get isConnected() {
    var node = this;
    while (node) {
      if (node.nodeType === 9) return true;
      // A ShadowRoot's own parentNode is null; connectivity crosses to the
      // host (shadow-including connectivity).
      if (node.nodeType === 11 && node.host) { node = node.host; continue; }
      node = node.parentNode;
    }
    return false;
  }
  remove() { if (this.parentNode) this.parentNode.removeChild(this); }
  // The ParentNode multi-arg mutators emit ONE childList record per call in
  // Chrome (append(i, b) → single record, addedNodes [i, b], siblings around
  // the whole range) — #272 Chrome probe. The per-node ops run with the
  // batch flag so appendChild/insertBefore stay silent, then one combined
  // notify fires; with no observers registered the notify is skipped and
  // the plain loop runs untouched (zero added cost on the hot path).
  append(...nodes) {
    const conv = _convertNodes(nodes);
    if (!conv.length) return;
    if (!globalThis.__mutationObservers?.length) {
      for (const n of conv) this.appendChild(n);
      return;
    }
    const prevNid = this.lastChild ? this.lastChild._nid : null;
    for (const n of conv) this.appendChild(n, true);
    const last = conv[conv.length - 1];
    globalThis.__notifyMutation('childList', this._nid, conv.map(n => n._nid), [],
      null, null, prevNid, last.nextSibling ? last.nextSibling._nid : null);
  }
  prepend(...nodes) {
    const conv = _convertNodes(nodes);
    if (!conv.length) return;
    const ref = this.firstChild;
    if (!globalThis.__mutationObservers?.length) {
      for (const n of conv) {
        if (ref) this.insertBefore(n, ref); else this.appendChild(n);
      }
      return;
    }
    const nextNid = ref ? ref._nid : null;
    for (const n of conv) {
      if (ref) this.insertBefore(n, ref, true); else this.appendChild(n, true);
    }
    globalThis.__notifyMutation('childList', this._nid, conv.map(n => n._nid), [],
      null, null, conv[0].previousSibling ? conv[0].previousSibling._nid : null, nextNid);
  }
  replaceChildren(...nodes) {
    const converted = _convertNodes(nodes);
    if (!globalThis.__mutationObservers?.length) {
      let c;
      while ((c = this.firstChild)) this.removeChild(c);
      for (const n of converted) this.appendChild(n);
      return;
    }
    // A whole-list swap is one replace-all record (same shape Chrome gives
    // textContent/innerHTML writes): added+removed in one entry, no siblings.
    const oldNids = Array.from(this.childNodes).map(n => n._nid);
    let c;
    while ((c = this.firstChild)) this.removeChild(c, true);
    for (const n of converted) this.appendChild(n, true);
    globalThis.__notifyMutation('childList', this._nid, converted.map(n => n._nid), oldNids,
      null, null, null, null);
  }
}

// WHATWG "convert nodes into a node": a Node argument passes through, anything
// else is stringified into a Text node, so e.g. append(null) inserts the text
// "null" and append(undefined) inserts "undefined" per the (Node or DOMString)
// union, rather than throwing.
function _convertNodes(nodes) {
  const out = [];
  for (let i = 0; i < nodes.length; i++) {
    const n = nodes[i];
    if (n && typeof n._nid === "number") out.push(n);
    else out.push(document.createTextNode(String(n)));
  }
  return out;
}

// ---- Reflected IDL attributes (WHATWG) ---------------------------------------
// Installed ONCE on Element.prototype as shared getter/setter pairs. This is
// data-driven so there is no per-element defineProperty: element creation and
// the querySelector/mutation hot paths are unaffected (each access is a normal
// prototype getter that reads the backing attribute). Covers the global content
// attributes reflected on every element plus the ARIAMixin (aria-* + ariaXxx).
(function installElementReflectors() {
  const P = Element.prototype;
  const def = (name, get, set) => {
    if (Object.prototype.hasOwnProperty.call(P, name)) return; // never clobber an existing member
    Object.defineProperty(P, name, { get, set, enumerable: true, configurable: true });
  };
  // WHATWG "rules for parsing integers"; returns a JS number or null on failure.
  const parseIntAttr = (s) => {
    if (s === null || s === undefined) return null;
    const m = /^[ \t\n\f\r]*([+-]?[0-9]+)/.exec(String(s));
    if (!m) return null;
    const n = parseInt(m[1], 10);
    return Number.isFinite(n) ? n : null;
  };
  // IDL `long` conversion (ToInt32): finite, truncated, wrapped to 32-bit signed.
  const toLong = (v) => {
    let n = Number(v);
    if (!Number.isFinite(n)) n = 0;
    n = Math.trunc(n) % 4294967296;
    if (n >= 2147483648) n -= 4294967296;
    else if (n < -2147483648) n += 4294967296;
    return n;
  };
  // DOMString reflect: get -> attribute or ""; set -> setAttribute(String(v)).
  const reflectStr = (name, attr) => def(name,
    function () { const v = this.getAttribute(attr); return v === null ? "" : v; },
    function (v) { this.setAttribute(attr, String(v)); });
  // boolean reflect: get -> hasAttribute; set -> truthy ? add("") : remove.
  const reflectBool = (name, attr) => def(name,
    function () { return this.hasAttribute(attr); },
    function (v) { if (v) this.setAttribute(attr, ""); else this.removeAttribute(attr); });
  // long reflect: get -> parse else default (static value or per-element fn);
  // set -> setAttribute(String(ToInt32(v))).
  const reflectLong = (name, attr, dflt) => def(name,
    function () {
      const r = parseIntAttr(this.getAttribute(attr));
      if (r !== null && r >= -2147483648 && r <= 2147483647) return r;
      return typeof dflt === "function" ? dflt.call(this) : dflt;
    },
    function (v) { this.setAttribute(attr, String(toLong(v))); });
  // enumerated reflect: get -> canonical (lowercased) keyword, else missing/
  // invalid default; set -> setAttribute(String(v)) (canonicalization on get).
  const reflectEnum = (name, attr, keywords, missingDefault, invalidDefault) => def(name,
    function () {
      const v = this.getAttribute(attr);
      if (v === null) return missingDefault;
      const lc = String(v).toLowerCase();
      return keywords.indexOf(lc) !== -1 ? lc : invalidDefault;
    },
    function (v) { this.setAttribute(attr, String(v)); });
  // nullable DOMString reflect (ARIA): get -> attribute or null; set -> null/
  // undefined removes, else setAttribute(String(v)).
  const reflectNullable = (name, attr) => def(name,
    function () { return this.getAttribute(attr); },
    function (v) { if (v === null || v === undefined) this.removeAttribute(attr); else this.setAttribute(attr, String(v)); });

  // Global content attributes reflected on every element (HTML "global attributes").
  reflectStr("title", "title");
  reflectStr("lang", "lang");
  reflectStr("accessKey", "accesskey");
  reflectStr("slot", "slot");
  reflectEnum("dir", "dir", ["ltr", "rtl", "auto"], "", "");
  reflectBool("autofocus", "autofocus");
  reflectBool("hidden", "hidden");
  // tabIndex default is element-dependent (0 for natively-focusable, else -1);
  // reflection.js does not assert it, but match the common case anyway.
  reflectLong("tabIndex", "tabindex", function () {
    const ln = this.localName;
    if (ln === "a" || ln === "area" || ln === "link") return this.hasAttribute("href") ? 0 : -1;
    return (ln === "button" || ln === "input" || ln === "select" || ln === "textarea" || ln === "iframe") ? 0 : -1;
  });

  // ARIAMixin: aria-* content attributes reflected as nullable DOMString IDL
  // properties (ariaAtomic <-> aria-atomic, ...).
  const ARIA = {
    ariaAtomic: "aria-atomic", ariaAutoComplete: "aria-autocomplete", ariaBrailleLabel: "aria-braillelabel",
    ariaBrailleRoleDescription: "aria-brailleroledescription", ariaBusy: "aria-busy", ariaChecked: "aria-checked",
    ariaColCount: "aria-colcount", ariaColIndex: "aria-colindex", ariaColIndexText: "aria-colindextext",
    ariaColSpan: "aria-colspan", ariaCurrent: "aria-current", ariaDescription: "aria-description",
    ariaDisabled: "aria-disabled", ariaExpanded: "aria-expanded", ariaHasPopup: "aria-haspopup",
    ariaHidden: "aria-hidden", ariaInvalid: "aria-invalid", ariaKeyShortcuts: "aria-keyshortcuts",
    ariaLabel: "aria-label", ariaLevel: "aria-level", ariaLive: "aria-live", ariaModal: "aria-modal",
    ariaMultiLine: "aria-multiline", ariaMultiSelectable: "aria-multiselectable", ariaOrientation: "aria-orientation",
    ariaPlaceholder: "aria-placeholder", ariaPosInSet: "aria-posinset", ariaPressed: "aria-pressed",
    ariaReadOnly: "aria-readonly", ariaRelevant: "aria-relevant", ariaRequired: "aria-required",
    ariaRoleDescription: "aria-roledescription", ariaRowCount: "aria-rowcount", ariaRowIndex: "aria-rowindex",
    ariaRowIndexText: "aria-rowindextext", ariaRowSpan: "aria-rowspan", ariaSelected: "aria-selected",
    ariaSetSize: "aria-setsize", ariaSort: "aria-sort", ariaValueMax: "aria-valuemax",
    ariaValueMin: "aria-valuemin", ariaValueNow: "aria-valuenow", ariaValueText: "aria-valuetext",
  };
  for (const prop in ARIA) reflectNullable(prop, ARIA[prop]);
})();

class Document extends Node {
  get documentElement() { return _wrapEl(+_dom("document_element")); }
  // ParentNode face (#250): document.children is the HTMLCollection holding
  // just the documentElement (doctype is a node, not an element). tbtracker
  // reads `var e = document.firstElementChild` at boot and attaches its
  // listeners from a DOMContentLoaded handler — undefined there crashed
  // `e.addEventListener` on myseller and poisoned the workbench boot.
  get children() {
    const de = this.documentElement;
    return __diting_mkCollection(de ? [de] : []);
  }
  get firstElementChild() { return this.documentElement || null; }
  get lastElementChild() { return this.documentElement || null; }
  get childElementCount() { return this.documentElement ? 1 : 0; }
  get head() { return this.querySelector("head"); }
  get body() { return this.querySelector("body"); }
  // #257: cloning a document makes a NEW document — doctype + element tree
  // when deep, an empty shell when shallow. The tree is rebuilt from
  // serialization through the DOMParser face: listeners and expandos never
  // copy (spec), which is exactly what that path produces, and scripts stay
  // inert in a detached tree. Readability-style extractors start from
  // document.cloneNode(true); the generic Node path returned null for
  // nodeType 9 (obscura #1230-shaped boot crash).
  cloneNode(deep) {
    try {
      const src = deep && this.documentElement ? '<!DOCTYPE html>' + this.documentElement.outerHTML : '';
      return new DOMParser().parseFromString(src, 'text/html');
    } catch (e) { return null; }
  }
  get doctype() {
    if (this._doctype !== undefined) return this._doctype;
    const info = _domParse("document_doctype");
    if (info && info.name) {
      __def(this, '_doctype', new DocumentType(info.nodeId, info.name, info.publicId || "", info.systemId || ""));
    } else {
      __def(this, '_doctype', null);
    }
    return this._doctype;
  }
  get title() { return _domParse("document_title") ?? ""; }
  // Spec: setting document.title updates the <title> element's text, creating
  // one under <head> if missing. The op keeps the Rust-side title (used for
  // navigation responses) in sync; the DOM write keeps querySelector('title')
  // and outerHTML consistent with what the page set.
  set title(v) {
    v = String(v);
    _dom("set_document_title", v);
    const head = this.head;
    if (!head) return;
    let el = head.querySelector("title");
    if (!el) { el = this.createElement("title"); head.appendChild(el); }
    el.textContent = v;
  }
  get URL() { return _domParse("document_url") ?? ""; }
  get documentURI() { return this.URL; }
  // #188-3: document.dir reflects the html element's dir (HTML §3.2.6 —
  // "the dir attribute on the html element" is the document-level one).
  // el.dir already reflects/updates the attribute, which the CSS hint arm
  // consumes, so delegating is both the spec shape and the wired path.
  get dir() { return this.documentElement.dir; }
  set dir(v) { this.documentElement.dir = v; }
  // URL of the document that initiated this navigation; empty for direct
  // automation navigations. Computed by the navigation layer per
  // strict-origin-when-cross-origin (upstream edb1785).
  get referrer() { return _domParse("document_referrer") ?? ""; }
  // 刀57: document.referrerPolicy deleted — Chrome 145's Document.prototype
  // carries no referrerPolicy (only the per-element/request faces have one).
  // The policy itself stays resolved Rust-side (document_referrer_policy op)
  // where the fetch/XHR loaders read it; the JS face was an invention.
  // document.domain: legacy same-origin surface. The getter returns the
  // origin's host — bilibili's log-reporter scopes its b_lsid cookie with
  // `document.domain.split(".")`, and an undefined domain took the whole
  // script down there. The setter is the legacy relaxation: assigning the
  // same host or a parent suffix sticks, anything else is a SecurityError.
  get domain() {
    if (this._domainOverride !== undefined) return this._domainOverride;
    const loc = globalThis.location;
    return (loc && loc.hostname) || "";
  }
  set domain(v) {
    v = String(v);
    const cur = this.domain;
    if (v === cur || (v && cur.endsWith("." + v))) __def(this, '_domainOverride', v);
    else throw new DOMException("Illegal document.domain value \"" + v + "\"", "SecurityError");
  }
  get location() { return globalThis.location; }
  set location(url) { _OPS.op_navigate(_resolveUrl(String(url)), 'GET', ''); }
  get defaultView() { return globalThis; }
  get nodeType() { return 9; }
  get nodeName() { return "#document"; }
  get ownerDocument() { return null; } // Document has no ownerDocument
  get compatMode() { return "CSS1Compat"; }
  // The document's character encoding, detected from the response charset
  // (HTTP Content-Type -> <meta charset>). characterSet/charset/inputEncoding
  // are WHATWG aliases. A node-less document (DOMParser/createDocument) has no
  // backing encoding and reports UTF-8.
  get characterSet() { return (this._nid === undefined || this._nid === null) ? "UTF-8" : _docEncoding(); }
  get charset() { return this.characterSet; }
  get inputEncoding() { return this.characterSet; }
  get contentType() {
    // An explicit type set by DOMParser/createDocument wins.
    if (this._contentType) return this._contentType;
    // `new Document()` (the WHATWG constructor, no backing node id) creates an
    // XML document, so createCDATASection/etc. must not throw. Live documents
    // wrapped from the tree carry a real nid and fall through to URL-derived.
    if (this._nid === undefined || this._nid === null) return "application/xml";
    // Chrome derives contentType from the response, not the URL: a text/plain
    // document served from an extension-less URL reports "text/plain". The
    // response type wins when the server delivered one.
    const responseType = _docContentType();
    if (responseType) return responseType;
    const url = this.URL || "";
    // data: URLs carry their MIME type explicitly.
    const dm = /^data:([^,;]+)/i.exec(url);
    if (dm) {
      const mime = dm[1].toLowerCase();
      if (mime === "application/xhtml+xml") return "application/xhtml+xml";
      if (mime === "text/xml") return "text/xml";
      if (mime === "application/xml" || mime.endsWith("+xml")) return "application/xml";
    }
    if (/\.xhtml(?:[?#]|$)/i.test(url)) return "application/xhtml+xml";
    if (/\.(?:xml|svg)(?:[?#]|$)/i.test(url)) return "application/xml";
    return "text/html";
  }
  get readyState() { return globalThis.__documentReadyState__ || 'complete'; }
  get currentScript() {
    // Next.js / Turbopack chunk loader reads document.currentScript.src to
    // derive its base path. page.rs sets __currentScriptNid before each
    // <script> body runs and clears it after, mirroring real Chrome.
    const nid = globalThis.__currentScriptNid;
    return nid ? _wrapEl(+nid) : null;
  }
  get hidden() { return false; }
  get visibilityState() { return "visible"; }
  getElementById(id) {
    // WebIDL: the argument string-coerces (null → "null" finds
    // <div id="null">), and the empty string never matches.
    const needle = String(id);
    return needle === "" ? null : _wrapEl(+_dom("get_element_by_id", needle));
  }
  querySelector(s) {
    const id = +_dom("query_selector", s);
    if (_parserScriptHidden(id)) return this.querySelectorAll(s)[0] || null;
    return _wrapEl(id);
  }
  querySelectorAll(s) {
    const ids = _parserVisibleIds(_domParse("query_selector_all", s) || []);
    return _nodeList(ids.map(_wrapEl).filter(Boolean));
  }
  getElementsByTagName(t) { return __diting_mkCollection(this.querySelectorAll(t)); }
  getElementsByClassName(c) { return _getElementsByClassName(this, c); }
  getElementsByName(name) { return this.querySelectorAll('[name="' + String(name).replace(/\\/g, '\\\\').replace(/"/g, '\\"') + '"]'); }
  createElement(t) {
    // The namespace is passed explicitly: create_element maps an empty
    // second arg to the null namespace (correct for XML elements with no
    // xmlns), so the HTML path must say XHTML here.
    const lower = t.toLowerCase();
    const el = _wrapEl(+_dom("create_element", lower, "http://www.w3.org/1999/xhtml"));
    if (el) {
      // Chrome runs a registered custom element's constructor during
      // createElement — the element is born upgraded, detached, so
      // connectedCallback waits for insertion (obscura #993 lineage).
      const reg = (typeof customElements !== 'undefined' && customElements._registry) ? customElements._registry : null;
      const cls = reg ? reg.get(lower) : null;
      if (cls) customElements._upgradeElement(el, cls);
    }
    if (el && lower === 'template') {
      el._templateContent = this.createDocumentFragment();
    }
    return el;
  }
  createElementNS(ns, t) {
    // Pass the namespace straight to the tree (create_element maps "" to the
    // null namespace and keeps the XML tag case), so tagName/localName/
    // namespaceURI all read true — the old path ran it through createElement,
    // which lowercased the tag and hardcoded XHTML (#28).
    const name = String(t);
    const el = _wrapEl(+_dom("create_element", name, String(ns ?? "")));
    if (el && name.toLowerCase() === 'template') {
      el._templateContent = this.createDocumentFragment();
    }
    return el;
  }
  createTextNode(t) { return _wrap(+_dom("create_text_node", String(t))); }
  createComment(t) {
    const nid = +_dom("create_comment_node", String(t ?? ""));
    const n = new Comment(nid);
    _cache.set(nid, n);
    return n;
  }
  createCDATASection(data) {
    // Spec: throw NotSupportedError on an HTML document, reject data
    // containing "]]>", then return a CDATASection node.
    if (!_isXMLDocument(this)) {
      throw new DOMException("createCDATASection is not supported in HTML documents", "NotSupportedError");
    }
    const str = String(data);
    if (str.indexOf("]]>") !== -1) {
      throw new DOMException("CDATA section data must not contain ']]>'", "InvalidCharacterError");
    }
    const nid = +_dom("create_text_node", str);
    const n = new CDATASection(nid);
    _cache.set(nid, n);
    return n;
  }
  createProcessingInstruction(target, data) {
    // Spec: not gated on document type. Reject targets that are not an XML
    // Name, then reject data containing "?>", then return a PI node.
    const tgt = String(target);
    const str = String(data);
    if (!_isValidPITarget(tgt)) {
      throw new DOMException("Invalid processing instruction target", "InvalidCharacterError");
    }
    if (str.indexOf("?>") !== -1) {
      throw new DOMException("Processing instruction data must not contain '?>'", "InvalidCharacterError");
    }
    const nid = +_dom("create_text_node", str);
    const n = new ProcessingInstruction(nid, tgt);
    _cache.set(nid, n);
    return n;
  }
  createDocumentFragment() {
    const nid = +_dom("create_document_fragment");
    const frag = new DocumentFragment(nid);
    _cache.set(nid, frag);
    return frag;
  }
  // Legacy DOM Level 2 event factory. Spec returns an event of the requested
  // class with an empty type until init*Event() is called. We previously
  // returned a generic Event for every type, which broke libraries that call
  // createEvent('CustomEvent').initCustomEvent(...) — see issue #41.
  createEvent(type) {
    const eventType = String(type || '');
    const normalized = eventType.toLowerCase();
    const map = {
      'event': Event, 'events': Event,
      'htmlevents': Event, 'svgevents': Event,
      'customevent': CustomEvent, 'customevents': CustomEvent,
      'mouseevent': MouseEvent,   'mouseevents': MouseEvent,
      'keyboardevent': KeyboardEvent, 'keyboardevents': KeyboardEvent,
      'focusevent': FocusEvent,
      'hashchangeevent': HashChangeEvent,
      'inputevent': InputEvent,
      'messageevent': MessageEvent,
      'closeevent': CloseEvent,
      'uievent': UIEvent, 'uievents': UIEvent,
      'compositionevent': CompositionEvent,
      'wheelevent': WheelEvent,
      'pointerevent': PointerEvent,
      'errorevent': ErrorEvent,
      'popstateevent': PopStateEvent,
      'animationevent': AnimationEvent,
      'transitionevent': TransitionEvent,
      'storageevent': StorageEvent,
    };
    // Chrome throws NotSupportedError for unknown interface names — silently
    // returning a generic Event hides typos from callers ('NotARealType' would
    // come back as an Event whose init* methods are all missing). Note
    // 'promiserejectionevent' is intentionally absent: Chrome rejects it too.
    const Cls = map[normalized];
    if (!Cls) {
      throw new DOMException(
        `The provided event type ('${eventType}') is invalid`,
        'NotSupportedError'
      );
    }
    return new Cls('');
  }
  createRange() { return new Range(); }
  // addEventListener/removeEventListener/dispatchEvent resolve through the
  // prototype chain to the NATIVE EventTarget core (M3 B3): the trio here
  // is gone. Document rides the same hooks as every Node (js/event_hooks.js)
  // — capture buckets, at-target both-bucket phase, and the document →
  // window propagation edge the old code lacked (bubbled events dispatched
  // on or through the document now reach window listeners, Chrome-true).
  getSelection() { return this.defaultView ? _selectionFor(this) : null; }
  get activeElement() { return globalThis.__diting_focused || this.body; }
  // The element that scrolls the viewport, and where the page offset lives.
  // Standards mode, so documentElement — quirks mode would be body, but we
  // never parse in quirks mode (upstream #468).
  get scrollingElement() { return this.documentElement; }
  get implementation() {
    // Chrome: one DOMImplementation per document (document.implementation === document.implementation); demote_own.js moves
    // these members onto DOMImplementation.prototype after the first read.
    if (this.__diting_impl) return this.__diting_impl;
    const ownerDoc = this;
    const impl = {
      // Spec: createHTMLDocument returns a NEW detached Document. jQuery
      // 3.x's selector feature-detect calls `body.innerHTML = '<form>'` on
      // the result — when we returned `globalThis.document`, the real
      // `<body>` was wiped, taking every page on the open web that ships
      // jQuery 3.x with it. Reuse the DOMParser path to build a detached
      // document, then optionally set the title.
      createHTMLDocument(title) {
        // Build head>title and body explicitly. Parsing a full skeleton string
        // as innerHTML of <html> collapses through the fragment parser (it
        // dropped head/body and kept only <title>), leaving doc.body null.
        const doc = new DOMParser().parseFromString("", "text/html");
        const root = doc.documentElement;
        const head = document.createElement("head");
        const titleEl = document.createElement("title");
        if (title != null) titleEl.textContent = String(title);
        head.appendChild(titleEl);
        const body = document.createElement("body");
        root.appendChild(head);
        root.appendChild(body);
        return doc;
      },
      // Real spec: createDocument(namespaceURI, qualifiedName, doctype) →
      // an XML document with a root element of the given name. We don't
      // have a separate XML stack, so return a minimal detached document
      // with an element of the requested local name as documentElement.
      createDocument(_ns, qualifiedName, _doctype) {
        const name = (qualifiedName && String(qualifiedName)) || "root";
        const safe = name.replace(/[^a-zA-Z0-9-]/g, "");
        const html = qualifiedName ? `<${safe}></${safe}>` : "";
        const doc = new DOMParser().parseFromString(html, "application/xml");
        if (_doctype) doc._docType = _doctype;
        return doc;
      },
      // createDocumentType(qualifiedName, publicId, systemId): build a detached
      // DocumentType node. Browsers validate leniently here (only a name with
      // ASCII whitespace or ">" is rejected, matching the WPT cases); the node's
      // owner document is the document whose implementation was used.
      createDocumentType(qualifiedName, publicId, systemId) {
        const name = String(qualifiedName);
        if (name === "" || /[\t\n\f\r >]/.test(name)) {
          throw new DOMException("The qualified name '" + name + "' contains an invalid character", "InvalidCharacterError");
        }
        const dt = new DocumentType(
          +_dom("create_comment_node", ""),
          name,
          publicId === undefined ? "" : String(publicId),
          systemId === undefined ? "" : String(systemId)
        );
        dt._ownerDocument = ownerDoc;
        return dt;
      },
      hasFeature() { return true; },
    };
    Object.defineProperty(this, '__diting_impl', { value: impl, configurable: true });
    if (typeof globalThis.__diting_demote_own === 'function') globalThis.__diting_demote_own();
    return impl;
  }
  get styleSheets() {
    // Document-order sheet list: <style> elements plus <link rel=stylesheet>
    // whose navigation-time fetch succeeded (Chrome only lists loaded
    // sheets). Rule content is parsed engine-side by the same parser the
    // cascade uses, so cssRules and getComputedStyle can never disagree
    // about what a sheet says. One truth with the per-element `sheet`
    // property (_elementSheet), so both faces can't drift apart.
    const out = [];
    for (const el of this.querySelectorAll("style, link")) {
      const sheet = _elementSheet(el);
      if (sheet) out.push(sheet);
    }
    return _sheetList(out);
  }
  // Spec collections. HTMLCollection (not NodeList): namedItem + Proxy named
  // access come free, and frameworks (react-router/Next.js form lookups) call
  // document.forms.namedItem — which used to crash hydration here.
  get forms() { return __diting_mkCollection(this.querySelectorAll('form')); }
  get images() { return __diting_mkCollection(this.querySelectorAll('img')); }
  get links() { return __diting_mkCollection(this.querySelectorAll('a[href], area[href]')); }
  get scripts() { return __diting_mkCollection(this.querySelectorAll('script')); }
  get cookie() {
    __cookieGate(false);
    return _OPS.op_get_cookies();
  }
  set cookie(v) {
    __cookieGate(true);
    if (!v) return;
    _OPS.op_set_cookie(v);
  }
  // Inserts into the document's input stream, which the host keeps alive across calls.
  // Parsing each call on its own would lose every construct that spans two of them — a
  // tag may be split anywhere, even mid tag-name (SAP UI5's cachebuster writes exactly
  // that way: one call for "<script", one per attribute, then ">").
  // https://html.spec.whatwg.org/multipage/dynamic-markup-insertion.html#dom-document-write
  write(...args) {
    var html = args.join('');
    if (!html) return;
    var body = this.body;
    if (!body) return;
    // The host parses into the input stream and returns [[parent, node], …], parents first.
    // The insertion stays here, because appendChild does more than append: it reports the
    // mutation and runs a written script.
    var placements = _domParse("document_write", "", html) || [];
    // The insertion point is the position of the running script. What it writes belongs
    // behind it, not at the end of the body. The point moves along with every node placed,
    // even across calls, so a script's second call lands behind the first instead of
    // directly behind the script again.
    var scriptNid = globalThis.__currentScriptNid || 0;
    var after = null;
    if (scriptNid) {
      var anchorNid = this._writeAnchorScript === scriptNid && this._writeAnchorNid
        ? this._writeAnchorNid
        : scriptNid;
      var anchor = _wrap(anchorNid);
      if (anchor && anchor.parentNode) after = anchor;
    }
    for (var i = 0; i < placements.length; i++) {
      var parentNid = +placements[i][0];
      var node = _wrap(+placements[i][1]);
      if (!node) continue;
      if (parentNid) {
        var parent = _wrap(parentNid);
        if (parent) parent.appendChild(node);
        continue;
      }
      if (after) {
        after.parentNode.insertBefore(node, after.nextSibling);
        after = node;
      } else {
        body.appendChild(node);
      }
    }
    if (scriptNid && after) {
      __def(this, '_writeAnchorScript', scriptNid);
      __def(this, '_writeAnchorNid', after._nid);
    }
    // An iframe inserted with a src attribute loads on insertion — the src
    // property setter only covers JS assignment, and the post-navigation
    // scan only covers the initial parse. Written markup (which is how
    // Playwright's setContent delivers the page) needs its own scan.
    var iframes = this.querySelectorAll('iframe[src],iframe[srcdoc]');
    for (var j = 0; j < iframes.length; j++) {
      var f = iframes[j];
      if (f.hasAttribute('srcdoc')) { f._loadIframeSrc('about:srcdoc'); continue; }
      var s = f.getAttribute('src');
      if (s && s !== 'about:blank') f._loadIframeSrc(s);
    }
  }
  writeln(...args) {
    this.write(args.join('') + '\n');
  }
  open() {
    var body = this.body;
    if (body) body.innerHTML = '';
    // A new parse begins. Whatever the input stream still held is gone.
    _dom("document_write_reset");
    __def(this, '_writeAnchorScript', 0);
    __def(this, '_writeAnchorNid', 0);
    return this;
  }
  close() {
    return;
  }
  hasFocus() { return true; }
  execCommand(cmd, _showUI, value) {
    cmd = String(cmd || '').toLowerCase();
    if (cmd !== 'inserttext' || value == null) return false;
    const el = this.activeElement;
    if (!el || !_ns_isTextEntry(el) || el.disabled || el.readOnly) return false;
    const text = String(value);
    // Chrome's beforeinput is cancelable; a page veto aborts the insert but
    // the command itself still ran (execCommand answers true).
    const bi = new InputEvent('beforeinput', { inputType: 'insertText', data: text, bubbles: true, cancelable: true });
    if (!el.dispatchEvent(bi)) return true;
    const cur = String(el.value ?? '');
    const s = _ns_selectionStart.get(el) ?? cur.length;
    const e = _ns_selectionEnd.get(el) ?? cur.length;
    el.value = cur.slice(0, s) + text + cur.slice(e);
    const caret = s + text.length;
    el.setSelectionRange(caret, caret);
    el.dispatchEvent(new InputEvent('input', { inputType: 'insertText', data: text, bubbles: true }));
    return true;
  }
}

// document.location lives on the main document instance (see the `document` getter); keep the descriptor, drop the prototype member.
Object.defineProperty(globalThis, '__diting_docLocDesc', { value: Object.getOwnPropertyDescriptor(Document.prototype, 'location'), writable: true, enumerable: false, configurable: true });
delete Document.prototype.location;
class DocumentFragment extends Node {
  // `new DocumentFragment()` is legal in real browsers (creates a fresh
  // detached fragment). Bot detectors' domrect fixtures use it; without this
  // fallback `_nid` stayed undefined and the first appendChild tripped the
  // _dom Illegal-invocation guard — an escaping error the detector treats as
  // a broken environment.
  constructor(nid) {
    if (nid === undefined || nid === null || isNaN(+nid)) {
      nid = +_dom("create_document_fragment");
    }
    super(nid);
  }
  get nodeType() { return 11; }
  get nodeName() { return "#document-fragment"; }
  // 刀56/57: no innerHTML here — Chrome's DocumentFragment.prototype has none
  // (a plain fragment answers undefined); ShadowRoot owns its own pair below.
  querySelector(s) {
    const id = +_dom("query_selector_scoped", this._nid, s);
    // Parser-order visibility (#120): if the engine's first match is a script
    // the parser has not reached yet, fall back to the filtered list so the
    // caller gets the first *visible* match, not null-with-phantoms.
    if (_parserScriptHidden(id)) return this.querySelectorAll(s)[0] || null;
    return _wrapEl(id);
  }
  querySelectorAll(s) {
    const ids = _parserVisibleIds(_domParse("query_selector_all_scoped", this._nid, s) || []);
    return _nodeList(ids.map(_wrapEl).filter(Boolean));
  }
  get children() {
    const ids = _domParse("element_children", this._nid) || [];
    return __diting_mkCollection(ids.map(_wrapEl).filter(Boolean));
  }
  get firstElementChild() { return this.children[0] || null; }
  get lastElementChild() { const ch = this.children; return ch[ch.length - 1] || null; }
  getElementById(id) {
    const needle = String(id);
    if (needle === "") return null;
    const stack = Array.from(this.childNodes || []).reverse();
    while (stack.length) {
      const node = stack.pop();
      if (!node) continue;
      if (node.nodeType === 1 && node.id === needle) return node;
      const children = node.childNodes || [];
      for (let i = children.length - 1; i >= 0; i--) stack.push(children[i]);
    }
    return null;
  }
  cloneNode(deep) {
    const frag = document.createDocumentFragment();
    // 刀57: fragments lost their innerHTML face (Chrome truth); clone
    // through the ops so `deep` still materializes children.
    if (deep) _dom("set_inner_html", frag._nid, _domParse("inner_html", this._nid) ?? "");
    return frag;
  }
}

class DocumentType extends Node {
  constructor(nid, name, publicId, systemId) {
    super(nid);
    this._name = name;
    this._publicId = publicId;
    this._systemId = systemId;
    __hideOwn(this);
  }
  get nodeType() { return 10; }
  get nodeName() { return this._name; }
  get name() { return this._name; }
  get publicId() { return this._publicId; }
  get systemId() { return this._systemId; }
  get nodeValue() { return null; }
  set nodeValue(v) {}
  get ownerDocument() { return this._ownerDocument || globalThis.document; }
}

const _cache = new Map();
// (#27) per-tag interface routing: a div wraps as HTMLDivElement (its OWN
// prototype, not Element's), an unknown tag as HTMLElement — so
// constructor.name and instanceof read like Chrome, and a patch on
// HTMLDivElement.prototype reaches divs but never spans. The registry is
// filled by _htmlInterface below; until bootstrap reaches that block the
// callers fall back to Element exactly like before.
const _tagInterfaces = new Map();
Object.defineProperty(globalThis, '__diting_htmlTagMap', { value: _tagInterfaces, writable: true, enumerable: false, configurable: true });
// (#28) SVG local names → interface, keyed by the EXACT local name (Rust
// keeps XML-namespace tags in source case), so SVG <a> and HTML <A> never
// collide — the HTML table above is only consulted for uppercase tags.
const _svgTagInterfaces = new Map();
Object.defineProperty(globalThis, '__diting_svgTagMap', { value: _svgTagInterfaces, writable: true, enumerable: false, configurable: true });
function _elementClassFor(nid) {
  const tag = _domParse("tag_name", nid);
  if (tag === "FORM" && globalThis.__diting_domNative && globalThis.__diting_domNative.HTMLFormElement) return globalThis.__diting_domNative.HTMLFormElement;
  // Rust reports HTML tags uppercase and XML-namespace tags in source case
  // (tag_name's convention), so a lowercase first letter means a non-HTML
  // namespace: confirm SVG (MathML falls to the default), then route through
  // the SVG table. The HTML hot path pays nothing extra.
  if (typeof tag === "string" && tag && tag[0] === tag[0].toLowerCase()) {
    if (_domParse("namespace_uri", nid) === "http://www.w3.org/2000/svg") {
      return _svgTagInterfaces.get(tag) || (globalThis.__diting_domNative && globalThis.__diting_domNative.SVGElement) || Element;
    }
    return (globalThis.__diting_domNative && globalThis.__diting_domNative.HTMLElement) || Element;
  }
  const C = typeof tag === "string" ? _tagInterfaces.get(tag.toLowerCase()) : null;
  if (C) return C;
  // #184: the bootstrap-time class, never the page-mutable global — a
  // polyfill's HTMLElement shim would otherwise swallow the nid.
  return (globalThis.__diting_domNative && globalThis.__diting_domNative.HTMLElement) || Element;
}
// (#167) HTMLDocument is the document's concrete interface: Chrome's chain
// is document → HTMLDocument.prototype → Document.prototype, with
// `document instanceof HTMLDocument` true and constructor.name
// "HTMLDocument". Site SDKs touch the global BY NAME — Alibaba's AWSC
// nc-slider init dies on `ReferenceError: HTMLDocument is not defined`
// (the punish-page slider track then stays at width 0, what 批163's
// "SDK 未初始化" verdict actually saw). Declared before _wrap so the
// nodeType-9 branch below can instantiate it with no TDZ window; the
// node-less documents (DOMParser / `new Document()`, which make XML-ish
// docs) never route through _wrap and stay plain Document.
class HTMLDocument extends Document {
  constructor(nid) {
    if (typeof nid !== "number") {
      // Chrome: `new HTMLDocument()` throws; `new Document()` (an XML
      // document) is the legal WHATWG constructor face.
      throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor");
    }
    super(nid);
  }
}
globalThis.HTMLDocument = HTMLDocument;
function _wrap(nid) {
  if (nid < 0 || nid === null || nid === undefined || isNaN(nid)) return null;
  if (_cache.has(nid)) return _cache.get(nid);
  const t = +_dom("node_type", nid);
  let n;
  if (t === 1) { const C = _elementClassFor(nid); n = new C(nid); if (globalThis.__diting_domNative && C === globalThis.__diting_domNative.HTMLFormElement) n = new Proxy(n, _formNamedProxy); }
  else if (t === 3) n = new Text(nid);
  else if (t === 8) n = new Comment(nid);
  else if (t === 9) n = new HTMLDocument(nid);
  else n = new Node(nid);
  _cache.set(nid, n);
  return n;
}
function _wrapEl(nid) {
  if (nid < 0 || nid === null || nid === undefined || isNaN(nid)) return null;
  if (_cache.has(nid)) return _cache.get(nid);
  const C = _elementClassFor(nid);
  const el = new C(nid);
  // The cache holds the form's Proxy (not the raw wrapper) so identity is
  // stable across every lookup path.
  const n = (globalThis.__diting_domNative && C === globalThis.__diting_domNative.HTMLFormElement) ? new Proxy(el, _formNamedProxy) : el;
  _cache.set(nid, n);
  return n;
}

globalThis._wrap = _wrap;
globalThis.self = globalThis;

// `document` is a lazy accessor: at snapshot/bootstrap time there is no DOM
// yet (ObscuraState.dom is None until Page::init_js calls set_dom after the
// runtime constructor ran __diting_init), so the document node id cannot be
// resolved eagerly. Resolving on first access also guarantees identity: the
// global document IS `_wrap(documentNid)` — the same cached instance that
// parentNode bubbling reaches. A separate `new Document(...)` here meant
// React's delegated listeners (attached to the global `document`) lived on a
// different object than the bubble target, so submit/click handlers never ran.
let _documentInstance = null;
Object.defineProperty(globalThis, 'document', {
  configurable: true,
  enumerable: true,
  get() {
    if (_documentInstance) return _documentInstance;
    try {
      const nid = +_dom("document_node_id"); // NaN while no DOM is attached
      if (isNaN(nid) || nid < 0) return null;
      _documentInstance = _wrap(nid);
      // Chrome: Document.location is [LegacyUnforgeable] - an OWN, non-configurable accessor of the document instance
      // (not a prototype member). The class accessor stays as the fallback for other Document wrappers.
      try {
        const ld = globalThis.__diting_docLocDesc || Object.getOwnPropertyDescriptor(Document.prototype, 'location');
        if (ld && !Object.prototype.hasOwnProperty.call(_documentInstance, 'location')) Object.defineProperty(_documentInstance, 'location', { get: ld.get, set: ld.set, enumerable: true, configurable: false });
      } catch (e) {}
      // First touch of `document` is also the moment the parser-built tree
      // becomes reachable (#48): mutation hooks never saw it, so pay the
      // one-time named-property scan here. Later changes all flow through
      // the hooks and keep the registry current.
      if (!_namedScanned) {
        _namedScanned = true;
        try { _namedScanIn(_documentInstance); } catch (e) {}
      }
    } catch (e) {
      return null; // ops unavailable during snapshot construction
    }
    return _documentInstance;
  },
  set(v) { _documentInstance = v; },
});

if (!Element.prototype.replaceWith) {
  Element.prototype.replaceWith = function(...nodes) {
    const parent = this.parentNode;
    if (!parent) return;
    for (const n of nodes) {
      // WebIDL (Node or string): non-Node values stringify — jQuery 1.12's
      // domManip feeds raw fragments through here and replaceWith(undefined)
      // must land the text "undefined", not silently drop the node.
      parent.insertBefore(n instanceof Node ? n : document.createTextNode(String(n)), this);
    }
    parent.removeChild(this);
  };
  _markNative(Element.prototype.replaceWith);
}
if (!Element.prototype.before) {
  Element.prototype.before = function(...nodes) {
    const parent = this.parentNode;
    if (!parent) return;
    for (const n of nodes) {
      parent.insertBefore(n instanceof Node ? n : document.createTextNode(String(n)), this);
    }
  };
  _markNative(Element.prototype.before);
}
if (!Element.prototype.after) {
  Element.prototype.after = function(...nodes) {
    const parent = this.parentNode;
    if (!parent) return;
    const ref = this.nextSibling;
    for (const n of nodes) {
      parent.insertBefore(n instanceof Node ? n : document.createTextNode(String(n)), ref);
    }
  };
  _markNative(Element.prototype.after);
}

// ChildNode mixin: also mix before/after/replaceWith/remove into
// CharacterData.prototype (covers Text, Comment, ProcessingInstruction).
// These are the same implementations as Element.prototype — frameworks
// (Svelte 5, Vue, Lit) anchor on Comment/Text nodes and call these methods.
if (!CharacterData.prototype.before) CharacterData.prototype.before = Element.prototype.before;
if (!CharacterData.prototype.after) CharacterData.prototype.after = Element.prototype.after;
if (!CharacterData.prototype.replaceWith) CharacterData.prototype.replaceWith = Element.prototype.replaceWith;
if (!CharacterData.prototype.remove) CharacterData.prototype.remove = Element.prototype.remove;

if (!('isConnected' in Node.prototype)) {
  // 刀56: configurable like every Chrome Node member (it was installed
  // non-configurable, which blocked the nodelevel dispatcher redefinition).
  Object.defineProperty(Node.prototype, 'isConnected', {
    get() {
      let node = this;
      while (node) {
        if (node.nodeType === 9) return true; // Document node
        if (node.nodeType === 11 && node.host) { node = node.host; continue; } // ShadowRoot -> host
        node = node.parentNode;
      }
      return false;
    },
    enumerable: false, configurable: true,
  });
}

// `labels` / `control` (obscura#835 lineage): label↔control association is a
// two-way street, and Playwright's getByLabel() reads both directions off the
// page. Per the HTML spec, `el.labels` lists every label whose labeled control
// is el — for-matched AND wrapping — which is exactly what _labeledControl
// resolves in the forward direction, so the reverse is a filter over the
// document's labels. Element wrappers share Element.prototype (see
// _htmlInterface), so both getters live there behind tagName/matchability
// guards instead of on per-interface prototypes; `div.control` reads
// undefined like Chrome's, though `in` diverges (true here) — the shared
// prototype is the price of keeping property patches on one object. Must run
// after `class Element` initializes (TDZ), hence the placement down here.
Object.defineProperty(Element.prototype, 'labels', {
  get() {
    if (!this.matches || !this.matches(_LABELABLE)) return _nodeList([]);
    const out = [];
    const doc = this.ownerDocument || globalThis.document;
    if (doc && doc.querySelectorAll) {
      const all = doc.querySelectorAll('label');
      for (let i = 0; i < all.length; i++) {
        if (_labeledControl(all[i]) === this) out.push(all[i]);
      }
    }
    // Detached trees: the document scan can't see ancestor labels there, so
    // walk up (for-association inside a detached tree stays unlisted —
    // getElementById only resolves the document, same approximation as
    // _labeledControl's for-arm).
    for (let p = this.parentElement; p; p = p.parentElement) {
      if (p.tagName === 'LABEL' && _labeledControl(p) === this && out.indexOf(p) === -1) out.push(p);
    }
    return _nodeList(out);
  },
  configurable: true,
});
Object.defineProperty(Element.prototype, 'control', {
  get() { return this.tagName === 'LABEL' ? _labeledControl(this) : undefined; },
  configurable: true,
});
// HTMLStyleElement.sheet / HTMLLinkElement.sheet (tag-gated, the same
// approximation the control getter uses for LABEL). #121: CKEditor's
// appendStyleText hands `styleEl.sheet` back to page code that reads
// `.ownerNode` on it.
Object.defineProperty(Element.prototype, 'sheet', {
  get() { return _elementSheet(this); },
  configurable: true,
});

Object.defineProperty(Element.prototype, 'assignedSlots', {
  get() {
    const s = +_dom("assigned_slot", this._nid);
    const w = s >= 0 ? _wrap(s) : null;
    return w ? [w] : [];
  },
  configurable: true,
});

// A native shadow root: a real arena node with its own child list (kept out
// of the host's light-tree links — see attach_shadow_root on the Rust side),
// so the inherited nid-based Node methods (innerHTML, appendChild,
// querySelector, ...) operate on the shadow tree directly. Constructed only
// by Element.prototype.attachShadow; there is no public constructor.
globalThis.ShadowRoot = class ShadowRoot extends DocumentFragment {
  constructor(nid, host, options) {
    if (host == null) throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor");
    super(nid);
    this._host = host;
    this._mode = options && options.mode === 'closed' ? 'closed' : 'open';
    this._delegatesFocus = !!(options && options.delegatesFocus);
    this._slotAssignment = options && options.slotAssignment === 'manual' ? 'manual' : 'named';
    this._clonable = !!(options && options.clonable);
    this._serializable = !!(options && options.serializable);
    __hideOwn(this);
  }
  get host() { return this._host; }
  get mode() { return this._mode; }
  // 刀57: ShadowRoot owns its innerHTML pair (Chrome truth: ShadowRoot.
  // prototype has it rw:ec; DocumentFragment.prototype has none, so the
  // inherited copy had to become ShadowRoot's own — template.content and
  // plain fragments answer undefined, exactly like Chrome 145).
  get innerHTML() { return _domParse("inner_html", this._nid) ?? ""; }
  set innerHTML(v) { _dom("set_inner_html", this._nid, String(v ?? "")); }
  get delegatesFocus() { return this._delegatesFocus; }
  get slotAssignment() { return this._slotAssignment; }
  get clonable() { return this._clonable; }
  get serializable() { return this._serializable; }
  get activeElement() { return null; }
  get styleSheets() { return []; }
  cloneNode() { throw new DOMException('Failed to execute cloneNode on Node: ShadowRoot nodes are not clonable.', 'NotSupportedError'); }
  setHTMLUnsafe(v) { this.innerHTML = String(v == null ? "" : v); }
  getHTML() { return this.innerHTML; }
};
// Constructible-stylesheet adoption, mirroring Document.adoptedStyleSheets.
Object.defineProperty(globalThis.ShadowRoot.prototype, 'adoptedStyleSheets', {
  get() { return this._adoptedStyleSheets || []; },
  set(sheets) { this._adoptedStyleSheets = globalThis.__adoptedSet(this, sheets); },
  configurable: true,
});
// 刀66: ShadowRoot's five Chrome-145 members (probes k66-*): the three
// element getters answer null on this engine (it never enters fullscreen,
// pointer lock or picture-in-picture — fullscreenElement keeps Chrome's rw
// shape with a no-throw no-op setter); onslotchange is the __onHandlers
// on* pattern; getSelection returns a Selection CACHED PER ROOT, distinct
// from the document's and from other roots' (same reference every call),
// starting empty (anchorNode null, type 'None').
(() => {
  const SRP = globalThis.ShadowRoot.prototype;
  const guard = function () { if (!(this instanceof globalThis.ShadowRoot)) throw new TypeError('Illegal invocation'); };
  const g = (name, get, set) => {
    Object.defineProperty(get, 'name', { value: 'get ' + name, configurable: true });
    if (set) { Object.defineProperty(set, 'name', { value: 'set ' + name, configurable: true }); Object.defineProperty(set, 'length', { value: 1, configurable: true }); }
    const d = { enumerable: true, configurable: true, get };
    if (set) d.set = set;
    Object.defineProperty(SRP, name, d);
  };
  g('fullscreenElement', function () { guard.call(this); return null; }, function (v) { guard.call(this); });
  g('pictureInPictureElement', function () { guard.call(this); return null; }, null);
  g('pointerLockElement', function () { guard.call(this); return null; }, null);
  g('onslotchange',
    function () { guard.call(this); const s = this.__onHandlers; return s && Object.prototype.hasOwnProperty.call(s, 'onslotchange') ? s.onslotchange : null; },
    function (fn) { guard.call(this); if (typeof fn !== 'function') fn = null; const s = this.__onHandlers || (this.__onHandlers = {}); s.onslotchange = fn; });
  const sels = new WeakMap();
  const gs = function getSelection() {
    guard.call(this);
    let s = sels.get(this);
    if (!s) { s = new Selection(this); sels.set(this, s); }
    return s;
  };
  Object.defineProperty(SRP, 'getSelection', { value: gs, writable: true, enumerable: true, configurable: true });
})();

globalThis.CharacterData = CharacterData;
globalThis.Text = Text;
globalThis.Comment = Comment;

globalThis.CDATASection = CDATASection;
globalThis.ProcessingInstruction = ProcessingInstruction;
// True when the document was loaded from an XML/XHTML source. Obscura has no
// native XML tree, so this is inferred from contentType (derived from the URL).
function _isXMLDocument(doc) {
  const ct = (doc && doc.contentType) || "text/html";
  return ct !== "text/html";
}
// XML Name production, sufficient for createProcessingInstruction targets.
const _piNameStart = "A-Za-z_:\\u00C0-\\u00D6\\u00D8-\\u00F6\\u00F8-\\u02FF\\u0370-\\u037D\\u037F-\\u1FFF\\u200C-\\u200D\\u2070-\\u218F\\u2C00-\\u2FEF\\u3001-\\uD7FF\\uF900-\\uFDCF\\uFDF0-\\uFFFD";
const _piNameChar = _piNameStart + "0-9.\\u00B7\\u0300-\\u036F\\u203F-\\u2040\\-";
const _piNameRe = new RegExp("^[" + _piNameStart + "][" + _piNameChar + "]*$");
function _isValidPITarget(target) {
  return typeof target === "string" && target.length > 0 && _piNameRe.test(target);
}
globalThis.DocumentFragment = DocumentFragment;
globalThis.DocumentType = DocumentType;
globalThis.Node = Node;
globalThis.Element = Element;
globalThis.Document = Document;

// XMLDocument is a subclass of Document (DOMParser of an XML type and
// implementation.createDocument produce one). The interface must exist globally.
if (typeof XMLDocument === "undefined") globalThis.XMLDocument = class XMLDocument extends Document {};

// Storage Access API: top-level same-site documents hold storage access by
// default, so the promise resolves true (Chrome semantics in a first-party
// context). Lives on Document.prototype, inherited by XMLDocument and any
// document flavor.
Document.prototype.hasStorageAccess = function hasStorageAccess() {
  return Promise.resolve(true);
};
Document.prototype.requestStorageAccess = function requestStorageAccess() {
  return Promise.resolve(undefined);
};

// MathML elements ride the Element tree like SVG does; the interface itself
// rejects construction (instances only come from the parser).
globalThis.MathMLElement = class MathMLElement extends Element {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
};
// ParentNode mixin: Document and DocumentFragment are ParentNodes too, so they
// share Element's append / prepend / replaceChildren.
for (const _proto of [Document.prototype, DocumentFragment.prototype]) {
  _proto.append = Element.prototype.append;
  _proto.prepend = Element.prototype.prepend;
  _proto.replaceChildren = Element.prototype.replaceChildren;
}

// (#27) EventTarget used to be a bare `= Node` alias, which made the chain
// Node → Object — `el instanceof EventTarget` was false. Real chain: Node's
// prototype inherits from EventTarget.prototype. Node-level methods live on
// Node.prototype and below (they shadow, so node behavior is unchanged),
// while the four EventTarget-level members Chrome exposes —
// addEventListener/removeEventListener/dispatchEvent/when (300015 P4) — sit
// here for every class that extends EventTarget without overriding them
// (the audio contexts; Chrome truth: tools/webplatform/chrome/chrome-audio-truth.json).
// EventTarget is a NATIVE interface (crates/diting_runtime/src/native/
// event_target.rs): one dispatch algorithm, listener tables in private slots.
// Its host hooks (event path parent, timer-deferral bracket, error report)
// are registered by event_hooks.js.
Object.setPrototypeOf(Node.prototype, globalThis.EventTarget.prototype);

globalThis.DOMTokenList = DOMTokenList;

Element.prototype.attachShadow = function attachShadow(opts) {
  var _mode = opts == null ? undefined : opts.mode;
  if (_mode !== 'open' && _mode !== 'closed') {
    throw new TypeError('Failed to execute attachShadow on Element: the mode value is not a valid ShadowRootMode.');
  }
  var _ln = (this.localName || '').toLowerCase();
  if (!globalThis.__diting_shadowHostNames.has(_ln) && _ln.indexOf('-') === -1) {
    throw new DOMException('Failed to execute attachShadow on Element: this element does not support attachShadow', 'NotSupportedError');
  }
  if (this._shadowRoot) {
    throw new DOMException('Failed to execute attachShadow on Element: the element already hosts a shadow tree.', 'NotSupportedError');
  }
  // Native root: the op registers a real arena node (its own child list,
  // out of the host's light-tree links). The wrapper is cached under the
  // root nid BEFORE anything else can wrap it — _wrap dispatches on
  // node_type and a shadow root reads as 9/Document to it, so without this
  // entry every later access would mint a Document wrapper instead of
  // returning this ShadowRoot.
  const rootNid = _OPS.op_shadow_attach(this._nid, _mode);
  // -2 = the tree already has a root for this host (the wrapper-level
  // `_shadowRoot` check above misses a host whose wrapper was rebuilt).
  if (rootNid === -2) {
    throw new DOMException('Failed to execute attachShadow on Element: the element already hosts a shadow tree.', 'NotSupportedError');
  }
  if (rootNid < 0) {
    throw new DOMException('Failed to execute attachShadow on Element: this element does not support attachShadow', 'NotSupportedError');
  }
  const shadow = new ShadowRoot(rootNid, this, opts);
  _cache.set(rootNid, shadow);
  __def(this, '_shadowRoot', shadow);
  return shadow;
};

_markNative(Element.prototype.attachShadow);

Object.defineProperty(Element.prototype, 'shadowRoot', {
  configurable: true,
  enumerable: true,
  get: function () {
    var sr = this._shadowRoot;
    if (!sr) {
      // A root the parser attached (declarative shadow DOM) never ran
      // attachShadow, so the wrapper cache is cold — ask the tree once and
      // memoize, so later reads behave exactly like attachShadow's.
      var got = _OPS.op_dom('shadow_root_of', String(this._nid), '');
      if (got === '-1') return null;
      var parts = got.split('|');
      sr = new ShadowRoot(+parts[0], this, { mode: parts[1] });
      _cache.set(+parts[0], sr);
      __def(this, '_shadowRoot', sr);
    }
    return sr && sr.mode === 'open' ? sr : null;
  },
});

if (typeof Document !== 'undefined' && !Document.prototype.importNode) {
  Document.prototype.importNode = function(node, deep) { return node?.cloneNode(!!deep) || null; };
}
// Document.adoptNode: standard DOM (HTML living spec). Frameworks that move
// nodes between documents (portals, iframe hand-off) call it; the missing
// method throws "adoptNode is not a function". With no second document to
// transfer ownership from, the node is already ours, so return it as-is,
// matching the observable effect of adoption into this document.
if (typeof Document !== 'undefined' && !Document.prototype.adoptNode) {
  Document.prototype.adoptNode = function(node) { return node || null; };
  _markNative(Document.prototype.adoptNode);
}
// Element.toggleAttribute: standard DOM. Lit/Stencil and several ad SDKs call
// it; the missing method throws. Spec semantics: no force arg toggles, force
// true adds, force false removes; returns the new presence.
if (typeof Element !== 'undefined' && !Element.prototype.toggleAttribute) {
  Element.prototype.toggleAttribute = function(name, force) {
    const n = String(name);
    const present = this.hasAttribute(n);
    const want = arguments.length < 2 ? !present : !!force;
    if (want && !present) { this.setAttribute(n, ''); return true; }
    if (!want && present) { this.removeAttribute(n); return false; }
    return want;
  };
  _markNative(Element.prototype.toggleAttribute);
}

// Document.elementFromPoint / elementsFromPoint — hit testing against the
// element rects (real layout geometry under the screenshot build, the
// synthetic grid otherwise; see issue #63 for the original stub rationale).
// Flat iteration over every element, NOT a tree walk: rects don't always
// nest (a synthetic child rect can lie outside its parent's), so a walk
// that only descends into containing ancestors would miss deep elements.
//
// Which overlapping candidate wins is a PAINT question, not a document
// order question (obscura #738): a close button with z-index 1002 that
// precedes a z-index 1001 overlay in the DOM is the element the pixel
// shows. The Rust side exports the layout walk's paint order
// (_domRaw("paint_order")); candidates rank by it, ties (and the
// no-layout fallback) fall back to document order via nid. Boxless inline
// wrappers (span/a/label) never get their own slot — rank them with their
// nearest boxed ancestor, whose subtree painted their hoisted ink anyway.
if (typeof Document !== 'undefined' && !Document.prototype.elementFromPoint) {
  function __ditingPaintRanks() {
    try {
      var raw = _domRaw("paint_order", "", "");
      if (!raw) return null;
      var ids = JSON.parse(raw);
      if (!ids || !ids.length) return null;
      var rank = {};
      for (var i = 0; i < ids.length; i++) rank[ids[i]] = i;
      return rank;
    } catch (e) {
      return null;
    }
  }
  function __ditingPaintRankOf(el, rank) {
    var v = rank[el._nid | 0];
    if (v !== undefined) return v;
    for (var p = el.parentElement; p; p = p.parentElement) {
      var pv = rank[p._nid | 0];
      if (pv !== undefined) return pv;
    }
    return -1;
  }
  // #95: Chrome's hit test pierces shadow trees — closed roots included
  // (elementFromPoint returns shadow-internal elements, which is why a
  // human can press xhs's closed publish button in a real browser). One
  // arena scan (`shadow_hosts`) hands back every host→root pair for BOTH
  // attach paths (the attachShadow op and declarative parse), and each
  // root's subtree joins the flat candidate set below. Shadow ink paints
  // above its host, so paint order ranks the inner element first; without
  // a paint table the nid tiebreak does (a root's subtree is numbered
  // after its host). Root wrappers are minted exactly the way the
  // shadowRoot getter does, so identity never forks.
  function __ditingHitUniverse(all) {
    var table = null;
    try {
      var raw = _domRaw("shadow_hosts", "", "");
      if (raw && raw !== 'null') table = JSON.parse(raw);
    } catch (e) { return all; }
    if (!table || !table.length) return all;
    var byHost = {};
    for (var i = 0; i < table.length; i++) byHost[table[i][0]] = table[i];
    var out = Array.prototype.slice.call(all);
    // Pushing while iterating: a shadow subtree that hosts another root
    // gets expanded when the walk reaches its elements.
    for (var oi = 0; oi < out.length; oi++) {
      var entry = byHost[out[oi]._nid | 0];
      if (!entry) continue;
      var root = _cache.get(entry[1]);
      if (!root) {
        root = new ShadowRoot(entry[1], out[oi], { mode: entry[2] });
        _cache.set(entry[1], root);
        __def(out[oi], '_shadowRoot', root);
      }
      try {
        var sub = root.querySelectorAll('*');
        for (var k = 0; k < sub.length; k++) out.push(sub[k]);
      } catch (e) {}
    }
    return out;
  }
  // WebIDL `double` arguments: Chrome throws on missing or non-finite
  // coordinates instead of answering null (JS-SURFACE-PLAN step 5 probe).
  function __ditingPointArgs(name, n, x, y) {
    if (n < 2) {
      throw new TypeError("Failed to execute '" + name + "' on 'Document': 2 arguments required, but only " + n + " present.");
    }
    x = Number(x); y = Number(y);
    if (!isFinite(x) || !isFinite(y)) {
      throw new TypeError("Failed to execute '" + name + "' on 'Document': The provided double value is non-finite.");
    }
  }
  Document.prototype.elementFromPoint = function(x, y) {
    __ditingPointArgs('elementFromPoint', arguments.length, x, y);
    var cands = __ditingHitCandidates.call(this, x, y);
    if (cands === null) return null;
    return cands.length ? cands[0] : (this.body || this.documentElement || null);
  };
  Document.prototype.elementsFromPoint = function(x, y) {
    __ditingPointArgs('elementsFromPoint', arguments.length, x, y);
    var cands = __ditingHitCandidates.call(this, x, y);
    if (cands === null) return [];
    if (!cands.length) {
      var el = this.elementFromPoint(x, y); // body fallback, as before
      if (el) cands.push(el);
    }
    // Chrome ends the stack with <body> then <html> (obscura PR #848): both
    // span the viewport, so putting them into the ranked candidate set
    // would shadow every real descendant in elementFromPoint — hence
    // appended after the ranked descendants, never ranked among them. The
    // point was already bounds-checked against the viewport, so both
    // contain it.
    var roots = [this.body, this.documentElement];
    for (var ri = 0; ri < roots.length; ri++) {
      if (roots[ri] && cands.indexOf(roots[ri]) === -1) cands.push(roots[ri]);
    }
    return cands;
  };
  // Shared candidate walk: every non-root element whose rect contains
  // (x, y), sorted front-to-back by (paint rank, document order). Invalid
  // coordinates return null so both entry points can distinguish "miss"
  // from "empty list".
  function __ditingHitCandidates(x, y) {
    if (typeof x !== 'number' || typeof y !== 'number' || !isFinite(x) || !isFinite(y)) {
      return null;
    }
    var w = (typeof window !== 'undefined' && window.innerWidth) || 1280;
    var h = (typeof window !== 'undefined' && window.innerHeight) || 720;
    if (x < 0 || y < 0 || x > w || y > h) return null;
    // Containment compares client-to-client: gBCR serves client-space
    // boxes (doc-space layout rects minus the root scroll, sticky shift
    // included), so the raw query point is already in the right space —
    // no scroll ride. The local_geom inverse-map below still needs the
    // DOC-space point (layout geometry is scroll-blind), and only the
    // main document rides the root scroll — frame content scroll is not
    // mirrored yet, so borrowing the MAIN page's scroll would poison
    // every in-frame hit test.
    var mainDoc = (typeof document !== 'undefined' && this === document);
    var qx = x;
    var qy = y;
    var gx = mainDoc ? x + (globalThis.scrollX || 0) : x;
    var gy = mainDoc ? y + (globalThis.scrollY || 0) : y;
    var all = __ditingHitUniverse(this.querySelectorAll('*'));
    var rank = __ditingPaintRanks();
    var cands = [];
    for (var i = 0; i < all.length; i++) {
      var el = all[i];
      if (!el || !el.getBoundingClientRect) continue;
      // documentElement / body span the viewport; skip them so we pick a
      // real descendant instead of falling back to <html>/<body>.
      if (el === this.documentElement || el === this.body) continue;
      var r = el.getBoundingClientRect();
      if (r.width === 0 || r.height === 0) continue;
      // Half-open [left,right)×[top,bottom): Chrome / obscura #1164 exclude
      // the right and bottom edges so adjacent boxes don't double-hit.
      if (qx >= r.left && qx < r.right && qy >= r.top && qy < r.bottom) {
        // Transform-tainted boxes (blitz #663 family): a rotated/skewed
        // element's mapped bounding box swallows corners no pixel of the
        // element covers. With local geometry available and a non-diagonal
        // total map, inverse-map the hit point (same space the mapped
        // rects live in) and keep only true-shape containment; diagonal
        // maps keep the AABB path, which is exact for them.
        var outside = false;
        try {
          if (el._nid !== undefined) {
            var lgraw = _domRaw('local_geom', String(el._nid | 0), '');
            if (lgraw && lgraw !== 'null') {
              var g = JSON.parse(lgraw);
              if (g.length === 10 && (g[5] !== 0 || g[6] !== 0)) {
                var det = g[4] * g[7] - g[5] * g[6];
                if (det !== 0) {
                  var lx = (g[7] * (gx - g[8]) - g[6] * (gy - g[9])) / det;
                  var ly = (g[4] * (gy - g[9]) - g[5] * (gx - g[8])) / det;
                  if (lx < g[0] || lx >= g[0] + g[2] || ly < g[1] || ly >= g[1] + g[3]) {
                    outside = true;
                  }
                }
              }
            }
          }
        } catch (e) { /* geometry unavailable — keep the AABB verdict */ }
        if (!outside) cands.push(el);
      }
    }
    if (!cands.length) return cands;
    cands.sort(function(a, b) {
      // No paint table (default build / synthetic grid): document order,
      // deepest (highest nid) wins — the pre-#738 behavior.
      if (!rank) return (b._nid | 0) - (a._nid | 0);
      return __ditingPaintRankOf(b, rank) - __ditingPaintRankOf(a, rank)
        || (b._nid | 0) - (a._nid | 0);
    });
    return cands;
  }
}
if (typeof ShadowRoot !== 'undefined' && !ShadowRoot.prototype.elementFromPoint) {
  ShadowRoot.prototype.elementFromPoint = function(x, y) {
    return Document.prototype.elementFromPoint.call(globalThis.document || this, x, y);
  };
  ShadowRoot.prototype.elementsFromPoint = function(x, y) {
    return Document.prototype.elementsFromPoint.call(globalThis.document || this, x, y);
  };
}

/* ===== WPT conformance shims: batch 2 ===== */

// ---- Node namespace lookup methods ----

Node.prototype.lookupNamespaceURI = function(prefix) {
  let node = this;
  if (node.nodeType === 9) node = node.documentElement;
  if (!node || node.nodeType !== 1) return null;
  const _ns_builtins = { 'xml': 'http://www.w3.org/XML/1998/namespace', 'xmlns': 'http://www.w3.org/2000/xmlns/' };
  if (prefix && _ns_builtins[prefix]) return _ns_builtins[prefix];
  while (node && node.nodeType === 1) {
    if (prefix) {
      if (node.prefix === prefix && node.namespaceURI) return node.namespaceURI;
      const nsAttr = node.getAttribute('xmlns:' + prefix);
      if (nsAttr !== null) return nsAttr || null;
    } else {
      const defaultNs = node.getAttribute('xmlns');
      if (defaultNs !== null) return defaultNs || null;
      if (node.prefix === null && node.namespaceURI) return node.namespaceURI;
    }
    node = node.parentElement;
  }
  return null;
};
_markNative(Node.prototype.lookupNamespaceURI);

Node.prototype.lookupPrefix = function(namespace) {
  namespace = namespace || null;
  let node = this;
  if (node.nodeType === 9) node = node.documentElement;
  if (!node || node.nodeType !== 1) return null;
  const _ns_builtins = { 'http://www.w3.org/XML/1998/namespace': 'xml', 'http://www.w3.org/2000/xmlns/': 'xmlns' };
  if (_ns_builtins[namespace]) return _ns_builtins[namespace];
  while (node && node.nodeType === 1) {
    if (node.namespaceURI === namespace) {
      const p = node.prefix;
      if (p) return p;
    }
    const attrs = node.attributes || [];
    for (let i = 0; i < attrs.length; i++) {
      const attr = attrs[i];
      const attrName = attr.name || attr.nodeName || '';
      const attrValue = attr.value || attr.nodeValue || '';
      if (attrName === 'xmlns' && attrValue === namespace) return '';
      if (attrName.startsWith('xmlns:')) {
        const prefix = attrName.substring(6);
        if (attrValue === namespace) return prefix;
      }
    }
    node = node.parentElement;
  }
  return null;
};
_markNative(Node.prototype.lookupPrefix);

Node.prototype.isDefaultNamespace = function(namespace) {
  return this.lookupNamespaceURI(null) === (namespace || null);
};
_markNative(Node.prototype.isDefaultNamespace);

// ---- getElementsByTagNameNS on Element and Document ----
// getElementsByTagNameNS on Element and Document
if (!Element.prototype.getElementsByTagNameNS) {
  Element.prototype.getElementsByTagNameNS = function(namespaceURI, localName) {
    const all = this.querySelectorAll('*');
    const filtered = [];
    const nsMatch = namespaceURI === '*';
    const tagMatch = localName === '*';
    for (let i = 0; i < all.length; i++) {
      const el = all[i];
      if (!el) continue;
      const elNs = el.namespaceURI;
      const elTag = el.localName;
      const nsOk = nsMatch || (elNs === (namespaceURI || null));
      const tagOk = tagMatch || (elTag === localName);
      if (nsOk && tagOk) filtered.push(el);
    }
    return __diting_mkCollection(filtered);
  };
  _markNative(Element.prototype.getElementsByTagNameNS);
}
if (!Document.prototype.getElementsByTagNameNS) {
  Document.prototype.getElementsByTagNameNS = function(namespaceURI, localName) {
    const all = this.querySelectorAll('*');
    const filtered = [];
    const nsMatch = namespaceURI === '*';
    const tagMatch = localName === '*';
    for (let i = 0; i < all.length; i++) {
      const el = all[i];
      if (!el) continue;
      const elNs = el.namespaceURI;
      const elTag = el.localName;
      const nsOk = nsMatch || (elNs === (namespaceURI || null));
      const tagOk = tagMatch || (elTag === localName);
      if (nsOk && tagOk) filtered.push(el);
    }
    return __diting_mkCollection(filtered);
  };
  _markNative(Document.prototype.getElementsByTagNameNS);
}

// ---- Attr nodes and createAttribute ----
// Attr class: represents attribute nodes (nodeType 2)
if (!globalThis.Attr) {
  // Internal construction token: Chrome's Attr has an illegal public constructor (nodes come from createAttribute / getAttributeNode).
  const _ATTR_TOK = Symbol('attr');
  globalThis.__diting_mkAttr = (name, value, ns, prefix) => new globalThis.Attr(name, value, ns, prefix, _ATTR_TOK);
  globalThis.Attr = class Attr {
    constructor(name, value = '', namespaceURI = null, prefix = null, tok) {
      if (tok !== _ATTR_TOK) throw new TypeError("Failed to construct 'Attr': Illegal constructor");
      const as = __diting_liftSlot(this);
      as.name = name;
      as.localName = name;
      this._v = String(value);
      as.namespaceURI = namespaceURI;
      as.prefix = prefix;
      as.ownerElement = null;
      as.specified = true;
    }
    // Live while attached (Chrome: an Attr and its element's attribute are one thing): reads/writes go through the owner element.
    get value() {
      const o = this.ownerElement;
      if (o) { const v = o.getAttribute(this.name); if (v !== null) return v; }
      return this._v;
    }
    set value(v) {
      v = String(v);
      this._v = v;
      const o = this.ownerElement;
      if (o) o.setAttribute(this.name, v);
    }
    get nodeName() { return this.name; }
    get nodeValue() { return this.value; }
    set nodeValue(v) { this.value = v; }
    get nodeType() { return 2; }
    // 刀56: the 11 Node-reflecting members below are HOISTED at load by
    // nodelevel.impl.js into its per-prototype dispatch table (deleted from
    // this prototype) — Node.prototype's members would otherwise throw for
    // an Attr (no DOM node id), and Chrome does not list any of these on
    // Attr.prototype. An attribute has no parent, children or siblings; its
    // text is its value.
    get parentNode() { return null; }
    get parentElement() { return null; }
    get firstChild() { return null; }
    get lastChild() { return null; }
    get previousSibling() { return null; }
    get nextSibling() { return null; }
    get childNodes() { return _nodeList([]); }
    hasChildNodes() { return false; }
    get isConnected() { return false; }
    get ownerDocument() { return globalThis.document; }
    get textContent() { return this.value; }
    set textContent(v) { this.value = v === null ? '' : String(v); }
  };
  // Chrome: Attr is a Node. Its id-based Node members are overridden above (an Attr has no DOM node id).
  Object.setPrototypeOf(globalThis.Attr.prototype, Node.prototype);
}

// XML Name validation helper for attribute/processing instruction names
const _ns_isValidXmlName = (name) => {
  if (typeof name !== 'string' || !name.length) return false;
  return /^[A-Za-z_:][\w.\-:]*$/.test(name);
};

// Document.prototype.createAttribute: create a detached Attr node
if (!Document.prototype.createAttribute) {
  Document.prototype.createAttribute = function(localName) {
    const name = String(localName || '');
    if (!_ns_isValidXmlName(name)) {
      throw new DOMException('Invalid attribute name', 'InvalidCharacterError');
    }
    return __diting_mkAttr(name, '', null, null);
  };
  _markNative(Document.prototype.createAttribute);
}

// Document.prototype.createAttributeNS: create a namespaced Attr node
if (!Document.prototype.createAttributeNS) {
  Document.prototype.createAttributeNS = function(namespaceURI, qualifiedName) {
    const ns = namespaceURI ? String(namespaceURI) : null;
    const qn = String(qualifiedName || '');
    if (!qn.length) {
      throw new DOMException('Invalid attribute name', 'InvalidCharacterError');
    }
    let prefix = null;
    let localName = qn;
    const colonIdx = qn.indexOf(':');
    if (colonIdx !== -1) {
      prefix = qn.substring(0, colonIdx);
      localName = qn.substring(colonIdx + 1);
      if (!_ns_isValidXmlName(prefix) || !_ns_isValidXmlName(localName)) {
        throw new DOMException('Invalid attribute name', 'InvalidCharacterError');
      }
    } else {
      if (!_ns_isValidXmlName(localName)) {
        throw new DOMException('Invalid attribute name', 'InvalidCharacterError');
      }
    }
    const attr = __diting_mkAttr(qn, '', ns, prefix);
    __diting_liftSlot(attr).localName = localName;
    return attr;
  };
  _markNative(Document.prototype.createAttributeNS);
}

// Element.prototype.getAttributeNode: return an Attr node or null
if (!Element.prototype.getAttributeNode) {
  Element.prototype.getAttributeNode = function(name) {
    const val = this.getAttribute(name);
    if (val === null) return null;
    const attr = __diting_mkAttr(name, val, null, null);
    __diting_liftSlot(attr).ownerElement = this;
    return attr;
  };
  _markNative(Element.prototype.getAttributeNode);
}

// Element.prototype.getAttributeNodeNS: return a namespaced Attr node or null
if (!Element.prototype.getAttributeNodeNS) {
  Element.prototype.getAttributeNodeNS = function(namespaceURI, localName) {
    const val = this.getAttributeNS(namespaceURI, localName);
    if (val === null) return null;
    const name = String(localName || '');
    const attr = __diting_mkAttr(name, val, namespaceURI ? String(namespaceURI) : null, null);
    __diting_liftSlot(attr).ownerElement = this;
    return attr;
  };
  _markNative(Element.prototype.getAttributeNodeNS);
}

// Element.prototype.setAttributeNode: set an Attr and return the previous one
if (!Element.prototype.setAttributeNode) {
  Element.prototype.setAttributeNode = function(attr) {
    if (!attr || typeof attr.name !== 'string') return null;
    const prevVal = this.getAttribute(attr.name);
    const prevAttr = prevVal !== null ? __diting_mkAttr(attr.name, prevVal, null, null) : null;
    if (prevAttr) __diting_liftSlot(prevAttr).ownerElement = this;
    this.setAttribute(attr.name, attr.value);
    __diting_liftSlot(attr).ownerElement = this;
    return prevAttr;
  };
  _markNative(Element.prototype.setAttributeNode);
}

// Element.prototype.setAttributeNodeNS: set a namespaced Attr and return the previous one
if (!Element.prototype.setAttributeNodeNS) {
  Element.prototype.setAttributeNodeNS = function(attr) {
    if (!attr || typeof attr.name !== 'string') return null;
    const prevVal = this.getAttribute(attr.name);
    const prevAttr = prevVal !== null 
      ? __diting_mkAttr(attr.name, prevVal, attr.namespaceURI || null, attr.prefix || null) 
      : null;
    if (prevAttr) __diting_liftSlot(prevAttr).ownerElement = this;
    this.setAttributeNS(attr.namespaceURI || null, attr.name, attr.value);
    __diting_liftSlot(attr).ownerElement = this;
    return prevAttr;
  };
  _markNative(Element.prototype.setAttributeNodeNS);
}

// Element.prototype.removeAttributeNode: remove and return an Attr
if (!Element.prototype.removeAttributeNode) {
  Element.prototype.removeAttributeNode = function(attr) {
    if (!attr || typeof attr.name !== 'string') return attr;
    const val = this.getAttribute(attr.name);
    if (val !== null) {
      this.removeAttribute(attr.name);
    }
    return attr;
  };
  _markNative(Element.prototype.removeAttributeNode);
}

// Chrome exposes `viewBox` on the fit-to-viewbox SVG elements (svg, marker,
// pattern, view) as an animated rect — never undefined on those elements, and
// an all-zero baseVal when the attribute is absent or malformed. Export and
// rasterize code leans on exactly that shape (`svg.viewBox.baseVal.width`),
// so a missing reflection reads as a crash one property later.
Object.defineProperty(Element.prototype, "viewBox", {
  configurable: true,
  get() {
    const ln = this.localName;
    if (ln !== "svg" && ln !== "marker" && ln !== "pattern" && ln !== "view") {
      return undefined;
    }
    const parts = (this.getAttribute("viewBox") || "").trim().split(/[\s,]+/);
    let v = [0, 0, 0, 0];
    if (parts.length === 4 &&
        parts.every((p) => p !== "" && !isNaN(Number(p)))) {
      v = parts.map(Number);
    }
    const rect = { x: v[0], y: v[1], width: v[2], height: v[3] };
    return { baseVal: rect, animVal: rect };
  },
});

// The DOM slice of bootstrap's big mark list, re-minted here. Same evaluated
// set as before the split: every entry below was already defined by this
// point in the old single file (the entries that were not — attachShadow,
// the canvas trio — were silently skipped by the typeof guard and are marked
// at their real install sites instead).
[
  Element.prototype.getAttribute, Element.prototype.setAttribute,
  Element.prototype.removeAttribute, Element.prototype.hasAttribute,
  Element.prototype.querySelector, Element.prototype.querySelectorAll,
  Element.prototype.getElementsByTagName, Element.prototype.getElementsByClassName,
  Element.prototype.matches, Element.prototype.closest,
  Element.prototype.getBoundingClientRect, Element.prototype.getClientRects,
  Element.prototype.checkVisibility,
  Element.prototype.addEventListener, Element.prototype.removeEventListener,
  Element.prototype.dispatchEvent, Element.prototype.click,
  Element.prototype.focus, Element.prototype.blur,
  Element.prototype.showPopover, Element.prototype.hidePopover, Element.prototype.togglePopover,
  Element.prototype.cloneNode,
  Element.prototype.insertAdjacentHTML, Element.prototype.insertAdjacentText,
  Element.prototype.scrollIntoView,
  Element.prototype.scrollTo, Element.prototype.scrollBy, Element.prototype.scroll,
  Element.prototype.append, Element.prototype.prepend, Element.prototype.remove,
  Element.prototype.before, Element.prototype.after, Element.prototype.replaceWith,
  Node.prototype.appendChild, Node.prototype.removeChild,
  Node.prototype.replaceChild, Node.prototype.insertBefore,
  Node.prototype.contains, Node.prototype.hasChildNodes, Node.prototype.cloneNode,
  CharacterData.prototype.before, CharacterData.prototype.after,
  CharacterData.prototype.replaceWith, CharacterData.prototype.remove,
  Document.prototype.getElementById, Document.prototype.querySelector,
  Document.prototype.querySelectorAll, Document.prototype.getElementsByTagName,
  Document.prototype.createElement, Document.prototype.createElementNS,
  Document.prototype.createTextNode, Document.prototype.createComment,
  Document.prototype.createCDATASection, Document.prototype.createProcessingInstruction,
  Document.prototype.createDocumentFragment, Document.prototype.createEvent,
  Document.prototype.hasFocus,
].forEach(fn => { if (typeof fn === 'function') _markNative(fn); });
// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('CDATASection', CDATASection);
  d('CharacterData', CharacterData);
  d('Comment', Comment);
  d('DOMTokenList', DOMTokenList);
  d('Document', Document);
  d('DocumentFragment', DocumentFragment);
  d('DocumentType', DocumentType);
  d('Element', Element);
  d('Node', Node);
  d('ProcessingInstruction', ProcessingInstruction);
  d('REFERRER_POLICIES', REFERRER_POLICIES);
  d('Text', Text);
  d('_cache', _cache);
  d('_docBase', _docBase);
  d('_docEncoding', _docEncoding);
  d('_elementClassFor', _elementClassFor);
  d('_isValidPITarget', _isValidPITarget);
  d('_tagInterfaces', _tagInterfaces);
  d('_wrap', _wrap);
  d('_wrapEl', _wrapEl);
}
// @@EXPORTS-END
// @@DRAIN-ANCHOR god-file split drain: bootstrap queued its two Element/
// Document on* install loops (touching these names at what would be load
// time); run them now that this file's exports block has published the
// classes — still ahead of the gen patches that prune the on* surface down
// to Chrome's prototype levels (install-then-prune, the original order).
(function () {
  for (const fn of (globalThis.__diting_dom_deferred || [])) fn();
})();
}).call(globalThis); // @@SCOPE-END
