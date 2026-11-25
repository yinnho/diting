//! `Page.captureScreenshot`: the session's own capture — the live viewport
//! band, or the whole document when the client asked past the viewport — cut
//! down to the `clip` the client asked for. Playwright sends a clip on every
//! screenshot, and an element screenshot answered with the whole viewport is
//! a wrong picture the client cannot tell from a right one.
//!
//! A clip is taken as Chrome takes one (`PageHandler::CaptureScreenshot`
//! and Blink's `DevToolsEmulator`, Chrome 158 measured): the image is
//! `round(edge × dpr × scale)` device pixels on each edge, one at least; its
//! corner is the clip's own, to the nearest device pixel; the page is
//! rasterized at `dpr × scale` over the layout viewport it already has — a
//! bar half the viewport wide stays that, it is not laid out to the clip. A
//! rect that starts left of the document is special in Blink: it gets
//! neither the offset nor the scale, and is answered with the viewport's
//! top-left corner at the page's own `dpr`.
//!
//! The document capture is a re-parse of the serialized page (see
//! `session::screenshot`): it shows what `outerHTML` shows. Below 1x there
//! is no raster and above 3x none is allocated; both are refused.
//!
//! What lies under no capture is white. The band covers the viewport and the
//! document capture the document; a rect past the edge of either is filled
//! out to its own size, and one off the viewport is asked of the document
//! only when `captureBeyondViewport` says so. A rect on the viewport is
//! always cut from the band — the live page, where the document capture is
//! a re-parse — whatever the flag says.
//!
//! The parameters are read as Chrome reads them ([`params`]): five numbers
//! in a `clip`, none left out, and a clip of no width or of no height is
//! refused in Chrome's words.
//!
//! Where Chrome's answers differ, in words:
//! - It answers a clip with a negative size or a scale of 0 with the whole
//!   view; here each is refused — an image of something else is not what
//!   was asked for.
//! - `fromSurface: false` is a snapshot of the browser window, and there is
//!   no window.
//! - jpeg and webp are not encoded.
//! - Under no page content it paints the page's canvas colour — the root's
//!   background, else the body's — where the engine paints an element's
//!   background over that element's box only; white is what it paints below
//!   a page shorter than the viewport, so white is what fills here.
//! - At a scale that is not a whole number each engine rounds the clip's
//!   corner its own way, and a pixel here is not a pixel there.
//!
//! A clip past [`MAX_PIXELS`] is refused in Chrome's words for a capture it
//! cannot make.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use diting::diting_layout::image::{decode_png, encode_png_rgba, DecodedImage};
use serde_json::{json, Value};

use super::params::{invalid, kinds, number, object, optional_boolean, optional_string, Kind};
use super::{forward, ok, page_of, session_msg, Failure, Out, HUB};
use crate::session::send_screenshot;

const METHOD: &str = "Page.captureScreenshot";
/// The largest raster the session allocates for (`resolve_dpr` clamps at it).
const MAX_SCALE: f64 = 3.0;
/// Rects this close to an edge are taken as on it: scroll offsets and
/// Playwright's integer-enclosing rects are not always exact.
const EDGE: f64 = 1.0;
/// The most device pixels one image is padded out to. A rect past the edge
/// of its capture costs a white field of its own size, and a page is no
/// bound on that.
const MAX_PIXELS: u64 = 1 << 28;

/// A `clip`: CSS pixels inside the document, rasterized at `scale`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Clip {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    scale: f64,
}

/// The page as the capture meets it, CSS pixels.
#[derive(Debug, PartialEq)]
struct Geometry {
    scroll_x: f64,
    scroll_y: f64,
    view_width: f64,
    view_height: f64,
    content_width: f64,
    content_height: f64,
    /// What the page reports as `devicePixelRatio`.
    dpr: f64,
}

