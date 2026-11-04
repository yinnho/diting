(function () { // @@SCOPE
// god-file split: bootstrap's real native marker, reached through its
// hidden prop (house style — see ctor_ladders.js) so no cross-file load
// edge appears in the dependency ratchet.
const _markNative = globalThis.__diting_markNative;

// Conservative XML well-formedness check for DOMParser (upstream 53295fa →
// 20c4628 final form). Only clear errors are flagged: tag mismatch, extra
// closing tag, extra root content, unclosed tags. Self-closing tags count as
// complete elements (869f700) and never push the stack.
const _checkXmlWellFormed = (html) => {
  // Strip comments, CDATA sections, processing instructions, and DOCTYPE
  // declarations — they may contain angle brackets.
  const s = html
    .replace(/<!--[\s\S]*?-->/g, '')
    .replace(/<!\[CDATA\[[\s\S]*?\]\]>/g, '')
    .replace(/<\?[\s\S]*?\?>/g, '')
    .replace(/<!DOCTYPE\s[^>]*?>/gi, '');

  const stack = [];
  // Match open / close / self-closing tags.
  // Group 1: tag name.  Group 2: optional '/' before '>'.
  const tagRe = /<\/?([a-zA-Z_][\w.\-:]*)(?:\s[^>]*?)?(\/)?>/g;
  let rootFound = false;
  let match;

  while ((match = tagRe.exec(s)) !== null) {
    const fullTag = match[0];
    const tagName = match[1];
    const isClosing = fullTag.startsWith('</');
    const isSelfClosing = match[2] === '/';

    if (isClosing) {
      if (stack.length === 0) {
        return { wellFormed: false, error: 'error on line 1: extra closing tag </' + tagName + '>' };
      }
      const open = stack.pop();
      if (open !== tagName) {
        return { wellFormed: false, error: 'error on line 1: opening and ending tag mismatch: ' + open + ' and ' + tagName };
      }
      if (stack.length === 0) rootFound = true;
    } else {
      // Opening or self-closing tag. Check for extra content after root.
      if (stack.length === 0 && rootFound) {
        return { wellFormed: false, error: 'error on line 1: extra content after root element' };
      }
      if (isSelfClosing) {
        // Self-closing: complete element, mark rootFound if at root level.
        if (stack.length === 0) rootFound = true;
      } else {
        stack.push(tagName);
      }
    }
  }

  if (stack.length > 0) {
    return { wellFormed: false, error: 'error on line 1: unclosed tag <' + stack[stack.length - 1] + '>' };
  }

  return { wellFormed: true };
};

// Stricter hand-rolled state machine: quote-aware '>' scanning, unterminated
// tags, and exactly one fully closed root element. Catches cases the regex
// pass above cannot (e.g. '>' inside attribute values, zero/multiple roots).
function _xmlWellFormed(src) {
  const s = String(src);
  const stack = [];
  let rootsClosed = 0; // top-level elements fully closed (or self-closed)
  let i = 0;
  const n = s.length;
  while (i < n) {
    const lt = s.indexOf('<', i);
    if (lt === -1) break;
    i = lt;
    if (s.startsWith('<!--', i)) { const e = s.indexOf('-->', i + 4); if (e === -1) return false; i = e + 3; continue; }
    if (s.startsWith('<![CDATA[', i)) { const e = s.indexOf(']]>', i + 9); if (e === -1) return false; i = e + 3; continue; }
    if (s.startsWith('<?', i)) { const e = s.indexOf('?>', i + 2); if (e === -1) return false; i = e + 2; continue; }
    if (s.startsWith('<!', i)) { const e = s.indexOf('>', i + 2); if (e === -1) return false; i = e + 1; continue; }
    // A start/end/self-closing tag: find its '>' while skipping quoted regions.
    let j = i + 1, quote = null;
    while (j < n) {
      const c = s[j];
      if (quote) { if (c === quote) quote = null; }
      else if (c === '"' || c === "'") quote = c;
      else if (c === '>') break;
      j++;
    }
    if (j >= n) return false; // unterminated tag
    const inner = s.slice(i + 1, j).trim();
    i = j + 1;
    if (!inner) return false;
    if (inner[0] === '/') {
      const name = inner.slice(1).trim().split(/\s/)[0];
      if (stack.length === 0 || stack[stack.length - 1] !== name) return false;
      stack.pop();
      if (stack.length === 0) rootsClosed++;
    } else if (inner[inner.length - 1] === '/') {
      if (stack.length === 0) rootsClosed++;
    } else {
      const name = inner.split(/\s/)[0];
      if (!name) return false;
      stack.push(name);
    }
  }
  return stack.length === 0 && rootsClosed === 1;
}

// XML entity decode for text and attribute values: the five predefined
// entities plus decimal/hex character references. Unknown named entities keep
// their literal text (strict XML would reject them; we stay conservative).
function _xmlDecodeEntities(s) {
  return s.replace(/&(#x?[0-9a-fA-F]+|[a-zA-Z][a-zA-Z0-9]*);/g, (m, body) => {
    if (body.charCodeAt(0) === 35) {
      let cp;
      if (body.charCodeAt(1) === 120 || body.charCodeAt(1) === 88) cp = parseInt(body.slice(2), 16);
      else cp = parseInt(body.slice(1), 10);
      if (!Number.isFinite(cp) || cp < 0 || cp > 0x10FFFF || (cp >= 0xD800 && cp <= 0xDFFF)) return "\ufffd";
      try { return String.fromCodePoint(cp); } catch (e) { return "\ufffd"; }
    }
    if (body === "amp") return "&";
    if (body === "lt") return "<";
    if (body === "gt") return ">";
    if (body === "quot") return '"';
    if (body === "apos") return "'";
    return m;
  });
}

// Build a real XML tree for DOMParser's XML mime types. Preconditions:
// _xmlWellFormed(src) already passed, so the state machine below sees one
// properly closed root and matched tags. What the HTML-parsing path cannot
// give us — and what obscura#883's callers inspect — lives here: the root
// keeps its source-case qualified name and namespace, self-closing tags
// produce siblings instead of unclosed ancestors, and xmlns/xmlns:prefix
// scopes resolve per element. Unbound prefixes resolve to the null namespace
// instead of erroring (namespace-well-formedness is stricter than the
// well-formedness gate we ran).
// Returns the root element wrapper, or null if anything unexpected shows up
// (the caller falls back to the parsererror path).
function _xmlBuildTree(src) {
  try {
    const s = String(src);
    const n = s.length;
    let root = null;
    const stack = [];       // open elements
    const scopes = [];      // parallel ns scopes: { def: string, pfx: Map }
    let i = 0;

    const nsFor = (qname, scope) => {
      const c = qname.indexOf(":");
      if (c === -1) return scope && scope.def !== undefined ? scope.def : "";
      if (!scope) return "";
      const v = scope.pfx.get(qname.slice(0, c));
      return v !== undefined ? v : "";
    };

    const mkEl = (qname, scope) => {
      const ns = nsFor(qname, scope);
      const el = _wrapEl(+_dom("create_element", qname, ns));
      if (!el) return null;
      el._ns = ns;
      const c = qname.indexOf(":");
      // localName must keep XML case (the prototype getter lowercases for
      // HTML); the cached _lname short-circuits it.
      el._lname = c === -1 ? qname : qname.slice(c + 1);
      return el;
    };

    const setAttrs = (el, attrPairs) => {
      for (let k = 0; k < attrPairs.length; k += 2) {
        try { el.setAttribute(attrPairs[k], attrPairs[k + 1]); } catch (e) { /* reserved name etc. */ }
      }
    };

    while (i < n) {
      if (s.startsWith("<!--", i)) {
        const e = s.indexOf("-->", i + 4);
        if (e === -1) return null;
        const parent = stack.length ? stack[stack.length - 1] : null;
        // Top-level comments are dropped: the builder's root doubles as
        // documentElement, so a leading comment must not claim its slot.
        if (parent) parent.appendChild(c);
        i = e + 3;
        continue;
      }
      if (s.startsWith("<![CDATA[", i)) {
        const e = s.indexOf("]]>", i + 9);
        if (e === -1) return null;
        const parent = stack.length ? stack[stack.length - 1] : null;
        if (!parent) return null;
        const data = s.slice(i + 9, e);
        const nid = +_dom("create_text_node", data);
        const cdata = new CDATASection(nid);
        _cache.set(nid, cdata); // keep the class across re-wraps (nodeType 4)
        parent.appendChild(cdata);
        i = e + 3;
        continue;
      }
      if (s.startsWith("<?", i)) {
        const e = s.indexOf("?>", i + 2);
        if (e === -1) return null;
        const body = s.slice(i + 2, e);
        const sp = body.search(/[\s]/);
        const target = sp === -1 ? body : body.slice(0, sp);
        const piData = sp === -1 ? "" : body.slice(sp + 1);
        const parent = stack.length ? stack[stack.length - 1] : null;
        if (parent && target) {
          const pid = +_dom("create_text_node", piData);
          const pin = new ProcessingInstruction(pid, target);
          _cache.set(pid, pin);
          parent.appendChild(pin);
        }
        i = e + 2;
        continue;
      }
      if (s.startsWith("<!", i)) {
        // DOCTYPE and other declarations: skipped (well-formed gate allowed
        // them; the document keeps its null _docType as before).
        const e = s.indexOf(">", i + 2);
        if (e === -1) return null;
        i = e + 1;
        continue;
      }
      if (s.startsWith("</", i)) {
        const e = s.indexOf(">", i + 2);
        if (e === -1) return null;
        if (stack.length) {
          stack.pop();
          scopes.pop();
        }
        i = e + 1;
        continue;
      }
      if (s.charCodeAt(i) === 60) { // '<'
        let j = i + 1;
        while (j < n && !/[\s\/>]/.test(s[j])) j++;
        const qname = s.slice(i + 1, j);
        if (!qname) return null;
        // Attributes: name, then '=' with a quoted or bare value (well-formed
        // XML requires quotes; bare kept as a safety net).
        const attrs = [];
        let selfClosing = false;
        while (j < n) {
          while (j < n && /[\s]/.test(s[j])) j++;
          if (j >= n) return null;
          if (s[j] === ">") { j++; break; }
          if (s[j] === "/" && s[j + 1] === ">") { selfClosing = true; j += 2; break; }
          let an = j;
          while (j < n && !/[\s=\/>]/.test(s[j])) j++;
          const aName = s.slice(an, j);
          if (!aName) return null;
          let aVal = "";
          while (j < n && /[\s]/.test(s[j])) j++;
          if (s[j] === "=") {
            j++;
            while (j < n && /[\s]/.test(s[j])) j++;
            if (s[j] === '"' || s[j] === "'") {
              const q = s[j]; j++;
              const e = s.indexOf(q, j);
              if (e === -1) return null;
              aVal = s.slice(j, e);
              j = e + 1;
            } else {
              let e = j;
              while (e < n && !/[\s>]/.test(s[e])) e++;
              aVal = s.slice(j, e);
              j = e;
            }
          }
          attrs.push(aName, _xmlDecodeEntities(aVal));
        }
        // This element's own xmlns declarations scope itself and children.
        // The default namespace inherits like the prefix map does: children
        // without their own xmlns stay in the ancestor's default namespace.
        let def = scopes.length ? scopes[scopes.length - 1].def : undefined;
        const pfx = scopes.length ? new Map(scopes[scopes.length - 1].pfx) : new Map();
        for (let k = 0; k < attrs.length; k += 2) {
          if (attrs[k] === "xmlns") def = attrs[k + 1];
          else if (attrs[k].startsWith("xmlns:")) pfx.set(attrs[k].slice(6), attrs[k + 1]);
        }
        const scope = { def, pfx };
        const el = mkEl(qname, scope);
        if (!el) return null;
        setAttrs(el, attrs);
        const parent = stack.length ? stack[stack.length - 1] : null;
        if (parent) parent.appendChild(el);
        else if (!root) root = el;
        else return null; // second root — the gate should have caught this
        if (!selfClosing) {
          stack.push(el);
          scopes.push(scope);
        }
        i = j;
        continue;
      }
      // Text run up to the next '<'.
      let e = s.indexOf("<", i);
      if (e === -1) e = n;
      if (stack.length) {
        const parent = stack[stack.length - 1];
        const txt = _xmlDecodeEntities(s.slice(i, e));
        if (txt) parent.appendChild(document.createTextNode(txt));
      }
      i = e;
    }
    return stack.length === 0 ? root : null;
  } catch (e) {
    return null;
  }
}

globalThis.XMLSerializer = class XMLSerializer {
  serializeToString(node) {
    if (!node) return "";
    if (node.nodeType === 10) {
      let s = "<!DOCTYPE " + (node.name || "html");
      if (node.publicId) s += ' PUBLIC "' + node.publicId + '"';
      if (node.systemId) {
        if (!node.publicId) s += " SYSTEM";
        s += ' "' + node.systemId + '"';
      }
      s += ">";
      return s;
    }
    if (node.outerHTML !== undefined) return node.outerHTML;
    if (node.nodeType === 9) {
      let s = "";
      if (node.doctype) s += this.serializeToString(node.doctype);
      if (node.documentElement) s += node.documentElement.outerHTML;
      return s;
    }
    if (node.nodeType === 3) return node.textContent || "";
    if (node.nodeType === 8) return "<!--" + (node.textContent || "") + "-->";
    return "";
  }
};


// This family's slice of bootstrap's big mark list, re-minted here (same
// evaluated set as before the split — the class is installed just above).
[
  XMLSerializer, XMLSerializer.prototype.serializeToString,
].forEach(fn => { if (typeof fn === 'function') _markNative(fn); });

// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('_checkXmlWellFormed', _checkXmlWellFormed);
  d('_xmlBuildTree', _xmlBuildTree);
  d('_xmlWellFormed', _xmlWellFormed);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
