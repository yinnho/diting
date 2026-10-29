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

Source: the crates.io release tarballs of the versions above (https://github.com/0x676e67).
Apache-2.0 obligations (section 4): each crate keeps its `LICENSE`; the repository `NOTICE`
names the origin; changes are ours and are recorded in git history. Files that were modified
carry no per-file banner (the whole crate is "modified"); `README.md` of each crate says so.

## What was changed on intake (commit introducing them)
- Renamed: packages, library names and every in-source path (`wreq::` -> `diting_wire::` etc.).
- Dropped: tests, examples, benches, `Cargo.toml.orig`, upstream README/CHANGELOG, publishing metadata.
- Library tests and doctests are off (`test = false`, `doctest = false`): the behaviour we depend on is
  pinned by our own tests (`diting_net::wreq_client::tests`, wire audits under `tools/webplatform`).
- Pruned to what the product uses: optional features and their dependencies that we never enable were
  emptied/removed (compio runtime, hickory DNS, QUIC, websocket, cookies, json/form/query/multipart,
  system proxy, tracing, serde, ...). The dependency closure in `Cargo.lock` is exactly the old one
  (no package added, removed or re-versioned except the four crates swapped for ours).

## Still external (on purpose)
`btls` / `btls-sys` (Apache-2.0, BoringSSL bindings; we call its FFI from `diting_wire/src/tls/ffi.rs`,
the crate's only `unsafe`; we do not edit BoringSSL's C).

`diting_wire_h2` was taken in a second step because `http2`'s `ExperimentalSettings` only accepts setting ids
1..=15, which cannot express the HTTP/2 GREASE setting (id `0x?A?A`) that real Chrome 145 sends. It is the
MIT-licensed part of this stack: its `LICENSE` is kept and `NOTICE` carries the attribution. It is wired in under
the dependency alias `http2` (`package = "diting_wire_h2"`) so the vendored sources keep their `http2::` paths.

## Security
We own the HTTP/TLS client layer now. Watch: the upstream `0x676e67/wreq*` release notes and advisories,
`cargo audit` for `http2`/`btls`, BoringSSL updates through `btls-sys`.
