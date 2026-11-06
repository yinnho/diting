//! Browser-profile import (#26): the cookies of a local Chrome or Firefox
//! profile, read off disk and handed to the account layer as export-form
//! Set-Cookie strings — the shape account records, `jar_for`'s cold seed and
//! the session `Cookies` write-back already share, so an imported login is
//! indistinguishable from one an account session captured itself.
//!
//! - Firefox: `cookies.sqlite` is plaintext (every platform).
//! - Chrome (macOS): `Cookies` sqlite, values in `encrypted_value` behind a
//!   `v10` tag: AES-128-CBC, IV = 16 spaces, key = PBKDF2-HMAC-SHA1(the
//!   Keychain's "Chrome Safe Storage" password, "saltysalt", 1003). Since
//!   cookie-db schema 24 the plaintext leads with SHA-256(host_key); the strip
//!   is self-validating (only when the 32 bytes ARE that hash) instead of
//!   keyed on the schema number. Windows (DPAPI + v20 app-bound) is phase 2.
//!
//! Profile dbs are copied (with their `-wal`) into a private TempDir before
//! opening: a running browser holds the db and keeps its newest writes in the
//! WAL. macOS guards other apps' data with TCC — a denied copy is reported as
//! the permission it is, not as "unable to open database file".
//!
//! Cookie values and the Keychain password are credentials: they never reach
//! a log line, an error string or the response (the report is counts only).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Browser {
    Chrome,
    Firefox,
}

impl Browser {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.to_ascii_lowercase().as_str() {
            "chrome" => Ok(Self::Chrome),
            "firefox" => Ok(Self::Firefox),
            _ => Err(format!("unsupported browser {s:?} (chrome | firefox)")),
        }
    }
}

/// One cookie as a profile db stores it, values already in the clear.
#[derive(Debug, Clone, PartialEq)]
struct Row {
    host: String,
    name: String,
    value: String,
    path: String,
    /// Unix seconds; None = session cookie.
    expires: Option<i64>,
    secure: bool,
    http_only: bool,
    /// "" (unspecified) | "None" | "Lax" | "Strict"
    same_site: &'static str,
}

impl Row {
    /// The export form (session/manager/actor.rs `Cookies`).
    fn to_set_cookie(&self) -> String {
        let mut s = format!(
            "{}={}; Domain={}; Path={}",
            self.name,
            self.value,
            self.host.trim_start_matches('.'),
            self.path
        );
        if let Some(date) = self.expires.and_then(crate::cookie::http_date) {
            s.push_str(&format!("; Expires={date}"));
        }
        if self.secure {
            s.push_str("; Secure");
        }
        if self.http_only {
            s.push_str("; HttpOnly");
        }
        if !self.same_site.is_empty() {
            s.push_str(&format!("; SameSite={}", self.same_site));
        }
        s
    }
}

/// What an import produced. `domains` counts imported cookies per registrable
/// host as stored (leading dot dropped) — metadata, never values.
#[derive(Debug, Default)]
pub struct Imported {
    pub cookies: Vec<String>,
    pub skipped_expired: usize,
    /// Encrypted values this build cannot open (non-v10 tags, wrong key).
    pub undecryptable: usize,
    /// Names/values a Set-Cookie line cannot carry (`;`, control bytes).
    pub skipped_invalid: usize,
    pub domains: BTreeMap<String, usize>,
}

