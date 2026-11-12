// DOM 命令 dispatch 脊柱：op_dom 分诊 + 写路径布局失效 + attachShadow — split out of the god file (ratchet, #75).
// Pure move: the functions see the same ops::* namespace they grew up in.
use super::*;

/// Attach a native shadow root to a host element (`Element.prototype.attachShadow`).
/// Returns the root's node id, or -2 when the host already has a shadow root
/// (JS maps that to the spec's NotSupportedError) and -1 for any other failure
/// (missing DOM, non-element host, bad mode).
#[diting_op]
pub(super) fn op_shadow_attach(state: &OpState, host_nid: u32, #[string] mode: String) -> i32 {
    let gs = state.borrow::<SharedState>().clone();
    let gs = gs.borrow();
    let dom = match &gs.dom {
        Some(d) => d,
        None => return -1,
    };
    let mode = match mode.as_str() {
        "open" => ShadowRootMode::Open,
        "closed" => ShadowRootMode::Closed,
        _ => return -1,
    };
    match dom.attach_shadow_root(NodeId::new(host_nid), mode) {
        Ok(root) => root.raw() as i32,
        Err(AttachShadowError::HostAlreadyHasShadowRoot) => -2,
        Err(_) => -1,
    }
}

#[diting_op]
#[string]
pub(super) fn op_dom(
    state: &OpState,
    #[string] cmd: String,
    #[string] arg1: String,
    #[string] arg2: String,
) -> String {
    // Anti-panic boundary: a panic in a DOM op would unwind through deno_core
    // into V8's FFI frame, where V8_Fatal calls abort(3) and takes the whole
    // engine (and every CDP client) down. Catch it so one malformed selector or
    // inconsistent tree node degrades to a null result for that single call.
    // No per-call clone: on the happy path this is just a landing pad, so the
    // hot DOM path (querySelector/getAttribute/...) pays nothing measurable.
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        op_dom_inner(state, cmd, arg1, arg2)
    }))
    .unwrap_or_else(|_| {
        tracing::error!("op_dom panicked; returning null");
        "null".to_string()
    })
}

pub(super) fn op_dom_inner(state: &OpState, cmd: String, arg1: String, arg2: String) -> String {
    // Font-metrics truth tables (font_tables.rs): pure data, no DOM or
    // shared state needed — answered before the no-dom guard so a canvas in
    // a DOM-less realm still measures.
    if cmd == "font_table" {
        return crate::font_tables::lookup(&arg1, &arg2);
    }
    // Title write goes to shared state (the getter reads gs.title), so it must
    // happen before the immutable borrow below — and before the `gs.dom` early
    // return, since a document always has a title even with an empty tree.
    if cmd == "set_document_title" {
        let gs = state.borrow::<SharedState>().clone();
        gs.borrow_mut().title = arg1;
        return "null".into();
    }
    #[cfg(feature = "screenshot")]
    drop_layout_on_write(state, &cmd, &arg1, &arg2);
    let (arg1, arg2) = match dom_cmd_mut_state(state, &cmd, arg1, arg2) {
        Ok(answer) => return answer,
        Err(args) => args,
    };
    let gs = state.borrow::<SharedState>().clone();
    let gs = gs.borrow();
    // The history seed answers without a DOM tree and must survive the
    // no-dom guard: bootstrap reads these to re-seed its history stack
    // before any page script runs (obscura#1105).
    if cmd == "history_pos" {
        return gs.history_pos.to_string();
    }
    if cmd == "history_total" {
        return gs.history_total.to_string();
    }
    let dom = match &gs.dom {
        Some(d) => d,
        None => return "null".to_string(),
    };

    dom_cmd_document(&gs, dom, &cmd, arg1, arg2)
        .or_else(|(arg1, arg2)| dom_cmd_query(&gs, dom, &cmd, arg1, arg2))
        .or_else(|(arg1, arg2)| dom_cmd_tree(&gs, dom, &cmd, arg1, arg2))
        .or_else(|(arg1, arg2)| dom_cmd_attr_state(&gs, dom, &cmd, arg1, arg2))
        .or_else(|(arg1, arg2)| dom_cmd_mutate(&gs, dom, &cmd, arg1, arg2))
        .or_else(|(arg1, arg2)| dom_cmd_create(&gs, dom, &cmd, arg1, arg2))
        .or_else(|(arg1, arg2)| dom_cmd_sheets(&gs, dom, &cmd, arg1, arg2))
        .or_else(|(arg1, arg2)| dom_cmd_geometry(&gs, dom, &cmd, arg1, arg2))
        .or_else(|(arg1, arg2)| dom_cmd_scroll_style(&gs, dom, &cmd, arg1, arg2))
        .unwrap_or_else(|_| "null".into())
}

