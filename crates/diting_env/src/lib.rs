//! Diting engine leaf crate (#58): the `DITING_*` runtime knobs and the
//! claimed Chrome target. Every upper engine crate reads both; nothing here
//! depends on the rest of the engine.

pub mod chrome_target;
// Engine runtime knobs (DITING_* env), owned by the engine since the
// workspace split; the product re-exports them from `config`.
pub mod env_knobs;
