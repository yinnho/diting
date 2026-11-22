//! Diting engine leaf crate (#58): the `DITING_*` runtime knobs and the
//! claimed Chrome target. Every upper engine crate reads both; nothing here
//! depends on the rest of the engine.

pub mod chrome_target;
// Engine runtime knobs (DITING_* env), owned by the engine since the
// workspace split; the product re-exports them from `config`.
pub mod env_knobs;
// Per-host route memory (direct-fail sentences + proxy-win marks), shared by
// every transport and persisted under app_data_dir.
pub mod route_table;
