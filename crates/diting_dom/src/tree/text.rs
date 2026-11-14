//! Text content of a node.
//!
//! `text_content` reads the DOM `textContent`. `append_text` extends the
//! last text child or creates one. Comment and processing-instruction
//! data is returned only when the node itself is that character data.

use super::{DomTree, DomTreeInner, NodeData, NodeId};

impl DomTree {
    pub fn text_content(&self, node_id: NodeId) -> String {
        let inner = self.inner.borrow();
        // Per DOM spec, calling textContent ON a CharacterData node
        // (Text, Comment, ProcessingInstruction) returns its .data.
        // Calling textContent on an Element walks descendants and
        // concatenates Text node content only (Comment + PI are
        // skipped). Handle the direct-CharacterData case here so the
        // descent helper can keep its element-centric behavior.
        if let Some(Some(node)) = inner.nodes.get(node_id.index()) {
            match &node.data {
                NodeData::Text { contents } => return contents.clone(),
                NodeData::Comment { contents } => return contents.clone(),
                NodeData::ProcessingInstruction { data, .. } => return data.clone(),
                _ => {}
            }
        }
        let mut result = String::new();
        collect_text_inner(&inner, node_id, &mut result);
        result
    }

    pub fn append_text(&self, parent_id: NodeId, text: &str) {
        let last_child_is_text = {
            let inner = self.inner.borrow();
            inner
                .nodes
                .get(parent_id.index())
                .and_then(|n| n.as_ref())
                .and_then(|n| n.last_child)
                .and_then(|lc| inner.nodes.get(lc.index()))
                .and_then(|n| n.as_ref())
                .map(|n| n.is_text())
                .unwrap_or(false)
        };

        if last_child_is_text {
            // Re-read last_child without unwrap: if it vanished between the two
            // borrows, fall through to appending a fresh text node rather than
            // panicking (a panic here aborts the whole engine via V8_Fatal).
            let last_child_id = {
                let inner = self.inner.borrow();
                inner
                    .nodes
                    .get(parent_id.index())
                    .and_then(|n| n.as_ref())
                    .and_then(|n| n.last_child)
            };
            if let Some(last_child_id) = last_child_id {
                let mut inner = self.inner.borrow_mut();
                if let Some(Some(node)) = inner.nodes.get_mut(last_child_id.index()) {
                    if let NodeData::Text { contents } = &mut node.data {
                        contents.push_str(text);
                        // An empty text child turning non-empty flips :empty.
                        inner.match_dirty.parent_zone(parent_id);
                        inner.match_dirty.structure = true;
                        return;
                    }
                }
            }
        }

        let text_id = self.new_node(NodeData::Text {
            contents: text.to_string(),
        });
        self.append_child(parent_id, text_id);
    }
}

fn collect_text_inner(inner: &DomTreeInner, node_id: NodeId, buf: &mut String) {
    // Iterative pre-order walk on an explicit heap stack. The recursive form
    // overflowed the thread stack and aborted the process on deeply nested
    // trees; descendants() is iterative + capped for the same reason. A valid
    // subtree visits at most nodes.len() nodes, so exceeding that means the
    // graph is cyclic (prevented by the append_child / insert_before guards);
    // stop rather than spin forever.
    let max_steps = inner.nodes.len().saturating_add(16);
    let mut steps = 0usize;
    let mut stack = vec![node_id];

    while let Some(id) = stack.pop() {
        steps += 1;
        if steps > max_steps {
            eprintln!("diting: collect_text_inner cap hit - tree has a cycle");
            break;
        }

        let node = match inner.nodes.get(id.index()) {
            Some(Some(n)) => n,
            _ => continue,
        };

        match &node.data {
            NodeData::Text { contents } => buf.push_str(contents),
            // Comment and ProcessingInstruction are intentionally NOT
            // appended when traversing descendants: per spec, textContent
            // on an Element only includes Text descendants. Direct
            // textContent on a Comment/PI is handled by the caller.
            _ => {
                // Collect children, then push in reverse so they pop in
                // document order.
                let mut kids = Vec::new();
                let mut child = node.first_child;
                while let Some(child_id) = child {
                    kids.push(child_id);
                    if kids.len() > inner.nodes.len() {
                        eprintln!("diting: collect_text_inner sibling cap hit - cycle");
                        break;
                    }
                    child = inner
                        .nodes
                        .get(child_id.index())
                        .and_then(|n| n.as_ref())
                        .and_then(|n| n.next_sibling);
                }
                for child_id in kids.into_iter().rev() {
                    stack.push(child_id);
                }
            }
        }
    }
}