/// Style/tree writes don't all bump the DomTree epoch (attribute-level
/// mutations don't allocate nodes), so the memoized layout run must be
/// dropped explicitly at the write commands — the bootstrap mirrors
/// this set as _DOM_MUTATION_COMMANDS for its getComputedStyle
/// snapshot epoch, and both sides see the same op_dom traffic through
/// the same choke point.
#[cfg(feature = "screenshot")]
pub(super) fn drop_layout_on_write(state: &OpState, cmd: &str, arg1: &str, arg2: &str) {
    if matches!(
        cmd,
        "append_child"
            | "insert_before"
            | "remove_child"
            | "set_attribute"
            | "remove_attribute"
            | "set_text_content"
            | "set_inner_html"
            | "set_live_value"
            | "set_live_checked"
            | "set_live_indeterminate"
            | "set_focused"
            | "set_selection"
            | "document_write_reset"
    ) {
        let gs = state.borrow::<SharedState>().clone();
        // Paint-only style write (#395): a timeline seek lands as
        // set_attribute("style", …) whose changed declarations sit inside
        // {transform, opacity} — the compositor-only set a real browser
        // never re-layouts for; the taffy solve ignores them, so it
        // survives and the next layout run re-collects against the cached
        // tree (measured 109ms/frame seeks → the paint floor). The diff is
        // per-property and value-aware (#220): a same-name rewrite of a
        // solve-feeding property (padding, width, …) must not ride this
        // path; runs pre-write, so the OLD attribute is still on the node.
        //
        // Value-identity short-circuit (#398): a style write whose full
        // serialized string is byte-identical to what's already on the node
        // cannot change any computed style, so no cache drops and layout_rev
        // holds — that flat revision is what the video pump's static-frame
        // reuse keys on. The DOM write below still happens (observers see
        // it, exactly as Chrome fires mutation records for a redundant
        // setAttribute); only the invalidation is skipped, the same way
        // Chrome's style system dedupes equal declarations.
        let mut paint_only = false;
        let mut identity = false;
        if cmd == "set_attribute"
            && arg2
                .split_once('\0')
                .is_some_and(|(name, _)| name.eq_ignore_ascii_case("style"))
        {
            let old = arg1.parse::<u32>().ok().map(NodeId::new).and_then(|id| {
                gs.borrow().dom.as_ref().and_then(|d| {
                    d.get_node(id)
                        .and_then(|n| n.get_attribute("style").map(|s| s.to_string()))
                })
            });
            let new = arg2.split_once('\0').map(|(_, v)| v);
            identity = old.as_deref() == new;
            paint_only = !identity && style_write_is_paint_only(old.as_deref(), new);
        }
        // Inert attribute writes (obscura#983): tabindex/title/role/
        // data-*/aria-* change no computed style and no layout input on
        // their own — a docs theme writing tabindex per heading used to pay
        // a full re-layout per write. Escape hatch: if any loaded rule's
        // selector references the name (`[data-x]`, `[aria-expanded]`), the
        // write still invalidates. Everything layout, pseudo-classes, or
        // fetch triggers read (class/id/style, width/height/colspan/…,
        // disabled/checked/href/src/rel) is deliberately NOT in the inert
        // set. The DOM write below still happens either way.
        let attr_name = match cmd {
            "set_attribute" => arg2.split_once('\0').map(|(n, _)| n),
            "remove_attribute" => Some(arg2),
            _ => None,
        };
        let inert = attr_name.is_some_and(|n| {
            let gs = gs.borrow();
            let skip = attr_write_is_layout_inert(n, gs.attr_selector_names.borrow().as_ref());
            skip
        });
        if !identity && !inert {
            if paint_only {
                gs.borrow().drop_paint_only();
            } else {
                gs.borrow().drop_layout();
            }
        }
    }
}

