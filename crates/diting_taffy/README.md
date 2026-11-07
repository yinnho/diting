# diting_taffy

Flexbox / CSS Grid / Block+Float layout, vendored from taffy upstream main
rev 802e497 (DioxusLabs/taffy, MIT) on 2026-11-07 and maintained here as our
own code. Not tracked against upstream; intake changes are recorded in the
repository git history and `crates/VENDORED.md`.

**Why this rev, not the 0.15.1 release:** 0.15.1 diverged from main
(ahead 22 / behind 20) and does not contain the float tail-segment fix
(a float landing inside the last existing segment but taller than its
remaining height never pushed a new segment) that #300's graduation battery
depends on — we rode it via `[patch.crates-io]` since then. The vendor
freezes that exact tree; absorbing the 0.15.1-side commits (float
inset/clamp, baseline consistency, flexbox perf passes) is an in-tree diff
batch gated by the layout parity battery, on our clock.

Intake pruning: tests, benches, examples, docs, test_fixtures, scripts and
the repo's tooling files are gone, inline `#[cfg(test)]` modules stripped
across 21 files, and the never-enabled feature faces went with their
optional deps (`serde`, `parse`/`parse_faster` — the latter pulled registry
`cssparser`, `document-features`). The algorithms themselves are unchanged.
Ring stays registry: `slotmap` 1.1 (taffy_tree), `smallvec` 1.16 (grid).

Product entry point: `diting::diting_layout` — box layout for the render
claim (block/flex/grid), the float engine probe (#299), and the fork-delta
tests in `diting_layout/fork_deltas.rs` (which lock our expectations, not a
second taffy build).
