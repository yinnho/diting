//! Route-decision and proxy-fallback seams for `/fetch`: the navigation
//! deadlines (full ceiling plus the starvation bail), the direct/proxy
//! route chooser, route-table bookkeeping, and the shared proxy retry
//! leg. Everything here is transport policy — which leg to take and what
//! to remember — with no content logic.

use super::browser::{build_browser, inject_cookies};
use crate::browser::Browser;
use crate::FetchRequest;
use anyhow::Result;

/// `/fetch` read ceiling. Interactive navigation stays at 30s so a hung
/// CDP session cannot wedge the dispatcher. This path holds no session
/// lock, and the dynamic-column client waits 100s. Angular Material's
/// autocomplete page commits, then spends the rest of a ~50s debug-engine
/// load on the module compile and the lazy route wave; the 30s ceiling
/// cancels that work and the live read is the title shell.
pub(super) const FETCH_NAV_TIMEOUT_MS: u64 = 60_000;

/// Direct-leg starvation bail deadline. A direct navigation with a proxy
/// behind it runs on this shorter deadline: a throttled CDN trickles a big
/// bundle for minutes at ~3KB/s while the worker's retry budget dies, and
/// the plain transport-death fallback never fires because the connection
/// is alive. 10s: a direct leg that has not even reached `load` by then is
/// either download-starved (bail to the proxy) or JS-bound (kept waiting
/// by the pending-script probe below) — a healthy direct load finishes
/// navigation well under it. `DITING_STARVE_BAIL_MS=0` disables (explicit
/// `DITING_NAV_TIMEOUT_MS` also keeps full operator control).
pub(super) const STARVE_BAIL_MS: u64 = 10_000;

/// Page-scoped navigation ceiling for one `/fetch`. `None` leaves the
/// process default, so an explicit `DITING_NAV_TIMEOUT_MS` still wins.
pub(super) fn fetch_nav_timeout_override(env_nav_timeout: Option<&str>) -> Option<u64> {
    match env_nav_timeout.map(str::trim).filter(|s| !s.is_empty()) {
        Some(_) => None,
        None => Some(FETCH_NAV_TIMEOUT_MS),
    }
}

/// The deadline the primary navigation actually runs on. A starving direct
/// leg (`starve_ms > 0`, already gated on direct + proxy configured) swaps
/// the 60s ceiling for the short bail; the operator's explicit
/// `DITING_NAV_TIMEOUT_MS` (override `None`) always wins over both.
pub(super) fn nav_deadline_ms(override_env: Option<&str>, starve_ms: u64) -> Option<u64> {
    fetch_nav_timeout_override(override_env).map(|ms| if starve_ms > 0 { starve_ms } else { ms })
}

/// Parse `DITING_STARVE_BAIL_MS`. Unparsable values keep the default: the
/// bail is fail-safe on, a typo should not silently re-enable 60s direct
/// trickle deaths. `0` is the explicit off switch.
pub(super) fn starve_bail_ms(env: Option<&str>) -> u64 {
    match env.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => s.parse().unwrap_or(STARVE_BAIL_MS),
        None => STARVE_BAIL_MS,
    }
}

/// Direct navigation outcomes update the route table only when the transport
/// really was direct. `use_proxy=false` can still mean a proxy leg through
/// should_auto_proxy; conversely, a requested proxy with no proxy configured
/// is still a direct leg. Proxy wins must stay sentenced, and proxy failures
/// are not evidence against the direct route.
pub(super) fn note_direct_navigation_outcome(url: &str, was_direct: bool, ok: bool) {
    note_direct_navigation_outcome_with(url, was_direct, ok, |host, ok| {
        diting::route_table::note_direct(host, ok);
    });
}

fn note_direct_navigation_outcome_with(
    url: &str,
    was_direct: bool,
    ok: bool,
    note: impl FnOnce(&str, bool),
) {
    if !was_direct {
        return;
    }
    if let Some(host) = diting::route_table::host_of(url) {
        note(&host, ok);
    }
}

