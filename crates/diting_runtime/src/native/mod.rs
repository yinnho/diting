//! Native (Rust-backed) Web IDL interfaces - the Moli-direction binding
//! layer (docs/JS-PLATFORM-ARCH sec 12, MOLI-STUDY sec 5).
//!
//! An interface is DATA: a static [`declare::Iface`] descriptor. A fixed set
//! of generic raw callbacks (constructor, getter, setter, toJSON, factory)
//! serves every descriptor, selecting it through the callback `data` word -
//! so adding an interface adds no external references and no hand-written
//! callbacks.
//!
//! The law every interface follows:
//!  * identity is a family TAG in internal field 0 (a Smi) checked by every
//!    accessor - no prototype-chain or JS-visible property decides "is this
//!    a DOMRect", so page scripts can neither forge nor observe it;
//!  * state lives in further internal fields as plain JS numbers, so the
//!    snapshot serializer needs no embedder callbacks to carry them;
//!  * every native callback is a raw `extern "C"` function listed in
//!    [`external_refs`] - V8 stores snapshotted callbacks by INDEX into that
//!    table (append-only, order is load-bearing).

pub(crate) mod declare;
pub(crate) mod geometry;

/// Native callbacks that must be registered as external references.
pub(crate) fn external_refs() -> Vec<v8::ExternalReference> {
    declare::external_refs()
}

/// Install every native interface onto the context's global. Runs inside
/// `install_faces`, so the bake and a fresh VM see the identical object.
pub(crate) fn install(scope: &mut v8::PinScope<'_, '_>, global: v8::Local<v8::Object>) {
    declare::install(scope, global, geometry::IFACES);
}
