(function () {
  // (k97 fix) UNQUOTED generic families — the earlier capture used quoted
  // names, and Chrome resolves quoted "monospace" to a different (hidden)
  // face than the unquoted generic. Capture adv+vert unquoted.
  const G = ['sans-serif', 'serif', 'monospace', 'cursive', 'fantasy', 'system-ui', '-apple-system', 'ui-sans-serif', 'ui-serif', 'ui-monospace', '__absent__'];
  const CHARS = Array.from({ length: 95 }, (_, i) => String.fromCharCode(32 + i));
  const c = document.createElement('canvas');
  const x = c.getContext('2d');
  const out = { adv: {}, vert: {} };
  for (const g of G) {
    x.font = '100px ' + g;
    out.adv[g] = CHARS.map((ch) => +x.measureText(ch).width.toFixed(4));
    x.font = '16px ' + g;
    const m = x.measureText('Hg');
    out.vert[g] = [m.fontBoundingBoxAscent, m.fontBoundingBoxDescent];
  }
  return JSON.stringify(out);
})()
