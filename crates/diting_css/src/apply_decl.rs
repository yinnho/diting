//! Apply a declaration block onto a `ComputedStyle`.
//!
//! Split from the css hub. `apply_one` still dispatches into the visual
//! and layout families. Pure move.

use super::*;

/// The element's used color-scheme, as a dark-branch bool for
/// `light-dark()` (#18). Own tokens decide: a list carrying `dark` without
/// `light` renders dark; anything else declared (light-only, both, normal)
/// renders light — with the persona's prefers-color-scheme=light, "both"
/// resolves light on Chrome-parity machines too. Undeclared walks the
/// ancestor chain via `scheme_dark_hint`. Probed Chrome 145, 2026-11-06.
pub(super) fn scheme_dark_of(style: &ComputedStyle) -> bool {
    match style.color_scheme.as_deref() {
        Some(list) => {
            let has = |kw: &str| list.split_whitespace().any(|t| t == kw);
            has("dark") && !has("light")
        }
        None => style.scheme_dark_hint,
    }
}

/// Apply declarations to a computed style. Returns whether any recognized
/// property was applied (the @supports leaf-probe contract). Uses the
/// default font context — callers that know the element's font-size (the
/// cascade) use `apply_declarations_with` so em/rem resolve correctly.
pub fn apply_declarations(style: &mut ComputedStyle, declarations: &str) -> bool {
    apply_declarations_with(style, declarations, &FontCtx::default())
}

pub fn apply_declarations_with(
    style: &mut ComputedStyle,
    declarations: &str,
    fonts: &FontCtx,
) -> bool {
    apply_declarations_importance(style, declarations, fonts, Importance::Any, None)
}

/// Which declarations an apply pass admits. The cascade runs the normal
/// pass (stylesheet rules in specificity+order sequence, then inline at the
/// top of normal) before the !important pass — author important beats every
/// author-normal declaration, inline style included (CSS 2.1 §6.4.1). That
/// ordering is what lets a stylesheet's `@media print { transform: none
/// !important }` override the inline `translateX(...)` a carousel script
/// parks on each slide.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Importance {
    /// Only declarations WITHOUT `!important`.
    Normal,
    /// Only declarations WITH `!important` (suffix stripped).
    Important,
    /// Everything — the single-pass posture for standalone callers
    /// (@supports probes, tests) where importance never reorders anything.
    Any,
}

/// Multiply every geometry px in a computed style by `z` — the #120 zoom
/// pass. `Length::Px` (and the px half of `Calc`) only: percentages resolve
/// against the already-scaled parent box at layout, `auto`/keywords carry no
/// magnitude, and font keywords were folded to px by the cascade. Font size
/// scales (text measurement and em-derived children read it after), the px
/// form of line-height scales while number multipliers stay (they multiply
/// the now-scaled font size), and shadow offsets/blur/spread scale. The
/// `zoom`/`zoom_accum` fields themselves are never touched here.
pub fn scale_computed_px(c: &mut ComputedStyle, z: f32) {
    fn scale_len(l: &mut Length, z: f32) {
        match l {
            Length::Px(px) => *px *= z,
            Length::Calc { px, .. } => *px *= z,
            _ => {}
        }
    }
    fn scale_sides(s: &mut Sides, z: f32) {
        for l in [&mut s.top, &mut s.right, &mut s.bottom, &mut s.left]
            .into_iter()
            .flatten()
        {
            scale_len(l, z);
        }
    }
    scale_sides(&mut c.margin, z);
    scale_sides(&mut c.padding, z);
    scale_sides(&mut c.border_width, z);
    for l in [
        &mut c.width,
        &mut c.height,
        &mut c.min_width,
        &mut c.max_width,
        &mut c.min_height,
        &mut c.max_height,
        &mut c.top,
        &mut c.right,
        &mut c.bottom,
        &mut c.left,
        &mut c.flex_basis,
        &mut c.column_gap,
        &mut c.row_gap,
        &mut c.border_radius,
    ]
    .into_iter()
    .flatten()
    {
        scale_len(l, z);
    }
    if let Some(radii) = c.corner_radii.as_mut() {
        for (h, v) in radii.iter_mut() {
            scale_len(h, z);
            scale_len(v, z);
        }
    }
    if let Some(fs) = c.font_size.as_mut() {
        *fs *= z;
    }
    if let Some(LineHeightSpec::Px(px)) = c.line_height.as_mut() {
        *px *= z;
    }
    if let Some(ws) = c.word_spacing.as_mut() {
        *ws *= z;
    }
    if let Some(layers) = c.box_shadow.as_mut() {
        for s in layers.iter_mut() {
            s.dx *= z;
            s.dy *= z;
            s.blur *= z;
            s.spread *= z;
        }
    }
    if let Some(layers) = c.text_shadow.as_mut() {
        for s in layers.iter_mut() {
            s.dx *= z;
            s.dy *= z;
            s.blur *= z;
        }
    }
}

