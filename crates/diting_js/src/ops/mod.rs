use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io::Write as _;
use std::rc::Rc;
use std::sync::{Arc, PoisonError};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use diting_dom::{AttachShadowError, DomTree, NodeData, NodeId, ShadowRootMode};
use diting_net::{CookieJar, HttpClient};
use diting_ops::diting_op;
use diting_runtime::state::OpState;
use url::Url;
#[cfg(feature = "screenshot")]
pub use storage::store_image_bytes;
#[cfg(test)]
pub(crate) use storage::reset_local_storage_for_tests;
use storage::storage_file;
pub(crate) use fetch_ops::{FetchCredentials, FetchSettlements, glob_match, op_fetch_url_body, FETCH_REDIRECT_LIMIT};
use fetch_ops::REFERRER_POLICY_TOKENS;
#[cfg(feature = "screenshot")]
pub use layout_run::text_ink_extent;
pub use state::{InterceptResolution, InterceptedRequest, INTERCEPT_RESOLUTION_TIMEOUT_MS, ResourceTimingRecord, JsState, JsNetworkEvent, StoredNetworkResponseBody, SharedState};
pub(crate) use state::{BlobStoreMap, fetch_body_byte_limit};
use state::{response_body_entry_limit, response_body_byte_limit};
use script_start::{mark_script_subtree_started, propagate_script_start_state};
use dom::{parse_nid, DomCmd};
use dom_cmds::{dom_cmd_attr_state, dom_cmd_create, dom_cmd_document, dom_cmd_mutate, dom_cmd_query, dom_cmd_tree};
use dom_style::{dom_cmd_geometry, dom_cmd_scroll_style, dom_cmd_sheets};
#[cfg(feature = "screenshot")]
use dom_style::style_property_map;
#[cfg(feature = "screenshot")]
use layout_run::{ensure_layout_run, iframe_layout_run, is_element_scroller, is_viewport_fixed, parse_iframe_viewport, sticky_shifts, LayoutRun};
#[cfg(feature = "screenshot")]
use dom_style::{attr_write_is_layout_inert, style_write_is_paint_only};

mod band;
mod canvas;
mod css;
mod ecdsa;
mod ed25519;
mod encoding;
mod fetch_gate;
mod fetch_walk;
mod node_order;
mod props;
mod text_geom;

#[cfg(feature = "screenshot")]
pub use band::BandFrame;
#[cfg(feature = "screenshot")]
pub use band::{
    band_frame, band_frame_cut, band_frame_cut_with_text, band_frame_scaled, band_frame_with_text,
};
pub(crate) use canvas::{op_canvas_jpeg, op_canvas_png};
#[cfg(feature = "screenshot")]
pub(crate) use canvas::{op_canvas_store, op_canvas_text};
pub(crate) use ecdsa::{
    op_subtle_ecdh_derive_bits, op_subtle_ecdsa_generate, op_subtle_ecdsa_import_private,
    op_subtle_ecdsa_import_public, op_subtle_ecdsa_sign, op_subtle_ecdsa_verify,
};
pub(crate) use ed25519::{
    op_subtle_ed25519_check, op_subtle_ed25519_public, op_subtle_ed25519_sign,
    op_subtle_ed25519_verify,
};
pub(crate) use fetch_walk::{fetch_url_walk, FetchFailure, FetchNetworkEvent, FetchWalkDeps};
use node_order::{compare_node_order, node_child_index};
mod cors;
mod crypto_secret;
mod referrer;
use referrer::resolve_document_referrer_policy;
pub use referrer::{fetch_referer, last_valid_referrer_token, resolve_referrer_policy_from};
mod fetch_client;
#[cfg(feature = "screenshot")]
use cssom_value::{cb_inline_for, computed_style_value};
pub use fetch_client::cached_request_client;
pub(crate) use fetch_client::select_request_client;
mod cssom_value;
mod image_info;
pub(crate) use cors::request_origin;
pub(crate) use crypto_secret::{
    crypto_err, op_random_bytes, op_subtle_aes_cbc, op_subtle_aes_ctr, op_subtle_aes_gcm,
    op_subtle_aes_kw, op_subtle_digest, op_subtle_hkdf, op_subtle_hmac, pbkdf2_derive,
};
pub use fetch_gate::validate_fetch_url;
pub(crate) use image_info::op_image_info;
mod video_info;
pub(crate) use video_info::op_video_info;
mod crypto_rsa;
pub(crate) use crypto_rsa::{
    op_subtle_rsa_import_pkcs8, op_subtle_rsa_import_private, op_subtle_rsa_import_public,
    op_subtle_rsa_import_spki, op_subtle_rsa_pkcs1v15_verify, op_subtle_rsa_pss_verify,
    rsa_generate_flat, rsa_oaep, rsa_pkcs1v15_sign, rsa_pss_sign,
};

