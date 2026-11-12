// web 存储与字节仓：localStorage 持久化 + cookie 双 op + blob 注册 + 图片字节 — split out of the god file (ratchet, #75).
// Pure move: the functions see the same ops::* namespace they grew up in.
use super::*;

/// Mirror a `URL.createObjectURL` registration into the Rust-side blob
/// table. The JS-side `__blobObjs` map is unreachable from the module
/// loader (which resolves outside the realm), so `import("blob:…")` and
/// `<script type=module src="blob:…">` resolve through this mirror.
#[diting_op]
pub(super) fn op_blob_register(
    state: &OpState,
    #[string] id: String,
    #[buffer] bytes: &[u8],
    #[string] mime: String,
) {
    let shared = state.borrow::<SharedState>().clone();
    shared
        .borrow()
        .blob_store
        .borrow_mut()
        .insert(id, (bytes.to_vec(), mime));
}

/// Drop a mirrored blob registration (`URL.revokeObjectURL`).
#[diting_op]
pub(super) fn op_blob_revoke(state: &OpState, #[string] id: String) {
    let shared = state.borrow::<SharedState>().clone();
    shared.borrow().blob_store.borrow_mut().remove(&id);
}

/// Insert a fetched image body into [`JsState::image_bytes`] (FIFO eviction
/// at the entry cap) and drop `layout_cache`: placeholder boxes and real
/// intrinsic sizes can reflow differently, so memoized geometry from the
/// pre-fetch run must not survive the insert.
#[cfg(feature = "screenshot")]
pub fn store_image_bytes(gs: &mut JsState, url: String, bytes: Vec<u8>) {
    const IMAGE_TABLE_CAP: usize = 64;
    let mut map = gs.image_bytes.borrow_mut();
    if map.contains_key(&url) {
        return;
    }
    if map.len() >= IMAGE_TABLE_CAP {
        if let Some(oldest) = gs.image_order.borrow_mut().pop_front() {
            map.remove(&oldest);
        }
    }
    map.insert(url.clone(), std::sync::Arc::new(bytes));
    drop(map);
    gs.image_order.borrow_mut().push_back(url);
    gs.drop_layout();
}

#[diting_op]
#[string]
pub(super) fn op_get_cookies(state: &OpState) -> String {
    let gs = state.borrow::<SharedState>().clone();
    let gs = gs.borrow();
    let jar = match &gs.cookie_jar {
        Some(j) => j,
        None => return String::new(),
    };
    let url = match url::Url::parse(&gs.url) {
        Ok(u) => u,
        Err(_) => return String::new(),
    };
    jar.get_js_visible_cookies(&url)
}

#[diting_op]
pub(super) fn op_set_cookie(state: &OpState, #[string] cookie_str: &str) {
    let gs = state.borrow::<SharedState>().clone();
    let gs = gs.borrow();
    let jar = match &gs.cookie_jar {
        Some(j) => j,
        None => return,
    };
    let url = match url::Url::parse(&gs.url) {
        Ok(u) => u,
        Err(_) => return,
    };
    jar.set_cookie_from_js(cookie_str, &url);
}

// localStorage persistence (obscura#629 class): the bootstrap store is a
// plain JS map, so logins/flags a page keeps in localStorage died with the
// process. The store now loads through op_storage_read on first access and
// flushes (debounced JS-side) through op_storage_write; the authoritative
// copy lives in this process-global map, mirrored to one JSON file per
// origin under DITING_STORAGE_DIR (falling back to the cookie store
// dir). sessionStorage deliberately stays memory-only — per-tab lifetime is
// the spec'd behavior. Both locks recover from poison (#336): they are
// process-wide, and the guarded sections are single insert/get calls.
pub(super) static LOCAL_STORAGE: std::sync::LazyLock<
    std::sync::Mutex<HashMap<String, HashMap<String, String>>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));
pub(super) static LOCAL_STORAGE_LOADED: std::sync::LazyLock<std::sync::Mutex<HashSet<String>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(HashSet::new()));

