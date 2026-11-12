// 脚本启动追踪：script 标记沿子树传播 + 动态脚本 fetch 门 — split out of the god file (ratchet, #75).
// Pure move: the functions see the same ops::* namespace they grew up in.
use super::*;

pub(crate) fn node_is_script(dom: &DomTree, node_id: NodeId) -> bool {
    dom.with_node(node_id, |node| {
        node.as_element()
            .map(|name| name.local.as_str().eq_ignore_ascii_case("script"))
            .unwrap_or(false)
    })
    .unwrap_or(false)
}

fn script_nodes_including_template_contents(dom: &DomTree, root: NodeId) -> Vec<NodeId> {
    let mut scripts = Vec::new();
    let mut stack = vec![root];
    while let Some(node_id) = stack.pop() {
        if node_is_script(dom, node_id) {
            scripts.push(node_id);
        }
        let template_contents = dom
            .with_node(node_id, |node| match &node.data {
                NodeData::Element {
                    template_contents, ..
                } => *template_contents,
                _ => None,
            })
            .flatten();
        if let Some(contents) = template_contents {
            stack.push(contents);
        }
        let children = dom.children(node_id);
        for child in children.into_iter().rev() {
            stack.push(child);
        }
    }
    scripts
}

pub(crate) fn mark_script_subtree_started(state: &JsState, root: NodeId) {
    let Some(dom) = state.dom.as_ref() else {
        return;
    };
    let scripts = script_nodes_including_template_contents(dom, root);
    state.already_started_scripts.borrow_mut().extend(scripts);
}

pub(super) fn propagate_script_start_state(
    dom: &DomTree,
    source_root: NodeId,
    cloned_root: NodeId,
    started: &RefCell<HashSet<NodeId>>,
) {
    let mut pairs = vec![(source_root, cloned_root)];
    let mut additions = Vec::new();
    let current = started.borrow();
    while let Some((source, cloned)) = pairs.pop() {
        if current.contains(&source) {
            additions.push(cloned);
        }

        let source_template = dom
            .with_node(source, |node| match &node.data {
                NodeData::Element {
                    template_contents, ..
                } => *template_contents,
                _ => None,
            })
            .flatten();
        let cloned_template = dom
            .with_node(cloned, |node| match &node.data {
                NodeData::Element {
                    template_contents, ..
                } => *template_contents,
                _ => None,
            })
            .flatten();
        if let (Some(source_contents), Some(cloned_contents)) = (source_template, cloned_template) {
            pairs.push((source_contents, cloned_contents));
        }

        let source_children = dom.children(source);
        let cloned_children = dom.children(cloned);
        for pair in source_children.into_iter().zip(cloned_children).rev() {
            pairs.push(pair);
        }
    }
    drop(current);
    started.borrow_mut().extend(additions);
}

#[diting_op]
pub(super) fn op_script_mark_started(state: &OpState, nid: u32) -> bool {
    let shared = state.borrow::<SharedState>().clone();
    let state = shared.borrow();
    let Some(dom) = state.dom.as_ref() else {
        return false;
    };
    let node_id = NodeId::new(nid);
    if !node_is_script(dom, node_id) {
        return false;
    }
    state.already_started_scripts.borrow_mut().insert(node_id);
    true
}

/// Atomically claim an executable script. A false result means the node was
/// created inert by an HTML-string API or has already been prepared once.
#[diting_op]
pub(super) fn op_script_try_start(state: &OpState, nid: u32) -> bool {
    let shared = state.borrow::<SharedState>().clone();
    let state = shared.borrow();
    let Some(dom) = state.dom.as_ref() else {
        return false;
    };
    let node_id = NodeId::new(nid);
    if !node_is_script(dom, node_id) {
        return false;
    }
    let newly_started = state.already_started_scripts.borrow_mut().insert(node_id);
    newly_started
}

/// Bracket a dynamic `<script src>` fetch so the settle loop can distinguish
/// "a script is still loading" from ordinary background XHR/fetch activity.
/// The page-level http_client's active_requests() counter never sees these —
/// they ride the op-level client cache — so without this bracket the settle
/// loop's fast path would strand scripts slower than its 500ms budget.
#[diting_op]
pub(super) fn op_dyn_script_fetch_begin(state: &OpState) {
    let shared = state.borrow::<SharedState>().clone();
    let state = shared.borrow();
    state
        .dynamic_script_fetches
        .set(state.dynamic_script_fetches.get() + 1);
}

#[diting_op]
pub(super) fn op_dyn_script_fetch_end(state: &OpState) {
    let shared = state.borrow::<SharedState>().clone();
    let state = shared.borrow();
    state
        .dynamic_script_fetches
        .set(state.dynamic_script_fetches.get().saturating_sub(1));
}

