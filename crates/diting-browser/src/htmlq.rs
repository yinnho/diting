//! HTML query layer — parse a raw HTML string, select elements with CSS
//! selectors, read text/attrs/serialized HTML (#42 ②).
//!
//! Absorbs the `scraper` 0.29 faces our ten call sites actually use
//! (search engines ×8, `html_md`, `render`/`screenshot` collectors): a
//! hair over 500 lines against ego-tree + the vendored servo family,
//! replacing a 2.3k-line crate (plus getopts/indexmap riders). Provenance:
//! node/tree-sink/selector/element-impl shapes ported from scraper 0.29
//! (MIT OR Apache-2.0); matching runs on `diting_selectors` 0.42 —
//! by-value `Element` passing, which `ElementRef` (being `Copy`) fits
//! natively.
//!
//! Deliberate faces NOT ported: fragment parsing (`parse_fragment`),
//! `root_element`, document-level `html()`, `DoubleEndedIterator` on
//! `Select`, the `errors` collection, and pseudo-class parsing
//! (`:hover` et al. are parse errors here, same as scraper's default —
//! static documents have no interaction state to match against).

use std::cell::OnceCell;
use std::fmt;
use std::iter::FusedIterator;
use std::ops::Deref;

use diting_html5ever::tendril::StrTendril;
use diting_html5ever::tendril::TendrilSink;
use diting_html5ever::{
    Attribute, LocalName, Namespace, QualName, driver, expanded_name, local_name, ns, serialize,
    tree_builder::{NodeOrText, QuirksMode, TreeSink},
    serialize::{Serialize, SerializeOpts, Serializer, TraversalScope},
};
use diting_selectors::SelectorList;
use diting_selectors::attr::{AttrSelectorOperation, CaseSensitivity, NamespaceConstraint};
use diting_selectors::bloom::BloomFilter;
use diting_selectors::context::SelectorCaches;
use diting_selectors::matching::{
    self, ElementSelectorFlags, MatchingContext, MatchingForInvalidation, MatchingMode,
    NeedsSelectorFlags,
};
use diting_selectors::parser::{self, ParseRelative, SelectorParseErrorKind};
use diting_selectors::OpaqueElement;
use ego_tree::NodeRef;
use ego_tree::iter::{Edge, Traverse};
use ego_tree::{NodeId, Tree};
use precomputed_hash::PrecomputedHash;

pub use diting_cssparser::ToCss;

/// An HTML node. The query-layer mirror of the engine DOM's `NodeData` —
/// owned, inert, cheap; a search-results page parses in one pass and drops.
#[allow(variant_size_differences)]
#[derive(Clone, PartialEq, Eq)]
pub enum Node {
    /// The document root.
    Document,
    /// The fragment root (template contents).
    Fragment,
    /// A doctype.
    Doctype(Doctype),
    /// A comment.
    Comment(Comment),
    /// Text.
    Text(Text),
    /// An element.
    Element(Element),
    /// A processing instruction.
    ProcessingInstruction(ProcessingInstruction),
}

impl Node {
    /// Returns true if node is the document root.
    pub fn is_document(&self) -> bool {
        matches!(*self, Node::Document)
    }

    /// Returns true if node is text.
    pub fn is_text(&self) -> bool {
        matches!(*self, Node::Text(_))
    }

    /// Returns true if node is an element.
    pub fn is_element(&self) -> bool {
        matches!(*self, Node::Element(_))
    }

    /// Returns self as text.
    #[cfg(test)]
    pub fn as_text(&self) -> Option<&Text> {
        match *self {
            Node::Text(ref t) => Some(t),
            _ => None,
        }
    }

    /// Returns self as an element.
    pub fn as_element(&self) -> Option<&Element> {
        match *self {
            Node::Element(ref e) => Some(e),
            _ => None,
        }
    }
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Node::Document => write!(f, "Document"),
            Node::Fragment => write!(f, "Fragment"),
            Node::Doctype(ref d) => write!(f, "Doctype({d:?})"),
            Node::Comment(ref c) => write!(f, "Comment({c:?})"),
            Node::Text(ref t) => write!(f, "Text({t:?})"),
            Node::Element(ref e) => write!(f, "Element({e:?})"),
            Node::ProcessingInstruction(ref pi) => write!(f, "ProcessingInstruction({pi:?})"),
        }
    }
}