#[diting_op]
fn op_navigate(state: &OpState, #[string] url: &str, #[string] method: &str, #[string] body: &str) {
    let gs = state.borrow::<SharedState>().clone();
    let mut gs = gs.borrow_mut();
    // Only queue the navigation — do NOT move the realm URL here. The URL is
    // written on commit (init_js → set_url from the page's committed URL).
    // Moving it early let synchronous JS between the assignment and the
    // actual navigation read and write another origin's cookies through
    // document.cookie, whose ops derive the domain from this URL
    // (SOP bypass, obscura #940).
    gs.pending_navigation = Some((url.to_string(), method.to_string(), body.to_string()));
}

/// Queue a session-history jump (history.back/forward/go across a document
/// boundary — obscura#1105). `target` is an absolute index into the page's
/// real session history; the pump (`Page::process_pending_navigation`) moves
/// the cursor and refetches the entry's document.
#[diting_op]
fn op_history_jump_to(state: &OpState, target: i32) {
    let gs = state.borrow::<SharedState>().clone();
    let mut gs = gs.borrow_mut();
    gs.pending_history_jump = Some(target);
}

/// Free body of `op_sleep` — the hand glue (`diting_js::async_glue`)
/// calls this directly off our Vm host (10d swap; the #[op2] wrapper died
/// with the deno host).
pub(crate) async fn op_sleep_body(millis: u64) {
    // #230: tokio's timer wheel ticks at 1ms granularity — even a
    // zero-length sleep parks until the next wheel tick before its
    // deadline reads as due, taxing every delay-0 round-trip with ~1ms of
    // pure wheel latency (measured 1154µs/trip while the JS work under it
    // cost 55ns/item: the macrotask trampoline takes one trip per 64
    // items, so a 340k-timer fxg boot drained at ~5.7k/s = 59s). A zero
    // sleep completes at the first drive instead — settlement still rides
    // the settle queue into the next loop pass, so the task-boundary
    // discipline (#37/#327 line: never settle on the JS stack) is intact;
    // it just doesn't wait for a wheel tick to say "due".
    if millis == 0 {
        return;
    }
    // Reactor-less contexts (plain #[test] isolates): a tokio timer would
    // panic mid-poll inside v8, which aborts the whole test binary. Resolve
    // at the next event-loop checkpoint instead — production always runs
    // under a reactor and never takes this path.
    if tokio::runtime::Handle::try_current().is_err() {
        return;
    }
    tokio::time::sleep(std::time::Duration::from_millis(millis)).await;
}

// Records a binding call from page JS. The CDP layer drains this queue
// after every dispatch and emits one `Runtime.bindingCalled` event per
// entry, that's how puppeteer's `page.exposeFunction` callbacks fire.
#[diting_op]
fn op_binding_called(state: &OpState, #[string] name: &str, #[string] payload: &str) {
    let gs = state.borrow::<SharedState>().clone();
    let mut gs = gs.borrow_mut();
    gs.pending_binding_calls
        .push((name.to_string(), payload.to_string()));
    // The CDP layer drains this after every dispatch, but a page can loop
    // calling an exposed binding many times inside one long evaluate — same
    // unbounded-queue shape as js_network_events (obscura #705 class).
    const MAX_PENDING_BINDING_CALLS: usize = 4096;
    if gs.pending_binding_calls.len() > MAX_PENDING_BINDING_CALLS {
        let overflow = gs.pending_binding_calls.len() - MAX_PENDING_BINDING_CALLS;
        gs.pending_binding_calls.drain(0..overflow);
    }
}

