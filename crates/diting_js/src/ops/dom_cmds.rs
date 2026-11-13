// dom_cmd 六族：document/query/tree/attr_state/mutate/create — split out of the god file (ratchet, #75).
// Pure move: the functions see the same ops::* namespace they grew up in.
use super::*;

/// `op_dom` family: document-level reads: ids, title/url/referrer, base url, doctype.
pub(super) fn dom_cmd_document(
    gs: &JsState,
    dom: &DomTree,
    cmd: &str,
    arg1: String,
    arg2: String,
) -> DomCmd {
    Ok(match cmd {
        "document_node_id" => dom.document().index().to_string(),
        "document_title" => serde_json::to_string(&gs.document.title()).unwrap_or("\"\"".into()),
        "document_referrer" => {
            serde_json::to_string(&gs.document.referrer()).unwrap_or("\"\"".into())
        }
        "document_url" => serde_json::to_string(&gs.document.url()).unwrap_or("\"\"".into()),
        // Document referrer policy (Referrer Policy §"Determine request's
        // Referrer Policy"): header-delivered policy beats every <meta
        // name=referrer>; "" = no policy (callers apply the spec default).
        "document_referrer_policy" => {
            let policy = resolve_referrer_policy_from(gs, dom);
            serde_json::to_string(&policy).unwrap_or("\"\"".into())
        }
        // Document BASE url (HTML §document-base-url): the document URL with
        // the first <base href> folded in. This is what relative URL
        // resolution (anchor/area href, form action, iframe src, fetch) must
        // resolve against — upstream obscura #658. document.URL and origin
        // checks stay on the plain "document_url".
        "document_base_url" => {
            let base = dom
                .query_selector("base[href]")
                .ok()
                .flatten()
                .and_then(|nid| {
                    dom.get_node(nid)
                        .and_then(|n| n.get_attribute("href").map(|v| v.to_string()))
                });
            let doc_url = gs.document.url();
            let folded = base.and_then(|href| {
                url::Url::parse(&doc_url)
                    .ok()
                    .and_then(|doc| doc.join(&href).ok())
                    .map(|u| u.to_string())
            });
            serde_json::to_string(&folded.unwrap_or(doc_url)).unwrap_or("\"\"".into())
        }
        "document_encoding" => {
            serde_json::to_string(&gs.document.encoding()).unwrap_or("\"UTF-8\"".into())
        }
        "document_content_type" => {
            serde_json::to_string(&gs.document.content_type()).unwrap_or("\"\"".into())
        }
        "document_element" => {
            for cid in dom.children(dom.document()) {
                if let Some(n) = dom.get_node(cid) {
                    if n.as_element()
                        .map(|name| name.local.as_str() == "html")
                        .unwrap_or(false)
                    {
                        return Ok(cid.index().to_string());
                    }
                }
            }
            "-1".into()
        }
        "document_doctype" => {
            for cid in dom.children(dom.document()) {
                if let Some(n) = dom.get_node(cid) {
                    if let diting_dom::NodeData::Doctype {
                        name,
                        public_id,
                        system_id,
                    } = &n.data
                    {
                        return Ok(serde_json::json!({
                            "name": name,
                            "publicId": public_id,
                            "systemId": system_id,
                            "nodeId": cid.index(),
                        })
                        .to_string());
                    }
                }
            }
            "null".into()
        }
        _ => return Err((arg1, arg2)),
    })
}

