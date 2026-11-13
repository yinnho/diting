//! Declaration appliers: display, paint, font, box model, text sizing.
//!
//! Split from the css hub. `apply_one` still dispatches into these.
//! Pure move.

use super::*;

/// `apply_one` family: display, table and list layout keywords, vertical-align.
/// `None` = not this family's property; the caller tries the next one.
pub(super) fn apply_display(
    style: &mut ComputedStyle,
    name: &str,
    _value: &str,
    v: &str,
    fonts: &FontCtx,
) -> Option<bool> {
    Some(match name {
        "display" => {
            // inline-flex/inline-grid (#107) and inline-table (its follow-up):
            // Fusion's .next-input family is `display: inline-table;
            // width: 200px` with `width: 100%` inputs inside — dropping the
            // declaration (the pre-#107 behavior for each missing value)
            // left 32 inputs at width 0 on the Tmall publish page. taffy
            // only needs the container mode, so they map onto plain
            // Flex/Grid/Table; the outer inline participation is a
            // refinement, not a correctness requirement.
            // -webkit-box (#275): the legacy-flexbox line-clamp display.
            // Before this arm existed the declaration DROPPED, so every
            // -webkit-box page already lived with plain block geometry —
            // mapping to Block/InlineBlock changes nothing about layout,
            // it only arms the clamp gate (see clamp_lines). The flag
            // resets on any later valid display declaration.
            let webkit_box = match v {
                "-webkit-box" => Some(WebkitBoxKind::Box),
                "-webkit-inline-box" => Some(WebkitBoxKind::InlineBox),
                _ => None,
            };
            style.display = match v {
                "block" => Some(Display::Block),
                "inline" => Some(Display::Inline),
                "inline-block" => Some(Display::InlineBlock),
                "flex" | "inline-flex" => Some(Display::Flex),
                "grid" | "inline-grid" => Some(Display::Grid),
                "table" | "inline-table" => Some(Display::Table),
                "table-row" => Some(Display::TableRow),
                "table-cell" => Some(Display::TableCell),
                "table-caption" => Some(Display::TableCaption),
                "none" => Some(Display::None),
                "-webkit-box" => Some(Display::Block),
                "-webkit-inline-box" => Some(Display::InlineBlock),
                _ => return Some(false),
            };
            style.webkit_box = webkit_box;
            style.display_from_ua = false;
            true
        }
        "border-collapse" => {
            style.border_collapse = match v {
                "collapse" => Some(BorderCollapse::Collapse),
                "separate" => Some(BorderCollapse::Separate),
                _ => return Some(false),
            };
            true
        }
        "table-layout" => {
            style.table_layout = match v {
                "auto" => Some(TableLayout::Auto),
                "fixed" => Some(TableLayout::Fixed),
                _ => return Some(false),
            };
            true
        }
        "list-style-type" => {
            style.list_style_type = parse_list_style_type(v);
            true
        }
        "list-style" => {
            // Shorthand: each token feeds its own slot; position
            // (inside/outside) parses but has no v1 paint distinction,
            // images (url()) drop. `none` alone is the classic marker
            // reset.
            let mut seen = false;
            for tok in v.split_whitespace() {
                if matches!(tok, "inside" | "outside") || tok.starts_with("url(") {
                    continue;
                }
                if let Some(t) = parse_list_style_type(tok) {
                    style.list_style_type = Some(t);
                    seen = true;
                }
            }
            seen
        }
        "vertical-align" => {
            match v {
                "top" => style.vertical_align = Some(VerticalAlign::Top),
                "baseline" => style.vertical_align = Some(VerticalAlign::Baseline),
                "middle" => style.vertical_align = Some(VerticalAlign::Middle),
                "bottom" => style.vertical_align = Some(VerticalAlign::Bottom),
                "sub" => style.vertical_align = Some(VerticalAlign::Sub),
                "super" => style.vertical_align = Some(VerticalAlign::Super),
                _ => match parse_css_length(v).map(|l| resolve_len(l, fonts)) {
                    // Lengths resolve to px now (em against the element's own
                    // font-size); the % keeps its shape for the layout side,
                    // which resolves it against the element's line-height.
                    Some(Length::Px(px)) => style.vertical_align = Some(VerticalAlign::Length(px)),
                    Some(Length::Percent(p)) => {
                        style.vertical_align = Some(VerticalAlign::Percent(p))
                    }
                    _ => return Some(false),
                },
            }
            true
        }
        _ => return None,
    })
}

