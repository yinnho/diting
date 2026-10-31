(function () {
  // (k97 EXT leg) non-ASCII advances: Latin-1 + General Punctuation + a few
  // symbols, per font at 100px; system-ui additionally at all optical knots
  // (its per-char em drifts with size for these too).
  const NAMES = ["American Typewriter", "Andale Mono", "Apple Braille", "Apple SD Gothic Neo", "AppleGothic", "Apple Symbols", "Arial", "Arial Black", "Arial Narrow", "Arial Unicode MS", "Avenir", "Avenir Next", "Baskerville", "Big Caslon", "Brush Script MT", "Chalkboard", "Chalkduster", "Cochin", "Comic Sans MS", "Copperplate", "Courier New", "Futura", "Geneva", "Georgia", "Gill Sans", "Helvetica", "Helvetica Neue", "Hiragino Maru Gothic ProN", "Hiragino Sans", "Hiragino Sans CNS", "Hiragino Sans GB", "Hoefler Text", "Impact", "Kaiti SC", "Kaiti SC Black", "Lucida Grande", "Marker Felt", "Menlo", "Microsoft YaHei", "Monaco", "Apple Color Emoji", "Noto Color Emoji", "Optima", "Palatino", "Papyrus", "PingFang HK", "PingFang SC", "PingFang TC", "SimSun", "Snell Roundhand", "Songti SC", "STHeiti", "STHeiti Light", "STKaiti", "STSong", "Tahoma", "Times New Roman", "Trebuchet MS", "Verdana", "Yuanti SC", "Zapfino"];
  const GENERIC = ['sans-serif', 'serif', 'monospace', 'cursive', 'fantasy', 'system-ui', '-apple-system', '__absent__'];
  const FONTS = NAMES.concat(GENERIC);
  const CP = [];
  for (let c = 0xA0; c <= 0xFF; c++) CP.push(c);
  for (const c of [0x131, 0x152, 0x153, 0x2C6, 0x2DC, 0xB5, 0x20AC, 0x2122, 0x2212, 0x2260, 0x2264, 0x2265]) CP.push(c);
  for (let c = 0x2000; c <= 0x205E; c++) CP.push(c);
  for (let c = 0x2660; c <= 0x2666; c++) CP.push(c);
  const CHARS = CP.map((c) => String.fromCodePoint(c));
  const c = document.createElement('canvas');
  const x = c.getContext('2d');
  const out = { cp: CP, adv: {}, su: {} };
  for (const f of FONTS) {
    x.font = '100px "' + f + '"';
    out.adv[f] = CHARS.map((ch) => +x.measureText(ch).width.toFixed(4));
  }
  for (const s of [8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 28, 32, 40, 64, 100]) {
    x.font = s + 'px system-ui';
    out.su[s] = CHARS.map((ch) => +x.measureText(ch).width.toFixed(3));
  }
  return JSON.stringify(out);
})()
