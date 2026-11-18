//! Layout and text keyword types: display, flex, grid, overflow, borders.
//!
//! The public names stay re-exported from the crate root. This module
//! names the color and length probes a shorthand token is tested against.

use crate::color::parse_color;
use crate::geom::{parse_css_length, resolve_len, FontCtx, Length};
use crate::len_parse::{css_len, split_sides};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionMode {
    Static,
    Relative,
    Absolute,
    Fixed,
    /// In-flow like relative, but the insets are STICK THRESHOLDS against
    /// the scrollport, not offsets: layout places the box in flow (taffy
    /// Relative, insets forced AUTO at the taffy boundary), and readers
    /// paint/measure the box shifted by a scroll-dependent amount clamped
    /// to the containing block (CSS Position 3; root-scroller v1).
    Sticky,
}

/// `box-sizing`: which box edge an authored width/height/min/max measures to.
/// CSS's initial is ContentBox; the near-universal `*, *::before, *::after
/// { box-sizing: border-box }` reset is why real pages declare BorderBox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxSizing {
    ContentBox,
    BorderBox,
}

/// `float` keyword (batch 8a). `None` (the initial value) stays None on
/// [`ComputedStyle`] — only floated boxes carry a side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatSide {
    Left,
    Right,
}

/// `clear` keyword: which side(s)' floats an element must move below.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearSide {
    Left,
    Right,
    Both,
    /// Logical keywords (CSS Logical Properties): inline-start/end resolve to
    /// left/right in this engine's LTR-only writing mode.
    InlineStart,
    InlineEnd,
}

/// Border line style. Only `Solid` paints faithfully in this slice; the
/// patterned styles occupy the same layout space but paint as solid
/// (documented approximation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderStyle {
    Solid,
    Dashed,
    Dotted,
    Double,
}

/// `border-collapse` (table layout). The layout engine maps Collapse to
/// zero cell gaps and Separate to the 2px UA `border-spacing`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderCollapse {
    Collapse,
    Separate,
}

/// `table-layout` (table layout). Auto is the initial content-measured
/// algorithm; Fixed pins columns from authored widths only — a column never
/// grows past its authored width for content, and the remaining width
/// splits equally over the auto columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableLayout {
    Auto,
    Fixed,
}

/// `list-style-type` (#206 v1). Every numbering style beyond the geometric
/// trio collapses onto Decimal — the marker exists, the numbering is right;
/// roman/alpha glyph work is a later batch. `None` is the author's
/// `list-style: none` reset (and its own value).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListStyleType {
    None,
    Disc,
    Circle,
    Square,
    Decimal,
}

impl ListStyleType {
    /// The marker's text run. Outside markers paint as text in the item's
    /// own typography; Chrome draws the geometric trio from the UA font,
    /// whose glyph shapes these codepoints mirror.
    pub fn marker_text(self, index: usize) -> String {
        match self {
            ListStyleType::None => String::new(),
            ListStyleType::Disc => "\u{2022}".into(),
            ListStyleType::Circle => "\u{25E6}".into(),
            ListStyleType::Square => "\u{25AA}".into(),
            ListStyleType::Decimal => format!("{index}."),
        }
    }
}

/// `vertical-align` modeled subset. On table cells (blitz#508)
/// top/middle/bottom move the cell's content within a taller cell; baseline
/// behaves like top for the flex-column cell model. On inline content
/// sub/super are baseline shifts the layout side resolves per text leaf.
/// Lengths resolve to px at declaration time (em against the element's own
/// font-size), percentages keep their % — the spec raises/lowers by a
/// percentage of the element's own line-height, only known at layout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VerticalAlign {
    Top,
    Middle,
    Bottom,
    Baseline,
    Sub,
    Super,
    /// Authored length in px, up-positive (CSS: positive raises the box).
    Length(f32),
    /// Authored percentage, up-positive (CSS: % of the element's line-height).
    Percent(f32),
}