/// A rect in device pixels, to cut out of a capture: its top-left corner
/// from the capture's — negative when it starts above or left of it — and
/// its size.
#[derive(Debug, PartialEq)]
struct Cut {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[derive(Debug, PartialEq)]
enum Take {
    /// The visible viewport, whole.
    Viewport,
    /// The document, whole.
    Document,
    /// The rect, cut out of the viewport band or — when `true` — of the
    /// whole document.
    Cut(Cut, bool),
    /// The rect, white: under no capture the client asked for.
    Plain(Cut),
}

pub(super) async fn capture(id: &Value, session: &str, params: &Value) -> Result<Out, Failure> {
    // Every field is read before any value is looked at, as Chrome reads them.
    let format = optional_string(params, "format")?;
    kinds(
        params,
        &[
            ("quality", Kind::Integer),
            ("fromSurface", Kind::Bool),
            ("captureBeyondViewport", Kind::Bool),
            ("optimizeForSpeed", Kind::Bool),
        ],
    )?;
    let clip = match object(params, "clip")? {
        None => None,
        Some(clip) => Some(clip_of(clip)?),
    };
    let beyond = optional_boolean(params, "captureBeyondViewport")? == Some(true);
    // Chrome's words, in Chrome's order — before it looks at the format.
    if clip.is_some_and(|clip| clip.width == 0.0) {
        return Err("Cannot take screenshot with 0 width.".into());
    }
    if clip.is_some_and(|clip| clip.height == 0.0) {
        return Err("Cannot take screenshot with 0 height.".into());
    }
    let format = format.unwrap_or("png");
    match format {
        "png" => {}
        "jpeg" | "webp" => {
            return Err(Failure::from(format!(
                "{METHOD}: only png is supported, not '{format}'"
            )));
        }
        _ => return Err(invalid("Invalid image format")),
    }
    if optional_boolean(params, "fromSurface")? == Some(false) {
        return Err(Failure::from(format!(
            "{METHOD}: fromSurface false is not supported; diting captures the page and has no window"
        )));
    }
    let mut out = Out::default();
    let metrics = forward(session, "Page.getLayoutMetrics", &json!({}), &mut out).await?;
    let ratio = forward(
        session,
        "Runtime.evaluate",
        &json!({ "expression": "devicePixelRatio", "returnByValue": true }),
        &mut out,
    )
    .await?;
    let dpr = ratio["result"]["value"]
        .as_f64()
        .filter(|dpr| dpr.is_finite() && *dpr > 0.0)
        .unwrap_or(1.0);
    let (take, scale) = plan(&geometry_of(&metrics, dpr), clip, beyond)?;
    let engine_session = {
        let hub = HUB.lock().await;
        page_of(&hub, session)?.engine_session.clone()
    };
    let data = match &take {
        Take::Plain(size) => white_base64(size)?,
        Take::Viewport => raster(&engine_session, false, scale).await?,
        Take::Document => raster(&engine_session, true, scale).await?,
        Take::Cut(cut, true) => cut_base64(&raster(&engine_session, true, scale).await?, cut)?,
        Take::Cut(cut, false) => cut_base64(&raster(&engine_session, false, scale).await?, cut)?,
    };
    out.frames
        .push(ok(id, Some(session), json!({ "data": data })));
    Ok(out)
}

/// A `clip` as Chrome reads one: five numbers, none left out.
fn clip_of(value: &Value) -> Result<Clip, Failure> {
    let clip = Clip {
        x: number(value, "x")?,
        y: number(value, "y")?,
        width: number(value, "width")?,
        height: number(value, "height")?,
        scale: number(value, "scale")?,
    };
    if clip.width < 0.0 || clip.height < 0.0 || clip.scale <= 0.0 {
        return Err(format!("{METHOD}: clip needs a positive scale and sizes from 0 up").into());
    }
    Ok(clip)
}

fn geometry_of(metrics: &Value, dpr: f64) -> Geometry {
    let f = |a: &str, b: &str| metrics[a][b].as_f64().unwrap_or(0.0);
    Geometry {
        scroll_x: f("layoutViewport", "pageX"),
        scroll_y: f("layoutViewport", "pageY"),
        view_width: f("visualViewport", "clientWidth"),
        view_height: f("visualViewport", "clientHeight"),
        content_width: f("contentSize", "width"),
        content_height: f("contentSize", "height"),
        dpr,
    }
}

/// What to capture for a `clip` (`None` is the visible viewport) and the
/// device pixels per CSS pixel to rasterize at. A rect is in document
/// pixels; from the document when it runs past the viewport and `beyond`
/// says so, otherwise from the viewport band wherever it reaches.
fn plan(geometry: &Geometry, clip: Option<Clip>, beyond: bool) -> Result<(Take, f64), String> {
    let Some(clip) = clip else {
        return Ok((
            if beyond {
                Take::Document
            } else {
                Take::Viewport
            },
            geometry.dpr,
        ));
    };
    let scale = clip.scale * geometry.dpr;
    if scale < 1.0 - f64::EPSILON || scale > MAX_SCALE + f64::EPSILON {
        return Err(format!(
            "{METHOD}: {scale} pixels per CSS pixel is not supported; diting captures between 1 and 3"
        ));
    }
    let size = cut(0.0, 0.0, clip.width, clip.height, scale);
    // Chrome's words for its own bound (a 100000×100000 clip, measured):
    // the padding would be an image no raster here is allocated for.
    if u64::from(size.width) * u64::from(size.height) > MAX_PIXELS {
        return Err("Unable to capture screenshot".to_string());
    }
    // Blink drops the whole override for a rect that starts left of the
    // document: no offset and no scale, so the page's own dpr and the
    // viewport's corner, cut to the size the clip asked for.
    if clip.x < 0.0 {
        return Ok((Take::Cut(Cut { x: 0, y: 0, ..size }, false), geometry.dpr));
    }
    let g = geometry;
    let Clip {
        x,
        y,
        width,
        height,
        scale: _,
    } = clip;
    if near(x, g.scroll_x)
        && near(y, g.scroll_y)
        && near(width, g.view_width)
        && near(height, g.view_height)
    {
        return Ok((Take::Viewport, scale));
    }
    let (right, bottom) = (x + width, y + height);
    let on_band = x < g.scroll_x + g.view_width
        && right > g.scroll_x
        && y < g.scroll_y + g.view_height
        && bottom > g.scroll_y;
    let inside = x >= g.scroll_x
        && y >= g.scroll_y
        && right <= g.scroll_x + g.view_width + EDGE
        && bottom <= g.scroll_y + g.view_height + EDGE;
    if !beyond || inside {
        // The band is the live page, and covers no more than the viewport.
        return Ok(if on_band {
            (
                Take::Cut(
                    cut(x - g.scroll_x, y - g.scroll_y, width, height, scale),
                    false,
                ),
                scale,
            )
        } else {
            (Take::Plain(size), scale)
        });
    }
    // A rect that is the document needs no cutting — and no second
    // encode of a tall image.
    if x <= EDGE
        && y <= EDGE
        && (right - g.content_width).abs() <= EDGE
        && (bottom - g.content_height).abs() <= EDGE
    {
        return Ok((Take::Document, scale));
    }
    Ok((Take::Cut(cut(x, y, width, height, scale), true), scale))
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= EDGE
}

/// A rect `width` × `height` CSS pixels whose corner is `x`, `y` CSS pixels
/// from the capture's, rasterized at `scale`: the corner and each edge land
/// on the nearest device pixel — half a pixel away from zero, as Chrome
/// rounds — and an edge is one device pixel at least.
fn cut(x: f64, y: f64, width: f64, height: f64, scale: f64) -> Cut {
    let px = |css: f64| (css * scale).round() as i32;
    Cut {
        x: px(x),
        y: px(y),
        width: (px(width) as u32).max(1),
        height: (px(height) as u32).max(1),
    }
}

/// The session's own capture: the viewport band, or the whole document,
/// rasterized at `scale` — as the engine encoded it.
async fn raster(engine_session: &str, whole_document: bool, scale: f64) -> Result<String, String> {
    let shot = send_screenshot(
        engine_session,
        None,
        None,
        whole_document,
        None,
        false,
        Some(scale as f32),
    )
    .await
    .map_err(session_msg)?;
    let shot: Value = serde_json::from_str(&shot).map_err(|err| err.to_string())?;
    shot["image_base64"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("{METHOD}: the capture came back without an image"))
}

fn encoded(image: DecodedImage) -> Result<String, String> {
    let png = encode_png_rgba(image.width, image.height, &image.rgba)
        .map_err(|err| format!("{METHOD}: {err}"))?;
    Ok(STANDARD.encode(png))
}

/// A white image of `cut`'s size.
fn white_base64(cut: &Cut) -> Result<String, String> {
    let rgba = vec![255; cut.width as usize * cut.height as usize * 4];
    encoded(DecodedImage::new(cut.width, cut.height, rgba))
}

/// A capture with `cut` taken out of it, re-encoded.
fn cut_base64(data: &str, cut: &Cut) -> Result<String, String> {
    let png = STANDARD
        .decode(data)
        .map_err(|err| format!("{METHOD}: the capture is not valid base64: {err}"))?;
    let image = decode_png(&png).ok_or_else(|| format!("{METHOD}: the capture is not a png"))?;
    encoded(cut_out(&image, cut))
}

/// The part of `image` under `cut`, on a white field of `cut`'s size. What
/// lies past the edge of the capture — of the viewport, or of the document —
/// or before its corner, is white, as in Chrome.
fn cut_out(image: &DecodedImage, cut: &Cut) -> DecodedImage {
    // The columns and rows that start before the capture's corner.
    let (lead_x, lead_y) = (cut.x.min(0).unsigned_abs(), cut.y.min(0).unsigned_abs());
    let (x, y) = (cut.x.max(0) as usize, cut.y.max(0) as usize);
    let width = cut
        .width
        .saturating_sub(lead_x)
        .min(image.width.saturating_sub(x as u32)) as usize;
    let height = cut
        .height
        .saturating_sub(lead_y)
        .min(image.height.saturating_sub(y as u32)) as usize;
    let mut rgba = vec![255; cut.width as usize * cut.height as usize * 4];
    for row in 0..height {
        let from = ((y + row) * image.width as usize + x) * 4;
        let to = ((lead_y as usize + row) * cut.width as usize + lead_x as usize) * 4;
        rgba[to..to + width * 4].copy_from_slice(&image.rgba[from..from + width * 4]);
    }
    DecodedImage::new(cut.width, cut.height, rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 1280×720 window 400px down a 3000px document, on a 2x display.
    fn geometry() -> Geometry {
        Geometry {
            scroll_x: 0.0,
            scroll_y: 400.0,
            view_width: 1280.0,
            view_height: 720.0,
            content_width: 1280.0,
            content_height: 3000.0,
            dpr: 2.0,
        }
    }

    fn clip(x: f64, y: f64, width: f64, height: f64, scale: f64) -> Clip {
        Clip {
            x,
            y,
            width,
            height,
            scale,
        }
    }

    fn cut(x: i32, y: i32, width: u32, height: u32) -> Cut {
        Cut {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn no_clip_is_the_viewport_or_the_document() {
        assert_eq!(plan(&geometry(), None, false), Ok((Take::Viewport, 2.0)));
        assert_eq!(plan(&geometry(), None, true), Ok((Take::Document, 2.0)));
    }

    #[test]
    fn playwrights_viewport_clip_is_the_viewport() {
        let taken = plan(
            &geometry(),
            Some(clip(0.0, 400.0, 1280.0, 720.0, 1.0)),
            true,
        );
        assert_eq!(taken, Ok((Take::Viewport, 2.0)));
    }

    #[test]
    fn a_rect_on_screen_is_cut_from_the_band() {
        let rect = clip(10.5, 450.0, 100.0, 50.0, 1.0);
        // Against the viewport's corner — with the flag too: the band is
        // the live page, where the document capture is a re-parse.
        let taken = plan(&geometry(), Some(rect), false);
        assert_eq!(taken, Ok((Take::Cut(cut(21, 100, 200, 100), false), 2.0)));
        let taken = plan(&geometry(), Some(rect), true);
        assert_eq!(taken, Ok((Take::Cut(cut(21, 100, 200, 100), false), 2.0)));
    }

    #[test]
    fn a_rect_past_the_viewport_edge() {
        // 400..1120 is the viewport; the rect runs to 1200.
        let rect = clip(10.0, 1000.0, 100.0, 200.0, 1.0);
        let taken = plan(&geometry(), Some(rect), false);
        // From the band: in-viewport rows real, the rest white.
        assert_eq!(taken, Ok((Take::Cut(cut(20, 1200, 200, 400), false), 2.0)));
        let taken = plan(&geometry(), Some(rect), true);
        assert_eq!(taken, Ok((Take::Cut(cut(20, 2000, 200, 400), true), 2.0)));
    }

    #[test]
    fn a_rect_off_the_viewport() {
        // Below it, and above it.
        for (y, from_document) in [
            (2800.0, cut(0, 5600, 2560, 200)),
            (0.0, cut(0, 0, 200, 100)),
        ] {
            let (width, height) = if y == 0.0 {
                (100.0, 50.0)
            } else {
                (1280.0, 100.0)
            };
            let rect = clip(0.0, y, width, height, 1.0);
            let taken = plan(&geometry(), Some(rect), false);
            assert_eq!(
                taken,
                Ok((
                    Take::Plain(cut(0, 0, width as u32 * 2, height as u32 * 2)),
                    2.0
                )),
                "{y} without the document"
            );
            let taken = plan(&geometry(), Some(rect), true);
            assert_eq!(
                taken,
                Ok((Take::Cut(from_document, true), 2.0)),
                "{y} with it"
            );
        }
    }

    #[test]
    fn a_document_sized_rect_is_the_document() {
        let taken = plan(&geometry(), Some(clip(0.0, 0.0, 1280.0, 3000.0, 1.0)), true);
        assert_eq!(taken, Ok((Take::Document, 2.0)));
        // Without the document it is a band cut, white below the viewport.
        let taken = plan(
            &geometry(),
            Some(clip(0.0, 0.0, 1280.0, 3000.0, 1.0)),
            false,
        );
        assert_eq!(taken, Ok((Take::Cut(cut(0, -800, 2560, 6000), false), 2.0)));
    }

    #[test]
    fn a_rect_past_the_document_keeps_its_size() {
        // 50px under the document's end, 120px to its right.
        let taken = plan(
            &geometry(),
            Some(clip(0.0, 2950.0, 1280.0, 100.0, 1.0)),
            true,
        );
        assert_eq!(taken, Ok((Take::Cut(cut(0, 5900, 2560, 200), true), 2.0)));
        let taken = plan(&geometry(), Some(clip(0.0, 0.0, 1400.0, 100.0, 1.0)), true);
        assert_eq!(taken, Ok((Take::Cut(cut(0, 0, 2800, 200), true), 2.0)));
    }

    #[test]
    fn a_rect_starting_left_of_the_document_is_the_viewport_corner() {
        // Blink drops the offset and the scale together: unscaled, from
        // the corner, the document not asked — the y is dropped too.
        for beyond in [false, true] {
            let rect = clip(-50.0, 50.0, 100.0, 100.0, 1.0);
            assert_eq!(
                plan(&geometry(), Some(rect), beyond),
                Ok((Take::Cut(cut(0, 0, 200, 200), false), 2.0)),
                "beyond {beyond}"
            );
        }
        // A hair left of the document is left of it.
        let rect = clip(-0.5, 0.0, 100.0, 100.0, 1.0);
        assert_eq!(
            plan(&geometry(), Some(rect), false),
            Ok((Take::Cut(cut(0, 0, 200, 200), false), 2.0))
        );
        // At 1x a 2× clip is still the corner unscaled: 100 × 2 CSS
        // pixels of it.
        let mut at_1x = geometry();
        at_1x.dpr = 1.0;
        assert_eq!(
            plan(&at_1x, Some(clip(-0.5, 400.0, 100.0, 100.0, 2.0)), false),
            Ok((Take::Cut(cut(0, 0, 200, 200), false), 1.0))
        );
    }

    #[test]
    fn a_clip_lands_on_the_pixels_chrome_lands_on() {
        // Chrome 158, measured: each edge rounds on its own — half a pixel
        // away from zero — and is never under one pixel. (The test module's
        // own `cut` builds a rect; this is the module's rule.)
        let corner = |x, y, width, height, scale| super::cut(x, y, width, height, scale);
        for ((width, height, scale), taken) in [
            ((10.25, 10.25, 1.0), cut(0, 0, 10, 10)),
            ((10.3, 10.3, 1.0), cut(0, 0, 10, 10)),
            ((0.49, 10.0, 1.0), cut(0, 0, 1, 10)),
            ((10.0, 10.0, 1.5), cut(0, 0, 15, 15)),
            ((10.25, 10.25, 2.0), cut(0, 0, 21, 21)),
            ((0.49, 10.0, 2.0), cut(0, 0, 1, 20)),
            ((100_000.4, 10.0, 2.0), cut(0, 0, 200_001, 20)),
            ((10.0, 10.0, 3.0), cut(0, 0, 30, 30)),
        ] {
            assert_eq!(
                corner(0.0, 0.0, width, height, scale),
                taken,
                "{width} at {scale}"
            );
        }
        // The corner lands the same way.
        assert_eq!(corner(0.25, 2.25, 4.0, 2.0, 2.0), cut(1, 5, 8, 4));
        assert_eq!(corner(0.6, -0.5, 10.0, 10.0, 1.0), cut(1, -1, 10, 10));
    }

    #[test]
    fn a_rect_partly_above_the_viewport_is_white_there() {
        // 350..450 against a viewport that starts at 400: the top half has
        // nothing under it.
        let taken = plan(
            &geometry(),
            Some(clip(0.0, 350.0, 100.0, 100.0, 1.0)),
            false,
        );
        assert_eq!(taken, Ok((Take::Cut(cut(0, -100, 200, 200), false), 2.0)));
    }

    #[test]
    fn css_scale_is_one_pixel_per_css_pixel() {
        // Playwright's `scale: "css"`: clip.scale is 1 / devicePixelRatio.
        let taken = plan(
            &geometry(),
            Some(clip(0.0, 400.0, 1280.0, 720.0, 0.5)),
            false,
        );
        assert_eq!(taken, Ok((Take::Viewport, 1.0)));
        let taken = plan(
            &geometry(),
            Some(clip(10.0, 450.0, 100.0, 50.0, 0.5)),
            false,
        );
        assert_eq!(taken, Ok((Take::Cut(cut(10, 50, 100, 50), false), 1.0)));
    }

    #[test]
    fn what_has_no_raster_is_refused() {
        for (dpr, scale) in [(1.0, 0.5), (2.0, 2.0)] {
            let mut geometry = geometry();
            geometry.dpr = dpr;
            let refused = plan(&geometry, Some(clip(0.0, 450.0, 100.0, 50.0, scale)), false);
            assert!(
                refused.unwrap_err().contains("not supported"),
                "{dpr} {scale}"
            );
        }
        // A clip a hair wide is a pixel wide — what Chrome answers too.
        let taken = plan(&geometry(), Some(clip(0.0, 450.0, 1e-9, 50.0, 1.0)), false);
        assert_eq!(taken, Ok((Take::Cut(cut(0, 100, 1, 100), false), 2.0)));
        // A rect nothing bounds — the padding is an image of its size.
        let refused = plan(
            &geometry(),
            Some(clip(0.0, 450.0, 100_000.0, 100_000.0, 1.0)),
            false,
        );
        assert_eq!(refused.unwrap_err(), "Unable to capture screenshot");
        // Chrome's words for a field left out.
        for clip in [
            serde_json::json!({ "x": 0, "y": 0, "width": 10 }),
            serde_json::json!({ "x": 0, "y": 0, "width": 10, "height": 10 }),
            serde_json::json!({ "x": 0, "y": 0, "width": 10, "height": 10, "scale": "1" }),
        ] {
            assert_eq!(
                clip_of(&clip),
                Err(Failure::Invalid("Invalid parameters".to_string()))
            );
        }
        let refused =
            clip_of(&serde_json::json!({ "x": 0, "y": 0, "width": 10, "height": 10, "scale": 0 }));
        assert_eq!(
            refused,
            Err(Failure::Refused(
                "Page.captureScreenshot: clip needs a positive scale and sizes from 0 up"
                    .to_string()
            ))
        );
    }

    /// Four columns × three rows; pixel (x, y) is `x + 10 * y` in red, the
    /// rest zero.
    fn image() -> DecodedImage {
        let mut rgba = Vec::new();
        for y in 0..3 {
            for x in 0..4u8 {
                rgba.extend_from_slice(&[x + 10 * y, 0, 0, 0]);
            }
        }
        DecodedImage::new(4, 3, rgba)
    }

    fn pixel(image: &DecodedImage, x: u32, y: u32) -> [u8; 4] {
        let at = ((y * image.width + x) * 4) as usize;
        image.rgba[at..at + 4].try_into().unwrap()
    }

    #[test]
    fn cut_out_takes_the_rect_and_only_the_rect() {
        let taken = cut_out(&image(), &cut(1, 1, 2, 2));
        assert_eq!((taken.width, taken.height), (2, 2));
        assert_eq!(pixel(&taken, 0, 0), [11, 0, 0, 0]);
        assert_eq!(pixel(&taken, 1, 0), [12, 0, 0, 0]);
        assert_eq!(pixel(&taken, 0, 1), [21, 0, 0, 0]);
        assert_eq!(pixel(&taken, 1, 1), [22, 0, 0, 0]);
    }

    #[test]
    fn cut_out_fills_what_lies_past_the_capture_with_white() {
        const WHITE: [u8; 4] = [255; 4];
        // Past the right and bottom edges.
        let taken = cut_out(&image(), &cut(3, 2, 5, 5));
        assert_eq!((taken.width, taken.height), (5, 5));
        assert_eq!(pixel(&taken, 0, 0), [23, 0, 0, 0]);
        assert_eq!(pixel(&taken, 1, 0), WHITE);
        assert_eq!(pixel(&taken, 0, 1), WHITE);
        assert_eq!(pixel(&taken, 4, 4), WHITE);
        // Before the capture's corner: the top row of a rect that starts
        // one row above it.
        let taken = cut_out(&image(), &cut(0, -1, 2, 2));
        assert_eq!((taken.width, taken.height), (2, 2));
        assert_eq!(pixel(&taken, 0, 0), WHITE);
        assert_eq!(pixel(&taken, 1, 0), WHITE);
        assert_eq!(pixel(&taken, 0, 1), [0, 0, 0, 0]);
        assert_eq!(pixel(&taken, 1, 1), [1, 0, 0, 0]);
        // Under none of it.
        for rect in [cut(4, 0, 2, 2), cut(0, 3, 2, 2), cut(9, 9, 2, 2)] {
            let taken = cut_out(&image(), &rect);
            assert_eq!((taken.width, taken.height), (2, 2));
            assert!(taken.rgba.chunks(4).all(|pixel| pixel == WHITE), "{rect:?}");
        }
    }
}
