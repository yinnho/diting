//! The only `unsafe` in this crate: calls into BoringSSL that `btls` has no safe wrapper for.
//!
//! Everything here takes a live `SslConnectorBuilder` (which owns its `SSL_CTX`) and exposes a
//! safe signature, so the rest of the crate keeps `#![deny(unsafe_code)]`.
#![allow(unsafe_code)]

use btls::{error::ErrorStack, ssl::SslConnectorBuilder};

/// Request trust anchors in the ClientHello (`trust_anchors` extension, 0xCA34).
///
/// `ids` is the wire-format list: a series of non-empty, 8-bit length-prefixed strings. An empty
/// slice still sends the extension, with an empty list (Chrome 145 does exactly that).
pub(crate) fn set_requested_trust_anchors(
    connector: &mut SslConnectorBuilder,
    ids: &[u8],
) -> Result<(), ErrorStack> {
    // SAFETY: `connector` owns a live SSL_CTX for the duration of the call, `ids` is a valid
    // readable slice of `ids.len()` bytes, and BoringSSL copies the list (it keeps no pointer).
    let ok = unsafe {
        btls_sys::SSL_CTX_set1_requested_trust_anchors(connector.as_ptr(), ids.as_ptr(), ids.len())
    };
    if ok == 1 { Ok(()) } else { Err(ErrorStack::get()) }
}