/// `text-decoration-line` keyword set (CSS Text Decoration 3): which lines
/// paint. The property PROPAGATES to inline descendants rather than
/// inheriting (CSS 2.1 §16.3.1) — the layout side resolves each text leaf's
/// used value by walking its ancestor chain and unioning these sets, so the
/// cascade here only records what the element itself declared. Color, style,
/// and thickness live on `ComputedStyle`; lines still paint solid in the
/// text's own color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextDecorations {
    pub underline: bool,
    pub overline: bool,
    pub line_through: bool,
    /// #218 (takumi #1802): the DECLARING element itself carries
    /// `background-clip: text` — Chrome then strokes the lines with the
    /// gradient (an explicit `text-decoration-color` loses too, probe
    /// 2026-10-05) instead of currentcolor, which under clip:text is
    /// usually `transparent` and would hide the stroke entirely.
    pub pierce: bool,
}

impl TextDecorations {
    pub fn is_empty(&self) -> bool {
        !self.underline && !self.overline && !self.line_through
    }
    pub fn union(self, other: Self) -> Self {
        Self {
            underline: self.underline || other.underline,
            overline: self.overline || other.overline,
            line_through: self.line_through || other.line_through,
            pierce: self.pierce || other.pierce,
        }
    }
}

pub fn deco_line_css(line: Option<TextDecorations>) -> String {
    match line {
        Some(d) if !d.is_empty() => [
            if d.underline { "underline" } else { "" },
            if d.overline { "overline" } else { "" },
            if d.line_through { "line-through" } else { "" },
        ]
        .into_iter()
        .filter(|k| !k.is_empty())
        .collect::<Vec<_>>()
        .join(" "),
        _ => "none".to_string(),
    }
}

pub fn deco_style_css(style: Option<DecoStyle>) -> &'static str {
    match style {
        Some(DecoStyle::Double) => "double",
        Some(DecoStyle::Dotted) => "dotted",
        Some(DecoStyle::Dashed) => "dashed",
        Some(DecoStyle::Wavy) => "wavy",
        None => "solid",
    }
}

pub fn deco_thick_css(thick: Option<DecoThick>) -> String {
    match thick {
        Some(DecoThick::FromFont) => "from-font".into(),
        Some(DecoThick::Len(Length::Px(v))) => {
            if (v - v.round()).abs() < f32::EPSILON {
                format!("{}px", v as i64)
            } else {
                let s = format!("{v}");
                format!("{}px", s.trim_end_matches('0').trim_end_matches('.'))
            }
        }
        Some(DecoThick::Len(Length::Percent(p))) => format!("{p}%"),
        _ => "auto".into(),
    }
}

/// `outline-style`. `None` on the computed slot is the initial `none`.
/// `hidden` computes to that same keyword.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutlineStyle {
    Solid,
    Dashed,
    Dotted,
    Double,
    Groove,
    Ridge,
    Inset,
    Outset,
    Auto,
}

pub(crate) enum OutlineStyleParse {
    /// `none` or `hidden`.
    NoneKw,
    Set(OutlineStyle),
}

pub(crate) enum OutlineWidthParse {
    /// Keyword `medium`, the initial width.
    Medium,
    Px(f32),
}

pub(crate) fn parse_outline_style_kw(t: &str) -> Option<OutlineStyleParse> {
    Some(match t.trim().to_ascii_lowercase().as_str() {
        "none" | "hidden" => OutlineStyleParse::NoneKw,
        "solid" => OutlineStyleParse::Set(OutlineStyle::Solid),
        "dashed" => OutlineStyleParse::Set(OutlineStyle::Dashed),
        "dotted" => OutlineStyleParse::Set(OutlineStyle::Dotted),
        "double" => OutlineStyleParse::Set(OutlineStyle::Double),
        "groove" => OutlineStyleParse::Set(OutlineStyle::Groove),
        "ridge" => OutlineStyleParse::Set(OutlineStyle::Ridge),
        "inset" => OutlineStyleParse::Set(OutlineStyle::Inset),
        "outset" => OutlineStyleParse::Set(OutlineStyle::Outset),
        "auto" => OutlineStyleParse::Set(OutlineStyle::Auto),
        _ => return None,
    })
}

/// A width token: `thin`/`medium`/`thick` or a non-negative length. Percents
/// and negatives are not outline widths.
pub(crate) fn parse_outline_width_kw(t: &str, fonts: &FontCtx) -> Option<OutlineWidthParse> {
    match t.trim().to_ascii_lowercase().as_str() {
        "thin" => Some(OutlineWidthParse::Px(1.0)),
        "medium" => Some(OutlineWidthParse::Medium),
        "thick" => Some(OutlineWidthParse::Px(5.0)),
        other => match css_len(other, fonts) {
            Some(Length::Px(v)) if v.is_finite() && v >= 0.0 => Some(OutlineWidthParse::Px(v)),
            _ => None,
        },
    }
}

