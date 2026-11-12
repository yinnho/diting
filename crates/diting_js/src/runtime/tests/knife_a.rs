//! WebIDL 真值电池 knife56-66（+TAGS 常量）
use super::super::*;
use super::common::setup_runtime;

/// M4 刀56: the wrong-prototype-level family folded. Shape truths:
/// (1) Attr.prototype owns exactly Chrome's 8 members — the 11
/// Node-reflecting leftovers (parentNode=null, childNodes=[], textContent
/// =value, …) now answer through the nodelevel dispatch table, not own
/// copies; (2) the Range boundary getters live on AbstractRange.prototype
/// ONLY — Range.prototype and StaticRange.prototype own none of the five;
/// (3) the EventTarget trio is inherited everywhere: none of the seven
/// former shims (BroadcastChannel/EventSource/IDBDatabase/IDBTransaction/
/// MediaQueryList/MessagePort/WebSocket) owns addEventListener/
/// removeEventListener/dispatchEvent. Behavior pins: attr tree answers,
/// the dispatcher's base fallback on elements, Range/StaticRange boundary
/// reads through the lifted getters, MQL change delivery through a native
/// listener + onchange in one flush.
#[tokio::test(flavor = "current_thread")]
async fn knife56_wrong_prototype_levels_fold_to_chrome() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let src = r#"
(() => {
  const r = {};
  // (1) Attr face + tree answers
  r.attrOwn = Object.getOwnPropertyNames(Attr.prototype).sort().join(',');
  const el = document.createElement('div');
  el.setAttribute('id', 'k56');
  const a = el.getAttributeNode('id');
  r.attrTree = [a.parentNode, a.parentElement, a.firstChild, a.lastChild,
    a.previousSibling, a.nextSibling, a.childNodes.length, a.hasChildNodes(),
    a.isConnected, a.nodeType, a.nodeName, a.textContent, a instanceof Node].join('|');
  a.textContent = 'zz';
  r.attrWriteThrough = el.getAttribute('id');
  // dispatcher base fallback: elements keep the real Node answers
  const span = document.createElement('span');
  document.body.appendChild(span);
  span.textContent = 't';
  r.elFallback = [span.isConnected, span.parentNode === document.body,
    span.firstChild && span.firstChild.nodeType, span.childNodes.length,
    span.ownerDocument === document, typeof span.cloneNode('deep')].join('|');
  // (2) AbstractRange owns the five, the leaves own none
  r.arOwn = Object.getOwnPropertyNames(AbstractRange.prototype).sort().join(',');
  r.leafOwn = ['Range', 'StaticRange'].map((n) => {
    const p = globalThis[n].prototype;
    return ['startContainer', 'startOffset', 'endContainer', 'endOffset', 'collapsed']
      .filter((k) => Object.prototype.hasOwnProperty.call(p, k)).length;
  }).join('|');
  const rng = document.createRange();
  rng.setStart(document.body, 0);
  rng.setEnd(document.body, 1);
  rng.collapse(true);
  const sr = new StaticRange({ startContainer: document.body, startOffset: 0,
    endContainer: document.body, endOffset: 2 });
  r.rangeRead = [rng.startContainer === document.body, rng.startOffset,
    rng.collapsed, sr.startContainer === document.body, sr.endOffset,
    sr.collapsed, rng instanceof AbstractRange, sr instanceof AbstractRange].join('|');
  // (3) the trio is inherited at every former shim
  r.trioOwn = ['BroadcastChannel', 'EventSource', 'IDBDatabase', 'IDBTransaction',
    'MediaQueryList', 'MessagePort', 'WebSocket'].map((n) => {
    const p = globalThis[n].prototype;
    return ['addEventListener', 'removeEventListener', 'dispatchEvent']
      .filter((k) => Object.prototype.hasOwnProperty.call(p, k)).length;
  }).join('|');
  r.etOwns = Object.getOwnPropertyNames(EventTarget.prototype).sort().join(',');
  return JSON.stringify(r);
})()
"#;
    let v: serde_json::Value =
        serde_json::from_str(rt.evaluate(src).unwrap().as_str().unwrap()).unwrap();
    // (1) Chrome 145 Attr.prototype owns exactly these 8.
    assert_eq!(
        v["attrOwn"].as_str().unwrap(),
        "constructor,localName,name,namespaceURI,ownerElement,prefix,specified,value",
        "Attr.prototype member set drifted"
    );
    assert_eq!(
        v["attrTree"].as_str().unwrap(),
        "||||||0|false|false|2|id|k56|true",
        "attr tree answers changed"
    );
    assert_eq!(
        v["attrWriteThrough"].as_str().unwrap(),
        "zz",
        "attr textContent write no longer reaches the element"
    );
    assert_eq!(
        v["elFallback"].as_str().unwrap(),
        "true|true|3|1|true|object",
        "element Node-member fallback broke"
    );
    // (2) Chrome 145 AbstractRange.prototype owns exactly these 6.
    assert_eq!(
        v["arOwn"].as_str().unwrap(),
        "collapsed,constructor,endContainer,endOffset,startContainer,startOffset",
        "AbstractRange.prototype member set drifted"
    );
    assert_eq!(
        v["leafOwn"].as_str().unwrap(),
        "0|0",
        "Range/StaticRange still own boundary getters"
    );
    assert_eq!(
        v["rangeRead"].as_str().unwrap(),
        "true|0|true|true|2|false|true|true",
        "range boundary reads broke"
    );
    // (3) zero own trio members at every former shim.
    assert_eq!(
        v["trioOwn"].as_str().unwrap(),
        "0|0|0|0|0|0|0",
        "a shim still owns trio members"
    );
    assert_eq!(
        v["etOwns"].as_str().unwrap(),
        "addEventListener,constructor,dispatchEvent,removeEventListener,when",
        "EventTarget.prototype member set drifted"
    );
}

/// M4 刀56 behavior: delivery through the native core — a BroadcastChannel
/// message reaches BOTH the onmessage handler and an addEventListener
/// listener exactly once each, as a REAL MessageEvent with the right
/// target; a MessagePort 'message' listener alone starts the port; a
/// MediaQueryList change fires its native listener and onchange in one
/// flush. These pin the event_hooks-registered path (listener-visible
/// liveness) that replaced the hand-rolled registries.
#[tokio::test(flavor = "current_thread")]
async fn knife56_native_core_delivery_single_fire() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate(
        r#"
(() => {
  window.__k56 = {};
  const a = new BroadcastChannel('k56t'), b = new BroadcastChannel('k56t');
  let on = 0, ls = 0, isME = null, data = null, tgt = null;
  b.onmessage = (e) => { on++; isME = e instanceof MessageEvent; data = e.data && e.data.v; tgt = e.target === b; };
  b.addEventListener('message', () => ls++);
  a.postMessage({ v: 7 });
  window.__k56.bcDone = () => [on, ls, isME, data, tgt].join('|');
  const mc = new MessageChannel();
  let n = 0;
  mc.port1.addEventListener('message', () => n++);
  mc.port2.postMessage(1);
  mc.port2.postMessage(2);
  window.__k56.portCount = () => n;
  const mq = matchMedia('(min-width: 500px)');
  let mls = 0, mon = 0;
  mq.addEventListener('change', () => mls++);
  mq.onchange = () => mon++;
  // deterministic regardless of the default viewport: flip narrow then
  // wide — at least one flip happens either way, listener and onchange
  // must fire the same number of times, and the final state is wide.
  __diting_setViewport(300, 800);
  __diting_setViewport(1200, 800);
  window.__k56.mqlState = () => [mls === mon && mls >= 1, mq.matches, mls, mon].join('|');
})();
"#,
    )
    .unwrap();
    let _ = rt.run_event_loop_bounded(200).await;
    let v: serde_json::Value = serde_json::from_str(
        rt.evaluate("JSON.stringify({bc: window.__k56.bcDone(), port: window.__k56.portCount(), mql: window.__k56.mqlState()})")
            .unwrap()
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        v["bc"].as_str().unwrap(),
        "1|1|true|7|true",
        "BroadcastChannel delivery changed"
    );
    assert_eq!(
        v["port"].as_i64().unwrap(),
        2,
        "listener-only MessagePort did not start/deliver"
    );
    assert!(
        v["mql"].as_str().unwrap().starts_with("true|true"),
        "MQL change delivery changed: {}",
        v["mql"]
    );
}

/// M4 刀57 (2026-10-31): the singleton one-off extras leave — the
/// whole-surface audit's "extra" class reaches ZERO (58→17→0 over 刀55–57).
/// 17 members across 16 interfaces, three verdicts from Chrome 145 truth:
///   DELETE (11 pure inventions): AudioContext.playbackStats,
///     BaseAudioContext.renderQuantumSize, CSSCounterStyleRule.style,
///     CSSPropertyRule.style, CSSScopeRule.conditionText, Document.referrerPolicy,
///     GPUAdapter.requestAdapterInfo, LaunchQueue.setTargetURL,
///     Navigator.cpuPerformance, ResizeObserver.takeRecords, WebTransport.congestionControl
///   MOVE→CSSConditionRule (3): Media/Container/Supports conditionText now
///     inherit (Chrome owns it there); CSSScopeRule — chained under
///     CSSGroupingRule, NOT Condition — loses it entirely (scope rules answer
///     start/end, never conditionText).
///   MOVE→Credential/ShadowRoot (3): PasswordCredential id/type answer through
///     Credential.prototype's real getters (ctor sets _id/_type);
///     DocumentFragment lost its innerHTML pair (plain fragments and
///     template.content answer undefined, assignment is a no-parse expando —
///     Chrome-probed) and ShadowRoot owns its own rw:ec pair.
/// The fragment face had three internal users that rode the deleted setter:
/// template parsing (Element.innerHTML's template case), DocumentFragment.
/// cloneNode(deep) and Range.createContextualFragment — all three now parse
/// through the set_inner_html op directly.
#[test]
fn knife57_singleton_extras_fold_to_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt.evaluate(r#"
        (function () {
          const own = (C) => Object.getOwnPropertyNames(C.prototype).join(',');
          const st = document.createElement('style');
          st.textContent = '@media (min-width:100px){a{color:red}} @supports (display:grid){b{c:d}} @container card (min-width:400px){e{f:g}} @scope (div) to (p) { i {color:red} }';
          document.head.appendChild(st);
          const rs = {}; for (const r of st.sheet.cssRules) rs[r.constructor.name] = r;
          const tpl = document.createElement('template');
          tpl.innerHTML = '<s>s</s><t>t</t>';
          const df = document.createDocumentFragment();
          const df2 = document.createDocumentFragment(); df2.innerHTML = '<i>i</i>';
          const host = document.createElement('div'); document.body.appendChild(host);
          const sr = host.attachShadow({mode:'open'});
          sr.innerHTML = '<u>u</u>';
          const r = document.createRange();
          const host2 = document.createElement('div'); host2.innerHTML = 'xx<b>bb</b>'; document.body.appendChild(host2);
          r.selectNode(host2.firstChild.nextSibling);
          const cf = r.createContextualFragment('<i>cf</i>');
          const pc = new PasswordCredential({ id: 'u1', password: 'p1', name: 'N' });
          const dfc = document.createDocumentFragment(); dfc.appendChild(document.createElement('p'));
          const cl = dfc.cloneNode(true);
          return JSON.stringify({
            acOwn: own(AudioContext), baseOwn: own(BaseAudioContext),
            mediaOwn: own(CSSMediaRule), scopeOwn: own(CSSScopeRule),
            condOwn: own(CSSConditionRule), propOwn: own(CSSPropertyRule),
            counterOwn: own(CSSCounterStyleRule), credOwn: own(Credential),
            pcOwn: own(PasswordCredential), dfHas: Object.getOwnPropertyDescriptor(DocumentFragment.prototype, 'innerHTML') === undefined,
            srHas: typeof Object.getOwnPropertyDescriptor(ShadowRoot.prototype, 'innerHTML').get,
            roOwn: own(ResizeObserver), lqOwn: own(LaunchQueue), gpuOwn: own(GPUAdapter),
            wtOwn: own(WebTransport), navCPU: typeof navigator.cpuPerformance,
            docRP: typeof Object.getOwnPropertyDescriptor(Document.prototype, 'referrerPolicy'),
            cond: [rs['CSSMediaRule'].conditionText, rs['CSSSupportsRule'].conditionText, rs['CSSContainerRule'].conditionText],
            scopeCond: typeof rs['CSSScopeRule'].conditionText,
            cred: [pc.id, pc.type, pc.name, pc.password],
            sr: [sr.innerHTML, sr.firstChild.tagName],
            df: [df.innerHTML, df2.innerHTML, df2.childNodes.length, String(df2.firstChild)],
            tpl: [tpl.content.childNodes.length, tpl.content.firstChild.tagName, tpl.innerHTML],
            clone: [cl.childNodes.length, cl.firstChild.tagName],
            cf: [cf.childNodes.length, cf.firstChild.tagName],
          });
        })()
    "#).unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    // ---- shape truths (extra class = 0) ----
    assert_eq!(v["acOwn"], serde_json::json!("baseLatency,outputLatency,onerror,close,createMediaElementSource,createMediaStreamDestination,createMediaStreamSource,getOutputTimestamp,resume,suspend,constructor,sinkId,onsinkchange,setSinkId"));
    assert!(!v["baseOwn"].as_str().unwrap().contains("renderQuantumSize"));
    assert_eq!(v["mediaOwn"], serde_json::json!("media,constructor"));
    assert_eq!(v["scopeOwn"], serde_json::json!("start,end,constructor")); // 刀75: Chrome 145 CSSScopeRule.start/end
    assert_eq!(v["condOwn"], serde_json::json!("conditionText,constructor"));
    assert_eq!(
        v["propOwn"],
        serde_json::json!("name,syntax,inherits,initialValue,constructor")
    );
    // 刀66 grew the face to Chrome's eleven rw descriptors (+shape_fix order).
    assert_eq!(v["counterOwn"], serde_json::json!("name,system,symbols,additiveSymbols,negative,prefix,suffix,range,pad,speakAs,fallback,constructor"));
    assert_eq!(v["credOwn"], serde_json::json!("id,type,constructor"));
    assert_eq!(
        v["pcOwn"],
        serde_json::json!("password,name,iconURL,constructor")
    );
    assert_eq!(
        v["dfHas"],
        serde_json::json!(true),
        "DocumentFragment.prototype has no innerHTML"
    );
    assert_eq!(
        v["srHas"],
        serde_json::json!("function"),
        "ShadowRoot owns its innerHTML pair"
    );
    assert_eq!(
        v["roOwn"],
        serde_json::json!("disconnect,observe,unobserve,constructor")
    );
    assert_eq!(v["lqOwn"], serde_json::json!("setConsumer,constructor"));
    assert_eq!(
        v["gpuOwn"],
        serde_json::json!("features,limits,info,requestDevice,constructor")
    );
    assert!(!v["wtOwn"].as_str().unwrap().contains("congestionControl"));
    assert_eq!(v["navCPU"], serde_json::json!("undefined"));
    assert_eq!(v["docRP"], serde_json::json!("undefined"));
    // ---- behavior through the moved/deleted faces ----
    assert_eq!(
        v["cond"],
        serde_json::json!([
            "(min-width: 100px)",
            "(display:grid)",
            "card (min-width: 400px)"
        ]),
        "conditionText inherits from CSSConditionRule with real values"
    );
    assert_eq!(
        v["scopeCond"],
        serde_json::json!("undefined"),
        "scope rules never answer conditionText"
    );
    assert_eq!(
        v["cred"],
        serde_json::json!(["u1", "password", "N", "p1"]),
        "PasswordCredential answers id/type through Credential"
    );
    assert_eq!(
        v["sr"],
        serde_json::json!(["<u>u</u>", "U"]),
        "ShadowRoot innerHTML round-trips"
    );
    assert_eq!(
        v["df"],
        serde_json::json!([null, "<i>i</i>", 0, "null"]),
        "plain fragment: read undefined, assignment is a no-parse expando (Chrome truth)"
    );
    assert_eq!(
        v["tpl"],
        serde_json::json!([2, "S", "<s>s</s><t>t</t>"]),
        "template parsing survived the fragment face loss"
    );
    assert_eq!(
        v["clone"],
        serde_json::json!([1, "P"]),
        "fragment cloneNode(deep) parses through the op"
    );
    assert_eq!(
        v["cf"],
        serde_json::json!([1, "I"]),
        "createContextualFragment parses through the op"
    );
}

