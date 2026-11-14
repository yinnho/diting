//! Emulation for different browsers.

pub mod chrome;
pub mod firefox;
pub mod okhttp;
pub mod opera;
pub mod safari;

use diting_wire::{Group, header::HeaderMap, http2::Http2Options, tls::TlsOptions};

pub(crate) fn build_standard_emulation(
    group: &'static str,
    tls_options: TlsOptions,
    http2_options: Option<Http2Options>,
    default_headers: Option<HeaderMap>,
) -> diting_wire::Emulation {
    let mut builder = diting_wire::Emulation::builder().tls_options(tls_options);

    if let Some(http2_options) = http2_options {
        builder = builder.http2_options(http2_options);
    }

    if let Some(headers) = default_headers {
        builder = builder.headers(headers);
    }

    builder.build(Group::new(group))
}
