(function () { // @@SCOPE
// god-file split (刀55, text codec fallbacks): TextEncoder / TextDecoder
// (with the fast pure-JS WHATWG UTF-8 decoder — maximal-subpart U+FFFD,
// Chrome-identical on overlongs/surrogates/truncated tails) plus the
// TextEncoderStream / TextDecoderStream fallbacks, moved verbatim from
// bootstrap. Every install is a typeof-guarded fallback — the host
// native wins when present; every consumer (bootstrap's charset/body
// helpers, XHR, websocket, streams) calls these at runtime only.
if (typeof TextEncoder === 'undefined') {
  globalThis.TextEncoder = class TextEncoder {
    get encoding() { return 'utf-8'; }
    encode(str) {
      str = String(str);
      const buf = [];
      for (let i = 0; i < str.length; i++) {
        let c = str.charCodeAt(i);
        if (c < 0x80) buf.push(c);
        else if (c < 0x800) { buf.push(0xC0|(c>>6), 0x80|(c&0x3F)); }
        else if (c < 0xD800 || c >= 0xE000) { buf.push(0xE0|(c>>12), 0x80|((c>>6)&0x3F), 0x80|(c&0x3F)); }
        else { c = 0x10000 + (((c & 0x3FF) << 10) | (str.charCodeAt(++i) & 0x3FF)); buf.push(0xF0|(c>>18), 0x80|((c>>12)&0x3F), 0x80|((c>>6)&0x3F), 0x80|(c&0x3F)); }
      }
      return new Uint8Array(buf);
    }
    encodeInto(str, dest) { const enc = this.encode(str); dest.set(enc.slice(0, dest.length)); return { read: str.length, written: Math.min(enc.length, dest.length) }; }
  };
}
// Fast pure-JS UTF-8 decode (the common case: Response/Blob .text(), most
// pages). Avoids the op + JSON round trip for plain UTF-8.
// WHATWG UTF-8 decoder (non-fatal): malformed input yields U+FFFD per the
// spec's "maximal subpart" rule, never garbage code points. Chrome-identical
// for overlongs, surrogates (ED A0..BF), > U+10FFFF and truncated tails.
function _utf8DecodeBytes(bytes, start) {
  const n = bytes.length;
  let i = start | 0, out = '';
  const buf = [];
  const flush = () => { out += String.fromCharCode.apply(null, buf); buf.length = 0; };
  while (i < n) {
    const c = bytes[i++];
    if (c < 0x80) { buf.push(c); }
    else {
      let need, cp, lo = 0x80, hi = 0xBF;
      if (c >= 0xC2 && c <= 0xDF) { need = 1; cp = c & 0x1F; }
      else if (c >= 0xE0 && c <= 0xEF) { need = 2; cp = c & 0x0F; if (c === 0xE0) lo = 0xA0; else if (c === 0xED) hi = 0x9F; }
      else if (c >= 0xF0 && c <= 0xF4) { need = 3; cp = c & 0x07; if (c === 0xF0) lo = 0x90; else if (c === 0xF4) hi = 0x8F; }
      else { buf.push(0xFFFD); continue; }
      let ok = true;
      for (let k = 0; k < need; k++) {
        const b = i < n ? bytes[i] : -1;
        if (b < lo || b > hi) { ok = false; break; }
        lo = 0x80; hi = 0xBF; cp = (cp << 6) | (b & 0x3F); i++;
      }
      if (!ok) { buf.push(0xFFFD); continue; }
      if (cp > 0xFFFF) { const t = cp - 0x10000; buf.push(0xD800 + (t >> 10), 0xDC00 + (t & 0x3FF)); }
      else buf.push(cp);
    }
    if (buf.length >= 8192) flush();
  }
  flush();
  return out;
}
if (typeof TextDecoder === 'undefined') {
  globalThis.TextDecoder = class TextDecoder {
    constructor(label, options) {
      // No-arg construction (Response.text()/Blob.text() and most pages) is
      // UTF-8; skip the label-validation op on that hot path.
      let name;
      if (label === undefined) {
        name = 'utf-8';
      } else {
        name = _OPS.op_encoding_for_label(String(label));
        if (!name) throw new RangeError("Failed to construct 'TextDecoder': The encoding label provided ('" + label + "') is invalid.");
      }
      const o = options || {};
      // Plain assignment: lift_own.js turns these into prototype accessors.
      const ds = __diting_liftSlot(this); ds.encoding = name; ds.fatal = !!o.fatal; ds.ignoreBOM = !!o.ignoreBOM;
      // Bytes carried over between decode(..., {stream:true}) calls
      // (TextDecoderStream splits chunks at arbitrary byte offsets).
      this._pending = new Uint8Array(0);
      this._bomChecked = false;
      __hideOwn(this);
    }
    decode(input, options) {
      const stream = !!(options && options.stream);
      let bytes = input === undefined
        ? new Uint8Array(0)
        : ArrayBuffer.isView(input)
          ? new Uint8Array(input.buffer, input.byteOffset, input.byteLength)
          : new Uint8Array(input);
      if (this._pending.length || stream) {
        const merged = new Uint8Array(this._pending.length + bytes.length);
        merged.set(this._pending);
        merged.set(bytes, this._pending.length);
        bytes = merged;
        if (stream) {
          if (this.encoding === 'utf-8' && !this.fatal) {
            // Emit only complete UTF-8 sequences; hold the incomplete tail.
            let cut = bytes.length;
            for (let i = bytes.length - 1; i >= 0 && i >= bytes.length - 4; i--) {
              const b = bytes[i];
              if ((b & 0xC0) === 0x80) continue;
              const need = b < 0x80 ? 0 : b < 0xE0 ? 1 : b < 0xF0 ? 2 : 3;
              if (i + need + 1 > bytes.length) cut = i;
              break;
            }
            this._pending = bytes.slice(cut);
            bytes = bytes.slice(0, cut);
          } else {
            // Legacy encodings / fatal mode: withhold output until flush.
            this._pending = bytes;
            return '';
          }
        } else {
          this._pending = new Uint8Array(0);
        }
      }
      if (!stream) this._pending = new Uint8Array(0);
      // Fast path: plain UTF-8, non-fatal (Response/Blob text, most pages).
      if (this.encoding === 'utf-8' && !this.fatal) {
        let off = 0;
        if (!this._bomChecked && bytes.length) {
          this._bomChecked = true;
          if (!this.ignoreBOM && bytes.length >= 3 && bytes[0] === 0xEF && bytes[1] === 0xBB && bytes[2] === 0xBF) off = 3;
        }
        return _utf8DecodeBytes(bytes, off);
      }
      // Legacy encodings / fatal mode: encoding_rs via the op.
      const r = JSON.parse(_OPS.op_text_decode(this.encoding, bytes, this.fatal, this.ignoreBOM));
      if (!r.ok) throw new TypeError("Failed to execute 'decode' on 'TextDecoder': The encoded data was not valid.");
      return r.v;
    }
  };
}

// React Router's SSR stream bootstrap does
// `new ReadableStream(...).pipeThrough(new TextEncoderStream())`; without
// these, hydration never runs and the whole client app is inert.
// Composition, not `extends TransformStream`: TransformStream is installed
// by a deno extension AFTER the snapshot is taken, so it does not exist yet
// when this file is evaluated at build time.
if (typeof TextEncoderStream === 'undefined') {
  globalThis.TextEncoderStream = class TextEncoderStream {
    constructor() {
      let pending = '';
      const enc = new TextEncoder();
      const ts = new TransformStream({
        transform(chunk, controller) {
          chunk = pending + String(chunk);
          pending = '';
          const last = chunk.charCodeAt(chunk.length - 1);
          if (last >= 0xD800 && last < 0xDC00) {
            pending = chunk[chunk.length - 1];
            chunk = chunk.slice(0, -1);
          }
          if (chunk) controller.enqueue(enc.encode(chunk));
        },
        flush(controller) {
          if (pending) controller.enqueue(enc.encode(pending));
        }
      });
      const es = __diting_liftSlot(this);
      es.readable = ts.readable;
      es.writable = ts.writable;
    }
    get encoding() { return 'utf-8'; }
  };
  globalThis.TextDecoderStream = class TextDecoderStream {
    constructor(label, options) {
      const dec = new TextDecoder(label, options);
      const ts = new TransformStream({
        transform(chunk, controller) {
          const out = dec.decode(chunk, { stream: true });
          if (out) controller.enqueue(out);
        },
        flush(controller) {
          const out = dec.decode();
          if (out) controller.enqueue(out);
        }
      });
      const ds2 = __diting_liftSlot(this);
      ds2.readable = ts.readable;
      ds2.writable = ts.writable;
      // 刀84: sealed ro members — internal writes go through the shared lift slot
      ds2.encoding = dec.encoding;
      ds2.fatal = dec.fatal;
      ds2.ignoreBOM = dec.ignoreBOM;
    }
  };
}

// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
