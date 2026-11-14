//! querySelector / matches against one tree.
//!
//! A bare `#id` uses the id index. Everything else parses a selector and
//! walks descendants. The parser adapter stays in the parent module.

use diting_selectors::context::QuirksMode;
use diting_selectors::matching::{
    MatchingContext, MatchingForInvalidation, MatchingMode, NeedsSelectorFlags,
};
use diting_selectors::Element;

use super::{parse_selector, DitingSelector, DomElement};
use crate::tree::{DomTree, NodeId};

/// If `selector` is a bare ASCII id selector like `#main`, return the id (without
/// the `#`). Conservative: escapes, combinators, commas, whitespace, non-ASCII, or
/// a non-letter/underscore first character fall through to the full selector engine.
fn simple_id_selector(selector: &str) -> Option<&str> {
    let id = selector.trim().strip_prefix('#')?;
    let first = id.as_bytes().first().copied()?;
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return None;
    }
    if id
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        Some(id)
    } else {
        None
    }
}

impl DomTree {
    pub fn query_selector(&self, selector: &str) -> Result<Option<NodeId>, String> {
        self.query_selector_from(self.document(), selector)
    }

    pub fn query_selector_all(&self, selector: &str) -> Result<Vec<NodeId>, String> {
        self.query_selector_all_from(self.document(), selector)
    }

    pub fn query_selector_from(
        &self,
        root: NodeId,
        selector: &str,
    ) -> Result<Option<NodeId>, String> {
        // Fast path: a bare "#id" selector resolves through the id index in O(1)
        // instead of scanning every descendant. The index holds the first element
        // in tree order per id, which is exactly what the full scan would return.
        // In quirks mode `#id` matches ASCII-case-insensitively, but the id index
        // is keyed on the exact-case id, so skip the fast path and let the
        // selector engine do the case-insensitive match.
        if !self.is_quirks() {
            if let Some(id) = simple_id_selector(selector) {
                match self.get_element_by_id(id) {
                    // querySelector matches strict descendants of root only, so the
                    // indexed element must have root among its ancestors.
                    Some(nid) if self.ancestors(nid).contains(&root) => return Ok(Some(nid)),
                    // No element has this id at all: a bare id selector cannot match.
                    None => return Ok(None),
                    // Indexed (first) element is not under a scoped root; a later
                    // duplicate could still be a descendant, so fall through to scan.
                    Some(_) => {}
                }
            }
        }
        let selector_list = parse_selector(selector)?;
        let mut caches = diting_selectors::context::SelectorCaches::default();
        let mut context = MatchingContext::new(
            MatchingMode::Normal,
            None,
            &mut caches,
            self.selector_quirks_mode(),
            NeedsSelectorFlags::No,
            MatchingForInvalidation::No,
        );
        self.bind_query_scope(&mut context, root);

        for desc_id in self.descendants(root) {
            let is_element = self.with_node(desc_id, |n| n.is_element()).unwrap_or(false);
            if is_element {
                let element = DomElement::new(self, desc_id);
                if diting_selectors::matching::matches_selector_list(
                    &selector_list,
                    element,
                    &mut context,
                ) {
                    return Ok(Some(desc_id));
                }
            }
        }
        Ok(None)
    }

    // Map the document's quirks flag onto the selector crate's QuirksMode. In
    // quirks mode the crate matches class/id selectors ASCII-case-insensitively.
    pub(super) fn selector_quirks_mode(&self) -> QuirksMode {
        if self.is_quirks() {
            QuirksMode::Quirks
        } else {
            QuirksMode::NoQuirks
        }
    }

    // `:scope` in a query rooted at an element is that element (the DOM spec's
    // scoping node for querySelector/querySelectorAll). A document-rooted
    // query leaves the scope unset, where the selectors crate falls back to
    // the root element — which is exactly how browsers resolve
    // `document.querySelector(':scope ...')` (it means html).
    fn bind_query_scope(&self, context: &mut MatchingContext<'_, DitingSelector>, root: NodeId) {
        let is_element = self.with_node(root, |n| n.is_element()).unwrap_or(false);
        if is_element {
            context.scope_element = Some(DomElement::new(self, root).opaque());
        }
    }

    pub fn query_selector_all_from(
        &self,
        root: NodeId,
        selector: &str,
    ) -> Result<Vec<NodeId>, String> {
        let selector_list = parse_selector(selector)?;
        let mut caches = diting_selectors::context::SelectorCaches::default();
        let mut context = MatchingContext::new(
            MatchingMode::Normal,
            None,
            &mut caches,
            self.selector_quirks_mode(),
            NeedsSelectorFlags::No,
            MatchingForInvalidation::No,
        );
        self.bind_query_scope(&mut context, root);
        let mut results = Vec::new();

        for desc_id in self.descendants(root) {
            let is_element = self.with_node(desc_id, |n| n.is_element()).unwrap_or(false);
            if is_element {
                let element = DomElement::new(self, desc_id);
                if diting_selectors::matching::matches_selector_list(
                    &selector_list,
                    element,
                    &mut context,
                ) {
                    results.push(desc_id);
                }
            }
        }
        Ok(results)
    }

    /// Test one element as the selector subject, including when it is
    /// detached from the document (query_selector only walks the tree, so a
    /// detached subject was untestable before this entry point).
    pub fn matches_selector(&self, nid: NodeId, selector: &str) -> Result<bool, String> {
        if !self
            .with_node(nid, |node| node.is_element())
            .unwrap_or(false)
        {
            return Ok(false);
        }
        let selector_list = parse_selector(selector)?;
        let mut caches = diting_selectors::context::SelectorCaches::default();
        let mut context = MatchingContext::new(
            MatchingMode::Normal,
            None,
            &mut caches,
            self.selector_quirks_mode(),
            NeedsSelectorFlags::No,
            MatchingForInvalidation::No,
        );
        // Element.matches() scopes `:scope` to the element being tested —
        // `el.matches(':scope')` is true and is the idiom libraries use to
        // feature-detect :scope support.
        context.scope_element = Some(DomElement::new(self, nid).opaque());
        Ok(diting_selectors::matching::matches_selector_list(
            &selector_list,
            DomElement::new(self, nid),
            &mut context,
        ))
    }
}
