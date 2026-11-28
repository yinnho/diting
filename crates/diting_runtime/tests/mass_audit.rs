//! Shell-mass audit (native-migration planning): bake PREFIXES of the
//! production snapshot script list and measure what each segment adds to
//! restore cost. Prefixes are valid because the scripts only touch earlier
//! bindings at call time, never at bake time (build.rs header law).
//!
//!   cargo test -p diting_runtime --test mass_audit -- --nocapture --test-threads=1
//!
//! Snapshot creation is a whole-isolate exclusive op, so this is its own
//! test binary, one test, everything sequential.

use diting_runtime::vm::{create_snapshot, Vm};
use std::panic::catch_unwind;
use std::time::Instant;

fn script_list() -> Vec<(String, String)> {
    // Parse the ordered (name, include_str!(path)) pairs out of build.rs —
    // the same table the production bake uses, in the same order.
    let build_rs = include_str!("../../diting_js/build.rs");
    let mut out = Vec::new();
    let re_start = "include_str!(\"js/";
    // deno_shell is inline glue, not a file — replicate it verbatim.
    out.push((
        "deno_shell".to_string(),
        "globalThis.Deno = { core: { ops: {}, setUnhandledPromiseRejectionHandler: __diting_set_unhandled_rejection_handler, setHandledPromiseRejectionHandler: __diting_set_handled_rejection_handler } };".to_string(),
    ));
    for line in build_rs.lines() {
        let line = line.trim_start();
        let Some(rest) = line.strip_prefix("(\"") else {
            continue;
        };
        let Some((name, rest)) = rest.split_once('"') else {
            continue;
        };
        let Some(at) = rest.find(re_start) else {
            continue;
        };
        let Some(path) = rest[at + re_start.len()..].split('"').next() else {
            continue;
        };
        let disk = format!("{}/../diting_js/js/{}", env!("CARGO_MANIFEST_DIR"), path);
        match std::fs::read_to_string(&disk) {
            Ok(src) => out.push((name.to_string(), src)),
            Err(e) => eprintln!("skip {name}: {e}"),
        }
    }
    out
}

// Manual-run audit: debug-profile bakes of 14 growing prefixes are far too
// slow for CI (release is ~2min). Run on demand with --ignored.
#[test]
#[ignore]
fn mass_audit() {
    let scripts = script_list();
    eprintln!("loaded {} scripts", scripts.len());
    // Sample every 12th boundary plus the final full bake.
    let mut points: Vec<usize> = (1..=scripts.len()).step_by(12).collect();
    if !points.contains(&scripts.len()) {
        points.push(scripts.len());
    }
    let leaked: Vec<(&str, &str)> = scripts
        .iter()
        .map(|(n, s)| {
            (
                Box::leak(n.clone().into_boxed_str()) as &str,
                Box::leak(s.clone().into_boxed_str()) as &str,
            )
        })
        .collect();
    eprintln!("idx,scripts,bytes,restore_ms,heap_used_mb,blob_kb");
    let mut prev_restore = 0.0;
    let mut prev_bytes = 0usize;
    let mut prev_heap = 0.0;
    for &n in &points {
        let subset = &leaked[..n];
        let bytes: usize = subset.iter().map(|(_, s)| s.len()).sum();
        let blob = match catch_unwind(|| create_snapshot(subset, &[])) {
            Ok(b) => b,
            Err(_) => {
                eprintln!("{n},{n},-,BAKE-PANIC,-,-");
                continue;
            }
        };
        let blob_kb = blob.len() / 1024;
        let blob: &'static [u8] = Box::leak(blob.into());
        // Median-ish of 2 restores.
        let mut times = Vec::new();
        let mut heap = 0.0;
        for _ in 0..2 {
            let t = Instant::now();
            let mut vm = Vm::from_snapshot(blob).expect("prefix snapshot revives");
            times.push(t.elapsed().as_secs_f64() * 1e3);
            let stats = vm.heap_statistics();
            heap = stats.used_heap_size() as f64 / 1e6;
            drop(vm);
        }
        let restore = times[0].min(times[1]);
        eprintln!(
            "{n},{bytes},{restore:.2},+{:.2}ms,{heap:.1},+{:.1}MB,blob {blob_kb}K  [last={}]",
            restore - prev_restore,
            heap - prev_heap,
            scripts[n - 1].0
        );
        prev_restore = restore;
        prev_bytes = bytes;
        prev_heap = heap;
    }
    let _ = prev_bytes;
}
