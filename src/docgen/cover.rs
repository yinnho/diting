//! Cover layout family — the render-a-page-then-paint-it product's first
//! template (微信封面复盘 2026-10-27: the "AI authors HTML, engine paints
//! it" chain needed a layout that knows where WeChat crops).
//!
//! One cover = title + optional subtitle/footer riding a theme palette.
//! Two size presets:
//!   - `square`   1024×1024 — the公众号大封面 upload shape
//!   - `wide`     1080×460  — the 2.35:1 feed crop already applied
//! The square layout keeps all text inside the **center safe area** (the
//! region that survives WeChat's 2.35:1 center crop) and can overlay the
//! crop guide lines (`show_crop: true`) so an agent can verify nothing it
//! cares about gets cut before shipping.

use crate::docgen::theme::Theme;

pub struct CoverInput<'a> {
    pub title: &'a str,
    pub subtitle: Option<&'a str>,
    pub footer: Option<&'a str>,
    /// `square` (default) or `wide`.
    pub size: &'a str,
    /// Draw the 2.35:1 crop guide over a square layout (square only).
    pub show_crop: bool,
    pub theme: &'a Theme,
}

/// (width, height) for a size name.
pub fn dimensions(size: &str) -> Option<(u32, u32)> {
    match size {
        "square" | "" => Some((1024, 1024)),
        "wide" => Some((1080, 460)),
        _ => None,
    }
}

/// Escape for safe embedding in generated markup.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The safe band WeChat's center crop keeps on a 1024×1024 upload:
/// crop height = 1024 / 2.35 ≈ 436px centered → y ∈ [294, 730].
fn square_safe_band() -> (f64, f64) {
    let h = 1024.0 / 2.35;
    let top = (1024.0 - h) / 2.0;
    (top, top + h)
}

/// Render the cover as a self-contained HTML document (system font stack;
/// the engine's bundled CJK font paints it at screenshot time).
pub fn render_cover(input: &CoverInput) -> String {
    let CoverInput { title, subtitle, footer, size, show_crop, theme } = input;
    let (w, h) = dimensions(size).unwrap_or((1024, 1024));
    let square = w == 1024 && h == 1024;

    // Wide IS the crop — no guides, text fills the canvas.
    // Square: text lives in the center safe band; the two flanks carry the
    // palette (visible in the square upload, gone in the feed crop).
    let (text_block, flank, crop_overlay) = if square {
        let (top, bottom) = square_safe_band();
        let band_css = format!(
            "position:absolute;left:0;right:0;top:{top:.0}px;height:{:.0}px;display:flex;flex-direction:column;justify-content:center;align-items:center;text-align:center;padding:0 72px;box-sizing:border-box",
            bottom - top
        );
        (
            format!("<div style=\"{band_css}\">{}</div>", inner_text(title, subtitle.as_deref(), footer.as_deref(), theme)),
            // Flank texture: a quiet oversized glyph of the palette, one per
            // side, clipped by overflow — cover art without an image asset.
            format!(
                "<div style='position:absolute;left:-40px;top:180px;font-size:560px;line-height:1;color:{};opacity:.10;font-weight:800;user-select:none'>〔〕</div>\
                 <div style='position:absolute;right:-60px;bottom:120px;font-size:480px;line-height:1;color:{};opacity:.10;font-weight:800;user-select:none'>」</div>",
                theme.ink, theme.edge_label
            ),
            if *show_crop {
                format!(
                    "<div style='position:absolute;left:0;right:0;top:{top:.0}px;height:{:.0}px;border:3px dashed {};box-sizing:border-box;pointer-events:none'></div>\
                     <div style='position:absolute;left:16px;top:{top:.0}px;font:22px sans-serif;color:{};background:{};padding:2px 10px'>2.35:1 crop — keep text inside</div>",
                    bottom - top, theme.danger, theme.danger, theme.page_bg
                )
            } else {
                String::new()
            },
        )
    } else {
        (
            format!(
                "<div style='position:absolute;inset:0;display:flex;flex-direction:column;justify-content:center;padding:0 88px;box-sizing:border-box'>{}</div>",
                inner_text(title, subtitle.as_deref(), footer.as_deref(), theme)
            ),
            format!(
                "<div style='position:absolute;right:40px;top:-70px;font-size:380px;line-height:1;color:{};opacity:.12;font-weight:800;user-select:none'>」</div>",
                theme.edge_label
            ),
            String::new(),
        )
    };

    format!(
        "<!doctype html><html><head><meta charset='utf-8'><style>html,body{{margin:0;padding:0}}*{{font-family:'PingFang SC','Hiragino Sans GB','Microsoft YaHei',sans-serif}}</style></head>\
         <body style='width:{w}px;height:{h}px;overflow:hidden;position:relative;background:{bg};color:{fg}'>\
         {flank}\
         {text_block}\
         {crop_overlay}\
         </body></html>",
        bg = theme.page_bg,
        fg = theme.page_fg,
    )
}