/// `apply_one` family: text decoration, color/color-scheme, SVG fill/stroke.
/// `None` = not this family's property; the caller tries the next one.
pub(super) fn apply_paint_text(
    style: &mut ComputedStyle,
    name: &str,
    _value: &str,
    v: &str,
    _fonts: &FontCtx,
) -> Option<bool> {
    Some(match name {
        "text-decoration-line" => match parse_text_decoration(v, false) {
            Some(d) => {
                style.text_decoration_line = Some(d);
                true
            }
            None => false,
        },
        "text-decoration" => match parse_text_decoration(v, true) {
            Some(d) => {
                style.text_decoration_line = Some(d);
                true
            }
            None => false,
        },
        "color" => color::parse_color_with_scheme(v, scheme_dark_of(style))
            .map(|(c, m)| {
                style.color = Some(c);
                style.color_modern = m;
            })
            .is_some(),
        // `color-scheme` (#18): `normal` (or nothing) stores None — the gCS
        // face reports "normal" for both; authored `light`/`dark` token
        // lists store verbatim (lowercased). `only` and custom schemes fail
        // closed (declaration drops, stays normal) — v1 models the two
        // schemes light-dark() can resolve.
        "color-scheme" => {
            let toks: Vec<String> = v
                .split_whitespace()
                .map(|t| t.to_ascii_lowercase())
                .collect();
            if toks.is_empty() {
                false
            } else if toks.iter().all(|t| t == "normal") {
                style.color_scheme = None;
                true
            } else if toks.iter().all(|t| t == "light" || t == "dark") {
                style.color_scheme = Some(toks.join(" "));
                true
            } else {
                false
            }
        }
        // SVG presentation properties (svg v1): they cascade like any other
        // property — archify-class diagrams color shapes through classes +
        // custom properties (`fill: var(--frontend-fill)`), so the values
        // arrive here var()-substituted. `none` maps to a fully transparent
        // paint (both mean "draw nothing"); `currentColor` stays symbolic
        // and resolves against the element's inherited `color` at svg
        // compile time. url(#paint) references (gradients/patterns) drop
        // the declaration, falling back to inherited/attribute/default.
        "fill" | "stroke" => {
            let paint = if v.eq_ignore_ascii_case("none") {
                SvgPaint::Color(Color(0, 0, 0, 0))
            } else if v.eq_ignore_ascii_case("currentColor") {
                SvgPaint::CurrentColor
            } else {
                match parse_color(v) {
                    Some(c) => SvgPaint::Color(c),
                    None => return Some(false),
                }
            };
            if name == "fill" {
                style.svg_fill = Some(paint);
            } else {
                style.svg_stroke = Some(paint);
            }
            true
        }
        "stroke-width" => match v.parse::<f32>() {
            Ok(w) if w.is_finite() && w >= 0.0 => {
                style.svg_stroke_width = Some(w);
                true
            }
            _ => false,
        },
        "stroke-dasharray" => {
            let list: Option<Vec<f32>> = v
                .split(|c: char| c == ',' || c.is_whitespace())
                .filter(|t| !t.is_empty())
                .map(|t| t.css_f32().filter(|n| n.is_finite() && *n >= 0.0))
                .collect();
            match list {
                Some(l) if !l.is_empty() => {
                    style.svg_dasharray = Some(l);
                    true
                }
                _ => false,
            }
        }
        "stroke-dashoffset" => match v.parse::<f32>() {
            Ok(n) if n.is_finite() => {
                style.svg_dashoffset = Some(n);
                true
            }
            _ => false,
        },
        _ => return None,
    })
}

