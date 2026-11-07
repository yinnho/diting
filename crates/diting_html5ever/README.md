# diting_html5ever

HTML parsing, vendored from the `html5ever` crate 0.40.1 (The html5ever
Project Developers, MIT OR Apache-2.0) on 2026-11-07 and maintained here as
our own code. Not tracked against upstream; intake changes are recorded in
the repository git history and `crates/VENDORED.md`.

Intake pruning: tests, benches, examples and inline `#[cfg(test)]` modules
are gone; the `markup5ever::` paths became `diting_markup5ever::`, and the
upstream `#![crate_name]`/`#![crate_type]` inner attributes (which pin the
canonical crate name) were dropped. The `serde` and `trace_tokenizer`
features are kept as upstream ships them. The parser itself (tokenizer,
tree builder, driver) is unchanged. Product entry points: the `TreeSink`
machinery behind `diting::diting_dom` and `diting-browser`'s htmlq, and
the serializer behind htmlq's outer/inner HTML.
