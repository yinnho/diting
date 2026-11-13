//! Length, side, grid-track and font-size parsing.
//!
//! Split from the css hub. Callers still see these names on the crate
//! root. Pure move.

use super::*;

/// Length arm helper for `apply_one`: parse + fold em/rem against the font
/// context; calc() bodies route through the evaluator (same folding per term).
pub(super) fn css_len(val: &str, fonts: &FontCtx) -> Option<Length> {
    let t = val.trim();
    if let Some(l) = eval_math_call(t, fonts) {
        return Some(l);
    }
    parse_css_length(t).map(|l| resolve_len(l, fonts))
}

/// Sizing keywords first: only the width family accepts them (the match
/// arms decide legality); everything else is a regular length.
pub(super) fn css_len_sizing(val: &str, fonts: &FontCtx) -> Option<Length> {
    match val.trim().to_ascii_lowercase().as_str() {
        "min-content" => Some(Length::MinContent),
        "max-content" => Some(Length::MaxContent),
        "fit-content" => Some(Length::FitContent),
        _ => css_len(val, fonts),
    }
}

/// Box-offset slot (#129 family): `auto` is a legal value that resets the
/// slot to the initial value; anything else unparseable is a declaration
/// Chrome drops at parse time — the already-cascaded slot must survive
/// untouched (a reset would yank a positioned box to its static spot).
pub(super) fn css_box_offset(slot: &mut Option<Length>, v: &str, fonts: &FontCtx) -> bool {
    if v.eq_ignore_ascii_case("auto") {
        *slot = None;
        return true;
    }
    match css_len(v, fonts) {
        Some(l) => {
            *slot = Some(l);
            true
        }
        None => false,
    }
}

/// `aspect-ratio: 1.5` or `16 / 9` (width / height).
pub(super) fn parse_aspect_ratio(v: &str) -> Option<f32> {
    let v = v.trim();
    if v == "auto" {
        return None;
    }
    let (w, h) = match v.split_once('/') {
        Some((w, h)) => (w.trim().css_f32()?, h.trim().css_f32()?),
        None => (v.css_f32()?, 1.0),
    };
    if w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0 {
        Some(w / h)
    } else {
        None
    }
}

/// Plain number (flex-grow: 1).
/// CSS number parser. Rust's float grammar also accepts `NaN`, `inf` and
/// `Infinity`; CSS's does not (numbers are finite decimal forms), so a page
/// writing `style.left = 'NaNpx'` must see the whole declaration dropped the
/// way Chrome drops it — a stored NaN poisons the layout solve and collapses
/// boxes (tmall's 选择视频 dialog, #129). Every numeric parse in this module
/// funnels through this one gate.
///
/// Finite-but-giant values saturate at Chrome's LayoutUnit bound (#147,
/// blitz#941): getComputedStyle of `height:3e38px` returns `3.35544e+07px`
/// measured headless — Lengths clamp at 2^25 px. Without this, two 3e38px
/// margins sum to inf in the taffy solve and line-height:3e37 × font-size
/// overflows at the product, whose inf−inf paint centering collapses boxes
/// to NaN geometry (the #129 class through a different door).
const MAX_CSS_ABS: f32 = 33_554_432.0; // 2^25 = LayoutUnit saturation
pub(super) fn css_f32(s: &str) -> Option<f32> {
    // NB: this body must stay on `.parse()` directly — routing through the
    // trait method below is mutual recursion (the sed that rewrote the forty
    // old call sites also rewrote this line once; the test run stack-overflowed).
    let n = s.parse::<f32>().ok()?;
    if !n.is_finite() {
        return None;
    }
    Some(n.clamp(-MAX_CSS_ABS, MAX_CSS_ABS))
}

/// Method posture over [`css_f32`] so the mechanical rewrite of the old
/// `.parse::<f32>().ok()` call sites stays one token wide.
pub(crate) trait CssF32 {
    fn css_f32(&self) -> Option<f32>;
}
impl CssF32 for str {
    fn css_f32(&self) -> Option<f32> {
        css_f32(self)
    }
}

pub(super) fn parse_num_f32(v: &str) -> Option<f32> {
    css_f32(v)
}

