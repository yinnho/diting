//! Length, side, grid-track and font-size parsing.
//!
//! Callers still see these names on the crate root. This module names the
//! length model, the math evaluator, and the grid-track keywords it fills.

use crate::calc::{eval_math_call, eval_math_call_with};
use crate::geom::{parse_css_length, resolve_len, CssLength, FontCtx, Length, Sides};
use crate::style_kw::{GridTrack, TrackSize};

/// Length arm helper for `apply_one`: parse + fold em/rem against the font
/// context; calc() bodies route through the evaluator (same folding per term).
pub(crate) fn css_len(val: &str, fonts: &FontCtx) -> Option<Length> {
    let t = val.trim();
    if let Some(l) = eval_math_call(t, fonts) {
        return Some(l);
    }
    parse_css_length(t).map(|l| resolve_len(l, fonts))
}

/// Sizing keywords first: only the width family accepts them (the match
/// arms decide legality); everything else is a regular length.
pub(crate) fn css_len_sizing(val: &str, fonts: &FontCtx) -> Option<Length> {
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
pub(crate) fn css_box_offset(slot: &mut Option<Length>, v: &str, fonts: &FontCtx) -> bool {
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
pub(crate) fn parse_aspect_ratio(v: &str) -> Option<f32> {
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

pub(crate) fn parse_num_f32(v: &str) -> Option<f32> {
    css_f32(v)
}

/// Whitespace-separated track list: `1fr 2fr 100px auto`, plus line names.
/// Unknown tokens abort the whole declaration (browsers drop it entirely).
pub(crate) fn parse_grid_tracks(v: &str, fonts: &FontCtx) -> Option<Vec<GridTrack>> {
    Some(parse_named_tracks(v, fonts)?.tracks)
}

/// Split off one track token: `minmax(a, b)` and `repeat(n, …)` contain
/// spaces, so they can't go through plain whitespace splitting — a token
/// whose first `(` precedes any whitespace runs to its balanced close
/// (repeat lists can nest a minmax, so the scan counts depth).
pub(super) fn next_track_token(v: &str) -> Option<(&str, &str)> {
    if v.starts_with('[') {
        let end = v.find(']')?;
        return Some(v.split_at(end + 1));
    }
    let ws = v.find(char::is_whitespace).unwrap_or(v.len());
    let paren = v.find('(').unwrap_or(v.len());
    let bracket = v.find('[').unwrap_or(v.len());
    if paren < ws && paren < bracket {
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
    let end = ws.min(bracket);
    if end == 0 {
        None
    } else {
        Some(v.split_at(end))
    }
}

/// One sizing token: `auto`, `<len>` (px; rem resolves against the 16px
/// browser root — grid tracks live at the top of the page where the root
/// size is what authors mean), `<percent>` (of the grid container's content
/// box, resolved at layout time), `Nfr`, `0`, or a `minmax(a, b)` pair.
/// `min-content`/`max-content` approximate to `auto` until the track
/// machinery learns them.
pub(super) fn parse_grid_track_token(tok: &str, fonts: &FontCtx) -> Option<GridTrack> {
    if let Some(inner) = paren_fn_inner(tok, "minmax") {
        let (a, b) = split_first_top_comma(inner)?;
        return Some(GridTrack::MinMax {
            min: parse_track_size(a, fonts)?,
            max: parse_track_size(b, fonts)?,
        });
    }
    parse_track_size(tok, fonts).map(|sz| match sz {
        TrackSize::Fr(f) => GridTrack::Fr(f),
        TrackSize::Px(px) => GridTrack::Px(px),
        TrackSize::Percent(p) => GridTrack::Percent(p),
        TrackSize::Calc { percent, px } => GridTrack::Calc { percent, px },
        TrackSize::Math { op, n, args } => GridTrack::Math { op, n, args },
        TrackSize::Auto => GridTrack::Auto,
    })
}

/// One minmax() argument (or a bare track token): same sizings as
/// [`parse_grid_track_token`], minus minmax itself.
pub(super) fn parse_track_size(tok: &str, fonts: &FontCtx) -> Option<TrackSize> {
    if tok.eq_ignore_ascii_case("auto")
        || tok.eq_ignore_ascii_case("min-content")
        || tok.eq_ignore_ascii_case("max-content")
    {
        return Some(TrackSize::Auto);
    }
    if let Some(fr) = strip_unit(tok, "fr") {
        return fr.css_f32().map(TrackSize::Fr);
    }
    if let Some(sz) = track_calc_size(tok, fonts) {
        return Some(sz);
    }
    if let Some(sz) = track_math_size(tok, fonts) {
        return Some(sz);
    }
    if let Some(px) = track_px(tok, fonts) {
        return Some(TrackSize::Px(px));
    }
    if let Some(p) = tok.strip_suffix('%') {
        return p.css_f32().map(TrackSize::Percent);
    }
    // Unitless zero is a length. `0fr` and `0%` already returned above.
    if unitless_zero(tok) {
        return Some(TrackSize::Px(0.0));
    }
    None
}

/// `calc()` that mixes a percentage with px. Chrome keeps it symbolic
/// (`calc(50% + 10px)`, percent term first) until the track is used.
/// `+`/`-` without surrounding whitespace is invalid. A zero px term
/// collapses to the percentage; a zero percent term stays `calc(0% + Npx)`.
fn track_calc_size(tok: &str, fonts: &FontCtx) -> Option<TrackSize> {
    let head = tok.split('(').next()?.trim();
    if !head.eq_ignore_ascii_case("calc") || !tok.contains('%') || !calc_ops_spaced(tok) {
        return None;
    }
    let Length::Px(px) = eval_math_call_with(tok, fonts, Some(0.0))? else {
        return None;
    };
    let Length::Px(full) = eval_math_call_with(tok, fonts, Some(100.0))? else {
        return None;
    };
    let percent = full - px;
    if px == 0.0 {
        Some(TrackSize::Percent(percent))
    } else {
        Some(TrackSize::Calc { percent, px })
    }
}

fn track_calc_css(percent: f32, px: f32) -> String {
    if px == 0.0 {
        format!("{}%", css_num(percent))
    } else if px < 0.0 {
        format!("calc({}% - {}px)", css_num(percent), css_num(-px))
    } else {
        format!("calc({}% + {}px)", css_num(percent), css_num(px))
    }
}

/// `min` / `max` / `clamp` that still contains a percentage. Pure lengths
/// fold in `track_px`. Arguments stay in source order; `em` becomes px.
fn track_math_size(tok: &str, fonts: &FontCtx) -> Option<TrackSize> {
    let head = tok.split('(').next()?.trim();
    let op = if head.eq_ignore_ascii_case("min") {
        0
    } else if head.eq_ignore_ascii_case("max") {
        1
    } else if head.eq_ignore_ascii_case("clamp") {
        2
    } else {
        return None;
    };
    if !tok.contains('%') {
        return None;
    }
    let name = match op {
        0 => "min",
        1 => "max",
        _ => "clamp",
    };
    let inner = paren_fn_inner(tok, name)?;
    let src = split_top_commas(inner);
    if src.is_empty() || src.len() > 4 || (op == 2 && src.len() != 3) {
        return None;
    }
    let mut args = [(0.0f32, 0.0f32); 4];
    for (i, arg) in src.iter().enumerate() {
        args[i] = math_arg(arg, fonts)?;
    }
    Some(TrackSize::Math {
        op,
        n: src.len() as u8,
        args,
    })
}

fn math_arg(arg: &str, fonts: &FontCtx) -> Option<(f32, f32)> {
    if let Some(sz) = track_calc_size(arg, fonts) {
        return match sz {
            TrackSize::Percent(p) => Some((p, 0.0)),
            TrackSize::Calc { percent, px } => Some((percent, px)),
            TrackSize::Px(px) => Some((0.0, px)),
            _ => None,
        };
    }
    if let Some(px) = track_px(arg, fonts) {
        return Some((0.0, px));
    }
    arg.strip_suffix('%')
        .and_then(|p| p.css_f32())
        .map(|p| (p, 0.0))
}

fn math_css(op: u8, n: u8, args: [(f32, f32); 4]) -> String {
    let name = match op {
        0 => "min",
        1 => "max",
        _ => "clamp",
    };
    let body = (0..n)
        .map(|i| term_css(args[i as usize]))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{name}({body})")
}

fn term_css((percent, px): (f32, f32)) -> String {
    if percent == 0.0 {
        format!("{}px", css_num(px))
    } else if px == 0.0 {
        format!("{}%", css_num(percent))
    } else {
        track_calc_css(percent, px)
    }
}

fn split_top_commas(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                out.push(s[start..i].trim());
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    out.push(s[start..].trim());
    out.retain(|p| !p.is_empty());
    out
}

/// `+` and `-` that join terms need whitespace. A sign after `(` is fine.
fn calc_ops_spaced(tok: &str) -> bool {
    let b = tok.as_bytes();
    for i in 0..b.len() {
        if b[i] != b'+' && b[i] != b'-' {
            continue;
        }
        let prev = if i == 0 { b'(' } else { b[i - 1] };
        if !prev.is_ascii_whitespace() && prev != b'(' {
            return false;
        }
    }
    true
}

/// Absolute track length: px, em against this element's font, rem against
/// the root, viewport units, and a calc/min/max/clamp that is all lengths.
fn track_px(tok: &str, fonts: &FontCtx) -> Option<f32> {
    if let Some(n) = strip_unit(tok, "px") {
        return n.css_f32();
    }
    // `rem` before `em`: `1rem` also ends in `em`.
    if let Some(n) = strip_unit(tok, "rem") {
        return Some(n.css_f32()? * fonts.root);
    }
    if let Some(n) = strip_unit(tok, "em") {
        return Some(n.css_f32()? * fonts.own);
    }
    if let Some(n) = strip_unit(tok, "vw") {
        return Some(n.css_f32()? * fonts.viewport_w / 100.0);
    }
    if let Some(n) = strip_unit(tok, "vh") {
        return Some(n.css_f32()? * fonts.viewport_h / 100.0);
    }
    if let Some(n) = strip_unit(tok, "vmin") {
        return Some(n.css_f32()? * fonts.viewport_w.min(fonts.viewport_h) / 100.0);
    }
    if let Some(n) = strip_unit(tok, "vmax") {
        return Some(n.css_f32()? * fonts.viewport_w.max(fonts.viewport_h) / 100.0);
    }
    let head = tok.split('(').next()?.trim();
    if !matches!(
        head.to_ascii_lowercase().as_str(),
        "calc" | "min" | "max" | "clamp"
    ) || tok.contains('%')
    {
        return None;
    }
    match eval_math_call(tok, fonts)? {
        Length::Px(px) => Some(px),
        _ => None,
    }
}

/// Chrome's computed track list. `repeat()` stays. A bare zero becomes
/// `0px`, `minmax(0, 1fr)` becomes `minmax(0px, 1fr)`, and `11.5rem`
/// becomes `184px`. `0fr` and `0%` stay flex and percent. Line names stay
/// in source order; an empty `[]` is omitted.
pub(crate) fn specified_track_list(v: &str, fonts: &FontCtx) -> Option<String> {
    Some(parse_named_tracks(v, fonts)?.css)
}

/// Expanded line-name groups for a track list Chrome has already spelled.
/// Empty when every line is unnamed. Length is `tracks + 1` otherwise,
/// with empty groups kept so a name stays on its own line.
pub fn explicit_track_line_names(v: &str) -> Vec<Vec<String>> {
    match parse_named_tracks(v, &FontCtx::default()) {
        Some(n) if n.names.iter().any(|g| !g.is_empty()) => n.names,
        _ => Vec::new(),
    }
}

struct NamedTracks {
    tracks: Vec<GridTrack>,
    names: Vec<Vec<String>>,
    css: String,
}

enum TrackPiece {
    Names(Vec<String>),
    Single(GridTrack, String),
    Repeat {
        count: usize,
        inner_css: String,
        tracks: Vec<GridTrack>,
        names: Vec<Vec<String>>,
    },
}

fn parse_named_tracks(v: &str, fonts: &FontCtx) -> Option<NamedTracks> {
    let mut rest = v.trim();
    if rest.is_empty() {
        return None;
    }
    let mut pieces = Vec::new();
    while !rest.is_empty() {
        if rest.starts_with('[') {
            let (tok, tail) = next_track_token(rest)?;
            pieces.push(TrackPiece::Names(parse_line_names(tok)?));
            rest = tail.trim_start();
            continue;
        }
        let (tok, tail) = next_track_token(rest)?;
        if let Some(inner) = paren_fn_inner(tok, "repeat") {
            let (count, list) = split_first_top_comma(inner)?;
            let count = repeat_count(count)?;
            let inner = parse_named_tracks(list, fonts)?;
            pieces.push(TrackPiece::Repeat {
                count,
                inner_css: inner.css,
                tracks: inner.tracks,
                names: inner.names,
            });
        } else {
            pieces.push(TrackPiece::Single(
                parse_grid_track_token(tok, fonts)?,
                specified_track_token(tok, fonts)?,
            ));
        }
        rest = tail.trim_start();
    }
    expand_track_pieces(&pieces)
}

/// One `<line-names>?` before each track, and the names of a `repeat()`
/// collapse with the next line. Two bracket groups in a row are invalid.
fn expand_track_pieces(pieces: &[TrackPiece]) -> Option<NamedTracks> {
    let mut tracks = Vec::new();
    let mut names = vec![Vec::new()];
    let mut css = String::new();
    let mut saw_track = false;
    let mut prev_names = false;
    for piece in pieces {
        match piece {
            TrackPiece::Names(group) => {
                if prev_names {
                    return None;
                }
                if let Some(slot) = names.last_mut() {
                    slot.extend(group.iter().cloned());
                }
                push_css(&mut css, &line_names_css(group));
                prev_names = true;
            }
            TrackPiece::Single(track, track_css) => {
                tracks.push(*track);
                names.push(Vec::new());
                push_css(&mut css, track_css);
                saw_track = true;
                prev_names = false;
            }
            TrackPiece::Repeat {
                count,
                inner_css,
                tracks: inner_tracks,
                names: inner_names,
            } => {
                push_css(&mut css, &format!("repeat({count}, {inner_css})"));
                for _ in 0..*count {
                    if let Some(first) = inner_names.first() {
                        if let Some(slot) = names.last_mut() {
                            slot.extend(first.iter().cloned());
                        }
                    }
                    for (i, track) in inner_tracks.iter().enumerate() {
                        tracks.push(*track);
                        let next = inner_names.get(i + 1).cloned().unwrap_or_default();
                        names.push(next);
                    }
                }
                saw_track = true;
                prev_names = false;
            }
        }
    }
    if !saw_track {
        return None;
    }
    Some(NamedTracks { tracks, names, css })
}

fn push_css(out: &mut String, piece: &str) {
    if piece.is_empty() {
        return;
    }
    if !out.is_empty() {
        out.push(' ');
    }
    out.push_str(piece);
}

fn line_names_css(names: &[String]) -> String {
    if names.is_empty() {
        return String::new();
    }
    let mut s = String::from("[");
    for (i, name) in names.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        s.push_str(name);
    }
    s.push(']');
    s
}

/// `[a b]` → `a`, `b`. `[]` is an empty group. `span`, `auto`, and the
/// CSS-wide keywords are not line names, so the declaration drops.
fn parse_line_names(tok: &str) -> Option<Vec<String>> {
    let inner = tok.strip_prefix('[')?.strip_suffix(']')?;
    let mut names = Vec::new();
    let mut rest = inner.trim_start();
    while !rest.is_empty() {
        let (name, tail) = take_custom_ident(rest)?;
        if reserved_line_name(&name) {
            return None;
        }
        names.push(name);
        if tail.is_empty() {
            break;
        }
        if !tail.starts_with(|c: char| c.is_whitespace()) {
            return None;
        }
        rest = tail.trim_start();
    }
    Some(names)
}

fn reserved_line_name(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "span" | "auto" | "initial" | "inherit" | "unset" | "revert" | "revert-layer" | "default"
    )
}

fn take_custom_ident(s: &str) -> Option<(String, &str)> {
    if s.is_empty() {
        return None;
    }
    let bytes = s.as_bytes();
    let mut out = String::new();
    let mut i = if bytes[0] == b'-' {
        out.push('-');
        if bytes.len() > 1 && bytes[1] == b'-' {
            out.push('-');
            2
        } else {
            let (ch, n) = take_ident_unit(s, 1, true)?;
            out.push(ch);
            n
        }
    } else {
        let (ch, n) = take_ident_unit(s, 0, true)?;
        out.push(ch);
        n
    };
    while let Some((ch, n)) = take_ident_unit(s, i, false) {
        out.push(ch);
        i = n;
    }
    if out == "-" {
        return None;
    }
    Some((out, &s[i..]))
}

fn take_ident_unit(s: &str, i: usize, start: bool) -> Option<(char, usize)> {
    if i >= s.len() {
        return None;
    }
    let c = s[i..].chars().next()?;
    if c == '\\' {
        return take_css_escape(s, i);
    }
    let ok = c.is_ascii_alphabetic()
        || c == '_'
        || (c as u32) >= 0x80
        || (!start && (c.is_ascii_digit() || c == '-'));
    if ok {
        Some((c, i + c.len_utf8()))
    } else {
        None
    }
}

/// CSS escape: hex (1–6 digits, one trailing whitespace eaten) or the
/// next character. A newline after `\` is not an escape.
fn take_css_escape(s: &str, i: usize) -> Option<(char, usize)> {
    let bytes = s.as_bytes();
    if i >= bytes.len() || bytes[i] != b'\\' {
        return None;
    }
    let mut j = i + 1;
    if j >= s.len() {
        return None;
    }
    let c = s[j..].chars().next()?;
    if c == '\n' || c == '\r' || c == '\u{000c}' {
        return None;
    }
    if !c.is_ascii_hexdigit() {
        return Some((c, j + c.len_utf8()));
    }
    let mut val: u32 = 0;
    let mut digits = 0u8;
    while j < s.len() && digits < 6 {
        let ch = s[j..].chars().next()?;
        if !ch.is_ascii_hexdigit() {
            break;
        }
        val = val * 16 + ch.to_digit(16)?;
        j += ch.len_utf8();
        digits += 1;
    }
    if j < s.len() {
        let ch = s[j..].chars().next()?;
        if ch == '\r' {
            j += 1;
            if j < s.len() && s.as_bytes()[j] == b'\n' {
                j += 1;
            }
        } else if matches!(ch, ' ' | '\t' | '\n' | '\u{000c}') {
            j += ch.len_utf8();
        }
    }
    let ch = if val == 0 || val > 0x10_FFFF || (0xD800..=0xDFFF).contains(&val) {
        '\u{FFFD}'
    } else {
        char::from_u32(val).unwrap_or('\u{FFFD}')
    };
    Some((ch, j))
}

/// `grid-auto-rows` / `grid-auto-columns`: one or more track sizes.
/// `repeat()` is not a track size, so Chrome drops it.
pub(crate) fn specified_auto_tracks(v: &str, fonts: &FontCtx) -> Option<String> {
    let mut rest = v.trim();
    if rest.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    while !rest.is_empty() {
        let (tok, tail) = next_track_token(rest)?;
        if paren_fn_inner(tok, "repeat").is_some() {
            return None;
        }
        parts.push(specified_track_token(tok, fonts)?);
        rest = tail.trim_start();
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}

fn specified_track_token(tok: &str, fonts: &FontCtx) -> Option<String> {
    if let Some(inner) = paren_fn_inner(tok, "repeat") {
        let (count, list) = split_first_top_comma(inner)?;
        let count = repeat_count(count)?;
        let inner = specified_track_list(list, fonts)?;
        return Some(format!("repeat({count}, {inner})"));
    }
    if let Some(inner) = paren_fn_inner(tok, "minmax") {
        let (a, b) = split_first_top_comma(inner)?;
        let a = specified_size(a, fonts)?;
        let b = specified_size(b, fonts)?;
        return Some(format!("minmax({a}, {b})"));
    }
    specified_size(tok, fonts)
}

fn specified_size(tok: &str, fonts: &FontCtx) -> Option<String> {
    if tok.eq_ignore_ascii_case("auto") {
        return Some("auto".to_string());
    }
    if tok.eq_ignore_ascii_case("min-content") {
        return Some("min-content".to_string());
    }
    if tok.eq_ignore_ascii_case("max-content") {
        return Some("max-content".to_string());
    }
    if let Some(fr) = strip_unit(tok, "fr") {
        return Some(format!("{}fr", css_num(fr.css_f32()?)));
    }
    if let Some(sz) = track_calc_size(tok, fonts) {
        return Some(match sz {
            TrackSize::Percent(p) => format!("{}%", css_num(p)),
            TrackSize::Calc { percent, px } => track_calc_css(percent, px),
            TrackSize::Px(px) => format!("{}px", css_num(px)),
            _ => return None,
        });
    }
    if let Some(sz) = track_math_size(tok, fonts) {
        return Some(match sz {
            TrackSize::Math { op, n, args } => math_css(op, n, args),
            TrackSize::Px(px) => format!("{}px", css_num(px)),
            _ => return None,
        });
    }
    if let Some(px) = track_px(tok, fonts) {
        return Some(format!("{}px", css_num(px)));
    }
    if let Some(p) = tok.strip_suffix('%') {
        return Some(format!("{}%", css_num(p.css_f32()?)));
    }
    if unitless_zero(tok) {
        return Some("0px".to_string());
    }
    None
}

fn css_num(n: f32) -> String {
    let r = n.round();
    if (n - r).abs() < 1e-3 && r.abs() < 1.0e7 {
        format!("{}", r as i64)
    } else {
        format!("{n:.6}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

fn unitless_zero(tok: &str) -> bool {
    if tok
        .chars()
        .any(|c| c.is_ascii_alphabetic() && !matches!(c, 'e' | 'E'))
    {
        return false;
    }
    matches!(tok.css_f32(), Some(n) if n == 0.0)
}

fn strip_unit<'a>(tok: &'a str, unit: &str) -> Option<&'a str> {
    if tok.len() > unit.len() && tok[tok.len() - unit.len()..].eq_ignore_ascii_case(unit) {
        Some(&tok[..tok.len() - unit.len()])
    } else {
        None
    }
}

fn paren_fn_inner<'a>(tok: &'a str, name: &str) -> Option<&'a str> {
    if tok.len() > name.len() + 1
        && tok.as_bytes().get(name.len()) == Some(&b'(')
        && tok.ends_with(')')
        && tok[..name.len()].eq_ignore_ascii_case(name)
    {
        Some(&tok[name.len() + 1..tok.len() - 1])
    } else {
        None
    }
}

fn split_first_top_comma(s: &str) -> Option<(&str, &str)> {
    let mut depth = 0i32;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => return Some((s[..i].trim(), s[i + 1..].trim())),
            _ => {}
        }
    }
    None
}

