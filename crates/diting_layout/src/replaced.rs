//! Replaced boxes: images, video, canvas, form controls, svg, iframe.
//!
//! Split from the layout hub. The public entry points stay re-exported
//! from the crate root. Pure move.

use super::*;

/// Tags whose layout box is a replaced leaf: intrinsic size + aspect ratio,
/// no children in the layout tree. `input`/`textarea` join the img/canvas
/// family: an unstyled control used to fall through to the plain-block path
/// and measure 0×N (no content to lay out), which reads as "invisible" to
/// every rect-based consumer — Playwright's actionability loop waits forever
/// on a zero-height input (obscura#807 class). `svg` joins for the same
/// reason plus a stronger one: its children are shapes/text in viewBox
/// user units, not flow content — flowing them is the text-leak bug the
/// AginxOS svg batch starts from. The subtree compiles to paint ops at
/// collect time instead (see [`svg::compile_svg`]).
pub(super) fn is_replaced_tag(tag: &str) -> bool {
    matches!(
        tag,
        "img"
            | "video"
            | "iframe"
            | "canvas"
            | "object"
            | "embed"
            | "input"
            | "textarea"
            | "select"
            | "svg"
    )
}

/// Canonicalize an `<img>`-shaped URL against the document base. The byte
/// tables both fetch paths key (`ImageCache::network_bytes`, the screenshot
/// prefetch, the band-pump's `image_bytes`) hold ABSOLUTE URLs, so a
/// relative `src="dot.png"` that passes through raw never matches its own
/// fetched body — the img paints a placeholder while the bytes sit in the
/// table. Resolution follows the fetchers' own join (`base.join(raw)`), so
/// both sides produce the same canonical `Url` string. data:/blob: are
/// self-contained and pass through; an already-absolute src canonicalizes
/// through `Url::parse` (same normalizer `join` applies); without a base —
/// or against an unparseable base — the raw string survives unchanged,
/// preserving the pre-base pass-through (miss → placeholder).
pub(super) fn absolutize_img_src(raw: &str, base_url: Option<&str>) -> String {
    let raw = raw.trim();
    if raw.is_empty() || raw.starts_with("data:") || raw.starts_with("blob:") {
        return raw.to_string();
    }
    if let Ok(u) = url::Url::parse(raw) {
        return u.to_string();
    }
    match base_url
        .and_then(|b| url::Url::parse(b).ok())
        .and_then(|b| b.join(raw).ok())
    {
        Some(joined) => joined.to_string(),
        None => raw.to_string(),
    }
}

/// The URL an `<img>` should load: its `<picture>` parent's first matching
/// `<source>` candidate when present (media query evaluated at the layout
/// viewport), else the img's own srcset selection, else plain `src`
/// (HTML §4.8.4.3.9, product-simplified). `base_url` is the owner
/// document's URL — relative sources absolutize against it (see
/// [`absolutize_img_src`]) so the returned string is the key the fetched
/// byte tables use. None → the placeholder path.
pub fn resolve_img_source(
    tree: &DomTree,
    img: NodeId,
    viewport_width: f32,
    base_url: Option<&str>,
) -> Option<String> {
    let attr = |id: NodeId, name: &str| {
        tree.with_node(id, |n| n.get_attribute(name).map(|v| v.to_string()))
            .flatten()
    };

    // A <picture> parent contributes candidates only from <source> children
    // that precede the img in document order; the img itself terminates the
    // scan.
    if let Some(parent) = tree.with_node(img, |n| n.parent).flatten() {
        let parent_is_picture = tree
            .with_node(parent, |n| {
                n.as_element().map(|e| e.local.to_string() == "picture")
            })
            .flatten()
            .unwrap_or(false);
        if parent_is_picture {
            for child in tree.children(parent) {
                let tag = tree
                    .with_node(child, |n| n.as_element().map(|e| e.local.to_string()))
                    .flatten()
                    .unwrap_or_default();
                if child == img {
                    break;
                }
                if tag != "source" {
                    continue;
                }
                // media="(min-width: …)" / absent media both evaluate here;
                // a non-matching source is skipped to the next sibling.
                if !media_matches_width(attr(child, "media").as_deref(), viewport_width) {
                    continue;
                }
                if let Some(srcset) = attr(child, "srcset") {
                    let cands = image::parse_srcset(&srcset);
                    if let Some(c) = image::select_srcset_candidate(&cands, viewport_width) {
                        return Some(absolutize_img_src(&c.url, base_url));
                    }
                }
                // type="image/webp" etc.: without the candidate set matching
                // we fall through — a source with no usable srcset never wins.
            }
        }
    }

    if let Some(srcset) = attr(img, "srcset") {
        let cands = image::parse_srcset(&srcset);
        if let Some(c) = image::select_srcset_candidate(&cands, viewport_width) {
            return Some(absolutize_img_src(&c.url, base_url));
        }
    }

    attr(img, "src").map(|s| absolutize_img_src(&s, base_url))
}

