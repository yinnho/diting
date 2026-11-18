//! diting_css — the cascade: stylesheet parsing (rule splitting + at-rule
//! dispatch incl. @media/@supports/@layer/@container/@keyframes/@property),
//! selector matching against diting_dom, and the computed-style model with
//! inheritance that diting_layout and the CSSOM ops read. Started as a port
//! of upstream obscura-render's CSS layer (render-claim batch 1).

mod color;
mod color_modern;
pub use color::{parse_color, parse_linear_gradient, LinearGradient};
pub use color_modern::computed_image_text;
pub use color_modern::ModernColor;
mod content;
pub use content::{
    format_counter_value, parse_content_value, parse_counter_modifiers, parse_quotes, ContentValue,
    CounterStyle, PseudoPair,
};
mod animation;
#[cfg(test)]
use animation::parse_animation_shorthand;
pub use diting_dom::split_top_level_commas;
mod calc;
mod layers;
mod nesting;
pub mod props;
mod scope;
mod supports;
mod vars;
pub use calc::{eval_calc, eval_math_call, eval_width_expr};
mod anim_sample;
mod apply_decl;
mod apply_layout;
mod apply_visual;
mod cascade;
mod computed;
mod font_shorthand;
mod geom;
mod len_parse;
mod media;
mod shadow_parse;
mod sheet;
mod style_kw;
mod supports_cond;
mod text_align;
mod transform;
mod transition;
mod ua;
pub use anim_sample::sample_css_animation;
pub use animation::{KeyframeStop, Keyframes, KeyframesMap};
pub use apply_decl::{apply_declarations, apply_declarations_with, scale_computed_px};
#[cfg(test)]
use apply_decl::{apply_one, css_wide_copy_field};
pub use cascade::{apply_inline_declarations, cascade_element, rule_layer_ranks};
pub use computed::ComputedStyle;
#[cfg(test)]
use font_shorthand::parse_font_shorthand;
pub use geom::{Color, CssLength, FontCtx, Length, LineHeightSpec, Sides, SvgPaint, Transform2D};
pub use geom::{DEFAULT_ROOT_FONT_SIZE, DEFAULT_VIEWPORT};
pub use media::{media_query_applies, media_query_applies_with, CssMediaType, MediaOverrides};
pub use shadow_parse::{
    parse_backdrop_blur, parse_box_shadow, parse_mix_blend_mode, parse_text_shadow, BoxShadow,
    MixBlendMode, TextShadow,
};
#[cfg(test)]
use sheet::parse_container_condition;
pub use sheet::{
    collect_selector_attr_names, parse_stylesheet, parse_stylesheet_for, parse_stylesheet_full,
    parse_stylesheet_timed, parse_stylesheet_timed_with, ContainerAxis, ContainerCmp,
    ContainerCondition, ContainerFeature, ContainerRule, ContainerType, ParsedRule,
};
pub use style_kw::{
    deco_style_css, deco_thick_css, overflow_name, AlignContentMode, AlignMode, BorderCollapse,
    BorderStyle, BoxSizing, ClearSide, DecoStyle, DecoThick, Display, FlexDirection, FlexWrapMode,
    FloatSide, GridTrack, JustifyMode, ListStyleType, ObjectFit, ObjectPositionPart, Overflow,
    PositionMode, TableLayout, TextDecorations, TextOverflow, TrackSize, VerticalAlign,
    WebkitBoxKind, WhiteSpace,
};
pub use supports::supports_declaration;
pub use supports_cond::supports_condition_applies;
pub use text_align::{TextAlign, TextAlignLast, TextDirection, TextTransform};
pub use transform::{parse_transform, parse_transform_with_vp};
pub use transition::{
    parse_easing_token, sample_css_transitions, AnimationSpec, CssTransition, Easing,
    TransitionSpec, TransitionValue,
};
pub use ua::{
    ua_border, ua_display, ua_font_family, ua_font_size, ua_font_weight, ua_list_style, ua_margin,
    ua_padding, ua_text_align, ua_text_decoration, ua_white_space, wants_monospace,
};

/// Split a declaration block into (name, value) pairs. Quote- and paren-aware;
/// nested `{}` blocks become one dropped chunk rather than leaking into the
/// parent rule (upstream split_declarations semantics).
pub fn split_declarations(css: &str) -> Vec<(String, String)> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut in_quote: Option<char> = None;
    let mut start = 0;
    for (i, c) in css.char_indices() {
        if let Some(q) = in_quote {
            if c == q {
                in_quote = None;
            }
            continue;
        }
        match c {
            '\'' | '"' => in_quote = Some(c),
            '(' | '{' => depth += 1,
            ')' | '}' => depth = (depth - 1).max(0),
            ';' if depth == 0 => {
                parts.push(&css[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&css[start..]);

    parts
        .into_iter()
        .filter_map(|decl| {
            let (name, value) = decl.split_once(':')?;
            let name = name.trim();
            if name.starts_with('!') || name.is_empty() {
                return None;
            }
            // Nested-rule chunks (`&:hover { ... }`) split into a bogus
            // (selector, body) pair whose value contains `{` — drop them
            // rather than treating the selector as a property name.
            if value.contains('{') {
                return None;
            }
            // Custom property names are case-SENSITIVE (--Main ≠ --main);
            // everything else lowercases.
            let name = if name.starts_with("--") {
                name.to_string()
            } else {
                name.to_ascii_lowercase()
            };
            Some((name, value.trim().to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests;
