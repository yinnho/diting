//! data: URL PNG decoding (render claim batch 5b).
//!
//! The first real pixels behind an `<img>`: a self-contained, no-network
//! data source whose bytes any snapshot of the engine can share.
//!
//! `decode_data_url_png` accepts `data:image/png[;base64],<payload>` only:
//! base64 is the one encoding real pages use for inline images, and the
//! unencoded (percent-encoded) form adds a decoder for zero real coverage.
//! Non-PNG media and network URLs return `None` — the img keeps its batch-5a
//! placeholder.
//!
//! Batch 6c adds [`ImageCache`]: decoded images keyed by their `src`, so a
//! layout re-run (or N imgs sharing one data: URL) decodes once, plus an
//! injected byte table for `http(s)` sources — the caller (the screenshot
//! prefetch path, over diting_net) fetches the bodies; the cache only ever
//! sees bytes. Batch 6d adds [`decode_jpeg`] and magic-byte dispatch in
//! [`decode_bytes`] (JPEG is the dominant photo format on real pages).
//! Issue #112 adds [`ImageCache::prefetch`]: the page's
//! unique srcs decode across scoped worker threads before the serial
//! layout walk, which then only clones cached `Arc`s.
//!
//! Since #42 ④ the codecs are driven directly (no `image` crate): PNG
//! through the vendored `diting_png`, JPEG through zune-jpeg, WebP through
//! image-webp, GIF through `gif`. Each decode leg reproduces the
//! normalization `image` 0.25.10 applied when it wrapped the same codecs
//! (colorspace selection, first-frame GIF composition, RGB→RGBA
//! promotion), so decoded pixels are unchanged from the intake baseline.

mod jpeg_encode;

pub use jpeg_encode::encode_jpeg_rgb;

/// Encode an 8-bit RGBA frame as PNG — the one encoder the full-page
/// screenshot, the page pump and the session frame path share.
pub fn encode_png_rgba(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity((width as usize) * (height as usize));
    {
        let mut encoder = diting_png::Encoder::new(std::io::Cursor::new(&mut out), width, height);
        encoder.set_color(diting_png::ColorType::Rgba);
        encoder.set_depth(diting_png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|e| format!("png encode header: {e}"))?;
        writer
            .write_image_data(rgba)
            .map_err(|e| format!("png encode data: {e}"))?;
        writer
            .finish()
            .map_err(|e| format!("png encode finish: {e}"))?;
    }
    Ok(out)
}

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use base64::Engine as _;

/// One decoded raster image: RGBA8, row-major, `width × height`.
#[derive(Debug, Clone)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    /// Straight-alpha RGBA8, `width * height * 4` bytes.
    pub rgba: Arc<Vec<u8>>,
}

impl DecodedImage {
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Self {
        Self {
            width,
            height,
            rgba: Arc::new(rgba),
        }
    }
}

/// Decoded-image store shared across layouts (batch 6c). `resolve(src)`
/// handles the two source forms the pipeline knows:
///
/// - `data:image/png;base64,…` — decoded on first sight, then cached.
/// - `http(s)://…` — looked up in the injected byte table (populated by
///   the caller's fetch pass) and decoded + cached. A miss stays a
///   miss (the img keeps its 5a placeholder); nothing here touches the
///   network.
pub struct ImageCache<'a> {
    network_bytes: Option<&'a HashMap<String, std::sync::Arc<Vec<u8>>>>,
    cache: RefCell<HashMap<String, Arc<DecodedImage>>>,
}

impl Default for ImageCache<'_> {
    fn default() -> Self {
        Self {
            network_bytes: None,
            cache: RefCell::new(HashMap::new()),
        }
    }
}

impl<'a> ImageCache<'a> {
    /// A cache that also resolves http(s)/file sources against `bytes`
    /// (absolute URL → response body).
    pub fn with_network(bytes: &'a HashMap<String, std::sync::Arc<Vec<u8>>>) -> Self {
        Self {
            network_bytes: Some(bytes),
            cache: RefCell::new(HashMap::new()),
        }
    }

    /// Decode (or recall from cache) the image behind an `<img src>`.
    pub fn resolve(&self, src: &str) -> Option<Arc<DecodedImage>> {
        if let Some(hit) = self.cache.borrow().get(src) {
            return Some(Arc::clone(hit));
        }
        let decoded = self.decode_uncached(src)?;
        self.cache
            .borrow_mut()
            .insert(src.to_string(), Arc::clone(&decoded));
        Some(decoded)
    }