/// Read `browser`'s `profile` (a profile directory name under the browser's
/// data root, or a path to the profile directory / cookie db; None = the
/// default profile), keeping cookies whose host matches one of `domains`
/// (exact or subdomain; empty = all).
pub fn import(browser: Browser, profile: Option<&str>, domains: &[String]) -> Result<Imported, String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let (rows, undecryptable) = match browser {
        Browser::Firefox => (read_firefox(&locate(browser, profile)?)?, 0),
        Browser::Chrome => {
            let db = locate(browser, profile)?;
            let key = chrome_key(&keychain_password()?);
            read_chrome(&db, &key)?
        }
    };
    let mut out = Imported { undecryptable, ..Default::default() };
    for r in rows {
        let host = r.host.trim_start_matches('.');
        if !domains.is_empty()
            && !domains.iter().any(|d| {
                let d = d.trim_start_matches('.');
                host == d || host.ends_with(&format!(".{d}"))
            })
        {
            continue;
        }
        if r.expires.is_some_and(|e| e <= now) {
            out.skipped_expired += 1;
            continue;
        }
        let bad = |s: &str| s.contains(';') || s.chars().any(|c| c.is_control());
        if r.name.is_empty() || bad(&r.name) || bad(&r.value) || bad(&r.path) {
            out.skipped_invalid += 1;
            continue;
        }
        *out.domains.entry(host.to_string()).or_default() += 1;
        out.cookies.push(r.to_set_cookie());
    }
    Ok(out)
}

// ---------------------------------------------------------------- locating

fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not set".to_string())
}

/// The cookie db for a profile argument. A path (absolute, or containing a
/// separator) is taken as given — the db itself or its profile directory.
fn locate(browser: Browser, profile: Option<&str>) -> Result<PathBuf, String> {
    let file = match browser {
        Browser::Chrome => "Cookies",
        Browser::Firefox => "cookies.sqlite",
    };
    if let Some(p) = profile.filter(|p| p.contains('/') || p.contains('\\')) {
        let p = PathBuf::from(p);
        return Ok(if p.is_dir() { p.join(file) } else { p });
    }
    match browser {
        Browser::Chrome => {
            if !cfg!(target_os = "macos") {
                return Err("Chrome import is macOS-only for now (Windows/Linux key stores are phase 2); pass a Firefox profile instead".into());
            }
            let dir = home()?
                .join("Library/Application Support/Google/Chrome")
                .join(profile.unwrap_or("Default"));
            Ok(dir.join(file))
        }
        Browser::Firefox => {
            let root = if cfg!(target_os = "macos") {
                home()?.join("Library/Application Support/Firefox/Profiles")
            } else if cfg!(target_os = "windows") {
                PathBuf::from(std::env::var_os("APPDATA").ok_or("APPDATA is not set")?)
                    .join("Mozilla/Firefox/Profiles")
            } else {
                home()?.join(".mozilla/firefox")
            };
            if let Some(name) = profile {
                return Ok(root.join(name).join(file));
            }
            firefox_default(&root, file)
        }
    }
}

/// Firefox's default profile: the `*.default-release` directory when there
/// is one (what a stock install creates and launches), else the profile
/// whose cookie db was written last.
fn firefox_default(root: &Path, file: &str) -> Result<PathBuf, String> {
    let entries = std::fs::read_dir(root).map_err(|e| denied_or(root, e))?;
    let mut best: Option<(bool, std::time::SystemTime, PathBuf)> = None;
    for e in entries.flatten() {
        let db = e.path().join(file);
        let Ok(meta) = std::fs::metadata(&db) else { continue };
        let release = e.file_name().to_string_lossy().ends_with(".default-release");
        let mtime = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
        if best.as_ref().is_none_or(|(r, t, _)| (release, mtime) > (*r, *t)) {
            best = Some((release, mtime, db));
        }
    }
    best.map(|(_, _, p)| p)
        .ok_or_else(|| format!("no Firefox profile with {file} under {}", root.display()))
}

/// macOS answers EPERM ("Operation not permitted") when TCC withholds another
/// app's data from this process — say what to grant, not what failed.
fn denied_or(path: &Path, e: std::io::Error) -> String {
    if e.kind() == std::io::ErrorKind::PermissionDenied || e.raw_os_error() == Some(1) {
        format!(
            "macOS denied access to {} (Privacy & Security): grant Full Disk Access to the app that runs diting (its terminal / launcher), then retry",
            path.display()
        )
    } else {
        format!("cannot read {}: {e}", path.display())
    }
}