/// `op_dom` family: selector queries and matching.
pub(super) fn dom_cmd_query(
    _gs: &JsState,
    dom: &DomTree,
    cmd: &str,
    arg1: String,
    arg2: String,
) -> DomCmd {
    Ok(match cmd {
        "get_element_by_id" => dom
            .get_element_by_id(&arg1)
            .map(|id| id.index().to_string())
            .unwrap_or("-1".into()),
        "query_selector" => dom
            .query_selector(&arg1)
            .ok()
            .flatten()
            .map(|id| id.index().to_string())
            .unwrap_or("-1".into()),
        "query_selector_all" => {
            let ids: Vec<i32> = dom
                .query_selector_all(&arg1)
                .ok()
                .map(|ids| ids.iter().map(|id| id.index() as i32).collect())
                .unwrap_or_default();
            serde_json::to_string(&ids).unwrap_or("[]".into())
        }
        "query_selector_scoped" => {
            let root_nid = arg1.parse::<u32>().unwrap_or(0);
            dom.query_selector_from(NodeId::new(root_nid), &arg2)
                .ok()
                .flatten()
                .map(|id| id.index().to_string())
                .unwrap_or("-1".into())
        }
        "query_selector_all_scoped" => {
            let root_nid = arg1.parse::<u32>().unwrap_or(0);
            let ids: Vec<i32> = dom
                .query_selector_all_from(NodeId::new(root_nid), &arg2)
                .ok()
                .map(|ids| ids.iter().map(|id| id.index() as i32).collect())
                .unwrap_or_default();
            serde_json::to_string(&ids).unwrap_or("[]".into())
        }
        // Single-element match with `:scope` bound to the element itself
        // (Element.matches semantics). "0" on a parse error, mirroring how the
        // query ops return empty results rather than surfacing the error.
        "matches_selector" => {
            let nid = arg1.parse::<u32>().unwrap_or(0);
            if dom
                .matches_selector(NodeId::new(nid), &arg2)
                .unwrap_or(false)
            {
                "1".into()
            } else {
                "0".into()
            }
        }
        _ => return Err((arg1, arg2)),
    })
}

