//! 节点操作：clone/insert/remove/treewalker/attr/选择器/table API
use super::common::setup_runtime;

/// Issue #51: `Element.getAttributeNames` was missing outright — the xhs
/// creator publish page's mount path calls it unguarded and the whole
/// component tree dies with `TypeError: B.getAttributeNames is not a
/// function` (`div.upload-content` never renders). `hasAttributes` was
/// hard-coded `true` in the same family; the cloneNode collateral (the
/// `_shallowCloneNode` names fallback returned `[]` forever, so element
/// clones lost all attributes) is pinned too.
#[test]
fn element_get_attribute_names() {
    let mut rt = setup_runtime("<html><body><div id='t'>x</div></body></html>");
    let out = rt
        .evaluate(
            r#"
            var bare = document.createElement('div');
            var d = document.createElement('div');
            d.setAttribute('class', 'a');
            d.setAttribute('data-x', '1');
            var clone = d.cloneNode(false);
            return JSON.stringify({
                fn: typeof bare.getAttributeNames,
                bareNames: bare.getAttributeNames(),
                bareHas: [bare.hasAttributes(), d.hasAttributes()],
                names: d.getAttributeNames(),
                proto: 'getAttributeNames' in Element.prototype
                        && Element.prototype.getAttributeNames === bare.getAttributeNames,
                cloneKeepsAttrs: clone.getAttributeNames(),
                cloneReads: [clone.getAttribute('class'), clone.getAttribute('data-x')],
                attrsIndexing: d.attributes.length,
                liveUpdate: (function () {
                    d.setAttribute('role', 'banner');
                    return d.getAttributeNames();
                })(),
            });
        "#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(v["fn"], serde_json::json!("function"));
    assert_eq!(
        v["bareNames"],
        serde_json::json!([]),
        "no attributes -> empty array"
    );
    assert_eq!(
        v["bareHas"],
        serde_json::json!([false, true]),
        "hasAttributes tracks reality"
    );
    assert_eq!(
        v["names"],
        serde_json::json!(["class", "data-x"]),
        "names come back in set order"
    );
    assert_eq!(
        v["proto"],
        serde_json::json!(true),
        "lives on Element.prototype"
    );
    assert_eq!(
        v["cloneKeepsAttrs"],
        serde_json::json!(["class", "data-x"]),
        "cloneNode shallow path no longer drops attributes (#51 collateral)"
    );
    assert_eq!(v["cloneReads"], serde_json::json!(["a", "1"]));
    assert_eq!(
        v["attrsIndexing"],
        serde_json::json!(2),
        "attributes list agrees"
    );
    assert_eq!(
        v["liveUpdate"],
        serde_json::json!(["class", "data-x", "role"]),
        "set of names reflects later setAttribute calls"
    );
}

#[test]
fn reflected_table_section_row_cell_families() {
    let mut rt = setup_runtime(
        "<html><body><table><thead char='.' charoff='2'><tr><th>H</th></tr></thead>\
             <tbody align='center' valign='top'><tr><td nowrap axis='name'>x</td></tr></tbody>\
             </table></body></html>",
    );
    let out = rt
        .evaluate(
            r#"
            var thead = document.querySelector('thead');
            var td = document.querySelector('td');
            var sec = document.createElement('tbody'), row = document.createElement('tr');
            var cell = document.createElement('td');
            sec.ch = '.'; sec.chOff = '3'; sec.align = 'left'; sec.vAlign = 'bottom';
            row.ch = '|'; row.bgColor = '#eee';
            cell.axis = 'ax'; cell.width = '30';
            var secNull = (sec.align = null, sec.getAttribute('align'));
            var rowBgNull = (row.bgColor = null, row.getAttribute('bgcolor'));
            var nw = document.createElement('td');
            var nwAbsent = nw.noWrap;
            nw.noWrap = true;
            var nwTrueAttr = nw.getAttribute('nowrap');
            var nwTrueGet = nw.noWrap;
            nw.noWrap = '0';
            var nwStringZero = nw.noWrap;
            nw.noWrap = false;
            var nwAfterFalse = [nw.hasAttribute('nowrap'), nw.noWrap];
            return JSON.stringify({
                faces: [Object.keys(HTMLTableSectionElement.prototype).sort(),
                        Object.keys(HTMLTableRowElement.prototype).sort(),
                        Object.keys(HTMLTableCellElement.prototype).sort()],
                parsed: [thead.ch, thead.chOff, td.noWrap, td.axis,
                         document.querySelector('tbody').align,
                         document.querySelector('tbody').vAlign],
                secAttrs: [sec.getAttribute('char'), sec.getAttribute('charoff'),
                           sec.getAttribute('valign')],
                rowAttrs: [row.getAttribute('char')],
                cellAttrs: [cell.getAttribute('axis'), cell.getAttribute('width')],
                attrToProp: (sec.setAttribute('char', '!'), sec.ch),
                nullModes: [secNull, rowBgNull],
                nw: [nwAbsent, nwTrueAttr, nwTrueGet, nwStringZero, nwAfterFalse],
            });
        "#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    let faces = v["faces"].as_array().unwrap();
    assert_eq!(
        faces[0],
        serde_json::json!([
            "align",
            "ch",
            "chOff",
            "deleteRow",
            "insertRow",
            "rows",
            "vAlign"
        ]),
        "section prototype face"
    );
    assert_eq!(
        faces[1],
        serde_json::json!([
            "align",
            "bgColor",
            "cells",
            "ch",
            "chOff",
            "deleteCell",
            "insertCell",
            "rowIndex",
            "sectionRowIndex",
            "vAlign"
        ]),
        "row prototype face: legacy five + the DOM members"
    );
    assert_eq!(
        faces[2],
        serde_json::json!([
            "abbr",
            "align",
            "axis",
            "bgColor",
            "cellIndex",
            "ch",
            "chOff",
            "colSpan",
            "headers",
            "height",
            "noWrap",
            "rowSpan",
            "scope",
            "vAlign",
            "width"
        ]),
        "cell prototype face: matches Chrome's enumerable face exactly (#35)"
    );
    assert_eq!(
        v["parsed"],
        serde_json::json!([".", "2", true, "name", "center", "top"]),
        "parsed-in markup reads through the accessors (char/charoff/nowrap/axis/align/valign)"
    );
    assert_eq!(
        v["secAttrs"],
        serde_json::json!([".", "3", "bottom"]),
        "ch/chOff write the char/charoff content attributes"
    );
    assert_eq!(v["rowAttrs"], serde_json::json!(["|"]));
    assert_eq!(v["cellAttrs"], serde_json::json!(["ax", "30"]));
    assert_eq!(
        v["attrToProp"],
        serde_json::json!("!"),
        "char attr write -> ch read"
    );
    assert_eq!(
        v["nullModes"],
        serde_json::json!(["null", ""]),
        "plain align -> \"null\"; nullEmpty bgColor -> \"\""
    );
    assert_eq!(
        v["nw"],
        serde_json::json!([false, "", true, true, [false, false]]),
        "boolean reflect: absent false; true -> empty-valued attribute; '0' -> true; false -> removed"
    );
}

#[test]
fn test_query_selector() {
    let mut rt = setup_runtime("<html><body><h1>Hello</h1><p>World</p></body></html>");
    let text = rt
        .evaluate("document.querySelector('h1').textContent")
        .unwrap();
    assert_eq!(text, serde_json::json!("Hello"));
}

#[test]
fn test_query_selector_all() {
    let mut rt = setup_runtime("<ul><li>A</li><li>B</li><li>C</li></ul>");
    let count = rt
        .evaluate("document.querySelectorAll('li').length")
        .unwrap();
    assert_eq!(count.as_f64().unwrap() as i64, 3);
}

#[test]
fn test_get_element_by_id() {
    let mut rt = setup_runtime(r#"<div id="test">Content</div>"#);
    let tag = rt
        .evaluate("document.getElementById('test').tagName")
        .unwrap();
    assert_eq!(tag, serde_json::json!("DIV"));
}

#[test]
fn document_fragment_get_element_by_id_searches_descendants() {
    let mut rt = setup_runtime(r#"<div id="target">document</div>"#);
    let result = rt
            .evaluate(
                r#"
                (() => {
                    const frag = document.createDocumentFragment();
                    const section = document.createElement('section');
                    section.innerHTML = '<div><span id="target">fragment</span></div><p id="a.b">literal</p>';
                    frag.appendChild(section);

                    const dup = document.createDocumentFragment();
                    const deepParent = document.createElement('div');
                    deepParent.innerHTML = '<span id="dup">deep</span>';
                    const shallow = document.createElement('p');
                    shallow.id = 'dup';
                    shallow.textContent = 'shallow';
                    dup.appendChild(deepParent);
                    dup.appendChild(shallow);

                    return [
                        frag.getElementById('target').textContent,
                        frag.getElementById('missing') === null,
                        frag.getElementById('a.b').textContent,
                        frag.getElementById(123) === null,
                        dup.getElementById('dup').textContent,
                    ];
                })()
                "#,
            )
            .unwrap();
    assert_eq!(
        result,
        serde_json::json!(["fragment", true, "literal", true, "deep"])
    );
}

/// Regression for #105: `element.querySelector` and `querySelectorAll`
/// must scope to the receiver's subtree, not the whole document.
#[test]
fn element_query_selector_is_scoped_to_subtree() {
    let mut rt = setup_runtime(
        r#"<div id="a"><span class="x">in a</span></div><div id="b"><span class="x">in b</span></div>"#,
    );
    let text = rt
        .evaluate("document.getElementById('a').querySelector('.x').textContent")
        .unwrap();
    assert_eq!(text, serde_json::json!("in a"));

    let count_in_a = rt
        .evaluate("document.getElementById('a').querySelectorAll('.x').length")
        .unwrap();
    assert_eq!(count_in_a.as_f64().unwrap() as i64, 1);

    // Document-scoped query still sees both.
    let count_doc = rt
        .evaluate("document.querySelectorAll('.x').length")
        .unwrap();
    assert_eq!(count_doc.as_f64().unwrap() as i64, 2);
}

/// Table DOM API family (WHATWG §4.9): script-built tables — insertRow
/// auto-creates the tbody, `rows` reads in spec order (thead, tbodies,
/// tfoot — not tree order), and the per-row/cell index getters report
/// position (Sina's TabSwitchController trips exactly these).
#[test]
fn table_dom_api_family() {
    let mut rt = setup_runtime(r#"<body><table id="t"></table></body>"#);
    let result = rt
        .evaluate(
            r#"
            const t = document.getElementById('t');
            const head = t.createTHead();
            const hr = head.insertRow(); hr.insertCell().textContent = 'h';
            const r0 = t.insertRow(); r0.insertCell().textContent = 'a';
            const r1 = t.insertRow(); r1.insertCell().textContent = 'b';
            const fr = t.createTFoot().insertRow(); fr.insertCell().textContent = 'f';
            const rows = Array.from(t.rows).map((r) => r.cells[0].textContent);
            const rowIndexes = Array.from(t.rows).map((r) => r.rowIndex);
            const sectionRowIndexes = [r0.sectionRowIndex, r1.sectionRowIndex];
            const cellIndex = r1.cells[0].cellIndex;
            const tbodies = t.tBodies.length;
            const caption = t.createCaption().localName;
            const mid = t.insertRow(0);
            let threw = null;
            try { t.insertRow(99); } catch (err) { threw = err.name; }
            t.deleteRow(-1);
            const blank = document.createElement('table');
            const auto = blank.insertRow();
            return {
                rows, rowIndexes, sectionRowIndexes, cellIndex, tbodies, caption,
                midInSection: mid.parentNode.localName,
                midCells: mid.cells.length,
                threw,
                afterDelete: t.rows.length,
                autoBody: [blank.tBodies.length, auto.parentNode.localName],
            };
        "#,
        )
        .unwrap();
    let v = result;
    assert_eq!(
        v["rows"],
        serde_json::json!(["h", "a", "b", "f"]),
        "spec order: thead rows, then tbodies, then tfoot — not tree order"
    );
    assert_eq!(v["rowIndexes"], serde_json::json!([0, 1, 2, 3]));
    assert_eq!(v["sectionRowIndexes"], serde_json::json!([0, 1]));
    assert_eq!(v["cellIndex"], serde_json::json!(0));
    assert_eq!(v["tbodies"], serde_json::json!(1));
    assert_eq!(v["caption"], serde_json::json!("caption"));
    assert_eq!(
        v["midInSection"],
        serde_json::json!("thead"),
        "insertRow(0) targets the section OWNING rows[0] (thead's row), never the table directly"
    );
    assert_eq!(
        v["midCells"],
        serde_json::json!(0),
        "insertRow creates bare rows"
    );
    assert_eq!(
        v["threw"],
        serde_json::json!("IndexSizeError"),
        "out-of-range index throws"
    );
    assert_eq!(
        v["afterDelete"],
        serde_json::json!(4),
        "deleteRow(-1) drops the table-order last row (tfoot's), the bare thead row stays"
    );
    assert_eq!(
        v["autoBody"],
        serde_json::json!([1, "tbody"]),
        "empty-table insertRow synthesizes a tbody"
    );
}

/// Regression for #222: HTML interface globals must discriminate by tag.
/// The old `= Element` aliases made `head instanceof HTMLIFrameElement`
/// true for every element, so webpack style-loader handed the head
/// element to `contentDocument.head` and bilibili's player core threw
/// "Couldn't find a style target" before the player could mount.
#[test]
fn html_interface_instanceof_discriminates_by_tag() {
    let mut rt = setup_runtime(
        r#"<head id="h"></head><body><div id="d"></div><iframe id="i"></iframe><h2 id="t"></h2><form id="f"></form></body>"#,
    );
    let checks: &[(&str, bool)] = &[
        (
            "document.getElementById('d') instanceof HTMLDivElement",
            true,
        ),
        ("document.getElementById('d') instanceof HTMLElement", true),
        (
            "document.getElementById('d') instanceof HTMLIFrameElement",
            false,
        ),
        (
            "document.getElementById('i') instanceof HTMLIFrameElement",
            true,
        ),
        (
            "document.getElementById('i') instanceof HTMLDivElement",
            false,
        ),
        (
            "document.getElementById('h') instanceof HTMLIFrameElement",
            false,
        ),
        (
            "document.getElementById('h') instanceof HTMLHeadElement",
            true,
        ),
        (
            "document.getElementById('t') instanceof HTMLHeadingElement",
            true,
        ),
        (
            "document.getElementById('t') instanceof HTMLDivElement",
            false,
        ),
        (
            "document.getElementById('f') instanceof HTMLFormElement",
            true,
        ),
        (
            "document.getElementById('f') instanceof HTMLDivElement",
            false,
        ),
        ("'x' instanceof HTMLIFrameElement", false),
        // #27: each interface owns its prototype now (they used to share
        // Element.prototype, which made constructor.name read "Element"
        // for every tag and let per-interface patches leak everywhere).
        ("HTMLDivElement.prototype === Element.prototype", false),
        (
            "document.getElementById('d').constructor === HTMLDivElement",
            true,
        ),
    ];
    for (expr, expected) in checks {
        let got = rt.evaluate(expr).unwrap();
        assert_eq!(got, serde_json::json!(expected), "expr: {expr}");
    }
    let ctor = rt
            .evaluate("(() => { try { new HTMLDivElement(); } catch (e) { return e.name; } return 'no-throw'; })()")
            .unwrap();
    assert_eq!(ctor, serde_json::json!("TypeError"));
}

/// Regression for #105: `Element.prepend` must actually insert at the
/// start, not silently no-op.
#[test]
fn element_prepend_inserts_at_start() {
    let mut rt = setup_runtime(r#"<div id="c"><span>existing</span></div>"#);
    rt.evaluate(
        r#"
            const c = document.getElementById('c');
            const n = document.createElement('span');
            n.id = 'first';
            c.prepend(n);
            "#,
    )
    .unwrap();
    let first_id = rt
        .evaluate("document.getElementById('c').firstChild.id")
        .unwrap();
    assert_eq!(first_id, serde_json::json!("first"));
    let count = rt
        .evaluate("document.getElementById('c').childNodes.length")
        .unwrap();
    assert_eq!(count.as_f64().unwrap() as i64, 2);
}

/// Regression for #105: `isEqualNode` compares structure, not identity.
/// Framework diff algorithms rely on this.
#[test]
fn is_equal_node_does_structural_compare() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
            .evaluate(
                r#"
                const a = document.createElement('div'); a.setAttribute('class', 'x'); a.innerHTML = '<span>hi</span>';
                const b = document.createElement('div'); b.setAttribute('class', 'x'); b.innerHTML = '<span>hi</span>';
                const c = document.createElement('div'); c.innerHTML = '<span>bye</span>';
                return [a.isEqualNode(b), a.isEqualNode(c), a.isSameNode(b)];
                "#,
            )
            .unwrap();
    assert_eq!(result, serde_json::json!([true, false, false]));
}

/// Regression for the long-standing insert_before arg-order bug noted
/// in CLAUDE.md: bootstrap.js was passing (parent, new, ref) but `_dom`
/// forwards only two args, silently dropping `ref`. With the fix,
/// `insertBefore` actually inserts.
#[test]
fn insert_before_inserts_node_at_correct_position() {
    let mut rt = setup_runtime(r#"<div id="p"><span id="b">b</span><span id="c">c</span></div>"#);
    let order = rt
        .evaluate(
            r#"
                const p = document.getElementById('p');
                const a = document.createElement('span');
                a.id = 'a';
                p.insertBefore(a, document.getElementById('b'));
                return Array.from(p.children).map(e => e.id).join(',');
                "#,
        )
        .unwrap();
    assert_eq!(order, serde_json::json!("a,b,c"));
}

/// #257: `document.cloneNode(true)` returns a NEW working document —
/// Readability-style extractors start from exactly this call, and the
/// generic Node path answered null for nodeType 9. Shallow clones come
/// back as an empty shell (no element children), like Chrome.
#[tokio::test(flavor = "current_thread")]
async fn document_clone_node_deep_returns_working_document() {
    let mut rt = setup_runtime(
        "<html><head><title>src</title></head><body><p class=\"x\">x1</p><p>y</p></body></html>",
    );
    let result = rt
        .call_function_on_remote(
            r#"async () => {
                    const deep = document.cloneNode(true);
                    const shallow = document.cloneNode(false);
                    const p = deep.querySelector('p.x');
                    return {
                        deepOk: String(deep != null),
                        deepBody: String(deep.body != null),
                        deepP: p ? p.textContent : 'none',
                        deepPCount: String(deep.querySelectorAll('p').length),
                        deepTitle: deep.title,
                        deepDetached: String(deep.defaultView === null),
                        shallowOk: String(shallow != null),
                        shallowEmpty: String(shallow.documentElement == null
                            || shallow.documentElement.children.length === 0),
                    };
                }"#,
            None,
            &[],
            true,
            true,
        )
        .await
        .unwrap();

    let v = result.value.unwrap();
    assert_eq!(v["deepOk"], serde_json::json!("true"));
    assert_eq!(v["deepBody"], serde_json::json!("true"));
    assert_eq!(v["deepP"], serde_json::json!("x1"));
    assert_eq!(v["deepPCount"], serde_json::json!("2"));
    assert_eq!(v["deepTitle"], serde_json::json!("src"));
    assert_eq!(v["deepDetached"], serde_json::json!("true"));
    assert_eq!(v["shallowOk"], serde_json::json!("true"));
    assert_eq!(v["shallowEmpty"], serde_json::json!("true"));
}

#[test]
fn test_node_iterator_returns_root_and_has_detach() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // createNodeIterator was an alias of createTreeWalker, so the first
    // nextNode() silently skipped the root and detach was missing (#467).
    let result = rt.evaluate(r#"
            const root = document.createElement('div');
            root.innerHTML = '<a></a>';
            const it = document.createNodeIterator(root, NodeFilter.SHOW_ELEMENT);
            const tags = [];
            let n;
            while ((n = it.nextNode())) tags.push(n.tagName);
            return [tags, typeof it.detach, it.root === root, it.referenceNode.tagName, it.pointerBeforeReferenceNode];
        "#).unwrap();
    assert_eq!(
        result,
        serde_json::json!([["DIV", "A"], "function", true, "A", false])
    );
}

#[test]
fn test_treewalker_next_document_order_and_reject_prunes() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // Document-order forward walk (#432); FILTER_REJECT prunes the whole
    // subtree (#461) rather than just skipping the node.
    let result = rt.evaluate(r#"
            const root = document.createElement('div');
            root.innerHTML = '<a><b></b></a><c></c>';
            const w = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT, {
                acceptNode(n) { return n.tagName === 'A' ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT; }
            });
            const tags = [];
            let n;
            while ((n = w.nextNode())) tags.push(n.tagName);
            return tags;
        "#).unwrap();
    // A is rejected, so its child B is pruned too; C still follows. Root
    // (DIV) is never returned by a TreeWalker's nextNode.
    assert_eq!(result, serde_json::json!(["C"]));
}

#[test]
fn test_treewalker_skip_descends_into_children() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // FILTER_SKIP must still expose a skipped node's children (#469); the
    // old firstChild stepped straight to the next sibling and returned null.
    let result = rt.evaluate(r#"
            const root = document.createElement('div');
            root.innerHTML = '<section><a></a></section>';
            const w = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT, {
                acceptNode(n) { return n.tagName === 'SECTION' ? NodeFilter.FILTER_SKIP : NodeFilter.FILTER_ACCEPT; }
            });
            const first = w.firstChild();
            return first ? first.tagName : null;
        "#).unwrap();
    assert_eq!(result, serde_json::json!("A"));
}

#[test]
fn test_treewalker_previousnode_reverse_order() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // previousNode walked reverse document order and died mid-tree when a
    // candidate was filtered (#462). Walk forward to the end, then back.
    let result = rt.evaluate(r#"
            const root = document.createElement('div');
            root.innerHTML = '<a><b></b></a><c></c>';
            const w = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT, {
                acceptNode(n) { return n.tagName === 'B' ? NodeFilter.FILTER_SKIP : NodeFilter.FILTER_ACCEPT; }
            });
            while (w.nextNode()) {}
            const tags = [];
            let n;
            while ((n = w.previousNode())) tags.push(n.tagName);
            return tags;
        "#).unwrap();
    // Reverse document order with B skipped: the walk from C finds A, then climbs to the parent - which is the
    // (accepted) root, and DOM 6.1 / Chrome 145 (treewalker_matches_chrome_truth, prev) return it. The old
    // expectation `["A"]` assumed a backward traversal never returns root; real Chrome does.
    assert_eq!(result, serde_json::json!(["A", "DIV"]));
}

#[test]
fn test_treewalker_parentnode_stays_within_root() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // parentNode returned a node OUTSIDE the subtree when currentNode was
    // root, and null instead of root when an accepted ancestor was root
    // itself (#475).
    let result = rt.evaluate(r#"
            const root = document.createElement('div');
            root.innerHTML = '<a><b></b></a>';
            // Skip A, so parentNode must climb past it to the accepted ancestor root.
            const w = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT, {
                acceptNode(n) { return n.tagName === 'A' ? NodeFilter.FILTER_SKIP : NodeFilter.FILTER_ACCEPT; }
            });
            const b = root.querySelector('b');
            w.currentNode = b;
            const parent = w.parentNode();
            // At root, parentNode must not surface <body> above it.
            w.currentNode = root;
            const above = w.parentNode();
            return [parent === root, above];
        "#).unwrap();
    assert_eq!(result, serde_json::json!([true, null]));
}

#[test]
fn test_insert_before_flattens_document_fragment_in_order() {
    let mut rt = setup_runtime(r#"<main id="host"><article id="last"></article></main>"#);
    let result = rt
        .evaluate(
            r#"
            const host = document.getElementById('host');
            const last = document.getElementById('last');
            const fragment = document.createDocumentFragment();
            const first = document.createElement('article');
            const second = document.createElement('article');
            first.id = 'first';
            second.id = 'second';
            fragment.appendChild(first);
            fragment.appendChild(second);

            const returned = host.insertBefore(fragment, last);
            return [
                returned === fragment,
                Array.from(host.children).map(node => node.id),
                fragment.childNodes.length,
                first.parentElement === host,
                second.parentElement === host,
            ];
        "#,
        )
        .unwrap();
    assert_eq!(
        result,
        serde_json::json!([true, ["first", "second", "last"], 0, true, true])
    );
}

#[test]
fn test_replace_child_flattens_document_fragment_and_removes_old_child() {
    let mut rt = setup_runtime(
        r#"<main id="host"><article id="old"></article><article id="tail"></article></main>"#,
    );
    let result = rt
        .evaluate(
            r#"
            const host = document.getElementById('host');
            const old = document.getElementById('old');
            const fragment = document.createDocumentFragment();
            const first = document.createElement('article');
            const second = document.createElement('article');
            first.id = 'first';
            second.id = 'second';
            fragment.appendChild(first);
            fragment.appendChild(second);

            const returned = host.replaceChild(fragment, old);
            return [
                returned === old,
                Array.from(host.children).map(node => node.id),
                fragment.childNodes.length,
                old.parentNode === null,
            ];
        "#,
        )
        .unwrap();
    assert_eq!(
        result,
        serde_json::json!([true, ["first", "second", "tail"], 0, true])
    );
}

#[test]
fn test_child_nodes_is_a_real_nodelist() {
    // childNodes returned a plain Array (Array.isArray true, toString
    // "[object Array]") — an instant fingerprinting tell. A real browser
    // reports "[object NodeList]" and Array.isArray false.
    let mut rt = setup_runtime(r#"<div id="host"><p>A</p><p>B</p></div>"#);
    let result = rt
        .evaluate(
            r#"
            const list = document.getElementById('host').childNodes;
            const seen = [];
            list.forEach((n, i) => seen.push([i, n.tagName]));
            return [
                Array.isArray(list),
                Object.prototype.toString.call(list),
                list instanceof NodeList,
                list.length,
                list.item(0).tagName,
                list.item(7),
                [...list].map(n => n.tagName),
                Array.from(list.keys()),
                seen,
            ];
        "#,
        )
        .unwrap();
    assert_eq!(
        result,
        serde_json::json!([
            false,
            "[object NodeList]",
            true,
            2,
            "P",
            null,
            ["P", "P"],
            [0, 1],
            [[0, "P"], [1, "P"]]
        ])
    );
}

#[test]
fn test_adopt_node_and_toggle_attribute() {
    // Lit/Stencil and several ad SDKs call both; the missing methods threw.
    let mut rt = setup_runtime(r#"<div id="host"></div>"#);
    let result = rt
        .evaluate(
            r#"
            const host = document.getElementById('host');
            const child = document.createElement('span');
            host.appendChild(child);
            const adopted = document.adoptNode(child);
            const toggles = [
                host.toggleAttribute('hidden'),
                host.toggleAttribute('hidden'),
                host.toggleAttribute('data-x', true),
                host.toggleAttribute('data-x', true),
                host.toggleAttribute('data-x', false),
                host.toggleAttribute('data-x', false),
            ];
            return [
                adopted === child,
                toggles,
                host.hasAttribute('hidden'),
                host.hasAttribute('data-x'),
            ];
        "#,
        )
        .unwrap();
    assert_eq!(
        result,
        serde_json::json!([true, [true, false, true, true, false, false], false, false])
    );
}

#[test]
fn test_clone_node_shallow_preserves_attributes_and_isolation() {
    let mut rt = setup_runtime(
        r#"<section id="src" class="source" data-token="original"><span>child</span></section>"#,
    );
    let result = rt
        .evaluate(
            r#"
            const source = document.getElementById('src');
            const clone = source.cloneNode(false);
            clone.className = 'clone';
            source.setAttribute('data-token', 'changed');
            return [
                clone instanceof Element,
                clone.tagName,
                clone.id,
                clone.className,
                clone.getAttribute('data-token'),
                clone.childNodes.length,
                clone.parentNode === null,
                clone !== source,
                source.className,
                source.getAttribute('data-token'),
                source.childNodes.length,
            ];
        "#,
        )
        .unwrap();
    assert_eq!(
        result,
        serde_json::json!([
            true, "SECTION", "src", "clone", "original", 0, true, true, "source", "changed", 1
        ])
    );
}

#[test]
fn test_clone_node_deep_keeps_table_children_and_template_contents() {
    // The old innerHTML round-trip parsed through a <div> context, which
    // discards <tr>/<td>/<option> as invalid children. Structural cloning
    // has no parsing context, so they survive; <template> contents hang
    // off a separate fragment and need their own remapped clone.
    let mut rt = setup_runtime(
        r#"<table id="tbl"><tr><td>c1</td></tr></table><template id="tpl"><p>in-template</p></template>"#,
    );
    let result = rt
        .evaluate(
            r#"
            const tblClone = document.getElementById('tbl').cloneNode(true);
            const tplClone = document.getElementById('tpl').cloneNode(true);
            return [
                tblClone.querySelectorAll('td').length,
                tblClone.querySelector('td').textContent,
                tplClone.content.childNodes.length,
                tplClone.content.querySelector('p').textContent,
                tplClone.content !== document.getElementById('tpl').content,
            ];
        "#,
        )
        .unwrap();
    assert_eq!(result, serde_json::json!([1, "c1", 1, "in-template", true]));
}

#[test]
fn test_clone_node_deep_subtree_does_not_overflow() {
    // Structural cloning uses an explicit stack in Rust, so a pathological
    // nesting depth cannot overflow the JS stack.
    let mut rt = setup_runtime(r#"<div id="host"></div>"#);
    let result = rt
        .evaluate(
            r#"
            let node = document.getElementById('host');
            for (let i = 0; i < 2000; i++) {
                const child = document.createElement('div');
                node.appendChild(child);
                node = child;
            }
            const clone = document.getElementById('host').cloneNode(true);
            let depth = 0, cur = clone;
            while (cur.firstChild) { depth++; cur = cur.firstChild; }
            return [depth];
        "#,
        )
        .unwrap();
    assert_eq!(result, serde_json::json!([2000]));
}

#[test]
fn test_matches_and_closest() {
    let mut rt = setup_runtime(
        r#"<div class="outer"><div class="inner"><span id="target">Hi</span></div></div>"#,
    );
    let matches = rt
        .evaluate("document.getElementById('target').matches('span')")
        .unwrap();
    assert_eq!(matches, serde_json::json!(true));
    let closest = rt
        .evaluate("document.getElementById('target').closest('.outer').className")
        .unwrap();
    assert_eq!(closest, serde_json::json!("outer"));
    let no_match = rt
        .evaluate("document.getElementById('target').closest('.nonexistent')")
        .unwrap();
    assert_eq!(no_match, serde_json::Value::Null);
}

#[test]
fn test_clone_node_deep() {
    let mut rt = setup_runtime(r#"<div id="src"><p>A</p><p>B</p></div>"#);
    rt.execute_script(
        "test",
        r#"
            var src = document.getElementById('src');
            var clone = src.cloneNode(true);
            document.body.appendChild(clone);
        "#,
    )
    .unwrap();
    let count = rt
        .evaluate("document.querySelectorAll('p').length")
        .unwrap();
    assert!(
        count.as_f64().unwrap() as i64 >= 4,
        "Deep clone should duplicate <p> children, got: {}",
        count
    );
}

// WebIDL (Node or string) coercion: replaceWith(undefined) inserts the
// text node "undefined" — jQuery 1.12's domManip routes raw fragments
// through here, and a silent no-op desyncs the DOM the page just asked
// to update (obscura#888 family). Detached no-parent calls stay no-ops.
#[test]
fn replace_with_coerces_non_node_arguments_to_text() {
    let mut rt = setup_runtime("<html><body><div id=p><b id=c>old</b></div></body></html>");
    let result = rt
        .evaluate(
            r#"(() => {
                    document.getElementById('c').replaceWith(undefined);
                    const coerced = document.getElementById('p').textContent;
                    const d = document.createElement('div');
                    let detachedNoop = false;
                    try { d.replaceWith(); d.replaceWith(undefined); detachedNoop = true; } catch {}
                    return [coerced, detachedNoop];
                })()"#,
        )
        .unwrap();
    assert_eq!(result, serde_json::json!(["undefined", true]));
}

/// Playwright's `getByRole` / `getByLabel` locators resolve via ARIA
/// reflection properties. Without the getters those locators always
/// fail. Reflect the underlying aria-* attributes.
#[test]
fn element_aria_reflection_properties_read_aria_attrs() {
    let mut rt = setup_runtime(
        r#"<button id="b" role="tab" aria-label="Settings" aria-selected="true">x</button>"#,
    );
    let result = rt
        .evaluate(
            r#"
                const el = document.getElementById('b');
                return [el.role, el.ariaLabel, el.ariaSelected];
                "#,
        )
        .unwrap();
    assert_eq!(result, serde_json::json!(["tab", "Settings", "true"]));
}

/// Setting an ARIA reflection property must write through to the
/// underlying attribute so frameworks that toggle state via
/// `el.ariaExpanded = 'true'` actually update the DOM.
#[test]
fn element_aria_reflection_setters_write_through() {
    let mut rt = setup_runtime(r#"<div id="d"></div>"#);
    let result = rt
        .evaluate(
            r#"
                const el = document.getElementById('d');
                el.role = 'menu';
                el.ariaExpanded = 'true';
                return [el.getAttribute('role'), el.getAttribute('aria-expanded')];
                "#,
        )
        .unwrap();
    assert_eq!(result, serde_json::json!(["menu", "true"]));
}

/// Upstream ec05ed0: dataset is backed by a real DOMStringMap instance
/// while data-* reflection stays dynamic.
#[test]
fn dom_string_map_is_exposed_and_backs_dataset() {
    let mut rt = setup_runtime(r#"<html><body><div id="x" data-foo="bar"></div></body></html>"#);
    let v = rt
        .evaluate(
            r#"(() => {
                    const el = document.getElementById("x");
                    const ds = el.dataset;
                    const iface = window.DOMStringMap;
                    const d = Object.getOwnPropertyDescriptor(window, "DOMStringMap");
                    let illegal = false;
                    try { new iface(); } catch (e) { illegal = e instanceof TypeError; }
                    ds.newKey = "1";
                    const reflected = el.getAttribute("data-new-key");
                    delete ds.foo;
                    return [typeof iface, ds instanceof iface,
                            Object.getPrototypeOf(ds) === iface.prototype,
                            ds.constructor === iface,
                            Object.prototype.toString.call(ds),
                            d ? d.enumerable : "missing", illegal, reflected,
                            el.hasAttribute("data-foo"), ds === el.dataset].join("|");
                })()"#,
        )
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!("function|true|true|true|[object DOMStringMap]|false|true|1|false|true")
    );
}

#[test]
fn test_dataset_in_and_object_keys() {
    // `'foo' in el.dataset` and Object.keys(el.dataset) must reflect data-*
    // attributes (CSSOM/DOMStringMap parity).
    let mut rt = setup_runtime(r#"<div id="el" data-foo-bar="1" data-baz="2"></div>"#);
    let result = rt
        .evaluate(
            r#"
            const d = document.getElementById('el').dataset;
            return [
                'fooBar' in d,
                'baz' in d,
                'missing' in d,
                Object.keys(d).sort(),
                d.fooBar,
                d.baz,
            ];
        "#,
        )
        .unwrap();
    assert_eq!(
        result,
        serde_json::json!([true, true, false, ["baz", "fooBar"], "1", "2"])
    );
}

#[test]
fn test_cloned_script_does_not_rerun() {
    // cloneNode of a subtree whose script already ran must not run the
    // clone's script (started state propagates to the clone).
    let mut rt = setup_runtime(r#"<div id="host"></div>"#);
    let result = rt
        .evaluate(
            r#"
            const host = document.getElementById('host');
            window.__count = 0;
            const box = document.createElement('div');
            const s = document.createElement('script');
            s.textContent = 'window.__count = (window.__count || 0) + 1;';
            box.appendChild(s);
            host.appendChild(box);
            const afterFirst = window.__count;
            const clone = box.cloneNode(true);
            host.appendChild(clone);
            return [afterFirst, window.__count];
        "#,
        )
        .unwrap();
    assert_eq!(result, serde_json::json!([1, 1]));
}

#[tokio::test(flavor = "current_thread")]
async fn test_html_unknown_element_is_real_interface() {
    // #203: HTMLUnknownElement used to be a bare alias of Element, and the
    // toStringTag brand sweep then stamped Element.prototype with THAT
    // name — every element answered '[object HTMLUnknownElement]'. It is
    // its own interface below HTMLElement with an Illegal constructor.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"() => {
            let threw = '';
            try { new HTMLUnknownElement(); } catch (e) { threw = e.name; }
            return [
                typeof HTMLUnknownElement,
                HTMLUnknownElement.prototype instanceof HTMLElement,
                document.createElement('div') instanceof HTMLUnknownElement,
                threw,
            ];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!(["function", true, false, "TypeError"])
    );
}

/// M4 knife 38: TreeWalker / NodeIterator (treewalker-probe.js, real Chrome 145): real interface instances (were object
/// literals with thirteen own keys), illegal constructors, prototype accessors (currentNode read-write), argument and filter
/// validation wording, the filter verdict rule (unsigned short: 1 accept, 2 reject, everything else skip), traversal in every
/// direction incl. FILTER_REJECT pruning and previousNode returning an accepted root, NodeIterator pointer semantics, and
/// NodeFilter as a prototype-less constants function.
#[tokio::test(flavor = "current_thread")]
async fn treewalker_matches_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/treewalker-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/treewalker-truth-145.json"
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

/// M4 knife 43: Attr is a Node (attr-probe.js, real Chrome 145): `attr instanceof Node`, Node members answer null / empty for an
/// attribute, nodeType/nodeName/nodeValue come from Node.prototype, the public constructor is illegal (nodes come from
/// createAttribute / getAttributeNode), no own keys on instances.
/// KNOWN: the lifted fields are rw accessors where Chrome's are read-only. (Attr identity and NamedNodeMap landed in knife 44:
/// see namednodemap_matches_chrome_truth.)
#[tokio::test(flavor = "current_thread")]
async fn attr_matches_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/attr-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/attr-truth-145.json"
    ))
    .unwrap();
    let mut bad = Vec::new();
    for (k, want) in truth.as_object().unwrap() {
        if k == "proto.kinds" {
            // only the six lifted ro->rw members are known; everything else must match
            let mut e = engine[k].clone();
            let mut w = want.clone();
            // the lifted ro->rw fields plus the safe Node-member overrides Attr.prototype carries (Chrome has them on Node.prototype)
            for m in [
                "name",
                "localName",
                "namespaceURI",
                "prefix",
                "ownerElement",
                "specified",
                "childNodes",
                "firstChild",
                "hasChildNodes",
                "isConnected",
                "lastChild",
                "nextSibling",
                "ownerDocument",
                "parentElement",
                "parentNode",
                "previousSibling",
                "textContent",
            ] {
                e.as_object_mut().unwrap().remove(m);
                w.as_object_mut().unwrap().remove(m);
            }
            if e != w {
                bad.push(format!("{k}:\n   engine {e}\n   chrome {w}"));
            }
            continue;
        }
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

/// M4 knife 44: NamedNodeMap + live Attr nodes (nnm-probe.js, 37 keys, real Chrome 145): one NamedNodeMap per element (indexed
/// and named properties, live length, iterable), one Attr per attribute that keeps its identity while the attribute exists,
/// follows the value both ways, is detached (ownerElement null, last value kept) on removal and replaced by a NEW node on
/// re-add; set/remove/getNamedItem(NS) and the Element attribute-node methods with exact error names and wording.
#[tokio::test(flavor = "current_thread")]
async fn namednodemap_matches_chrome_truth() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let probe = include_str!("../../../../../tools/webplatform/chrome/nnm-probe.js");
    let result = rt
        .call_function_on_remote(&format!("() => ({})", probe), None, &[], true, true)
        .await
        .unwrap();
    let engine: serde_json::Value =
        serde_json::from_str(result.value.unwrap().as_str().unwrap()).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tools/webplatform/chrome/nnm-truth-145.json"
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
