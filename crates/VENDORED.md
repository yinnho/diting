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
