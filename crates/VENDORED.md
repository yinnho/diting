# Vendored crates

Third-party source we took into this repository and now maintain as our own code.
They are NOT tracked against upstream: no `upstream <commit>` markers, no periodic re-sync.
Upstream security advisories are watched by hand (see "Security" below).

| Crate here | Taken from | Upstream version | License | Upstream author |
|---|---|---|---|---|
| `diting_wire` | `wreq` | 6.0.0-rc.31 | Apache-2.0 | 0x676e67 and contributors |
| `diting_wire_proto` | `wreq-proto` | 0.2.6 | Apache-2.0 | 0x676e67, Sean McArthur and contributors |
| `diting_wire_rt` | `wreq-rt` | 0.2.2-rc.4 | Apache-2.0 | 0x676e67 and contributors |
| `diting_profiles` | `wreq-util` | 3.0.0-rc.14 | Apache-2.0 | 0x676e67 and contributors |
| `diting_wire_h2` | `http2` | 0.5.20 | MIT (derived from the `h2` crate) | 0x676e67, Carl Lerche, Sean McArthur and contributors |
| `diting_btls_sys` | `btls-sys` | 0.5.6 | Apache-2.0 / BoringSSL (ISC-style) | 0x676e67, Alex Crichton, Steven Fackler, BoringSSL authors |
| `diting_socks` | `tokio-socks` | 0.5.3 | MIT | Yilin Chen (sticnarf) and contributors |
| `diting_png` | `png` | 0.18.1 | MIT OR Apache-2.0 | image-rs contributors (nokiatechxdct for `chunk.rs`) |
| `diting_html5ever` | `html5ever` | 0.40.1 | MIT OR Apache-2.0 | The html5ever Project Developers |
| `diting_markup5ever` | `markup5ever` | 0.40.0 | MIT OR Apache-2.0 | The html5ever Project Developers |
| `diting_cssparser` | `cssparser` | 0.38.0 | MPL-2.0 | Simon Sapin and contributors |
| `diting_selectors` | `selectors` | 0.42.0 | MPL-2.0 | The Servo Project Developers |
| `diting_swash` | `swash` | 0.2.10 | Apache-2.0 OR MIT | Chad Brokaw (dfrg) |
| `diting_taffy` | `taffy` (main rev `802e497` + #1358) | 0.15.0 | MIT | Visly Inc. and the Taffy Authors (DioxusLabs) |

Source: the crates.io release tarballs of the versions above (https://github.com/0x676e67).
Apache-2.0 obligations (section 4): each crate keeps its `LICENSE`; the repository `NOTICE`
names the origin; changes are ours and are recorded in git history. Files that were modified
carry no per-file banner (the whole crate is "modified"); `README.md` of each crate says so.

## What was changed on intake (commit introducing them)
- Renamed: packages, library names and every in-source path (`wreq::` -> `diting_wire::` etc.).
- Dropped: tests, examples, benches, `Cargo.toml.orig`, upstream README/CHANGELOG, publishing metadata.
- Library tests and doctests are off (`test = false`, `doctest = false`): the behaviour we depend on is
  pinned by our own tests (`diting_net::wreq_client::tests`, wire audits under `tools/webplatform`).
- 2026-11-06 tail sweep: the intake "Dropped: tests" line was only ever true for `tests/` dirs —
  inline `#[cfg(test)]` modules survived in `src/` with the dev-dependencies stripped, which
  never compiled under `cargo check --all-targets` (39 E0432s) and never ran under `cargo test`.
  All of it is now actually gone (≈6.5k lines across the family). One deliberate survivor:
  `diting_wire_proto/tests/grease_setting.rs` is OURS — it pins the HTTP/2 GREASE setting
  generator (Chrome 145 trait) and runs like any house test.
- Pruned to what the product uses: optional features and their dependencies that we never enable were
  emptied/removed (compio runtime, hickory DNS, QUIC, websocket, cookies, json/form/query/multipart,
  system proxy, tracing, serde, ...). The dependency closure in `Cargo.lock` is exactly the old one
  (no package added, removed or re-versioned except the four crates swapped for ours).

## Image stack (#42 ④, 2026-11-07)

The `image` crate left the tree; what replaced it splits two ways.

**`diting_png`** (full crate, renamed from `png` 0.18.1, crates.io tarball). Intake same
shape as the wire family: both LICENSEs kept, `test = false`/`doctest = false`, tests and
benches dropped including the inline `#[cfg(test)]` modules and their `test_utils` /
`benchable_apis` helpers (tail-sweep lesson applied at intake this time). Beyond that:
the `unstable` feature (portable_simd filter variants + `filter/simd.rs`) was pruned at
intake — the four dead `#[cfg(feature = "unstable")]` blocks and the cfg_attr went with
it — and `filter_paeth_stbi` is `#[cfg(target_arch = "x86_64")]` because only the x86_64
`unfilter` calls it (upstream silenced nothing; we build both faces clean). Product use:
decode with `Transformations::EXPAND` + gray/RGB→RGBA promotion (`diting_layout::image`),
8-bit RGBA/RGB encode (screenshots, authoring pages, PPTX assets). The decode oracle in
diting's dev-dependencies is the SAME vendored crate — an oracle that shipped its own
copy would prove nothing about what we ship.

**JPEG encoder** (partial, `crates/diting/src/diting_layout/image/jpeg_encode.rs`, from
`image` 0.25.10 `codecs/jpeg/{encoder,entropy,transform}.rs`, MIT OR Apache-2.0). Only
the baseline RGB8 face came over — the one `authoring/pages.rs`' PDF embedding used —
as `encode_jpeg_rgb(out, rgb, w, h, quality)`: tables K.1–K.6, the bit writer,
fixed-point RGB→YCbCr, and fdct (an IJG libjpeg 9a translation whose license block is
kept verbatim in the file header; the IJG acknowledgment lives in `NOTICE`). Pruned:
the generic `GenericImageView` machinery, gray path, ICC/EXIF/pixel-density surface,
`image`'s error plumbing. Output is byte-identical to what `image` 0.25.10's
`JpegEncoder` produced (fixed 1:1 JFIF, 4:4:4, libjpeg quality scaling). This is the
SECOND house JPEG encoder on purpose: `diting_js/ops/canvas.rs` has its own 4:2:0
encoder (canvas `toDataURL`, quality 0.0–1.0, spec-default 0.92) pinned against Chrome
— different subsampling, different quality semantics, different master.

**Decode legs now driven direct** (`diting_layout::image`): zune-jpeg (the decoder
`image` 0.25.10 wrapped; colorspace selection and non-strict mode mirrored exactly),
image-webp and gif as leaf codecs. Versions pin what image 0.25.10 wrapped at intake —
decode output is unchanged from the `image`-crate baseline. GIF first-frame composition
transparent-canvas semantics mirror image 0.25.10's (background color ignored).

## Servo parse family (#42 ②, 2026-11-07)

The four top crates of the Servo parse/match train are ours; `scraper` (and its
`getopts` rider) left the tree with them. The intake reason is the version wall,
not aesthetics: `selectors` releases on the Servo train and `scraper` pins the
generation it was built against, so every selectors bump was either blocked on
scraper or re-created a dual family (#38 was exactly this — 0.42's
`parse_non_ts_pseudo_class(&str)` + by-value `Element` matching sat behind
scraper's `^0.41`). With the top crates vendored, the whole train moves only
when we say so; #38's wall dies permanently.

**`diting_html5ever`** (0.40.1, MIT OR Apache-2.0), **`diting_markup5ever`**
(0.40.0, MIT OR Apache-2.0), **`diting_cssparser`** (0.38.0, MPL-2.0),
**`diting_selectors`** (0.42.0, MPL-2.0 — jumped straight past 0.41, absorbing
#38's face migration into the intake). Same intake shape as the wire family:
crates.io tarballs, both LICENSEs where dual (selectors ships no LICENSE file —
MPL-2.0 text copied in from cssparser), `test = false`/`doctest = false`,
tests/benches/examples/data/docs dropped, inline `#[cfg(test)]` modules stripped
at intake (tail-sweep lesson), package + lib + in-source paths renamed
(`markup5ever::` → `diting_markup5ever::` under html5ever, `cssparser::` →
`diting_cssparser::` under selectors), and the two upstream
`#![crate_name]`/`#![crate_type]` inner attributes dropped (they pin the
canonical crate name and now contradict ours).

Deliberately still registry — the small servo ring under the vendored tops:
`tendril` 0.5, `web_atoms` 0.3 (markup5ever's), `servo_arc` 0.6, `to_shmem` 0.7
(selectors 0.42's, optional feature — locked, never enabled), and
`cssparser-macros` 0.7 (`match_byte!` — a 150-line proc-macro whose generated
code is fully local, so it works unchanged against the renamed crate; upstream
default features `fast_match_byte`/`fast_match_color` kept as-is for a
behavior-preserving intake). Walls are dead because dependabot has no registry
`selectors`/`html5ever` to bump in our graph anymore; when we WANT an upstream
train update, the diff lands in these trees on our clock.

**`htmlq`** (`diting-browser/src/htmlq.rs`, ~700 lines, ours) replaces
`scraper` 0.29 for the ten call sites that used it (eight search engines,
`html_md`, `render`/`screenshot` collectors). It ports the faces scraper
actually gave us — `Html::parse_document` over an ego-tree + html5ever
`TreeSink`, `Selector`/`ElementRef` over `diting_selectors` 0.42 by-value
matching, text/attr/classes/ancestors navigation, outer/inner HTML via the
html5ever serializer — and prunes what nothing used (fragment parsing,
`root_element`, document-level `html()`, reversed select, error collection,
`:hover`-family parsing stays a parse error exactly like scraper's default).
Provenance comments in the module header; scraper's MIT OR Apache-2.0
attribution stays in `NOTICE`. `ego-tree` itself stays registry (generic tree
utility, like `flate2` under `diting_png`).

## Font stack (#42 ③ / #45, 2026-11-07)

`swash` 0.2.10 — the whole font line: shaping (`shape::ShapeContext`, the
HarfBuzz-class engine — NOT behind a feature, upstream compiles it always and
our `diting_layout::text` shapes through it), metrics/scale (`ScaleContext`,
`Scaler`, `Source::ColorBitmap` color-emoji strikes, the built-in PNG decoder
over `yazi` zlib, `proxy::MetricsProxy`), and glyph rasterization (`render`,
zeno outlines). Vendored as **`diting_swash`** for the CJK-fallback
differentiation face (#42 ③): the font line is where our moat lives, and we
were already paying rent in its deep corners — dfrg/swash#139 (width-0 EBDT
bitmap → `chunks(0)` panic) was absorbed by a glyph denylist in
`diting_layout/text.rs`; with the crate in-tree, fixing that at the source
instead is on our clock.

**First source modifications (#48/#49, 2026-11-07):**

- `strike.rs` `Bitmap::decode` gained upstream's one-line guard for
  zero-sized bitmaps (`w == 0 || h == 0 → decode fails`, callers fall
  through to the next source — the Png branch's `return false`
  precedent). Marked `upstream dfrg/swash#139` in-code; that fix is
  upstream's own suggested patch, verified by its reporter across
  SimSun's full BMP charmap. Our `render_guarded` panic guard RETIRED
  from this duty but stays deployed for the remaining debug-only
  arithmetic panic family (dfrg/swash#123-126, unfixed upstream).
- The arithmetic trio (#49): `parse.rs check_range` non-short-circuit
  `&` → `&&` (offset-past-end usize underflow; result provably
  unchanged in every case), `metrics.rs` descent negations →
  `wrapping_neg()` (i16::MIN descenders, three sites), `at.rs
  script_language_count` → `wrapping_add(1)` (langSysCount u16::MAX).
  All three verified with the issues' minimized fuzz fonts / byte-level
  unit tests: pre-fix panics reproduce at the exact reported
  file:line:col, post-fix clean. Debug-only family — release behavior
  unchanged by construction. The fourth sibling (dfrg/swash#124) lives
  in zeno (registry ring dep), not fixed here. Upstream PRs: swash#150
  (#139), and the overflow trio under the same outreach policy.

Intake: same shape as the other leaf vendors (png, socks) — tarball src, both
LICENSEs, `test = false`/`doctest = false` equivalents via no test targets
(upstream ships zero inline `#[cfg(test)]` — nothing to sweep), package + lib
renamed, our README replacing upstream's. Pruned: the `libm` feature
(no_std float math via `core_maths`) and its three guarded imports — we always
run std. Everything else is untouched; the feature face (std/scale/render and
the optional-dep structure) matches upstream exactly. No `#![crate_name]`
attrs existed. Upstream took `skrifa ">= 0.31.1, <= 0.44"`; we pin `0.44`
(the line in the lock) — the fontations train moves when we move it.

Ring stays registry, none used directly outside this crate: `skrifa` 0.44
(+`read-fonts`/`font-types`, the Google fontations family), `zeno` 0.3.3
(path rasterizer), `yazi` 0.2.1 (zlib decode for the embedded PNG bitmaps).
Reverse dependency was single-hop (`swash ← diting ← diting-browser`), so the
manifest flip is one line plus the `dep:` feature key.

Not in this batch: the CJK fallback work itself (exotic-script fallback,
86quan emoji font absence — the font-fallback gap notes) — vendoring buys the
right to fix at the source, the fixes are their own batches.

## Layout core (#42 ① / #46, 2026-11-07)

The queue's endgame: taffy — flexbox, CSS grid, block+float layout — vendored
as **`diting_taffy`**, retiring the last big differentiating registry dep and
with it the `[patch.crates-io]` git pin we had carried since #300. #42's
queue is now fully in-tree (parse family, font stack, image stack, layout
core).

**Pinned tree, not a release.** We rode upstream main rev `802e497` via the
patch pin because it carries the float tail-segment fix (#300: a float
landing inside the last segment but taller than its remaining extent never
pushed the new segment) — and the later 0.15.1 release DIVERGED from main
(ahead 22 / behind 20) without that fix. The vendor freezes the exact tree
the battery was green on. Version stays 0.15.0 (the release this rev tracks).

The 0.15.1-side ledger closed the same day (#47): `git cherry` + per-commit
diffing proved 14 of the 22 "ahead" commits are verbatim cherry-picks of
main commits already in our pinned tree (the release branch cherry-picked
FROM main, not the other way), 7 are benchmarks/changelog/version bumps, and
exactly one real fix existed — #1358 (a hidden subtree's out-of-flow boxes
never got their rounded layout reset, keeping the previous visible pass's
`final_layout`, because `round_layout` only reaches OOF boxes through their
containing block's hoisted child list). Those 4 lines are absorbed in-tree;
with them there is nothing left in 0.15.1 we lack. The divergence's real
substance is main being AHEAD: 20 commits beyond our pin at vendor time
(incl. the #300 fix), and another 20 more upstream by the ledger close —
the block/BFC campaign #1387–#1401 is the next absorption frontier.

Intake: cargo's git checkout tree (src + LICENSE only; the repo's
tests/benches/examples/docs/test_fixtures dropped), package + lib renamed,
~3.6k lines of inline `#[cfg(test)]` modules and test-only items stripped —
the strip runs brace-aware (strings/comments/char literals) because a naive
brace count over-consumed past module ends on the first attempt and ate
production traits behind them. Never-enabled feature faces went with their
optional deps (`serde`, `parse`/`parse_faster` — the latter needed registry
`cssparser`; `document-features`); their names stay declared as empty shells
so the ~138 in-source cfg gates resolve as known (code never compiles — do
not enable). `strict_provenance`/`debug`/`profile` kept as upstream ships
(pure knobs). Ring stays registry: `slotmap` 1.1 (taffy_tree), `smallvec`
1.16 (grid).

Flip: one dep line in `crates/diting` (`dep:taffy` → `dep:diting_taffy`,
path dep, same 10-feature explicit set = upstream default), 250 `taffy::`
path renames across 13 `diting_layout` files (`fork_deltas.rs` is OUR test
file locking fork expectations, not a second taffy build), and the root
`[patch.crates-io]` taffy entry deleted. Gate faces: layout battery 273/273,
engine screenshot face, product stealth,screenshot face, and
`cargo tree -i taffy` empty.

## Still external (on purpose)
`btls` (the safe Rust wrapper; we call its API from `diting_wire`).

`diting_btls_sys` (btls-sys 0.5.6) was taken in 2026-11-06 (#25) for ONE reason: the bundled
BoringSSL snapshot predates the ML-DSA signature algorithms (0x0904/0x0905/0x0906) that Chrome
156+ leads its ClientHello `signature_algorithms` with, and rejects them at set time. The crate
keeps its upstream name and version and is wired in via `[patch.crates-io]`, so `btls` 0.5.6
from crates.io links against our copy unchanged. Unlike the renamed crates above it is a
minimal-divergence fork: upstream files are byte-identical except for two additions —
`patches/diting-mldsa-sigalgs.patch` (31 lines of C: three table entries + canonical-GREASE
passthrough in `set_sigalg_prefs`; the ClientHello serializer and verify paths needed nothing)
and its invocation in `build/main.rs`. The copy itself never retires: it puts BoringSSL snapshot
upgrades on OUR clock — persona bumps follow Chrome stable, and when the next bump needs a wire
feature the upstream snapshot lacks, this is where we patch forward instead of waiting (same
reason the wreq family is vendored). When upstream learns the ML-DSA values, only the 31-line
delta is dropped; the `[patch.crates-io]` entry and the vendored tree stay. Building with `BORING_BSSL_SOURCE_PATH`/`BORING_BSSL_ASSUME_PATCHED` bypasses all crate
patches, ours included — `diting_wire` degrades the sigalg lead then (conn.rs warns).

`diting_wire_h2` was taken in a second step because `http2`'s `ExperimentalSettings` only accepts setting ids
1..=15, which cannot express the HTTP/2 GREASE setting (id `0x?A?A`) that real Chrome 145 sends. It is the
MIT-licensed part of this stack: its `LICENSE` is kept and `NOTICE` carries the attribution. It is wired in under
the dependency alias `http2` (`package = "diting_wire_h2"`) so the vendored sources keep their `http2::` paths.

## Security
We own the HTTP/TLS client layer now. Watch: the upstream `0x676e67/wreq*` release notes and advisories,
`cargo audit` for `http2`/`btls`, and upstream `btls-sys` releases — bundle a newer BoringSSL snapshot
into the vendored copy on our schedule; drop our mldsa delta when the snapshot learns it.
