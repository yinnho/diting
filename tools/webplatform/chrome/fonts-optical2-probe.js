(function () {
  const c = document.createElement('canvas');
  const x = c.getContext('2d');
  const CHARS = Array.from({ length: 95 }, (_, i) => String.fromCharCode(32 + i));
  const SIZES = [9, 11, 13, 15, 17, 23, 28, 40, 64];
  const out = {};
  for (const f of ['system-ui', 'Apple Color Emoji']) {
    out[f] = {};
    for (const s of SIZES) {
      x.font = s + 'px ' + f;
      out[f][s] = CHARS.map((ch) => +x.measureText(ch).width.toFixed(3));
    }
  }
  return JSON.stringify(out);
})()
