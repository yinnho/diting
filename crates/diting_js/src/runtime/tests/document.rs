//! document 面：write 流解析/innerHTML/DOMParser/XMLSerializer/元数据
use super::common::setup_runtime;

#[test]
fn test_document_title() {
    let mut rt = setup_runtime("<html><head><title>Test</title></head><body></body></html>");
    let title = rt.evaluate("document.title").unwrap();
    assert_eq!(title, serde_json::json!("Test Page"));
}

/// #177: `link.rev` and `meta.scheme` — the obsolete-but-reflected
/// DOMString twins (HTML spec keeps them as [Reflect] on the per-tag
/// interfaces). Legacy link-directory readers walk `link.rev`; the
/// pre-fix answer (`undefined`) breaks them; Chrome answers `""` when
/// the attribute is absent and round-trips get/set through the content
/// attribute.
#[test]
fn link_rev_and_meta_scheme_reflect() {
    let mut rt = setup_runtime("<html><body><div id='t'>x</div></body></html>");
    let out = rt
        .evaluate(
            r#"
            var link = document.createElement('link');
            link.setAttribute('rev', 'made');
            var revViaGetter = link.rev;
            link.rev = 'friend';
            var revViaSetter = link.getAttribute('rev');
            var meta = document.createElement('meta');
            meta.setAttribute('scheme', 'ISBN');
            var schemeViaGetter = meta.scheme;
            meta.scheme = 'DOI';
            var schemeViaSetter = meta.getAttribute('scheme');
            return JSON.stringify({
                faces: [
                    Object.keys(HTMLLinkElement.prototype).indexOf('rev') >= 0,
                    Object.keys(HTMLMetaElement.prototype).indexOf('scheme') >= 0,
                ],
                rev: [revViaGetter, revViaSetter],
                scheme: [schemeViaGetter, schemeViaSetter],
                blank: [document.createElement('link').rev,
                        document.createElement('meta').scheme],
                divRev: document.createElement('div').rev,
            });
        "#,
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(out.as_str().unwrap()).unwrap();
    assert_eq!(
        v["faces"],
        serde_json::json!([true, true]),
        "rev lives on HTMLLinkElement.prototype, scheme on HTMLMetaElement.prototype"
    );
    assert_eq!(
        v["rev"],
        serde_json::json!(["made", "friend"]),
        "link.rev get/set round-trips through the content attribute"
    );
    assert_eq!(
        v["scheme"],
        serde_json::json!(["ISBN", "DOI"]),
        "meta.scheme get/set round-trips through the content attribute"
    );
    assert_eq!(
        v["blank"],
        serde_json::json!(["", ""]),
        "absent attribute reflects the empty string, not undefined"
    );
    assert_eq!(
        v["divRev"],
        serde_json::Value::Null,
        "rev stays off the generic element face"
    );
}

#[test]
fn test_document_url() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let url = rt.evaluate("document.URL").unwrap();
    assert_eq!(url, serde_json::json!("http://example.com/test"));
}

