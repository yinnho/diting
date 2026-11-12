//! MutationObserver 全族 + takeRecords
use super::common::setup_runtime;

/// #236: `attributeFilter` narrows attribute records — a filter of
/// ['data-allowed'] must not deliver a 'data-other' record. The observer
/// matching loop used to check only `options.attributes`, so the filter
/// was ignored entirely (the merchant's anonymous probe on #203).
#[tokio::test(flavor = "current_thread")]
async fn mutation_observer_attribute_filter_is_honored() {
    let mut rt = setup_runtime(r#"<body><div id="d"></div></body>"#);
    let result = rt
            .call_function_on_remote(
                r#"async () => {
                    const el = document.getElementById('d');
                    const records = [];
                    const mo = new MutationObserver(xs => records.push(...xs.map(x => x.attributeName)));
                    mo.observe(el, {attributes: true, attributeFilter: ['data-allowed']});
                    el.setAttribute('data-other', '1');
                    el.setAttribute('data-allowed', '2');
                    await new Promise(r => setTimeout(r, 50));
                    mo.disconnect();
                    return JSON.stringify(records);
                }"#,
                None,
                &[],
                true,
                true,
            )
            .await
            .unwrap();
    let records = result.value.unwrap();
    assert_eq!(
        records,
        serde_json::json!(r#"["data-allowed"]"#),
        "data-other must be filtered out, data-allowed delivered"
    );
}

/// #39 (obscura#999 residue): WebKitMutationObserver is an alias of the
/// same constructor (zone.js patches both names; Chrome 151 face), and
/// IntersectionObserver's prototype carries scrollMargin/delay/
/// trackVisibility alongside root/rootMargin/thresholds — the sorted
/// 10-name Chrome-parity face, getters enumerable like every other
/// interface member.
#[test]
fn webkit_mutation_observer_alias_and_io_attribute_face() {
    let mut rt = setup_runtime(r#"<body><div id="d"></div></body>"#);
    let out = rt
        .evaluate(
            r#"(() => {
                    const ioKeys = Object.keys(IntersectionObserver.prototype).sort();
                    const io = new IntersectionObserver(function(){});
                    let sawScrollMargin = false;
                    for (const k in io) { if (k === 'scrollMargin') sawScrollMargin = true; }
                    return JSON.stringify([
                        typeof WebKitMutationObserver,
                        WebKitMutationObserver === MutationObserver,
                        ioKeys,
                        io.delay, io.trackVisibility, io.scrollMargin,
                        sawScrollMargin,
                    ]);
                })()"#,
        )
        .unwrap();
    assert_eq!(
        out,
        serde_json::json!(
            r#"["function",true,["delay","disconnect","observe","root","rootMargin","scrollMargin","takeRecords","thresholds","trackVisibility","unobserve"],0,false,"0px 0px 0px 0px",true]"#
        )
    );
}

#[test]
fn test_insert_before_and_replace_child_report_to_mutation_observers() {
    // insertBefore/replaceChild ran the tree mutation but never notified
    // MutationObserver — before()/after()/replaceWith() route through
    // insertBefore, so they were silent too.
    let mut rt = setup_runtime(r#"<main id="host"><p id="a"></p><p id="b"></p></main>"#);
    let result = rt
        .evaluate(
            r#"
            const host = document.getElementById('host');
            const observer = new MutationObserver(() => {});
            observer.observe(host, { childList: true });
            const x = document.createElement('x-i');
            host.insertBefore(x, document.getElementById('b'));
            const y = document.createElement('x-r');
            host.replaceChild(y, document.getElementById('a'));
            // Observer delivery is a microtask; takeRecords() drains
            // synchronously.
            return observer.takeRecords().map(r => [r.addedNodes.length, r.removedNodes.length]);
        "#,
        )
        .unwrap();
    assert_eq!(result, serde_json::json!([[1, 0], [1, 1]]));
}

#[test]
fn document_write_reports_to_mutation_observers() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate(
            r#"
                var scriptTestSetup = true;
                globalThis.__seen = [];
                const observer = new MutationObserver((records) => {
                  for (const record of records) {
                    for (const node of record.addedNodes) globalThis.__seen.push(node.nodeName);
                  }
                });
                observer.observe(document.body, { childList: true });
                document.write('<span id="watched">z</span>');
                observer.takeRecords().forEach((record) => {
                  for (const node of record.addedNodes) globalThis.__seen.push(node.nodeName);
                });
                return globalThis.__seen.join(',');
                "#,
        )
        .unwrap();
    assert_eq!(result, serde_json::json!("SPAN"));
}

