//! Realm lifecycle: build, park, and resume the page's JS isolate.
//!
//! `init_js` is the navigation commit. `suspend_js` / `resume_js` park a
//! target switch. Session storage is snapshotted across both teardowns.

use diting_js::runtime::JsRuntime;

use super::Page;

impl Page {
    /// #276 idle shrink: full-GC the live realm at an idle command boundary
    /// (`DITING_REALM_SHRINK`, same knob as the post-init shrink).
    /// For parked sessions on garbage-heavy pages — a merchant console
    /// churning for minutes — this returns accumulated garbage to the pool
    /// without waiting for the next navigation's realm rebuild. Safe here
    /// by construction: no script is executing (command boundary, pump
    /// slice done) and the call rides the session thread that owns the
    /// isolate.
    pub fn shrink_js_heap(&mut self) {
        if let Some(js) = self.js.as_mut() {
            js.shrink_heap();
        }
    }

    /// #70: put the realm's timers on a background budget — 0 foreground,
    /// 1 wake-ups aligned to 1s, 2 chained timers aligned to 1min (see
    /// `__diting_set_background` in timer_belt.js).
    pub fn set_timer_background(&mut self, level: u8) {
        if let Some(js) = self.js.as_mut() {
            let _ = js.evaluate(&format!(
                "globalThis.__diting_set_background && __diting_set_background({level})"
            ));
        }
    }

    /// Heap-guard terminations this realm has survived
    /// (hosted-incident 10-14) — see [`JsRuntime::heap_guard_strikes`].
    /// The session actor drops the realm once this crosses its strike
    /// limit. Zero when no runtime is mounted yet.
    pub fn heap_guard_strikes(&self) -> usize {
        self.js.as_ref().map_or(0, JsRuntime::heap_guard_strikes)
    }

    /// Pin the hardware persona seed (screen/dpr/GPU/canvas all draw from
    /// it). Must run before the first navigation — init_js bakes the seed
    /// into the JS runtime on every navigation, so a pre-goto pin holds for
    /// the page's whole life. The account layer uses this to give each
    /// named identity one stable device.
    pub fn set_fingerprint_seed(&mut self, seed: u64) {
        self.fp_seed = seed;
    }

