//! Documents that `/fetch` must not hand to the renderer: a binary response
//! (PDF / DOCX) asked back as text goes through the salvage extractors, and a
//! document embedded in a live page (pdf.js viewers, `<embed>` PDFs) is
//! probed out of the DOM and appended to the text.

use crate::FetchRequest;
use std::sync::Arc;

/// Byte-fetch a binary document (PDF / DOCX) and pull its text out with the
/// salvage extractor ([`crate::server::pdf_text`]). `None` on any failure —
/// the caller then keeps the binary-document refusal, so a parser miss
/// degrades to the honest error rather than a half-empty answer. Mirrors
/// `http_fetch`'s proxy decision instead of the browser's because the bytes
/// are the same origin the navigation just reached; no rate re-check (the
/// navigation already counted this request).
pub(super) async fn binary_document_text(url: &str, use_proxy: bool) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let use_proxy = use_proxy || crate::config::should_auto_proxy(url);
    let proxy = if use_proxy {
        crate::config::proxy_from_env()
    } else {
        None
    };
    let jar = Arc::new(diting::diting_net::CookieJar::new());
    let mut client =
        diting::diting_net::HttpClient::with_full_options(jar, proxy.as_deref(), false);
    client.emulate_chrome = true;
    let resp = client.fetch(&parsed).await.ok()?;
    if resp.status != 200 {
        return None;
    }
    let text = if resp.body.starts_with(b"%PDF") {
        crate::server::pdf_text::pdf_to_text(&resp.body)
    } else if resp.body.starts_with(b"PK\x03\x04") {
        crate::server::docx_text::docx_to_text(&resp.body)
    } else {
        None
    }?;
    // A parse that yielded soup or nothing is not an answer.
    if text.trim().len() < 200 {
        return None;
    }
    Some(text)
}

/// DOM+app probe for embedded documents (returns a JSON array of absolute
/// URLs). Two families, both invisible to the navigation response:
/// - HTML embeds: `<embed>/<object>/<iframe>` with a `.pdf` src or an
///   `application/pdf` type (PDFObject and friends put the URL in the DOM).
/// - pdf.js viewers: no DOM trace at all — the document URL lives in the
///   app's own options (`PDFViewerApplicationOptions.get("defaultUrl")`,
///   the build default kept by virtually every deployment), the opened
///   document's `PDFViewerApplication.url`/`_downloadUrl`, or the `?file=`
///   query param. In our engine pdf.js initializes but never fetches its
///   default document, so this config read is the only signal there is.
const EMBEDDED_DOC_PROBE_JS: &str = r#"
(function(){
  var out = [], seen = {};
  function add(u){
    if (!u) return;
    try { u = new URL(u, location.href).href } catch(e){ return }
    u = u.split('#')[0];
    if (u && !seen[u]) { seen[u] = 1; out.push(u); }
  }
  document.querySelectorAll('embed,object,iframe').forEach(function(el){
    var u = (el.getAttribute('src') || el.getAttribute('data') || '').trim();
    var t = (el.getAttribute('type') || '').toLowerCase();
    if (!u || (!/\.pdf([?#]|$)/i.test(u) && t.indexOf('pdf') < 0)) return;
    add(u);
  });
  try {
    var q = new URLSearchParams(location.search).get('file');
    if (q) add(q);
  } catch(e){}
  try {
    var o = window.PDFViewerApplicationOptions;
    var d = o && o.get && o.get('defaultUrl');
    if (d) add(d);
  } catch(e){}
  try {
    var a = window.PDFViewerApplication;
    if (a) { if (a.url) add(a.url); if (a._downloadUrl) add(a._downloadUrl); }
  } catch(e){}
  return JSON.stringify(out);
})()
"#;

/// Embedded-document splicer for text/markdown fetches: probe, dedupe,
/// byte-fetch each candidate through [`binary_document_text`], join the
/// survivors under labeled headers. Guarded by the request clock — a page
/// that burned its budget on settle must not add 2× the fetch timeout on
/// top. `None` when there is nothing to append (no hits, no extractable
/// bytes, or over budget).
pub(super) async fn embedded_documents_text(
    req: &FetchRequest,
    page: &mut crate::page::Page,
    started: std::time::Instant,
) -> Option<String> {
    // Same frame as the settle tail's 23s budget, but tighter: leave the
    // response-transfer room that tail comment promises.
    const EMBED_DOC_DEADLINE_MS: u64 = 18_000;
    const EMBED_DOC_MAX: usize = 2;
    let urls: Vec<String> =
        serde_json::from_str(page.evaluate(EMBEDDED_DOC_PROBE_JS).as_str()?).ok()?;
    let mut parts: Vec<String> = Vec::new();
    for url in urls {
        if parts.len() >= EMBED_DOC_MAX
            || started.elapsed().as_millis() as u64 > EMBED_DOC_DEADLINE_MS
        {
            break;
        }
        // The navigation response itself already went through the
        // content-type gate above — refetching it here would double-count
        // the request and duplicate its text.
        if url == req.url || url.is_empty() {
            continue;
        }
        let url = url.trim().to_string();
        if let Some(text) = binary_document_text(&url, req.use_proxy).await {
            tracing::info!(
                url = %url,
                chars = text.len(),
                "fetch: embedded document extracted"
            );
            parts.push(format!("[Embedded document: {url}]\n\n{text}"));
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n\n"))
    }
}