/// #272: MutationRecord alignment with Chrome (obscura #1266's checklist
/// turned into pinned probes; every expected value below was captured from
/// Chrome 155 via CDP, not derived from the spec). This one covers the
/// childList sibling slots: previousSibling/nextSibling around added and
/// removed nodes, replaceChild's single combined record, and the callback
/// receiver (`this` and the second argument are both the observer).
#[tokio::test(flavor = "current_thread")]
async fn mutation_records_child_list_sibling_slots() {
    let mut rt = setup_runtime(r#"<body></body>"#);
    let script = r#"async () => {
        const shape = rs => rs.map(r => ({
            added: [...r.addedNodes].map(n => n.id || n.nodeName),
            removed: [...r.removedNodes].map(n => n.id || n.nodeName),
            prev: r.previousSibling ? (r.previousSibling.id || r.previousSibling.nodeName) : null,
            next: r.nextSibling ? (r.nextSibling.id || r.nextSibling.nodeName) : null,
            attr: r.attributeName, old: r.oldValue, type: r.type }));
        const out = {};
        // append at the end: prev = old lastChild, next = null
        {
            const div = document.createElement('div');
            div.innerHTML = '<i id=a></i><i id=b></i>';
            let nth = null, thisOk = null, obsOk = null;
            const mo = new MutationObserver(function(rs, obs) {
                thisOk = this === mo; obsOk = obs === mo; nth = shape(rs);
            });
            mo.observe(div, {childList: true});
            const c = document.createElement('i'); c.id = 'c';
            div.appendChild(c);
            await new Promise(r => setTimeout(r, 20));
            mo.disconnect();
            out.append = { nth, thisOk, obsOk };
        }
        // insertBefore in the middle: neighbours from the post-insert state
        {
            const div = document.createElement('div');
            div.innerHTML = '<i id=a></i><i id=b></i><i id=d></i>';
            let nth = null;
            const mo = new MutationObserver(rs => { nth = shape(rs); });
            mo.observe(div, {childList: true});
            const c = document.createElement('i'); c.id = 'c';
            div.insertBefore(c, div.children[2]);
            await new Promise(r => setTimeout(r, 20));
            mo.disconnect();
            out.insertMiddle = nth;
        }
        // removeChild in the middle: neighbours captured BEFORE removal
        {
            const div = document.createElement('div');
            div.innerHTML = '<i id=a></i><i id=b></i><i id=c></i>';
            let nth = null;
            const mo = new MutationObserver(rs => { nth = shape(rs); });
            mo.observe(div, {childList: true});
            div.removeChild(div.children[1]);
            await new Promise(r => setTimeout(r, 20));
            mo.disconnect();
            out.removeMiddle = nth;
        }
        // replaceChild: ONE combined record (Chrome), not remove+insert pair
        {
            const div = document.createElement('div');
            div.innerHTML = '<i id=a></i><i id=b></i><i id=c></i>';
            let nth = null;
            const mo = new MutationObserver(rs => { nth = shape(rs); });
            mo.observe(div, {childList: true});
            const n = document.createElement('i'); n.id = 'N';
            div.replaceChild(n, div.children[1]);
            await new Promise(r => setTimeout(r, 20));
            mo.disconnect();
            out.replaceChild = nth;
        }
        return out;
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!({
            "append": {
                "nth": [{"added": ["c"], "removed": [], "prev": "b", "next": null,
                          "attr": null, "old": null, "type": "childList"}],
                "thisOk": true, "obsOk": true
            },
            "insertMiddle": [{"added": ["c"], "removed": [], "prev": "b", "next": "d",
                               "attr": null, "old": null, "type": "childList"}],
            "removeMiddle": [{"added": [], "removed": ["b"], "prev": "a", "next": "c",
                               "attr": null, "old": null, "type": "childList"}],
            "replaceChild": [{"added": ["N"], "removed": ["b"], "prev": "a", "next": "c",
                               "attr": null, "old": null, "type": "childList"}]
        })
    );
}

