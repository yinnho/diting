//! The passive request/response observer face (upstream issue #408), split
//! out of client/mod.rs (god-file ratchet): RequestInfo, the callback
//! aliases, and the page-scoped CallbackRegistry.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use url::Url;

use super::metadata::ResourceType;
use super::Response;

/// CDP Fetch-domain resolution: what a client answers a paused request
/// with. Produced by the CDP bridge's `Fetch.continueRequest` /
/// `fulfillRequest` / `failRequest` handlers.
#[derive(Debug)]
pub enum InterceptResolution {
    Continue {
        url: Option<String>,
        method: Option<String>,
        headers: Option<HashMap<String, String>>,
        body: Option<Vec<u8>>,
    },
    Fulfill {
        status: u16,
        headers: HashMap<String, String>,
        body: Vec<u8>,
    },
    Fail {
        reason: String,
    },
}

/// A paused request surfaced to the interception channel (CDP
/// `Fetch.requestPaused` shape). The resolver answers with an
/// `InterceptResolution`. Field readers are the CDP layer, which mints its
/// own pause ids; engine code only moves the struct through the channel.
#[derive(Debug)]
pub struct InterceptedRequest {
    pub url: String,
    pub method: String,
    pub headers: HashMap<String, String>,
    /// The request body as raw bytes (`postData` on the wire is base64).
    /// `None` for requests that carry no body.
    pub post_data: Option<Vec<u8>>,
    pub resource_type: String,
    /// The network-row id this request will be recorded under, minted at
    /// the pause point when the pause point shares a scope with the row's
    /// recording (a JS fetch inside its op, a scripted subresource at its
    /// call site). The CDP layer pairs `Fetch.requestPaused` with the
    /// row's `Network.requestWillBeSent` on this id; empty where the pause
    /// sits inside the transport, below the recording scope, and pairing
    /// is impossible. Document pauses leave it empty too: a document's
    /// wire id is its loader's, which the CDP layer numbers itself.
    pub request_id: String,
    pub resolver: tokio::sync::oneshot::Sender<InterceptResolution>,
}

/// How long a paused request waits for a CDP client's resolution before
/// falling through to the real request (Continue semantics). The CDP bridge
/// can only answer between websocket reads, so a pause raised inside a
/// command that itself waits on the fetch (a navigate, an evaluate of a
/// fetch expression) is bounded: without it such a call would hang forever.
/// Real clients resolve in milliseconds; the default only shows up on
/// client-less pages. Tests shorten it via the atomic.
pub static INTERCEPT_RESOLUTION_TIMEOUT_MS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(10_000);

/// A request about to be sent (or just answered), as seen by an
/// on_request / on_response observer. Headers are the fully-built set the
/// transport sent, lowercased like `Response.headers`.
#[derive(Debug, Clone)]
pub struct RequestInfo {
    pub url: Url,
    pub method: String,
    pub headers: HashMap<String, String>,
    pub resource_type: ResourceType,
}

pub type RequestCallback = Arc<dyn Fn(&RequestInfo) + Send + Sync>;
pub type ResponseCallback = Arc<dyn Fn(&RequestInfo, &Response) + Send + Sync>;

/// Page-scoped store for the passive on_request/on_response callbacks (upstream
/// issue #408). Each `Page` owns one, so a callback never fires for another
/// page's requests and dies with its page. The HTTP client itself stays
/// callback-free; page-driven fetches pass the page's registry in. Ids keep
/// the `u64` shape upstream established on `Page::on_request`/`on_response`.
pub struct CallbackRegistry {
    on_request: RwLock<Vec<(u64, RequestCallback)>>,
    on_response: RwLock<Vec<(u64, ResponseCallback)>>,
    id_counter: std::sync::atomic::AtomicU64,
    /// The Fetch-domain interception channel, armed by `Fetch.enable` via
    /// `Page::set_fetch_intercept`. Held under a std mutex: readers take it
    /// once per request hop on the async path, and a std lock's critical
    /// section is a clone — no await can sit inside it.
    intercept_tx: std::sync::Mutex<Option<tokio::sync::mpsc::UnboundedSender<InterceptedRequest>>>,
    /// Mints row ids for pauses raised at subresource call sites, which sit
    /// outside the page's own counter. `isub-{N}` shares no format with the
    /// page rows' `{page}.{counter}` or the JS rows' `fetch-{N}`, so the id
    /// spaces never collide within a page — and the registry itself is
    /// page-scoped, so uniqueness across pages is never needed.
    intercept_id_counter: std::sync::atomic::AtomicU64,
}