/// The longhand table the css-wide keywords (#218, takumi #1795 lineage)
/// copy at field level: `initial` = un-declare (a fresh default holds None
/// everywhere, and the CSSOM + layout consumers already serve Chrome's
/// initial values for None fields), `inherit` = the parent's field, `unset`
/// = by the inherited bit. Deliberately OUT of the table: `display` (the
/// cascade seeds the UA per-tag value and `display_from_ua` tracks it),
/// `zoom` (the declared/accum twins must stay consistent), `content`/
/// counters/grid placement (generated-content plumbing), `custom` (keywords
/// on `--*` are handled in the custom-property branch), and the pseudos box
/// (a pseudo pair must never copy down from the parent).
macro_rules! css_wide_longhands {
    ($( $name:literal => $field:ident $(.$seg:ident)*, $inh:literal ; )*) => {
        pub(super) fn css_wide_copy_field(
            style: &mut ComputedStyle,
            src: &ComputedStyle,
            name: &str,
        ) -> bool {
            let hit = match name {
                $( $name => {
                    style.$field$(.$seg)* = src.$field$(.$seg)*.clone();
                    true
                } )*
                _ => false,
            };
            // #17: the authored-space twins ride their primaries through
            // css-wide keywords (color:inherit picks up the parent's oklch
            // spelling, initial clears it).
            if hit {
                match name {
                    "color" => style.color_modern = src.color_modern,
                    "background-color" => style.background_color_modern = src.background_color_modern,
                    "border-color" => style.border_color_modern = src.border_color_modern,
                    _ => {}
                }
            }
            hit
        }

        fn css_wide_is_inherited(name: &str) -> bool {
            match name {
                $( $name => $inh, )*
                _ => false,
            }
        }

        /// Every covered longhand, for `all: initial/inherit/unset`.
        const CSS_WIDE_ALL: &[&str] = &[ $( $name ),* ];
    }
}

css_wide_longhands! {
    // --- inherited (unset behaves like inherit) ---
    "color" => color, true;
    "font-size" => font_size, true;
    "font-weight" => font_weight, true;
    "font-family" => font_family, true;
    "line-height" => line_height, true;
    "word-spacing" => word_spacing, true;
    "white-space" => white_space, true;
    "font-variant-caps" => font_variant_caps, true;
    "text-transform" => text_transform, true;
    "text-align" => text_align, true;
    "text-align-last" => text_align_last, true;
    "direction" => direction, true;
    "list-style-type" => list_style_type, true;
    "quotes" => quotes, true;
    "text-shadow" => text_shadow, true;
    // --- non-inherited (unset behaves like initial) ---
    "color-scheme" => color_scheme, false;
    "display" => display, false;
    "background-color" => background_color, false;
    "background-image" => background_image, false;
    "mask-image" => mask_image, false;
    "clip-path" => clip_path, false;
    "background-clip" => background_clip_text, false;
    "box-decoration-break" => box_decoration_clone, false;
    "box-shadow" => box_shadow, false;
    "backdrop-filter" => backdrop_blur, false;
    "filter" => filter, false;
    "perspective" => perspective, false;
    "mix-blend-mode" => mix_blend_mode, false;
    "opacity" => opacity, false;
    "transform" => transform, false;
    "translate" => translate_prop, false;
    "rotate" => rotate_prop, false;
    "scale" => scale_prop, false;
    "transform-origin" => transform_origin, false;
    // Logical box longhands (#244): same slots as their physical twins.
    "margin-block-start" => margin.top, false;
    "margin-block-end" => margin.bottom, false;
    "margin-inline-start" => margin.left, false;
    "margin-inline-end" => margin.right, false;
    "padding-block-start" => padding.top, false;
    "padding-block-end" => padding.bottom, false;
    "padding-inline-start" => padding.left, false;
    "padding-inline-end" => padding.right, false;
    "animation" => animation, false;
    "transition" => transition, false;
    "border-radius" => border_radius, false;
    "float" => float_side, false;
    "clear" => clear_side, false;
    "border-collapse" => border_collapse, false;
    "table-layout" => table_layout, false;
    "vertical-align" => vertical_align, false;
    "text-decoration-line" => text_decoration_line, false;
    "text-overflow" => text_overflow, false;
    "line-clamp" => line_clamp, false;
    "-webkit-line-clamp" => line_clamp, false;
    "position" => position, false;
    "top" => top, false;
    "right" => right, false;
    "bottom" => bottom, false;
    "left" => left, false;
    "z-index" => z_index, false;
    "box-sizing" => box_sizing, false;
    "width" => width, false;
    "height" => height, false;
    "min-width" => min_width, false;
    "max-width" => max_width, false;
    "min-height" => min_height, false;
    "max-height" => max_height, false;
    "aspect-ratio" => aspect_ratio, false;
    "object-fit" => object_fit, false;
    "object-position" => object_position, false;
    "overflow-x" => overflow_x, false;
    "overflow-y" => overflow_y, false;
    "flex-direction" => flex_direction, false;
    "flex-wrap" => flex_wrap, false;
    "justify-content" => justify_content, false;
    "align-items" => align_items, false;
    "align-content" => align_content, false;
    "align-self" => align_self, false;
    "justify-self" => justify_self, false;
    "justify-items" => justify_items, false;
    "flex-grow" => flex_grow, false;
    "flex-shrink" => flex_shrink, false;
    "flex-basis" => flex_basis, false;
    "column-gap" => column_gap, false;
    "row-gap" => row_gap, false;
    "margin-top" => margin.top, false;
    "margin-right" => margin.right, false;
    "margin-bottom" => margin.bottom, false;
    "margin-left" => margin.left, false;
    "padding-top" => padding.top, false;
    "padding-right" => padding.right, false;
    "padding-bottom" => padding.bottom, false;
    "padding-left" => padding.left, false;
    "border-top-width" => border_width.top, false;
    "border-right-width" => border_width.right, false;
    "border-bottom-width" => border_width.bottom, false;
    "border-left-width" => border_width.left, false;
    "border-style" => border_style, false;
    "border-color" => border_color, false;
}

