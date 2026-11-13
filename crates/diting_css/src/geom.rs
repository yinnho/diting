//! Lengths, colors, transforms, and the font context that resolves them.
//!
//! Split from the css hub. The public names stay re-exported from the
//! crate root. Pure move.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(pub u8, pub u8, pub u8, pub u8);

/// An SVG paint value (svg v1): a resolved color, or `currentColor` which
/// stays symbolic through the cascade and resolves against the element's
/// inherited `color` when the svg compiler flattens the subtree. "none" is
/// stored as a fully transparent color — both mean "draw nothing".
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SvgPaint {
    Color(Color),
    CurrentColor,
}

/// A computed length (batch 2e). `Px` is fully resolved — em/rem were folded
/// in during the cascade (em against the element's own font-size, rem
/// against the root font-size, matching CSS computed-value semantics).
/// `Percent` stays symbolic here and resolves against the containing block
/// in the layout engine (taffy's percent semantics match CSS: margins and
/// paddings against CB width, insets per-axis).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Length {
    Px(f32),
    Percent(f32),
    /// A calc() mixing percent and px (`calc(100% - 30px)`), kept symbolic:
    /// percent in CSS 0-100 scale resolves against the containing block in
    /// the layout engine's post-layout repair pass (width/min/max only —
    /// other slots keep the percent-only placeholder or drop, see
    /// to_taffy_style). The px part is always exact here.
    Calc {
        percent: f32,
        px: f32,
    },
    /// CSS `auto` - legal ONLY for margins (the horizontal centering idiom
    /// and the abspos §10.3.3 centering pattern). Padding and borders reject
    /// it at parse time; layout maps it to taffy's
    /// `LengthPercentageAuto::auto()`, which implements both the in-flow
    /// auto-margin expansion and abspos auto-margin resolution.
    Auto,
    /// CSS sizing keywords, accepted only in the width/min-width/max-width
    /// slots. taffy's Dimension has no intrinsic keyword, so layout resolves
    /// them against the subtree's intrinsic width with measure passes at
    /// build time (`resolve_sizing_keywords`); every other slot keeps them
    /// unreachable and maps them like `Auto` defensively.
    MinContent,
    MaxContent,
    FitContent,
}

/// The 2D affine `transform` this pipeline computes (CSS matrix
/// convention): x' = a·x + c·y + tx, y' = b·x + d·y + ty. The translate
/// stays a symbolic Length — percentages resolve against the element's own
/// border box at collect time. Function lists accumulate in CSS order (the
/// first-listed function is the outermost map).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform2D {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub tx: Length,
    pub ty: Length,
}

impl Transform2D {
    /// Whether the linear part is diagonal (translate/scale only, no
    /// rotate/skew): the layout collect walk keeps pre-baking geometry into
    /// item fields on that path; anything else flips the element's paint
    /// items into local coordinates bracketed by SetXf/ClearXf.
    pub fn is_axis_aligned(&self) -> bool {
        self.b == 0.0 && self.c == 0.0
    }

    /// Concrete [a b c d e f] affine. Percentage translates resolve against
    /// the reference size the caller supplies (0 when unknown — SVG subtree
    /// consumers pass the viewBox extent). Non-px/percent translates can't
    /// appear in a valid transform, so they resolve to 0.
    pub fn to_matrix_with(self, ref_w: f32, ref_h: f32) -> [f32; 6] {
        let px = |l: Length, r: f32| match l {
            Length::Px(v) => v,
            Length::Percent(p) => p * r / 100.0,
            _ => 0.0,
        };
        [
            self.a,
            self.b,
            self.c,
            self.d,
            px(self.tx, ref_w),
            px(self.ty, ref_h),
        ]
    }
}

/// Declaration-level length: em/rem can't resolve until the font context is
/// known, so parsing keeps them symbolic for the cascade to fold in. vw/vh
/// keep the same treatment against the viewport pair on [`FontCtx`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CssLength {
    Px(f32),
    Em(f32),
    Rem(f32),
    Percent(f32),
    Vw(f32),
    Vh(f32),
    Vmin(f32),
    Vmax(f32),
    Ch(f32),
    Ex(f32),
}

/// `line-height` computed value. Inherited as-is: the number form keeps its
/// multiplier (spec: computed value of a unitless line-height IS the number,
/// resolved against each descendant's own font-size), lengths are already
/// absolute px. `Normal` is the initial value (layout maps it to the same
/// `font-size * 1.2` blitz pins for `normal`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineHeightSpec {
    Normal,
    /// Unitless multiplier (and `%`, folded at parse time — a percentage
    /// resolves against the element's own font-size, exactly like a number).
    Number(f32),
    Px(f32),
}

/// Shape-level parse for @supports probing and cascade application; em/rem
/// fold to px later against the element's FontCtx.
pub(super) enum LineHeightRaw {
    Normal,
    Number(f32),
    Len(CssLength),
}