/// M4 刀58 (2026-10-31): ElementInternals' ARIAMixin face — 51 members
/// (role + 43 nullable-DOMString aria* + 7 element-ref aria*) the engine
/// lacked entirely, the audit's biggest missing family. Chrome-truth storage
/// semantics probed all three ways: the store is INTERNAL ONLY — the host's
/// aria-*/role content attributes never feed these getters, these setters
/// never touch the attributes, and the element's own ARIAMixin face (which
/// DOES reflect attributes) stays independent (el.ariaLabel='A' +
/// internals.ariaLabel='B' coexist). Element-ref faces store the array
/// as-is, no attribute sync either direction; null clears; [] stays empty.
#[test]
fn knife58_element_internals_aria_reflection_family() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt.evaluate(r#"
        (function () {
          if (!customElements.get('x-knife58')) customElements.define('x-knife58', class extends HTMLElement {});
          const el = document.createElement('x-knife58');
          document.body.appendChild(el);
          const inr = el.attachInternals();
          const el2 = document.createElement('span'); el2.id = 'k58b'; document.body.appendChild(el2);
          const d = Object.getOwnPropertyDescriptor(ElementInternals.prototype, 'ariaLabel');
          const dref = Object.getOwnPropertyDescriptor(ElementInternals.prototype, 'ariaLabelledByElements');
          const out = {};
          out.own = Object.getOwnPropertyNames(ElementInternals.prototype).length;
          out.absent = [inr.ariaLabel, inr.role, inr.ariaLabelledByElements, inr.ariaColIndexText];
          el.setAttribute('aria-label', 'FROM-ATTR');
          out.attrToGetter = [inr.ariaLabel, el.ariaLabel];
          inr.ariaLabel = 'L1';
          out.setLabel = [el.getAttribute('aria-label'), inr.ariaLabel, el.ariaLabel];
          out.coerce = (inr.ariaLabel = 5, inr.ariaLabel);
          out.nullClears = (inr.ariaLabel = null, inr.ariaLabel);
          inr.role = 'switch';
          out.role = [inr.role, el.getAttribute('role')];
          inr.ariaLabelledByElements = [el2];
          out.refSet = [inr.ariaLabelledByElements.length, inr.ariaLabelledByElements[0] === el2, el.getAttribute('aria-labelledby')];
          out.refNull = (inr.ariaLabelledByElements = null, inr.ariaLabelledByElements);
          out.refEmpty = (inr.ariaLabelledByElements = [], inr.ariaLabelledByElements.length);
          out.desc = [d.enumerable, d.configurable, typeof d.get, typeof d.set];
          out.descRef = [dref.enumerable, dref.configurable];
          out.hidden = Object.getOwnPropertyNames(inr).join(',');
          out.tag = Object.prototype.toString.call(inr);
          return JSON.stringify(out);
        })()
    "#).unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(
        v["own"],
        serde_json::json!(63),
        "Chrome's exact 63-member prototype face"
    );
    assert_eq!(
        v["absent"],
        serde_json::json!([null, null, null, null]),
        "absent faces read null"
    );
    assert_eq!(
        v["attrToGetter"],
        serde_json::json!([null, "FROM-ATTR"]),
        "content attribute does not feed the internals getter"
    );
    assert_eq!(
        v["setLabel"],
        serde_json::json!(["FROM-ATTR", "L1", "FROM-ATTR"]),
        "internals setter never touches the attribute; element face independent"
    );
    assert_eq!(
        v["coerce"],
        serde_json::json!("5"),
        "WebIDL DOMString coercion"
    );
    assert_eq!(v["nullClears"], serde_json::json!(null));
    assert_eq!(
        v["role"],
        serde_json::json!(["switch", null]),
        "role stores internally, attribute untouched"
    );
    assert_eq!(
        v["refSet"],
        serde_json::json!([1, true, null]),
        "element-ref stores the array, no attribute sync"
    );
    assert_eq!(v["refNull"], serde_json::json!(null));
    assert_eq!(v["refEmpty"], serde_json::json!(0));
    assert_eq!(
        v["desc"],
        serde_json::json!([true, true, "function", "function"]),
        "rw:ec"
    );
    assert_eq!(v["descRef"], serde_json::json!([true, true]));
    assert_eq!(
        v["hidden"],
        serde_json::json!(""),
        "_aria/_refs and ctor state stay hidden on the instance"
    );
    assert_eq!(v["tag"], serde_json::json!("[object ElementInternals]"));
}

