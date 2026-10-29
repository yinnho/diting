//! EventTarget as a native interface + the ONE dispatch algorithm
//! (DOM Standard sec 2.9 "dispatch"). Replaces the five-plus hand-rolled JS
//! dispatchers over time; this module is the algorithm, the host-specific
//! parts reach it through a hooks object (see [`Hooks`]).
//!
//! Listener storage: a V8 *private slot* on the target holding
//! `{ type -> [entry, ...] }`, entry = `[callback, capture, passive, once, removed]`.
//! Private slots work on ANY object (plain, global, legacy-JS-built nodes),
//! survive snapshots and are invisible to page reflection - proven by the
//! 2026-10-29 spike (SURFACE.md).
//!
//! Hooks (a JS object registered once at bake time through the one-shot
//! global `__diting_event_hooks_set`, kept in a private slot on the global):
//!   enter()/exit()        bracket around a target's listener loop (the
//!                          `_mtWindow` timer-deferral window)
//!   report(err)           report an uncaught listener exception
//!   parent(target, ev)    next target on the event path, or null
//!   isEvent(v)            is `v` an Event?
//!
//! Event access goes through the small `EventView` surface below; today an
//! Event is a JS object with data properties, after the Event migration only
//! this surface changes.

use v8::{FunctionCallbackArguments, FunctionCallbackInfo, FunctionTemplate, Local, PropertyAttribute, ReturnValue};

type Cb = unsafe extern "C" fn(*const FunctionCallbackInfo);

pub(crate) fn external_refs() -> Vec<v8::ExternalReference> {
    [ctor as Cb, add as Cb, remove as Cb, dispatch as Cb, when as Cb, when_resolve as Cb, when_exec as Cb, hooks_set as Cb, abort_remove as Cb]
        .into_iter()
        .map(|f| v8::ExternalReference { function: f })
        .collect()
}

// ---- small helpers -------------------------------------------------------

fn s<'a>(scope: &mut v8::PinScope<'a, '_, ()>, text: &str) -> Local<'a, v8::String> {
    v8::String::new(scope, text).unwrap()
}

fn private<'a>(scope: &mut v8::PinScope<'a, '_, ()>, name: &str) -> Local<'a, v8::Private> {
    let n = s(scope, name);
    v8::Private::for_api(scope, Some(n))
}

fn throw_type(scope: &mut v8::PinScope<'_, '_>, msg: &str) {
    let m = s(scope, msg);
    let e = v8::Exception::type_error(scope, m);
    scope.throw_exception(e);
}

fn throw_dom(scope: &mut v8::PinScope<'_, '_>, msg: &str, name: &str) {
    // The native DOMException constructor captured at install time (a page
    // overwriting the global cannot redirect it).
    let global = scope.get_current_context().global(scope);
    let key = private(scope, "diting#dom-exception");
    let ctor = global.get_private(scope, key).and_then(|v| Local::<v8::Function>::try_from(v).ok());
    if let Some(c) = ctor {
        let (m, n) = (s(scope, msg), s(scope, name));
        if let Some(e) = c.new_instance(scope, &[m.into(), n.into()]) {
            scope.throw_exception(e.into());
            return;
        }
    }
    throw_type(scope, msg);
}

fn hooks<'a>(scope: &mut v8::PinScope<'a, '_>) -> Option<Local<'a, v8::Object>> {
    let global = scope.get_current_context().global(scope);
    let key = private(scope, "diting#event-hooks");
    global.get_private(scope, key).and_then(|v| Local::<v8::Object>::try_from(v).ok())
}

fn hook_fn<'a>(scope: &mut v8::PinScope<'a, '_>, h: Local<'a, v8::Object>, name: &str) -> Option<Local<'a, v8::Function>> {
    let k = s(scope, name);
    h.get(scope, k.into()).and_then(|v| Local::<v8::Function>::try_from(v).ok())
}

