//! Declaration appliers: effects, logical motion, flex/grid, position.
//!
//! `apply_one` dispatches into these. This module names the shorthand
//! parsers, the length helpers, and the keyword types each property writes.

use crate::animation::parse_animation_shorthand;
use crate::calc;
use crate::computed::ComputedStyle;
use crate::content::{parse_content_value, parse_counter_modifiers, parse_quotes};
use crate::geom::{FontCtx, Length};
use crate::len_parse::{
    css_box_offset, css_len, expand_sides, grid_areas_css, parse_aspect_ratio, parse_grid_area,
    parse_grid_areas_template, parse_grid_auto_flow, parse_grid_template_areas, parse_grid_tracks,
    parse_num_f32, parse_px_f32, specified_auto_tracks, specified_track_list, split_sides, CssF32,
    GridAreasTemplate,
};
use crate::shadow_parse::{
    parse_backdrop_blur, parse_box_shadow, parse_mix_blend_mode, parse_text_shadow,
};
use crate::sheet::ContainerType;
use crate::style_kw::{
    parse_column_count_token, parse_column_width_token, parse_columns_shorthand, AlignContentMode,
    AlignMode, BoxSizing, ClearSide, ColCount, ColWidth, FlexDirection, FlexWrapMode, FloatSide,
    JustifyMode, ObjectFit, ObjectPositionPart, Overflow, PositionMode, TextOverflow, WhiteSpace,
};
use crate::transform::{parse_transform_origin, parse_transform_with_vp};
use crate::transition::{
    parse_easing_token, parse_rotate_prop, parse_scale_prop, parse_transition_shorthand,
    parse_translate_prop, Easing, TransitionSpec,
};

/// `apply_one` family: containers, overflow, shadows/filters, clamping, transforms.
/// `None` = not this family's property; the caller tries the next one.
pub(crate) fn apply_effects(
    style: &mut ComputedStyle,
    name: &str,
    _value: &str,
    v: &str,
    fonts: &FontCtx,
) -> Option<bool> {
    Some(match name {
        "container-type" => {
            style.container_type = match v {
                "size" => ContainerType::Size,
                "inline-size" => ContainerType::InlineSize,
                "normal" => ContainerType::Normal,
                _ => return Some(false),
            };
            true
        }
        "container-name" => {
            style.container_name = match v {
                "none" | "" => None,
                ident => Some(ident.to_string()),
            };
            true
        }
        // `font-variant` shorthand: v1 models only the caps subset the
        // engine can synthesize; other feature keywords (ligatures,
        // numeric, east-asian...) reject like other unmodeled values.
        "font-variant" => match v {
            "small-caps" => {
                style.font_variant_caps = Some(true);
                true
            }
            "normal" => {
                style.font_variant_caps = Some(false);
                true
            }
            _ => false,
        },
        "overflow" => {
            let (a, b) = match v.split_whitespace().collect::<Vec<_>>().as_slice() {
                [one] => (*one, *one),
                [x, y] => (*x, *y),
                _ => return Some(false),
            };
            let kw = |t: &str| match t {
                "visible" => Some(Overflow::Visible),
                "hidden" => Some(Overflow::Hidden),
                "clip" => Some(Overflow::Clip),
                "scroll" => Some(Overflow::Scroll),
                "auto" => Some(Overflow::Auto),
                _ => None,
            };
            let (Some(x), Some(y)) = (kw(a), kw(b)) else {
                return Some(false);
            };
            style.overflow_x = Some(x);
            style.overflow_y = Some(y);
            true
        }
        "overflow-x" | "overflow-y" => {
            let Some(o) = (match v.trim() {
                "visible" => Some(Overflow::Visible),
                "hidden" => Some(Overflow::Hidden),
                "clip" => Some(Overflow::Clip),
                "scroll" => Some(Overflow::Scroll),
                "auto" => Some(Overflow::Auto),
                _ => None,
            }) else {
                return Some(false);
            };
            if name.ends_with("-x") {
                style.overflow_x = Some(o);
            } else {
                style.overflow_y = Some(o);
            }
            true
        }
        "box-shadow" => {
            if v.eq_ignore_ascii_case("none") {
                style.box_shadow = None;
                return Some(true);
            }
            match parse_box_shadow(v, style.color, fonts) {
                Some(layers) if !layers.is_empty() => {
                    style.box_shadow = Some(layers);
                    true
                }
                _ => false,
            }
        }
        "backdrop-filter" => {
            if v.eq_ignore_ascii_case("none") {
                style.backdrop_blur = None;
                return Some(true);
            }
            match parse_backdrop_blur(v, fonts) {
                Some(px) => {
                    style.backdrop_blur = Some(px);
                    true
                }
                _ => false,
            }
        }
        // Position-face only (#316): any non-none value hijacks fixed and
        // absolute descendants to this containing block; the raw string is
        // kept as-is for the computed-style echo.
        "filter" | "perspective" => {
            let raw = if v.eq_ignore_ascii_case("none") {
                None
            } else {
                Some(v.to_string())
            };
            if name.eq_ignore_ascii_case("filter") {
                style.filter = raw;
            } else {
                style.perspective = raw;
            }
            true
        }
        "mix-blend-mode" => match parse_mix_blend_mode(v) {
            Some(mode) => {
                style.mix_blend_mode = Some(mode);
                true
            }
            None => false,
        },
        "text-shadow" => {
            if v.eq_ignore_ascii_case("none") {
                style.text_shadow = None;
                return Some(true);
            }
            match parse_text_shadow(v, style.color, fonts) {
                Some(layers) if !layers.is_empty() => {
                    style.text_shadow = Some(layers);
                    true
                }
                _ => false,
            }
        }
        "white-space" => {
            style.white_space = match v {
                "normal" => Some(WhiteSpace::Normal),
                "nowrap" => Some(WhiteSpace::Nowrap),
                "pre" => Some(WhiteSpace::Pre),
                "pre-wrap" => Some(WhiteSpace::PreWrap),
                "pre-line" => Some(WhiteSpace::PreLine),
                "break-spaces" => Some(WhiteSpace::BreakSpaces),
                _ => return Some(false),
            };
            true
        }
        "text-overflow" => {
            style.text_overflow = match v {
                "clip" => Some(TextOverflow::Clip),
                "ellipsis" => Some(TextOverflow::Ellipsis),
                _ => return Some(false),
            };
            true
        }
        // Both spellings write the same slot; the legacy prefixed name is
        // what card templates actually ship (`display:-webkit-box` +
        // `-webkit-line-clamp:2`). The standard form's trailing
        // `/ <block-ellipsis>` component parses but is ignored (the marker
        // is always U+2026 here). 0 and negatives are invalid per spec —
        // the declaration drops, not clamps to 1.
        "line-clamp" | "-webkit-line-clamp" => {
            let head = v.split('/').next().unwrap_or("").trim();
            style.line_clamp = if head.eq_ignore_ascii_case("none") {
                None
            } else {
                match head.parse::<u32>() {
                    Ok(n) if n >= 1 => Some(n),
                    _ => return Some(false),
                }
            };
            true
        }
        // #275: the legacy-flexbox orient — the second half of Chrome's
        // clamp idiom (`display:-webkit-box; -webkit-box-orient:vertical;
        // -webkit-line-clamp:N`). Only `vertical` (and its block-axis
        // synonym) arms the clamp; horizontal/inline-axis is the initial
        // value and disarms it.
        "-webkit-box-orient" => {
            style.webkit_box_vertical = match v {
                "vertical" | "block-axis" => true,
                "horizontal" | "inline-axis" => false,
                _ => return Some(false),
            };
            true
        }
        "object-fit" => {
            style.object_fit = match v {
                "fill" => Some(ObjectFit::Fill),
                "contain" => Some(ObjectFit::Contain),
                "cover" => Some(ObjectFit::Cover),
                "none" => Some(ObjectFit::None),
                "scale-down" => Some(ObjectFit::ScaleDown),
                _ => return Some(false),
            };
            true
        }
        "z-index" => {
            // `auto` (the initial value) stays None.
            style.z_index = match v {
                "auto" => None,
                _ => match v.parse::<i32>() {
                    n @ Ok(_) => n.ok(),
                    Err(_) => return Some(false),
                },
            };
            true
        }
        "transform" => {
            // Full 2D function list (obscura #740 lineage, widened twice):
            // translate/scale/rotate/skew/matrix compose into one affine.
            // One unknown function still invalidates the whole declaration
            // (spec) — parse_transform returns None and the element paints
            // where layout put it. Viewport-threaded so translateX(100vw)
            // — the deck/carousel idiom — folds against the page viewport.
            style.transform = parse_transform_with_vp(v, fonts.viewport_w, fonts.viewport_h);
            style.transform.is_some()
        }
        // CSS Transforms L2 individual properties (#140) — the modern
        // longhand spelling Motion/Framer/GSAP sites emit. Each folds into
        // effective_transform() at collect time; `none` resets to nothing.
        "translate" => {
            style.translate_prop = parse_translate_prop(v, fonts.viewport_w, fonts.viewport_h);
            style.translate_prop.is_some()
        }
        "rotate" => {
            style.rotate_prop = parse_rotate_prop(v);
            style.rotate_prop.is_some()
        }
        "scale" => {
            style.scale_prop = parse_scale_prop(v);
            style.scale_prop.is_some()
        }
        // transform-origin (#244): keywords normalize to percentages. The
        // 2D pivot resolves x/y at collect time; z is stored for the CSSOM.
        "transform-origin" => match parse_transform_origin(v, fonts) {
            Some(origin) => {
                style.transform_origin = Some(origin);
                true
            }
            None => false,
        },
        _ => return None,
    })
}

