//! Native DOMMatrix pilot — same pattern as native_pilot.rs: the Chrome
//! geometry probe runs in a bare isolate whose DOMMatrix/DOMMatrixReadOnly
//! are Rust-native, through both the snapshot path and a fresh VM. Only
//! the matrix+quad keys are diffed here: the
//! WebKitCSSMatrix alias is not native yet, and shape keys cover the
//! whole geometry family.

use diting_runtime::vm::{create_snapshot, Vm};
use std::collections::BTreeMap;
use std::sync::Mutex;

static ISOLATE_LOCK: Mutex<()> = Mutex::new(());

const PROBE: &str = include_str!("../../../tools/webplatform/chrome/geometry-probe.js");
const TRUTH: &str = include_str!("../../../tools/webplatform/chrome/geometry-truth-157.json");

fn run(vm: &mut Vm) -> BTreeMap<String, serde_json::Value> {
    let out = vm.eval_string(PROBE).expect("probe runs");
    serde_json::from_str(&out).expect("probe returns JSON")
}

fn diff(got: &BTreeMap<String, serde_json::Value>) -> Vec<String> {
    let truth: BTreeMap<String, serde_json::Value> = serde_json::from_str(TRUTH).unwrap();
    let mut bad = Vec::new();
    for (k, want) in &truth {
        // matrix keys only — quad/point/rect/alias land separately.
        if !(k.starts_with("dm.")
            || k.starts_with("dmro.")
            || k.starts_with("ro.")
            || k.starts_with("dq.")
            || k.starts_with("ar.")
            || k.starts_with("wkcm."))
        {
            continue;
        }
        if got.get(k) != Some(want) {
            bad.push(format!(
                "{k}\n   chrome={want}\n   native={}",
                got.get(k).map(|v| v.to_string()).unwrap_or_default()
            ));
        }
    }
    bad
}

#[test]
fn native_matrix_matches_chrome_after_snapshot_restore() {
    let _g = ISOLATE_LOCK.lock().unwrap();
    let blob: &'static [u8] = Box::leak(create_snapshot(&[], &[]).into());
    let mut vm = Vm::from_snapshot(blob).unwrap();
    let bad = diff(&run(&mut vm));
    assert!(
        bad.is_empty(),
        "{} matrix probes differ from Chrome:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

#[test]
fn native_matrix_matches_chrome_in_fresh_vm() {
    let _g = ISOLATE_LOCK.lock().unwrap();
    let mut vm = Vm::new();
    let bad = diff(&run(&mut vm));
    assert!(
        bad.is_empty(),
        "{} matrix probes differ from Chrome:\n{}",
        bad.len(),
        bad.join("\n")
    );
}