/// `this` for the EventTarget methods: undefined/null -> the global (a bare
/// `addEventListener(...)` in a classic script); an object must have
/// EventTarget.prototype in its chain (so `{}` is an illegal invocation).
fn receiver<'a>(scope: &mut v8::PinScope<'a, '_>, this: Local<'a, v8::Value>) -> Option<Local<'a, v8::Object>> {
    if this.is_undefined() || this.is_null() {
        return Some(scope.get_current_context().global(scope));
    }
    let obj = Local::<v8::Object>::try_from(this).ok()?;
    let global = scope.get_current_context().global(scope);
    let key = private(scope, "diting#et-proto");
    let et_proto = global.get_private(scope, key)?;
    let mut cur = obj.get_prototype(scope);
    while let Some(p) = cur {
        if p.strict_equals(et_proto) {
            return Some(obj);
        }
        cur = Local::<v8::Object>::try_from(p).ok().and_then(|o| o.get_prototype(scope));
    }
    // The global object itself has window's prototype chain through a proxy;
    // accept it explicitly.
    if obj.strict_equals(global.into()) { return Some(obj); }
    None
}

// ---- listener store ------------------------------------------------------

const E_CB: u32 = 0;
const E_CAPTURE: u32 = 1;
const E_PASSIVE: u32 = 2;
const E_ONCE: u32 = 3;
const E_REMOVED: u32 = 4;

fn store<'a>(scope: &mut v8::PinScope<'a, '_>, target: Local<'a, v8::Object>, create: bool) -> Option<Local<'a, v8::Object>> {
    let key = private(scope, "diting#et-listeners");
    if let Some(o) = target.get_private(scope, key).and_then(|v| Local::<v8::Object>::try_from(v).ok()) {
        return Some(o);
    }
    if !create { return None; }
    let o = v8::Object::with_prototype_and_properties(scope, v8::null(scope).into(), &[], &[]);
    target.set_private(scope, key, o.into());
    Some(o)
}

fn list<'a>(scope: &mut v8::PinScope<'a, '_>, st: Local<'a, v8::Object>, ty: Local<'a, v8::String>, create: bool) -> Option<Local<'a, v8::Array>> {
    if let Some(a) = st.get(scope, ty.into()).and_then(|v| Local::<v8::Array>::try_from(v).ok()) {
        return Some(a);
    }
    if !create { return None; }
    let a = v8::Array::new(scope, 0);
    st.set(scope, ty.into(), a.into());
    Some(a)
}

fn flag<'a>(scope: &mut v8::PinScope<'a, '_>, entry: Local<'a, v8::Array>, i: u32) -> bool {
    entry.get_index(scope, i).map(|v| v.is_true()).unwrap_or(false)
}

/// Parsed addEventListener / removeEventListener options.
struct Opts<'a> { capture: bool, passive: bool, once: bool, signal: Option<Local<'a, v8::Value>> }

/// Web IDL: `(AddEventListenerOptions or boolean) options`. Dictionary members
/// are read in lexicographic order (capture, once, passive, signal), once each.
fn parse_options<'a>(scope: &mut v8::PinScope<'a, '_>, v: Local<'a, v8::Value>, prefix: &str, want_signal: bool) -> Option<Opts<'a>> {
    let mut o = Opts { capture: false, passive: false, once: false, signal: None };
    if v.is_undefined() || v.is_null() { return Some(o); }
    if let Ok(d) = Local::<v8::Object>::try_from(v) {
        let get = |scope: &mut v8::PinScope<'a, '_>, n: &str| { let k = s(scope, n); d.get(scope, k.into()) };
        o.capture = get(scope, "capture")?.boolean_value(scope);
        if want_signal {
            o.once = get(scope, "once")?.boolean_value(scope);
            o.passive = get(scope, "passive")?.boolean_value(scope);
            let sig = get(scope, "signal")?;
            if !sig.is_undefined() {
                if Local::<v8::Object>::try_from(sig).is_err() {
                    throw_type(scope, &format!("{prefix}: Failed to read the 'signal' property from 'AddEventListenerOptions': Failed to convert value to 'AbortSignal'."));
                    return None;
                }
                o.signal = Some(sig);
            }
        }
    } else {
        o.capture = v.boolean_value(scope);
    }
    Some(o)
}

fn same_cb<'a>(scope: &mut v8::PinScope<'a, '_>, e: Local<'a, v8::Array>, cb: Local<'a, v8::Value>, capture: bool) -> bool {
    let c = e.get_index(scope, E_CB);
    flag(scope, e, E_CAPTURE) == capture && c.map(|c| c.strict_equals(cb)).unwrap_or(false)
}