impl CallbackRegistry {
    pub fn new() -> Self {
        CallbackRegistry {
            on_request: RwLock::new(Vec::new()),
            on_response: RwLock::new(Vec::new()),
            id_counter: std::sync::atomic::AtomicU64::new(1),
            intercept_tx: std::sync::Mutex::new(None),
            intercept_id_counter: std::sync::atomic::AtomicU64::new(1),
        }
    }

    /// A fresh row id for an interception pause raised at a call site —
    /// recorded into the pause so the row, written when the fetch lands,
    /// can be rekeyed to it and the CDP pairing holds.
    pub fn mint_intercept_row_id(&self) -> String {
        format!(
            "isub-{}",
            self.intercept_id_counter
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        )
    }

    /// Arm / disarm request interception for every fetch this registry
    /// sees. `Some(tx)` routes matching requests through the channel as
    /// `InterceptedRequest`s; `None` disarms. The send side is the CDP
    /// bridge's drainer.
    pub fn set_intercept_tx(
        &self,
        tx: Option<tokio::sync::mpsc::UnboundedSender<InterceptedRequest>>,
    ) {
        *self
            .intercept_tx
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = tx;
    }

    /// The armed interception channel, cloned for one pause. `None` when
    /// interception is off — the overwhelmingly common case, so callers
    /// check this before building a pause payload.
    pub fn intercept_tx(&self) -> Option<tokio::sync::mpsc::UnboundedSender<InterceptedRequest>> {
        self.intercept_tx
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn next_id(&self) -> u64 {
        self.id_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    /// Register a request callback; the returned id detaches it via
    /// `remove_request`. Sync like the pre-registry push path: registration
    /// happens from `Page` setup where no reader holds the lock, so
    /// `try_write` cannot fail there.
    pub fn add_request(&self, cb: RequestCallback) -> u64 {
        let id = self.next_id();
        if let Ok(mut v) = self.on_request.try_write() {
            v.push((id, cb));
        }
        id
    }

    /// Register a response callback; see `add_request`.
    pub fn add_response(&self, cb: ResponseCallback) -> u64 {
        let id = self.next_id();
        if let Ok(mut v) = self.on_response.try_write() {
            v.push((id, cb));
        }
        id
    }

    /// Detach a request callback. Returns true when the id was found and
    /// removed, so a double detach is a visible no-op.
    pub fn remove_request(&self, id: u64) -> bool {
        match self.on_request.try_write() {
            Ok(mut v) => {
                let before = v.len();
                v.retain(|(cid, _)| *cid != id);
                v.len() != before
            }
            Err(_) => false,
        }
    }

    /// True when at least one request callback is registered. Lets fire sites
    /// skip building a `RequestInfo` when nobody listens.
    pub async fn has_request_callbacks(&self) -> bool {
        !self.on_request.read().await.is_empty()
    }

    /// True when at least one response callback is registered.
    pub async fn has_response_callbacks(&self) -> bool {
        !self.on_response.read().await.is_empty()
    }

    pub async fn fire_request(&self, info: &RequestInfo) {
        for (_, cb) in self.on_request.read().await.iter() {
            cb(info);
        }
    }

    pub async fn fire_response(&self, info: &RequestInfo, resp: &Response) {
        for (_, cb) in self.on_response.read().await.iter() {
            cb(info, resp);
        }
    }
}

impl Default for CallbackRegistry {
    fn default() -> Self {
        Self::new()
    }
}
