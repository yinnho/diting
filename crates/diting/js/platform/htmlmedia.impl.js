//! module: htmlmedia
//! layer: L2
//! exposes: VideoPlaybackQuality
//! requires-load: -
//! requires-call: legacy:DOMException, legacy:DOMTokenList, legacy:MediaStream, legacy:RemotePlayback, legacy:TextTrack, legacy:TextTrackList

// HTMLMediaElement/HTMLVideoElement/HTMLImageElement behavior only (M4
// knife 5); the member SHAPE (placement, own order, descriptor kinds,
// arity) is generated from the Chrome truth overlay in
// gen/htmlmedia.gen.js, and uniform attribute reflection is DATA there
// (P.reflector, incl. the unsigned-long/crossorigin/url kinds core.js
// grew for this family). This module owns the NON-uniform semantics;
// every fact below that could be misremembered is pinned by a key in
// tools/webplatform/chrome/htmlmedia-truth-145.json (real Chrome 145)
// and diffed by the htmlmedia_members_match_chrome_truth test.
//
// Faces: TextTrackList/TextTrack/RemotePlayback stay the global_surface
// stubs (their ctor messages and @@toStringTag already match Chrome
// word for word) and serve as mint prototypes. VideoPlaybackQuality IS
// taken over here - the stub's ctor message lacked the Chrome prefix
// (truth faces: "Failed to construct 'VideoPlaybackQuality': Illegal
// constructor"); same handover pattern Performance took at knife 3.

// ---- VideoPlaybackQuality: ctor shell only; getVideoPlaybackQuality()
// mints bare instances (truth vid.gvpq: no own enumerable members).
function VideoPlaybackQuality() {
  P.fail(VideoPlaybackQuality, 'Illegal constructor', "Failed to construct 'VideoPlaybackQuality'");
}
P.iface(VideoPlaybackQuality, 'VideoPlaybackQuality', 0);
P.seal(VideoPlaybackQuality);

// ---- per-element state. volume/muted/preservesPitch/defaultPlaybackRate/
// srcObject/mediaKeys are pure state (muted deliberately does NOT track the
// 'muted' attribute - truth med.muted/med.defaultMuted); textTracks/remote/
// controlsList cache their minted or DOMTokenList faces per element; vfc is
// the requestVideoFrameCallback id counter (truth vid.rvcb: 1 then 2).
const S = new WeakMap();
function st(el) {
  let s = S.get(el);
  if (!s) {
    s = { volume: 1, muted: false, preservesPitch: true, defaultPlaybackRate: 1,
      mediaKeys: null, srcObject: null, ttList: null, tracks: null, remote: null,
      controlsList: null, vfc: 0 };
    S.set(el, s);
  }
  return s;
}

const GET = {
  volume(el) { return st(el).volume; },
  muted(el) { return st(el).muted; },
  preservesPitch(el) { return st(el).preservesPitch; },
  defaultPlaybackRate(el) { return st(el).defaultPlaybackRate; },
  mediaKeys(el) { return st(el).mediaKeys; },
  srcObject(el) { return st(el).srcObject; },
  textTracks(el) {
    const s = st(el);
    if (!s.ttList) {
      s.tracks = [];
      s.ttList = Object.create(TextTrackList.prototype);
      Object.defineProperty(s.ttList, 'length', {
        get: function () { return s.tracks.length; },
        enumerable: true, configurable: true,
      });
    }
    return s.ttList;
  },
  remote(el) {
    const s = st(el);
    if (!s.remote) s.remote = Object.create(RemotePlayback.prototype);
    return s.remote;
  },
  controlsList(el) {
    const s = st(el);
    if (!s.controlsList) s.controlsList = new DOMTokenList(el, 'controlslist');
    return s.controlsList;
  },
  // truth img.defaults/img.complete.appended: a sourceless image reports
  // complete immediately (attached or not). The engine settles image loads
  // instantly, so true is also the with-src answer; pinned only sourceless.
  complete() { return true; },
};

