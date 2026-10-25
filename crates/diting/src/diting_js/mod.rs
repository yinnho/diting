pub mod module_loader;
pub mod runtime;
pub mod ops;
pub mod async_glue;
pub mod ws;

/// The snapshot's script sources as process-static UTF-16 buffers —
/// same generated file the build script baked with (#276 memory
/// campaign). Registered with diting_runtime before the first realm so
/// restore-side external strings resolve by index into OUR .rodata.
pub mod snapshot_sources {
    include!(concat!(env!("OUT_DIR"), "/snapshot_sources_ascii.rs"));
}
mod import_map;
pub mod markdown;
mod write_stream;

#[cfg_attr(not(test), allow(unused_imports))] // runtime tests are the sole consumer
pub use markdown::HTML_TO_MARKDOWN_JS;

/// The V8 version the kernel actually executes scripts on (#102) — the
/// engine truth behind the UA string a site sees. Safe to call before any
/// isolate exists (a static string in the binary).
pub fn v8_version() -> &'static str {
    v8::V8::get_version()
}