/// A doctype.
#[derive(Clone, PartialEq, Eq)]
pub struct Doctype {
    /// The doctype name.
    pub name: StrTendril,
    /// The doctype public ID.
    pub public_id: StrTendril,
    /// The doctype system ID.
    pub system_id: StrTendril,
}

impl fmt::Debug for Doctype {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "<!DOCTYPE {} PUBLIC {:?} {:?}>",
            self.name.deref(),
            self.public_id.deref(),
            self.system_id.deref()
        )
    }
}

/// An HTML comment; derefs to its text.
#[derive(Clone, PartialEq, Eq)]
pub struct Comment {
    /// The comment text.
    pub comment: StrTendril,
}

impl Deref for Comment {
    type Target = str;

    fn deref(&self) -> &str {
        self.comment.deref()
    }
}

impl fmt::Debug for Comment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<!-- {:?} -->", self.deref())
    }
}

/// HTML text; derefs to the string.
#[derive(Clone, PartialEq, Eq)]
pub struct Text {
    /// The text.
    pub text: StrTendril,
}

impl Deref for Text {
    type Target = str;

    fn deref(&self) -> &str {
        self.text.deref()
    }
}

impl fmt::Debug for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.deref())
    }
}

/// HTML processing instruction; derefs to the data.
#[derive(Clone, PartialEq, Eq)]
pub struct ProcessingInstruction {
    /// The PI target.
    pub target: StrTendril,
    /// The PI data.
    pub data: StrTendril,
}

impl Deref for ProcessingInstruction {
    type Target = str;

    fn deref(&self) -> &str {
        self.data.deref()
    }
}

impl fmt::Debug for ProcessingInstruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.deref())
    }
}

/// An HTML element — the data stored in a [`Node::Element`] node. Distinct
/// from the `selectors::Element` trait, which [`ElementRef`] implements for
/// matching; call [`ElementRef::value`] to reach this.
#[derive(Clone, PartialEq, Eq)]
pub struct Element {
    /// The element name.
    pub name: QualName,

    /// Attributes, sorted by `QualName` (parser order is not preserved;
    /// lookups binary-search, mirroring scraper's non-deterministic face).
    pub attrs: Vec<(QualName, StrTendril)>,

    id: OnceCell<Option<StrTendril>>,

    classes: OnceCell<Box<[LocalName]>>,
}

impl Element {
    fn new(name: QualName, attributes: Vec<Attribute>) -> Self {
        let mut attrs: Vec<(QualName, StrTendril)> =
            attributes.into_iter().map(|attr| (attr.name, attr.value)).collect();
        attrs.sort_unstable_by(|lhs, rhs| lhs.0.cmp(&rhs.0));
        Element {
            attrs,
            name,
            id: OnceCell::new(),
            classes: OnceCell::new(),
        }
    }

    /// Returns the element name (local name, lowercased for HTML).
    pub fn name(&self) -> &str {
        self.name.local.deref()
    }

    /// Returns the element ID.
    pub fn id(&self) -> Option<&str> {
        self.id
            .get_or_init(|| {
                self.attrs
                    .iter()
                    .find(|(name, _)| name.local.as_str() == "id")
                    .map(|(_, value)| value.clone())
            })
            .as_deref()
    }

    /// Returns true if element has the class.
    pub fn has_class(&self, class: &str, case_sensitive: CaseSensitivity) -> bool {
        self.classes()
            .any(|c| case_sensitive.eq(c.as_bytes(), class.as_bytes()))
    }

    /// Returns an iterator over the element's classes.
    pub fn classes(&self) -> Classes<'_> {
        let classes = self.classes.get_or_init(|| {
            let mut classes: Vec<LocalName> = self
                .attrs
                .iter()
                .filter(|(name, _)| name.local.as_str() == "class")
                .flat_map(|(_, value)| value.split_ascii_whitespace().map(LocalName::from))
                .collect();
            classes.sort_unstable();
            classes.dedup();
            classes.into_boxed_slice()
        });
        Classes {
            inner: classes.iter(),
        }
    }

    /// Returns the value of an attribute (matched on local name, any namespace).
    pub fn attr(&self, attr: &str) -> Option<&str> {
        let qualname = QualName::new(None, ns!(), LocalName::from(attr));
        self.attrs
            .binary_search_by(|(name, _)| name.cmp(&qualname))
            .ok()
            .map(|idx| &*self.attrs[idx].1)
    }
}

