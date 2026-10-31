(function () {
  const NAMES = ["American Typewriter", "Andale Mono", "Apple Braille", "Apple SD Gothic Neo", "AppleGothic", "Apple Symbols", "Arial", "Arial Black", "Arial Narrow", "Arial Unicode MS", "Avenir", "Avenir Next", "Baskerville", "Big Caslon", "Brush Script MT", "Chalkboard", "Chalkduster", "Cochin", "Comic Sans MS", "Copperplate", "Courier New", "Futura", "Geneva", "Georgia", "Gill Sans", "Helvetica", "Helvetica Neue", "Hiragino Maru Gothic ProN", "Hiragino Sans", "Hiragino Sans CNS", "Hiragino Sans GB", "Hoefler Text", "Impact", "Kaiti SC", "Kaiti SC Black", "Lucida Grande", "Marker Felt", "Menlo", "Microsoft YaHei", "Monaco", "Apple Color Emoji", "Noto Color Emoji", "Optima", "Palatino", "Papyrus", "PingFang HK", "PingFang SC", "PingFang TC", "SimSun", "Snell Roundhand", "Songti SC", "STHeiti", "STHeiti Light", "STKaiti", "STSong", "Tahoma", "Times New Roman", "Trebuchet MS", "Verdana", "Yuanti SC", "Zapfino"];
  const c = document.createElement('canvas');
  const x = c.getContext('2d');
  const S = 'mmMwWLli10Oo#@ AVTo f';
  x.font = "100px '__absent_diting_x__'";
  const base = x.measureText(S).width;
  const r = {};
  for (const n of NAMES) { x.font = '100px "' + n + '"'; r[n] = +x.measureText(S).width.toFixed(2); }
  return JSON.stringify({ base: base, fonts: r });
})()