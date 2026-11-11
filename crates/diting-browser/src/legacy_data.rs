//! Compat bridge for the app-data directory (the diting rename, v0.10.0).
//!
//! The rename moved the default home of credential-bearing state from
//! `…/aginxbrowser` to `…/diting` and nothing carried the old one over: an
//! upgraded install read the new, near-empty directory and every site came
//! back logged out — the old jar still held the logins (found with x.com:
//! auth_token/ct0/twid sat in `aginxbrowser/cookie-store.json` while the
//! engine read guest cookies only). This merges the old state in ONCE:
//!
//! - cookie jar: old cookies whose (domain, path, name) the new jar lacks are
//!   appended — the new jar wins every collision (it is the fresher state);
//! - localStorage: per-origin files the new directory lacks are copied;
//! - the old directory is left untouched; a marker in the new one makes it a
//!   one-shot.
//!
//! Only the DEFAULT location is bridged: an explicit DITING_COOKIE_STORE_DIR /
//! DITING_STORAGE_DIR or an ephemeral run never touches it. Remove after one
//! migration cycle, like the env-var bridge in main.rs.

use std::path::{Path, PathBuf};

const MARKER: &str = ".migrated-from-aginxbrowser";

/// The pre-rename default directory, beside the current one.
fn legacy_dir(current: &Path) -> Option<PathBuf> {
    let parent = current.parent()?;
    let name = current.file_name()?.to_str()?;
    if name != "diting" {
        return None;
    }
    Some(parent.join("aginxbrowser"))
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Migrated {
    pub cookies: usize,
    pub storage_files: usize,
}

/// (old, current) when the bridge applies to this process at all.
fn dirs() -> Option<(PathBuf, PathBuf)> {
    if crate::config::ephemeral()
        || std::env::var_os("DITING_COOKIE_STORE_DIR").is_some()
        || std::env::var_os("DITING_STORAGE_DIR").is_some()
    {
        return None;
    }
    let current = crate::config::app_data_dir()?;
    Some((legacy_dir(&current)?, current))
}

/// The pre-rename directory still waiting for the next server start
/// (doctor reports it: a user who reads "logged out everywhere" there gets
/// told where the logins are).
pub(crate) fn pending() -> Option<PathBuf> {
    let (old, current) = dirs()?;
    (old.is_dir() && !current.join(MARKER).exists()).then_some(old)
}

/// Run the bridge for the process's default app-data directory.
pub(crate) fn run() {
    let Some((old, current)) = dirs() else {
        return;
    };
    match migrate(&old, &current) {
        Ok(Some(m)) => tracing::info!(
            "carried pre-rename state over from {}: {} cookie(s), {} localStorage origin file(s)",
            old.display(),
            m.cookies,
            m.storage_files
        ),
        Ok(None) => {}
        Err(e) => tracing::warn!(
            "pre-rename state migration from {} failed: {}",
            old.display(),
            e
        ),
    }
}

/// Merge `old` into `current`. `Ok(None)` = nothing to do (no old directory,
/// or already migrated).
pub(crate) fn migrate(old: &Path, current: &Path) -> std::io::Result<Option<Migrated>> {
    if !old.is_dir() || current.join(MARKER).exists() {
        return Ok(None);
    }
    std::fs::create_dir_all(current)?;
    let mut m = Migrated::default();

    let old_jar = old.join("cookie-store.json");
    if old_jar.is_file() {
        let new_jar = current.join("cookie-store.json");
        let mut merged = read_jar(&new_jar)?;
        let key = |c: &serde_json::Value| {
            (
                c.get("domain")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_ascii_lowercase(),
                c.get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("/")
                    .to_string(),
                c.get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            )
        };
        let have: std::collections::HashSet<_> = merged.iter().map(key).collect();
        for c in read_jar(&old_jar)? {
            if !have.contains(&key(&c)) {
                merged.push(c);
                m.cookies += 1;
            }
        }
        if m.cookies > 0 {
            let body = serde_json::to_string(&merged).map_err(std::io::Error::other)?;
            write_private(&new_jar, body.as_bytes())?;
        }
    }

    let old_ls = old.join("localStorage");
    if old_ls.is_dir() {
        let new_ls = current.join("localStorage");
        std::fs::create_dir_all(&new_ls)?;
        for entry in std::fs::read_dir(&old_ls)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let dst = new_ls.join(entry.file_name());
            if !dst.exists() {
                write_private(&dst, &std::fs::read(entry.path())?)?;
                m.storage_files += 1;
            }
        }
    }

    write_private(
        &current.join(MARKER),
        format!("cookies={} storage_files={}\n", m.cookies, m.storage_files).as_bytes(),
    )?;
    Ok(Some(m))
}

