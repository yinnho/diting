//! Challenge ride-out for the acquire paths: Cloudflare Turnstile, the
//! ByteDance WAF JS challenge, the Anubis proof-of-work interstitial.
//!
//! Challenge heuristics here are acquisition-path *shape* detection — not
//! platform business. Site policy stays in `workflow/`.

use anyhow::Result;

/// Check if the current page is a Cloudflare challenge.
fn is_cloudflare_challenge(page: &mut crate::page::Page) -> bool {
    let title_val = page.evaluate("document.title");
    let title = title_val.as_str().unwrap_or("");
    if title.contains("Just a moment") || title.contains("Attention Required") {
        return true;
    }
    let has_turnstile_val =
        page.evaluate(r#"!!document.querySelector('iframe[src*="challenges.cloudflare.com"]')"#);
    has_turnstile_val.as_bool().unwrap_or(false)
}

/// After goto(), detect and auto-bypass Cloudflare Turnstile challenges.
/// Waits for `cf_clearance` cookie, then re-navigates if the page hasn't
/// auto-redirected.
pub(crate) async fn maybe_bypass_challenge(page: &mut crate::page::Page) -> Result<()> {
    if !is_cloudflare_challenge(page) {
        return Ok(());
    }
    let url = page.url();
    tracing::info!(
        "Cloudflare challenge detected at {}, auto-bypassing...",
        url
    );

    // Give Turnstile JS time to execute (managed challenge auto-completes).
    page.settle(5000).await;

    // Wait for cf_clearance cookie (the signal that Turnstile passed).
    match page
        .wait_for_cookie("cf_clearance", std::time::Duration::from_secs(25))
        .await
    {
        Ok(()) => {
            tracing::info!("cf_clearance cookie received, challenge passed");
            // If the page didn't auto-redirect, re-navigate.
            if is_cloudflare_challenge(page) {
                tracing::info!("Re-navigating to {} after challenge pass", url);
                page.goto(&url).await?;
                page.settle(3000).await;
            }
        }
        Err(e) => {
            tracing::warn!("cf_clearance timeout: {}", e);
            // Don't fail hard — the page might still have usable content
            // (e.g. invisible challenge that completed without cookie).
        }
    }
    Ok(())
}

/// Check if the current page is a bytedance WAF JS challenge (juejin.cn
/// class): a tiny stub whose `<body onload="readygo()">` brute-forces a
/// SHA256 PoW over 1ms ticks and reloads with a `_wafchallengeid` answer
/// cookie. Signature: a global `readygo` function + almost no visible text
/// (real pages don't define that global; challenge stubs carry no content).
///
/// **Gray / deploy-facing (not platform business):** this is acquisition-path
/// challenge *shape* detection for `/fetch` (and kin) so a PoW interstitial
/// does not look like empty content. It is not a seller/login flow and must
/// not grow into site-specific publish logic — leave WAF ride-out here;
/// site policy stays in `workflow/`. See architecture note: core = generic
/// engine, platform compat → flow.
fn is_byte_waf_challenge(page: &mut crate::page::Page) -> bool {
    let val = page.evaluate(
        "(function() {\
            try {\
                if (!document.body) return false;\
                var text = (document.body.innerText || '').replace(/\\s+/g, '');\
                return text.length < 100 && typeof window.readygo === 'function';\
            } catch (e) { return false; }\
        })()",
    );
    val.as_bool().unwrap_or(false)
}

/// After goto(), ride out the bytedance WAF JS challenge. The challenge
/// solves itself in-page — readygo() sets the answer cookie, then calls
/// location.reload() — but the reload is recorded as a pending JS
/// navigation that nobody in the stateless fetch path drains (sessions
/// have a command-loop pump; do_fetch doesn't), and the PoW ticks need
/// event-loop pumping that the caller's default 0s settle never gives.
/// So: pump, drain the reload, settle the real page.
pub(super) async fn maybe_bypass_byte_waf(page: &mut crate::page::Page) -> Result<()> {
    if !is_byte_waf_challenge(page) {
        return Ok(());
    }
    let url = page.url();
    tracing::info!(
        "byte-WAF challenge detected at {}, waiting for PoW + reload",
        url
    );

    // Pump so readygo's 1ms interval ticks (the PoW is trivially small —
    // observed answers are single/low double digits) and the reload lands
    // in pending_navigation.
    page.settle(2000).await;

    // Drain the JS-initiated reload; the re-navigation runs load events
    // again, this time with the answer cookie attached by the client.
    match page.process_pending_navigation().await {
        Ok(true) => {
            page.settle(1500).await;
        }
        Ok(false) => {
            // No reload recorded — either it fired inside settle's own
            // navigation handling or the solver failed. Re-check below.
        }
        Err(e) => {
            tracing::warn!("byte-WAF reload navigation failed: {}", e);
        }
    }

    if is_byte_waf_challenge(page) {
        tracing::warn!("byte-WAF challenge at {} still present after settle", url);
    }
    Ok(())
}

/// Check if the current page is an Anubis proof-of-work interstitial
/// (TecharoHQ anubis — winehq / kernel.org / fedora-docs class). The
/// challenge HTML carries `script[src*="/.within.website/x/cmd/anubis/"]`
/// plus a `Making sure you're not a bot!` title; both markers vanish the
/// moment the pass-challenge navigation replaces the document, which is
/// exactly the window this ride-out covers. Gray / deploy-facing, same
/// stance as the byte-WAF shape above: acquisition-path detection only.
fn is_anubis_challenge(page: &mut crate::page::Page) -> bool {
    let val = page.evaluate(
        "(function(){\
            try {\
                if (document.querySelector('script[src*=\"/.within.website/x/cmd/anubis/\"]')) return true;\
                return (document.title || '').indexOf(\"Making sure you're not a bot\") >= 0;\
            } catch (e) { return false; }\
        })()",
    );
    val.as_bool().unwrap_or(false)
}

/// The `#status` line of an Anubis interstitial: `Loading…` until
/// make-challenge answers, then `Speed: …kH/s` while the workers race,
/// `Success!` when a nonce lands.
fn anubis_status(page: &mut crate::page::Page) -> String {
    page.evaluate("((document.getElementById('status')||{}).textContent||'').trim()")
        .as_str()
        .unwrap_or("")
        .to_string()
}

/// Ride out an Anubis PoW interstitial. The challenge solves itself
/// in-page (8 workers racing SHA-256 to difficulty 4, ~65k hashes median —
/// ~10s in this engine with an unlucky tail past 20s), then
/// `location.replace()`s to pass-challenge. That navigation lands in
/// `pending_navigation`, and nothing else in the stateless fetch path
/// drains it after the initial goto chain closed — the exact shape the
/// byte-WAF reload had. Pump in slices, drain the navigation when it
/// lands, and bail early when the status never leaves `Loading…` (the
/// module never executed — engine module-referrer bug class), so a dead
/// challenge doesn't burn the full budget.
pub(super) async fn maybe_bypass_anubis(page: &mut crate::page::Page) -> Result<()> {
    if !is_anubis_challenge(page) {
        return Ok(());
    }
    let url = page.url();
    tracing::info!(
        "anubis PoW challenge detected at {}, riding out the proof-of-work",
        url
    );

    const SLICE_MS: u64 = 500;
    // Difficulty-4 PoW is ~65k hashes median — ~12s in this engine — with
    // an unlucky tail (winehq's IP-deterministic challenge needs 136k ≈
    // 20-25s). The fetch worker wall is 27s: goto ~4s + this budget +
    // extraction ~2s. 22s buys the median twice over and most of the tail;
    // past it the case is lost either way.
    const BUDGET_MS: u64 = 22_000;
    // make-challenge through a slow tunnel can take seconds; 8s of a
    // status frozen at the initial value means the module is dead, not slow.
    const LOADING_STALL_MS: u64 = 8_000;
    let started = std::time::Instant::now();
    let mut loading_since: Option<std::time::Instant> = None;
    loop {
        page.settle(SLICE_MS).await;
        // Drain a landed pass-challenge navigation (or any JS navigation
        // the settle woke up); navigation failure keeps the challenge page,
        // which the caller extracts as-is.
        match page.process_pending_navigation().await {
            Ok(true) => {
                // Real document (or SPA route sync): give the landing page
                // a beat before re-testing for a re-challenge below.
                page.settle(2_000).await;
            }
            Ok(false) => {}
            Err(e) => {
                tracing::warn!("anubis pass-challenge navigation failed: {}", e);
            }
        }
        if !is_anubis_challenge(page) {
            tracing::info!(
                elapsed_ms = started.elapsed().as_millis() as u64,
                "anubis challenge cleared at {}",
                url
            );
            return Ok(());
        }
        if started.elapsed().as_millis() as u64 >= BUDGET_MS {
            break;
        }
        let status = anubis_status(page);
        if status == "Loading..." {
            let since = *loading_since.get_or_insert_with(std::time::Instant::now);
            if since.elapsed().as_millis() as u64 >= LOADING_STALL_MS {
                tracing::warn!(
                    "anubis status stalled at 'Loading...' for {}ms at {} — module never executed",
                    since.elapsed().as_millis() as u64,
                    url
                );
                return Ok(());
            }
        } else {
            loading_since = None;
        }
    }
    tracing::warn!(
        "anubis challenge at {} still present after {}ms",
        url,
        BUDGET_MS
    );
    Ok(())
}