/// `apply_one` family: font shorthand, image/mask/clip-path layers, font-family, background.
/// `None` = not this family's property; the caller tries the next one.
pub(super) fn apply_font_image(
    style: &mut ComputedStyle,
    name: &str,
    _value: &str,
    v: &str,
    fonts: &FontCtx,
) -> Option<bool> {
    Some(match name {
        // `font` shorthand: `[ <style> || <variant> || <weight> || <stretch> ]?
        // <size> [ / <line-height> ]? <family>`. font-style/variant/stretch
        // tokens are accepted and skipped (the style struct models none of
        // them); weight/size/line-height/family apply through the same parse
        // paths as the longhands. Per shorthand reset semantics the modeled
        // longhands reset to their initial first, so `font: 20px serif` on a
        // <b> clears the bold like Chrome (the expanded author-initial
        // declaration beats the UA bolder).
        "font" => match parse_font_shorthand(v, fonts) {
            Some(p) => {
                style.font_weight = Some(p.weight);
                style.line_height = Some(p.line_height.unwrap_or(LineHeightSpec::Normal));
                if let Some(px) = p.size_px {
                    style.font_size = Some(px);
                }
                style.font_family = Some(p.family);
                // font-variant-caps is modeled now, so the shorthand reset
                // covers it too: `font: 20px serif` after small-caps drops
                // the caps like Chrome; `font: small-caps 20px serif` sets it.
                style.font_variant_caps = Some(p.small_caps);
                true
            }
            None => false,
        },
        // Raw passthrough: no parsed form, no layout effect — the CSSOM
        // layer reports it verbatim (var() already substituted upstream).
        "background-image" => {
            style.background_image = Some(v.to_string());
            true
        }
        // Same raw passthrough as background-image; the CSSOM arm runs the
        // #17 stop re-serialization over it, so only sane image grammars
        // should land in the slot (#22).
        "mask-image" => {
            let lower = v.to_ascii_lowercase();
            if v.eq_ignore_ascii_case("none")
                || lower.contains("gradient(")
                || lower.starts_with("url(")
                || lower.contains("image(")
                || lower.contains("cross-fade(")
                || lower.contains("element(")
            {
                style.mask_image = Some(v.to_string());
                true
            } else {
                false
            }
        }
        // `clip-path` raw store, the mask-image posture over shapes (#24):
        // basic shapes, url() references and `none`; anything else (a bare
        // color, a length) is invalid and drops like Chrome's recovery.
        "clip-path" => {
            let lower = v.to_ascii_lowercase();
            if v.eq_ignore_ascii_case("none")
                || lower.starts_with("url(")
                || lower.contains("circle(")
                || lower.contains("ellipse(")
                || lower.contains("inset(")
                || lower.contains("polygon(")
                || lower.contains("path(")
            {
                style.clip_path = Some(v.to_string());
                true
            } else {
                false
            }
        }
        // `mask` shorthand: the `background` posture over mask-image —
        // paren-aware split, gradient/url tokens feed mask_image (layers
        // comma-joined, Chrome-style), `none` and the repeat/position/clip/
        // composite/mode keywords are recognized and ignored (Chrome
        // expands eight longhands; only the image one is modeled, #22).
        // Shorthand resets first like every CSS shorthand.
        "mask" => {
            style.mask_image = None;
            let mut layers: Vec<&str> = Vec::new();
            let mut applied = false;
            for tok in split_sides(v) {
                let t = tok.trim_end_matches(',');
                if t.to_ascii_lowercase().contains("gradient(") || t.starts_with("url(") {
                    layers.push(t);
                    applied = true;
                } else if t.eq_ignore_ascii_case("none") {
                    applied = true;
                }
            }
            if !layers.is_empty() {
                style.mask_image = Some(layers.join(", "));
            }
            applied
        }
        // Same posture as background-image: the author's stack, verbatim.
        "font-family" => {
            style.font_family = Some(v.to_string());
            true
        }
        "background-color" => color::parse_color_with_scheme(v, scheme_dark_of(style))
            .map(|(c, m)| {
                style.background_color = Some(c);
                style.background_color_modern = m;
            })
            .is_some(),
        // `background` shorthand: paren-aware token split, then classify —
        // a *-gradient(...) function feeds background_image (the paint
        // layer parses it there), a color token feeds background_color;
        // repeat/position/attachment tokens are accepted and ignored (v1
        // models no other layer longhand). Like every CSS shorthand it
        // RESETS the sub-longhands first — both modeled ones — so
        // `background: #fff` after a gradient clears the image and
        // `background: none` after `background-color: red` clears the
        // color (css-backgrounds-3 §3).
        "background" => {
            style.background_image = None;
            style.background_color = None;
            style.background_color_modern = None;
            let mut applied = false;
            for tok in split_sides(v) {
                let t = tok.trim();
                if let Some((c, m)) = color::parse_color_with_scheme(t, scheme_dark_of(style)) {
                    style.background_color = Some(c);
                    style.background_color_modern = m;
                    applied = true;
                } else if t.to_ascii_lowercase().contains("gradient(") {
                    style.background_image = Some(t.to_string());
                    applied = true;
                } else if t.eq_ignore_ascii_case("none") || t.starts_with("url(") {
                    // Recognized layer values this slice doesn't model.
                    applied = true;
                }
            }
            applied
        }
        _ => return None,
    })
}

