//! restore-time payoff measurement for the native geometry migration.
//! Manual-run (#[ignore]) — ledger number, not a CI gate.

#[test]
#[ignore]
fn geometry_restore_payoff() {
    super::super::v8_flags_before_first_isolate();
    diting_runtime::snapshot::ensure_script_consts(crate::snapshot_sources::SNAPSHOT_SRC_ASCII);
    for _ in 0..3 {
        let _ = diting_runtime::vm::Vm::from_snapshot(super::super::SNAPSHOT).unwrap();
    }
    let mut best = f64::INFINITY;
    for _ in 0..15 {
        let t = std::time::Instant::now();
        let vm = diting_runtime::vm::Vm::from_snapshot(super::super::SNAPSHOT).unwrap();
        let e = t.elapsed().as_secs_f64() * 1000.0;
        best = best.min(e);
        drop(vm);
    }
    println!(
        "new-native: best {best:.2}ms (blob {} bytes)",
        super::super::SNAPSHOT.len()
    );
    panic!("ledger: new-native-geometry best {best:.2}ms");
}