pub(super) fn parse_line_height(v: &str) -> Option<LineHeightRaw> {
    let v = v.trim();
    if v.eq_ignore_ascii_case("normal") {
        return Some(LineHeightRaw::Normal);
    }
    // Unitless numbers are legal ONLY for line-height (line-height: 1.6).
    if let Ok(n) = v.parse::<f32>() {
        let numeric_shape = v
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == '-');
        return (n.is_finite() && n > 0.0 && numeric_shape).then_some(LineHeightRaw::Number(n));
    }
    parse_css_length(v).map(LineHeightRaw::Len)
}

/// The initial / default root font size (CSS `medium`).
pub const DEFAULT_ROOT_FONT_SIZE: f32 = 16.0;

/// Font context a length resolves against. `own` is this element's computed
/// font-size (font-size itself resolves em/% against the PARENT, per spec).
/// `viewport_w/h` back the viewport units (vw/vh); the default pair is the
/// 1280×720 ICB convention, and every real cascade threads the page's actual
/// viewport through [`diting_layout`] seam functions.
#[derive(Debug, Clone, Copy)]
pub struct FontCtx {
    pub own: f32,
    pub root: f32,
    pub viewport_w: f32,
    pub viewport_h: f32,
}

impl Default for FontCtx {
    fn default() -> Self {
        FontCtx {
            own: DEFAULT_ROOT_FONT_SIZE,
            root: DEFAULT_ROOT_FONT_SIZE,
            viewport_w: DEFAULT_VIEWPORT.0,
            viewport_h: DEFAULT_VIEWPORT.1,
        }
    }
}

/// Standalone parse/apply contexts without a threaded viewport resolve
/// against this pair (matches the screenshot default width).
pub const DEFAULT_VIEWPORT: (f32, f32) = (1280.0, 720.0);

pub(super) fn resolve_len(l: CssLength, fonts: &FontCtx) -> Length {
    match l {
        CssLength::Px(x) => Length::Px(x),
        CssLength::Em(n) => Length::Px(n * fonts.own),
        CssLength::Rem(n) => Length::Px(n * fonts.root),
        // ch/ex (#138): the spec's mandated fallback when the '0'/'x'
        // advance can't be determined — 0.5em. Font selection happens per
        // text run in layout, so the element's actual font isn't known at
        // cascade fold time; measuring real advances is a layout-metrics
        // follow-up. Before this arm `4ch` failed the parse and the whole
        // declaration was dropped — worse than any approximation.
        CssLength::Ch(n) => Length::Px(n * 0.5 * fonts.own),
        CssLength::Ex(n) => Length::Px(n * 0.5 * fonts.own),
        CssLength::Percent(p) => Length::Percent(p),
        CssLength::Vw(n) => Length::Px(n * fonts.viewport_w / 100.0),
        CssLength::Vh(n) => Length::Px(n * fonts.viewport_h / 100.0),
        CssLength::Vmin(n) => Length::Px(n * fonts.viewport_w.min(fonts.viewport_h) / 100.0),
        CssLength::Vmax(n) => Length::Px(n * fonts.viewport_w.max(fonts.viewport_h) / 100.0),
    }
}

/// px / em / rem / % (unitless `0` is a legal length; `rem` before `em`
/// because "rem" also ends in "em").
pub(super) fn parse_css_length(v: &str) -> Option<CssLength> {
    let v = v.trim();
    if v == "0" {
        return Some(CssLength::Px(0.0));
    }
    if let Some(p) = v.strip_suffix('%') {
        return p.css_f32().map(CssLength::Percent);
    }
    if let Some(r) = v.strip_suffix("rem") {
        return r.css_f32().map(CssLength::Rem);
    }
    if let Some(l) = calc::parse_extra_unit(v) {
        return Some(l);
    }
    // "vw" before "vh" is arbitrary (no suffix overlap like rem/em), but
    // both must precede nothing else — no other unit ends in w/h.
    if let Some(n) = v.strip_suffix("vw") {
        return n.css_f32().map(CssLength::Vw);
    }
    if let Some(n) = v.strip_suffix("vh") {
        return n.css_f32().map(CssLength::Vh);
    }
    // ch/ex (#138): no suffix overlap with the other units (nothing else
    // ends in ch/ex), so they slot anywhere after the % arm.
    if let Some(n) = v.strip_suffix("ch") {
        return n.css_f32().map(CssLength::Ch);
    }
    if let Some(n) = v.strip_suffix("ex") {
        return n.css_f32().map(CssLength::Ex);
    }
    if let Some(e) = v.strip_suffix("em") {
        return e.css_f32().map(CssLength::Em);
    }
    if let Some(px) = v.strip_suffix("px") {
        return px.css_f32().map(CssLength::Px);
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Sides {
    pub top: Option<Length>,
    pub right: Option<Length>,
    pub bottom: Option<Length>,
    pub left: Option<Length>,
}
