//! Response facts for `/fetch`: the media manifest (#232), the `layout:true`
//! geometry, and the `resources:true` body pass (#279).

/// Media-manifest probe (#232). Runs after the page settles, so img
/// `currentSrc` is the post-lazyload live value. Mirrors the attribute set
/// of render.rs::collect_media (the Tier-1 static twin) — keep the two in
/// sync when adding a source.
pub(super) const MEDIA_PROBE: &str = r#"(function(){
var out=[],seen={};
function add(u){if(!u)return;u=String(u).trim();if(!u||u.indexOf('data:')===0)return;if(seen[u])return;seen[u]=1;out.push(u)}
function plain(el,names){for(var i=0;i<names.length;i++){var v=el.getAttribute(names[i]);if(v)add(v)}}
function first(el,names){for(var i=0;i<names.length;i++){var v=el.getAttribute(names[i]);if(v){add(v.split(',')[0].trim().split(/\s+/)[0])}}}
var imgs=document.querySelectorAll('img');
for(var i=0;i<imgs.length;i++){var im=imgs[i];add(im.currentSrc);plain(im,['src','data-src','data-original','data-lazy-src']);first(im,['srcset','data-srcset'])}
var ms=document.querySelectorAll('meta[property="og:image"],meta[name="og:image"]');
for(var i=0;i<ms.length;i++){add(ms[i].getAttribute('content'))}
var vs=document.querySelectorAll('video,audio,video source,audio source');
for(var i=0;i<vs.length;i++){var v=vs[i];add(v.src);plain(v,['data-src','poster'])}
return JSON.stringify(out)})()"#;

/// Parse the MEDIA_PROBE result (a JSON string of URLs) into the response's
/// `media` list. Any probe hiccup yields an empty list — media is a lead
/// surface, never worth failing the fetch over.
pub(crate) fn media_from_eval(v: serde_json::Value) -> Vec<String> {
    let raw = match v.as_str() {
        Some(s) => s,
        None => return Vec::new(),
    };
    match serde_json::from_str::<Vec<String>>(raw) {
        Ok(urls) => crate::render::finalize_media(urls),
        Err(_) => Vec::new(),
    }
}

/// Parse the `layout:true` geometry probe: the eval returns a JSON string
/// `{"h":<scrollHeight>,"w":<scrollWidth>}`. Any hiccup (null, non-JSON,
/// NaN-ish values) is `None` — the knob reports what layout measured, not
/// an error page's zeros.
pub(crate) fn parse_layout_facts(v: &serde_json::Value) -> Option<crate::LayoutFacts> {
    let raw = v.as_str()?;
    let obj: serde_json::Map<String, serde_json::Value> = serde_json::from_str(raw).ok()?;
    Some(crate::LayoutFacts {
        scroll_height: obj.get("h")?.as_i64()?,
        scroll_width: obj.get("w")?.as_i64()?,
    })
}

/// Turn the prefetched bodies into `resources` response rows: size always,
/// pixel dimensions when the bytes decode (magic sniff, PNG/JPEG/WebP/GIF).
/// Bodies stay out of the response — the caller has the URLs in `media`.
/// The `resources:true` body pass, feature-split: with `screenshot` it
/// prefetches through the bounded machinery the screenshot path uses
/// (#279); without, the knob answers an empty table (the capability
/// rides the feature, like /screenshot itself).
pub(super) async fn prefetch_resource_facts(
    // Only the `screenshot` build reads the page; the bare build answers
    // an empty table without touching it.
    #[cfg_attr(not(feature = "screenshot"), allow(unused_variables))] page: &mut crate::page::Page,
) -> Vec<crate::ResourceFact> {
    #[cfg(feature = "screenshot")]
    {
        let t0 = std::time::Instant::now();
        let html = page.content();
        let url = page.url();
        let fetched = crate::screenshot::prefetch_render_resources(page, &url, &html, 1280.0).await;
        let facts = resource_facts(&fetched);
        tracing::info!(
            "fetch resources forced: {} bodies, {} with dimensions, in {:?}",
            facts.len(),
            facts.iter().filter(|f| f.width.is_some()).count(),
            t0.elapsed()
        );
        facts
    }
    #[cfg(not(feature = "screenshot"))]
    {
        Vec::new()
    }
}

#[cfg(feature = "screenshot")]
pub(crate) fn resource_facts(
    fetched: &crate::screenshot::PrefetchedResources,
) -> Vec<crate::ResourceFact> {
    let mut facts: Vec<crate::ResourceFact> = fetched
        .iter()
        .map(|(url, body)| {
            let decoded = diting::diting_layout::image::decode_bytes(body);
            crate::ResourceFact {
                url: url.clone(),
                bytes: body.len(),
                width: decoded.as_ref().map(|d| d.width),
                height: decoded.as_ref().map(|d| d.height),
            }
        })
        .collect();
    // Stable order across runs — HashMap iteration is not.
    facts.sort_by(|a, b| a.url.cmp(&b.url));
    facts
}
