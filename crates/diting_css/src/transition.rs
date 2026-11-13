//! Transitions, easing, and the individual translate/rotate/scale props.
//!
//! Split from the css hub. The public names stay re-exported from the
//! crate root. Pure move.

use super::*;

/// Parsed `animation` shorthand, v1 subset: ONE animation (the first of a
/// comma list), direction normal, play-state running. That covers the
/// declarative-SVG motion grammar (fade / rise / self-draw); the sampler
/// cycles through `iterations` iterations and holds the element at the final
/// iteration's state once past `delay + duration * iterations` when fill-mode
/// is `forwards`/`both`.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimationSpec {
    pub name: String,
    /// Seconds; 0 means the animation snaps (only end states ever visible).
    pub duration: f32,
    /// Seconds before the animation starts; before it the element shows its
    /// underlying cascade value (fill-mode none semantics).
    pub delay: f32,
    pub easing: Easing,
    pub fill_forwards: bool,
    /// Iteration count; `f32::INFINITY` for `infinite`. Non-integer counts
    /// are legal (the active duration ends mid-iteration). The sampler loops
    /// per-iteration (easing restarts each cycle) and the after-phase fill
    /// holds the fractional-final-iteration state.
    pub iterations: f32,
}

/// Timing functions the sampler evaluates. Keywords map to their canonical
/// cubic-bezier control points; `steps()` invalidates the declaration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Easing {
    Linear,
    CubicBezier(f32, f32, f32, f32),
}

/// One comma entry of the `transition` shorthand, shared shape with
/// AnimationSpec: `transition-property` (`None` = all), duration and delay
/// in seconds, timing function. Like animation lists, a comma-separated
/// transition list is tracked by its first entry only.
#[derive(Debug, Clone, PartialEq)]
pub struct TransitionSpec {
    pub property: Option<String>,
    pub duration: f32,
    pub delay: f32,
    pub easing: Easing,
}

/// `linear` / `ease` family keywords and `cubic-bezier(...)`. `steps()` and
/// other timing functions we don't evaluate return None (the caller
/// invalidates the whole declaration rather than animating with the wrong
/// curve). Shared by the animation and transition shorthands, and by the
/// `add_css_transition` op (the JS face passes the easing token through).
pub fn parse_easing_token(tok: &str) -> Option<Easing> {
    match tok {
        "linear" => Some(Easing::Linear),
        "ease" => Some(Easing::CubicBezier(0.25, 0.1, 0.25, 1.0)),
        "ease-in" => Some(Easing::CubicBezier(0.42, 0.0, 1.0, 1.0)),
        "ease-out" => Some(Easing::CubicBezier(0.0, 0.0, 0.58, 1.0)),
        "ease-in-out" => Some(Easing::CubicBezier(0.42, 0.0, 0.58, 1.0)),
        _ => {
            let rest = tok.strip_prefix("cubic-bezier(")?;
            let args: Vec<Option<f32>> = rest
                .strip_suffix(')')?
                .split(',')
                .map(|a| a.trim().css_f32())
                .collect();
            let pts: Option<Vec<f32>> = args.into_iter().collect();
            let pts = pts?;
            if pts.len() == 4 && pts.iter().all(|p| p.is_finite()) {
                Some(Easing::CubicBezier(pts[0], pts[1], pts[2], pts[3]))
            } else {
                None
            }
        }
    }
}

/// `transition` shorthand: `<property> <duration> <easing> <delay>` in any
/// order, times by position (first time = duration, second = delay).
/// `transition-property: none` disables the entry (returns None); `steps()`
/// invalidates the whole shorthand. Comma lists collapse to the first entry.
pub(super) fn parse_transition_shorthand(v: &str) -> Option<TransitionSpec> {
    let mut duration: Option<f32> = None;
    let mut delay = 0.0f32;
    let mut easing: Option<Easing> = None;
    let mut property: Option<String> = None;
    for tok in paren_aware_tokens(v) {
        let secs = tok
            .strip_suffix("ms")
            .and_then(|n| n.css_f32().map(|n| n / 1000.0))
            .or_else(|| tok.strip_suffix('s').and_then(|n| n.css_f32()));
        if let Some(s) = secs.filter(|s| s.is_finite() && *s >= 0.0) {
            if duration.is_none() {
                duration = Some(s);
            } else if delay == 0.0 {
                delay = s;
            }
            continue;
        }
        if let Some(e) = parse_easing_token(&tok) {
            easing = Some(e);
            continue;
        }
        if tok.contains('(') {
            return None;
        }
        if tok
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
            && !tok.is_empty()
        {
            match property {
                None if tok == "none" => return None,
                None => property = Some(tok),
                _ => {}
            }
        }
    }
    Some(TransitionSpec {
        property,
        duration: duration.unwrap_or(0.0),
        delay,
        easing: easing.unwrap_or(Easing::CubicBezier(0.25, 0.1, 0.25, 1.0)),
    })
}

/// The interpolable value set: opacity scalars, RGBA colors, and 2D
/// affines (batch 93; the affine lerps per component via `lerp_transform`,
/// the same path CSS keyframe animations use).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransitionValue {
    Opacity(f32),
    Color([f32; 4]),
    Transform(Option<Transform2D>),
}