/// M4 刀59 (2026-10-31): the SVG root face — SVGSVGElement's 34 missing
/// members + 3 zoom statics (audit 37 -> 0), SVGPoint's x/y, and the real
/// storage the face reaches through: the legacy SVG value types were
/// stub shells returning undefined. All semantics probed against Chrome
/// 145: x/y default 0-number while width/height default 100% resolving
/// against the 300x150 default viewport; attached lengths are a live VIEW
/// of the attribute (write-back both ways); animVal writes throw
/// NoModificationAllowedError; SVG numbers are float32 (5pt reads
/// 6.666666507720947); SVGMatrix ops are PURE (receiver never mutated);
/// zoomAndPan defaults 2, invalid setters land on 0, never sync the
/// attribute; suspendRedraw always returns 1; the redraw/animation clock
/// no-ops; hit-testing lists from attribute geometry.
#[test]
fn knife59_svg_root_face_and_value_types() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt.evaluate(r#"
        (function () {
          const NS = 'http://www.w3.org/2000/svg';
          const svg = document.createElementNS(NS, 'svg');
          document.body.appendChild(svg);
          const mkc = (cx, cy, r, id) => {
            const c = document.createElementNS(NS, 'circle');
            c.setAttribute('cx', cx); c.setAttribute('cy', cy); c.setAttribute('r', r);
            if (id) c.id = id;
            svg.appendChild(c);
            return c;
          };
          const near = mkc(10, 10, 5, 'near');
          mkc(500, 500, 2, 'far');
          const rect = svg.createSVGRect();
          rect.x = 0; rect.y = 0; rect.width = 100; rect.height = 100;
          const out = {};
          out.proto = Object.getOwnPropertyNames(Object.getPrototypeOf(svg));
          out.statics = [SVGSVGElement.SVG_ZOOMANDPAN_DISABLE, SVGSVGElement.SVG_ZOOMANDPAN_MAGNIFY, SVGSVGElement.SVG_ZOOMANDPAN_UNKNOWN,
                         SVGSVGElement.prototype.SVG_ZOOMANDPAN_DISABLE];
          out.xyw = [svg.x.baseVal.value, svg.x.baseVal.unitType, svg.x.baseVal.valueAsString,
                     svg.width.baseVal.value, svg.width.baseVal.unitType, svg.width.baseVal.valueAsString,
                     svg.height.baseVal.value, svg.y.baseVal.value];
          out.live = [svg.x === svg.x, svg.x.baseVal === svg.x.baseVal, svg.x.baseVal === svg.x.animVal];
          out.attrIn = (() => { svg.setAttribute('width', '100'); svg.setAttribute('x', '5pt');
                                return [svg.width.baseVal.value, svg.x.baseVal.value, svg.width.animVal.value]; })();
          out.attrOut = (() => { svg.x.baseVal.value = 3; return [svg.x.baseVal.value, svg.getAttribute('x')]; })();
          out.animWrite = (() => { try { svg.width.animVal.value = 5; return 'no-throw'; } catch (e) { return e.name; } })();
          out.par = [svg.preserveAspectRatio.baseVal.align, svg.preserveAspectRatio.baseVal.meetOrSlice,
                     svg.preserveAspectRatio === svg.preserveAspectRatio,
                     svg.preserveAspectRatio.baseVal === svg.preserveAspectRatio.animVal];
          out.parAttr = (() => { svg.setAttribute('preserveAspectRatio', 'xMinYMax slice');
                                 return [svg.preserveAspectRatio.baseVal.align, svg.preserveAspectRatio.baseVal.meetOrSlice]; })();
          out.parOut = (() => { svg.preserveAspectRatio.baseVal.align = 7; return svg.getAttribute('preserveAspectRatio'); })();
          out.zoom = [svg.zoomAndPan, (svg.zoomAndPan = 1, svg.zoomAndPan), svg.getAttribute('zoomAndPan'),
                      (svg.zoomAndPan = 5, svg.zoomAndPan), (svg.zoomAndPan = 0, svg.zoomAndPan)];
          out.scale = [svg.currentScale, (svg.currentScale = 2, svg.currentScale)];
          out.ct = (() => { const p = svg.currentTranslate; p.x = 5; return [svg.currentTranslate === p, svg.currentTranslate.x]; })();
          out.factories = ['createSVGAngle','createSVGLength','createSVGMatrix','createSVGNumber','createSVGPoint','createSVGRect','createSVGTransform']
            .map((n) => svg[n]().constructor.name);
          out.angle = (() => { const a = svg.createSVGAngle(); return [a.value, a.valueAsString, a.unitType, a.valueInSpecifiedUnits]; })();
          out.angleSet = (() => { const a = svg.createSVGAngle(); a.newValueSpecifiedUnits(2, 90); return [a.unitType, a.value, a.valueInSpecifiedUnits, a.valueAsString]; })();
          out.angleValue = (() => { const a = svg.createSVGAngle(); a.value = 30; return [a.value, a.valueAsString, a.unitType]; })();
          out.len = (() => { const l = svg.createSVGLength(); l.valueAsString = '10mm'; return [l.value, l.valueAsString, l.unitType]; })();
          out.lenBad = (() => { try { svg.createSVGLength().valueAsString = 'abc'; return 'no-throw'; } catch (e) { return e.name; } })();
          out.lenPct = (() => { const l = svg.createSVGLength(); l.valueAsString = '50%'; try { l.value; return 'read-ok'; } catch (e) { return e.name; } })();
          out.lenConv = (() => { const l = svg.createSVGLength(); l.newValueSpecifiedUnits(9, 72); return [l.value, l.unitType]; })();
          out.lenConvert = (() => { const l = svg.createSVGLength(); l.newValueSpecifiedUnits(9, 72); l.convertToSpecifiedUnits(10); return [l.value, l.unitType, l.valueAsString]; })();
          out.num = (() => { const n = svg.createSVGNumber(); n.value = 7; return n.value; })();
          out.pt = (() => { const p = svg.createSVGPoint(); p.x = 3; p.y = 4; return [p.x, p.y, p.matrixTransform(svg.createSVGMatrix()).x]; })();
          out.ptProto = [Object.getOwnPropertyNames(SVGPoint.prototype).join(','), SVGPoint.length, (() => { try { new SVGPoint(1, 2); return 'no-throw'; } catch (e) { return e.name; } })()];
          out.rect0 = (() => { const r = svg.createSVGRect(); r.width = 9; return [r.x, r.width]; })();
          out.matrix = (() => { const m = svg.createSVGMatrix(); return [m.a, m.d, m.e, m.f]; })();
          out.matrixPure = (() => { const m = svg.createSVGMatrix(); m.a = 2; const t = m.translate(1, 2);
                                    return [m.a, m.e, m.f, t.a, t.e, t.f, t === m]; })();
          out.matrixInv = (() => { const m = svg.createSVGMatrix(); m.a = 2; m.d = 2; return m.inverse().a; })();
          out.tfm = (() => { const t = svg.createSVGTransform(); return [t.type, t.angle, t.matrix === t.matrix, t.matrix.a]; })();
          out.tfmCopy = (() => { const m = svg.createSVGMatrix(); m.e = 4; const t = svg.createSVGTransformFromMatrix(m);
                                 m.e = 9; return [t.type, t.matrix.e]; })();
          out.tfmRot = (() => { const t = svg.createSVGTransform(); t.setRotate(90, 0, 0);
                                return [t.type, t.angle, t.matrix.a, t.matrix.d]; })();
          out.clock = [svg.getCurrentTime(), (svg.setCurrentTime(1.5), svg.getCurrentTime()), svg.animationsPaused()];
          out.redraw = [svg.suspendRedraw(10), svg.suspendRedraw(1), svg.unsuspendRedraw(0), svg.unsuspendRedrawAll(), svg.forceRedraw(), svg.deselectAll()];
          out.hit = [svg.getIntersectionList(rect, null).length, svg.getEnclosureList(rect, null).length,
                     svg.checkIntersection(near, rect), svg.checkEnclosure(near, rect)];
          out.hitNone = (() => { const z = svg.createSVGRect(); const l = svg.getIntersectionList(z, null);
                                 return [l === null, l.length]; })();
          out.hitRef = (() => { const g = document.createElementNS(NS, 'g'); svg.appendChild(g); g.appendChild(near);
                                return svg.getIntersectionList(rect, g).length; })();
          out.gebi = [svg.getElementById('near') === near, svg.getElementById('nope') === null];
          out.hidden = Object.getOwnPropertyNames(svg).filter((k) => k[0] === '_').join(',');
          return JSON.stringify(out);
        })()
    "#).unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    let want = serde_json::json!([
        "x",
        "y",
        "width",
        "height",
        "currentScale",
        "currentTranslate",
        "viewBox",
        "preserveAspectRatio",
        "zoomAndPan",
        "SVG_ZOOMANDPAN_UNKNOWN",
        "SVG_ZOOMANDPAN_DISABLE",
        "SVG_ZOOMANDPAN_MAGNIFY",
        "animationsPaused",
        "checkEnclosure",
        "checkIntersection",
        "createSVGAngle",
        "createSVGLength",
        "createSVGMatrix",
        "createSVGNumber",
        "createSVGPoint",
        "createSVGRect",
        "createSVGTransform",
        "createSVGTransformFromMatrix",
        "deselectAll",
        "forceRedraw",
        "getCurrentTime",
        "getElementById",
        "getEnclosureList",
        "getIntersectionList",
        "pauseAnimations",
        "setCurrentTime",
        "suspendRedraw",
        "unpauseAnimations",
        "unsuspendRedraw",
        "unsuspendRedrawAll",
        "constructor"
    ]);
    assert_eq!(v["proto"], want, "Chrome's exact 36-member order");
    assert_eq!(
        v["statics"],
        serde_json::json!([1, 2, 0, 1]),
        "zoom constants on ctor and proto"
    );
    assert_eq!(
        v["xyw"],
        serde_json::json!([0, 1, "0", 300, 2, "100%", 150, 0]),
        "x/y default 0-number; width/height default 100% against the 300x150 viewport"
    );
    assert_eq!(
        v["live"],
        serde_json::json!([true, true, false]),
        "live wrapper, stable baseVal, distinct animVal"
    );
    assert_eq!(
        v["attrIn"],
        serde_json::json!([100, 6.666666507720947, 100]),
        "attribute feeds the view; float32 storage"
    );
    assert_eq!(
        v["attrOut"],
        serde_json::json!([3, "2.25pt"]),
        "baseVal.value writes back in the CURRENT unit (probed: 3px on a pt length -> 2.25pt)"
    );
    assert_eq!(
        v["animWrite"],
        serde_json::json!("NoModificationAllowedError")
    );
    assert_eq!(
        v["par"],
        serde_json::json!([6, 1, true, false]),
        "default xMidYMid meet, live wrapper"
    );
    assert_eq!(
        v["parAttr"],
        serde_json::json!([8, 2]),
        "attribute parses into align/meetOrSlice"
    );
    assert_eq!(v["parOut"], serde_json::json!("xMaxYMid slice"), "align setter writes the full keyword back, KEEPING meetOrSlice (probed after a slice attr)");
    assert_eq!(
        v["zoom"],
        serde_json::json!([2, 1, null, 0, 0]),
        "default 2, valid stored, invalid lands 0, no attribute sync"
    );
    assert_eq!(v["scale"], serde_json::json!([1, 2]));
    assert_eq!(
        v["ct"],
        serde_json::json!([false, 5]),
        "fresh SVGPoint each read, shared storage keeps writes"
    );
    assert_eq!(
        v["factories"],
        serde_json::json!([
            "SVGAngle",
            "SVGLength",
            "SVGMatrix",
            "SVGNumber",
            "SVGPoint",
            "SVGRect",
            "SVGTransform"
        ])
    );
    assert_eq!(v["angle"], serde_json::json!([0, "0", 1, 0]));
    assert_eq!(v["angleSet"], serde_json::json!([2, 90, 90, "90deg"]));
    assert_eq!(v["angleValue"], serde_json::json!([30, "30", 1]));
    assert_eq!(
        v["len"],
        serde_json::json!([37.7952766418457, "10mm", 7]),
        "float32 mm conversion"
    );
    assert_eq!(v["lenBad"], serde_json::json!("SyntaxError"));
    assert_eq!(
        v["lenPct"],
        serde_json::json!("NotSupportedError"),
        "detached % has no resolution context"
    );
    assert_eq!(v["lenConv"], serde_json::json!([96, 9]), "72pt = 96px");
    assert_eq!(
        v["lenConvert"],
        serde_json::json!([96, 10, "6pc"]),
        "convert keeps px, reformats string"
    );
    assert_eq!(v["num"], serde_json::json!(7));
    assert_eq!(
        v["pt"],
        serde_json::json!([3, 4, 3]),
        "matrixTransform computes"
    );
    assert_eq!(
        v["ptProto"],
        serde_json::json!(["x,y,matrixTransform,constructor", 0, "TypeError"]),
        "x/y prototype accessors; 刀91 ctor always Illegal constructor, length 0"
    );
    assert_eq!(v["rect0"], serde_json::json!([0, 9]));
    assert_eq!(
        v["matrix"],
        serde_json::json!([1, 1, 0, 0]),
        "identity default"
    );
    assert_eq!(
        v["matrixPure"],
        serde_json::json!([2, 0, 0, 2, 2, 2, false]),
        "translate returns a NEW matrix; receiver untouched"
    );
    assert_eq!(v["matrixInv"], serde_json::json!(0.5));
    assert_eq!(
        v["tfm"],
        serde_json::json!([1, 0, true, 1]),
        "MATRIX type, cached identity matrix"
    );
    assert_eq!(
        v["tfmCopy"],
        serde_json::json!([1, 4]),
        "createSVGTransformFromMatrix copies"
    );
    assert_eq!(
        v["tfmRot"],
        serde_json::json!([4, 90, 6.123233995736766e-17, 6.123233995736766e-17]),
        "Chrome's exact cos(90°)"
    );
    assert_eq!(
        v["clock"],
        serde_json::json!([0, 0, false]),
        "no SMIL: clock reads 0 even after setCurrentTime"
    );
    assert_eq!(
        v["redraw"],
        serde_json::json!([1, 1, null, null, null, null]),
        "suspendRedraw always 1; the rest undefined"
    );
    assert_eq!(
        v["hit"],
        serde_json::json!([1, 1, true, true]),
        "circle in rect: one hit, enclosed"
    );
    assert_eq!(
        v["hitNone"],
        serde_json::json!([false, 0]),
        "empty result is an empty NodeList, not null"
    );
    assert_eq!(
        v["hitRef"],
        serde_json::json!(1),
        "reference element scopes the walk"
    );
    assert_eq!(v["gebi"], serde_json::json!([true, true]));
    assert_eq!(
        v["hidden"],
        serde_json::json!(""),
        "engine helper state stays hidden on the element"
    );
}

/// 刀60: GPUSupportedLimits is pure prototype shape — 31 ro:ec limit getters,
/// `constructor` moved LAST in-file so the raw capture order already equals
/// Chrome truth (no ORDER entry, no reorder). No instance is reachable in
/// either world (Chrome for Testing 145 headless and the engine both answer
/// requestAdapter() with null — probed), so the getters read an internal
/// `_lim` store and answer undefined on bare objects, exactly what an
/// un-slotted read would show.
#[test]
fn knife60_gpu_supported_limits_face() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt.evaluate(r#"
        (function () {
          const TRUTH = ['maxTextureDimension1D', 'maxTextureDimension2D', 'maxTextureDimension3D',
            'maxTextureArrayLayers', 'maxBindGroups', 'maxBindGroupsPlusVertexBuffers',
            'maxBindingsPerBindGroup', 'maxDynamicUniformBuffersPerPipelineLayout',
            'maxDynamicStorageBuffersPerPipelineLayout', 'maxSampledTexturesPerShaderStage',
            'maxSamplersPerShaderStage', 'maxStorageBuffersPerShaderStage',
            'maxStorageTexturesPerShaderStage', 'maxUniformBuffersPerShaderStage',
            'maxUniformBufferBindingSize', 'maxStorageBufferBindingSize',
            'minUniformBufferOffsetAlignment', 'minStorageBufferOffsetAlignment',
            'maxVertexBuffers', 'maxBufferSize', 'maxVertexAttributes',
            'maxVertexBufferArrayStride', 'maxInterStageShaderVariables', 'maxColorAttachments',
            'maxColorAttachmentBytesPerSample', 'maxComputeWorkgroupStorageSize',
            'maxComputeInvocationsPerWorkgroup', 'maxComputeWorkgroupSizeX',
            'maxComputeWorkgroupSizeY', 'maxComputeWorkgroupSizeZ',
            'maxComputeWorkgroupsPerDimension'].concat(['constructor']);
          const P = GPUSupportedLimits.prototype;
          const names = Object.getOwnPropertyNames(P);
          const order = names.length === 32 && names.every((n, i) => n === TRUTH[i]);
          const d = Object.getOwnPropertyDescriptor(P, 'maxTextureDimension1D');
          const flags = [!!d && typeof d.get === 'function', d.set === undefined, !!d && d.enumerable, !!d && d.configurable];
          const bare = Object.create(P).maxTextureDimension2D;
          const o = Object.create(P);
          o._lim = { maxTextureDimension2D: 8192 };
          const plumb = o.maxTextureDimension2D;
          const named = TRUTH.slice(0, 31).filter((n) => Object.getOwnPropertyDescriptor(P, n).get.name === n).length;
          const statics = Object.getOwnPropertyNames(GPUSupportedLimits).sort();
          return JSON.stringify({ order, flags, bare, plumb, named, statics });
        })()
    "#).unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(
        v["order"],
        serde_json::json!(true),
        "32 own names, constructor LAST, truth order"
    );
    assert_eq!(
        v["flags"],
        serde_json::json!([true, true, true, true]),
        "ro:ec — getter, no setter"
    );
    assert_eq!(
        v["bare"],
        serde_json::json!(null),
        "un-slotted read answers undefined"
    );
    assert_eq!(
        v["plumb"],
        serde_json::json!(8192),
        "getters read the internal _lim store"
    );
    assert_eq!(
        v["named"],
        serde_json::json!(31),
        "every getter carries its limit's own name"
    );
    assert_eq!(
        v["statics"],
        serde_json::json!(["length", "name", "prototype"]),
        "no statics"
    );
}