/// Minimal media-query width gate for `<source media>`: supports the two
/// real-world forms `(min-width: Npx)` / `(max-width: Npx)` joined by
/// `and`, plus absent/`all`. Anything else declines (conservative: falls
/// through to the next source or the fallback img).
pub fn media_matches_width(media: Option<&str>, vw: f32) -> bool {
    let Some(media) = media.map(str::trim).filter(|m| !m.is_empty()) else {
        return true;
    };
    if media.eq_ignore_ascii_case("all") {
        return true;
    }
    // NOT-prefixed or print media queries: decline rather than guess.
    if media.contains("not ") || media.contains("print") {
        return false;
    }
    let mut ok = true;
    for clause in media.split(" and ") {
        let clause = clause.trim().trim_start_matches('(').trim_end_matches(')');
        if let Some(w) = clause
            .strip_prefix("min-width:")
            .and_then(|v| v.trim().strip_suffix("px"))
            .and_then(|v| v.trim().parse::<f32>().ok())
        {
            ok &= vw >= w;
        } else if let Some(w) = clause
            .strip_prefix("max-width:")
            .and_then(|v| v.trim().strip_suffix("px"))
            .and_then(|v| v.trim().parse::<f32>().ok())
        {
            ok &= vw <= w;
        }
        // Unknown feature inside a conjunction: leave `ok` untouched (the
        // common case is a single min/max-width clause anyway).
    }
    ok
}

/// The rect replaced content paints into, per object-fit/object-position
/// (batch 5c) — the port of blitz-paint's sizing.rs + render.rs draw_image
/// offset math. `box_rect` is the content box, (nw, nh) the natural size:
///
/// - Fill: the whole box (the pre-5c behavior).
/// - Contain / Cover: scale by min/max of the two axis ratios; blitz's
///   four-arm `(x<1, y<1)` match reduces to exactly that.
/// - None: natural size. ScaleDown: Contain unless natural is already
///   smaller than the contain result on width (then natural).
/// - Offset: each object-position part resolves against the free space
///   `box − paint` — percentages scale it, px lengths use it directly;
///   the initial 50%/50% centers. Cover offsets go negative (overflow).
pub fn object_paint_rect(
    box_rect: Rect,
    nw: f32,
    nh: f32,
    fit: ObjectFit,
    pos: (ObjectPositionPart, ObjectPositionPart),
) -> Rect {
    let (bw, bh) = (box_rect.width, box_rect.height);
    let paint = |w: f32, h: f32| Rect {
        x: 0.0,
        y: 0.0,
        width: w,
        height: h,
    };
    let size = if nw <= 0.0 || nh <= 0.0 || bw <= 0.0 || bh <= 0.0 {
        match fit {
            ObjectFit::None => paint(nw, nh),
            _ => paint(bw, bh),
        }
    } else {
        let (xr, yr) = (bw / nw, bh / nh);
        match fit {
            ObjectFit::Fill => paint(bw, bh),
            ObjectFit::Contain => paint(nw * xr.min(yr), nh * xr.min(yr)),
            ObjectFit::Cover => paint(nw * xr.max(yr), nh * xr.max(yr)),
            ObjectFit::None => paint(nw, nh),
            ObjectFit::ScaleDown => {
                let (cw, ch) = (nw * xr.min(yr), nh * xr.min(yr));
                if nw < cw {
                    paint(nw, nh)
                } else {
                    paint(cw, ch)
                }
            }
        }
    };
    let resolve = |part: ObjectPositionPart, free: f32| match part {
        ObjectPositionPart::Percent(p) => free * p / 100.0,
        ObjectPositionPart::Px(x) => x,
    };
    Rect {
        x: box_rect.x + resolve(pos.0, bw - size.width),
        y: box_rect.y + resolve(pos.1, bh - size.height),
        width: size.width,
        height: size.height,
    }
}