/// `op_dom` commands that write shared state through a mutable borrow
/// (viewport, media env, root scroll, CSS clock, transition registry), so
/// they answer before the read-only borrow the families share.
#[cfg_attr(not(feature = "screenshot"), allow(unused_variables))]
pub(super) fn dom_cmd_mut_state(state: &OpState, cmd: &str, arg1: String, arg2: String) -> DomCmd {
    // Persona viewport: needs a mutable borrow, so it runs before the main
    // read-only `gs` alias below (same pattern as set_document_title).
    #[cfg(feature = "screenshot")]
    if cmd == "set_viewport" {
        let gs = state.borrow::<SharedState>().clone();
        let w = arg1.parse::<f32>().unwrap_or(1920.0);
        let h = arg2.parse::<f32>().unwrap_or(1000.0);
        if w.is_finite() && w > 0.0 && h.is_finite() && h > 0.0 {
            let mut gs = gs.borrow_mut();
            gs.viewport = (w, h);
            // Any rects memoized under the old ICB are stale now.
            gs.drop_layout();
        }
        return Ok("ok".into());
    }
    // Emulated media environment from CDP `Emulation.setEmulatedMedia`
    // (Playwright's page.emulateMedia). The bootstrap recomputes its
    // matchMedia tables first and then pushes the same pairs here, so the
    // cascade face and the script face flip together; @media arms re-parse
    // on the next layout run.
    #[cfg(feature = "screenshot")]
    if cmd == "set_media_env" {
        let gs = state.borrow::<SharedState>().clone();
        let features: Vec<(String, String)> = serde_json::from_str(&arg1).unwrap_or_default();
        let media_type = if arg2.eq_ignore_ascii_case("print") {
            diting_css::CssMediaType::Print
        } else {
            diting_css::CssMediaType::Screen
        };
        {
            let mut gs = gs.borrow_mut();
            gs.media_type = media_type;
            gs.media_overrides.features = features;
            gs.drop_layout();
        }
        return Ok("ok".into());
    }
    // Root-scroller offset mirror from the bootstrap (viewport roots only;
    // element-level scrolling is not mirrored). The bootstrap already clamps
    // negatives to 0; band paint clamps to the scrollable range itself.
    #[cfg(feature = "screenshot")]
    if cmd == "set_scroll_offset" {
        let gs = state.borrow::<SharedState>().clone();
        let x = arg1.parse::<f32>().unwrap_or(0.0);
        let y = arg2.parse::<f32>().unwrap_or(0.0);
        if x.is_finite() && y.is_finite() {
            gs.borrow_mut().scroll_offset = (x.max(0.0), y.max(0.0));
        }
        return Ok("ok".into());
    }
    // CSS animation clock, driven by the video pump. Negative/NaN rejected;
    // t=0 (the pre-delay state) is a legal sample.
    #[cfg(feature = "screenshot")]
    if cmd == "set_css_time" {
        let gs = state.borrow::<SharedState>().clone();
        let t = arg1.parse::<f64>().unwrap_or(f64::NAN);
        if t.is_finite() && t >= 0.0 {
            let mut gs = gs.borrow_mut();
            if gs.css_time != Some(t) {
                gs.css_time = Some(t);
                gs.drop_paint_only();
            }
        }
        return Ok("ok".into());
    }
    // Transition registry (JS face detects the trigger and hands us the
    // before/after computed values). The a2 slot carries a JSON blob —
    // _domRaw only forwards two args. start=0 on the video timeline: page
    // scripts run once during settle, so a registered transition plays from
    // the pump's t=0. Live screenshots keep css_time=None and the sampler
    // no-ops (end state stands) — the settled-posture collapse.
    #[cfg(feature = "screenshot")]
    if cmd == "add_css_transition" {
        let gs = state.borrow::<SharedState>().clone();
        let Some(nid) = arg1.parse::<u32>().ok().map(NodeId::new) else {
            return Ok("ok".into());
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&arg2) else {
            return Ok("ok".into());
        };
        let Some(prop) = v
            .get("prop")
            .and_then(|x| x.as_str())
            .map(|s| s.to_ascii_lowercase())
        else {
            return Ok("ok".into());
        };
        let parse_val = |name: &str, s: &str| -> Option<diting_css::TransitionValue> {
            match name {
                "opacity" => s
                    .trim()
                    .parse::<f32>()
                    .ok()
                    .map(|n| diting_css::TransitionValue::Opacity(n.clamp(0.0, 1.0))),
                "color" | "background-color" => diting_css::parse_color(s).map(|c| {
                    diting_css::TransitionValue::Color([
                        c.0 as f32, c.1 as f32, c.2 as f32, c.3 as f32,
                    ])
                }),
                // The computed snapshot is "none" or a `matrix(...)` string;
                // `none` maps to the identity-side arm of lerp_transform so
                // none↔matrix transitions still interpolate. A percentage
                // translate has no used value at snapshot time (the computed
                // table omits it) and fails to parse — no transition.
                "transform" => {
                    let s = s.trim();
                    if s == "none" || s.is_empty() {
                        Some(diting_css::TransitionValue::Transform(None))
                    } else {
                        diting_css::parse_transform(s)
                            .map(|t| diting_css::TransitionValue::Transform(Some(t)))
                    }
                }
                _ => None,
            }
        };
        let (Some(from), Some(to)) = (
            v.get("from")
                .and_then(|x| x.as_str())
                .and_then(|s| parse_val(&prop, s)),
            v.get("to")
                .and_then(|x| x.as_str())
                .and_then(|s| parse_val(&prop, s)),
        ) else {
            return Ok("ok".into());
        };
        let duration = (v.get("duration").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32) / 1000.0;
        let delay = (v.get("delay").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32) / 1000.0;
        if !duration.is_finite()
            || duration < 0.0
            || !delay.is_finite()
            || delay < 0.0
            || duration + delay <= 0.0
        {
            return Ok("ok".into());
        }
        let easing = v
            .get("easing")
            .and_then(|x| x.as_str())
            .and_then(diting_css::parse_easing_token)
            .unwrap_or(diting_css::Easing::CubicBezier(0.25, 0.1, 0.25, 1.0));
        let gs = gs.borrow_mut();
        let mut list = gs.css_transitions.borrow_mut();
        // A re-trigger on the same (node, property) replaces the pending
        // entry; the cap bounds a page that spams writes every frame.
        list.retain(|tr| !(tr.nid == nid.index() && tr.property == prop));
        if list.len() >= 256 {
            list.remove(0);
        }
        list.push(diting_css::CssTransition {
            nid: nid.index(),
            property: prop,
            from,
            to,
            start: 0.0,
            duration,
            delay,
            easing,
        });
        drop(list);
        gs.drop_paint_only();
        return Ok("ok".into());
    }
    Err((arg1, arg2))
}

/// One `op_dom` family's answer, or the args handed back: not this
/// family's command, the dispatcher tries the next one.
pub(super) type DomCmd = Result<String, (String, String)>;

/// Node-id args that fail to parse must NOT silently become node 0 (the
/// document root) — a fake-receiver call like
/// `Element.prototype.setHTMLUnsafe.call({})` would otherwise wipe the
/// whole document. Mutating commands no-op on an invalid id.
pub(super) fn parse_nid(s: &str) -> Option<NodeId> {
    s.parse::<u32>().ok().map(NodeId::new)
}