/// 刀61: RTCPeerConnection face — 46 truth members (12 state getters, 11
/// on*-handlers as proto accessors, methods) + generateCertificate static +
/// Chrome's error taxonomy: WebIDL type checks (TypeError) before closed
/// guards (InvalidStateError) before algorithm errors; setConfiguration
/// locks bundlePolicy/rtcpMuxPolicy/encodedInsertableStreams at construction
/// (InvalidModificationError) while iceServers/iceCandidatePoolSize/
/// iceTransportPolicy stay mutable with full replace semantics; iceServers
/// entries normalize with credential/urls/username keys always present as
/// strings. All probed against Chrome for Testing 145. Known divergence,
/// engine-wide (event_hooks): handler fires before listeners (['H','L'] vs
/// Chrome ['L','H']).
#[test]
fn knife61_rtc_peer_connection_face() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt.evaluate(r#"
        (function () {
          const out = {};
          const pc = new RTCPeerConnection();
          out.desc = [pc.localDescription, pc.currentLocalDescription, pc.pendingLocalDescription, pc.remoteDescription, pc.currentRemoteDescription, pc.pendingRemoteDescription];
          out.states = [pc.signalingState, pc.iceGatheringState, pc.iceConnectionState, pc.connectionState];
          out.cfg = pc.getConfiguration();
          out.keys = Object.keys(pc);
          const src = new RTCPeerConnection();
          const tr = src.addTransceiver('video').receiver.track;
          const ms = new MediaStream();
          const sender = pc.addTrack(tr, ms);
          out.senderBrand = Object.prototype.toString.call(sender);
          out.afterAdd = [pc.getSenders().length, pc.getTransceivers().length, pc.getLocalStreams().length];
          const t1 = pc.getTransceivers()[0];
          out.tr1 = [t1.mid, t1.direction, t1.currentDirection, t1.stopped, Object.prototype.toString.call(t1.sender), Object.prototype.toString.call(t1.receiver)];
          out.senderTrack = [sender.track === tr, sender.track instanceof MediaStreamTrack];
          const pc2 = new RTCPeerConnection();
          const t2 = pc2.addTransceiver('audio');
          out.recvTrack = [t2.receiver.track.kind, t2.receiver.track instanceof MediaStreamTrack];
          out.errors = [];
          const f = (fn) => { try { fn(); out.errors.push('no-throw'); } catch (e) { out.errors.push(e.name); } };
          f(() => pc.addTrack());
          f(() => pc.addTrack({}));
          f(() => pc.removeTrack());
          f(() => pc.removeTrack({}));
          f(() => pc2.removeTrack(sender));
          f(() => pc.addTransceiver('bogus'));
          f(() => pc.addTransceiver({}));
          f(() => pc.setConfiguration({ iceServers: [{ urls: 'not a url' }] }));
          f(() => pc.setConfiguration({ iceServers: [{ urls: 'stun:' }] }));
          f(() => pc.setConfiguration({ bundlePolicy: 'max-bundle' }));
          f(() => pc.setConfiguration({ rtcpMuxPolicy: 'negotiate' }));
          f(() => pc.setConfiguration({ encodedInsertableStreams: true }));
          const mut = new RTCPeerConnection();
          mut.setConfiguration({ iceServers: [{ urls: 'stun:stun.example.net', username: 7, credential: 8 }] });
          const g = mut.getConfiguration();
          out.mutEntry = [JSON.stringify(g.iceServers), Object.keys(g.iceServers[0]).join(',')];
          mut.setConfiguration({ iceCandidatePoolSize: 5 });
          out.mutReplace = [JSON.stringify(mut.getConfiguration().iceServers), mut.getConfiguration().iceCandidatePoolSize];
          const atr = t2.receiver.track;
          pc.addTrack(atr);
          const dtmf = pc.createDTMFSender(atr);
          out.dtmf = [dtmf instanceof RTCDTMFSender, dtmf.canInsertLetter, dtmf.duration, dtmf.interToneGap, dtmf.toneBuffer];
          f(() => pc.createDTMFSender(tr));
          out.genCert = [Object.prototype.toString.call(RTCPeerConnection.generateCertificate({ name: 'ECDSA', namedCurve: 'P-256' })), typeof RTCPeerConnection.generateCertificate];
          out.ctor = [RTCPeerConnection.length, Object.getOwnPropertyNames(RTCPeerConnection).join(',')];
          out.fire = (() => { let n = 0; pc.ontrack = function () { n++; }; pc.dispatchEvent(new Event('track')); pc.ontrack = null; pc.dispatchEvent(new Event('track')); return n; })();
          out.proto = Object.getOwnPropertyNames(RTCPeerConnection.prototype).length;
          out.roFlags = (() => { const d = Object.getOwnPropertyDescriptor(RTCPeerConnection.prototype, 'connectionState'); return [d.set === undefined, d.enumerable, d.configurable]; })();
          const closed = new RTCPeerConnection();
          closed.close();
          out.closed = [closed.signalingState, closed.iceConnectionState, closed.connectionState, closed.iceGatheringState];
          f(() => closed.addTransceiver('audio'));
          f(() => closed.setConfiguration({}));
          return JSON.stringify(out);
        })()
    "#).unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(
        v["desc"],
        serde_json::json!([null, null, null, null, null, null]),
        "fresh pc: all six descriptions null"
    );
    assert_eq!(
        v["states"],
        serde_json::json!(["stable", "new", "new", "new"]),
        "fresh states"
    );
    assert_eq!(
        v["cfg"],
        serde_json::json!({
            "bundlePolicy": "balanced", "encodedInsertableStreams": false, "iceCandidatePoolSize": 0,
            "iceServers": [], "iceTransportPolicy": "all", "rtcpMuxPolicy": "require"
        }),
        "six-key default dictionary"
    );
    assert_eq!(
        v["keys"],
        serde_json::json!([]),
        "Chrome's pc carries no own enumerable keys"
    );
    assert_eq!(v["senderBrand"], serde_json::json!("[object RTCRtpSender]"));
    assert_eq!(
        v["afterAdd"],
        serde_json::json!([1, 1, 1]),
        "sender+transceiver minted, stream recorded"
    );
    assert_eq!(
        v["tr1"],
        serde_json::json!([
            null,
            "sendrecv",
            null,
            false,
            "[object RTCRtpSender]",
            "[object RTCRtpReceiver]"
        ])
    );
    assert_eq!(
        v["senderTrack"],
        serde_json::json!([true, true]),
        "sender.track plumbs the given track"
    );
    assert_eq!(
        v["recvTrack"],
        serde_json::json!(["audio", true]),
        "addTransceiver('audio') receiver track"
    );
    assert_eq!(v["errors"], serde_json::json!([
        "TypeError", "TypeError",                       // addTrack: missing arg, non-track
        "TypeError", "TypeError",                       // removeTrack: missing arg, non-sender
        "InvalidAccessError",                           // foreign sender
        "TypeError", "TypeError",                       // addTransceiver: bogus string, non-kind object
        "SyntaxError", "SyntaxError",                   // setConfiguration: non-url, hostless stun:
        "InvalidModificationError", "InvalidModificationError", "InvalidModificationError", // bundle/rtcpMux/eis locked
        "SyntaxError",                                  // createDTMFSender on a video track
        "InvalidStateError", "InvalidStateError"        // closed pc: addTransceiver + setConfiguration
    ]), "Chrome's error taxonomy: WebIDL type checks, then algorithm errors, then immutability locks, then closed guards");
    assert_eq!(
        v["mutEntry"],
        serde_json::json!([
            "[{\"credential\":\"8\",\"urls\":[\"stun:stun.example.net\"],\"username\":\"7\"}]",
            "credential,urls,username"
        ]),
        "entries normalize: keys always present, numbers coerced, alphabetical order"
    );
    assert_eq!(
        v["mutReplace"],
        serde_json::json!(["[]", 5]),
        "setConfiguration replaces the whole dictionary (probed Chrome)"
    );
    assert_eq!(
        v["dtmf"],
        serde_json::json!([true, null, null, null, ""]),
        "DTMF sender face"
    );
    assert_eq!(
        v["genCert"],
        serde_json::json!(["[object Promise]", "function"]),
        "static generateCertificate"
    );
    assert_eq!(
        v["ctor"],
        serde_json::json!([0, "length,name,prototype,generateCertificate"])
    );
    assert_eq!(
        v["fire"],
        serde_json::json!(1),
        "ontrack fires via the proto accessor; null clears"
    );
    assert_eq!(v["proto"], serde_json::json!(46), "truth member count");
    assert_eq!(
        v["roFlags"],
        serde_json::json!([true, true, true]),
        "state getters are ro:ec"
    );
    assert_eq!(
        v["closed"],
        serde_json::json!(["closed", "closed", "closed", "new"]),
        "close() leaves iceGatheringState at 'new'"
    );
    assert_eq!(
        v["errors"].as_array().map(|a| a.len()),
        Some(15),
        "closed guards: addTransceiver + setConfiguration both InvalidStateError"
    );
}

/// 刀62 — RTC family stragglers: MediaStreamTrack / RTCDataChannel /
/// RTCRtpTransceiver full faces + RTCTrackEvent / RTCPeerConnectionIceErrorEvent
/// rebinds, every member in Chrome 145 truth raw order (constructor last).
/// Behaviors probed against Chrome for Testing 145: MST `new` = Illegal
/// constructor, clone preserves readyState/label but mints a fresh unique id,
/// stop() = 'ended'; DC defaults (arraybuffer/0/0/null/connecting/''),
/// invalid binaryType silently ignored, send on non-open = InvalidStateError,
/// close() on connecting = 'closing'; transceiver direction ignores junk,
/// post-stop setter = InvalidStateError, header-extension tables audio 5 /
/// video 16, negotiated view all-'stopped', setHeaderExtensionsToNegotiate([])
/// = InvalidModificationError; TrackEvent requires receiver/track/transceiver
/// (TypeError) and freezes streams; IceErrorEvent requires errorCode, defaults
/// address/port null + url/errorText/hostCandidate ''. Known divergences
/// (recorded): getCapabilities/getSettings return {} (no real source),
/// engine canvas has no captureStream (tracks minted via addTransceiver).
#[test]
fn knife62_rtc_family_stragglers() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt.evaluate(r#"
        (function () {
          const out = {};
          out.mstOrder = Object.getOwnPropertyNames(MediaStreamTrack.prototype);
          out.mstParent = Object.getPrototypeOf(MediaStreamTrack.prototype) === EventTarget.prototype;
          out.mstCtor = (() => { try { new MediaStreamTrack(); return 'no-throw'; } catch (e) { return e.name; } })();
          const pc = new RTCPeerConnection();
          const tr = pc.addTransceiver('audio');
          const t = tr.receiver.track;
          out.track = [t.kind, typeof t.id, t.muted, t.readyState, t.enabled, t.contentHint, t.getConstraints(), t.getCaptureHandle(), t.stats];
          out.clone = (() => { const c = t.clone(); return [c.kind, c.readyState, c.id !== t.id]; })();
          out.stopped = (() => { const s = pc.addTransceiver('video').receiver.track; s.stop(); return [s.readyState, s.clone().readyState]; })();
          out.onfire = (() => { const f = pc.addTransceiver('audio').receiver.track; let n = 0; f.onmute = function () { n++; }; f.dispatchEvent(new Event('mute')); return n; })();
          const dc = pc.createDataChannel('lbl');
          out.dc = [dc.label, dc.ordered, dc.maxPacketLifeTime, dc.maxRetransmits, dc.protocol, dc.negotiated, dc.id, dc.readyState, dc.bufferedAmount, dc.bufferedAmountLowThreshold, dc.binaryType, dc.reliable];
          const dc2 = pc.createDataChannel('l2', { ordered: false, maxRetransmits: 3, protocol: 'p1', negotiated: true, id: 7 });
          out.dc2 = [dc2.ordered, dc2.maxRetransmits, dc2.protocol, dc2.negotiated, dc2.id, dc2.reliable];
          out.dcBin = (() => { dc.binaryType = 'blob'; const v = dc.binaryType; dc.binaryType = 'junk'; return [v, dc.binaryType]; })();
          out.dcErr = [(() => { try { dc.send('x'); return 'no-throw'; } catch (e) { return e.name; } })(), (() => { dc.close(); return dc.readyState; })()];
          out.dcOrder = Object.getOwnPropertyNames(RTCDataChannel.prototype);
          out.trOrder = Object.getOwnPropertyNames(RTCRtpTransceiver.prototype);
          out.trDir = (() => { tr.direction = 'recvonly'; const a = tr.direction; tr.direction = 'junk'; return [a, tr.direction]; })();
          out.trStop = (() => { tr.stop(); return [tr.stopped, tr.direction, tr.currentDirection, (() => { try { tr.direction = 'sendonly'; return 'no-throw'; } catch (e) { return e.name; } })()]; })();
          out.hdr = [pc.addTransceiver('audio').getHeaderExtensionsToNegotiate().length, pc.addTransceiver('video').getHeaderExtensionsToNegotiate().length, (() => { const x = pc.addTransceiver('audio'); return x.getNegotiatedHeaderExtensions().every((e) => e.direction === 'stopped') && x.getNegotiatedHeaderExtensions().length; })()];
          out.hdrSet = (() => { const x = pc.addTransceiver('audio'); const f = []; f.push((() => { try { x.setCodecPreferences([]); return 'ok'; } catch (e) { return e.name; } })()); f.push((() => { try { x.setCodecPreferences(null); return 'no-throw'; } catch (e) { return e.name; } })()); f.push((() => { try { x.setHeaderExtensionsToNegotiate([]); return 'no-throw'; } catch (e) { return e.name; } })()); f.push((() => { try { x.setHeaderExtensionsToNegotiate([{}]); return 'no-throw'; } catch (e) { return e.name; } })()); f.push((() => { try { x.setHeaderExtensionsToNegotiate(x.getHeaderExtensionsToNegotiate()); return x.getHeaderExtensionsToNegotiate().length; } catch (e) { return 'ERR:' + e.name; } })()); return f; })();
          out.teOrder = Object.getOwnPropertyNames(RTCTrackEvent.prototype);
          out.te = (() => { const e = new RTCTrackEvent('track', { receiver: tr.receiver, track: tr.receiver.track, transceiver: tr }); return [e.receiver === tr.receiver, e.track === tr.receiver.track, e.transceiver === tr, Object.prototype.toString.call(e.streams), e.streams.length, Object.isFrozen(e.streams), Object.prototype.toString.call(e)]; })();
          out.teErrs = [(() => { try { new RTCTrackEvent('track', {}); return 'no-throw'; } catch (e) { return e.name; } })(), (() => { try { new RTCTrackEvent('track', { receiver: tr.receiver, track: {}, transceiver: tr }); return 'no-throw'; } catch (e) { return e.name; } })()];
          out.ieeOrder = Object.getOwnPropertyNames(RTCPeerConnectionIceErrorEvent.prototype);
          out.iee = (() => { const e = new RTCPeerConnectionIceErrorEvent('x', { address: '1.2.3.4', port: 3478, url: 'turn:x', errorCode: 401, errorText: ' t ', hostCandidate: '5.6.7.8:1' }); return [e.address, e.port, e.url, e.errorCode, e.errorText, e.hostCandidate, Object.prototype.toString.call(e)]; })();
          out.ieeMin = (() => { const e = new RTCPeerConnectionIceErrorEvent('x', { errorCode: 401 }); return [e.address, e.port, e.url, e.errorCode, e.errorText, e.hostCandidate]; })();
          out.ieeEmpty = (() => { try { new RTCPeerConnectionIceErrorEvent('x', {}); return 'no-throw'; } catch (e) { return e.name; } })();
          return JSON.stringify(out);
        })()
    "#).unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(
        v["mstOrder"],
        serde_json::json!([
            "kind",
            "id",
            "label",
            "enabled",
            "muted",
            "onmute",
            "onunmute",
            "readyState",
            "onended",
            "stats",
            "contentHint",
            "applyConstraints",
            "clone",
            "getCapabilities",
            "getConstraints",
            "getSettings",
            "stop",
            "oncapturehandlechange",
            "getCaptureHandle",
            "constructor"
        ]),
        "MST truth raw order"
    );
    assert_eq!(
        v["mstParent"],
        serde_json::json!(true),
        "MST extends EventTarget"
    );
    assert_eq!(
        v["mstCtor"],
        serde_json::json!("TypeError"),
        "MST Illegal constructor"
    );
    assert_eq!(
        v["track"],
        serde_json::json!(["audio", "string", false, "live", true, "", {}, null, null]),
        "MST defaults"
    );
    assert_eq!(
        v["clone"],
        serde_json::json!(["audio", "live", true]),
        "clone keeps kind/readyState, fresh id"
    );
    assert_eq!(
        v["stopped"],
        serde_json::json!(["ended", "ended"]),
        "stop() + clone of stopped stays ended"
    );
    assert_eq!(v["onfire"], serde_json::json!(1), "onmute handler fires");
    assert_eq!(
        v["dc"],
        serde_json::json!([
            "lbl",
            true,
            null,
            null,
            "",
            false,
            null,
            "connecting",
            0,
            0,
            "arraybuffer",
            true
        ]),
        "DC defaults"
    );
    assert_eq!(
        v["dc2"],
        serde_json::json!([false, 3, "p1", true, 7, false]),
        "DC options parsed"
    );
    assert_eq!(
        v["dcBin"],
        serde_json::json!(["blob", "blob"]),
        "invalid binaryType ignored"
    );
    assert_eq!(
        v["dcErr"],
        serde_json::json!(["InvalidStateError", "closing"]),
        "send guard + close on connecting"
    );
    assert_eq!(
        v["dcOrder"],
        serde_json::json!([
            "label",
            "ordered",
            "maxPacketLifeTime",
            "maxRetransmits",
            "protocol",
            "negotiated",
            "id",
            "readyState",
            "bufferedAmount",
            "bufferedAmountLowThreshold",
            "onopen",
            "onbufferedamountlow",
            "onerror",
            "onclosing",
            "onclose",
            "onmessage",
            "binaryType",
            "reliable",
            "close",
            "send",
            "constructor"
        ]),
        "DC truth raw order"
    );
    assert_eq!(
        v["trOrder"],
        serde_json::json!([
            "mid",
            "sender",
            "receiver",
            "stopped",
            "direction",
            "currentDirection",
            "getHeaderExtensionsToNegotiate",
            "getNegotiatedHeaderExtensions",
            "setCodecPreferences",
            "setHeaderExtensionsToNegotiate",
            "stop",
            "constructor"
        ]),
        "transceiver truth raw order"
    );
    assert_eq!(
        v["trDir"],
        serde_json::json!(["recvonly", "recvonly"]),
        "junk direction silently ignored"
    );
    assert_eq!(
        v["trStop"],
        serde_json::json!([true, "stopped", null, "InvalidStateError"]),
        "stop() locks direction"
    );
    assert_eq!(
        v["hdr"],
        serde_json::json!([5, 16, 5]),
        "header-extension tables audio 5 / video 16"
    );
    assert_eq!(
        v["hdrSet"],
        serde_json::json!([
            "ok",
            "TypeError",
            "InvalidModificationError",
            "TypeError",
            5
        ]),
        "setHeaderExtensions taxonomy"
    );
    assert_eq!(
        v["teOrder"],
        serde_json::json!(["receiver", "track", "streams", "transceiver", "constructor"]),
        "TrackEvent truth raw order"
    );
    assert_eq!(
        v["te"],
        serde_json::json!([
            true,
            true,
            true,
            "[object Array]",
            0,
            true,
            "[object RTCTrackEvent]"
        ]),
        "TrackEvent identity + frozen streams"
    );
    assert_eq!(
        v["teErrs"],
        serde_json::json!(["TypeError", "TypeError"]),
        "TrackEvent required members"
    );
    assert_eq!(
        v["ieeOrder"],
        serde_json::json!([
            "address",
            "port",
            "hostCandidate",
            "url",
            "errorCode",
            "errorText",
            "constructor"
        ]),
        "IceErrorEvent truth raw order"
    );
    assert_eq!(
        v["iee"],
        serde_json::json!([
            "1.2.3.4",
            3478,
            "turn:x",
            401,
            " t ",
            "5.6.7.8:1",
            "[object RTCPeerConnectionIceErrorEvent]"
        ]),
        "IceErrorEvent values + brand"
    );
    assert_eq!(
        v["ieeMin"],
        serde_json::json!([null, null, "", 401, "", ""]),
        "IceErrorEvent defaults"
    );
    assert_eq!(
        v["ieeEmpty"],
        serde_json::json!("TypeError"),
        "errorCode required"
    );
}

