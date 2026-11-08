use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use diting_wire::header::{HeaderMap, HeaderName, HeaderValue, USER_AGENT};
use diting_wire::redirect::Policy;
use diting_wire::{Client, Method};
use tokio::sync::RwLock;
use url::Url;

use crate::cookies::CookieJar;

pub(crate) mod metadata;
mod observer;
pub mod policy;
mod ssrf;

pub use metadata::ResourceType;
use metadata::{metadata_for, FetchMetadata};
pub use observer::{CallbackRegistry, RequestCallback, RequestInfo, ResponseCallback};

use policy::custom_cert_store_requested;
pub use policy::{
    allow_file_access, env_allows_private_network, parse_scoped_cidrs, set_allow_file_access,
    set_allow_network, set_allow_private_network, validate_url, ScopedCidr,
};

pub use ssrf::{forbidden_after_dns, is_forbidden_base, is_forbidden_ip, SsrfGuardResolver};

/// One hop a redirected request followed: the Location target and the 3xx
/// status that sent it there. Network rows keep the REQUESTED url in
/// `url` (#203: a taobao submit bounced to error.taobao.com used to read
/// as "never issued" because the failure row carried only the final
/// target), so the hop list is the only place the wander is visible.
#[derive(Debug, Clone)]
pub struct RedirectHop {
    pub url: String,
    pub status: u16,
}

/// A script-initiated fetch()/XHR currently flying (one entry per redirect
/// hop, mirroring the `in_flight` counter). Exists so a caller whose eval
/// timed out can ask "is it still executing?" instead of re-issuing the
/// request — a hung scripted request used to be invisible until its 30s
/// read timeout (tmall publish report, #116). `age_ms` is computed by the
/// reader against `dispatched_at_ms`.
#[derive(Debug, Clone)]
pub struct InFlightScripted {
    pub url: String,
    pub method: String,
    pub dispatched_at_ms: u64,
}

