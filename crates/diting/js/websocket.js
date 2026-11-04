(function () { // @@SCOPE
// god-file split: the 刀84 hidden-slot accessor installers, aliased from
// bootstrap's hidden d() exports (exports-extra.json pins both publishes to
// bootstrap — the alias consts would otherwise flip gen-exports' attribution
// here, past the aliases' own load point; the 刀5 trap).
const __acc = globalThis.__acc;
const __accRo = globalThis.__accRo;

if (typeof WebSocket === 'undefined') {
  // Real page-facing WebSocket over the op_ws_* family (diting_js::ws): the
  // DDoS-Guard challenge negotiates its answer through a WS round-trip, so a
  // stub that fires `open` from a microtask and drops `send` can never finish.
  globalThis.WebSocket = class WebSocket {
    constructor(url, protocols) {
      // 刀87: Chrome 145 ladder (k87-ctor-e/f) — argc TypeError; http(s)
      // converts to ws(s) per WHATWG; 4-scheme message; subprotocol token
      // validation runs before the duplicate check.
      if (arguments.length < 1) throw new TypeError("Failed to construct 'WebSocket': 1 argument required, but only 0 present.");
      let parsed;
      try {
        parsed = new URL(url, (globalThis.location && globalThis.location.href) || undefined);
      } catch (e) {
        throw new DOMException("Failed to construct 'WebSocket': The URL '" + url + "' is invalid.", 'SyntaxError');
      }
      let scheme = (parsed.protocol || '').replace(/:$/, '').toLowerCase();
      if (scheme === 'http' || scheme === 'https') {
        parsed = new URL(parsed.href.replace(/^https?:/i, scheme === 'http' ? 'ws:' : 'wss:'));
        scheme = scheme === 'http' ? 'ws' : 'wss';
      }
      if (scheme !== 'ws' && scheme !== 'wss') {
        throw new DOMException("Failed to construct 'WebSocket': The URL's scheme must be either 'http', 'https', 'ws', or 'wss'. '" + scheme + "' is not allowed.", 'SyntaxError');
      }
      if (parsed.hash) {
        throw new DOMException("Failed to construct 'WebSocket': The URL contains a fragment identifier ('" + parsed.hash.slice(1) + "'). Fragment identifiers are not allowed in WebSocket URLs.", 'SyntaxError');
      }
      let list = [];
      if (protocols !== undefined && protocols !== null) {
        list = Array.isArray(protocols) ? protocols.map(String) : [String(protocols)];
        const tok = /^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$/;
        const seen = new Set();
        for (const p of list) {
          if (!tok.test(p)) {
            throw new DOMException("Failed to construct 'WebSocket': The subprotocol '" + p + "' is invalid.", 'SyntaxError');
          }
          if (seen.has(p)) {
            throw new DOMException("Failed to construct 'WebSocket': The subprotocol '" + p + "' is duplicated.", 'SyntaxError');
          }
          seen.add(p);
        }
      }
      __def(this, '_url', parsed.href);
      __def(this, '_readyState', 0); // CONNECTING
      __def(this, '_bufferedAmount', 0);
      this.binaryType = 'blob';
      __def(this, '_extensions', '');
      __def(this, '_protocol', list[0] || '');
      this.onopen = null; this.onmessage = null; this.onerror = null; this.onclose = null;
      this._id = undefined;
      this._terminal = false; // close event fired; no further events
      // #161: send() during CONNECTING buffers (Chrome/spec) instead of
      // throwing — flushed in order when the pump reaches `open`.
      this._sendQueue = [];
      const protocolsJson = JSON.stringify(list);
      Promise.resolve().then(() => {
        _OPS.op_ws_open(this.url, protocolsJson).then((raw) => {
          let res;
          try { res = JSON.parse(raw); } catch (e) { res = { error: 'malformed open response' }; }
          if (res.error) {
            if (!this._terminal) this._fail();
            return;
          }
          if (this._terminal) {
            // User closed while CONNECTING: release the just-registered socket.
            try { _OPS.op_ws_close(res.id, 1000, ''); } catch (e) {}
            return;
          }
          this._id = res.id;
          this._pump(res.id);
        }).catch((e) => {
          if (!this._terminal) this._fail();
        });
      });
      __hideOwn(this);
    }
    _emit(ev) {
      // 刀56: on* fires through the native dispatch's handler hook — no
      // manual leg, or an assigned onmessage would run twice.
      try { this.dispatchEvent(ev); } catch (e) {}
    }
    _fail() {
      if (this._terminal) return;
      this._terminal = true;
      __def(this, '_readyState', 3); // CLOSED
      this._id = undefined;
      this._sendQueue.length = 0; // queued CONNECTING sends die with the attempt
      this._emit(new Event('error'));
      this._fireCloseEvent(1006, '', false);
    }
    _finish(code, reason, wasClean) {
      if (this._terminal) return;
      this._terminal = true;
      __def(this, '_readyState', 3); // CLOSED
      this._id = undefined;
      this._sendQueue.length = 0;
      this._fireCloseEvent(code, reason, wasClean);
    }
    _fireCloseEvent(code, reason, wasClean) {
      const ev = new CloseEvent('close', { code, reason, wasClean });
      this._emit(ev);
    }
    async _pump(id) {
      while (!this._terminal && this._id === id) {
        let raw;
        try { raw = await _OPS.op_ws_next_message(id); } catch (e) { break; }
        let ev;
        try { ev = JSON.parse(raw); } catch (e) { break; }
        if (!ev || !ev.kind) break;
        if (ev.kind === 'open') {
          if (this._terminal || this._id !== id) break;
          __def(this, '_readyState', 1); // OPEN
          for (const item of this._sendQueue.splice(0)) {
            try { _OPS.op_ws_send(id, item[0], item[1]); } catch (e) { break; }
          }
          __def(this, '_bufferedAmount', 0);
          this._emit(new Event('open'));
        } else if (ev.kind === 'message') {
          if (this._terminal) break;
          let data = ev.data;
          if (ev.binary) {
            // Spec: binary frames surface as ArrayBuffer when binaryType is
            // 'arraybuffer', as Blob otherwise (PDD's decoder does
            // `new Uint8Array(buf, 16)`, which a Uint8Array silently breaks).
            const bytes = _base64ToUint8Array(ev.data);
            data = this.binaryType === 'arraybuffer'
              ? _arrayBufferFromBytes(bytes)
              : new Blob([bytes]);
          }
          this._emit(new MessageEvent('message', { data }));
        } else if (ev.kind === 'error') {
          if (this._terminal) break;
          this._emit(new Event('error'));
        } else if (ev.kind === 'close') {
          const code = ev.code || 1006;
          this._finish(code, ev.reason || '', code !== 1006);
          break;
        }
      }
    }
    send(data) {
      if (this.readyState >= 2) {
        throw new DOMException("Failed to execute 'send' on 'WebSocket': WebSocket is already in CLOSING or CLOSED state.", 'InvalidStateError');
      }
      let kind, payload, byteLen;
      if (typeof data === 'string') {
        kind = 'text'; payload = data;
        byteLen = new TextEncoder().encode(data).length;
      } else if (data instanceof Blob) {
        kind = 'binary'; payload = _bytesToBase64(data._bytes);
        byteLen = data._bytes.length;
      } else if (data instanceof ArrayBuffer || ArrayBuffer.isView(data)) {
        const bytes = data instanceof ArrayBuffer
          ? new Uint8Array(data)
          : new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
        kind = 'binary'; payload = _bytesToBase64(bytes);
        byteLen = bytes.length;
      } else {
        throw new TypeError("Failed to execute 'send' on 'WebSocket': parameter 1 is not of type 'ArrayBuffer' or 'ArrayBufferView'.");
      }
      if (this.readyState === 0) {
        // Chrome/spec: CONNECTING sends buffer (in order) and flush on
        // open; only CLOSING/CLOSED throw (#161). bufferedAmount tracks
        // the queued bytes like Chrome's.
        this._sendQueue.push([kind, payload]);
        __def(this, '_bufferedAmount', this._bufferedAmount + byteLen);
        return;
      }
      _OPS.op_ws_send(this._id, kind, payload);
    }
    close(code, reason) {
      if (this.readyState >= 2) return;
      if (code !== undefined && code !== null) {
        const c = Number(code);
        if (c !== 1000 && !(c >= 3000 && c <= 4999)) {
          throw new DOMException("Failed to execute 'close' on 'WebSocket': The code must be either 1000, or between 3000 and 4999. " + c + " is neither.", 'InvalidAccessError');
        }
      }
      const r = reason === undefined || reason === null ? '' : String(reason);
      if (r.length > 123) {
        throw new DOMException("Failed to execute 'close' on 'WebSocket': The message must not be greater than 123 bytes.", 'SyntaxError');
      }
      const c2 = code === undefined || code === null ? 1000 : Number(code);
      if (this._id !== undefined) {
        try { _OPS.op_ws_close(this._id, c2, r); } catch (e) {}
      }
      if (this.readyState === 0) {
        // CONNECTING: fail the connection attempt — close event now, clean.
        this._finish(c2, r, true);
        return;
      }
      __def(this, '_readyState', 2); // CLOSING; pump delivers the negotiated close later
    }
    static CONNECTING = 0; static OPEN = 1; static CLOSING = 2; static CLOSED = 3;
  };
  __accRo(globalThis.WebSocket, ['url', 'readyState', 'bufferedAmount',
    'extensions', 'protocol']);
  __acc(globalThis.WebSocket, ['binaryType',
    'onopen', 'onmessage', 'onerror', 'onclose']);
}
// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('__acc', __acc);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
