//! Native XPath evaluation core: build a moli-xpath snapshot from the live
//! DomTree, evaluate, map results back to node ids. The JS face stays thin
//! (xpath.impl.js keeps the XPathResult interfaces and node wrapping) — the
//! 39KB engine mass is what moves here. Pure functions over &DomTree so the
//! unit tests need no runtime.

use diting_dom::{DomTree, NodeData, NodeId};
use diting_xpath::{
    evaluate_parsed_snapshot_xpath, parse, SnapshotBuilder, SnapshotNodeId, SnapshotValue,
};

/// A snapshot node that maps back to the live tree — elements/text/etc. by
/// NodeId, attributes by owner + qualified name (they have no NodeId).
#[derive(Clone, Debug, PartialEq)]
pub enum BackRef {
    Node(NodeId),
    Attr(NodeId, String),
}

pub struct Built {
    pub snapshot: diting_xpath::Snapshot,
    /// index = SnapshotNodeId
    pub back: Vec<Option<BackRef>>,
    pub root_snapshot_id: SnapshotNodeId,
    pub context_snapshot_id: Option<SnapshotNodeId>,
}

fn qual_parts(name: &diting_html5ever::QualName) -> (String, Option<String>, Option<String>) {
    let local = name.local.to_string();
    let prefix = name.prefix.as_ref().map(|p| p.to_string());
    let ns = if name.ns.is_empty() {
        None
    } else {
        Some(name.ns.to_string())
    };
    (local, prefix, ns)
}

/// Build a snapshot covering the whole tree the context node belongs to
/// (detached trees included: the root is the context's topmost ancestor).
pub fn build_snapshot(tree: &DomTree, context: NodeId) -> Option<Built> {
    // topmost ancestor of the context node
    let mut root = context;
    while let Some(p) = tree.get_node(root).and_then(|n| n.parent) {
        root = p;
    }
    let is_html_doc = !tree.is_quirks(); // namespace html-ness: diting documents are HTML documents; quirks is unrelated but the flag plumbing lands with the truth pass
    let _ = is_html_doc;

    let mut b = SnapshotBuilder::new();
    let mut back: Vec<Option<BackRef>> = Vec::new();
    let mut ctx_snap: Option<SnapshotNodeId> = None;

    // preorder walk; the builder indexes nodes in push order.
    fn push_tracking(back: &mut Vec<Option<BackRef>>, id: SnapshotNodeId, r: Option<BackRef>) {
        if back.len() <= id {
            back.resize(id + 1, None);
        }
        back[id] = r;
    }

    let doc_snap = b.append_document();
    push_tracking(&mut back, doc_snap, None);
    let root_snap;

    // iterative preorder walk with explicit stack: (live node, snapshot parent)
    let mut stack: Vec<(NodeId, SnapshotNodeId)> = Vec::new();
    // the root itself may be the document or a detached element
    let root_data = tree.get_node(root)?;
    match &root_data.data {
        NodeData::Document => {
            root_snap = doc_snap;
            if root == context {
                ctx_snap = Some(doc_snap);
            }
            for c in tree.children(root).into_iter().rev() {
                stack.push((c, doc_snap));
            }
        }
        _ => {
            // detached subtree: still needs a document-ish root — the builder
            // has no bare element root, so parent it under the document node.
            root_snap = doc_snap;
            stack.push((root, doc_snap));
        }
    }

    while let Some((nid, parent_snap)) = stack.pop() {
        let Some(node) = tree.get_node(nid) else {
            continue;
        };
        let snap = match &node.data {
            NodeData::Document => {
                for c in tree.children(nid).into_iter().rev() {
                    stack.push((c, parent_snap));
                }
                continue;
            }
            NodeData::Element { name, attrs, .. } => {
                let (local, prefix, ns) = qual_parts(name);
                let html_el = ns.as_deref() == Some("http://www.w3.org/1999/xhtml") || ns.is_none();
                let id = b.append_element(parent_snap, local, prefix, ns, html_el);
                push_tracking(&mut back, id, Some(BackRef::Node(nid)));
                for a in attrs {
                    let (alocal, aprefix, ans) = qual_parts(&a.name);
                    let aid = b.append_attribute(id, alocal, a.value.clone(), aprefix, ans);
                    let qualified = if let Some(p) = &a.name.prefix {
                        format!("{}:{}", p, a.name.local)
                    } else {
                        a.name.local.to_string()
                    };
                    push_tracking(&mut back, aid, Some(BackRef::Attr(nid, qualified)));
                }
                for c in tree.children(nid).into_iter().rev() {
                    stack.push((c, id));
                }
                id
            }
            NodeData::Text { contents } => {
                let id = b.append_text(parent_snap, contents.clone());
                push_tracking(&mut back, id, Some(BackRef::Node(nid)));
                id
            }
            NodeData::Comment { contents } => {
                let id = b.append_comment(parent_snap, contents.clone());
                push_tracking(&mut back, id, Some(BackRef::Node(nid)));
                id
            }
            NodeData::ProcessingInstruction { .. } => {
                // moli-xpath's snapshot path has no PI kind — they fall out.
                // The truth probe tells whether any key depends on one.
                continue;
            }
            NodeData::Doctype { .. } => continue,
        };
        if nid == context {
            ctx_snap = Some(snap);
        }
    }

    Some(Built {
        snapshot: b.finish(root_snap),
        back,
        root_snapshot_id: root_snap,
        context_snapshot_id: ctx_snap,
    })
}

