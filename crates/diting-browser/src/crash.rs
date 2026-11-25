//! Panic persistence (aginxos REQ #414, device respawn #19): the process
//! died mid-search with zero trace anywhere — no OOM in dmesg, no panic on
//! stderr in the service log. This hook appends the panic payload, thread,
//! location and a forced backtrace to `{data}/crash.log` synchronously
//! before the default reporting runs, so a respawned process can read how
//! its predecessor died.
//!
//! Two caps keep one sick process from burying that answer. Each distinct
//! panic (message + location) is written once per process; its repeats cost
//! a count line at each power of ten. And a full file is rotated to
//! `crash.log.1` (one generation kept) — see [`append_entry`].
//!
//! SIGKILL deaths (OOM killer, `kill -9`) are outside a process's own
//! reach — exit-code/signal capture belongs to the supervising daemon and
//! is tracked on that side, not here.

use std::collections::HashMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};

/// A log at or past this size is rotated out before the next record lands.
const MAX_LOG_BYTES: u64 = 1 << 20;

/// Distinct panics remembered for folding. Past this a new one is still
/// written in full — just not remembered — so a storm of unique messages
/// can neither grow the table nor escape the file cap.
const MAX_REMEMBERED: usize = 64;

/// Panic identities already written to a log, with their repeat counts.
/// Panics arrive in storms: one process wrote 17,805 copies of a single
/// "RefCell already borrowed" in 84 seconds (2026-11-21), each with a forced
/// backtrace — 92.7 of the log's 93 MB was that one line of news, and the
/// 18 records of six other panics in it were buried.
/// Fold memory, per log file: a storm is bounded per path, and one test's
/// cap-filling log cannot evict another file's remembered identities.
static WRITTEN: std::sync::Mutex<Option<HashMap<PathBuf, HashMap<String, u64>>>> =
    std::sync::Mutex::new(None);

/// Install the crash-log hook. Writes go to `{app_data_dir}/crash.log`
/// (the credential-bearing state home — 0600 at creation on unix). When
/// no data dir resolves (stripped container without HOME/XDG), the default
/// hook stays; a missing log is better than a crash log next to the binary.
pub fn install() {
    let Some(dir) = crate::config::app_data_dir() else {
        return;
    };
    install_at(&dir);
}

/// [`install`] against an explicit directory (tests use a temp dir; the
/// hook itself stays process-global either way).
pub(crate) fn install_at(dir: &Path) {
    let log = dir.join("crash.log");
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let (identity, entry) = record(
            crate::now_secs(),
            info.payload(),
            info.location(),
            &std::thread::current(),
        );
        append_entry(&log, &identity, &entry);
        default_hook(info);
    }));
}

/// One crash record: what makes it *this* panic (message and location) and
/// the text. The timestamp stays out of the identity — a storm runs a few
/// panics a second, so it would start a new record every second.
/// `PanicHookInfo` cannot be constructed outside a real panic, so the hook
/// adapter extracts the pieces and the formatting stays testable.
fn record(
    now: u64,
    payload: &(dyn std::any::Any + Send),
    location: Option<&std::panic::Location<'_>>,
    thread: &std::thread::Thread,
) -> (String, String) {
    let msg = payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "<non-string panic payload>".to_string());
    let at = location
        .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
        .unwrap_or_else(|| "<unknown>".to_string());
    let entry = format!(
        "[{now}] diting {} pid {} panic in thread {:?}\n  msg: {msg}\n  at: {at}\n  backtrace:\n{}\n",
        env!("CARGO_PKG_VERSION"),
        std::process::id(),
        thread.name().unwrap_or("<unnamed>"),
        std::backtrace::Backtrace::force_capture()
    );
    let identity = format!("{} @ {at}", msg.replace('\n', " "));
    (identity, entry)
}

/// Synchronous append — the panic path must not depend on an async runtime
/// or a worker that may itself be wedged. Failures are silent by design:
/// the default hook still reports to stderr, which is the floor we found
/// missing on the device only because the process vanished faster than
/// stderr drained.
///
/// The first record of an identity lands in full; the first repeat and
/// each power of ten after it add one count line. Counting as it goes
/// (instead of a summary at the end) means nothing waits to be flushed
/// while the storm is still running, and the size reads right to within
/// 10×. The count line names the identity, so it still makes sense after
/// other records or a rotation have come between.
fn append_entry(path: &Path, identity: &str, entry: &str) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let repeats = {
        let mut written = WRITTEN
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let files = written.get_or_insert_with(HashMap::new);
        let seen = files.entry(path.to_path_buf()).or_default();
        let known = seen.contains_key(identity);
        let repeats = match seen.get_mut(identity) {
            Some(n) => {
                *n += 1;
                Some(*n)
            }
            None => None,
        };
        if !known && seen.len() < MAX_REMEMBERED {
            seen.insert(identity.to_string(), 0);
        }
        repeats
    };
    let line = match repeats {
        None => {
            rotate_if_full(path);
            format!("{entry}\n\n")
        }
        Some(n) if is_power_of_ten(n) => format!("(+ {n} more of: {identity})\n\n"),
        Some(_) => return,
    };
    #[cfg(unix)]
    let mut options = {
        use std::os::unix::fs::OpenOptionsExt as _;
        let mut o = std::fs::OpenOptions::new();
        o.mode(0o600);
        o
    };
    #[cfg(not(unix))]
    let mut options = std::fs::OpenOptions::new();
    if let Ok(mut f) = options.create(true).append(true).open(path) {
        let _ = f.write_all(line.as_bytes());
    }
}