/// `apply_one` family: logical box edges, opacity/zoom, animation/transition, radius.
/// `None` = not this family's property; the caller tries the next one.
pub(crate) fn apply_logical_motion(
    style: &mut ComputedStyle,
    name: &str,
    _value: &str,
    v: &str,
    fonts: &FontCtx,
) -> Option<bool> {
    let len = |val: &str| css_len(val, fonts);
    Some(match name {
        // CSS logical box longhands (#244): horizontal-tb maps block→
        // top/bottom, inline→left/right. Our direction face doesn't mirror
        // inline flow (rtl is a read/alignment face), so inline-start=left
        // is the self-consistent mapping — the declared value lands in the
        // physical slot and layout consumes it like the physical longhand.
        "margin-block-start"
        | "margin-block-end"
        | "margin-inline-start"
        | "margin-inline-end"
        | "padding-block-start"
        | "padding-block-end"
        | "padding-inline-start"
        | "padding-inline-end" => {
            if name.starts_with("padding") && v.eq_ignore_ascii_case("auto") {
                // padding: auto is illegal — drop the declaration.
                return Some(false);
            }
            let len_v = if v.eq_ignore_ascii_case("auto") {
                Some(Length::Auto)
            } else {
                len(v)
            };
            // Direct slot write — set_side keys on the full physical name's
            // "-top"-style suffix, which a bare side word never matches.
            let sides = if name.starts_with("margin") {
                &mut style.margin
            } else {
                &mut style.padding
            };
            match name {
                "margin-block-start" | "padding-block-start" => sides.top = len_v,
                "margin-block-end" | "padding-block-end" => sides.bottom = len_v,
                "margin-inline-start" | "padding-inline-start" => sides.left = len_v,
                _ => sides.right = len_v,
            }
            true
        }
        // Logical 1-2 value shorthands (#244): one value fills start+end,
        // two are (start, end) — NOT the physical TRBL rotation. Invalid
        // tokens drop the whole declaration.
        "margin-block" | "margin-inline" | "padding-block" | "padding-inline" => {
            let parts: Vec<&str> = split_sides(v);
            if parts.is_empty() || parts.len() > 2 {
                return Some(false);
            }
            let (a, b) = (parts[0], if parts.len() == 2 { parts[1] } else { parts[0] });
            if name.starts_with("padding")
                && (a.eq_ignore_ascii_case("auto") || b.eq_ignore_ascii_case("auto"))
            {
                return Some(false);
            }
            let mk = |t: &str| {
                if t.eq_ignore_ascii_case("auto") {
                    Some(Length::Auto)
                } else {
                    len(t)
                }
            };
            let (start, end) = match (mk(a), mk(b)) {
                (Some(s), Some(e)) => (s, e),
                _ => return Some(false),
            };
            let block = name.ends_with("block");
            let margins = name.starts_with("margin");
            let sides = if margins {
                &mut style.margin
            } else {
                &mut style.padding
            };
            if block {
                sides.top = Some(start);
                sides.bottom = Some(end);
            } else {
                sides.left = Some(start);
                sides.right = Some(end);
            }
            true
        }
        "opacity" => {
            // A number in [0, 1] (animation batch A). The initial value 1
            // stores like any other — paint skips alpha work when the
            // accumulated alpha is exactly 1.
            match v.trim().parse::<f32>() {
                Ok(n) if n.is_finite() => {
                    style.opacity = Some(n.clamp(0.0, 1.0));
                    true
                }
                _ => false,
            }
        }
        "zoom" => {
            // Chrome's non-standard zoom (#120): <number> | <percentage> |
            // normal (=1). "reset" (new Chrome's unzoom) stays out of scope —
            // unparsed, like any keyword we don't model. Zero/negative zoom
            // is invalid per Chrome's parser.
            let t = v.trim();
            let z = if t.eq_ignore_ascii_case("normal") {
                Some(1.0)
            } else if let Some(pct) = t.strip_suffix('%') {
                pct.trim().css_f32().map(|n| n / 100.0)
            } else {
                t.css_f32()
            };
            match z {
                Some(n) if n.is_finite() && n > 0.0 => {
                    style.zoom = Some(n);
                    style.zoom_declared = Some(n);
                    true
                }
                _ => false,
            }
        }
        "animation" => {
            // Stored, not applied here: the sampler resolves it against
            // the stylesheet's @keyframes table at sample time.
            style.animation = parse_animation_shorthand(v);
            style.animation.is_some()
        }
        "transition" => {
            // Same storage-only contract as `animation`: the cascade
            // records the spec; trigger detection lives in the JS face
            // and interpolation in the exit sampler. Comma lists collapse
            // to the first entry.
            style.transition = parse_transition_shorthand(v);
            style.transition.is_some()
        }
        "transition-property" => {
            // First comma entry only. `none` disables transitions for the
            // element (the spec is dropped); a later duration/delay
            // longhand would re-create it with the initial value "all".
            let tok = v.split(',').next().unwrap_or("").trim();
            if tok.eq_ignore_ascii_case("none") {
                style.transition = None;
                true
            } else if !tok.is_empty()
                && tok
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
            {
                let spec = style.transition.get_or_insert(TransitionSpec {
                    property: None,
                    duration: 0.0,
                    delay: 0.0,
                    easing: Easing::CubicBezier(0.25, 0.1, 0.25, 1.0),
                });
                spec.property = Some(tok.to_ascii_lowercase());
                true
            } else {
                false
            }
        }
        "transition-duration" | "transition-delay" => {
            // First comma entry; ms folds to seconds. Negative times are
            // invalid here (the sampler never sees a phase offset).
            let t = v.split(',').next().unwrap_or("").trim();
            let secs = t
                .strip_suffix("ms")
                .and_then(|n| n.css_f32().map(|n| n / 1000.0))
                .or_else(|| t.strip_suffix('s').and_then(|n| n.css_f32()));
            match secs {
                Some(s) if s.is_finite() && s >= 0.0 => {
                    let spec = style.transition.get_or_insert(TransitionSpec {
                        property: None,
                        duration: 0.0,
                        delay: 0.0,
                        easing: Easing::CubicBezier(0.25, 0.1, 0.25, 1.0),
                    });
                    if name == "transition-duration" {
                        spec.duration = s;
                    } else {
                        spec.delay = s;
                    }
                    true
                }
                _ => false,
            }
        }
        "transition-timing-function" => {
            // Keyword or cubic-bezier(); steps() and other curves we do
            // not evaluate invalidate the declaration, same rule as the
            // shorthand.
            match parse_easing_token(v.trim()) {
                Some(e) => {
                    style.transition.get_or_insert(TransitionSpec {
                        property: None,
                        duration: 0.0,
                        delay: 0.0,
                        easing: e,
                    });
                    true
                }
                None => false,
            }
        }
        "border-radius" => {
            // CSS syntax: 1-4 horizontal radii, optionally `/` plus 1-4
            // vertical radii (the elliptical form). Corners fill in CSS
            // order (TL TR BR BL) from however many values are given.
            let (horiz, vert) = match v.split_once('/') {
                Some((h, s)) => (h, Some(s)),
                None => (v, None),
            };
            let parse_list = |s: &str| -> Option<Vec<Length>> {
                let vals: Vec<Option<Length>> = s.split_whitespace().map(len).collect();
                if vals.iter().any(|v| v.is_none()) || vals.is_empty() || vals.len() > 4 {
                    return None;
                }
                Some(vals.into_iter().flatten().collect())
            };
            let Some(h) = parse_list(horiz) else {
                return Some(false);
            };
            let vlist = match vert {
                Some(s) => match parse_list(s) {
                    Some(v) if v.len() == h.len() => v,
                    _ => return Some(false),
                },
                None => h.clone(),
            };
            // Expand n values to four corners per the CSS mirror rule.
            let expand = |vals: &[Length]| -> [Length; 4] {
                match vals {
                    [a] => [*a, *a, *a, *a],
                    [a, b] => [*a, *b, *a, *b],
                    [a, b, c] => [*a, *b, *c, *b],
                    vals => [vals[0], vals[1], vals[2], vals[3]],
                }
            };
            let hc = expand(&h);
            let vc = expand(&vlist);
            style.corner_radii = Some([
                (hc[0], vc[0]),
                (hc[1], vc[1]),
                (hc[2], vc[2]),
                (hc[3], vc[3]),
            ]);
            // Keep the uniform shortcut in sync for the common 1-value case.
            if matches!(&hc, [a, b, c, d] if a == b && b == c && c == d) && hc[0] == vc[0] {
                style.border_radius = Some(hc[0]);
            } else {
                style.border_radius = None;
            }
            true
        }
        "object-position" => {
            let part = |s: &str| -> Option<ObjectPositionPart> {
                match s {
                    // Keyword positions are their percentage equivalents.
                    "left" | "top" => Some(ObjectPositionPart::Percent(0.0)),
                    "center" => Some(ObjectPositionPart::Percent(50.0)),
                    "right" | "bottom" => Some(ObjectPositionPart::Percent(100.0)),
                    _ => {
                        let num = s.strip_suffix('%').and_then(|n| n.css_f32());
                        if let Some(p) = num {
                            return Some(ObjectPositionPart::Percent(p));
                        }
                        parse_px_f32(s).map(ObjectPositionPart::Px)
                    }
                }
            };
            let vals: Vec<ObjectPositionPart> = v.split_whitespace().filter_map(part).collect();
            match vals.as_slice() {
                [x] => {
                    style.object_position = Some((*x, ObjectPositionPart::Percent(50.0)));
                }
                [x, y] => {
                    style.object_position = Some((*x, *y));
                }
                _ => return Some(false),
            }
            true
        }
        _ => return None,
    })
}

