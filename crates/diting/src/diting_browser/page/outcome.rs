//! D19 outcome signatures — the session-level success observer.
//!
//! Billing moved to outcomes (DUPHUB.md 〇之二): a task arms a signature
//! group on the session and ANY execution path — flow steps, manual
//! takeover (eval/click), direct session API — that lands a matching
//! network row counts a success. The match lives HERE, in the one funnel
//! every network row crosses (`record_network_event` / `_with_body`),
//! so it cannot be bypassed by choosing how to drive the page.
//!
//! Signatures are an OR-list; one row matching ANY signature is ONE hit
//! (a submit XHR and its success redirect declare one canonical signature,
//! not two). Duplicate rows (redirect re-records, double fetch) collapse
//! within `DEDUP_SECS`. This layer only matches and records; the billing
//! call happens in the engine layer that drains `take_hits`.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// A first-party success signature. Kinds:
/// - `url`: the row's URL contains `contains` (a landing page —
///   `success.htm?primaryId=` — is the canonical publish receipt).
/// - `resp`: URL contains `url_contains` AND the retained body contains
///   `body_contains` (an API reply shape).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeSignature {
    pub kind: String,
    #[serde(default)]
    pub contains: String,
    #[serde(default)]
    pub url_contains: String,
    #[serde(default)]
    pub body_contains: String,
}

/// One matched row.
#[derive(Debug, Clone, Serialize)]
pub struct OutcomeHit {
    pub sig: usize,
    pub url: String,
    pub at: f64,
}

/// Matching rows within this window of the SAME signature and URL are one
/// hit (redirect/retry re-records).
const DEDUP_SECS: f64 = 3.0;

#[derive(Default)]
pub struct OutcomeObserver {
    sigs: Vec<OutcomeSignature>,
    hits: Vec<OutcomeHit>,
}

impl OutcomeObserver {
    /// Arm (replace) the signature group. Empty list disarms.
    pub fn set(&mut self, sigs: Vec<OutcomeSignature>) -> usize {
        self.sigs = sigs;
        self.sigs.len()
    }

    pub fn is_armed(&self) -> bool {
        !self.sigs.is_empty()
    }

    /// Drain recorded hits (the engine layer settles against them).
    pub fn take_hits(&mut self) -> Vec<OutcomeHit> {
        std::mem::take(&mut self.hits)
    }

    /// The funnel hook for every network row. `body` is present only on the
    /// with-body path (`kind:"resp"` needs it; `kind:"url"` never does).
    pub fn observe(&mut self, url: &str, status: u16, body: Option<&[u8]>) {
        if self.sigs.is_empty() {
            return;
        }
        // Success receipts are 2xx/3xx landings or redirects; failures
        // (4xx/5xx) carrying the same URL shape are not successes.
        let ok = (200..400).contains(&status) || status == 0;
        if !ok {
            return;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        for (i, sig) in self.sigs.iter().enumerate() {
            let matched = match sig.kind.as_str() {
                "url" => !sig.contains.is_empty() && url.contains(&sig.contains),
                "resp" => {
                    let url_ok =
                        sig.url_contains.is_empty() || url.contains(&sig.url_contains);
                    let body_ok = match (body, sig.body_contains.is_empty()) {
                        (Some(b), false) => {
                            String::from_utf8_lossy(b).contains(&sig.body_contains)
                        }
                        (_, true) => true,
                        (None, false) => false,
                    };
                    url_ok && body_ok && !sig.body_contains.is_empty()
                }
                _ => false,
            };
            if !matched {
                continue;
            }
            // Same signature + same URL inside the dedup window is the same
            // success re-recorded, not a second one.
            if let Some(last) = self.hits.last() {
                if last.sig == i && last.url == url && now - last.at < DEDUP_SECS {
                    return;
                }
            }
            self.hits.push(OutcomeHit {
                sig: i,
                url: url.to_string(),
                at: now,
            });
            return; // one row = at most one hit (OR-list)
        }
    }
}
