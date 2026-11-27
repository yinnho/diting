use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use diting::diting_browser::{BrowserContext, Page as InnerPage};
use serde_json::Value;

use crate::error::Error;

/// Watchdog budget for evaluates that dispatch into page event handlers
/// (click/input). 10s is far beyond any legitimate synchronous handler; a
/// page that exceeds it (infinite loop, MutationObserver storm) would
/// otherwise pin the session thread inside V8 forever — unreachable by
/// tokio timeouts, unclosable, leaking 100% CPU.
pub(crate) const INTERACTION_EVAL_TIMEOUT: Duration = Duration::from_secs(10);

static NEXT_HANDLE_SCOPE: AtomicU64 = AtomicU64::new(1);

/// Read a DOM node id from a JS `evaluate` result. the engine serializes JS numbers
/// as f64, so `Value::as_u64` returns None for an integer-valued result; accept
/// either an integer or a non-negative finite float. null / non-numbers -> None.
fn nid_from_value(v: &Value) -> Option<u64> {
    v.as_u64().or_else(|| {
        v.as_f64()
            .filter(|f| f.is_finite() && *f >= 0.0)
            .map(|f| f as u64)
    })
}

/// A browser tab/page.
pub struct Page {
    pub(crate) inner: InnerPage,
    pub(crate) context: Arc<BrowserContext>,
    handle_scope: u64,
    /// #189: the last default-path band frame plus the damage signature
    /// that produced it. The HTTP /screenshot poll path (browser86's
    /// display, the hosted live view) re-rasterizes nothing when the
    /// signature is unchanged — a static page's frame goes from seconds of
    /// band paint + PNG encode to a base64 memcpy.
    #[cfg(feature = "screenshot")]
    pub(crate) band_frame_cache: Option<BandFrameCache>,
}

/// One cached viewport-band frame: the encoded PNG plus the damage
/// signature (realm gen, dom epoch, layout rev, scroll, viewport, dpr)
/// under which it was produced — the same signature the CDP screencast
/// pump skips on, extended with the requested scale so a dpr=2 poll never
/// receives a dpr=1 frame.
#[cfg(feature = "screenshot")]
#[derive(Clone)]
pub(crate) struct BandFrameCache {
    pub sig: (u64, u64, u64, f32, f32, f32, f32, f32),
    pub width: u32,
    pub height: u32,
    pub png: Vec<u8>,
}

impl Page {
    pub(crate) fn new(inner: InnerPage, context: Arc<BrowserContext>) -> Self {
        Self {
            inner,
            context,
            handle_scope: NEXT_HANDLE_SCOPE.fetch_add(1, Ordering::Relaxed),
            #[cfg(feature = "screenshot")]
            band_frame_cache: None,
        }
    }

    /// #276 idle shrink (delegates to diting Page::shrink_js_heap).
    pub(crate) fn shrink_js_heap(&mut self) {
        self.inner.shrink_js_heap();
    }

    /// Navigate to URL and wait for load.
    pub async fn goto(&mut self, url: &str) -> Result<(), Error> {
        self.inner
            .navigate(url)
            .await
            .map_err(crate::error::navigation)
    }

    /// Get current URL.
    pub fn url(&self) -> String {
        self.inner.url_string()
    }

    /// Drain a JS-initiated navigation (location.href / form.submit recorded
    /// while running an `evaluate`) through the real navigation path. JS
    /// navigation set during an evaluate would otherwise sit unconsumed.
    /// Returns true if a pending navigation was drained.
    pub async fn process_pending_navigation(&mut self) -> Result<bool, Error> {
        self.inner
            .process_pending_navigation()
            .await
            .map_err(crate::error::navigation)
    }

    /// Execute JS in the page.
    pub fn evaluate(&mut self, expression: &str) -> Value {
        self.inner.evaluate(expression)
    }

    /// Execute JS in the page, awaiting any returned Promise.
    ///
    /// Use this for async scripts (fetch, IIFEs returning a Promise). The
    /// result is resolved by value, so JSON-stringified objects come back
    /// as strings just like the synchronous path.
    pub async fn evaluate_async(&mut self, expression: &str) -> Value {
        let info = self.inner.evaluate_remote(expression, true, true).await;
        info.value.unwrap_or(Value::Null)
    }