    fn decode_uncached(&self, src: &str) -> Option<Arc<DecodedImage>> {
        decode_src_uncached(src, self.network_bytes).map(Arc::new)
    }

    /// Pre-decode a page's image srcs across the cores (issue #112). The
    /// layout pre-pass used to resolve img srcs inline — one decode at a
    /// time on the layout thread, invisible to the layout trace (which
    /// times styles/taffy, not this pass) — and an image-heavy page spent
    /// its whole screenshot wall clock there (~10.5s on the tmall publish
    /// page). Srcs already cached are skipped; each remaining unique src
    /// decodes exactly once on a scoped worker thread (round-robin chunks,
    /// capped at 8 threads so concurrent session layouts don't oversubscribe
    /// the box), and the serial `resolve` walk that follows only clones
    /// Arcs. A src that fails to decode simply stays uncached — the caller
    /// keeps the placeholder path, same as a serial miss.
    pub fn prefetch(&self, srcs: &[String]) {
        let todo: Vec<&String> = {
            let cached = self.cache.borrow();
            srcs.iter().filter(|s| !cached.contains_key(*s)).collect()
        };
        if todo.is_empty() {
            return;
        }
        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
            .clamp(1, 8)
            .min(todo.len());
        let bytes = self.network_bytes;
        // Round-robin chunks, not contiguous slices: decode cost varies
        // wildly by image size, and interleaving keeps the tail from idling
        // while one thread grinds a 2000×2000 master.
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..threads)
                .map(|t| {
                    let chunk: Vec<String> = todo
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| i % threads == t)
                        .map(|(_, s)| (*s).clone())
                        .collect();
                    scope.spawn(move || {
                        chunk
                            .into_iter()
                            .filter_map(|src| decode_src_uncached(&src, bytes).map(|d| (src, d)))
                            .collect::<Vec<(String, DecodedImage)>>()
                    })
                })
                .collect();
            for handle in handles {
                // A decode panic in a worker costs that chunk's images (the
                // serial resolve fallback re-attempts them); the rest of the
                // page's decode results still land.
                if let Ok(rows) = handle.join() {
                    let mut cached = self.cache.borrow_mut();
                    for (src, decoded) in rows {
                        cached.insert(src, Arc::new(decoded));
                    }
                }
            }
        });
    }
}

/// The pure half of the cache's decode path — `(src, byte table) → decoded
/// image`, no shared state — so [`ImageCache::prefetch`]'s scoped worker
/// threads can run it in parallel (the `RefCell` cache itself never crosses
/// a thread boundary).
fn decode_src_uncached(
    src: &str,
    network_bytes: Option<&HashMap<String, std::sync::Arc<Vec<u8>>>>,
) -> Option<DecodedImage> {
    if let Some(rest) = src.strip_prefix("data:") {
        return decode_data_url_png(&format!("data:{rest}"));
    }
    if src.starts_with("http://") || src.starts_with("https://") || src.starts_with("file://") {
        // file:// bytes only reach the table when the net-layer gate
        // admitted them (the collector refuses otherwise), so decoding
        // them here grants nothing the gate didn't already grant.
        if let Some(table) = network_bytes {
            let bytes = table.get(src)?;
            return decode_bytes(bytes);
        }
    }
    None
}

/// Decode a `data:image/png;base64,…` URL into a [`DecodedImage`].
/// Returns `None` for anything else (other media types, plain data URLs,
/// malformed base64/PNG) — callers fall back to the placeholder path.
pub fn decode_data_url_png(src: &str) -> Option<DecodedImage> {
    let rest = src.strip_prefix("data:")?;
    let comma = rest.find(',')?;
    let meta = &rest[..comma];
    let payload = &rest[comma + 1..];
    if !meta.starts_with("image/png") || !meta.contains(";base64") {
        return None;
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .ok()?;
    decode_png(&bytes)
}

/// Decode fetched image bytes by magic-number sniffing (batch 6d) —
/// content-type headers are untrusted/absent in practice. PNG, JPEG, WebP
/// and GIF each land in their leaf codec ([`diting_png`], zune-jpeg,
/// image-webp, `gif`), normalized to RGBA8 the way `image` 0.25.10's
/// wrappers did at intake (#42 ④).
pub fn decode_bytes(bytes: &[u8]) -> Option<DecodedImage> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        decode_png(bytes)
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        decode_jpeg(bytes)
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        decode_webp(bytes)
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        decode_gif(bytes)
    } else {
        None
    }
}