/// `outline-offset` is a length and may be negative. Percents drop.
pub(crate) fn parse_outline_offset_kw(t: &str, fonts: &FontCtx) -> Option<f32> {
    match css_len(t.trim(), fonts) {
        Some(Length::Px(v)) if v.is_finite() => Some(v),
        _ => None,
    }
}

pub fn outline_style_css(style: Option<OutlineStyle>) -> &'static str {
    match style {
        Some(OutlineStyle::Solid) => "solid",
        Some(OutlineStyle::Dashed) => "dashed",
        Some(OutlineStyle::Dotted) => "dotted",
        Some(OutlineStyle::Double) => "double",
        Some(OutlineStyle::Groove) => "groove",
        Some(OutlineStyle::Ridge) => "ridge",
        Some(OutlineStyle::Inset) => "inset",
        Some(OutlineStyle::Outset) => "outset",
        Some(OutlineStyle::Auto) => "auto",
        None => "none",
    }
}

/// Chrome 157 computed outline lengths at device scale 1: a non-zero value
/// thinner than 1px becomes ±1px, otherwise the px truncates toward zero.
pub fn outline_px_css(len: Option<Length>, initial: &str) -> String {
    match len {
        Some(Length::Px(v)) => format!("{}px", snap_outline_px(v)),
        _ => initial.to_string(),
    }
}

fn snap_outline_px(v: f32) -> i64 {
    if !v.is_finite() || v == 0.0 {
        return 0;
    }
    let t = v.trunc() as i64;
    if t == 0 {
        if v > 0.0 {
            1
        } else {
            -1
        }
    } else {
        t
    }
}

/// One `outline` shorthand. Omitted legs stay at their initials (`none`,
/// currentcolor, `medium`). A repeated leg invalidates the declaration.
pub(crate) struct ParsedOutline {
    pub style: Option<OutlineStyle>,
    pub color: Option<crate::geom::Color>,
    pub width: Option<Length>,
}

pub(crate) fn parse_outline_shorthand(v: &str, fonts: &FontCtx) -> Option<ParsedOutline> {
    let mut style = None;
    let mut color = None;
    let mut width = None;
    let mut saw_style = false;
    let mut saw_color = false;
    let mut saw_width = false;
    let mut saw = false;
    for tok in split_sides(v) {
        let t = tok.to_ascii_lowercase();
        if let Some(st) = parse_outline_style_kw(&t) {
            if saw_style {
                return None;
            }
            style = match st {
                OutlineStyleParse::NoneKw => None,
                OutlineStyleParse::Set(s) => Some(s),
            };
            saw_style = true;
            saw = true;
            continue;
        }
        if let Some(w) = parse_outline_width_kw(&t, fonts) {
            if saw_width {
                return None;
            }
            width = match w {
                OutlineWidthParse::Medium => None,
                OutlineWidthParse::Px(px) => Some(Length::Px(px)),
            };
            saw_width = true;
            saw = true;
            continue;
        }
        if t == "currentcolor" {
            if saw_color {
                return None;
            }
            color = None;
            saw_color = true;
            saw = true;
            continue;
        }
        if let Some(c) = parse_color(&t) {
            if saw_color {
                return None;
            }
            color = Some(c);
            saw_color = true;
            saw = true;
            continue;
        }
        return None;
    }
    if !saw {
        return None;
    }
    Some(ParsedOutline {
        style,
        color,
        width,
    })
}

/// `text-decoration-style`. `None` on the style slot is the initial `solid`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecoStyle {
    Double,
    Dotted,
    Dashed,
    Wavy,
}

/// `text-decoration-thickness` other than the initial `auto` (`None`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DecoThick {
    FromFont,
    Len(Length),
}

/// One `text-decoration` shorthand. Omitted legs stay `None` (solid /
/// currentcolor / auto). The line set is empty when the shorthand named no
/// line keyword — that still resets the line to `none`.
pub(crate) struct ParsedTextDecoration {
    pub line: TextDecorations,
    pub style: Option<DecoStyle>,
    pub color: Option<crate::geom::Color>,
    pub thick: Option<DecoThick>,
}

