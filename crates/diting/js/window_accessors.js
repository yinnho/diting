// Window own-accessor face (#30). Real Chrome 157: every one of window's 188 own accessors is a native
// `get <name>` / `set <name>` pair (getter.name, setter.name and Function.prototype.toString all carry it).
// The engine installs them from a dozen modules (interface_sweep's slot converters, global_surface's
// singletons, window_faces, location_face, ...) and none named them: 155 read `get`, 28 `wrapped`, plus
// `docGet` / bare names / anonymous setters - a one-line Object.getOwnPropertyDescriptor(window, 'navigator')
// .get.name probe told the engine apart. Truth: probe over all window own accessors on Chrome 157.0.8103.38
// (2026-11-06); configurable/enumerable already matched 188/188.
//
// Second gap, same probe: 20 [Replaceable] attributes Chrome gives a setter that the engine lacked (assignment
// silently ignored, or a strict-mode TypeError). Chrome's setter replaces the accessor IN PLACE (own-name order
// unchanged) by a {value, writable, enumerable, configurable: true} data property; a receiver other than the
// window throws "Illegal invocation"; length 1; no argument stores undefined. Deliberate exception (#105): the four
// window SIZE attributes get Chrome's setter face (name / length / brand check) but keep ignoring the write - a douyin
// resize polyfill overwrote them with document extents, and the engine's layout extents are not Chrome's, so taking the
// write skewed every page coordinate by a viewport. Flip when layout extents match.
//
// Runs once at snapshot time and again at the end of every page init (window_faces / __diting_init reinstall some
// accessors per page), before window_order (which re-defines descriptors with the same functions).
(function () {
  const g = globalThis, gopd = Object.getOwnPropertyDescriptor, def = Object.defineProperty;
  const gopn = Object.getOwnPropertyNames, mark = g.__diting_markNative;
  const REPLACEABLE = ['navigation', 'locationbar', 'menubar', 'personalbar', 'scrollbars', 'statusbar', 'toolbar',
    'origin', 'external', 'innerWidth', 'innerHeight', 'scrollX', 'pageXOffset', 'scrollY', 'pageYOffset',
    'outerWidth', 'outerHeight', 'clientInformation', 'scheduler', 'viewport'];
  // fn -> the accessor name it was given; a function two accessors share gets its own wrapper for the second.
  const given = new WeakMap();
  const face = (fn, want, len, canSwap) => {
    const had = given.get(fn);
    if (had === want) return fn;
    if (had !== undefined) {
      if (!canSwap) return fn; // frozen slot: the function cannot be replaced, keep its first name
      const inner = fn;
      fn = function () { return inner.apply(this, arguments); };
    }
    def(fn, 'name', { value: want, configurable: true });
    def(fn, 'length', { value: len, configurable: true });
    mark(fn);
    given.set(fn, want);
    return fn;
  };
  const KEEP_SIZE = ['innerWidth', 'innerHeight', 'outerWidth', 'outerHeight'];
  const replaceSetter = (n) => KEEP_SIZE.includes(n)
    ? function (v) { if (this !== g) throw new TypeError('Illegal invocation'); }
    : function (v) {
      if (this !== g) throw new TypeError('Illegal invocation');
      def(g, n, { value: v, writable: true, enumerable: true, configurable: true });
    };
  const run = function () {
    for (const n of gopn(g)) {
      const d = gopd(g, n);
      if (!d || !(d.get || d.set) || n.startsWith('__')) continue;
      let get = d.get, set = d.set;
      if (set === undefined && d.configurable && REPLACEABLE.includes(n)) set = replaceSetter(n);
      // Frozen accessors keep their functions: renaming in place is all that can (and needs to) happen.
      const nget = get && face(get, 'get ' + n, 0, d.configurable);
      const nset = set && face(set, 'set ' + n, 1, d.configurable);
      if ((nget !== d.get || nset !== d.set) && d.configurable) {
        def(g, n, { get: nget, set: nset, enumerable: d.enumerable, configurable: true });
      }
    }
  };
  run();
  def(g, '__diting_window_accessors', { value: run, writable: true, enumerable: false, configurable: true });
})();