/// Copy `db` (+ `-wal`, `-shm` when present) into a fresh private TempDir and
/// open the copy read-only. The TempDir (credentials inside) is deleted when
/// the returned guard drops.
fn snapshot(db: &Path) -> Result<(tempfile::TempDir, rusqlite::Connection), String> {
    if !db.exists() {
        // exists() is false on EPERM too: tell the two apart by statting.
        return Err(match std::fs::metadata(db) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => denied_or(db, e),
            _ => format!("no cookie db at {}", db.display()),
        });
    }
    let dir = tempfile::Builder::new()
        .prefix("diting-import-")
        .tempdir()
        .map_err(|e| format!("tempdir: {e}"))?;
    let copy = dir.path().join("cookies.db");
    std::fs::copy(db, &copy).map_err(|e| denied_or(db, e))?;
    for side in ["-wal", "-shm"] {
        let mut src = db.as_os_str().to_owned();
        src.push(side);
        let src = PathBuf::from(src);
        if src.exists() {
            let mut dst = copy.as_os_str().to_owned();
            dst.push(side);
            std::fs::copy(&src, PathBuf::from(dst)).map_err(|e| denied_or(&src, e))?;
        }
    }
    let conn = rusqlite::Connection::open_with_flags(
        &copy,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("open {}: {e}", db.display()))?;
    Ok((dir, conn))
}

// ---------------------------------------------------------------- Firefox

fn read_firefox(db: &Path) -> Result<Vec<Row>, String> {
    let (_dir, conn) = snapshot(db)?;
    let mut st = conn
        .prepare(
            "SELECT host, name, value, path, expiry, isSecure, isHttpOnly, sameSite \
             FROM moz_cookies WHERE originAttributes = ''",
        )
        .map_err(|e| format!("not a Firefox cookie db ({e})"))?;
    let rows = st
        .query_map([], |r| {
            let expiry: i64 = r.get(4)?;
            Ok(Row {
                host: r.get(0)?,
                name: r.get(1)?,
                value: r.get(2)?,
                path: r.get(3)?,
                // Newer Firefox stores expiry in ms; seconds never reach 1e11.
                expires: Some(if expiry > 100_000_000_000 { expiry / 1000 } else { expiry }),
                secure: r.get::<_, i64>(5)? != 0,
                http_only: r.get::<_, i64>(6)? != 0,
                same_site: match r.get::<_, i64>(7)? {
                    1 => "Lax",
                    2 => "Strict",
                    _ => "None",
                },
            })
        })
        .map_err(|e| format!("read moz_cookies: {e}"))?;
    rows.collect::<Result<_, _>>().map_err(|e| format!("read moz_cookies: {e}"))
}

// ---------------------------------------------------------------- Chrome

/// The Keychain's "Chrome Safe Storage" password via the stock `security`
/// CLI (macOS asks the user to allow it once). Never logged.
fn keychain_password() -> Result<Vec<u8>, String> {
    let out = std::process::Command::new("security")
        .args(["find-generic-password", "-w", "-s", "Chrome Safe Storage"])
        .output()
        .map_err(|e| format!("cannot run `security`: {e}"))?;
    if !out.status.success() {
        return Err("Keychain refused \"Chrome Safe Storage\" (denied, or Chrome never ran as this user)".into());
    }
    let mut pw = out.stdout;
    while pw.last().is_some_and(|b| *b == b'\n' || *b == b'\r') {
        pw.pop();
    }
    Ok(pw)
}

fn chrome_key(password: &[u8]) -> [u8; 16] {
    let mut key = [0u8; 16];
    pbkdf2::pbkdf2_hmac::<sha1::Sha1>(password, b"saltysalt", 1003, &mut key);
    key
}