/// CSS 2.1 §10.5: a percentage height resolves only when the containing
/// block's height is specified EXPLICITLY; against a content-sized
/// (indefinite) ancestor the percentage behaves as `auto`. Walk the DOM
/// ancestors of `id`: Px (or a viewport unit already folded to px at
/// cascade) is definite; Percent/Calc defers the question one level up;
/// auto/keywords/nothing stops the walk as indefinite. The document root
/// counts as definite — its percentage resolves against the viewport.
pub(super) fn cb_height_definite(
    tree: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    id: NodeId,
) -> bool {
    let mut cur = tree.with_node(id, |n| n.parent).flatten();
    while let Some(nid) = cur {
        if tree.with_node(nid, |n| n.parent).flatten().is_none() {
            return true;
        }
        match styles.get(&nid).and_then(|s| s.height) {
            Some(diting_css::Length::Px(_)) => return true,
            Some(diting_css::Length::Percent(_)) | Some(diting_css::Length::Calc { .. }) => {
                cur = tree.with_node(nid, |n| n.parent).flatten();
            }
            _ => return false,
        }
    }
    false
}

/// Empty every cached layout in `root`'s subtree (taffy's `mark_dirty`
/// propagates only to ancestors). The two post-layout repair passes use it
/// when a fixup touched a float: a repaired float's new collision extent is
/// invisible to its siblings' cache keys — (style, constraints) — so a
/// cached text leaf would keep its float_lines slots wrapped against the
/// float's old extent and paint across the repaired box.
pub(super) fn bust_subtree_cache(node: diting_taffy::tree::NodeId, tree: &mut TaffyTree<TextLeaf>) {
    let _ = tree.mark_dirty(node);
    for c in tree.children(node).unwrap_or_default().to_vec() {
        bust_subtree_cache(c, tree);
    }
}

/// Depth of a taffy node below the ICB — the post-layout repair passes sort
/// their fixups shallow-first so an inner fixup feeds an outer one's basis.
pub(super) fn taffy_depth(
    taffy_tree: &TaffyTree<TextLeaf>,
    node: diting_taffy::tree::NodeId,
    icb: diting_taffy::tree::NodeId,
) -> usize {
    let (mut d, mut cur) = (0usize, node);
    while cur != icb {
        match taffy_tree.parent(cur) {
            Some(p) => {
                cur = p;
                d += 1;
            }
            None => break,
        }
    }
    d
}

/// Whether `height: <percent>` on element `id` resolves, or folds to `auto`
/// per CSS 2.1 §10.5: it resolves when the containing-block chain is definite
/// (see [`cb_height_definite`]) OR the element is absolutely positioned (the
/// §10.5 exception — its containing block is the nearest positioned
/// ancestor's padding box, definite by construction once that ancestor has
/// laid out).
pub(super) fn pct_height_resolves(
    tree: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    id: NodeId,
) -> bool {
    let abspos = matches!(
        styles.get(&id).and_then(|s| s.position),
        Some(PositionMode::Absolute) | Some(PositionMode::Fixed)
    );
    abspos || cb_height_definite(tree, styles, id)
}

