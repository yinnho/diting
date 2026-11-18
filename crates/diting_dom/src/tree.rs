use diting_html5ever::{LocalName, Namespace, QualName};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;

use super::selector::{MatchCacheBox, MatchDirty};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(pub u32);

impl NodeId {
    pub fn new(val: u32) -> Self {
        NodeId(val)
    }

    pub fn index(self) -> usize {
        self.0 as usize
    }

    pub fn raw(self) -> u32 {
        self.0
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeId({})", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attribute {
    pub name: QualName,
    pub value: String,
}

impl Attribute {
    pub fn qualified_name_eq(&self, name: &str) -> bool {
        match &self.name.prefix {
            Some(prefix) => {
                name.len() == prefix.len() + self.name.local.len() + 1
                    && name.starts_with(prefix.as_str())
                    && name.as_bytes().get(prefix.len()) == Some(&b':')
                    && &name[prefix.len() + 1..] == self.name.local.as_str()
            }
            None => self.name.local.as_str() == name,
        }
    }
}

#[derive(Clone, Debug)]
pub enum NodeData {
    Document,
    Doctype {
        name: String,
        public_id: String,
        system_id: String,
    },
    Element {
        name: QualName,
        attrs: Vec<Attribute>,
        template_contents: Option<NodeId>,
        mathml_annotation_xml_integration_point: bool,
        /// A form control's dirty value (`el.value = x`) mirrored from the
        /// bootstrap's prototype setter. Deliberately NOT an attribute: it
        /// must stay invisible to `getAttribute`/`outerHTML`/CSS selectors
        /// (Chrome keeps the dirty value out of the DOM the same way), while
        /// the paint walk reads it to draw the text an input/textarea shows.
        live_value: Option<String>,
        /// A checkbox/radio's dirty checkedness (`el.checked = x`), same
        /// mirror posture as [`NodeData::Element::live_value`]: `None` means
        /// "never dirtied" and readers fall back to the parsed `checked`
        /// attribute — the exact precedence the JS getter resolves with.
        live_checked: Option<bool>,
        /// A checkbox's `indeterminate` IDL state (`el.indeterminate = x`),
        /// mirrored so `:indeterminate` can match; never an attribute.
        live_indeterminate: bool,
    },
    Text {
        contents: String,
    },
    Comment {
        contents: String,
    },
    ProcessingInstruction {
        target: String,
        data: String,
    },
}

#[derive(Clone, Debug)]
pub struct Node {
    pub parent: Option<NodeId>,
    pub first_child: Option<NodeId>,
    pub last_child: Option<NodeId>,
    pub prev_sibling: Option<NodeId>,
    pub next_sibling: Option<NodeId>,
    pub data: NodeData,
}

impl Node {
    pub fn is_document(&self) -> bool {
        matches!(self.data, NodeData::Document)
    }

    pub fn is_element(&self) -> bool {
        matches!(self.data, NodeData::Element { .. })
    }

    pub fn is_text(&self) -> bool {
        matches!(self.data, NodeData::Text { .. })
    }

    pub fn as_element(&self) -> Option<&QualName> {
        match &self.data {
            NodeData::Element { name, .. } => Some(name),
            _ => None,
        }
    }

    pub fn attrs(&self) -> Option<&[Attribute]> {
        match &self.data {
            NodeData::Element { attrs, .. } => Some(attrs),
            _ => None,
        }
    }

    pub fn get_attribute(&self, name: &str) -> Option<&str> {
        self.attrs()?.iter().find_map(|a| {
            if a.qualified_name_eq(name) {
                Some(a.value.as_str())
            } else {
                None
            }
        })
    }

    pub fn set_attribute(&mut self, name: &str, value: String) {
        if let NodeData::Element { attrs, .. } = &mut self.data {
            // Match by qualified name: a parsed namespaced attribute is stored
            // with a separate prefix (xlink:href -> prefix="xlink", local="href"),
            // so matching on local name alone would miss it and push a duplicate.
            if let Some(attr) = attrs.iter_mut().find(|a| a.qualified_name_eq(name)) {
                attr.value = value;
            } else {
                attrs.push(Attribute {
                    name: QualName::new(None, Namespace::default(), LocalName::from(name)),
                    value,
                });
            }
        }
    }

    pub fn live_value(&self) -> Option<&str> {
        match &self.data {
            NodeData::Element { live_value, .. } => live_value.as_deref(),
            _ => None,
        }
    }

    pub fn set_live_value(&mut self, value: String) {
        if let NodeData::Element { live_value, .. } = &mut self.data {
            *live_value = Some(value);
        }
    }

    pub fn live_checked(&self) -> Option<bool> {
        match &self.data {
            NodeData::Element { live_checked, .. } => *live_checked,
            _ => None,
        }
    }

    pub fn live_indeterminate(&self) -> bool {
        match &self.data {
            NodeData::Element {
                live_indeterminate, ..
            } => *live_indeterminate,
            _ => false,
        }
    }