/// `apply_one` family: margins, paddings, borders.
/// `None` = not this family's property; the caller tries the next one.
pub(super) fn apply_box_model(
    style: &mut ComputedStyle,
    name: &str,
    _value: &str,
    v: &str,
    fonts: &FontCtx,
) -> Option<bool> {
    let len = |val: &str| css_len(val, fonts);
    Some(match name {
        "margin" => {
            let sides = expand_sides(v, fonts);
            style.margin = sides;
            true
        }
        "padding" => {
            // `auto` is illegal for padding: drop the declaration entirely
            // (CSS invalid-declaration recovery).
            let sides = expand_sides(v, fonts);
            match sides_no_auto(&sides) {
                Some(clean) => {
                    style.padding = clean;
                    true
                }
                None => false,
            }
        }
        "margin-top" | "margin-right" | "margin-bottom" | "margin-left" => {
            let len = if v.eq_ignore_ascii_case("auto") {
                Some(Length::Auto)
            } else {
                len(v)
            };
            set_side(&mut style.margin, name, len);
            true
        }
        "padding-top" | "padding-right" | "padding-bottom" | "padding-left" => {
            // padding: auto is illegal - drop the declaration.
            if v.eq_ignore_ascii_case("auto") {
                return Some(false);
            }
            set_side(&mut style.padding, name, len(v));
            true
        }
        "border" => {
            // <border-width> || <border-style> || <color>, any order. Parse
            // everything first and only then apply: an unparseable token
            // drops the WHOLE declaration (CSS invalid-declaration recovery),
            // with no partial side effects.
            let mut width: Option<Length> = None;
            let mut line: Option<Option<BorderStyle>> = None; // Some(None) = explicit none
            let mut color: Option<Color> = None;
            let mut color_modern: Option<ModernColor> = None;
            for tok in split_sides(v) {
                let w = len(tok).or(match tok {
                    "thin" => Some(Length::Px(1.0)),
                    "medium" => Some(Length::Px(3.0)),
                    "thick" => Some(Length::Px(5.0)),
                    _ => None,
                });
                if let Some(l) = w {
                    width = Some(l);
                    continue;
                }
                match border_style_kw(tok) {
                    BorderStyleKw::Line(bs) => line = Some(Some(bs)),
                    BorderStyleKw::NoBorder => line = Some(None),
                    BorderStyleKw::NotAStyle => {
                        match color::parse_color_with_scheme(tok, scheme_dark_of(style)) {
                            Some((c, m)) => {
                                color = Some(c);
                                color_modern = m;
                            }
                            None => return Some(false),
                        }
                    }
                }
            }
            if let Some(l) = width {
                style.border_width = Sides {
                    top: Some(l),
                    right: Some(l),
                    bottom: Some(l),
                    left: Some(l),
                };
            }
            match line {
                Some(Some(bs)) => style.border_style = Some(bs),
                Some(None) => style.border_style = None,
                None => {}
            }
            if let Some(c) = color {
                style.border_color = Some(c);
                style.border_color_modern = color_modern;
            }
            width.is_some() || line.is_some() || color.is_some()
        }
        // Per-side shorthands (`border-top: 2px solid red`, …): same
        // triple grammar as `border`, width applied to the named side only.
        // border_style/border_color stay uniform-for-all-sides in this model
        // (documented approximation), so a side shorthand's style/color
        // applies globally.
        "border-top" | "border-right" | "border-bottom" | "border-left" => {
            let mut width: Option<Length> = None;
            let mut line: Option<Option<BorderStyle>> = None;
            let mut color: Option<Color> = None;
            let mut color_modern: Option<ModernColor> = None;
            for tok in split_sides(v) {
                let w = len(tok).or(match tok {
                    "thin" => Some(Length::Px(1.0)),
                    "medium" => Some(Length::Px(3.0)),
                    "thick" => Some(Length::Px(5.0)),
                    _ => None,
                });
                if let Some(l) = w {
                    width = Some(l);
                    continue;
                }
                match border_style_kw(tok) {
                    BorderStyleKw::Line(bs) => line = Some(Some(bs)),
                    BorderStyleKw::NoBorder => line = Some(None),
                    BorderStyleKw::NotAStyle => {
                        match color::parse_color_with_scheme(tok, scheme_dark_of(style)) {
                            Some((c, m)) => {
                                color = Some(c);
                                color_modern = m;
                            }
                            None => return Some(false),
                        }
                    }
                }
            }
            if let Some(l) = width {
                match name {
                    "border-top" => style.border_width.top = Some(l),
                    "border-right" => style.border_width.right = Some(l),
                    "border-bottom" => style.border_width.bottom = Some(l),
                    "border-left" => style.border_width.left = Some(l),
                    _ => unreachable!(),
                }
            }
            match line {
                Some(Some(bs)) => style.border_style = Some(bs),
                Some(None) => style.border_style = None,
                None => {}
            }
            if let Some(c) = color {
                style.border_color = Some(c);
                style.border_color_modern = color_modern;
            }
            width.is_some() || line.is_some() || color.is_some()
        }
        // Per-side width longhands: `border: 1px solid; border-top-width: 0`
        // must leave the other three sides intact (Chrome shows a three-sided
        // outline there — blitz#837's repro shape). The property used to be
        // unknown here, so the side kept the shorthand's width.
        "border-top-width" | "border-right-width" | "border-bottom-width" | "border-left-width" => {
            let l = match v {
                "thin" => Some(Length::Px(1.0)),
                "medium" => Some(Length::Px(3.0)),
                "thick" => Some(Length::Px(5.0)),
                _ => len(v),
            };
            let Some(l) = l else { return Some(false) };
            match name {
                "border-top-width" => style.border_width.top = Some(l),
                "border-right-width" => style.border_width.right = Some(l),
                "border-bottom-width" => style.border_width.bottom = Some(l),
                _ => style.border_width.left = Some(l),
            }
            true
        }
        "border-width" => {
            let sides = expand_sides(v, fonts);
            style.border_width = sides;
            sides.top.is_some()
        }
        "border-style" => {
            // Uniform keyword only (per-side style lists are later batches).
            match border_style_kw(v) {
                BorderStyleKw::Line(bs) => {
                    style.border_style = Some(bs);
                    true
                }
                BorderStyleKw::NoBorder => {
                    style.border_style = None;
                    true
                }
                BorderStyleKw::NotAStyle => false,
            }
        }
        "border-color" => color::parse_color_with_scheme(v, scheme_dark_of(style))
            .map(|(c, m)| {
                style.border_color = Some(c);
                style.border_color_modern = m;
            })
            .is_some(),
        _ => return None,
    })
}

