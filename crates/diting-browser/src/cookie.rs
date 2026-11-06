use diting::diting_net::CookieJar;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// A cookie as exposed to the Rust API. Bin-dead by itself; the round-trip
/// tests read cookies back through [`CookieStore::get_for_url`].
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
}

/// Cookie management for a browser session.
pub struct CookieStore {
    jar: Arc<CookieJar>,
}

impl CookieStore {
    pub(crate) fn new(jar: Arc<CookieJar>) -> Self {
        Self { jar }
    }

    /// Set a cookie via Set-Cookie header string.
    ///
    /// Example: `store.set("session=abc123; Domain=example.com; Path=/; HttpOnly")?;`
    ///
    /// Seeded, not observed: the mutation ring tags these writes "import"
    /// (session-create injection / account seeding carried no response).
    pub fn set(&self, set_cookie_str: &str, url: &str) -> Result<(), crate::error::Error> {
        let parsed = url::Url::parse(url).map_err(|e| crate::error::Error::Internal(e.into()))?;
        self.jar.set_cookie_seeded(set_cookie_str, &parsed);
        Ok(())
    }

    /// Get cookies for a specific URL.
    #[allow(dead_code)] // bin-dead; server round-trip test reads cookies back
    pub fn get_for_url(&self, url: &str) -> Result<Vec<Cookie>, crate::error::Error> {
        let parsed = url::Url::parse(url).map_err(|e| crate::error::Error::Internal(e.into()))?;
        let header = self.jar.get_cookie_header(&parsed);
        Ok(header
            .split("; ")
            .filter(|s| !s.is_empty())
            .filter_map(|pair| {
                let mut parts = pair.splitn(2, '=');
                Some(Cookie {
                    name: parts.next()?.to_string(),
                    value: parts.next().unwrap_or("").to_string(),
                    domain: parsed.host_str()?.to_string(),
                    path: "/".into(),
                    secure: false,
                    http_only: false,
                })
            })
            .collect())
    }
}

/// Inverse of diting's `parse_http_date` for the cookie export form
/// (session `Cookies` command, account records, browser-profile import): epoch seconds
/// → "Wdy, DD Mon YYYY HH:MM:SS GMT". Dates outside the parser's accepted
/// year range surface as None (the entry then round-trips as a session
/// cookie — the old behavior — rather than a broken date).
pub(crate) fn http_date(secs: i64) -> Option<String> {
    if !(0..=253_402_300_799).contains(&secs) {
        return None;
    }
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // civil_from_days (Hinnant): epoch-day → (year, month, day)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mth = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if mth <= 2 { y + 1 } else { y };
    // Day 0 of the epoch was a Thursday.
    let wd = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"][days.rem_euclid(7) as usize];
    let mon = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ][(mth - 1) as usize];
    Some(format!(
        "{wd}, {d:02} {mon} {year:04} {h:02}:{m:02}:{s:02} GMT"
    ))
}
