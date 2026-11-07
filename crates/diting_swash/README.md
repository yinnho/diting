# diting_swash

Font introspection, complex text shaping and glyph rendering, vendored
from the `swash` crate 0.2.10 (Chad Brokaw / dfrg, Apache-2.0 OR MIT) on
2026-11-07 and maintained here as our own code. Not tracked against
upstream; intake changes are recorded in the repository git history and
`crates/VENDORED.md`.

Intake pruning: the `libm` feature (no_std float math via `core_maths`)
and its three guarded imports are gone — we always run std. Everything
else is untouched: the `shape` module (HarfBuzz-class shaping engine —
note it is NOT behind a feature; upstream compiles it always, and our
`diting_layout::text` shapes through it), `scale` (outlines, color
bitmaps via the built-in PNG decoder over `yazi` zlib, metrics, proxies)
and `render` (zeno rasterization). The dependency ring stays registry:
`skrifa` 0.44 (fontations parser; pinned to the line we ride),
`zeno` 0.3.3, `yazi` 0.2.1 — none used directly outside this crate.

Product entry points: `diting::diting_layout::text` (FontBook shaping,
fallback segmentation, glyph rasters for the screenshot/canvas paths)
and `diting_js/ops/canvas.rs` fillText. Known deep corner owned since
intake: dfrg/swash#139 (width-0 EBDT bitmap makes `chunks(0)` panic) is
absorbed by a glyph denylist in `diting_layout/text.rs`; with the crate
in-tree the source fix is now on our clock.