struct ParsedGridShorthand {
    rows: Option<Vec<crate::style_kw::GridTrack>>,
    cols: Option<Vec<crate::style_kw::GridTrack>>,
    rows_css: Option<String>,
    cols_css: Option<String>,
    /// `None` is the initial `row`. `dense` is the row-axis dense form.
    flow: Option<String>,
    auto_rows: Option<String>,
    auto_cols: Option<String>,
}

fn set_grid_tracks(
    v: &str,
    tracks: &mut Option<Vec<crate::style_kw::GridTrack>>,
    specified: &mut Option<String>,
    fonts: &FontCtx,
) -> bool {
    if v.eq_ignore_ascii_case("none") {
        *tracks = None;
        *specified = None;
        return true;
    }
    match (parse_grid_tracks(v, fonts), specified_track_list(v, fonts)) {
        (Some(parsed), Some(css)) => {
            *tracks = Some(parsed);
            *specified = Some(css);
            true
        }
        _ => false,
    }
}

fn set_auto_tracks(v: &str, slot: &mut Option<String>, fonts: &FontCtx) -> bool {
    match specified_auto_tracks(v, fonts) {
        Some(css) => {
            *slot = Some(css);
            true
        }
        None => false,
    }
}

fn split_one_slash(v: &str) -> Option<(&str, &str)> {
    let mut depth = 0i32;
    let mut at = None;
    for (i, c) in v.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            '/' if depth == 0 => {
                if at.is_some() {
                    return None;
                }
                at = Some(i);
            }
            _ => {}
        }
    }
    let i = at?;
    Some((v[..i].trim(), v[i + 1..].trim()))
}

