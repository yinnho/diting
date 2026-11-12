//! Rule-dependency collection for the incremental match (#111): which
//! parts of the tree a rule's match reads, split by WHERE in the selector
//! each class/id/attr is consumed — the sync's zone routing keys off these
//! side-split deps and flags. Split from match_index.rs (god-file ratchet,
//! ARCHITECTURE.md §6 P2).

use diting_selectors::parser::{self, SelectorList};

use super::DitingSelector;

/// What a rule's match can depend on besides the element's own tag, for
/// the rule-scoped invalidation pass. Collected over every component of
/// every sub-selector, nested lists included (`:not`/`:is`/`:where`/`:has`/
/// `:nth-child(of …)`), keys ASCII-lowercased (over-inclusion is the only
/// failure mode: an extra rule re-matches, a missing one would go stale).
///
/// The class/id/attr deps are split by WHERE in the selector they are
/// read, because that decides how far a write propagates:
/// - subject side (`classes`/`ids`/`attrs`): read on the element itself
///   (or a nested list evaluated against it). A write flips the rule at
///   the written element alone.
/// - descendant side (`*_desc`): left of a descendant/child combinator
///   (`.a .b`'s `.a`, `#p .a`'s `#p`). A write on E flips every subject
///   below E — the re-match needs E's subtree.
/// - sibling side (`*_sib`): left of `+`/`~`. A write on E flips the
///   matches of E's siblings and their subtrees — the re-match needs E's
///   parent's subtree.
///
/// Other flags:
/// - `state`: hover/active/focus pseudo-classes.
/// - `structural`: reads siblings or children (`+`/`~`, `:nth-*`,
///   `:first/last/only-*`, `:empty`, `:root`, `:has`) — the only way an
///   insert/remove/text edit can flip an element OUTSIDE the inserted or
///   moved subtree, whose own ancestors and attributes did not change.
///   Descendant/child combinators alone are not structural for that
///   reason; the moved subtree itself always re-probes against every rule.
/// - `wild`: anything not scoped above (form/link/dir/open pseudo-classes,
///   live control state, slots, `&`, invalid) — re-matches on every sync
///   that has an attr write or a live flip, inside the written element's
///   subtree (form/link pseudos read the element; `:dir`/`:read-only`
///   inherit downward). A pure structure/state window cannot flip them on
///   an untouched element — the inserted subtree re-probes in pass 1.
/// - `zone_unsafe`: matching crosses a boundary the zone walks cannot
///   see (slot assignment, shadow ::slotted/::part/:host, `&` nesting,
///   parse-invalid selectors, `:has`). Such a rule still re-matches on
///   any change, but always document-wide.
#[derive(Clone, Default)]
pub(super) struct RuleDeps {
    pub(super) classes: Vec<String>,
    pub(super) ids: Vec<String>,
    pub(super) attrs: Vec<String>,
    pub(super) classes_desc: Vec<String>,
    pub(super) ids_desc: Vec<String>,
    pub(super) attrs_desc: Vec<String>,
    pub(super) classes_sib: Vec<String>,
    pub(super) ids_sib: Vec<String>,
    pub(super) attrs_sib: Vec<String>,
    pub(super) state: bool,
    pub(super) structural: bool,
    pub(super) wild: bool,
    /// A wild pseudo that inherits from ancestors (`:dir`,
    /// `:read-only`): its re-match zone is the written element's subtree,
    /// not the element alone.
    pub(super) wild_inherited: bool,
    pub(super) zone_unsafe: bool,
}

/// Which part of the tree a compound reads — see [`RuleDeps`].
#[derive(Clone, Copy, PartialEq)]
enum Side {
    Subject,
    Descendant,
    Sibling,
}

impl RuleDeps {
    pub(super) fn add_selector(&mut self, sel: &parser::Selector<DitingSelector>) {
        self.add_selector_at(sel, Side::Subject);
    }

    /// Walk one selector's components in match order (right-to-left: the
    /// subject compound comes first — `iter()` would stop at the compound
    /// boundary, and every compound counts here). Each component's side is
    /// fixed by the combinator immediately right of it; a nested list
    /// starts from the side of the compound it sits in (`:not(.a .b)`
    /// reads `.b` on the element itself, `.a` on whatever the `:not`
    /// compound's leftward combinator points at).
    fn add_selector_at(&mut self, sel: &parser::Selector<DitingSelector>, side: Side) {
        let mut side = side;
        for c in sel.iter_raw_match_order() {
            self.add_component(c, side);
            if let parser::Component::Combinator(comb) = c {
                side = match comb {
                    parser::Combinator::NextSibling | parser::Combinator::LaterSibling => {
                        Side::Sibling
                    }
                    _ => Side::Descendant,
                };
            }
        }
    }

