//! Shadow DOM/slot/custom elements/命名访问阴影
use super::super::*;
use super::common::setup_runtime;

/// obscura #930 family: slot assignment APIs exist. A slot OUTSIDE any
/// shadow tree has no assignment and no fallback — the spec-correct
/// answer is empty, which keeps feature-detect code on its slot-aware
/// path instead of crashing on `undefined`. `assignedSlots` itself is
/// NOT a member anywhere on Chrome 145's surface (shape audit: the
/// engine's Element-level ghost left with the 刀55 kitchen sink), so a
/// light child answers undefined there.
#[test]
fn slot_assignment_apis_are_present_and_empty() {
    let mut rt = setup_runtime(r#"<slot id="s"><span id="c"></span></slot>"#);
    let assigned = rt
        .evaluate(
            "const s = document.getElementById('s'); \
                 [typeof s.assignedElements, s.assignedElements().length, \
                  typeof s.assignedNodes, s.assignedNodes().length, \
                  String(document.getElementById('c').assignedSlots)].join('|')",
        )
        .unwrap();
    assert_eq!(
        assigned,
        serde_json::json!("function|0|function|0|undefined")
    );
    let is_slot = rt
        .evaluate("document.getElementById('s') instanceof HTMLSlotElement")
        .unwrap();
    assert_eq!(is_slot, serde_json::json!(true));
}

/// Native shadow trees: attachShadow registers a real arena root with
/// its own child list, so the ordinary nid-based Node methods operate on
/// the shadow tree — while the light tree, document queries, and shadow
/// queries each stay inside their own scope.
#[test]
fn attach_shadow_builds_a_real_scoped_tree() {
    let mut rt = setup_runtime(r#"<div id="host"><span id="light">L</span></div>"#);
    let out = rt
        .evaluate(
            "const host = document.getElementById('host'); \
                 const sr = host.attachShadow({ mode: 'open' }); \
                 sr.innerHTML = '<p id=\"shadow-p\">S</p>'; \
                 [ \
                   sr instanceof ShadowRoot, \
                   sr instanceof DocumentFragment, \
                   sr.nodeType, \
                   sr.nodeName, \
                   sr.host === host, \
                   host.shadowRoot === sr, \
                   sr.querySelector('#shadow-p').textContent, \
                   document.querySelector('#shadow-p') === null, \
                   document.getElementById('shadow-p') === null, \
                   host.querySelector('#light') !== null, \
                   sr.querySelector('#light') === null, \
                   sr.firstChild.nodeType, \
                   sr.textContent \
                 ].join('|')",
        )
        .unwrap();
    assert_eq!(
        out,
        serde_json::json!("true|true|11|#document-fragment|true|true|S|true|true|true|true|1|S")
    );
}

/// The declarative face of the same contract (#87): a
/// `<template shadowrootmode>` parsed straight from HTML upgrades into
/// the native shadow root the platform builds at parse time — the
/// shell is gone, the host's light tree keeps only real children, and
/// JS sees the root exactly as if attachShadow had run.
#[test]
fn declarative_shadow_template_upgrades_at_parse() {
    let mut rt = setup_runtime(
        r#"<div id="host"><template shadowrootmode="open"><p id="shadow-p">S</p></template><span id="light">L</span></div>"#,
    );
    let out = rt
        .evaluate(
            "const host = document.getElementById('host'); \
                 const sr = host.shadowRoot; \
                 [ \
                   sr instanceof ShadowRoot, \
                   sr.host === host, \
                   sr.querySelector('#shadow-p').textContent, \
                   document.querySelector('#shadow-p') === null, \
                   host.querySelector('#light') !== null, \
                   host.children.length, \
                   document.querySelector('template') === null \
                 ].join('|')",
        )
        .unwrap();
    assert_eq!(out, serde_json::json!("true|true|S|true|true|1|true"));
}

/// Shadow-root identity contract: closed mode hides the root from the
/// host property, getRootNode answers per scope (default stops at the
/// ShadowRoot, composed crosses the host edge), connectivity follows the
/// host, re-attachment throws, and there is no public constructor.
#[test]
fn shadow_root_identity_get_root_and_connectivity() {
    let mut rt = setup_runtime(r#"<div id="host"></div>"#);
    let out = rt
        .evaluate(
            "const host = document.getElementById('host'); \
                 const sr = host.attachShadow({ mode: 'closed' }); \
                 sr.innerHTML = '<span id=\"in-shadow\"></span>'; \
                 const inner = sr.querySelector('#in-shadow'); \
                 const results = [ \
                   host.shadowRoot === null, \
                   inner.getRootNode() === sr, \
                   inner.getRootNode({ composed: true }) === document, \
                   inner.isConnected === true, \
                   sr.isConnected === true \
                 ]; \
                 try { host.attachShadow({ mode: 'open' }); results.push(false); } \
                 catch (e) { results.push(e.name === 'NotSupportedError'); } \
                 try { new ShadowRoot(); results.push(false); } \
                 catch (e) { results.push(e instanceof TypeError); } \
                 results.join('|')",
        )
        .unwrap();
    assert_eq!(out, serde_json::json!("true|true|true|true|true|true|true"));
}

/// The DocumentFragment-unwrap paths (appendChild/insertBefore/
/// replaceChild) splice a fragment's children into the parent — a
/// ShadowRoot shares nodeType/nodeName with a fragment and must NOT be
/// unwrapped, or appending a root would destructively empty the shadow
/// tree. Detached hosts: content is connected exactly when the host is.
#[test]
fn shadow_content_is_never_unwrapped_and_follows_host_connectivity() {
    let mut rt = setup_runtime(r#"<div id="host"></div>"#);
    let out = rt
        .evaluate(
            "const host = document.getElementById('host'); \
                 const sr = host.attachShadow({ mode: 'open' }); \
                 sr.innerHTML = '<b>one</b>'; \
                 document.body.appendChild(sr); \
                 const guardHeld = sr.innerHTML === '<b>one</b>' \
                   && document.body.querySelector('b') === null; \
                 const detached = document.createElement('div'); \
                 const sr2 = detached.attachShadow({ mode: 'open' }); \
                 sr2.innerHTML = '<i>x</i>'; \
                 const inner = sr2.querySelector('i'); \
                 const whileDetached = inner.isConnected === false; \
                 document.body.appendChild(detached); \
                 [guardHeld, whileDetached, inner.isConnected === true].join('|')",
        )
        .unwrap();
    assert_eq!(out, serde_json::json!("true|true|true"));
}

/// Slot assignment real values over a native shadow tree: exact name
/// matching (slot attr ↔ slot name), first unnamed slot absorbs
/// nameless light children, an unreferenced name leaves assignedSlots
/// empty, and an unassigned slot serves its fallback children.
#[test]
fn slot_assignment_serves_light_children_through_the_shadow() {
    let mut rt = setup_runtime(
        r#"<my-host id="h"><span slot="a" id="sa">A</span><span id="anon">B</span><span slot="nobody" id="nb"></span></my-host>"#,
    );
    let out = rt
            .evaluate(
                "const host = document.getElementById('h'); \
                 const sr = host.attachShadow({ mode: 'open' }); \
                 sr.innerHTML = '<slot name=\"a\"></slot><slot id=\"def\"></slot><slot name=\"ghost\">fallback</slot>'; \
                 const named = sr.querySelector('slot[name=\"a\"]'); \
                 const def = sr.getElementById('def'); \
                 const ghost = sr.querySelector('slot[name=\"ghost\"]'); \
                 const sa = document.getElementById('sa'); \
                 const anon = document.getElementById('anon'); \
                 const nb = document.getElementById('nb'); \
                 [ \
                   named.assignedNodes().length, \
                   named.assignedNodes()[0] === sa, \
                   named.assignedElements().length, \
                   typeof sa.assignedSlots, \
                   sa.assignedSlot === null, \
                   def.assignedNodes().length, \
                   def.assignedNodes()[0] === anon, \
                   def.assignedElements().length, \
                   ghost.assignedNodes().length, \
                   ghost.assignedNodes()[0].textContent, \
                   ghost.assignedElements().length \
                 ].join('|')",
            )
            .unwrap();
    assert_eq!(
        out,
        serde_json::json!("1|true|1|undefined|true|1|true|1|1|fallback|0")
    );
}

/// Shadow layout phase 2, composition: a shadow host's box contains its
/// shadow tree, not its light children — gBCR on shadow content reports
/// real composed-tree geometry, while unassigned light children report
/// nothing (no box, per spec).
#[cfg(feature = "screenshot")]
#[test]
fn shadow_content_renders_unassigned_light_children_do_not() {
    let mut rt = setup_runtime(r#"<div id="host"><span id="orphan">hidden</span></div>"#);
    let result = rt
        .evaluate(
            r#"
            const host = document.getElementById('host');
            const sr = host.attachShadow({ mode: 'open' });
            sr.innerHTML = '<div id="in-shadow" style="width:60px;height:30px"></div>';
            const inner = sr.getElementById('in-shadow');
            const r = inner.getBoundingClientRect();
            const orphan = document.getElementById('orphan').getBoundingClientRect();
            const hostR = host.getBoundingClientRect();
            return [r.width, r.height, orphan.width, orphan.height,
                    hostR.width >= r.width - 0.5, hostR.height >= r.height - 0.5];
        "#,
        )
        .unwrap();
    let parts = result.as_array().expect("array result");
    assert_eq!(parts[0], serde_json::json!(60), "shadow box has real width");
    assert_eq!(
        parts[1],
        serde_json::json!(30),
        "shadow box has real height"
    );
    assert_eq!(
        parts[2],
        serde_json::json!(0),
        "unassigned light child renders nothing"
    );
    assert_eq!(parts[3], serde_json::json!(0), "no height either");
    assert_eq!(
        parts[4],
        serde_json::json!(true),
        "host wraps the shadow box"
    );
    assert_eq!(
        parts[5],
        serde_json::json!(true),
        "host height tracks shadow content"
    );
}

/// Slot composition: slotted light children render AT their slot's
/// position inside the shadow tree, and a slot with no assignment serves
/// its fallback children in place.
#[cfg(feature = "screenshot")]
#[test]
fn slotted_light_children_render_at_the_slot_position() {
    let mut rt = setup_runtime(
        r#"<my-el id="h"><b slot="t" id="slotted" style="display:block;width:44px;height:11px">S</b></my-el>"#,
    );
    let result = rt.evaluate(r#"
            const host = document.getElementById('h');
            const sr = host.attachShadow({ mode: 'open' });
            sr.innerHTML = '<div style="height:10px" id="pad"></div><slot name="t" id="sl"></slot><slot name="none" id="fb"><i id="fi" style="display:block;width:22px;height:7px"></i></slot>';
            const slotted = document.getElementById('slotted').getBoundingClientRect();
            const pad = sr.getElementById('pad').getBoundingClientRect();
            const fb = sr.getElementById('fi').getBoundingClientRect();
            const slotBox = sr.getElementById('sl').getBoundingClientRect();
            return [slotted.width, slotted.height,
                    Math.abs(slotted.y - (pad.y + pad.height)) < 0.5,
                    fb.width, slotBox.width];
        "#).unwrap();
    let parts = result.as_array().expect("array result");
    assert_eq!(
        parts[0],
        serde_json::json!(44),
        "slotted child keeps its own size"
    );
    assert_eq!(parts[1], serde_json::json!(11), "slotted child height");
    assert_eq!(
        parts[2],
        serde_json::json!(true),
        "slotted child sits at the slot's position (after the pad div)"
    );
    assert_eq!(
        parts[3],
        serde_json::json!(22),
        "fallback child renders in the empty slot"
    );
    assert_eq!(
        parts[4],
        serde_json::json!(0),
        "the slot element itself never boxes"
    );
}

/// Flat-tree inheritance through the slot, pinned to local Chrome
/// (2026-09-16, headless 152): inherited properties reach a slotted
/// light child FROM THE SLOT — `color` declared on the slot colors the
/// slotted span, and with it undeclared the host's value walks through.
/// (The classic "slotted content only takes ::slotted rules" gotcha is
/// about selector matching crossing the boundary, not inheritance.)
#[cfg(feature = "screenshot")]
#[test]
fn slotted_content_inherits_through_the_slot() {
    let mut rt = setup_runtime(
        r#"<div id="outer" style="color:rgb(0,0,255)"><my-el id="h1"><span id="s1">a</span></my-el><my-el id="h2"><span id="s2">b</span></my-el></div>"#,
    );
    let result = rt
        .evaluate(
            r#"
            const sr1 = document.getElementById('h1').attachShadow({ mode: 'open' });
            sr1.innerHTML = '<slot style="color:rgb(255,0,0)"></slot>';
            const sr2 = document.getElementById('h2').attachShadow({ mode: 'open' });
            sr2.innerHTML = '<slot></slot>';
            return [getComputedStyle(document.getElementById('s1')).color,
                    getComputedStyle(sr1.querySelector('slot')).color,
                    getComputedStyle(document.getElementById('s2')).color,
                    getComputedStyle(sr2.querySelector('slot')).color];
        "#,
        )
        .unwrap();
    let parts = result.as_array().expect("array result");
    assert_eq!(
        parts[0],
        serde_json::json!("rgb(255, 0, 0)"),
        "color declared on the slot inherits into the slotted light child"
    );
    assert_eq!(
        parts[1],
        serde_json::json!("rgb(255, 0, 0)"),
        "the slot itself computes its inline color (not a JS-side fallback)"
    );
    assert_eq!(
        parts[2],
        serde_json::json!("rgb(0, 0, 255)"),
        "with the slot undeclared, blue walks outer -> host -> slot -> slotted"
    );
    assert_eq!(
        parts[3],
        serde_json::json!("rgb(0, 0, 255)"),
        "shadow content itself inherits from the host"
    );
}

/// Shadow `<style>` joins the CSS pool and styles shadow content; rules
/// match by class across the tree scopes (global-pool approximation).
#[cfg(feature = "screenshot")]
#[test]
fn shadow_style_sheets_style_shadow_content() {
    let mut rt = setup_runtime(r#"<div id="host"></div>"#);
    let result = rt.evaluate(r#"
            const host = document.getElementById('host');
            const sr = host.attachShadow({ mode: 'open' });
            sr.innerHTML = '<style>.card { width: 120px; height: 55px; }</style><div class="card" id="c"></div>';
            const c = sr.getElementById('c');
            const r = c.getBoundingClientRect();
            const cs = getComputedStyle(c);
            return [r.width, r.height, cs.width];
        "#).unwrap();
    let parts = result.as_array().expect("array result");
    assert_eq!(
        parts[0],
        serde_json::json!(120),
        "shadow style sizes the card"
    );
    assert_eq!(
        parts[1],
        serde_json::json!(55),
        "height from the shadow sheet"
    );
    assert_eq!(
        parts[2],
        serde_json::json!("120px"),
        "getComputedStyle sees the shadow rule"
    );
}

/// obscura#993 lineage: custom elements created AFTER define() must upgrade.
/// craigslist's search shell defines <cl-search-result> at boot, then
/// document.createElement's one per result inside the fetch callback — the
/// parse-time sweep in define() never sees those, so the shell rendered an
/// empty list. createElement must hand back an already-upgraded element
/// (methods attached, connectedCallback held while detached) and appendChild
/// must fire the callback so the element can render its own children.
#[test]
fn custom_elements_created_after_define_upgrade_on_insert() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let js = r#"
        globalThis.__log = [];
        customElements.define('ce-after', class extends HTMLElement {
          connectedCallback() {
            globalThis.__log.push('ccb');
            const a = document.createElement('a');
            a.className = 'hit'; a.textContent = 'go';
            this.appendChild(a);
          }
        });
        const el = document.createElement('ce-after');
        const bornUpgraded = typeof el.connectedCallback === 'function';
        const silentWhileDetached = globalThis.__log.join('') === '';
        document.body.appendChild(el);
        const out = {
          bornUpgraded: bornUpgraded,
          silentWhileDetached: silentWhileDetached,
          firedOnInsert: globalThis.__log.join('') === 'ccb',
          rendered: document.querySelectorAll('ce-after a.hit').length === 1,
          text: document.querySelector('ce-after a.hit').textContent
        };
        out
    "#;
    let v = rt.evaluate(js).unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "bornUpgraded": true,
            "silentWhileDetached": true,
            "firedOnInsert": true,
            "rendered": true,
            "text": "go"
        })
    );
}

/// The markup half of the same hole: elements entering through innerHTML
/// markup (never seen by createElement) must upgrade on the connected
/// container. Scripts inside innerHTML stay inert per spec — elements do not.
#[test]
fn custom_elements_in_innerhtml_markup_upgrade_when_connected() {
    let mut rt = setup_runtime("<html><body><div id='host'></div></body></html>");
    let js = r#"
        globalThis.__n = 0;
        customElements.define('ce-markup', class extends HTMLElement {
          connectedCallback() { globalThis.__n++; this.setAttribute('data-seen', 'yes'); }
        });
        const host = document.getElementById('host');
        host.innerHTML = '<p>x</p><ce-markup></ce-markup>';
        const fired = globalThis.__n === 1;
        const seen = document.querySelector('ce-markup').getAttribute('data-seen');
        host.innerHTML = '<ce-markup id="two"></ce-markup>';
        ({ fired: fired, seen: seen, refire: globalThis.__n === 2 })
    "#;
    let v = rt.evaluate(js).unwrap();
    assert_eq!(
        v,
        serde_json::json!({ "fired": true, "seen": "yes", "refire": true })
    );
}

/// Chrome's connect/disconnect cycle: an upgraded element leaving the
/// document fires disconnectedCallback, and a later re-insertion fires
/// connectedCallback again — move/reshuffle logic in lit-style frameworks
/// depends on the pairing.
#[test]
fn custom_elements_reconnect_refires_connected_callback() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let js = r#"
        globalThis.__log = [];
        customElements.define('ce-cycle', class extends HTMLElement {
          connectedCallback() { globalThis.__log.push('c'); }
          disconnectedCallback() { globalThis.__log.push('d'); }
        });
        const el = document.createElement('ce-cycle');
        document.body.appendChild(el);
        document.body.removeChild(el);
        document.body.appendChild(el);
        globalThis.__log.join('')
    "#;
    let v = rt.evaluate(js).unwrap();
    assert_eq!(v, serde_json::json!("cdc"));
}

/// #55: upgrades never ran the author's constructor. The old code called
/// `cls.call(el)`, which throws for any real class — ES classes refuse a
/// plain .call, Babel's _classCallCheck refuses a foreign `this` — and the
/// registry's inner catch swallowed the throw, so constructor field
/// initialization (shadow roots, reactive stores) never landed and every
/// custom element stayed a dead shell. Upgrade must drive the real
/// [[Construct]] path with `this` rebound to the live element, on all three
/// upgrade routes: define-sweep of pre-existing markup, createElement
/// born-upgrade, and innerHTML insertion.
#[test]
fn custom_elements_upgrade_runs_constructor_field_init() {
    let mut rt = setup_runtime("<html><body><ce-field></ce-field></body></html>");
    let js = r#"
        globalThis.__ran = 0;
        class FieldEl extends HTMLElement {
          constructor() {
            super();
            globalThis.__ran++;
            this._store = { n: 7 };
            this.attachShadow({ mode: 'open' });
          }
          get storeN() { return this._store.n; }
        }
        customElements.define('ce-field', FieldEl);
        const swept = document.querySelector('ce-field');
        const made = document.createElement('ce-field');
        document.body.appendChild(made);
        const host = document.createElement('div');
        host.innerHTML = '<ce-field></ce-field>';
        document.body.appendChild(host);
        const markup = host.querySelector('ce-field');
        ({
          sweepFields: swept._store !== undefined && swept._store.n === 7,
          sweepShadow: !!swept.shadowRoot,
          sweepInstanceOf: swept instanceof FieldEl,
          sweepMethod: swept.storeN === 7,
          madeFields: made._store !== undefined && made._store.n === 7,
          madeShadow: !!made.shadowRoot,
          markupFields: markup._store !== undefined,
          ctorRanOnAllRoutes: globalThis.__ran === 3,
          identityHoldsAfterUpgrade: document.querySelector('ce-field') === swept
        })
    "#;
    let v = rt.evaluate(js).unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "sweepFields": true,
            "sweepShadow": true,
            "sweepInstanceOf": true,
            "sweepMethod": true,
            "madeFields": true,
            "madeShadow": true,
            "markupFields": true,
            "ctorRanOnAllRoutes": true,
            "identityHoldsAfterUpgrade": true
        })
    );
}

