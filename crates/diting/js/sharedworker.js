(function () { // @@SCOPE
// god-file split: bootstrap's real native marker, reached through its
// hidden prop (house style — see serializer.js) so no cross-file load
// edge appears in the dependency ratchet.
const _markNative = globalThis.__diting_markNative;

// SharedWorker (#213): one script instance per (url, name), with a real
// MessagePort pair bridging every constructor call to it. The old stub
// swallowed each port.postMessage and never replied, so an SDK that
// coordinates or signs through a worker hung on its first request — 抖店's
// sif-dxs security layer builds exactly that channel (self.onconnect →
// event.ports[0] → port.start(), then every page request waits on it) and the
// page died on the SDK's own timeout with a blank screen. Scripts execute in
// the page realm like the dedicated-worker path; sharing is per page and per
// (url, name), which is what a single document can observe.
if (typeof SharedWorker === 'undefined') {
  globalThis.__sharedWorkerInstances = globalThis.__sharedWorkerInstances || new Map();
  function _sharedWorkerKey(url, name) {
    let abs = String(url == null ? '' : url);
    try { abs = new URL(abs, (globalThis.location && globalThis.location.href) || 'https://example.com/').href; } catch {}
    return abs + '\u0000' + (name == null ? '' : String(name));
  }
  // SharedWorkerGlobalScope shape: the common worker surface plus onconnect.
  // No postMessage on the global — messages travel on connected ports, which
  // arrive as the connect event's ports[0], exactly as Chrome delivers them.
  function _sharedWorkerScope(shim) {
    const scope = _workerSurfaceCommon(_workerImportScripts(shim));
    scope.onconnect = null;
    scope.onmessage = null;
    scope.addEventListener = (type, fn) => { scope['on' + type] = fn; };
    scope.removeEventListener = (type, fn) => { if (scope['on' + type] === fn) scope['on' + type] = null; };
    scope.close = () => { shim._closed = true; };
    scope.self = scope;
    return scope;
  }
  function _runSharedScript(inst) {
    const scope = inst.scope;
    const fn = new Function('self', 'window', 'document', 'navigator', 'location', 'importScripts',
      'with (self) {\n' + inst.code + '\n}');
    const restore = _maskMainThreadGlobals();
    try { fn(_workerScopeChain(scope), undefined, undefined, scope.navigator, scope.location, scope.importScripts); }
    finally { restore(); }
  }
  function _connectSharedPort(inst, workerEnd) {
    const scope = inst.scope;
    if (!scope) return;
    const evt = { type: 'connect', ports: [workerEnd], target: scope, currentTarget: scope };
    // Worker-code entry point: mask the page global for the synchronous
    // dispatch (the wheel reads the DOM family off globalThis via
    // webpack's global helper, which no with-scope can shadow).
    const restore = _maskMainThreadGlobals();
    try {
      // A SharedWorker script may register onconnect (the spec path) or, in
      // the wild, a plain onmessage; either way the port is what carries the
      // traffic, and the worker-side port buffers page messages sent before
      // the handler attached.
      if (typeof scope.onconnect === 'function') scope.onconnect(evt);
      else if (typeof scope.onmessage === 'function') scope.onmessage(evt);
    } catch (e) {
      console.error('SharedWorker error:', e && e.message, e && e.stack);
      if (typeof inst.onerror === 'function') { try { inst.onerror({ message: String((e && e.message) || e), error: e }); } catch (_) {} }
    } finally { restore(); }
  }
  globalThis.SharedWorker = class SharedWorker {
    constructor(url, name) {
      this.onerror = null;
      __hideOwn(this);
      const key = _sharedWorkerKey(url, name);
      let inst = globalThis.__sharedWorkerInstances.get(key);
      if (!inst) {
        inst = { code: null, scope: null, ready: false, onerror: null, shim: { _url: null, _imported: {} }, pendingPorts: [] };
        globalThis.__sharedWorkerInstances.set(key, inst);
        _workerSourceFor(url)
          .then(res => {
            inst.shim._url = res.fetchUrl;
            return _preloadWorkerImports(inst.shim, res.code).then(() => res.code);
          })
          .then(code => {
            inst.code = code;
            if (typeof code === 'string') {
              try {
                inst.scope = _sharedWorkerScope(inst.shim);
                _runSharedScript(inst);
              } catch (e) {
                console.error('SharedWorker error:', e && e.message, e && e.stack);
                inst.error = e;
              }
            }
            inst.ready = true;
            const ports = inst.pendingPorts;
            inst.pendingPorts = [];
            for (const p of ports) _connectSharedPort(inst, p);
          });
      }
      const [pageEnd, workerEnd] = _newPortPair();
      if (inst.ready && inst.scope) _connectSharedPort(inst, workerEnd);
      else inst.pendingPorts.push(workerEnd);
      __diting_liftSlot(this).port = pageEnd;
    }
  };
  try { _markNative(globalThis.SharedWorker); } catch (_) {}
}
// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
