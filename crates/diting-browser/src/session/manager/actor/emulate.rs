//! `SessionCommand::Emulate` on the session thread — the `Emulation.*`
//! page-state pins. Split from actor.rs for the god-file cap; params are
//! already checked on the face, this only applies them to the page.

use tokio::sync::oneshot;

use super::super::super::commands::EmulateOp;
use super::SessionActor;

impl SessionActor {
    /// Apply one emulation pin. A refusal the page itself can only answer
    /// — a timezone the runtime does not know — comes back as Chrome's
    /// word; the face maps it to `invalid`.
    pub(super) async fn on_emulate(
        &mut self,
        op: EmulateOp,
        reply: oneshot::Sender<Result<(), String>>,
    ) {
        let page = &mut self.page;
        let result = match op {
            EmulateOp::EmulatedMedia { features, media } => {
                page.inner.set_emulated_media(Some(features), Some(media));
                Ok(())
            }
            EmulateOp::TouchEmulation {
                enabled,
                max_points,
            } => {
                page.inner.set_touch_emulation(enabled, max_points);
                Ok(())
            }
            EmulateOp::Timezone { timezone } => {
                // An empty id clears the zone — the face sends `None`
                // for both "cleared" and "never set".
                if let Some(tz) = &timezone {
                    if !tz.is_empty() && !page.inner.timezone_id_supported(tz) {
                        Err("Invalid timezone id".to_string())
                    } else {
                        page.inner
                            .set_timezone_override((!tz.is_empty()).then(|| tz.clone()));
                        Ok(())
                    }
                } else {
                    page.inner.set_timezone_override(None);
                    Ok(())
                }
            }
            EmulateOp::Locale { locale } => {
                page.inner.set_locale_override(locale);
                Ok(())
            }
            EmulateOp::UserAgent {
                user_agent,
                accept_language,
                platform,
            } => {
                page.inner
                    .set_user_agent_override(
                        &user_agent,
                        accept_language.as_deref(),
                        platform.as_deref(),
                    )
                    .await;
                Ok(())
            }
            EmulateOp::DeviceMetrics {
                width,
                height,
                mobile,
                ratio,
                panel,
                orientation,
            } => {
                page.inner
                    .set_viewport_override(width, height, mobile, Some(ratio));
                page.inner.set_screen_override(panel, orientation);
                Ok(())
            }
            EmulateOp::ClearDeviceMetrics => {
                page.inner.clear_viewport_override();
                Ok(())
            }
        };
        let _ = reply.send(result);
    }
}
