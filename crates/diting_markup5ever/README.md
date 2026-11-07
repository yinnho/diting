# diting_markup5ever

Support crate for the HTML parser — qualified names, namespace atoms and
the serializer interface — vendored from the `markup5ever` crate 0.40.0
(The html5ever Project Developers, MIT OR Apache-2.0) on 2026-11-07 and
maintained here as our own code. Not tracked against upstream; intake
changes are recorded in the repository git history and
`crates/VENDORED.md`. (Upstream ships no README of its own.)

Intake pruning: tests and inline `#[cfg(test)]` modules are gone; `tendril`
and `web_atoms` stay registry dependencies as upstream has them. The crate
body is unchanged. Consumed by `diting_html5ever` (renamed paths) and via
re-export by everything that touches `QualName` or the tree builder's
`QuirksMode`.
