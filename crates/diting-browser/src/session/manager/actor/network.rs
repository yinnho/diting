//! `SessionCommand::NavigateCdp` and `Subscribe` on the session thread —
//! the pieces of the page's own request and console traffic a face tells
//! on its own terms. Split from actor.rs for the god-file cap.

use diting::diting_net::NetFailure;
use serde_json::json;
use tokio::sync::{mpsc, oneshot};

use super::super::super::commands::{
    NavigateCdpReply, PageNavigation, SameDocumentKind, SessionCommand, SessionEvent,
    SessionNavResponse,
};
use super::super::super::interact::{merge_dom_candidates, DOM_MEDIA_SCRIPT};
use super::SessionActor;

/// A script-initiated request older than this is not "slow", it is hung —
/// every transport the engine rides carries a 30s read timeout and a 120s
/// sync-XHR cap, so a 13-minute in-flight entry (the #203 taobao submit
/// shape) means the completion never reached JS at all. `hung: true` tells
/// the reading agent that waiting longer is pointless and the page's own
/// handler will never fire — the honest signal before a re-issue decision.
const HUNG_INFLIGHT_AFTER_MS: u64 = 300_000;

/// One `in_flight` row for the Network payload: url/method/age, plus the
/// `hung` verdict once the age crosses [`HUNG_INFLIGHT_AFTER_MS`].
fn in_flight_row(f: &diting::diting_net::InFlightScripted, now_ms: u64) -> serde_json::Value {
    let age_ms = now_ms.saturating_sub(f.dispatched_at_ms);
    json!({
        "url": f.url,
        "method": f.method,
        "age_ms": age_ms,
        "hung": age_ms >= HUNG_INFLIGHT_AFTER_MS,
    })
}

/// `url` without its `#fragment`.
fn without_fragment(url: &str) -> &str {
    url.split('#').next().unwrap_or(url)
}

impl SessionActor {
    /// Navigate, then claim the request rows filed meanwhile: they come
    /// back with the answer, in order, and move the cursor so the idle
    /// pump does not push them a second time.
    pub(super) async fn on_navigate_cdp(
        &mut self,
        url: String,
        reply: NavigateCdpReply,
        cmd_rx: &mut mpsc::UnboundedReceiver<SessionCommand>,
    ) {
        let (tx, rx) = oneshot::channel();
        self.last_nav_failure = None;
        self.on_navigate(url, tx, cmd_rx).await;
        let nav = rx
            .await
            .unwrap_or_else(|_| Err("navigation dropped".to_string()));
        let rows = self.page.inner.network_events_since(self.net_seen);
        if let Some(last) = rows.last() {
            self.net_seen = last.seq;
        }
        let failure = self.last_nav_failure.take().unwrap_or(NetFailure::Other);
        let calls =
            super::super::super::state::drain_console(&mut self.page, &mut self.console_ring);
        let _ = reply.send(Ok((nav, rows, failure, calls)));
    }

    /// `SessionCommand::NavigateHistoryCdp`: jump the cursor and navigate,
    /// the rows claimed exactly as a fresh navigation's are.
    pub(super) async fn on_navigate_history_cdp(&mut self, index: usize, reply: NavigateCdpReply) {
        self.last_nav_failure = None;
        let nav = match self.page.inner.jump_history_to(index).await {
            Ok(_) => Ok(SessionNavResponse {
                url: self.page.url(),
                title: Some(self.page.inner.title()),
                challenge: None,
                redirected_from: self.page.inner.navigation_record().redirect_chain.clone(),
                preloads: Vec::new(),
            }),
            Err(e) => {
                self.last_nav_failure = Some(e.failure());
                Err(format!("navigation failed: {e}"))
            }
        };
        let rows = self.page.inner.network_events_since(self.net_seen);
        if let Some(last) = rows.last() {
            self.net_seen = last.seq;
        }
        let failure = self.last_nav_failure.take().unwrap_or(NetFailure::Other);
        let calls =
            super::super::super::state::drain_console(&mut self.page, &mut self.console_ring);
        let _ = reply.send(Ok((nav, rows, failure, calls)));
    }

    /// `SessionCommand::NavigationHistory`: the URLs and where the cursor is.
    pub(super) fn on_navigation_history(
        &mut self,
        reply: oneshot::Sender<Result<(Vec<String>, usize), String>>,
    ) {
        let inner = &self.page.inner;
        let _ = reply.send(Ok((inner.history.clone(), inner.history_index)));
    }

    /// The stored response body of `request_id`, if the page kept one.
    pub(super) fn on_response_body(
        &mut self,
        request_id: String,
        reply: oneshot::Sender<
            Result<Option<diting::diting_browser::page::StoredResponseBody>, String>,
        >,
    ) {
        let _ = reply.send(Ok(self.page.inner.get_response_body(&request_id)));
    }

    /// Follow a navigation the page queued for itself — a link clicked, a
    /// `location` assigned by a script — and tell the subscribers how it
    /// went, with the request rows it filed. A command's own handler
    /// drains these for the faces that wait on it; a session that is
    /// followed has nobody waiting, so the idle pump does it.
    pub(super) async fn follow_page_navigation(&mut self) {
        if self.console_subs.is_empty() {
            return;
        }
        let history = self.page.inner.has_pending_history_jump();
        if !history && !self.page.inner.has_pending_navigation() {
            self.follow_same_document_move();
            return;
        }
        let ran = self.page.process_pending_navigation().await;
        let rows = self.page.inner.network_events_since(self.net_seen);
        if let Some(last) = rows.last() {
            self.net_seen = last.seq;
        }
        let landed = match ran {
            Ok(_) => Some((self.page.url(), self.page.inner.title())),
            Err(error) => {
                tracing::debug!("page-started navigation failed: {error}");
                None
            }
        };
        let event = SessionEvent::Navigated(Box::new(PageNavigation {
            landed,
            history,
            rows,
        }));
        self.console_subs
            .retain(|sub| sub.send(event.clone()).is_ok());
    }