/// `+2` is a count. Zero and counts above 1000 are invalid.
fn repeat_count(raw: &str) -> Option<usize> {
    let raw = raw.trim();
    let digits = match raw.strip_prefix('+') {
        Some(rest) if !rest.is_empty() && !rest.starts_with('+') => rest,
        Some(_) => return None,
        None => raw,
    };
    let count: usize = digits.parse().ok()?;
    if count == 0 || count > 1000 {
        None
    } else {
        Some(count)
    }
}

/// `'a b' 'c d'` → `[[a, b], [c, d]]`. Every string row must have the same
/// cell count (CSS §7.3) or the whole declaration is invalid → None.
/// Single and double quotes both work. A run of `.` is one null cell.
pub(crate) fn parse_grid_template_areas(v: &str) -> Option<Vec<Vec<String>>> {
    let mut rest = v.trim();
    if rest.is_empty() {
        return None;
    }
    let mut rows = Vec::new();
    while !rest.is_empty() {
        if !rest.starts_with('\'') && !rest.starts_with('"') {
            return None;
        }
        let (content, after) = take_css_string(rest)?;
        rows.push(area_cells(&content)?);
        rest = after.trim_start();
    }
    same_width(rows)
}

/// `grid` / `grid-template` areas form: `[s] 'a a' 20px [e] / [c] 1fr`.
/// Omitted row sizes are `auto`. No slash leaves columns at `none`.
/// A line-name group may sit before each string and after each row size.
/// The name after one row and the name before the next are the same line,
/// so they join: `[e] [m]` becomes `[e m]`.
pub(crate) struct GridAreasTemplate {
    pub rows: Vec<GridTrack>,
    pub rows_css: String,
    pub cols: Option<Vec<GridTrack>>,
    pub cols_css: Option<String>,
    pub areas: Vec<Vec<String>>,
    pub areas_css: String,
}

