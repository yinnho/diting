//! `Network.setBlockedURLs` and `setExtraHTTPHeaders`: a blocked URL is
//! never requested, and the headers reach the wire.

use super::*;

/// `/page/{n}` pulls a stylesheet, a script and an image of its number;
/// `/echo` answers with the `x-probe` header it was sent. Every path
/// requested is appended to the log.
///
/// The routes take the whole segment after the slash as their name —
/// `"1.css"`, not `1`. A path parameter is a whole segment; one followed
/// by a suffix (`/sheet/:n.css`) matches `/sheet/1.css` only with `n` as
/// `"1.css"`, and a `u32` extractor answers 400 before the handler runs.
#[derive(Clone, Default)]
struct Site(std::sync::Arc<std::sync::Mutex<Vec<String>>>);

impl Site {
    fn hits(&self, path: &str) -> usize {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter(|hit| hit.as_str() == path)
            .count()
    }

    fn log(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

async fn site_page(
    axum::extract::State(site): axum::extract::State<Site>,
    axum::extract::Path(n): axum::extract::Path<u32>,
) -> axum::response::Html<String> {
    site.0.lock().unwrap().push("/page".into());
    axum::response::Html(format!(
        "<html><head><link rel=\"stylesheet\" href=\"/sheet/{n}.css\"></head>\
         <body style=\"margin:0\"><img src=\"/pixel/{n}.png\" width=20 height=20>\
         <div id=s>no</div><script src=\"/script/{n}.js\"></script></body></html>"
    ))
}

async fn site_pixel(
    axum::extract::State(site): axum::extract::State<Site>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> impl axum::response::IntoResponse {
    site.0.lock().unwrap().push(format!("/pixel/{name}"));
    (
        [(axum::http::header::CONTENT_TYPE, "image/png")],
        PIXEL.to_vec(),
    )
}

async fn site_sheet(
    axum::extract::State(site): axum::extract::State<Site>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> impl axum::response::IntoResponse {
    site.0.lock().unwrap().push(format!("/sheet/{name}"));
    (
        [(axum::http::header::CONTENT_TYPE, "text/css")],
        "#s{color:rgb(7,8,9)}",
    )
}

async fn site_script(
    axum::extract::State(site): axum::extract::State<Site>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> impl axum::response::IntoResponse {
    site.0.lock().unwrap().push(format!("/script/{name}"));
    (
        [(axum::http::header::CONTENT_TYPE, "text/javascript")],
        "document.getElementById('s').textContent += 'js'",
    )
}

async fn site_echo(
    axum::extract::State(site): axum::extract::State<Site>,
    headers: axum::http::HeaderMap,
) -> axum::response::Html<String> {
    site.0.lock().unwrap().push("/echo".into());
    let probe = headers
        .get("x-probe")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("none");
    axum::response::Html(format!("<html><body id=b>{probe}</body></html>"))
}

/// A 1×1 png.
static PIXEL: std::sync::LazyLock<Vec<u8>> = std::sync::LazyLock::new(|| {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    STANDARD
        .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==")
        .expect("1x1 png")
});

fn site() -> (axum::Router, Site) {
    let log = Site::default();
    let router = axum::Router::new()
        .route("/page/:n", axum::routing::get(site_page))
        .route("/pixel/:n.png", axum::routing::get(site_pixel))
        .route("/sheet/:n.css", axum::routing::get(site_sheet))
        .route("/script/:n.js", axum::routing::get(site_script))
        .route("/echo", axum::routing::get(site_echo))
        .with_state(log.clone());
    (router, log)
}

async fn await_hits(site: &Site, path: &str, count: usize) -> bool {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while site.hits(path) < count {
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    true
}

#[tokio::test]
async fn blocked_urls_never_reach_the_wire_and_extra_headers_do() {
    let _guard = CDP_LOCK.lock().await;
    fresh_hub().await;
    let _net = crate::server::test_util::net_env_guard();
    let (router, site) = site();
    let (mut client, addr) = connect(router).await;
    let origin = format!("http://{addr}");
    let session = page_at(&mut client, 1, &format!("{origin}/page/1")).await;
    let on = Some(session.as_str());
    // The script's text and the stylesheet's colour: what each left on
    // the page.
    let shown = "document.getElementById('s').textContent";
    // The colour needs the style engine, which the `screenshot` feature
    // brings; the wire does not.
    #[cfg(feature = "screenshot")]
    let color = "getComputedStyle(document.getElementById('s')).color";
    for path in ["/sheet/1.css", "/script/1.js"] {
        assert!(
            await_hits(&site, path, 1).await,
            "{path} was never requested: {:?}",
            site.log()
        );
    }
    assert_eq!(client.eval(shown, &session).await, "nojs");
    #[cfg(feature = "screenshot")]
    assert_eq!(client.eval(color, &session).await, "rgb(7, 8, 9)");

    // An image, a stylesheet and a script, blocked: none is requested, and
    // the page shows the two it was to change.
    client
        .run(
            "Network.setBlockedURLs",
            json!({ "urls": ["*/pixel/2.png", "*/script/2.js", "*/sheet/2.css"] }),
            on,
        )
        .await;
    client
        .run(
            "Page.navigate",
            json!({ "url": format!("{origin}/page/2") }),
            on,
        )
        .await;
    let text = client.eval(shown, &session).await;
    tokio::time::sleep(Duration::from_millis(1000)).await;
    assert_eq!(
        (
            site.hits("/pixel/2.png"),
            site.hits("/sheet/2.css"),
            site.hits("/script/2.js"),
        ),
        (0, 0, 0),
        "a blocked resource was requested: {:?}",
        site.log()
    );
    assert_eq!(text, "no");
    #[cfg(feature = "screenshot")]
    assert_eq!(client.eval(color, &session).await, "rgb(0, 0, 0)");

    // Unblocked, the next page is its whole self.
    client
        .run("Network.setBlockedURLs", json!({ "urls": [] }), on)
        .await;
    client
        .run(
            "Page.navigate",
            json!({ "url": format!("{origin}/page/3") }),
            on,
        )
        .await;
    assert!(
        await_hits(&site, "/script/3.js", 1).await,
        "an unblocked script was not requested: {:?}",
        site.log()
    );
    assert_eq!(client.eval(shown, &session).await, "nojs");
    #[cfg(feature = "screenshot")]
    assert_eq!(client.eval(color, &session).await, "rgb(7, 8, 9)");

    client
        .run(
            "Network.setExtraHTTPHeaders",
            json!({ "headers": { "X-Probe": "sent" } }),
            on,
        )
        .await;
    client
        .run(
            "Page.navigate",
            json!({ "url": format!("{origin}/echo") }),
            on,
        )
        .await;
    assert_eq!(
        client
            .eval("document.getElementById('b').textContent", &session)
            .await,
        "sent"
    );
    client
        .run("Network.setExtraHTTPHeaders", json!({ "headers": {} }), on)
        .await;
    client
        .run(
            "Page.navigate",
            json!({ "url": format!("{origin}/echo?again") }),
            on,
        )
        .await;
    assert_eq!(
        client
            .eval("document.getElementById('b').textContent", &session)
            .await,
        "none"
    );
}
