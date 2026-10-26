//! D19 task orchestration — billing on outcomes, engine side.
//!
//! One armed task per flow-run (market branch arms automatically) or per
//! manual `/task/start`. The page-layer observer records signature hits
//! for EVERY execution path; this layer drains them and settles against
//! the hub:
//! - flow run end: settle(successes = hits) — 0 hits refunds the hold
//!   (nothing published, nothing charged);
//! - manual: settle when the agent finishes (Tier-2 declaration wins,
//!   hits ride along as evidence), TTL on the hub is the backstop.
//!
//! The observer is force-armed with the flow's signatures on every market
//! run — there is no opt-out knob (DUPHUB.md 〇之二.2).

use serde_json::{json, Value};

/// Signatures carried by a flow document (first-party convention):
/// `"signatures": [{"kind":"url","contains":"success.htm?primaryId="}, …]`
pub fn flow_signatures(doc: &Value) -> Vec<Value> {
    doc.get("signatures")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

/// Arm a task for a session: hub arm + force-arm the page observer.
/// Returns the hub task row on success. `signatures` empty = Tier-2
/// (no observer; the agent's finish declaration settles).
pub async fn arm(
    anchor: &str,
    session_id: &str,
    expected_n: u32,
    price_cents: i64,
    signatures: &[Value],
) -> Result<Value, String> {
    // Hub arm. The OBSERVER arms in two other places: the flow executor
    // (signature-bearing flows, right after the session exists) and here —
    // for /task/start on a LIVE session (manual takeover path).
    if !signatures.is_empty() && !session_id.is_empty() {
        let sig_json = serde_json::to_string(signatures).unwrap_or_else(|_| "[]".into());
        if let Err(e) = crate::session::send_command(session_id, |reply| {
            crate::session::SessionCommand::SetOutcome {
                signatures: sig_json,
                reply,
            }
        })
        .await
        {
            return Err(format!("arm observer: {e}"));
        }
    }
    crate::activation::task_arm(anchor, session_id, expected_n, price_cents, 0)
        .await
        .map_err(|e| e.to_string())
}

/// Drain the session's recorded hits (uncharged evidence rows).
pub async fn hits(session_id: &str) -> Vec<Value> {
    // The dispatch replies with a JSON OBJECT ({"hits": [...]}) — read it
    // directly; no string round-trip to get wrong.
    crate::session::send_command(session_id, |reply| {
        crate::session::SessionCommand::OutcomeHits { reply }
    })
    .await
    .ok()
    .and_then(|v| v.get("hits").and_then(|h| h.as_array()).cloned())
    .unwrap_or_default()
}


/// Flow-run end: settle with the observed hit count, refund when zero.
/// Returns the receipt's `task` field.
pub async fn settle_run(task: &Value, session_id: &str, anchor: &str) -> Value {
    let id = task.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    if id == 0 {
        return json!({"error": "task arm produced no id"});
    }
    let hits = hits(session_id).await;
    let n = hits.len() as u32;
    let evidence = json!({
        "anchor": anchor,
        "observer_hits": hits,
        "hits": n,
    });
    let outcome = if n > 0 {
        crate::activation::task_settle(id, n, &evidence).await
    } else {
        // No signature hit — nothing published: give the hold back now
        // rather than parking it until TTL.
        crate::activation::task_refund(id).await
    };
    match outcome {
        Ok(row) => json!({
            "id": id,
            "state": row.get("state").cloned().unwrap_or(Value::Null),
            "successes": row.get("successes").cloned().unwrap_or(Value::Null),
            "charged_cents": row.get("charged_cents").cloned().unwrap_or(Value::Null),
            "hold_cents": row.get("hold_cents").cloned().unwrap_or(Value::Null),
            "price_cents": row.get("price_cents").cloned().unwrap_or(Value::Null),
            "balance_cents": row.get("balance_cents").cloned().unwrap_or(Value::Null),
        }),
        Err(e) => json!({"id": id, "settle_error": e.to_string()}),
    }
}

// ---------------------------------------------------------------------------
// HTTP face — manual task path (the flow-run market branch arms itself).

use axum::extract::Query as HttpQuery;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;

#[derive(Deserialize)]
pub(crate) struct TaskStartBody {
    /// Anchor flow name (hub prices it). Empty = Tier-2 bare task on an
    /// undefined site (`price_cents` explicit, default 0).
    #[serde(default)]
    pub anchor: String,
    /// Live session the work happens in — the observer arms on it.
    pub session_id: String,
    #[serde(default = "one")]
    pub expected_n: u32,
    #[serde(default)]
    pub price_cents: i64,
    /// Signatures; defaults to the anchor flow's bundled group when the
    /// anchor is given and the flow carries one.
    #[serde(default)]
    pub signatures: Option<Vec<Value>>,
}

fn one() -> u32 {
    1
}

pub(crate) async fn task_start_handler(Json(body): Json<TaskStartBody>) -> Response {
    // Signatures: explicit > anchor flow's bundled group.
    let mut sigs = body.signatures.clone().unwrap_or_default();
    if sigs.is_empty() && !body.anchor.is_empty() {
        if let Ok(Some(_price)) = crate::activation::price(&body.anchor).await {
            if let Ok(loaded) = crate::activation::market_fetch(&body.anchor).await {
                sigs = flow_signatures(&loaded.doc);
            }
        }
    }
    match arm(&body.anchor, &body.session_id, body.expected_n, body.price_cents, &sigs).await {
        Ok(task) => (StatusCode::OK, Json(task)).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub(crate) struct TaskFinishBody {
    pub task_id: i64,
    /// Tier-2 declaration: how many successes the agent claims. The
    /// observer's hits (if any) ride along as corroborating evidence.
    #[serde(default = "one")]
    pub successes: u32,
    /// Free-form evidence (final URL, notes); the engine adds observer hits.
    #[serde(default)]
    pub evidence: Value,
    #[serde(default)]
    pub session_id: Option<String>,
}

pub(crate) async fn task_finish_handler(Json(body): Json<TaskFinishBody>) -> Response {
    let hits = match &body.session_id {
        Some(sid) => hits(sid).await,
        None => Vec::new(),
    };
    let mut evidence = body.evidence.clone();
    if let Some(obj) = evidence.as_object_mut() {
        obj.insert("observer_hits".into(), json!(hits));
    } else {
        evidence = json!({"custom": body.evidence, "observer_hits": hits});
    }
    match crate::activation::task_settle(body.task_id, body.successes, &evidence).await {
        Ok(task) => (StatusCode::OK, Json(task)).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub(crate) struct TaskStatusQuery {
    pub task_id: i64,
    #[serde(default)]
    pub session_id: Option<String>,
}

pub(crate) async fn task_status_handler(HttpQuery(q): HttpQuery<TaskStatusQuery>) -> Response {
    // Drain the observer first so a pending success is visible in the row.
    if let Some(sid) = &q.session_id {
        let pending = hits(sid).await;
        if !pending.is_empty() {
            // Park them back? v1: report alongside; the settle call drains.
            if let Ok(mut row) = crate::activation::task_get(q.task_id).await {
                if let Some(obj) = row.as_object_mut() {
                    obj.insert("pending_hits".into(), json!(pending));
                }
                return (StatusCode::OK, Json(row)).into_response();
            }
        }
    }
    match crate::activation::task_get(q.task_id).await {
        Ok(row) => (StatusCode::OK, Json(row)).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}
