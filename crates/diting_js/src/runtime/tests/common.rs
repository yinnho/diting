//! 测试共享底座：runtime 构造器与跨域锁。桶文件经 `use super::common::…` 引用。
use super::super::*;
use diting_dom::parse_html;

pub(super) fn setup_runtime(html: &str) -> JsRuntime {
    let dom = parse_html(html);
    let rt = JsRuntime::new();
    rt.set_dom(dom);
    rt.set_url("http://example.com/test");
    rt.set_title("Test Page");
    rt
}

pub(super) fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

pub(super) fn setup_runtime_with_cookies(html: &str) -> (JsRuntime, std::sync::Arc<diting_net::CookieJar>) {
    let dom = diting_dom::parse_html(html);
    let jar = std::sync::Arc::new(diting_net::CookieJar::new());
    let rt = JsRuntime::new();
    rt.set_dom(dom);
    rt.set_url("http://example.com/test");
    rt.set_title("Test Page");
    rt.set_cookie_jar(jar.clone());
    (rt, jar)
}

// localStorage persistence (obscura#629 class): writes flush (debounced)
// to one JSON file per origin, and a fresh realm — the process-restart
// equivalent — reads them back. sessionStorage stays memory-only: its
// per-tab lifetime is the spec'd behavior, and nothing it holds may
// reach the disk.
pub(super) static STORAGE_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