/// Serialize a parsed URL into the WHATWG IDL component shape consumed by the
/// `URL` class in bootstrap.js. Getters read these fields directly so no op
/// call happens per property access.
fn url_components(u: &url::Url) -> serde_json::Value {
    let port = u.port().map(|p| p.to_string()).unwrap_or_default();
    let hostname = u.host_str().unwrap_or("").to_string();
    let host = if hostname.is_empty() {
        String::new()
    } else if port.is_empty() {
        hostname.clone()
    } else {
        format!("{hostname}:{port}")
    };
    // WHATWG search/hash getters return "" for a null OR empty component.
    let search = match u.query() {
        Some(q) if !q.is_empty() => format!("?{q}"),
        _ => String::new(),
    };
    let hash = match u.fragment() {
        Some(f) if !f.is_empty() => format!("#{f}"),
        _ => String::new(),
    };
    serde_json::json!({
        "ok": true,
        "href": u.as_str(),
        "protocol": format!("{}:", u.scheme()),
        "username": u.username(),
        "password": u.password().unwrap_or(""),
        "host": host,
        "hostname": hostname,
        "port": port,
        "pathname": u.path(),
        "search": search,
        "hash": hash,
        "origin": u.origin().ascii_serialization(),
    })
}

/// Parse `href` (optionally resolved against `base`) with the WHATWG-compliant
/// `url` crate. Returns the component JSON, or `{"ok":false}` when the input is
/// not a valid URL (the JS side turns that into a TypeError, per spec).
#[diting_op]
#[string]
fn op_url_parse(#[string] href: &str, #[string] base: &str) -> String {
    // The url crate can panic on a few pathological inputs (internal range
    // slicing); catch it so a bad URL never aborts the process.
    std::panic::catch_unwind(|| {
        let parsed = if base.is_empty() {
            url::Url::parse(href)
        } else {
            url::Url::parse(base).and_then(|b| b.join(href))
        };
        match parsed {
            Ok(u) => url_components(&u).to_string(),
            Err(_) => "{\"ok\":false}".to_string(),
        }
    })
    .unwrap_or_else(|_| "{\"ok\":false}".to_string())
}

/// Apply a WHATWG URL setter (`part` = href/protocol/username/password/host/
/// hostname/port/pathname/search/hash) to `href` and return the new components.
fn url_set_inner(href: &str, part: &str, value: &str) -> Option<serde_json::Value> {
    let mut u = url::Url::parse(href).ok()?;
    match part {
        "href" => {
            let nu = url::Url::parse(value).ok()?;
            return Some(url_components(&nu));
        }
        "protocol" => {
            let _ = u.set_scheme(value.trim_end_matches(':'));
        }
        "username" => {
            let _ = u.set_username(value);
        }
        "password" => {
            let _ = u.set_password(if value.is_empty() { None } else { Some(value) });
        }
        "host" => set_host_port(&mut u, value),
        "hostname" => {
            if !value.is_empty() {
                let _ = u.set_host(Some(value));
            }
        }
        "port" => {
            // WHATWG port state consumes the leading ASCII digits and stops
            // at the first non-digit, so "8080abc" sets 8080; no digits or a
            // number above 65535 leaves the port untouched.
            if value.is_empty() {
                let _ = u.set_port(None);
            } else {
                let digits: String = value.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(p) = digits.parse::<u32>() {
                    if p <= 65535 {
                        let _ = u.set_port(Some(p as u16));
                    }
                }
            }
        }
        "pathname" => u.set_path(value),
        "search" => {
            // Null only when the ORIGINAL value is the empty string; a bare
            // "?" sets an empty query that still serializes its delimiter
            // (href ends in "?").
            let q = value.strip_prefix('?').unwrap_or(value);
            u.set_query(if value.is_empty() { None } else { Some(q) });
        }
        "hash" => {
            let f = value.strip_prefix('#').unwrap_or(value);
            u.set_fragment(if value.is_empty() { None } else { Some(f) });
        }
        _ => {}
    }
    Some(url_components(&u))
}

#[diting_op]
#[string]
fn op_url_set(#[string] href: &str, #[string] part: &str, #[string] value: &str) -> String {
    // Some url-crate setters panic on pathological inputs (the url-setters WPT
    // tests exercise these). Catch the unwind and treat it as a no-op setter,
    // returning the URL unchanged, which matches WHATWG "do nothing on invalid".
    match std::panic::catch_unwind(|| url_set_inner(href, part, value)) {
        Ok(Some(v)) => v.to_string(),
        _ => match url::Url::parse(href) {
            Ok(u) => url_components(&u).to_string(),
            Err(_) => "{\"ok\":false}".to_string(),
        },
    }
}