    pub(super) fn init_js(&mut self) {
        // Drop any existing runtime so the JS realm starts clean on
        // every navigation. The old code reused the V8 isolate and
        // only re-bound `globalThis.document`, leaving window.onload,
        // custom window properties and event handlers from the prior
        // page in place. That made it possible for a page to set
        // attacker-controlled state, trigger a navigation, and then
        // run code in the next document's context.
        self.snapshot_session_storage();
        // Same drain as suspend_js: console calls the outgoing document
        // logged since the last pump must survive the realm swap (Chrome
        // preserves per-tab console history across navigations).
        if let Some(js) = self.js.as_mut() {
            let calls = js.take_pending_console_calls();
            self.suspended_console.extend(calls);
        }
        if self.js.is_some() {
            let _ = self.js.take();
        }
        // The realm swap IS the navigation's commit point: from here on,
        // the live document is the new one (init_js runs once the response
        // arrived and parsed — #101's phase evidence).
        self.realm_epoch = self.navigation_epoch;
        self.nav_progress.note_committed();

        // Thread the BrowserContext's proxy through to the ES-module loader
        // and op_fetch_url so dynamic imports and JS fetch() honour the
        // configured upstream proxy (#139). When proxy_url is None this is
        // equivalent to with_base_url() (direct connection).
        let mut rt =
            JsRuntime::with_base_url_and_proxy(&self.url_string(), self.context.proxy_url.clone());
        // Commit the in-flight document onto this realm. Navigation staged
        // title/referrer/encoding on `pending_document` so the outgoing
        // realm kept its own document; resume (no pending) reattaches the live
        // one. URL and the history seed are page-owned and stamped here,
        // after the constructor — `__diting_init` runs inside it, and the
        // old setters ran after that return too.
        let doc = self
            .pending_document
            .take()
            .unwrap_or_else(|| self.document.clone());
        doc.set_url(&self.url_string());
        // Seed the JS history stack from the REAL session history
        // (obscura#1105): a fresh runtime would report history.length === 1
        // and swallow back(). push_history has not run yet — the cursor
        // still names the outgoing document. The landing entry either
        // appends after it (dedupe miss → doc index = cursor+1) or reuses
        // the cursor's own entry (dedupe hit: re-navigation to the same
        // URL, or a history jump whose cursor was pre-moved by the pump).
        {
            let landing = self.url_string();
            let dedupe_hit = self
                .history
                .get(self.history_index)
                .is_some_and(|u| *u == landing);
            let (pos, total) = if self.history.is_empty() {
                (0, 1)
            } else if dedupe_hit {
                (self.history_index, self.history.len())
            } else {
                (self.history_index + 1, self.history.len() + 1)
            };
            doc.set_history(pos as i32, total as i32);
        }
        self.document = doc.clone();
        rt.attach_document(doc);
        rt.adopt_layout_policy(self.layout_policy);
        // Re-pin this page's hardware persona before anything else reads it
        // (the fresh realm just drew a throwaway identity at construction).
        rt.set_fingerprint_seed(self.fp_seed);

        // JS-layer UA must match the HTTP-layer UA we advertise (set via
        // DITING_UA / context.user_agent). Hardcoding the stealth
        // client's Linux UA here left navigator.userAgent as Linux while HTTP
        // headers said macOS — anti-bot checks that read navigator (Baidu
        // Wenku's 安全验证) caught the mismatch. Prefer the context UA; fall
        // back to the stealth client's UA only if none is set.
        let ua_to_set = if let Ok(ua) = self.http_client.user_agent.try_read() {
            ua.clone()
        } else {
            {
                if self.stealth_client.is_some() {
                    diting_net::STEALTH_USER_AGENT.to_string()
                } else {
                    String::new()
                }
            }
        };
        if !ua_to_set.is_empty() {
            rt.set_user_agent(&ua_to_set);
        }
        // Route-following language (geo-variant class, LexBench challenge
        // 2026-11-21): a document loaded through the proxy tunnel or from a
        // variant-table domain advertises the international variant in
        // navigator.language too — client-side locale pickers (canva-class
        // SPAs) read it, and the HTTP legs already made this same choice
        // (INTERNATIONAL_LANGUAGE). An explicit DITING_ACCEPT_LANGUAGE /
        // set_accept_language pins every route.
        let via_proxy = self.context.proxy_url.is_some()
            || (diting_env::env_knobs::proxy_from_env().is_some()
                && diting_env::env_knobs::should_auto_proxy(&self.url_string()));
        let lang =
            if diting_net::client::wants_international_language(&self.url_string(), via_proxy)
                && !self
                    .http_client
                    .lang_pinned
                    .load(std::sync::atomic::Ordering::Relaxed)
            {
                diting_net::client::INTERNATIONAL_LANGUAGE.to_string()
            } else if let Ok(l) = self.http_client.accept_language.try_read() {
                l.clone()
            } else {
                "zh-CN,zh;q=0.9".to_string()
            };
        rt.set_language(&lang);

        rt.set_cookie_jar(self.context.cookie_jar.clone());
        rt.set_http_client(self.http_client.clone());

        if let Some(tx) = &self.intercept_tx {
            rt.set_intercept_tx(tx.clone());
            // tx presence == armed: `set_fetch_intercept(None)` clears both,
            // so a realm rebuilt after navigation (init_js runs on every
            // document) resumes intercepting instead of silently passing
            // fetches straight through while the bridge still waits.
            rt.set_intercept_enabled(true);
        }
        if !self.blocked_urls.is_empty() {
            // setBlockedURLs must survive navigation like the intercept arm
            // does — the static loaders read the Page field directly, the
            // JS path (fetch()/XHR) needs it replayed into the fresh realm.
            rt.set_blocked_urls(self.blocked_urls.clone());
        }

        // Script-initiated fetch()/XHR fire the page's passive observers too
        // (upstream #408).
        rt.set_callbacks(self.callbacks.clone());

        if let Some(dom) = self.dom.take() {
            rt.set_dom(dom);
        }

        self.js = Some(rt);
        // Fresh document ⇒ fresh JS accounting: a busy-freeze from the
        // previous realm must not follow the operator into the new page
        // (#66 — Navigate/SetContent is the documented unfreeze path).
        self.busy_frozen = false;
        self.busy_probe_pending = false;
        self.busy_mark = None;
        self.restore_session_storage();
        self.apply_viewport_override();
        self.apply_emulated_media();
        self.apply_touch_override();
        self.apply_timezone_override();
        self.apply_locale_override();
    }

