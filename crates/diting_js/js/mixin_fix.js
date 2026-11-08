// Hand-written, then frozen: MathMLElement carries the HTMLOrSVGElement / GlobalEventHandlers members in
// Chrome (110 of them: every on* handler attribute plus dataset/style/tabIndex/autofocus/nonce/blur/click/focus),
// the engine's MathMLElement class was an empty shell. HTMLElement.prototype already implements all of them
// against the node id, so the missing ones are shared by descriptor (same getter/setter/method functions).
(function () {
  const WANT = ["onabort", "onbeforeinput", "onbeforematch", "onbeforetoggle", "onblur", "oncancel", "oncanplay", "oncanplaythrough", "onchange", "onclick", "onclose", "oncommand", "oncontentvisibilityautostatechange", "oncontextlost", "oncontextmenu", "oncontextrestored", "oncuechange", "ondblclick", "ondrag", "ondragend", "ondragenter", "ondragleave", "ondragover", "ondragstart", "ondrop", "ondurationchange", "onemptied", "onended", "onerror", "onfocus", "onformdata", "oninput", "oninvalid", "onkeydown", "onkeypress", "onkeyup", "onload", "onloadeddata", "onloadedmetadata", "onloadstart", "onmousedown", "onmouseenter", "onmouseleave", "onmousemove", "onmouseout", "onmouseover", "onmouseup", "onmousewheel", "onpause", "onplay", "onplaying", "onprogress", "onratechange", "onreset", "onresize", "onscroll", "onscrollend", "onsecuritypolicyviolation", "onseeked", "onseeking", "onselect", "onslotchange", "onstalled", "onsubmit", "onsuspend", "ontimeupdate", "ontoggle", "onvolumechange", "onwaiting", "onwebkitanimationend", "onwebkitanimationiteration", "onwebkitanimationstart", "onwebkittransitionend", "onwheel", "onauxclick", "ongotpointercapture", "onlostpointercapture", "onpointerdown", "onpointermove", "onpointerup", "onpointercancel", "onpointerover", "onpointerout", "onpointerenter", "onpointerleave", "onselectstart", "onselectionchange", "onanimationcancel", "onanimationend", "onanimationiteration", "onanimationstart", "ontransitionrun", "ontransitionstart", "ontransitionend", "ontransitioncancel", "onbeforexrselect", "oncopy", "oncut", "onpaste", "dataset", "nonce", "autofocus", "tabIndex", "style", "attributeStyleMap", "blur", "focus", "onscrollsnapchange", "onscrollsnapchanging", "onpointerrawupdate"];
  const PAIRS = [['MathMLElement', 'HTMLElement']];
  for (const [t, s] of PAIRS) {
    const T = globalThis[t], S = globalThis[s];
    if (typeof T !== 'function' || typeof S !== 'function') continue;
    for (const k of WANT) {
      try {
        if (Object.prototype.hasOwnProperty.call(T.prototype, k)) continue;
        const d = Object.getOwnPropertyDescriptor(S.prototype, k);
        if (d) Object.defineProperty(T.prototype, k, d);
      } catch (e) {}
    }
  }
})();