/// Best-effort `host` setter: split `host[:port]` (handling bracketed IPv6) and
/// apply hostname and port separately, since `url::Url::set_host` rejects a port.
fn set_host_port(u: &mut url::Url, value: &str) {
    // IPv6 literals are bracketed; never split inside the brackets.
    if value.starts_with('[') {
        if let Some(close) = value.find(']') {
            let host = &value[..=close];
            let rest = &value[close + 1..];
            if u.set_host(Some(host)).is_ok() {
                if let Some(p) = rest.strip_prefix(':') {
                    if let Ok(pn) = p.parse::<u16>() {
                        let _ = u.set_port(Some(pn));
                    }
                }
            }
            return;
        }
    }
    if let Some(idx) = value.rfind(':') {
        let (h, p) = (&value[..idx], &value[idx + 1..]);
        if p.is_empty() || p.chars().all(|c| c.is_ascii_digit()) {
            if u.set_host(Some(h)).is_ok() {
                if p.is_empty() {
                    let _ = u.set_port(None);
                } else if let Ok(pn) = p.parse::<u16>() {
                    let _ = u.set_port(Some(pn));
                }
            }
            return;
        }
    }
    let _ = u.set_host(Some(value));
}

/// Resolve `href` against optional `base` and return only the serialized
/// absolute URL (no component breakdown). Used by the hot `a.href`/`area.href`
/// getter, which only needs the resolved string, so it avoids building and
/// re-parsing the full component JSON. Returns "" when the input is invalid.
#[diting_op]
#[string]
fn op_url_resolve(#[string] href: &str, #[string] base: &str) -> String {
    std::panic::catch_unwind(|| {
        let parsed = if base.is_empty() {
            url::Url::parse(href)
        } else {
            url::Url::parse(base).and_then(|b| b.join(href))
        };
        parsed.map(|u| u.as_str().to_string()).unwrap_or_default()
    })
    .unwrap_or_default()
}

/// Parse and merge an inline document import map (upstream 34373c3). Returns
/// "" on success or the parse/merge error message, so the bootstrap caller can
/// surface it as a script error event without a rejected-op round-trip.
#[diting_op]
#[string]
fn op_add_import_map(
    state: &OpState,
    #[string] source: String,
    #[string] base_url: String,
) -> String {
    let shared = state.borrow::<SharedState>().clone();
    let import_map = shared.borrow().import_map.clone();
    let parsed = match crate::import_map::ImportMap::parse(&source, &base_url) {
        Ok(map) => map,
        Err(error) => return error,
    };
    let result = match import_map.try_borrow_mut() {
        Ok(mut current) => {
            current.merge(parsed);
            String::new()
        }
        Err(_) => "Import map is already borrowed".to_string(),
    };
    result
}

/// 批240 / obscura#1100: SVGGeometryElement.getTotalLength() backing. Chrome
/// measures through SkPathMeasure (circle r=25 → 156.0674, not 2πr), so these
/// numbers line up with Chrome (conic chord measure for ovals, analytic for
/// explicit d beziers). Non-geometry nodes / missing geometry read 0.
/// Anti-panic boundary: the path data comes from the page, and a parser panic
/// must degrade to 0 (the op_dom posture) rather than unwind through the FFI
/// frame. Measurement lives in the layout crate, so the no-screenshot build
/// reads 0 — geometry measurement without the layout engine is meaningless
/// anyway.
#[cfg(feature = "screenshot")]
#[diting_op]
fn op_svg_path_len(state: &OpState, #[smi] nid: u32) -> f64 {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        svg_measure(state, nid)
            .map(|m| m.total() as f64)
            .unwrap_or(0.0)
    }))
    .unwrap_or(0.0)
}

#[cfg(not(feature = "screenshot"))]
#[diting_op]
fn op_svg_path_len(_: &OpState, #[smi] _: u32) -> f64 {
    0.0
}

/// getPointAtLength backing — "x\0y" (cheap unambiguous pair; the bootstrap
/// side splits and wraps it in an SVGPoint). Negative/overlong targets
/// clamp to the endpoints, Chrome's contract.
#[cfg(feature = "screenshot")]
#[diting_op]
#[string]
fn op_svg_path_point(state: &OpState, #[smi] nid: u32, target: f64) -> String {
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        svg_measure(state, nid).map(|m| m.point_at(target as f32))
    }))
    .unwrap_or(None);
    match r {
        Some((x, y)) if x.is_finite() && y.is_finite() => format!("{x}\0{y}"),
        _ => "0\u{0}0".to_string(),
    }
}

#[cfg(not(feature = "screenshot"))]
#[diting_op]
#[string]
fn op_svg_path_point(_: &OpState, #[smi] _: u32, _: f64) -> String {
    "0\u{0}0".to_string()
}