    pub fn set_live_indeterminate(&mut self, v: bool) {
        if let NodeData::Element {
            live_indeterminate, ..
        } = &mut self.data
        {
            *live_indeterminate = v;
        }
    }

    pub fn set_live_checked(&mut self, checked: bool) {
        if let NodeData::Element { live_checked, .. } = &mut self.data {
            *live_checked = Some(checked);
        }
    }

    pub fn get_attribute_ns(&self, ns: &str, local: &str) -> Option<&str> {
        self.attrs()?.iter().find_map(|a| {
            if a.name.ns.as_str() == ns && a.name.local.as_str() == local {
                Some(a.value.as_str())
            } else {
                None
            }
        })
    }

    pub fn set_attribute_ns(&mut self, ns: &str, qualified: &str, value: String) {
        if let NodeData::Element { attrs, .. } = &mut self.data {
            let (prefix, local) = match qualified.split_once(':') {
                Some((p, l)) => (Some(diting_html5ever::Prefix::from(p)), l),
                None => (None, qualified),
            };
            if let Some(attr) = attrs
                .iter_mut()
                .find(|a| a.name.ns.as_str() == ns && a.name.local.as_str() == local)
            {
                attr.name.prefix = prefix;
                attr.value = value;
            } else {
                attrs.push(Attribute {
                    name: QualName::new(prefix, Namespace::from(ns), LocalName::from(local)),
                    value,
                });
            }
        }
    }

    pub fn remove_attribute_ns(&mut self, ns: &str, local: &str) {
        if let NodeData::Element { attrs, .. } = &mut self.data {
            attrs.retain(|a| !(a.name.ns.as_str() == ns && a.name.local.as_str() == local));
        }
    }

    pub fn text_content_of_text_node(&self) -> Option<&str> {
        match &self.data {
            NodeData::Text { contents } => Some(contents),
            _ => None,
        }
    }
}

pub struct DomTree {
    inner: RefCell<DomTreeInner>,
}

pub(crate) struct DomTreeInner {
    pub(crate) nodes: Vec<Option<Node>>,
    pub(crate) free_list: Vec<u32>,
    pub(crate) document: NodeId,
    pub(crate) id_index: HashMap<String, NodeId>,
    /// Shadow roots are arena nodes with their own child list. They are kept
    /// outside the ordinary parent links so light-tree traversal never crosses
    /// into a shadow tree by accident.
    pub(crate) shadow_roots: HashMap<NodeId, ShadowRoot>,
    shadow_roots_by_host: HashMap<NodeId, NodeId>,
    // Whether the document was parsed in (full) quirks mode; quirks makes CSS
    // class/id selectors ASCII-case-insensitive.
    pub(crate) quirks: bool,
    /// The document's focused element — see [`DomTree::focused_node`].
    focused_node: Option<NodeId>,
    /// The element under the pointer — see [`DomTree::hovered_node`] (#152).
    hovered_node: Option<NodeId>,
    /// The element mid-press (mousedown → mouseup window) — see
    /// [`DomTree::active_node`] (#152).
    active_node: Option<NodeId>,
    /// Decoded URL fragment that `:target` matches. Empty means the
    /// document has no fragment, so nothing is the target.
    css_target: String,
    /// Text selection in a text-entry control as (node, start, end) — see
    /// [`DomTree::selection`].
    selection: Option<(NodeId, usize, usize)>,
    /// Per-node element-scroller offsets (scrollTop/scrollLeft mirrors,
    /// sticky v2). Truth lives HERE, not in a JsState map: NodeId slots are
    /// recycled through `free_list`, so a side table that outlives its node
    /// would hand a new node a dead offset. Slot freeing drops the entry —
    /// the offset dies with its node, exactly like a real scroll container.
    scroll_offsets: HashMap<NodeId, [f32; 2]>,
    /// Bumped on every `set_node_scroll` write. Scroll must NOT invalidate
    /// the layout caches (it is read-time paint state, like the root
    /// offset), so consumers fingerprint their caches with this instead of
    /// the tree epoch.
    scroll_gen: u64,
    /// Bumped on every allocation/free. Layout caches fingerprint the tree
    /// with it ("tree shape changed"), so it must never repeat a value —
    /// see [`DomTree::epoch`].
    epoch_gen: u64,
    /// Structural/attribute mutations since the last incremental selector
    /// match sync — see [`super::selector::MatchDirty`]. Stamped by the
    /// mutation methods themselves so no caller can forget.
    match_dirty: MatchDirty,
    /// The persisted rule-match state for
    /// [`super::selector::MatchCacheBox`] incremental syncs (#111):
    /// compiled buckets + live hit sets for one selector list, valid while
    /// every mutation carried a stamp.
    match_cache: Option<MatchCacheBox>,
    /// The layout crate's cascade memo (opaque; see tree/caches.rs).
    style_memo: Option<Box<dyn std::any::Any>>,
}

// Persisted match/cascade caches: tree/caches.rs (god-file ratchet).
mod caches;
mod slots;
// Live interaction state (focus/hover/active/selection/scroll) lives in the
// state child module; split for the god-file ratchet (ARCHITECTURE.md §6).
mod state;

mod clone;
mod fragment;
mod id_index;
mod mutate;
mod shadow;
mod text;
mod walk;
pub use shadow::{AttachShadowError, ShadowRoot, ShadowRootMode};

impl DomTree {
    pub fn new() -> Self {
        let doc_node = Node {
            parent: None,
            first_child: None,
            last_child: None,
            prev_sibling: None,
            next_sibling: None,
            data: NodeData::Document,
        };
        DomTree {
            inner: RefCell::new(DomTreeInner {
                nodes: vec![Some(doc_node)],
                free_list: Vec::new(),
                document: NodeId(0),
                id_index: HashMap::new(),
                shadow_roots: HashMap::new(),
                shadow_roots_by_host: HashMap::new(),
                quirks: false,
                focused_node: None,
                hovered_node: None,
                active_node: None,
                css_target: String::new(),
                selection: None,
                scroll_offsets: HashMap::new(),
                scroll_gen: 0,
                epoch_gen: 0,
                match_dirty: MatchDirty::default(),
                match_cache: None,
                style_memo: None,
            }),
        }
    }