#[test]
fn knife63_svg_family_five_faces() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt.evaluate(r#"
        (function () {
          const NS = 'http://www.w3.org/2000/svg';
          const E = (t) => document.createElementNS(NS, t);
          const t = (f) => { try { return f(); } catch (e) { return 'E:' + e.name; } };
          const out = {};
          // marker: defaults, orient tri-state, setOrient*, reflector identity
          const mk = E('marker');
          out.mkOrder = Object.getOwnPropertyNames(SVGMarkerElement.prototype);
          out.mkDef = [mk.refX.baseVal.value, mk.refY.baseVal.value, mk.markerWidth.baseVal.value, mk.markerHeight.baseVal.value,
                       mk.markerUnits.baseVal, mk.orientType.baseVal, mk.orientAngle.baseVal.value, mk.orientAngle.baseVal.unitType];
          out.mkIdent = [mk.orientAngle === mk.orientAngle, mk.orientAngle.baseVal !== mk.orientAngle.animVal];
          mk.setAttribute('orient', 'auto'); const o1 = mk.orientType.baseVal;
          mk.setAttribute('orient', 'auto-start-reverse'); const o2 = mk.orientType.baseVal;
          mk.setAttribute('orient', 'junk'); const o3 = [mk.orientType.baseVal, mk.orientAngle.baseVal.value];
          mk.setAttribute('orient', '45'); const o4 = [mk.orientType.baseVal, mk.orientAngle.baseVal.value];
          out.mkOrient = [o1, o2, o3, o4];
          const svg0 = E('svg');
          out.mkSetAngle = [t(() => { const a = svg0.createSVGAngle(); a.newValueSpecifiedUnits(1, 45); mk.setOrientToAngle(a); return mk.getAttribute('orient'); }),
                            t(() => { const a = svg0.createSVGAngle(); a.newValueSpecifiedUnits(2, 30); mk.setOrientToAngle(a); return mk.getAttribute('orient'); }),
                            t(() => { mk.setOrientToAngle(45); return 'no-throw'; })];
          out.mkAuto = t(() => { mk.setOrientToAuto(); return mk.getAttribute('orient'); });
          out.mkAngleWrite = t(() => { mk.orientAngle.baseVal.value = 20; return mk.getAttribute('orient'); });
          out.mkUnits = [t(() => { mk.setAttribute('markerUnits', 'userSpaceOnUse'); return mk.markerUnits.baseVal; }),
                         t(() => { mk.setAttribute('markerUnits', 'junk'); return mk.markerUnits.baseVal; })];
          out.mkWrite = t(() => { mk.markerWidth.baseVal.value = 8; return mk.getAttribute('markerWidth'); });
          out.mkStatic = [SVGMarkerElement.prototype.SVG_MARKERUNITS_STROKEWIDTH, SVGMarkerElement.SVG_MARKER_ORIENT_ANGLE,
                          Object.getOwnPropertyDescriptor(SVGMarkerElement.prototype, 'SVG_MARKERUNITS_UNKNOWN').enumerable];
          // animation: null target, on* coercion, no-op methods, dur parse
          const an = E('animate');
          out.anOrder = Object.getOwnPropertyNames(SVGAnimationElement.prototype);
          out.anTarget = an.targetElement;
          out.anOn = [t(() => { an.onbegin = function () {}; return typeof an.onbegin; }),
                      t(() => { an.onend = 5; return an.onend; }),
                      t(() => { an.onrepeat = 'x'; return an.onrepeat; })];
          out.anM = [an.beginElement(), an.beginElementAt(2), an.endElement(), an.endElementAt(2), an.getCurrentTime(), t(() => an.getStartTime())];
          out.anDur = [t(() => { an.setAttribute('dur', '5s'); return an.getSimpleDuration(); }),
                       t(() => { an.removeAttribute('dur'); return an.getSimpleDuration(); }),
                       t(() => { an.setAttribute('dur', '250ms'); return an.getSimpleDuration(); })];
          out.anLists = [an.requiredExtensions.length, an.requiredExtensions === an.requiredExtensions, an.systemLanguage.length,
                         Object.prototype.toString.call(an.requiredExtensions)];
          // text: no-layout nchars, textLength view, per-char throws
          const tx = E('text');
          out.txOrder = Object.getOwnPropertyNames(SVGTextContentElement.prototype);
          out.txN = [tx.getNumberOfChars(), tx.getComputedTextLength(), tx.getCharNumAtPosition({ x: 0, y: 0 })];
          out.txTL = [t(() => tx.textLength.baseVal.value),
                      t(() => { tx.textLength.baseVal.value = 60; return [tx.getAttribute('textLength'), tx.textLength.baseVal.value]; }),
                      t(() => { tx.textLength.baseVal.valueAsString = '70px'; return [tx.getAttribute('textLength'), tx.textLength.baseVal.unitType]; })];
          out.txLA = [tx.lengthAdjust.baseVal, t(() => { tx.setAttribute('lengthAdjust', 'spacingAndGlyphs'); return tx.lengthAdjust.baseVal; }),
                      t(() => { tx.setAttribute('lengthAdjust', 'junk'); return tx.lengthAdjust.baseVal; })];
          out.txChar = [t(() => tx.getSubStringLength(0, 0)), t(() => tx.getSubStringLength(0, 5)), t(() => tx.getStartPositionOfChar(0)),
                        t(() => tx.getEndPositionOfChar(0)), t(() => tx.getExtentOfChar(0)), t(() => tx.getRotationOfChar(0)), t(() => tx.selectSubString(0, 0))];
          out.txParent = Object.getPrototypeOf(SVGTextContentElement.prototype) === SVGGraphicsElement.prototype;
          // feDropShadow: number/string views, pair stdDeviation, internal override, FE-region %
          const fe = E('feDropShadow');
          out.feOrder = Object.getOwnPropertyNames(SVGFEDropShadowElement.prototype);
          out.feDef = [fe.dx.baseVal, fe.dy.baseVal, fe.stdDeviationX.baseVal, fe.stdDeviationY.baseVal,
                       fe.x.baseVal.valueInSpecifiedUnits, fe.x.baseVal.unitType, fe.in1.baseVal, fe.result.baseVal];
          out.feParent = Object.getPrototypeOf(SVGFEDropShadowElement.prototype) === SVGElement.prototype;
          out.feIn = t(() => { fe.in1.baseVal = 'SourceGraphic'; return [fe.getAttribute('in'), fe.in1.animVal]; });
          out.feDx = [t(() => { fe.dx.baseVal = 3.5; return [fe.getAttribute('dx'), fe.dx.baseVal]; }),
                      t(() => { fe.setAttribute('dx', 'junk'); return fe.dx.baseVal; })];
          out.feStd = [t(() => { fe.setAttribute('stdDeviation', '4 6'); return [fe.stdDeviationX.baseVal, fe.stdDeviationY.baseVal]; }),
                       t(() => { fe.setAttribute('stdDeviation', '5'); return [fe.stdDeviationX.baseVal, fe.stdDeviationY.baseVal]; })];
          out.feSet = [t(() => { fe.setStdDeviation(1.5, 2.5); return [fe.stdDeviationX.baseVal, fe.stdDeviationY.baseVal, fe.getAttribute('stdDeviation')]; }),
                       t(() => { fe.setStdDeviation(); return 'no-throw'; }), t(() => { fe.setStdDeviation('a', 'b'); return 'no-throw'; })];
          out.feX = [t(() => { fe.setAttribute('x', '-10%'); return [fe.x.baseVal.valueInSpecifiedUnits, fe.x.baseVal.unitType]; }),
                     t(() => fe.x.baseVal.value), t(() => { fe.setAttribute('x', '7'); return fe.x.baseVal.value; })];
          // pattern: enums, transform list view, PAR, string lists
          const pt = E('pattern');
          out.ptOrder = Object.getOwnPropertyNames(SVGPatternElement.prototype);
          out.ptDef = [pt.patternUnits.baseVal, pt.patternContentUnits.baseVal, pt.x.baseVal.value, pt.width.baseVal.value];
          out.ptU = [t(() => { pt.setAttribute('patternUnits', 'userSpaceOnUse'); return pt.patternUnits.baseVal; }),
                     t(() => { pt.setAttribute('patternUnits', 'junk'); return pt.patternUnits.baseVal; }),
                     t(() => { pt.setAttribute('patternContentUnits', 'objectBoundingBox'); return pt.patternContentUnits.baseVal; })];
          out.ptT = [t(() => { pt.setAttribute('patternTransform', 'rotate(30)'); return [pt.patternTransform.baseVal.numberOfItems, pt.patternTransform.baseVal.getItem(0).type, pt.patternTransform.baseVal.getItem(0).angle]; }),
                     t(() => { pt.setAttribute('patternTransform', ''); return pt.patternTransform.baseVal.getItem(0); }),
                     t(() => { pt.setAttribute('patternTransform', 'rotate(30)'); return [pt.patternTransform.baseVal !== pt.patternTransform.animVal, pt.patternTransform.baseVal === pt.patternTransform.baseVal]; })];
          out.ptPAR = pt.preserveAspectRatio.baseVal.align;
          out.ptLists = [pt.requiredExtensions.length, pt.systemLanguage.length];
          out.ptHrefKind = [Object.getOwnPropertyDescriptor(SVGPatternElement.prototype, 'href').set === undefined,
                            Object.prototype.toString.call(pt.href)];
          return JSON.stringify(out);
        })()
    "#).unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(
        v["mkOrder"],
        serde_json::json!([
            "refX",
            "refY",
            "markerUnits",
            "markerWidth",
            "markerHeight",
            "orientType",
            "orientAngle",
            "viewBox",
            "preserveAspectRatio",
            "SVG_MARKERUNITS_UNKNOWN",
            "SVG_MARKERUNITS_USERSPACEONUSE",
            "SVG_MARKERUNITS_STROKEWIDTH",
            "SVG_MARKER_ORIENT_UNKNOWN",
            "SVG_MARKER_ORIENT_AUTO",
            "SVG_MARKER_ORIENT_ANGLE",
            "setOrientToAngle",
            "setOrientToAuto",
            "constructor"
        ]),
        "marker truth raw order"
    );
    assert_eq!(
        v["mkDef"],
        serde_json::json!([0, 0, 3, 3, 2, 2, 0, 1]),
        "marker defaults (probed: width/height 3, orientType ANGLE 2)"
    );
    assert_eq!(
        v["mkIdent"],
        serde_json::json!([true, true]),
        "orientAngle reflector identity"
    );
    assert_eq!(
        v["mkOrient"],
        serde_json::json!([1, 0, [2, 0], [2, 45]]),
        "orient keyword tri-state (probed)"
    );
    assert_eq!(
        v["mkSetAngle"],
        serde_json::json!(["45", "30deg", "E:TypeError"]),
        "setOrientToAngle serializes value+unit, rejects non-SVGAngle (probed)"
    );
    assert_eq!(
        v["mkAuto"],
        serde_json::json!("auto"),
        "setOrientToAuto writes attr"
    );
    assert_eq!(
        v["mkAngleWrite"],
        serde_json::json!("20"),
        "orientAngle.baseVal.value write serializes"
    );
    assert_eq!(
        v["mkUnits"],
        serde_json::json!([1, 2]),
        "markerUnits keyword + junk->default (probed)"
    );
    assert_eq!(
        v["mkWrite"],
        serde_json::json!("8"),
        "markerWidth write-through"
    );
    assert_eq!(
        v["mkStatic"],
        serde_json::json!([2, 2, true]),
        "marker constants on proto+ctor"
    );
    assert_eq!(
        v["anOrder"],
        serde_json::json!([
            "targetElement",
            "onbegin",
            "onend",
            "onrepeat",
            "requiredExtensions",
            "systemLanguage",
            "beginElement",
            "beginElementAt",
            "endElement",
            "endElementAt",
            "getCurrentTime",
            "getSimpleDuration",
            "getStartTime",
            "constructor"
        ]),
        "animation truth raw order"
    );
    assert_eq!(
        v["anTarget"],
        serde_json::json!(null),
        "bare animate targetElement null (probed)"
    );
    assert_eq!(
        v["anOn"],
        serde_json::json!(["function", null, null]),
        "on* accessor coerces non-function to null (probed)"
    );
    assert_eq!(
        v["anM"],
        serde_json::json!([null, null, null, null, 0, "E:InvalidStateError"]),
        "animation no-ops + probed errors"
    );
    assert_eq!(
        v["anDur"],
        serde_json::json!([5, "E:NotSupportedError", 0.25]),
        "dur parse + missing (probed)"
    );
    assert_eq!(
        v["anLists"],
        serde_json::json!([0, true, 0, "[object SVGStringList]"]),
        "SVGStringList views identity-stable"
    );
    assert_eq!(
        v["txOrder"],
        serde_json::json!([
            "textLength",
            "lengthAdjust",
            "LENGTHADJUST_UNKNOWN",
            "LENGTHADJUST_SPACING",
            "LENGTHADJUST_SPACINGANDGLYPHS",
            "getCharNumAtPosition",
            "getComputedTextLength",
            "getEndPositionOfChar",
            "getExtentOfChar",
            "getNumberOfChars",
            "getRotationOfChar",
            "getStartPositionOfChar",
            "getSubStringLength",
            "selectSubString",
            "constructor"
        ]),
        "text truth raw order"
    );
    assert_eq!(
        v["txN"],
        serde_json::json!([0, 0, -1]),
        "no text layout: nchars 0 (probed)"
    );
    assert_eq!(
        v["txTL"],
        serde_json::json!([0, ["60", 60], ["70px", 5]]),
        "textLength view write-through (probed)"
    );
    assert_eq!(
        v["txLA"],
        serde_json::json!([1, 2, 1]),
        "lengthAdjust enum + junk->default (probed)"
    );
    assert_eq!(
        v["txChar"],
        serde_json::json!([
            0,
            "E:IndexSizeError",
            "E:IndexSizeError",
            "E:IndexSizeError",
            "E:IndexSizeError",
            "E:IndexSizeError",
            "E:IndexSizeError"
        ]),
        "per-char APIs throw on nchars 0 (probed)"
    );
    assert_eq!(
        v["txParent"],
        serde_json::json!(true),
        "text parent = SVGGraphicsElement"
    );
    assert_eq!(
        v["feOrder"],
        serde_json::json!([
            "in1",
            "dx",
            "dy",
            "stdDeviationX",
            "stdDeviationY",
            "x",
            "y",
            "width",
            "height",
            "result",
            "setStdDeviation",
            "constructor"
        ]),
        "feDropShadow truth raw order"
    );
    assert_eq!(
        v["feDef"],
        serde_json::json!([2, 2, 2, 2, 0, 2, "", ""]),
        "feDropShadow defaults (probed: dx/dy/std 2, region [0,%])"
    );
    assert_eq!(
        v["feParent"],
        serde_json::json!(true),
        "feDropShadow parent = SVGElement (empty intermediate cut)"
    );
    assert_eq!(
        v["feIn"],
        serde_json::json!(["SourceGraphic", "SourceGraphic"]),
        "in1 string view + animVal mirror (probed)"
    );
    assert_eq!(
        v["feDx"],
        serde_json::json!([["3.5", 3.5], 2]),
        "dx float view + junk->default (probed)"
    );
    assert_eq!(
        v["feStd"],
        serde_json::json!([[4, 6], [5, 5]]),
        "stdDeviation pair parse (probed)"
    );
    assert_eq!(v["feSet"], serde_json::json!([[1.5,2.5,"5"],"E:TypeError","E:TypeError"]), "setStdDeviation internal override (reads win, attr untouched — probed null only on attr-less element) + restricted float");
    assert_eq!(
        v["feX"],
        serde_json::json!([[-10, 2], "E:NotSupportedError", 7]),
        "FE region % never resolves (probed)"
    );
    assert_eq!(
        v["ptOrder"],
        serde_json::json!([
            "patternUnits",
            "patternContentUnits",
            "patternTransform",
            "x",
            "y",
            "width",
            "height",
            "viewBox",
            "preserveAspectRatio",
            "href",
            "requiredExtensions",
            "systemLanguage",
            "constructor"
        ]),
        "pattern truth raw order"
    );
    assert_eq!(
        v["ptDef"],
        serde_json::json!([2, 1, 0, 0]),
        "pattern defaults (probed)"
    );
    assert_eq!(
        v["ptU"],
        serde_json::json!([1, 2, 2]),
        "pattern enums + junk->default (probed)"
    );
    assert_eq!(
        v["ptT"],
        serde_json::json!([[1, 4, 30], "E:IndexSizeError", [true, true]]),
        "patternTransform view (probed: rotate type 4, OOR throws)"
    );
    assert_eq!(
        v["ptPAR"],
        serde_json::json!(6),
        "pattern PAR default xMidYMid"
    );
    assert_eq!(
        v["ptLists"],
        serde_json::json!([0, 0]),
        "pattern string lists"
    );
    assert_eq!(
        v["ptHrefKind"],
        serde_json::json!([true, "[object SVGAnimatedString]"]),
        "href ro:ec + animated-string view"
    );
}

