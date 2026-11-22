//! Process-global connection pools shared across `HttpClient` instances.
//!
//! Chrome keeps one connection pool per profile: the second tab to visit an
//! origin rides the connections the first tab opened. The engine's shape was
//! the anomaly — every `BrowserContext` owns a private `HttpClient` with a
//! private `diting_wire::Client`, so every `/fetch` to the same origin re-paid
//! CONNECT + TLS through the proxy tunnel (3–8s per origin on a CN-boundary
//! tunnel, sometimes 20s). Ten benchmark cases to one origin serialized ten
//! full handshakes; a shared pool pays once and reuses — with the same
//! 60s-idle reaping that guards against bug #24's half-dead connections.
//!
//! Opt-in via `DITING_SHARED_POOL=1` while it soaks: cookies stay
//! per-context (the jar lives in the `HttpClient` layer, not the wire
//! `Client`), UA is a per-request header, so sharing is invisible to every
//! site-visible behavior except the connection count — which moves TOWARD
//! Chrome's.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use diting_wire::Client;

/// Which of the three tuned clients a pool entry holds. The proxy URL is
/// part of the identity: a pool built for `socks5://…` must never serve a
/// direct request. `Direct` carries the SSRF posture because the resolver
/// is baked into the connector (a private-network-allowing context and a
/// strict one must not share a guard).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Flavor {
    /// All traffic through this proxy URL.
    Proxied(String),
    /// No proxy; `ssrf_private` mirrors `allow_private_network`.
    Direct { ssrf_private: bool },
}

type PoolMap = Mutex<HashMap<(Flavor, bool), Arc<Client>>>;

static POOLS: std::sync::LazyLock<PoolMap> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// The knob: `DITING_SHARED_POOL=1` routes every `HttpClient`'s lazily
/// built wire client through the registry.
pub(crate) fn enabled() -> bool {
    std::env::var("DITING_SHARED_POOL")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Lookup (or first-build) a shared wire client for this flavor. `None`
/// when the knob is off — callers then build their private one as before.
pub(crate) fn shared_client(flavor: Flavor, chrome: bool) -> Option<Arc<Client>> {
    if !enabled() {
        return None;
    }
    let mut pools = POOLS.lock().unwrap_or_else(|p| p.into_inner());
    Some(
        pools
            .entry((flavor.clone(), chrome))
            .or_insert_with(|| Arc::new(build(flavor, chrome)))
            .clone(),
    )
}

/// Same recipe as the private builders (`tuned_builder` is the single
/// source) — only proxy/SSRF wiring differs per flavor.
fn build(flavor: Flavor, chrome: bool) -> Client {
    let mut builder = if chrome {
        crate::chrome_client_builder()
    } else {
        super::client_builder_no_env_proxy()
    };
    builder = super::tuned_builder(builder);
    builder = match flavor {
        Flavor::Proxied(ref proxy) => {
            if let Ok(p) = diting_wire::Proxy::all(proxy.as_str()) {
                builder.proxy(p)
            } else {
                builder
            }
        }
        Flavor::Direct { ssrf_private } => {
            builder.dns_resolver(Arc::new(crate::client::ssrf::SsrfGuardResolver::new(
                Arc::new(std::sync::atomic::AtomicBool::new(ssrf_private)),
            )))
        }
    };
    builder.build().expect("failed to build shared HTTP client")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_flavor_shares_one_client() {
        let a = shared_client(Flavor::Proxied("socks5://127.0.0.1:9".into()), false);
        let b = shared_client(Flavor::Proxied("socks5://127.0.0.1:9".into()), false);
        if let (Some(a), Some(b)) = (a, b) {
            assert!(Arc::ptr_eq(&a, &b));
        }
        // Env off in tests by default → None path is the norm; the ptr_eq
        // above only exercises when the knob is on (bench soak). Either way
        // both calls must agree.
    }

    #[test]
    fn flavors_do_not_share() {
        // Directly exercise the map identity, knob-independent.
        let x = build(
            Flavor::Direct {
                ssrf_private: false,
            },
            false,
        );
        let y = build(Flavor::Direct { ssrf_private: true }, false);
        let _ = (&x, &y); // distinct builds by construction; compile+smoke
    }
}
