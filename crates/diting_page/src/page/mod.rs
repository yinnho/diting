use std::rc::Rc;
use std::sync::Arc;

use diting_dom::DomTree;
use diting_js::runtime::JsRuntime;
use diting_js::{Document, LayoutPolicy};
use diting_net::{HttpClient, NetError, StealthHttpClient};
use url::Url;

use crate::context::BrowserContext;
use crate::lifecycle::LifecycleState;

use self::viewport::{EmulatedMedia, ScreenOverride};
// Page decomposition (ARCHITECTURE.md P2 batch 3): navigation & settle,
// script execution, viewport/band capture, JS evaluation and network
// bookkeeping live in sibling files under this directory. The `page::`
// paths consumers use are unchanged — everything below still hangs off
// this module.
mod eval;
mod history;
mod load;
mod navigate;
mod net;
mod pump;
mod realm;
pub use net::{NavigationRecord, NetworkEvent, StoredResponseBody};
mod scripts;
mod viewport;

#[cfg(test)]
mod tests;

/// Engine-side progress of the in-flight navigation, shared with the
/// navigation's caller as an `Arc` so an observer can poll it WHILE the
/// navigation future is still running (the `/fetch` starvation
/// checkpoint). A JS probe cannot serve this job: our performance
/// resource timings are only pushed when a whole fetch phase completes,
/// so a live navigation's script deliveries are invisible to JS — the
/// engine is the only side that sees bodies land as they land. All
/// fields are atomics because the observer thread/future is not the one
/// driving the navigation.
#[derive(Debug, Default)]
pub struct NavProgress {
    /// The realm swap (`realm_epoch` catching up) has happened: the
    /// requested document is the live page.
    doc_committed: std::sync::atomic::AtomicBool,
    /// Parser-phase external scripts discovered by the current
    /// `execute_scripts` phase, and how many of their bodies have
    /// arrived. `total > 0 && arrived == 0` at a checkpoint = the script
    /// trickle never moved.
    script_total: std::sync::atomic::AtomicUsize,
    script_arrived: std::sync::atomic::AtomicUsize,
}

impl NavProgress {
    fn reset(&self) {
        self.doc_committed
            .store(false, std::sync::atomic::Ordering::Release);
        self.script_total
            .store(0, std::sync::atomic::Ordering::Release);
        self.script_arrived
            .store(0, std::sync::atomic::Ordering::Release);
    }
    fn note_committed(&self) {
        self.doc_committed
            .store(true, std::sync::atomic::Ordering::Release);
    }
    fn note_script_phase(&self, total: usize) {
        self.script_total
            .store(total, std::sync::atomic::Ordering::Release);
        self.script_arrived
            .store(0, std::sync::atomic::Ordering::Release);
    }
    fn note_script_arrival(&self) {
        self.script_arrived
            .fetch_add(1, std::sync::atomic::Ordering::Release);
    }

    /// Did the requested document commit? (The realm swap happened.)
    pub fn committed(&self) -> bool {
        self.doc_committed
            .load(std::sync::atomic::Ordering::Acquire)
    }
    /// Bodies landed by the current parser-phase script fetch wave.
    pub fn script_arrivals(&self) -> usize {
        self.script_arrived
            .load(std::sync::atomic::Ordering::Acquire)
    }
    /// Parser-phase scripts were discovered and not one body has landed —
    /// the download is starving, not slow. A page with no external
    /// scripts (`total == 0`) is never starving.
    pub fn script_zero_progress(&self) -> bool {
        self.script_total.load(std::sync::atomic::Ordering::Acquire) > 0
            && self
                .script_arrived
                .load(std::sync::atomic::Ordering::Acquire)
                == 0
    }
}

/// #276 memory forensics: prove which layer holds a closed session's
/// realm alive. Gated by DITING_REALM_HEAP_STATS.
impl Drop for Page {
    fn drop(&mut self) {
        if std::env::var("DITING_REALM_HEAP_STATS")
            .map(|v| v != "0" && !v.is_empty())
            .unwrap_or(false)
        {
            eprintln!("diting Page dropped url={}", self.url_string());
        }
    }
}