/// #272: attribute records — oldValue only for observers that asked
/// (attributeOldValue), attributeFilter narrowing, removeAttribute carrying
/// the removed value as oldValue, and no record for removing an absent
/// attribute. Expected sequence captured from Chrome 155.
#[tokio::test(flavor = "current_thread")]
async fn mutation_records_attribute_old_value_gating() {
    let mut rt = setup_runtime(r#"<body></body>"#);
    let script = r#"async () => {
        const el = document.createElement('x');
        el.setAttribute('k', 'v0');
        const seen = [];
        const mk = tag => new MutationObserver(rs =>
            seen.push([tag, rs.map(r => ({attr: r.attributeName, old: r.oldValue, type: r.type}))]));
        const m1 = mk('noOld'); m1.observe(el, {attributes: true});
        const m2 = mk('withOld'); m2.observe(el, {attributes: true, attributeOldValue: true});
        el.setAttribute('k', 'v1');
        el.setAttribute('k', 'v2');
        const m3 = mk('filter'); m3.observe(el, {attributes: true, attributeFilter: ['z']});
        el.setAttribute('k', 'v3');
        el.setAttribute('z', 'zz');
        el.removeAttribute('k');
        m1.observe(el, {attributes: true}); // absent: no record
        el.removeAttribute('never-existed');
        await new Promise(r => setTimeout(r, 20));
        m1.disconnect(); m2.disconnect(); m3.disconnect();
        return seen;
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([
            ["noOld", [
                {"attr": "k", "old": null, "type": "attributes"},
                {"attr": "k", "old": null, "type": "attributes"},
                {"attr": "k", "old": null, "type": "attributes"},
                {"attr": "z", "old": null, "type": "attributes"},
                {"attr": "k", "old": null, "type": "attributes"}
            ]],
            ["withOld", [
                {"attr": "k", "old": "v0", "type": "attributes"},
                {"attr": "k", "old": "v1", "type": "attributes"},
                {"attr": "k", "old": "v2", "type": "attributes"},
                {"attr": "z", "old": null, "type": "attributes"},
                {"attr": "k", "old": "v3", "type": "attributes"}
            ]],
            ["filter", [
                {"attr": "z", "old": null, "type": "attributes"}
            ]]
        ]),
        "attribute sequence must match Chrome 155's probe exactly"
    );
}

/// #272: characterData records — one record per data write (Chrome never
/// coalesces), oldValue only when characterDataOldValue was requested, and
/// the record's target is the text node itself.
#[tokio::test(flavor = "current_thread")]
async fn mutation_records_character_data_old_value_gating() {
    let mut rt = setup_runtime(r#"<body></body>"#);
    let script = r#"async () => {
        const el = document.createElement('p'); el.textContent = 'a';
        const t = el.firstChild;
        const seen = [];
        const mk = tag => new MutationObserver(rs =>
            seen.push([tag, rs.map(r => ({type: r.type, old: r.oldValue, tgtIsText: r.target === t}))]));
        const m1 = mk('noOld'); m1.observe(t, {characterData: true});
        const m2 = mk('withOld'); m2.observe(t, {characterData: true, characterDataOldValue: true});
        t.data = 'b'; t.data = 'c';
        await new Promise(r => setTimeout(r, 20));
        m1.disconnect(); m2.disconnect();
        return seen;
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([
            ["noOld", [
                {"type": "characterData", "old": null, "tgtIsText": true},
                {"type": "characterData", "old": null, "tgtIsText": true}
            ]],
            ["withOld", [
                {"type": "characterData", "old": "a", "tgtIsText": true},
                {"type": "characterData", "old": "b", "tgtIsText": true}
            ]]
        ])
    );
}