/// `apply_one` family: font-size, width/height, text alignment and spacing.
/// `None` = not this family's property; the caller tries the next one.
pub(super) fn apply_text_sizing(
    style: &mut ComputedStyle,
    name: &str,
    _value: &str,
    v: &str,
    fonts: &FontCtx,
) -> Option<bool> {
    let len = |val: &str| css_len(val, fonts);
    let len_sizing = |val: &str| css_len_sizing(val, fonts);
    Some(match name {
        "font-size" => {
            // Resolved by the cascade's font-size pre-pass (em/% need the
            // PARENT font-size); here only px/keywords can apply directly.
            // em/rem/% handled by the pre-pass, not this arm.
            parse_font_size_len(v)
                .map(|l| {
                    if let CssLength::Px(px) = l {
                        style.font_size = Some(px)
                    }
                })
                .is_some()
        }
        "width" => {
            // `auto` is a LEGAL value (None IS the auto slot) — dropping it
            // would strand an earlier `width: 200px` past an intended reset,
            // the #65 height trap in miniature. Everything else that fails
            // to reduce is a parse-time drop in Chrome: the already-cascaded
            // slot survives untouched (box_offset posture, #334).
            if v.eq_ignore_ascii_case("auto") {
                style.width = None;
                style.width_expr = None;
                return Some(true);
            }
            match (len_sizing(v), calc::pct_math(v, fonts)) {
                (Some(l), _) => {
                    style.width = Some(l);
                    style.width_expr = None;
                    true
                }
                (None, Some((l, e))) => {
                    style.width = Some(l);
                    style.width_expr = Some(e);
                    true
                }
                (None, None) => false,
            }
        }
        // `height: auto` must APPLY as Length::Auto, not drop: `auto` is a
        // legal value, and dropping the declaration leaves an earlier
        // `height: 100%` standing — a print block's `html,body{height:auto}`
        // then never clears the screen value, so the §10.5 fold (#65) has
        // nothing to fold (the computed height stays a viewport percent and
        // the deck stays pinned at one screen tall).
        "height" => {
            let l = if v.eq_ignore_ascii_case("auto") {
                Some(Length::Auto)
            } else {
                len(v)
            };
            l.map(|l| style.height = Some(l)).is_some()
        }
        "font-weight" => {
            let weight = parse_font_weight(v);
            style.font_weight = weight;
            weight.is_some()
        }
        "line-height" => parse_line_height(v)
            .map(|raw| {
                style.line_height = Some(match raw {
                    LineHeightRaw::Normal => LineHeightSpec::Normal,
                    LineHeightRaw::Number(n) => LineHeightSpec::Number(n),
                    LineHeightRaw::Len(CssLength::Px(px)) => LineHeightSpec::Px(px),
                    // em/rem fold against the element's OWN font-size (the
                    // cascade's FontCtx carries it); % behaves like a number.
                    LineHeightRaw::Len(CssLength::Em(n)) => LineHeightSpec::Px(n * fonts.own),
                    LineHeightRaw::Len(CssLength::Rem(n)) => LineHeightSpec::Px(n * fonts.root),
                    LineHeightRaw::Len(CssLength::Percent(p)) => LineHeightSpec::Number(p / 100.0),
                    LineHeightRaw::Len(CssLength::Vw(n)) => {
                        LineHeightSpec::Px(n * fonts.viewport_w / 100.0)
                    }
                    LineHeightRaw::Len(CssLength::Vh(n)) => {
                        LineHeightSpec::Px(n * fonts.viewport_h / 100.0)
                    }
                    LineHeightRaw::Len(CssLength::Vmin(n)) => {
                        LineHeightSpec::Px(n * fonts.viewport_w.min(fonts.viewport_h) / 100.0)
                    }
                    LineHeightRaw::Len(CssLength::Vmax(n)) => {
                        LineHeightSpec::Px(n * fonts.viewport_w.max(fonts.viewport_h) / 100.0)
                    }
                    // ch/ex fold at the same 0.5em fallback as resolve_len.
                    LineHeightRaw::Len(CssLength::Ch(n)) | LineHeightRaw::Len(CssLength::Ex(n)) => {
                        LineHeightSpec::Px(n * 0.5 * fonts.own)
                    }
                })
            })
            .is_some(),
        "text-align" => {
            style.text_align = match v {
                "left" => Some(TextAlign::Left),
                "start" => Some(TextAlign::Start),
                "center" => Some(TextAlign::Center),
                "right" => Some(TextAlign::Right),
                "end" => Some(TextAlign::End),
                // Consumed as start by layout (no stretch machinery, #214) —
                // parsing it keeps the computed face honest and lets
                // text-align-last target justify paragraphs' last line.
                "justify" => Some(TextAlign::Justify),
                _ => return Some(false),
            };
            true
        }
        "text-align-last" => {
            style.text_align_last = match v {
                "auto" => Some(TextAlignLast::Auto),
                "left" => Some(TextAlignLast::Left),
                "start" => Some(TextAlignLast::Start),
                "center" => Some(TextAlignLast::Center),
                "right" => Some(TextAlignLast::Right),
                "end" => Some(TextAlignLast::End),
                "justify" => Some(TextAlignLast::Justify),
                _ => return Some(false),
            };
            true
        }
        "direction" => {
            style.direction = match v {
                "ltr" => Some(TextDirection::Ltr),
                "rtl" => Some(TextDirection::Rtl),
                _ => return Some(false),
            };
            true
        }
        "word-spacing" => {
            // `normal` is the explicit initial; px lengths (negative allowed)
            // apply as-is, em/rem fold against the font context. Percent is
            // relative to the containing block's inline advance (needs
            // geometry this layer lacks) — rejected like other %-lengths.
            if v.eq_ignore_ascii_case("normal") {
                style.word_spacing = None;
                return Some(true);
            }
            match len(v) {
                Some(Length::Px(px)) => {
                    style.word_spacing = Some(px);
                    true
                }
                _ => false,
            }
        }
        "font-variant-caps" => {
            style.font_variant_caps = match v {
                "small-caps" => Some(true),
                "normal" => Some(false),
                _ => return Some(false),
            };
            true
        }
        "text-transform" => {
            style.text_transform = match v {
                "none" => Some(TextTransform::None),
                "uppercase" => Some(TextTransform::Uppercase),
                "lowercase" => Some(TextTransform::Lowercase),
                "capitalize" => Some(TextTransform::Capitalize),
                _ => return Some(false),
            };
            true
        }
        _ => return None,
    })
}