/// The page's identity as it stood before its first CDP
/// `setUserAgentOverride`: what an empty `userAgent` or `acceptLanguage`
/// puts back.
#[derive(Clone)]
struct IdentityDefaults {
    /// The plain transport's UA and the stealth transport's.
    user_agent: (String, Option<String>),
    /// The language, and whether it was pinned (`DITING_ACCEPT_LANGUAGE`).
    language: (String, bool),
}

pub struct Page {
    pub id: String,
    /// Upstream frame-realm identifier: one Page can host sub-frame realms
    /// keyed by frame id. We run a single realm per page, so nothing reads
    /// it yet (frame-realm absorption is parked).
    pub frame_id: String,
    pub url: Option<Url>,
    pub dom: Option<DomTree>,
    pub js: Option<JsRuntime>,
    /// sessionStorage snapshot for the CURRENT document, as `(origin, entries)`.
    /// sessionStorage is per-tab-per-origin: it survives a same-origin
    /// navigation (like a reload) but a cross-origin one discards it. The realm
    /// is rebuilt on every navigation (`init_js`) and parked on target switch
    /// (`suspend_js`/`resume_js`), so we snapshot before teardown and re-seed
    /// after rebuild — otherwise a same-origin navigation or a second target's
    /// evaluate wipes the store (upstream #678).
    session_storage: Option<(String, std::collections::HashMap<String, String>)>,
    /// Console calls drained out of the realm when it was suspended. Without
    /// this buffer, anything logged since the last drain died with the realm
    /// on a target switch or storm-control park, and the client saw an empty
    /// console after resume (obscura#971 same hole).
    suspended_console: Vec<(String, String, String)>,
    /// Viewport override for session_viewport / CDP
    /// setDeviceMetricsOverride: (width, height, mobile). Lives on the
    /// Page, not the realm, because every navigation rebuilds the realm and
    /// republishes the desktop persona viewport (`set_user_agent` →
    /// `__diting_setPersona`) — the override has to be replayed after that
    /// or the page silently flips back mid-session.
    viewport_override: Option<(f32, f32, bool)>,
    /// Box-tree posture. `Mock` until a face that paints or hits real
    /// geometry opts into `OnDemand`. Replayed onto each new realm.
    layout_policy: LayoutPolicy,
    /// devicePixelRatio pin from the same override (CDP deviceScaleFactor
    /// above zero); None keeps the persona's dpr reporting through. Rides
    /// the same replay as `viewport_override`.
    dpr_override: Option<f64>,
    /// `screenWidth`/`screenHeight`/`screenOrientation` of the same CDP
    /// override. `None` leaves `screen` on the persona's panel. Rides
    /// `viewport_override`'s replay and its clear.
    screen_override: Option<ScreenOverride>,
    /// Emulated media environment from CDP `Emulation.setEmulatedMedia`
    /// (Playwright's `page.emulateMedia`). Lives on the Page for the same
    /// reason as `viewport_override` — every navigation rebuilds the realm,
    /// so the emulation has to be replayed or the page silently flips back
    /// to the persona defaults mid-session (#29).
    emulated_media: Option<EmulatedMedia>,
    /// Touch emulation pin from CDP `Emulation.setTouchEmulationEnabled`
    /// (#151): the emulated `navigator.maxTouchPoints` while set. Narrower
    /// than the mobile viewport pin — pointer/hover answers and touch points
    /// move, the viewport does not. Replayed after every realm rebuild for
    /// the same reason as `viewport_override`.
    touch_override: Option<u32>,
    /// CDP `Emulation.setTimezoneOverride`. Lives on the Page so a
    /// navigation, which rebuilds the realm, replays it. Empty means the
    /// bootstrap's language-derived zone.
    timezone_override: Option<String>,
    /// CDP `Emulation.setLocaleOverride` (#153). Lives on the Page so a
    /// navigation, which rebuilds the realm, replays it. It pins Intl's
    /// default locale only — navigator.language stays with the language
    /// (persona, or `setUserAgentOverride`'s `acceptLanguage`), the way
    /// Chrome keeps the two pins apart. Empty means no pin: Intl's default
    /// locale derives from the language again.
    locale_override: Option<String>,
    /// CDP `setUserAgentOverride`'s `platform` (navigator.platform). Lives
    /// on the Page for the same reason: the pin is a global of the realm
    /// and every navigation builds a new one.
    platform_override: Option<String>,
    /// What `setUserAgentOverride` found on its first call: each
    /// transport's UA, and the language with its pin. An empty `userAgent`
    /// or `acceptLanguage` puts them back, as it does in Chrome.
    identity_defaults: Option<IdentityDefaults>,
    /// 32-bit seed the JS persona draws its hardware identity from
    /// (screen/dpr/GPU/canvas). Lives on the Page because every navigation
    /// rebuilds the realm and `__diting_init` self-deletes after drawing a
    /// fresh identity — without a re-pin, one visit would flip its screen
    /// and GPU page to page, its own automation tell. Same pattern as the
    /// viewport override above.
    fp_seed: u64,
    pub lifecycle: LifecycleState,
    pub http_client: Arc<HttpClient>,
    pub context: Arc<BrowserContext>,
    /// The live document. After commit the current realm holds this same
    /// `Rc`. Public reads (`title`) see it only — a navigation that has
    /// not committed must not blank or replace it.
    document: Rc<Document>,
    /// In-flight navigation draft. Staging writes (referrer, encoding,
    /// content type, title) land here while the outgoing realm is still
    /// current, so they do not change what that document's JS sees.
    /// `init_js` takes it; a failed attempt drops it.
    pending_document: Option<Rc<Document>>,
    /// Navigation history for Page.getNavigationHistory / navigateToHistoryEntry.
    /// Entries are URLs in visit order; `history_index` is the current position.
    /// Pushed on every successful navigation; truncated on goBack -> new nav.
    pub history: Vec<String>,
    pub history_index: usize,
    pub(crate) network_events: Vec<NetworkEvent>,
    /// Events of the outgoing document, carried across the per-navigation
    /// reset of `network_events`. Filled at the top of `navigate_single`.
    /// The live navigation's log is [`Page::navigation_record`]; this is
    /// the previous document's rows.
    pub(crate) carried_network_events: Vec<NetworkEvent>,
    /// The outgoing document's URL, kept beside the carried events. A
    /// timed-out attempt that never committed rolls `url` back to it.
    pub(crate) carried_network_url: String,
    /// Navigation generation counter (#101): bumped once per
    /// `navigate_single` hop, so `NetworkEvent.nav` rows attribute to the
    /// attempt that issued them. The product reads it through
    /// [`Page::navigation_record`].
    pub(crate) navigation_epoch: u64,
    /// The current document's HTTP redirect trail (#102):
    /// `redirect_chain[0]` is the URL the navigation asked for, `url` is
    /// where the document actually came from; empty when no redirect
    /// happened. Reset at the start of each attempt and set from the
    /// document response, so a failed attempt never leaks a stale trail.
    /// The product reads it through [`Page::navigation_record`].
    pub(crate) redirect_chain: Vec<String>,
    /// The generation whose document the live JS realm runs — `init_js`
    /// stamps it on every realm swap, which is the commit point of a
    /// navigation. `realm_epoch < navigation_epoch` means the newest
    /// attempt never committed: the outgoing document is still the active
    /// page and the optimistic `url` must not shadow it (#101).
    pub realm_epoch: u64,
    network_event_counter: u32,
    /// Last [`NetworkEvent::seq`] handed out.
    network_seq: u64,
    /// Passive on_request/on_response callbacks, scoped to this page (upstream
    /// issue #408): they fire for document/subresource fetches this Page makes
    /// and for script-initiated fetch()/XHR in its realm, never for a sibling
    /// page's traffic, and die with the page.
    callbacks: Arc<diting_net::CallbackRegistry>,
    /// Response bodies retained for `get_response_body`, keyed by the
    /// NetworkEvent request id (`{page}.{N}` for page-side fetches,
    /// `fetch-{N}` for script-initiated ones). LRU-bounded by
    /// `response_body_entry_limit` / `response_body_byte_limit`.
    response_bodies: std::collections::HashMap<String, StoredResponseBody>,
    response_body_order: std::collections::VecDeque<String>,
    /// `Network.setBlockedURLs` patterns: a hard block — matched static
    /// subresources (parser-time `<script src>`, `<link rel=stylesheet>`)
    /// fail to load without any client interaction. Deliberately separate
    /// from `Fetch.enable` interception, whose pause flow cannot cover
    /// navigation-time loads on this bridge (see `domains::fetch` docs).
    pub blocked_urls: Vec<String>,
    /// Fetch-domain interception kernel channel. `Some` means armed — the
    /// CDP bridge receives every script-initiated fetch()/XHR as an
    /// `InterceptedRequest` and answers with a resolution. Cleared by
    /// `set_fetch_intercept(None)`.
    intercept_tx: Option<tokio::sync::mpsc::UnboundedSender<diting_js::ops::InterceptedRequest>>,
    // Scripts to execute in the page's JS context BEFORE any of the page's
    // own scripts run — the CDP `Page.addScriptToEvaluateOnNewDocument`
    // contract. Includes `Runtime.addBinding` shims so puppeteer's
    // `exposeFunction` bindings exist before inline `<script>` tags execute.
    //
    // `user_preload_scripts` is the group the caller registered. The product
    // merges host recipes into `preload_scripts` for one navigation; that
    // merge must not be written back here, or the next navigation duplicates
    // the recipes. The session actor used to keep this second list itself.
    user_preload_scripts: Vec<String>,
    preload_scripts: Vec<String>,
    /// The live document is about:blank and its JS realm was never built.
    /// A blank page has no scripts of its own, so the realm is pure cost
    /// until something actually touches JS — the classic shape is
    /// puppeteer's `newPage()` (lands on about:blank) immediately followed
    /// by `goto()`, which would build the blank realm and throw it away
    /// within milliseconds. Deferring skips that construction entirely;
    /// the first eval-side operation materializes the realm through
    /// [`Page::materialize_deferred_realm`] (init_js + preloads, same as
    /// an eager blank). Distinct from "no runtime because the navigation
    /// failed" (#80): deferred is a healthy page that simply has not
    /// needed JS yet.
    deferred_blank_realm: bool,
    /// Page-scoped navigation deadline override. `None` falls back to
    /// `DITING_NAV_TIMEOUT_MS` (default 30s); set via
    /// `set_navigation_timeout` when an automation request carries an
    /// explicit per-call timeout (upstream parity).
    navigation_timeout_ms: Option<u64>,
    /// Storm backoff for the idle pump: when a pump slice ends in a watchdog
    /// termination, the page's runaway loop (e.g. a MutationObserver that
    /// mutates in its own callback) re-queues itself on every subsequent
    /// pump, so re-entering immediately just re-feeds it at 100% CPU for the
    /// session's life. Park until `storm_hot_until` instead; the backoff
    /// doubles per termination (200ms floor, 5s ceiling) and resets on the
    /// first clean idle settle.
    storm_backoff_ms: u64,
    storm_hot_until: Option<tokio::time::Instant>,
    /// #66 busy-storm accounting: (window start, `v8_active_ns` at window
    /// start). Every idle-pump iteration diffs the runtime's cumulative
    /// poll-execution time against this mark; when a trailing window of
    /// `js_busy_limit_secs()` (default 30s) shows ≥40% of wall time spent
    /// inside `poll_event_loop`, the realm froze. Such a burner never fires
    /// a watchdog (every callback stays under budget) and never reports
    /// idle (self-rescheduling driver), so the storm backoff above can't
    /// see it — measured 45-48% of a core for as long as the session lives.
    busy_mark: Option<(tokio::time::Instant, u64)>,
    /// Set when the busy window tripped: the idle pump throttles to bounded
    /// 50ms bursts (≈25% duty) instead of free-running. Commands still run;
    /// a cool burst clears this without a document swap (#240), a session
    /// command arms a full-speed probe window (2026-11-02, tmall publish),
    /// and Navigate/SetContent still rebuilds the realm and clears it.
    busy_frozen: bool,
    /// #66 command probe: armed by `note_command_arrived` while frozen; the
    /// next pump slice lifts the freeze and runs at full speed for a fresh
    /// duty window. A legitimately heavy editor (tmall publish.htm, 6.4s
    /// flush() queues) re-feeds its work queue, so the #240 cool-burst never
    /// fired and the realm sat at 25% for life — its dialogs dispatched but
    /// never rendered (the operator's blank 图片空间 picker). Chrome never
    /// throttles a page the user is driving; neither do we. A still-hot
    /// realm re-freezes one window after the last command; a parked burner
    /// (no commands) keeps the original containment.
    busy_probe_pending: bool,
    /// Token bucket weighting the #66 command probe (11f): a full-speed
    /// probe slice spends wall time 1:1 from this budget, and the budget
    /// re-accrues at only a quarter of wall while the realm stays frozen —
    /// so even a polling client (waitForFunction, an agent loop) can hold
    /// the lift open for at most 25% of wall time aggregate instead of
    /// defeating the containment outright (measured 98% for as long as
    /// the commands keep arriving, the hours-long spin incident). Capped
    /// so a quiet-then-driven realm still gets a solid interactive window.
    probe_budget_ns: u64,
    /// The wall instant the budget last accrued against; `None` until the
    /// first frozen slice (or a fresh trip) starts the meter.
    probe_tick: Option<tokio::time::Instant>,
    /// Wall-clock mark of the last throttled burst start (11f). The frozen
    /// burst/park arithmetic only yields 25% duty when slices COMPLETE —
    /// but a polling client's next command preempts the park, never the
    /// burst (cancellation lands at await points), so slice churn turned
    /// the throttle into a burst-only loop at nearly full duty. Rate-limit
    /// the bursts by wall clock instead: one per 200ms, whatever the
    /// slicing.
    throttled_burst_mark: Option<tokio::time::Instant>,
    /// Progress of the navigation currently in flight on this page, polled
    /// by the `/fetch` starvation checkpoint while `goto` still runs. See
    /// [`NavProgress`].
    nav_progress: std::sync::Arc<NavProgress>,
    pub stealth_client: Option<Arc<StealthHttpClient>>,
}