/// #272: delivery mechanics — one callback per microtask checkpoint for a
/// synchronous burst, both observers sharing the same record object when
/// their options agree (Chrome hands them the identical instance),
/// takeRecords draining the queue, disconnect dropping what was still
/// queued (takeRecords returns [] after; the callback never fires), and
/// re-observe replacing the options instead of stacking duplicates.
#[tokio::test(flavor = "current_thread")]
async fn mutation_observer_delivery_batching_identity_disconnect() {
    let mut rt = setup_runtime(r#"<body></body>"#);
    let script = r#"async () => {
        const out = {};
        {
            const div = document.createElement('div');
            let calls = 0, total = 0;
            const mo = new MutationObserver(rs => { calls++; total += rs.length; });
            mo.observe(div, {childList: true});
            div.appendChild(document.createElement('i'));
            div.appendChild(document.createElement('i'));
            div.appendChild(document.createElement('i'));
            await new Promise(r => setTimeout(r, 20));
            mo.disconnect();
            out.batching = { calls, total };
        }
        {
            const div = document.createElement('div');
            let r1 = null, r2 = null;
            const mo1 = new MutationObserver(rs => { r1 = rs; });
            const mo2 = new MutationObserver(rs => { r2 = rs; });
            mo1.observe(div, {childList: true}); mo2.observe(div, {childList: true});
            div.appendChild(document.createElement('i'));
            await new Promise(r => setTimeout(r, 20));
            mo1.disconnect(); mo2.disconnect();
            out.identity = { same: r1[0] === r2[0], len1: r1.length, len2: r2.length };
        }
        {
            const div = document.createElement('div');
            let fired = false;
            const mo = new MutationObserver(() => { fired = true; });
            mo.observe(div, {childList: true});
            div.appendChild(document.createElement('i'));
            const q1 = mo.takeRecords().length;
            div.appendChild(document.createElement('i'));
            mo.disconnect();
            const q2 = mo.takeRecords().length;
            await new Promise(r => setTimeout(r, 20));
            out.takeDisconnect = { q1, q2, firedAfterDisconnect: fired };
        }
        {
            const div = document.createElement('div');
            let n = 0;
            const mo = new MutationObserver(rs => { n += rs.length; });
            mo.observe(div, {attributes: true});
            mo.observe(div, {childList: true}); // replaces, does not stack
            div.setAttribute('k', 'v');          // no longer observed
            div.appendChild(document.createElement('i'));
            await new Promise(r => setTimeout(r, 20));
            mo.disconnect();
            out.reobserve = n;
        }
        return out;
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!({
            "batching": {"calls": 1, "total": 3},
            "identity": {"same": true, "len1": 1, "len2": 1},
            "takeDisconnect": {"q1": 1, "q2": 0, "firedAfterDisconnect": false},
            "reobserve": 1
        })
    );
}

/// #272: replace-all and multi-arg mutators all deliver ONE record —
/// innerHTML/textContent swaps (added+removed in one entry, no siblings),
/// Element.append/prepend batching their per-node inserts (Chrome: a single
/// record with both nodes), and replaceChildren as a whole-list swap.
#[tokio::test(flavor = "current_thread")]
async fn mutation_records_replace_all_and_multi_arg_shapes() {
    let mut rt = setup_runtime(r#"<body></body>"#);
    let script = r#"async () => {
        const out = {};
        const probe = async (build, mutate) => {
            const div = build();
            let nth = null;
            const mo = new MutationObserver(rs => { nth = rs.map(r => ({
                added: [...r.addedNodes].map(n => n.id || n.nodeName),
                removed: [...r.removedNodes].map(n => n.id || n.nodeName),
                prev: r.previousSibling ? r.previousSibling.nodeName : null,
                next: r.nextSibling ? r.nextSibling.id : null })); });
            mo.observe(div, {childList: true});
            mutate(div);
            await new Promise(r => setTimeout(r, 20));
            mo.disconnect();
            return nth;
        };
        out.innerHTML = await probe(
            () => { const d = document.createElement('div'); d.innerHTML = '<i id=a></i>'; return d; },
            d => { d.innerHTML = '<b id=b></b>'; });
        out.textContent = await probe(
            () => { const d = document.createElement('div'); d.innerHTML = '<i id=a></i><b id=b></b>'; return d; },
            d => { d.textContent = 't'; });
        out.append = await probe(
            () => document.createElement('div'),
            d => { d.append(document.createElement('i'), document.createElement('b')); });
        out.prepend = await probe(
            () => { const d = document.createElement('div'); d.innerHTML = '<i id=a></i>'; return d; },
            d => { d.prepend(document.createElement('i'), document.createElement('b')); });
        out.replaceChildren = await probe(
            () => { const d = document.createElement('div'); d.innerHTML = '<i id=a></i><b id=b></b>'; return d; },
            d => { d.replaceChildren(document.createElement('i'), document.createTextNode('t')); });
        return out;
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!({
            "innerHTML": [{"added": ["b"], "removed": ["a"], "prev": null, "next": null}],
            "textContent": [{"added": ["#text"], "removed": ["a", "b"], "prev": null, "next": null}],
            "append": [{"added": ["I", "B"], "removed": [], "prev": null, "next": null}],
            "prepend": [{"added": ["I", "B"], "removed": [], "prev": null, "next": "a"}],
            "replaceChildren": [{"added": ["I", "#text"], "removed": ["a", "b"], "prev": null, "next": null}]
        })
    );
}

