// SVG 路径度量：path_len/point_at（screenshot 真测 + 空桩双版本） — split out of the god file (ratchet, #75).
// Pure move: the functions see the same ops::* namespace they grew up in.
use super::*;

/// 批240 / obscura#1100: SVGGeometryElement.getTotalLength() backing. Chrome
/// measures through SkPathMeasure (circle r=25 → 156.0674, not 2πr), so these
/// numbers line up with Chrome (conic chord measure for ovals, analytic for
/// explicit d beziers). Non-geometry nodes / missing geometry read 0.
/// Anti-panic boundary: the path data comes from the page, and a parser panic
/// must degrade to 0 (the op_dom posture) rather than unwind through the FFI
/// frame. Measurement lives in the layout crate, so the no-screenshot build
/// reads 0 — geometry measurement without the layout engine is meaningless
/// anyway.
#[cfg(feature = "screenshot")]
#[diting_op]
pub(super) fn op_svg_path_len(state: &OpState, #[smi] nid: u32) -> f64 {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        svg_measure(state, nid)
            .map(|m| m.total() as f64)
            .unwrap_or(0.0)
    }))
    .unwrap_or(0.0)
}

#[cfg(not(feature = "screenshot"))]
#[diting_op]
pub(super) fn op_svg_path_len(_: &OpState, #[smi] _: u32) -> f64 {
    0.0
}

/// getPointAtLength backing — "x\0y" (cheap unambiguous pair; the bootstrap
/// side splits and wraps it in an SVGPoint). Negative/overlong targets
/// clamp to the endpoints, Chrome's contract.
#[cfg(feature = "screenshot")]
#[diting_op]
#[string]
pub(super) fn op_svg_path_point(state: &OpState, #[smi] nid: u32, target: f64) -> String {
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        svg_measure(state, nid).map(|m| m.point_at(target as f32))
    }))
    .unwrap_or(None);
    match r {
        Some((x, y)) if x.is_finite() && y.is_finite() => format!("{x}\0{y}"),
        _ => "0\u{0}0".to_string(),
    }
}

#[cfg(not(feature = "screenshot"))]
#[diting_op]
#[string]
pub(super) fn op_svg_path_point(_: &OpState, #[smi] _: u32, _: f64) -> String {
    "0\u{0}0".to_string()
}

#[cfg(feature = "screenshot")]
pub(super) fn svg_measure(state: &OpState, nid: u32) -> Option<diting_layout::svg::PathMeasure> {
    let gs = state.borrow::<SharedState>().clone();
    let gs = gs.borrow();
    // `&gs.dom?` is the lint's shape but moves out of the RefCell borrow;
    // `as_ref()` keeps the Option borrowed.
    let dom = gs.dom.as_ref()?;
    let node = dom.get_node(NodeId::new(nid))?;
    let tag = match &node.data {
        diting_dom::NodeData::Element { name, .. } => name.local.as_str(),
        _ => return None,
    };
    diting_layout::svg::build_path_measure(tag, &|a| node.get_attribute(a).map(|s| s.to_string()))
}
