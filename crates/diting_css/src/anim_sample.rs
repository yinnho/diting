//! Keyframe sampling onto a computed style.
//!
//! Split from the css hub. `sample_css_animation` stays re-exported from
//! the crate root. Pure move.

use super::*;

/// Resolve one element's CSS animation against the stylesheet's keyframes
/// table at time `t` (seconds since document start), mutating the computed
/// style in place. `t: None` means "no clock" — static renders (screenshot,
/// PDF) sample the END state, so an animated SVG's poster frame is the
/// finished diagram rather than the blank t=0 one; the engine has no wall
/// clock and the end state is the only deterministic poster.
///
/// v1 interpolates the paint-channel properties (opacity, transform,
/// stroke-dashoffset — the fade/rise/self-draw grammar); other properties
/// inside keyframes are ignored. Before the delay the element keeps its
/// underlying cascade value (fill-mode none semantics); after the end it
/// holds the final stop when fill-mode is `forwards`/`both`.
pub fn sample_css_animation(style: &mut ComputedStyle, keyframes: &KeyframesMap, t: Option<f64>) {
    let Some(anim) = style.animation.clone() else {
        return;
    };
    let Some(kf) = keyframes.get(&anim.name) else {
        return;
    };
    let progress = match t {
        None => 1.0,
        Some(t) => {
            let elapsed = t as f32 - anim.delay;
            // Strict: t == delay is the first instant of the active phase
            // (progress 0, from-stop applies), not the pre-delay hold.
            if elapsed < 0.0 {
                return;
            }
            // Active duration spans iteration-count cycles (CSS Animations 1).
            // `infinite` multiplies out to +inf, so the animation never ends;
            // a zero count never enters the active phase at all.
            let active = anim.duration * anim.iterations;
            if anim.duration <= 0.0 || elapsed >= active {
                if !anim.fill_forwards {
                    return;
                }
                // After-phase fill holds the final iteration's state (Web
                // Animations after-phase iteration progress): the fractional
                // part of the count, or the 100% keyframe when it lands on a
                // whole iteration (a zero count holds the 0% one).
                if anim.iterations == 0.0 {
                    0.0
                } else if anim.iterations.fract() == 0.0 {
                    1.0
                } else {
                    anim.iterations.fract()
                }
            } else {
                // Easing transforms progress within each iteration, so it
                // restarts every cycle.
                anim.easing.map((elapsed / anim.duration) % 1.0)
            }
        }
    };
    apply_stops(style, kf, progress.clamp(0.0, 1.0));
}

fn lerp(a: f32, b: f32, l: f32) -> f32 {
    a + (b - a) * l
}

/// A keyframe transform value, keeping "stop lacks the property" (use the
/// underlying transform) apart from `transform: none` (identity).
enum StopTransform {
    Absent,
    Identity,
    T(Transform2D),
}

fn stop_transform(
    stop: &KeyframeStop,
    custom: &std::collections::HashMap<String, String>,
) -> StopTransform {
    let Some((_, raw)) = stop.decls.iter().rev().find(|(k, _)| k == "transform") else {
        return StopTransform::Absent;
    };
    let sub = substitute_vars(raw, custom, 0).unwrap_or_else(|| raw.clone());
    if sub.trim() == "none" {
        return StopTransform::Identity;
    }
    parse_transform(&sub)
        .map(StopTransform::T)
        .unwrap_or(StopTransform::Absent)
}

fn stop_num(
    stop: &KeyframeStop,
    prop: &str,
    custom: &std::collections::HashMap<String, String>,
) -> Option<f32> {
    let (_, raw) = stop.decls.iter().rev().find(|(k, _)| k == prop)?;
    let sub = substitute_vars(raw, custom, 0).unwrap_or_else(|| raw.clone());
    sub.trim().css_f32().filter(|n| n.is_finite())
}

fn lerp_len(a: Length, b: Length, l: f32) -> Length {
    match (a, b) {
        (Length::Px(x), Length::Px(y)) => Length::Px(lerp(x, y, l)),
        (Length::Percent(x), Length::Percent(y)) => Length::Percent(lerp(x, y, l)),
        // Mixed units have no box to resolve against mid-flight: snap to
        // the nearer side (discrete interpolation).
        _ => {
            if l < 0.5 {
                a
            } else {
                b
            }
        }
    }
}

/// Interpolate two transforms. `None` on a side is identity for this
/// purpose (`transform: none` in a `to` stop); the mixed-unit lengths snap
/// discretely, everything else lerps componentwise.
pub(super) fn lerp_transform(
    a: Option<Transform2D>,
    b: Option<Transform2D>,
    l: f32,
) -> Option<Transform2D> {
    let mix_to_identity = |x: &Transform2D, l: f32| Transform2D {
        a: lerp(x.a, 1.0, l),
        b: lerp(x.b, 0.0, l),
        c: lerp(x.c, 0.0, l),
        d: lerp(x.d, 1.0, l),
        tx: lerp_len(x.tx, Length::Px(0.0), l),
        ty: lerp_len(x.ty, Length::Px(0.0), l),
    };
    match (a, b) {
        (None, None) => None,
        (Some(x), None) => Some(mix_to_identity(&x, l)),
        (None, Some(y)) => Some(mix_to_identity(&y, 1.0 - l)),
        (Some(x), Some(y)) => Some(Transform2D {
            a: lerp(x.a, y.a, l),
            b: lerp(x.b, y.b, l),
            c: lerp(x.c, y.c, l),
            d: lerp(x.d, y.d, l),
            tx: lerp_len(x.tx, y.tx, l),
            ty: lerp_len(x.ty, y.ty, l),
        }),
    }
}

/// Apply the keyframe bracket around `p` to the three animated properties,
/// with the element's underlying cascade values as the fallback for stops
/// that omit them (CSS missing-keyframe semantics).
fn apply_stops(style: &mut ComputedStyle, kf: &Keyframes, p: f32) {
    let stops = &kf.stops;
    if stops.is_empty() {
        return;
    }
    let mut i0 = 0;
    for (i, s) in stops.iter().enumerate() {
        if s.offset <= p {
            i0 = i;
        } else {
            break;
        }
    }
    let (local, s1) = match stops.get(i0 + 1) {
        Some(next) => {
            let span = next.offset - stops[i0].offset;
            let local = if span <= 0.0 {
                1.0
            } else {
                ((p - stops[i0].offset) / span).clamp(0.0, 1.0)
            };
            (local, next)
        }
        None => (1.0, &stops[i0]),
    };
    let s0 = &stops[i0];

    let base_opacity = style.opacity.unwrap_or(1.0);
    let v0 = stop_num(s0, "opacity", &style.custom).unwrap_or(base_opacity);
    let v1 = stop_num(s1, "opacity", &style.custom).unwrap_or(base_opacity);
    style.opacity = Some(lerp(v0, v1, local).clamp(0.0, 1.0));

    let base_dash = style.svg_dashoffset.unwrap_or(0.0);
    let d0 = stop_num(s0, "stroke-dashoffset", &style.custom).unwrap_or(base_dash);
    let d1 = stop_num(s1, "stroke-dashoffset", &style.custom).unwrap_or(base_dash);
    style.svg_dashoffset = Some(lerp(d0, d1, local));

    let underlying = style.transform;
    let resolve = |st: StopTransform| match st {
        StopTransform::Absent => underlying,
        StopTransform::Identity => None,
        StopTransform::T(t) => Some(t),
    };
    let t0 = resolve(stop_transform(s0, &style.custom));
    let t1 = resolve(stop_transform(s1, &style.custom));
    style.transform = lerp_transform(t0, t1, local);
}