/// Title + subtitle + footer, text-left on wide / centered on square.
fn inner_text(title: &str, subtitle: Option<&str>, footer: Option<&str>, theme: &Theme) -> String {
    let mut out = String::new();
    // 900/64 for square fill; wide is shorter so the title sizes up.
    out.push_str(&format!(
        "<div style='font-size:64px;font-weight:800;line-height:1.25;color:{}'>{}</div>",
        theme.ink,
        esc(title)
    ));
    if let Some(sub) = subtitle {
        out.push_str(&format!(
            "<div style='margin-top:18px;font-size:30px;line-height:1.5;color:{}'>{}</div>",
            theme.ink_muted,
            esc(sub)
        ));
    }
    if let Some(foot) = footer {
        out.push_str(&format!(
            "<div style='margin-top:26px;font-size:22px;letter-spacing:.08em;color:{};border-top:2px solid {};display:inline-block;padding-top:12px'>{}</div>",
            theme.ink_soft,
            theme.guide,
            esc(foot)
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docgen::theme::LIGHT;

    #[test]
    fn square_cover_keeps_text_in_safe_band() {
        let html = render_cover(&CoverInput {
            title: "越用越快的浏览器",
            subtitle: Some("技能沉淀叙事"),
            footer: Some("diting"),
            size: "square",
            show_crop: false,
            theme: &LIGHT,
        });
        assert!(html.contains("width:1024px"), "square dims");
        assert!(html.contains("top:294px"), "safe band top");
        // No crop guide unless asked.
        assert!(!html.contains("crop — keep text"));
    }

    #[test]
    fn crop_guide_only_when_requested() {
        let html = render_cover(&CoverInput {
            title: "t",
            subtitle: None,
            footer: None,
            size: "square",
            show_crop: true,
            theme: &LIGHT,
        });
        assert!(html.contains("2.35:1 crop"), "guide present");
        let wide = render_cover(&CoverInput {
            title: "t",
            subtitle: None,
            footer: None,
            size: "wide",
            show_crop: true, // ignored: wide IS the crop
            theme: &LIGHT,
        });
        assert!(wide.contains("width:1080px"));
        assert!(!wide.contains("2.35:1 crop"));
    }

    #[test]
    fn markup_is_escaped() {
        let html = render_cover(&CoverInput {
            title: "<script>alert(1)</script>",
            subtitle: None,
            footer: None,
            size: "square",
            show_crop: false,
            theme: &LIGHT,
        });
        assert!(!html.contains("<script>alert"), "title escaped");
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn dimensions_reject_unknown() {
        assert_eq!(dimensions("square"), Some((1024, 1024)));
        assert_eq!(dimensions(""), Some((1024, 1024)));
        assert_eq!(dimensions("wide"), Some((1080, 460)));
        assert_eq!(dimensions("huge"), None);
    }
}
