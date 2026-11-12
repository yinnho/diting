// JsState 脊柱：拦截决议/资源计时/blob 存/吸附位移缓存的状态面 + 会话限流 — split out of the god file (ratchet, #75).
// Pure move: the functions see the same ops::* namespace they grew up in.
use super::*;

/// CDP Fetch-domain resolution: what a client answers a paused request with.
/// Produced by the CDP bridge's `Fetch.continueRequest` / `fulfillRequest` /
/// `failRequest` handlers.
#[derive(Debug)]
pub enum InterceptResolution {
    Continue {
        url: Option<String>,
        method: Option<String>,
        headers: Option<HashMap<String, String>>,
        body: Option<Vec<u8>>,
    },
    Fulfill {
        status: u16,
        headers: HashMap<String, String>,
        body: Vec<u8>,
    },
    Fail {
        reason: String,
    },
}

/// A paused request surfaced to the interception channel (CDP
/// `Fetch.requestPaused` shape). The resolver answers with an
/// `InterceptResolution`. Field readers are the CDP layer, which mints its
/// own pause ids; ops code only moves the struct through the channel.
pub struct InterceptedRequest {
    pub url: String,
    pub method: String,
    pub headers: HashMap<String, String>,
    pub resource_type: String,
    pub resolver: tokio::sync::oneshot::Sender<InterceptResolution>,
}

/// How long a paused fetch()/XHR waits for a CDP client's resolution before
/// falling through to the real request (Continue semantics). The CDP bridge
/// can only drain pauses between commands — a pause created inside a
/// dispatch that itself waits on the fetch (Runtime.evaluate of a fetch
/// expression) is undeliverable until that dispatch ends, so without a
/// bound such a call would hang forever. Real clients resolve in
/// milliseconds; the default only shows up on client-less pages. Tests
/// shorten it via the atomic.
pub static INTERCEPT_RESOLUTION_TIMEOUT_MS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(10_000);

/// (epoch, subtree root, viewport w bits, viewport h bits, run) — the
/// iframe sub-run memo key, see `JsState::iframe_layout_cache`.
#[cfg(feature = "screenshot")]
type IframeLayoutCache =
    std::cell::RefCell<Option<(u64, NodeId, u32, u32, std::rc::Rc<LayoutRun>)>>;

/// One REAL per-request timing sample for the JS `performance` resource
/// buffer (#126): captured around the actual stylesheet/script fetches in
/// the navigation pipeline, epoch-stamped so the JS side can convert into
/// the resource clock (`start - performance.timeOrigin`, clamped ≥0 —
/// pre-init fetches genuinely complete before timeOrigin). Nothing here
/// is fabricated; requests we cannot time simply get no record.
#[derive(Debug, Clone)]
pub struct ResourceTimingRecord {
    /// Absolute request URL (the entry's `name`).
    pub name: String,
    /// W3C initiatorType: "link" (stylesheet) / "script".
    pub initiator: &'static str,
    /// Fetch start as epoch milliseconds (SystemTime), not resource-clock.
    pub start_epoch_ms: f64,
    /// Wire duration in ms, measured with a monotonic `Instant`.
    pub duration_ms: f64,
    /// HTTP status (0 = the fetch failed before a response).
    pub status: u16,
    /// Body byte count (the pipeline stores decoded bodies, so this is
    /// both encodedBodySize and decodedBodySize).
    pub body_size: usize,
    /// Parser/render-blocking posture for `renderBlockingStatus`.
    pub render_blocking: bool,
}

/// Blob URL → (bytes, MIME) mirror shared with the module loader.
pub(crate) type BlobStoreMap = HashMap<String, (Vec<u8>, String)>;
/// (epoch, layout_rev, root_scroll_x/y, viewport_w/h, scroll_gen) key
/// with the memoized sticky shifts — see the `sticky_shift_cache` field.
#[cfg(feature = "screenshot")]
type StickyShiftCache = Option<(
    (u64, u64, f32, f32, f32, f32, u64),
    std::rc::Rc<HashMap<NodeId, [f32; 2]>>,
)>;

