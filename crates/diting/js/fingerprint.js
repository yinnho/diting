(function () { // @@SCOPE
// god-file split (刀63, persona fingerprint): the deterministic fingerprint
// draw — _fpRand (seeded PRNG), _fpPlatform (platform table pick),
// _getFp (the full persona draw: gpu/screens/canvas-font hash/audio
// thresholds/battery, cached against platform), _fp accessor. The seed and
// cache are the mutable pair: __diting_init and the persona-apply fn in
// bootstrap WRITE them through the a()-published accessors (sloppy-mode
// global assignment routes into the setters), everyone else reads _fp at
// runtime (audio/bchk_apis/canvas2d/navigator_persona/performance/uadata/
// webrtc). Pins _getFp/_fpCache/_fpSeed flipped bootstrap -> here (刀52).
let _fpSeed = 0;
function _fpRand(salt) {
  let h = (_fpSeed ^ (salt || 0)) | 0;
  h = Math.imul(h ^ (h >>> 16), 0x45d9f3b);
  h = Math.imul(h ^ (h >>> 13), 0x45d9f3b);
  return ((h ^ (h >>> 16)) >>> 0) / 0xFFFFFFFF;
}

var _fpCache = null;
// The persona must cohere with the UA the HTTP layer sends. A Windows
// Direct3D11 ANGLE renderer behind a macOS or Linux UA (what the old
// single-pool randomization produced) is a geographic/hardware
// impossibility that risk engines (Castle) score immediately.
function _fpPlatform() {
  const ua = globalThis.__diting_ua || '';
  if (ua.indexOf('Windows') !== -1) return 'win';
  if (ua.indexOf('Macintosh') !== -1 || ua.indexOf('Mac OS X') !== -1) return 'mac';
  if (ua.indexOf('Android') !== -1) return 'linux';
  return 'linux';
}
function _getFp() {
  // The cache is keyed by platform: during the V8 snapshot build no UA is
  // configured yet, so the first materialization defaults to the linux
  // persona. Once __diting_ua is set (per context, before __diting_init),
  // the cache rebuilds for the real platform instead of serving the frozen
  // snapshot-time one (which put Mesa GL strings behind a macOS UA).
  if (_fpCache && _fpCache.platform === _fpPlatform()) return _fpCache;
  const plat = _fpPlatform();
  // Per-platform GPU pools — vendor string and renderer must describe the
  // same machine the UA describes.
  const pools = {
    win: {
      gpu: [
        'ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 Direct3D11 vs_5_0 ps_5_0, D3D11)',
        'ANGLE (NVIDIA, NVIDIA GeForce GTX 1660 SUPER Direct3D11 vs_5_0 ps_5_0, D3D11)',
        'ANGLE (NVIDIA, NVIDIA GeForce RTX 2070 SUPER Direct3D11 vs_5_0 ps_5_0, D3D11)',
        'ANGLE (Intel, Intel(R) UHD Graphics 630 Direct3D11 vs_5_0 ps_5_0, D3D11)',
        'ANGLE (Intel, Intel(R) Iris(R) Xe Graphics Direct3D11 vs_5_0 ps_5_0, D3D11)',
        'ANGLE (AMD, AMD Radeon RX 580 Direct3D11 vs_5_0 ps_5_0, D3D11)',
        'ANGLE (AMD, AMD Radeon RX 6700 XT Direct3D11 vs_5_0 ps_5_0, D3D11)',
        'ANGLE (NVIDIA, NVIDIA GeForce RTX 4070 Direct3D11 vs_5_0 ps_5_0, D3D11)',
        'ANGLE (NVIDIA, NVIDIA GeForce GTX 1080 Ti Direct3D11 vs_5_0 ps_5_0, D3D11)',
        'ANGLE (Intel, Intel(R) UHD Graphics 770 Direct3D11 vs_5_0 ps_5_0, D3D11)',
        'ANGLE (AMD, AMD Radeon RX 5700 XT Direct3D11 vs_5_0 ps_5_0, D3D11)',
        'ANGLE (NVIDIA, NVIDIA GeForce RTX 3080 Direct3D11 vs_5_0 ps_5_0, D3D11)',
      ],
      // Device coherence (fp-diff-1014 P1): [arch, cores] per GPU entry, so
      // ANGLE Apple M3 can never report architecture x86 or 12 cores — a
      // detector cross-checking UNMASKED_RENDERER vs getHighEntropyValues
      // vs hardwareConcurrency saw three different machines before.
      hw: [
        ['x86', 12], ['x86', 12], ['x86', 16], ['x86', 8], ['x86', 8],
        ['x86', 16], ['x86', 12], ['x86', 16], ['x86', 16], ['x86', 12],
        ['x86', 16], ['x86', 16],
      ],
      // RAM options per GPU entry, already in deviceMemory shape (Chrome
      // rounds actual GiB down to a power of 2; budget boxes 8/16, gaming
      // rigs 16/32 — see the P8 note by __diting_setPersona).
      mem: [
        [16, 32], [16, 32], [16, 32], [8, 16], [8, 16],
        [8, 16], [16, 32], [16, 32], [16, 32], [16, 32],
        [16, 32], [16, 32],
      ],
      vendor: [
        'Google Inc. (NVIDIA)','Google Inc. (NVIDIA)','Google Inc. (NVIDIA)',
        'Google Inc. (Intel)','Google Inc. (Intel)',
        'Google Inc. (AMD)','Google Inc. (AMD)',
        'Google Inc. (NVIDIA)','Google Inc. (NVIDIA)',
        'Google Inc. (Intel)','Google Inc. (AMD)','Google Inc. (NVIDIA)',
      ],
      // UA-CH platformVersion per OS family (mac = Sequoia 15.2, win = 11 23H2,
      // linux = Ubuntu 24.04 kernel line).
      platformVersion: '15.0.0',
      screens: [[1920,1080],[2560,1440],[1366,768],[1536,864],[1440,900],[1680,1050],[1280,720],[3840,2160]],
      dprs: [1,1,1,1.25,1,1,1,1],
    },
    mac: {
      gpu: [
        'ANGLE (Apple, ANGLE Metal Renderer: Apple M1, Unspecified Version)',
        'ANGLE (Apple, ANGLE Metal Renderer: Apple M2, Unspecified Version)',
        'ANGLE (Apple, ANGLE Metal Renderer: Apple M2 Pro, Unspecified Version)',
        'ANGLE (Apple, ANGLE Metal Renderer: Apple M3, Unspecified Version)',
        'ANGLE (Apple, ANGLE Metal Renderer: Apple M3 Pro, Unspecified Version)',
        'ANGLE (Apple, ANGLE Metal Renderer: Apple M4, Unspecified Version)',
        'ANGLE (Intel, Intel(R) Iris(TM) Plus Graphics 645, OpenGL 4.1)',
      ],
      // M1/M2/M3 = 4P+4E, M2 Pro = 6P+4E, M3 Pro = 6P+6E, M4 = 4P+6E,
      // Iris 645 = 2018 MacBook Air (i5-8210Y, 2c/4t → 4).
      hw: [
        ['arm', 8], ['arm', 8], ['arm', 10], ['arm', 8], ['arm', 12],
        ['arm', 10], ['x86', 4],
      ],
      // RAM options per chip, in deviceMemory shape (GiB rounded down to a
      // power of 2 — 18 GB M3 Pro reports 16, 36 GB reports 32, 24 GB M4
      // reports 16). P8: an M3 Pro persona claiming 8 GB describes a machine
      // Apple never sold — the exact class of cross-field lie 300015 hunts.
      mem: [
        [8, 16], [8, 16], [16, 32], [8, 16], [16, 32],
        [16, 32], [8, 16],
      ],
      vendor: [
        'Google Inc. (Apple)','Google Inc. (Apple)','Google Inc. (Apple)',
        'Google Inc. (Apple)','Google Inc. (Apple)','Google Inc. (Apple)',
        'Google Inc. (Intel)',
      ],
      platformVersion: '15.2.0',
      screens: [[1512,982],[1728,1117],[1440,900],[1920,1080],[2560,1440],[1170,2532]],
      // Built-in retina panels run at 2x; the two external sizes run at 1x.
      dprs: [2,2,2,1,1,3],
    },
    linux: {
      gpu: [
        'ANGLE (Intel, Mesa Intel(R) UHD Graphics 630 (CML GT2), OpenGL 4.6 (Core Profile) Mesa 23.2.1)',
        'ANGLE (Intel, Mesa Intel(R) Iris(R) Xe Graphics (TGL GT2), OpenGL 4.6 (Core Profile) Mesa 23.2.1)',
        'ANGLE (AMD, AMD Radeon RX 6700 XT (navi22, LLVM 15.0.7, DRM 3.54, LLVM 15.0.7), OpenGL 4.6 (Core Profile) Mesa 23.2.1)',
        'ANGLE (NVIDIA, NVIDIA GeForce RTX 3060/PCIe/SSE2, OpenGL 4.6.0 NVIDIA 535.183.01)',
      ],
      hw: [['x86', 12], ['x86', 8], ['x86', 16], ['x86', 12]],
      mem: [[8, 16], [8, 16], [16, 32], [16, 32]],
      vendor: [
        'Google Inc. (Intel)','Google Inc. (Intel)','Google Inc. (AMD)','Google Inc. (NVIDIA)',
      ],
      platformVersion: '6.8.0',
      screens: [[1920,1080],[2560,1440],[1366,768],[3840,2160],[1680,1050]],
      dprs: [1,1,1,1,1],
    },
  };
  const pool = pools[plat];
  const idx = Math.floor(_fpRand(42) * pool.gpu.length);
  const sIdx = Math.floor(_fpRand(300) * pool.screens.length);
  const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
  let cfp = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUg';
  for (let i = 0; i < 40; i++) cfp += chars[Math.floor(_fpRand(500 + i) * 64)];
  cfp += '==';
  _fpCache = {
    platform: plat,
    gpu: pool.gpu[idx], gpuVendor: pool.vendor[idx],
    arch: pool.hw[idx][0], hw: pool.hw[idx][1], bitness: '64',
    mem: pool.mem[idx][Math.floor(_fpRand(401) * pool.mem[idx].length)],
    platformVersion: pool.platformVersion,
    screen: pool.screens[sIdx], dpr: pool.dprs[sIdx],
    audioBaseLatency: 0.002 + _fpRand(100) * 0.008,
    audioSampleRate: [44100, 48000][Math.floor(_fpRand(101) * 2)],
    compThreshold: -24 + (_fpRand(102) - 0.5) * 4,
    compKnee: 30 + (_fpRand(103) - 0.5) * 4,
    compRatio: 12 + (_fpRand(104) - 0.5) * 4,
    batteryLevel: 0.5 + _fpRand(200) * 0.5,
    batteryCharging: _fpRand(201) > 0.3,
    canvasFingerprint: cfp,
  };
  return _fpCache;
}
function _fp(key) { return _getFp()[key]; }

// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('_fp', _fp);
  a('_fpCache', () => _fpCache, (v) => { _fpCache = v; });
  d('_fpPlatform', _fpPlatform);
  d('_fpRand', _fpRand);
  a('_fpSeed', () => _fpSeed, (v) => { _fpSeed = v; });
  d('_getFp', _getFp);
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
