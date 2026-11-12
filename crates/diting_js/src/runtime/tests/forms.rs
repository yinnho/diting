//! 表单控件：checkbox/radio/select/label/校验/paint mirror
use super::super::*;
use super::common::setup_runtime;

/// Select surface residuals (obscura#991's list): selectedOptions /
/// multiple / size, and the per-tag interface discriminators the table
/// family relies on.
#[test]
fn select_options_surface_and_table_interfaces() {
    let mut rt = setup_runtime(
        r#"<body><select id="s" multiple size="3"><option value="x">x</option><option value="y" selected>y</option></select></body>"#,
    );
    let result = rt
        .evaluate(
            r#"
            const s = document.getElementById('s');
            s.options.add(new Option('z', 'z'));
            return {
                optionsLen: s.options.length,
                selected: Array.from(s.selectedOptions).map((o) => o.value),
                multiple: s.multiple,
                size: s.size,
                optgroup: document.createElement('optgroup') instanceof HTMLOptGroupElement,
                row: document.createElement('tr') instanceof HTMLTableRowElement,
                cell: document.createElement('th') instanceof HTMLTableCellElement,
                optCtor: s.options[2] instanceof HTMLOptionElement,
            };
        "#,
        )
        .unwrap();
    let v = result;
    assert_eq!(
        v["optionsLen"],
        serde_json::json!(3),
        "new Option + options.add"
    );
    assert_eq!(v["selected"], serde_json::json!(["y"]));
    assert_eq!(v["multiple"], serde_json::json!(true));
    assert_eq!(v["size"], serde_json::json!(3));
    assert_eq!(v["optgroup"], serde_json::json!(true));
    assert_eq!(v["row"], serde_json::json!(true));
    assert_eq!(v["cell"], serde_json::json!(true));
    assert_eq!(v["optCtor"], serde_json::json!(true));
}

/// Regression for #105: `HTMLFormElement` must expose `.elements` so
/// frameworks that probe form field collections work.
#[test]
fn html_form_element_exposes_elements_collection() {
    let mut rt =
        setup_runtime(r#"<form id="f"><input name=a><input name=b><textarea></textarea></form>"#);
    let n = rt
        .evaluate("document.getElementById('f').elements.length")
        .unwrap();
    assert_eq!(n.as_f64().unwrap() as i64, 3);
    let is_form = rt
        .evaluate("document.getElementById('f') instanceof HTMLFormElement")
        .unwrap();
    assert_eq!(is_form, serde_json::json!(true));
}

/// alert/confirm/prompt are auto-answered from the dialog policy and
/// recorded as level-"dialog" console entries. Default dismisses; the
/// accepted prompt falls back to the call's default argument when no
/// session prompt_text is set.
#[test]
fn test_dialogs_answer_from_policy_and_log() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt
            .evaluate(
                "[String(alert('hi')), String(confirm('go?')), String(prompt('name?', 'anon'))].join('|')",
            )
            .unwrap();
    assert_eq!(
        out,
        serde_json::json!("undefined|false|null"),
        "dismiss default"
    );
    let stripped: Vec<(String, String)> = rt
        .take_pending_console_calls()
        .into_iter()
        .map(|(level, msg, _url)| (level, msg))
        .collect();
    let parsed: Vec<serde_json::Value> = stripped
        .iter()
        .map(|(l, m)| {
            assert_eq!(l, "dialog");
            serde_json::from_str(m).unwrap()
        })
        .collect();
    assert_eq!(parsed[0]["dialog"], "alert");
    assert_eq!(parsed[0]["message"], "hi");
    assert!(parsed[0].get("answer").is_none(), "alert carries no answer");
    assert_eq!(parsed[1]["dialog"], "confirm");
    assert_eq!(parsed[1]["answer"], false);
    assert_eq!(parsed[2]["dialog"], "prompt");
    assert_eq!(parsed[2]["answer"], false);

    rt.set_dialog_policy(true, None);
    let out = rt
            .evaluate("[String(confirm('go?')), String(prompt('name?', 'anon')), String(prompt('name?'))].join('|')")
            .unwrap();
    assert_eq!(
        out,
        serde_json::json!("true|anon|"),
        "accept: prompt falls back to its default argument"
    );
    // prompt_text wins over the call's default once set.
    rt.set_dialog_policy(true, Some("ada".into()));
    let out = rt.evaluate("prompt('name?', 'anon')").unwrap();
    assert_eq!(out, serde_json::json!("ada"));

    rt.set_dialog_policy(false, None);
    let out = rt.evaluate("String(confirm('go?'))").unwrap();
    assert_eq!(out, serde_json::json!("false"));
}

#[test]
fn test_checkbox_radio_default_value_on() {
    // A checkbox/radio with no value attribute returns "on" in a real
    // browser, not the empty string; an explicit value attribute wins.
    let mut rt = setup_runtime(
        r#"<input id="cb" type="checkbox"><input id="rd" type="radio"><input id="cbv" type="checkbox" value="yes"><input id="txt" type="text">"#,
    );
    let result = rt
        .evaluate(
            r#"
            return [
                document.getElementById('cb').value,
                document.getElementById('rd').value,
                document.getElementById('cbv').value,
                document.getElementById('txt').value,
            ];
        "#,
        )
        .unwrap();
    assert_eq!(result, serde_json::json!(["on", "on", "yes", ""]));
}

