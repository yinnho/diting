# diting_cssparser

CSS tokenizer/parser, vendored from the `cssparser` crate 0.38.0 (Simon
Sapin and contributors, MPL-2.0) on 2026-11-07 and maintained here as our
own code. Not tracked against upstream; intake changes are recorded in the
repository git history and `crates/VENDORED.md`.

Intake pruning: `src/tests.rs` and `src/size_of_tests.rs` are gone; the
upstream `#![crate_name]`/`#![crate_type]` inner attributes were dropped.
The default features (`fast_match_byte`, `fast_match_color`) are kept
exactly as upstream ships them — this is a behavior-preserving intake, not
a tuning pass. The parser itself is unchanged; consumers rename only
(`cssparser::` → `diting_cssparser::`, driven by `diting_selectors` and
the house CSS layer).
