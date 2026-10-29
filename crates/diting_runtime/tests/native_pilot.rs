//! Native-interface pilot (DOMRectReadOnly / DOMRect) - own test binary like
//! snapshot_restore.rs (snapshot creation is a whole-isolate exclusive op).
//!
//! The probe (tools/webplatform/chrome/native-pilot-probe.js) ran in real
//! Chrome 145 -> native-pilot-truth-145.json. Here the SAME probe runs in a
//! bare V8 isolate whose DOMRect is a Rust native interface, both through
//! the snapshot path (bake -> restore) and the fresh-VM path. Every probe
//! must equal Chrome's, except the documented bare-isolate allowances.

use diting_runtime::vm::{create_snapshot, Vm};
use std::collections::BTreeMap;
use std::sync::Mutex;

static ISOLATE_LOCK: Mutex<()> = Mutex::new(());

const PROBE: &str = include_str!("../../../tools/webplatform/chrome/native-pilot-probe.js");
const TRUTH: &str = include_str!("../../../tools/webplatform/chrome/native-pilot-truth-145.json");
const DEX_PROBE: &str = include_str!("../../../tools/webplatform/chrome/domexception-probe.js");
const DEX_TRUTH: &str = include_str!("../../../tools/webplatform/chrome/domexception-truth-145.json");

/// Probes a bare isolate cannot answer (no structuredClone, etc.).
const ALLOWED_BARE: &[&str] = &["structuredClone"];

fn run_probe(vm: &mut Vm) -> BTreeMap<String, serde_json::Value> {
    run(vm, PROBE)
}

fn run(vm: &mut Vm, probe: &str) -> BTreeMap<String, serde_json::Value> {
    let out = vm.eval_string(probe).expect("probe runs");
    serde_json::from_str(&out).expect("probe returns JSON")
}

fn diff(got: &BTreeMap<String, serde_json::Value>) -> Vec<String> {
    diff_against(got, TRUTH)
}

fn diff_against(got: &BTreeMap<String, serde_json::Value>, truth_json: &str) -> Vec<String> {
    let mut truth: BTreeMap<String, serde_json::Value> = serde_json::from_str(truth_json).unwrap();
    // Known gap: DOMPointReadOnly.matrixTransform needs DOMMatrix (not native yet).
    if let Some(shape) = truth.get_mut("shape.PointRO") {
        shape["members"].as_object_mut().unwrap().remove("matrixTransform");
        let order = shape["order"].as_array_mut().unwrap();
        order.retain(|v| v != "matrixTransform");
    }
    let mut bad = Vec::new();
    for (k, want) in &truth {
        if ALLOWED_BARE.iter().any(|p| k.starts_with(p)) {
            continue;
        }
        if got.get(k) != Some(want) {
            bad.push(format!("{k}\n   chrome={want}\n   native={}", got.get(k).map(|v| v.to_string()).unwrap_or_default()));
        }
    }
    bad
}

#[test]
fn native_dom_rect_matches_chrome_after_snapshot_restore() {
    let _g = ISOLATE_LOCK.lock().unwrap();
    let blob: &'static [u8] = Box::leak(create_snapshot(&[], &[]).into());
    let mut vm = Vm::from_snapshot(blob).unwrap();
    let bad = diff(&run_probe(&mut vm));
    assert!(bad.is_empty(), "{} probes differ from Chrome:\n{}", bad.len(), bad.join("\n"));
}

#[test]
fn native_dom_rect_matches_chrome_in_fresh_vm() {
    let _g = ISOLATE_LOCK.lock().unwrap();
    let mut vm = Vm::new();
    let bad = diff(&run_probe(&mut vm));
    assert!(bad.is_empty(), "{} probes differ from Chrome:\n{}", bad.len(), bad.join("\n"));
}

/// DOMException (exception interface: Error-chained prototype, string slots,
/// legacy constants) - snapshot-restored and fresh, vs Chrome 145.
#[test]
fn native_dom_exception_matches_chrome() {
    let _g = ISOLATE_LOCK.lock().unwrap();
    let blob: &'static [u8] = Box::leak(create_snapshot(&[], &[]).into());
    for label in ["snapshot-restored", "fresh"] {
        // One isolate at a time (V8 requires reverse-order drops).
        let mut vm = if label == "fresh" { Vm::new() } else { Vm::from_snapshot(blob).unwrap() };
        let bad = diff_against(&run(&mut vm, DEX_PROBE), DEX_TRUTH);
        assert!(bad.is_empty(), "[{label}] {} DOMException probes differ from Chrome:\n{}", bad.len(), bad.join("\n"));
    }
}

/// Pilot measurements (run with `--ignored --nocapture`): snapshot size,
/// restore latency and per-object cost, native vs an equivalent JS class.
#[test]
#[ignore]
fn pilot_measurements() {
    let _g = ISOLATE_LOCK.lock().unwrap();
    let blob = create_snapshot(&[], &[]);
    println!("snapshot bytes (with native DOMRect pair): {}", blob.len());
    let blob: &'static [u8] = Box::leak(blob.into());
    let t = std::time::Instant::now();
    for _ in 0..20 {
        let _vm = Vm::from_snapshot(blob).unwrap();
    }
    println!("restore: {:?} / isolate", t.elapsed() / 20);

    let js_class = r#"
      globalThis.JsRect = (function(){
        const slots = new WeakMap();
        class JsRect { constructor(x=0,y=0,w=0,h=0){ slots.set(this,[+x,+y,+w,+h]); }
          get x(){ return slots.get(this)[0]; } get y(){ return slots.get(this)[1]; }
          get width(){ return slots.get(this)[2]; } get height(){ return slots.get(this)[3]; }
          get right(){ const s=slots.get(this); return Math.max(s[0], s[0]+s[2]); } }
        return JsRect; })();
    "#;
    let mut vm = Vm::from_snapshot(blob).unwrap();
    vm.eval_string(js_class).unwrap();
    for (label, ctor) in [("native DOMRect", "DOMRect"), ("JS class (WeakMap slots)", "JsRect")] {
        let script = format!(
            "(function(){{ var s=0; for (var i=0;i<300000;i++){{ var r=new {ctor}(i,2,3,4); s+=r.x+r.width+r.right; }} return String(s); }})()"
        );
        vm.eval_string(&script).unwrap(); // warm
        let t = std::time::Instant::now();
        vm.eval_string(&script).unwrap();
        println!("{label}: {:?} per create+3 reads (300k iterations)", t.elapsed() / 300_000);
    }
}
