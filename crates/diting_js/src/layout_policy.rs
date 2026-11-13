//! Whether a page builds a box tree.
//!
//! `Mock` is the default. Styles still cascade, so `getComputedStyle` and
//! `innerText` see stylesheet `display`. Geometry (`getBoundingClientRect`,
//! hit testing) stays on the synthetic grid, and paint finds no boxes.
//! `OnDemand` is the interactive and print face: a geometry read or a band
//! paint builds the box tree and reuses it until the tree changes.

/// One page's layout posture. Copied onto each realm at `init_js`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LayoutPolicy {
    /// Cascade only. No box tree.
    #[default]
    Mock,
    /// Build a box tree when geometry or paint asks.
    OnDemand,
}

impl LayoutPolicy {
    pub const fn uses_real_layout(self) -> bool {
        matches!(self, Self::OnDemand)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_policy_defaults_to_mock() {
        assert_eq!(LayoutPolicy::default(), LayoutPolicy::Mock);
        assert!(!LayoutPolicy::Mock.uses_real_layout());
        assert!(LayoutPolicy::OnDemand.uses_real_layout());
    }
}
