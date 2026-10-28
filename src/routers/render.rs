//! POST /render_markdown — markdown in, deterministic self-contained HTML
//! artifact out (the document layer, adapted from archify, MIT). The
//! parameters mirror what the removed MCP tool exposed; artifact bytes and
//! the receipt are unchanged.

use axum::extract::Json;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

use crate::{spawn_blocking, AppError};

#[derive(Debug, Deserialize)]
pub(crate) struct RenderMarkdownRequest {
    /// The markdown document. Prose plus typed archify fenced blocks
    /// (sequence/workflow/architecture/dataflow/lifecycle) that render to
    /// inline SVG.
    pub markdown: String,
    /// `light` (default) or `dark`.
    #[serde(default)]
    pub theme: Option<String>,
    /// Palette family: `classic` (default), `signal-flow`, `blueprint`,
    /// `editorial` — orthogonal to theme.
    #[serde(default)]
    pub preset: Option<String>,
    /// Composition audit profile: `standard` (default) or `showcase`.
    /// Grades the receipt only; the artifact bytes are identical.
    #[serde(default)]
    pub quality: Option<String>,
    /// Bake the declarative entrance choreography into the artifact
    /// (CSS keyframes, zero scripts). Default false.
    #[serde(default)]
    pub motion: Option<bool>,
}

#[derive(Serialize)]
struct RenderMarkdownResponse {
    html: String,
    receipt: serde_json::Value,
}

/// POST /render — template family face: `kind` selects a layout (today
/// `cover`; the docgen prose family stays on /render_markdown), the
/// engine returns self-contained HTML sized for one /screenshot {html}
/// call. Cover sizes: `square` 1024×1024 (公众号 upload) / `wide`
/// 1080×460 (the 2.35:1 feed crop pre-applied).
#[derive(Debug, Deserialize)]
pub(crate) struct RenderRequest {
    /// Layout family. `cover` today.
    pub kind: String,
    /// cover/card/title: the headline.
    pub title: String,
    /// card: content lines rendered as stacked rows.
    #[serde(default)]
    pub body: Option<Vec<String>>,
    /// cover/card/title: optional second line.
    #[serde(default)]
    pub subtitle: Option<String>,
    /// cover: optional small footer strip.
    #[serde(default)]
    pub footer: Option<String>,
    /// cover: `square` (default) or `wide`.
    #[serde(default)]
    pub size: Option<String>,
    /// cover square only: overlay the 2.35:1 crop guide.
    #[serde(default)]
    pub show_crop: Option<bool>,
    /// `light` (default) or `dark` — same vocabulary as /render_markdown.
    #[serde(default)]
    pub theme: Option<String>,
    /// Palette family: `classic` (default), `signal-flow`, `blueprint`,
    /// `editorial`.
    #[serde(default)]
    pub preset: Option<String>,
}

#[derive(Serialize, Debug)]
pub(crate) struct RenderResponse {
    kind: String,
    width: u32,
    height: u32,
    html: String,
}

pub(crate) async fn render_handler(
    Json(req): Json<RenderRequest>,
) -> Result<(StatusCode, Json<RenderResponse>), AppError> {
    use crate::docgen::theme::Theme;

    // The layout family shares one skeleton; kind picks the default size
    // (cover/card: square, title: vertical 1080×1920).
    let size_default = match req.kind.as_str() {
        "cover" | "card" => "square",
        "title" => "vertical",
        other => {
            return Err(AppError::BadRequest(format!(
                "unknown kind {other:?} — expected one of: cover, card, title"
            )))
        }
    };
    let theme: &'static Theme = match (&req.preset, &req.theme) {
        (None, None) => &crate::docgen::theme::LIGHT,
        (None, Some(mode)) => Theme::by_name(mode).unwrap_or(&crate::docgen::theme::LIGHT),
        (Some(preset), mode) => {
            Theme::resolve(preset, mode.as_deref().unwrap_or("light"))
                .unwrap_or(&crate::docgen::theme::LIGHT)
        }
    };
    let size = req.size.as_deref().unwrap_or(size_default);
    let (w, h) = crate::docgen::cover::dimensions(size).ok_or_else(|| {
        AppError::BadRequest(format!("unknown size {size:?} — expected one of: square, wide, vertical"))
    })?;
    let html = crate::docgen::cover::render_cover(&crate::docgen::cover::CoverInput {
        title: &req.title,
        subtitle: req.subtitle.as_deref(),
        footer: req.footer.as_deref(),
        body: req.body.as_deref(),
        size,
        show_crop: req.show_crop.unwrap_or(false),
        theme,
    });
    Ok((
        StatusCode::OK,
        Json(RenderResponse { kind: req.kind.clone(), width: w, height: h, html }),
    ))
}

