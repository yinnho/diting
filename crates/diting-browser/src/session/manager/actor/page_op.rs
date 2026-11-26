//! `SessionCommand::Page`/`PageMetrics` on the session thread — the
//! `Page.*`/`Network.*` page-level ops and the layout-metrics read.
//! Split from actor.rs for the god-file cap.

use serde_json::Value;
use tokio::sync::oneshot;

use super::super::super::commands::PageOp;
use super::SessionActor;

/// The script `Page.getLayoutMetrics` runs — moved here from the CDP
/// face: it is engine-internal page JS, not wire vocabulary.
const LAYOUT_METRICS: &str = "(function(){\
    var vv = window.visualViewport || {};\
    var root = document.documentElement || document.body;\
    var iw = window.innerWidth || 0, ih = window.innerHeight || 0;\
    var sx = window.scrollX || 0, sy = window.scrollY || 0;\
    return {\
        iw: iw, ih: ih, sx: sx, sy: sy,\
        vw: vv.width || iw, vh: vv.height || ih,\
        ox: vv.offsetLeft || 0, oy: vv.offsetTop || 0, scale: vv.scale || 1,\
        cw: Math.max(iw, root ? root.scrollWidth || 0 : 0),\
        ch: Math.max(ih, root ? root.scrollHeight || 0 : 0)\
    };\
})()";

impl SessionActor {
    /// Apply one `Page.*`/`Network.*` op. The init-script registry is the
    /// actor's own `cdp_init_scripts` — document-start registrations the
    /// page merges on every navigation.
    pub(super) async fn on_page_op(
        &mut self,
        op: PageOp,
        reply: oneshot::Sender<Result<(), String>>,
    ) {
        let result = match op {
            PageOp::AddInitScript {
                identifier,
                source,
                run_immediately,
            } => {
                self.page.inner.add_preload_script(source.clone());
                self.cdp_init_scripts.push((identifier, source.clone()));
                if run_immediately {
                    self.page
                        .inner
                        .evaluate_remote_outcome(&source, false, false, 30_000, None)
                        .await;
                }
                Ok(())
            }
            PageOp::RemoveInitScript { identifier } => {
                match self
                    .cdp_init_scripts
                    .iter()
                    .position(|(id, _)| *id == identifier)
                {
                    Some(at) => {
                        let (_, source) = self.cdp_init_scripts.remove(at);
                        self.page.inner.remove_preload_script(&source);
                        Ok(())
                    }
                    None => Err("Script not found".to_string()),
                }
            }
            PageOp::BlockedUrls { urls } => {
                self.page.inner.set_blocked_urls(urls);
                Ok(())
            }
            PageOp::ExtraHeaders { headers } => {
                self.page.inner.set_extra_headers(headers).await;
                Ok(())
            }
        };
        let _ = reply.send(result);
    }

    /// `Page.getLayoutMetrics`: the raw numbers the face shapes into
    /// Chrome's layout/visual/content triplet.
    pub(super) async fn on_page_metrics(&mut self, reply: oneshot::Sender<Result<Value, String>>) {
        let _ = reply.send(
            self.page
                .evaluate_with_timeout_result(LAYOUT_METRICS, crate::page::INTERACTION_EVAL_TIMEOUT)
                .map_err(|err| format!("Page.getLayoutMetrics: {err}")),
        );
    }
}
