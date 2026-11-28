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
    let Some(built) = build_snapshot(dom, context) else {
        return serde_json::json!({ "err": "no-context" }).to_string();
    };
    let ctx_snap = if attr_name.is_empty() {
        built.context_snapshot_id
    } else {
        // attribute context: the context node is the attribute itself — find
        // the snapshot slot whose back-ref is this owner+name pair
        let want = crate::xpath_eval::BackRef::Attr(context, attr_name.to_string());
        built.back.iter().position(|r| r.as_ref() == Some(&want))
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
        Err(e) => serde_json::json!({ "err": "eval", "detail": format!("{e:?}") }).to_string(),
    }
}