    /// A move the page made inside its document — `history.pushState`, a
    /// fragment, history stepped within it — adopted as the page's URL and
    /// told to the subscribers, as what it is: no new document.
    fn follow_same_document_move(&mut self) {
        let before = self.page.url();
        if !self.page.inner.sync_virtual_url() {
            return;
        }
        let url = self.page.url();
        let kind = if without_fragment(&before) == without_fragment(&url) {
            SameDocumentKind::Fragment
        } else {
            SameDocumentKind::History
        };
        let event = SessionEvent::SameDocument { url, kind };
        self.console_subs
            .retain(|sub| sub.send(event.clone()).is_ok());
    }

    /// Push this session's page events to `sink` from now on.
    pub(super) fn on_subscribe(
        &mut self,
        sink: mpsc::UnboundedSender<SessionEvent>,
        reply: oneshot::Sender<Result<(), String>>,
    ) {
        self.console_subs.push(sink);
        let _ = reply.send(Ok(()));
    }

    pub(super) async fn on_network(
        &mut self,
        media_only: bool,
        include_bodies: bool,
        include_headers: bool,
        url_contains: Option<String>,
        body_max_chars: usize,
        reply: oneshot::Sender<Result<String, String>>,
    ) {
        let Self { page, .. } = self;
        let record = page.inner.navigation_record();
        let payload = if media_only {
            // Media elements and player iframes are never
            // fetched by the engine (no media/frame
            // loading), so they cannot surface in the
            // network log — collect their DOM sources and
            // merge them in as candidates.
            let dom = page
                .evaluate(DOM_MEDIA_SCRIPT)
                .as_str()
                .unwrap_or("[]")
                .to_string();
            let events = &record.events;
            let mut media = crate::har::media_entries(events);
            merge_dom_candidates(&mut media, &dom);
            let body_of = |rid: &str| page.inner.get_response_body(rid);
            media.extend(crate::har::media_from_bodies(events, &body_of));
            serde_json::json!({
                "url": page.url(),
                "media": media,
            })
        } else {
            let events = &record.events;
            let body_of = |rid: &str| page.inner.get_response_body(rid);
            let mut payload = serde_json::json!({
                "url": page.url(),
                // Current navigation generation (#101):
                // rows with a lower "nav" below are
                // earlier attempts' leftovers.
                "nav": record.epoch,
                "total": events.len(),
                "requests": crate::har::compact_events(events, include_headers),
            });
            // #116: hung script-initiated fetches are
            // invisible in the request log (entries land
            // at completion) — surface what is still
            // flying so a timed-out caller can tell a
            // slow save from a dead one before re-issuing.
            // #130: the key is always present, `[]` when
            // quiet — an absent field is indistinguishable
            // from a wrong endpoint / stale version for
            // the reading agent, so "zero in flight"
            // must read as an empty array, not silence.
            let in_flight = page.inner.scripted_in_flight();
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            payload["in_flight"] = json!(in_flight
                .iter()
                .map(|f| in_flight_row(f, now_ms))
                .collect::<Vec<_>>());
            // Anti-bot challenges answer 200, so they
            // hide among successful rows — surface the
            // count at the top level so an agent that
            // just asks "did we get punished" doesn't
            // have to scan every URL. Same detection as
            // the Challenges command (URL shape + the
            // MTop risk-control bodies), so the numbers
            // agree. Same always-present contract as
            // in_flight (#130).
            let challenges = crate::har::challenge_rows(events, &body_of).len();
            payload["challenges"] = json!(challenges);
            if include_bodies {
                let filters = url_contains
                    .as_deref()
                    .map(|s| vec![s.to_string()])
                    .unwrap_or_default();
                payload["xhr"] = json!(crate::har::xhr_bodies(
                    events,
                    &filters,
                    body_max_chars,
                    include_headers,
                    &body_of
                ));
            }
            payload
        };
        let _ = reply.send(Ok(payload.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::{in_flight_row, HUNG_INFLIGHT_AFTER_MS};
    use diting::diting_net::InFlightScripted;

    /// The #203 taobao submit shape: an entry aged past every engine-side
    /// timeout must read `hung: true` — the reading agent's stop-waiting
    /// signal. A young entry stays `hung: false` even though it is already
    /// surfaced (slow ≠ hung).
    #[test]
    fn in_flight_row_flags_entries_older_than_the_hung_threshold() {
        let now = 1_000_000u64;
        let young = in_flight_row(
            &InFlightScripted {
                url: "https://x/slow".into(),
                method: "POST".into(),
                dispatched_at_ms: now - 5_000,
            },
            now,
        );
        assert_eq!(young["hung"], false, "a 5s-old entry is slow, not hung");
        assert_eq!(young["age_ms"], 5_000);

        let hung = in_flight_row(
            &InFlightScripted {
                url: "https://x/submit.htm".into(),
                method: "POST".into(),
                dispatched_at_ms: now - HUNG_INFLIGHT_AFTER_MS,
            },
            now,
        );
        assert_eq!(hung["hung"], true, "age == threshold counts as hung");
        assert_eq!(hung["age_ms"], HUNG_INFLIGHT_AFTER_MS);
    }
}
