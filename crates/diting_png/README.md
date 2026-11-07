# diting_png

PNG decode/encode, vendored from the `png` crate 0.18.1 (image-rs) on
2026-11-07 and maintained here as our own code. Not tracked against
upstream; intake changes are recorded in the repository git history and
`crates/VENDORED.md`.

Intake pruning: tests, benches, examples, `Cargo.toml.orig`, the
`test_utils`/`benchable_apis` dev modules and the `benchmarks`/`unstable`
feature gates (with `filter/simd.rs` and its four call blocks) are gone;
`filter_paeth_stbi` is `#[cfg(target_arch = "x86_64")]` (only the x86_64
`unfilter` calls it, and we build both faces warning-clean). The codec
itself (decoder/encoder/filter/adam7) is unchanged. Product entry points:
`Decoder` with `Transformations::EXPAND` (decode to expanded 8-bit rows)
and `Encoder` 8-bit RGBA/RGB write — the screenshot and canvas PNG paths.
