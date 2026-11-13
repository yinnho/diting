//! Declaration appliers: effects, logical motion, flex/grid, position.
//!
//! Split from the css hub. `apply_one` still dispatches into these.
//! Pure move.

use super::*;

/// `apply_one` family: containers, overflow, shadows/filters, clamping, transforms.
/// `None` = not this family's property; the caller tries the next one.
pub(super) fn apply_effects(
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
        // transform-origin (#244): keywords normalize to percentages; the
        // pivot itself resolves at collect time (layout mod), this only
        // stores the pair.
        "transform-origin" => {
            style.transform_origin = parse_transform_origin(v, fonts);
            style.transform_origin.is_some()
        }
        _ => return None,
    })
}

/// `apply_one` family: logical box edges, opacity/zoom, animation/transition, radius.
/// `None` = not this family's property; the caller tries the next one.
pub(super) fn apply_logical_motion(
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

/// `apply_one` family: flex and grid alignment and tracks.
/// `None` = not this family's property; the caller tries the next one.
pub(super) fn apply_flex_grid(
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
        "justify-content" => {
            style.justify_content = match v {
                "flex-start" | "start" => Some(JustifyMode::FlexStart),
                "center" => Some(JustifyMode::Center),
                "flex-end" | "end" => Some(JustifyMode::FlexEnd),
                "space-between" => Some(JustifyMode::SpaceBetween),
                "space-around" => Some(JustifyMode::SpaceAround),
                "space-evenly" => Some(JustifyMode::SpaceEvenly),
                _ => return Some(false),
            };
            true
        }
        "align-items" => {
            style.align_items = match v {
                "stretch" => Some(AlignMode::Stretch),
                "flex-start" | "start" => Some(AlignMode::FlexStart),
                "center" => Some(AlignMode::Center),
                "flex-end" | "end" => Some(AlignMode::FlexEnd),
                _ => return Some(false),
            };
            true
        }
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
            style.align_content = match v {
                "flex-start" | "start" => Some(AlignContentMode::FlexStart),
                "center" => Some(AlignContentMode::Center),
                "flex-end" | "end" => Some(AlignContentMode::FlexEnd),
                "space-between" => Some(AlignContentMode::SpaceBetween),
                "space-around" => Some(AlignContentMode::SpaceAround),
                "space-evenly" => Some(AlignContentMode::SpaceEvenly),
                "stretch" => Some(AlignContentMode::Stretch),
                "normal" | "auto" => None,
                _ => return Some(false),
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
                "flex-start" | "start" | "self-start" | "baseline" | "first baseline" => {
                    Some(AlignMode::FlexStart)
                }
                "center" => Some(AlignMode::Center),
                "flex-end" | "end" | "self-end" | "last baseline" => Some(AlignMode::FlexEnd),
                _ => return Some(false),
            };
            true
        }
        // JustifyMode has no Stretch variant: with auto insets and a
        // definite size the box doesn't stretch anyway (CSS2 §10.3.7), so
        // `stretch` behaving as start is the documented approximation.
        "justify-self" => {
            let v = v
                .strip_prefix("safe ")
                .or_else(|| v.strip_prefix("unsafe "))
                .unwrap_or(v);
            style.justify_self = match v {
                "auto" | "normal" => None,
                "start" | "left" | "self-start" | "stretch" => Some(JustifyMode::FlexStart),
                "center" => Some(JustifyMode::Center),
                "end" | "right" | "self-end" => Some(JustifyMode::FlexEnd),
                _ => return Some(false),
            };
            true
        }
        "justify-items" => {
            let v = v
                .strip_prefix("safe ")
                .or_else(|| v.strip_prefix("unsafe "))
                .unwrap_or(v);
            style.justify_items = match v {
                "auto" | "normal" | "legacy" => None,
                "start" | "left" | "stretch" | "legacy left" => Some(JustifyMode::FlexStart),
                "center" | "legacy center" => Some(JustifyMode::Center),
                "end" | "right" | "legacy right" => Some(JustifyMode::FlexEnd),
                _ => return Some(false),
            };
            true
        }
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
                [Some(c), Some(r)] => {
                    style.column_gap = Some(*c);
                    style.row_gap = Some(*r);
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
        "grid-template-columns" => {
            style.grid_template_columns = parse_grid_tracks(v);
            style.grid_template_columns.is_some()
        }
        "grid-template-rows" => {
            style.grid_template_rows = parse_grid_tracks(v);
            style.grid_template_rows.is_some()
        }
        "grid-template-areas" => {
            style.grid_template_areas = parse_grid_template_areas(v);
            style.grid_template_areas.is_some()
        }
        // Shorthand `rows / columns`. The string-literal-areas form
        // (`grid-template: 'a b' 1fr / 2rem`) is not supported — both halves
        // fall through to None rather than half-parse.
        "grid-template" => {
            if let Some((rows, cols)) = v.split_once('/') {
                if !v.contains('\'') {
                    style.grid_template_rows = parse_grid_tracks(rows);
                    style.grid_template_columns = parse_grid_tracks(cols);
                }
            }
            true
        }
        // Single-ident named form only (`grid-area: main`); the 4-value
        // numeric form is a later batch.
        "grid-area" => {
            let name = v.trim();
            if !name.contains(' ') && !name.contains('/') && !name.is_empty() {
                style.grid_area = Some(name.to_string());
            }
            true
        }
        _ => return None,
    })
}

/// `apply_one` family: position/float/clear, generated content, insets, min/max sizes.
/// `None` = not this family's property; the caller tries the next one.
pub(super) fn apply_position(
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
