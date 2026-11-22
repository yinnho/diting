//! Per-host route memory (aginxos REQ #414, lifted out of search/dual_path.rs
//! 2026-11-22): which hosts' direct leg keeps dying, and which the proxy leg
//! carried after direct failed. One table for every transport — fetch,
//! download, render, robots, search all consult it through
//! `env_knobs::should_auto_proxy`, and it survives a process restart:
//! persisted to `app_data_dir/route-table.json` (0600, atomic tmp+rename, the
//! cookie store's discipline). Ephemeral instances keep it in memory only —
//! the benchmark fleet's learning is real within a run but writes nothing.
//!
//! Semantics (unchanged from dual_path): a direct success clears the host; a
//! direct failure bumps the streak and sentences the host at
//! [`DIRECT_FAIL_THRESHOLD`] for [`DIRECT_SKIP_SECS`]. A proxy leg that
//! carries the page after direct failed ([`note_proxy_win`]) is stronger
//! evidence than a bare probe failure, so it sentences longer
//! ([`PROXY_WIN_SKIP_SECS`]). Sentences expire and the next direct probe is
//! allowed through — networks change.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Consecutive direct failures before a host is sentenced to proxy-only.
pub const DIRECT_FAIL_THRESHOLD: u8 = 2;
/// How long a probe-failure sentence lasts; any direct success lifts it.
pub const DIRECT_SKIP_SECS: u64 = 600;
/// Proxy-confirmed sentence: the proxy leg carrying the page after direct
/// died is proof, not suspicion — the sentence runs six hours, not minutes.
pub const PROXY_WIN_SKIP_SECS: u64 = 6 * 3600;
/// Hosts tracked before eviction (expired first, then soonest-to-expire).
pub const DIRECT_FAILS_CAP: usize = 64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DirectFailState {
    pub fails: u8,
    pub skip_until: u64,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Host part of a URL — the table's key shape. None on unparseable input.
pub fn host_of(url: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
}

/// Seconds left on a host's proxy-only sentence (0 = probe allowed). Pure
/// half — the global path feeds it the shared map, tests feed their own.
pub fn direct_skip_remaining(map: &HashMap<String, DirectFailState>, host: &str, now: u64) -> u64 {
    match map.get(host) {
        Some(s) if s.skip_until > now => s.skip_until - now,
        _ => 0,
    }
}

/// Record a direct-probe outcome over any map. Ok clears the host; a fail
/// bumps the streak, sentencing it once the threshold is reached, and the
/// map is capped (expired entries first, then the soonest-to-expire).
pub fn note_direct_outcome(
    map: &mut HashMap<String, DirectFailState>,
    host: &str,
    ok: bool,
    now: u64,
) {
    if ok {
        map.remove(host);
        return;
    }
    let st = map.entry(host.to_string()).or_insert(DirectFailState {
        fails: 0,
        skip_until: 0,
    });
    st.fails = st.fails.saturating_add(1);
    if st.fails >= DIRECT_FAIL_THRESHOLD {
        st.skip_until = now.saturating_add(DIRECT_SKIP_SECS);
    }
    if map.len() > DIRECT_FAILS_CAP {
        // Expired sentences are dead weight; an unsentenced streak
        // (skip_until == 0) is live evidence — same rule as load_table.
        map.retain(|_, s| s.skip_until > now || s.skip_until == 0);
        while map.len() > DIRECT_FAILS_CAP {
            // The host this call just noted is excluded — fresh evidence
            // always lands; the weakest OTHER entry goes.
            let Some(k) = map
                .iter()
                .filter(|(k, _)| k.as_str() != host)
                .min_by_key(|(_, s)| (s.skip_until, s.fails))
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            map.remove(&k);
        }
    }
}

/// The table file: `DITING_STORAGE_DIR` / `DITING_COOKIE_STORE_DIR` pin an
/// isolated instance's table (same isolation contract as cache.db and the
/// cookie jar); everyone else shares `app_data_dir/route-table.json`.
/// Ephemeral instances get `None` — memory only, zero disk touch.
fn table_path() -> Option<PathBuf> {
    if crate::env_knobs::ephemeral() {
        return None;
    }
    if let Ok(dir) =
        std::env::var("DITING_STORAGE_DIR").or_else(|_| std::env::var("DITING_COOKIE_STORE_DIR"))
    {
        if !dir.is_empty() {
            return Some(PathBuf::from(dir).join("route-table.json"));
        }
    }
    crate::env_knobs::app_data_dir().map(|d| d.join("route-table.json"))
}

fn load_table() -> HashMap<String, DirectFailState> {
    let Some(path) = table_path() else {
        return HashMap::new();
    };
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return HashMap::new();
    };
    let mut map: HashMap<String, DirectFailState> = serde_json::from_str(&raw).unwrap_or_default();
    // Expired sentences are dead weight on both memory and disk.
    let now = now_secs();
    map.retain(|_, s| s.skip_until > now || s.fails > 0 && s.skip_until == 0);
    map
}