/// Expand a property name to the longhands a css-wide keyword covers.
/// `None` = not a shorthand in the table: a bare longhand (or anything
/// unknown, so `@supports` stays honest) covers itself.
fn css_wide_expand(name: &str) -> Option<&'static [&'static str]> {
    const PADDING: &[&str] = &[
        "padding-top",
        "padding-right",
        "padding-bottom",
        "padding-left",
    ];
    const MARGIN: &[&str] = &["margin-top", "margin-right", "margin-bottom", "margin-left"];
    const BORDER_WIDTH: &[&str] = &[
        "border-top-width",
        "border-right-width",
        "border-bottom-width",
        "border-left-width",
    ];
    Some(match name {
        "all" => CSS_WIDE_ALL,
        "padding" => PADDING,
        "margin" => MARGIN,
        "border-width" => BORDER_WIDTH,
        "border" => &[
            "border-top-width",
            "border-right-width",
            "border-bottom-width",
            "border-left-width",
            "border-style",
            "border-color",
        ],
        "background" => &["background-color", "background-image"],
        "font" => &["font-size", "font-weight", "font-family", "line-height"],
        "flex" => &["flex-grow", "flex-shrink", "flex-basis"],
        "overflow" => &["overflow-x", "overflow-y"],
        "text-decoration" => &["text-decoration-line"],
        "list-style" => &["list-style-type"],
        // Logical shorthands (#244) expand to their logical longhands (which
        // css_wide_longhands maps onto the physical slots).
        "margin-block" => &["margin-block-start", "margin-block-end"],
        "margin-inline" => &["margin-inline-start", "margin-inline-end"],
        "padding-block" => &["padding-block-start", "padding-block-end"],
        "padding-inline" => &["padding-inline-start", "padding-inline-end"],
        _ => return None,
    })
}

/// Apply one css-wide keyword declaration (`initial`/`inherit`/`unset`) by
/// copying fields: inherited posture copies the parent's field (rootless
/// elements and standalone callers fall back to the initial default, which
/// is what Chrome does for `inherit` on the root), otherwise the fresh
/// default. Returns false when no covered longhand matched, so unknown
/// properties keep reporting unapplied.
fn apply_css_wide_keyword(
    style: &mut ComputedStyle,
    name: &str,
    keyword: &str,
    parent: Option<&ComputedStyle>,
) -> bool {
    let initial_src = ComputedStyle::default();
    let mut applied = false;
    let single = [name];
    let expanded: &[&str] = css_wide_expand(name).unwrap_or(&single);
    for long in expanded {
        let inherits = keyword == "inherit" || (keyword == "unset" && css_wide_is_inherited(long));
        let src = if inherits {
            parent.unwrap_or(&initial_src)
        } else {
            &initial_src
        };
        applied |= css_wide_copy_field(style, src, long);
    }
    applied
}