/// Build the taffy leaf for a replaced element. Per-tag natural-size
/// semantics (batch 7a), mirroring blitz-dom layout/mod.rs:
///
/// - `img`: a decoded image gives the natural size/ratio; attributes are
///   presentational hints overriding per-axis with the image ratio
///   back-filling; nothing at all → the CSS default replaced box 300×150.
/// - `canvas`: its width/height attributes ARE the intrinsic size
///   (defaulting 300×150) and it carries an aspect RATIO — a missing axis
///   transfers through it.
/// - `video`/`iframe`/`embed`: attribute-or-300×150 per axis, NO ratio —
///   `<video width=600>` lays out 600×150, height does not transfer.
pub(super) fn build_replaced_leaf(
    tree: &DomTree,
    id: NodeId,
    styles: &HashMap<NodeId, ComputedStyle>,
    images: &HashMap<NodeId, DecodedImage>,
    taffy_tree: &mut TaffyTree<TextLeaf>,
    node_map: &mut HashMap<diting_taffy::tree::NodeId, NodeId>,
    strut_descent: f32,
) -> Option<diting_taffy::tree::NodeId> {
    // #196: replaced elements skip build_element_inner's display gate (they
    // attach from three run-attach sites that call here directly), so a
    // display:none textarea — baidu/sina ship page templates as hidden
    // textareas — used to lay out as a real control and paint its value
    // (the raw `<style …>` template text) as visible content. One gate
    // here closes all three attach sites at once.
    if styles.get(&id).and_then(|s| s.display) == Some(CssDisplay::None) {
        return None;
    }
    let style = styles.get(&id).cloned().unwrap_or_default();
    let tag = tree
        .with_node(id, |n| n.as_element().map(|e| e.local.to_string()))
        .flatten()
        .unwrap_or_default();
    let attr = |name: &str| {
        tree.with_node(id, |n| n.get_attribute(name).map(|v| v.to_string()))
            .flatten()
            .and_then(|v| v.parse::<f32>().ok())
    };
    let (aw, ah) = (attr("width"), attr("height"));
    // Natural size AND whether a missing axis derives from the ratio.
    let (nat_w, nat_h, ratio_transfer) = match tag.as_str() {
        // Form controls (obscura#807 class): Chrome-shaped default boxes.
        // Text-like inputs ≈ the size=20 default column; checkbox/radio are
        // square 13px widgets; button-like inputs approximate label width
        // from the value attribute (input has no laid-out text of its own);
        // textarea is the cols=20/rows=2 default box. All three axes are
        // overridable per-axis by CSS/attribute width & height below.
        "input" => {
            let ty = tree
                .with_node(id, |n| {
                    n.get_attribute("type").map(|v| v.to_ascii_lowercase())
                })
                .flatten()
                .unwrap_or_default();
            match ty.as_str() {
                "checkbox" | "radio" => (13.0, 13.0, false),
                // Chrome's default slider box (blitz#456).
                "range" => (129.0, 16.0, false),
                "button" | "submit" | "reset" => {
                    // Same precedence as the paint run: dirty value, then the
                    // parsed value attribute, then Chrome's default "Submit".
                    let label = tree
                        .with_node(id, |n| {
                            n.live_value()
                                .map(|v| v.to_string())
                                .or_else(|| n.get_attribute("value").map(|v| v.to_string()))
                        })
                        .flatten()
                        .unwrap_or_default();
                    let label: String = if label.is_empty() && ty == "submit" {
                        "Submit".to_string()
                    } else {
                        label
                    };
                    (label.chars().count() as f32 * 8.8 + 20.0, 22.0, false)
                }
                _ => (177.0, 22.0, false),
            }
        }
        "textarea" => (177.0, 38.0, false),
        // An atomic replaced box like the others: options never lay out as
        // page text (a closed select paints one label). Width tracks the
        // label it will paint — the same precedence form_control_run
        // resolves — with control padding and dropdown-arrow room added,
        // matching how the button arm sizes from its label.
        "select" => {
            let label = tree
                .with_node(id, |n| n.live_value().map(|v| v.to_string()))
                .flatten()
                .filter(|s| !s.trim().is_empty())
                .or_else(|| selected_option_label(tree, id))
                .unwrap_or_default();
            (label.chars().count() as f32 * 8.8 + 36.0, 22.0, false)
        }
        "canvas" => (aw.unwrap_or(300.0), ah.unwrap_or(150.0), true),
        "video" | "iframe" | "embed" | "object" => {
            (aw.unwrap_or(300.0), ah.unwrap_or(150.0), false)
        }
        // svg: the viewBox IS the intrinsic size/ratio — the element box
        // maps onto it at paint (that map is the zoom semantics of this
        // batch). Attr w/h override per-axis and back-fill through the
        // viewBox ratio; no viewBox: attr-or-300×150 like the others.
        "svg" => {
            let vb = tree
                .with_node(id, |n| n.get_attribute("viewBox").map(|v| v.to_string()))
                .flatten()
                .and_then(|v| svg::parse_view_box(&v));
            match vb {
                Some((_, _, vw, vh)) => {
                    let r = vw / vh;
                    let (w, h) = match (aw, ah) {
                        (Some(w), Some(h)) if h > 0.0 => (w, h),
                        (Some(w), None) => (w, w / r),
                        (None, Some(h)) => (h * r, h),
                        _ => (vw, vh),
                    };
                    (w, h, true)
                }
                None => (aw.unwrap_or(300.0), ah.unwrap_or(150.0), false),
            }
        }
        // img: attrs override the decoded image per-axis, the image ratio
        // back-fills a free axis; with nothing at all, 300×150.
        _ => {
            let img_ratio = images.get(&id).map(|i| i.width as f32 / i.height as f32);
            let (w, h) = match (aw, ah) {
                (Some(w), Some(h)) if h > 0.0 => (w, h),
                (Some(w), None) => (w, w / img_ratio.unwrap_or(2.0)),
                (None, Some(h)) => (h * img_ratio.unwrap_or(2.0), h),
                (None, None) => images
                    .get(&id)
                    .map(|i| (i.width as f32, i.height as f32))
                    .unwrap_or((300.0, 150.0)),
                // Degenerate attrs (e.g. height="0"): the CSS default box.
                _ => (300.0, 150.0),
            };
            (w, h, true)
        }
    };
    // #120 zoom: the natural box is a declaration-class size — attrs are
    // presentational hints, decoded image dimensions and the form-control
    // default boxes are intrinsic — and Chrome renders all of them scaled
    // inside a zoom subtree. Scaling here (after the match) keeps the ratio
    // arithmetic above in unscaled coordinates, so ratio_transfer is
    // unaffected (both axes scale together).
    let (nat_w, nat_h) = (nat_w * style.zoom_accum, nat_h * style.zoom_accum);

    // Attrs block ratio transfer for img and svg: width/height attrs are
    // presentational-hint DECLARATIONS, so a CSS override of one axis does
    // not re-derive the other (#cssw stays 100×200, not 100×50). Canvas
    // attrs ARE the intrinsic size and never block.
    let (attr_dw, attr_dh) = if tag == "img" || tag == "svg" {
        (aw.is_some(), ah.is_some())
    } else {
        (false, false)
    };
    let ratio = ratio_transfer.then(|| nat_w / nat_h);
    let css_w_px = match style.width {
        Some(diting_css::Length::Px(w)) => Some(w),
        _ => None,
    };
    let css_h_px = match style.height {
        Some(diting_css::Length::Px(h)) => Some(h),
        _ => None,
    };
    // Build-time ratio transfer: with exactly one authored CSS px axis and
    // nothing declaring the other, the free axis derives through the
    // natural ratio. taffy's aspect_ratio only re-derives in block/column
    // flows — not the flex-row run wrapper inline atoms now join — while
    // Chrome transfers in every container.
    let derived_h = match (ratio, css_w_px) {
        (Some(r), Some(w)) if !attr_dh => Some(w / r),
        _ => None,
    };
    let derived_w = match (ratio, css_h_px) {
        (Some(r), Some(h)) if !attr_dw => Some(h * r),
        _ => None,
    };

    // Positioning rides into replaced leaves too (to_taffy_style does this
    // for every boxed element): without it an absolutely-positioned
    // input/img/select kept its taffy-relative style, the reparent pass
    // still moved it to the containing block, and it then stacked in flow
    // there ignoring its insets.
    let position = match style.position {
        Some(diting_css::PositionMode::Absolute) | Some(diting_css::PositionMode::Fixed) => {
            Position::Absolute
        }
        _ => Position::Relative,
    };
    let mut s = Style {
        item_is_replaced: true,
        position,
        ..Default::default()
    };
    // Inset/clamp unset values are CSS `auto` (same mapping as
    // to_taffy_style's local lpa_auto).
    let lpa_auto = |v: Option<diting_css::Length>| match v {
        Some(diting_css::Length::Px(px)) => LengthPercentageAuto::length(px),
        Some(diting_css::Length::Percent(p)) => LengthPercentageAuto::percent(p / 100.0),
        Some(diting_css::Length::Calc { percent, .. }) => {
            LengthPercentageAuto::percent(percent / 100.0)
        }
        Some(
            diting_css::Length::Auto
            | diting_css::Length::MinContent
            | diting_css::Length::MaxContent
            | diting_css::Length::FitContent,
        )
        | None => LengthPercentageAuto::auto(),
    };
    if style.top.is_some()
        || style.right.is_some()
        || style.bottom.is_some()
        || style.left.is_some()
    {
        s.inset = diting_taffy::geometry::Rect {
            top: lpa_auto(style.top),
            right: lpa_auto(style.right),
            bottom: lpa_auto(style.bottom),
            left: lpa_auto(style.left),
        };
    }
    // UA/author border lays out on replaced boxes too (batch 7a): the
    // iframe's UA `2px inset` makes a width=600 attr box come out 604.
    // taffy sizes are border-box, so the attr/CSS size gets the widths
    // added (content-box semantics of the HTML attributes).
    let bline = style.border_style.is_some();
    let bw = |which: f32| -> LengthPercentage {
        LengthPercentage::length(if bline { which } else { 0.0 })
    };
    let (bt, br, bb, bl) = (
        side_px(style.border_width.top),
        side_px(style.border_width.right),
        side_px(style.border_width.bottom),
        side_px(style.border_width.left),
    );
    s.border = diting_taffy::geometry::Rect {
        top: bw(bt),
        right: bw(br),
        bottom: bw(bb),
        left: bw(bl),
    };
    // The strut descent bakes into the leaf's DEFINITE height (line-box
    // bookkeeping; collect subtracts it back from the rect). taffy's ratio
    // transfer off that height — the cyclic-percent case, where a percent
    // width against a content-sized ancestor can't resolve and the width
    // derives from the height — must land on nat_w, not nat_w + strut×ratio
    // (the +9px hero width). Fold the strut into the denominator instead.
    // Both-definite leaves never transfer (the ratio is inert there) and the
    // width→height direction can't fire for inline atoms — their height arm
    // is always the baked definite length — so the compensated ratio only
    // ever feeds the height→width transfer. Block-level callers pass strut
    // 0 and keep the raw natural ratio.
    s.aspect_ratio = ratio_transfer.then(|| nat_w / (nat_h + strut_descent));
    // CSS width/height win per axis; missing axis derives from the ratio.
    // Percent CSS sizes pass through (the CB resolves them; the natural
    // ratio only backfills auto axes). Px sizes are content-box per the
    // attribute semantics, so border widths ride on top — except under
    // authored `box-sizing: border-box`, where the px already measures to
    // the border edge.
    let border_box = matches!(style.box_sizing, Some(diting_css::BoxSizing::BorderBox));
    // The strut descent (inline atoms only — block-level callers pass 0)
    // extends the LEAF below the element box so the line box spans the
    // strut's full extent; collect subtracts it back from the recorded
    // rect, so percent/calc arms (CB-resolved approximations) skip the pad
    // rather than grow an axis the rect shrink could not undo exactly.
    s.size = Size {
        width: match style.width {
            Some(diting_css::Length::Px(w)) => Dimension::length(if border_box {
                w
            } else {
                w + if bline { bl + br } else { 0.0 }
            }),
            Some(diting_css::Length::Percent(p)) => Dimension::percent(p / 100.0),
            // Percent-only placeholder (same convention as to_taffy_style).
            Some(diting_css::Length::Calc { percent, .. }) => Dimension::percent(percent / 100.0),
            // Sizing keywords on replaced boxes: intrinsic = natural size
            // (css-sizing-3), same fallback as auto.
            Some(
                diting_css::Length::Auto
                | diting_css::Length::MinContent
                | diting_css::Length::MaxContent
                | diting_css::Length::FitContent,
            )
            | None => {
                Dimension::length(derived_w.unwrap_or(nat_w) + if bline { bl + br } else { 0.0 })
            }
        },
        height: match style.height {
            Some(diting_css::Length::Px(h)) => Dimension::length(
                (if border_box {
                    h
                } else {
                    h + if bline { bt + bb } else { 0.0 }
                }) + strut_descent,
            ),
            Some(diting_css::Length::Percent(p)) if pct_height_resolves(tree, styles, id) => {
                Dimension::percent(p / 100.0)
            }
            Some(diting_css::Length::Calc { percent, .. })
                if pct_height_resolves(tree, styles, id) =>
            {
                Dimension::percent(percent / 100.0)
            }
            // Percent (or calc with a percent part) against a content-sized
            // ancestor behaves as `auto` (§10.5) — for a replaced box its
            // natural height. Taffy would resolve the percent against an
            // indefinite CB to 0, collapsing `height:100%` iframes inside
            // auto-height wrappers (the webtop .window-body case) to zero.
            Some(
                diting_css::Length::Auto
                | diting_css::Length::MinContent
                | diting_css::Length::MaxContent
                | diting_css::Length::FitContent
                | diting_css::Length::Percent(_)
                | diting_css::Length::Calc { .. },
            )
            | None => Dimension::length(
                derived_h.unwrap_or(nat_h) + if bline { bt + bb } else { 0.0 } + strut_descent,
            ),
        },
    };

    let node = taffy_tree
        .new_leaf_with_context(s, TextLeaf::Replaced { strut_descent })
        .ok()?;
    node_map.insert(node, id);
    Some(node)
}
