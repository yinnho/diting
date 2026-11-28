//! XPath op bridge: the JS face (xpath face in the platform bundle) scans
//! prefixes via op_xpath_prefixes, resolves them JS-side (node resolvers go
//! through Node.lookupNamespaceURI, function/object resolvers are user JS),
//! then op_xpath_eval parses with the resolution map and evaluates over a
//! snapshot of the live tree. Stringly-typed per the house op style.

use std::cell::RefCell;
use std::collections::HashMap;

use diting_dom::NodeId;
use diting_ops::diting_op;
use diting_runtime::state::OpState;
use diting_xpath::{evaluate_parsed_snapshot_xpath, parse, NamespaceResolver, SnapshotValue};

use super::SharedState;
use crate::xpath_eval::{build_snapshot, BackRef};

/// Records every prefix the parser asks about, answering with a dummy
/// namespace so parse completes; the real resolutions come back from JS.
/// Cloneable so the parse can consume one copy while the caller keeps one.
#[derive(Clone)]
struct Collecting {
    seen: std::rc::Rc<RefCell<Vec<String>>>,
}
impl NamespaceResolver for Collecting {
    fn resolve_namespace_prefix(&self, prefix: &str) -> Option<String> {
        let mut seen = self.seen.borrow_mut();
        if !seen.iter().any(|p| p == prefix) {
            seen.push(prefix.to_string());
        }
        Some("urn:diting-collect".to_string())
    }
}

#[diting_op]
#[string]
pub(super) fn op_xpath_prefixes(state: &OpState, #[string] expr: String) -> String {
    let _ = state;
    let c = Collecting {
        seen: std::rc::Rc::new(RefCell::new(Vec::new())),
    };
    match parse(&expr, Some(c.clone()), true) {
        Ok(_) => serde_json::json!({ "ok": c.seen.borrow().clone() }).to_string(),
        Err(e) => serde_json::json!({ "err": format!("{e:?}") }).to_string(),
    }
}

#[derive(Clone)]
struct MapResolver {
    map: HashMap<String, String>,
}
impl NamespaceResolver for MapResolver {
    fn resolve_namespace_prefix(&self, prefix: &str) -> Option<String> {
        self.map.get(prefix).cloned()
    }
}

#[diting_op]
#[string]
pub(super) fn op_xpath_eval(
    state: &OpState,
    #[string] expr: String,
    ctx_nid: i32,
    #[string] resolutions: String,
    #[string] attr_name: String,
) -> String {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        op_xpath_eval_inner(state, &expr, ctx_nid, &resolutions, &attr_name)
    }))
    .unwrap_or_else(|_| serde_json::json!({ "err": "panic" }).to_string())
}

fn op_xpath_eval_inner(
    state: &OpState,
    expr: &str,
    ctx_nid: i32,
    resolutions: &str,
    attr_name: &str,
) -> String {
    let gs = state.borrow::<SharedState>().clone();
    let gs = gs.borrow();
    let Some(dom) = &gs.dom else {
        return serde_json::json!({ "err": "no-dom" }).to_string();
    };
    let map: HashMap<String, String> = serde_json::from_str(resolutions).unwrap_or_default();
    let resolver = if map.is_empty() {
        None
    } else {
        Some(MapResolver { map })
    };
    let expr_parsed = match parse(expr, resolver, true) {
        Ok(e) => e,
        Err(diting_xpath::ParserError::FailedToResolveNamespacePrefix) => {
            return serde_json::json!({ "err": "namespace" }).to_string();
        }
        Err(e) => {
            return serde_json::json!({ "err": "syntax", "detail": format!("{e:?}") }).to_string();
        }
    };
    if ctx_nid < 0 {
        return serde_json::json!({ "err": "no-context" }).to_string();
    }
    let context = NodeId::new(ctx_nid as u32);
    // Snapshot cache: keyed on the tree's content-mutation epoch plus the
    // context's topmost ancestor (the snapshot root). A batch of evaluate
    // calls against an unmutated document shares one rebuild instead of
    // paying O(tree) per call.
    let mut root = context;
    while let Some(p) = dom.get_node(root).and_then(|n| n.parent) {
        root = p;
    }
    let epoch = dom.mutation_epoch();
    let cached = gs
        .xpath_snapshot_cache
        .borrow()
        .as_ref()
        .filter(|(e, r, _)| *e == epoch && *r == root)
        .map(|(_, _, b)| b.clone());
    let built = match cached {
        Some(b) => b,
        None => {
            let Some(b) = build_snapshot(dom, context) else {
                return serde_json::json!({ "err": "no-context" }).to_string();
            };
            let b = std::rc::Rc::new(b);
            *gs.xpath_snapshot_cache.borrow_mut() = Some((epoch, root, b.clone()));
            b
        }
    };
    let ctx_snap = if attr_name.is_empty() {
        built.node_snap.get(&context).copied()
    } else {
        // attribute context: the context node is the attribute itself
        built
            .attr_snap
            .get(&(context, attr_name.to_string()))
            .copied()
    };
    let Some(ctx_snap) = ctx_snap else {
        return serde_json::json!({ "err": "no-context" }).to_string();
    };
    match evaluate_parsed_snapshot_xpath(&built.snapshot, &expr_parsed, ctx_snap) {
        Ok(SnapshotValue::Boolean(v)) => {
            serde_json::json!({ "ok": { "kind": "boolean", "v": v } }).to_string()
        }
        Ok(SnapshotValue::Number(v)) => {
            // serde_json drops non-finite doubles to null; sentinel-string them
            // (the probe's own "#NaN" spelling) so JS can revive the exact value.
            let sv = if v.is_nan() {
                serde_json::json!("#NaN")
            } else if v.is_infinite() {
                serde_json::json!(if v > 0.0 { "#Infinity" } else { "#-Infinity" })
            } else {
                serde_json::json!(v)
            };
            serde_json::json!({ "ok": { "kind": "number", "v": sv } }).to_string()
        }
        Ok(SnapshotValue::String(v)) => {
            serde_json::json!({ "ok": { "kind": "string", "v": v } }).to_string()
        }
        Ok(SnapshotValue::Nodes(ids)) => {
            let nodes: Vec<serde_json::Value> = ids
                .iter()
                .filter_map(|i| built.back.get(*i).cloned().flatten())
                .map(|r| match r {
                    BackRef::Node(n) => serde_json::json!({ "nid": n.raw() }),
                    BackRef::Attr(owner, q) => {
                        serde_json::json!({ "attr": owner.raw(), "name": q })
                    }
                })
                .collect();
            serde_json::json!({ "ok": { "kind": "nodes", "nodes": nodes } }).to_string()
        }
        Err(e) => {
            // Chrome distinguishes type conversion failures ("Type conversion
            // failed while evaluating the expression.") from syntax errors;
            // both surface as SyntaxError but with different messages.
            let kind = if matches!(
                e,
                diting_xpath::SnapshotXPathEvaluationError::Evaluation(
                    diting_xpath::Error::NotANodeset
                )
            ) {
                "typeconv"
            } else {
                "eval"
            };
            serde_json::json!({ "err": kind, "detail": format!("{e:?}") }).to_string()
        }
    }
}
