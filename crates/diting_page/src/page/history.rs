//! Session history: the cursor moves and entry appends a navigation or a
//! `history.back()` jump drives. Split from navigate.rs when it crossed the
//! god-file cap; behavior unchanged.

use super::{Page, PageError};

impl Page {
    /// Jump the session-history cursor to `target` and navigate to the
    /// entry there — what `Page.navigateToHistoryEntry` asks for. The same
    /// move the page's own `history.back()` makes: the cursor moves first
    /// (the landing document's JS history seeds from it), and a failed
    /// navigation restores it.
    pub async fn jump_history_to(&mut self, target: usize) -> Result<bool, PageError> {
        if target == self.history_index || target >= self.history.len() {
            return Ok(false);
        }
        let url = self.history[target].clone();
        let prev_index = self.history_index;
        self.history_index = target;
        let result = self
            .navigate_with_wait_post_ref(&url, crate::lifecycle::WaitUntil::Load, "GET", "", "")
            .await;
        if result.is_err() {
            self.history_index = prev_index;
        }
        result.map(|_| true)
    }

    /// Append the current URL to the history stack, truncating any forward
    /// entries past the cursor (matches real Chrome: navigating after a
    /// goBack clobbers the forward history).
    pub fn push_history(&mut self, url: String) {
        if url.is_empty() {
            return;
        }
        // Don't dupe consecutive entries (Page.reload would otherwise pile up).
        if self.history.get(self.history_index) == Some(&url) {
            return;
        }
        if !self.history.is_empty() && self.history_index < self.history.len() - 1 {
            self.history.truncate(self.history_index + 1);
        }
        self.history.push(url);
        self.history_index = self.history.len() - 1;
    }

    /// Move the history cursor without re-navigating; used by
    /// Page.navigateToHistoryEntry which then drives the actual fetch.
    pub fn set_history_index(&mut self, idx: usize) {
        if idx < self.history.len() {
            self.history_index = idx;
        }
    }
}