/// Placeholder resolver: no prefix mappings (createNSResolver wiring lands
/// with the namespace truth keys).
#[derive(Clone)]
pub struct NoResolver;
impl diting_xpath::NamespaceResolver for NoResolver {
    fn resolve_namespace_prefix(&self, _prefix: &str) -> Option<String> {
        None
    }
}

pub enum EvalOut {
    Boolean(bool),
    Number(f64),
    String(String),
    Nodes(Vec<BackRef>),
}

pub fn evaluate(tree: &DomTree, expr_src: &str, context: NodeId) -> Result<EvalOut, String> {
    let built = build_snapshot(tree, context).ok_or("context node not in tree")?;
    let ctx_snap = built
        .context_snapshot_id
        .ok_or("context node not in snapshot")?;
    let expr = parse(expr_src, None::<NoResolver>, true).map_err(|e| format!("parse: {e:?}"))?;
    let value = evaluate_parsed_snapshot_xpath(&built.snapshot, &expr, ctx_snap)
        .map_err(|e| format!("eval: {e:?}"))?;
    Ok(match value {
        SnapshotValue::Boolean(v) => EvalOut::Boolean(v),
        SnapshotValue::Number(v) => EvalOut::Number(v),
        SnapshotValue::String(v) => EvalOut::String(v),
        SnapshotValue::Nodes(ids) => EvalOut::Nodes(
            ids.into_iter()
                .filter_map(|i| built.back.get(i).cloned().flatten())
                .collect(),
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use diting_dom::NodeData;
    use diting_html5ever::{local_name, ns, LocalName, Namespace, QualName};

    fn el(tree: &DomTree, local: &str) -> NodeId {
        tree.new_node(NodeData::Element {
            name: QualName::new(None, ns!(html), LocalName::from(local)),
            attrs: vec![],
            template_contents: None,
            mathml_annotation_xml_integration_point: false,
            live_value: None,
            live_checked: None,
            live_indeterminate: false,
        })
    }

    fn el_id(tree: &DomTree, local: &str, id: &str) -> NodeId {
        tree.new_node(NodeData::Element {
            name: QualName::new(None, ns!(html), LocalName::from(local)),
            attrs: vec![diting_dom::tree::Attribute {
                name: QualName::new(None, Namespace::default(), LocalName::from("id")),
                value: id.into(),
            }],
            template_contents: None,
            mathml_annotation_xml_integration_point: false,
            live_value: None,
            live_checked: None,
            live_indeterminate: false,
        })
    }

    fn text(tree: &DomTree, s: &str) -> NodeId {
        tree.new_node(NodeData::Text { contents: s.into() })
    }

    fn fixture() -> (DomTree, NodeId, NodeId, NodeId) {
        // document > html > body > (div#a, div#b > span, text)
        let tree = DomTree::new();
        let html = el(&tree, "html");
        let body = el(&tree, "body");
        let a = el_id(&tree, "div", "a");
        let b = el_id(&tree, "div", "b");
        let span = el(&tree, "span");
        let t = text(&tree, "hello");
        tree.append_child(tree.document(), html);
        tree.append_child(html, body);
        tree.append_child(body, a);
        tree.append_child(body, b);
        tree.append_child(b, span);
        tree.append_child(span, t);
        (tree, body, a, b)
    }

    #[test]
    fn selects_elements_by_path() {
        let (tree, body, a, b) = fixture();
        let out = evaluate(&tree, "//div", body).unwrap();
        let EvalOut::Nodes(nodes) = out else {
            panic!("expected nodes")
        };
        let ids: Vec<NodeId> = nodes
            .iter()
            .filter_map(|r| match r {
                BackRef::Node(n) => Some(*n),
                _ => None,
            })
            .collect();
        assert!(ids.contains(&a) && ids.contains(&b), "got {ids:?}");
    }

    #[test]
    fn count_returns_number() {
        let (tree, body, _, _) = fixture();
        let out = evaluate(&tree, "count(//div)", body).unwrap();
        assert!(matches!(out, EvalOut::Number(n) if (n - 2.0).abs() < 1e-9));
    }

    #[test]
    fn string_conversion_of_text() {
        let (tree, body, _, _) = fixture();
        let out = evaluate(&tree, "string(//span)", body).unwrap();
        assert!(matches!(out, EvalOut::String(s) if s == "hello"));
    }

    #[test]
    fn attribute_selection_maps_back_to_owner() {
        let (tree, body, a, _) = fixture();
        let out = evaluate(&tree, "//div[@id='a']/@id", body).unwrap();
        let EvalOut::Nodes(nodes) = out else {
            panic!("expected nodes")
        };
        assert_eq!(nodes.len(), 1);
        assert!(matches!(&nodes[0], BackRef::Attr(owner, q) if *owner == a && q == "id"));
    }

    #[test]
    fn predicate_filters() {
        let (tree, body, _, b) = fixture();
        let out = evaluate(&tree, "//div[@id='b']", body).unwrap();
        let EvalOut::Nodes(nodes) = out else {
            panic!("expected nodes")
        };
        assert_eq!(nodes.len(), 1);
        assert!(matches!(&nodes[0], BackRef::Node(n) if *n == b));
    }

    #[test]
    fn child_axis_and_parent_axis() {
        let (tree, body, _, b) = fixture();
        let out = evaluate(&tree, "//span/parent::div", body).unwrap();
        let EvalOut::Nodes(nodes) = out else {
            panic!("expected nodes")
        };
        assert_eq!(nodes.len(), 1);
        assert!(matches!(&nodes[0], BackRef::Node(n) if *n == b));
    }

    #[test]
    fn bad_expression_errors() {
        let (tree, body, _, _) = fixture();
        assert!(evaluate(&tree, "//[", body).is_err());
    }
}