/// The route build_browser will actually take, including the no-proxy-
/// configured fallback to direct.
pub(super) fn navigation_uses_direct(url: &str, use_proxy: bool, proxy_configured: bool) -> bool {
    !((use_proxy || crate::config::should_auto_proxy(url)) && proxy_configured)
}

/// The proxy fallback is starting after direct died. Pin the host to the
/// proxy for a short window (`route_table::note_proxy_pending`): the retry
/// this fetch's caller fires right after our failure must go straight
/// through the tunnel with a full budget — waiting for this leg's own
/// success to sentence the host (`note_proxy_win`) is too late when the
/// direct leg already ate most of the budget. A proxy win supersedes the
/// pin; a later direct success clears it.
pub(super) fn note_proxy_fallback_started(url: &str, pending: impl FnOnce(&str)) {
    if let Some(host) = diting::route_table::host_of(url) {
        pending(&host);
    }
}

/// Starvation discriminator, evaluated on the live page when the early
/// direct deadline fires with the document committed: how many
/// `<script src>` elements still have no completed `resource` performance
/// entry. Pending remote scripts at the bail point = the network is the
/// bottleneck (throttled CDN trickle). Zero pending = the page is JS-bound
/// (PoW challenge, hydration) and must keep its original budget. Errors
/// read as 0: fail toward keeping the page, never toward dropping it.
pub(super) const PENDING_REMOTE_SCRIPTS_JS: &str = r#"(function(){try{var done={};var es=performance.getEntriesByType('resource');for(var i=0;i<es.length;i++){done[es[i].name]=1;}var pend=0;var ss=document.querySelectorAll('script[src]');for(var j=0;j<ss.length;j++){if(!done[ss[j].src]){pend++;}}return pend;}catch(e){return 0;}})()"#;

