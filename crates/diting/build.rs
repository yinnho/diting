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
    ("window_order.js", include_str!("js/window_order.js")),
];

/// Read `chrome-target.toml` (a flat key = value subset of TOML) and emit
/// `$OUT_DIR/chrome_target.rs` + `DITING_CHROME_MAJOR`. The target Chrome is declared in
/// exactly one place; this is how the Rust side receives it.
fn emit_chrome_target() {
    let path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("chrome-target.toml");
    println!("cargo:rerun-if-changed=chrome-target.toml");
    let text = std::fs::read_to_string(&path).expect("chrome-target.toml missing");
    let mut kv = std::collections::HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            kv.insert(k.trim().to_string(), v.trim().trim_matches('"').to_string());
        }
    }
    let get = |k: &str| kv.get(k).unwrap_or_else(|| panic!("chrome-target.toml: missing `{k}`")).clone();
    let major: u32 = get("major").parse().expect("chrome-target.toml: major must be an integer");
    let full = get("full_version");
    let tls = get("tls_profile");
    assert!(full.starts_with(&format!("{major}.")), "full_version {full} is not Chrome {major}");
    assert_eq!(tls, format!("chrome{major}"), "tls_profile must be chrome{{major}}");
    let code = format!(
        "// GENERATED by build.rs from chrome-target.toml - do not edit.\n\
         pub const CHROME_MAJOR: u32 = {major};\n\
         pub const CHROME_FULL_VERSION: &str = \"{full}\";\n\
         pub const TLS_PROFILE_NAME: &str = \"{tls}\";\n\
         pub const PLATFORM_OS: &str = \"{os}\";\n\
         pub const PLATFORM_ARCH: &str = \"{arch}\";\n\
         /// The wreq-util preset for the target major. A preset the TLS library lacks is a\n\
         /// COMPILE error here - the TLS layer cannot silently fall behind the target.\n\
         #[cfg(feature = \"stealth\")]\n\
         pub fn target_tls_profile() -> diting_profiles::Profile {{ diting_profiles::Profile::Chrome{major} }}\n",
        os = get("platform"),
        arch = get("arch"),
    );
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("chrome_target.rs");
    std::fs::write(out, code).expect("write chrome_target.rs");
    println!("cargo:rustc-env=DITING_CHROME_MAJOR={major}");
}