#[test]
fn knife64_svg_geometry_reflector_family() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt.evaluate(r#"
        (function () {
          const NS = 'http://www.w3.org/2000/svg';
          const E = (t) => document.createElementNS(NS, t);
          const t = (f) => { try { return f(); } catch (e) { return 'E:' + e.name; } };
          const out = {};
          // rect: defaults are [0, number] on the whole family — NOT the 刀59 width/height heuristic
          const rc = E('rect');
          out.rcOrder = Object.getOwnPropertyNames(SVGRectElement.prototype);
          out.rcDef = [rc.x.baseVal.value, rc.x.baseVal.unitType, rc.width.baseVal.value, rc.rx.baseVal.unitType];
          rc.setAttribute('x', '50%');
          out.rcPct = t(() => rc.x.baseVal.value);
          rc.setAttribute('x', '7'); rc.setAttribute('width', '3.5em');
          out.rcRead = [rc.x.baseVal.value, rc.x.baseVal.valueAsString, rc.width.baseVal.valueInSpecifiedUnits, rc.width.baseVal.unitType];
          rc.x.baseVal.value = 42;
          out.rcWrite = [rc.getAttribute('x'), rc.x.baseVal.value];
          out.rcIdent = [rc.x === rc.x, rc.x.baseVal !== rc.x.animVal];
          const ci = E('circle'), el = E('ellipse'), ln = E('line'), fo = E('foreignObject');
          out.famDef = [ci.cx.baseVal.value, ci.r.baseVal.value, el.cy.baseVal.value, el.ry.baseVal.value,
                        ln.x1.baseVal.value, ln.y2.baseVal.value, fo.x.baseVal.value, fo.width.baseVal.value];
          // gradients: % defaults, % .value unresolvable, fx independent of cx
          const lg = E('linearGradient');
          out.lgOrder = Object.getOwnPropertyNames(SVGLinearGradientElement.prototype);
          out.lgDef = [lg.x1.baseVal.unitType, lg.x1.baseVal.valueInSpecifiedUnits, lg.x2.baseVal.valueInSpecifiedUnits,
                       lg.y1.baseVal.valueInSpecifiedUnits, lg.y2.baseVal.valueInSpecifiedUnits];
          lg.setAttribute('x1', '50%');
          out.lgPct = t(() => lg.x1.baseVal.value);
          lg.setAttribute('x1', '7');
          out.lgNum = lg.x1.baseVal.value;
          const rg = E('radialGradient');
          out.rgOrder = Object.getOwnPropertyNames(SVGRadialGradientElement.prototype);
          out.rgDef = [rg.cx.baseVal.unitType, rg.cx.baseVal.valueInSpecifiedUnits, rg.fx.baseVal.valueInSpecifiedUnits, rg.fr.baseVal.valueInSpecifiedUnits];
          rg.setAttribute('cx', '30%');
          out.rgFx = rg.fx.baseVal.valueInSpecifiedUnits;
          // stop: '%' reads /100, junk->0, writes serialize, never clamped
          const st = E('stop');
          out.stOrder = Object.getOwnPropertyNames(SVGStopElement.prototype);
          out.stOff = [st.offset.baseVal];
          st.setAttribute('offset', '0.5'); out.stOff.push(st.offset.baseVal);
          st.setAttribute('offset', '50%'); out.stOff.push(st.offset.baseVal);
          st.setAttribute('offset', 'junk'); out.stOff.push(st.offset.baseVal);
          st.setAttribute('offset', '-0.5'); out.stOff.push(st.offset.baseVal);
          st.setAttribute('offset', '1.5'); out.stOff.push(st.offset.baseVal);
          st.offset.baseVal = 0.75;
          out.stWrite = st.getAttribute('offset');
          // symbol: SVGAnimatedRect view of viewBox + PAR
          const sy = E('symbol');
          out.syOrder = Object.getOwnPropertyNames(SVGSymbolElement.prototype);
          out.syVB = [sy.viewBox.baseVal.width, sy.viewBox.baseVal.height, Object.prototype.toString.call(sy.viewBox)];
          sy.setAttribute('viewBox', '0 0 10 20');
          out.syVB2 = [sy.viewBox.baseVal.x, sy.viewBox.baseVal.width, sy.viewBox.baseVal.height, sy.viewBox === sy.viewBox];
          out.syPAR = sy.preserveAspectRatio.baseVal.align;
          // clipPath: units enum default 1 (NOT pattern's 2), transform list view
          const cp = E('clipPath');
          out.cpOrder = Object.getOwnPropertyNames(SVGClipPathElement.prototype);
          out.cpU = [cp.clipPathUnits.baseVal];
          cp.setAttribute('clipPathUnits', 'objectBoundingBox'); out.cpU.push(cp.clipPathUnits.baseVal);
          cp.setAttribute('clipPathUnits', 'junk'); out.cpU.push(cp.clipPathUnits.baseVal);
          cp.setAttribute('transform', 'rotate(30)');
          out.cpT = [Object.prototype.toString.call(cp.transform), cp.transform.baseVal.numberOfItems, cp.transform.baseVal.getItem(0).type];
          // textPositioning: length/number list views
          const tp = E('text');
          out.tpOrder = Object.getOwnPropertyNames(SVGTextPositioningElement.prototype);
          out.tpDef = [tp.x.baseVal.numberOfItems, tp.rotate.baseVal.numberOfItems, tp.x.baseVal !== tp.x.animVal];
          out.tpOOR = t(() => tp.x.baseVal.getItem(0));
          tp.setAttribute('x', '1 2 3');
          out.tpX = [tp.x.baseVal.length, tp.x.baseVal.numberOfItems];
          tp.setAttribute('y', '10px 5');
          out.tpY = [tp.y.baseVal.getItem(0).value, tp.y.baseVal.getItem(0).unitType, tp.y.baseVal.getItem(1).value, Object.prototype.toString.call(tp.y.baseVal.getItem(0))];
          tp.setAttribute('rotate', '90 60');
          out.tpRot = [Object.prototype.toString.call(tp.rotate.baseVal.getItem(0)), tp.rotate.baseVal.getItem(0).value, tp.rotate.baseVal.getItem(1).value];
          tp.rotate.baseVal.getItem(0).value = 45;
          out.tpRotW = tp.getAttribute('rotate');
          tp.setAttribute('x', '');
          out.tpClear = tp.x.baseVal.numberOfItems;
          // image: decoding/crossOrigin reflected legs, href view, decode()
          const im = E('image');
          out.imOrder = Object.getOwnPropertyNames(SVGImageElement.prototype);
          out.imDef = [im.decoding, im.crossOrigin, im.x.baseVal.value, im.preserveAspectRatio.baseVal.align];
          im.setAttribute('decoding', 'ASYNC'); out.imDec = [im.decoding];
          im.setAttribute('decoding', 'junk'); out.imDec.push(im.decoding);
          im.decoding = 'sync'; out.imDec.push([im.decoding, im.getAttribute('decoding')]);
          im.setAttribute('crossorigin', 'USE-CREDENTIALS'); out.imCo = [im.crossOrigin];
          im.setAttribute('crossorigin', ''); out.imCo.push(im.crossOrigin);
          im.crossOrigin = 'anonymous'; out.imCo.push([im.crossOrigin, im.getAttribute('crossorigin')]);
          im.removeAttribute('crossorigin'); out.imCo.push(im.crossOrigin);
          im.href.baseVal = 'a.png';
          out.imHref = [Object.prototype.toString.call(im.href), im.getAttribute('href'), im.href.animVal];
          out.imDecode = im.decode() instanceof Promise;
          // use: geometry + href view
          const us = E('use');
          out.usOrder = Object.getOwnPropertyNames(SVGUseElement.prototype);
          out.usDef = [us.x.baseVal.value, us.width.baseVal.value];
          us.href.baseVal = '#t';
          out.usHref = [Object.prototype.toString.call(us.href), us.getAttribute('href')];
          // value-type list brands
          out.brands = [Object.prototype.toString.call(tp.x), Object.prototype.toString.call(tp.x.baseVal), Object.prototype.toString.call(tp.rotate.baseVal)];
          return JSON.stringify(out);
        })()
    "#).unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(
        v["rcOrder"],
        serde_json::json!(["x", "y", "width", "height", "rx", "ry", "constructor"]),
        "rect truth raw order"
    );
    assert_eq!(
        v["rcDef"],
        serde_json::json!([0, 1, 0, 1]),
        "rect defaults [0, number] (probed — NOT [100,%])"
    );
    assert_eq!(
        v["rcPct"],
        serde_json::json!("E:NotSupportedError"),
        "rect % never resolves (probed)"
    );
    assert_eq!(
        v["rcRead"],
        serde_json::json!([7, "7", 3.5, 3]),
        "rect unitless + em reads"
    );
    assert_eq!(
        v["rcWrite"],
        serde_json::json!(["42", 42]),
        "rect write-through"
    );
    assert_eq!(
        v["rcIdent"],
        serde_json::json!([true, true]),
        "rect reflector identity"
    );
    assert_eq!(
        v["famDef"],
        serde_json::json!([0, 0, 0, 0, 0, 0, 0, 0]),
        "circle/ellipse/line/foreignObject defaults (probed family)"
    );
    assert_eq!(
        v["lgOrder"],
        serde_json::json!(["x1", "y1", "x2", "y2", "constructor"]),
        "linearGradient truth raw order"
    );
    assert_eq!(
        v["lgDef"],
        serde_json::json!([2, 0, 100, 0, 0]),
        "linear x2 alone defaults 100% (probed)"
    );
    assert_eq!(
        v["lgPct"],
        serde_json::json!("E:NotSupportedError"),
        "gradient % .value unresolvable (probed)"
    );
    assert_eq!(
        v["lgNum"],
        serde_json::json!(7),
        "gradient unitless reads through (probed)"
    );
    assert_eq!(
        v["rgOrder"],
        serde_json::json!(["cx", "cy", "r", "fx", "fy", "fr", "constructor"]),
        "radialGradient truth raw order"
    );
    assert_eq!(
        v["rgDef"],
        serde_json::json!([2, 50, 50, 0]),
        "radial cx/cy/r/fx/fy 50%, fr 0% (probed)"
    );
    assert_eq!(
        v["rgFx"],
        serde_json::json!(50),
        "fx independent of cx (probed)"
    );
    assert_eq!(
        v["stOrder"],
        serde_json::json!(["offset", "constructor"]),
        "stop truth raw order"
    );
    assert_eq!(
        v["stOff"],
        serde_json::json!([0, 0.5, 0.5, 0, -0.5, 1.5]),
        "stop offset: default/number/%/junk/unclamped (all probed)"
    );
    assert_eq!(
        v["stWrite"],
        serde_json::json!("0.75"),
        "stop write serializes (probed)"
    );
    assert_eq!(
        v["syOrder"],
        serde_json::json!(["viewBox", "preserveAspectRatio", "constructor"]),
        "symbol truth raw order"
    );
    assert_eq!(
        v["syVB"],
        serde_json::json!([0, 0, "[object SVGAnimatedRect]"]),
        "symbol viewBox absent -> zeros + brand"
    );
    assert_eq!(
        v["syVB2"],
        serde_json::json!([0, 10, 20, true]),
        "symbol viewBox parse '0 0 10 20' (probed) + identity"
    );
    assert_eq!(v["syPAR"], serde_json::json!(6), "symbol PAR default");
    assert_eq!(
        v["cpOrder"],
        serde_json::json!(["clipPathUnits", "transform", "constructor"]),
        "clipPath truth raw order"
    );
    assert_eq!(v["cpU"], serde_json::json!([1,2,1]), "clipPathUnits default 1 + obb + junk->default (probed; pattern's default is 2, this one is NOT)");
    assert_eq!(
        v["cpT"],
        serde_json::json!(["[object SVGAnimatedTransformList]", 1, 4]),
        "clipPath transform list view (probed)"
    );
    assert_eq!(
        v["tpOrder"],
        serde_json::json!(["x", "y", "dx", "dy", "rotate", "constructor"]),
        "textPositioning truth raw order"
    );
    assert_eq!(
        v["tpDef"],
        serde_json::json!([0, 0, true]),
        "text lists empty by default, base/anim distinct"
    );
    assert_eq!(
        v["tpOOR"],
        serde_json::json!("E:IndexSizeError"),
        "empty getItem throws (probed)"
    );
    assert_eq!(
        v["tpX"],
        serde_json::json!([3, 3]),
        "x '1 2 3' -> 3 items (probed)"
    );
    assert_eq!(
        v["tpY"],
        serde_json::json!([10, 5, 5, "[object SVGLength]"]),
        "y '10px 5' -> SVGLength items with units (probed)"
    );
    assert_eq!(
        v["tpRot"],
        serde_json::json!(["[object SVGNumber]", 90, 60]),
        "rotate items are SVGNumber (probed)"
    );
    assert_eq!(
        v["tpRotW"],
        serde_json::json!("45 60"),
        "rotate item write serializes the list (probed '90 60' shape)"
    );
    assert_eq!(
        v["tpClear"],
        serde_json::json!(0),
        "attr cleared -> list empties (live re-parse)"
    );
    assert_eq!(
        v["imOrder"],
        serde_json::json!([
            "x",
            "y",
            "width",
            "height",
            "preserveAspectRatio",
            "decoding",
            "crossOrigin",
            "href",
            "decode",
            "constructor"
        ]),
        "image truth raw order"
    );
    assert_eq!(
        v["imDef"],
        serde_json::json!(["auto", null, 0, 6]),
        "image decoding 'auto', crossOrigin null, geometry 0, PAR 6 (probed)"
    );
    assert_eq!(
        v["imDec"],
        serde_json::json!(["async", "auto", ["sync", "sync"]]),
        "decoding case-insensitive + junk + write-through (probed)"
    );
    assert_eq!(
        v["imCo"],
        serde_json::json!(["use-credentials", null, ["anonymous", "anonymous"], null]),
        "crossOrigin lowercase mapped attr, ''->null (probed)"
    );
    assert_eq!(
        v["imHref"],
        serde_json::json!(["[object SVGAnimatedString]", "a.png", "a.png"]),
        "image href view + animVal mirror (probed)"
    );
    assert_eq!(
        v["imDecode"],
        serde_json::json!(true),
        "decode() returns a Promise (probed)"
    );
    assert_eq!(
        v["usOrder"],
        serde_json::json!(["x", "y", "width", "height", "href", "constructor"]),
        "use truth raw order"
    );
    assert_eq!(
        v["usDef"],
        serde_json::json!([0, 0]),
        "use geometry defaults"
    );
    assert_eq!(
        v["usHref"],
        serde_json::json!(["[object SVGAnimatedString]", "#t"]),
        "use href view serializes"
    );
    assert_eq!(
        v["brands"],
        serde_json::json!([
            "[object SVGAnimatedLengthList]",
            "[object SVGLengthList]",
            "[object SVGNumberList]"
        ]),
        "list type brands"
    );
}