    pub fn document(&self) -> NodeId {
        self.inner.borrow().document
    }

    // Document generation stamp: bumps on every allocation/free — and on
    // focus/hover/active flips (tree/state.rs; style-carrying caches re-key
    // on it alone) — so a layout cache keyed to it invalidates when the tree
    // mutates. Not a mutation counter: attribute writes don't bump.
    // A monotonic counter, never a packing of (len, free): the packed form
    // collided whenever free_list crossed a 256 boundary without an
    // allocation (nodes.len() is unchanged by pure removals), handing
    // epoch-keyed caches the same stamp for a mutated tree (#124).
    pub fn epoch(&self) -> u64 {
        self.inner.borrow().epoch_gen
    }

    pub fn set_quirks(&self, quirks: bool) {
        let mut inner = self.inner.borrow_mut();
        inner.quirks = quirks;
        // The match buckets were (or will be) compiled under one quirks
        // spelling; a flip needs a rebuild.
        inner.match_dirty.full = true;
    }

    pub fn is_quirks(&self) -> bool {
        self.inner.borrow().quirks
    }

    // Number of node slots (live plus freed). A well-formed subtree has at
    // most this many nodes, so it is a safe ceiling for iterative walkers
    // that need a cycle backstop.
    pub fn node_slot_count(&self) -> usize {
        self.inner.borrow().nodes.len()
    }

    pub(crate) fn borrow_inner(&self) -> std::cell::Ref<'_, DomTreeInner> {
        self.inner.borrow()
    }

    pub fn new_node(&self, data: NodeData) -> NodeId {
        let mut inner = self.inner.borrow_mut();
        let id = if let Some(slot) = inner.free_list.pop() {
            NodeId(slot)
        } else {
            let idx = inner.nodes.len() as u32;
            inner.nodes.push(None);
            NodeId(idx)
        };

        if let NodeData::Element { ref attrs, .. } = data {
            if let Some(id_attr) = attrs.iter().find(|a| a.name.local.as_str() == "id") {
                // Keep the FIRST element created with a given id. Parse order is
                // document order, so getElementById / querySelector('#id') return
                // the first-in-tree-order element on duplicate ids, per spec.
                inner.id_index.entry(id_attr.value.clone()).or_insert(id);
            }
        }

        inner.nodes[id.index()] = Some(Node {
            parent: None,
            first_child: None,
            last_child: None,
            prev_sibling: None,
            next_sibling: None,
            data,
        });
        inner.epoch_gen = inner.epoch_gen.wrapping_add(1);
        id
    }

    pub fn get_node(&self, id: NodeId) -> Option<Node> {
        self.inner.borrow().nodes.get(id.index())?.clone()
    }

    pub fn with_node<F, R>(&self, id: NodeId, f: F) -> Option<R>
    where
        F: FnOnce(&Node) -> R,
    {
        let inner = self.inner.borrow();
        inner.nodes.get(id.index())?.as_ref().map(f)
    }

    pub fn with_node_mut<F, R>(&self, id: NodeId, f: F) -> Option<R>
    where
        F: FnOnce(&mut Node) -> R,
    {
        let mut inner = self.inner.borrow_mut();
        inner.nodes.get_mut(id.index())?.as_mut().map(f)
    }

    pub fn len(&self) -> usize {
        self.inner
            .borrow()
            .nodes
            .iter()
            .filter(|n| n.is_some())
            .count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() <= 1
    }
}

impl Default for DomTree {
    fn default() -> Self {
        Self::new()
    }
}