fn add_entry<'a>(scope: &mut v8::PinScope<'a, '_>, target: Local<'a, v8::Object>, ty: Local<'a, v8::String>, cb: Local<'a, v8::Value>, o: &Opts<'a>) -> Option<Local<'a, v8::Array>> {
    let st = store(scope, target, true)?;
    let l = list(scope, st, ty, true)?;
    for i in 0..l.length() {
        if let Some(e) = l.get_index(scope, i).and_then(|v| Local::<v8::Array>::try_from(v).ok()) {
            if same_cb(scope, e, cb, o.capture) { return None; }
        }
    }
    let b = |scope: &mut v8::PinScope<'a, '_>, x: bool| -> Local<'a, v8::Value> { v8::Boolean::new(scope, x).into() };
    let (cap, pas, onc, rem) = (b(scope, o.capture), b(scope, o.passive), b(scope, o.once), b(scope, false));
    let entry = v8::Array::new_with_elements(scope, &[cb, cap, pas, onc, rem]);
    let n = l.length();
    l.set_index(scope, n, entry.into());
    Some(entry)
}

fn remove_entry<'a>(scope: &mut v8::PinScope<'a, '_>, target: Local<'a, v8::Object>, ty: Local<'a, v8::String>, cb: Local<'a, v8::Value>, capture: bool) {
    let Some(st) = store(scope, target, false) else { return };
    let Some(l) = list(scope, st, ty, false) else { return };
    for i in 0..l.length() {
        let Some(e) = l.get_index(scope, i).and_then(|v| Local::<v8::Array>::try_from(v).ok()) else { continue };
        if same_cb(scope, e, cb, capture) {
            let t = v8::Boolean::new(scope, true);
            e.set_index(scope, E_REMOVED, t.into());
            // rebuild without the removed entry (in-flight dispatch iterates a snapshot)
            let keep = v8::Array::new(scope, 0);
            let mut n = 0;
            for j in 0..l.length() {
                if j == i { continue; }
                if let Some(x) = l.get_index(scope, j) { keep.set_index(scope, n, x); n += 1; }
            }
            st.set(scope, ty.into(), keep.into());
            return;
        }
    }
}

// ---- constructors / methods ---------------------------------------------

unsafe extern "C" fn ctor(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    if !args.is_construct_call() {
        throw_type(scope, "Failed to construct 'EventTarget': Please use the 'new' operator, this DOM object constructor cannot be called as a function.");
    }
}

unsafe extern "C" fn add(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let prefix = "Failed to execute 'addEventListener' on 'EventTarget'";
    let Some(target) = receiver(scope, args.this().into()) else { throw_type(scope, "Illegal invocation"); return };
    if args.length() < 2 {
        throw_type(scope, &format!("{prefix}: 2 arguments required, but only {} present.", args.length()));
        return;
    }
    if args.get(0).is_symbol() { throw_type(scope, &format!("{prefix}: Cannot convert a Symbol value to a string")); return; }
    let Some(ty) = args.get(0).to_string(scope) else { return };
    let cb = args.get(1);
    if cb.is_null() || cb.is_undefined() { return; } // EventListener? - inert registration
    if !(cb.is_object()) { throw_type(scope, &format!("{prefix}: parameter 2 is not of type 'Object'.")); return; }
    let Some(o) = parse_options(scope, args.get(2), prefix, true) else { return };
    // already-aborted signal: do not register
    if let Some(sig) = o.signal {
        if let Ok(so) = Local::<v8::Object>::try_from(sig) {
            let k = s(scope, "aborted");
            if so.get(scope, k.into()).map(|v| v.is_true()).unwrap_or(false) { return; }
        }
    }
    if add_entry(scope, target, ty, cb, &o).is_none() { return; }
    if let Some(sig) = o.signal {
        // remove on abort: a per-listener native function, registered through
        // the signal's own addEventListener (once).
        let data = v8::Array::new_with_elements(scope, &[target.into(), ty.into(), cb, v8::Boolean::new(scope, o.capture).into()]);
        let f = FunctionTemplate::builder_raw(abort_remove).data(data.into()).build(scope).get_function(scope);
        if let (Ok(so), Some(f)) = (Local::<v8::Object>::try_from(sig), f) {
            let k = s(scope, "addEventListener");
            if let Some(add_fn) = so.get(scope, k.into()).and_then(|v| Local::<v8::Function>::try_from(v).ok()) {
                let opts = v8::Object::new(scope);
                let ok = s(scope, "once");
                let t = v8::Boolean::new(scope, true);
                opts.set(scope, ok.into(), t.into());
                let ab = s(scope, "abort");
                add_fn.call(scope, so.into(), &[ab.into(), f.into(), opts.into()]);
            }
        }
    }
}