fn paren_tokens(v: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = None;
    for (i, c) in v.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ch if ch.is_whitespace() && depth == 0 => {
                if let Some(s) = start.take() {
                    out.push(&v[s..i]);
                }
            }
            _ => {
                if start.is_none() {
                    start = Some(i);
                }
            }
        }
    }
    if let Some(s) = start {
        out.push(&v[s..]);
    }
    out
}

fn side_has_auto_flow(side: &str) -> bool {
    paren_tokens(side)
        .iter()
        .any(|t| t.eq_ignore_ascii_case("auto-flow"))
}

/// `(dense, optional auto-track spelling)`.
fn parse_auto_flow_side(side: &str, fonts: &FontCtx) -> Option<(bool, Option<String>)> {
    let mut dense = false;
    let mut saw = false;
    let mut rest = String::new();
    for tok in paren_tokens(side) {
        if tok.eq_ignore_ascii_case("auto-flow") {
            if saw {
                return None;
            }
            saw = true;
        } else if tok.eq_ignore_ascii_case("dense") {
            if dense {
                return None;
            }
            dense = true;
        } else {
            if !rest.is_empty() {
                rest.push(' ');
            }
            rest.push_str(tok);
        }
    }
    if !saw {
        return None;
    }
    if rest.is_empty() {
        return Some((dense, None));
    }
    // Auto tracks are `<track-size>+`. A line name makes the shorthand invalid.
    if rest.contains('[') {
        return None;
    }
    parse_grid_tracks(&rest, fonts)?;
    Some((dense, Some(specified_track_list(&rest, fonts)?)))
}

fn parse_track_side(
    side: &str,
    fonts: &FontCtx,
) -> Option<(Option<Vec<crate::style_kw::GridTrack>>, Option<String>)> {
    let t = side.trim();
    if t.is_empty() {
        return None;
    }
    if t.eq_ignore_ascii_case("none") {
        return Some((None, None));
    }
    let parsed = parse_grid_tracks(t, fonts)?;
    Some((Some(parsed), Some(specified_track_list(t, fonts)?)))
}

fn parse_grid_shorthand(v: &str, fonts: &FontCtx) -> Option<ParsedGridShorthand> {
    let v = v.trim();
    if v.contains('\'') || v.contains('"') {
        return None;
    }
    if !v.contains('/') {
        if v.eq_ignore_ascii_case("none") {
            return Some(ParsedGridShorthand {
                rows: None,
                cols: None,
                rows_css: None,
                cols_css: None,
                flow: None,
                auto_rows: None,
                auto_cols: None,
            });
        }
        return None;
    }
    let (left, right) = split_one_slash(v)?;
    let left_flow = side_has_auto_flow(left);
    let right_flow = side_has_auto_flow(right);
    if left_flow && right_flow {
        return None;
    }
    if left_flow {
        let (dense, auto_rows) = parse_auto_flow_side(left, fonts)?;
        let (cols, cols_css) = parse_track_side(right, fonts)?;
        return Some(ParsedGridShorthand {
            rows: None,
            cols,
            rows_css: None,
            cols_css,
            flow: dense.then(|| "dense".to_string()),
            auto_rows,
            auto_cols: None,
        });
    }
    if right_flow {
        let (rows, rows_css) = parse_track_side(left, fonts)?;
        let (dense, auto_cols) = parse_auto_flow_side(right, fonts)?;
        return Some(ParsedGridShorthand {
            rows,
            cols: None,
            rows_css,
            cols_css: None,
            flow: Some(if dense { "column dense" } else { "column" }.to_string()),
            auto_rows: None,
            auto_cols,
        });
    }
    let (rows, rows_css) = parse_track_side(left, fonts)?;
    let (cols, cols_css) = parse_track_side(right, fonts)?;
    Some(ParsedGridShorthand {
        rows,
        cols,
        rows_css,
        cols_css,
        flow: None,
        auto_rows: None,
        auto_cols: None,
    })
}