impl TransitionValue {
    /// Pairwise lerp. Mismatched kinds (never produced by the register
    /// op, which snapshots one property) yield None and the caller snaps
    /// to the end value.
    pub fn lerp(a: TransitionValue, b: TransitionValue, p: f32) -> Option<TransitionValue> {
        match (a, b) {
            (TransitionValue::Opacity(x), TransitionValue::Opacity(y)) => {
                Some(TransitionValue::Opacity(x + (y - x) * p))
            }
            (TransitionValue::Color(x), TransitionValue::Color(y)) => {
                let mix = |i: usize| x[i] + (y[i] - x[i]) * p;
                Some(TransitionValue::Color([mix(0), mix(1), mix(2), mix(3)]))
            }
            (TransitionValue::Transform(x), TransitionValue::Transform(y)) => {
                lerp_transform(x, y, p).map(|t| TransitionValue::Transform(Some(t)))
            }
            _ => None,
        }
    }
}

/// One registered property transition (CSS transitions batch): the JS
/// face detects a tracked-property style write, snapshots both computed
/// values, and registers one of these through the `add_css_transition`
/// op. `start` is page-timeline seconds; the /video path collapses it to
/// 0 so clips play transitions from their outset.
#[derive(Debug, Clone)]
pub struct CssTransition {
    pub nid: usize,
    pub property: String,
    pub from: TransitionValue,
    pub to: TransitionValue,
    pub start: f64,
    pub duration: f32,
    pub delay: f32,
    pub easing: Easing,
}

/// Exit-phase transition sampling (CSS transitions batch). Runs once per
/// style pass after the cascade visit: registered entries override the
/// resolved values. Per-node sampling inside the visit would instead let
/// a transitioning parent drag its whole subtree through the interpolated
/// value (children cascade the parent) — v1 collapses that corner and
/// keeps children on the pre-transition parent value. `t = None` is the
/// live path and no-ops: the style write already carries the end state,
/// so the cascade shows it and a registered entry would only re-derive
/// the same numbers.
pub fn sample_css_transitions(
    transitions: &[CssTransition],
    t: Option<f64>,
    styles: &mut std::collections::HashMap<diting_dom::NodeId, ComputedStyle>,
) {
    let Some(t) = t else { return };
    for tr in transitions {
        let Some(cs) = styles.get_mut(&diting_dom::NodeId(tr.nid as u32)) else {
            continue;
        };
        let elapsed = t - tr.start - tr.delay as f64;
        let p = if tr.duration <= 0.0 {
            if elapsed >= 0.0 {
                1.0
            } else {
                0.0
            }
        } else {
            ((elapsed / tr.duration as f64).clamp(0.0, 1.0)) as f32
        };
        let eased = tr.easing.map(p);
        let Some(v) = TransitionValue::lerp(tr.from, tr.to, eased) else {
            continue;
        };
        match (tr.property.as_str(), v) {
            ("opacity", TransitionValue::Opacity(o)) => cs.opacity = Some(o),
            ("color", TransitionValue::Color(c)) => {
                cs.color = Some(to_color(c));
            }
            ("background-color", TransitionValue::Color(c)) => {
                cs.background_color = Some(to_color(c));
            }
            ("transform", TransitionValue::Transform(m)) => {
                cs.transform = m;
            }
            _ => {}
        }
    }
}

fn to_color(c: [f32; 4]) -> Color {
    let ch = |x: f32| -> u8 { (x.round().clamp(0.0, 255.0)) as u8 };
    Color(ch(c[0]), ch(c[1]), ch(c[2]), ch(c[3]))
}

impl Easing {
    /// Map linear progress [0, 1] through the curve. `ease`-family curves
    /// stay monotone in x, so one Newton pass with a bisection fallback
    /// solves the parameter; a few iterations suffice at f32 precision.
    pub fn map(&self, p: f32) -> f32 {
        match *self {
            Easing::Linear => p,
            Easing::CubicBezier(x1, y1, x2, y2) => {
                let x = p.clamp(0.0, 1.0);
                let bezier = |t: f32, a: f32, b: f32| {
                    // Bernstein form of 3(1-t)^2 t a + 3(1-t) t^2 b + t^3
                    let u = 1.0 - t;
                    3.0 * u * u * t * a + 3.0 * u * t * t * b + t * t * t
                };
                let mut t = x;
                let mut solved = false;
                for _ in 0..8 {
                    let xt = bezier(t, x1, x2) - x;
                    if xt.abs() < 1e-5 {
                        solved = true;
                        break;
                    }
                    // derivative of the x component
                    let u = 1.0 - t;
                    let d = 3.0 * u * u * x1 + 6.0 * u * t * (x2 - x1) + 3.0 * t * t * (1.0 - x2);
                    if d.abs() < 1e-6 {
                        break;
                    }
                    t -= xt / d;
                }
                if !solved && !(0.0..=1.0).contains(&t) {
                    // Newton diverged (degenerate control points): bisect
                    let (mut lo, mut hi) = (0.0f32, 1.0f32);
                    for _ in 0..24 {
                        t = 0.5 * (lo + hi);
                        let xt = bezier(t, x1, x2);
                        if xt < x {
                            lo = t;
                        } else {
                            hi = t;
                        }
                    }
                }
                bezier(t.clamp(0.0, 1.0), y1, y2)
            }
        }
    }
}