fn main() {
    emit_chrome_target();
    for (name, _) in SNAPSHOT_SCRIPTS {
        println!("cargo:rerun-if-changed=js/{name}");
    }
    println!("cargo:rerun-if-changed=build.rs");
    // Re-embed the commit when HEAD moves (branch switches change .git/HEAD;
    // same-branch commits move .git/refs/heads/<branch> — cargo watches both
    // when present). Release CI builds from a fresh checkout either way.
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads");

    // Source revision baked into the binary — /health reports it so a caller
    // can verify doc/tag/binary/source are the same commit without sniffing
    // anything (0.3.0 tmall report P2). "unknown" for git-less builds.
    // DITING_BUILD_COMMIT overrides git: the 86quan deploy rsyncs
    // without .git (by design — the server tree is a build input, not a
    // checkout), so the deploy step passes the commit explicitly and /health
    // stops reporting "unknown".
    let commit = std::env::var("DITING_BUILD_COMMIT")
        .or_else(|_| std::env::var("AGINXBROWSER_BUILD_COMMIT"))
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| {
            std::process::Command::new("git")
                .args(["rev-parse", "--short", "HEAD"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|| "unknown".to_string())
        });
    println!("cargo:rustc-env=DITING_BUILD_COMMIT={commit}");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let snapshot_path = out_dir.join("DITING_SNAPSHOT.bin");

    // 10d: our own snapshot creator (diting_runtime::vm::create_snapshot) —
    // PRELUDE + raw faces + the script list above, in our [blob][sidecar]
    // container. Scripts run origin-less: stack frames surface as
    // "<anonymous>" (300015 brand-leak law; the name rides only panics).
    // #276 shared-source campaign: scripts whose code region is pure
    // ASCII compile from process-static OneByteConst bytes (external
    // strings — the realm heap holds a handle, not the text). Comment
    // non-ASCII maps to '?': the guarantee below is that stripping
    // comments from the original and the mapped source yields IDENTICAL
    // code regions, so runtime behavior is untouched; a script that
    // fails the check rides the inline fallback (empty const).
    fn strip_comments_js(src: &str) -> String {
        // Conservative comment stripper for the equality check only:
        // block comments, line comments, and string/template/regex
        // literals passed through verbatim so their contents count as
        // code. Chars: enough for these files (no regex-literal edge
        // ambiguity after '/' inside strings — literals are carried
        // whole, so a // inside a string stays).
        let b: Vec<char> = src.chars().collect();
        let mut out = String::new();
        let mut i = 0;
        while i < b.len() {
            let c = b[i];
            if c == '/' && i + 1 < b.len() && b[i + 1] == '/' {
                while i < b.len() && b[i] != '\n' {
                    i += 1;
                }
            } else if c == '/' && i + 1 < b.len() && b[i + 1] == '*' {
                i += 2;
                while i + 1 < b.len() && !(b[i] == '*' && b[i + 1] == '/') {
                    i += 1;
                }
                i = (i + 2).min(b.len());
            } else if c == '"' || c == '\'' || c == '`' {
                let q = c;
                out.push(q);
                i += 1;
                while i < b.len() && b[i] != q {
                    if b[i] == '\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            if i < b.len() {
                out.push(b[i]);
            }
            i += 1;
        }
        out
    }

    let mut generated =
        String::from("// GENERATED by build.rs — ASCII snapshot script sources.\n// #276 shared-source campaign: do not hand-edit.\npub static SNAPSHOT_SRC_ASCII: &[&[u8]] = &[\n");
    let mut externalized = 0usize;
    let mut ascii_sources: Vec<&'static [u8]> = Vec::new();
    for (name, src) in SNAPSHOT_SCRIPTS {
        let mapped: String = src
            .chars()
            .map(|c| if c.is_ascii() { c } else { '?' })
            .collect();
        let ok = mapped.is_ascii()
            && strip_comments_js(src) == strip_comments_js(&mapped);
        if ok {
            externalized += 1;
        }
        let bytes: &[u8] = if ok { mapped.as_bytes() } else { b"" };
        let leaked: &'static [u8] = Box::leak(bytes.to_vec().into_boxed_slice());
        ascii_sources.push(leaked);
        generated.push_str(&format!(
            "    // {name} — {}\n",
            if ok { format!("{} bytes externalized", leaked.len()) } else { "inline fallback (non-ASCII code region)".to_string() }
        ));
        let mut lit = String::from("    b\"");
        for ch in leaked.iter() {
            match ch {
                b'"' => lit.push_str("\\\""),
                b'\\' => lit.push_str("\\\\"),
                b'\n' => lit.push_str("\\n"),
                0x20..=0x7e => lit.push(*ch as char),
                _ => lit.push_str(&format!("\\x{:02x}", ch)),
            }
        }
        lit.push_str("\",\n");
        generated.push_str(&lit);
    }
    generated.push_str("];\n");
    std::fs::write(out_dir.join("snapshot_sources_ascii.rs"), generated)
        .expect("Failed to write snapshot_sources_ascii.rs");
    println!("cargo:warning=#276 shared-source: {externalized}/{} scripts externalized", SNAPSHOT_SCRIPTS.len());

    let bake_consts: Vec<&'static diting_runtime::v8::OneByteConst> = ascii_sources
        .iter()
        .map(|b| diting_runtime::snapshot::ascii_const(b))
        .collect();

    let output = diting_runtime::vm::create_snapshot(SNAPSHOT_SCRIPTS, &bake_consts);

    std::fs::write(&snapshot_path, &output).expect("Failed to write snapshot");
    println!(
        "cargo:rustc-env=DITING_SNAPSHOT_PATH={}",
        snapshot_path.display()
    );
}