impl fmt::Debug for Element {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{}", self.name())?;
        for (key, value) in self.attrs.iter() {
            write!(f, " {}={value:?}", key.local.deref())?;
        }
        write!(f, ">")
    }
}

/// Iterator over classes.
#[derive(Clone)]
pub struct Classes<'a> {
    inner: std::slice::Iter<'a, LocalName>,
}

impl<'a> Iterator for Classes<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        self.inner.next().map(Deref::deref)
    }
}

/// An HTML document parsed into an owned ego-tree. Parsing never fails
/// hard: quirks land in `quirks_mode`, the tree fills as best it can —
/// same contract as scraper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Html {
    /// The quirks mode the parser settled on.
    pub quirks_mode: QuirksMode,

    /// The node tree.
    pub tree: Tree<Node>,
}

impl Html {
    /// Creates an empty HTML document.
    fn new_document() -> Self {
        Html {
            quirks_mode: QuirksMode::NoQuirks,
            tree: Tree::new(Node::Document),
        }
    }

    /// Parses a string of HTML as a document.
    pub fn parse_document(document: &str) -> Self {
        let parser =
            driver::parse_document(HtmlTreeSink::new(Self::new_document()), Default::default());
        parser.one(document)
    }

    /// Returns an iterator over elements matching a selector.
    pub fn select<'a, 'b>(&'a self, selector: &'b Selector) -> Select<'a, 'b> {
        Select {
            // Traverse from the document root: the root itself is not an
            // element, so ElementRef::wrap filters it — same node order as
            // ego-tree's flat nodes() walk.
            inner: self.tree.root().traverse(),
            selector,
            caches: Default::default(),
        }
    }
}

/// Wraps `Html` as a sink to drive parsing (scraper's tree_sink.rs shape).
#[derive(Debug)]
pub struct HtmlTreeSink(pub std::cell::RefCell<Html>);

impl HtmlTreeSink {
    /// Wrap an `Html` instance as a sink to drive parsing.
    pub fn new(html: Html) -> Self {
        Self(std::cell::RefCell::new(html))
    }
}

impl TreeSink for HtmlTreeSink {
    type Output = Html;
    type Handle = NodeId;
    type ElemName<'a> = std::cell::Ref<'a, QualName>;

    fn finish(self) -> Html {
        self.0.into_inner()
    }