const SET = {
  volume(el, v) {
    // truth med.vol.clamp: exact IndexSizeError wording, the offending value
    // stringified as a number ('5', 'NaN'); med.vol.set: numeric strings
    // convert (volume '0.25' -> 0.25).
    const n = Number(v);
    if (isNaN(n) || n < 0 || n > 1) {
      throw new DOMException("Failed to set the 'volume' property on 'HTMLMediaElement': The volume provided (" + n + ") is outside the range [0, 1].", 'IndexSizeError');
    }
    st(el).volume = n;
  },
  muted(el, v) { st(el).muted = !!v; },
  preservesPitch(el, v) { st(el).preservesPitch = !!v; },
  defaultPlaybackRate(el, v) { st(el).defaultPlaybackRate = Number(v); },
  mediaKeys(el, v) { st(el).mediaKeys = v === undefined ? null : v; },
  srcObject(el, v) { st(el).srcObject = v === undefined || v === null ? null : v; },
  controlsList(el, v) { el.setAttribute('controlslist', v === undefined || v === null ? '' : String(v)); },
};

const CALL = {
  // truth med.addTextTrack: ('subtitles') -> TextTrack instance with
  // kind/label/language carried and mode 'hidden'; the track joins
  // textTracks (length grows - spec-true, unpinned face).
  addTextTrack(el, ctx, args) {
    const kind = String(args[0]);
    if (['subtitles', 'captions', 'descriptions', 'chapters', 'metadata'].indexOf(kind) < 0) {
      throw new TypeError(ctx + ": The provided value '" + kind + "' is not a valid enum value of type TextTrackKind.");
    }
    GET.textTracks(el);
    const s = st(el);
    const tt = Object.create(TextTrack.prototype);
    const fields = { kind: kind, label: args[1] === undefined ? '' : String(args[1]),
      language: args[2] === undefined ? '' : String(args[2]), mode: 'hidden' };
    Object.keys(fields).forEach(function (k) {
      Object.defineProperty(tt, k, { value: fields[k], writable: true, enumerable: true, configurable: true });
    });
    s.tracks.push(tt);
    return tt;
  },
  captureStream() { return new MediaStream(); },
  // truth med.setSinkId.plain: rejects NotFoundError 'Requested device not
  // found' (no sink machinery - null is not the default device).
  setSinkId() { return Promise.reject(new DOMException('Requested device not found', 'NotFoundError')); },
  // truth med.setMediaKeys.plain: (null) resolves undefined.
  setMediaKeys(el, ctx, args) { SET.mediaKeys(el, args[0]); return Promise.resolve(); },
  // truth vid.gvpq: no own enumerable members (a bare mint, branded).
  getVideoPlaybackQuality() { return Object.create(VideoPlaybackQuality.prototype); },
  requestVideoFrameCallback(el) { const s = st(el); return ++s.vfc; },
  cancelVideoFrameCallback() { return undefined; },
  // carried over from media_late.js (that block is deleted): a video with
  // no loaded metadata rejects InvalidStateError (Chrome face unpinned).
  requestPictureInPicture() {
    return Promise.reject(new DOMException('Metadata for the video element are not loaded yet.', 'InvalidStateError'));
  },
  // truth img.decode.plain: a sourceless image rejects EncodingError 'The
  // source image cannot be decoded.'; anything with a src(srcset) resolves.
  decode(el) {
    if (el.getAttribute('src') === null && el.getAttribute('srcset') === null) {
      return Promise.reject(new DOMException('The source image cannot be decoded.', 'EncodingError'));
    }
    return Promise.resolve();
  },
};

exports.get = function (name, el) {
  const f = GET[name];
  if (!f) throw new Error('htmlmedia impl: no getter ' + name);
  return f(el);
};
exports.set = function (name, el, v) {
  const f = SET[name];
  if (!f) throw new Error('htmlmedia impl: no setter ' + name);
  f(el, v);
};
exports.call = function (name, el, ctx, args) {
  const f = CALL[name];
  if (!f) throw new Error('htmlmedia impl: no operation ' + name);
  return f(el, ctx, args);
};

P.expose('VideoPlaybackQuality', VideoPlaybackQuality);
