# Gate contract (#64)

Every Rust/Cargo change passes all four local gates before commit. CI runs
the same jobs — a red `CI` check on main is a breakage, not noise (the
`Plugin Security Scan` check that also appears next to commits is a separate
scan job, not CI; don't read it as green).

```sh
./scripts/gate_fmt.sh             # fmt over house crates; vendored trees keep upstream shape
python3 scripts/audit_layers.py   # layering R1-R3 + god-file ratchet (grandfathered files only shrink)
tools/webplatform/check_fresh.sh  # generated JS must equal what the checked-in generators emit
cargo test --workspace --features screenshot   # if the rusty_v8 fetch stalls, point RUSTY_V8_ARCHIVE at a local copy
```

- Format changes go in their own commit, never mixed into a feature batch.
- A god-file ratchet failure is fixed by splitting code out, never by
  raising the recorded count.
- Push target is the `diting` remote (`origin` is the retired repo).
- Clippy runs at `-D warnings` over the whole workspace (stealth,screenshot
  face, all targets): a new warning fails CI, fix it or scope an `#[allow]`
  with a reason at the site. Vendored crates carry `#![allow(clippy::all)]`
  at their root instead — upstream byte-shape wins (#64).
