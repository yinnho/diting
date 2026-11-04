(function () { // @@SCOPE
// god-file split (刀58, MutationObserver family): the observer class
// (registration options validation, record queueing), the microtask
// flush (__moFlush — one checkpoint, registration order, re-arm on
// callback), the WebKitMutationObserver alias constructor, and
// __notifyMutation (the engine's record entry point: sibling slots,
// per-observer optioned oldValue). All installs are globalThis member
// forms; cross-file consumers (domcore dispatch, ctor_ladders,
// shape_fix, global_surface, window_order, worker) call at runtime.
// _wrap (domcore global) and __diting_mkMutationRecord (platform_bundle
// published) resolve at runtime. Native marking migrated here (刀22).
const _markNative = globalThis.__diting_markNative;
globalThis.__mutationObservers = [];
globalThis.MutationObserver = class MutationObserver {
  constructor(callback) {
    this._callback = callback;
    this._targets = [];
    this._records = [];
    __hideOwn(this);
  }
  observe(target, options) {
    // WebIDL validates parameter 1 first (Chrome: "parameter 1 is not of
    // type 'Node'."), then the dictionary needs at least one of the three
    // mutation kinds true — both messages pinned against Chrome 155 (#272).
    if (!target || typeof target._nid !== 'number') {
      throw new TypeError("Failed to execute 'observe' on 'MutationObserver': parameter 1 is not of type 'Node'.");
    }
    const o = options || {};
    // Chrome's dictionary coercion: the option-value keys imply their
    // family when the family's own key is ABSENT — attributeOldValue/
    // attributeFilter imply attributes (and records really are delivered
    // for {attributeOldValue:true} alone), characterDataOldValue implies
    // characterData. An explicitly-false family key kills the implication:
    // {attributes:false, attributeOldValue:true} throws (Chrome 155 probe).
    const effAttrs = o.attributes !== undefined
      ? !!o.attributes : (!!o.attributeOldValue || Array.isArray(o.attributeFilter));
    const effCd = o.characterData !== undefined ? !!o.characterData : !!o.characterDataOldValue;
    if (!o.childList && !effAttrs && !effCd) {
      throw new TypeError("Failed to execute 'observe' on 'MutationObserver': The options object must set at least one of 'attributes', 'characterData', or 'childList' to true.");
    }
    // Normalize once (the spec snapshots the dictionary at observe time):
    // the notify bridge reads the effective booleans, not the raw ones.
    const normalized = {
      childList: !!o.childList,
      attributes: effAttrs,
      characterData: effCd,
      subtree: !!o.subtree,
      attributeOldValue: !!o.attributeOldValue,
      characterDataOldValue: !!o.characterDataOldValue,
    };
    if (Array.isArray(o.attributeFilter)) normalized.attributeFilter = o.attributeFilter;
    // Re-observing the same target replaces the options (never stacks a
    // second entry — Chrome delivers one record per mutation per observer).
    this._targets = this._targets.filter(t => t.target !== target);
    this._targets.push({ target, options: normalized });
    if (globalThis.__mutationObservers.indexOf(this) < 0) globalThis.__mutationObservers.push(this);
  }
  disconnect() {
    this._targets = [];
    // Chrome empties the record queue on disconnect: a queued-but-undelivered
    // record is dropped (takeRecords returns [] after), and the flush loop
    // skips unregistered observers, so no late callback fires either.
    this._records = [];
    const idx = globalThis.__mutationObservers.indexOf(this);
    if (idx >= 0) globalThis.__mutationObservers.splice(idx, 1);
  }
  takeRecords() {
    const r = this._records.slice();
    this._records = [];
    return r;
  }
};
// All records queued in the same synchronous block deliver in ONE callback
// per observer, at one microtask checkpoint, in registration order (Chrome
// truth: 3 sync childList mutations → calls=1, records=3). Mutations during
// a callback re-arm the flush for the next checkpoint.
globalThis.__moFlush = function () {
  globalThis.__moFlushArmed = false;
  const list = globalThis.__mutationObservers.slice();
  for (const obs of list) {
    if (!obs._records.length) continue;
    const batch = obs._records.splice(0);
    try { obs._callback.call(obs, batch, obs); }
    catch (e) { globalThis.__diting_reportUncaughtError(e); }
  }
};
globalThis.__moFlushArmed = false;
// (#39) Chrome exposes WebKitMutationObserver as an alias of the same
// constructor (zone.js patches both names; it skips absent classes, but
// `typeof WebKitMutationObserver` must be 'function'). A data prop holding
// the shared function survives the V8 snapshot and shares its native mark.
globalThis.WebKitMutationObserver = globalThis.MutationObserver;
// prevNid/nextNid carry the record's sibling slots (post-insertion
// neighbours for additions, pre-removal neighbours for removals; null for
// replace-all records like innerHTML/textContent writes). oldValue arrives
// from the mutation site (previous attribute value / previous text) and is
// attached per observer only when that observer asked for it — Chrome
// leaves oldValue null for observers without attributeOldValue/
// characterDataOldValue.
globalThis.__notifyMutation = function(type, target_nid, addedNodes, removedNodes, attributeName, oldValue, prevNid, nextNid) {
  if (!globalThis.__mutationObservers.length) return;
  // Use `_wrap` (the canonical node-id → wrapper resolver) instead of a
  // direct cache poke. The previous code referenced `globalThis._cache`,
  // but `_cache` is a module-local Map — the lookup always returned
  // undefined, so the function silently bailed every time. Result: no
  // MutationObserver fired in obscura, ever, despite the call sites being
  // wired up at appendChild / setAttribute. _wrap also lazily creates a
  // wrapper for nodes that didn't have one yet (e.g. children parsed from
  // `set innerHTML`), which we need for record.target/added/removed.
  const target = _wrap(target_nid);
  if (!target) return;
  // Match observers FIRST and build the (allocating) records only if
  // someone wants this mutation — under attribute storms the overwhelming
  // majority of calls match nobody, and the record build (wrap + array
  // map/filter) was pure waste. Semantics unchanged: one record per
  // matching observer; observers whose options produce identical records
  // share one object (Chrome hands both observers the very same record
  // instance when their option sets agree — #272 probe).
  const matched = [];
  // Ancestor nids, collected at most once and shared by every subtree
  // root — one parentNode walk instead of one per observed root.
  let chain = -1;
  for (const obs of globalThis.__mutationObservers) {
    for (const t of obs._targets) {
      const root = t.target;
      if (!root) continue;
      // Filter by type per the observer options. Default behaviour matches
      // real MutationObserver: attribute mutations need options.attributes,
      // characterData mutations need options.characterData, childList
      // needs options.childList. #236: attributeFilter narrows the
      // attribute case — a filter of ['data-allowed'] must not deliver
      // 'data-other' records (spec: filter matches the attribute's
      // local name; ours are unqualified names, so a direct includes).
      const wantsType =
        (type === 'attributes' &&
          t.options.attributes &&
          (!Array.isArray(t.options.attributeFilter) ||
            t.options.attributeFilter.includes(attributeName))) ||
        (type === 'characterData' && t.options.characterData) ||
        (type === 'childList' && t.options.childList);
      if (!wantsType) continue;
      if (root._nid === target_nid) { matched.push([obs, t.options]); break; }
      if (t.options.subtree) {
        if (chain === -1) {
          chain = [];
          for (let cur = target.parentNode; cur; cur = cur.parentNode) chain.push(cur._nid);
        }
        let hit = false;
        for (let i = 0; i < chain.length; i++) {
          if (chain[i] === root._nid) { hit = true; break; }
        }
        if (hit) { matched.push([obs, t.options]); break; }
      }
    }
  }
  if (!matched.length) return;
  // One record object per distinct effective oldValue: observers with the
  // same gating share the instance (Chrome's identity sharing), observers
  // that gated differently get their own.
  const cache = new Map();
  for (const [obs, opts] of matched) {
    let eff = null;
    if (type === 'attributes' && opts.attributeOldValue) eff = oldValue ?? null;
    else if (type === 'characterData' && opts.characterDataOldValue) eff = oldValue ?? null;
    let record = cache.get(eff);
    if (!record) {
      record = __diting_mkMutationRecord({
        type: type, // 'childList', 'attributes', 'characterData'
        target: target,
        addedNodes: addedNodes ? __diting_mkNodeList(addedNodes.map(nid => _wrap(nid)).filter(Boolean)) : __diting_mkNodeList([]),
        removedNodes: removedNodes ? __diting_mkNodeList(removedNodes.map(nid => _wrap(nid)).filter(Boolean)) : __diting_mkNodeList([]),
        attributeName: attributeName || null,
        oldValue: eff,
        previousSibling: prevNid != null ? (_wrap(prevNid) || null) : null,
        nextSibling: nextNid != null ? (_wrap(nextNid) || null) : null,
      });
      cache.set(eff, record);
    }
    obs._records.push(record);
  }
  if (!globalThis.__moFlushArmed) {
    globalThis.__moFlushArmed = true;
    Promise.resolve().then(globalThis.__moFlush);
  }
};

// 刀58: marking migrated from bootstrap's native-mark list.
_markNative(globalThis.MutationObserver);

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