#[test]
fn test_inner_html() {
    let mut rt = setup_runtime(r#"<div id="x"><p>Hello</p></div>"#);
    let html = rt
        .evaluate("document.getElementById('x').innerHTML")
        .unwrap();
    assert!(html.as_str().unwrap().contains("<p>"));
}

/// Regression for #105: `document.forms` / `images` / `links` must be
/// live, not hardcoded `[]`. jQuery 1.x's submit-event setup iterates
/// `document.forms` and crashes when it's empty for pages that have forms.
#[test]
fn document_forms_images_links_are_live() {
    let mut rt = setup_runtime(r#"<form></form><form></form><img><a href="x">l</a><a>no-href</a>"#);
    assert_eq!(
        rt.evaluate("document.forms.length")
            .unwrap()
            .as_f64()
            .unwrap() as i64,
        2
    );
    assert_eq!(
        rt.evaluate("document.images.length")
            .unwrap()
            .as_f64()
            .unwrap() as i64,
        1
    );
    assert_eq!(
        rt.evaluate("document.links.length")
            .unwrap()
            .as_f64()
            .unwrap() as i64,
        1
    );
}

#[test]
fn test_document_spec_collections_forms_images_links_scripts() {
    // glama.ai admin hydration died on `document.forms.namedItem is not a
    // function` — the Document class had no collection getters at all.
    // forms/images/links/scripts are the spec set frameworks probe first;
    // namedItem and Proxy named access come from HTMLCollection itself.
    let mut rt = setup_runtime(
        r#"<form id=login name=primary></form>
            <form name=secondary></form>
            <img id=picture>
            <a id=withhref href="/x"></a><a id=nohref></a>
            <script id=inline>void 0;</script>"#,
    );
    let result = rt
        .evaluate(
            r#"
            return [
                document.forms instanceof HTMLCollection,
                document.forms.length,
                document.forms.namedItem('login').name,
                document.forms.namedItem('primary').id,
                document.forms.primary === document.forms.namedItem('primary'),
                document.forms.item(1).name,
                document.images.length, document.images.namedItem('picture').id,
                // links counts only a[href]/area[href]: the bare <a> is out
                document.links.length, document.links[0].id,
                document.scripts.length,
            ];
        "#,
        )
        .unwrap();
    assert_eq!(
        result,
        serde_json::json!([
            true,
            2,
            "primary",
            "login",
            true,
            "secondary",
            1,
            "picture",
            1,
            "withhref",
            1,
        ])
    );
}

/// Document's ParentNode face (#250): children/firstElementChild/
/// lastElementChild/childElementCount mirror Chrome — the collection
/// holds exactly the documentElement. tbtracker on myseller boots with
/// `var e = document.firstElementChild` and attaches its listeners from
/// DOMContentLoaded; the old undefined crashed `e.addEventListener`.
#[tokio::test(flavor = "current_thread")]
async fn document_parent_node_face() {
    let mut rt = setup_runtime("<html><head></head><body><p>hi</p></body></html>");
    let result = rt
        .evaluate_remote(
            r#"(function() {
                    const c = document.children;
                    const de = document.documentElement;
                    return {
                        len: c.length,
                        isHtml: c[0] === de && de.localName === 'html',
                        first: document.firstElementChild === de,
                        last: document.lastElementChild === de,
                        count: document.childElementCount,
                        tag: String(document.firstElementChild),
                    };
                })()"#,
            true,
            false,
        )
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!({
            "len": 1,
            "isHtml": true,
            "first": true,
            "last": true,
            "count": 1,
            "tag": "[object HTMLHtmlElement]",
        })
    );
}

#[test]
fn test_inner_html_setter() {
    let mut rt = setup_runtime(r#"<div id="target"><p>Old</p></div>"#);
    rt.execute_script(
        "test",
        r#"
            var el = document.getElementById('target');
            el.innerHTML = '<strong>Bold</strong><em>Italic</em>';
        "#,
    )
    .unwrap();
    let result = rt
        .evaluate("document.getElementById('target').innerHTML")
        .unwrap();
    let html = result.as_str().unwrap();
    assert!(
        html.contains("<strong>"),
        "innerHTML should contain <strong>, got: {}",
        html
    );
    assert!(
        html.contains("<em>"),
        "innerHTML should contain <em>, got: {}",
        html
    );
    assert!(
        !html.contains("Old"),
        "innerHTML should not contain old content, got: {}",
        html
    );
}

#[test]
fn test_inner_html_with_nested() {
    let mut rt = setup_runtime(r#"<div id="root"></div>"#);
    rt.execute_script(
        "test",
        r#"
            var el = document.getElementById('root');
            el.innerHTML = '<ul><li>A</li><li>B</li><li>C</li></ul>';
        "#,
    )
    .unwrap();
    let count = rt
        .evaluate("document.querySelectorAll('li').length")
        .unwrap();
    assert_eq!(
        count.as_f64().unwrap() as i64,
        3,
        "Should find 3 li elements after innerHTML set"
    );

    let text = rt
        .evaluate("document.querySelector('li').textContent")
        .unwrap();
    assert_eq!(text, serde_json::json!("A"));
}

#[test]
fn test_document_write_appends_to_body() {
    let mut rt = setup_runtime("<html><body><p>Existing</p></body></html>");
    rt.evaluate("document.write('<div>Added</div>')").unwrap();
    let html = rt.evaluate("document.body.innerHTML").unwrap();
    let body = html.as_str().unwrap();
    assert!(
        body.contains("Existing"),
        "existing content should remain, got: {}",
        body
    );
    assert!(
        body.contains("Added"),
        "written content should appear, got: {}",
        body
    );
}

#[test]
fn test_document_writeln() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate("document.writeln('Hello')").unwrap();
    let html = rt.evaluate("document.body.innerHTML").unwrap();
    assert!(html.as_str().unwrap().contains("Hello"));
}

#[test]
fn test_document_write_multiple_args() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate("document.write('Hello', ' ', 'World')")
        .unwrap();
    let text = rt.evaluate("document.body.textContent").unwrap();
    assert_eq!(text.as_str().unwrap().trim(), "Hello World");
}