#[test]
fn knife65_document_family() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt.evaluate(r#"
        (function () {
          const D = Document.prototype;
          const names = Object.getOwnPropertyNames(D);
          const t = (f) => { try { const v = f(); return v === undefined ? 'undef' : (v === null ? 'null' : v); } catch (e) { return 'E:' + e.name + ':' + e.message; } };
          const out = {};
          out.has24 = ['xmlEncoding','xmlVersion','xmlStandalone','onreadystatechange','onpointerlockchange','onpointerlockerror','featurePolicy','webkitVisibilityState','webkitHidden','onfreeze','onprerenderingchange','onresume','onvisibilitychange','webkitIsFullScreen','webkitCurrentFullScreenElement','webkitFullscreenEnabled','webkitFullscreenElement','caretRangeFromPoint','requestStorageAccessFor','webkitCancelFullScreen','webkitExitFullscreen','browsingTopics','activeViewTransition','ariaNotify'].filter((n) => names.indexOf(n) < 0);
          // xml trio (probed: null/null/false; '1.0' sticks; others NotSupportedError; standalone coerces)
          out.xml = [document.xmlEncoding, document.xmlVersion, document.xmlStandalone];
          document.xmlVersion = '1.0';
          out.xml10 = [document.xmlVersion, document.xmlEncoding, document.xmlStandalone];
          out.xmlBad = t(() => { document.xmlVersion = '1.1'; });
          out.xmlNull = t(() => { document.xmlVersion = null; });
          document.xmlVersion = '1.0'; document.xmlStandalone = 'nope';
          out.xmlBool = document.xmlStandalone;
          // on* seven: null default, round trip, names
          const f = () => {};
          document.onvisibilitychange = f;
          out.onRT = [document.onvisibilitychange === f];
          document.onvisibilitychange = null;
          out.onRT.push(document.onvisibilitychange === null, document.onfreeze, document.onreadystatechange);
          const od = Object.getOwnPropertyDescriptor(D, 'onvisibilitychange');
          out.onDsc = [od.enumerable, od.configurable, !!od.get, !!od.set, od.get.name, od.set.length];
          // webkit mirrors (probed values)
          out.webkit = [document.webkitVisibilityState, document.webkitHidden, document.webkitIsFullScreen,
                        document.webkitCurrentFullScreenElement, document.webkitFullscreenEnabled, document.webkitFullscreenElement];
          // featurePolicy: cached singleton + working stub takeover
          const fp = document.featurePolicy;
          out.fp = [typeof fp, fp === document.featurePolicy, fp.constructor.name, Object.prototype.toString.call(fp), Object.getOwnPropertyNames(fp).length];
          out.fpM = [fp.allowedFeatures().length, fp.features().length, fp.allowedFeatures() !== fp.allowedFeatures(),
                     fp.allowsFeature('geolocation'), fp.allowsFeature('zzz'), fp.allowsFeature(''), JSON.stringify(fp.getAllowlistForFeature('camera')), JSON.stringify(fp.getAllowlistForFeature('zzz'))];
          out.fpErr = [t(() => fp.allowsFeature()), t(() => FeaturePolicy.prototype.allowedFeatures.call({}))];
          // activeViewTransition: null at rest, === during
          out.avtRest = document.activeViewTransition;
          const vt = document.startViewTransition(() => {});
          out.avtDuring = document.activeViewTransition === vt;
          // caretRangeFromPoint (probed: element anchor, collapsed, offset 0; OOR null; arity 0)
          const r = document.caretRangeFromPoint(5, 5);
          out.crfp = [r.constructor.name, Object.prototype.toString.call(r), r.collapsed, r.startContainer.nodeType, r.startOffset, document.caretRangeFromPoint(-9999, -9999), D.caretRangeFromPoint.length];
          // requestStorageAccessFor (probed messages)
          // (刀95) the zero-arg case now REJECTS with Chrome's argc TypeError (see
          // knife95) — a sync catch can no longer see it; keep the sync-visible rows.
          out.rsaf = [t(() => document.requestStorageAccessFor(123)), document.requestStorageAccessFor('https://example.com').constructor.name, D.requestStorageAccessFor.length];
          // webkit verbs: undefined return, meta
          out.wv = [document.webkitCancelFullScreen(), document.webkitExitFullscreen(), D.webkitCancelFullScreen.name, D.webkitCancelFullScreen.length, D.webkitExitFullscreen.name, D.webkitExitFullscreen.length];
          // browsingTopics / ariaNotify (probed: promise [] / void + arity messages)
          out.bt = [document.browsingTopics().constructor.name, D.browsingTopics.length];
          out.an = [t(() => document.ariaNotify('hi', { role: 'polite' })), t(() => document.ariaNotify()), t(() => document.ariaNotify('a', 'b')), D.ariaNotify.length, D.ariaNotify.name];
          // body rw (probed exact messages; swaps read through; frameset counts as body)
          const b0 = document.body;
          out.bodyDsc = (d => [d.enumerable, d.configurable, !!d.get, !!d.set, d.get.name, d.set.name, d.set.length])(Object.getOwnPropertyDescriptor(D, 'body'));
          out.bodyErr = [t(() => { document.body = document.createElement('div'); }), t(() => { document.body = null; }), t(() => { document.body = 'x'; })];
          document.body = b0;
          out.bodySelf = document.body === b0;
          const nb = document.createElement('body');
          document.body = nb;
          out.bodyNew = [document.body === nb, document.body.tagName];
          document.body = b0;
          const fs = document.createElement('frameset');
          document.body = fs;
          out.bodyFs = [document.body === fs, document.body.tagName];
          document.body = b0;
          out.bodyBack = document.body === b0;
          // adoptedStyleSheets ec fix + round trip
          const ad = Object.getOwnPropertyDescriptor(D, 'adoptedStyleSheets');
          document.adoptedStyleSheets = [];
          out.adopted = [ad.enumerable, ad.configurable, !!ad.get, !!ad.set, JSON.stringify(document.adoptedStyleSheets)];
          return JSON.stringify(out);
        })()
    "#).unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(v["has24"], serde_json::json!([]), "all 24 members present");
    assert_eq!(
        v["xml"],
        serde_json::json!([null, null, false]),
        "xml trio defaults (probed)"
    );
    assert_eq!(
        v["xml10"],
        serde_json::json!(["1.0", null, false]),
        "xmlVersion '1.0' sticks (probed)"
    );
    assert_eq!(v["xmlBad"], serde_json::json!("E:NotSupportedError:Failed to set the 'xmlVersion' property on 'Document': This document does not support the XML version '1.1'."), "xmlVersion '1.1' exact message (probed)");
    assert_eq!(v["xmlNull"], serde_json::json!("E:NotSupportedError:Failed to set the 'xmlVersion' property on 'Document': This document does not support the XML version ''."), "xmlVersion null -> '' message (probed)");
    assert_eq!(
        v["xmlBool"],
        serde_json::json!(true),
        "xmlStandalone coerces (probed: 'nope' -> true)"
    );
    assert_eq!(
        v["onRT"],
        serde_json::json!([true, true, null, null]),
        "on* round trip + null defaults (probed)"
    );
    assert_eq!(
        v["onDsc"],
        serde_json::json!([true, true, true, true, "get onvisibilitychange", 1]),
        "on* descriptor ec + names (probed)"
    );
    assert_eq!(
        v["webkit"],
        serde_json::json!(["visible", false, false, null, true, null]),
        "webkit mirrors (probed)"
    );
    assert_eq!(
        v["fp"],
        serde_json::json!(["object", true, "FeaturePolicy", "[object FeaturePolicy]", 0]),
        "featurePolicy singleton, brand, no own props (probed)"
    );
    assert_eq!(v["fpM"], serde_json::json!([80, 81, true, true, false, false, "[\"http://example.com\"]", "[]"]), "FeaturePolicy methods Chrome-145 lists (probed 80/81) + allowlist (origin = test page origin)");
    assert_eq!(v["fpErr"], serde_json::json!(["E:TypeError:Failed to execute 'allowsFeature' on 'FeaturePolicy': 1 argument required, but only 0 present.", "E:TypeError:Illegal invocation"]), "FeaturePolicy arity + illegal invocation (probed)");
    assert_eq!(
        v["avtRest"],
        serde_json::json!(null),
        "activeViewTransition null at rest (probed)"
    );
    assert_eq!(
        v["avtDuring"],
        serde_json::json!(true),
        "activeViewTransition === transition object (probed)"
    );
    assert_eq!(
        v["crfp"],
        serde_json::json!(["Range", "[object Range]", true, 1, 0, null, 0]),
        "caretRangeFromPoint element anchor + OOR + arity 0 (probed)"
    );
    assert_eq!(
        v["rsaf"],
        serde_json::json!(["E:TypeError:Invalid origin", "Promise", 1]),
        "requestStorageAccessFor error + promise + arity (zero-arg rejection asserted in knife95)"
    );
    assert_eq!(
        v["wv"],
        serde_json::json!([
            null,
            null,
            "webkitCancelFullScreen",
            0,
            "webkitExitFullscreen",
            0
        ]),
        "webkit verbs undefined + meta (probed)"
    );
    assert_eq!(
        v["bt"],
        serde_json::json!(["Promise", 0]),
        "browsingTopics promise + arity (probed)"
    );
    assert_eq!(v["an"], serde_json::json!(["undef", "E:TypeError:Failed to execute 'ariaNotify' on 'Document': 1 argument required, but only 0 present.", "E:TypeError:Failed to execute 'ariaNotify' on 'Document': The provided value is not of type 'AriaNotificationOptions'.", 1, "ariaNotify"]), "ariaNotify void + messages + arity 1 (probed)");
    assert_eq!(
        v["bodyDsc"],
        serde_json::json!([true, true, true, true, "get body", "set body", 1]),
        "body rw:ec descriptor + names"
    );
    assert_eq!(v["bodyErr"], serde_json::json!(["E:HierarchyRequestError:Failed to set the 'body' property on 'Document': The new body element is of type 'DIV'. It must be either a 'BODY' or 'FRAMESET' element.", "E:HierarchyRequestError:Failed to set the 'body' property on 'Document': The 1st argument provided is either null, or an invalid HTMLElement object.", "E:TypeError:Failed to set the 'body' property on 'Document': Failed to convert value to 'HTMLElement'."]), "body setter exact messages (probed)");
    assert_eq!(
        v["bodySelf"],
        serde_json::json!(true),
        "body self-assignment no-op"
    );
    assert_eq!(
        v["bodyNew"],
        serde_json::json!([true, "BODY"]),
        "body swap to new body element (probed)"
    );
    assert_eq!(
        v["bodyFs"],
        serde_json::json!([true, "FRAMESET"]),
        "body swap to frameset reads through (probed)"
    );
    assert_eq!(v["bodyBack"], serde_json::json!(true), "body restore");
    assert_eq!(
        v["adopted"],
        serde_json::json!([true, true, true, true, "[]"]),
        "adoptedStyleSheets ec descriptor (kind fix)"
    );
}