pub struct JsState {
    pub dom: Option<DomTree>,
    pub url: String,
    /// WHATWG canonical name of the document's character encoding (e.g.
    /// "UTF-8", "EUC-JP"). Backs `document.characterSet` and the URL query
    /// encoding override for `<a>`/`<area>` hrefs in legacy-charset documents.
    pub encoding: String,
    /// MIME type of the main response (lowercased, parameters stripped), e.g.
    /// "text/plain". Backs `document.contentType`. Empty = the response
    /// carried no Content-Type (JS falls back to URL sniffing).
    pub content_type: String,
    pub title: String,
    /// URL of the document that initiated this document's navigation. Direct
    /// automation navigations leave this empty; document-initiated navigations
    /// set it per the strict-origin-when-cross-origin policy (upstream
    /// edb1785).
    pub referrer: String,
    /// Referrer Policy delivered by the main response's `Referrer-Policy`
    /// header (last valid comma token). Empty = none delivered; the document
    /// policy then comes from `<meta name=referrer>` (see
    /// resolve_referrer_policy_from), falling back to the spec default.
    pub referrer_policy_header: String,
    pub blocked_urls: Vec<String>,
    pub cookie_jar: Option<Arc<CookieJar>>,
    pub http_client: Option<Arc<HttpClient>>,
    pub pending_navigation: Option<(String, String, String)>,
    // Session-history seed (obscura#1105): the JS runtime is rebuilt on every
    // cross-document navigation, so bootstrap re-seeds its history stack from
    // these before any page script runs. `history_pos` is this document's
    // entry index, `history_total` the whole session history length.
    pub history_pos: i32,
    pub history_total: i32,
    // Session-history jump queued by page JS (history.back/forward/go past
    // this document's boundaries) — an absolute entry index into the page's
    // real history, consumed by `Page::process_pending_navigation`.
    pub pending_history_jump: Option<i32>,
    pub intercept_tx: Option<tokio::sync::mpsc::UnboundedSender<InterceptedRequest>>,
    pub intercept_enabled: bool,
    // Queue of (binding_name, payload) calls made by page JS via the
    // `op_binding_called` op. Drained by the CDP layer after each dispatch
    // and emitted as `Runtime.bindingCalled` events.
    pub pending_binding_calls: Vec<(String, String)>,
    // Queue of (level, message) console calls made by page JS via the
    // `op_console_msg` op. Drained by the CDP layer after each dispatch and
    // emitted as `Runtime.consoleAPICalled` events so console output is
    // visible to Playwright/Puppeteer instead of only the tracing log.
    pub pending_console_calls: Vec<(String, String, String)>,
    /// Set when page JS fed `document.write()` — per HTML spec that parse
    /// produces a fresh document whose `load` fires again (Playwright's
    /// setContent waits on it). Drained by the CDP layer after the console
    /// drain so the setContent tag message clears the frame's lifecycle
    /// state before the new load events land.
    pub pending_write_nav: std::cell::Cell<bool>,
    /// Dialog policy applied by `op_dialog` to subsequent window.confirm/
    /// prompt calls (alert has no answer). Default false = auto-dismiss, so
    /// dialogs can never block: the thread that would show the dialog is the
    /// same one running the page script. Flipped via the session_dialog
    /// command; each dialog is recorded into `pending_console_calls` at
    /// level "dialog" so session_console shows what the page asked.
    pub dialog_accept: bool,
    /// Text returned by window.prompt when a dialog is accepted and this is
    /// set; unset falls back to the call's default argument (or "").
    pub dialog_prompt_text: Option<String>,
    /// The document's input stream for `document.write()`, created on the
    /// first call. Why the calls share one parser is in `write_stream`.
    pub(crate) write_stream: std::cell::RefCell<Option<crate::write_stream::DocumentWriteStream>>,
    /// HTML's per-script "already started" flag. This is native page state
    /// rather than wrapper state, because it must survive moves and clones and
    /// because fragment parsing can create nodes before a JS wrapper exists.
    pub(crate) already_started_scripts: RefCell<HashSet<NodeId>>,
    /// Window-global import-map state shared by parser-discovered scripts,
    /// dynamically inserted import maps, and the module loader.
    pub(crate) import_map: Rc<RefCell<crate::import_map::ImportMap>>,
    /// Blob bodies mirrored from JS `URL.createObjectURL` (blob URL →
    /// bytes + MIME), shared with the module loader so `import("blob:…")`
    /// and `<script type=module src="blob:…">` resolve URLs no HTTP client
    /// can fetch. Realm-scoped like the JS-side `__blobObjs`.
    pub(crate) blob_store: Rc<RefCell<BlobStoreMap>>,
    /// In-flight dynamic `<script src>` fetches. Dynamic scripts fetch via the
    /// op-level client, invisible to the page-level http_client's
    /// active_requests() counter — without this, the post-script settle loop
    /// exits at its 500ms deadline while a slow external script is still in
    /// flight, and the CDP consumer snapshots before the script (and its
    /// load event) lands (upstream a6bb741).
    pub(crate) dynamic_script_fetches: std::cell::Cell<u32>,
    /// Passive on_request/on_response registry owned by the page this realm
    /// renders. JS fetch()/XHR requests fire it so page-scoped observers see
    /// script-initiated traffic too (upstream #408). None when the runtime
    /// has no owning page (bare module-loader runtimes).
    pub(crate) callbacks: Option<std::sync::Arc<diting_net::CallbackRegistry>>,
    /// Response bodies retained for script-initiated requests (fetch/XHR),
    /// keyed `fetch-{N}` — the same id the paired `js_network_events` entry
    /// carries, so CDP `Network.getResponseBody` resolves. LRU-bounded by
    /// `response_body_entry_limit` / `response_body_byte_limit`.
    pub(crate) network_response_bodies:
        std::collections::HashMap<String, StoredNetworkResponseBody>,
    pub(crate) network_response_body_order: std::collections::VecDeque<String>,
    pub(crate) network_response_body_counter: u64,
    /// Network events recorded for script-initiated requests (fetch/XHR),
    /// drained into the owning Page's `network_events` by
    /// `Page::sync_js_network_events` so the CDP layer emits
    /// requestWillBeSent / responseReceived for them (upstream #406).
    pub(crate) js_network_events: Vec<JsNetworkEvent>,
    /// Live page-facing WebSocket sockets (registry + channels, op land
    /// owned); see `diting_js::ws`.
    pub(crate) ws_registry: crate::ws::WsRegistry,
    /// Memoized diting-layout run for the live DOM tree, keyed by the
    /// tree's epoch (see DomTree::epoch): element rects, the paint order,
    /// and the cascaded ComputedStyle per element. Filled on the first
    /// `layout_rect` / `paint_order` / `computed_style` op after each
    /// mutation; backs getBoundingClientRect, elementFromPoint and
    /// getComputedStyle.
    #[cfg(feature = "screenshot")] // the layout ops run only in the screenshot-gated pipeline
    pub(super) layout_cache: std::cell::RefCell<Option<(u64, LayoutRun)>>,
    /// Memoized taffy solve for the live DOM tree, keyed by the same epoch
    /// — the geometry half of a layout run (see `SolvedGeometry`). A
    /// paint-only style write (transform/opacity, everything a timeline
    /// seek lands) drops `layout_cache` but keeps this one: the solve
    /// provably ignores those properties, so the next run re-collects
    /// against the cached tree instead of re-solving the full page.
    #[cfg(feature = "screenshot")]
    pub(super) geometry_cache: std::cell::RefCell<Option<(u64, diting_layout::SolvedGeometry)>>,
    /// Memoized layout run for an orphan subtree (a fabricated iframe
    /// document, obscura #976 family), keyed (epoch, subtree root, viewport
    /// w/h bits). Kept apart from `layout_cache` on purpose: a sub-run's
    /// rects answer in the iframe's OWN viewport (the iframe element's
    /// laid-out size on the host page, defaulting to the 300x150 box the
    /// fabricated windows publish), never the main page's.
    #[cfg(feature = "screenshot")]
    pub(super) iframe_layout_cache: IframeLayoutCache,
    /// Full taffy solves this state has run — a test probe, so the
    /// paint-only path can assert it stays flat across seeks.
    #[cfg(feature = "screenshot")]
    #[cfg_attr(not(test), allow(dead_code))]
    pub solves: std::cell::Cell<u64>,
    /// Band paints this state has produced — a test probe, so the video
    /// pump's static-frame reuse can assert held frames skip the paint.
    #[cfg(feature = "screenshot")]
    #[cfg_attr(not(test), allow(dead_code))]
    pub band_paints: std::cell::Cell<u64>,
    /// External stylesheet bodies fetched at navigation (absolute URL →
    /// decoded CSS text). Joined into the cascade by [`layout_run_all`]
    /// and served to the JS side as `document.styleSheets` rule content.
    pub(crate) ext_sheets: std::cell::RefCell<std::collections::HashMap<String, String>>,
    /// Real per-request timings backing
    /// `performance.getEntriesByType('resource')` (#126). Populated by the
    /// navigation pipeline (stylesheet + script fetches); the JS fetch/XHR
    /// shims add their own entries locally in the resource clock.
    pub(crate) resource_timings: std::cell::RefCell<Vec<ResourceTimingRecord>>,
    /// Viewport the layout pipeline should anchor the initial containing
    /// block to, published by the JS persona (`__diting_setPersona`) so
    /// getBoundingClientRect agrees with window.innerWidth/innerHeight.
    /// Defaults to the bootstrap's pre-persona 1920x1000.
    #[cfg(feature = "screenshot")]
    pub viewport: (f32, f32),
    /// Emulated media environment pushed via CDP `Emulation.setEmulatedMedia`
    /// (Playwright's `page.emulateMedia`): the media type + `prefers-*`
    /// overrides the @media cascade re-parses against — the Rust face of the
    /// JS `matchMedia` truth tables (two faces, one truth).
    #[cfg(feature = "screenshot")]
    pub(crate) media_type: diting_css::CssMediaType,
    #[cfg(feature = "screenshot")]
    pub(crate) media_overrides: diting_css::MediaOverrides,
    /// The root scroller's offset, mirrored from the bootstrap's
    /// scrollTop/scrollLeft setters (viewport roots only) so the CDP frame
    /// pump can paint the viewport band without re-serializing the DOM.
    /// Layout is scroll-blind, so this never feeds the layout cache.
    #[cfg(feature = "screenshot")]
    pub scroll_offset: (f32, f32),
    /// CSS animation clock in seconds. `None` = static render, which samples
    /// every animation at its end state (animated SVGs show the finished
    /// diagram, not the blank t=0 frame). When the video pump drives it, the
    /// sampler interpolates keyframes at that time. Sampled props are
    /// paint-only (opacity/transform/stroke-dashoffset), so advancing it only
    /// drops the paint caches, never the taffy solve (#395).
    #[cfg(feature = "screenshot")]
    pub(crate) css_time: Option<f64>,
    /// Max (delay + duration) over every element's computed `animation`,
    /// recomputed each layout run. The video pump uses it as the timeline
    /// extent for CSS-animated pages (no `__timelines` needed).
    #[cfg(feature = "screenshot")]
    pub css_extent: std::cell::Cell<f64>,
    /// Registered property transitions (CSS transitions batch): the JS
    /// face snapshots a tracked-property style write and registers one
    /// entry per (nid, property); a re-trigger on the same pair replaces
    /// the old entry. Sampled at the exit of compute_styles_timed.
    #[cfg(feature = "screenshot")]
    pub(crate) css_transitions: std::cell::RefCell<Vec<diting_css::CssTransition>>,
    /// Layout invalidation revision: bumped wherever `layout_cache` is
    /// dropped. The DomTree epoch is a tree-shape stamp — attribute-level
    /// writes (style/class/attr) clear the cache without allocating nodes,
    /// so the epoch alone never sees them. Consumers deciding "did layout
    /// change" from the epoch (the screencast damage signature) must fold
    /// this rev in, or a style change freezes the cast while layout probes
    /// report fresh geometry (the AginxOS five-mutation report).
    #[cfg(feature = "screenshot")]
    pub layout_rev: std::cell::Cell<u64>,
    /// Process-wide generation stamp of this realm (#189). The tree epoch
    /// and `layout_rev` are per-realm counters that restart at 0 on every
    /// navigation, so two equally static documents can collide on
    /// (epoch, rev, scroll, viewport) — and every damage-signature consumer
    /// (the screencast pump's skip logic, the HTTP /screenshot frame cache)
    /// would then reuse the previous page's frame. This stamp never
    /// repeats within a process.
    #[cfg(feature = "screenshot")]
    pub realm_gen: std::cell::Cell<u64>,
    /// Attribute names referenced by attribute selectors in the last parsed
    /// rule pool, lowercased (obscura#983). Lets a write to a layout-inert
    /// attribute skip the layout-cache drop; `None` until the first layout
    /// run (no caches exist yet, so skipping is free either way).
    #[cfg(feature = "screenshot")]
    pub(crate) attr_selector_names: std::cell::RefCell<Option<std::collections::HashSet<String>>>,
    /// Absolute-URL → fetched image body, filled on demand by the CDP
    /// viewport capture / screencast pump so band paint renders real rasters
    /// instead of placeholders (the outerHTML re-render path pre-fetches;
    /// the live-tree path fills lazily). Bounded entry-wise; every insert
    /// also invalidates `layout_cache` (placeholder boxes vs real intrinsic
    /// sizes can reflow).
    #[cfg(feature = "screenshot")]
    pub(crate) image_bytes: std::cell::RefCell<HashMap<String, std::sync::Arc<Vec<u8>>>>,
    /// Insertion order for `image_bytes` (FIFO eviction at the entry cap —
    /// clearing wholesale would thrash pages with more images than the cap).
    #[cfg(feature = "screenshot")]
    pub(crate) image_order: std::cell::RefCell<std::collections::VecDeque<String>>,
    /// Canvas playback bitmaps (#210), keyed by the canvas element's nid.
    /// The 2D buffer lives in the JS realm, so the host pushes a snapshot in
    /// before it renders (`op_canvas_store`); `layout_collect_with_images`
    /// layers these over the solve's `<img>` map and the replaced-element
    /// branch paints them like any other bitmap. A store that changes pixels
    /// goes through `drop_paint_only` — canvas draw is a paint-only
    /// invalidation (geometry never moves), which also moves `layout_rev` so
    /// the damage signature lets the screencast/screenshot caches see it.
    #[cfg(feature = "screenshot")]
    pub canvas_images: std::cell::RefCell<HashMap<NodeId, diting_layout::DecodedImage>>,
    /// Sticky (v2): per-node scroll-dependent shift map ([dx, dy] in
    /// document space), memoized per (epoch, layout_rev, root scroll,
    /// viewport, scroll_gen) — scroll and viewport moves don't bump the
    /// epoch, layout_rev covers attr-level writes the epoch can't see (the
    /// same pairing as the screencast damage signature), and scroll_gen
    /// covers element-scroller writes, which bump neither. Cleared
    /// alongside the layout caches in drop_layout. See `sticky_shifts`.
    #[cfg(feature = "screenshot")]
    pub(crate) sticky_shift_cache: std::cell::RefCell<StickyShiftCache>,
}

