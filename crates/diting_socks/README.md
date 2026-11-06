# diting_socks

Vendored from [tokio-socks] 0.5.3 (MIT, `LICENSE` in this directory) and
pruned to what this repository uses — see the crate-root doc comment for
the exact keep/drop list. Changes from upstream are ours; this copy is not
tracked against upstream releases. See `crates/VENDORED.md`.

Vendored reason: upstream 0.5.3 is the latest release and pins thiserror 1
(we are all-in on thiserror 2); BIND/Tor/futures-io faces we never used
were dropped on intake.

[tokio-socks]: https://crates.io/crates/tokio-socks