    /// Capture the live realm's `sessionStorage` into `self.session_storage`
    /// before the realm is dropped (navigation teardown or target switch).
    /// Runs one synchronous round-trip reading `location.origin` + every entry.
    pub(super) fn snapshot_session_storage(&mut self) {
        let js = match self.js.as_mut() {
            Some(js) => js,
            None => return,
        };
        let expr = "(function(){ var o={}; var ks=Object.keys(sessionStorage); for (var i=0;i<ks.length;i++){ o[ks[i]]=sessionStorage.getItem(ks[i]); } return { origin: location.origin, entries: o }; })()";
        let val = match js.evaluate(expr) {
            Ok(v) => v,
            Err(_) => return,
        };
        let mut map = match val {
            serde_json::Value::Object(m) => m,
            _ => return,
        };
        let entries = map.remove("entries");
        let origin = match map.get("origin").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => return,
        };
        let entries = match entries {
            Some(serde_json::Value::Object(e)) => e,
            _ => return,
        };
        let mut store = std::collections::HashMap::new();
        for (k, v) in entries {
            if let Some(s) = v.as_str() {
                store.insert(k, s.to_string());
            }
        }
        self.session_storage = Some((origin, store));
    }

    /// Re-seed `sessionStorage` into the freshly rebuilt realm if its origin
    /// matches the snapshot's origin (same-origin navigation, or resume after
    /// a target switch). A cross-origin navigation leaves the snapshot behind
    /// and gets a fresh empty store, matching the per-tab-per-origin spec.
    fn restore_session_storage(&mut self) {
        let Some((origin, entries)) = self.session_storage.take() else {
            return;
        };
        let js = match self.js.as_mut() {
            Some(js) => js,
            None => return,
        };
        let new_origin = match js.evaluate("location.origin") {
            Ok(serde_json::Value::String(s)) => s,
            _ => return,
        };
        if new_origin != origin {
            return;
        }
        let mut seed = String::from("(function(){");
        for (k, v) in &entries {
            seed.push_str("sessionStorage.setItem(");
            seed.push_str(&serde_json::to_string(k).unwrap_or_else(|_| "null".to_string()));
            seed.push(',');
            seed.push_str(&serde_json::to_string(v).unwrap_or_else(|_| "null".to_string()));
            seed.push_str(");");
        }
        seed.push_str("})()");
        let _ = js.evaluate(&seed);
    }

    pub fn suspend_js(&mut self) {
        self.snapshot_session_storage();
        if let Some(js) = &self.js {
            if let Some(dom) = js.take_dom() {
                self.dom = Some(dom);
            }
            // Drain before the realm drops: console calls logged since the
            // last pump must survive suspension like the DOM does.
            self.suspended_console
                .extend(js.take_pending_console_calls());
        }
        self.js = None;
    }

    pub fn resume_js(&mut self) {
        if self.js.is_some() {
            return;
        }
        self.init_js();
    }

    pub fn has_js(&self) -> bool {
        self.js.is_some()
    }

    /// Drain queued console calls (level, message, page URL at log time)
    /// for CDP `Runtime.consoleAPICalled` and the session console ring.
    /// Returns the suspension buffer first, then the live realm's pending
    /// calls, so ordering across a suspend/resume round-trip is preserved.
    pub fn take_pending_console_calls(&mut self) -> Vec<(String, String, String)> {
        let mut calls = std::mem::take(&mut self.suspended_console);
        if let Some(js) = &self.js {
            calls.extend(js.take_pending_console_calls());
        }
        calls
    }

    /// Set the session-side dialog policy for window.confirm/prompt (see
    /// `JsState::dialog_accept`); None keeps the stored prompt text.
    pub fn set_dialog_policy(&self, accept: bool, prompt_text: Option<String>) {
        if let Some(js) = &self.js {
            js.set_dialog_policy(accept, prompt_text);
        }
    }

    /// Current dialog policy: (accept, prompt_text).
    pub fn dialog_policy(&self) -> (bool, Option<String>) {
        match &self.js {
            Some(js) => js.dialog_policy(),
            None => (false, None),
        }
    }
}