fn assign_grid_areas(style: &mut ComputedStyle, t: GridAreasTemplate, reset_auto: bool) {
    style.grid_template_rows = Some(t.rows);
    style.grid_template_columns = t.cols;
    style.grid_template_areas = Some(t.areas);
    style.grid_rows_specified = Some(t.rows_css);
    style.grid_cols_specified = t.cols_css;
    style.grid_areas_specified = Some(t.areas_css);
    if reset_auto {
        style.grid_auto_flow_specified = None;
        style.grid_auto_rows_specified = None;
        style.grid_auto_cols_specified = None;
    }
}

/// One value copies to both axes. Two values are align then justify.
fn place_pair(v: &str) -> Option<(String, String)> {
    let toks = split_sides(v);
    let low: Vec<String> = toks.iter().map(|t| t.to_ascii_lowercase()).collect();
    match low.as_slice() {
        [one] => Some((one.clone(), one.clone())),
        [a, j] => Some((a.clone(), j.clone())),
        _ => None,
    }
}

enum AxisKw<T> {
    Initial,
    Set(T),
}

fn justify_kw(t: &str) -> Option<AxisKw<JustifyMode>> {
    Some(match t {
        "normal" | "auto" => AxisKw::Initial,
        "flex-start" => AxisKw::Set(JustifyMode::FlexStart),
        "start" => AxisKw::Set(JustifyMode::Start),
        "flex-end" => AxisKw::Set(JustifyMode::FlexEnd),
        "end" => AxisKw::Set(JustifyMode::End),
        "center" => AxisKw::Set(JustifyMode::Center),
        "space-between" => AxisKw::Set(JustifyMode::SpaceBetween),
        "space-around" => AxisKw::Set(JustifyMode::SpaceAround),
        "space-evenly" => AxisKw::Set(JustifyMode::SpaceEvenly),
        _ => return None,
    })
}

fn align_content_kw(t: &str) -> Option<AxisKw<AlignContentMode>> {
    Some(match t {
        "normal" | "auto" => AxisKw::Initial,
        "flex-start" => AxisKw::Set(AlignContentMode::FlexStart),
        "start" => AxisKw::Set(AlignContentMode::Start),
        "flex-end" => AxisKw::Set(AlignContentMode::FlexEnd),
        "end" => AxisKw::Set(AlignContentMode::End),
        "center" => AxisKw::Set(AlignContentMode::Center),
        "stretch" => AxisKw::Set(AlignContentMode::Stretch),
        "space-between" => AxisKw::Set(AlignContentMode::SpaceBetween),
        "space-around" => AxisKw::Set(AlignContentMode::SpaceAround),
        "space-evenly" => AxisKw::Set(AlignContentMode::SpaceEvenly),
        _ => return None,
    })
}

fn align_kw(t: &str) -> Option<AxisKw<AlignMode>> {
    Some(match t {
        "normal" => AxisKw::Initial,
        "stretch" => AxisKw::Set(AlignMode::Stretch),
        "flex-start" => AxisKw::Set(AlignMode::FlexStart),
        "start" => AxisKw::Set(AlignMode::Start),
        "center" => AxisKw::Set(AlignMode::Center),
        "flex-end" => AxisKw::Set(AlignMode::FlexEnd),
        "end" => AxisKw::Set(AlignMode::End),
        _ => return None,
    })
}

fn align_self_kw(t: &str) -> Option<AxisKw<AlignMode>> {
    Some(match t {
        "auto" | "normal" => AxisKw::Initial,
        "stretch" => AxisKw::Set(AlignMode::Stretch),
        "flex-start" => AxisKw::Set(AlignMode::FlexStart),
        "start" => AxisKw::Set(AlignMode::Start),
        "center" => AxisKw::Set(AlignMode::Center),
        "flex-end" => AxisKw::Set(AlignMode::FlexEnd),
        "end" => AxisKw::Set(AlignMode::End),
        _ => return None,
    })
}

/// `justify-items` / `justify-self`, including the legacy keyword forms.
fn justify_item_kw(t: &str) -> Option<AxisKw<JustifyMode>> {
    Some(match t {
        "auto" | "normal" | "legacy" => AxisKw::Initial,
        "flex-start" => AxisKw::Set(JustifyMode::FlexStart),
        "start" | "self-start" => AxisKw::Set(JustifyMode::Start),
        "left" | "legacy left" => AxisKw::Set(JustifyMode::FlexStart),
        "stretch" => AxisKw::Set(JustifyMode::Stretch),
        "center" | "legacy center" => AxisKw::Set(JustifyMode::Center),
        "flex-end" => AxisKw::Set(JustifyMode::FlexEnd),
        "end" | "self-end" => AxisKw::Set(JustifyMode::End),
        "right" | "legacy right" => AxisKw::Set(JustifyMode::FlexEnd),
        _ => return None,
    })
}

fn axis_slot<T>(kw: AxisKw<T>) -> Option<T> {
    match kw {
        AxisKw::Initial => None,
        AxisKw::Set(v) => Some(v),
    }
}

/// One parenthesis-aware token. A second token rejects the longhand.
fn one_col_token(v: &str) -> Option<String> {
    let toks = split_sides(v);
    if toks.len() == 1 {
        Some(toks[0].to_ascii_lowercase())
    } else {
        None
    }
}