/// Whitespace-separated track list: `1fr 2fr 100px auto`. Unknown tokens
/// abort the whole declaration (browsers drop it entirely).
pub(super) fn parse_grid_tracks(v: &str) -> Option<Vec<GridTrack>> {
    let mut tracks = Vec::new();
    let mut rest = v.trim();
    while !rest.is_empty() {
        let (token, tail) = next_track_token(rest)?;
        // repeat(N, <track-list>) expands inline to N copies of the list.
        // auto-fill/auto-fit counts need the container's size, which the
        // cascade doesn't have — those reject like any unknown token.
        if let Some(inner) = token
            .strip_prefix("repeat(")
            .and_then(|t| t.strip_suffix(')'))
        {
            let (count, list) = inner.split_once(',')?;
            let count: usize = count.trim().parse().ok()?;
            // Chrome caps explicit tracks at 1000; repeat expansion must not
            // let a stylesheet allocate unboundedly.
            if count == 0 || count > 1000 {
                return None;
            }
            let expanded = parse_grid_tracks(list.trim())?;
            for _ in 0..count {
                tracks.extend_from_slice(&expanded);
            }
        } else {
            tracks.push(parse_grid_track_token(token)?);
        }
        rest = tail.trim_start();
    }
    if tracks.is_empty() {
        None
    } else {
        Some(tracks)
    }
}

/// Split off one track token: `minmax(a, b)` and `repeat(n, …)` contain
/// spaces, so they can't go through plain whitespace splitting — a token
/// whose first `(` precedes any whitespace runs to its balanced close
/// (repeat lists can nest a minmax, so the scan counts depth).
pub(super) fn next_track_token(v: &str) -> Option<(&str, &str)> {
    let ws = v.find(char::is_whitespace).unwrap_or(v.len());
    let paren = v.find('(').unwrap_or(v.len());
    if paren < ws {
        let mut depth = 0usize;
        for (i, c) in v.char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth = depth.checked_sub(1)?;
                    if depth == 0 {
                        return Some(v.split_at(i + 1));
                    }
                }
                _ => {}
            }
        }
        return None;
    }
    if ws == 0 {
        None
    } else {
        Some(v.split_at(ws))
    }
}

/// One sizing token: `auto`, `<len>` (px; rem resolves against the 16px
/// browser root — grid tracks live at the top of the page where the root
/// size is what authors mean), `<percent>` (of the grid container's content
/// box, resolved at layout time), `Nfr`, `0`, or a `minmax(a, b)` pair.
/// `min-content`/`max-content` approximate to `auto` until the track
/// machinery learns them.
pub(super) fn parse_grid_track_token(tok: &str) -> Option<GridTrack> {
    if let Some(inner) = tok
        .strip_prefix("minmax(")
        .and_then(|t| t.strip_suffix(')'))
    {
        let (a, b) = inner.split_once(',')?;
        return Some(GridTrack::MinMax {
            min: parse_track_size(a.trim())?,
            max: parse_track_size(b.trim())?,
        });
    }
    parse_track_size(tok).map(|sz| match sz {
        TrackSize::Fr(f) => GridTrack::Fr(f),
        TrackSize::Px(px) => GridTrack::Px(px),
        TrackSize::Percent(p) => GridTrack::Percent(p),
        TrackSize::Auto => GridTrack::Auto,
    })
}

/// One minmax() argument (or a bare track token): same sizings as
/// [`parse_grid_track_token`], minus minmax itself.
pub(super) fn parse_track_size(tok: &str) -> Option<TrackSize> {
    if tok.eq_ignore_ascii_case("auto")
        || tok.eq_ignore_ascii_case("min-content")
        || tok.eq_ignore_ascii_case("max-content")
    {
        return Some(TrackSize::Auto);
    }
    if let Some(fr) = tok.strip_suffix("fr") {
        return fr.css_f32().map(TrackSize::Fr);
    }
    if let Some(px) = parse_px_f32(tok) {
        return Some(TrackSize::Px(px));
    }
    if let Some(r) = tok.strip_suffix("rem") {
        return r.css_f32().map(|n| TrackSize::Px(n * 16.0));
    }
    if let Some(p) = tok.strip_suffix('%') {
        return p.css_f32().map(TrackSize::Percent);
    }
    if tok == "0" {
        return Some(TrackSize::Px(0.0));
    }
    None
}

/// `'a b' 'c d'` → `[[a, b], [c, d]]`. Every string row must have the same
/// cell count (CSS §7.3) or the whole declaration is invalid → None.
pub(super) fn parse_grid_template_areas(v: &str) -> Option<Vec<Vec<String>>> {
    let mut rows = Vec::new();
    for raw in v.split('\'') {
        let row = raw.trim();
        if row.is_empty() {
            continue;
        }
        let cells: Vec<String> = row.split_whitespace().map(str::to_string).collect();
        if cells.is_empty() {
            return None;
        }
        rows.push(cells);
    }
    if rows.is_empty() {
        return None;
    }
    let width = rows[0].len();
    if rows.iter().any(|r| r.len() != width) {
        return None;
    }
    Some(rows)
}

pub(super) fn set_side(sides: &mut Sides, name: &str, value: Option<Length>) {
    let slot = match name.rsplit_once('-').map(|(_, side)| side) {
        Some("top") => &mut sides.top,
        Some("right") => &mut sides.right,
        Some("bottom") => &mut sides.bottom,
        Some("left") => &mut sides.left,
        _ => return,
    };
    *slot = value;
}