/// Decode JPEG bytes to RGBA8 through zune-jpeg, mirroring image 0.25.10's
/// wrapper (#42 ④): non-strict, no dimension cap, output colorspace = the
/// input's when it is one of RGB/RGBA/Luma/LumaA (else RGB), then promoted
/// to RGBA.
pub fn decode_jpeg(bytes: &[u8]) -> Option<DecodedImage> {
    use zune_core::bytestream::ZCursor;
    use zune_core::colorspace::ColorSpace;
    use zune_core::options::DecoderOptions;

    let options = DecoderOptions::default()
        .set_strict_mode(false)
        .set_max_width(usize::MAX)
        .set_max_height(usize::MAX);
    let mut decoder = zune_jpeg::JpegDecoder::new_with_options(ZCursor::new(bytes), options);
    decoder.decode_headers().ok()?;
    let (width, height) = decoder.dimensions()?;
    let orig = decoder.input_colorspace()?;
    let output = match orig {
        ColorSpace::RGB | ColorSpace::RGBA | ColorSpace::Luma | ColorSpace::LumaA => orig,
        _ => ColorSpace::RGB,
    };
    decoder.set_options(decoder.options().jpeg_set_out_colorspace(output));
    let buf = decoder.decode().ok()?;
    let rgba = match output {
        ColorSpace::Luma => {
            let mut out = Vec::with_capacity(buf.len() * 4);
            out.extend(buf.iter().flat_map(|&g| [g, g, g, 255]));
            out
        }
        ColorSpace::LumaA => {
            let mut out = Vec::with_capacity(buf.len() * 2);
            out.extend(
                buf.as_chunks::<2>()
                    .0
                    .iter()
                    .flat_map(|&[g, a]| [g, g, g, a]),
            );
            out
        }
        ColorSpace::RGBA => buf,
        _ => {
            let mut out = Vec::with_capacity(buf.len() / 3 * 4);
            out.extend(
                buf.as_chunks::<3>()
                    .0
                    .iter()
                    .flat_map(|&[r, g, b]| [r, g, b, 255]),
            );
            out
        }
    };
    Some(DecodedImage::new(width as u32, height as u32, rgba))
}

/// Decode WebP bytes (lossy and lossless) through image-webp (batch 7b):
/// `read_image` hands back RGB8 or RGBA8 depending on `has_alpha`; promote
/// the former, so both legs land as RGBA8.
pub fn decode_webp(bytes: &[u8]) -> Option<DecodedImage> {
    let mut decoder = image_webp::WebPDecoder::new(std::io::Cursor::new(bytes)).ok()?;
    let (w, h) = decoder.dimensions();
    let has_alpha = decoder.has_alpha();
    let mut buf = vec![0u8; decoder.output_buffer_size()?];
    decoder.read_image(&mut buf).ok()?;
    let rgba = if has_alpha {
        buf
    } else {
        let mut out = Vec::with_capacity(buf.len() / 3 * 4);
        out.extend(
            buf.as_chunks::<3>()
                .0
                .iter()
                .flat_map(|&[r, g, b]| [r, g, b, 255]),
        );
        out
    };
    Some(DecodedImage::new(w, h, rgba))
}

/// Decode a GIF's FIRST frame (batch 7e): static rendering has no
/// animation timeline. The frame is composed at its `(left, top)` offset
/// onto a fully transparent logical-screen canvas — the same first-frame
/// semantics image 0.25.10 applied (background color deliberately
/// ignored).
pub fn decode_gif(bytes: &[u8]) -> Option<DecodedImage> {
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut reader = options.read_info(bytes).ok()?;
    let (w, h) = (u32::from(reader.width()), u32::from(reader.height()));
    let mut canvas = vec![0u8; w as usize * h as usize * 4];
    let frame = reader.read_next_frame().ok()??;
    let (fw, fh) = (frame.width as usize, frame.height as usize);
    let (left, top) = (frame.left as usize, frame.top as usize);
    // Rows that hang off the canvas (broken files) clamp rather than panic.
    let copy = fw.min((w as usize).saturating_sub(left));
    for row in 0..fh {
        let dst_y = top + row;
        if dst_y >= h as usize {
            break;
        }
        let src_off = row * fw * 4;
        let dst_off = (dst_y * w as usize + left) * 4;
        canvas[dst_off..dst_off + copy * 4]
            .copy_from_slice(&frame.buffer[src_off..src_off + copy * 4]);
    }
    Some(DecodedImage::new(w, h, canvas))
}