#[test]
fn test_document_open_clears_body() {
    let mut rt = setup_runtime("<html><body><p>Old content</p></body></html>");
    rt.evaluate("document.open()").unwrap();
    let html = rt.evaluate("document.body.innerHTML").unwrap();
    assert_eq!(html.as_str().unwrap(), "");
}

#[test]
fn test_document_write_html_elements() {
    let mut rt = setup_runtime("<html><body></body></html>");
    rt.evaluate(r#"document.write('<h1 id="title">Test</h1><p>Para</p>')"#)
        .unwrap();
    let h1 = rt
        .evaluate("document.querySelector('h1').textContent")
        .unwrap();
    assert_eq!(h1.as_str().unwrap(), "Test");
    let p = rt
        .evaluate("document.querySelector('p').textContent")
        .unwrap();
    assert_eq!(p.as_str().unwrap(), "Para");
}

// One stream per document. The tokenizer carries its state across the calls.
// https://html.spec.whatwg.org/multipage/dynamic-markup-insertion.html#dom-document-write
#[test]
fn document_write_joins_an_element_split_across_calls() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate(
            r#"
                var scriptTestSetup = true;
                document.write('<di');
                document.write('v id="split">');
                document.write('content</div>');
                const el = document.getElementById('split');
                return el ? el.textContent : null;
                "#,
        )
        .unwrap();
    assert_eq!(result, serde_json::json!("content"));
}

#[test]
fn document_write_joins_a_tag_name_split_across_calls() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate(
            r#"
                var scriptTestSetup = true;
                document.write('<spa');
                document.write('n id="half">x</span>');
                const el = document.getElementById('half');
                return el ? el.tagName : null;
                "#,
        )
        .unwrap();
    assert_eq!(result, serde_json::json!("SPAN"));
}

// The shape the UI5 cachebuster writes: "<script", one per attribute, then ">".
#[test]
fn document_write_runs_a_script_split_across_calls() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate(
            r#"
                var scriptTestSetup = true;
                globalThis.__splitScriptRan = false;
                document.write('<scr' + 'ipt');
                document.write(' id="split-script"');
                document.write('>');
                document.write('globalThis.__splitScriptRan = true;');
                document.write('<\/scr' + 'ipt>');
                return [!!document.getElementById('split-script'), globalThis.__splitScriptRan];
                "#,
        )
        .unwrap();
    assert_eq!(result, serde_json::json!([true, true]));
}

// A script in the <head> inserts behind itself, so that what it writes runs before what
// the parser saw after it.
#[test]
fn document_write_inserts_at_the_writing_scripts_position() {
    let mut rt = setup_runtime(
        r#"<html><head><script id="writer"></script></head><body><p id="existing">x</p></body></html>"#,
    );
    let result = rt
        .evaluate(
            r#"
                var scriptTestSetup = true;
                // What the production path sets while a script runs; bootstrap.js
                // assigns __currentScriptNid around every script it prepares.
                globalThis.__currentScriptNid = document.getElementById('writer')._nid;
                document.write('<span id="written"></span>');
                return JSON.stringify({
                  head: Array.from(document.head.children).map(e => e.id || e.tagName),
                  body: Array.from(document.body.children).map(e => e.id || e.tagName),
                });
                "#,
        )
        .unwrap();
    assert_eq!(
        result,
        serde_json::json!(r#"{"head":["writer","written"],"body":["existing"]}"#)
    );
}

// Holding back until the close would lose everything written after it. It belongs inside.
#[test]
fn document_write_shows_an_element_that_is_never_closed() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate(
            r#"
                var scriptTestSetup = true;
                document.write('<div id="unclosed">hello');
                const el = document.getElementById('unclosed');
                return el ? el.textContent : null;
                "#,
        )
        .unwrap();
    assert_eq!(result, serde_json::json!("hello"));
}

#[test]
fn document_write_grows_an_open_element_across_calls() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate(
            r#"
                var scriptTestSetup = true;
                document.write('<div id="wrap">');
                document.write('<span id="inner">y</span>');
                const inner = document.getElementById('inner');
                return JSON.stringify({
                  wrap: !!document.getElementById('wrap'),
                  inner: !!inner,
                  nested: !!(inner && inner.parentElement && inner.parentElement.id === 'wrap'),
                });
                "#,
        )
        .unwrap();
    assert_eq!(
        result,
        serde_json::json!(r#"{"wrap":true,"inner":true,"nested":true}"#)
    );
}