/// #55's dogfood shape: xhs's <xhs-publish-btn> is a Babel-compiled
/// function-class (not native class syntax) whose super-call goes through
/// Reflect.construct(HTMLElement, [], Derived) — Babel's _callSuper. The
/// upgrade path must land its field initialization exactly like a native
/// class: _possibleConstructorReturn adopts the object the base constructor
/// returned (the live element) and every `_classCallCheck`/field write that
/// follows targets it.
#[test]
fn custom_elements_babel_style_class_upgrades_with_fields() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let js = r#"
        globalThis.__log = [];
        function _check(self, t) {
          if (!(self instanceof t)) throw new TypeError('Cannot call a class as a function');
        }
        function _possible(self, call) {
          if (call && (typeof call === 'object' || typeof call === 'function')) return call;
          throw new TypeError('Super expression must either be null or a function');
        }
        // Babel ES5 shape: function class, prototype chain wired by hand,
        // super via Reflect.construct(HTMLElement, [], Derived).
        function BabelCE() {
          _check(this, BabelCE);
          var e = _possible(this, Reflect.construct(HTMLElement, [], BabelCE));
          e._sr = e.attachShadow({ mode: 'open' });
          e._props = { isPublish: 'yes' };
          e._listener = function (ev) { globalThis.__log.push('pub'); };
          e.addEventListener('publish', e._listener);
          return e;
        }
        Object.setPrototypeOf(BabelCE, HTMLElement);
        BabelCE.prototype = Object.create(HTMLElement.prototype);
        BabelCE.prototype.constructor = BabelCE;
        BabelCE.prototype.readBack = function () { return this._props.isPublish; };
        customElements.define('ce-babel', BabelCE);
        const el = document.createElement('ce-babel');
        document.body.appendChild(el);
        el.dispatchEvent(new CustomEvent('publish'));
        ({
          fieldsLanded: el._props !== undefined && el._props.isPublish === 'yes',
          shadowLanded: !!el.shadowRoot,
          instanceof: el instanceof BabelCE,
          methodWorks: el.readBack() === 'yes',
          listenerWired: globalThis.__log.join('') === 'pub'
        })
    "#;
    let v = rt.evaluate(js).unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "fieldsLanded": true,
            "shadowLanded": true,
            "instanceof": true,
            "methodWorks": true,
            "listenerWired": true
        })
    );
}

