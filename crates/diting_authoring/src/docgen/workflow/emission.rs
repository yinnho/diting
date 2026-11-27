//! SVG emission for the workflow family.
//!
//! Kept out of the workflow compiler file: layout/routing semantics live in
//! `workflow.rs`; this module turns the already-measured scene into markup.

use super::super::svgutil::{esc, fitted_font};
use super::*;

/// (stroke, width, dash, label fill) per edge variant — the sequence table,
/// with the grid families' plain default (no accent label). Sizes and dash
/// patterns are this adapter's typography; colors come from the theme.
fn variant_style(
    theme: &Theme,
    variant: &str,
) -> (&'static str, i32, Option<&'static str>, &'static str) {
    match variant {
        "emphasis" => (theme.ink, 18, None, theme.ink),
        "security" => (theme.danger, 14, None, theme.danger),
        "dashed" => (theme.skip, 14, Some("6 4"), theme.skip),
        "return" => (theme.ink_muted, 14, Some("3 5"), theme.ink_muted),
        _ => (theme.ink_soft, 14, None, theme.ink_soft),
    }
}

pub(super) fn emit_svg(
    doc: &Canon,
    laid: &Laid,
    placed: &[(usize, usize, Vec<Pt>, Option<Rect>)],
    view_box: [i32; 2],
    legend_rows: &[Vec<(i32, i32, usize)>],
    legend_y: i32,
    theme: &'static Theme,
) -> String {
    let mut s = String::with_capacity(8192);
    s.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" role=\"img\" aria-label=\"{}\">",
        tx(view_box[0]),
        tx(view_box[1]),
        esc(doc.title)
    ));

    // Lanes.
    for (i, lane) in doc.lanes.iter().enumerate() {
        let y = laid.lane_tops[i];
        let h = laid.lane_heights[i];
        let prefix = if lane.variant.as_deref() == Some("exception") {
            "EX".to_string()
        } else {
            format!("{:02}", i + 1)
        };
        let exception = lane.variant.as_deref() == Some("exception");
        s.push_str(&format!(
            "<rect data-lane-id=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"10\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1\"/>",
            esc(&lane.id),
            tx(LANE_X),
            tx(y),
            tx(laid.lane_w),
            tx(h),
            theme.panel,
            if exception { theme.danger } else { theme.frame }
        ));
        s.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" font-size=\"10\" font-weight=\"600\" fill=\"{}\">{} / {}</text>",
            tx(LANE_X + 140),
            tx(y + 220),
            if exception { theme.danger } else { theme.ink_soft },
            esc(&prefix),
            esc(&lane.label)
        ));
    }

    // Phases.
    for phase in &doc.phases {
        let (x, width) = phase_span(phase, &laid.col_xs);
        let accent = if phase.variant.as_deref() == Some("security") {
            theme.danger
        } else {
            theme.ink_soft
        };
        s.push_str(&format!(
            "<line x1=\"{}\" y1=\"35\" x2=\"{}\" y2=\"35\" stroke=\"{}\" stroke-width=\"1.1\"/>",
            tx(x),
            tx(x + width),
            theme.ink_muted
        ));
        s.push_str(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"4\" fill=\"{}\"/>",
            tx(x),
            tx(PHASE_BAND_Y),
            tx(width),
            tx(PHASE_BAND_H),
            theme.panel_alt
        ));
        s.push_str(&format!(
            "<text x=\"{}\" y=\"39\" font-size=\"8\" font-weight=\"600\" fill=\"{}\" text-anchor=\"middle\">{}</text>",
            tx(x + width / 2),
            accent,
            esc(&phase.label)
        ));
    }

    // Groups.
    for group in &doc.groups {
        let security = group.variant.as_deref() == Some("security");
        let Rect {
            x: gx,
            y: fy,
            w: gw,
            h: gh,
        } = group_rect(doc, group, laid);
        s.push_str(&format!(
            "<rect data-group-id=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"9\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1\"/>",
            esc(&group.id),
            tx(gx),
            tx(fy),
            tx(gw),
            tx(gh),
            if security { theme.panel_danger } else { theme.panel_alt },
            if security { theme.danger } else { theme.guide }
        ));
        s.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" font-size=\"7\" font-weight=\"600\" fill=\"{}\">{}</text>",
            tx(gx + GROUP_LABEL_X_INSET),
            tx(fy + GROUP_LABEL_BASELINE_OFFSET),
            if security {
                theme.danger
            } else {
                theme.ink_soft
            },
            esc(&group.label)
        ));
    }

    // Edge paths + arrowheads.
    for (i, edge) in doc.edges.iter().enumerate() {
        let (_, _, points, _) = &placed[i];
        let variant = edge.variant.as_deref().unwrap_or("default");
        let (stroke, stroke_w, dash, _) = variant_style(theme, variant);
        let dash_attr = dash.map_or(String::new(), |d| format!(" stroke-dasharray=\"{d}\""));
        s.push_str(&format!(
            "<path data-from=\"{}\" data-to=\"{}\" d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"{}/>",
            esc(&edge.from),
            esc(&edge.to),
            polyline_d(points),
            stroke,
            tx(stroke_w),
            dash_attr
        ));
        s.push_str(&arrowhead(points, stroke));
    }

    // Nodes.
    for (i, node) in doc.nodes.iter().enumerate() {
        let rect = &laid.rects[i];
        let node_colors = theme.node(&node.kind);
        s.push_str(&format!("<g data-node-id=\"{}\">", esc(&node.id)));
        s.push_str(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"6\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1.5\"/>",
            tx(rect.x),
            tx(rect.y),
            tx(rect.w),
            tx(rect.h),
            node_colors.fill,
            node_colors.stroke
        ));
        s.push_str(&sigil::sigil(&node.kind, rect.x + 60, rect.y + 60, theme));
        let label_font = fitted_font(
            &node.label,
            NODE_W - LABEL_RESERVE,
            LABEL_PREFERRED,
            LABEL_MIN,
        );
        s.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" font-size=\"{}\" font-weight=\"600\" fill=\"{}\" text-anchor=\"middle\">{}</text>",
            tx(rect.cx()),
            tx(rect.y + 210),
            tx(label_font),
            theme.ink,
            esc(&node.label)
        ));
        if let Some(sub) = node.sublabel.as_deref().filter(|s| !s.trim().is_empty()) {
            let sub_font = fitted_font(sub, NODE_W, SUB_PREFERRED, SUB_MIN);
            s.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" font-size=\"{}\" fill=\"{}\" text-anchor=\"middle\">{}</text>",
                tx(rect.cx()),
                tx(rect.y + 380),
                tx(sub_font),
                theme.ink_muted,
                esc(sub)
            ));
        }
        if let Some(tag) = node.tag.as_deref().filter(|t| !t.trim().is_empty()) {
            let tag_font = fitted_font(tag, NODE_W, TAG_PREFERRED, TAG_MIN);
            s.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" font-size=\"{}\" fill=\"{}\" text-anchor=\"middle\">{}</text>",
                tx(rect.cx()),
                tx(rect.y + rect.h - 120),
                tx(tag_font),
                node_colors.text,
                esc(tag)
            ));
        }
        s.push_str("</g>");
    }

    // Edge labels.
    for (i, edge) in doc.edges.iter().enumerate() {
        let Some(label) = &edge.label else { continue };
        let (_, _, points, lrect) = &placed[i];
        let Some(lr) = lrect else { continue };
        let at = label_point(points);
        let (_, _, _, label_fill) =
            variant_style(theme, edge.variant.as_deref().unwrap_or("default"));
        s.push_str(&format!(
            "<g data-from=\"{}\" data-to=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"14\" rx=\"3\" fill=\"{}\"/><text x=\"{}\" y=\"{}\" font-size=\"8\" fill=\"{}\" text-anchor=\"middle\">{}</text></g>",
            esc(&edge.from),
            esc(&edge.to),
            tx(lr.x),
            tx(lr.y),
            tx(lr.w),
            theme.panel_alt,
            tx(at.0),
            tx(at.1),
            label_fill,
            esc(label)
        ));
    }

    // Legend.
    if legend_rows.iter().any(|r| !r.is_empty()) {
        for (row, entries) in legend_rows.iter().enumerate() {
            let baseline = legend_y - (legend_rows.len() as i32 - 1 - row as i32) * LEGEND_ROW_H;
            if row == 0 {
                s.push_str(&format!(
                    "<text x=\"{}\" y=\"{}\" font-size=\"7\" font-weight=\"600\" fill=\"{}\">Legend</text>",
                    tx(LEGEND_X),
                    tx(baseline),
                    theme.ink_soft
                ));
            }
            for &(x, _w, idx) in entries {
                let (kind, label) = LEGEND_CATALOG[idx];
                let swatch = theme.node(kind);
                s.push_str(&format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"2\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1\"/><text x=\"{}\" y=\"{}\" font-size=\"7\" fill=\"{}\">{}</text>",
                    tx(x),
                    tx(baseline - 80),
                    tx(LEGEND_SWATCH_W),
                    tx(LEGEND_SWATCH_H),
                    swatch.fill,
                    swatch.stroke,
                    tx(x + LEGEND_SWATCH_W + LEGEND_SWATCH_TEXT_GAP),
                    tx(baseline),
                    theme.ink_soft,
                    esc(label)
                ));
            }
        }
    }
    s.push_str("</svg>");
    s
}