    /// Like [`Self::evaluate_async`], but a thrown/rejected script comes
    /// back as `Err("TypeError: boom (line 1, col 14)\n    at <anonymous>:1:14")`
    /// instead of collapsing to Null — agents can't debug what they can't
    /// see. The position is the script's own frame from the error's stack,
    /// and the frames after it are the script's, innermost first, as Chrome
    /// would spell them.
    /// `timeout_ms` widens the await budget for scripts whose page-side work
    /// (uploads via the page's own fetch) legitimately outlives the default
    /// 5s; on expiry the call errors with EVAL_TIMEOUT instead of a silent
    /// null — the script may still be running, so verify side effects
    /// before retrying. Clamped to 100ms..120s.
    pub async fn evaluate_async_checked(
        &mut self,
        expression: &str,
        timeout_ms: Option<u64>,
    ) -> Result<Value, String> {
        let budget = timeout_ms
            .unwrap_or(diting::diting_js::runtime::DEFAULT_AWAIT_BUDGET_MS)
            .clamp(100, 120_000);
        let outcome = self
            .inner
            .evaluate_remote_outcome(expression, true, true, budget, None)
            .await;
        if let Some(exc) = outcome.exception {
            let mut msg = exc.description.clone();
            if let Some(frame) = exc.own_frame() {
                msg = format!("{} (line {}, col {})", msg, frame.line, frame.column);
            }
            for frame in &exc.frames {
                msg.push_str("\n    at ");
                msg.push_str(&frame.text);
            }
            return Err(msg);
        }
        Ok(outcome.info.value.unwrap_or(Value::Null))
    }

    /// Bounded evaluate for interaction dispatch (click/input). A runaway
    /// event handler — or a MutationObserver microtask storm it triggers —
    /// pins the session thread inside V8 where tokio timeouts cannot reach;
    /// the watchdog terminates the isolate at `timeout` so the session's
    /// command loop (including Close) regains control. Returns Null when
    /// terminated.
    pub fn evaluate_with_timeout(&mut self, expression: &str, timeout: Duration) -> Value {
        self.inner.evaluate_with_timeout(expression, timeout)
    }

    /// Result-shaped twin (#236): click_xy must distinguish "dispatched"
    /// from "terminated mid-chain" instead of folding the error into Null.
    pub fn evaluate_with_timeout_result(
        &mut self,
        expression: &str,
        timeout: Duration,
    ) -> Result<Value, String> {
        self.inner.evaluate_with_timeout_result(expression, timeout)
    }

    /// Pin the viewport (device emulation); survives navigation. `dpr` pins
    /// `window.devicePixelRatio` when > 0, None keeps the persona's.
    pub fn set_viewport_override(&mut self, w: f32, h: f32, mobile: bool, dpr: Option<f64>) {
        self.inner.set_viewport_override(w, h, mobile, dpr);
    }

    pub fn viewport_override(&self) -> Option<(f32, f32, bool)> {
        self.inner.viewport_override()
    }

    /// Get page HTML content.
    pub fn content(&mut self) -> String {
        let val = self.evaluate("document.documentElement.outerHTML");
        val.as_str().unwrap_or("").to_string()
    }

    /// Query a single element by CSS selector.
    ///
    /// The returned handle is intentionally opaque and lifetimeless: it is a
    /// page-side DOM id plus this page instance's scope and realm epoch, not
    /// a Rust pointer. Navigation or use on another page rejects the handle;
    /// a same-document mutation can still invalidate the underlying node, in
    /// which case the click script reports failure. None of those cases may
    /// dereference invalid memory.
    pub fn query_selector(&mut self, selector: &str) -> Option<ElementHandle> {
        let escaped = selector.replace('\\', "\\\\").replace('\'', "\\'");
        let js = format!(
            "(function() {{ var el = document.querySelector('{}'); return el ? el._nid : null; }})()",
            escaped
        );
        let val = self.evaluate(&js);
        nid_from_value(&val).map(|node_id| ElementHandle {
            node_id,
            scope: self.handle_scope,
            realm_epoch: self.inner.realm_epoch,
        })
    }

    /// Click an element previously returned by [`Page::query_selector`].
    pub fn click_element(&mut self, element: ElementHandle) -> Result<(), Error> {
        if element.scope != self.handle_scope || element.realm_epoch != self.inner.realm_epoch {
            return Err(Error::ElementNotFound(
                "element handle is stale or belongs to another page".into(),
            ));
        }
        let node_id = element.node_id;
        // Scroll into view
        self.evaluate_with_timeout(
            &format!(
                "(function() {{ var el = globalThis._wrap && globalThis._wrap({}); if (el) el.scrollIntoView({{block:'center'}}); }})()",
                node_id
            ),
            INTERACTION_EVAL_TIMEOUT,
        );
        // Click
        let result = self.evaluate_with_timeout(
            &format!(
                "(function() {{ var el = globalThis._wrap && globalThis._wrap({}); if (el) {{ el.click(); return true; }} return false; }})()",
                node_id
            ),
            INTERACTION_EVAL_TIMEOUT,
        );
        if result.as_bool().unwrap_or(false) {
            Ok(())
        } else {
            Err(Error::ElementNotFound("click failed".into()))
        }
    }