#[test]
fn document_write_keeps_call_order_at_the_insertion_point() {
    let mut rt =
        setup_runtime(r#"<html><head><script id="writer"></script></head><body></body></html>"#);
    let result = rt
        .evaluate(
            r#"
                var scriptTestSetup = true;
                globalThis.__currentScriptNid = document.getElementById('writer')._nid;
                document.write('<span id="one"></span>');
                document.write('<span id="two"></span>');
                return Array.from(document.head.children).map(e => e.id).join(',');
                "#,
        )
        .unwrap();
    assert_eq!(result, serde_json::json!("writer,one,two"));
}

/// Setting innerHTML on the <html> element parses in the "before head"
/// insertion mode, which synthesizes head and body. The importer must keep
/// both; it previously returned the synthesized body and dropped the head
/// (so a <title>/<meta> assigned this way vanished).
#[test]
fn documentelement_inner_html_keeps_head_and_body() {
    let mut rt = setup_runtime("<html><head></head><body></body></html>");
    let v = rt
            .evaluate(
                "(function(){ document.documentElement.innerHTML = '<head><title>T</title></head><body><p>hi</p></body>'; \
                 var t = document.querySelector('title'); var p = document.querySelector('p'); \
                 return (t ? t.textContent : 'no-title') + '|' + (p ? p.textContent : 'no-p'); })()",
            )
            .unwrap();
    assert_eq!(v, serde_json::json!("T|hi"));
}

/// Regression guard: innerHTML on an ordinary element still imports the
/// parsed nodes directly (no head/body is synthesized for a div context),
/// so the fix above must not change the common case.
#[test]
fn ordinary_element_inner_html_imports_content_directly() {
    let mut rt = setup_runtime("<html><body><div id=\"d\"></div></body></html>");
    let v = rt
            .evaluate(
                "(function(){ var d=document.getElementById('d'); d.innerHTML='<span>a</span><span>b</span>'; \
                 return d.children.length + '|' + d.textContent; })()",
            )
            .unwrap();
    assert_eq!(v, serde_json::json!("2|ab"));
}

#[test]
fn insert_adjacent_html_keeps_leading_comments_in_table_contexts() {
    let mut rt = setup_runtime(
        r#"<html><body><table><tbody id="tb"><tr id="row"></tr></tbody></table></body></html>"#,
    );
    let out = rt
            .evaluate(
                "(function(){var tb=document.getElementById('tb');tb.insertAdjacentHTML('beforeend','<!--m--><tr><td>v</td></tr>');var row=document.getElementById('row');row.insertAdjacentHTML('beforeend','<!--n--><td>x</td>');return Array.from(tb.childNodes).map(function(n){return n.nodeName}).join('|')+';'+Array.from(row.childNodes).map(function(n){return n.nodeName}).join('|');})()",
            )
            .unwrap();
    assert_eq!(out, serde_json::json!("TR|#comment|TR;#comment|TD"));
}

#[test]
fn insert_adjacent_html_uses_the_insertion_element_as_context() {
    let mut rt = setup_runtime(
        r#"<html><body><div id="d"></div><table id="table"><tbody id="tb"></tbody></table></body></html>"#,
    );
    let out = rt
            .evaluate(
                "(function(){var d=document.getElementById('d');d.insertAdjacentHTML('beforeend','<tr><td>v</td></tr>');var table=document.getElementById('table');table.insertAdjacentHTML('beforeend','<tr><td>x</td></tr>');var tb=document.getElementById('tb');tb.insertAdjacentHTML('beforeend','<tr><td>y</td></tr>tail');return d.firstChild.nodeName+':'+d.textContent+';'+table.lastElementChild.tagName+';'+Array.from(tb.childNodes).map(function(n){return n.nodeName+(n.data?':'+n.data:'')}).join('|');})()",
            )
            .unwrap();
    assert_eq!(out, serde_json::json!("#text:v;TBODY;TR|#text:tail"));
}

/// tmp.childNodes is a LIVE list: indexing it while moving nodes into the
/// document skips every other node. Regression guard for the firstChild-pop
/// loop in insertAdjacentHTML.
#[test]
fn insert_adjacent_html_moves_all_sibling_nodes() {
    let mut rt = setup_runtime(r#"<html><body><div id="d"></div></body></html>"#);
    let out = rt
            .evaluate(
                "(function(){var d=document.getElementById('d');d.insertAdjacentHTML('beforeend','<span>a</span><span>b</span><span>c</span><span>d</span>');return d.children.length+'|'+d.textContent;})()",
            )
            .unwrap();
    assert_eq!(out, serde_json::json!("4|abcd"));
}

#[test]
fn test_document_doctype() {
    let mut rt = setup_runtime("<!DOCTYPE html><html><body></body></html>");
    let result = rt.evaluate("document.doctype !== null").unwrap();
    assert_eq!(result, serde_json::json!(true));

    let name = rt.evaluate("document.doctype.name").unwrap();
    assert_eq!(name, serde_json::json!("html"));

    let node_type = rt.evaluate("document.doctype.nodeType").unwrap();
    assert_eq!(node_type.as_f64().unwrap() as i64, 10);
}

#[test]
fn test_document_doctype_null_when_missing() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt.evaluate("document.doctype === null").unwrap();
    assert_eq!(result, serde_json::json!(true));
}

#[test]
fn test_xml_serializer_doctype() {
    let mut rt = setup_runtime("<!DOCTYPE html><html><body></body></html>");
    let result = rt
        .evaluate("new XMLSerializer().serializeToString(document.doctype)")
        .unwrap();
    assert_eq!(result.as_str().unwrap(), "<!DOCTYPE html>");
}

#[test]
fn test_xml_serializer_element() {
    let mut rt = setup_runtime(r#"<html><body><div id="x">Hello</div></body></html>"#);
    let result = rt
        .evaluate("new XMLSerializer().serializeToString(document.getElementById('x'))")
        .unwrap();
    let html = result.as_str().unwrap();
    assert!(html.contains("<div"));
    assert!(html.contains("Hello"));
}

#[test]
fn test_insert_adjacent_html_case_insensitive_and_syntax_error() {
    // Position was matched case-sensitively (so 'BeforeEnd' silently
    // no-op'd) and an invalid position didn't throw SyntaxError.
    let mut rt = setup_runtime(r#"<div id="el"><span>child</span></div>"#);
    let result = rt
        .evaluate(
            r#"
            const el = document.getElementById('el');
            el.insertAdjacentHTML('BeforeEnd', '<b>X</b>');
            let threw = null;
            try { el.insertAdjacentHTML('sideways', '<i>Y</i>'); } catch (e) { threw = e.name; }
            return [el.innerHTML, threw];
        "#,
        )
        .unwrap();
    assert_eq!(
        result,
        serde_json::json!(["<span>child</span><b>X</b>", "SyntaxError"])
    );
}

#[test]
fn test_innerhtml_script_is_inert() {
    // Scripts created by innerHTML never execute (per spec), unlike direct
    // DOM insertion.
    let mut rt = setup_runtime(r#"<div id="host"></div>"#);
    let result = rt
        .evaluate(
            r#"
            const host = document.getElementById('host');
            window.__count = 0;
            host.innerHTML = '<script>window.__count = 1;</script>';
            const afterInner = window.__count;
            // A directly-inserted script still runs.
            const s = document.createElement('script');
            s.textContent = 'window.__count = 2;';
            host.appendChild(s);
            return [afterInner, window.__count];
        "#,
        )
        .unwrap();
    assert_eq!(result, serde_json::json!([0, 2]));
}

#[test]
fn test_domparser_xml_parsererror_on_malformed() {
    // Upstream 53295fa+6927f11+869f700+20c4628: XML mime types get a
    // well-formedness pass; malformed input yields a <parsererror>
    // documentElement that querySelector('parsererror') finds, matching
    // Chrome. Self-closing roots count as complete elements. Well-formed
    // input (obscura#883) now builds a real tree: the no-xmlns root keeps
    // its source name read back through the HTML-namespace tagName
    // convention (uppercased), not an <html> collapse.
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt.evaluate(r#"
            const check = (src) => {
                const doc = new DOMParser().parseFromString(src, "application/xml");
                const err = doc.querySelector('parsererror');
                return err ? ('E:' + doc.documentElement.tagName) : ('OK:' + doc.documentElement.tagName);
            };
            return [
                check('<root><a></b></root>'),   // tag mismatch
                check('<root></a></root>'),      // closing tag mismatch
                check('<root/><b/>'),            // extra content after root
                check('<root><a>'),              // unclosed tag
                check('<root><a>1</a></root>'),  // well-formed
                check('<root/>'),                // self-closing root is complete
            ];
        "#).unwrap();
    assert_eq!(
        result,
        serde_json::json!([
            "E:PARSERERROR",
            "E:PARSERERROR",
            "E:PARSERERROR",
            "E:PARSERERROR",
            "OK:root",
            "OK:root",
        ])
    );
}

#[test]
fn test_domparser_xml_strict_fallback_and_html_unaffected() {
    // The hand-rolled state machine catches what the regex pass cannot
    // (here: zero root elements) and swaps in the generic parsererror.
    // HTML mime types never run either check; comments/CDATA/PI/DOCTYPE
    // are skipped by both layers.
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate(
            r#"
            const doc1 = new DOMParser().parseFromString('not xml at all', 'application/xml');
            const textOnly = !!doc1.querySelector('parsererror');
            const doc2 = new DOMParser().parseFromString('<div>hi</div>', 'text/html');
            const htmlOk = !doc2.querySelector('parsererror') && !!doc2.querySelector('div');
            const doc3 = new DOMParser().parseFromString(
                '<?xml version="1.0"?><!-- c --><root><![CDATA[x<y]]></root>', 'application/xml');
            const skipsNoise = !doc3.querySelector('parsererror');
            return [textOnly, htmlOk, skipsNoise];
        "#,
        )
        .unwrap();
    assert_eq!(result, serde_json::json!([true, true, true]));
}

#[test]
fn test_domparser_spec_collections_on_parsed_documents() {
    // XHS 2026-10-12: an SDK read document.scripts on a DOMParser-parsed
    // document and got undefined — the hand-rolled docNode predated the
    // spec collection family. Chrome hands every HTML document the
    // scripts/forms/images/links HTMLCollections (document order across
    // head AND body) and a null currentScript (no script of a detached
    // document executes).
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate(
            r#"
            const doc = new DOMParser().parseFromString(
                '<html><head><script src="a.js"><' + '/script></head>'
                + '<body><form id="f1"></form><img src="i.png">'
                + '<a href="x">l</a><a>no-href</a><script>1<' + '/script></body></html>',
                'text/html');
            return [
                typeof doc.scripts,
                doc.scripts.length,
                doc.scripts[0].getAttribute('src'),
                doc.scripts instanceof HTMLCollection,
                doc.scripts.namedItem ? doc.scripts.item(1).tagName : 'no-namedItem',
                doc.forms.length,
                doc.forms.namedItem('f1') === doc.forms[0],
                doc.images.length,
                doc.links.length,
                doc.currentScript === null,
            ];
        "#,
        )
        .unwrap();
    assert_eq!(
        result,
        serde_json::json!(["object", 2, "a.js", true, "SCRIPT", 1, true, 1, 1, true,])
    );
}

#[test]
fn test_domparser_xml_tree_builder_namespaces_and_structure() {
    // obscura#883: valid XML under an XML mime must come back as a real
    // tree — source-case qualified names, xmlns-resolved namespaceURI
    // (prefixed scopes and default-namespace inheritance), self-closing
    // nesting, CDATA sections, decoded attribute entities — not the
    // <html> collapse the HTML fragment parser produced.
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt.evaluate(r#"
            const p = (src, mime) => new DOMParser().parseFromString(src, mime || 'application/xml');
            const d1 = p('<edmx:Edmx xmlns:edmx="http://docs.oasis-open.org/odata/ns/edmx"><edmx:DataServices/></edmx:Edmx>');
            const d2 = p('<feed xmlns="http://www.w3.org/2005/Atom"><entry title="a&amp;b">hi</entry></feed>');
            const d3 = p('<r xmlns="http://example.com/x"><c><![CDATA[a<b & c]]></c><pi/></r>');
            const d4 = p('<bad><a></b></bad>');
            const d5 = p('<div>html path</div>', 'text/html');
            const root1 = d1.documentElement;
            const entry = d2.documentElement.firstElementChild;
            return [
                root1.tagName,
                root1.namespaceURI,
                root1.firstElementChild.tagName,
                root1.firstElementChild.localName,
                root1.firstElementChild.namespaceURI,
                d2.documentElement.tagName,
                d2.documentElement.namespaceURI,
                entry.namespaceURI,
                entry.getAttribute('title'),
                entry.querySelectorAll('ENTRY').length,
                d2.documentElement.querySelectorAll('entry').length,
                d3.documentElement.querySelectorAll('c').length,
                d3.documentElement.firstElementChild.firstChild.nodeType,
                d3.documentElement.firstElementChild.firstChild.data,
                d3.documentElement.children.length,
                !!d4.querySelector('parsererror'),
                d5.documentElement.tagName,
            ];
        "#).unwrap();
    assert_eq!(
        result,
        serde_json::json!([
            "edmx:Edmx",
            "http://docs.oasis-open.org/odata/ns/edmx",
            "edmx:DataServices",
            "DataServices",
            "http://docs.oasis-open.org/odata/ns/edmx",
            "feed",
            "http://www.w3.org/2005/Atom",
            "http://www.w3.org/2005/Atom",
            "a&b",
            0,
            1,
            1,
            4,
            "a<b & c",
            2,
            true,
            "HTML",
        ])
    );
}

#[test]
fn test_domparser_xml_img_serializes_with_closing_tag() {
    // Report 2026-09-15: an XML `<img size="123">text</img>` serialized
    // as a self-closing HTML void element — text child and closing tag
    // both dropped from outerHTML. Void-element self-closing is HTML-
    // only; null-namespace (no xmlns) XML elements must keep theirs.
    // No-xmlns XML elements also carry the null namespace, so tagName
    // keeps its source case (Chrome: XML docs never uppercase).
    let mut rt = setup_runtime("<html><body></body></html>");
    let result = rt
        .evaluate(
            r#"
            const doc = new DOMParser().parseFromString('<img size="123">text</img>', 'text/xml');
            const root = doc.documentElement;
            return [
                root.tagName,
                root.getAttribute('size'),
                root.textContent,
                root.outerHTML,
            ];
        "#,
        )
        .unwrap();
    assert_eq!(
        result,
        serde_json::json!(["img", "123", "text", "<img size=\"123\">text</img>",])
    );
}

#[test]
fn test_document_referrer_semantics() {
    // Upstream edb1785: document.referrer is explicit navigation state —
    // empty for direct automation navigations, the strict-origin-
    // when-cross-origin value for document-initiated hops.
    let mut rt = setup_runtime("<html><body></body></html>");
    assert_eq!(
        rt.evaluate("document.referrer").unwrap(),
        serde_json::json!("")
    );
    rt.set_referrer("https://source.example/path?q=1");
    assert_eq!(
        rt.evaluate("document.referrer").unwrap(),
        serde_json::json!("https://source.example/path?q=1")
    );
}

/// #16: document named access. Chrome truth (headless probe 2026-11-05):
/// names come from the name attribute of applet/embed/form/iframe/img/
/// object (plus the id of an element that ALSO has a name — a bare id never
/// resolves on document, that's the window half), the exposed face is a
/// plain own VALUE prop (writable/enumerable/configurable), it shadows
/// PROTOTYPE members ([LegacyOverrideBuiltIns]) but real own props
/// (location) win, removal uninstalls live, and two bearers give an
/// HTMLCollection.
#[test]
fn document_named_access_resolves_and_shadows() {
    let mut rt = setup_runtime(
        "<html><body><div id=\"fooId\"></div><img name=\"barName\" id=\"barId\">\
         <form name=\"frmName\"></form><img name=\"forms\">\
         <iframe name=\"ifrName\" src=\"about:blank\"></iframe></body></html>",
    );
    let out = rt
        .evaluate(
            r#"
        (function () { try {
        const gopd = (n) => { const d = Object.getOwnPropertyDescriptor(document, n); return d ? Object.keys(d).join('+') : 'none'; };
        globalThis.__out = JSON.stringify([
            typeof document.fooId,                       // bare id: NOT a document name (Chrome: undefined)
            document.barName.tagName,                    // img name resolves
            document.frmName.tagName,                    // form name resolves
            document.barId.tagName,                      // id resolves when a name is also present
            gopd('barName'),                             // value+writable+enumerable+configurable
            Object.getOwnPropertyNames(document).includes('barName'),
            Object.keys(document).includes('barName'),
            (function () { for (const k in document) if (k === 'barName') return true; return false; })(),
            document.forms.tagName,                      // [LegacyOverrideBuiltIns]: img shadows the collection
            typeof document.location === 'object',       // real own prop (location) wins over <iframe name=…>… none here, sanity
            (function () { const i = document.querySelector('iframe[name=ifrName]'); return document.ifrName === i.contentWindow; })(),
            (function () { document.body.insertAdjacentHTML('beforeend', '<img name=\"q1\"><img name=\"q1\">'); return [Object.prototype.toString.call(document.q1), document.q1.length]; })(),
            (function () { const e = document.querySelector('img[name=barName]'); e.remove(); return typeof document.barName; })(),
            (function () { document.body.insertAdjacentHTML('beforeend', '<img name=\"retName\">'); return typeof document.retName; })(),
        ]);
        return globalThis.__out;
        } catch (e) { return 'ERR ' + e.name + ': ' + e.message; } })()
    "#,
        )
        .unwrap();
    let s = out.as_str().unwrap();
    assert!(
        !s.starts_with("ERR"),
        "document named access probe threw: {}",
        s
    );
    let v: serde_json::Value = serde_json::from_str(s).unwrap();
    assert_eq!(
        v,
        serde_json::json!([
            "undefined",
            "IMG",
            "FORM",
            "IMG",
            "value+writable+enumerable+configurable",
            true,
            true,
            true,
            "IMG",
            true,
            true,
            ["[object HTMLCollection]", 2],
            "undefined",
            "object",
        ]),
        "document named access: name-based resolution, value-prop face, override-builtins, live tracking"
    );
}

/// #59: document.evaluate + a global XPathResult. Covers the node-test
/// shapes, positional/[last()] predicates, attribute results, contains/not and
/// the constant surface (static + prototype). Originally written against the
/// compact subset; since knife 21 (full XPath 1.0, xpath_matches_chrome_truth)
/// the assertions follow Chrome: an ANY_TYPE node-set result is an iterator
/// (stringValue on it throws), and unions are legal.
#[test]
fn document_evaluate_xpath_subset() {
    let mut rt = setup_runtime(
        "<html><body><div id=\"list\"><p class=\"a\">one</p><p class=\"b\">two</p>\
         <p class=\"a\">three</p></div><a href=\"/x\">link</a></body></html>",
    );
    let _out = rt.evaluate(r#"
        try {
        const snap = document.evaluate('//p', document, null, XPathResult.ORDERED_NODE_SNAPSHOT_TYPE, null);
        const count = snap.snapshotLength;
        const second = snap.snapshotItem(1).textContent;
        const first = document.evaluate('//p[@class="a"]', document, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null).singleNodeValue.textContent;
        const contains = document.evaluate('//p[contains(text(),"tw")]', document, null, 9, null).singleNodeValue.textContent;
        const notA = document.evaluate('//p[not(@class="a")]', document, null, 7, null);
        const attr = document.evaluate('//a/@href', document, null, 0, null);
        const attrVal = attr.iterateNext().value;
        const pos2 = document.evaluate('//p[2]', document, null, 9, null).singleNodeValue.textContent;
        const lastP = document.evaluate('//p[last()]', document, null, 9, null).singleNodeValue.textContent;
        const dotEq = document.evaluate('//p[.="three"]', document, null, 9, null).singleNodeValue.textContent;
        const iter = document.evaluate('//p', document, null, 5, null);
        const iterFirst = iter.iterateNext().textContent;
        // Drain the remaining two nodes, then the next call must be null —
        // iterateNext yields one node per call, exhaustion comes after the last.
        iter.iterateNext();
        iter.iterateNext();
        const iterFourth = iter.iterateNext();
        const consts = [XPathResult.ANY_TYPE, XPathResult.FIRST_ORDERED_NODE_TYPE,
                        XPathResult.prototype.ORDERED_NODE_SNAPSHOT_TYPE, typeof XPathResult];
        let threw = null;
        try { threw = document.evaluate('//p | //a', document, null, 9, null).singleNodeValue.textContent; } catch (e) { threw = e.name; }
        globalThis.__out = JSON.stringify([
            count, second, first, contains, notA.snapshotLength,
            attr.resultType, attrVal, pos2, lastP, dotEq,
            iterFirst, iterFourth === null, consts, threw,
        ]);
        } catch (e) { globalThis.__out = 'ERR ' + e.name + ': ' + e.message; }
    "#).unwrap();
    let raw = rt
        .evaluate("globalThis.__out")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    assert!(raw.starts_with('['), "xpath probe failed: {}", raw);
    let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(
        v,
        serde_json::json!([
            3,
            "two",
            "one",
            "two",
            1,
            4,
            "/x",
            "two",
            "three",
            "three",
            "one",
            true,
            [0, 9, 7, "function"],
            "one"
        ]),
        "evaluate: snapshots, predicates, attr results, iterator exhaustion, constants, unions"
    );
}

/// Re-rendering a container with the same ids must hand back the NEW elements: getElementById only finds nodes in the document
/// (the id index used to keep pointing at the detached old node, and never learned the new one).
#[tokio::test(flavor = "current_thread")]
async fn get_element_by_id_ignores_detached_nodes_after_inner_html() {
    let mut rt = setup_runtime("<html><body></body></html>");
    let out = rt
        .evaluate(
            r#"(function () {
        var h = document.createElement('div'); document.body.appendChild(h);
        var H = '<select id="s"><option>a</option></select><p id="p">x</p>';
        h.innerHTML = H; var s1 = document.getElementById('s');
        h.innerHTML = H; var s2 = document.getElementById('s');
        var d = document.createElement('div'); d.id = 'loose';
        return JSON.stringify([s1 === s2, s1.isConnected, s2.isConnected, document.querySelectorAll('select').length, document.getElementById('loose')]);
    })()"#,
        )
        .unwrap();
    assert_eq!(out.as_str().unwrap(), "[false,false,true,1,null]");
}
