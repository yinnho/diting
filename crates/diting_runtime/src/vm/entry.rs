//! V8 entry-point helpers for [`super::Vm`].
//!
//! The VM impl owns loop state and dispatch policy; these helpers own the
//! scoped script/eval/settle crossings into V8. Keeping the two apart also
//! keeps each file below the production-line ratchet without cutting a live
//! algorithm into artificial pieces.

use crate::modules::eval::ModuleEvalError;
use crate::modules::ModuleError;

use super::{LoopError, Triplet};

/// A module-machinery failure → the host's message-shaped error face.
/// Live exceptions string-ify (`String(v)` law, same as eval_string);
/// termination stays termination.
pub(super) fn module_error_to_eval(
    isolate: &mut v8::OwnedIsolate,
    context: &v8::Global<v8::Context>,
    error: ModuleError,
) -> ModuleEvalError {
    match error {
        ModuleError::Terminated => ModuleEvalError::Terminated,
        ModuleError::Loader(e) => ModuleEvalError::Threw(e.message().to_string()),
        ModuleError::Exception(exception) => {
            v8::scope_with_context!(cs, isolate, context);
            let exception = v8::Local::new(cs, exception);
            let message = exception
                .to_string(cs)
                .unwrap_or_else(|| v8::String::new(cs, "<exception>").unwrap());
            ModuleEvalError::Threw(message.to_rust_string_lossy(cs))
        }
    }
}

pub(super) fn eval_string_impl(
    isolate: &mut v8::OwnedIsolate,
    context: &v8::Global<v8::Context>,
    source: &str,
) -> Result<String, LoopError> {
    v8::scope_with_context!(cs, isolate, context);
    v8::tc_scope!(let tc, cs);
    let src = v8::String::new(tc, source).expect("script source fits");
    let value = match v8::Script::compile(tc, src, None).and_then(|script| script.run(tc)) {
        Some(v) => v,
        None if tc.has_terminated() => return Err(LoopError::Terminated),
        None => {
            let msg = match tc.message() {
                Some(m) => m.get(tc).to_rust_string_lossy(tc),
                None => "<no message>".into(),
            };
            return Err(LoopError::TickThrew(msg));
        }
    };
    // String(value) coercion through the global constructor.
    let key = v8::String::new(tc, "String").unwrap();
    let string_fn = tc
        .get_current_context()
        .global(tc)
        .get(tc, key.into())
        .and_then(|f| f.try_cast::<v8::Function>().ok())
        .expect("String global exists");
    let coerced = string_fn
        .call(tc, v8::undefined(tc).into(), &[value])
        .expect("String() of a primitive does not throw");
    let s = coerced
        .try_cast::<v8::String>()
        .expect("String() returns string");
    Ok(s.to_rust_string_lossy(tc))
}

/// [`Vm::execute_script`]'s body: same entry mapping as
/// `eval_string_impl`, but the value survives as a Global and the script
/// carries the caller's name in its origin.
pub(super) fn execute_script_impl(
    isolate: &mut v8::OwnedIsolate,
    context: &v8::Global<v8::Context>,
    name: &str,
    source: &str,
) -> Result<v8::Global<v8::Value>, LoopError> {
    v8::scope_with_context!(cs, isolate, context);
    v8::tc_scope!(let tc, cs);
    let src = v8::String::new(tc, source).expect("script source fits");
    let name_s = v8::String::new(tc, name).expect("script name fits");
    let origin = v8::ScriptOrigin::new(
        tc,
        name_s.into(),
        0,
        0,
        false,
        -1,
        None,
        false,
        false,
        false,
        None,
    );
    let value = match v8::Script::compile(tc, src, Some(&origin)).and_then(|s| s.run(tc)) {
        Some(v) => v,
        None if tc.has_terminated() => return Err(LoopError::Terminated),
        None => {
            let msg = match tc.message() {
                Some(m) => m.get(tc).to_rust_string_lossy(tc),
                None => "<no message>".into(),
            };
            return Err(LoopError::TickThrew(msg));
        }
    };
    Ok(v8::Global::new(tc, value))
}

/// Deliver one batch: per-triplet `__diting_settle(id, ok, payload)` calls.
/// A prefix may already have settled on a failed earlier attempt — the JS
/// side absorbs the re-delivery (Map miss) — so the whole batch re-feeds.
pub(super) fn deliver_batch(
    isolate: &mut v8::OwnedIsolate,
    context: &v8::Global<v8::Context>,
    settle_fn: &v8::Global<v8::Function>,
    batch: &[Triplet],
) -> Result<(), LoopError> {
    // What tripped the first failing call. None-variant = the termination
    // interrupt; Some = a JS exception with its message.
    let mut failure: Option<Option<String>> = None;
    {
        v8::scope_with_context!(cs, isolate, context);
        v8::tc_scope!(let tc, cs);
        let f = v8::Local::new(tc, settle_fn);
        for t in batch {
            let id = v8::Integer::new(tc, t.promise_id);
            let ok = v8::Boolean::new(tc, t.ok);
            let payload = v8::String::new(tc, &t.payload).expect("settle payload fits a V8 string");
            let recv: v8::Local<v8::Value> = v8::undefined(tc).into();
            let args: [v8::Local<v8::Value>; 3] = [id.into(), ok.into(), payload.into()];
            if f.call(tc, recv, &args).is_none() {
                // has_terminated() is the precise signal: the termination
                // interrupt has no message, and is_execution_terminating()
                // is only reliable while the exception is still unwinding
                // (both pinned by the tripwire test below).
                failure = Some(if tc.has_terminated() {
                    None
                } else {
                    Some(match tc.message() {
                        Some(m) => m.get(tc).to_rust_string_lossy(tc),
                        None => "<no message>".into(),
                    })
                });
                break;
            }
        }
    }
    match failure {
        None => Ok(()),
        Some(None) => Err(LoopError::Terminated),
        Some(Some(msg)) => Err(LoopError::TickThrew(msg)),
    }
}

/// The live handler under a rejection-face global, if the page registered
/// one (PRELUDE's registration face stores it there — baked into the blob
/// with the registering bootstrap, so restores need no rehydration).
pub(super) fn read_rejection_handler<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    name: &str,
) -> Option<v8::Global<v8::Function>> {
    let key = v8::String::new(scope, name)?;
    let global = scope.get_current_context().global(scope);
    global
        .get(scope, key.into())
        .and_then(|v| v.try_cast::<v8::Function>().ok())
        .map(|f| v8::Global::new(scope, f))
}