/// `op_dom` family: node identity and tree/shadow navigation.
pub(super) fn dom_cmd_tree(
    _gs: &JsState,
    dom: &DomTree,
    cmd: &str,
    arg1: String,
    arg2: String,
) -> DomCmd {
    Ok(match cmd {
        "node_type" => {
            let nid = arg1.parse::<u32>().unwrap_or(0);
            dom.get_node(NodeId::new(nid))
                .map(|n| match &n.data {
                    NodeData::Document => "9",
                    NodeData::Element { .. } => "1",
                    NodeData::Text { .. } => "3",
                    NodeData::Comment { .. } => "8",
                    NodeData::Doctype { .. } => "10",
                    NodeData::ProcessingInstruction { .. } => "7",
                })
                .unwrap_or("0")
                .into()
        }
        "node_name" => {
            let nid = arg1.parse::<u32>().unwrap_or(0);
            let name: String = dom
                .get_node(NodeId::new(nid))
                .map(|n| match &n.data {
                    NodeData::Document => "#document".to_string(),
                    NodeData::Element { name, .. } => {
                        if name.ns == diting_html5ever::ns!(html) {
                            name.local.as_str().to_ascii_uppercase()
                        } else {
                            name.local.as_str().to_string()
                        }
                    }
                    NodeData::Text { .. } => "#text".to_string(),
                    NodeData::Comment { .. } => "#comment".to_string(),
                    NodeData::Doctype { name, .. } => name.clone(),
                    NodeData::ProcessingInstruction { target, .. } => target.clone(),
                })
                .unwrap_or_default();
            serde_json::to_string(&name).unwrap_or("\"\"".into())
        }
        "text_content" => {
            let nid = arg1.parse::<u32>().unwrap_or(0);
            serde_json::to_string(&dom.text_content(NodeId::new(nid))).unwrap_or("\"\"".into())
        }
        "parent_node" | "first_child" | "last_child" | "next_sibling" | "prev_sibling" => {
            let nid = arg1.parse::<u32>().unwrap_or(0);
            dom.get_node(NodeId::new(nid))
                .and_then(|n| match cmd {
                    "parent_node" => n.parent,
                    "first_child" => n.first_child,
                    "last_child" => n.last_child,
                    "next_sibling" => n.next_sibling,
                    "prev_sibling" => n.prev_sibling,
                    _ => None,
                })
                .map(|id| id.index().to_string())
                .unwrap_or("-1".into())
        }
        "next_in_subtree" => {
            let root = NodeId::new(arg1.parse::<u32>().unwrap_or(0));
            let current = NodeId::new(arg2.parse::<u32>().unwrap_or(0));
            dom.next_in_subtree(root, current)
                .map(|id| id.index().to_string())
                .unwrap_or("-1".into())
        }
        "next_after_subtree" => {
            let root = NodeId::new(arg1.parse::<u32>().unwrap_or(0));
            let current = NodeId::new(arg2.parse::<u32>().unwrap_or(0));
            dom.next_after_subtree(root, current)
                .map(|id| id.index().to_string())
                .unwrap_or("-1".into())
        }
        "prev_in_subtree" => {
            let root = NodeId::new(arg1.parse::<u32>().unwrap_or(0));
            let current = NodeId::new(arg2.parse::<u32>().unwrap_or(0));
            dom.prev_in_subtree(root, current)
                .map(|id| id.index().to_string())
                .unwrap_or("-1".into())
        }
        // Root of the local tree scope: ordinary parents only, so a shadow
        // descendant resolves to its ShadowRoot (getRootNode default).
        "tree_scope_root" => {
            let nid = arg1.parse::<u32>().unwrap_or(0);
            dom.tree_scope_root(NodeId::new(nid))
                .map(|id| id.index().to_string())
                .unwrap_or("-1".into())
        }
        // Topmost root after crossing ShadowRoot-to-host edges
        // (getRootNode({ composed: true })).
        "shadow_including_root" => {
            let nid = arg1.parse::<u32>().unwrap_or(0);
            dom.shadow_including_root(NodeId::new(nid))
                .map(|id| id.index().to_string())
                .unwrap_or("-1".into())
        }
        // The shadow root a host carries, as "root_index|mode" (or "-1").
        // The JS shadowRoot getter consults this for roots the parser
        // attached (declarative shadow DOM) — those never ran attachShadow,
        // so the wrapper-side cache is cold.
        "shadow_root_of" => {
            let nid = arg1.parse::<u32>().unwrap_or(0);
            match dom.shadow_root(NodeId::new(nid)) {
                Some(root) => {
                    let mode = match dom.shadow_root_info(root).map(|i| i.mode) {
                        Some(ShadowRootMode::Closed) => "closed",
                        _ => "open",
                    };
                    format!("{}|{}", root.index(), mode)
                }
                None => "-1".into(),
            }
        }
        // Every shadow root in the tree as JSON [[host, root, mode], ...].
        // #95's hit-test pierce consumes the whole table in one call — the
        // engine's own registry, so closed roots are indistinguishable from
        // open ones here and page JS cannot forge entries.
        "shadow_hosts" => {
            let mut hosts: Vec<(usize, usize, &str)> = dom
                .shadow_hosts()
                .into_iter()
                .map(|(host, root, mode)| {
                    let m = if matches!(mode, ShadowRootMode::Closed) {
                        "closed"
                    } else {
                        "open"
                    };
                    (host.index(), root.index(), m)
                })
                .collect();
            hosts.sort();
            serde_json::to_string(&hosts).unwrap_or_else(|_| "[]".to_string())
        }
        "assigned_slot" => {
            let nid = arg1.parse::<u32>().unwrap_or(0);
            dom.assigned_slot(NodeId::new(nid))
                .map(|id| id.index().to_string())
                .unwrap_or("-1".into())
        }
        "assigned_nodes" => {
            let nid = arg1.parse::<u32>().unwrap_or(0);
            // "null" vs "[]": a slot outside any shadow tree has no
            // assignment AND no fallback; a slot inside one that nothing is
            // assigned to serves its fallback children (the JS side reads
            // the distinction).
            match dom.assigned_nodes(NodeId::new(nid)) {
                Some(ids) => {
                    serde_json::to_string(&ids.iter().map(|id| id.index()).collect::<Vec<_>>())
                        .unwrap_or("[]".into())
                }
                None => "null".into(),
            }
        }
        "child_nodes" => {
            let nid = arg1.parse::<u32>().unwrap_or(0);
            let ids: Vec<i32> = dom
                .children(NodeId::new(nid))
                .iter()
                .map(|id| id.index() as i32)
                .collect();
            serde_json::to_string(&ids).unwrap_or("[]".into())
        }
        "tag_name" => {
            let nid = arg1.parse::<u32>().unwrap_or(0);
            let name = dom.get_node(NodeId::new(nid)).and_then(|n| n.as_element().map(|name|
                // HTML elements read uppercase (Chrome tagName convention);
                // XML-namespace elements keep their source case.
                if name.ns == diting_html5ever::ns!(html) { name.local.as_str().to_ascii_uppercase() } else { name.local.as_str().to_string() }
            )).unwrap_or_default();
            serde_json::to_string(&name).unwrap_or("\"\"".into())
        }
        // True per-node namespace from the tree (parsed SVG children inherit
        // theirs from the html5ever foreign-content parse; create_element
        // records what the caller passed). The JS-side namespaceURI getter
        // reads this — a JS-only heuristic used to report XHTML for every
        // parsed <path>/<g>/... (#28).
        "namespace_uri" => {
            let nid = arg1.parse::<u32>().unwrap_or(0);
            let ns = dom
                .get_node(NodeId::new(nid))
                .and_then(|n| n.as_element().map(|name| name.ns.as_str().to_string()));
            serde_json::to_string(&ns).unwrap_or("null".into())
        }
        _ => return Err((arg1, arg2)),
    })
}