#[test]
fn knife66_counter_style_and_shadowroot() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt.evaluate(r#"
        (function () {
          const t = (f) => { try { const v = f(); return v === undefined ? 'undef' : (v === null ? 'null' : v); } catch (e) { return 'E:' + e.name + ':' + e.message; } };
          const out = {};
          // ---- CSSCounterStyleRule: 11 rw descriptors ----------------------
          const P = CSSCounterStyleRule.prototype;
          const names = Object.getOwnPropertyNames(P);
          out.has11 = ['name','system','symbols','additiveSymbols','negative','prefix','suffix','range','pad','fallback','speakAs'].filter((n) => names.indexOf(n) < 0);
          const sd = Object.getOwnPropertyDescriptor(P, 'symbols');
          out.dsc = [sd.enumerable, sd.configurable, !!sd.get, !!sd.set, sd.get.name, sd.set.length];
          const st = document.createElement('style');
          st.textContent = '@counter-style thumbs { system: cyclic; symbols: "A" b; suffix: " "; additive-symbols: 3 "a", 2 b; negative: (-)(-) }';
          document.head.appendChild(st);
          const r = st.sheet.cssRules[0];
          out.reads = [r.name, r.system, r.symbols, r.additiveSymbols, r.suffix, r.negative, r.prefix, r.range, r.pad, r.fallback, r.speakAs];
          out.cssText0 = r.cssText;
          // setters: accept valid, silently ignore invalid (Chrome never throws)
          r.symbols = 'x "y"'; r.system = 'numeric'; r.range = '2 5'; r.pad = '3 "0"';
          r.suffix = ''; r.negative = '"-"';
          out.sets = [r.symbols, r.system, r.range, r.pad, r.suffix, r.negative];
          r.name = 'renamed';
          out.name = [r.name, r.cssText.slice(0, 22)];
          r.name = 'none'; r.name = '123bad';
          out.nameBad = r.name;
          out.badSets = [t(() => { r.pad = '"0"'; }), t(() => { r.speakAs = 'numbers spell-out'; }), t(() => { r.suffix = 'initial'; }), t(() => { r.range = 'infinite'; })];
          out.badAfter = [r.pad, r.speakAs, r.suffix, r.range];
          // canonical serialisation order regardless of set order
          const st2 = document.createElement('style');
          st2.textContent = '@counter-style ord { system: cyclic; symbols: "A"; }';
          document.head.appendChild(st2);
          const o = st2.sheet.cssRules[0];
          o.speakAs = 'numbers'; o.fallback = 'decimal'; o.pad = '2 "0"'; o.range = '1 9';
          o.suffix = '"s"'; o.prefix = '"p"'; o.negative = '"-"'; o.additiveSymbols = '3 "x"'; o.name = 'ord2';
          out.canon = o.cssText;
          // foreign receiver -> Illegal invocation (Chrome truth)
          out.illegal = [t(() => P.symbols), t(() => { P.suffix = '"x"'; })];
          // ---- ShadowRoot: five members -----------------------------------
          const SP = ShadowRoot.prototype;
          const sn = Object.getOwnPropertyNames(SP);
          out.has5 = ['fullscreenElement','pictureInPictureElement','pointerLockElement','onslotchange','getSelection'].filter((n) => sn.indexOf(n) < 0);
          const host = document.createElement('div');
          document.body.appendChild(host);
          const sr = host.attachShadow({ mode: 'open' });
          sr.innerHTML = '<p>hi</p>';
          out.srNulls = [sr.fullscreenElement, sr.pictureInPictureElement, sr.pointerLockElement];
          out.srFsSet = t(() => { sr.fullscreenElement = 1; return sr.fullscreenElement; });
          const fn = () => {};
          sr.onslotchange = fn;
          out.srOn = [sr.onslotchange === fn];
          sr.onslotchange = 'nope';
          out.srOn.push(sr.onslotchange);
          const ond = Object.getOwnPropertyDescriptor(SP, 'onslotchange');
          out.srOnDsc = [ond.enumerable, ond.configurable, ond.get.name, ond.set.length];
          const sel = sr.getSelection();
          const host2 = document.createElement('div');
          document.body.appendChild(host2);
          const sr2 = host2.attachShadow({ mode: 'open' });
          out.srSel = [sel.constructor.name, sr.getSelection() === sel, sel === sr2.getSelection(), sel === document.getSelection(), sel.type, sel.rangeCount, sel.anchorNode];
          const rg = document.createRange();
          rg.selectNodeContents(sr.querySelector('p'));
          sel.removeAllRanges(); sel.addRange(rg);
          out.srSelRange = [sel.rangeCount, sel.type, sel.anchorNode === sr.querySelector('p')];
          out.srIllegal = [t(() => SP.fullscreenElement), t(() => SP.getSelection())];
          return JSON.stringify(out);
        })()
    "#).unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    let g = |k: &str| v.get(k).cloned().unwrap_or(serde_json::Value::Null);
    assert_eq!(g("has11"), serde_json::json!([]));
    assert_eq!(
        g("dsc"),
        serde_json::json!([true, true, true, true, "get symbols", 1])
    );
    assert_eq!(
        g("reads"),
        serde_json::json!([
            "thumbs",
            "cyclic",
            "\"A\" b",
            "3 \"a\", 2 b",
            "\" \"",
            "",
            "",
            "",
            "",
            "",
            ""
        ])
    );
    assert_eq!(g("cssText0"), serde_json::json!("@counter-style thumbs { system: cyclic; symbols: \"A\" b; additive-symbols: 3 \"a\", 2 b; suffix: \" \"; }"));
    assert_eq!(
        g("sets"),
        serde_json::json!(["x \"y\"", "cyclic", "2 5", "3 \"0\"", "\" \"", "\"-\""])
    );
    assert_eq!(
        g("name"),
        serde_json::json!(["renamed", "@counter-style renamed"])
    );
    assert_eq!(g("nameBad"), serde_json::json!("renamed"));
    assert_eq!(
        g("badSets"),
        serde_json::json!(["undef", "undef", "undef", "undef"])
    );
    assert_eq!(
        g("badAfter"),
        serde_json::json!(["3 \"0\"", "", "\" \"", "2 5"])
    );
    assert_eq!(g("canon"), serde_json::json!("@counter-style ord2 { system: cyclic; symbols: \"A\"; additive-symbols: 3 \"x\"; negative: \"-\"; prefix: \"p\"; suffix: \"s\"; pad: 2 \"0\"; range: 1 9; fallback: decimal; speak-as: numbers; }"));
    assert_eq!(
        g("illegal"),
        serde_json::json!([
            "E:TypeError:Illegal invocation",
            "E:TypeError:Illegal invocation"
        ])
    );
    assert_eq!(g("has5"), serde_json::json!([]));
    assert_eq!(g("srNulls"), serde_json::json!([null, null, null]));
    assert_eq!(g("srFsSet"), serde_json::json!("null"));
    assert_eq!(g("srOn"), serde_json::json!([true, null]));
    assert_eq!(
        g("srOnDsc"),
        serde_json::json!([true, true, "get onslotchange", 1])
    );
    assert_eq!(
        g("srSel"),
        serde_json::json!(["Selection", true, false, false, "None", 0, null])
    );
    assert_eq!(g("srSelRange"), serde_json::json!([1, "Range", true]));
    assert_eq!(
        g("srIllegal"),
        serde_json::json!([
            "E:TypeError:Illegal invocation",
            "E:TypeError:Illegal invocation"
        ])
    );
}