unsafe extern "C" fn abort_remove(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let Ok(d) = Local::<v8::Array>::try_from(args.data()) else { return };
    let (Some(t), Some(ty), Some(cb), Some(cap)) = (d.get_index(scope, 0), d.get_index(scope, 1), d.get_index(scope, 2), d.get_index(scope, 3)) else { return };
    if let (Ok(t), Ok(ty)) = (Local::<v8::Object>::try_from(t), Local::<v8::String>::try_from(ty)) {
        remove_entry(scope, t, ty, cb, cap.is_true());
    }
}

unsafe extern "C" fn remove(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let prefix = "Failed to execute 'removeEventListener' on 'EventTarget'";
    let Some(target) = receiver(scope, args.this().into()) else { throw_type(scope, "Illegal invocation"); return };
    if args.length() < 2 {
        throw_type(scope, &format!("{prefix}: 2 arguments required, but only {} present.", args.length()));
        return;
    }
    if args.get(0).is_symbol() { throw_type(scope, &format!("{prefix}: Cannot convert a Symbol value to a string")); return; }
    let Some(ty) = args.get(0).to_string(scope) else { return };
    let cb = args.get(1);
    if !(cb.is_object() || cb.is_null() || cb.is_undefined()) { throw_type(scope, &format!("{prefix}: parameter 2 is not of type 'Object'.")); return; }
    if cb.is_null() || cb.is_undefined() { return; }
    let Some(o) = parse_options(scope, args.get(2), prefix, false) else { return };
    remove_entry(scope, target, ty, cb, o.capture);
}

// ---- Event access (the only place that knows how an Event stores state) --

struct EventView<'a> { obj: Local<'a, v8::Object> }

impl<'a> EventView<'a> {
    fn get(&self, scope: &mut v8::PinScope<'a, '_>, name: &str) -> Local<'a, v8::Value> {
        let k = s(scope, name);
        self.obj.get(scope, k.into()).unwrap_or_else(|| v8::undefined(scope).into())
    }
    fn set(&self, scope: &mut v8::PinScope<'a, '_>, name: &str, v: Local<'a, v8::Value>) {
        let k = s(scope, name);
        // define (not set): the property may be a data property made
        // non-writable later, and must stay non-enumerable if internal.
        self.obj.set(scope, k.into(), v);
    }
    fn set_hidden(&self, scope: &mut v8::PinScope<'a, '_>, name: &str, v: Local<'a, v8::Value>) {
        let k = s(scope, name);
        self.obj.define_own_property(scope, k.into(), v, PropertyAttribute::DONT_ENUM);
    }
    fn truthy(&self, scope: &mut v8::PinScope<'a, '_>, name: &str) -> bool { self.get(scope, name).boolean_value(scope) }
    fn set_phase(&self, scope: &mut v8::PinScope<'a, '_>, phase: i32, current: Local<'a, v8::Value>) {
        let p = v8::Integer::new(scope, phase);
        self.set(scope, "eventPhase", p.into());
        self.set(scope, "currentTarget", current);
    }
}

fn report<'a>(scope: &mut v8::PinScope<'a, '_>, h: Option<Local<'a, v8::Object>>, err: Local<'a, v8::Value>) {
    if let Some(h) = h {
        if let Some(f) = hook_fn(scope, h, "report") {
            let u = v8::undefined(scope);
            f.call(scope, u.into(), &[err]);
        }
    }
}

