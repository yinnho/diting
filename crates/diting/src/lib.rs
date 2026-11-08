//! Diting (谛听) — the browser engine under Diting.
//!
//! One responsibility: load and run a page — network → DOM → CSS → layout →
//! paint, with a built-in V8 runtime — and expose it through the `Page`-level
//! API. No product concerns live here (ARCHITECTURE.md §2, rule R2).
//!
//! Facade since #58: the engine lives in the `diting_*` crates (env → dom →
//! css → layout; net; js; page) and this crate re-exports them under their
//! old module paths, so `diting::diting_dom::…` and friends keep working.

pub use diting_css;
pub use diting_dom;
pub use diting_env::{chrome_target, env_knobs};
pub use diting_js;
#[cfg(feature = "screenshot")]
pub use diting_layout;
pub use diting_net;
pub use diting_page as diting_browser;
// Bundled CJK font supply for /screenshot determinism (batch 3c) — lives
// inside diting_layout (fonts and layout reference each other).
#[cfg(feature = "screenshot")]
pub use diting_layout::diting_fonts;

/// Source revision the engine was built from (build.rs git rev-parse;
/// DITING_BUILD_COMMIT overrides for the .git-less 86quan tree).
/// Lives HERE because our build script's `rustc-env` only reaches this
/// crate's compilation — dependent crates' `option_env!` never see it
/// (the local /health "unknown" face). The .git/HEAD rerun pins keep it
/// fresh per commit.
pub const BUILD_COMMIT: &str = env!("DITING_BUILD_COMMIT");