/// A jar file as its cookie list; missing = empty, unreadable = error (a
/// corrupt jar must not be silently replaced).
fn read_jar(path: &Path) -> std::io::Result<Vec<serde_json::Value>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = std::fs::read_to_string(path)?;
    serde_json::from_str(&data).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// Atomic, owner-only write (tmp + rename), the jar's own discipline.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("migrating");
    std::fs::write(&tmp, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cookie(domain: &str, name: &str, value: &str) -> serde_json::Value {
        serde_json::json!({
            "name": name, "value": value, "domain": domain, "path": "/",
            "secure": true, "httpOnly": true, "sameSite": "Lax", "expires": -1.0
        })
    }

    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "diting-legacy-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn pre_rename_state_is_merged_once_and_the_new_jar_wins() {
        let root = scratch("merge");
        let old = root.join("aginxbrowser");
        let new = root.join("diting");
        std::fs::create_dir_all(old.join("localStorage")).unwrap();
        std::fs::create_dir_all(new.join("localStorage")).unwrap();
        std::fs::write(
            old.join("cookie-store.json"),
            serde_json::to_string(&vec![
                cookie(".x.com", "auth_token", "old-login"),
                cookie(".x.com", "guest_id", "old-guest"),
            ])
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            new.join("cookie-store.json"),
            serde_json::to_string(&vec![cookie(".x.com", "guest_id", "new-guest")]).unwrap(),
        )
        .unwrap();
        std::fs::write(old.join("localStorage/a.json"), "{\"k\":\"old\"}").unwrap();
        std::fs::write(old.join("localStorage/b.json"), "{\"k\":\"old\"}").unwrap();
        std::fs::write(new.join("localStorage/b.json"), "{\"k\":\"new\"}").unwrap();

        let m = migrate(&old, &new).unwrap().unwrap();
        assert_eq!(
            m,
            Migrated {
                cookies: 1,
                storage_files: 1
            }
        );
        let jar = read_jar(&new.join("cookie-store.json")).unwrap();
        let value = |n: &str| {
            jar.iter()
                .find(|c| c["name"] == n)
                .map(|c| c["value"].as_str().unwrap().to_string())
        };
        assert_eq!(
            value("auth_token").as_deref(),
            Some("old-login"),
            "the lost login comes back"
        );
        assert_eq!(
            value("guest_id").as_deref(),
            Some("new-guest"),
            "the new jar wins a collision"
        );
        assert_eq!(jar.len(), 2);
        assert_eq!(
            std::fs::read_to_string(new.join("localStorage/b.json")).unwrap(),
            "{\"k\":\"new\"}",
            "an origin the new directory already has is kept"
        );
        assert!(new.join("localStorage/a.json").is_file());
        assert!(
            old.join("cookie-store.json").is_file(),
            "the old directory is left as it was"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(new.join("cookie-store.json"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }

        // One-shot: a second start changes nothing, even if the new jar was
        // since emptied by the user (logging out must stay logged out).
        std::fs::write(new.join("cookie-store.json"), "[]").unwrap();
        assert_eq!(migrate(&old, &new).unwrap(), None);
        assert_eq!(read_jar(&new.join("cookie-store.json")).unwrap().len(), 0);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn no_old_directory_and_corrupt_jars_are_left_alone() {
        let root = scratch("edge");
        let new = root.join("diting");
        assert_eq!(migrate(&root.join("aginxbrowser"), &new).unwrap(), None);
        assert!(!new.join(MARKER).exists());

        let old = root.join("aginxbrowser");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(old.join("cookie-store.json"), "[]").unwrap();
        std::fs::write(new.join("cookie-store.json"), "{not json").unwrap();
        assert!(
            migrate(&old, &new).is_err(),
            "a corrupt jar is reported, not overwritten"
        );
        assert_eq!(
            std::fs::read_to_string(new.join("cookie-store.json")).unwrap(),
            "{not json"
        );
        assert!(
            !new.join(MARKER).exists(),
            "a failed run retries next start"
        );

        assert_eq!(
            legacy_dir(Path::new("/a/diting")),
            Some(PathBuf::from("/a/aginxbrowser"))
        );
        assert_eq!(legacy_dir(Path::new("/a/other")), None);
        let _ = std::fs::remove_dir_all(&root);
    }
}