#[test]
fn test_submit_button_click_handler_can_prevent_default_and_navigate() {
    let mut rt = setup_runtime(r#"<form><button type="submit" id="submit">Submit</button></form>"#);
    let href = rt
        .evaluate(
            r#"
            const form = document.querySelector('form');
            form.addEventListener('submit', (event) => {
                event.preventDefault();
                location.href = '/submitted';
            });
            document.getElementById('submit').click();
            return location.href;
        "#,
        )
        .unwrap();
    assert_eq!(href, serde_json::json!("http://example.com/submitted"));
    assert_eq!(
        rt.take_pending_navigation(),
        Some((
            "http://example.com/submitted".to_string(),
            "GET".to_string(),
            "".to_string()
        ))
    );
}

#[test]
fn test_click_fieldset_disabled_controls_do_not_activate() {
    // <fieldset disabled> disables its descendant controls — no toggle and
    // no click event at all — except descendants of its FIRST <legend>
    // (HTML spec actually-disabled semantics; obscura#721 edge matrix).
    let mut rt = setup_runtime(
        r#"<form><fieldset disabled>
            <legend><input type=checkbox id=first></legend>
            <legend><input type=checkbox id=second></legend>
            <input type=checkbox id=body>
            </fieldset><input type=checkbox id=outside></form>"#,
    );
    let result = rt
        .evaluate(
            r#"
            const hits = [];
            for (const id of ['first','second','body','outside']) {
                const el = document.getElementById(id);
                el.addEventListener('click', () => hits.push(id));
                el.click();
            }
            return [document.getElementById('first').checked,
                    document.getElementById('second').checked,
                    document.getElementById('body').checked,
                    document.getElementById('outside').checked,
                    hits];
        "#,
        )
        .unwrap();
    // First-legend control activates; second-legend and fieldset-body
    // controls are actually-disabled (no toggle, no event); outside is
    // unaffected.
    assert_eq!(
        result,
        serde_json::json!([true, false, false, true, ["first", "outside"]])
    );
}

#[test]
fn test_checkbox_click_clears_indeterminate_and_cancel_restores() {
    // Checkbox activation clears `indeterminate` before the event fires
    // (a listener sees the cleared state); a cancelled click restores
    // both `checked` and `indeterminate`. `indeterminate` is a real IDL
    // property on the prototype, not an expando — `'indeterminate' in el`
    // is true and fresh elements default to false (obscura#721 edge matrix;
    // upstream deliberately skipped this because stock has no property).
    let mut rt = setup_runtime(
        r#"<input type=checkbox id=plain><input type=checkbox id=cxl><input type=checkbox id=fresh>"#,
    );
    let result = rt.evaluate(r#"
            const plain = document.getElementById('plain'), cxl = document.getElementById('cxl');
            const propReal = ['indeterminate' in plain,
                              Object.getOwnPropertyNames(Object.getPrototypeOf(plain)).includes('indeterminate'),
                              document.getElementById('fresh').indeterminate];
            plain.indeterminate = true;
            let seen = null;
            plain.addEventListener('click', () => { seen = [plain.checked, plain.indeterminate]; });
            plain.click();
            const plainAfter = [plain.checked, plain.indeterminate];
            cxl.indeterminate = true;
            cxl.addEventListener('click', (e) => e.preventDefault());
            cxl.click();
            return [propReal, seen, plainAfter, [cxl.checked, cxl.indeterminate]];
        "#).unwrap();
    assert_eq!(
        result,
        serde_json::json!([
            [true, true, false], // real prototype property; defaults false
            [true, false],       // handler observes the flip AND the cleared indeterminate
            [true, false],       // uncancelled click keeps the cleared state
            [false, true],       // cancelled click restores checked AND indeterminate
        ])
    );
}

#[test]
fn clicking_label_wrapping_range_input_terminates() {
    // blitz#957: upstream's label default action re-dispatches a click
    // to the wrapped control, whose activation bubbles back to the
    // label's default action again — mutual recursion, stack overflow.
    // Our _postClickActivation breaks the cycle structurally:
    // activation runs at the click TARGET only, and a control's own
    // click is self-interactive (never forwards through a wrapping
    // label again), so the label forwards ONCE and the forwarded click
    // is terminal. The test completing IS the probe; the counters pin
    // the forwarding count.
    let mut rt = setup_runtime(
        r#"<html><body style="margin:0">
            <label id="hue" style="display:block; width:180px; height:40px;">
                Hue
                <input id="range" type="range" min="0" max="1000" style="width:120px; height:20px;">
            </label>
        </body></html>"#,
    );
    let result = rt
        .evaluate(
            r#"
            let rangeClicks = 0, labelClicks = 0;
            const hue = document.getElementById('hue');
            const range = document.getElementById('range');
            hue.addEventListener('click', () => labelClicks++);
            range.addEventListener('click', () => rangeClicks++);
            hue.click();
            range.click();
            // Trusted-path arm: the CDP Input domain drives the same
            // activation sequence through this entry point.
            globalThis.__diting_dispatchTrustedClick(
                hue, new MouseEvent('click', {bubbles: true, cancelable: true}), false);
            return [labelClicks, rangeClicks];
        "#,
        )
        .unwrap();
    // Per-click accounting (Chrome): a label click fires the label's
    // own listener, forwards one synthetic click to the range (which
    // bubbles back through the label — listener count, no re-forward),
    // and every click at/below the label bubbles to it. label: 2 (own
    // + forwarded-bubble) + 1 (direct range click bubbling) + 2
    // (trusted label click, same shape) = 5; range: 1 forwarded × 2 + 1
    // direct = 3.
    assert_eq!(result, serde_json::json!([5, 3,]));
}

#[test]
fn test_element_labels_and_label_control_getters() {
    // obscura#835 lineage: label↔control association must be readable in
    // both directions — Playwright's getByLabel() leans on `label.control`
    // and `el.labels` in the page. Per spec, a label's control is the
    // for-referenced element (if `for` is present, empty for = nothing)
    // else the first labelable descendant; `labels` is the reverse map.
    let mut rt = setup_runtime(
        r#"<div id=d></div>
            <input type=hidden id=h>
            <label for=a id=la>Name</label><input id=a>
            <label id=lb><input id=b><input id=b2></label>
            <label for=a id=lc><input id=c></label>"#,
    );
    let result = rt
        .evaluate(
            r#"
            const $ = (id) => document.getElementById(id);
            return [
                // non-labelable: labels is undefined (Chrome keeps it on the
                // labelable interfaces only — 刀55 took the Element ghost off);
                // hidden input keeps the property
                typeof $('d').labels, $('h').labels.length,
                // for-linked: BOTH for=a labels associate (la and lc), tree order
                $('a').labels.length, $('a').labels[0] === $('la'), $('la').control === $('a'),
                // wrapping: only the FIRST labelable descendant associates
                $('b').labels.length, $('b').labels[0] === $('lb'), $('b2').labels.length,
                // a wrapping label whose `for` points elsewhere: not associated
                // with the wrapped input, and still controls its for-target
                $('c').labels.length, $('lc').control === $('a'),
                // label with no for and no labelable descendant: control null
                $('d').closest ? $('lc').control !== $('c') : true,
                // NodeList shape
                $('a').labels instanceof NodeList,
            ];
        "#,
        )
        .unwrap();
    assert_eq!(
        result,
        serde_json::json!([
            "undefined",
            0,
            2,
            true,
            true,
            1,
            true,
            0,
            0,
            true,
            true,
            true,
        ])
    );
}

#[test]
fn test_form_named_control_access() {
    // `form.fieldName` — HTMLFormElement's own named access to its listed
    // controls. Surfaced by the hosted live-view page: an inline onclick
    // reading document.forms[0].q.value threw, because the forms
    // *collection* shipped without the form element's named access. Real
    // props must still win, so a control named "submit" cannot shadow
    // form.submit (same precedence as Chrome).
    let mut rt = setup_runtime(
        r#"<form id=f>
            <input name=q value=hello><input id=alt name=second value=world><button name=go>Go</button>
            </form>
            <input name=q value=outside>"#,
    );
    let result = rt.evaluate(r#"
            const f = document.forms[0];
            return [
                f.q.value,                                  // by name
                f.alt.value,                                // by id
                f.second === f.alt,                         // both names hit one control
                f.go.tagName,                               // buttons are listed elements too
                ('q' in f), ('nope' in f),
                f.q === document.forms.namedItem('f').q,    // stable identity through the cache
                typeof f.submit === 'function',             // real methods win over named props
                f.q === document.querySelector('input[name=q]'), // the wrapper itself
                document.createElement('form').constructor === HTMLFormElement, // createElement path too
            ];
        "#).unwrap();
    assert_eq!(
        result,
        serde_json::json!(["hello", "world", true, "BUTTON", true, false, true, true, true, true,])
    );
}

#[test]
fn test_form_value_paint_mirror() {
    // el.value = x is DIRTY state: the prototype setter mirrors it into
    // NodeData::Element::live_value for the paint walk, and it must never
    // leak into any serialized DOM surface (Chrome's dirty value lives in
    // the same nowhere-land). form.reset() then restores the PARSED
    // defaults (value attr / child text / selected attr / checked attr)
    // after a cancelable 'reset' event — the old reset wiped everything
    // to '' without firing anything.
    let mut rt = setup_runtime(
        r#"<form id=f>
            <input id=t value=stale><textarea id=ta>ta0</textarea>
            <input id=c type=checkbox checked>
            <select id=s><option value=a>A</option><option value=b selected>B</option></select>
            </form>"#,
    );
    let result = rt.evaluate(r#"
            const f = document.getElementById('f');
            const t = document.getElementById('t');
            const ta = document.getElementById('ta');
            const c = document.getElementById('c');
            const s = document.getElementById('s');
            t.value = 'typed';
            ta.value = 'typed ta';
            c.checked = false;
            s.value = 'a';
            const before = [
                t.value, ta.value,
                t.getAttribute('value'),
                t.outerHTML.includes('typed'),
                ta.outerHTML.includes('typed ta'),
                document.forms[0].t.value,
                c.checked, s.value,
            ];
            let sawReset = 0, cancelled = false;
            f.addEventListener('reset', (e) => { sawReset++; cancelled = e.cancelable; });
            f.reset();
            const after = [sawReset, cancelled, t.value, ta.value, c.checked, s.value];
            // A vetoed reset leaves every control untouched.
            f.addEventListener('reset', (e) => e.preventDefault());
            t.value = 'typed2';
            f.reset();
            return [before, after, t.value, t.getAttribute('value'), t.outerHTML.includes('typed2')];
        "#).unwrap();
    assert_eq!(
        result,
        serde_json::json!([
            ["typed", "typed ta", "stale", false, false, "typed", false, "a"],
            [1, true, "stale", "ta0", true, "b"],
            "typed2",
            "stale",
            false,
        ])
    );
}

#[test]
fn test_form_paint_mirrors_from_js() {
    // The form paint batch's JS halves, read from the Rust tree: every
    // select mutation entry point (value / selectedIndex / option
    // .selected / form.reset) recomputes the displayed label into
    // live_value, and a checked write lands as live_checked — the layout
    // tests drive the Rust setters directly, so this is the only place
    // proving the bootstrap actually fires the mirrors.
    let mut rt = setup_runtime(
        r#"<form id=f>
            <select id=s><option value=a>A</option><option value=b selected>B</option></select>
            <input id=c type=checkbox>
            </form>"#,
    );
    let label_of = |rt: &JsRuntime| {
        rt.with_dom(|dom| {
            let s = dom.query_selector_all("#s").unwrap()[0];
            dom.with_node(s, |n| n.live_value().map(str::to_string))
                .flatten()
        })
        .flatten()
    };
    let checked_of = |rt: &JsRuntime| {
        rt.with_dom(|dom| {
            let c = dom.query_selector_all("#c").unwrap()[0];
            dom.with_node(c, |n| n.live_checked()).flatten()
        })
        .flatten()
    };

    rt.evaluate("document.getElementById('s').value = 'a'")
        .unwrap();
    assert_eq!(
        label_of(&rt).as_deref(),
        Some("A"),
        "value setter mirrors the label"
    );

    rt.evaluate("document.getElementById('s').selectedIndex = 1")
        .unwrap();
    assert_eq!(
        label_of(&rt).as_deref(),
        Some("B"),
        "selectedIndex mirrors despite bypassing the property setter"
    );

    rt.evaluate("document.getElementById('s').options[0].selected = true")
        .unwrap();
    assert_eq!(
        label_of(&rt).as_deref(),
        Some("A"),
        "option.selected mirrors through its owning select"
    );

    rt.evaluate("document.getElementById('c').checked = true")
        .unwrap();
    assert_eq!(
        checked_of(&rt),
        Some(true),
        "checked setter mirrors live_checked"
    );

    rt.evaluate("document.getElementById('f').reset()").unwrap();
    assert_eq!(
        label_of(&rt).as_deref(),
        Some("B"),
        "reset mirrors the parsed selected attr"
    );
    assert_eq!(
        checked_of(&rt),
        Some(false),
        "reset mirrors the parsed (absent) checked attr"
    );
}

#[test]
fn test_selection_mirror_records_js_writes() {
    // The typing-cursor batch's JS halves, read from the Rust tree: focus,
    // setSelectionRange, value writes, selectionStart writes, and blur all
    // keep the (node, start, end) selection mirror true to Chrome's
    // semantics — focus with no record parks the caret at the value end,
    // a value write resets it to the new end, blur keeps the record while
    // the focus moves on.
    let mut rt = setup_runtime(r#"<input id="q" value="abcd">"#);
    let sel_of = |rt: &JsRuntime| {
        rt.with_dom(|dom| {
            let q = dom.query_selector_all("#q").unwrap()[0];
            (
                dom.focused_node() == Some(q),
                dom.selection().filter(|(nid, _, _)| *nid == q),
            )
        })
        .expect("dom handle")
    };

    rt.evaluate("document.getElementById('q').focus()").unwrap();
    let (focused, sel) = sel_of(&rt);
    assert!(focused, "focus() records focused_node");
    let (_q, x, y) = sel.expect("focus() writes a selection record");
    assert_eq!(
        (x, y),
        (4, 4),
        "focus() with no record parks at the value end"
    );

    rt.evaluate("document.getElementById('q').setSelectionRange(1, 3)")
        .unwrap();
    let (_, sel) = sel_of(&rt);
    assert_eq!(
        sel.map(|(_, s, e)| (s, e)),
        Some((1, 3)),
        "setSelectionRange mirrors"
    );

    rt.evaluate("document.getElementById('q').value = 'ab'")
        .unwrap();
    let (_, sel) = sel_of(&rt);
    assert_eq!(
        sel.map(|(_, s, e)| (s, e)),
        Some((2, 2)),
        "value write resets the mirror to the new end"
    );

    rt.evaluate("document.getElementById('q').selectionStart = 1")
        .unwrap();
    let (_, sel) = sel_of(&rt);
    assert_eq!(
        sel.map(|(_, s, e)| (s, e)),
        Some((1, 2)),
        "selectionStart write mirrors"
    );

    rt.evaluate("document.getElementById('q').blur()").unwrap();
    let (focused, sel) = sel_of(&rt);
    assert!(!focused, "blur clears focused_node");
    assert_eq!(
        sel.map(|(_, s, e)| (s, e)),
        Some((1, 2)),
        "blur keeps the selection record for the refocus"
    );
}

#[test]
fn test_set_range_text_chrome_pinned_selection() {
    // obscura #1414 absorption: setRangeText must insert into an EMPTY
    // value (no early return) and place the selection the way real Chrome
    // 157 does — pinned by CDP probe on 2026-11-06, all six shapes
    // byte-identical. 'preserve' keys off the PRE-CALL selection: spanning
    // the replaced range selects the replacement, before it stays, strictly
    // after it shifts by the length delta.
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate(
            r#"
            const out = [];
            const shape = (fn) => { const t = document.createElement('textarea'); fn(t); out.push(t.value + '@' + t.selectionStart + '-' + t.selectionEnd); };
            shape((t) => { t.value = 'hello'; t.setSelectionRange(0, 0); t.setRangeText('XY'); });
            shape((t) => { t.setRangeText('A'); });
            shape((t) => { t.value = 'abcdef'; t.setSelectionRange(2, 4, 'forward'); t.setRangeText('Z'); });
            shape((t) => { t.value = 'abcdef'; t.setSelectionRange(5, 5); t.setRangeText('Z', 0, 2); });
            shape((t) => { t.value = 'abcdef'; t.setSelectionRange(3, 3); t.setRangeText('LONG', 2, 4); });
            shape((t) => { t.value = 'abcdef'; t.setRangeText('!', 2, 4, 'select'); });
            return out.join('|');
        "#,
        )
        .unwrap();
    assert_eq!(
        result,
        serde_json::json!("XYhello@0-0|A@0-0|abZef@2-3|Zcdef@4-4|abLONGef@2-6|ab!ef@2-3"),
        "all six shapes pinned to real Chrome 157"
    );
}

#[test]
fn test_selection_api_throws_on_non_text_inputs() {
    // obscura 11-04 absorption: the selection API is text-entry-only.
    // On a checkbox, setSelectionRange/setRangeText/selectionStart writes
    // throw InvalidStateError (before touching any mirror) and the
    // getters answer null; a text input right next to it keeps working.
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate(
            r#"
            const out = [];
            const cb = document.createElement('input');
            cb.type = 'checkbox';
            document.body.appendChild(cb);
            const name = (fn) => { try { fn(); out.push('no-throw'); } catch (e) { out.push(e.name); } };
            name(() => cb.setSelectionRange(0, 1));
            name(() => cb.setRangeText('x'));
            name(() => { cb.selectionStart = 1; });
            out.push(String(cb.selectionStart) + '/' + String(cb.selectionEnd));
            const ti = document.createElement('input');
            ti.value = 'hello';
            document.body.appendChild(ti);
            ti.setSelectionRange(1, 3);
            ti.setRangeText('XY', 0, 0);
            out.push(ti.value + '@' + ti.selectionStart + '-' + ti.selectionEnd);
            return out.join('|');
        "#,
        )
        .unwrap();
    assert_eq!(
        result,
        serde_json::json!(
            "InvalidStateError|InvalidStateError|InvalidStateError|null/null|XYhello@3-5"
        ),
        "checkbox throws on all three, getters null; text input unaffected"
    );
}

#[test]
fn test_option_legacy_factory() {
    // glama.ai admin form chunk populates selects via `new Option(label,
    // value)`; without the global its hydration died on "Option is not
    // defined". Same deal as `new Image()`: return a real <option> element
    // so value/selected semantics and select.add() come for free.
    let mut rt = setup_runtime("<select id=s></select>");
    let result = rt
        .evaluate(
            r#"
            const sel = document.getElementById('s');
            const opt = new Option('Label A', 'a');
            sel.appendChild(opt);
            const opt2 = new Option('B', 'b', false, true);
            sel.appendChild(opt2);
            // HTMLOptionsCollection surface on select.options
            sel.options.add(new Option('C', 'c'));
            sel.options.remove(1);
            return [
                typeof Option,
                opt.tagName, opt.text, opt.value,
                sel.options.length, sel.options[0].value,
                opt2.selected, opt2.textContent,
                typeof sel.options.add,
                sel.options.length, sel.options[1].value,
                sel.options.selectedIndex,
                // option.selected = true is exclusive: the sibling that carried
                // the selected attribute loses value/selectedIndex (glama form).
                (sel.options[1].selected = true),
                sel.value,
            ];
        "#,
        )
        .unwrap();
    assert_eq!(
        result,
        serde_json::json!([
            "function", "OPTION", "Label A", "a", 2, "a", true, "B", "function", 2, "c", 0, true,
            "c",
        ])
    );
}

#[test]
fn test_input_value() {
    let mut rt = setup_runtime(
        r#"<form><input id="name" type="text" value="initial"><textarea id="bio">old text</textarea></form>"#,
    );
    let val = rt
        .evaluate("document.getElementById('name').value")
        .unwrap();
    assert_eq!(val, serde_json::json!("initial"));
    rt.execute_script(
        "test",
        "document.getElementById('name').value = 'new value';",
    )
    .unwrap();
    let val2 = rt
        .evaluate("document.getElementById('name').value")
        .unwrap();
    assert_eq!(val2, serde_json::json!("new value"));
    let bio = rt.evaluate("document.getElementById('bio').value").unwrap();
    assert_eq!(bio, serde_json::json!("old text"));
}

#[test]
fn test_checkbox_checked() {
    let mut rt = setup_runtime(r#"<input id="cb" type="checkbox" checked>"#);
    let checked = rt
        .evaluate("document.getElementById('cb').checked")
        .unwrap();
    assert_eq!(checked, serde_json::json!(true));
    rt.execute_script("test", "document.getElementById('cb').checked = false;")
        .unwrap();
    let checked2 = rt
        .evaluate("document.getElementById('cb').checked")
        .unwrap();
    assert_eq!(checked2, serde_json::json!(false));
}

// Batch-67 leftover: form widgets honor position:absolute. An input
// carrying `position:absolute; left/top` must land at that offset like
// any other box — replaced-widget boxes used to stay in flow position.
#[test]
#[cfg(feature = "screenshot")]
fn form_widget_honors_absolute_positioning() {
    let mut rt = setup_runtime(
        r#"<html><head><style>
          .abs { position: absolute; left: 100px; top: 50px; }
        </style></head><body>
        <div id="d" class="abs">x</div>
        <input id="txt" class="abs" type="text" value="hi">
        <input id="chk" class="abs" type="checkbox">
        </body></html>"#,
    );
    let diag = rt
            .evaluate(
                r#"JSON.stringify((() => {
                    try {
                        const g = id => { const r = document.getElementById(id).getBoundingClientRect();
                                          return [Math.round(r.left), Math.round(r.top)]; };
                        return { d: g('d'), txt: g('txt'), chk: g('chk') };
                    } catch (err) { return { error: String(err) }; }
                })())"#,
            )
            .unwrap();
    println!("form widget abspos diagnostics: {diag}");
    let v: serde_json::Value = serde_json::from_str(diag.as_str().unwrap()).unwrap();
    if v.get("error").is_some() {
        panic!("probe threw: {diag}");
    }
    assert_eq!(v["d"], serde_json::json!([100, 50]), "control div: {diag}");
    assert_eq!(
        v["txt"],
        serde_json::json!([100, 50]),
        "text input must honor position:absolute: {diag}"
    );
    assert_eq!(
        v["chk"],
        serde_json::json!([100, 50]),
        "checkbox must honor position:absolute: {diag}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_dialog_escape_helper_closes_modal_only() {
    // obscura#952: the bootstrap helper behind the CDP Escape arm walks
    // from the focused element to the containing modal dialog (falls back
    // to the open modal dialog in document order), runs the close request
    // (cancelable `cancel`), and never touches non-modal dialogs.
    // show()/showModal() schedule setTimeout toggles, whose op_sleep
    // needs a current-thread reactor — plain #[test] evaluate has none.
    let mut rt = setup_runtime(
        r#"<html><body>
            <dialog id="m"><input id="in"><button id="ok">OK</button></dialog>
            <dialog id="nm">non-modal</dialog>
        </body></html>"#,
    );
    let result = rt.evaluate(r#"
            const m = document.getElementById('m');
            const nm = document.getElementById('nm');
            nm.show();
            let cancels = 0, wasCancelable = false;
            m.addEventListener('cancel', () => { cancels++; });
            // No modal dialog anywhere: helper is a no-op, non-modal untouched.
            const noneOpen = globalThis.__diting_dialogEscapeClose();
            const nmUntouched = nm.hasAttribute('open');
            // Modal open, focus inside it: cancel fires, dialog closes.
            m.showModal();
            document.getElementById('in').focus();
            const viaFocus = globalThis.__diting_dialogEscapeClose();
            const closedAfterFocus = !m.hasAttribute('open');
            // preventDefault inside cancel keeps the modal open (Chrome Escape
            // semantics); the CDP arm checks keydown defaultPrevented too.
            m.showModal();
            m.addEventListener('cancel', (e) => { wasCancelable = e.cancelable && !e.defaultPrevented; e.preventDefault(); });
            document.getElementById('ok').focus();
            globalThis.__diting_dialogEscapeClose();
            const stillOpenAfterPrevent = m.hasAttribute('open');
            return [noneOpen, nmUntouched, viaFocus, closedAfterFocus, stillOpenAfterPrevent, cancels, wasCancelable];
        "#).unwrap();
    assert_eq!(
        result,
        serde_json::json!([false, true, true, true, true, 2, true])
    );
}

#[test]
fn test_form_submit_bypasses_event_request_submit_fires_it() {
    // Upstream 7e2cabf + ccfa5fb: submit() is a direct pass-through that
    // a submit listener cannot veto; only requestSubmit() (and user
    // clicks) fire the cancelable submit event. requestSubmit's submitter
    // must be a submit button owned by this form.
    let mut rt = setup_runtime(
        r#"
            <form id="f" action="/go"><input name="q" value="x">
                <button type="submit" id="b">Go</button></form>
            <form id="other"><button type="submit" id="ob">Go</button></form>
            <div id="notabutton"></div>"#,
    );
    // submit(): no event, navigation happens.
    let r = rt
        .evaluate(
            r#"
            const form = document.getElementById('f');
            globalThis.__evts = 0;
            form.addEventListener('submit', () => globalThis.__evts++);
            form.submit();
            return [globalThis.__evts];
        "#,
        )
        .unwrap();
    assert_eq!(r, serde_json::json!([0]));
    assert!(
        rt.take_pending_navigation().is_some(),
        "submit() must navigate"
    );

    // requestSubmit(): event fires; preventDefault stops navigation.
    let r = rt
        .evaluate(
            r#"
            const form = document.getElementById('f');
            form.addEventListener('submit', e => e.preventDefault());
            form.requestSubmit();
            return [globalThis.__evts];
        "#,
        )
        .unwrap();
    assert_eq!(r, serde_json::json!([1]));
    assert!(
        rt.take_pending_navigation().is_none(),
        "preventDefault must veto navigation"
    );

    // Submitter validation (ccfa5fb): non-submit-button -> TypeError;
    // foreign submit button -> NotFoundError; valid one fires the event.
    let r = rt
        .evaluate(
            r#"
            const form = document.getElementById('f');
            const out = {};
            try { form.requestSubmit(document.getElementById('notabutton')); out.a = 'no-throw'; }
            catch (e) { out.a = e.name; }
            try { form.requestSubmit(document.getElementById('ob')); out.b = 'no-throw'; }
            catch (e) { out.b = e.name; }
            form.requestSubmit(document.getElementById('b'));
            out.c = globalThis.__evts;
            return [out.a, out.b, out.c];
        "#,
        )
        .unwrap();
    // The preventDefault listener from the previous step is still attached,
    // so the valid requestSubmit fires the event (2 total) but does not
    // navigate.
    assert_eq!(r, serde_json::json!(["TypeError", "NotFoundError", 2]));
    assert!(rt.take_pending_navigation().is_none());
}

#[test]
fn test_select_parity_type_selectedindex_add_no_change_on_assign() {
    // Upstream 5308e04: select/textarea report fixed IDL types;
    // a single select implicitly selects its first option (a multiple
    // one idles at -1); programmatic value assignment never fires
    // change (assigning inside a change handler used to loop forever).
    let mut rt = setup_runtime(
        r#"
            <select id="s"><option value="a">A</option><option value="b">B</option></select>
            <select id="m" multiple><option value="a">A</option></select>
            <textarea id="t"></textarea>"#,
    );
    let result = rt
        .evaluate(
            r#"
            const s = document.getElementById('s');
            const m = document.getElementById('m');
            const t = document.getElementById('t');
            let changes = 0;
            s.addEventListener('change', () => changes++);
            s.value = 'b';
            const afterAssign = [changes, s.value, s.selectedIndex];
            s.selectedIndex = 0;
            const afterIndex = [s.value, s.selectedIndex];
            const types = [s.type, m.type, t.type];
            const emptySingle = document.createElement('select');
            const emptyMultiple = document.createElement('select');
            emptyMultiple.setAttribute('multiple', '');
            const opt = document.createElement('option');
            opt.setAttribute('value', 'c'); opt.textContent = 'C';
            s.add(opt);
            return [
                afterAssign, afterIndex, types,
                emptySingle.selectedIndex, emptyMultiple.selectedIndex,
                s.options.length, changes,
            ];
        "#,
        )
        .unwrap();
    assert_eq!(
        result,
        serde_json::json!([
            [0, "b", 1], // no change on assignment; selection moved
            ["a", 0],    // selectedIndex setter works both ways
            ["select-one", "select-multiple", "textarea"],
            -1,
            -1, // empty selects idle at -1
            3,  // add() appended the option
            0,  // assignment never fired change
        ])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_exec_command_insert_text_event_shape() {
    // document.execCommand('insertText') is the Chrome-parity typing
    // channel that scripted forms accept: beforeinput (cancelable, with
    // data) → caret-shaped insert → input (InputEvent carrying
    // inputType/data). A plain Event('input') has no payload, and
    // controlled widget layers (taobao publish form's Fusion Next)
    // reject the value without it.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"() => {
            const inp = document.createElement('input');
            document.body.appendChild(inp);
            inp.value = 'ab';
            inp.focus();
            const log = [];
            inp.addEventListener('beforeinput', (e) => {
                log.push(['bi', e.inputType, e.data, e.cancelable]);
            });
            inp.addEventListener('input', (e) => {
                log.push(['in', e.inputType, e.data, e.cancelable]);
            });
            const ok = document.execCommand('insertText', false, 'XY');
            const afterEnd = { v: inp.value, s: inp.selectionStart, e: inp.selectionEnd };
            inp.setSelectionRange(1, 3);
            const ok2 = document.execCommand('insertText', false, 'W');
            const afterRange = { v: inp.value, s: inp.selectionStart, e: inp.selectionEnd };
            inp.addEventListener('beforeinput', (e) => e.preventDefault(), { once: true });
            const ok3 = document.execCommand('insertText', false, 'V');
            const afterVeto = inp.value;
            const ok4 = document.execCommand('bold');
            inp.blur();
            const ok5 = document.execCommand('insertText', false, 'Q');
            return Promise.resolve({ ok, ok2, ok3, ok4, ok5, afterEnd, afterRange, afterVeto, log });
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    assert!(v.is_object(), "script must finish");
    assert_eq!(v["ok"], true, "end-of-value insert runs");
    assert_eq!(v["afterEnd"]["v"], "abXY", "appends at caret-at-end");
    assert_eq!(v["afterEnd"]["s"], 4);
    assert_eq!(v["afterEnd"]["e"], 4, "caret lands after insertion");
    assert_eq!(v["ok2"], true, "range insert runs");
    assert_eq!(v["afterRange"]["v"], "aWY", "selection span is replaced");
    assert_eq!(v["afterRange"]["s"], 2, "caret after range insert");
    assert_eq!(v["ok3"], true, "vetoed command still answers true");
    assert_eq!(v["afterVeto"], "aWY", "preventDefault aborts the insert");
    assert_eq!(v["ok4"], false, "unsupported command answers false");
    assert_eq!(v["ok5"], false, "no text-entry focus answers false");
    let log = v["log"].as_array().unwrap();
    assert_eq!(
        log.len(),
        5,
        "bi+in per insert, bi only for veto: {:?}",
        log
    );
    assert_eq!(log[0][0], "bi");
    assert_eq!(log[0][1], "insertText");
    assert_eq!(log[0][2], "XY");
    assert_eq!(log[0][3], true, "beforeinput is cancelable");
    assert_eq!(log[1][0], "in");
    assert_eq!(log[1][1], "insertText");
    assert_eq!(log[1][2], "XY", "input carries the data payload");
    assert_eq!(log[1][3], false, "input is not cancelable");
}

#[tokio::test(flavor = "current_thread")]
async fn test_summary_click_toggles_details() {
    // #283, shape checked against headless Chrome.
    let mut rt = setup_runtime(
            "<html><body><details id=d><summary id=s>sum <b id=bb>bold</b> <a id=lnk href=\"#x\">l</a></summary>body</details>\
             <details id=e name=g open><summary>e</summary>x</details><details id=f name=g><summary>f</summary>y</details>\
             <details id=n><div>no summary</div></details></body></html>",
        );
    let v = rt
            .evaluate(
                "(() => { const r = []; globalThis.__tg = [];
                  for (const x of [d, e, f]) x.addEventListener('toggle', ev => __tg.push(x.id + ':' + ev.oldState + '>' + ev.newState));
                  r.push(typeof d.open + d.open);
                  s.click(); r.push(d.open);
                  bb.click(); r.push(d.open);
                  lnk.addEventListener('click', ev => ev.preventDefault()); lnk.click(); r.push(d.open);
                  s.click();
                  n.querySelector('div').click(); r.push('n' + n.open);
                  f.open = true; r.push(e.open + ',' + f.open);
                  const sm = document.createElement('summary'); d.append(sm); sm.click(); r.push('second' + d.open);
                  return r.join('|'); })()",
            )
            .unwrap();
    assert_eq!(
        v,
        "booleanfalse|true|false|false|nfalse|false,true|secondtrue"
    );
    let _ = rt.run_event_loop_bounded(100).await;
    let tg = rt.evaluate("__tg.slice().sort().join(',')").unwrap();
    assert_eq!(tg, "d:closed>open,e:open>closed,f:closed>open");
}

#[tokio::test(flavor = "current_thread")]
async fn test_radio_indeterminate_and_details_attribute_toggle() {
    // Chrome truth (headless Chrome, 2026-10-22).
    let mut rt = setup_runtime(
            "<html><body><form id=f1><input type=radio name=g id=r1><input type=radio name=g id=r2 checked></form>\
             <form id=f2><input type=radio name=g id=r3><input type=radio name=g id=r4></form>\
             <input type=radio name=h id=r5><input type=radio id=r6><input type=radio id=r7 checked>\
             <details id=d><summary>s</summary>x</details></body></html>",
        );
    let v = rt
            .evaluate(
                "(() => { const r = [];
                  r.push(['r1','r2','r3','r4','r5','r6','r7'].map(i => document.getElementById(i).matches(':indeterminate')).join());
                  r2.checked = false; r.push(r1.matches(':indeterminate') + ',' + r2.matches(':indeterminate'));
                  r5.checked = true; r.push(r5.matches(':indeterminate'));
                  globalThis.__tg = []; d.addEventListener('toggle', e => __tg.push(e.oldState + '>' + e.newState));
                  d.setAttribute('open', '');
                  return r.join('|'); })()",
            )
            .unwrap();
    assert_eq!(v, "false,false,true,true,true,true,false|true,true|false");
    let _ = rt.run_event_loop_bounded(100).await;
    rt.evaluate("d.removeAttribute('open')").unwrap();
    let _ = rt.run_event_loop_bounded(100).await;
    rt.evaluate("(d.setAttribute('open', ''), d.removeAttribute('open'))")
        .unwrap();
    let _ = rt.run_event_loop_bounded(100).await;
    assert_eq!(
        rt.evaluate("__tg.join()").unwrap(),
        "closed>open,open>closed,closed>closed"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_constraint_validation_matches_chrome() {
    // Chrome truth (headless Chrome, 2026-10-22): flags, willValidate,
    // radio-group required, custom validity, invalid events, and
    // requestSubmit blocked by interactive validation unless novalidate.
    let mut rt = setup_runtime(
            "<html><body><form id=f>
              <input id=a required><input id=b type=email value=x><input id=c type=email value='a@'>
              <input id=d type=number min=5 max=10 step=2 value=3><input id=e type=number min=5 max=10 step=2 value=11>
              <input id=g type=number min=5 step=2 value=8><input id=h pattern='[a-z]+' value=AB><input id=i type=url value=nope>
              <input id=j type=checkbox required><input id=k type=date min='2024-01-01' value='2023-05-05'>
              <input id=l disabled required><input id=m readonly required>
              <select id=n required><option value=''>pick</option><option>x</option></select><textarea id=o required></textarea>
              <input id=p type=radio name=r required><input id=q type=radio name=r><input id=ok value=fine required>
             </form></body></html>",
        );
    let v = rt
            .evaluate(
                "(() => { const r = [];
                  for (const i of 'abcdeghijklmnopq'.split('').concat(['ok'])) { const e = document.getElementById(i); const v = e.validity;
                    r.push(i + ':' + (['valueMissing','typeMismatch','patternMismatch','rangeUnderflow','rangeOverflow','stepMismatch','customError'].filter(k => v[k]).join('+') || '-') + ':' + e.willValidate); }
                  const a = document.getElementById('a'); a.setCustomValidity('boo');
                  r.push(a.validity.customError + a.validationMessage + a.checkValidity()); a.setCustomValidity('');
                  const log = []; document.getElementById('b').addEventListener('invalid', e => log.push(e.cancelable + '' + e.bubbles));
                  r.push(document.getElementById('b').checkValidity() + log.join());
                  const f = document.getElementById('f'); let n = 0; f.addEventListener('submit', e => { n++; e.preventDefault(); });
                  f.requestSubmit(); r.push('blocked' + n); f.noValidate = true; f.requestSubmit(); r.push('novalidate' + n);
                  r.push(typeof document.createElement('div').validity);
                  return r.join('|'); })()",
            )
            .unwrap();
    assert_eq!(
            v,
            "a:valueMissing:true|b:typeMismatch:true|c:typeMismatch:true|d:rangeUnderflow:true|e:rangeOverflow:true|g:stepMismatch:true|h:patternMismatch:true|i:typeMismatch:true|j:valueMissing:true|k:rangeUnderflow:true|l:-:false|m:-:false|n:valueMissing:true|o:valueMissing:true|p:valueMissing:true|q:valueMissing:true|ok:-:true|trueboofalse|falsetruefalse|blocked0|novalidate1|undefined"
        );
}

/// M4 knife 7: behavior of the form-family members the reflect fingerprints
/// cannot classify (htmlform-probe.js, 36 keys, real Chrome 145): showPicker's
/// NotAllowedError wording, input.list/webkitEntries/type=image dimensions,
/// popoverTargetElement/commandForElement element reflection, select.length/
/// item/namedItem, option.index, output value/defaultValue (default-value
/// override flag), object/fieldset/output constraint-validation surface,
/// fieldset.elements / map.areas, textarea defaultValue/textLength, marquee
/// (scroll*, loop's IndexSizeError, start/stop), meter (clamped doubles) and
/// progress (position), slot.assign, track (readyState/track/constants/kind
/// enum), template shadowroot* enum. Every value is diffed whole against the
/// truth; page URLs are normalized (#B/#O) so the unit page and Chrome's
/// ephemeral server compare equal.
#[tokio::test(flavor = "current_thread")]
async fn htmlform_behaviors_match_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/htmlform-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/htmlform-truth-145.json"
    ))
    .unwrap();
    let mut bad = Vec::new();
    for (k, want) in truth.as_object().unwrap() {
        if &engine[k] != want {
            bad.push(format!("{k}:\n   engine {}\n   chrome {want}", engine[k]));
        }
    }
    assert!(
        bad.is_empty(),
        "{} probe(s) off:\n{}",
        bad.len(),
        bad.join("\n")
    );
}
