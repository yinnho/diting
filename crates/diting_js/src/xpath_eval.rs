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
    /// live node -> snapshot slot, for context resolution on cache hits:
    /// a reused snapshot's evaluation context is the CALLER's node, never
    /// the node the snapshot was first built for.
    pub node_snap: std::collections::HashMap<NodeId, SnapshotNodeId>,
    /// (attribute owner, qualified name) -> snapshot slot, same purpose.
    pub attr_snap: std::collections::HashMap<(NodeId, String), SnapshotNodeId>,
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
    let mut b = SnapshotBuilder::new();
    let mut back: Vec<Option<BackRef>> = Vec::new();
    let mut node_snap: std::collections::HashMap<NodeId, SnapshotNodeId> =
        std::collections::HashMap::new();
    let mut attr_snap: std::collections::HashMap<(NodeId, String), SnapshotNodeId> =
        std::collections::HashMap::new();
    let mut ctx_snap: Option<SnapshotNodeId> = None;

    // preorder walk; the builder indexes nodes in push order.
    fn push_tracking(
        back: &mut Vec<Option<BackRef>>,
        node_snap: &mut std::collections::HashMap<NodeId, SnapshotNodeId>,
        attr_snap: &mut std::collections::HashMap<(NodeId, String), SnapshotNodeId>,
        id: SnapshotNodeId,
        r: Option<BackRef>,
    ) {
        if back.len() <= id {
            back.resize(id + 1, None);
        }
        if let Some(ref br) = r {
            match br {
                BackRef::Node(n) => {
                    node_snap.insert(*n, id);
                }
                BackRef::Attr(owner, q) => {
                    attr_snap.insert((*owner, q.clone()), id);
                }
            }
        }
        back[id] = r;
    }

    // iterative preorder walk with explicit stack: (live node, snapshot parent)
    let mut stack: Vec<(NodeId, Option<SnapshotNodeId>)> = Vec::new();
    // The root itself may be the document or a detached element. Chrome puts
    // no document node above a detached tree: the topmost element IS the
    // root, so `/` selects it and `/x` walks its children.
    let root_data = tree.get_node(root)?;
    let root_snap;
    if matches!(root_data.data, NodeData::Document) {
        let doc_snap = b.append_document();
        // the document node itself is a selectable result (`/`, `/.`)
        push_tracking(
            &mut back,
            &mut node_snap,
            &mut attr_snap,
            doc_snap,
            Some(BackRef::Node(root)),
        );
        root_snap = doc_snap;
        if root == context {
            ctx_snap = Some(doc_snap);
        }
        for c in tree.children(root).into_iter().rev() {
            stack.push((c, Some(doc_snap)));
        }
    } else {
        // detached subtree: the walk below creates the root node parentless
        // (None parent), making it the snapshot root itself — any node kind,
        // not just elements: a bare text node is its own root too.
        root_snap = usize::MAX; // reassigned when the root node is pushed
        stack.push((root, None));
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
                push_tracking(
                    &mut back,
                    &mut node_snap,
                    &mut attr_snap,
                    id,
                    Some(BackRef::Node(nid)),
                );
                for a in attrs {
                    let (alocal, aprefix, ans) = qual_parts(&a.name);
                    let aid = b.append_attribute(id, alocal, a.value.clone(), aprefix, ans);
                    let qualified = if let Some(p) = &a.name.prefix {
                        format!("{}:{}", p, a.name.local)
                    } else {
                        a.name.local.to_string()
                    };
                    push_tracking(
                        &mut back,
                        &mut node_snap,
                        &mut attr_snap,
                        aid,
                        Some(BackRef::Attr(nid, qualified)),
                    );
                }
                for c in tree.children(nid).into_iter().rev() {
                    stack.push((c, Some(id)));
                }
                id
            }
            NodeData::Text { contents } => {
                let id = b.append_text(parent_snap, contents.clone());
                push_tracking(
                    &mut back,
                    &mut node_snap,
                    &mut attr_snap,
                    id,
                    Some(BackRef::Node(nid)),
                );
                id
            }
            NodeData::Comment { contents } => {
                let id = b.append_comment(parent_snap, contents.clone());
                push_tracking(
                    &mut back,
                    &mut node_snap,
                    &mut attr_snap,
                    id,
                    Some(BackRef::Node(nid)),
                );
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

    // detached trees: the root node was the first node the builder pushed
    let root_snap = if root_snap == usize::MAX {
        0
    } else {
        root_snap
    };
    Some(Built {
        snapshot: b.finish(root_snap),
        back,
        node_snap,
        attr_snap,
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

#[derive(Debug)]
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
    use diting_html5ever::{ns, LocalName, Namespace, QualName};

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
    fn attribute_axis_star() {
        let (tree, body, a, _b) = fixture();
        let out = evaluate(&tree, "//div[@id='a']/@*", body).unwrap();
        eprintln!("attr star: {out:?}");
        let out2 = evaluate(&tree, "//@id", body).unwrap();
        eprintln!("id axis: {out2:?}");
        let _ = a;
    }
    #[test]
    fn bad_expression_errors() {
        let (tree, body, _, _) = fixture();
        assert!(evaluate(&tree, "//[", body).is_err());
    }
}