/// Source of `JsState::realm_gen` (#189): a fresh stamp per realm, never
/// reused within a process.
#[cfg(feature = "screenshot")]
static REALM_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl Default for JsState {
    fn default() -> Self {
        Self::new()
    }
}

/// A script-initiated request as a CDP-shaped network event. Static
/// navigation subresources go through Page::record_network_event; this is
/// the parallel channel for script-initiated requests, which run in the V8
/// op layer and would otherwise never surface as Network events (#406).
#[cfg_attr(not(test), allow(dead_code))] // tests assert every field; /network endpoint is the pending reader
#[derive(Debug, Clone)]
pub struct JsNetworkEvent {
    /// Matches the `fetch-{N}` id under which the body is stored, so CDP
    /// Network.getResponseBody resolves for the same request.
    pub request_id: String,
    pub url: String,
    pub method: String,
    pub status: u16,
    pub response_headers: HashMap<String, String>,
    /// The outbound header set of the final hop as the walk shaped it (#97):
    /// script customs (x-s, x-s-common …) plus the browser-default set the
    /// engine joined in. Lowercased. Failed rows leave it empty — a failure
    /// is often blocked before any wire (SSRF/CORS-refused at preflight).
    pub request_headers: HashMap<String, String>,
    pub body_size: usize,
    pub timestamp: f64,
    /// Why a `status: 0` entry never produced a servable response — SSRF
    /// block, CORS refusal, transport failure. Chrome DevTools annotates
    /// failed rows with the reason; without it a page whose API calls all
    /// die here reads as "never issued a request" (the taobao shop-SPA
    /// report chased exactly that ghost).
    pub error: Option<String>,
    /// Where the request actually landed when redirects carried it (#203);
    /// equal to `url` when it went straight through. `url` stays the
    /// REQUESTED address on both success and failure rows so a bounced
    /// submit can still be found by its original URL.
    pub final_url: String,
    /// Hops followed in order, each with the 3xx status (#203).
    pub redirects: Vec<diting_net::RedirectHop>,
    /// Request dispatch, unix seconds (#229). `timestamp` is response
    /// completion (walk return), so the pair reconstructs which request
    /// left first — completion order alone cannot.
    pub started: f64,
}