    fn parse_error(&self, _msg: std::borrow::Cow<'static, str>) {}

    fn set_quirks_mode(&self, mode: QuirksMode) {
        self.0.borrow_mut().quirks_mode = mode;
    }

    fn get_document(&self) -> Self::Handle {
        self.0.borrow().tree.root().id()
    }

    fn same_node(&self, x: &Self::Handle, y: &Self::Handle) -> bool {
        x == y
    }

    fn elem_name<'a>(&'a self, target: &Self::Handle) -> Self::ElemName<'a> {
        std::cell::Ref::map(self.0.borrow(), |this| {
            &this
                .tree
                .get(*target)
                .unwrap()
                .value()
                .as_element()
                .unwrap()
                .name
        })
    }

    fn create_element(
        &self,
        name: QualName,
        attrs: Vec<Attribute>,
        _flags: diting_html5ever::tree_builder::ElementFlags,
    ) -> Self::Handle {
        let fragment = name.expanded() == expanded_name!(html "template");

        let mut this = self.0.borrow_mut();
        let mut node = this.tree.orphan(Node::Element(Element::new(name, attrs)));

        if fragment {
            node.append(Node::Fragment);
        }

        node.id()
    }

    fn create_comment(&self, text: StrTendril) -> Self::Handle {
        self.0
            .borrow_mut()
            .tree
            .orphan(Node::Comment(Comment { comment: text }))
            .id()
    }

    fn append_doctype_to_document(
        &self,
        name: StrTendril,
        public_id: StrTendril,
        system_id: StrTendril,
    ) {
        let doctype = Doctype {
            name,
            public_id,
            system_id,
        };
        self.0
            .borrow_mut()
            .tree
            .root_mut()
            .append(Node::Doctype(doctype));
    }

    fn append(&self, parent: &Self::Handle, child: NodeOrText<Self::Handle>) {
        let mut this = self.0.borrow_mut();
        let mut parent = this.tree.get_mut(*parent).unwrap();

        match child {
            NodeOrText::AppendNode(id) => {
                parent.append_id(id);
            }

            NodeOrText::AppendText(text) => {
                // Adjacent text nodes concatenate — the tree builder feeds
                // text in arbitrary chunk boundaries.
                let did_concat = parent.last_child().is_some_and(|mut n| match n.value() {
                    Node::Text(t) => {
                        t.text.push_tendril(&text);
                        true
                    }
                    _ => false,
                });

                if !did_concat {
                    parent.append(Node::Text(Text { text }));
                }
            }
        }
    }

    fn append_before_sibling(&self, sibling: &Self::Handle, new_node: NodeOrText<Self::Handle>) {
        let mut this = self.0.borrow_mut();

        if let NodeOrText::AppendNode(id) = new_node {
            this.tree.get_mut(id).unwrap().detach();
        }

        let mut sibling = this.tree.get_mut(*sibling).unwrap();
        if sibling.parent().is_some() {
            match new_node {
                NodeOrText::AppendNode(id) => {
                    sibling.insert_id_before(id);
                }

                NodeOrText::AppendText(text) => {
                    let did_concat =
                        sibling.prev_sibling().is_some_and(|mut n| match n.value() {
                            Node::Text(t) => {
                                t.text.push_tendril(&text);
                                true
                            }
                            _ => false,
                        });

                    if !did_concat {
                        sibling.insert_before(Node::Text(Text { text }));
                    }
                }
            }
        }
    }

    fn remove_from_parent(&self, target: &Self::Handle) {
        self.0.borrow_mut().tree.get_mut(*target).unwrap().detach();
    }

    fn reparent_children(&self, node: &Self::Handle, new_parent: &Self::Handle) {
        self.0
            .borrow_mut()
            .tree
            .get_mut(*new_parent)
            .unwrap()
            .reparent_from_id_append(*node);
    }

    fn add_attrs_if_missing(&self, target: &Self::Handle, attrs: Vec<Attribute>) {
        let mut this = self.0.borrow_mut();
        let mut node = this.tree.get_mut(*target).unwrap();
        let element = match *node.value() {
            Node::Element(ref mut e) => e,
            _ => unreachable!(),
        };

        for attr in attrs {
            if let Err(idx) = element.attrs.binary_search_by(|(name, _)| name.cmp(&attr.name)) {
                element.attrs.insert(idx, (attr.name, attr.value));
            }
        }
    }

    fn get_template_contents(&self, target: &Self::Handle) -> Self::Handle {
        self.0
            .borrow()
            .tree
            .get(*target)
            .unwrap()
            .first_child()
            .unwrap()
            .id()
    }

    fn mark_script_already_started(&self, _node: &Self::Handle) {}

    fn create_pi(&self, target: StrTendril, data: StrTendril) -> Self::Handle {
        self.0
            .borrow_mut()
            .tree
            .orphan(Node::ProcessingInstruction(ProcessingInstruction {
                target,
                data,
            }))
            .id()
    }

    fn append_based_on_parent_node(
        &self,
        element: &Self::Handle,
        prev_element: &Self::Handle,
        child: NodeOrText<Self::Handle>,
    ) {
        let has_parent = self
            .0
            .borrow()
            .tree
            .get(*element)
            .unwrap()
            .parent()
            .is_some();

        if has_parent {
            self.append_before_sibling(element, child)
        } else {
            self.append(prev_element, child)
        }
    }
}

/// Wrapper around a reference to an element node. Derefs to the underlying
/// `NodeRef`, so tree navigation (`parent`, `children`, `ancestors`, …)
/// comes from ego-tree; this wrapper adds the selectors matching face.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ElementRef<'a> {
    node: NodeRef<'a, Node>,
}