pub fn epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Drop-decrement twin of the manual `in_flight` fetch_add (#309): the
/// add/sub pairs leaked a count whenever the enclosing future was
/// cancelled between them — a realm swap (init_js drops the old Vm)
/// mid-send kills a page's fetch() op future, none of the exit paths run
/// the sub, and the session's settle fast paths (both require
/// `active_requests() == 0`) stay dead for its remaining life. In the
/// 940-page crawl the leak climbed monotonically per session and every
/// settle window burned its full budget. `drop(guard)` at the old sub
/// sites keeps the counted window identical; cancellation lands on Drop
/// instead of on nothing. `None` (a runtime with no owning HttpClient)
/// is a no-op, matching the old `if let Some(counter)` subs.
pub struct InFlightDecrement(pub Option<Arc<std::sync::atomic::AtomicU32>>);

impl Drop for InFlightDecrement {
    fn drop(&mut self) {
        if let Some(counter) = self.0.take() {
            counter.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

/// A client builder with the implicit system/env proxy matcher turned off.
/// Every HTTP client the engine builds goes through here — the plain
/// clients below and the stealth ones in `wreq_client`.
///
/// The transport would otherwise read `HTTP_PROXY` / `HTTPS_PROXY` /
/// `ALL_PROXY` from the environment, which silently routes engine traffic
/// through a proxy the operator never configured in the engine —
/// `use_proxy:false` fetches, search's direct-first tier, robots checks,
/// downloads, all of it. When that env proxy dies, every public fetch fails
/// with an error that never mentions a proxy (obscura#491). The engine's
/// proxy decision is explicit instead: `DITING_PROXY` / the context
/// `proxy_url`, attached with `.proxy()` at the call sites — and
/// `.no_proxy()` must come BEFORE that attach, since it also clears any
/// already-pushed proxy.
///
/// SSL_CERT_FILE / SSL_CERT_DIR (opt-in only) load a cert store from those
/// paths, so hosts behind a private/national CA verify; unset (or a failed
/// build) keeps the bundled default roots.
pub fn client_builder_no_env_proxy() -> diting_wire::ClientBuilder {
    let builder = diting_wire::Client::builder().no_proxy();
    if !custom_cert_store_requested(
        std::env::var_os("SSL_CERT_FILE").as_deref(),
        std::env::var_os("SSL_CERT_DIR").as_deref(),
    ) {
        return builder;
    }
    match diting_wire::tls::trust::CertStore::builder()
        .set_default_paths()
        .build()
    {
        Ok(store) => builder.tls_cert_store(store),
        Err(e) => {
            tracing::warn!(
                "SSL_CERT_FILE/SSL_CERT_DIR set but cert store failed to build ({}); using default roots",
                e
            );
            builder
        }
    }
}

#[derive(Debug, Clone)]
pub struct Response {
    pub url: Url,
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub redirected_from: Vec<Url>,
    /// The redirect trail WITH per-hop status: TARGET url + the 3xx that
    /// sent it — the same shape the network-event face reports (#203).
    /// Filled by both engine walks (plain and the wreq stealth
    /// stack), so the scripted-fetch walk can surface a stealth-served
    /// chain's hops (#231 — wreq resolves redirects internally and the
    /// trail used to vanish on the stealth build). Synthesized responses
    /// (tracker blocks, file://, intercepts) carry an empty vec.
    pub redirect_hops: Vec<RedirectHop>,
    /// The outbound header set of the final hop, lowercased like
    /// `headers` (#97). Snapshot of what the engine itself put on the
    /// request — the fully-built set on the plain transport, the
    /// builder-level set on the wreq transport (the emulation's
    /// transport defaults merge at send time inside wreq and are not
    /// visible here). Synthesized responses (file://, data:, tracker
    /// blocks, intercept fulfillments) carry an honest empty map: no
    /// request went on any wire.
    pub request_headers: HashMap<String, String>,
}

impl Response {
    /// Decode the body as text, honoring the response charset.
    ///
    /// Uses the HTTP `Content-Type` header's `charset=` parameter, then for
    /// HTML responses falls back to sniffing `<meta charset>` in the first
    /// 1KB, then UTF-8. Mirrors browser behaviour per the HTML5 spec.
    pub fn text(&self) -> String {
        if self.is_html() {
            crate::encoding::decode_response(&self.body, self.content_type())
        } else {
            crate::encoding::decode_non_html(&self.body, self.content_type())
        }
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(&name.to_lowercase()).map(|s| s.as_str())
    }

    pub fn content_type(&self) -> Option<&str> {
        self.header("content-type")
    }

    pub fn is_html(&self) -> bool {
        self.content_type()
            .map(|ct| ct.contains("text/html"))
            .unwrap_or(false)
    }
}

pub(crate) async fn fetch_file_url(url: &Url) -> Result<Response, NetError> {
    if !allow_file_access() {
        return Err(NetError::Network(
            "file:// access is disabled. Restart with --allow-file-access or set \
             DITING_ALLOW_FILE_ACCESS=1 to enable."
                .to_string(),
        ));
    }
    let path = url
        .to_file_path()
        .map_err(|_| NetError::Network("Invalid file URL".to_string()))?;
    let body = tokio::fs::read(&path)
        .await
        .map_err(|e| NetError::Network(format!("Failed to read file: {}", e)))?;

    let mut headers = HashMap::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ct = match ext.to_lowercase().as_str() {
            "html" | "htm" => "text/html",
            "css" => "text/css",
            "js" | "mjs" => "application/javascript",
            "json" => "application/json",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "svg" => "image/svg+xml",
            "webp" => "image/webp",
            "ico" => "image/x-icon",
            _ => "application/octet-stream",
        };
        headers.insert("content-type".to_string(), ct.to_string());
    }

    Ok(Response {
        url: url.clone(),
        status: 200,
        headers,
        body,
        redirected_from: Vec::new(),
        redirect_hops: Vec::new(),
        request_headers: HashMap::new(),
    })
}

pub struct HttpClient {
    client: tokio::sync::OnceCell<Client>,
    /// Direct-connect client (no proxy). Built once on first use.
    direct_client: tokio::sync::OnceCell<Client>,
    /// Proxy client for known-blocked domains when no explicit proxy was
    /// configured. Built once on first auto-proxy hit from `DITING_PROXY`;
    /// `None` inside means the env var is unset (every later check falls
    /// through to the direct client without rebuilding).
    auto_proxy_client: tokio::sync::OnceCell<Option<Client>>,
    proxy_url: Option<String>,
    pub cookie_jar: Arc<CookieJar>,
    pub user_agent: RwLock<String>,
    /// Same default and env knob as the stealth transport (`diting_net::wreq_client`),
    /// so both transports advertise one Accept-Language (obscura #777 class).
    pub accept_language: RwLock<String>,
    pub extra_headers: RwLock<HashMap<String, String>>,
    pub in_flight: Arc<std::sync::atomic::AtomicU32>,
    /// Per-hop registry of the requests behind `in_flight` — url/method/
    /// dispatch time, so hung scripted fetches are observable while they
    /// hang, not only after their 30s read timeout (#116).
    pub in_flight_list: std::sync::Arc<std::sync::Mutex<Vec<InFlightScripted>>>,
    pub block_trackers: bool,
    /// When true, `validate_url` lets localhost / RFC1918 / link-local addresses
    /// through in addition to the `DITING_ALLOW_PRIVATE_NETWORK` env var.
    /// Set via `--allow-private-network` on the CLI (issue #33).
    pub allow_private_network: bool,
    /// On a stealth page every request this client makes rides the wreq
    /// emulation stack (same TLS fingerprint the navigation presented):
    /// stylesheets, module scripts, render prefetches and form posts
    /// through the `fetch*` entry points, and JS fetch()/XHR hops through
    /// [`Self::scripted_stealth_hop`] — one cookie jar behind two JA3s is a
    /// bot tell (#116, the bilibili 412 lesson: the WAF held the scripted
    /// request until xhr.timeout fired while the document sailed through on
    /// the stealth handshake). Set by `BrowserContext` when
    /// `context.stealth`; plain contexts never set it.
    pub rides_stealth: bool,
    /// The context's requested TLS fingerprint (`tls_fingerprint` param),
    /// baked into the wreq transport so the scripted hop presents the same
    /// profile — and the same platform, derived from the UA — as the page's
    /// own stealth client.
    pub tls_fingerprint: Option<String>,
    /// Build the plain clients on [`crate::chrome_client_builder`] (Chrome
    /// handshake + default headers) instead of the stock BoringSSL one. Set
    /// before the first request; the product tools that borrow this client
    /// (download) turn it on so a tool fetch presents the same handshake a
    /// stealth session does.
    pub emulate_chrome: bool,
    /// The emulating transport scripted hops ride on a stealth page, built
    /// once on first need, sharing this client's cookie jar and proxy
    /// posture.
    stealth_transport: tokio::sync::OnceCell<Arc<crate::StealthHttpClient>>,
}

impl HttpClient {
    pub fn new() -> Self {
        Self::with_cookie_jar(Arc::new(CookieJar::new()))
    }

    pub fn with_cookie_jar(cookie_jar: Arc<CookieJar>) -> Self {
        Self::with_options(cookie_jar, None)
    }

    pub fn with_options(cookie_jar: Arc<CookieJar>, proxy_url: Option<&str>) -> Self {
        Self::with_full_options(cookie_jar, proxy_url, false)
    }

    pub fn with_full_options(
        cookie_jar: Arc<CookieJar>,
        proxy_url: Option<&str>,
        allow_private_network: bool,
    ) -> Self {
        HttpClient {
            client: tokio::sync::OnceCell::new(),
            direct_client: tokio::sync::OnceCell::new(),
            auto_proxy_client: tokio::sync::OnceCell::new(),
            proxy_url: proxy_url.map(|s| s.to_string()),
            cookie_jar,
            user_agent: RwLock::new(
                std::env::var("DITING_UA")
                    .unwrap_or_else(|_| diting_env::chrome_target::UA_DEFAULT.to_string()),
            ),
            accept_language: RwLock::new(
                std::env::var("DITING_ACCEPT_LANGUAGE")
                    .unwrap_or_else(|_| "zh-CN,zh;q=0.9".to_string()),
            ),
            extra_headers: RwLock::new(HashMap::new()),
            in_flight: Arc::new(std::sync::atomic::AtomicU32::new(0)),
            in_flight_list: Arc::new(std::sync::Mutex::new(Vec::new())),
            block_trackers: false,
            allow_private_network,
            rides_stealth: false,
            tls_fingerprint: None,
            emulate_chrome: false,
            stealth_transport: tokio::sync::OnceCell::new(),
        }
    }

    fn base_builder(&self) -> diting_wire::ClientBuilder {
        if self.emulate_chrome {
            crate::chrome_client_builder()
        } else {
            client_builder_no_env_proxy()
        }
    }

    async fn get_client(&self) -> &Client {
        self.client
            .get_or_init(|| async {
                let mut builder = self.base_builder()
                    .redirect(Policy::none())
                    // Browser timeout semantics (#49), not scraper semantics:
                    // `.timeout()` caps the WHOLE request — connect through body
                    // read — so a large-but-live body (the xhs publish page's
                    // 32MB ffmpeg-core.wasm at real CDN speed, ~40s) is killed
                    // mid-stream at 30s with "error decoding response body" while
                    // every browser streams it to completion. `read_timeout` is
                    // the browser shape: headers get 30s from request start, then
                    // every body chunk resets the stall clock — no total cap.
                    .read_timeout(Duration::from_secs(30))
                    .connect_timeout(Duration::from_secs(10))
                    // Bug #24 (long-run degradation): a pooled connection that went
                    // half-dead while idle (NAT drop, proxy reset) used to be handed
                    // back out and stall every subsequent request until the process
                    // was restarted. Keep the idle window short and send TCP
                    // keepalive so the pool reaps stale connections instead.
                    .pool_idle_timeout(Duration::from_secs(60))
                    .tcp_keepalive(Duration::from_secs(30));
                // No manual Accept-Encoding header: with the gzip/brotli/
                // deflate/zstd features the client advertises and decodes on
                // its own, by the RESPONSE's Content-Encoding (pinned by
                // render.rs's unconditional-gzip fixture test), so Aliyun
                // Tengine fronts that compress unrequested still decode. A
                // manual Accept-Encoding disables the auto-decode and raw
                // gzip then reaches the HTML parser.

                if let Some(ref proxy) = self.proxy_url {
                    if let Ok(p) = diting_wire::Proxy::all(proxy.as_str()) {
                        builder = builder.proxy(p);
                    }
                }

                builder.build().expect("failed to build HTTP client")
            })
            .await
    }

    /// Build (once) a direct-connect client with no upstream proxy.
    async fn get_direct_client(&self) -> &Client {
        self.direct_client
            .get_or_init(|| async {
                self.base_builder()
                    .redirect(Policy::none())
                    // read_timeout (per-chunk stall, #49), not a total-duration
                    // cap — see get_client.
                    .read_timeout(Duration::from_secs(30))
                    .connect_timeout(Duration::from_secs(10))
                    // See get_client: short idle window + keepalive against
                    // half-dead pooled connections (bug #24).
                    .pool_idle_timeout(Duration::from_secs(60))
                    .tcp_keepalive(Duration::from_secs(30))
                    // SSRF guard: reject hostnames that resolve to a
                    // private/loopback IP at connect time (replaces the old
                    // TOCTOU pre-resolution check). Only on the direct client —
                    // with a proxy, the proxy resolves target DNS and the only
                    // local lookup is the proxy host itself, which is often
                    // deliberately a loopback address (e.g. socks5://127.0.0.1).
                    .dns_resolver(Arc::new(SsrfGuardResolver::new(Arc::new(
                        std::sync::atomic::AtomicBool::new(self.allow_private_network),
                    ))))
                    .build()
                    .expect("failed to build direct HTTP client")
            })
            .await
    }

    /// Pick the client for this request. An explicitly configured proxy
    /// (`context proxy_url`) routes ALL traffic — that is the operator's
    /// opt-in. Without one, requests go direct except known-blocked domains
    /// (the same list /fetch and download honor), which ride
    /// `DITING_PROXY` when set. That per-request fallback is what keeps
    /// page/session/CDP navigations working on the CN boundary: a session
    /// created without use_proxy used to hard-fail on wikipedia while /fetch
    /// succeeded for the same origin.
    async fn get_client_for(&self, url: &Url) -> &Client {
        if self.proxy_url.is_some() {
            return self.get_client().await;
        }
        if let Some(c) = self.get_auto_proxy_client(url).await {
            return c;
        }
        self.get_direct_client().await
    }

    /// Proxied client for known-blocked domains, built once from
    /// `DITING_PROXY`. `None` when the domain isn't listed or no proxy
    /// is configured — callers fall through to the direct client.
    ///
    /// Like `get_client`, no SSRF DNS resolver here: with a proxy the target
    /// resolves at the proxy, and the proxy host itself is often loopback.
    async fn get_auto_proxy_client(&self, url: &Url) -> Option<&Client> {
        if !diting_env::env_knobs::should_auto_proxy(url.as_str()) {
            return None;
        }
        let proxy = match diting_env::env_knobs::proxy_from_env() {
            Some(p) => p,
            None => return None,
        };
        self.auto_proxy_client
            .get_or_init(|| async move {
                let mut builder = self.base_builder()
                    .redirect(Policy::none())
                    // read_timeout (per-chunk stall, #49), not a total-duration
                    // cap — see get_client.
                    .read_timeout(Duration::from_secs(30))
                    .connect_timeout(Duration::from_secs(10))
                    .pool_idle_timeout(Duration::from_secs(60))
                    .tcp_keepalive(Duration::from_secs(30));
                if let Ok(p) = diting_wire::Proxy::all(&proxy) {
                    builder = builder.proxy(p);
                }
                Some(
                    builder
                        .build()
                        .expect("failed to build auto-proxy HTTP client"),
                )
            })
            .await
            .as_ref()
    }

    /// Read-only accessor for the proxy URL the client was configured with
    /// (if any). Exposed so callers outside the net module — notably
    /// `op_fetch_url` in `diting-js` (#139) — can route their own
    /// requests through the same upstream proxy.
    pub fn proxy_url(&self) -> Option<&str> {
        self.proxy_url.as_deref()
    }

    /// The plain client this request should use (context-scoped, tied to
    /// this browser context). Cloning a `Client` is cheap — it shares
    /// the underlying connection pool — so callers that need an owned handle
    /// (e.g. `op_fetch_url`, which builds a request and follows redirects
    /// itself) can take one without copying the pool.
    pub async fn request_client(&self, url: &str) -> Client {
        match url::Url::parse(url) {
            Ok(u) => self.get_client_for(&u).await.clone(),
            Err(_) => self.get_direct_client().await.clone(),
        }
    }

    /// Currently-flying script-initiated fetch()/XHR hops (per-redirect-hop,
    /// like the counter). Readers compute `age_ms` from `dispatched_at_ms`.
    pub fn scripted_in_flight(&self) -> Vec<InFlightScripted> {
        self.in_flight_list
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub async fn fetch(&self, url: &Url) -> Result<Response, NetError> {
        self.fetch_with_method(Method::GET, url, None).await
    }

    /// Compute the default `strict-origin-when-cross-origin` referrer for a
    /// document-initiated request (upstream edb1785). Same-origin sends the
    /// full source URL minus fragment/credentials; cross-origin sends only
    /// the origin; downgrades (https -> http) and non-HTTP(S) schemes send
    /// nothing. Referrer-Policy overrides are not yet plumbed through.
    pub fn navigation_referrer(source: &Url, target: &Url) -> String {
        if !matches!(source.scheme(), "http" | "https")
            || !matches!(target.scheme(), "http" | "https")
            || (source.scheme() == "https" && target.scheme() == "http")
        {
            return String::new();
        }

        if source.origin() == target.origin() {
            let mut sanitized = source.clone();
            sanitized.set_fragment(None);
            let _ = sanitized.set_username("");
            let _ = sanitized.set_password(None);
            return sanitized.to_string();
        }

        let mut origin = source.origin().ascii_serialization();
        origin.push('/');
        origin
    }

    /// Page-subresource fetch (render-time img/media prefetch): a plain GET
    /// that carries the initiating document's Referer. `referrer` is the raw
    /// document URL; the policy trim happens per hop in the traced path.
    pub async fn fetch_subresource(
        &self,
        url: &Url,
        referrer: Option<&str>,
    ) -> Result<Response, NetError> {
        self.fetch_with_method_traced(Method::GET, url, None, None, ResourceType::Image, referrer)
            .await
    }

    /// Passive-observer variants (upstream issue #408): fire the registry's
    /// on_request callbacks with the fully-built request just before each hop
    /// is sent, and on_response with the completed response. `None` behaves
    /// exactly like the untraced entry points. `referrer` is the initiating
    /// document URL for page-driven loads; top-level tool fetches pass None.
    pub async fn fetch_with_callbacks(
        &self,
        url: &Url,
        callbacks: Option<&CallbackRegistry>,
        resource_type: ResourceType,
        referrer: Option<&str>,
    ) -> Result<Response, NetError> {
        self.fetch_with_method_traced(Method::GET, url, None, callbacks, resource_type, referrer)
            .await
    }

    /// See `fetch_with_callbacks`.
    pub async fn post_form_with_callbacks(
        &self,
        url: &Url,
        body: &str,
        callbacks: Option<&CallbackRegistry>,
        resource_type: ResourceType,
        referrer: Option<&str>,
    ) -> Result<Response, NetError> {
        self.fetch_with_method_traced(
            Method::POST,
            url,
            Some(body.as_bytes().to_vec()),
            callbacks,
            resource_type,
            referrer,
        )
        .await
    }

    pub async fn fetch_with_method(
        &self,
        initial_method: Method,
        url: &Url,
        initial_body: Option<Vec<u8>>,
    ) -> Result<Response, NetError> {
        self.fetch_with_method_traced(
            initial_method,
            url,
            initial_body,
            None,
            ResourceType::Document,
            None,
        )
        .await
    }

    async fn fetch_with_method_traced(
        &self,
        initial_method: Method,
        url: &Url,
        initial_body: Option<Vec<u8>>,
        callbacks: Option<&CallbackRegistry>,
        resource_type: ResourceType,
        referrer: Option<&str>,
    ) -> Result<Response, NetError> {
        if self.rides_stealth && url.scheme() != "file" {
            return self
                .fetch_via_stealth(
                    initial_method,
                    url,
                    initial_body.as_deref(),
                    callbacks,
                    resource_type,
                    referrer,
                )
                .await;
        }
        self.fetch_with_method_traced_inner(
            initial_method,
            url,
            initial_body.as_deref(),
            callbacks,
            resource_type,
            referrer,
        )
        .await
    }

    /// The stealth-context arm of the `fetch*` entry points: same request on
    /// the emulating transport — per-class Fetch-Metadata, Chrome header
    /// order and Priority come from the stealth walk, which also re-validates
    /// SSRF per hop and stores cookies. The transport has no observer hook,
    /// so callbacks fire here with the headers that actually went out, and
    /// the in-flight counter the settle loop reads covers it like the plain
    /// path.
    async fn fetch_via_stealth(
        &self,
        method: Method,
        url: &Url,
        body: Option<&[u8]>,
        callbacks: Option<&CallbackRegistry>,
        resource_type: ResourceType,
        referrer: Option<&str>,
    ) -> Result<Response, NetError> {
        // Mirror the plain path's default for a POST with a body.
        let content_type = (body.is_some() && method == Method::POST)
            .then_some("application/x-www-form-urlencoded");
        let stealth = self.synced_stealth_transport().await;
        self.in_flight
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let in_flight = InFlightDecrement(Some(self.in_flight.clone()));
        let resp = stealth
            .fetch_inner(
                url,
                referrer,
                method.clone(),
                body,
                content_type,
                None,
                true,
                resource_type,
            )
            .await;
        drop(in_flight);
        let resp = resp?;
        if let Some(cbs) = callbacks {
            if cbs.has_request_callbacks().await || cbs.has_response_callbacks().await {
                let info = RequestInfo {
                    url: resp.url.clone(),
                    method: method.as_str().to_string(),
                    headers: resp.request_headers.clone(),
                    resource_type,
                };
                cbs.fire_request(&info).await;
                cbs.fire_response(&info, &resp).await;
            }
        }
        Ok(resp)
    }

    /// The stealth transport with this client's current identity (UA,
    /// Accept-Language, extra headers) copied in — they can change after
    /// the transport was built.
    async fn synced_stealth_transport(&self) -> &Arc<crate::StealthHttpClient> {
        let stealth = self.stealth_transport().await;
        stealth
            .set_user_agent(&self.user_agent.read().await.clone())
            .await;
        stealth
            .set_accept_language(&self.accept_language.read().await.clone())
            .await;
        stealth
            .set_extra_headers(self.extra_headers.read().await.clone())
            .await;
        stealth
    }

    /// #116 transport-layer fix: the stealth-first scripted hop. On a
    /// stealth page the JS fetch()/XHR attempt rides the wreq emulation
    /// stack — the same TLS fingerprint the navigation above it presented
    /// — BEFORE the plain client: one cookie jar behind one JA3, not two.
    ///
    /// Returns `None` when the stealth posture doesn't apply (a plain
    /// context) and the caller keeps its plain path unchanged. `Some(Ok)`
    /// is a complete response — redirects walked, cookies sent and stored,
    /// every hop re-validated against SSRF inside the stealth walk — and
    /// `Some(Err)` means the stealth stack was tried and failed; the
    /// caller then falls through to plain.
    #[allow(clippy::too_many_arguments)]
    pub async fn scripted_stealth_hop(
        &self,
        method: &Method,
        url: &Url,
        body: Option<&[u8]>,
        content_type: Option<&str>,
        request_headers: Option<&HashMap<String, String>>,
        include_cookies: bool,
    ) -> Option<Result<Response, NetError>> {
        if !self.rides_stealth {
            return None;
        }
        if validate_url(url, self.allow_private_network).is_err() {
            // A gate rejection is a policy answer, not a transport
            // failure — hand the URL to the plain path so the existing
            // gate error surfaces keep their shape.
            return None;
        }
        let stealth = self.synced_stealth_transport().await;
        Some(
            stealth
                .fetch_with_body(
                    url,
                    None,
                    method.as_str(),
                    body,
                    content_type,
                    request_headers,
                    include_cookies,
                )
                .await,
        )
    }

    /// Build the stealth transport on first need, mirroring this client's
    /// cookie jar, proxy, private-network and tracker-blocking posture. It bakes the page's
    /// own handshake posture — the context `tls_fingerprint` profile and the
    /// UA-derived platform — instead of the stock chrome-target default, so
    /// the scripted hop never swaps one JA3-mismatch tell for another (#116).
    async fn stealth_transport(&self) -> &Arc<crate::StealthHttpClient> {
        self.stealth_transport
            .get_or_init(|| async {
                let profile = self
                    .tls_fingerprint
                    .as_deref()
                    .and_then(crate::parse_tls_fingerprint)
                    .unwrap_or_else(diting_env::chrome_target::target_tls_profile);
                let os = crate::emulation_os_for_ua(&self.user_agent.read().await.clone());
                let mut client = crate::StealthHttpClient::with_proxy_and_emulation(
                    self.cookie_jar.clone(),
                    self.proxy_url.as_deref(),
                    Some(os),
                    profile,
                );
                client.allow_private_network.store(
                    self.allow_private_network,
                    std::sync::atomic::Ordering::Relaxed,
                );
                client.block_trackers = self.block_trackers;
                Arc::new(client)
            })
            .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn fetch_with_method_traced_inner(
        &self,
        initial_method: Method,
        url: &Url,
        initial_body: Option<&[u8]>,
        callbacks: Option<&CallbackRegistry>,
        resource_type: ResourceType,
        referrer: Option<&str>,
    ) -> Result<Response, NetError> {
        validate_url(url, self.allow_private_network)?;

        if url.scheme() == "file" {
            return fetch_file_url(url).await;
        }

        let mut method = initial_method;
        let mut body = initial_body;
        if self.block_trackers {
            if let Some(host) = url.host_str() {
                if crate::blocklist::is_blocked(host) {
                    tracing::debug!("Blocked tracker: {}", url);
                    return Ok(Response {
                        status: 0,
                        url: url.clone(),
                        headers: HashMap::new(),
                        body: Vec::new(),
                        redirected_from: Vec::new(),
                        redirect_hops: Vec::new(),
                        request_headers: HashMap::new(),
                    });
                }
            }
        }

        let mut current_url = url.clone();
        let mut redirects = Vec::new();
        // #231 twin of `redirects`: hop targets WITH status — the network
        // face shape, so a scripted walk served by this client can adopt
        // the whole trail without re-deriving it.
        let mut redirect_hops: Vec<RedirectHop> = Vec::new();
        let max_redirects = 20;

        // Fetch-Metadata + SameSite context, fixed for the whole redirect
        // chain: the initiator is the referring document (None = address-bar
        // / tool navigation); the header triple comes from the resource
        // class (metadata.rs, #203).
        let initiator = referrer.and_then(|s| url::Url::parse(s).ok());
        let is_navigation = metadata::is_navigation(resource_type);
        let FetchMetadata {
            dest: fetch_dest,
            mode: fetch_mode,
            default_accept,
        } = metadata_for(resource_type);

        for _redirect_count in 0..max_redirects {
            let ua = self.user_agent.read().await.clone();
            let (sec_ch_ua, platform) = derive_client_hints(&ua);
            let mut headers = HeaderMap::new();
            headers.insert(
                USER_AGENT,
                HeaderValue::from_str(&ua).unwrap_or_else(|_| {
                    HeaderValue::from_static(diting_env::chrome_target::UA_DEFAULT)
                }),
            );
            headers.insert(
                diting_wire::header::ACCEPT,
                HeaderValue::from_static(default_accept),
            );
            headers.insert(
                diting_wire::header::ACCEPT_LANGUAGE,
                HeaderValue::from_str(&self.accept_language.read().await.clone())
                    .unwrap_or_else(|_| HeaderValue::from_static("zh-CN,zh;q=0.9")),
            );
            headers.insert(
                HeaderName::from_static("sec-ch-ua"),
                HeaderValue::from_str(&sec_ch_ua).unwrap(),
            );
            headers.insert(
                HeaderName::from_static("sec-ch-ua-mobile"),
                HeaderValue::from_static("?0"),
            );
            headers.insert(
                HeaderName::from_static("sec-ch-ua-platform"),
                HeaderValue::from_str(&platform).unwrap(),
            );
            headers.insert(
                HeaderName::from_static("sec-fetch-dest"),
                HeaderValue::from_static(fetch_dest),
            );
            headers.insert(
                HeaderName::from_static("sec-fetch-mode"),
                HeaderValue::from_static(fetch_mode),
            );
            // Per-hop against the ORIGINAL initiator (the whole chain keeps
            // the request's initiator): same-origin / same-site (registrable
            // domain, so sibling subdomains are same-site) / cross-site /
            // none (no referring document).
            headers.insert(
                HeaderName::from_static("sec-fetch-site"),
                HeaderValue::from_str(crate::site::sec_fetch_site(
                    initiator.as_ref(),
                    &current_url,
                ))
                .unwrap(),
            );
            if is_navigation {
                headers.insert(
                    HeaderName::from_static("sec-fetch-user"),
                    HeaderValue::from_static("?1"),
                );
                headers.insert(
                    HeaderName::from_static("upgrade-insecure-requests"),
                    HeaderValue::from_static("1"),
                );
            }
            // Document-initiated requests carry the initiator's Referer —
            // whitelist-checked service APIs (map vendors, CDN anti-leech)
            // reject bare requests — trimmed per hop by the navigation
            // policy, since a redirect can change the same/cross-origin
            // answer. Non-GET/HEAD also carries Origin (same-origin POSTs
            // included, matching the fetch() op's b744b9b semantics).
            // extra_headers below can override both.
            if let Some(src) = referrer {
                if let Ok(source) = Url::parse(src) {
                    let ref_value = Self::navigation_referrer(&source, &current_url);
                    if !ref_value.is_empty() {
                        if let Ok(v) = HeaderValue::from_str(&ref_value) {
                            headers.insert(diting_wire::header::REFERER, v);
                        }
                    }
                    if method != Method::GET
                        && method != Method::HEAD
                        && !headers.contains_key(diting_wire::header::ORIGIN)
                    {
                        let origin_value = source.origin().ascii_serialization();
                        if let Ok(v) = HeaderValue::from_str(&origin_value) {
                            headers.insert(diting_wire::header::ORIGIN, v);
                        }
                    }
                }
            }

            // SameSite context: cross-site against the initiator document
            // (no initiator = address-bar navigation, never cross-site);
            // navigations may ride Lax cookies cross-site with a safe
            // method, subresources never do.
            let cross_site = initiator
                .as_ref()
                .map(|i| !crate::site::is_same_site(i, &current_url))
                .unwrap_or(false);
            let cookie_ctx = if is_navigation {
                crate::cookies::SendContext::navigation(
                    cross_site,
                    method == Method::GET || method == Method::HEAD,
                )
            } else {
                crate::cookies::SendContext::subresource(cross_site)
            };
            // Module scripts: a cross-origin hop is CORS with credentials
            // "same-origin" — Origin on, cookies off.
            let module_origin =
                metadata::cross_origin_module(resource_type, initiator.as_ref(), &current_url);
            if let Some(origin) = &module_origin {
                if let Ok(v) = HeaderValue::from_str(origin) {
                    headers.insert(diting_wire::header::ORIGIN, v);
                }
            }
            let cookie_header = if module_origin.is_some() {
                String::new()
            } else {
                self.cookie_jar
                    .get_cookie_header_for(&current_url, &cookie_ctx)
            };
            tracing::debug!(
                "Cookie header for {}: {} cookies ({} bytes)",
                current_url.host_str().unwrap_or("?"),
                cookie_header.split("; ").filter(|s| !s.is_empty()).count(),
                cookie_header.len(),
            );
            if !cookie_header.is_empty() {
                match HeaderValue::from_str(&cookie_header) {
                    Ok(val) => {
                        headers.insert(diting_wire::header::COOKIE, val);
                    }
                    Err(_) => {
                        let filtered: String = cookie_header
                            .split("; ")
                            .filter(|pair| HeaderValue::from_str(pair).is_ok())
                            .collect::<Vec<_>>()
                            .join("; ");
                        if !filtered.is_empty() {
                            if let Ok(val) = HeaderValue::from_str(&filtered) {
                                headers.insert(diting_wire::header::COOKIE, val);
                            }
                        }
                        tracing::debug!(
                            "Cookie header invalid chars, filtered {} -> {} bytes",
                            cookie_header.len(),
                            filtered.len(),
                        );
                    }
                }
            }

            for (k, v) in self.extra_headers.read().await.iter() {
                if let (Ok(name), Ok(val)) = (
                    HeaderName::from_bytes(k.as_bytes()),
                    HeaderValue::from_str(v),
                ) {
                    headers.insert(name, val);
                }
            }

            // POST bodies carry a default content-type (the fetch()/XHR
            // callers pass their own through extra/req-local headers). It
            // rides the HeaderMap like everything else so the #97 wire
            // snapshot below sees the complete set.
            if body.is_some() && method == Method::POST {
                headers.insert(
                    diting_wire::header::CONTENT_TYPE,
                    HeaderValue::from_static("application/x-www-form-urlencoded"),
                );
            }

            // #97: snapshot the fully-built outbound header set (it is
            // moved into the request below) — lowercased like
            // `Response.headers`. Computed unconditionally: the response
            // carries it to the network-event faces even when no passive
            // on_request observer listens.
            let wire_headers: HashMap<String, String> = headers
                .iter()
                .map(|(k, v)| {
                    (
                        k.as_str().to_lowercase(),
                        v.to_str().unwrap_or("").to_string(),
                    )
                })
                .collect();
            // Passive on_request observers (upstream #408): fire with the
            // same snapshot just before the request goes out. Skipped
            // entirely when nobody listens.
            let fire_request = match callbacks {
                Some(cbs) if cbs.has_request_callbacks().await => Some(cbs),
                _ => None,
            };

            let mut req_builder = self
                .get_client_for(&current_url)
                .await
                .request(method.clone(), current_url.as_str())
                .headers(headers);

            if let Some(b) = body {
                req_builder = req_builder.body(b.to_vec());
            }

            if let Some(cbs) = fire_request {
                let info = RequestInfo {
                    url: current_url.clone(),
                    method: method.as_str().to_string(),
                    headers: wire_headers.clone(),
                    resource_type,
                };
                cbs.fire_request(&info).await;
            }

            self.in_flight
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            // #309: the counter decrements on Drop, so a cancelled
            // navigation (this future dropped mid-send by a newer goto)
            // can no longer strand it at +1 for the session's life.
            let _in_flight = InFlightDecrement(Some(self.in_flight.clone()));
            let resp = match req_builder.send().await {
                Ok(resp) => resp,
                Err(e) => {
                    drop(_in_flight);
                    // Name the culprit when the configured upstream proxy is
                    // the part that cannot be reached: the target URL alone
                    // reads as "site down" and sends the operator debugging
                    // the wrong layer (obscura#491's debugging cost).
                    let unreachable = e.is_connect() || e.is_proxy_connect();
                    return Err(match (&self.proxy_url, unreachable) {
                        (Some(proxy), true) => NetError::Network(format!(
                            "upstream proxy {} unreachable while fetching {}: {} — unset DITING_PROXY to connect directly",
                            proxy, current_url, e
                        )),
                        _ => NetError::Network(format!("{}: {}", current_url, e)),
                    });
                }
            };
            drop(_in_flight);

            let status = resp.status();

            for val in resp.headers().get_all(diting_wire::header::SET_COOKIE) {
                if let Ok(s) = val.to_str() {
                    self.cookie_jar
                        .set_cookie_in_context(s, &current_url, &cookie_ctx, "http");
                }
            }

            let response_headers = crate::collect_response_headers(resp.headers());

            if status.is_redirection() {
                if let Some(location) = resp.headers().get(diting_wire::header::LOCATION) {
                    let location_str = location.to_str().map_err(|_| {
                        NetError::Network("Invalid redirect Location header".into())
                    })?;
                    let next_url = current_url
                        .join(location_str)
                        .map_err(|e| NetError::Network(format!("Invalid redirect URL: {}", e)))?;
                    validate_url(&next_url, self.allow_private_network)?;
                    redirects.push(current_url.clone());
                    redirect_hops.push(RedirectHop {
                        url: next_url.to_string(),
                        status: status.as_u16(),
                    });
                    current_url = next_url;
                    if status == diting_wire::StatusCode::MOVED_PERMANENTLY
                        || status == diting_wire::StatusCode::FOUND
                        || status == diting_wire::StatusCode::SEE_OTHER
                    {
                        method = Method::GET;
                        body = None;
                    }
                    continue;
                }
            }

            let body_bytes = resp
                .bytes()
                .await
                .map_err(|e| {
                    tracing::warn!(
                        "body read failed for {}: {} (status={}, ctype={:?})",
                        current_url,
                        e,
                        status,
                        response_headers.get("content-type")
                    );
                    NetError::Network(format!("Failed to read body: {}", e))
                })?
                .to_vec();

            let response = Response {
                url: current_url,
                status: status.as_u16(),
                headers: response_headers,
                body: body_bytes,
                redirected_from: redirects,
                redirect_hops,
                request_headers: wire_headers,
            };

            // Passive on_response observers: fired with the completed final
            // response (post-redirect, body read).
            if let Some(cbs) = callbacks {
                if cbs.has_response_callbacks().await {
                    let info = RequestInfo {
                        url: response.url.clone(),
                        method: method.as_str().to_string(),
                        headers: response.headers.clone(),
                        resource_type,
                    };
                    cbs.fire_response(&info, &response).await;
                }
            }

            return Ok(response);
        }

        Err(NetError::TooManyRedirects(current_url.to_string()))
    }

    pub async fn set_user_agent(&self, ua: &str) {
        *self.user_agent.write().await = ua.to_string();
    }

    pub async fn set_accept_language(&self, lang: &str) {
        *self.accept_language.write().await = lang.to_string();
    }

    pub async fn set_extra_headers(&self, headers: HashMap<String, String>) {
        *self.extra_headers.write().await = headers;
    }

    pub fn active_requests(&self) -> u32 {
        self.in_flight.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Derive `sec-ch-ua` and `sec-ch-ua-platform` from the User-Agent string so
/// the client hints stay consistent with the advertised UA. Anti-bot systems
/// (WeChat, etc.) flag mismatches like a macOS UA paired with a "Linux"
/// sec-ch-ua-platform or a version drift between UA and sec-ch-ua.
///
/// Returns `(sec_ch_ua_header, sec_ch_ua_platform_header)`.
pub fn derive_client_hints(ua: &str) -> (String, String) {
    // Major version: first \d+ after "Chrome/". Fleet fallback claims 145 —
    // the TLS preset's own major, and one the xhs edge routes to production
    // (146-153/155/156 land in its customer.sit loop).
    let version = ua
        .split("Chrome/")
        .nth(1)
        .and_then(|s| s.split('.').next())
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(145);

    let platform = if ua.contains("Macintosh") || ua.contains("Mac OS X") {
        "\"macOS\""
    } else if ua.contains("Windows") {
        "\"Windows\""
    } else if ua.contains("iPhone") || ua.contains("Android") {
        "\"Android\""
    } else {
        "\"Linux\""
    };

    (grease_brand_list(version), platform.to_string())
}

/// Chromium's UA-CH GREASE generator (user_agent_utils.cc), ported: the
/// grease brand, its version, and the shuffle order are all derived from
/// the major version — never random. Keeping the algorithm instead of a
/// per-major literal makes the header match the JS face
/// (bootstrap.js `__ditingBrandList`) for any major the fleet claims;
/// 145 and 155 were both hardcoded wrong before this.
/// 154 → `"Chromium";v="154", "Google Chrome";v="154", "Not A(Brand";v="99"`
/// (algorithm example; the fleet itself claims 145).
fn grease_brand_list(version: u32) -> String {
    const CHARS: [&str; 11] = [" ", "(", ":", "-", ".", "/", ")", ";", "=", "?", "_"];
    const VERS: [&str; 3] = ["8", "99", "24"];
    const ORDERS: [[usize; 3]; 6] = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    let seed = version as usize;
    let grease = format!("Not{}A{}Brand", CHARS[seed % 11], CHARS[(seed + 1) % 11]);
    let grease_ver = VERS[seed % 3];
    let vstr = version.to_string();
    let pre_shuffled = [
        (grease.as_str(), grease_ver),
        ("Chromium", vstr.as_str()),
        ("Google Chrome", vstr.as_str()),
    ];
    let order = ORDERS[seed % 6];
    let mut shuffled: [(&str, &str); 3] = [("", ""); 3];
    for (i, slot) in order.iter().enumerate() {
        shuffled[*slot] = pre_shuffled[i];
    }
    let parts: Vec<String> = shuffled
        .iter()
        .map(|(brand, ver)| format!("\"{brand}\";v=\"{ver}\""))
        .collect();
    parts.join(", ")
}

#[derive(Debug, thiserror::Error)]
pub enum NetError {
    #[error("Network error: {0}")]
    Network(String),

    #[error("Too many redirects: {0}")]
    TooManyRedirects(String),
}

#[cfg(test)]
mod tests;
