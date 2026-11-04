(function () { // @@SCOPE
// god-file split: bootstrap's real proto-marker, aliased from its hidden d()
// export (exports-extra.json forces bootstrap to publish it even though the
// alias below makes this file the name's last declarer — gen-exports would
// otherwise move the publish here, past the load point of the alias itself).
const _markNativeProto = globalThis._markNativeProto;

const _rsStream = new WeakMap(); // ReadableStreamDefaultController | ReadableStreamDefaultReader -> its ReadableStream
if (typeof ReadableStream === 'undefined') {
  globalThis.ReadableStream = class ReadableStream {
    constructor(source = {}, strategy = {}) {
      this._source = source; this._queue = []; this._closed = false; this._errored = null;
      this._locked = false; this._pullBusy = false; this._pendingReads = [];
      const stream = this;
      // the controller / reader are instances of their own interfaces; state lives in _rsStream (members are on the prototypes)
      this._controller = Object.create(ReadableStreamDefaultController.prototype);
      _rsStream.set(this._controller, this);
      if (source.start) {
        try { source.start(this._controller); } catch (e) { this._controller.error(e); }
      }
      __hideOwn(this);
    }
    get locked() { return this._locked; }
    getReader() {
      this._locked = true;
      const stream = this;
      const reader = Object.create(ReadableStreamDefaultReader.prototype);
      _rsStream.set(reader, stream);
      return reader;
    }
    cancel() { this._closed = true; return Promise.resolve(); }
    async pipeTo(dest) {
      const reader = this.getReader();
      const writer = dest.getWriter();
      try {
        for (;;) {
          const r = await reader.read();
          if (r.done) break;
          await writer.write(r.value);
        }
        await writer.close();
      } catch (e) {
        try { await writer.abort(e); } catch (_) {}
        throw e;
      } finally {
        try { reader.releaseLock(); } catch (_) {}
        try { writer.releaseLock(); } catch (_) {}
      }
    }
    pipeThrough(transform) {
      const out = transform && transform.readable;
      if (!out) return new ReadableStream();
      this.pipeTo(transform.writable).catch(() => {});
      return out;
    }
    tee() {
      // Materialize what's buffered, then hand out two independent streams
      // over a copy — clone-style consumption. (Interleaved tee of a live
      // pull-driven source is not supported; no in-the-wild consumer we
      // care about does that.)
      const chunks = this._queue.slice();
      const wasClosed = this._closed;
      const wasErr = this._errored;
      const mk = () => new ReadableStream({
        start(c) { for (const ch of chunks) c.enqueue(ch); if (wasErr) c.error(wasErr); else if (wasClosed) c.close(); },
      });
      return [mk(), mk()];
    }
    // (#32) Chrome: values() is an enumerable prototype member returning the
    // async iterator, and Symbol.asyncIterator aliases it. The returned
    // iterator is itself async-iterable ([Symbol.asyncIterator] => this) —
    // for-await over rs.values() directly needs that.
    values() {
      const reader = this.getReader();
      const it = {
        next: () => reader.read(),
        return: () => { reader.releaseLock(); return Promise.resolve({done:true}); },
      };
      Object.defineProperty(it, Symbol.asyncIterator, {
        value: function () { return this; },
        writable: true, configurable: true, enumerable: false,
      });
      return it;
    }
  };
  Object.defineProperty(globalThis.ReadableStream.prototype, Symbol.asyncIterator, {
    value: globalThis.ReadableStream.prototype.values,
    writable: true, configurable: true, enumerable: false,
  });
}
// Stream controller/reader faces. Abstract in Chrome (Illegal constructor);
// the live instances created by the shims above get their prototypes attached
// so getReader().constructor.name and instanceof read like the real thing.
globalThis.ReadableStreamDefaultController = class ReadableStreamDefaultController {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  get desiredSize() { return 1; }
  close() {
    const stream = _rsStream.get(this);
    if (stream._closed) return;
    stream._closed = true;
    const waiters = stream._pendingReads.splice(0);
    for (const w of waiters) w.resolve({ value: undefined, done: true });
  }
  enqueue(chunk) {
    const stream = _rsStream.get(this);
    if (stream._closed) return;
    // A parked read gets the chunk directly; otherwise buffer it.
    if (stream._pendingReads.length > 0) {
      stream._pendingReads.shift().resolve({ value: chunk, done: false });
    } else {
      stream._queue.push(chunk);
    }
  }
  error(e) {
    const stream = _rsStream.get(this);
    if (stream._closed) return;
    stream._errored = e || new Error("stream error");
    stream._closed = true;
    const waiters = stream._pendingReads.splice(0);
    for (const w of waiters) w.reject(stream._errored);
  }
};
_markNativeProto(globalThis.ReadableStreamDefaultController.prototype);
globalThis.ReadableStreamDefaultReader = class ReadableStreamDefaultReader {
  // 刀87: Chrome 145 ctor ladder (k87-ctor-e) — argc, stream type, lock.
  constructor(stream) {
    if (arguments.length < 1) throw new TypeError("Failed to construct 'ReadableStreamDefaultReader': 1 argument required, but only 0 present.");
    if (!(typeof ReadableStream === 'function' && stream instanceof ReadableStream)) throw new TypeError("Failed to construct 'ReadableStreamDefaultReader': parameter 1 is not of type 'ReadableStream'.");
    if (stream._locked) throw new TypeError("Failed to construct 'ReadableStreamDefaultReader': ReadableStreamDefaultReader constructor can only accept readable streams that are not yet locked to a reader");
    stream._locked = true;
    _rsStream.set(this, stream);
  }
  read() {
    const stream = _rsStream.get(this);
    if (stream._queue.length > 0) return Promise.resolve({ value: stream._queue.shift(), done: false });
    if (stream._closed) {
      if (stream._errored) return Promise.reject(stream._errored);
      return Promise.resolve({ value: undefined, done: true });
    }
    // Pull-driven source (e.g. React's partial-flight re-wrapping
    // `new ReadableStream({async pull(controller){...}})`): ask the
    // source for a chunk, then hand back whatever it enqueued.
    if (stream._source.pull && !stream._pullBusy) {
      stream._pullBusy = true;
      return Promise.resolve()
        .then(() => stream._source.pull(stream._controller))
        .then(() => {
          stream._pullBusy = false;
          if (stream._queue.length > 0) return { value: stream._queue.shift(), done: false };
          if (stream._errored) throw stream._errored;
          return { value: undefined, done: true };
        }, (e) => {
          stream._pullBusy = false;
          stream._controller.error(e);
          throw stream._errored;
        });
    }
    // Live producer (a TransformStream's readable side, or any source
    // that enqueues after read): park the read until
    // enqueue/close/error wakes it.
    return new Promise((resolve, reject) => {
      stream._pendingReads.push({ resolve, reject });
    });
  }
  releaseLock() { _rsStream.get(this)._locked = false; }
  cancel() {
    const stream = _rsStream.get(this);
    stream._closed = true;
    if (stream._source.cancel) { try { stream._source.cancel(); } catch (e) {} }
    return Promise.resolve();
  }
  get closed() { return _rsStream.get(this)._closed ? Promise.resolve() : new Promise(() => {}); }
};
_markNativeProto(globalThis.ReadableStreamDefaultReader.prototype);
globalThis.WritableStreamDefaultController = class WritableStreamDefaultController {
  constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
  error() {}
};
_markNativeProto(globalThis.WritableStreamDefaultController.prototype);
if (typeof WritableStream === 'undefined') {
  globalThis.WritableStream = class WritableStream {
    constructor(sink = {}) { this._sink = sink; this._locked = false; this._controller = Object.create(WritableStreamDefaultController.prototype); __hideOwn(this);}
    get locked() { return this._locked; }
    getWriter() {
      this._locked = true;
      const stream = this;
      return {
        write(chunk) { if (stream._sink.write) stream._sink.write(chunk); return Promise.resolve(); },
        close() { if (stream._sink.close) stream._sink.close(); return Promise.resolve(); },
        abort() { return Promise.resolve(); },
        releaseLock() { stream._locked = false; },
        get ready() { return Promise.resolve(); },
        get closed() { return Promise.resolve(); },
        get desiredSize() { return 1; },
      };
    }
    close() { return Promise.resolve(); }
    abort() { return Promise.resolve(); }
  };
}
if (typeof TransformStream === 'undefined') {
  globalThis.TransformStream = class TransformStream {
    // Minimal but real: writes flow through `transformer.transform` (or pass
    // through untouched) into the readable side, so pipeThrough() doesn't
    // silently drop the payload the way an empty stub does.
    constructor(transformer = {}) {
      let rc = null;
      const controller = {
        enqueue: (c2) => { if (rc) rc.enqueue(c2); },
        close: () => { if (rc) rc.close(); },
        error: (e) => { if (rc) rc.error(e); },
      };
      __diting_liftSlot(this).readable = new ReadableStream({ start(c) { rc = c; } });
      __diting_liftSlot(this).writable = new WritableStream({
        write(chunk) {
          try {
            if (transformer.transform) {
              transformer.transform(chunk, controller);
            } else if (rc) rc.enqueue(chunk);
          } catch (e) { if (rc) rc.error(e); }
        },
        close() {
          try { if (transformer.flush) transformer.flush(controller); }
          catch (e) { if (rc) { rc.error(e); return; } }
          if (rc) rc.close();
        },
      });
    }
  };
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
