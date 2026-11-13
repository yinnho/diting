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
use animation::{parse_animation_shorthand, parse_keyframes_body};
pub use diting_dom::split_top_level_commas;
mod calc;
mod layers;
mod nesting;
pub mod props;
mod scope;
mod supports;
mod vars;
pub use calc::{eval_calc, eval_math_call, eval_width_expr};
use vars::substitute_vars;
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
use anim_sample::lerp_transform;
pub use anim_sample::sample_css_animation;
pub use apply_decl::{apply_declarations, apply_declarations_with, scale_computed_px};
use apply_decl::{apply_declarations_importance, scheme_dark_of, Importance};
#[cfg(test)]
use apply_decl::{apply_one, css_wide_copy_field};
use apply_layout::{apply_effects, apply_flex_grid, apply_logical_motion, apply_position};
use apply_visual::{
    apply_box_model, apply_display, apply_font_image, apply_paint_text, apply_text_sizing,
};
pub use cascade::{apply_inline_declarations, cascade_element, rule_layer_ranks};
pub use computed::ComputedStyle;
use font_shorthand::{parse_font_shorthand, parse_font_weight};
use geom::{parse_css_length, parse_line_height, resolve_len, LineHeightRaw};
pub use geom::{Color, CssLength, FontCtx, Length, LineHeightSpec, Sides, SvgPaint, Transform2D};
pub use geom::{DEFAULT_ROOT_FONT_SIZE, DEFAULT_VIEWPORT};
use len_parse::{
    css_box_offset, css_len, css_len_sizing, expand_sides, font_size_px, parse_aspect_ratio,
    parse_font_size_len, parse_grid_template_areas, parse_grid_tracks, parse_num_f32, parse_px_f32,
    set_side, sides_no_auto, split_sides, CssF32,
};
use media::parse_px;
pub use media::{media_query_applies, media_query_applies_with};
pub use shadow_parse::{
    parse_backdrop_blur, parse_box_shadow, parse_mix_blend_mode, parse_text_shadow,
};
#[cfg(test)]
use sheet::parse_container_condition;
pub use sheet::{
    parse_stylesheet, parse_stylesheet_for, parse_stylesheet_full, parse_stylesheet_timed,
    parse_stylesheet_timed_with, ContainerAxis, ContainerCmp, ContainerCondition, ContainerFeature,
    ContainerRule, ContainerType,
};
use style_kw::{border_style_kw, parse_text_decoration, BorderStyleKw};
pub use style_kw::{
    overflow_name, AlignContentMode, AlignMode, BorderCollapse, BorderStyle, BoxSizing, ClearSide,
    Display, FlexDirection, FlexWrapMode, FloatSide, GridTrack, JustifyMode, ListStyleType,
    ObjectFit, ObjectPositionPart, Overflow, PositionMode, TableLayout, TextDecorations,
    TextOverflow, TrackSize, VerticalAlign, WebkitBoxKind, WhiteSpace,
};
pub use supports::supports_declaration;
pub use supports_cond::supports_condition_applies;
pub use text_align::{TextAlign, TextAlignLast, TextDirection, TextTransform};
use transform::parse_transform_origin;
pub use transform::{parse_transform, parse_transform_with_vp};
use transition::{
    add_len, mul_len, paren_aware_tokens, parse_angle_deg, parse_rotate_prop, parse_scale_prop,
    parse_transition_shorthand, parse_translate_prop,
};
pub use transition::{
    parse_easing_token, sample_css_transitions, AnimationSpec, CssTransition, Easing,
    TransitionSpec, TransitionValue,
};
use ua::parse_list_style_type;
pub use ua::{
    ua_border, ua_display, ua_font_family, ua_font_size, ua_font_weight, ua_list_style, ua_margin,
    ua_padding, ua_text_align, ua_text_decoration, ua_white_space, wants_monospace,
};

/// Which media a stylesheet evaluation targets. `print` rules only apply
/// inside `@media print` bodies; `screen` rules apply outside it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CssMediaType {
    #[default]
    Screen,
    Print,
}

/// Emulated media features pushed via CDP `Emulation.setEmulatedMedia`
/// (Playwright's `page.emulateMedia`): (name, value) pairs such as
/// `("prefers-reduced-motion", "reduce")`. A feature present here answers
/// every query for that name from its emulated value; the rest keep the
/// persona defaults `media_pref_default` holds — the same table the JS
/// `matchMedia` publishes, so the cascade and the scripts agree (#29).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MediaOverrides {
    pub features: Vec<(String, String)>,
}