impl Page {
    pub fn new(id: String, context: Arc<BrowserContext>) -> Self {
        let http_client = context.http_client.clone();
        // Chromium convention: the main frame's frameId == the targetId.
        // Playwright's frame manager looks up the main frame by targetId
        // (via target._targetInfo.targetId), so any divergence here makes
        // Page.getFrameTree return a frame the client cannot match,
        // triggering a Target.closeTarget and "Frame has been detached".
        let frame_id = id.clone();
        let stealth_client = if context.stealth {
            let emulation = context
                .tls_fingerprint
                .as_deref()
                .and_then(diting_net::parse_tls_fingerprint)
                .unwrap_or_else(diting_env::chrome_target::target_tls_profile);
            // Single source of truth for the page's identity: the context's
            // resolved UA drives every surface. Left to itself the stealth
            // client falls back to DITING_UA / a Linux default, which
            // put a Linux User-Agent and a Linux TLS fingerprint on stealth
            // document requests behind a macOS navigator/platform persona —
            // a one-glance bot tell (obscura #481 class).
            let os = diting_net::emulation_os_for_ua(&context.user_agent);
            let client = StealthHttpClient::with_proxy_and_emulation(
                context.cookie_jar.clone(),
                context.proxy_url.as_deref(),
                Some(os),
                emulation,
            );
            // The context's private-network opt-in must reach the stealth
            // transport too, or a context that allows RFC1918 targets opens
            // its document requests on a client that rejects them (the
            // half-threaded-flag shape of obscura#793).
            client.allow_private_network.store(
                context.allow_private_network,
                std::sync::atomic::Ordering::Relaxed,
            );
            if let Ok(mut guard) = client.user_agent.try_write() {
                *guard = context.user_agent.clone();
            }
            Some(Arc::new(client))
        } else {
            None
        };

        Page {
            id,
            frame_id,
            url: None,
            dom: None,
            js: None,
            lifecycle: LifecycleState::Idle,
            http_client,
            context,
            document: Document::new(),
            pending_document: None,
            history: Vec::new(),
            history_index: 0,
            network_events: Vec::new(),
            carried_network_events: Vec::new(),
            carried_network_url: String::new(),
            navigation_epoch: 0,
            redirect_chain: Vec::new(),
            realm_epoch: 0,
            network_event_counter: 0,
            network_seq: 0,
            session_storage: None,
            suspended_console: Vec::new(),
            viewport_override: None,
            layout_policy: LayoutPolicy::Mock,
            dpr_override: None,
            screen_override: None,
            platform_override: None,
            identity_defaults: None,
            emulated_media: None,
            touch_override: None,
            timezone_override: None,
            locale_override: None,
            fp_seed: u64::from_be_bytes(uuid::Uuid::new_v4().into_bytes()[..8].try_into().unwrap()),
            callbacks: Arc::new(diting_net::CallbackRegistry::new()),
            response_bodies: std::collections::HashMap::new(),
            response_body_order: std::collections::VecDeque::new(),
            blocked_urls: Vec::new(),
            intercept_tx: None,
            user_preload_scripts: Vec::new(),
            preload_scripts: Vec::new(),
            deferred_blank_realm: false,
            navigation_timeout_ms: None,
            storm_backoff_ms: 0,
            storm_hot_until: None,
            busy_mark: None,
            busy_frozen: false,
            busy_probe_pending: false,
            probe_budget_ns: 0,
            probe_tick: None,
            throttled_burst_mark: None,
            nav_progress: std::sync::Arc::new(NavProgress::default()),
            stealth_client,
        }
    }