/// CSS 1–4 value expansion (px/em/rem/%/calc(); unknown units drop to None).
/// `auto` tokens pass through as `Length::Auto` (margin centering) - the
/// padding/border call sites reject them by dropping the declaration.
pub(super) fn expand_sides(value: &str, fonts: &FontCtx) -> Sides {
    let vals: Vec<Option<Length>> = split_sides(value)
        .into_iter()
        .map(|tok| {
            if tok.eq_ignore_ascii_case("auto") {
                Some(Length::Auto)
            } else {
                eval_math_call(tok, fonts)
                    .or_else(|| parse_css_length(tok).map(|l| resolve_len(l, fonts)))
            }
        })
        .collect();
    match vals.as_slice() {
        [one] => Sides {
            top: *one,
            right: *one,
            bottom: *one,
            left: *one,
        },
        [t, r] => Sides {
            top: *t,
            right: *r,
            bottom: *t,
            left: *r,
        },
        [t, r, b] => Sides {
            top: *t,
            right: *r,
            bottom: *b,
            left: *r,
        },
        [t, r, b, l] => Sides {
            top: *t,
            right: *r,
            bottom: *b,
            left: *l,
        },
        _ => Sides::default(),
    }
}

/// Split a shorthand value into its top-level whitespace-separated tokens,
/// keeping whitespace INSIDE balanced parentheses together —
/// `calc(100% - 30px) 5px` is two tokens, not four.
pub(super) fn split_sides(value: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut start: Option<usize> = None;
    for (idx, ch) in value.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
        if depth == 0 && ch.is_whitespace() {
            if let Some(s) = start.take() {
                out.push(&value[s..idx]);
            }
        } else if start.is_none() {
            start = Some(idx);
        }
    }
    if let Some(s) = start {
        out.push(&value[s..]);
    }
    out
}

/// Reject `Length::Auto` entries (padding/border grammar: auto is illegal
/// there, CSS drops the whole declaration). Returns None if any side is auto.
pub(super) fn sides_no_auto(s: &Sides) -> Option<Sides> {
    let ok = |v: Option<Length>| match v {
        Some(Length::Auto) => None,
        v => Some(v),
    };
    Some(Sides {
        top: ok(s.top)?,
        right: ok(s.right)?,
        bottom: ok(s.bottom)?,
        left: ok(s.left)?,
    })
}

/// px-only float length (gaps, flex-basis, grid tracks: fractional px legal,
/// units beyond px are later batches).
pub(super) fn parse_px_f32(v: &str) -> Option<f32> {
    let v = v.trim();
    if v == "0" {
        return Some(0.0);
    }
    v.strip_suffix("px")?.css_f32()
}

/// font-size accepts px/em/rem/% and the common absolute keywords. em/%
/// resolve against the PARENT font-size, rem against the root — done by the
/// cascade's pre-pass via `font_size_px`. The relative keywords fold to em
/// factors (CSS 2.1 leaves the exact ratio UA-defined; 1.2 is the Chrome
/// ladder, so smaller == ÷1.2, larger == ×1.2).
pub(super) fn parse_font_size_len(v: &str) -> Option<CssLength> {
    let v = v.trim();
    if let Some(l) = parse_css_length(v) {
        return Some(l);
    }
    match v {
        "small" => Some(CssLength::Px(13.0)),
        "medium" => Some(CssLength::Px(16.0)),
        "large" => Some(CssLength::Px(18.0)),
        "x-large" => Some(CssLength::Px(24.0)),
        "larger" => Some(CssLength::Em(1.2)),
        "smaller" => Some(CssLength::Em(1.0 / 1.2)),
        _ => None,
    }
}

/// Fold a parsed font-size against its resolution bases.
pub(super) fn font_size_px(
    l: CssLength,
    parent_fs: f32,
    root_fs: f32,
    viewport: (f32, f32),
) -> f32 {
    match l {
        CssLength::Px(x) => x,
        CssLength::Em(n) => n * parent_fs,
        CssLength::Rem(n) => n * root_fs,
        CssLength::Percent(p) => p / 100.0 * parent_fs,
        CssLength::Vw(n) => n * viewport.0 / 100.0,
        CssLength::Vh(n) => n * viewport.1 / 100.0,
        CssLength::Vmin(n) => n * viewport.0.min(viewport.1) / 100.0,
        CssLength::Vmax(n) => n * viewport.0.max(viewport.1) / 100.0,
        // font-size in ch/ex is self-referential; the 0.5em fallback basis
        // is the parent font-size, same as Em here.
        CssLength::Ch(n) | CssLength::Ex(n) => n * 0.5 * parent_fs,
    }
}