/// Read the pending-scripts probe with retries. Two read hazards, both
/// handled: V8 numbers arrive as `Number(3.0)` — an f64 — so `as_u64` is
/// always `None` and the probe would read 0 forever (use `as_f64`); and a
/// just-cancelled navigation pipeline briefly holds the realm, so the first
/// evaluate right at the deadline can come back Null — retry until the
/// probe answers. `None` = the realm never answered; callers fail toward
/// keeping the direct page.
pub(super) fn pending_remote_scripts(page: &mut crate::page::Page) -> Option<u64> {
    for attempt in 0..4 {
        if let Some(n) = page.evaluate(PENDING_REMOTE_SCRIPTS_JS).as_f64() {
            return Some(n.max(0.0) as u64);
        }
        if attempt < 3 {
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
    }
    None
}

/// Build the proxy fallback leg shared by every retry arm: fresh proxied
/// browser + page, cookies injected, full navigation deadline, tolerant of
/// a committed-document deadline, and a route-table proxy win on success.
/// Callers must have dropped the direct browser and its page first (V8
/// isolates must be dropped in reverse creation order). `cause` prefixes
/// every error this can produce.
pub(super) async fn proxy_retry_pair(
    req: &FetchRequest,
    cause: &str,
) -> Result<(crate::page::Page, Browser)> {
    let browser2 = build_browser(true, &req.url, req.tls_fingerprint.as_deref())
        .map_err(|e| anyhow::anyhow!("{cause}; proxy retry failed to start: {e}"))?;
    inject_cookies(&browser2, &req.cookies, &req.url);
    let mut page2 = browser2
        .new_page()
        .await
        .map_err(|e| anyhow::anyhow!("{cause}; proxy retry failed to open a page: {e}"))?;
    if let Some(ms) =
        fetch_nav_timeout_override(std::env::var("DITING_NAV_TIMEOUT_MS").ok().as_deref())
    {
        page2.inner.set_navigation_timeout(Some(ms));
    }
    // Same committed-document tolerance as the primary navigation above: a
    // deadline after commit still leaves a readable live page.
    if let Err(e) = page2.goto(&req.url).await {
        if !e.to_string().contains("document committed") {
            return Err(anyhow::anyhow!(
                "{cause}; proxy retry failed to navigate: {e}"
            ));
        }
    }
    if let Some(h) = diting::route_table::host_of(&req.url) {
        diting::route_table::note_proxy_win(&h);
    }
    Ok((page2, browser2))
}

/// Pump the live page until `load` fires (`readyState` complete) or the
/// original navigation budget runs out. Only reached when the early
/// starvation deadline fired but the page is JS-bound: the original
/// deadline is what a non-starved run would have had.
pub(super) fn wait_for_load_or_deadline(page: &mut crate::page::Page, until: std::time::Instant) {
    while std::time::Instant::now() < until {
        if page.evaluate("document.readyState === 'complete'") == serde_json::Value::Bool(true) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
        let _ = page.evaluate("1+1");
    }
}

#[cfg(test)]
mod tests {
    use super::{
        fetch_nav_timeout_override, nav_deadline_ms, navigation_uses_direct,
        note_direct_navigation_outcome_with, note_proxy_fallback_started, pending_remote_scripts,
        starve_bail_ms,
    };
    use crate::browser::Browser;

    #[test]
    fn starve_bail_knob_default_on_zero_off_junk_keeps_default() {
        assert_eq!(starve_bail_ms(None), 10_000);
        assert_eq!(starve_bail_ms(Some("")), 10_000);
        assert_eq!(starve_bail_ms(Some("0")), 0);
        assert_eq!(starve_bail_ms(Some(" 6000 ")), 6_000);
        assert_eq!(starve_bail_ms(Some("junk")), 10_000);
    }

    #[test]
    fn nav_ceiling_env_wins_over_default() {
        assert_eq!(fetch_nav_timeout_override(None), Some(60_000));
        assert_eq!(fetch_nav_timeout_override(Some("30000")), None);
        assert_eq!(fetch_nav_timeout_override(Some("")), Some(60_000));
    }

    #[test]
    fn starve_bail_swaps_the_default_ceiling_but_never_the_operator_env() {
        // The bug this pins: the bail must beat the 60s default, not hide
        // behind an else-branch that the default already took.
        assert_eq!(nav_deadline_ms(None, 10_000), Some(10_000));
        assert_eq!(nav_deadline_ms(Some(""), 10_000), Some(10_000));
        assert_eq!(nav_deadline_ms(None, 0), Some(60_000));
        // An explicit operator deadline outranks the bail entirely.
        assert_eq!(nav_deadline_ms(Some("30000"), 10_000), None);
    }

    #[test]
    fn direct_navigation_outcomes_only_record_direct_evidence() {
        let mut calls = Vec::new();
        note_direct_navigation_outcome_with(
            "https://direct.example/path",
            true,
            true,
            |host, ok| {
                calls.push((host.to_string(), ok));
            },
        );
        note_direct_navigation_outcome_with(
            "https://direct.example/path",
            true,
            false,
            |host, ok| {
                calls.push((host.to_string(), ok));
            },
        );
        assert_eq!(
            calls,
            [
                ("direct.example".to_string(), true),
                ("direct.example".to_string(), false),
            ]
        );

        let mut proxied = false;
        for ok in [true, false] {
            note_direct_navigation_outcome_with("https://proxy.example/path", false, ok, |_, _| {
                proxied = true;
            });
        }
        assert!(
            !proxied,
            "proxy outcomes must not alter direct-route evidence"
        );
    }

    #[test]
    fn navigation_route_accounts_for_auto_proxy_and_missing_proxy_config() {
        assert!(navigation_uses_direct("https://example.com/", false, true));
        assert!(navigation_uses_direct("https://example.com/", true, false));
        assert!(!navigation_uses_direct("https://example.com/", true, true));
        assert!(!navigation_uses_direct(
            "https://en.wikipedia.org/wiki/Rust",
            false,
            true
        ));
        assert!(navigation_uses_direct(
            "https://en.wikipedia.org/wiki/Rust",
            false,
            false
        ));
    }

    #[test]
    fn proxy_fallback_start_pins_the_host_once() {
        let mut calls = Vec::new();
        note_proxy_fallback_started("https://direct.example/path", |host| {
            calls.push(host.to_string());
        });
        assert_eq!(calls, ["direct.example".to_string()]);
        note_proxy_fallback_started("not a url", |host| {
            calls.push(host.to_string());
        });
        assert_eq!(
            calls.len(),
            1,
            "unparseable URLs carry no host, so no pin may land"
        );
    }

    /// The starvation probe's contract on the field-failure shape: fast
    /// script + trickling script + instantly-refused script, navigation
    /// deadline firing mid-script-fetch with the document committed. At that
    /// instant all three `<script src>` are visible in the DOM and none has
    /// a resource entry (script timings are only pushed when the whole fetch
    /// phase finishes — a cancelled phase pushes nothing), so the probe must
    /// count every one of them undelivered: a true CDN trickle at the bail
    /// deadline reads > 0 and jumps to the proxy. The `readyState` quirk is
    /// pinned too: a cancelled pipeline already reports "complete" even with
    /// no script executed, which is why the JS-bound wait pump's early exit
    /// is never worse than reading the live page immediately (run12 parity).
    #[test]
    fn starve_probe_sees_undelivered_scripts_at_the_deadline() {
        let _env = crate::server::test_util::net_env_guard();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            use std::io::{Read as _, Write as _};
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut buf = [0u8; 8192];
                let n = stream.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = req
                    .lines()
                    .next()
                    .unwrap_or("")
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("")
                    .to_string();
                if path == "/slow.js" {
                    // Trickle: headers plus a first byte, then hold the
                    // socket open without ever finishing the body.
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\ncontent-type: application/javascript\r\n\
                         content-length: 2000\r\nconnection: close\r\n\r\nwindow.__slow=1; // "
                    );
                    let _ = stream.flush();
                    std::thread::sleep(std::time::Duration::from_secs(10));
                    continue;
                }
                let body: &str = if path == "/doc.html" {
                    r#"<html><head>
                        <script src="/ok.js"></script>
                        <script src="/slow.js"></script>
                        <script src="http://127.0.0.1:1/dead.js"></script>
                    </head><body>probe</body></html>"#
                } else {
                    "window.__ok=1;"
                };
                let resp = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: text/html\r\ncontent-length: {}\r\n\
                     connection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(resp.as_bytes());
                let _ = stream.flush();
            }
        });
        let (nav, probed, lens) = super::super::browser::run_on_local_runtime(move |_rt| {
                Box::pin(async move {
                    let browser = Browser::builder().stealth(false).build()?;
                    let mut page = browser.new_page().await?;
                    page.inner.set_navigation_timeout(Some(1500));
                    let nav = page
                        .goto(&format!("http://127.0.0.1:{port}/doc.html"))
                        .await
                        .err()
                        .map(|e| e.to_string())
                        .unwrap_or_default();
                    let probed = pending_remote_scripts(&mut page);
                    let lens = page.evaluate(
                        r#"(function(){try{
                        var es=performance.getEntriesByType('resource');
                        var rows=es.map(function(e){return e.name+' ts='+(e.transferSize===undefined?'u':e.transferSize)+' eb='+(e.encodedBodySize===undefined?'u':e.encodedBodySize);});
                        var ss=[].map.call(document.querySelectorAll('script[src]'),function(s){return String(s.src);});
                        return JSON.stringify({ready:document.readyState,entries:rows,scripts:ss,ok:window.__ok||null});
                    }catch(e){return 'ERR:'+e.message;}})()"#,
                    );
                    Ok((nav, probed, lens.as_str().unwrap_or("").to_string()))
                })
            })
            .unwrap();
        assert!(
            nav.contains("document committed"),
            "harness shape: the deadline must fire with the document committed, got: {nav}"
        );
        assert_eq!(
            probed,
            Some(3),
            "probe must count all three undelivered scripts; raw evidence:\n{lens}"
        );

        assert!(lens.contains("/ok.js"), "script elements visible: {lens}");
        assert!(
            lens.contains("\"ready\":\"complete\""),
            "readyState quirk: {lens}"
        );
        assert!(
            lens.contains("\"entries\":[]"),
            "a cancelled fetch phase pushes no resource timings: {lens}"
        );
    }
}