impl<'a> ElementRef<'a> {
    fn new(node: NodeRef<'a, Node>) -> Self {
        ElementRef { node }
    }

    /// Wraps a `NodeRef` only if it references a `Node::Element`.
    pub fn wrap(node: NodeRef<'a, Node>) -> Option<Self> {
        if node.value().is_element() {
            Some(ElementRef::new(node))
        } else {
            None
        }
    }

    /// Returns the `Element` referenced by `self`.
    pub fn value(&self) -> &'a Element {
        self.node.value().as_element().unwrap()
    }

    /// Returns an iterator over descendent elements matching a selector.
    pub fn select<'b>(&self, selector: &'b Selector) -> Select<'a, 'b> {
        let mut inner = self.traverse();
        inner.next(); // Skip Edge::Open(self).

        Select {
            inner,
            selector,
            caches: Default::default(),
        }
    }

    fn serialize_node(&self, traversal_scope: TraversalScope) -> String {
        let opts = SerializeOpts {
            scripting_enabled: false,
            traversal_scope,
            create_missing_parent: false,
        };
        let mut buf = Vec::new();
        serialize(&mut buf, self, opts).unwrap();
        String::from_utf8(buf).unwrap()
    }

    /// Returns the HTML of this element (outer).
    pub fn html(&self) -> String {
        self.serialize_node(TraversalScope::IncludeNode)
    }

    /// Returns the inner HTML of this element.
    #[cfg(test)]
    pub fn inner_html(&self) -> String {
        self.serialize_node(TraversalScope::ChildrenOnly(None))
    }

    /// Returns an iterator over descendent text nodes.
    pub fn text(&self) -> TextIter<'a> {
        TextIter {
            inner: self.traverse(),
        }
    }

    /// Iterate over all child nodes which are elements.
    #[cfg_attr(not(feature = "screenshot"), allow(dead_code))] // screenshot.rs <picture> walk
    pub fn child_elements(&self) -> impl Iterator<Item = ElementRef<'a>> + use<'a> {
        self.children().filter_map(ElementRef::wrap)
    }
}

impl fmt::Debug for ElementRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.value(), f)
    }
}

impl<'a> Deref for ElementRef<'a> {
    type Target = NodeRef<'a, Node>;

    fn deref(&self) -> &NodeRef<'a, Node> {
        &self.node
    }
}

/// Iterator over descendent elements matching a selector.
pub struct Select<'a, 'b> {
    inner: Traverse<'a, Node>,
    selector: &'b Selector,
    caches: SelectorCaches,
}

impl<'a> Iterator for Select<'a, '_> {
    type Item = ElementRef<'a>;

    fn next(&mut self) -> Option<ElementRef<'a>> {
        for edge in &mut self.inner {
            if let Edge::Open(node) = edge {
                if let Some(element) = ElementRef::wrap(node) {
                    if element.parent().is_some()
                        && self
                            .selector
                            .matches_with_scope_and_cache(element, None, &mut self.caches)
                    {
                        return Some(element);
                    }
                }
            }
        }
        None
    }
}

impl FusedIterator for Select<'_, '_> {}

/// Iterator over descendent text nodes.
#[derive(Debug, Clone)]
pub struct TextIter<'a> {
    inner: Traverse<'a, Node>,
}

impl<'a> Iterator for TextIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        for edge in &mut self.inner {
            if let Edge::Open(node) = edge {
                if let Node::Text(text) = node.value() {
                    return Some(&**text);
                }
            }
        }
        None
    }
}

impl FusedIterator for TextIter<'_> {}

/// Wrapper around a CSS selector group (comma-separated list).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selector {
    selectors: SelectorList<Simple>,
}

impl Selector {
    /// Parses a CSS selector group.
    pub fn parse(selectors: &str) -> Result<Self, String> {
        let mut parser = diting_cssparser::Parser::new(selectors);

        SelectorList::parse(&QParser, &mut parser, ParseRelative::No)
            .map(|selectors| Self { selectors })
            .map_err(|e| format!("{e:?}"))
    }

