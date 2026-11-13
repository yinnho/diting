//! Layout and text keyword types: display, flex, grid, overflow, borders.
//!
//! Split from the css hub. The public names stay re-exported from the
//! crate root. Pure move.

use super::*;

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
/// cascade here only records what the element itself declared. Color/style/
/// thickness legs of the shorthand parse-and-drop: lines always paint solid
/// in the text's own color.
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

/// Shared grammar for `text-decoration-line` (shorthand=false) and the
/// `text-decoration` shorthand (true: style keywords, colors and thickness
/// tokens are accepted and dropped). `none` clears the set; an unknown token
/// invalidates the whole declaration (None) like any CSS parse error, so a
/// bogus value never strips a UA decoration by accident.
pub(super) fn parse_text_decoration(v: &str, shorthand: bool) -> Option<TextDecorations> {
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
pub(super) enum BorderStyleKw {
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

pub(super) fn border_style_kw(v: &str) -> BorderStyleKw {
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
}

impl JustifyMode {
    /// Static-position offset along a justify axis (grid inline axis). The
    /// distributed <space-*> values only apply BETWEEN multiple items; a
    /// sole out-of-flow static item treats them as center, which is also
    /// where they land in Chrome for a single-item area.
    pub fn static_offset(self, free: f32) -> f32 {
        match self {
            JustifyMode::FlexStart | JustifyMode::SpaceBetween => 0.0,
            JustifyMode::Center | JustifyMode::SpaceAround | JustifyMode::SpaceEvenly => free / 2.0,
            JustifyMode::FlexEnd => free,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignMode {
    Stretch,
    FlexStart,
    Center,
    FlexEnd,
}

impl AlignMode {
    /// Static-position offset along an align axis (flex cross axis, grid
    /// block axis): stretch/flex-start park at the start edge — with auto
    /// insets and a definite size the box never stretches (CSS2 §10.3.7),
    /// so stretch degenerates to start.
    pub fn static_offset(self, free: f32) -> f32 {
        match self {
            AlignMode::Stretch | AlignMode::FlexStart => 0.0,
            AlignMode::Center => free / 2.0,
            AlignMode::FlexEnd => free,
        }
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