/// `apply_one` family: flex and grid alignment and tracks.
/// `None` = not this family's property; the caller tries the next one.
pub(crate) fn apply_flex_grid(
    style: &mut ComputedStyle,
    name: &str,
    _value: &str,
    v: &str,
    fonts: &FontCtx,
) -> Option<bool> {
    let len = |val: &str| css_len(val, fonts);
    Some(match name {
        "flex-direction" => {
            style.flex_direction = match v {
                "row" => Some(FlexDirection::Row),
                "row-reverse" => Some(FlexDirection::RowReverse),
                "column" => Some(FlexDirection::Column),
                "column-reverse" => Some(FlexDirection::ColumnReverse),
                _ => return Some(false),
            };
            true
        }
        "flex-wrap" => {
            style.flex_wrap = match v {
                "nowrap" => Some(FlexWrapMode::NoWrap),
                "wrap" => Some(FlexWrapMode::Wrap),
                _ => return Some(false),
            };
            true
        }
        "justify-content" => match justify_kw(v) {
            Some(mode) => {
                style.justify_content = axis_slot(mode);
                true
            }
            None => false,
        },
        "align-items" => match align_kw(v) {
            Some(mode) => {
                style.align_items = axis_slot(mode);
                true
            }
            None => false,
        },
        // Cross-axis line/track distribution (blitz#1059). `normal`/`auto`
        // = the initial value, kept unset like the computed-value face
        // answers "normal"; `safe `/`unsafe ` prefixes drop like
        // align-self's.
        "align-content" => {
            let v = v
                .trim()
                .strip_prefix("safe ")
                .or_else(|| v.trim().strip_prefix("unsafe "))
                .unwrap_or(v.trim());
            style.align_content = match align_content_kw(v) {
                Some(mode) => axis_slot(mode),
                None => return Some(false),
            };
            true
        }
        // Item-side self-alignment (#188-2, blitz#977). `auto`/`normal`
        // defer to the container (stored as None); `safe `/`unsafe `
        // prefixes drop (alignment subjects only, they don't move the
        // static position for our purposes). `baseline` on a sole
        // out-of-flow item pins to its start edge — FlexStart is the
        // measurable approximation.
        "align-self" => {
            let v = v
                .strip_prefix("safe ")
                .or_else(|| v.strip_prefix("unsafe "))
                .unwrap_or(v);
            style.align_self = match v {
                "auto" | "normal" => None,
                "stretch" => Some(AlignMode::Stretch),
                "flex-start" | "self-start" | "baseline" | "first baseline" => {
                    Some(AlignMode::FlexStart)
                }
                "start" => Some(AlignMode::Start),
                "center" => Some(AlignMode::Center),
                "flex-end" | "self-end" | "last baseline" => Some(AlignMode::FlexEnd),
                "end" => Some(AlignMode::End),
                _ => return Some(false),
            };
            true
        }
        "justify-self" => {
            let v = v
                .strip_prefix("safe ")
                .or_else(|| v.strip_prefix("unsafe "))
                .unwrap_or(v);
            style.justify_self = match justify_item_kw(v) {
                Some(mode) => axis_slot(mode),
                None => return Some(false),
            };
            true
        }
        "justify-items" => {
            let v = v
                .strip_prefix("safe ")
                .or_else(|| v.strip_prefix("unsafe "))
                .unwrap_or(v);
            style.justify_items = match justify_item_kw(v) {
                Some(mode) => axis_slot(mode),
                None => return Some(false),
            };
            true
        }
        "place-content" => match place_pair(v) {
            Some((a, j)) => match (align_content_kw(&a), justify_kw(&j)) {
                (Some(ac), Some(jc)) => {
                    style.align_content = axis_slot(ac);
                    style.justify_content = axis_slot(jc);
                    true
                }
                _ => false,
            },
            None => false,
        },
        "place-items" => match place_pair(v) {
            Some((a, j)) => match (align_kw(&a), justify_item_kw(&j)) {
                (Some(ai), Some(ji)) => {
                    style.align_items = axis_slot(ai);
                    style.justify_items = axis_slot(ji);
                    true
                }
                _ => false,
            },
            None => false,
        },
        "place-self" => match place_pair(v) {
            Some((a, j)) => match (align_self_kw(&a), justify_item_kw(&j)) {
                (Some(aself), Some(jself)) => {
                    style.align_self = axis_slot(aself);
                    style.justify_self = axis_slot(jself);
                    true
                }
                _ => false,
            },
            None => false,
        },
        "flex-grow" => {
            style.flex_grow = parse_num_f32(v);
            style.flex_grow.is_some()
        }
        "flex-shrink" => {
            style.flex_shrink = parse_num_f32(v);
            style.flex_shrink.is_some()
        }
        // `flex` shorthand (css-flexbox-1 §7.1.1):
        // `none | [ <'flex-grow'> <'flex-shrink'>? || <'flex-basis'> ]`.
        // The gotcha: omitted components take the SHORTHAND's initial values
        // (1 / 1 / 0%), not the longhands' (0 / 1 / auto) — `flex: 1` is
        // `1 1 0%`, so equal-grow cards align even with unequal content,
        // and `flex: 0 0 100vw` (the deck/carousel idiom, #61) sizes empty
        // items. Tokens classify positionally: numbers fill grow then
        // shrink, a length/percent/`auto` fills basis, either order (`||`
        // admits `flex: 100px 0`). Like every shorthand it sets all three
        // sub-longhands, so it resets earlier longhand writes.
        "flex" => {
            if v.eq_ignore_ascii_case("none") {
                style.flex_grow = Some(0.0);
                style.flex_shrink = Some(0.0);
                style.flex_basis = None; // auto
                true
            } else {
                let mut grow: Option<f32> = None;
                let mut shrink: Option<f32> = None;
                // Some(None) = explicit `auto`; bare None = omitted → 0%.
                let mut basis: Option<Option<Length>> = None;
                for tok in split_sides(v) {
                    let t = tok.trim();
                    if t.eq_ignore_ascii_case("auto") {
                        if basis.is_some() {
                            return Some(false);
                        }
                        basis = Some(None);
                    } else if let Some(n) = parse_num_f32(t) {
                        if grow.is_none() {
                            grow = Some(n);
                        } else if shrink.is_none() {
                            shrink = Some(n);
                        } else {
                            return Some(false);
                        }
                    } else if let Some(l) = len(t) {
                        if basis.is_some() {
                            return Some(false);
                        }
                        basis = Some(Some(l));
                    } else {
                        return Some(false);
                    }
                }
                match grow {
                    Some(g) => {
                        style.flex_grow = Some(g);
                        style.flex_shrink = Some(shrink.unwrap_or(1.0));
                        // Explicit `auto` (Some(None)) must survive — only an
                        // omitted basis becomes the shorthand's 0% initial.
                        style.flex_basis = match basis {
                            Some(b) => b,
                            None => Some(Length::Percent(0.0)),
                        };
                        true
                    }
                    // No number anywhere (`flex: auto`-less forms like
                    // `flex: 10px`) are invalid — drop the declaration.
                    None => false,
                }
            }
        }
        "flex-basis" => {
            // <length-percentage> | auto (the initial value, stays None).
            style.flex_basis = len(v);
            style.flex_basis.is_some()
        }
        "gap" => {
            let vals: Vec<Option<Length>> = split_sides(v).into_iter().map(&len).collect();
            match vals.as_slice() {
                [Some(one)] => {
                    style.column_gap = Some(*one);
                    style.row_gap = Some(*one);
                    true
                }
                // CSS gap is <row-gap> <column-gap>, not column then row.
                [Some(row), Some(col)] => {
                    style.row_gap = Some(*row);
                    style.column_gap = Some(*col);
                    true
                }
                // One invalid component invalidates the whole declaration
                // (CSS syntax) — no partial application. `auto` is not in
                // gap's grammar: len() rejects it, same outcome.
                _ => false,
            }
        }
        "column-gap" => {
            style.column_gap = len(v);
            style.column_gap.is_some()
        }
        "row-gap" => {
            style.row_gap = len(v);
            style.row_gap.is_some()
        }
        "grid-template-columns" => set_grid_tracks(
            v,
            &mut style.grid_template_columns,
            &mut style.grid_cols_specified,
            fonts,
        ),
        "grid-template-rows" => set_grid_tracks(
            v,
            &mut style.grid_template_rows,
            &mut style.grid_rows_specified,
            fonts,
        ),
        "grid-auto-rows" => set_auto_tracks(v, &mut style.grid_auto_rows_specified, fonts),
        "grid-auto-columns" => set_auto_tracks(v, &mut style.grid_auto_cols_specified, fonts),
        "grid-template-areas" => {
            if v.eq_ignore_ascii_case("none") {
                style.grid_template_areas = None;
                style.grid_areas_specified = None;
                true
            } else {
                match parse_grid_template_areas(v) {
                    Some(areas) => {
                        style.grid_areas_specified = Some(grid_areas_css(&areas));
                        style.grid_template_areas = Some(areas);
                        true
                    }
                    None => false,
                }
            }
        }
        // `grid` shorthand. The areas form (`'a a' / 1fr 1fr`) sets rows,
        // columns and areas, and resets the auto-* longhands. A grid
        // container's used px still comes from layout.
        "grid" => {
            if v.contains('\'') || v.contains('"') {
                match parse_grid_areas_template(v, fonts) {
                    Some(t) => {
                        assign_grid_areas(style, t, true);
                        true
                    }
                    None => false,
                }
            } else {
                match parse_grid_shorthand(v, fonts) {
                    Some(g) => {
                        style.grid_template_rows = g.rows;
                        style.grid_template_columns = g.cols;
                        style.grid_template_areas = None;
                        style.grid_rows_specified = g.rows_css;
                        style.grid_cols_specified = g.cols_css;
                        style.grid_areas_specified = None;
                        style.grid_auto_flow_specified = g.flow;
                        style.grid_auto_rows_specified = g.auto_rows;
                        style.grid_auto_cols_specified = g.auto_cols;
                        true
                    }
                    None => false,
                }
            }
        }
        // Shorthand `none`, `rows / columns`, or the areas form. All three
        // set rows, columns and areas together. Unlike `grid`, this does
        // not reset grid-auto-flow / grid-auto-rows / columns. A value with
        // no slash and no area strings is invalid, so the previous template
        // stays.
        "grid-template" => {
            if v.eq_ignore_ascii_case("none") {
                style.grid_template_rows = None;
                style.grid_template_columns = None;
                style.grid_template_areas = None;
                style.grid_rows_specified = None;
                style.grid_cols_specified = None;
                style.grid_areas_specified = None;
                true
            } else if v.contains('\'') || v.contains('"') {
                match parse_grid_areas_template(v, fonts) {
                    Some(t) => {
                        assign_grid_areas(style, t, false);
                        true
                    }
                    None => false,
                }
            } else if let Some((rows, cols)) = split_one_slash(v) {
                // Parse both sides before writing. One invalid side drops
                // the whole declaration.
                match (parse_track_side(rows, fonts), parse_track_side(cols, fonts)) {
                    (Some((rows, rows_css)), Some((cols, cols_css))) => {
                        style.grid_template_rows = rows;
                        style.grid_template_columns = cols;
                        style.grid_rows_specified = rows_css;
                        style.grid_cols_specified = cols_css;
                        style.grid_template_areas = None;
                        style.grid_areas_specified = None;
                        true
                    }
                    _ => false,
                }
            } else {
                false
            }
        }
        "grid-auto-flow" => match parse_grid_auto_flow(v) {
            Some(flow) => {
                style.grid_auto_flow_specified = flow;
                true
            }
            None => false,
        },
        // `<name>` fills all four lines. Up to four grid lines are
        // `row-start / column-start / row-end / column-end`.
        "grid-area" => match parse_grid_area(v) {
            Some(g) => {
                style.grid_area = g.area_name;
                style.grid_row_start = g.row_start;
                style.grid_row_end = g.row_end;
                style.grid_column_start = g.column_start;
                style.grid_column_end = g.column_end;
                true
            }
            None => false,
        },
        "column-width" => {
            match one_col_token(v).and_then(|t| parse_column_width_token(&t, fonts)) {
                Some(ColWidth::Auto) => {
                    style.column_width = None;
                    true
                }
                Some(ColWidth::Px(px)) => {
                    style.column_width = Some(Length::Px(px));
                    true
                }
                None => false,
            }
        }
        "column-count" => match one_col_token(v).and_then(|t| parse_column_count_token(&t)) {
            Some(ColCount::Auto) => {
                style.column_count = None;
                true
            }
            Some(ColCount::N(n)) => {
                style.column_count = Some(n);
                true
            }
            None => false,
        },
        "columns" => match parse_columns_shorthand(v, fonts) {
            Some((width, count)) => {
                style.column_width = match width {
                    ColWidth::Auto => None,
                    ColWidth::Px(px) => Some(Length::Px(px)),
                };
                style.column_count = match count {
                    ColCount::Auto => None,
                    ColCount::N(n) => Some(n),
                };
                true
            }
            None => false,
        },
        _ => return None,
    })
}