/// Run the listeners of `target` registered for the event's type that match
/// `phase`. `capture_pass`: Some(true) = only capture listeners, Some(false)
/// = only non-capture, None never happens.
fn invoke<'a>(scope: &mut v8::PinScope<'a, '_>, h: Option<Local<'a, v8::Object>>, ev: &EventView<'a>, target: Local<'a, v8::Object>, ty: Local<'a, v8::String>, phase: i32, capture_pass: bool) {
    let Some(st) = store(scope, target, false) else { return };
    let Some(l) = list(scope, st, ty, false) else { return };
    // Spec: iterate a clone; listeners added during the walk do not run,
    // removed ones (flag) are skipped.
    let n = l.length();
    let snapshot: Vec<Local<'a, v8::Array>> = (0..n).filter_map(|i| l.get_index(scope, i).and_then(|v| Local::<v8::Array>::try_from(v).ok())).collect();
    ev.set_phase(scope, phase, target.into());
    if let Some(h) = h { if let Some(f) = hook_fn(scope, h, "enter") { let u = v8::undefined(scope); f.call(scope, u.into(), &[]); } }
    for e in snapshot {
        if flag(scope, e, E_REMOVED) || flag(scope, e, E_CAPTURE) != capture_pass { continue; }
        if flag(scope, e, E_ONCE) { remove_entry_by(scope, target, ty, e); }
        let passive = flag(scope, e, E_PASSIVE);
        if passive { let t = v8::Boolean::new(scope, true); ev.set_hidden(scope, "_inPassive", t.into()); }
        let Some(cb) = e.get_index(scope, E_CB) else { continue };
        v8::tc_scope!(let tc, scope);
        let recv: Local<v8::Value> = target.into();
        let evv: Local<v8::Value> = ev.obj.into();
        if let Ok(f) = Local::<v8::Function>::try_from(cb) {
            f.call(tc, recv, &[evv]);
        } else if let Ok(o) = Local::<v8::Object>::try_from(cb) {
            let k = s(tc, "handleEvent");
            match o.get(tc, k.into()).and_then(|v| Local::<v8::Function>::try_from(v).ok()) {
                Some(hf) => { hf.call(tc, o.into(), &[evv]); }
                None => {
                    let m = s(tc, "Failed to execute 'handleEvent' on 'EventListener': The provided callback is not callable.");
                    let ex = v8::Exception::type_error(tc, m);
                    tc.throw_exception(ex);
                }
            }
        }
        if tc.has_caught() && !tc.has_terminated() {
            if let Some(exc) = tc.exception() { report(tc, h, exc); }
            tc.reset();
        }
        if passive { let f = v8::Boolean::new(tc, false); ev.set_hidden(tc, "_inPassive", f.into()); }
        if ev.truthy(tc, "_immediatePropagationStopped") { break; }
    }
    // Event handler IDL attribute (`target.onfoo`): fires once per target,
    // in the bubble leg (after the listeners), via the host hook.
    if !capture_pass && !ev.truthy(scope, "_immediatePropagationStopped") {
        if let Some(hf) = h.and_then(|h| hook_fn(scope, h, "handler")) {
            let evv: Local<v8::Value> = ev.obj.into();
            if let Some(handler) = hf.call(scope, h.unwrap().into(), &[target.into(), evv]).and_then(|v| Local::<v8::Function>::try_from(v).ok()) {
                v8::tc_scope!(let tc, scope);
                handler.call(tc, target.into(), &[evv]);
                if tc.has_caught() && !tc.has_terminated() {
                    if let Some(exc) = tc.exception() { report(tc, h, exc); }
                    tc.reset();
                }
            }
        }
    }
    if let Some(h) = h { if let Some(f) = hook_fn(scope, h, "exit") { let u = v8::undefined(scope); f.call(scope, u.into(), &[]); } }
}

fn remove_entry_by<'a>(scope: &mut v8::PinScope<'a, '_>, target: Local<'a, v8::Object>, ty: Local<'a, v8::String>, entry: Local<'a, v8::Array>) {
    if let (Some(cb), cap) = (entry.get_index(scope, E_CB), flag(scope, entry, E_CAPTURE)) {
        remove_entry(scope, target, ty, cb, cap);
    }
}

