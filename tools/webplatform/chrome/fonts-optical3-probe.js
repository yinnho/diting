(function () {
  const c = document.createElement('canvas');
  const x = c.getContext('2d');
  const CHARS = Array.from({ length: 95 }, (_, i) => String.fromCharCode(32 + i));
  const SIZES = [22, 25, 26, 27];
  const out = {};
  for (const f of ['system-ui', 'Apple Color Emoji']) {
    out[f] = {};
    for (const s of SIZES) {
      x.font = s + 'px ' + f;
      out[f][s] = CHARS.map((ch) => +x.measureText(ch).width.toFixed(3));
    }
  }
  // also confirm the 13 emoji chars are identical across the set at 3 sizes
  x.font = '12px "Apple Color Emoji"';
  out.emoji13 = [0, 3, 10, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25].map((i) => +x.measureText(CHARS[i]).width.toFixed(3));
  return JSON.stringify(out);
})()