/// `text-decoration` shorthand. A style, color, or thickness with no line
/// keyword is valid and resets the line to `none`.
pub(crate) fn parse_text_decoration_shorthand(
    v: &str,
    fonts: &FontCtx,
) -> Option<ParsedTextDecoration> {
    let mut line = TextDecorations::default();
    let mut saw_line = false;
    let mut saw = false;
    let mut style = None;
    let mut color = None;
    let mut thick = None;
    for tok in split_sides(v) {
        let t = tok.to_ascii_lowercase();
        match t.as_str() {
            "underline" => {
                line.underline = true;
                saw_line = true;
                saw = true;
            }
            "overline" => {
                line.overline = true;
                saw_line = true;
                saw = true;
            }
            "line-through" => {
                line.line_through = true;
                saw_line = true;
                saw = true;
            }
            "none" if !saw_line => {
                line = TextDecorations::default();
                saw_line = true;
                saw = true;
            }
            "solid" => {
                style = None;
                saw = true;
            }
            "double" => {
                style = Some(DecoStyle::Double);
                saw = true;
            }
            "dotted" => {
                style = Some(DecoStyle::Dotted);
                saw = true;
            }
            "dashed" => {
                style = Some(DecoStyle::Dashed);
                saw = true;
            }
            "wavy" => {
                style = Some(DecoStyle::Wavy);
                saw = true;
            }
            "auto" => {
                thick = None;
                saw = true;
            }
            "from-font" => {
                thick = Some(DecoThick::FromFont);
                saw = true;
            }
            "currentcolor" => {
                color = None;
                saw = true;
            }
            _ => {
                if let Some(c) = parse_color(&t) {
                    color = Some(c);
                    saw = true;
                } else {
                    let len = parse_css_length(&t)?;
                    thick = Some(DecoThick::Len(resolve_len(len, fonts)));
                    saw = true;
                }
            }
        }
    }
    if !saw {
        return None;
    }
    Some(ParsedTextDecoration {
        line,
        style,
        color,
        thick,
    })
}

/// Shared grammar for `text-decoration-line`. `none` clears the set; an
/// unknown token invalidates the whole declaration (None).
pub(crate) fn parse_text_decoration(v: &str, shorthand: bool) -> Option<TextDecorations> {
    let mut out = TextDecorations::default();
    let mut saw_line = false;
    for tok in split_sides(v) {
        let t = tok.to_ascii_lowercase();
        match t.as_str() {
            "underline" => {
                out.underline = true;
                saw_line = true;
            }
            "overline" => {
                out.overline = true;
                saw_line = true;
            }
            "line-through" => {
                out.line_through = true;
                saw_line = true;
            }
            "none" => {
                if !saw_line {
                    out = TextDecorations::default();
                    saw_line = true;
                }
            }
            "solid" | "double" | "dotted" | "dashed" | "wavy" if shorthand => {}
            _ if shorthand
                && (parse_color(&t).is_some()
                    || parse_css_length(&t).is_some()
                    || t == "auto"
                    || t == "from-font") => {}
            _ => return None,
        }
    }
    if saw_line {
        Some(out)
    } else {
        None
    }
}

/// What a border-style token means: not a style keyword at all, an explicit
/// no-border, or a line style.
pub(crate) enum BorderStyleKw {
    NotAStyle,
    NoBorder,
    Line(BorderStyle),
}

/// Keyword serialization for [`Overflow`] (the getComputedStyle face).
pub fn overflow_name(o: Overflow) -> &'static str {
    match o {
        Overflow::Visible => "visible",
        Overflow::Hidden => "hidden",
        Overflow::Clip => "clip",
        Overflow::Scroll => "scroll",
        Overflow::Auto => "auto",
    }
}

/// Overflow behavior. Paint-side only in this slice: non-visible values
/// clip descendants to the padding box. (CSS also makes clipping boxes
/// establish a BFC — margin-collapse/float containment is a later batch.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overflow {
    Visible,
    Hidden,
    Clip,
    Scroll,
    Auto,
}