/// `op_dom` family: attributes and live form/interaction state.
pub(super) fn dom_cmd_attr_state(
    _gs: &JsState,
    dom: &DomTree,
    cmd: &str,
    arg1: String,
    arg2: String,
) -> DomCmd {
    Ok(match cmd {
        "get_attribute" => {
            let nid = arg1.parse::<u32>().unwrap_or(0);
            let val = dom
                .get_node(NodeId::new(nid))
                .and_then(|n| n.get_attribute(&arg2).map(|s| s.to_string()));
            serde_json::to_string(&val).unwrap_or("null".into())
        }
        "attribute_names" => {
            let nid = arg1.parse::<u32>().unwrap_or(0);
            let names: Vec<String> = dom
                .get_node(NodeId::new(nid))
                .map(|n| {
                    n.attrs()
                        .map(|a| {
                            a.iter()
                                .map(|x| x.name.local.as_str().to_string())
                                .collect()
                        })
                        .unwrap_or_default()
                })
                .unwrap_or_default();
            serde_json::to_string(&names).unwrap_or("[]".into())
        }
        "set_attribute" => {
            let node_id = match parse_nid(&arg1) {
                Some(id) => id,
                None => return Ok("false".into()),
            };
            if let Some((name, value)) = arg2.split_once('\0') {
                // The write stamps exactly what changed (class tokens, id,
                // attribute name): the incremental matcher re-matches only
                // the rules that read those.
                let old = dom.set_attribute_noted(node_id, name, value);
                if name == "id" {
                    dom.update_id_index(node_id, old.as_deref(), Some(value));
                }
            }
            "true".into()
        }
        // The form-control dirty value mirrored from the bootstrap's value
        // setter (see NodeData::Element::live_value). Not an attribute on
        // purpose: getAttribute/outerHTML must keep showing the ORIGINAL
        // value attribute like Chrome, while paint reads this instead.
        "set_live_value" => {
            let node_id = match parse_nid(&arg1) {
                Some(id) => id,
                None => return Ok("false".into()),
            };
            dom.with_node_mut(node_id, |n| n.set_live_value(arg2.to_string()));
            dom.note_live_change();
            "true".into()
        }
        // The checkable-input dirty checkedness mirror, same posture as
        // set_live_value: paint reads it to draw the checkbox/radio widget
        // state, while getAttribute/outerHTML keep showing only the parsed
        // `checked` attribute like Chrome.
        "set_live_checked" | "set_live_indeterminate" => {
            let node_id = match parse_nid(&arg1) {
                Some(id) => id,
                None => return Ok("false".into()),
            };
            dom.with_node_mut(node_id, |n| {
                if cmd == "set_live_checked" {
                    n.set_live_checked(arg2 == "1")
                } else {
                    n.set_live_indeterminate(arg2 == "1")
                }
            });
            dom.note_live_change();
            "true".into()
        }
        // Focus mirror (blitz#839): the bootstrap's focus()/blur() keep the
        // JS-side __diting_focused global for activeElement, and this op
        // mirrors the same fact into the tree so :focus/:focus-within/
        // :focus-visible selectors match on the next style run. arg2 "0"
        // clears (the JS side passes the node it is blurring; a blur when
        // some OTHER node holds focus must not steal it away).
        "set_focused" => {
            let node_id = match parse_nid(&arg1) {
                Some(id) => id,
                None => return Ok("false".into()),
            };
            if arg2 == "1" {
                dom.set_focused_node(Some(node_id));
            } else if dom.focused_node() == Some(node_id) {
                dom.set_focused_node(None);
            }
            "true".into()
        }
        // Hover/active mirrors (#152): the interaction layer's hover
        // transitions and press/release builders keep JS-side markers and
        // mirror the same facts into the tree so :hover/:active match on
        // the next style run. Same identity guard as set_focused's clear
        // arm — a stale blur/clear for a node that no longer holds the
        // state must not steal it.
        "set_hover" => {
            let node_id = match parse_nid(&arg1) {
                Some(id) => id,
                None => return Ok("false".into()),
            };
            if arg2 == "1" {
                dom.set_hovered_node(Some(node_id));
            } else if dom.hovered_node() == Some(node_id) {
                dom.set_hovered_node(None);
            }
            "true".into()
        }
        "set_active" => {
            let node_id = match parse_nid(&arg1) {
                Some(id) => id,
                None => return Ok("false".into()),
            };
            if arg2 == "1" {
                dom.set_active_node(Some(node_id));
            } else if dom.active_node() == Some(node_id) {
                dom.set_active_node(None);
            }
            "true".into()
        }
        // Selection mirror (caret batch): the bootstrap's selection APIs
        // keep (start, end) in WeakMaps for JS reads; this op mirrors the
        // same record into the tree so paint can resolve the caret at
        // collect time. arg2 is "start,end" — the JS numbers verbatim;
        // paint clamps to the value's char count (UTF-16 vs chars diverge
        // on astral text, accepted).
        "set_selection" => {
            let node_id = match parse_nid(&arg1) {
                Some(id) => id,
                None => return Ok("false".into()),
            };
            let Some((start, end)) = arg2.split_once(',') else {
                return Ok("false".into());
            };
            let (Ok(start), Ok(end)) = (start.trim().parse::<usize>(), end.trim().parse::<usize>())
            else {
                return Ok("false".into());
            };
            dom.set_selection(Some((node_id, start, end)));
            "true".into()
        }
        _ => return Err((arg1, arg2)),
    })
}

