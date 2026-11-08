use std::path::PathBuf;

// Snapshot scripts, in load order (table order = execution order). All run
// in one realm inside the snapshot runtime; each script only declares
// bindings at top level and touches earlier scripts' bindings at call time.
const SNAPSHOT_SCRIPTS: &[(&str, &str)] = &[
    // 10d host flip: the Deno shell. The retired deno_core snapshot carried
    // the whole Deno.core object (promise ring, tick, encode/decode); the
    // only face our scripts ever read is `Deno.core.ops` — bootstrap.js
    // captures it at line 110 as a top-level const (`const _OPS =
    // Deno.core.ops`) that every later script shares through the global
    // lexical environment. The runtime installs the ops INTO that same
    // object (Vm::install_ops_under), never onto globalThis — op names as
    // window properties would change the fingerprint face. __diting_init
    // deletes the `Deno` property per page exactly as before (anti-bot
    // law: a Deno prop on window is the canonical deno-runtime marker).
    // Must run FIRST: bootstrap's const capture is the first Deno read.
    // 10d rejection face: bootstrap.js:10819+ registers its window-event
    // handlers through Deno.core.set{Unhandled,Handled}PromiseRejectionHandler
    // at bake time; the PRELUDE's registration functions store the closures
    // into heap globals (baked, restore-free) where Vm::dispatch_rejections
    // reads them live each round.
    ("deno_shell", "globalThis.Deno = { core: { ops: {}, setUnhandledPromiseRejectionHandler: __diting_set_unhandled_rejection_handler, setHandledPromiseRejectionHandler: __diting_set_handled_rejection_handler } };"),
    // Main script: the bootstrap god-file-turned-root.
    ("bootstrap.js", include_str!("js/bootstrap.js")),
    // god-file split (刀89, dom op data tables): the op channel's five data
    // tables (epoch-bump set, #225 tree-edge memo pair, receiver-validation
    // id/arg tables). Consumers (bootstrap's _domRaw/_dom, domcore's
    // parentNode getter) read at call time through d()-published bindings;
    // the Set/Map bindings are never reassigned so value publish is correct.
    // Zero load-time callers exist anywhere, so the slot is free; it sits
    // right after bootstrap today, but safety comes from the analyzer's
    // forward-load ratchet (forwardLoadRefs = 0), not from adjacency.
    ("dom_op_tables.js", include_str!("js/dom_op_tables.js")),
    // god-file split (刀90, shallow clone): _shallowCloneNode — the one-node
    // clone primitive behind domcore's cloneNode deep path. Free names are
    // only document/node members read at call time; consumers (domcore
    // cloneNode) call through the d()-published binding. Any slot safe.
    ("shallow_clone.js", include_str!("js/shallow_clone.js")),
    // god-file split (刀91, select label mirror): _mirrorSelectLabel — the
    // live_value mirror write the form paint batch reads. Free names are
    // member reads plus _domRaw, all call-time through the d()-published
    // binding; consumers (domcore, htmlelem) call at call time. Any slot
    // safe. Host pin flipped here from bootstrap in this knife.
    ("mirror_select.js", include_str!("js/mirror_select.js")),
    // god-file split (刀92, snapshot sweeps): the fp-diff-1014 P0 name sweep
    // (anonymous function-valued globals named; detached-EventTarget wrap)
    // + the frame-copyable globals builder, load-time calls intact. Slot is
    // free: interface_sweep (the LAST snapshot module) re-runs both, and the
    // only load-time-sensitive consumer (navigation's addEventListener
    // capture) loads after it. _markNative arrives via the bootstrap hidden-
    // global bridge captured at this file's head (member form, ratchet-flat).
    ("snapshot_sweeps.js", include_str!("js/snapshot_sweeps.js")),
    // god-file split (刀83, event-state facade + native trio): _eventState
    // capture off the engine-installed bridge global (deleted in the same
    // breath) + the M3-B1 addEventListener/removeEventListener/dispatchEvent
    // install. Load-time capture semantics: the capture+delete must run
    // before any file that reads the captured names loads (the trio install
    // is what event_classes/pointer_touch extend at their later slots), and
    // the trio reads only the host EventTarget (present before any JS). The
    // analyzer's forward-load ratchet enforces that machine-side — no
    // adjacency to bootstrap is required. The d() row migrated with the
    // declaration; consumers (domcore, platform_bundle) read at call time.
    ("event_facade.js", include_str!("js/event_facade.js")),
    // god-file split (刀84, document membership + shadow host gate):
    // _nodeInDocument (the 200-step shadow-including document climb behind
    // animation/lifecycle gating and named-scan reindex) + the attachShadow
    // host-name allowlist (__diting_shadowHostNames; domcore reads the Set
    // at call time). Pure function/data, zero load-time reads; the d() row
    // migrated with the declaration. Early slot keeps the load window tight.
    ("document_membership.js", include_str!("js/document_membership.js")),
    // god-file split (刀85, window flags + zero constants):
    // crossOriginIsolated flag (fp-diff-1014 P1; the per-realm SAB
    // intrinsic delete stays in __diting_init — see the block comment) +
    // the screen/scroll zero data constants (scroll_family later replaces
    // the scroll quartet with accessors — this slot loads before it, so
    // the data-then-accessor order holds). Pure installs, zero load-time
    // cross-file reads. reportError stays in bootstrap: its load-time
    // mark call here would cost two cross-file load-time names.
    // (Name checked against this registry — 刀46 already owns window_misc.)
    ("window_flags.js", include_str!("js/window_flags.js")),
    // god-file split (刀86, dom JSON face): _domParse — the JSON.parse
    // wrapper over the _dom op channel (parse-or-null). Free _dom resolves
    // at call time through the d()-published global; consumers (domcore,
    // animation_lifecycle, cookie_gate) are all call-time. Any slot safe.
    ("dom_parse.js", include_str!("js/dom_parse.js")),
    // god-file split (刀79, window identity): window/self/top/parent/frames
    // aliases + frameElement/length. Pure installs, zero reads, and no
    // location READERS exist anywhere at load — so the 刀74 belt argument
    // survives whatever slot this file takes; the analyzer's forward-load
    // ratchet holds that as files move.
    ("window_identity.js", include_str!("js/window_identity.js")),
    // god-file split (刀74, Location face): _resolveUrl + __virtualUrl +
    // __currentUrl + the __locParts memo + the location object + the
    // unforgeable accessor. 刀74 人肉审: the only load-time snapshot of
    // G.location is method_ladders at its later slot; every other consumer
    // is runtime — the load window is empty by construction, and the
    // analyzer's forward-load ratchet keeps it that way.
    ("location_face.js", include_str!("js/location_face.js")),
    // god-file split (刀82, Secure Contexts): the isSecureContext accessor,
    // computed per access off __currentUrl() (location_face, earlier slot,
    // call-time). Tmall AWSC walks its degraded branch when missing — any
    // load slot precedes page scripts.
    ("secure_context.js", include_str!("js/secure_context.js")),
    // god-file split (刀77, uncaught-error reporting): __diting_errors sink +
    // default onerror capture + reportUncaught/reportUncaughtError pipeline.
    // All consumers runtime (js member reads, Rust string bridge, console
    // face lazy guard); after location_face to keep 刀74's first-slot law.
    ("error_report.js", include_str!("js/error_report.js")),
    // god-file split (刀59, console argument formatter): _fmtVal + _peek +
    // the _FMT_* clamps — pure helpers, zero installs. _consoleFn calls
    // them at runtime via the published hidden globals, and any load-time
    // console use in later files resolves here whatever the slot order.
    ("console_format.js", include_str!("js/console_format.js")),
    // god-file split (刀78, console face): _consoleFn + the globalThis.console
    // install (createTask/context/assert). Consumes console_format's
    // formatters one slot earlier at call time; the seven marked members
    // self-mark here (marking travels — bootstrap would mark the host console).
    ("console_face.js", include_str!("js/console_face.js")),
    // god-file split (刀65, event subclass family): the _evx WeakMap state
    // scheme, the deno_core unhandledrejection/rejectionhandled routing
    // IIFE, and the subclass ladder (UIEvent brood + plain Event brood;
    // the validating PromiseRejectionEvent install overwrites the routing
    // block's basic one — both kept verbatim). Extends host-native Event/
    // EventTarget at load; early slot so later files extending these
    // classes (pointer_touch) and runtime dispatch all resolve.
    ("event_classes.js", include_str!("js/event_classes.js")),
    // god-file split (刀66, abort family): AbortSignal/AbortController —
    // abort reasons + event + static abort()/timeout()/any()/throwIfAborted
    // replacing the dead stub; construction gate key is module-local.
    // Extends host-native EventTarget; consumers (fetch family, worker,
    // navigation) runtime or string tables — early slot for uniformity.
    // god-file split (刀80, lift-slot channel): the 刀84 hidden-slot channel
    // for sealed prototype accessors — 20 consumer files, all runtime
    // (constructor/method bodies); just before abort_signals, the first
    // consumer in load order.
    ("lift_slot.js", include_str!("js/lift_slot.js")),
    // god-file split (刀81, Error face): name-descriptor normalization (7
    // builtin prototypes, Chrome -cw face) + the lazy stack cache. Lexical
    // to the file; descriptor capture is load-time but only touches
    // Error.prototype, untouched by any earlier slot.
    ("error_face.js", include_str!("js/error_face.js")),
    ("abort_signals.js", include_str!("js/abort_signals.js")),
    // god-file split (刀3, DOM core): Node/Element/Document/ShadowRoot/Attr —
    // the DOM core family bootstrap.js carried wholesale. Straight after it:
    // everything later (canvas, sweeps) needs these classes in scope.
    ("domcore.js", include_str!("js/domcore.js")),
    // god-file split (刀3, HTML elements): the interface mints, SVG family,
    // selection mirror, Image/Audio/Option factories and the body-window
    // on* cluster. After domcore.js: its mints extend the core classes.
    ("htmlelem.js", include_str!("js/htmlelem.js")),
    // god-file split (刀4, serializer family): the XML well-formedness gate,
    // _xmlBuildTree (domparser.js's XML tree builder) and XMLSerializer.
    // Before domparser.js (call-time) and web_api_gaps.js (load-time member
    // read on XMLSerializer.prototype).
    ("serializer.js", include_str!("js/serializer.js")),
    // god-file split (刀5, streams family): _rsStream + the ReadableStream /
    // WritableStream / TransformStream installs and their controller/reader
    // faces. Everyone downstream (TextEncoderStream in bootstrap, fetch.js,
    // interface_sweep.js) touches these at call time or via globalThis reads.
    ("streams.js", include_str!("js/streams.js")),
    // god-file split (刀15, BroadcastChannel family): the same-origin message
    // bus — registry-backed class with real MessageEvent dispatch, acc/accRo
    // accessor installs on the 刀6 alias device (publishes pinned to
    // bootstrap). Sits before websocket to keep original statement order.
    ("broadcastchannel.js", include_str!("js/broadcastchannel.js")),
    // god-file split (刀16, Selection/Range fallback stubs): the two guarded
    // installs load ahead of the real selection implementation (刀73) so the
    // platform phase still wins when it runs. Right after broadcastchannel
    // to keep original statement order.
    ("selection_range.js", include_str!("js/selection_range.js")),
    // god-file split (刀17, guard-stub tail pair): ServiceWorkerContainer +
    // URLPattern fallback stubs — same guarded pattern as 刀16; the real
    // URLPattern (platform/urlpattern.impl.js) still loads later and wins.
    ("swc_urlpattern.js", include_str!("js/swc_urlpattern.js")),
    // god-file split (刀6, WebSocket family): the guarded install — class over
    // the op_ws_* family, ready-state constants, 刀84 __accRo/__acc accessor
    // installs. No load-time consumers anywhere ahead of it.
    ("websocket.js", include_str!("js/websocket.js")),
    // god-file split (刀8, getComputedStyle family): the Chrome-145 longhand
    // table, camelCase cache, snapshot caches (element + pseudo), the gCS
    // install with its Proxy-backed declaration. All cross-file traffic is
    // call-time; the mark-list entry moved with the install. Bootstrap keeps
    // the epoch let (the mutation choke point) and publishes _ditingEpochNow
    // so the snapshot staleness read stays live.
    ("computed_style.js", include_str!("js/computed_style.js")),
    // god-file split (刀9, image-bitmap family): the guarded installs for
    // ImageData / OffscreenCanvas / ImageBitmap + createImageBitmap (header-
    // parsed dims on blobs, live-dims passthrough for canvas/Image sources).
    // Sole installer; no mark entries; all consumers are call-time or sit
    // later in the order (worker.js realm capture, videoframe error strings).
    ("image_bitmap.js", include_str!("js/image_bitmap.js")),
    // god-file split (刀10, FileReader family): the guarded install —
    // readAs* over the blob store with ready-state/on* accessor fills
    // (刀84 __accRo/__acc). extends EventTarget is a native-name ref
    // (audio/worker/indexeddb precedent), not a cross-file pair.
    ("filereader.js", include_str!("js/filereader.js")),
    // god-file split (刀11, EventSource family): the guarded SSE stub —
    // fires `open` after a microtask (docs-site HMR shape), accessor fills
    // on the 刀6 alias device.
    ("eventsource.js", include_str!("js/eventsource.js")),
    // god-file split (刀12, SharedWorker family): one script instance per
    // (url, name) with real MessagePort pairs (#213), the SharedWorkerGlobal-
    // Scope shape, inline _markNative — the serializer head-alias device.
    ("sharedworker.js", include_str!("js/sharedworker.js")),
    // god-file split (刀19, UA persona helpers): navigator.platform /
    // UA-CH brand grease / per-major full-build table, all derived from the
    // HTTP-layer UA. Pure functions + consts, no installs; consumers
    // (uadata.js, navigator_persona.js, bootstrap's font metrics) read them
    // at call time through the generated d() publishes. Loads before
    // plugins.js to keep the original bootstrap statement order.
    ("ua_helpers.js", include_str!("js/ua_helpers.js")),
    // god-file split (刀21, font-presence tables + layout measurement):
    // the per-OS installed-font tables + the font-box/local-box/layout-box
    // measurement primitives + the offsetParent walk. Everything element
    // geometry emulation reads at call time; external deps are the published
    // _domRaw bridge and ua_helpers' __ditingPlatformFromUA; the four
    // internal tables stay file-private. NOT the 刀97 generated advance
    // tables (that artifact keeps the font_metrics.js name, loaded later).
    // Original statement order puts this right after the UA helpers and
    // before NetworkInformation.
    ("font_presence.js", include_str!("js/font_presence.js")),
    // god-file split (刀20, NetworkInformation): the navigator.connection
    // class (illegal constructor + the five getters), its _markNative call,
    // the EventTarget chain relink and the WebIDL reorder — both migrated
    // from bootstrap's DOM-cluster tail / bottom try-cluster at the carve,
    // reached via the 刀13 head-alias device (navigator_persona.js stays the
    // last declarer, so the alias publishes do not flip). Loads before
    // plugins.js to keep the original bootstrap statement order.
    ("networkinformation.js", include_str!("js/networkinformation.js")),
    // god-file split (刀22, bchkAPIs faces): the batch166 anti-bot probe
    // surface — illegal-constructor interface faces for `'X' in window`
    // checks + the Object.create'd singletons + the chrome object + the
    // 刀69 WebGL stubs. BatteryManager's EventTarget relink migrated here
    // from bootstrap's DOM-cluster tail (member form). __diting_fragmentDirective
    // is exports-extra-pinned for bootstrap's window-face getter;
    // __diting_launchQueue has zero readers and stays file-private.
    ("bchk_apis.js", include_str!("js/bchk_apis.js")),
    // god-file split (刀23, Screen / viewport family): the Screen face +
    // hidden __scrStore + screenApply, VisualViewport + factory + the
    // installed visualViewport/devicePixelRatio, the __vp* vars (persona
    // refresh writes them from bootstrap through a() live accessors — 刀8
    // live-binding law) + installViewportAccessors + scroll/page offsets.
    // Screen.prototype's onchange accessor + WebIDL reorder migrated from
    // bootstrap's bottom try-cluster (member form).
    ("screen_viewport.js", include_str!("js/screen_viewport.js")),
    // god-file split (刀24, fetch helper belt): byte codecs (base64 both
    // ways, binary string, concat, ArrayBuffer view) + the wasm streaming
    // fallback (+ its load-time install) + the WHATWG data: URL processor +
    // percent-decode + the AbortSignal race + __recordFetchTiming + the
    // guarded Headers polyfill. The two CDP intercept flags stay globalThis
    // member installs. Runtime-only cross-file consumers (fetch.js, xhr.js,
    // websocket.js, filereader.js, bootstrap's atob/init legs) — order-free;
    // _percentDecodeToBytes is file-private (in-zone sole consumer).
    ("fetch_helpers.js", include_str!("js/fetch_helpers.js")),
    // god-file split (刀25, History API family): the functional History
    // IIFE — session-history stack + push/replace/go/back/forward +
    // popstate/hashchange dispatch + the __diting_history CDP accessor.
    // MUST stay before navigation.js (captures history.pushState at load)
    // and interface_sweep.js (reads the singleton at load); both load
    // later in this order. __virtualUrl stays bootstrap's — Location and
    // __diting_init touch it via member form.
    ("history_api.js", include_str!("js/history_api.js")),
    // god-file split (刀26, structuredClone family): the HTML structured-
    // clone serializer (#389: buffers/typed arrays survive postMessage)
    // + the guarded globalThis.structuredClone install. Self-contained —
    // free ids are builtins only; callers go through the installed global.
    ("structured_clone.js", include_str!("js/structured_clone.js")),
    // god-file split (刀27, Intl/Date persona family): TZ/locale derivation
    // + Intl.DateTimeFormat proxy + Date prototype wall-clock wraps + the
    // __ditingNativeOf disguise map + the Function.prototype.toString
    // proxy. The map is internal to this belt (_markNative's registry is
    // _nativeFnAdd, a different mechanism). Load-time installs are member
    // form; navigator_persona reads __ditingLangList at runtime.
    ("intl_date_persona.js", include_str!("js/intl_date_persona.js")),
    // god-file split (刀28, CSS property-name tables): camel/kebab codecs +
    // the standard camelCase property list + Set form + hidden
    // __diting_cssNames install. platform_bundle.js bare-reads
    // __diting_cssNames at ITS load — this file must precede it.
    ("css_names.js", include_str!("js/css_names.js")),
    // god-file split (刀68, dataset + style proxy): DOMStringMap (dataset
    // backing face, construction-key gated, synthetic — no native marking)
    // + _styleProxy (CSSStyleDeclaration-style proxy over a declaration;
    // trap bodies read css_names tables at runtime — CSS cluster slot).
    // Consumers (domcore dataset/style accessors, css_rules decl factory)
    // are runtime reads; the single globalThis install travels with the
    // class.
    ("dataset_style.js", include_str!("js/dataset_style.js")),
    // god-file split (刀29, CSS animation/transition lifecycle): the
    // timer-armed animation event engine + transition watcher family,
    // state included. domcore.js / event_hooks.js call the schedulers at
    // runtime from mutation hooks — order-free.
    ("animation_lifecycle.js", include_str!("js/animation_lifecycle.js")),
    // god-file split (刀30, custom elements): CustomElementRegistry class +
    // singleton install (constructor calls __hideOwn at load — published by
    // bootstrap, which loads earlier) + the dynamic upgrade/disconnect walks.
    // Cross-file consumers all RUNTIME: htmlelem.js Element constructor reads
    // the live __ceUpgradeTarget (a() accessor), domcore.js insertion/removal
    // paths and bootstrap's __prepareInsertedSubtree call the walks, ce_attr
    // plugin module reached via optional __diting_ce* members. Order-free.
    // god-file split (刀72, ElementInternals): form-associated custom
    // element internals — validity/state faces + the 刀58 ARIAMixin belt
    // (internal-only store, two independent stores from the element's
    // own face). Synthetic: plain global install, no d() row, no native
    // marking. attachInternals bodies consume at runtime. Custom-element
    // cluster slot.
    ("element_internals.js", include_str!("js/element_internals.js")),
    ("custom_elements.js", include_str!("js/custom_elements.js")),
    // god-file split (刀31, window named access #48): the _named* family +
    // the load-time _namedHolder IIFE (parks the gate proxy under
    // globalThis; window_extras.js's EventTarget rewire reads
    // __diting_namedHolder at its later load). Named getters only appear
    // at runtime, so the gate is a no-op layer during every platform
    // file's load — order-free. domcore.js calls the scan/unscan walkers
    // at runtime; Rust entry points call globalThis._namedBoot
    // (analyzer-blind — pinned in exports-extra so the publish stays).
    ("named_access.js", include_str!("js/named_access.js")),
    // god-file split (刀32, insertion preparation): the dynamic-insertion
    // pipeline (subtree walk -> CE upgrades / script prep / stylesheet +
    // style linking), <style>/<link> load-event faces, initial-parse
    // stylesheet pass, parser-path script load/error events
    // (globalThis.__fireParserScriptEvent — Rust scripts.rs, member
    // install). domcore.js's hooks call every face at runtime;
    // __prepareInitialStylesheets is also called via injected bare
    // identifier (analyzer-blind — pinned in exports-extra).
    ("insertion_prep.js", include_str!("js/insertion_prep.js")),
    // god-file split (刀33, btoa/atob): Latin1 binary-string codecs per the
    // HTML spec, guarded installs (realm natives win). The bootstrap
    // load-time _markNative sweep row for these two migrated with the
    // family (刀22 Notification precedent) — this file marks its own
    // installs at its load tail.
    ("btoa_atob.js", include_str!("js/btoa_atob.js")),
    // god-file split (刀34, CSSStyleDeclaration): style-attribute parse/
    // serialize pair + the illegal-constructor class (mint door _newCssDecl)
    // + native-proto marking. computed_style.js / domcore.js call
    // _newCssDecl at runtime; the class name's other-file footprint is
    // string tables only. Load-time marking via the streams.js member
    // alias (globalThis._markNativeProto) — a bare load call would trip
    // the crossFileLoadTimeNames ratchet.
    ("css_style_decl.js", include_str!("js/css_style_decl.js")),
    // god-file split (刀35, CSS static face): CSS.supports (Rust @supports
    // evaluator leg) + CSS.escape + the 刀88 typed-om unit family
    // (CSSUnitValue ctor + 64 CSS.px-style factories + unit table). Same-
    // file member installs; css_register.js (#263, later) adds
    // registerProperty onto the namespace, typedom_math.js (#336) reads
    // the members inside a guarded IIFE at its load. Units/reg/factory
    // tables are file-private.
    ("css_static_face.js", include_str!("js/css_static_face.js")),
    // god-file split (刀36, iframe registration): the browsing-context
    // registry — insertion-time registration (numeric window indexing in
    // tree order), srcdoc script runs as a document load, insertion starts
    // src/srcdoc navigation. domcore.js and iframe_realm.js (owner of
    // __iframeBlankLoad) call the walkers at runtime; _loadIframeSrc is an
    // instance member. Registry is file-private.
    ("iframe_registry.js", include_str!("js/iframe_registry.js")),
    // god-file split (刀38, idle callbacks): requestIdleCallback /
    // cancelIdleCallback guarded installs + immediate native marking.
    // Consumers elsewhere are pure string tables (method_ladders SYNC,
    // window_order ORDER) - no load-time member reads anywhere.
    ("idle_callbacks.js", include_str!("js/idle_callbacks.js")),
    // god-file split (刀39, Blob/File): guarded fallback installs (realm
    // natives win), member-form guards. _bytesToBinaryString stays published
    // (fetch_helpers/filereader runtime consumers); _blobPartConvert is
    // file-private. No native marking for this family.
    ("blob_file.js", include_str!("js/blob_file.js")),
    // god-file split (刀40, DataTransfer): the #58 file-upload injection
    // face. Member-form guarded install; platform_bundle's load-time
    // DataTransfer.prototype read sits well after this position.
    ("data_transfer.js", include_str!("js/data_transfer.js")),
    // god-file split (刀41, FormData): the form-data-set constructor face.
    // Member-form guarded install; interface_sweep's load-time member-read
    // list sits after this position.
    ("form_data.js", include_str!("js/form_data.js")),
    // god-file split (刀42, URLSearchParams): query-string face + its two
    // serializers (_formEncode/_hexv). _hexv keeps bootstrap consumers as
    // cross-file runtime refs (published from here).
    ("url_search_params.js", include_str!("js/url_search_params.js")),
    // god-file split (刀43, crypto fallback): getRandomValues + randomUUID.
    // Guard already member form. crypto_subtle.js (#330) and ed25519.js
    // (#331) read globalThis.crypto at their load time - both after this
    // position, so the fallback is in place first.
    ("crypto_fallback.js", include_str!("js/crypto_fallback.js")),
    // god-file split (刀44, keyboard interaction handlers): the two CDP
    // keyDown arms - Escape closes the modal dialog (obscura#952), Tab
    // walks Chrome's focus order (#14). Plain member installs; consumers
    // are Rust (member form) and tests.
    ("interaction_handlers.js", include_str!("js/interaction_handlers.js")),
    // god-file split (刀76, hover/active bridges): __diting_setHoverTree /
    // __diting_setActiveTree — pure runtime bridges, only Rust-side runtime
    // tests call them; _domRaw value-captured off __diting_domRaw (#1).
    ("hover_bridges.js", include_str!("js/hover_bridges.js")),
    // god-file split (刀45, window scroll family): _scrollRoot/_windowScroll
    // + scrollTo/scrollBy/scroll installs (marked, alias head) + the
    // read-only scrollX/pageXOffset/scrollY/pageYOffset accessor loop that
    // replaces bootstrap's hard-coded 0 data properties (bootstrap loads
    // first, so accessor-after-data order holds). No export rows, no pins.
    ("scroll_family.js", include_str!("js/scroll_family.js")),
    // god-file split (刀46, misc window methods): focus/blur/print + open/
    // close/stop stubs, the never-blocking dialog family (alert/confirm/
    // prompt via _OPS.op_dialog), self-targeted postMessage (#704). Plain
    // member installs; no marking, no export rows.
    ("window_misc.js", include_str!("js/window_misc.js")),
    // god-file split (刀47, WebGPU constructor stubs): GPUAdapter/GPU - pages
    // detect via navigator.gpu persona; constructors exist for the surface.
    ("gpu_stubs.js", include_str!("js/gpu_stubs.js")),
    // god-file split (刀48, NodeList belt): spec collection class + ctor-mint
    // sealing factory + marking. Registers before platform_bundle (its load
    // reads bare NodeList for the RadioNodeList iface row). _nodeList d()
    // row auto-migrates here from bootstrap.
    ("nodelist.js", include_str!("js/nodelist.js")),
    // god-file split (刀57, URL face): _urlParseOp/_urlSetOp/
    // _urlResolveOp (Rust url crate delegation) + the URL constructor
    // (_URLw) with its load-time native marking. MUST load before
    // blob_urls.js — it mutates the URL constructor (createObjectURL)
    // at load time; runtime-only consumers otherwise.
    ("url_face.js", include_str!("js/url_face.js")),
    // god-file split (刀58, MutationObserver family): the observer class,
    // the microtask flush (__moFlush), the WebKitMutationObserver alias and
    // __notifyMutation (the engine's record entry point). All installs are
    // globalThis member forms; cross-file consumers (domcore dispatch,
    // ctor_ladders, shape_fix, global_surface, window_order, worker) are
    // runtime-only. Native marking migrated (刀22 precedent).
    ("mutation_observer.js", include_str!("js/mutation_observer.js")),
    // god-file split (刀70, ResizeObserver): the real per-target firing
    // class (observe/unobserve/disconnect only, 刀57 shape); install
    // self-marks (marking travels, 刀22; the bootstrap marking-list row
    // became a note). Observer cluster slot. interface_sweep reads the
    // global at its load — this file is earlier.
    ("resize_observer.js", include_str!("js/resize_observer.js")),
    // god-file split (刀73, NodeFilter): the full standard constant table
    // (issue #439 — FILTER_* + whole SHOW_* mask set). Plain data install,
    // no d() row, no marking; later layers build module-local faces and
    // read the global guardedly. DOM cluster slot.
    ("node_filter.js", include_str!("js/node_filter.js")),
    // god-file split (刀75, PerformanceObserver stub): the placeholder class
    // performance.js replaces wholesale at its later slot; kept for the
    // interface sweeps (#108 member read — far later). Observer cluster;
    // the install self-marks (marking travels, 刀22/刀54).
    ("performance_observer.js", include_str!("js/performance_observer.js")),
    // god-file split (刀60, form activation belt): the pre/post click
    // activation machinery — lazy prototype-setter cache (bypasses instance
    // traps like React's _valueTracker), flip/cancel/commit triplet,
    // _postClickActivation (label/link/submit). Pure function family, zero
    // installs; bootstrap's trusted-CDP block and details_summary/event_hooks/
    // domcore call at runtime. Zone refs out to bootstrap-published names
    // are runtime reads.
    // god-file split (刀67, form control semantics): submitter validator,
    // label association (_LABELABLE + _labeledControl), the CDP
    // trusted-click dispatch (flip via form_activation triplet), the
    // fieldset-disabled chain (first-legend rule) and _fireBlurFamily
    // (focus switch-away leg). Zero installs, zero marking; consumers
    // (domcore, form_activation, interaction_handlers) all runtime reads.
    ("form_controls.js", include_str!("js/form_controls.js")),
    ("form_activation.js", include_str!("js/form_activation.js")),
    // god-file split (刀61, form state mirrors + FileList face): the four
    // nid-keyed value mirrors (idempotent globalThis installs + const
    // aliases) and the FileList-shaped face (illegal ctor, 刀75 prototype
    // shape, WeakMap state, _makeFileList). domcore accessors and
    // data_transfer's files getter read at runtime; FileList install is
    // typeof-guarded (host native wins). No native marking — synthetic face.
    ("form_state.js", include_str!("js/form_state.js")),
    // god-file split (刀62, DOM query + parser-visibility helpers): ASCII
    // whitespace splitting, shared getElementsByClassName (fast path +
    // O(n) fallback, collections via __diting_mkCollection), and the #120
    // parser-order script visibility pair. domcore/domparser/iframe_realm
    // call at runtime; bootstrap holds zero refs (pure d() migration).
    ("dom_query.js", include_str!("js/dom_query.js")),
    // god-file split (刀63, persona fingerprint): _fpRand/_fpPlatform/
    // _getFp/_fp + the mutable seed/cache pair. bootstrap's init and
    // persona-apply WRITE _fpSeed/_fpCache through the a()-published
    // accessors (sloppy global assignment routes into the setters);
    // consumers (audio/bchk/canvas2d/nav_persona/perf/uadata/webrtc) read
    // at runtime. Pins _getFp/_fpCache/_fpSeed flipped here (刀52).
    ("fingerprint.js", include_str!("js/fingerprint.js")),
    // god-file split (刀64, media queries): persona/mobile/touch tables,
    // __diting_mqRecompute fold, _mqExpr/_mqClause/_mqMatches parser trio,
    // MediaQueryList + registry, __diting_mqFlush pump, matchMedia face.
    // Bootstrap's CDP emulation hooks (now cdp_emulation.js, 刀87) call the
    // globalThis hooks at runtime;
    // tables publish via d(). Marking travels (install-site wrapper, 刀22).
    ("media_queries.js", include_str!("js/media_queries.js")),
    // god-file split (刀87, CDP emulation belt): setPersona / setViewport /
    // clearViewport / touchMax+setTouch / setMediaFeatures / setFpSeed —
    // the knobs Rust (set_user_agent, CDP override paths) and
    // __diting_init drive. Every cross-file touch is member-form or
    // accessor/d()-mediated and resolved at call time (fingerprint's
    // _fp/_fpPlatform d() + _fpSeed/_fpCache a(), screen_viewport's __vp*
    // a(), media_queries' mqRecompute/mqFlush members). Any slot safe;
    // beside media_queries, whose hooks it drives.
    ("cdp_emulation.js", include_str!("js/cdp_emulation.js")),
    // god-file split (刀88, per-realm initializer): __diting_init — the
    // whole per-realm setup Rust calls after every context restore (fp
    // seed redraw, on*/viewport accessor re-installs, persona, SAB
    // intrinsic delete, NavigationTiming seeding, the interface hide
    // belt, self-delete). Free names accessor/d()-mediated at call time;
    // nothing captured at load; the install changes no visibility window
    // (hide-list capture runs at bootstrap's load; init self-deletes).
    ("diting_init.js", include_str!("js/diting_init.js")),
    // god-file split (刀49, blob URL minting belt): __blobStore/__blobObjs
    // registries + v4-UUID minter + URL.createObjectURL/revokeObjectURL
    // (member-form writes onto the url_face.js constructor, this
    // overwrites later - order preserved). opener null folded in.
    ("blob_urls.js", include_str!("js/blob_urls.js")),
    // god-file split (刀50, input text-selection + date-draft belt): _ns_
    // WeakMap mirrors + selection/typing helpers + date sanitize ladder +
    // __diting_dateDraftBase engine hook. domcore consumes at runtime via
    // the auto-migrated d() rows. Validation lives in form_validation.js.
    ("input_selection.js", include_str!("js/input_selection.js")),
    // god-file split (刀51, Response body methods): blob()/text()/json() on
    // the real fetch path; Response is a host native, typeof guards read a
    // host global in both homes - verbatim move. Marking travels (alias).
    ("response_methods.js", include_str!("js/response_methods.js")),
    // god-file split (刀52, GlobalEventHandlers on* IDL belt): on<event>
    // accessors on Element/Document prototypes (deferred) + window (direct,
    // chrome-155 truth set). Deferred registration via the published
    // __diting_domDeferred member (ratchet-flat form).
    ("on_handlers.js", include_str!("js/on_handlers.js")),
    // god-file split (刀54, timer wheel + macrotask trampoline belt): the
    // #38 sorted timer heap + patrol/arm/wake, the #30 macrotask trampoline
    // (drain/arm/run/window + #39 stall predicates + recover hook +
    // __MT_STATE dump), the #240 fetch post-mortem ring, and the
    // setTimeout/Interval/rAF/queueMicrotask faces with their migrated
    // native marking. Cross-file consumers are all runtime reads of
    // published hidden globals; message_port.js wraps the recover hook
    // late in the order.
    ("timer_belt.js", include_str!("js/timer_belt.js")),
    // god-file split (刀55, text codec fallbacks): typeof-guarded
    // TextEncoder/TextDecoder (pure-JS WHATWG UTF-8 decoder) +
    // TextEncoderStream/TextDecoderStream. Host native wins when present;
    // all consumers are runtime-only calls.
    ("text_codec.js", include_str!("js/text_codec.js")),
    // god-file split (刀56, dynamic script preparation):
    // __prepareInsertedScript in full — type triage, fetch-driven
    // execution with retry policy, JSONP/mtop, data:/blob: URLs, inline
    // eval bracketing. Runtime-only consumers via published hidden
    // global (insertion_prep.js walk, domcore.js script creation).
    ("script_prep.js", include_str!("js/script_prep.js")),
    // god-file split (刀14, Plugin/MimeType family): the four array-like
    // classes (Plugin, MimeType, PluginArray, MimeTypeArray) + WeakMap stores
    // + the cached PDF persona builder. Self-contained installs on globalThis;
    // navigator_persona reads the two singleton lets and the builder at
    // runtime through the generated a()/d() publishes. Loads before
    // navigator_persona to keep the original bootstrap statement order.
    ("plugins.js", include_str!("js/plugins.js")),
    // god-file split (刀18, NavigatorUAData): the illegal-constructor class
    // (brands/mobile/platform, getHighEntropyValues, toJSON) + the WebIDL
    // prototype reorder, which migrates here from bootstrap's bottom cluster
    // (bootstrap loads before this file installs the class). Runtime helper
    // reads (_getFp, the UA-major/build table pair) ride the exports-extra
    // pins. Loads before navigator_persona — its store's userAgentData
    // Object.creates the prototype at load time.
    ("uadata.js", include_str!("js/uadata.js")),
    // god-file split (刀13, navigator persona): __navStore literal, langs
    // memo, the Navigator class + prototype setup walk + singleton +
    // __diting_navSet. The mark-list navigator cluster, the credentials
    // fallback and the prototype reorder try-block migrate with it (they
    // load-order-depend on the family). Load-time refs to the UA-data /
    // NetworkInformation classes go through globalThis member reads;
    // __ditingReorderProto rides the 刀5 alias + exports-extra pin.
    ("navigator_persona.js", include_str!("js/navigator_persona.js")),
    // god-file split: the canvas font shorthand parser (刀117), call-time only.
    ("canvas_font.js", include_str!("js/canvas_font.js")),
    // god-file split (刀7, Path2D family): the guarded install — subpaths in
    // user space, the SVG path-data constructor parser, addPath. Sits before
    // canvas2d.js, whose context methods construct Path2D at call time
    // (_p2d()/beginPath).
    ("path2d.js", include_str!("js/path2d.js")),
    // god-file split: the canvas 2D family — TextMetrics, the _Canvas2D class
    // (it IS CanvasRenderingContext2D), HTMLCanvasElement's canvas methods and
    // the bitmap flush hook the host evaluates before a render.
    ("canvas2d.js", include_str!("js/canvas2d.js")),
    // god-file split: VideoFrame (刀67) — before videoframe_gaps.js and the sweeps.
    ("videoframe.js", include_str!("js/videoframe.js")),
    // god-file split: the WebGL context machinery (tables, state, 刀92 extension methods).
    ("webgl.js", include_str!("js/webgl.js")),
    // (刀97) generated per-glyph canvas advance tables (Chrome 145 macOS
    // truth); measureText reads globalThis.__ditingFontMetrics at call time.
    ("font_metrics.js", include_str!("js/font_metrics.js")),
    // Ratchet split (JS-SURFACE-PLAN step 3): CSSOM sheet objects. Must stay
    // first after bootstrap — global_surface and the sweep walk CSSStyleSheet.
    ("cssom_sheets.js", include_str!("js/cssom_sheets.js")),
    // Ratchet split (JS-SURFACE-PLAN step 3): PointerEvent/pointer capture/Touch.
    ("pointer_touch.js", include_str!("js/pointer_touch.js")),
    // #252 god-file ratchet split: the fetch family moved out of
    // bootstrap.js wholesale. Its top level only declares bindings; every
    // helper it references (_docBase, __recordFetchTiming, Headers…) is
    // touched at call time, by which point bootstrap.js has run. Must
    // precede xhr.js, whose send() rides __ditingFetchShim.
    ("webrtc.js", include_str!("js/webrtc.js")),
    ("location_nav.js", include_str!("js/location_nav.js")),
    ("list_reflect.js", include_str!("js/list_reflect.js")),
    ("details_summary.js", include_str!("js/details_summary.js")),
    ("css_register.js", include_str!("js/css_register.js")),
    ("css_decl_door.js", include_str!("js/css_decl_door.js")),
    ("raf.js", include_str!("js/raf.js")),
    ("computed_size.js", include_str!("js/computed_size.js")),
    ("form_validation.js", include_str!("js/form_validation.js")),
    ("fetch.js", include_str!("js/fetch.js")),
    // Ratchet splits (pure moves): the audio/speechSynthesis faces, the
    // Worker family, media_source, iframe_realm, range_selection,
    // indexeddb and crypto_subtle left bootstrap.js wholesale. All only
    // declare bindings at top level and touch bootstrap helpers at call time.
    ("audio.js", include_str!("js/audio.js")),
    ("worker.js", include_str!("js/worker.js")),
    ("media_source.js", include_str!("js/media_source.js")),
    ("iframe_realm.js", include_str!("js/iframe_realm.js")),
    ("range_selection.js", include_str!("js/range_selection.js")),
    ("indexeddb.js", include_str!("js/indexeddb.js")),
    // Ratchet split (JS-SURFACE-PLAN step 3): the document.cookie access gate
    // (legacy tier; leaves with the Document.cookie family).
    ("cookie_gate.js", include_str!("js/cookie_gate.js")),
    // JS-PLATFORM-ARCH M2: the module bundle (platform/core.js + IDL-generated
    // and hand-written modules, topologically ordered). GENERATED by
    // tools/webplatform/bundle.py. Storage/StorageEvent live here now.
    ("platform_bundle.js", include_str!("js/platform_bundle.js")),
    // 刀55: after the bundle's generated installs have landed, the SVG tags
    // get their inherited-descriptor homes and the Element kitchen sink
    // (94 members Chrome has nowhere on Element) leaves Element.prototype.
    ("element_sink.js", include_str!("js/element_sink.js")),
    // Ratchet split: crypto.subtle / CryptoKey left bootstrap.js wholesale.
    // Touches _OPS and __diting_clone_hooks only at call time; basic
    // getRandomValues/randomUUID stay in bootstrap.
    ("crypto_subtle.js", include_str!("js/crypto_subtle.js")),
    ("ed25519.js", include_str!("js/ed25519.js")),
    // #236 absorption split: the XHR family moved out of bootstrap.js
    // wholesale — consumes the fetch shim closure and the initiator-hint
    // var fetch.js defined, both at class-definition and at send() time.
    ("xhr.js", include_str!("js/xhr.js")),
    // #97 ratchet split: DOMParser wholesale — the XML gates and node
    // classes it names are bootstrap bindings, touched only at
    // parseFromString call time.
    ("domparser.js", include_str!("js/domparser.js")),
    // #98 ratchet split: the Performance family wholesale.
    ("performance.js", include_str!("js/performance.js")),
    // Window-level scalar faces (name/status/origin/closed) — defaults plus
    // live getters, nothing call-time dependent on earlier scripts.
    ("window_faces.js", include_str!("js/window_faces.js")),
    // #291 / #226 splits: message_port and the fetch-settlement registry —
    // both only touch bootstrap-defined bindings, and only at call time.
    ("message_port.js", include_str!("js/message_port.js")),
    ("fetch_probe.js", include_str!("js/fetch_probe.js")),
    // 300015 P2: the chrome-truth window own-prop surface (generated — see
    // the file header). Loads after every behavior module, before the
    // sweeps.
    ("global_surface.js", include_str!("js/global_surface.js")),
    ("media_late.js", include_str!("js/media_late.js")),
    // 300015 P3: Chrome-exact toDataURL/toBlob bytes for blank canvases
    // (generated — see the file header). Wraps bootstrap's toDataURL; must
    // load before the sweep so the wrappers get native-marked with the rest.
    ("canvas_face.js", include_str!("js/canvas_face.js")),
    // LAST (#236 absorption split): the snapshot-time interface sweeps walk
    // globalThis to finish every interface (native marks, non-enumerable
    // globals, enumerable members, WebIDL toStringTag brands) — running last
    // is the point, so every earlier script's globals get swept.
    ("interface_sweep.js", include_str!("js/interface_sweep.js")),
    // Load after the sweep: the CSSOM rule classes, scheduler & friends are minted by
    // global_surface; adopted_sheets wraps the sheet methods css_rules installs.
    ("css_rules.js", include_str!("js/css_rules.js")),
    ("adopted_sheets.js", include_str!("js/adopted_sheets.js")),
    ("web_api_gaps.js", include_str!("js/web_api_gaps.js")),
    // 刀89: typed-om arithmetic — CSSNumericValue add/sub/mul/div/min/max/
    // equals/to/toSum/type + parse statics + CSSMath instance members + the
    // CSSNumericArray face. After global_surface (reads its CSSNumericValue/
    // CSSMath* stubs and the mkCssMath kids storage); before stub_members so
    // these takeovers survive the fill-if-missing pass.
    ("typedom_math.js", include_str!("js/typedom_math.js")),
    // 刀59: the SVG root face — SVGSVGElement's 34 missing members + 3 zoom
    // statics, and the real storage the face reaches through (the legacy SVG
    // value types were stub shells). After web_api_gaps so the minted shells
    // exist; before stub_members so the fill-if-missing pass stays out.
    ("svg_root.js", include_str!("js/svg_root.js")),
    // 刀65: Document's 24 missing members + body setter + FeaturePolicy
    // instance machinery. After web_api_gaps (wraps startViewTransition);
    // BEFORE stub_members so the FeaturePolicy method takeover wins the
    // fill-if-missing pass.
    ("document_gaps.js", include_str!("js/document_gaps.js")),
    // 刀56: after global_surface minted AbstractRange and wired the Range
    // chains, the shared boundary getters land on AbstractRange.prototype
    // and leave the two class prototypes (before stub_members' fill).
    ("range_lift.js", include_str!("js/range_lift.js")),
    ("navigation.js", include_str!("js/navigation.js")),
    ("client_rects.js", include_str!("js/client_rects.js")),
    // Clone hooks for native (Rust-backed) interfaces; needs the natives (installed with the faces).
    ("native_clone.js", include_str!("js/native_clone.js")),
    // Host hooks for the native EventTarget core (needs Node/Event from bootstrap).
    ("event_hooks.js", include_str!("js/event_hooks.js")),
    // 刀67: VideoColorSpace behavior takeover — must run after the surface
    // mint (the class exists) and before stub_members/lift_own (the fill
    // skips what's installed here; the minted prototype identity is kept).
    ("videoframe_gaps.js", include_str!("js/videoframe_gaps.js")),
    // 刀68: IDBVersionChangeEvent behavior takeover (indexeddb.js loads before Event exists).
    ("idb_gaps.js", include_str!("js/idb_gaps.js")),
    // 刀69: WebGL extension objects (GENERATED from the Chrome capture; getExtension reads it at call time).
    ("webgl_ext.js", include_str!("js/webgl_ext.js")),
    // 刀70: SpeechSynthesis voice instances + event ctor ladders (takeover of the minted faces).
    ("speech_gaps.js", include_str!("js/speech_gaps.js")),
    // 刀71: interestForElement on the four interfaces Chrome carries it on.
    ("interest_for.js", include_str!("js/interest_for.js")),
    // 刀75: the missing-member tail (handlers, legacy constants, small reflections).
    ("tail_members.js", include_str!("js/tail_members.js")),
    // 刀77: SVG tags the engine still wrapped as bare SVGElement get their interface prototype.
    ("svg_tags.js", include_str!("js/svg_tags.js")),
    // 刀77: real reflectors for the SVG interfaces that were pure stubs (GENERATED from the Chrome capture).
    ("svg_filters.js", include_str!("js/svg_filters.js")),
    // 刀78: constructor argument ladders + zero-arg Illegal constructor for the nid-based node classes.
    ("ctor_ladders.js", include_str!("js/ctor_ladders.js")),
    // LAST of all: descriptor-level conformance table (generated from the shape audit).
    ("lift_own.js", include_str!("js/lift_own.js")),
    ("demote_own.js", include_str!("js/demote_own.js")),
    ("internal_names.js", include_str!("js/internal_names.js")),
    ("hide_priv.js", include_str!("js/hide_priv.js")),
    ("mixin_fix.js", include_str!("js/mixin_fix.js")),
    ("stub_members.js", include_str!("js/stub_members.js")),
    // 刀79: primitive/null defaults for the pure-stub getters (loaded right after the stubs exist).
    ("stub_defaults.js", include_str!("js/stub_defaults.js")),
    // VTTCue/TextTrackCue behavior over the minted stubs (hls.js builds a cue at module top level).
    ("text_cue.js", include_str!("js/text_cue.js")),
    // 刀80: WebIDL required-argument ladders for every interface method (GENERATED from the Chrome lengths).
    ("method_ladders.js", include_str!("js/method_ladders.js")),
    ("sym_fix.js", include_str!("js/sym_fix.js")),
    ("window_extras.js", include_str!("js/window_extras.js")),
    ("shape_fix.js", include_str!("js/shape_fix.js")),
    ("version_prune.js", include_str!("js/version_prune.js")),
    // #30: `get X`/`set X` names on every window accessor + the [Replaceable] setters (before the reorder).
    ("window_accessors.js", include_str!("js/window_accessors.js")),
    ("window_order.js", include_str!("js/window_order.js")),
];

