pub mod context;
pub mod fork_virtual_url;
pub mod lifecycle;
pub mod page;
pub mod profiles;

pub use context::BrowserContext;
pub use diting_js::LayoutPolicy;
pub use page::Page;

/// Serializes this crate's tests that read or mutate
/// `DITING_ALLOW_PRIVATE_NETWORK` — process env is shared across parallel
/// test threads. Each engine crate's test binary is its own process since
/// #58, so each carries its own lock (diting_net's is test-only and
/// invisible here).
#[cfg(test)]
pub(crate) static PRIVATE_NET_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