/// Move a full log aside as `<name>.1`, replacing the generation there.
fn rotate_if_full(path: &Path) {
    let full = std::fs::metadata(path).is_ok_and(|m| m.len() >= MAX_LOG_BYTES);
    if !full {
        return;
    }
    let mut previous = path.file_name().unwrap_or_default().to_os_string();
    previous.push(".1");
    let _ = std::fs::rename(path, path.with_file_name(previous));
}

fn is_power_of_ten(n: u64) -> bool {
    let mut power = 1u64;
    loop {
        if power == n {
            return true;
        }
        match power.checked_mul(10) {
            Some(next) => power = next,
            None => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A fresh `{temp}/agx_crash_{tag}_{pid}/crash.log`. The tag keeps
    /// tests apart in one process: folding state is process-global and
    /// keyed by path.
    fn temp_log(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("agx_crash_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp crash dir");
        dir.join("crash.log")
    }

    fn done(path: &Path) {
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn record_carries_payload_version_and_thread() {
        let payload = "boom: ladder retracted".to_string();
        let thread = std::thread::current();
        let (identity, entry) = record(1_761_000_000, &payload, None, &thread);
        assert_eq!(identity, "boom: ladder retracted @ <unknown>");
        assert!(entry.contains("[1761000000]"));
        assert!(entry.contains(env!("CARGO_PKG_VERSION")));
        assert!(entry.contains("boom: ladder retracted"));
        assert!(entry.contains("at: <unknown>"));
        assert!(entry.contains("backtrace:"));
    }

    #[test]
    fn record_takes_str_payloads_too() {
        let payload: &str = "static boom";
        let thread = std::thread::current();
        // &&str coerces to &(dyn Any + Send) — `str` itself is !Sized, so
        // the reference &str is the Any referent, exactly the shape a
        // `panic!("literal")` payload arrives in.
        let (_, entry) = record(0, &payload, None, &thread);
        assert!(entry.contains("static boom"));
    }

    #[test]
    fn append_entry_writes_the_record() {
        let path = temp_log("append");
        append_entry(&path, "first @ a.rs:1:1", "first");
        append_entry(&path, "second @ a.rs:2:2", "second");
        let body = std::fs::read_to_string(&path).expect("crash log readable");
        assert!(body.contains("first"));
        assert!(body.contains("second"));
        done(&path);
    }

    // 2026-11-21: one process logged 17,805 copies of the same panic in 84
    // seconds (92.7 MB), about 210 a second — so the timestamp can't be
    // part of what makes two records "the same".
    #[test]
    fn repeats_of_a_panic_are_folded_whatever_their_timestamps() {
        let path = temp_log("storm");
        let identity = "RefCell already borrowed @ x.rs:4:33";
        for second in 0..12 {
            let entry = format!(
                "[{second}] diting 0.1.0 pid 7 panic in thread \"main\"\n  msg: RefCell already borrowed\n"
            );
            append_entry(&path, identity, &entry);
        }
        // Another panic in between doesn't un-fold the first.
        append_entry(&path, "other @ y.rs:9:1", "other");
        append_entry(&path, identity, "again");
        let body = std::fs::read_to_string(&path).expect("crash log readable");
        assert_eq!(
            body.matches("msg: RefCell already borrowed").count(),
            1,
            "{body}"
        );
        assert_eq!(body.matches("again").count(), 0, "{body}");
        assert!(body.contains("other"), "{body}");
        // The 1st and the 10th repeat; the 13th lands between.
        assert_eq!(body.matches("more of:").count(), 2, "{body}");
        assert!(
            body.contains("(+ 10 more of: RefCell already borrowed @ x.rs:4:33)"),
            "{body}"
        );
        done(&path);
    }

    #[test]
    fn past_the_remembered_cap_a_new_panic_is_still_written_in_full() {
        let path = temp_log("cap");
        for i in 0..MAX_REMEMBERED {
            append_entry(&path, &format!("p{i} @ a.rs:1:1"), "filler");
        }
        append_entry(&path, "over @ a.rs:2:2", "over");
        append_entry(&path, "over @ a.rs:2:2", "over");
        let body = std::fs::read_to_string(&path).expect("crash log readable");
        assert_eq!(body.matches("over").count(), 2, "{body}");
        done(&path);
    }

    #[test]
    fn a_full_log_rotates_and_keeps_one_generation() {
        let path = temp_log("rotate");
        let older = path.with_file_name("crash.log.1");
        std::fs::write(&path, vec![b'x'; MAX_LOG_BYTES as usize]).unwrap();
        append_entry(&path, "first @ a.rs:1:1", "after-rotate");
        assert_eq!(
            std::fs::read(&older)
                .expect("previous generation kept")
                .len(),
            MAX_LOG_BYTES as usize
        );
        // A second rotation replaces the generation instead of stacking .2.
        std::fs::write(&path, vec![b'y'; MAX_LOG_BYTES as usize]).unwrap();
        append_entry(&path, "second @ a.rs:2:2", "after-rotate-2");
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.ends_with("after-rotate-2\n\n"), "{body}");
        assert_eq!(
            std::fs::read_to_string(&older).unwrap(),
            "y".repeat(MAX_LOG_BYTES as usize)
        );
        assert!(!older.with_extension("2").exists());
        done(&path);
    }

    // End-to-end: install_at swaps the process hook (chained onto the
    // previous one, so later panics still report normally), a real panic
    // fires it, and the record lands on disk.
    #[test]
    fn installed_hook_persists_a_real_panic() {
        let dir = std::env::temp_dir().join(format!("agx_crash_e2e_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        install_at(&dir);
        let marker = format!("crash-test-marker-{}", std::process::id());
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            panic!("{marker}");
        }))
        .is_err();
        assert!(caught);
        let body = std::fs::read_to_string(dir.join("crash.log")).expect("crash log written");
        assert!(body.contains(&format!("crash-test-marker-{}", std::process::id())));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
