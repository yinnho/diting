//! Native (Rust-backed) Web IDL interfaces - the pilot of the Moli-direction
//! binding layer (docs/JS-PLATFORM-ARCH sec 12, MOLI-STUDY sec 5).
//!
//! The law each interface follows:
//!  * identity is a TAG in internal field 0 (a Smi), checked by every
//!    accessor - no prototype-chain or JS-visible property decides "is this
//!    a DOMRect", so page scripts can neither forge nor observe it;
//!  * state lives in further internal fields as plain JS numbers, so the
//!    snapshot serializer needs no embedder callbacks to carry them;
//!  * every native callback is a raw `extern "C"` function listed in
//!    [`external_refs`] - V8 stores snapshotted callbacks by INDEX into
//!    that table (append-only, order is load-bearing).
//!
//! PILOT status: DOMRectReadOnly / DOMRect only. The questions it answers
//! are recorded in docs (snapshot survival, runtime delta, brand checks).

pub(crate) mod dom_rect;

/// Native callbacks that must be registered as external references.
pub(crate) fn external_refs() -> Vec<v8::ExternalReference> {
    dom_rect::external_refs()
}

/// Install every native interface onto the context's global. Runs inside
/// `install_faces`, so the bake and a fresh VM see the identical object.
pub(crate) fn install(
    scope: &mut v8::PinScope<'_, '_>,
    global: v8::Local<v8::Object>,
) {
    dom_rect::install(scope, global);
}