fn main() {
    for (name, _) in SNAPSHOT_SCRIPTS {
        println!("cargo:rerun-if-changed=js/{name}");
    }
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let snapshot_path = out_dir.join("DITING_SNAPSHOT.bin");

    // 10d: our own snapshot creator (diting_runtime::vm::create_snapshot) —
    // PRELUDE + raw faces + the script list above, in our [blob][sidecar]
    // container. Scripts run origin-less: stack frames surface as
    // "<anonymous>" (300015 brand-leak law; the name rides only panics).
    // #276 shared-source campaign: scripts whose code region is pure ASCII
    // compile from process-static OneByteConst bytes (external strings —
    // the realm heap holds a handle, not the text). Non-ASCII maps to '?',
    // which is only safe inside comments (inert text). The guarantee: strip
    // NOTHING BUT comments from the original and the mapped source and the
    // two must be identical — so every character the snapshot executes is
    // ASCII on both sides; a script that fails the check rides the inline
    // fallback (empty const).
    fn strip_comments_js(src: &str) -> String {
        // Mini-lexer: drops line/block comments; carries code, string and
        // template bodies, and regex literals (char classes included), so a
        // non-ASCII byte in any EXECUTED position breaks the equality and
        // forces the fallback — a mangled string must never slip through
        // (2026-11-05 audit: the old string-dropping stripper let
        // 'Google 日本語' externalize as 'Google ??'; Chinese form-validation
        // copy had already shipped mangled the same way).
        // Regex-vs-division is decided from the previous significant char
        // (a value-ending context means division; keywords exempted); a
        // misparse only ever mislabels a span as code — and code is kept —
        // so the failure mode is a lost optimization, never a wrong verdict.
        let b: Vec<char> = src.chars().collect();
        let mut out = String::new();
        let mut i = 0;
        let mut prev = '\0'; // last significant char; whitespace/comments skip it
        let mut word = String::new(); // identifier ending at `prev`, for keyword checks
        while i < b.len() {
            let c = b[i];
            if c == '/' && i + 1 < b.len() && b[i + 1] == '/' {
                while i < b.len() && b[i] != '\n' {
                    i += 1;
                }
                continue; // the newline itself stays code
            } else if c == '/' && i + 1 < b.len() && b[i + 1] == '*' {
                i += 2;
                while i + 1 < b.len() && !(b[i] == '*' && b[i + 1] == '/') {
                    i += 1;
                }
                i = (i + 2).min(b.len());
                continue;
            } else if c == '"' || c == '\'' || c == '`' {
                let q = c;
                out.push(q);
                i += 1;
                while i < b.len() && b[i] != q {
                    if b[i] == '\\' && i + 1 < b.len() {
                        out.push(b[i]);
                        out.push(b[i + 1]);
                        i += 2;
                        continue;
                    }
                    out.push(b[i]);
                    i += 1;
                }
                if i < b.len() {
                    out.push(b[i]); // closing quote
                }
                i += 1;
                prev = q;
                word.clear();
            } else if c == '/' && is_regex_position(prev, &word) {
                out.push(c);
                i += 1;
                let mut in_class = false;
                while i < b.len() {
                    let r = b[i];
                    if r == '\\' && i + 1 < b.len() {
                        out.push(r);
                        out.push(b[i + 1]);
                        i += 2;
                        continue;
                    }
                    if r == '\n' {
                        break; // not a regex after all — bail, chars stay code
                    }
                    if r == '[' {
                        in_class = true;
                    } else if r == ']' {
                        in_class = false;
                    } else if r == '/' && !in_class {
                        break;
                    }
                    out.push(r);
                    i += 1;
                }
                if i < b.len() && b[i] != '\n' {
                    out.push(b[i]); // closing '/'
                    i += 1;
                    while i < b.len() && b[i].is_ascii_alphabetic() {
                        out.push(b[i]); // flags
                        i += 1;
                    }
                }
                prev = '0'; // a completed regex ends a value (division may follow)
                word.clear();
            } else {
                out.push(c);
                i += 1;
                if c.is_ascii_alphanumeric() || c == '_' || c == '$' {
                    if !(prev.is_ascii_alphanumeric() || prev == '_' || prev == '$') {
                        word.clear();
                    }
                    word.push(c);
                } else if !c.is_whitespace() {
                    word.clear();
                }
                if !c.is_whitespace() {
                    prev = c;
                }
            }
        }
        out
    }

    // A '/' starts a regex literal except right after something that could
    // end a value (identifier/number/)/]/quote-close) — there it is division.
    // Keywords (return/typeof/case/…) end no value, so they still open one.
    fn is_regex_position(prev: char, last_word: &str) -> bool {
        const KEYWORDS: [&str; 12] = [
            "return",
            "typeof",
            "instanceof",
            "in",
            "of",
            "new",
            "delete",
            "void",
            "case",
            "do",
            "else",
            "yield",
        ];
        let value_ending = prev.is_ascii_alphanumeric()
            || prev == '_'
            || prev == '$'
            || prev == ')'
            || prev == ']'
            || prev == '"'
            || prev == '\''
            || prev == '`';
        !value_ending || KEYWORDS.contains(&last_word)
    }

    // The sources ride into the binary as ONE blob the assembler pulls in
    // with `.incbin` (see `incbin_items`), sliced by offset below. Both a
    // b"..." literal and include_bytes! made rustc/LLVM hold several copies
    // of the bytes: these ~92MB (font_metrics.js alone ~90MB) cost ~7GB of
    // the diting_js rustc peak, the 228MB snapshot another ~3.5GB, the
    // crate's own code under 1GB.
    let mut generated = String::from(
        "// GENERATED by build.rs — ASCII snapshot script sources.\n// #276 shared-source campaign: do not hand-edit.\n",
    );
    let mut table = String::from("pub static SNAPSHOT_SRC_ASCII: &[&[u8]] = &[\n");
    let mut blob: Vec<u8> = Vec::new();
    let mut externalized = 0usize;
    let mut ascii_sources: Vec<&'static [u8]> = Vec::new();
    for (name, src) in SNAPSHOT_SCRIPTS {
        let mapped: String = src
            .chars()
            .map(|c| if c.is_ascii() { c } else { '?' })
            .collect();
        let ok = mapped.is_ascii() && strip_comments_js(src) == strip_comments_js(&mapped);
        if ok {
            externalized += 1;
        }
        let bytes: &[u8] = if ok { mapped.as_bytes() } else { b"" };
        let leaked: &'static [u8] = Box::leak(bytes.to_vec().into_boxed_slice());
        ascii_sources.push(leaked);
        table.push_str(&format!(
            "    // {name} — {}\n",
            if ok {
                format!("{} bytes externalized", leaked.len())
            } else {
                "inline fallback (non-ASCII code region)".to_string()
            }
        ));
        table.push_str(&format!(
            "    unsafe {{ core::slice::from_raw_parts((&raw const DITING_SNAPSHOT_SRC).cast::<u8>().add({}), {}) }},\n",
            blob.len(),
            leaked.len()
        ));
        blob.extend_from_slice(leaked);
    }
    table.push_str("];\n");
    let blob_path = out_dir.join("snapshot_src.bin");
    std::fs::write(&blob_path, &blob).expect("Failed to write snapshot_src.bin");
    generated.push_str(&incbin_items("DITING_SNAPSHOT_SRC", &blob_path, blob.len()));
    generated.push_str(&table);
    std::fs::write(out_dir.join("snapshot_sources_ascii.rs"), generated)
        .expect("Failed to write snapshot_sources_ascii.rs");
    println!(
        "cargo:warning=#276 shared-source: {externalized}/{} scripts externalized",
        SNAPSHOT_SCRIPTS.len()
    );

    let bake_consts: Vec<&'static diting_runtime::v8::OneByteConst> = ascii_sources
        .iter()
        .map(|b| diting_runtime::snapshot::ascii_const(b))
        .collect();

    let output = diting_runtime::vm::create_snapshot(SNAPSHOT_SCRIPTS, &bake_consts);

    std::fs::write(&snapshot_path, &output).expect("Failed to write snapshot");
    let mut blob_rs = String::from("// GENERATED by build.rs — the V8 startup snapshot.\n");
    blob_rs.push_str(&incbin_items(
        "DITING_SNAPSHOT",
        &snapshot_path,
        output.len(),
    ));
    blob_rs.push_str("pub(crate) static SNAPSHOT: &[u8] = unsafe { &DITING_SNAPSHOT };\n");
    std::fs::write(out_dir.join("snapshot_blob.rs"), blob_rs)
        .expect("Failed to write snapshot_blob.rs");
}