/// Open one `encrypted_value`. None = not something this build can decrypt.
fn chrome_decrypt(key: &[u8; 16], host: &str, enc: &[u8]) -> Option<String> {
    use cbc::cipher::block_padding::Pkcs7;
    use cbc::cipher::{BlockDecryptMut, KeyIvInit};
    let body = enc.strip_prefix(b"v10")?;
    let plain = cbc::Decryptor::<aes::Aes128>::new_from_slices(key, &[b' '; 16])
        .ok()?
        .decrypt_padded_vec_mut::<Pkcs7>(body)
        .ok()?;
    let digest = <sha2::Sha256 as sha2::Digest>::digest(host.as_bytes());
    let value = match plain.strip_prefix(digest.as_slice()) {
        Some(rest) => rest.to_vec(),
        None => plain,
    };
    String::from_utf8(value).ok()
}

/// Chrome's expires_utc: microseconds since 1601-01-01 UTC.
fn chrome_time(us: i64) -> i64 {
    us / 1_000_000 - 11_644_473_600
}

fn read_chrome(db: &Path, key: &[u8; 16]) -> Result<(Vec<Row>, usize), String> {
    let (_dir, conn) = snapshot(db)?;
    let mut st = conn
        .prepare(
            "SELECT host_key, name, value, encrypted_value, path, expires_utc, has_expires, \
             is_secure, is_httponly, samesite FROM cookies",
        )
        .map_err(|e| format!("not a Chrome cookie db ({e})"))?;
    let mut rows = Vec::new();
    let mut undecryptable = 0;
    let mut q = st.query([]).map_err(|e| format!("read cookies: {e}"))?;
    while let Some(r) = q.next().map_err(|e| format!("read cookies: {e}"))? {
        let get = |e: rusqlite::Error| format!("read cookies: {e}");
        let host: String = r.get(0).map_err(get)?;
        let plain: String = r.get(2).map_err(get)?;
        let enc: Vec<u8> = r.get(3).map_err(get)?;
        let value = if !plain.is_empty() || enc.is_empty() {
            plain
        } else if let Some(v) = chrome_decrypt(key, &host, &enc) {
            v
        } else {
            undecryptable += 1;
            continue;
        };
        let expires_utc: i64 = r.get(5).map_err(get)?;
        let has_expires: i64 = r.get(6).map_err(get)?;
        rows.push(Row {
            name: r.get(1).map_err(get)?,
            value,
            path: r.get(4).map_err(get)?,
            expires: (has_expires != 0 && expires_utc != 0).then(|| chrome_time(expires_utc)),
            secure: r.get::<_, i64>(7).map_err(get)? != 0,
            http_only: r.get::<_, i64>(8).map_err(get)? != 0,
            same_site: match r.get::<_, i64>(9).map_err(get)? {
                0 => "None",
                1 => "Lax",
                2 => "Strict",
                _ => "",
            },
            host,
        });
    }
    Ok((rows, undecryptable))
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY_PW: &[u8] = b"fixture-safe-storage";

    fn enc(key: &[u8; 16], host: &str, value: &str, hashed: bool) -> Vec<u8> {
        use cbc::cipher::block_padding::Pkcs7;
        use cbc::cipher::{BlockEncryptMut, KeyIvInit};
        let mut plain = Vec::new();
        if hashed {
            plain.extend_from_slice(&<sha2::Sha256 as sha2::Digest>::digest(host.as_bytes()));
        }
        plain.extend_from_slice(value.as_bytes());
        let mut out = b"v10".to_vec();
        out.extend(
            cbc::Encryptor::<aes::Aes128>::new_from_slices(key, &[b' '; 16])
                .unwrap()
                .encrypt_padded_vec_mut::<Pkcs7>(&plain),
        );
        out
    }

    /// (host_key, name, value, encrypted_value, expires_utc, is_secure, samesite)
    type ChromeRow<'a> = (&'a str, &'a str, &'a str, Vec<u8>, i64, i64, i64);

    /// A Chrome-shaped cookie db (the columns read_chrome selects).
    fn chrome_db(dir: &Path, rows: &[ChromeRow]) -> PathBuf {
        let p = dir.join("Cookies");
        let c = rusqlite::Connection::open(&p).unwrap();
        c.execute_batch(
            "CREATE TABLE cookies (host_key TEXT, name TEXT, value TEXT, encrypted_value BLOB, path TEXT, \
             expires_utc INTEGER, has_expires INTEGER, is_secure INTEGER, is_httponly INTEGER, samesite INTEGER);",
        )
        .unwrap();
        for (host, name, value, encv, exp, secure, ss) in rows {
            c.execute(
                "INSERT INTO cookies VALUES (?1, ?2, ?3, ?4, '/', ?5, ?6, ?7, 1, ?8)",
                rusqlite::params![host, name, value, encv, exp, (*exp != 0) as i64, secure, ss],
            )
            .unwrap();
        }
        p
    }

    fn far_future_chrome() -> i64 {
        (4_102_444_800 + 11_644_473_600) * 1_000_000 // 2100-01-01
    }

    #[test]
    fn chrome_key_is_pbkdf2_sha1_saltysalt_1003() {
        // Independent oracle: python3 -c "import hashlib; print(hashlib.pbkdf2_hmac(
        //   'sha1', b'fixture-safe-storage', b'saltysalt', 1003, 16).hex())"
        let hex: String = chrome_key(KEY_PW).iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, "db16859dab5511521615299fa064b832");
    }

    #[test]
    fn chrome_rows_decrypt_both_schema_eras_and_count_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let key = chrome_key(KEY_PW);
        let ff = far_future_chrome();
        let db = chrome_db(
            dir.path(),
            &[
                // schema >= 24: SHA-256(host_key) leads the plaintext
                (".taobao.com", "cookie2", "", enc(&key, ".taobao.com", "c2-value", true), ff, 1, 0),
                // pre-24: bare plaintext
                ("www.example.com", "sid", "", enc(&key, "www.example.com", "s-value", false), 0, 0, 1),
                // plaintext column (very old rows / unencrypted builds)
                ("plain.test", "p", "pv", Vec::new(), ff, 0, -1),
                // wrong key and a non-v10 tag: counted, never guessed
                ("x.test", "bad", "", enc(&chrome_key(b"other"), "x.test", "zz", true), ff, 0, 2),
                ("y.test", "v11", "", b"v11garbage-bytes".to_vec(), ff, 0, 2),
            ],
        );
        let (rows, undecryptable) = read_chrome(&db, &key).unwrap();
        assert_eq!(undecryptable, 2);
        let got: Vec<(&str, &str, Option<i64>, &str)> = rows
            .iter()
            .map(|r| (r.name.as_str(), r.value.as_str(), r.expires, r.same_site))
            .collect();
        assert_eq!(
            got,
            vec![
                ("cookie2", "c2-value", Some(4_102_444_800), "None"),
                ("sid", "s-value", None, "Lax"),
                ("p", "pv", Some(4_102_444_800), ""),
            ]
        );
        assert_eq!(
            rows[0].to_set_cookie(),
            "cookie2=c2-value; Domain=taobao.com; Path=/; Expires=Fri, 01 Jan 2100 00:00:00 GMT; Secure; HttpOnly; SameSite=None"
        );
        assert_eq!(rows[1].to_set_cookie(), "sid=s-value; Domain=www.example.com; Path=/; HttpOnly; SameSite=Lax");
    }

    #[test]
    fn firefox_rows_read_plaintext_skip_containers_and_fold_ms_expiry() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("cookies.sqlite");
        let c = rusqlite::Connection::open(&p).unwrap();
        c.execute_batch(
            "CREATE TABLE moz_cookies (id INTEGER PRIMARY KEY, originAttributes TEXT NOT NULL DEFAULT '', \
             name TEXT, value TEXT, host TEXT, path TEXT, expiry INTEGER, isSecure INTEGER, isHttpOnly INTEGER, sameSite INTEGER);
             INSERT INTO moz_cookies (originAttributes,name,value,host,path,expiry,isSecure,isHttpOnly,sameSite) VALUES
               ('', 'a', '1', '.example.com', '/', 4102444800, 1, 0, 1),
               ('', 'b', '2', 'sub.example.com', '/p', 4102444800000, 0, 1, 2),
               ('^userContextId=2', 'c', '3', '.example.com', '/', 4102444800, 0, 0, 0);",
        )
        .unwrap();
        drop(c);
        let rows = read_firefox(&p).unwrap();
        assert_eq!(rows.len(), 2, "container cookies stay out of the default jar");
        assert_eq!(rows[0].to_set_cookie(), "a=1; Domain=example.com; Path=/; Expires=Fri, 01 Jan 2100 00:00:00 GMT; Secure; SameSite=Lax");
        assert_eq!(rows[1].expires, Some(4_102_444_800), "ms expiry folds to seconds");
        assert_eq!(rows[1].same_site, "Strict");
    }

    #[test]
    fn snapshot_reads_through_the_wal_of_a_live_db() {
        // A browser that is running keeps fresh rows in the -wal; copying only
        // the main file would silently drop them.
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("cookies.sqlite");
        let live = rusqlite::Connection::open(&p).unwrap();
        live.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;
             CREATE TABLE moz_cookies (originAttributes TEXT DEFAULT '', name TEXT, value TEXT, host TEXT, path TEXT,
               expiry INTEGER, isSecure INTEGER, isHttpOnly INTEGER, sameSite INTEGER);
             INSERT INTO moz_cookies VALUES ('', 'fresh', 'v', 'wal.test', '/', 4102444800, 0, 0, 0);",
        )
        .unwrap();
        let mut wal = p.as_os_str().to_owned();
        wal.push("-wal");
        assert!(PathBuf::from(wal).exists(), "fixture keeps the row in the WAL");
        let rows = read_firefox(&p).unwrap();
        assert_eq!(rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["fresh"]);
        drop(live);
    }

    #[test]
    fn import_filters_domains_expiry_and_unsendable_values() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("cookies.sqlite");
        let c = rusqlite::Connection::open(&p).unwrap();
        c.execute_batch(
            "CREATE TABLE moz_cookies (originAttributes TEXT DEFAULT '', name TEXT, value TEXT, host TEXT, path TEXT,
               expiry INTEGER, isSecure INTEGER, isHttpOnly INTEGER, sameSite INTEGER);
             INSERT INTO moz_cookies VALUES
               ('', 'keep', 'v', '.taobao.com', '/', 4102444800, 0, 0, 0),
               ('', 'sub', 'v', 'login.taobao.com', '/', 4102444800, 0, 0, 0),
               ('', 'other', 'v', '.tmall.com', '/', 4102444800, 0, 0, 0),
               ('', 'old', 'v', '.taobao.com', '/', 1000, 0, 0, 0),
               ('', 'semi', 'a;b', '.taobao.com', '/', 4102444800, 0, 0, 0),
               ('', 'look', 'v', 'nottaobao.com', '/', 4102444800, 0, 0, 0);",
        )
        .unwrap();
        drop(c);
        let out = import(Browser::Firefox, Some(p.to_str().unwrap()), &["taobao.com".to_string()]).unwrap();
        let names: Vec<&str> = out.cookies.iter().map(|s| s.split('=').next().unwrap()).collect();
        assert_eq!(names, ["keep", "sub"], "suffix match is label-bounded: nottaobao.com stays out");
        assert_eq!((out.skipped_expired, out.skipped_invalid), (1, 1));
        assert_eq!(out.domains.get("taobao.com"), Some(&1));
        assert_eq!(out.domains.get("login.taobao.com"), Some(&1));
    }

    #[test]
    fn missing_db_is_named_not_mistaken_for_a_permission_wall() {
        let dir = tempfile::tempdir().unwrap();
        let err = read_firefox(&dir.path().join("nope/cookies.sqlite")).unwrap_err();
        assert!(err.starts_with("no cookie db at "), "{err}");
    }
}
