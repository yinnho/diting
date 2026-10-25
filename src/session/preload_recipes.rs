//! Named document-start recipes — the MECHANISM, bodies private.
//!
//! Asset separation (2026-10-25): the engine is open, platform signing
//! chains are not. The registry ships EMPTY — `resolve` answers None for
//! every name (the unknown-name error contract is unchanged), no
//! include_str! bodies, no per-host shields. Operators deploy recipe
//! sources privately via the `scripts[]` document-start group or their
//! own flow assets; this module keeps the resolution surface and the
//! always-off auto-mount stubs so navigate/create wiring is untouched.
//! History: the recipes this module once embedded (xhs-sign, x-p256,
//! x-bearer, x-auth-ctx, xhs-qrcode, taobao-block-*) moved out of the
//! public tree together with workflow/ — engine open, recipes private.

/// Resolve a recipe name (aliases included) to its source. The public
/// registry is EMPTY (asset separation 2026-10-25): every name resolves
/// to None and callers surface the error rather than silently mounting
/// nothing — the same contract as an unknown name before.
pub fn resolve(_name: &str) -> Option<&'static str> {
    None
}

/// Builtin recipe scripts that apply to this navigation URL.
///
/// Always empty: site recipes are not auto-mounted. Kept so navigate /
/// create keep calling one function (`effective_scripts`) without a
/// special case; a future opt-in host table would land here behind an
/// explicit product decision, not by default.
fn builtin_scripts(_url: &str) -> Vec<&'static str> {
    Vec::new()
}

/// Report-only view of [`builtin_scripts`] — the names surfaces quote
/// (navigate responses) without the source body. Always empty while
/// auto-mount is off.
pub fn builtin_names(url: &str) -> Vec<&'static str> {
    let _ = url;
    Vec::new()
}

/// The document-start group for a navigation: any future builtin
/// recipes would ride first; today this is just the user group.
pub fn effective_scripts(url: &str, user: &[String]) -> Vec<String> {
    builtin_scripts(url)
        .into_iter()
        .map(str::to_string)
        .chain(user.iter().cloned())
        .collect()
}

/// Recipes the account probes must mount before navigating `url`.
///
/// Empty since the asset separation (2026-10-25): the shield bodies
/// (taobao everyhelp block, xhs logout fuse) were platform recipes and
/// moved out of the public tree with workflow/. Private deployments
/// re-add them via `scripts[]`; the account probe path keeps calling
/// this so a future private table lands without rewiring.
pub fn probe_shield_recipes(_url: &str) -> Vec<&'static str> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_host_auto_mount() {
        for url in [
            "https://www.xiaohongshu.com/explore?channel_id=homefeed_recommend",
            "https://xiaohongshu.com/",
            "http://edith.xiaohongshu.com/api/sns/web/v1/feed",
            "https://xhslink.com/aBc123",
            "https://example.com/",
            "about:blank",
        ] {
            assert!(
                builtin_names(url).is_empty(),
                "auto-mount must stay off: {url}"
            );
            assert_eq!(
                effective_scripts(url, &[]),
                Vec::<String>::new(),
                "empty user group stays empty: {url}"
            );
        }
    }

    #[test]
    fn effective_passes_user_group_unchanged() {
        let user = vec!["window.__mine = 1;".to_string()];
        assert_eq!(
            effective_scripts("https://www.xiaohongshu.com/explore", &user),
            user
        );
        assert_eq!(effective_scripts("https://example.com/", &user), user);
    }

    #[test]
    fn resolve_is_empty_after_asset_separation() {
        for name in ["xhs-sign", "xhs", "taobao-block-everyhelp", "x-p256", "anything"] {
            assert!(resolve(name).is_none(), "public registry stays empty: {name}");
        }
        assert!(probe_shield_recipes("https://www.xiaohongshu.com/").is_empty());
        assert!(probe_shield_recipes("https://www.taobao.com/").is_empty());
    }

#[test]
    fn probe_shields_empty_everywhere_after_separation() {
        for url in [
            "https://www.taobao.com/",
            "https://www.xiaohongshu.com/explore",
            "https://xhslink.com/aBc",
            "https://mms.pinduoduo.com/home/",
            "https://example.com/",
            "about:blank",
        ] {
            assert!(
                probe_shield_recipes(url).is_empty(),
                "public tree ships no platform shields: {url}"
            );
            assert!(builtin_names(url).is_empty());
        }
    }
}