/// `apply_one` family: position/float/clear, generated content, insets, min/max sizes.
/// `None` = not this family's property; the caller tries the next one.
pub(crate) fn apply_position(
    style: &mut ComputedStyle,
    name: &str,
    value: &str,
    v: &str,
    fonts: &FontCtx,
) -> Option<bool> {
    let len = |val: &str| css_len(val, fonts);
    let box_offset = |slot: &mut Option<Length>| css_box_offset(slot, v, fonts);
    Some(match name {
        "position" => {
            style.position = match v {
                "static" => Some(PositionMode::Static),
                "relative" => Some(PositionMode::Relative),
                "absolute" => Some(PositionMode::Absolute),
                "fixed" => Some(PositionMode::Fixed),
                "sticky" => Some(PositionMode::Sticky),
                _ => return Some(false),
            };
            true
        }
        "float" => {
            style.float_side = match v {
                "left" => Some(FloatSide::Left),
                "right" => Some(FloatSide::Right),
                "none" => None,
                _ => return Some(false),
            };
            true
        }
        "clear" => {
            style.clear_side = match v {
                "left" => Some(ClearSide::Left),
                "right" => Some(ClearSide::Right),
                "both" => Some(ClearSide::Both),
                "inline-start" => Some(ClearSide::InlineStart),
                "inline-end" => Some(ClearSide::InlineEnd),
                "none" => None,
                _ => return Some(false),
            };
            true
        }
        "content" => match parse_content_value(value) {
            // none/normal (or unparseable) leave the field unset: no pseudo
            // box. `content` only ever matters on a generated-content pseudo.
            Some(cv) => {
                style.content = Some(cv);
                true
            }
            None => false,
        },
        "counter-reset" => match parse_counter_modifiers(value, false) {
            Some(m) => {
                style.counter_reset = m;
                true
            }
            None => false,
        },
        "counter-increment" => match parse_counter_modifiers(value, true) {
            Some(m) => {
                style.counter_increment = m;
                true
            }
            None => false,
        },
        "quotes" => match parse_quotes(value) {
            Some(q) => {
                style.quotes = Some(q);
                true
            }
            None => false,
        },
        // Box offsets route through `box_offset` (#129 family): `auto`
        // resets, unparseable values drop without touching the cascade.
        "top" => box_offset(&mut style.top),
        "right" => box_offset(&mut style.right),
        "bottom" => box_offset(&mut style.bottom),
        "left" => box_offset(&mut style.left),
        // inset shorthand: <length-percentage>{1,4} | auto. Same expansion
        // family as margin, but auto is the property initial value here
        // (None slots, not Length::Auto) and `inset: auto` resets all four.
        "inset" => {
            if v.eq_ignore_ascii_case("auto") {
                style.top = None;
                style.right = None;
                style.bottom = None;
                style.left = None;
                true
            } else {
                let sides = expand_sides(v, fonts);
                let conv = |x: Option<Length>| match x {
                    Some(Length::Auto) => None,
                    other => other,
                };
                style.top = conv(sides.top);
                style.right = conv(sides.right);
                style.bottom = conv(sides.bottom);
                style.left = conv(sides.left);
                style.top.is_some()
                    || style.right.is_some()
                    || style.bottom.is_some()
                    || style.left.is_some()
            }
        }
        // min/max-width sizing keywords are valid CSS but not resolved by the
        // layout layer yet — they stay unparsed (drop to auto) rather than
        // parse-into-nothing.
        "min-width" => {
            style.min_width = len(v);
            style.min_width_expr = None;
            if let (None, Some((l, e))) = (style.min_width, calc::pct_math(v, fonts)) {
                (style.min_width, style.min_width_expr) = (Some(l), Some(e));
            }
            style.min_width.is_some()
        }
        "max-width" => {
            style.max_width = len(v);
            style.max_width_expr = None;
            if let (None, Some((l, e))) = (style.max_width, calc::pct_math(v, fonts)) {
                (style.max_width, style.max_width_expr) = (Some(l), Some(e));
            }
            style.max_width.is_some()
        }
        "min-height" => {
            style.min_height = len(v);
            style.min_height.is_some()
        }
        "max-height" => {
            style.max_height = len(v);
            style.max_height.is_some()
        }
        "aspect-ratio" => {
            // `1.5` or `16 / 9`; `auto` and invalid ratios stay None.
            style.aspect_ratio = parse_aspect_ratio(v);
            style.aspect_ratio.is_some()
        }
        "box-sizing" => {
            style.box_sizing = match v {
                "content-box" => Some(BoxSizing::ContentBox),
                "border-box" => Some(BoxSizing::BorderBox),
                _ => return Some(false),
            };
            true
        }
        "background-clip" | "-webkit-background-clip" => {
            style.background_clip_text = match v {
                "text" => true,
                "border-box" | "padding-box" | "content-box" => false,
                _ => return Some(false),
            };
            true
        }
        // box-decoration-break (#218): v1 consumed only by the inline
        // background-clip: text fill — `clone` restarts the gradient per
        // wrapped line, the initial `slice` keeps one continuous box.
        "box-decoration-break" | "-webkit-box-decoration-break" => {
            style.box_decoration_clone = match v {
                "clone" => true,
                "slice" => false,
                _ => return Some(false),
            };
            true
        }
        _ => return None,
    })
}