/// `op_dom` family: serialization and tree mutation (innerHTML, append/insert/remove, document.write).
pub(super) fn dom_cmd_mutate(
    gs: &JsState,
    dom: &DomTree,
    cmd: &str,
    arg1: String,
    arg2: String,
) -> DomCmd {
    Ok(match cmd {
        "inner_html" => {
            let nid = arg1.parse::<u32>().unwrap_or(0);
            serde_json::to_string(&dom.inner_html(NodeId::new(nid))).unwrap_or("\"\"".into())
        }
        "outer_html" => {
            let nid = arg1.parse::<u32>().unwrap_or(0);
            serde_json::to_string(&dom.outer_html(NodeId::new(nid))).unwrap_or("\"\"".into())
        }
        "append_child" => {
            let (parent, child) = match (parse_nid(&arg1), parse_nid(&arg2)) {
                (Some(p), Some(c)) => (p, c),
                _ => return Ok("false".into()),
            };
            dom.append_child(parent, child);
            "true".into()
        }
        "remove_child" => {
            let child = match parse_nid(&arg1) {
                Some(id) => id,
                None => return Ok("false".into()),
            };
            dom.remove_child(child);
            "true".into()
        }
        "insert_before" => {
            let (new_node, ref_node) = match (parse_nid(&arg1), parse_nid(&arg2)) {
                (Some(n), Some(r)) => (n, r),
                _ => return Ok("false".into()),
            };
            dom.insert_before(ref_node, new_node);
            "true".into()
        }
        "remove_attribute" => {
            let nid = match parse_nid(&arg1) {
                Some(id) => id,
                None => return Ok("false".into()),
            };
            let old = dom
                .get_node(nid)
                .and_then(|n| n.get_attribute(&arg2).map(|s| s.to_string()));
            dom.with_node_mut(nid, |n| {
                if let NodeData::Element { attrs, .. } = &mut n.data {
                    attrs.retain(|a| a.name.local.as_str() != arg2.as_str());
                }
            });
            // `Some("")` stands in for a present attribute whose value the
            // qualified-name read missed (namespaced): presence changed.
            dom.note_attr_change(nid, &arg2, Some(old.as_deref().unwrap_or("")), None);
            "true".into()
        }
        "set_inner_html" => {
            let target = match parse_nid(&arg1) {
                Some(id) => id,
                None => return Ok("false".into()),
            };
            let children = dom.children(target);
            for child in children {
                dom.detach(child);
            }
            if !arg2.is_empty() {
                let context_name = dom
                    .with_node(target, |node| match &node.data {
                        NodeData::Element { name, .. } => Some(name.clone()),
                        _ => None,
                    })
                    .flatten();
                let fragment = match context_name {
                    Some(name) => diting_dom::parse_fragment_with_context(&arg2, name),
                    None => diting_dom::parse_fragment(&arg2),
                };
                let import_root = fragment.fragment_root();
                dom.import_children_from(target, &fragment, import_root);
                // innerHTML-created scripts are inert per spec — mark them
                // started so a later move/clone never executes them.
                for child in dom.children(target) {
                    mark_script_subtree_started(gs, child);
                }
            }
            "true".into()
        }
        // document.write() feeds the document's input stream, so the calls
        // share one parser and one tokenizer state. Returns the nodes that
        // became complete with this call as [[parent, node], …], parents
        // before children; a `parent` of 0 means the node belongs at the
        // insertion point, which the JS caller knows. Nothing is inserted
        // here: insertion must go through Node.appendChild on the JS side,
        // which also reports the mutation and runs written scripts.
        "document_write" => {
            gs.pending_write_nav.set(true);
            let mut slot = gs.write_stream.borrow_mut();
            let stream = slot.get_or_insert_with(crate::write_stream::DocumentWriteStream::new);
            let pairs: Vec<[i32; 2]> = stream
                .write(&arg2, dom)
                .iter()
                .map(|placement| {
                    [
                        placement.parent.map_or(0, |id| id.index() as i32),
                        placement.node.index() as i32,
                    ]
                })
                .collect();
            serde_json::to_string(&pairs).unwrap_or("[]".into())
        }
        // document.open() discards what the input stream holds and starts over.
        "document_write_reset" => {
            *gs.write_stream.borrow_mut() = None;
            "true".into()
        }
        "set_text_content" => {
            let nid = match parse_nid(&arg1) {
                Some(id) => id,
                None => return Ok("false".into()),
            };
            dom.with_node_mut(nid, |n| match &mut n.data {
                NodeData::Text { contents } => {
                    *contents = arg2.clone();
                }
                NodeData::Comment { contents } => {
                    *contents = arg2.clone();
                }
                NodeData::ProcessingInstruction { data, .. } => {
                    *data = arg2.clone();
                }
                _ => {}
            });
            // Text data feeds :empty on the parent.
            dom.note_text_change(nid);
            "true".into()
        }
        _ => return Err((arg1, arg2)),
    })
}

