//! One document, one owner.
//!
//! `Page` holds the `Rc`. A navigation writes a pending document so the
//! outgoing realm keeps the live one until commit; `init_js` then
//! attaches that pending document to the new realm. The realm does not
//! keep a second copy of these facts.
//!
//! `Rc`, not `Arc`: the realm is already `Rc<RefCell<JsState>>` and the
//! document is shared on that same session thread. Interior mutability
//! is a `RefCell` for the same reason.

use std::cell::RefCell;
use std::rc::Rc;

struct Facts {
    url: String,
    title: String,
    /// URL of the document that initiated this navigation. Direct
    /// automation leaves it empty; document-initiated navigations set it
    /// per strict-origin-when-cross-origin (upstream edb1785).
    referrer: String,
    /// Referrer Policy from the main response's `Referrer-Policy` header
    /// (last valid comma token). Empty = none delivered; `<meta
    /// name=referrer>` then the spec default take over.
    referrer_policy_header: String,
    /// WHATWG canonical name (`"UTF-8"`, `"EUC-JP"`). Backs
    /// `document.characterSet`.
    encoding: String,
    /// MIME of the main response, lowercased, parameters stripped. Empty
    /// = the response carried no Content-Type.
    content_type: String,
    history_pos: i32,
    history_total: i32,
}

/// Document identity shared by the page and the committed realm.
pub struct Document {
    facts: RefCell<Facts>,
}

impl Document {
    /// A blank document: `about:blank`, UTF-8, empty strings, history 0/1.
    /// Returns the shared handle — a document is never owned outright.
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            facts: RefCell::new(Facts {
                url: "about:blank".to_string(),
                title: String::new(),
                referrer: String::new(),
                referrer_policy_header: String::new(),
                encoding: "UTF-8".to_string(),
                content_type: String::new(),
                history_pos: 0,
                history_total: 1,
            }),
        })
    }

    pub fn url(&self) -> String {
        self.facts.borrow().url.clone()
    }

    pub fn set_url(&self, url: &str) {
        self.facts.borrow_mut().url = url.to_string();
    }

    pub fn title(&self) -> String {
        self.facts.borrow().title.clone()
    }

    pub fn set_title(&self, title: &str) {
        self.facts.borrow_mut().title = title.to_string();
    }

    pub fn referrer(&self) -> String {
        self.facts.borrow().referrer.clone()
    }

    pub fn set_referrer(&self, referrer: &str) {
        self.facts.borrow_mut().referrer = referrer.to_string();
    }

    pub fn referrer_policy(&self) -> String {
        self.facts.borrow().referrer_policy_header.clone()
    }

    pub fn set_referrer_policy(&self, policy: &str) {
        self.facts.borrow_mut().referrer_policy_header = policy.to_string();
    }

    pub fn encoding(&self) -> String {
        self.facts.borrow().encoding.clone()
    }

    pub fn set_encoding(&self, encoding: &str) {
        self.facts.borrow_mut().encoding = encoding.to_string();
    }

    pub fn content_type(&self) -> String {
        self.facts.borrow().content_type.clone()
    }

    pub fn set_content_type(&self, content_type: &str) {
        self.facts.borrow_mut().content_type = content_type.to_string();
    }

    pub fn history_pos(&self) -> i32 {
        self.facts.borrow().history_pos
    }

    pub fn history_total(&self) -> i32 {
        self.facts.borrow().history_total
    }

    pub fn set_history(&self, pos: i32, total: i32) {
        let mut facts = self.facts.borrow_mut();
        facts.history_pos = pos;
        facts.history_total = total;
    }
}