/// Atomic, owner-only write (tmp + rename) — legacy_data's write_private
/// discipline, so a crash mid-write never leaves a torn table.
fn persist(map: &HashMap<String, DirectFailState>) {
    let Some(path) = table_path() else {
        return;
    };
    let Ok(body) = serde_json::to_string(map) else {
        return;
    };
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, body.as_bytes()).is_err() {
        return;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    let _ = std::fs::rename(&tmp, &path);
}

static TABLE: std::sync::OnceLock<std::sync::Mutex<HashMap<String, DirectFailState>>> =
    std::sync::OnceLock::new();

fn with_table<R>(f: impl FnOnce(&mut HashMap<String, DirectFailState>) -> R) -> R {
    let m = TABLE.get_or_init(|| std::sync::Mutex::new(load_table()));
    let mut map = m.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut map)
}

/// Seconds left on a host's proxy-only sentence (0 = direct allowed).
/// Read path — consults the shared table.
pub fn skip_direct_remaining(host: &str) -> u64 {
    with_table(|m| direct_skip_remaining(m, host, now_secs()))
}

/// A direct leg's outcome. Success lifts any sentence (and only persists
/// when an entry actually existed); failure bumps the streak, sentencing at
/// the threshold.
pub fn note_direct(host: &str, ok: bool) {
    with_table(|m| {
        let changed = if ok {
            m.remove(host).is_some()
        } else {
            note_direct_outcome(m, host, false, now_secs());
            true
        };
        if changed {
            persist(m);
        }
    });
}

/// The proxy leg carried the page after direct failed (transport death or a
/// geo-stub body) — proof, not suspicion. Sentence the host to proxy for
/// [`PROXY_WIN_SKIP_SECS`].
pub fn note_proxy_win(host: &str) {
    with_table(|m| {
        m.insert(
            host.to_string(),
            DirectFailState {
                fails: DIRECT_FAIL_THRESHOLD,
                skip_until: now_secs().saturating_add(PROXY_WIN_SKIP_SECS),
            },
        );
        persist(m);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fail_streak_sentences_at_threshold_and_success_lifts() {
        let mut m = HashMap::new();
        note_direct_outcome(&mut m, "x.com", false, 1000);
        assert_eq!(direct_skip_remaining(&m, "x.com", 1000), 0);
        note_direct_outcome(&mut m, "x.com", false, 1001);
        assert_eq!(direct_skip_remaining(&m, "x.com", 1001), DIRECT_SKIP_SECS);
        note_direct_outcome(&mut m, "x.com", true, 1002);
        assert_eq!(direct_skip_remaining(&m, "x.com", 1002), 0);
        assert!(!m.contains_key("x.com"));
    }

    #[test]
    fn sentence_expires_and_cap_evicts_soonest() {
        let mut m = HashMap::new();
        for i in 0..DIRECT_FAILS_CAP {
            let h = format!("h{i}.example");
            note_direct_outcome(&mut m, &h, false, 1000 + i as u64);
            note_direct_outcome(&mut m, &h, false, 1000 + i as u64);
        }
        assert_eq!(m.len(), DIRECT_FAILS_CAP);
        // One more host over the cap, noted while every sentence is still
        // inside its window (they run 1600..1663): the soonest-to-expire
        // (h0) is evicted, the fresh evidence lands.
        note_direct_outcome(&mut m, "overflow.example", false, 1100);
        note_direct_outcome(&mut m, "overflow.example", false, 1100);
        assert_eq!(m.len(), DIRECT_FAILS_CAP);
        assert!(!m.contains_key("h0.example"));
        assert!(m.contains_key("overflow.example"));
        // Expired entries stop sentencing.
        let past = 1000 + DIRECT_SKIP_SECS + 1;
        assert_eq!(direct_skip_remaining(&m, "h1.example", past + 1), 0);
    }

    #[test]
    fn file_roundtrip_and_corrupt_tolerance() {
        let dir =
            std::env::temp_dir().join(format!("diting-route-table-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("route-table.json");
        let mut m = HashMap::new();
        note_direct_outcome(&mut m, "a.example", false, 1000);
        note_direct_outcome(&mut m, "a.example", false, 1001);
        std::fs::write(&path, serde_json::to_string(&m).unwrap()).unwrap();
        let back: HashMap<String, DirectFailState> =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(back, m);
        std::fs::write(&path, b"not json").unwrap();
        let corrupt: HashMap<String, DirectFailState> =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap_or_default();
        assert!(corrupt.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