/// A response body retained for `Network.getResponseBody`. Bodies are
/// classified with the Chromium DevTools policy (see
/// `diting_net::decode_devtools_body`): replacement-free text is stored as a
/// string (`base64_encoded = false`, declared-GBK pages included, matching
/// Chrome); opaque or undecodable bodies are stored base64.
#[derive(Debug, Clone)]
pub struct StoredNetworkResponseBody {
    pub body: String,
    pub base64_encoded: bool,
}

pub(super) fn response_body_entry_limit() -> usize {
    std::env::var("DITING_NETWORK_BODY_BUFFER_ENTRIES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(128)
}

pub(super) fn response_body_byte_limit() -> usize {
    std::env::var("DITING_NETWORK_BODY_BUFFER_BYTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2 * 1024 * 1024)
}

/// Hard ceiling on a single JS fetch()/XHR response body (upstream #581).
/// Unlike the two limits above — which bound what is *retained* for CDP —
/// this one bounds the initial allocation itself: page JS can fetch() any
/// URL, so a server streaming gigabytes must fail as a network error
/// instead of OOMing the process (plus the UTF-8 and base64 copies that
/// follow the raw buffer). Bulk transfer is the streaming download layer's
/// job, not page fetch(). `0` disables the cap, matching the 0-disables
/// convention of the two limits above. Shared with the ES module loader
/// (obscura #849) so page-controlled remote code has one size policy.
pub(crate) fn fetch_body_byte_limit() -> usize {
    std::env::var("DITING_FETCH_BODY_LIMIT")
        .ok()
        .and_then(|v| v.parse().ok())
        .map(|v| if v == 0 { usize::MAX } else { v })
        .unwrap_or(64 * 1024 * 1024)
}

impl JsState {
    /// Drop the memoized layout run and stamp the invalidation revision.
    /// Every cache-drop site must go through here — a bare cache drop hides
    /// the invalidation from epoch-keyed consumers like the screencast
    /// pump's damage signature.
    #[cfg(feature = "screenshot")]
    pub(crate) fn drop_layout(&self) {
        *self.layout_cache.borrow_mut() = None;
        *self.geometry_cache.borrow_mut() = None;
        // A style/class write inside a fabricated iframe document doesn't
        // bump the tree epoch, so the (epoch, root) key can't see it — the
        // shared drop points are the only reliable invalidation.
        *self.iframe_layout_cache.borrow_mut() = None;
        // Sticky shifts key on (epoch, layout_rev, scroll, viewport); the
        // rev bump below kills the entry either way — clear for tidiness.
        *self.sticky_shift_cache.borrow_mut() = None;
        self.layout_rev.set(self.layout_rev.get().wrapping_add(1));
    }

    /// The paint-only invalidation (#395): a transform/opacity style write
    /// changes pixels but not geometry — the taffy solve provably ignores
    /// both (`to_taffy_style` maps only geometry families), so the solve
    /// cache survives and only the collected half re-runs. `layout_rev`
    /// still moves: the pixels DID change, and damage-signature consumers
    /// key on it.
    #[cfg(feature = "screenshot")]
    pub(crate) fn drop_paint_only(&self) {
        *self.layout_cache.borrow_mut() = None;
        *self.iframe_layout_cache.borrow_mut() = None;
        self.layout_rev.set(self.layout_rev.get().wrapping_add(1));
    }

    /// #278 on-demand layout: clear the same caches [`Self::drop_layout`]
    /// clears, WITHOUT moving the invalidation rev. A release changes no
    /// pixels — the next solve rebuilds byte-identical output from the
    /// same DOM and viewport — so damage-signature consumers (the band
    /// frame cache, #189) must keep hitting across it. Every
    /// [`Self::drop_layout`] caller is a real content change; this one's
    /// only caller is the session actor's command boundary.
    #[cfg(feature = "screenshot")]
    pub fn release_layout_quiet(&self) {
        *self.layout_cache.borrow_mut() = None;
        *self.geometry_cache.borrow_mut() = None;
        *self.iframe_layout_cache.borrow_mut() = None;
        *self.sticky_shift_cache.borrow_mut() = None;
    }

    pub fn new() -> Self {
        JsState {
            dom: None,
            url: "about:blank".to_string(),
            encoding: "UTF-8".to_string(),
            content_type: String::new(),
            title: String::new(),
            referrer: String::new(),
            referrer_policy_header: String::new(),
            blocked_urls: Vec::new(),
            cookie_jar: None,
            http_client: None,
            pending_navigation: None,
            history_pos: 0,
            history_total: 1,
            pending_history_jump: None,
            intercept_tx: None,
            intercept_enabled: false,
            pending_binding_calls: Vec::new(),
            pending_console_calls: Vec::new(),
            pending_write_nav: std::cell::Cell::new(false),
            dialog_accept: false,
            dialog_prompt_text: None,
            write_stream: std::cell::RefCell::new(None),
            already_started_scripts: RefCell::new(HashSet::new()),
            import_map: Rc::new(RefCell::new(crate::import_map::ImportMap::default())),
            blob_store: Rc::new(RefCell::new(HashMap::new())),
            dynamic_script_fetches: std::cell::Cell::new(0),
            callbacks: None,
            network_response_bodies: std::collections::HashMap::new(),
            network_response_body_order: std::collections::VecDeque::new(),
            network_response_body_counter: 0,
            js_network_events: Vec::new(),
            ws_registry: Default::default(),
            // Memoized diting-layout rects for the live DOM tree, keyed by
            // the tree's epoch (see DomTree::epoch). Filled on the first
            // `layout_rect` op after each mutation; backs getBoundingClientRect.
            #[cfg(feature = "screenshot")]
            layout_cache: std::cell::RefCell::new(None),
            #[cfg(feature = "screenshot")]
            geometry_cache: std::cell::RefCell::new(None),
            #[cfg(feature = "screenshot")]
            iframe_layout_cache: std::cell::RefCell::new(None),
            #[cfg(feature = "screenshot")]
            solves: std::cell::Cell::new(0),
            #[cfg(feature = "screenshot")]
            band_paints: std::cell::Cell::new(0),
            #[cfg(feature = "screenshot")]
            viewport: (1920.0, 1000.0),
            #[cfg(feature = "screenshot")]
            media_type: diting_css::CssMediaType::Screen,
            #[cfg(feature = "screenshot")]
            media_overrides: diting_css::MediaOverrides::default(),
            #[cfg(feature = "screenshot")]
            scroll_offset: (0.0, 0.0),
            #[cfg(feature = "screenshot")]
            sticky_shift_cache: std::cell::RefCell::new(None),
            #[cfg(feature = "screenshot")]
            css_time: None,
            #[cfg(feature = "screenshot")]
            css_extent: std::cell::Cell::new(0.0),
            #[cfg(feature = "screenshot")]
            css_transitions: std::cell::RefCell::new(Vec::new()),
            #[cfg(feature = "screenshot")]
            layout_rev: std::cell::Cell::new(0),
            #[cfg(feature = "screenshot")]
            realm_gen: std::cell::Cell::new(
                REALM_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            ),
            #[cfg(feature = "screenshot")]
            attr_selector_names: std::cell::RefCell::new(None),
            #[cfg(feature = "screenshot")]
            image_bytes: std::cell::RefCell::new(HashMap::new()),
            #[cfg(feature = "screenshot")]
            image_order: std::cell::RefCell::new(std::collections::VecDeque::new()),
            #[cfg(feature = "screenshot")]
            canvas_images: std::cell::RefCell::new(HashMap::new()),
            // External stylesheet bodies fetched at navigation, keyed by
            // absolute URL. The layout run joins them with the live <style>
            // blocks in document order — without this table an author sheet
            // that arrives over <link> never reaches the cascade, so
            // getComputedStyle answers initial values and every rect-based
            // consumer (gBCR, elementFromPoint) sees unstyled geometry.
            ext_sheets: std::cell::RefCell::new(std::collections::HashMap::new()),
            resource_timings: std::cell::RefCell::new(Vec::new()),
        }
    }
}

pub type SharedState = Rc<RefCell<JsState>>;