pub(super) fn storage_file(origin: &str) -> Option<std::path::PathBuf> {
    if diting_env::env_knobs::ephemeral() {
        return None;
    }
    let dir = std::env::var("DITING_STORAGE_DIR")
        .ok()
        .or_else(|| std::env::var("DITING_COOKIE_STORE_DIR").ok())
        .or_else(|| diting_env::env_knobs::app_data_dir().map(|p| p.to_string_lossy().into_owned()))
        .unwrap_or_else(|| ".".to_string());
    let mut sanitized = String::with_capacity(origin.len());
    for c in origin.chars() {
        if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
            sanitized.push(c);
        } else {
            sanitized.push('_');
        }
    }
    // FNV-1a suffix: sanitized names collide ("a:b" vs "a_b"); the hash does not.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in origin.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    Some(
        std::path::Path::new(&dir)
            .join("localStorage")
            .join(format!("{sanitized}-{:016x}.json", hash)),
    )
}

pub(super) fn storage_load_locked(origin: &str) {
    let mut loaded = LOCAL_STORAGE_LOADED
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if !loaded.insert(origin.to_string()) {
        return;
    }
    let Some(path) = storage_file(origin) else {
        return;
    };
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return;
    };
    let Ok(entries) = serde_json::from_str::<HashMap<String, String>>(&raw) else {
        return;
    };
    LOCAL_STORAGE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(origin.to_string(), entries);
}

#[diting_op]
#[string]
pub(super) fn op_storage_read(#[string] origin: &str) -> String {
    storage_load_locked(origin);
    let map = LOCAL_STORAGE.lock().unwrap_or_else(PoisonError::into_inner);
    let entries = map.get(origin);
    serde_json::to_string(&entries.unwrap_or(&HashMap::new())).unwrap_or_else(|_| "{}".to_string())
}

#[diting_op]
pub(super) fn op_storage_write(#[string] origin: &str, #[string] json: &str) {
    let Ok(entries) = serde_json::from_str::<HashMap<String, String>>(json) else {
        return;
    };
    // Load-first: a write for a never-read origin must not be clobbered by
    // the on-disk copy when the reader side initializes later.
    storage_load_locked(origin);
    LOCAL_STORAGE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(origin.to_string(), entries);
    let Some(path) = storage_file(origin) else {
        return;
    };
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            // An unwritable store must be loud (obscura#855 item 1 shape):
            // silently returning here loses every subsequent localStorage
            // write with zero diagnostics.
            tracing::warn!(
                "localStorage persist: create_dir_all({}) failed: {}",
                parent.display(),
                e
            );
            return;
        }
    }
    let payload = LOCAL_STORAGE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(origin)
        .map(|entries| serde_json::to_string(entries).unwrap_or_else(|_| "{}".to_string()));
    let Some(payload) = payload else { return };
    // tmp + rename so a mid-write kill never leaves a truncated store;
    // 0600 to match the cookie store — this file carries login tokens too
    // (the xinzao-class session shape).
    let tmp = path.with_extension("tmp");
    #[cfg(unix)]
    let write_ok = {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)
            .and_then(|mut f| f.write_all(payload.as_bytes()))
            .is_ok()
    };
    // Windows has no mode bits; the file lives in the per-user app-data
    // directory (config::app_data_dir), whose ACLs already scope it to the
    // user.
    #[cfg(not(unix))]
    let write_ok = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&tmp)
        .and_then(|mut f| f.write_all(payload.as_bytes()))
        .is_ok();
    if write_ok {
        if let Err(e) = std::fs::rename(&tmp, &path) {
            tracing::warn!(
                "localStorage persist: rename to {} failed: {}",
                path.display(),
                e
            );
        }
    } else {
        tracing::warn!("localStorage persist: write to {} failed", tmp.display());
    }
}

#[cfg(test)]
pub(crate) fn reset_local_storage_for_tests() {
    LOCAL_STORAGE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
    LOCAL_STORAGE_LOADED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
}