    fn add_list(&mut self, list: &SelectorList<DitingSelector>, side: Side) {
        for sel in list.slice() {
            self.add_selector_at(sel, side);
        }
    }

    fn add_component(&mut self, c: &parser::Component<DitingSelector>, side: Side) {
        use super::PseudoClass;
        use parser::{Combinator, Component};
        let (ids, classes, attrs) = match side {
            Side::Subject => (&mut self.ids, &mut self.classes, &mut self.attrs),
            Side::Descendant => (
                &mut self.ids_desc,
                &mut self.classes_desc,
                &mut self.attrs_desc,
            ),
            Side::Sibling => (
                &mut self.ids_sib,
                &mut self.classes_sib,
                &mut self.attrs_sib,
            ),
        };
        // Exhaustive on purpose: a new component kind must be classified
        // before it compiles.
        match c {
            Component::LocalName(_)
            | Component::ExplicitUniversalType
            | Component::ExplicitAnyNamespace
            | Component::ExplicitNoNamespace
            | Component::DefaultNamespace(_)
            | Component::Namespace(..)
            | Component::PseudoElement(_)
            | Component::RelativeSelectorAnchor => {}
            Component::ID(id) => ids.push(id.0.to_ascii_lowercase()),
            Component::Class(class) => classes.push(class.0.to_ascii_lowercase()),
            Component::AttributeInNoNamespaceExists {
                local_name_lower, ..
            } => attrs.push(local_name_lower.0.to_string().to_ascii_lowercase()),
            Component::AttributeInNoNamespace { local_name, .. } => {
                attrs.push(local_name.0.to_string().to_ascii_lowercase())
            }
            Component::AttributeOther(a) => {
                attrs.push(a.local_name_lower.0.to_string().to_ascii_lowercase())
            }
            Component::Combinator(comb) => match comb {
                Combinator::Child | Combinator::Descendant | Combinator::PseudoElement => {}
                Combinator::NextSibling | Combinator::LaterSibling => self.structural = true,
                Combinator::SlotAssignment | Combinator::Part => {
                    self.wild = true;
                    self.zone_unsafe = true;
                }
            },
            Component::Root | Component::Empty | Component::Nth(_) => self.structural = true,
            Component::NthOf(nth) => {
                self.structural = true;
                for sel in nth.selectors() {
                    // `:nth-child(2 of .a)` indexes the subject's SIBLINGS:
                    // writing `.a` on one of them reshuffles the count for
                    // every other, so the argument's deps are sibling-side.
                    self.add_selector_at(sel, Side::Sibling);
                }
            }
            Component::Has(rel) => {
                self.structural = true;
                // A :has argument matches elements OTHER than the subject,
                // so its flip propagates from the changed element up to any
                // ancestor and back DOWN that ancestor's combinators
                // (`section:has(.c) span` — the spans are nowhere near the
                // write). The attr-write zone cannot see that far: any rule
                // bearing :has re-matches document-wide.
                self.zone_unsafe = true;
                for r in rel.iter() {
                    self.add_selector_at(&r.selector, Side::Descendant);
                }
            }
            Component::Negation(list) | Component::Is(list) | Component::Where(list) => {
                self.add_list(list, side)
            }
            Component::NonTSPseudoClass(pc) => match pc {
                PseudoClass::Hover
                | PseudoClass::Active
                | PseudoClass::Focus
                | PseudoClass::FocusVisible
                | PseudoClass::FocusWithin => self.state = true,
                PseudoClass::Enabled
                | PseudoClass::Disabled
                | PseudoClass::Checked
                | PseudoClass::Indeterminate
                | PseudoClass::Link
                | PseudoClass::Visited
                | PseudoClass::Root
                | PseudoClass::Open => self.wild = true,
                // Read on the element but INHERITED from ancestors (`:dir`
                // directionality, `:read-only` editability): an attr write
                // on E can flip these on E's whole subtree, so they re-match
                // there, not at the element alone.
                PseudoClass::Dir { .. } | PseudoClass::ReadOnly | PseudoClass::ReadWrite => {
                    self.wild = true;
                    self.wild_inherited = true;
                }
            },
            Component::Scope
            | Component::ImplicitScope
            | Component::ParentSelector
            | Component::Slotted(_)
            | Component::Part(_)
            | Component::Host(_)
            | Component::Invalid(_) => {
                self.wild = true;
                self.zone_unsafe = true;
            }
        }
    }
}
