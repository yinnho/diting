(async function () {
  // (刀95) instance-requiring interface ladders: for interfaces that cannot be
  // `new`-ed with zero args, take a REAL instance, enumerate prototype methods
  // with fn.length >= 1 and call each with ZERO args. In Chrome the argc
  // TypeError fires before any side effect, so the hang risk that killed the
  // wide probe does not apply to this filtered set.
  const out = {};
  const val = (v) => { if (v === null) return 'null'; const t = typeof v; if (t === 'number') return Number.isNaN(v) ? 'NaN' : v; if (t === 'string') return v.length > 40 ? v.slice(0, 40) : v; if (t === 'boolean' || t === 'undefined') return String(v); if (t === 'function') return 'fn'; if (t === 'object') return 'obj:' + ((v.constructor && v.constructor.name) || '?'); return t; };
  const div = document.createElement('div');
  const svgNS = 'http://www.w3.org/2000/svg';
  const path = document.createElementNS(svgNS, 'path');
  const P = {};
  const T = (n, f) => { try { const v = f(); if (v !== undefined) P[n] = v; } catch (e) { out[n] = { __SKIP: (e && e.name) || 'skip' }; } };
  T('Node', () => div);
  T('Element', () => div);
  T('HTMLElement', () => div);
  T('Document', () => document);
  T('CharacterData', () => document.createTextNode('x'));
  T('Text', () => document.createTextNode('x'));
  T('Comment', () => document.createComment('x'));
  T('Attr', () => document.createAttribute('a'));
  T('NamedNodeMap', () => div.attributes);
  T('DOMTokenList', () => div.classList);
  T('CSSStyleDeclaration', () => div.style);
  T('Selection', () => getSelection());
  T('ShadowRoot', () => div.attachShadow({ mode: 'open' }));
  T('HTMLInputElement', () => document.createElement('input'));
  T('HTMLTextAreaElement', () => document.createElement('textarea'));
  T('HTMLSelectElement', () => document.createElement('select'));
  T('HTMLOptionsCollection', () => document.createElement('select').options);
  T('HTMLFormElement', () => document.createElement('form'));
  T('HTMLButtonElement', () => document.createElement('button'));
  T('HTMLAnchorElement', () => document.createElement('a'));
  T('HTMLImageElement', () => document.createElement('img'));
  T('HTMLIFrameElement', () => document.createElement('iframe'));
  T('HTMLCanvasElement', () => document.createElement('canvas'));
  T('HTMLMediaElement', () => document.createElement('video'));
  T('HTMLTableElement', () => document.createElement('table'));
  T('HTMLTableRowElement', () => document.createElement('tr'));
  T('HTMLTableSectionElement', () => document.createElement('tbody'));
  T('SVGElement', () => document.createElementNS(svgNS, 'svg'));
  T('SVGGraphicsElement', () => path);
  T('SVGSVGElement', () => document.createElementNS(svgNS, 'svg'));
  T('SVGGeometryElement', () => path);
  T('SVGPathElement', () => path);
  T('SVGTextContentElement', () => document.createElementNS(svgNS, 'text'));
  T('Window', () => self);
  T('Location', () => location);
  T('History', () => history);
  T('Navigator', () => navigator);
  T('Storage', () => localStorage);
  T('Screen', () => screen);
  T('Performance', () => performance);
  T('MutationObserver', () => new MutationObserver(function () {}));
  T('ResizeObserver', () => new ResizeObserver(function () {}));
  T('IntersectionObserver', () => new IntersectionObserver(function () {}));
  T('PerformanceObserver', () => new PerformanceObserver(function () {}));
  T('MessagePort', () => new MessageChannel().port1);
  T('WebSocket', () => new WebSocket('ws://127.0.0.1:__WS_PORT__'));
  T('BroadcastChannel', () => new BroadcastChannel('k95'));
  T('CanvasRenderingContext2D', () => document.createElement('canvas').getContext('2d'));
  T('CanvasGradient', () => document.createElement('canvas').getContext('2d').createLinearGradient(0, 0, 1, 1));
  T('CanvasPattern', () => { const c = document.createElement('canvas'); c.width = c.height = 1; return c.getContext('2d').createPattern(c, 'repeat'); });
  T('AudioNode', () => new AudioContext().createGain());
  T('TrustedTypePolicyFactory', () => trustedTypes);

  // (刀95) Chrome placement truths: Location/Window methods are OWN on the
  // instance (their prototypes carry only constructor). For Window only the
  // lowercase WebIDL methods matter — the uppercase own props are language/
  // DOM constructors whose no-new call was covered by the ctor-ladder knife.
  const OWN = { Window: self, Location: location };
  for (const name of Object.keys(P).concat(Object.keys(OWN))) {
    const r = {};
    const srcObj = OWN[name] || null;
    if (!srcObj) {
      const C = globalThis[name];
      if (typeof C !== 'function' || !C.prototype) { out[name] = { __NOCtor: typeof C }; continue; }
    }
    const holder = srcObj || globalThis[name].prototype;
    const inst = srcObj || P[name];
    const keys = Object.getOwnPropertyNames(holder).filter((k) => k !== 'constructor' && !(srcObj && (k[0] >= 'A' && k[0] <= 'Z')));
    for (const k of keys) {
      const d = Object.getOwnPropertyDescriptor(holder, k);
      if (!d || typeof d.value !== 'function' || d.value.length < 1) continue;
      try {
        let v = inst[k]();
        if (v && typeof v.then === 'function') {
          v = await Promise.race([v.then((a) => 'resolved:' + val(a), (e) => 'rejected:' + e.name + ':' + String(e.message).slice(0, 80)),
            new Promise((res) => setTimeout(() => res('pending'), 20))]);
          r[k] = 'P:' + v;
        } else r[k] = val(v);
      } catch (e) { r[k] = 'T:' + e.name + ':' + String(e.message).slice(0, 90); }
    }
    out[name] = r;
  }
  try { const po = new PerformanceObserver(function () {}); out.__po = 'len' + PerformanceObserver.prototype.observe.length + ' zero:' + (function(){try{po.observe();return 'ok'}catch(e){return e.name+':'+String(e.message).slice(0,110)}})(); } catch (e) { out.__po = 'SKIP ' + e.name; }
  return JSON.stringify(out);
})()