/// `op_dom` family: node creation, cloning and structural predicates.
pub(super) fn dom_cmd_create(
    gs: &JsState,
    dom: &DomTree,
    cmd: &str,
    arg1: String,
    arg2: String,
) -> DomCmd {
    Ok(match cmd {
        "create_document_fragment" => dom.new_node(NodeData::Document).index().to_string(),
        "clone_node" => {
            let nid = match arg1.parse::<u32>() {
                Ok(n) => n,
                Err(_) => return Ok("-1".into()),
            };
            let source = NodeId::new(nid);
            match dom.clone_node(source, arg2 == "true") {
                Some(cloned) => {
                    propagate_script_start_state(dom, source, cloned, &gs.already_started_scripts);
                    cloned.index().to_string()
                }
                None => "-1".into(),
            }
        }
        "template_contents" => {
            let nid = arg1.parse::<u32>().unwrap_or(0);
            dom.template_contents(NodeId::new(nid))
                .map(|id| id.index().to_string())
                .unwrap_or("-1".into())
        }
        "create_element" => {
            // arg2 records a namespace on the node. The HTML path passes the
            // XHTML namespace explicitly (keeping tag_name/node_name's
            // uppercase convention); DOMParser's XML tree builder passes the
            // xmlns-resolved namespace, and an empty string is the null
            // namespace — what XML semantics require for elements with no
            // xmlns in scope. Serialization keys off this distinction: HTML
            // void elements self-close, XML elements always keep their
            // closing tag and text children.
            let ns = if arg2 == "http://www.w3.org/1999/xhtml" {
                diting_html5ever::ns!(html)
            } else {
                diting_html5ever::Namespace::from(arg2.as_str())
            };
            dom.new_node(NodeData::Element {
                name: diting_html5ever::QualName::new(
                    None,
                    ns,
                    diting_html5ever::LocalName::from(arg1.as_str()),
                ),
                attrs: vec![],
                template_contents: None,
                mathml_annotation_xml_integration_point: false,
                live_value: None,
                live_checked: None,
                live_indeterminate: false,
            })
            .index()
            .to_string()
        }
        "create_text_node" => dom
            .new_node(NodeData::Text {
                contents: arg1.clone(),
            })
            .index()
            .to_string(),
        "create_comment_node" => dom
            .new_node(NodeData::Comment {
                contents: arg1.clone(),
            })
            .index()
            .to_string(),
        "create_processing_instruction" => {
            // arg1 = target, arg2 = data
            dom.new_node(NodeData::ProcessingInstruction {
                target: arg1.clone(),
                data: arg2.clone(),
            })
            .index()
            .to_string()
        }
        "create_doctype" => {
            // arg1 = name, arg2 = public_id. system_id stored only in the
            // JS wrapper since neither current WPT test reads it back from
            // the underlying tree.
            dom.new_node(NodeData::Doctype {
                name: arg1.clone(),
                public_id: arg2.clone(),
                system_id: String::new(),
            })
            .index()
            .to_string()
        }
        "pi_target" => {
            let nid = arg1.parse::<u32>().unwrap_or(0);
            let val = dom
                .get_node(NodeId::new(nid))
                .and_then(|n| match &n.data {
                    NodeData::ProcessingInstruction { target, .. } => Some(target.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            serde_json::to_string(&val).unwrap_or("\"\"".into())
        }
        "doctype_name" => {
            let nid = arg1.parse::<u32>().unwrap_or(0);
            let val = dom
                .get_node(NodeId::new(nid))
                .and_then(|n| match &n.data {
                    NodeData::Doctype { name, .. } => Some(name.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            serde_json::to_string(&val).unwrap_or("\"\"".into())
        }
        "doctype_public_id" => {
            let nid = arg1.parse::<u32>().unwrap_or(0);
            let val = dom
                .get_node(NodeId::new(nid))
                .and_then(|n| match &n.data {
                    NodeData::Doctype { public_id, .. } => Some(public_id.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            serde_json::to_string(&val).unwrap_or("\"\"".into())
        }
        "element_children" => {
            let nid = arg1.parse::<u32>().unwrap_or(0);
            let ids: Vec<i32> = dom
                .children(NodeId::new(nid))
                .iter()
                .filter(|&&id| dom.get_node(id).map(|n| n.is_element()).unwrap_or(false))
                .map(|id| id.index() as i32)
                .collect();
            serde_json::to_string(&ids).unwrap_or("[]".into())
        }
        "has_child_nodes" => {
            let nid = arg1.parse::<u32>().unwrap_or(0);
            dom.get_node(NodeId::new(nid))
                .map(|n| n.first_child.is_some())
                .unwrap_or(false)
                .to_string()
        }
        "contains" => {
            let nid = arg1.parse::<u32>().unwrap_or(0);
            let other = arg2.parse::<u32>().unwrap_or(0);
            dom.descendants(NodeId::new(nid))
                .contains(&NodeId::new(other))
                .to_string()
        }
        // Index of a node among its parent's children. Walks prev siblings in
        // Rust, avoiding the per-step JS->op round trips a Range comparison
        // would otherwise make.
        "node_index" => {
            let nid = arg1.parse::<u32>().unwrap_or(0);
            node_child_index(dom, NodeId::new(nid)).to_string()
        }
        // Document (preorder) tree order of two nodes: -1 if a precedes b, 1 if
        // a follows b, 0 if equal. Used by the Range boundary-point algorithms.
        "compare_order" => {
            let a = NodeId::new(arg1.parse::<u32>().unwrap_or(0));
            let b = NodeId::new(arg2.parse::<u32>().unwrap_or(0));
            compare_node_order(dom, a, b).to_string()
        }
        // Root (topmost ancestor) of a node, in one op rather than an O(depth)
        // walk of parentNode ops from JS.
        "node_root" => {
            let mut cur = NodeId::new(arg1.parse::<u32>().unwrap_or(0));
            while let Some(p) = dom.get_node(cur).and_then(|x| x.parent) {
                cur = p;
            }
            cur.index().to_string()
        }
        _ => return Err((arg1, arg2)),
    })
}