unsafe extern "C" fn dispatch(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let prefix = "Failed to execute 'dispatchEvent' on 'EventTarget'";
    let Some(target) = receiver(scope, args.this().into()) else { throw_type(scope, "Illegal invocation"); return };
    if args.length() < 1 { throw_type(scope, &format!("{prefix}: 1 argument required, but only 0 present.")); return; }
    let h = hooks(scope);
    let evv = args.get(0);
    let is_event = match h.and_then(|h| hook_fn(scope, h, "isEvent")) {
        Some(f) => { let u = v8::undefined(scope); f.call(scope, u.into(), &[evv]).map(|r| r.is_true()).unwrap_or(false) }
        None => evv.is_object(),
    };
    let Ok(evo) = Local::<v8::Object>::try_from(evv) else { throw_type(scope, &format!("{prefix}: parameter 1 is not of type 'Event'.")); return };
    if !is_event { throw_type(scope, &format!("{prefix}: parameter 1 is not of type 'Event'.")); return; }
    let ev = EventView { obj: evo };
    let flag_key = private(scope, "diting#ev-dispatching");
    if evo.get_private(scope, flag_key).map(|v| v.is_true()).unwrap_or(false) {
        throw_dom(scope, &format!("{prefix}: The event is already being dispatched."), "InvalidStateError");
        return;
    }
    let tt = v8::Boolean::new(scope, true);
    evo.set_private(scope, flag_key, tt.into());

    let ty = match ev.get(scope, "type").to_string(scope) { Some(t) => t, None => { return } };
    ev.set(scope, "target", target.into());

    // event path: target first, then ancestors through the parent hook
    let mut path: Vec<Local<v8::Object>> = vec![target];
    if let Some(pf) = h.and_then(|h| hook_fn(scope, h, "parent")) {
        let mut cur = target;
        for _ in 0..4096 {
            let u = v8::undefined(scope);
            let next = pf.call(scope, u.into(), &[cur.into(), ev.obj.into()]);
            match next.and_then(|v| Local::<v8::Object>::try_from(v).ok()) { Some(n) => { path.push(n); cur = n; } None => break }
        }
    }

    // capture phase: root first
    for i in (1..path.len()).rev() {
        if ev.truthy(scope, "_propagationStopped") { break; }
        invoke(scope, h, &ev, path[i], ty, 1, true);
    }
    // target: capture listeners, then bubble listeners
    if !ev.truthy(scope, "_propagationStopped") {
        invoke(scope, h, &ev, path[0], ty, 2, true);
    }
    if !ev.truthy(scope, "_propagationStopped") {
        invoke(scope, h, &ev, path[0], ty, 2, false);
    }
    // bubble phase
    if ev.truthy(scope, "bubbles") {
        for i in 1..path.len() {
            if ev.truthy(scope, "_propagationStopped") { break; }
            invoke(scope, h, &ev, path[i], ty, 3, false);
        }
    }
    // dispatch epilogue
    let nul = v8::null(scope);
    ev.set_phase(scope, 0, nul.into());
    let f = v8::Boolean::new(scope, false);
    ev.set(scope, "_propagationStopped", f.into());
    ev.set(scope, "_immediatePropagationStopped", f.into());
    evo.set_private(scope, flag_key, f.into());
    let canceled = ev.truthy(scope, "defaultPrevented");
    ReturnValue::from_function_callback_info(info).set(v8::Boolean::new(scope, !canceled).into());
}

// ---- when(type) ----------------------------------------------------------

/// Promise executor: stash `resolve` into the holder array (callback data).
unsafe extern "C" fn when_exec(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    if let Ok(holder) = Local::<v8::Array>::try_from(args.data()) {
        holder.set_index(scope, 0, args.get(0));
    }
}

/// The one-shot listener: resolve the promise with the event.
unsafe extern "C" fn when_resolve(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let Ok(holder) = Local::<v8::Array>::try_from(args.data()) else { return };
    if let Some(res) = holder.get_index(scope, 0).and_then(|v| Local::<v8::Function>::try_from(v).ok()) {
        let u = v8::undefined(scope);
        res.call(scope, u.into(), &[args.get(0)]);
    }
}

