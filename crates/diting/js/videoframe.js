// VideoFrame (刀67), split out of bootstrap.js (god-file ratchet). Loads right
// after bootstrap; videoframe_gaps.js (later) layers the remaining faces on it.
// 刀67: VideoFrame's full Chrome-145 face (probes k67-chrome{,2,3,4}):
// both constructor shapes — CanvasImageSource (canvas pixels read as real
// BGRA/BGRX bytes, alpha:'discard' flips the format) and BufferSource+
// VideoFrameBufferInit with Chrome's exact ladder (source union incl. the
// buffer-without-init case, init-null type error, codedHeight→codedWidth→
// format member order, nonzero dims, visibleRect floor+even-snap with
// x/y-negative and w/h-nonzero messages and right/bottom bounds, display
// pair rule, data size against the PARSED coded dims, then the frame
// collapses to the visible rect re-anchored at the origin — vis_x2/x3),
// the cached codedRect/visibleRect/colorSpace getters, plane math for
// allocationSize/copyTo (copyTo resolves the plane-layout array; every
// rejection is asynchronous), and the closed matrix (timestamp/duration
// survive, geometry zeroes, rects null, colorSpace all-null).
const _vfPlanes = (fmt, w, h) => {
  const c2w = Math.ceil(w / 2), c2h = Math.ceil(h / 2), y = w * h;
  if (fmt === 'I420') return [{ o: 0, s: w, n: y }, { o: y, s: c2w, n: c2w * c2h }, { o: y + c2w * c2h, s: c2w, n: c2w * c2h }];
  if (fmt === 'I420A') return [{ o: 0, s: w, n: y }, { o: y, s: c2w, n: c2w * c2h }, { o: y + c2w * c2h, s: c2w, n: c2w * c2h }, { o: y + 2 * c2w * c2h, s: w, n: y }];
  if (fmt === 'I422') return [{ o: 0, s: w, n: y }, { o: y, s: c2w, n: c2w * h }, { o: y + c2w * h, s: c2w, n: c2w * h }];
  if (fmt === 'I444') return [{ o: 0, s: w, n: y }, { o: y, s: w, n: y }, { o: 2 * y, s: w, n: y }];
  if (fmt === 'NV12') return [{ o: 0, s: w, n: y }, { o: y, s: w, n: w * c2h }];
  return [{ o: 0, s: w * 4, n: w * h * 4 }]; // RGBA/RGBX/BGRA/BGRX
};
const _vfUL = (v, m, init) => {
  const n = Math.trunc(Number(v));
  if (!(n >= 0) || n > 4294967295) throw new TypeError("Failed to construct 'VideoFrame': Failed to read the '" + m + "' property from '" + init + "': Value is outside the 'unsigned long' value range.");
  return n;
};
const _vfULL = (v, m, init) => {
  const n = Math.trunc(Number(v));
  if (!(n >= 0)) throw new TypeError("Failed to construct 'VideoFrame': Failed to read the '" + m + "' property from '" + init + "': Value is outside the 'unsigned long long' value range.");
  return n;
};
const _vfGuard = (self) => {
  if (self === null || typeof self !== 'object' || !(self instanceof VideoFrame)) throw new TypeError('Illegal invocation');
};
globalThis.VideoFrame = class VideoFrame {
  constructor(image, options) {
    if (arguments.length === 0) throw new TypeError("Failed to construct 'VideoFrame': 1 argument required, but only 0 present.");
    const SRC = "Failed to construct 'VideoFrame': The provided value is not of type '(CSSImageValue or HTMLCanvasElement or HTMLImageElement or HTMLVideoElement or ImageBitmap or OffscreenCanvas or SVGImageElement or VideoFrame)'.";
    const G = globalThis;
    const isImg = (o) => o !== null && typeof o === 'object' && ['HTMLCanvasElement', 'HTMLImageElement', 'HTMLVideoElement', 'ImageBitmap', 'OffscreenCanvas', 'SVGImageElement']
      .some((n) => typeof G[n] === 'function' && o instanceof G[n]) || o instanceof VideoFrame;
    const isBuf = image instanceof ArrayBuffer || ArrayBuffer.isView(image);
    if (!isImg(image) && !isBuf) throw new TypeError(SRC);
    let opts, init;
    if (isBuf) {
      init = 'VideoFrameBufferInit';
      if (options === undefined) throw new TypeError(SRC);
      if (options === null || typeof options !== 'object') throw new TypeError("Failed to construct 'VideoFrame': The provided value is not of type 'VideoFrameBufferInit'.");
      opts = options;
    } else {
      init = 'VideoFrameInit';
      opts = options === null || typeof options !== 'object' ? {} : options;
    }
    let fmt, ts, dur = null, cw = 0, ch = 0, vw = 0, vh = 0, dw = 0, dh = 0, rot = 0, flip = false;
    let cs = { fullRange: true, matrix: 'rgb', primaries: 'bt709', transfer: 'iec61966-2-1' }, bytes = null;
    if (image instanceof VideoFrame) {
      if (image._closed) throw new DOMException("Failed to construct 'VideoFrame': Cannot clone closed VideoFrame.", "InvalidStateError");
      fmt = image._fmt; cw = image._cw; ch = image._ch; vw = image._vw; vh = image._vh; dw = image._dw; dh = image._dh;
      rot = image._rot; flip = image._flip; cs = image._cs; bytes = image._bytes;
      ts = opts.timestamp !== undefined ? Math.trunc(Number(opts.timestamp)) : image._ts;
      dur = opts.duration !== undefined ? _vfULL(opts.duration, 'duration', init) : image._dur;
    } else if (isBuf) {
      if (opts.codedHeight === undefined) throw new TypeError("Failed to construct 'VideoFrame': Failed to read the 'codedHeight' property from 'VideoFrameBufferInit': Required member is undefined.");
      ch = _vfUL(opts.codedHeight, 'codedHeight', init);
      if (opts.codedWidth === undefined) throw new TypeError("Failed to construct 'VideoFrame': Failed to read the 'codedWidth' property from 'VideoFrameBufferInit': Required member is undefined.");
      cw = _vfUL(opts.codedWidth, 'codedWidth', init);
      if (opts.format === undefined) throw new TypeError("Failed to construct 'VideoFrame': Failed to read the 'format' property from 'VideoFrameBufferInit': Required member is undefined.");
      fmt = String(opts.format);
      if (!(fmt in { I420: 1, I420A: 1, I422: 1, I444: 1, NV12: 1, RGBA: 1, RGBX: 1, BGRA: 1, BGRX: 1 })) throw new TypeError("Failed to construct 'VideoFrame': Failed to read the 'format' property from 'VideoFrameBufferInit': The provided value '" + fmt + "' is not a valid enum value of type VideoPixelFormat.");
      if (cw === 0) throw new TypeError("Failed to construct 'VideoFrame': codedWidth must be nonzero.");
      if (ch === 0) throw new TypeError("Failed to construct 'VideoFrame': codedHeight must be nonzero.");
      if (opts.visibleRect !== undefined && opts.visibleRect !== null) {
        const vr = opts.visibleRect;
        const rd = (v) => Math.floor(Number(v === undefined ? 0 : v));
        const rx = rd(vr.x), ry = rd(vr.y), rw = rd(vr.width), rh = rd(vr.height);
        if (rx < 0) throw new TypeError("Failed to construct 'VideoFrame': Invalid visibleRect. x cannot be negative.");
        if (ry < 0) throw new TypeError("Failed to construct 'VideoFrame': Invalid visibleRect. y cannot be negative.");
        const sx = rx - (rx % 2), sy = ry - (ry % 2); // even-snap (vis_x1: 1→0, vis_x3: 3→2)
        if (rw === 0) throw new TypeError("Failed to construct 'VideoFrame': Invalid visibleRect. width must be nonzero.");
        if (rh === 0) throw new TypeError("Failed to construct 'VideoFrame': Invalid visibleRect. height must be nonzero.");
        if (sx + rw > cw) throw new TypeError("Failed to construct 'VideoFrame': Invalid visibleRect. right " + (sx + rw) + " exceeds codedWidth " + cw + ".");
        if (sy + rh > ch) throw new TypeError("Failed to construct 'VideoFrame': Invalid visibleRect. bottom " + (sy + rh) + " exceeds codedHeight " + ch + ".");
        // the frame collapses to the visible rect, re-anchored at the origin;
        // the data-size check uses the PARSED coded dims (vis_x3)
        vw = rw; vh = rh;
        const srcP = _vfPlanes(fmt, cw, ch), dstP = _vfPlanes(fmt, vw, vh);
        const view = image instanceof ArrayBuffer ? new Uint8Array(image) : new Uint8Array(image.buffer, image.byteOffset, image.byteLength);
        const needP = srcP[srcP.length - 1].o + srcP[srcP.length - 1].n;
        if (view.byteLength < needP) throw new TypeError("Failed to construct 'VideoFrame': data is not large enough.");
        bytes = new Uint8Array(dstP[dstP.length - 1].o + dstP[dstP.length - 1].n);
        const packed = fmt === 'RGBA' || fmt === 'RGBX' || fmt === 'BGRA' || fmt === 'BGRX';
        for (let i = 0; i < dstP.length; i++) {
          const sub = !packed && i > 0 && fmt !== 'I444' && fmt !== 'NV12'; // half-res chroma planes
          const xOff = packed ? sx * 4 : (sub ? sx >> 1 : sx);
          const yOff = sub ? sy >> 1 : sy;
          const rows = dstP[i].n / dstP[i].s;
          for (let r = 0; r < rows; r++) {
            const from = srcP[i].o + xOff + (yOff + r) * srcP[i].s;
            bytes.set(view.subarray(from, from + dstP[i].s), dstP[i].o + r * dstP[i].s);
          }
        }
        cw = vw; ch = vh; // the frame collapses to the visible rect (vis_x2)
      } else {
        vw = cw; vh = ch;
        const planes = _vfPlanes(fmt, cw, ch), need = planes[planes.length - 1].o + planes[planes.length - 1].n;
        const view = image instanceof ArrayBuffer ? new Uint8Array(image) : new Uint8Array(image.buffer, image.byteOffset, image.byteLength);
        if (view.byteLength < need) throw new TypeError("Failed to construct 'VideoFrame': data is not large enough.");
        bytes = view.slice(0, need);
      }
      if (opts.displayWidth !== undefined || opts.displayHeight !== undefined) {
        if (opts.displayWidth === undefined) throw new TypeError("Failed to construct 'VideoFrame': displayHeight specified without displayWidth.");
        if (opts.displayHeight === undefined) throw new TypeError("Failed to construct 'VideoFrame': displayWidth specified without displayHeight.");
        dw = _vfUL(opts.displayWidth, 'displayWidth', init);
        dh = _vfUL(opts.displayHeight, 'displayHeight', init);
      } else { dw = vw; dh = vh; }
      if (opts.timestamp === undefined) throw new TypeError("Failed to construct 'VideoFrame': VideoFrameInit must provide timestamp");
      ts = Math.trunc(Number(opts.timestamp));
      if (opts.duration !== undefined) dur = _vfULL(opts.duration, 'duration', init);
    } else {
      if (opts.timestamp === undefined) throw new TypeError("Failed to construct 'VideoFrame': VideoFrameInit must provide timestamp");
      ts = Math.trunc(Number(opts.timestamp));
      if (opts.duration !== undefined) dur = _vfULL(opts.duration, 'duration', init);
      fmt = opts.alpha === 'discard' ? 'BGRX' : 'BGRA';
      cw = image.width >>> 0; ch = image.height >>> 0;
      if (cw === 0) throw new TypeError("Failed to construct 'VideoFrame': codedWidth must be nonzero.");
      if (ch === 0) throw new TypeError("Failed to construct 'VideoFrame': codedHeight must be nonzero.");
      vw = cw; vh = ch; dw = cw; dh = ch;
      bytes = new Uint8Array(cw * ch * 4);
      if (typeof image.getContext === 'function') {
        const ctx = image.getContext('2d');
        if (ctx && typeof ctx.getImageData === 'function') {
          const d = ctx.getImageData(0, 0, cw, ch).data;
          for (let i = 0; i < cw * ch; i++) { bytes[i * 4] = d[i * 4 + 2]; bytes[i * 4 + 1] = d[i * 4 + 1]; bytes[i * 4 + 2] = d[i * 4]; bytes[i * 4 + 3] = d[i * 4 + 3]; }
        }
      }
    }
    if (opts.rotation !== undefined) {
      const n = Math.trunc(Number(opts.rotation));
      rot = [0, 90, 180, 270].reduce((a, k) => Math.abs(k - n) <= Math.abs(a - n) ? k : a, 0);
    }
    if (opts.flip !== undefined) flip = Boolean(opts.flip);
    if (opts.colorSpace !== null && typeof opts.colorSpace === 'object') {
      const c = opts.colorSpace;
      cs = { fullRange: c.fullRange === undefined || c.fullRange === null ? true : Boolean(c.fullRange), matrix: c.matrix === undefined || c.matrix === null ? 'rgb' : c.matrix, primaries: c.primaries === undefined || c.primaries === null ? 'bt709' : c.primaries, transfer: c.transfer === undefined || c.transfer === null ? 'iec61966-2-1' : c.transfer };
    }
    this._fmt = fmt; this._ts = ts; this._dur = dur; this._cw = cw; this._ch = ch;
    this._vw = vw; this._vh = vh; this._dw = dw; this._dh = dh;
    this._rot = rot; this._flip = flip; this._cs = cs; this._bytes = bytes;
    this._closed = false; this._cr = null; this._vr = null; this._vcs = null;
    __hideOwn(this);
  }
  get format() { _vfGuard(this); return this._closed ? null : this._fmt; }
  get timestamp() { _vfGuard(this); return this._ts; }
  get duration() { _vfGuard(this); return this._dur; }
  get codedWidth() { _vfGuard(this); return this._closed ? 0 : this._cw; }
  get codedHeight() { _vfGuard(this); return this._closed ? 0 : this._ch; }
  get displayWidth() { _vfGuard(this); return this._closed ? 0 : this._dw; }
  get displayHeight() { _vfGuard(this); return this._closed ? 0 : this._dh; }
  get rotation() { _vfGuard(this); return this._closed ? 0 : this._rot; }
  get flip() { _vfGuard(this); return this._closed ? false : this._flip; }
  get codedRect() { _vfGuard(this); if (this._closed) return null; return this._cr || (this._cr = new DOMRectReadOnly(0, 0, this._cw, this._ch)); }
  get visibleRect() { _vfGuard(this); if (this._closed) return null; return this._vr || (this._vr = new DOMRectReadOnly(0, 0, this._vw, this._vh)); }
  get colorSpace() {
    _vfGuard(this);
    if (this._closed) return new VideoColorSpace({ fullRange: null, matrix: null, primaries: null, transfer: null });
    return this._vcs || (this._vcs = new VideoColorSpace(this._cs));
  }
  allocationSize() {
    _vfGuard(this);
    if (this._closed) throw new DOMException("Failed to execute 'allocationSize' on 'VideoFrame': VideoFrame is closed.", "InvalidStateError");
    const p = _vfPlanes(this._fmt, this._vw, this._vh);
    return p[p.length - 1].o + p[p.length - 1].n;
  }
  metadata() {
    _vfGuard(this);
    if (this._closed) throw new DOMException("Failed to execute 'metadata' on 'VideoFrame': VideoFrame is closed.", "InvalidStateError");
    return {};
  }
  clone() {
    _vfGuard(this);
    if (this._closed) throw new DOMException("Failed to execute 'clone' on 'VideoFrame': Cannot clone closed VideoFrame.", "InvalidStateError");
    const c = Object.create(VideoFrame.prototype);
    c._fmt = this._fmt; c._ts = this._ts; c._dur = this._dur; c._cw = this._cw; c._ch = this._ch;
    c._vw = this._vw; c._vh = this._vh; c._dw = this._dw; c._dh = this._dh;
    c._rot = this._rot; c._flip = this._flip; c._cs = this._cs; c._bytes = this._bytes;
    c._closed = false; c._cr = null; c._vr = null; c._vcs = null;
    __hideOwn(c);
    return c;
  }
  copyTo(dst) {
    _vfGuard(this);
    if (!(dst instanceof ArrayBuffer) && !ArrayBuffer.isView(dst)) return Promise.reject(new TypeError("Failed to execute 'copyTo' on 'VideoFrame': The provided value is not of type '([AllowShared] ArrayBuffer or [AllowShared] ArrayBufferView)'."));
    if (this._closed) return Promise.reject(new DOMException("Failed to execute 'copyTo' on 'VideoFrame': Cannot copy closed VideoFrame.", "InvalidStateError"));
    const planes = _vfPlanes(this._fmt, this._vw, this._vh);
    const need = planes[planes.length - 1].o + planes[planes.length - 1].n;
    if (dst.byteLength < need) return Promise.reject(new TypeError("Failed to execute 'copyTo' on 'VideoFrame': destination is not large enough."));
    const out = dst instanceof ArrayBuffer ? new Uint8Array(dst) : new Uint8Array(dst.buffer, dst.byteOffset, need);
    const src = this._bytes;
    return Promise.resolve().then(() => {
      for (const p of planes) out.set(src.subarray(p.o, p.o + p.n), p.o);
      return planes.map((p) => ({ offset: p.o, stride: p.s }));
    });
  }
  close() { _vfGuard(this); this._closed = true; this._cr = null; this._vr = null; this._vcs = null; }
};