pub(crate) fn apply_declarations_importance(
    style: &mut ComputedStyle,
    declarations: &str,
    fonts: &FontCtx,
    imp: Importance,
    parent: Option<&ComputedStyle>,
) -> bool {
    let mut applied = false;
    for (name, value) in split_declarations(declarations) {
        // Strip a trailing !important before anything sees the value: the
        // flag reorders the cascade passes, and the suffix would otherwise
        // poison every parser (`none !important` != `none`).
        let (value, important) = strip_important(&value);
        match imp {
            Importance::Normal if important => continue,
            Importance::Important if !important => continue,
            _ => {}
        }
        if name.starts_with("--") {
            // Custom property: store the token stream raw. Values may hold
            // anything (including semicolon-free junk). Empty value =
            // guaranteed-invalid → unset. The map is Arc-shared down the
            // inheritance chain (custom props inherit computed wholesale);
            // make_mut clones only when THIS element re-declares (#109: a
            // 500-var design system made the per-element inheritance clone
            // the dominant cost of a whole-document style pass). The
            // css-wide keywords apply here too (#218): initial empties the
            // slot; inherit/unset copy the parent's (custom props are
            // inherited by default, so unset = inherit), or empty when the
            // parent has nothing to give.
            let v = value.trim();
            if !props::value_acceptable(&name, v) {
                continue;
            }
            let kw = v.to_ascii_lowercase();
            if kw == "initial" {
                std::sync::Arc::make_mut(&mut style.custom).remove(&name);
            } else if kw == "inherit" || kw == "unset" {
                let inherited = parent.and_then(|p| p.custom.get(&name)).cloned();
                match inherited {
                    Some(pv) => {
                        std::sync::Arc::make_mut(&mut style.custom).insert(name.clone(), pv);
                    }
                    None => {
                        std::sync::Arc::make_mut(&mut style.custom).remove(&name);
                    }
                }
            } else if v.is_empty() {
                std::sync::Arc::make_mut(&mut style.custom).remove(&name);
            } else {
                std::sync::Arc::make_mut(&mut style.custom).insert(name.clone(), v.to_string());
            }
            applied = true;
            continue;
        }
        // Normal property: substitute var() references against the custom
        // map (inherited + already-applied declarations). Unresolved without
        // fallback ⇒ treat the declaration as invalid-at-computed-value-time
        // and drop it — inherited/earlier values survive, which matches the
        // common case of IACVT on non-inherited properties closely enough.
        let value = if value.to_ascii_lowercase().contains("var(") {
            match substitute_vars(&value, &style.custom, 0) {
                Some(v) => v,
                None => continue,
            }
        } else {
            value.to_string()
        };
        // CSS-wide keywords (#218): initial/inherit/unset resolve by field
        // copy before apply_one ever sees the value — the per-property
        // parsers below would otherwise drop the declaration and the
        // inherited/default value would survive (the accidental-correctness
        // hole: dropped declarations match keyword semantics only where the
        // property is naturally inherited). Checked after var()
        // substitution so a fallback can substitute a keyword token too.
        let kw = value.trim().to_ascii_lowercase();
        if matches!(kw.as_str(), "initial" | "inherit" | "unset")
            && apply_css_wide_keyword(style, &name, &kw, parent)
        {
            applied = true;
            continue;
        }
        if apply_one(style, &name, &value, fonts) {
            applied = true;
        }
    }
    applied
}

/// Split a trailing `!important` off a declaration value (case-insensitive,
/// whitespace-tolerant). Returns the stripped value plus whether the flag
/// was present.
fn strip_important(value: &str) -> (String, bool) {
    let v = value.trim_end();
    if v.to_ascii_lowercase().ends_with("!important") {
        (
            v[..v.len() - "!important".len()].trim_end().to_string(),
            true,
        )
    } else {
        (value.to_string(), false)
    }
}

pub(super) fn apply_one(
    style: &mut ComputedStyle,
    name: &str,
    value: &str,
    fonts: &FontCtx,
) -> bool {
    let v = value.trim();
    apply_display(style, name, value, v, fonts)
        .or_else(|| apply_paint_text(style, name, value, v, fonts))
        .or_else(|| apply_font_image(style, name, value, v, fonts))
        .or_else(|| apply_box_model(style, name, value, v, fonts))
        .or_else(|| apply_text_sizing(style, name, value, v, fonts))
        .or_else(|| apply_effects(style, name, value, v, fonts))
        .or_else(|| apply_logical_motion(style, name, value, v, fonts))
        .or_else(|| apply_flex_grid(style, name, value, v, fonts))
        .or_else(|| apply_position(style, name, value, v, fonts))
        .unwrap_or(false)
}