/// The un-emulated value a preference feature holds — mirrors the
/// `_MQ_PERSONA_BOOL` desktop-light persona in the bootstrap. Features not
/// listed have no default and stay false in both faces until emulated.
fn media_pref_default(name: &str) -> Option<&'static str> {
    match name {
        "prefers-color-scheme" => Some("light"),
        "prefers-reduced-motion" => Some("no-preference"),
        "prefers-reduced-transparency" => Some("no-preference"),
        _ => None,
    }
}

/// One flattened stylesheet rule: selector text plus its declaration block.
/// Selectors are compiled lazily by the cascade via diting_dom.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct ParsedRule {
    pub selector: String,
    pub declarations: String,
    /// `@layer` the rule was written in (0 = unlayered); see `layers`.
    pub layer: u32,
}

/// Attribute names referenced by attribute selectors in a rule pool
/// (`[tabindex]`, `[data-x="1"]`, `[lang|=en]`), lowercased. Feeds the
/// layout-cache invalidation decision for attribute writes: a write to a
/// layout-inert attribute is only safe to skip when no rule selects on it
/// (obscura#983).
pub fn collect_selector_attr_names(rules: &[ParsedRule]) -> std::collections::HashSet<String> {
    let mut out = std::collections::HashSet::new();
    for rule in rules {
        let s = rule.selector.as_bytes();
        let mut i = 0;
        while i < s.len() {
            let Some(open) = s[i..].iter().position(|&b| b == b'[').map(|p| p + i) else {
                break;
            };
            let mut j = open + 1;
            while j < s.len() && (s[j].is_ascii_alphanumeric() || s[j] == b'-' || s[j] == b'_') {
                j += 1;
            }
            // A `|` right after the ident followed by another ident is a
            // namespace prefix, not the attribute name — skip conservatively.
            // `|=` is the dash-match operator and keeps the name.
            let namespaced = s.get(j) == Some(&b'|') && s.get(j + 1) != Some(&b'=');
            if j > open + 1 && !namespaced {
                out.insert(String::from_utf8_lossy(&s[open + 1..j]).to_ascii_lowercase());
            }
            i = j.max(open + 1);
        }
    }
    out
}

/// One `@keyframes` stop (`from` = 0, `to` = 1, `NN%` = NN/100) with raw
/// declarations. Values stay raw strings because `var()` in stop values
/// (e.g. `to { opacity: var(--o, 1) }`) substitutes against the custom
/// properties of whichever element declares `animation:` — resolved per
/// element at sample time, not at parse time.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyframeStop {
    pub offset: f32,
    pub decls: Vec<(String, String)>,
}

/// All stops of one `@keyframes` name, sorted by offset ascending.
#[derive(Debug, Clone, PartialEq)]
pub struct Keyframes {
    pub stops: Vec<KeyframeStop>,
}

pub type KeyframesMap = std::collections::HashMap<String, Keyframes>;

// ---------------------------------------------------------------------------
// Computed style: minimal property subset + inheritance
// ---------------------------------------------------------------------------

/// One parsed `box-shadow` layer (blitz#349 family, v1): offsets/blur/
/// spread in px (em/rem folded at cascade time), color resolved at parse
/// time (`currentColor` folds against the color value cascaded so far).
/// `inset` parses and reports through the CSSOM but the paint half is
/// outer-only for v1 — a documented divergence, not a parse failure.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxShadow {
    pub dx: f32,
    pub dy: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: Color,
    pub inset: bool,
}

/// One `text-shadow` layer (blitz#271 family): offset + optional blur
/// around a color. No spread, no `inset` — both are parse errors here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextShadow {
    pub dx: f32,
    pub dy: f32,
    pub blur: f32,
    pub color: Color,
}

/// `mix-blend-mode` (#222): the mode an element's whole rendering composites
/// onto its backdrop with. The separable modes blend per channel; the last
/// four operate on the color's hue/saturation/luminosity triple instead.
/// `plus-lighter` is deliberately absent — it needs a second source plane
/// the compositing model here doesn't have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MixBlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

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