/// #272: subtree delivery — records for descendant mutations name the actual
/// mutated node as target (the span for a childList insert into it, the text
/// node for a characterData write), not the observed root.
#[tokio::test(flavor = "current_thread")]
async fn mutation_records_subtree_targets_actual_node() {
    let mut rt = setup_runtime(r#"<body></body>"#);
    let script = r#"async () => {
        const div = document.createElement('div');
        div.innerHTML = '<p><span id=s>x</span></p>';
        const span = div.querySelector('#s');
        let nth = null;
        const mo = new MutationObserver(rs => { nth = rs.map(r => ({
            tgtIsSpan: r.target === span, tgtIsDiv: r.target === div, tgtIsText: r.target === span.firstChild })); });
        mo.observe(div, {childList: true, subtree: true, characterData: true});
        span.appendChild(document.createElement('i'));
        span.firstChild.data = 'y';
        await new Promise(r => setTimeout(r, 20));
        mo.disconnect();
        return nth;
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([
            {"tgtIsSpan": true, "tgtIsDiv": false, "tgtIsText": false},
            {"tgtIsSpan": false, "tgtIsDiv": false, "tgtIsText": true}
        ])
    );
}

/// #272: observer interface shapes — observe() validates (TypeError for a
/// non-Node target, TypeError when none of the three mutation kinds is
/// true; attributeOldValue alone is NOT sufficient per Chrome), and the
/// document-child constraint throws HierarchyRequestError for a second
/// element child on Document. Messages pinned against Chrome 155.
#[tokio::test(flavor = "current_thread")]
async fn mutation_observer_validation_and_document_child_constraint() {
    let mut rt = setup_runtime(r#"<body></body>"#);
    let script = r#"async () => {
        const el = document.createElement('x');
        const attempt = f => { try { f(); return 'no-throw'; } catch (e) { return e.name + ': ' + e.message; } };
        return {
            nullTarget: attempt(() => new MutationObserver(() => {}).observe(null, {childList: true})),
            empty: attempt(() => new MutationObserver(() => {}).observe(el, {})),
            noOpts: attempt(() => new MutationObserver(() => {}).observe(el)),
            onlyOld: attempt(() => new MutationObserver(() => {}).observe(el, {attributeOldValue: true})),
            docSecondElement: attempt(() => document.appendChild(document.createElement('x2'))),
        };
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    let obj = v.as_object().expect("probe object");
    assert_eq!(
        obj["nullTarget"],
        serde_json::json!(
            "TypeError: Failed to execute 'observe' on 'MutationObserver': parameter 1 is not of type 'Node'."
        )
    );
    assert_eq!(
        obj["empty"],
        serde_json::json!(
            "TypeError: Failed to execute 'observe' on 'MutationObserver': The options object must set at least one of 'attributes', 'characterData', or 'childList' to true."
        )
    );
    assert_eq!(
        obj["noOpts"], obj["empty"],
        "observe() with no options hits the same dictionary error"
    );
    assert_eq!(
        obj["onlyOld"],
        serde_json::json!("no-throw"),
        "attributeOldValue alone implies attributes and must not throw (Chrome truth)"
    );
    assert_eq!(
        obj["docSecondElement"],
        serde_json::json!(
            "HierarchyRequestError: Failed to execute 'appendChild' on 'Node': Only one element on document allowed."
        )
    );

    // The implied family really delivers: {attributeOldValue:true} alone
    // sees attribute records with old values attached (Chrome: [null,"v1"]).
    let script = r#"async () => {
        const el = document.createElement('x');
        let n = -1, olds = null;
        const mo = new MutationObserver(rs => { n = rs.length; olds = rs.map(r => r.oldValue); });
        mo.observe(el, {attributeOldValue: true});
        el.setAttribute('k', 'v1');
        el.setAttribute('k', 'v2');
        await new Promise(r => setTimeout(r, 20));
        mo.disconnect();
        return { n: String(n), olds: JSON.stringify(olds) };
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!({"n": "2", "olds": "[null,\"v1\"]"}),
        "attributeOldValue alone implies attributes=true and delivers records"
    );
}

