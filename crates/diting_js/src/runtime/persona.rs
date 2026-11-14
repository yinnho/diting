//! Navigator persona written into a live realm: user agent, fingerprint
//! seed, language, and platform. Each write executes script, so this is
//! not the wiring module.

use super::JsRuntime;

impl JsRuntime {
    pub fn set_user_agent(&mut self, ua: &str) {
        let escaped = ua.replace('\\', "\\\\").replace('\'', "\\'");
        // After the UA lands, refresh the platform persona (GPU pool, screen,
        // dpr, hw/memory). The runtime constructor ran __diting_init before
        // any UA was known, so the persona materialized from the linux
        // default — leaving Mesa GL strings behind a macOS UA.
        let _ = self.runtime.execute_script(
            "<set-ua>",
            &format!(
                "globalThis.__diting_ua = '{}'; \
                 globalThis._fpCache = null; globalThis.__diting_hw_plat = undefined; \
                 globalThis.__diting_setPersona();",
                escaped
            ),
        );
    }

    /// Re-pin the hardware-persona seed on a freshly booted realm. The realm
    /// is rebuilt per navigation and `__diting_init` self-deletes after
    /// drawing, so cross-page continuity of screen/dpr/GPU/canvas comes from
    /// the Page handing the same seed back every time (see Page::fp_seed).
    pub fn set_fingerprint_seed(&mut self, seed: u64) {
        let s32 = (seed & 0xFFFF_FFFF) as u32;
        let _ = self.runtime.execute_script(
            "<set-fp-seed>",
            &format!("globalThis.__diting_setFpSeed({});", s32),
        );
    }

    /// Move only the navigator.language(s) persona (the `__diting_lang`
    /// global behind the bootstrap live getters). Deliberately does NOT
    /// re-pin the ICU default: `set_default_locale` is process-global and
    /// the per-isolate Intl cache is sticky anyway (obscura #734), so a
    /// mid-session re-pin could not take effect and would contaminate
    /// sibling isolates (obscura #778 hazard). Intl locale-argument
    /// binding follows via the bootstrap wrapper reading this same global.
    pub fn set_navigator_language(&mut self, lang: &str) {
        let escaped = lang.replace('\\', "\\\\").replace('\'', "\\'");
        let _ = self.runtime.execute_script(
            "<set-lang>",
            &format!("globalThis.__diting_lang = '{}';", escaped),
        );
    }

    /// Move only the navigator.platform persona (the CDP `platform` field of
    /// setUserAgentOverride — per protocol it targets navigator.platform
    /// only, never the UA string, the transport headers, or
    /// userAgentData.platform, which is userAgentMetadata territory).
    pub fn set_navigator_platform(&mut self, platform: &str) {
        let escaped = platform.replace('\\', "\\\\").replace('\'', "\\'");
        let _ = self.runtime.execute_script(
            "<set-platform>",
            &format!("globalThis.__diting_platform_override = '{}';", escaped),
        );
    }

    pub fn set_language(&mut self, lang: &str) {
        self.set_navigator_language(lang);
        // Pin ICU's default locale to the SAME source (obscura#734 lineage):
        // V8's Intl follows the process locale otherwise, so a non-matching
        // LANG leaves Intl.DateTimeFormat().resolvedOptions().locale (and
        // every Intl default) disagreeing with navigator.language and the
        // Accept-Language header the net layer sends - a three-way locale
        // mismatch that's a hard headless tell. Take the first q-weights-
        // stripped BCP-47 tag (the same fold __ditingLangList does for
        // navigator.language).
        let first_tag = lang
            .split(',')
            .next()
            .and_then(|t| t.split(';').next())
            .map(|t| t.trim())
            .filter(|t| !t.is_empty())
            .unwrap_or("zh-CN");
        v8::icu::set_default_locale(first_tag);
    }
}