/// Decode PNG bytes to RGBA8 (palette and sub-byte formats expanded, gray
/// promoted, missing alpha filled opaque) — the same normalization upstream
/// applies before handing `RasterImageData` to the painter.
pub fn decode_png(bytes: &[u8]) -> Option<DecodedImage> {
    let mut decoder = diting_png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(diting_png::Transformations::EXPAND);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size().unwrap_or(0)];
    let info = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width, info.height);
    let rgba = match info.color_type {
        diting_png::ColorType::Rgba => buf,
        diting_png::ColorType::Rgb => {
            let mut out = Vec::with_capacity(buf.len() / 3 * 4);
            for &[r, g, b] in buf.as_chunks::<3>().0 {
                out.extend_from_slice(&[r, g, b, 255]);
            }
            out
        }
        diting_png::ColorType::Grayscale => {
            let mut out = Vec::with_capacity(buf.len() * 4);
            for &g in &buf {
                out.extend_from_slice(&[g, g, g, 255]);
            }
            out
        }
        diting_png::ColorType::GrayscaleAlpha => {
            let mut out = Vec::with_capacity(buf.len() / 2 * 4);
            for &[g, a] in buf.as_chunks::<2>().0 {
                out.extend_from_slice(&[g, g, g, a]);
            }
            out
        }
        diting_png::ColorType::Indexed => return None, // EXPAND promised to remove this
    };
    Some(DecodedImage::new(w, h, rgba))
}

/// One parsed `srcset` candidate: the URL plus its descriptor.
#[derive(Debug, PartialEq)]
pub struct SrcsetCandidate {
    pub url: String,
    /// Pixel-density descriptor (`2x`); `None` when the candidate uses a
    /// width descriptor or no descriptor at all.
    pub density: Option<f32>,
    /// Intrinsic-width descriptor (`640w`), in CSS px.
    pub width: Option<f32>,
}

/// Parse a `srcset` attribute into its candidates (HTML §4.8.4.3.9,
/// simplified): comma-separated entries of `url [descriptor]`, where the URL
/// ends at the first whitespace (URLs with spaces must be percent-encoded —
/// same rule browsers apply) and the descriptor is either `Nx` (float) or
/// `W` (integer). Entries that parse to nothing (trailing comma, empty
/// descriptors) are skipped rather than poisoning their neighbors.
pub fn parse_srcset(srcset: &str) -> Vec<SrcsetCandidate> {
    let mut out = Vec::new();
    // A comma inside a URL is legal (data: payloads), so split only on
    // commas that terminate an entry: one followed by whitespace/end. The
    // common real-world form is ", " between entries.
    for part in split_srcset_entries(srcset) {
        let mut tokens = part.split_whitespace();
        let Some(url) = tokens.next() else { continue };
        let mut density = None;
        let mut width = None;
        if let Some(desc) = tokens.next() {
            if let Some(n) = desc.strip_suffix('w') {
                width = n.parse::<f32>().ok().filter(|w| *w > 0.0);
            } else if let Some(n) = desc.strip_suffix('x') {
                density = n.parse::<f32>().ok().filter(|d| *d > 0.0);
            } else {
                continue; // unknown descriptor unit → entry invalid per spec
            }
        }
        // A bare-URL candidate means 1x.
        out.push(SrcsetCandidate {
            url: url.to_string(),
            density: Some(density.unwrap_or(1.0)).filter(|_| density.is_some()),
            width,
        });
    }
    out
}

fn split_srcset_entries(srcset: &str) -> Vec<String> {
    let mut entries = Vec::new();
    let mut cur = String::new();
    let chars: Vec<char> = srcset.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == ',' {
            let rest_empty_or_ws = chars[i + 1..].first().is_none_or(|n| n.is_whitespace());
            if rest_empty_or_ws || cur.contains(char::is_whitespace) {
                entries.push(std::mem::take(&mut cur));
                i += 1;
                continue;
            }
        }
        cur.push(c);
        i += 1;
    }
    if !cur.trim().is_empty() {
        entries.push(cur);
    }
    entries
}