/// Split on whitespace at paren depth 0 so `cubic-bezier(.33, 1, .68, 1)`
/// stays one token.
pub(super) fn paren_aware_tokens(v: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0usize;
    for c in v.chars() {
        match c {
            '(' => {
                depth += 1;
                cur.push(c);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                cur.push(c);
            }
            _ if c.is_whitespace() && depth == 0 => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Length arithmetic for transform accumulation: same-unit adds fold, a
/// zero px collapses into the other side; mixed units without a box to
/// resolve against can't compose and invalidate the whole list.
pub(super) fn add_len(a: Length, b: Length) -> Option<Length> {
    match (a, b) {
        (Length::Px(x), Length::Px(y)) => Some(Length::Px(x + y)),
        (Length::Percent(x), Length::Percent(y)) => Some(Length::Percent(x + y)),
        (Length::Px(0.0), b) => Some(b),
        (a, Length::Px(0.0)) => Some(a),
        _ => None,
    }
}

/// Scale a Length by a factor: px scales numerically, and a percent of the
/// box times the factor is the same percent scaled (w·f·s == Percent(f·s)).
pub(super) fn mul_len(a: Length, s: f32) -> Length {
    match a {
        Length::Px(x) => Length::Px(x * s),
        Length::Percent(f) => Length::Percent(f * s),
        other => other,
    }
}

/// Angle in CSS units: deg (the default suffix real sheets write), rad,
/// turn, grad. Bare nonzero numbers are invalid CSS (Chrome rejects).
pub(super) fn parse_angle_deg(s: &str) -> Option<f32> {
    let s = s.trim();
    let (n, scale) = if let Some(n) = s.strip_suffix("deg") {
        (n, 1.0f32)
    } else if let Some(n) = s.strip_suffix("rad") {
        (n, 180.0 / std::f32::consts::PI)
    } else if let Some(n) = s.strip_suffix("turn") {
        (n, 360.0)
    } else if let Some(n) = s.strip_suffix("grad") {
        (n, 0.9)
    } else if s == "0" {
        ("0", 1.0)
    } else {
        return None;
    };
    let deg = n.trim().css_f32()? * scale;
    deg.is_finite().then_some(deg)
}

/// `translate: <x> [y]` — the individual property (#140, css-transforms-2).
/// Slots share the transform-list translate grammar (px/percent/0, viewport
/// units fold at parse time); a third z component is 3D and drops the
/// declaration, same 2D posture as the list. `none` is the initial value.
pub(super) fn parse_translate_prop(v: &str, vw: f32, vh: f32) -> Option<(Length, Length)> {
    let v = v.trim();
    if v.eq_ignore_ascii_case("none") {
        return None;
    }
    let one = |s: &str| -> Option<Length> {
        let s = s.trim();
        if let Some(num) = s.strip_suffix('%') {
            num.trim().css_f32().map(Length::Percent)
        } else if let Some(num) = s.strip_suffix("px") {
            num.trim().css_f32().map(Length::Px)
        } else if let Some(num) = s.strip_suffix("vw") {
            num.trim().css_f32().map(|n| Length::Px(n * vw / 100.0))
        } else if let Some(num) = s.strip_suffix("vh") {
            num.trim().css_f32().map(|n| Length::Px(n * vh / 100.0))
        } else {
            (s == "0").then_some(Length::Px(0.0))
        }
    };
    let mut parts = v.split_whitespace();
    let x = one(parts.next()?)?;
    let y = match parts.next() {
        Some(tok) => one(tok)?,
        None => Length::Px(0.0),
    };
    if parts.next().is_some() {
        return None; // z component: 3D, out of v1 scope
    }
    Some((x, y))
}

/// `rotate: <angle>` — Z-axis only in v1; the axis forms (`x 45deg`) are 3D
/// and drop the declaration. `none` is the initial value.
pub(super) fn parse_rotate_prop(v: &str) -> Option<f32> {
    let v = v.trim();
    if v.eq_ignore_ascii_case("none") {
        return None;
    }
    if v.contains(' ') {
        return None;
    }
    parse_angle_deg(v)
}

/// `scale: <x> [y]` — single value is uniform; negatives are legal (the
/// flip idiom `scale: -1` relies on them). `none` is the initial value.
pub(super) fn parse_scale_prop(v: &str) -> Option<(f32, f32)> {
    let v = v.trim();
    if v.eq_ignore_ascii_case("none") {
        return None;
    }
    let num = |s: &str| s.trim().css_f32().filter(|n| n.is_finite());
    let mut parts = v.split_whitespace();
    let x = num(parts.next()?)?;
    let y = match parts.next() {
        Some(tok) => num(tok)?,
        None => x,
    };
    if parts.next().is_some() {
        return None; // z component: 3D, out of v1 scope
    }
    Some((x, y))
}
