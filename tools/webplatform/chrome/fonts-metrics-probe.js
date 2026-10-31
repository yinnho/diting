(function () {
  const NAMES = ["American Typewriter", "Andale Mono", "Apple Braille", "Apple SD Gothic Neo", "AppleGothic", "Apple Symbols", "Arial", "Arial Black", "Arial Narrow", "Arial Unicode MS", "Avenir", "Avenir Next", "Baskerville", "Big Caslon", "Brush Script MT", "Chalkboard", "Chalkduster", "Cochin", "Comic Sans MS", "Copperplate", "Courier New", "Futura", "Geneva", "Georgia", "Gill Sans", "Helvetica", "Helvetica Neue", "Hiragino Maru Gothic ProN", "Hiragino Sans", "Hiragino Sans CNS", "Hiragino Sans GB", "Hoefler Text", "Impact", "Kaiti SC", "Kaiti SC Black", "Lucida Grande", "Marker Felt", "Menlo", "Microsoft YaHei", "Monaco", "Apple Color Emoji", "Noto Color Emoji", "Optima", "Palatino", "Papyrus", "PingFang HK", "PingFang SC", "PingFang TC", "SimSun", "Snell Roundhand", "Songti SC", "STHeiti", "STHeiti Light", "STKaiti", "STSong", "Tahoma", "Times New Roman", "Trebuchet MS", "Verdana", "Yuanti SC", "Zapfino"];
  const GENERIC = ['sans-serif','serif','monospace','cursive','fantasy','system-ui','-apple-system','__absent__'];
  const FONTS = NAMES.concat(GENERIC);
  const CHARS = Array.from({ length: 95 }, (_, i) => String.fromCharCode(32 + i));
  const c = document.createElement('canvas');
  const x = c.getContext('2d');
  const F = (f, s) => { x.font = s + 'px "' + f + '"'; };
  const out = { adv: {}, vert: {}, lin: {}, kern: {} };
  const PAIRS = ['AV','To','Ta','YV','fo','Tr','Wa','LT','rya'];
  const LIN = 'Hello World gjpq 123';
  for (const f of FONTS) {
    F(f, 100);
    out.adv[f] = CHARS.map((ch) => +x.measureText(ch).width.toFixed(4));
    out.kern[f] = PAIRS.map((p) => +(x.measureText(p).width - x.measureText(p[0]).width - x.measureText(p[1]).width).toFixed(4));
    F(f, 16);
    const m = x.measureText('Hg');
    out.vert[f] = [+m.fontBoundingBoxAscent.toFixed(3), +m.fontBoundingBoxDescent.toFixed(3)];
    out.lin[f] = [10, 16, 32].map((s) => { F(f, s); return +x.measureText(LIN).width.toFixed(4); });
  }
  // default-table CJK & symbol legs (persona is zh)
  F('__absent__', 100);
  out.cjk = ['你好世界中文测试','한국어','·—…“”'].map((s) => +x.measureText(s).width.toFixed(4));
  out.cjkChars = Array.from('你好世办长度们').map((ch) => +x.measureText(ch).width.toFixed(4));
  return JSON.stringify(out);
})()