/// `white-space`. `normal` collapses whitespace runs and wraps; `nowrap`
/// collapses but never wraps. The `pre` family preserves: `pre` never wraps
/// at all, `pre-wrap` wraps with hanging spaces, `break-spaces` lets every
/// preserved space end a line (spaces never hang), `pre-line` collapses
/// spaces but keeps newlines as hard breaks. Honored end to end for
/// pure-text runs — mixed inline runs keep normal collapsing, the same
/// limitation `nowrap` already has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhiteSpace {
    Normal,
    Nowrap,
    Pre,
    PreWrap,
    PreLine,
    BreakSpaces,
}

impl WhiteSpace {
    /// Modes with no soft wrap opportunities at all: the run measures and
    /// paints as one line per hard break only.
    pub fn no_soft_wrap(self) -> bool {
        matches!(self, WhiteSpace::Nowrap | WhiteSpace::Pre)
    }

    /// Modes that keep whitespace characters verbatim (no trim, no run
    /// collapsing). `pre-line` is NOT here: it collapses spaces.
    pub fn preserves_spaces(self) -> bool {
        matches!(
            self,
            WhiteSpace::Pre | WhiteSpace::PreWrap | WhiteSpace::BreakSpaces
        )
    }

    /// Modes that render newline characters as line breaks.
    pub fn preserves_newlines(self) -> bool {
        !matches!(self, WhiteSpace::Normal | WhiteSpace::Nowrap)
    }
}

/// `text-overflow` (non-inherited). Only meaningful on a box that clips
/// (`overflow` non-visible); the marker itself is applied at paint/raster
/// time — layout rects, scroll extents and selection keep the full text,
/// per spec. The `<string>` form is a later batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextOverflow {
    Clip,
    Ellipsis,
}

/// `object-fit` (batch 5c): how replaced content maps into its box. The
/// math mirrors blitz-paint/src/sizing.rs `compute_object_fit` — Fill
/// stretches, Contain/Cover pick min/max of the per-axis scale ratios,
/// None uses the natural size, ScaleDown is Contain unless the natural
/// size is already smaller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ObjectFit {
    #[default]
    Fill,
    Contain,
    Cover,
    None,
    ScaleDown,
}

/// One axis of `object-position` (batch 5c): a percentage of the free
/// space (box − painted object) or an absolute px offset. The initial
/// value is 50% both axes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ObjectPositionPart {
    Percent(f32),
    Px(f32),
}

pub(crate) fn border_style_kw(v: &str) -> BorderStyleKw {
    match v {
        "solid" => BorderStyleKw::Line(BorderStyle::Solid),
        "dashed" => BorderStyleKw::Line(BorderStyle::Dashed),
        "dotted" => BorderStyleKw::Line(BorderStyle::Dotted),
        "double" => BorderStyleKw::Line(BorderStyle::Double),
        "none" | "hidden" => BorderStyleKw::NoBorder,
        _ => BorderStyleKw::NotAStyle,
    }
}

/// One grid track sizing. `1fr` / `100px` / `25%` / `auto` / `minmax(a, b)`
/// — repeat() expands away at parse time, so it never reaches this enum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridTrack {
    Fr(f32),
    Px(f32),
    Percent(f32),
    /// `calc(percent% + px)`. `percent` is the CSS 0–100 number.
    Calc {
        percent: f32,
        px: f32,
    },
    Auto,
    /// minmax(min, max); the two arguments are shallow (nested minmax is
    /// invalid CSS), which also keeps the enum non-recursive.
    MinMax {
        min: TrackSize,
        max: TrackSize,
    },
}

