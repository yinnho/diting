// 时间面：sleep body（async glue 消费）+ 时钟毫秒 — split out of the god file (ratchet, #75).
// Pure move: the functions see the same ops::* namespace they grew up in.
use super::*;

/// Free body of `op_sleep` — the hand glue (`diting_js::async_glue`)
/// calls this directly off our Vm host (10d swap; the #[op2] wrapper died
/// with the deno host).
pub(crate) async fn op_sleep_body(millis: u64) {
    // #230: tokio's timer wheel ticks at 1ms granularity — even a
    // zero-length sleep parks until the next wheel tick before its
    // deadline reads as due, taxing every delay-0 round-trip with ~1ms of
    // pure wheel latency (measured 1154µs/trip while the JS work under it
    // cost 55ns/item: the macrotask trampoline takes one trip per 64
    // items, so a 340k-timer fxg boot drained at ~5.7k/s = 59s). A zero
    // sleep completes at the first drive instead — settlement still rides
    // the settle queue into the next loop pass, so the task-boundary
    // discipline (#37/#327 line: never settle on the JS stack) is intact;
    // it just doesn't wait for a wheel tick to say "due".
    if millis == 0 {
        return;
    }
    // Reactor-less contexts (plain #[test] isolates): a tokio timer would
    // panic mid-poll inside v8, which aborts the whole test binary. Resolve
    // at the next event-loop checkpoint instead — production always runs
    // under a reactor and never takes this path.
    if tokio::runtime::Handle::try_current().is_err() {
        return;
    }
    tokio::time::sleep(std::time::Duration::from_millis(millis)).await;
}

/// (#78) Sub-ms wall clock for performance.now(). Chrome coarsens the timer
/// to 100µs (non-crossOriginIsolated) and reports floats; an integral
/// Date.now()-based value is a timing-entropy tell on the jsvmp environment
/// scan path. Quantized to 0.1ms steps — epoch-ms times ten stays well under
/// 2^53, so the f64 rounding is exact (finer quantization would not be).
#[diting_op]
pub(super) fn op_clock_ms() -> f64 {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
        * 1000.0;
    (ms * 10.0).round() / 10.0
}