/// Rust items embedding `path` as a read-only `[u8; len]` named `sym`, via
/// the assembler's `.incbin` — the bytes never pass through rustc or LLVM
/// IR as a constant. Directives follow the target's object format (read
/// from CARGO_CFG_*, so cross builds pick the target's, not the host's).
fn incbin_items(sym: &str, path: &std::path::Path, len: usize) -> String {
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let family = std::env::var("CARGO_CFG_TARGET_FAMILY").unwrap_or_default();
    // Forward slashes: `.incbin` takes a C-style string, where Windows
    // backslashes would read as escapes.
    let file = path.display().to_string().replace('\\', "/");
    let asm = if os == "macos" || os == "ios" {
        // Mach-O: C symbols carry a leading underscore.
        format!(
            ".section __DATA,__const\n.p2align 4\n.private_extern _{sym}\n.globl _{sym}\n_{sym}:\n.incbin \"{file}\"\n"
        )
    } else if family == "windows" {
        format!(".section .rdata,\"dr\"\n.p2align 4\n.globl {sym}\n{sym}:\n.incbin \"{file}\"\n")
    } else {
        // ELF. `%` (not `@`) for the type markers: `@` starts a comment on ARM.
        format!(
            ".section .rodata.{sym},\"a\",%progbits\n.p2align 4\n.globl {sym}\n.hidden {sym}\n.type {sym},%object\n.size {sym},{len}\n{sym}:\n.incbin \"{file}\"\n"
        )
    };
    format!(
        "std::arch::global_asm!({asm:?});\nextern \"C\" {{\n    static {sym}: [u8; {len}];\n}}\n"
    )
}