pub(crate) fn parse_grid_areas_template(v: &str, fonts: &FontCtx) -> Option<GridAreasTemplate> {
    let mut rest = v.trim();
    if rest.is_empty() {
        return None;
    }
    let mut areas = Vec::new();
    let mut rows = Vec::new();
    let mut row_css = Vec::new();
    let mut leads = Vec::new();
    let mut trails = Vec::new();
    while !rest.is_empty() && !rest.starts_with('/') {
        let (lead, after_lead) = take_optional_names(rest)?;
        if after_lead.starts_with('[') {
            return None;
        }
        if !after_lead.starts_with('\'') && !after_lead.starts_with('"') {
            return None;
        }
        let (content, after) = take_css_string(after_lead)?;
        areas.push(area_cells(&content)?);
        rest = after.trim_start();
        let (track, css, after_size) = if rest.is_empty()
            || rest.starts_with('/')
            || rest.starts_with('\'')
            || rest.starts_with('"')
            || rest.starts_with('[')
        {
            (GridTrack::Auto, "auto".to_string(), rest)
        } else {
            let (tok, tail) = next_track_token(rest)?;
            (
                parse_grid_track_token(tok, fonts)?,
                specified_track_list(tok, fonts)?,
                tail.trim_start(),
            )
        };
        let (trail, after_trail) = take_optional_names(after_size)?;
        // A second bracket group is the next row's leading names when a
        // string follows it. Anything else is two groups on one line.
        if after_trail.starts_with('[') {
            let (_, after_next) = take_optional_names(after_trail)?;
            if !after_next.starts_with('\'') && !after_next.starts_with('"') {
                return None;
            }
        }
        leads.push(lead);
        trails.push(trail);
        rows.push(track);
        row_css.push(css);
        rest = after_trail;
    }
    if rows.is_empty() {
        return None;
    }
    let mut name_lines = Vec::with_capacity(rows.len() + 1);
    name_lines.push(leads[0].clone());
    for i in 0..rows.len() - 1 {
        let mut line = trails[i].clone();
        line.extend(leads[i + 1].iter().cloned());
        name_lines.push(line);
    }
    name_lines.push(trails[rows.len() - 1].clone());
    let areas = same_width(areas)?;
    let (cols, cols_css) = if let Some(after) = rest.strip_prefix('/') {
        let cols = after.trim();
        if cols.is_empty() || cols.eq_ignore_ascii_case("none") {
            return None;
        }
        (
            Some(parse_grid_tracks(cols, fonts)?),
            Some(specified_track_list(cols, fonts)?),
        )
    } else if rest.is_empty() {
        (None, None)
    } else {
        return None;
    };
    Some(GridAreasTemplate {
        rows,
        rows_css: named_track_css(&name_lines, &row_css),
        cols,
        cols_css,
        areas_css: grid_areas_css(&areas),
        areas,
    })
}

