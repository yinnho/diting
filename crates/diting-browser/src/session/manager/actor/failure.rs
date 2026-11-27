//! Initial-navigation failure classification for the session actor.

/// Should a failed initial navigation be replaced with about:blank?
///
/// A pre-commit failure leaves no usable document or JS realm, so the actor
/// lands on about:blank to guarantee a working context. A deadline after the
/// document committed is different: the live page may still be initializing,
/// and replacing it would discard exactly the document the caller asked for.
pub(super) fn should_blank_after_initial_nav_error(message: &str) -> bool {
    !message.contains("document committed")
}

#[cfg(test)]
mod tests {
    use super::should_blank_after_initial_nav_error;

    #[test]
    fn pre_commit_navigation_failure_lands_on_blank() {
        assert!(should_blank_after_initial_nav_error(
            "navigation error: Network error: navigation exceeded 30000ms deadline \
             (document fetch in flight; active document: about:blank)"
        ));
        assert!(should_blank_after_initial_nav_error(
            "navigation error: Network error: navigation exceeded 30000ms deadline \
             (document received, not yet committed; active document: about:blank)"
        ));
    }

    #[test]
    fn committed_document_deadline_preserves_live_page() {
        assert!(!should_blank_after_initial_nav_error(
            "navigation error: Network error: navigation exceeded 30000ms deadline \
             (document committed, scripts/settle still running; active document: \
             https://example.test/)"
        ));
    }
}