#[cfg(feature = "screenshot")]
fn svg_measure(state: &OpState, nid: u32) -> Option<diting_layout::svg::PathMeasure> {
    let gs = state.borrow::<SharedState>().clone();
    let gs = gs.borrow();
    // `&gs.dom?` is the lint's shape but moves out of the RefCell borrow;
    // `as_ref()` keeps the Option borrowed.
    let dom = gs.dom.as_ref()?;
    let node = dom.get_node(NodeId::new(nid))?;
    let tag = match &node.data {
        diting_dom::NodeData::Element { name, .. } => name.local.as_str(),
        _ => return None,
    };
    diting_layout::svg::build_path_measure(tag, &|a| node.get_attribute(a).map(|s| s.to_string()))
}

/// (#78) Sub-ms wall clock for performance.now(). Chrome coarsens the timer
/// to 100µs (non-crossOriginIsolated) and reports floats; an integral
/// Date.now()-based value is a timing-entropy tell on the jsvmp environment
/// scan path. Quantized to 0.1ms steps — epoch-ms times ten stays well under
/// 2^53, so the f64 rounding is exact (finer quantization would not be).
#[diting_op]
fn op_clock_ms() -> f64 {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
        * 1000.0;
    (ms * 10.0).round() / 10.0
}

/// The op table our own host installs (10d flip): every #[diting_op]
/// wrapper already returns OUR OpDecl, and the four async ops come in as
/// hand glue from `diting_js::async_glue` (op_fetch_url / op_sleep /
/// op_ws_open / op_ws_next_message). The list mirrors the retired
/// build_extension() entry-for-entry; the to_deno adapter and the
/// Extension wrapper died with the deno host.
pub fn our_decls() -> Vec<diting_runtime::op_reg::OpDecl> {
    let mut decls = vec![
        css::op_css_supports(),
        props::op_register_property(),
        dom::op_dom(),
        op_clock_ms(),
        op_svg_path_len(),
        op_svg_path_point(),
        dom::op_shadow_attach(),
        console::op_console_msg(),
        console::op_dialog(),
        script_start::op_script_mark_started(),
        script_start::op_script_try_start(),
        script_start::op_dyn_script_fetch_begin(),
        script_start::op_dyn_script_fetch_end(),
        storage::op_blob_register(),
        storage::op_blob_revoke(),
        fetch_ops::op_fetch_take(),
        fetch_ops::op_fetch_url_sync(),
        op_image_info(),
        op_video_info(),
        op_canvas_png(),
        op_canvas_jpeg(),
        #[cfg(feature = "screenshot")]
        op_canvas_store(),
        #[cfg(feature = "screenshot")]
        op_canvas_text(),
        storage::op_get_cookies(),
        storage::op_set_cookie(),
        storage::op_storage_read(),
        storage::op_storage_write(),
        op_navigate(),
        op_history_jump_to(),
        op_binding_called(),
        op_subtle_digest(),
        op_subtle_hmac(),
        op_subtle_aes_gcm(),
        op_subtle_aes_cbc(),
        op_subtle_aes_ctr(),
        op_subtle_aes_kw(),
        op_subtle_rsa_import_public(),
        op_subtle_rsa_import_spki(),
        op_subtle_rsa_import_private(),
        op_subtle_rsa_import_pkcs8(),
        op_subtle_rsa_pkcs1v15_verify(),
        op_subtle_rsa_pss_verify(),
        op_subtle_hkdf(),
        op_subtle_ecdsa_generate(),
        op_subtle_ecdsa_import_private(),
        op_subtle_ecdsa_import_public(),
        op_subtle_ecdh_derive_bits(),
        op_subtle_ecdsa_sign(),
        op_subtle_ecdsa_verify(),
        op_subtle_ed25519_check(),
        op_subtle_ed25519_public(),
        op_subtle_ed25519_sign(),
        op_subtle_ed25519_verify(),
        op_random_bytes(),
        op_url_parse(),
        op_url_set(),
        op_url_resolve(),
        op_add_import_map(),
        encoding::op_encoding_for_label(),
        encoding::op_text_decode(),
        encoding::op_url_encode_query(),
        crate::ws::op_ws_send(),
        crate::ws::op_ws_close(),
    ];
    // The async four ride the hand glue (promise stub + OpDriver spawn).
    decls.extend(crate::async_glue::all_decls());
    decls
}

mod state;
mod script_start;
mod dom;
mod dom_cmds;
mod dom_style;
mod layout_run;
mod console;
mod fetch_ops;
mod storage;
#[cfg(test)]
mod tests;