pub(crate) async fn render_markdown_handler(
    Json(req): Json<RenderMarkdownRequest>,
) -> Result<impl IntoResponse, AppError> {
    use crate::docgen::theme::Theme;

    // (preset, mode) → theme, with the classic/light defaults filling
    // one-sided requests.
    let theme: Result<&'static Theme, String> = match (&req.preset, &req.theme) {
        (None, None) => Ok(&crate::docgen::theme::LIGHT),
        (None, Some(mode)) => Theme::by_name(mode).ok_or_else(|| {
            format!(
                "unknown theme \"{mode}\" — expected one of: {}",
                Theme::names().join(", ")
            )
        }),
        (Some(preset), mode) => Theme::resolve(preset, mode.as_deref().unwrap_or("light"))
            .ok_or_else(|| {
                format!(
                    "unknown preset \"{preset}\" or theme \"{}\" — presets: {}; themes: {}",
                    mode.as_deref().unwrap_or("light"),
                    Theme::presets().join(", "),
                    Theme::names().join(", ")
                )
            }),
    };
    let quality = req.quality.as_deref().map_or(
        Ok(crate::docgen::checks::Quality::Standard),
        |name| {
            crate::docgen::checks::Quality::by_name(name).ok_or_else(|| {
                format!("unknown quality \"{name}\" — expected one of: standard, showcase")
            })
        },
    );
    let (theme, quality) = match (theme, quality) {
        (Ok(t), Ok(q)) => (t, q),
        (Err(e), _) | (_, Err(e)) => return Err(AppError::BadRequest(e)),
    };
    let motion = req.motion.unwrap_or(false);
    let outcome = spawn_blocking(move || {
        crate::docgen::render_with_quality(&req.markdown, theme, quality, motion)
    })
    .await
    .map_err(|e| AppError::Internal(format!("render task failed: {e}")))?;
    Ok((
        StatusCode::OK,
        Json(RenderMarkdownResponse {
            html: outcome.html,
            receipt: outcome.receipt,
        }),
    ))
}

#[cfg(test)]
mod render_tests {
    use super::*;

    #[tokio::test]
    async fn render_cover_squares_and_wides() {
        // Exercise the handler's pure core (theme resolve + layout + dims)
        // the same way the route does, asserting on the CoverInput output.
        let theme = &crate::docgen::theme::LIGHT;
        let html = crate::docgen::cover::render_cover(&crate::docgen::cover::CoverInput {
            title: "测试封面",
            subtitle: Some("副题"),
            footer: Some("diting"),
            body: None,
            size: "square",
            show_crop: false,
            theme,
        });
        assert!(html.contains("width:1024px"));
        assert!(html.contains("测试封面"));

        let wide = crate::docgen::cover::render_cover(&crate::docgen::cover::CoverInput {
            title: "宽版",
            subtitle: None,
            footer: None,
            body: None,
            size: "wide",
            show_crop: false,
            theme,
        });
        assert!(wide.contains("width:1080px"));
    }

    #[tokio::test]
    async fn render_card_and_title_kinds_default_sizes() {
        // card → square with body rows; title → vertical 1080×1920.
        let card: RenderRequest = serde_json::from_str(
            r#"{"kind":"card","title":"知识卡","body":["要点一","要点二"]}"#,
        )
        .unwrap();
        let (_, Json(res)) = render_handler(axum::Json(card)).await.ok().unwrap();
        assert_eq!((res.width, res.height), (1024, 1024));
        assert!(res.html.contains("要点一") && res.html.contains("border-left:10px solid"));

        let title: RenderRequest =
            serde_json::from_str(r#"{"kind":"title","title":"竖版"}"#).unwrap();
        let (_, Json(res)) = render_handler(axum::Json(title)).await.ok().unwrap();
        assert_eq!((res.width, res.height), (1080, 1920));
        assert!(res.html.contains("font-size:88px"));
    }

    #[tokio::test]
    async fn render_rejects_unknown_kind_and_size() {
        let bad_kind: RenderRequest =
            serde_json::from_str(r#"{"kind":"poster","title":"x"}"#).unwrap();
        assert!(render_handler(axum::Json(bad_kind)).await.is_err());
        let bad_size: RenderRequest = serde_json::from_str(
            r#"{"kind":"cover","title":"x","size":"a3"}"#,
        )
        .unwrap();
        assert!(render_handler(axum::Json(bad_size)).await.is_err());
    }
}