/// #203: the observer families Chrome gives takeRecords carry it — the 抖店
/// SDK calls it on its PerformanceObserver instance at startup. 刀57 tightened
/// the family to Chrome truth: ResizeObserver is NOT one of them (its face is
/// observe/unobserve/disconnect only).
#[test]
fn observer_take_records_on_all_families() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt.evaluate(r#"
        (function () {
          const out = {};
          for (const n of ['MutationObserver', 'IntersectionObserver', 'ResizeObserver', 'PerformanceObserver']) {
            const C = globalThis[n];
            out[n] = { proto: typeof C.prototype.takeRecords };
          }
          const po = new PerformanceObserver(function () {});
          out.perfCall = Array.isArray(po.takeRecords());
          return JSON.stringify(out);
        })()
    "#).unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    for n in [
        "MutationObserver",
        "IntersectionObserver",
        "PerformanceObserver",
    ] {
        assert_eq!(
            v[n]["proto"],
            serde_json::json!("function"),
            "{n}.prototype.takeRecords missing"
        );
    }
    assert_eq!(
        v["ResizeObserver"]["proto"],
        serde_json::json!("undefined"),
        "ResizeObserver.takeRecords absent (Chrome truth, 刀57)"
    );
    assert_eq!(v["perfCall"], serde_json::json!(true));
}

#[tokio::test(flavor = "current_thread")]
async fn observers_redeliver_only_on_change() {
    // IntersectionObserver's DOM-mutation re-fire used to re-deliver every
    // target of every live observer on each mutation batch, and
    // ResizeObserver re-delivered the same size at 200/800ms — on x.com's
    // virtualized timeline each delivery caused more DOM, which re-fired them
    // all (the realm sat in the #66 busy freeze). A mutation that doesn't
    // move the target must not re-deliver it; a size that didn't change must
    // not re-notify (Chrome: first observation, then on change only).
    let mut rt = setup_runtime(
        "<html><body><div id=t style='width:50px;height:20px'></div><div id=other></div></body></html>",
    );
    let script = r#"async () => {
        const t = document.getElementById('t');
        let io = 0, ro = 0;
        new IntersectionObserver(() => { io++; }).observe(t);
        new ResizeObserver(() => { ro++; }).observe(t);
        await new Promise(r => setTimeout(r, 20));
        const ioFirst = io, roFirst = ro;
        // Mutations elsewhere that leave t where it is.
        for (let i = 0; i < 20; i++) {
            document.getElementById('other').appendChild(document.createElement('span'));
            await new Promise(r => setTimeout(r, 5));
        }
        await new Promise(r => setTimeout(r, 1000));
        return { ioFirst, roFirst, roAfter: ro, ioMutationRedeliveries: io - ioFirst };
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    assert_eq!(v["ioFirst"], serde_json::json!(1));
    assert_eq!(v["roFirst"], serde_json::json!(1));
    assert_eq!(
        v["roAfter"],
        serde_json::json!(1),
        "unchanged size re-notified: {v}"
    );
    // The burst schedule (120/500ms within the window) may still deliver;
    // the 20 mutation batches must add nothing on top of it.
    assert!(
        v["ioMutationRedeliveries"].as_f64().unwrap() <= 2.0,
        "mutation batches re-delivered an unmoved target: {v}"
    );
}