/// #55 second face: Chrome lets page code mint an element with
/// new (customElements.get(name))() — detached, constructor run,
/// connectedCallback waiting for insertion, identity equal to every later
/// lookup. Unregistered construction (new HTMLElement(), or a subclass
/// never passed to define) keeps throwing the Illegal constructor.
#[test]
fn custom_elements_construct_via_registry_new_is_authorized() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let js = r#"
        globalThis.__log = [];
        const cls = class MintedEl extends HTMLElement {
          constructor() { super(); this._made = true; }
          connectedCallback() { globalThis.__log.push('c'); }
        };
        customElements.define('ce-mint', cls);
        const el = new (customElements.get('ce-mint'))();
        const detachedQuiet = globalThis.__log.join('') === '';
        const wasDetached = el.isConnected === false; // read before insertion
        el.id = 'minted';
        document.body.appendChild(el);
        ({
          tagName: el.tagName,
          detached: wasDetached && detachedQuiet,
          fieldsRan: el._made === true,
          instanceof: el instanceof cls,
          firedOnInsert: globalThis.__log.join('') === 'c',
          identityAfterInsert: document.getElementById('minted') === el
        })
    "#;
    let v = rt.evaluate(js).unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "tagName": "CE-MINT",
            "detached": true,
            "fieldsRan": true,
            "instanceof": true,
            "firedOnInsert": true,
            "identityAfterInsert": true
        })
    );
}