    /// Share the navigation progress cell with an observer while a
    /// navigation runs (the `/fetch` starvation checkpoint clones this
    /// before `goto` and polls it without touching the page).
    pub fn nav_progress(&self) -> std::sync::Arc<NavProgress> {
        std::sync::Arc::clone(&self.nav_progress)
    }

    /// In-flight navigation draft. The first staging write creates it so
    /// the outgoing realm keeps the live document until `init_js` commits.
    fn draft(&mut self) -> Rc<Document> {
        Rc::clone(self.pending_document.get_or_insert_with(Document::new))
    }

    /// Facts the in-flight navigation reads (HTTP Referer). Public `title`
    /// stays on the live document.
    fn facts(&self) -> &Document {
        self.pending_document.as_deref().unwrap_or(&self.document)
    }

    pub fn title(&self) -> String {
        self.document.title()
    }

    /// The committed document's declared content type (response header
    /// value, or empty when none arrived). Callers use it to refuse
    /// non-text documents before rendering — a PDF HTML-parsed into a
    /// DOM is megabytes of binary soup in every text face.
    pub fn content_type(&self) -> String {
        self.document.content_type()
    }

    /// Write the live document, or the in-flight draft when a navigation
    /// is already staging. `navigate_blank`'s caller sets the live title
    /// before any draft exists.
    pub fn set_title(&mut self, title: &str) {
        if let Some(pending) = &self.pending_document {
            pending.set_title(title);
        } else {
            self.document.set_title(title);
        }
    }

