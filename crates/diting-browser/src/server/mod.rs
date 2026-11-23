//! Acquire façade: browser factory, stateless handlers, search orchestration.
//!
//! Split by **job** (not line count):
//! - [`browser`] — jar / factory / cookie normalize / V8 host thread
//! - [`fetch`] — do_fetch / do_click / do_eval, plus its helpers by job:
//!   [`challenge`] ride-out, [`fetch_text`] rendered-text read,
//!   [`fetch_documents`] PDF/DOCX bytes → text, [`fetch_facts`] response
//!   facts, [`fetch_splice`] markdown splice, [`route_fallback`] route seams
//! - [`outputs`] — do_screenshot / do_video / do_pdf (`screenshot` feature)
//! - [`search`] — do_search + cache + engine health
//!
//! Callers keep using `crate::server::…`; this module re-exports the public API.

mod browser;
/// Challenge ride-out for the acquire paths: Cloudflare Turnstile, ByteDance
/// WAF PoW, Anubis PoW — shape detection and pumping, no site policy.
mod challenge;
/// Salvage DOCX → text (benchmark document column): zip local-file-header
/// walk + `<w:t>` run extraction. DOCX family of the pdf_text salvage.
mod docx_text;
mod fetch;
/// Binary and embedded documents for [`fetch`] (PDF / DOCX): byte-fetch plus
/// salvage extraction, and the embedded-document probe.
mod fetch_documents;
/// Response facts for [`fetch`]: media manifest, layout geometry,
/// resource rows.
mod fetch_facts;
mod fetch_splice;
/// Rendered-text read for [`fetch`]: empty-text grace, SPA-shell probes, the
/// innerText read.
mod fetch_text;
#[cfg(feature = "screenshot")]
mod outputs;
/// Salvage PDF → text (benchmark document column): linear object scan +
/// ObjStm expansion + ToUnicode replay of text operators.
mod pdf_text;
/// Route-decision and proxy-fallback seams for [`fetch`]: nav deadlines
/// (full ceiling + starvation bail), route chooser, route-table
/// bookkeeping, shared proxy retry leg.
mod route_fallback;
mod search;

// Façade re-exports — names exist for other modules / future callers even when
// this file itself does not mention them again.
#[allow(unused_imports)]
pub use browser::{
    build_browser, build_browser_for_account, build_browser_with_jar, persist_shared_cookies,
};
pub(crate) use browser::{
    cookie_list_from_json, inject_cookies, normalize_cookie_entry, run_on_local_runtime,
    v8_stack_size,
};
// cookie_store_path: diting doctor reports the jar's location (#65).
pub(crate) use browser::cookie_store_path;
#[cfg(test)]
pub(crate) use browser::shared_cookie_jar_for_tests;

pub(crate) use challenge::maybe_bypass_challenge;
pub use fetch::{do_click, do_eval, do_fetch};
#[cfg(feature = "screenshot")]
#[allow(unused_imports)]
pub(crate) use fetch_facts::resource_facts;
#[cfg(test)]
pub(crate) use fetch_facts::{media_from_eval, parse_layout_facts};
#[cfg(feature = "screenshot")]
pub use outputs::do_screenshot;
#[cfg(all(feature = "screenshot", feature = "authoring"))]
pub use outputs::{do_pdf, do_video};

#[cfg(test)]
pub(crate) use search::do_search_with_registry;
pub(crate) use search::search_engine_health;
pub use search::{do_search, SearchError};

/// Shared test plumbing for handler-level tests that need a real local
/// origin: an env guard that opens the private-network door under the same
/// process-wide lock diting_net uses, and a request-recording HTTP server.
#[cfg(test)]
pub(crate) mod test_util;

#[cfg(test)]
mod tests;
