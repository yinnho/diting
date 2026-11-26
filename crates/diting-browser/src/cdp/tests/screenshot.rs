//! `Page.captureScreenshot`: a clip is cut out of the capture, and what
//! lies past it comes back white, as in Chrome.

use super::*;

/// A `Page.captureScreenshot` with `clip` (and whatever else is given),
/// decoded.
#[cfg(feature = "screenshot")]
async fn shot(
    client: &mut Client,
    session: &str,
    params: Value,
) -> diting::diting_layout::DecodedImage {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let mut params = params;
    params["format"] = json!("png");
    let reply = match client
        .ask("Page.captureScreenshot", params.clone(), Some(session))
        .await
    {
        Ok(reply) => reply,
        Err(err) => panic!("captureScreenshot {params} -> {err:?}"),
    };
    let png = STANDARD
        .decode(reply["data"].as_str().unwrap_or(""))
        .expect("png base64");
    diting::diting_layout::image::decode_png(&png).expect("a png")
}

#[cfg(feature = "screenshot")]
fn rgba(image: &diting::diting_layout::DecodedImage, x: u32, y: u32) -> [u8; 4] {
    let at = ((y * image.width + x) * 4) as usize;
    image.rgba[at..at + 4].try_into().unwrap()
}

#[cfg(feature = "screenshot")]
fn is(pixel: [u8; 4], color: [u8; 3]) -> bool {
    pixel[..3] == color && pixel[3] == 255
}

#[cfg(feature = "screenshot")]
const RED: [u8; 3] = [255, 0, 0];
#[cfg(feature = "screenshot")]
const BLUE: [u8; 3] = [0, 0, 255];
#[cfg(feature = "screenshot")]
const WHITE: [u8; 3] = [255, 255, 255];

/// A `clip` of CSS pixels at `scale`.
#[cfg(feature = "screenshot")]
fn rect(x: f64, y: f64, width: f64, height: f64, scale: f64) -> Value {
    json!({ "x": x, "y": y, "width": width, "height": height, "scale": scale })
}