    pub fn layout_policy(&self) -> LayoutPolicy {
        self.layout_policy
    }

    /// Record the box-tree posture. A live realm drops its layout memo so
    /// the next geometry read or paint follows the new policy.
    pub fn set_layout_policy(&mut self, policy: LayoutPolicy) {
        self.layout_policy = policy;
        if let Some(js) = &self.js {
            js.set_layout_policy(policy);
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PageError {
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    /// A request that failed in transit, with how. Reads exactly as
    /// [`PageError::NetworkError`] does.
    #[error("Network error: {message}")]
    Transport {
        kind: diting_net::NetFailure,
        message: String,
    },

    /// Not HTTP 3xx redirects (those are `NetError::TooManyRedirects`) —
    /// this counts documents in a JS-initiated navigation chain. The count
    /// includes the requested document, so a limit of N buys N-1
    /// navigations on top. The message must name the layer: operators
    /// debugging "too many redirects" against a server that never 3xx'd
    /// lose hours one layer down (obscura#664).
    #[error(
        "client navigation chain exceeded {0} documents (JS location/form hops, not HTTP redirects) — raise DITING_NAV_CHAIN_LIMIT if this flow is legitimate"
    )]
    TooManyClientNavigations(usize),
}

impl From<NetError> for PageError {
    fn from(e: NetError) -> Self {
        match &e {
            NetError::Transport { kind, .. } => PageError::Transport {
                kind: kind.clone(),
                message: e.to_string(),
            },
            _ => PageError::NetworkError(e.to_string()),
        }
    }
}

impl PageError {
    /// How the request failed, where that is known.
    pub fn failure(&self) -> diting_net::NetFailure {
        match self {
            PageError::Transport { kind, .. } => kind.clone(),
            _ => diting_net::NetFailure::Other,
        }
    }
}