    /// Wait for a named cookie to appear (polls every 200ms).
    pub async fn wait_for_cookie(&self, name: &str, timeout: Duration) -> Result<(), Error> {
        let start = std::time::Instant::now();
        loop {
            let url_str = self.url();
            if let Ok(parsed) = url::Url::parse(&url_str) {
                let header = self.context.cookie_jar.get_cookie_header(&parsed);
                // Cookie header format: "name1=value1; name2=value2"
                if header
                    .split("; ")
                    .any(|pair| pair.split('=').next().map(|n| n == name).unwrap_or(false))
                {
                    return Ok(());
                }
            }
            if start.elapsed() > timeout {
                return Err(Error::Timeout(format!(
                    "wait_for_cookie({}) timed out after {}ms",
                    name,
                    timeout.as_millis()
                )));
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    /// Drive the page's JS event loop for up to `max_ms` milliseconds.
    pub async fn settle(&mut self, max_ms: u64) {
        self.inner.settle(max_ms).await
    }

    /// Drive the JS event loop until quiescent, capped at `max_ms`. See
    /// [`diting::diting_browser::Page::settle_until_idle`].
    pub async fn settle_until_idle(&mut self, max_ms: u64) -> bool {
        self.inner.settle_until_idle(max_ms).await
    }

    /// Wait until the page stops producing content (nothing in flight and
    /// body text stable). `min_ms` is the never-cut floor; the tail beyond
    /// it is clamped to `tail_budget_ms`. See
    /// [`diting::diting_browser::Page::settle_content_stable`].
    pub async fn settle_content_stable(&mut self, min_ms: u64, tail_budget_ms: u64) {
        self.inner
            .settle_content_stable(min_ms, tail_budget_ms)
            .await
    }

    /// One background event-loop slice (pump, then park when quiescent).
    /// See [`diting::diting_browser::Page::pump_event_loop_slice`].
    pub async fn pump_event_loop_slice(&mut self, ms: u64) {
        self.inner.pump_event_loop_slice(ms).await
    }

    /// Arm the #66 command probe on a busy-frozen realm (no-op otherwise).
    /// The session actor calls this on every command arrival so the async
    /// work a command starts runs the next pump slice at full speed.
    pub fn note_command_arrived(&mut self) {
        self.inner.note_command_arrived()
    }

    /// #70: the realm's timer background budget (0 foreground, 1 one-second
    /// alignment, 2 one-minute alignment for chained timers).
    pub fn set_timer_background(&mut self, level: u8) {
        self.inner.set_timer_background(level)
    }

    /// Scroll the page by a relative offset.
    pub fn scroll_by(&mut self, dx: i32, dy: i32) {
        self.evaluate(&format!("window.scrollBy({}, {})", dx, dy));
    }
}

/// Opaque handle to an element in one page's live DOM.
///
/// This is deliberately not a borrow of [`Page`]: interaction methods stay on
/// `Page`, so Rust's ownership rules continue to govern page access. The scope
/// and realm epoch make cross-page and cross-navigation use explicit runtime
/// failures rather than accidental node-slot reuse.
#[derive(Clone, Copy, Debug)]
pub struct ElementHandle {
    node_id: u64,
    scope: u64,
    realm_epoch: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn handle_test_page() -> Page {
        let context = Arc::new(BrowserContext::with_storage_and_network(
            "handle-test".into(),
            None,
            false,
            None,
            None,
            true,
            None,
        ));
        Page::new(
            InnerPage::new("handle-test-page".into(), context.clone()),
            context,
        )
    }

    #[test]
    fn foreign_or_cross_realm_handle_is_rejected_before_dispatch() {
        let mut page = handle_test_page();
        let foreign_scope = ElementHandle {
            node_id: 1,
            scope: page.handle_scope.wrapping_add(1),
            realm_epoch: page.inner.realm_epoch,
        };
        let wrong_epoch = ElementHandle {
            node_id: 1,
            scope: page.handle_scope,
            realm_epoch: page.inner.realm_epoch.wrapping_add(1),
        };
        for handle in [foreign_scope, wrong_epoch] {
            match page.click_element(handle) {
                Err(Error::ElementNotFound(message)) => {
                    assert!(message.contains("stale or belongs to another page"));
                }
                other => panic!("foreign/stale handle must be rejected before dispatch: {other:?}"),
            }
        }
    }
}
