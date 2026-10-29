// HTMLMediaElement / HTMLVideoElement / HTMLImageElement member-family
// probe (M4 knife 5): the media/image reflected-attribute layer. Runs in
// real Chrome AND in the diting engine; diffed. The 55 gap members split as
// media-level 22 (audio row) + video-only 9 + image 24 (SURFACE-WEBIDL
// rows HTMLVideoElement 31 / HTMLAudioElement 22 / HTMLImageElement 24).
// Promise faces (decode/setSinkId/setMediaKeys) settle against a 3s race.
(function () {
  var R = {};
  function t(name, f) { try { var v = f(); R[name] = v === undefined ? '#undefined' : JSON.parse(JSON.stringify(v)); } catch (e) { R[name] = 'THROW ' + (e && e.constructor && e.constructor.name) + (e && e.name && e.name !== (e && e.constructor && e.constructor.name) ? '(' + e.name + ')' : '') + ': ' + (e && e.message); } }
  function lvls(P) {
    var o = {};
    Object.getOwnPropertyNames(P).forEach(function (k) {
      var d = Object.getOwnPropertyDescriptor(P, k);
      o[k] = d.get || d.set ? (d.set ? 'rw' : 'ro') : typeof d.value;
    });
    return o;
  }
  function ownerOf(inst, name) {
    var o = inst;
    while (o) {
      var proto = Object.getPrototypeOf(o);
      if (Object.prototype.hasOwnProperty.call(proto, name)) return (proto.constructor && proto.constructor.name) || '?';
      o = proto;
    }
    return 'ABSENT';
  }
  var PEND = [];
  function settle(key, pr) {
    PEND.push(Promise.race([
      pr.then(function (v) { return ['resolve', v === undefined ? '#undefined' : v]; },
              function (e) { return ['reject', e && e.name, String(e && e.message).slice(0, 220)]; }),
      new Promise(function (res) { setTimeout(function () { res(['pending']); }, 3000); })
    ]).then(function (v) { R[key] = v; }));
  }

  var v = document.createElement('video');
  document.body.appendChild(v);
  var a = document.createElement('audio');
  document.body.appendChild(a);
  var i = document.createElement('img');

  // ---- shape: own order + kinds of the three prototypes (the contract)
  t('MED.order', function () { return Object.getOwnPropertyNames(HTMLMediaElement.prototype); });
  t('MED.kinds', function () { return lvls(HTMLMediaElement.prototype); });
  t('VID.order', function () { return Object.getOwnPropertyNames(HTMLVideoElement.prototype); });
  t('VID.kinds', function () { return lvls(HTMLVideoElement.prototype); });
  t('IMG.order', function () { return Object.getOwnPropertyNames(HTMLImageElement.prototype); });
  t('IMG.kinds', function () { return lvls(HTMLImageElement.prototype); });

  var MED_NAMES = ['addTextTrack', 'autoplay', 'captureStream', 'controls', 'crossOrigin', 'currentSrc',
    'defaultMuted', 'defaultPlaybackRate', 'disableRemotePlayback', 'error', 'loop', 'mediaKeys', 'muted',
    'preload', 'preservesPitch', 'remote', 'setMediaKeys', 'setSinkId', 'sinkId', 'srcObject', 'textTracks', 'volume'];
  var VID_NAMES = ['cancelVideoFrameCallback', 'getVideoPlaybackQuality', 'height', 'playsInline', 'poster',
    'requestVideoFrameCallback', 'videoHeight', 'videoWidth', 'width'];
  var IMG_NAMES = ['align', 'alt', 'border', 'complete', 'crossOrigin', 'currentSrc', 'decode', 'decoding',
    'fetchPriority', 'height', 'hspace', 'isMap', 'loading', 'longDesc', 'lowsrc', 'naturalHeight', 'naturalWidth',
    'referrerPolicy', 'srcset', 'useMap', 'vspace', 'width', 'x', 'y'];
  t('owner.med', function () { var o = {}; MED_NAMES.forEach(function (n) { o[n] = ownerOf(v, n); }); return o; });
  t('owner.vid', function () { var o = {}; VID_NAMES.forEach(function (n) { o[n] = ownerOf(v, n); }); return o; });
  t('owner.img', function () { var o = {}; IMG_NAMES.forEach(function (n) { o[n] = ownerOf(i, n); }); return o; });

  // ---- window faces the instances lean on
  t('faces', function () {
    var o = {};
    ['TextTrackList', 'TextTrack', 'RemotePlayback', 'VideoPlaybackQuality', 'AudioTrackList', 'VideoTrackList'].forEach(function (n) {
      var C = window[n];
      o[n] = typeof C;
      if (typeof C === 'function') { try { new C(); } catch (e) { o[n + '.ctor'] = 'THROW ' + (e && e.name) + ': ' + (e && e.message); } }
    });
    return o;
  });

  // ---- HTMLMediaElement behavior (audio instance)
  t('med.defaults', function () {
    return { autoplay: a.autoplay, controls: a.controls, loop: a.loop, muted: a.muted,
      defaultMuted: a.defaultMuted, preload: a.preload, volume: a.volume,
      defaultPlaybackRate: a.defaultPlaybackRate, preservesPitch: a.preservesPitch,
      crossOrigin: a.crossOrigin, error: a.error, currentSrc: a.currentSrc,
      srcObject: a.srcObject, mediaKeys: a.mediaKeys, sinkId: a.sinkId,
      disableRemotePlayback: a.disableRemotePlayback };
  });
  t('med.vol.clamp', function () { var r = []; a.volume = 5; r.push(a.volume); a.volume = -1; r.push(a.volume); return r; });
  t('med.vol.set', function () { a.volume = 0.5; var r1 = a.volume; a.volume = '0.25'; return [r1, a.volume]; });
  t('med.muted', function () { a.muted = true; var r = [a.muted, a.hasAttribute('muted')]; a.muted = false; return r.concat([a.muted, a.hasAttribute('muted')]); });
  t('med.defaultMuted', function () { a.defaultMuted = true; var r = [a.defaultMuted, a.muted, a.hasAttribute('muted')]; a.setAttribute('muted', ''); return r.concat([a.muted, a.defaultMuted]); });
  t('med.autoplay', function () { a.autoplay = true; return [a.autoplay, a.hasAttribute('autoplay')]; });
  t('med.controls', function () { a.controls = 1; return [a.controls, a.hasAttribute('controls')]; });
  t('med.loop', function () { a.loop = 'x'; return [a.loop, a.hasAttribute('loop')]; });
  t('med.preload', function () { a.preload = 'NONE'; var r1 = [a.preload, a.getAttribute('preload')]; a.preload = 'x'; return r1.concat([[a.preload, a.getAttribute('preload')]]); });
  t('med.preload.attr', function () { a.removeAttribute('preload'); var d = a.preload; a.setAttribute('preload', 'METADATA'); return [d, a.preload]; });
  t('med.cross', function () { var d = a.crossOrigin; a.crossOrigin = 'ANONYMOUS'; var r1 = [a.crossOrigin, a.getAttribute('crossorigin')]; a.crossOrigin = ''; return [d].concat([r1, [a.crossOrigin, a.getAttribute('crossorigin')]]); });
  t('med.textTracks', function () { var tt = a.textTracks; return [typeof tt, tt instanceof TextTrackList, tt.length]; });
  t('med.addTextTrack', function () { var tt = a.addTextTrack('subtitles'); return [typeof tt, tt && tt.constructor && tt.constructor.name, tt.kind, tt.label, tt.language, tt.mode]; });
  t('med.captureStream', function () { var s = a.captureStream(); return [typeof s, s instanceof MediaStream]; });
  t('med.setSinkId', function () { var p = a.setSinkId(null); settle('med.setSinkId.plain', p); return [typeof p, p instanceof Promise]; });
  t('med.setMediaKeys', function () { var p = a.setMediaKeys(null); settle('med.setMediaKeys.plain', p); return [typeof p, p instanceof Promise]; });

  // ---- HTMLVideoElement behavior
  t('vid.defaults', function () {
    return { videoWidth: v.videoWidth, videoHeight: v.videoHeight, poster: v.poster,
      playsInline: v.playsInline, width: v.width, height: v.height };
  });
  t('vid.poster', function () {
    // poster READS resolve against the document base; record path+query so
    // the value does not depend on the probe page's absolute URL.
    v.poster = 'x.png';
    var u1 = new URL(v.poster, location.href);
    var r1 = [u1.pathname + u1.search, v.getAttribute('poster')];
    v.poster = '';
    var u2 = new URL(v.poster, location.href);
    return [r1, [u2.pathname + u2.search === location.pathname + location.search ? '#base' : u2.pathname, v.getAttribute('poster')]];
  });
  t('vid.wh', function () { v.width = 5; var r1 = [v.width, v.getAttribute('width')]; v.width = 'abc'; var r2 = [v.width, v.getAttribute('width')]; v.height = '7'; return [r1, r2, [v.height, v.getAttribute('height')]]; });
  t('vid.playsInline', function () { v.playsInline = true; return [v.playsInline, v.hasAttribute('playsinline')]; });
  t('vid.remote', function () { var r = v.remote; return [typeof r, r instanceof RemotePlayback, String(r)]; });
  t('vid.gvpq', function () { var q = v.getVideoPlaybackQuality(); var o = {}; Object.keys(q).sort().forEach(function (k) { o[k] = q[k]; }); return o; });
  t('vid.rvcb', function () { var i1 = v.requestVideoFrameCallback(function () {}); var i2 = v.requestVideoFrameCallback(function () {}); return [typeof i1, i1, i2]; });
  t('vid.cvcb', function () { return [v.cancelVideoFrameCallback(1), v.cancelVideoFrameCallback(9999)]; });

  // ---- HTMLImageElement behavior (unattached, no src)
  t('img.defaults', function () {
    return { complete: i.complete, naturalWidth: i.naturalWidth, naturalHeight: i.naturalHeight,
      x: i.x, y: i.y, currentSrc: i.currentSrc, width: i.width, height: i.height,
      decoding: i.decoding, loading: i.loading, referrerPolicy: i.referrerPolicy,
      fetchPriority: i.fetchPriority, align: i.align, border: i.border, hspace: i.hspace,
      vspace: i.vspace, longDesc: i.longDesc, lowsrc: i.lowsrc, alt: i.alt,
      srcset: i.srcset, isMap: i.isMap, useMap: i.useMap, crossOrigin: i.crossOrigin };
  });
  t('img.complete.appended', function () { var j = document.createElement('img'); document.body.appendChild(j); return j.complete; });
  t('img.wh', function () { i.width = 5; var r1 = [i.width, i.getAttribute('width')]; i.width = 'abc'; var r2 = [i.width, i.getAttribute('width')]; i.height = '7'; return [r1, r2, [i.height, i.getAttribute('height')]]; });
  t('img.hsv', function () { i.hspace = 10; var r1 = [i.hspace, i.getAttribute('hspace')]; i.align = 'left'; var r2 = [i.align, i.getAttribute('align')]; i.border = '2'; return [r1, r2, [i.border, i.getAttribute('border')]]; });
  t('img.cross', function () { var d = i.crossOrigin; i.crossOrigin = 'ANONYMOUS'; var r1 = [i.crossOrigin, i.getAttribute('crossorigin')]; i.crossOrigin = ''; return [d].concat([r1, [i.crossOrigin, i.getAttribute('crossorigin')]]); });
  t('img.decode', function () { var p = i.decode(); settle('img.decode.plain', p); return [typeof p, p instanceof Promise]; });
  t('img.decoding', function () { i.decoding = 'ASYNC'; var r1 = [i.decoding, i.getAttribute('decoding')]; i.decoding = 'x'; return [r1, [i.decoding, i.getAttribute('decoding')]]; });
  t('img.loading', function () { i.loading = 'EAGER'; var r1 = [i.loading, i.getAttribute('loading')]; i.loading = 'x'; return [r1, [i.loading, i.getAttribute('loading')]]; });
  t('img.fetchPriority', function () { var d = i.fetchPriority; i.fetchPriority = 'HIGH'; return [d, i.fetchPriority, i.getAttribute('fetchpriority')]; });
  t('img.referrerPolicy', function () { var d = i.referrerPolicy; i.referrerPolicy = 'no-referrer'; return [d, i.referrerPolicy, i.getAttribute('referrerpolicy')]; });
  t('img.map', function () { i.isMap = true; var r1 = [i.isMap, i.hasAttribute('ismap')]; i.useMap = '#m'; var r2 = [i.useMap, i.getAttribute('usemap')]; i.longDesc = 'http://x/'; var r3 = [i.longDesc, i.getAttribute('longdesc')]; i.lowsrc = 'http://y/'; return [r1, r2, r3, [i.lowsrc, i.getAttribute('lowsrc')]]; });

  t('fn.length', function () {
    return { addTextTrack: HTMLMediaElement.prototype.addTextTrack.length,
      captureStream: HTMLMediaElement.prototype.captureStream.length,
      setSinkId: HTMLMediaElement.prototype.setSinkId.length,
      setMediaKeys: HTMLMediaElement.prototype.setMediaKeys.length,
      decode: HTMLImageElement.prototype.decode.length,
      getVideoPlaybackQuality: HTMLVideoElement.prototype.getVideoPlaybackQuality.length,
      requestVideoFrameCallback: HTMLVideoElement.prototype.requestVideoFrameCallback.length,
      cancelVideoFrameCallback: HTMLVideoElement.prototype.cancelVideoFrameCallback.length,
      requestPictureInPicture: HTMLVideoElement.prototype.requestPictureInPicture.length };
  });

  return Promise.all(PEND).then(function () { return JSON.stringify(R); });
})()