/// Pick the winning srcset candidate for a viewport (HTML selection rules,
/// product-simplified): prefer the smallest density ≥ 1x (we render at dpr 1);
/// otherwise, among width-descriptor candidates, the largest that fits the
/// viewport; when none fit, the smallest available. No candidates → None.
pub fn select_srcset_candidate(
    candidates: &[SrcsetCandidate],
    viewport_width: f32,
) -> Option<&SrcsetCandidate> {
    if candidates.is_empty() {
        return None;
    }
    // Density-based selection (no width descriptors anywhere).
    if candidates.iter().all(|c| c.width.is_none()) {
        return candidates
            .iter()
            .filter(|c| c.density.unwrap_or(1.0) >= 1.0)
            .min_by(|a, b| {
                a.density
                    .partial_cmp(&b.density)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .or_else(|| {
                candidates.iter().min_by(|a, b| {
                    a.density
                        .partial_cmp(&b.density)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
            });
    }
    // Width-based selection.
    let fitting: Vec<_> = candidates
        .iter()
        .filter(|c| c.width.map(|w| w <= viewport_width).unwrap_or(false))
        .collect();
    match fitting.iter().max_by_key(|c| c.width.unwrap_or(0.0) as u32) {
        Some(c) => Some(c),
        // Nothing fits: the smallest candidate keeps the box bounded.
        None => candidates
            .iter()
            .min_by_key(|c| c.width.unwrap_or(f32::MAX) as u32),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trip: encode a two-quadrant RGBA PNG, wrap it in a data URL,
    /// decode it back — bytes and dimensions identical.
    #[test]
    fn data_url_png_round_trip() {
        let (w, h) = (4u32, 2u32);
        let mut rgba = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let left = x < w / 2;
                rgba.extend_from_slice(if left ^ (y < h / 2) {
                    &[200, 40, 40, 255]
                } else {
                    &[40, 40, 200, 255]
                });
            }
        }

        let mut png_bytes = Vec::new();
        {
            let mut enc = diting_png::Encoder::new(&mut png_bytes, w, h);
            enc.set_color(diting_png::ColorType::Rgba);
            enc.set_depth(diting_png::BitDepth::Eight);
            let mut writer = enc.write_header().unwrap();
            writer.write_image_data(&rgba).unwrap();
        }

        let url = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&png_bytes)
        );
        let img = decode_data_url_png(&url).expect("decodes");
        assert_eq!((img.width, img.height), (w, h));
        assert_eq!(img.rgba.as_slice(), rgba.as_slice());
    }

    /// Non-PNG media, missing base64, and garbage payloads all decline
    /// (None → the caller keeps the placeholder path).
    #[test]
    fn data_url_rejects_non_png_and_garbage() {
        assert!(decode_data_url_png("https://example.com/x.png").is_none());
        assert!(decode_data_url_png("data:image/jpeg;base64,AAAA").is_none());
        assert!(decode_data_url_png("data:image/png,raw-not-supported").is_none());
        assert!(decode_data_url_png("data:image/png;base64,!!!not-b64!!!").is_none());
    }

    fn two_by_one_png() -> (Vec<u8>, DecodedImage) {
        let rgba = vec![200u8, 40, 40, 255, 40, 40, 200, 255];
        let mut png_bytes = Vec::new();
        {
            let mut enc = diting_png::Encoder::new(&mut png_bytes, 2, 1);
            enc.set_color(diting_png::ColorType::Rgba);
            enc.set_depth(diting_png::BitDepth::Eight);
            let mut writer = enc.write_header().unwrap();
            writer.write_image_data(&rgba).unwrap();
        }
        (png_bytes, DecodedImage::new(2, 1, rgba))
    }

    /// The cache decodes each distinct src once: repeated resolves hand
    /// back the SAME Arc, and two imgs sharing one data: URL share it too.
    #[test]
    fn cache_dedupes_decodes() {
        let (png_bytes, img) = two_by_one_png();
        let url = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&png_bytes)
        );
        let cache = ImageCache::default();
        let a = cache.resolve(&url).expect("data URL decodes");
        let b = cache.resolve(&url).expect("cached");
        assert!(Arc::ptr_eq(&a, &b), "second resolve is the cached Arc");
        assert_eq!((a.width, a.height), (img.width, img.height));
    }

    /// http(s) sources decode from the injected byte table — the fetch is
    /// the caller's job. Unknown URLs stay None (placeholder path); a
    /// non-PNG body also declines.
    #[test]
    fn network_sources_resolve_from_injected_bytes() {
        let (png_bytes, _img) = two_by_one_png();
        let mut bytes: HashMap<String, std::sync::Arc<Vec<u8>>> = HashMap::new();
        bytes.insert(
            "https://example.com/a.png".to_string(),
            std::sync::Arc::new(png_bytes.clone()),
        );
        bytes.insert(
            "https://example.com/bad.png".to_string(),
            std::sync::Arc::new(b"not a png".to_vec()),
        );

        let cache = ImageCache::with_network(&bytes);
        let hit = cache
            .resolve("https://example.com/a.png")
            .expect("fetched PNG decodes");
        assert_eq!((hit.width, hit.height), (2, 1));
        assert!(
            cache.resolve("https://example.com/bad.png").is_none(),
            "non-PNG body declines"
        );
        assert!(
            cache.resolve("https://example.com/missing.png").is_none(),
            "URL not in the table stays a placeholder"
        );
        // No network table at all: http(s) never resolves.
        assert!(ImageCache::default()
            .resolve("https://example.com/a.png")
            .is_none());
    }

    /// #112: prefetch decodes a page's unique srcs across scoped worker
    /// threads and fills the cache, so the layout walk's resolve() only
    /// clones Arcs. 64 distinct data: PNGs (pixel-tagged by index) mixed
    /// with entries that can't decode — a missing URL and a garbage body —
    /// must decode every hit to its own pixels without the misses poisoning
    /// their chunk neighbors, and a src resolved BEFORE the prefetch keeps
    /// its Arc (prefetch skips, never replaces, cached entries).
    #[test]
    fn prefetch_decodes_unique_srcs_in_parallel() {
        let n = 64; // enough to exercise the multi-thread round-robin chunks
        let mut urls = Vec::with_capacity(n);
        for i in 0..n {
            let rgba = vec![i as u8, 40, 200, 255, 40, 40, 200, 255];
            let mut png_bytes = Vec::new();
            {
                let mut enc = diting_png::Encoder::new(&mut png_bytes, 2, 1);
                enc.set_color(diting_png::ColorType::Rgba);
                enc.set_depth(diting_png::BitDepth::Eight);
                let mut writer = enc.write_header().unwrap();
                writer.write_image_data(&rgba).unwrap();
            }
            urls.push(format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(&png_bytes)
            ));
        }

        let mut bytes: HashMap<String, std::sync::Arc<Vec<u8>>> = HashMap::new();
        bytes.insert(
            "https://example.com/bad.png".to_string(),
            std::sync::Arc::new(b"not a png".to_vec()),
        );
        let all: Vec<String> = urls
            .iter()
            .cloned()
            .chain([
                "https://example.com/missing.png".to_string(),
                "https://example.com/bad.png".to_string(),
            ])
            .collect();

        let cache = ImageCache::with_network(&bytes);
        // Resolve one up front: prefetch must skip it, not re-decode/replace.
        let early = cache.resolve(&urls[0]).expect("data URL decodes");
        cache.prefetch(&all);
        let again = cache.resolve(&urls[0]).expect("still resolves");
        assert!(Arc::ptr_eq(&early, &again), "prefetch keeps cached Arcs");

        for (i, url) in urls.iter().enumerate() {
            let img = cache
                .resolve(url)
                .unwrap_or_else(|| panic!("src {i} prefetched"));
            assert_eq!(&img.rgba[0..4], &[i as u8, 40, 200, 255], "pixel tag {i}");
        }
        assert!(
            cache.resolve("https://example.com/missing.png").is_none(),
            "URL not in the table stays a placeholder"
        );
        assert!(
            cache.resolve("https://example.com/bad.png").is_none(),
            "garbage body declines without poisoning chunk neighbors"
        );
        // Empty input is a no-op, not a panic.
        cache.prefetch(&[]);
    }

    /// JPEG bodies: encode a solid image with the house encoder
    /// (`encode_jpeg_rgb`, the port of image 0.25.10's baseline encoder),
    /// decode it via `decode_bytes` (magic sniff) and via the data: URL
    /// path.
    #[test]
    fn jpeg_decodes_and_sniffs_by_magic() {
        let mut jpeg_bytes = Vec::new();
        encode_jpeg_rgb(&mut jpeg_bytes, &[204u8; 4 * 2 * 3], 4, 2, 75).expect("encodes");
        assert!(jpeg_bytes.starts_with(&[0xFF, 0xD8, 0xFF]), "JPEG magic");

        let decoded = decode_bytes(&jpeg_bytes).expect("sniffs and decodes JPEG");
        assert_eq!((decoded.width, decoded.height), (4, 2));

        // Garbage with no known magic declines.
        assert!(decode_bytes(b"GIF89a-nope").is_none());

        // The data: URL path stays PNG-gated (image/jpeg data URLs decline
        // — inline images on real pages are PNG/base64; network JPEG comes
        // through the byte table instead).
        let b64 = base64::engine::general_purpose::STANDARD.encode(&jpeg_bytes);
        assert!(decode_data_url_png(&format!("data:image/jpeg;base64,{b64}")).is_none());
    }

    /// Progressive JPEG (SOF2): zune-jpeg decodes it like baseline. Real
    /// pages ship plenty of these; a decode failure would render the
    /// placeholder instead of the image.
    #[test]
    fn progressive_jpeg_decodes() {
        let bytes = include_bytes!("fixtures/progressive.jpg");
        assert!(bytes.starts_with(&[0xFF, 0xD8, 0xFF]), "JPEG magic");
        let decoded = decode_bytes(bytes).expect("progressive JPEG decodes");
        assert_eq!((decoded.width, decoded.height), (60, 40));
    }

    /// WebP bodies decode through the RIFF/WEBP magic branch (batch 7b).
    /// The fixture is a Pillow-encoded lossless VP8L with an alpha channel
    /// (regenerated 2026-11-07: image 0.25.10's lossless WebP encoder left
    /// with the crate, and image-webp is decode-only), so the decoded RGBA
    /// must round-trip the source pixels bit-for-bit.
    #[test]
    fn webp_decodes_and_sniffs_by_magic() {
        let mut rgba = Vec::new();
        for y in 0..4u32 {
            for x in 0..6u32 {
                let red = (x < 3) ^ (y < 2);
                let a = if (x, y) == (0, 0) { 128 } else { 255 };
                let px: [u8; 4] = if red {
                    [200, 40, 40, a]
                } else {
                    [40, 40, 200, a]
                };
                rgba.extend_from_slice(&px);
            }
        }
        let webp_bytes = include_bytes!("fixtures/lossless.webp");
        assert_eq!(&webp_bytes[0..4], b"RIFF", "RIFF magic");
        assert_eq!(&webp_bytes[8..12], b"WEBP", "WEBP magic");

        let decoded = decode_bytes(webp_bytes).expect("sniffs and decodes WebP");
        assert_eq!((decoded.width, decoded.height), (6, 4));
        assert_eq!(
            decoded.rgba.as_slice(),
            rgba.as_slice(),
            "lossless round-trip"
        );

        // Truncated RIFF header declines rather than panicking.
        assert!(decode_bytes(b"RIFF\x00\x00\x00\x00WEB").is_none());
    }

    /// GIF bodies decode to their FIRST frame (batch 7e). A real 2×1 GIF
    /// (palette: index 0 = red, index 1 = blue; generated once with
    /// Pillow's encoder) decodes through the magic branch to exactly those
    /// pixels; a truncated header declines.
    #[test]
    fn gif_decodes_first_frame() {
        #[rustfmt::skip]
        let b: [u8; 44] = [
            71, 73, 70, 56, 55, 97, 2, 0, 1, 0, 129, 0,
            0, 200, 40, 40, 40, 40, 200, 0, 0, 0, 0, 0,
            0, 44, 0, 0, 0, 0, 2, 0, 1, 0, 0, 8,
            5, 0, 1, 4, 8, 8, 0, 59,
        ];

        let decoded = decode_bytes(&b).expect("decodes real GIF");
        assert_eq!((decoded.width, decoded.height), (2, 1));
        assert_eq!(&decoded.rgba[0..4], &[200, 40, 40, 255], "pixel 0 = red");
        assert_eq!(&decoded.rgba[4..8], &[40, 40, 200, 255], "pixel 1 = blue");

        // Truncated header declines rather than panicking.
        assert!(decode_bytes(b"GIF89a-truncated").is_none());
    }

    /// #112 phase-split probe (ignored by default — run with
    /// `--ignored --nocapture` to read the split). An image-heavy page's
    /// screenshot wall clock decomposed into pure decode / compute_styles /
    /// full layout (which includes the decode via this cache) / paint, on
    /// 30 distinct 800×800 JPEGs — the tmall publish-page shape (the
    /// layout trace showed styles 0.5s while the wall clock was 11.5s, so
    /// the missing ~10s lives somewhere this test names). Asserts only
    /// correctness (every image resolved, painted); the timings are prints.
    #[test]
    #[ignore = "perf probe: prints a phase split, asserts only correctness"]
    fn image_heavy_page_phase_split() {
        use std::time::Instant;

        const N: usize = 30;
        let mut jpgs: Vec<Vec<u8>> = Vec::with_capacity(N);
        for i in 0..N {
            // Gradient + per-image offset: distinct bytes per URL (the cache
            // would dedupe identical srcs) and compressible like a photo.
            let mut rgb = Vec::with_capacity(800 * 800 * 3);
            for y in 0..800u32 {
                for x in 0..800u32 {
                    rgb.extend_from_slice(&[
                        ((x / 3) as u8).wrapping_add(i as u8 * 8),
                        ((y / 3) as u8).wrapping_add(i as u8 * 5),
                        (((x + y) / 4) as u8).wrapping_add(i as u8 * 3),
                    ]);
                }
            }
            let mut buf = Vec::new();
            encode_jpeg_rgb(&mut buf, &rgb, 800, 800, 75).expect("encodes jpeg");
            jpgs.push(buf);
        }

        // Phase 1: pure decode of the same bodies, serial loop — the cost
        // the layout pass pays inline today.
        let t = Instant::now();
        for b in &jpgs {
            assert!(decode_bytes(b).is_some(), "fixture must decode");
        }
        let t_decode = t.elapsed();

        // The page: a grid of N imgs at tmall SKU scale, absolute URLs
        // resolved against the injected byte table (the session
        // screenshot's prefetch route).
        let html = {
            let mut s = String::from("<html><body><div>");
            for i in 0..N {
                s.push_str(&format!(
                    "<img src=\"http://grid.test/img{i}.jpg\" width=\"200\" height=\"200\">"
                ));
            }
            s.push_str("</div></body></html>");
            s
        };
        let tree = diting_dom::tree_sink::parse_html(&html);
        let rules =
            diting_css::parse_stylesheet_for("", (1280.0, 900.0), diting_css::CssMediaType::Screen);
        let t = Instant::now();
        let styles = crate::compute_styles(&tree, &rules, (1280.0, 900.0));
        let t_styles = t.elapsed();

        let table: HashMap<String, std::sync::Arc<Vec<u8>>> = jpgs
            .iter()
            .enumerate()
            .map(|(i, b)| {
                (
                    format!("http://grid.test/img{i}.jpg"),
                    std::sync::Arc::new(b.clone()),
                )
            })
            .collect();
        let fonts = crate::diting_fonts::font_book();
        let t = Instant::now();
        let (rects, items) = crate::layout_dom_with_paint_and_images(
            &tree,
            &styles,
            &fonts,
            1280.0,
            900.0,
            Some(&table),
            Some("http://grid.test/"),
        );
        let t_layout = t.elapsed();
        let n_img_items = items
            .iter()
            .filter(|it| matches!(it, crate::PaintItem::Image { .. }))
            .count();
        assert_eq!(n_img_items, N, "every img must carry decoded pixels");

        let t = Instant::now();
        let mut canvas = crate::paint::Canvas::new_filled(1280, 900, [255, 255, 255, 255]);
        crate::paint::execute(&items, &fonts, &mut canvas);
        let t_paint = t.elapsed();

        println!(
            "#112 phase split (N={N} x 800x800 JPEG, dev build): \
             decode={t_decode:?} styles={t_styles:?} layout_full={t_layout:?} \
             paint={t_paint:?} rects={} items={}",
            rects.len(),
            items.len()
        );
    }
}
