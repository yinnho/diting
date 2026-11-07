# diting_selectors

CSS selector matching, vendored from the `selectors` crate 0.42.0 (The
Servo Project Developers, MPL-2.0) on 2026-11-07 and maintained here as
our own code — jumping straight past 0.41, absorbing the face migration
that was issue #38 (`parse_non_ts_pseudo_class` now takes `&str`, matching
entry points take the `Element` by value, `SelectorCaches` lives in
`selectors::context`). Not tracked against upstream; intake changes are
recorded in the repository git history and `crates/VENDORED.md`.

Intake pruning: tests, benches and inline test modules are gone;
`cssparser::` paths became `diting_cssparser::` throughout. Upstream ships no LICENSE file in the
tarball; the MPL-2.0 text was copied in from cssparser's. The matcher and
parser are unchanged. Product entry points: `diting::diting_dom::selector`
(DomElement) and `diting-browser`'s htmlq (ElementRef), both already on
the 0.42 by-value face.