/// A minmax() argument — one of the simple sizings, never another minmax.
/// Percent resolves against the grid container's content box at layout
/// time, so the cascade stores the raw number.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrackSize {
    Fr(f32),
    Px(f32),
    Percent(f32),
    /// `calc(percent% + px)`. `percent` is the CSS 0–100 number.
    Calc {
        percent: f32,
        px: f32,
    },
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexDirection {
    Row,
    RowReverse,
    Column,
    ColumnReverse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexWrapMode {
    NoWrap,
    Wrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JustifyMode {
    FlexStart,
    Center,
    FlexEnd,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
    /// `start`. LTR horizontal-tb packs on the same edge as flex-start.
    Start,
    /// `end`.
    End,
    /// `justify-items: stretch` / `justify-self: stretch`.
    Stretch,
}

/// `align-content` keywords. Carries `Stretch` (content lines grow to fill
/// the cross axis), which justify-content's set lacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignContentMode {
    FlexStart,
    Center,
    FlexEnd,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
    Stretch,
    /// `start`. Same edge as flex-start in this LTR mode.
    Start,
    /// `end`.
    End,
}

impl JustifyMode {
    /// Static-position offset along a justify axis (grid inline axis). The
    /// distributed <space-*> values only apply BETWEEN multiple items; a
    /// sole out-of-flow static item treats them as center, which is also
    /// where they land in Chrome for a single-item area.
    pub fn static_offset(self, free: f32) -> f32 {
        match self {
            JustifyMode::FlexStart
            | JustifyMode::Start
            | JustifyMode::Stretch
            | JustifyMode::SpaceBetween => 0.0,
            JustifyMode::Center | JustifyMode::SpaceAround | JustifyMode::SpaceEvenly => free / 2.0,
            JustifyMode::FlexEnd | JustifyMode::End => free,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignMode {
    Stretch,
    FlexStart,
    Center,
    FlexEnd,
    /// `start`. Same edge as flex-start in this LTR mode.
    Start,
    /// `end`.
    End,
}

impl AlignMode {
    /// Static-position offset along an align axis (flex cross axis, grid
    /// block axis): stretch/flex-start park at the start edge — with auto
    /// insets and a definite size the box never stretches (CSS2 §10.3.7),
    /// so stretch degenerates to start.
    pub fn static_offset(self, free: f32) -> f32 {
        match self {
            AlignMode::Stretch | AlignMode::FlexStart | AlignMode::Start => 0.0,
            AlignMode::Center => free / 2.0,
            AlignMode::FlexEnd | AlignMode::End => free,
        }
    }
}

pub fn justify_mode_css(mode: Option<JustifyMode>, unset: &str) -> String {
    match mode {
        Some(JustifyMode::FlexStart) => "flex-start".into(),
        Some(JustifyMode::Center) => "center".into(),
        Some(JustifyMode::FlexEnd) => "flex-end".into(),
        Some(JustifyMode::SpaceBetween) => "space-between".into(),
        Some(JustifyMode::SpaceAround) => "space-around".into(),
        Some(JustifyMode::SpaceEvenly) => "space-evenly".into(),
        Some(JustifyMode::Start) => "start".into(),
        Some(JustifyMode::End) => "end".into(),
        Some(JustifyMode::Stretch) => "stretch".into(),
        None => unset.into(),
    }
}

pub fn align_mode_css(mode: Option<AlignMode>, unset: &str) -> String {
    match mode {
        Some(AlignMode::Stretch) => "stretch".into(),
        Some(AlignMode::FlexStart) => "flex-start".into(),
        Some(AlignMode::Center) => "center".into(),
        Some(AlignMode::FlexEnd) => "flex-end".into(),
        Some(AlignMode::Start) => "start".into(),
        Some(AlignMode::End) => "end".into(),
        None => unset.into(),
    }
}

pub fn align_content_css(mode: Option<AlignContentMode>) -> &'static str {
    match mode {
        Some(AlignContentMode::FlexStart) => "flex-start",
        Some(AlignContentMode::Center) => "center",
        Some(AlignContentMode::FlexEnd) => "flex-end",
        Some(AlignContentMode::SpaceBetween) => "space-between",
        Some(AlignContentMode::SpaceAround) => "space-around",
        Some(AlignContentMode::SpaceEvenly) => "space-evenly",
        Some(AlignContentMode::Stretch) => "stretch",
        Some(AlignContentMode::Start) => "start",
        Some(AlignContentMode::End) => "end",
        None => "normal",
    }
}

/// Which legacy-flexbox display the cascade declared (#275). Kept besides
/// the Block/InlineBlock geometry mapping so the clamp gate knows the box
/// was declared as `-webkit-box` at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WebkitBoxKind {
    #[default]
    Box,
    InlineBox,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Display {
    #[default]
    Block,
    Inline,
    InlineBlock,
    Flex,
    Grid,
    /// Table family (batch: table layout). The three inner roles drive the
    /// layout engine's table branch; `Display::Table` on the element means
    /// "shrink-to-fit grid of rows", not a block.
    Table,
    TableRow,
    TableCell,
    /// `display: table-caption`: a table's caption box. Its text counts toward
    /// the table's minimum width only, never its maximum.
    TableCaption,
    None,
}

/// `column-width`. `Auto` is the initial. A length is already absolute px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ColWidth {
    Auto,
    Px(f32),
}

/// `column-count`. `Auto` is the initial. `N` is an integer ≥ 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColCount {
    Auto,
    N(u32),
}

pub fn column_width_css(width: Option<Length>) -> String {
    match width {
        Some(Length::Px(v)) => {
            if (v - v.round()).abs() < f32::EPSILON {
                format!("{}px", v as i64)
            } else {
                let s = format!("{v}");
                format!("{}px", s.trim_end_matches('0').trim_end_matches('.'))
            }
        }
        _ => "auto".into(),
    }
}

/// Chrome omits a zero `transform-origin` z.
pub fn append_origin_z(xy: String, z: &Length) -> String {
    match z {
        Length::Px(v) if v.abs() >= f32::EPSILON => {
            let n = if (v - v.round()).abs() < f32::EPSILON {
                (v.round() as i64).to_string()
            } else {
                format!("{v}")
            };
            format!("{xy} {n}px")
        }
        _ => xy,
    }
}

pub fn column_count_css(count: Option<u32>) -> String {
    match count {
        Some(n) => n.to_string(),
        None => "auto".into(),
    }
}

/// One token of `column-width`: `auto` or a non-negative length. Percents
/// and negatives are invalid.
pub(crate) fn parse_column_width_token(t: &str, fonts: &FontCtx) -> Option<ColWidth> {
    if t.eq_ignore_ascii_case("auto") {
        return Some(ColWidth::Auto);
    }
    match css_len(t.trim(), fonts) {
        Some(Length::Px(px)) if px.is_finite() && px >= 0.0 => Some(ColWidth::Px(px)),
        _ => None,
    }
}

/// One token of `column-count`: `auto` or an integer ≥ 1 (`+3` counts).
pub(crate) fn parse_column_count_token(t: &str) -> Option<ColCount> {
    if t.eq_ignore_ascii_case("auto") {
        return Some(ColCount::Auto);
    }
    let digits = t.trim().strip_prefix('+').unwrap_or(t.trim());
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n: u32 = digits.parse().ok()?;
    (n >= 1).then_some(ColCount::N(n))
}

enum ColPiece {
    Auto,
    Width(f32),
    Count(u32),
}

fn column_piece(t: &str, fonts: &FontCtx) -> Option<ColPiece> {
    // A positive integer is a count. Unitless `0` is a length, not a count.
    // `auto` fails the integer test and is a width keyword.
    if let Some(ColCount::N(n)) = parse_column_count_token(t) {
        return Some(ColPiece::Count(n));
    }
    match parse_column_width_token(t, fonts)? {
        ColWidth::Px(px) => Some(ColPiece::Width(px)),
        ColWidth::Auto => Some(ColPiece::Auto),
    }
}

/// `columns` shorthand. One `auto` resets both legs. Two tokens may be a
/// width and a count in either order; a repeated leg is invalid.
pub(crate) fn parse_columns_shorthand(v: &str, fonts: &FontCtx) -> Option<(ColWidth, ColCount)> {
    let toks = split_sides(v);
    if toks.is_empty() || toks.len() > 2 {
        return None;
    }
    let mut pieces = Vec::with_capacity(toks.len());
    for tok in toks {
        pieces.push(column_piece(tok, fonts)?);
    }
    Some(match pieces.as_slice() {
        [ColPiece::Auto] | [ColPiece::Auto, ColPiece::Auto] => (ColWidth::Auto, ColCount::Auto),
        [ColPiece::Count(n)] => (ColWidth::Auto, ColCount::N(*n)),
        [ColPiece::Width(px)] => (ColWidth::Px(*px), ColCount::Auto),
        [ColPiece::Auto, ColPiece::Count(n)] | [ColPiece::Count(n), ColPiece::Auto] => {
            (ColWidth::Auto, ColCount::N(*n))
        }
        [ColPiece::Auto, ColPiece::Width(px)] | [ColPiece::Width(px), ColPiece::Auto] => {
            (ColWidth::Px(*px), ColCount::Auto)
        }
        [ColPiece::Count(n), ColPiece::Width(px)] | [ColPiece::Width(px), ColPiece::Count(n)] => {
            (ColWidth::Px(*px), ColCount::N(*n))
        }
        _ => return None,
    })
}