fn take_optional_names(rest: &str) -> Option<(Vec<String>, &str)> {
    let rest = rest.trim_start();
    if !rest.starts_with('[') {
        return Some((Vec::new(), rest));
    }
    let (tok, tail) = next_track_token(rest)?;
    Some((parse_line_names(tok)?, tail.trim_start()))
}

fn named_track_css(names: &[Vec<String>], tracks: &[String]) -> String {
    let mut parts = Vec::new();
    for (i, track) in tracks.iter().enumerate() {
        let lead = line_names_css(names.get(i).map(Vec::as_slice).unwrap_or(&[]));
        if !lead.is_empty() {
            parts.push(lead);
        }
        parts.push(track.clone());
    }
    if let Some(tail) = names.get(tracks.len()) {
        let tail = line_names_css(tail);
        if !tail.is_empty() {
            parts.push(tail);
        }
    }
    parts.join(" ")
}

/// Chrome's computed `grid-template-areas`: double quotes, one space
/// between cells and between rows. `"a a" "b b"`.
pub(crate) fn grid_areas_css(areas: &[Vec<String>]) -> String {
    areas
        .iter()
        .map(|row| format!("\"{}\"", row.join(" ")))
        .collect::<Vec<_>>()
        .join(" ")
}

fn same_width(rows: Vec<Vec<String>>) -> Option<Vec<Vec<String>>> {
    let width = rows.first()?.len();
    if width == 0 || rows.iter().any(|r| r.len() != width) {
        None
    } else {
        Some(rows)
    }
}

