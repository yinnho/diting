(function () {
  const NAMES = ["American Typewriter", "Andale Mono", "Apple Braille", "Apple SD Gothic Neo", "AppleGothic", "Apple Symbols", "Arial", "Arial Black", "Arial Narrow", "Arial Unicode MS", "Avenir", "Avenir Next", "Baskerville", "Big Caslon", "Brush Script MT", "Chalkboard", "Chalkduster", "Cochin", "Comic Sans MS", "Copperplate", "Courier New", "Futura", "Geneva", "Georgia", "Gill Sans", "Helvetica", "Helvetica Neue", "Hiragino Maru Gothic ProN", "Hiragino Sans", "Hiragino Sans CNS", "Hiragino Sans GB", "Hoefler Text", "Impact", "Kaiti SC", "Kaiti SC Black", "Lucida Grande", "Marker Felt", "Menlo", "Microsoft YaHei", "Monaco", "Apple Color Emoji", "Noto Color Emoji", "Optima", "Palatino", "Papyrus", "PingFang HK", "PingFang SC", "PingFang TC", "SimSun", "Snell Roundhand", "Songti SC", "STHeiti", "STHeiti Light", "STKaiti", "STSong", "Tahoma", "Times New Roman", "Trebuchet MS", "Verdana", "Yuanti SC", "Zapfino"].concat(['sans-serif','serif','monospace','cursive','fantasy','system-ui','-apple-system','__absent__']);
  const c = document.createElement('canvas');
  const x = c.getContext('2d');
  const out = { v1000: {}, v100: {}, vspot: {} };
  for (const f of NAMES) {
    x.font = '1000px "' + f + '"';
    const m1 = x.measureText('Hg');
    out.v1000[f] = [m1.fontBoundingBoxAscent, m1.fontBoundingBoxDescent];
    x.font = '100px "' + f + '"';
    const m2 = x.measureText('Hg');
    out.v100[f] = [m2.fontBoundingBoxAscent, m2.fontBoundingBoxDescent];
    out.vspot[f] = [10, 12, 16, 20, 32].map((s) => { x.font = s + 'px "' + f + '"'; const m = x.measureText('Hg'); return [m.fontBoundingBoxAscent, m.fontBoundingBoxDescent]; });
  }
  return JSON.stringify(out);
})()