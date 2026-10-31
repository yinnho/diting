(function () {
  // system-ui (SF Pro) optical variant probe: per-char advances at sizes
  // across the 20px Text/Display boundary. If two clean tables with a
  // boundary, model it; same leg for Apple Color Emoji (bitmap fallback).
  const c = document.createElement('canvas');
  const x = c.getContext('2d');
  const CHARS = Array.from({ length: 95 }, (_, i) => String.fromCharCode(32 + i));
  const SIZES = [8, 10, 12, 14, 16, 18, 19, 20, 21, 24, 32, 100];
  const out = {};
  for (const f of ['system-ui', '__absent__', 'Apple Color Emoji']) {
    out[f] = {};
    for (const s of SIZES) {
      x.font = s + 'px ' + f;
      out[f][s] = CHARS.map((ch) => +x.measureText(ch).width.toFixed(3));
    }
  }
  return JSON.stringify(out);
})()