    fn matches_with_scope_and_cache(
        &self,
        element: ElementRef<'_>,
        scope: Option<ElementRef<'_>>,
        caches: &mut SelectorCaches,
    ) -> bool {
        let mut context = MatchingContext::new(
            MatchingMode::Normal,
            None,
            caches,
            diting_selectors::context::QuirksMode::NoQuirks,
            NeedsSelectorFlags::No,
            MatchingForInvalidation::No,
        );
        context.scope_element = scope.map(|x| diting_selectors::Element::opaque(&x));
        self.selectors
            .slice()
            .iter()
            .any(|s| matching::matches_selector(s, 0, None, element, &mut context))
    }
}

/// Parser glue for `diting_selectors` — enables `:is`/`:where`/`:has`
/// parsing (matching is built into the crate), rejects everything else.
#[derive(Clone, Copy, Debug)]
struct QParser;

impl parser::Parser<'_> for QParser {
    type Impl = Simple;
    type Error = SelectorParseErrorKind;

    fn parse_is_and_where(&self) -> bool {
        true
    }

    fn parse_has(&self) -> bool {
        true
    }
}

/// A `SelectorImpl` with no pseudo-classes or pseudo-elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Simple;

impl parser::SelectorImpl for Simple {
    type AttrValue = CssString;
    type Identifier = CssLocalName;
    type LocalName = CssLocalName;
    type NamespacePrefix = CssLocalName;
    type NamespaceUrl = Namespace;
    type BorrowedNamespaceUrl = Namespace;
    type BorrowedLocalName = CssLocalName;

    type NonTSPseudoClass = NonTSPseudoClass;
    type PseudoElement = PseudoElement;

    // see: https://github.com/servo/servo/pull/19747#issuecomment-357106065
    type ExtraMatchingData<'a> = ();
}

/// Wraps [`String`] for `diting_selectors` (identifier/attr-value face).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CssString(pub String);

impl<'a> From<&'a str> for CssString {
    fn from(val: &'a str) -> Self {
        Self(val.to_owned())
    }
}

impl AsRef<str> for CssString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl ToCss for CssString {
    fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
        diting_cssparser::serialize_string(&self.0, dest)
    }
}

/// Wraps [`LocalName`] for `diting_selectors`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CssLocalName(pub LocalName);

impl<'a> From<&'a str> for CssLocalName {
    fn from(val: &'a str) -> Self {
        Self(val.into())
    }
}

impl ToCss for CssLocalName {
    fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
        dest.write_str(&self.0)
    }
}

impl PrecomputedHash for CssLocalName {
    fn precomputed_hash(&self) -> u32 {
        self.0.precomputed_hash()
    }
}

/// Non tree-structural pseudo-class — empty: static documents have no
/// interaction state, so `:hover` and friends fail to parse, exactly like
/// scraper's default face.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonTSPseudoClass {}

impl parser::NonTSPseudoClass for NonTSPseudoClass {
    fn is_active_or_hover(&self) -> bool {
        false
    }

    fn is_user_action_state(&self) -> bool {
        false
    }
}

impl ToCss for NonTSPseudoClass {
    fn to_css<W: fmt::Write>(&self, _dest: &mut W) -> fmt::Result {
        Ok(())
    }
}

/// CSS pseudo-element — empty, same reasoning as [`NonTSPseudoClass`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PseudoElement {}

impl parser::PseudoElement for PseudoElement {}

impl ToCss for PseudoElement {
    fn to_css<W: fmt::Write>(&self, _dest: &mut W) -> fmt::Result {
        Ok(())
    }
}