#[cfg(feature = "screenshot")]
#[tokio::test]
async fn a_clip_is_cut_out_of_the_capture_and_padded_white() {
    let _guard = CDP_LOCK.lock().await;
    let (mut client, _) = connect(axum::Router::new()).await;
    let (_, session) = open_owned_page(&mut client, 1).await;
    let measured = client
        .eval("[innerWidth, innerHeight, devicePixelRatio]", &session)
        .await;
    let [width, height, dpr] = measured
        .as_array()
        .map(|size| {
            [
                size[0].as_f64().unwrap(),
                size[1].as_f64().unwrap(),
                size[2].as_f64().unwrap(),
            ]
        })
        .unwrap();
    assert!(
        height > 400.0,
        "the fixture needs a viewport taller than 400: {height}"
    );
    // A red 100×50 block on a blue page 1500px taller than the viewport.
    let document = height + 1500.0;
    let page = format!(
        "data:text/html,<body style=\"margin:0\"><div style=\"width:100px;height:50px;background:#ff0000\"></div><div style=\"height:{}px;background:#0000ff\"></div></body>",
        document - 50.0
    );
    client
        .run(
            "Page.navigate",
            json!({ "url": page }),
            Some(session.as_str()),
        )
        .await;
    let on = Some(session.as_str());
    let px = |css: f64| (css * dpr).round() as u32;

    // A rect in the viewport, and the viewport itself — Playwright's two.
    let image = shot(
        &mut client,
        &session,
        json!({ "clip": rect(0.0, 0.0, 100.0, 50.0, 1.0) }),
    )
    .await;
    assert_eq!((image.width, image.height), (px(100.0), px(50.0)));
    assert!(is(rgba(&image, px(10.0), px(10.0)), RED));
    let image = shot(
        &mut client,
        &session,
        json!({ "clip": rect(0.0, 0.0, width, height, 1.0) }),
    )
    .await;
    assert_eq!((image.width, image.height), (px(width), px(height)));

    // The whole document asked for by name, and without a clip.
    let image = shot(
        &mut client,
        &session,
        json!({ "clip": rect(0.0, 0.0, width, document, 1.0), "captureBeyondViewport": true }),
    )
    .await;
    assert_eq!(image.width, px(width));
    assert!(image.height >= px(document - 100.0), "{}", image.height);
    assert!(is(rgba(&image, px(50.0), image.height - 1), BLUE));
    let image = shot(
        &mut client,
        &session,
        json!({ "captureBeyondViewport": true }),
    )
    .await;
    assert!(image.height >= px(document - 100.0), "{}", image.height);
    // The same clip without the document: the viewport's rows, white below.
    let image = shot(
        &mut client,
        &session,
        json!({ "clip": rect(0.0, 0.0, width, document, 1.0) }),
    )
    .await;
    assert_eq!((image.width, image.height), (px(width), px(document)));
    assert!(is(rgba(&image, px(50.0), px(height - 5.0)), BLUE));
    assert!(is(rgba(&image, px(50.0), px(height + 5.0)), WHITE));

    // Scrolled down: the rect is in document coordinates.
    client.eval("window.scrollTo(0, 500)", &session).await;
    let metrics = client.run("Page.getLayoutMetrics", json!({}), on).await;
    assert_eq!(metrics["layoutViewport"]["pageY"], 500);
    let image = shot(
        &mut client,
        &session,
        json!({ "clip": rect(20.0, 800.0, 60.0, 40.0, 1.0) }),
    )
    .await;
    assert_eq!((image.width, image.height), (px(60.0), px(40.0)));
    assert!(is(rgba(&image, px(5.0), px(5.0)), BLUE));
    // Off the viewport: white, unless the document is asked for. The rect
    // sits under the scrolled-to viewport's bottom edge, inside the blue.
    let below = rect(0.0, 500.0 + height + 100.0, 100.0, 100.0, 1.0);
    let image = shot(&mut client, &session, json!({ "clip": below })).await;
    assert_eq!((image.width, image.height), (px(100.0), px(100.0)));
    assert!(is(rgba(&image, px(50.0), px(50.0)), WHITE));
    let image = shot(
        &mut client,
        &session,
        json!({ "clip": below, "captureBeyondViewport": true }),
    )
    .await;
    assert_eq!((image.width, image.height), (px(100.0), px(100.0)));
    assert!(is(rgba(&image, px(50.0), px(50.0)), BLUE));

    // Past the document's end: blue as far as it goes, white after.
    let image = shot(
        &mut client,
        &session,
        json!({ "clip": rect(0.0, document - 50.0, 100.0, 100.0, 1.0), "captureBeyondViewport": true }),
    )
    .await;
    assert_eq!((image.width, image.height), (px(100.0), px(100.0)));
    assert!(is(rgba(&image, px(50.0), px(25.0)), BLUE));
    assert!(is(rgba(&image, px(50.0), px(75.0)), WHITE));

    // Off the top-left corner: moved onto it, size kept.
    client.eval("window.scrollTo(0, 0)", &session).await;
    let image = shot(
        &mut client,
        &session,
        json!({ "clip": rect(-20.0, -20.0, 100.0, 100.0, 1.0) }),
    )
    .await;
    assert_eq!((image.width, image.height), (px(100.0), px(100.0)));
    assert!(is(rgba(&image, px(30.0), px(20.0)), RED));
    assert!(is(rgba(&image, px(30.0), px(60.0)), BLUE));

    if dpr == 2.0 {
        // Playwright's `scale: "css"`: one pixel per CSS pixel.
        let image = shot(
            &mut client,
            &session,
            json!({ "clip": rect(0.0, 0.0, 100.0, 50.0, 0.5) }),
        )
        .await;
        assert_eq!((image.width, image.height), (100, 50));
    }

    for (params, reason) in [
        (json!({ "format": "jpeg" }), "only png is supported"),
        (
            json!({ "clip": { "x": 0, "y": 0, "width": 0, "height": 10, "scale": 1 } }),
            "0 width",
        ),
        (
            json!({ "clip": { "x": 0, "y": 0, "width": 10, "height": 10, "scale": 0 } }),
            "clip needs a positive scale",
        ),
        (
            json!({ "clip": { "x": 0, "y": 0, "width": 100000, "height": 100000, "scale": 1 } }),
            "Unable to capture screenshot",
        ),
    ] {
        assert!(client
            .refusal("Page.captureScreenshot", params, on)
            .await
            .contains(reason));
    }
}