unsafe extern "C" fn when(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let prefix = "Failed to execute 'when' on 'EventTarget'";
    let Some(target) = receiver(scope, args.this().into()) else { throw_type(scope, "Illegal invocation"); return };
    if args.length() < 1 { throw_type(scope, &format!("{prefix}: 1 argument required, but only 0 present.")); return; }
    if args.get(0).is_symbol() { throw_type(scope, &format!("{prefix}: Cannot convert a Symbol value to a string")); return; }
    let Some(ty) = args.get(0).to_string(scope) else { return };
    // The intrinsic Promise captured at install (immune to page overwrites).
    let global = scope.get_current_context().global(scope);
    let pkey = private(scope, "diting#promise");
    let Some(pc) = global.get_private(scope, pkey).and_then(|v| Local::<v8::Function>::try_from(v).ok()) else { return };
    let holder = v8::Array::new(scope, 1);
    let Some(exec) = FunctionTemplate::builder_raw(when_exec).data(holder.into()).build(scope).get_function(scope) else { return };
    let Some(promise) = pc.new_instance(scope, &[exec.into()]) else { return };
    let Some(f) = FunctionTemplate::builder_raw(when_resolve).data(holder.into()).build(scope).get_function(scope) else { return };
    let o = Opts { capture: false, passive: false, once: true, signal: None };
    add_entry(scope, target, ty, f.into(), &o);
    ReturnValue::from_function_callback_info(info).set(promise.into());
}

// ---- hooks registration (one-shot, JS side) -----------------------------

unsafe extern "C" fn hooks_set(info: *const FunctionCallbackInfo) {
    let info = unsafe { &*info };
    let parts = info.get_parts();
    let args = FunctionCallbackArguments::from_function_callback_info_parts(info, &parts);
    v8::callback_scope!(unsafe scope, info);
    let global = scope.get_current_context().global(scope);
    let key = private(scope, "diting#event-hooks");
    global.set_private(scope, key, args.get(0));
}

// ---- install -------------------------------------------------------------

pub(crate) fn install(scope: &mut v8::PinScope<'_, '_>, global: Local<v8::Object>) {
    let t = FunctionTemplate::builder_raw(ctor).length(0).build(scope);
    let name = s(scope, "EventTarget");
    t.set_class_name(name);
    t.read_only_prototype();
    let proto = t.prototype_template(scope);
    let tag = v8::Symbol::get_to_string_tag(scope);
    proto.set_with_attr(tag.into(), name.into(), PropertyAttribute::READ_ONLY | PropertyAttribute::DONT_ENUM);
    // Chrome's order: addEventListener, dispatchEvent, removeEventListener, when
    for (n, cb, len) in [("addEventListener", add as Cb, 2), ("dispatchEvent", dispatch as Cb, 1), ("removeEventListener", remove as Cb, 2), ("when", when as Cb, 1)] {
        let f = FunctionTemplate::builder_raw(cb).length(len).build(scope);
        let fname = s(scope, n);
        f.set_class_name(fname);
        proto.set_with_attr(fname.into(), f.into(), PropertyAttribute::NONE);
    }
    let ctor_fn = t.get_function(scope).expect("template instantiates");
    let pk = s(scope, "prototype");
    let proto_obj = ctor_fn.get(scope, pk.into()).expect("prototype");
    let key = private(scope, "diting#et-proto");
    global.set_private(scope, key, proto_obj);
    let k = s(scope, "EventTarget");
    global.define_own_property(scope, k.into(), ctor_fn.into(), PropertyAttribute::DONT_ENUM);
    // one-shot hook registration function (JS deletes it after use)
    let hs = v8::Function::new_raw(scope, hooks_set as Cb).unwrap();
    let hk = s(scope, "__diting_event_hooks_set");
    global.set(scope, hk.into(), hs.into());
}

/// Called by `install` after the other interfaces exist: remember the native
/// DOMException and intrinsic Promise constructors (InvalidStateError, when()).
pub(crate) fn capture_dom_exception(scope: &mut v8::PinScope<'_, '_>, global: Local<v8::Object>) {
    for (name, slot) in [("DOMException", "diting#dom-exception"), ("Promise", "diting#promise")] {
        let k = s(scope, name);
        if let Some(c) = global.get(scope, k.into()) {
            let key = private(scope, slot);
            global.set_private(scope, key, c);
        }
    }
}