/// `selectors::Element` for `ElementRef`. Will never match non-tree-structure
/// pseudo-classes (there are none to parse).
impl diting_selectors::Element for ElementRef<'_> {
    type Impl = Simple;

    fn opaque(&self) -> OpaqueElement {
        OpaqueElement::new(self.node.value())
    }

    fn parent_element(&self) -> Option<Self> {
        self.parent().and_then(ElementRef::wrap)
    }

    fn parent_node_is_shadow_root(&self) -> bool {
        false
    }

    fn containing_shadow_host(&self) -> Option<Self> {
        None
    }

    fn is_pseudo_element(&self) -> bool {
        false
    }

    fn is_part(&self, _name: &CssLocalName) -> bool {
        false
    }

    fn is_same_type(&self, other: &Self) -> bool {
        self.value().name == other.value().name
    }

    fn imported_part(&self, _: &CssLocalName) -> Option<CssLocalName> {
        None
    }

    fn prev_sibling_element(&self) -> Option<Self> {
        self.prev_siblings()
            .find(|sibling| sibling.value().is_element())
            .map(ElementRef::new)
    }

    fn next_sibling_element(&self) -> Option<Self> {
        self.next_siblings()
            .find(|sibling| sibling.value().is_element())
            .map(ElementRef::new)
    }

    fn first_element_child(&self) -> Option<Self> {
        self.children()
            .find(|child| child.value().is_element())
            .map(ElementRef::new)
    }

    fn is_html_element_in_html_document(&self) -> bool {
        self.value().name.ns == ns!(html)
    }

    fn has_local_name(&self, name: &CssLocalName) -> bool {
        self.value().name.local == name.0
    }

    fn has_namespace(&self, namespace: &Namespace) -> bool {
        &self.value().name.ns == namespace
    }

    fn attr_matches(
        &self,
        ns: &NamespaceConstraint<&Namespace>,
        local_name: &CssLocalName,
        operation: &AttrSelectorOperation<&CssString>,
    ) -> bool {
        self.value().attrs.iter().any(|(key, value)| {
            !matches!(*ns, NamespaceConstraint::Specific(url) if *url != key.ns)
                && local_name.0 == key.local
                && operation.eval_str(value)
        })
    }

    fn match_non_ts_pseudo_class(
        &self,
        _pc: &NonTSPseudoClass,
        _context: &mut MatchingContext<'_, Self::Impl>,
    ) -> bool {
        false
    }

    fn match_pseudo_element(
        &self,
        _pe: &PseudoElement,
        _context: &mut MatchingContext<Self::Impl>,
    ) -> bool {
        false
    }

    fn is_link(&self) -> bool {
        self.value().name() == "link"
    }

    fn is_html_slot_element(&self) -> bool {
        true
    }

    fn has_id(&self, id: &CssLocalName, case_sensitivity: CaseSensitivity) -> bool {
        match self.value().id() {
            Some(val) => case_sensitivity.eq(id.0.as_bytes(), val.as_bytes()),
            None => false,
        }
    }

    fn has_class(&self, name: &CssLocalName, case_sensitivity: CaseSensitivity) -> bool {
        self.value().has_class(&name.0, case_sensitivity)
    }

    fn has_custom_state(&self, _name: &CssLocalName) -> bool {
        false
    }

    fn is_empty(&self) -> bool {
        !self
            .children()
            .any(|child| child.value().is_element() || child.value().is_text())
    }

    fn is_root(&self) -> bool {
        self.parent()
            .is_some_and(|parent| parent.value().is_document())
    }

    fn apply_selector_flags(&self, _flags: ElementSelectorFlags) {}

    fn add_element_unique_hashes(&self, _filter: &mut BloomFilter) -> bool {
        false
    }
}

/// Serialize an HTML node subtree via the html5ever serializer
/// (scraper's node/serializable.rs shape).
fn serialize_node_subtree<S: Serializer>(
    self_node: NodeRef<'_, Node>,
    serializer: &mut S,
    traversal_scope: TraversalScope,
) -> Result<(), std::io::Error> {
    for edge in self_node.traverse() {
        match edge {
            Edge::Open(node) => {
                if node == self_node && traversal_scope == TraversalScope::ChildrenOnly(None) {
                    continue;
                }

                match *node.value() {
                    Node::Doctype(ref doctype) => {
                        serializer.write_doctype(doctype.name.deref())?;
                    }
                    Node::Comment(ref comment) => {
                        serializer.write_comment(comment)?;
                    }
                    Node::Text(ref text) => {
                        serializer.write_text(text)?;
                    }
                    Node::Element(ref elem) => {
                        let attrs = elem.attrs.iter().map(|(k, v)| (k, &v[..]));
                        serializer.start_elem(elem.name.clone(), attrs)?;
                    }
                    _ => (),
                }
            }

            Edge::Close(node) => {
                if node == self_node && traversal_scope == TraversalScope::ChildrenOnly(None) {
                    continue;
                }

                if let Some(elem) = node.value().as_element() {
                    serializer.end_elem(elem.name.clone())?;
                }
            }
        }
    }

    Ok(())
}