fn area_cells(content: &str) -> Option<Vec<String>> {
    let cells: Vec<String> = content
        .split_whitespace()
        .map(|tok| {
            if tok.chars().all(|c| c == '.') {
                ".".to_string()
            } else {
                tok.to_string()
            }
        })
        .collect();
    if cells.is_empty() {
        None
    } else {
        Some(cells)
    }
}

/// One CSS string, quote included at `v`. The returned tail starts after
/// the closing quote. A raw newline ends the string and is invalid.
fn take_css_string(v: &str) -> Option<(String, &str)> {
    let mut chars = v.char_indices();
    let quote = chars.next()?.1;
    if quote != '\'' && quote != '"' {
        return None;
    }
    let mut out = String::new();
    while let Some((i, c)) = chars.next() {
        if c == '\\' {
            out.push(chars.next()?.1);
            continue;
        }
        if c == quote {
            return Some((out, &v[i + c.len_utf8()..]));
        }
        if c == '\n' || c == '\r' {
            return None;
        }
        out.push(c);
    }
    None
}

pub(crate) fn set_side(sides: &mut Sides, name: &str, value: Option<Length>) {
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
pub(crate) fn expand_sides(value: &str, fonts: &FontCtx) -> Sides {
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
pub(crate) fn split_sides(value: &str) -> Vec<&str> {
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
pub(crate) fn sides_no_auto(s: &Sides) -> Option<Sides> {
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
pub(crate) fn parse_px_f32(v: &str) -> Option<f32> {
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
pub(crate) fn parse_font_size_len(v: &str) -> Option<CssLength> {
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
pub(crate) fn font_size_px(
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