/// The authorization gate's negative space (#55): direct construction of
/// the engine's interface classes stays illegal — `new HTMLElement()` and
/// unregistered subclasses throw the same Illegal constructor Chrome does,
/// so the nid gate can't be bypassed by page code that merely looks like a
/// registered definition.
#[test]
fn custom_elements_unregistered_construction_still_throws() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let js = r#"
        let direct = '', sub = '';
        try { new HTMLElement(); } catch (e) { direct = e.name; }
        const Unregistered = class extends HTMLElement {};
        try { new Unregistered(); } catch (e) { sub = e.name; }
        ({ direct: direct, sub: sub })
    "#;
    let v = rt.evaluate(js).unwrap();
    assert_eq!(
        v,
        serde_json::json!({ "direct": "TypeError", "sub": "TypeError" })
    );
}

/// attributeChangedCallback never fired (upgrade, parser-created, setAttribute
/// …), so micro-app's <micro-app name url> never learned its app name and the
/// WeChat store goods page stayed blank (2026-11-03). The expected log is the
/// real-Chrome answer (Chrome 156 headless, same script, line for line).
#[tokio::test(flavor = "current_thread")]
async fn custom_element_attribute_changed_like_chrome() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let v = rt.evaluate(r##"(function () {
        const log=[];class X extends HTMLElement{static get observedAttributes(){return ["name","url","class","id"]}constructor(){super();log.push("ctor:"+this.getAttribute("name"))}attributeChangedCallback(n,o,v,ns){log.push("acc:"+n+"="+o+">"+v+(ns?"@"+ns:""))}connectedCallback(){log.push("conn")}}
var d=document.createElement("div");d.innerHTML="<x-pre name=a url=b other=q></x-pre>";document.body.appendChild(d);customElements.define("x-pre",X);log.push("#define");
var d2=document.createElement("div");d2.innerHTML="<x-pre name=c></x-pre>";log.push("#innerHTML-detached");document.body.appendChild(d2);log.push("#append");
var e=document.createElement("x-pre");log.push("#create");e.setAttribute("name","z");e.setAttribute("name","z");e.setAttribute("other","1");e.removeAttribute("name");e.removeAttribute("name");e.toggleAttribute("url");e.id="i1";e.className="k";e.classList.add("k2");e.setAttributeNS("urn:x","name","ns");log.push("#attrs");
var a=document.createAttribute("url");a.value="v1";e.setAttributeNode(a);a.value="v2";log.push("#attrnode");
var c=e.cloneNode();log.push("#clone");
class Y extends HTMLElement{attributeChangedCallback(n){log.push("Y:"+n)}}Y.observedAttributes=["p"];customElements.define("y-el",Y);var y=document.createElement("y-el");y.setAttribute("p","1");
class Z extends HTMLElement{static get observedAttributes(){return ["p"]}}customElements.define("z-el",Z);Z.prototype.attributeChangedCallback=function(n){log.push("Z-late:"+n)};var z=document.createElement("z-el");z.setAttribute("p","1");
        return JSON.stringify(log);
    })()"##).unwrap();
    let got: Vec<String> = serde_json::from_str(v.as_str().unwrap()).unwrap();
    let want: Vec<&str> = vec![
        "ctor:a",
        "acc:name=null>a",
        "acc:url=null>b",
        "conn",
        "#define",
        "ctor:c",
        "acc:name=null>c",
        "#innerHTML-detached",
        "conn",
        "#append",
        "ctor:null",
        "#create",
        "acc:name=null>z",
        "acc:name=z>z",
        "acc:name=z>null",
        "acc:url=null>",
        "acc:id=null>i1",
        "acc:class=null>k",
        "acc:class=k>k k2",
        "acc:name=null>ns@urn:x",
        "#attrs",
        "acc:url=>v1",
        "acc:url=v1>v2",
        "#attrnode",
        "ctor:ns",
        "acc:url=null>v2",
        "acc:id=null>i1",
        "acc:class=null>k k2",
        "acc:name=null>ns@urn:x",
        "#clone",
        "Y:p",
    ];
    assert_eq!(got, want);
}