impl Serialize for ElementRef<'_> {
    fn serialize<S: Serializer>(
        &self,
        serializer: &mut S,
        traversal_scope: TraversalScope,
    ) -> Result<(), std::io::Error> {
        serialize_node_subtree(**self, serializer, traversal_scope)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_select_text_attr() {
        let html = r#"<!DOCTYPE html><html><body><div id="a" class="x y"><p>one</p></div><div class="x">two <b>bold</b></div></body></html>"#;
        let doc = Html::parse_document(html);
        let sel = Selector::parse("div.x").unwrap();
        let texts: Vec<String> = doc
            .select(&sel)
            .map(|el| el.text().collect::<String>())
            .collect();
        assert_eq!(texts, vec!["one", "two bold"]);

        let a = doc.select(&Selector::parse("#a").unwrap()).next().unwrap();
        assert_eq!(a.value().attr("id"), Some("a"));
        assert!(a.value().has_class("y", CaseSensitivity::CaseSensitive));
    }

    #[test]
    fn scoped_select_under_element() {
        let html = r#"<ol><li class="item"><a href="/1">first</a></li><li><a href="/2">second</a></li></ol>"#;
        let doc = Html::parse_document(html);
        let item = doc.select(&Selector::parse("li.item").unwrap()).next().unwrap();
        let link = item.select(&Selector::parse("a").unwrap()).next().unwrap();
        assert_eq!(link.value().attr("href"), Some("/1"));
        assert_eq!(link.text().collect::<String>(), "first");
    }

    #[test]
    fn ancestors_and_classes() {
        let html = r#"<div class="result"><div class="c-container"><h3><a>x</a></h3></div></div>"#;
        let doc = Html::parse_document(html);
        let link = doc.select(&Selector::parse("h3 a").unwrap()).next().unwrap();
        let hit = link
            .ancestors()
            .find(|a| {
                a.value()
                    .as_element()
                    .is_some_and(|el| el.classes().any(|c| c == "result" || c == "c-container"))
            })
            .unwrap();
        assert_eq!(hit.value().as_element().unwrap().name(), "div");
    }

    #[test]
    fn outer_html_serialization() {
        let html = r#"<div><p class="q">hello &amp; <em>bye</em></p></div>"#;
        let doc = Html::parse_document(html);
        let p = doc.select(&Selector::parse("p.q").unwrap()).next().unwrap();
        assert_eq!(p.html(), r#"<p class="q">hello &amp; <em>bye</em></p>"#);
        assert_eq!(p.inner_html(), r#"hello &amp; <em>bye</em>"#);
    }

    #[test]
    fn adjacent_text_chunks_concat() {
        // Parser feed boundaries land as separate AppendText calls; the sink
        // must merge them into one text node.
        let doc = Html::parse_document("<p>hello world</p>");
        let p = doc.select(&Selector::parse("p").unwrap()).next().unwrap();
        let text_nodes: Vec<&str> = p.children().filter_map(|c| c.value().as_text().map(|t| &**t)).collect();
        assert_eq!(text_nodes, vec!["hello world"]);
    }

    #[test]
    fn pseudo_class_is_parse_error() {
        assert!(Selector::parse("a:hover").is_err());
        assert!(Selector::parse("a[href]").is_ok());
        assert!(Selector::parse("div > span, a.title").is_ok());
    }

    #[test]
    fn attr_substring_selector() {
        // The search faces lean on [class*='...'] substring matching.
        let html = r#"<div class="summary-text-bold">s</div><div class="other">t</div>"#;
        let doc = Html::parse_document(html);
        let sel = Selector::parse("div[class*='summary-text']").unwrap();
        let hits: Vec<String> = doc
            .select(&sel)
            .map(|el| el.text().collect::<String>().trim().to_string())
            .collect();
        assert_eq!(hits, vec!["s"]);
    }
}
