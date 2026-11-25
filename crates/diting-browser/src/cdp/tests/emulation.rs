//! The `Emulation` domain: what each pin moves, that a pin is the
//! session's — it outlives the page it was set on — and that a field sent
//! empty is the page's own again. The pins are read through `matchMedia`,
//! `Intl` and `navigator`, which every build has. What the cascade and a
//! body's width make of a pin needs the style and layout engine, which the
//! `screenshot` feature brings.

use super::*;

const PAGE: &str = "data:text/html,<style>body{margin:0}@media (prefers-color-scheme: dark){#t{color:rgb(1, 2, 3)}}</style><div id=t style=\"width:100%\">x</div>";
const NEXT: &str =
    "data:text/html,<style>body{margin:0}</style><div id=t style=\"width:100%\">next</div>";

/// `(screen, print, dark, light)`: what the media queries answer.
async fn media(client: &mut Client, session: &str) -> Value {
    client
        .eval(
            "[matchMedia('screen').matches, matchMedia('print').matches, \
             matchMedia('(prefers-color-scheme: dark)').matches, \
             matchMedia('(prefers-color-scheme: light)').matches]",
            session,
        )
        .await
}

/// `(zone, Intl's locale, a number, the UA, the language, the platform)`.
async fn identity(client: &mut Client, session: &str) -> Value {
    client
        .eval(
            "[Intl.DateTimeFormat().resolvedOptions().timeZone, \
             Intl.DateTimeFormat().resolvedOptions().locale, \
             (1234.5).toLocaleString(), navigator.userAgent, navigator.language, \
             navigator.platform]",
            session,
        )
        .await
}

/// `(width, height, pixel ratio, the panel, what is free of it, which way
/// it is turned)`.
async fn size(client: &mut Client, session: &str) -> Value {
    client
        .eval(
            "[innerWidth, innerHeight, devicePixelRatio, screen.width, screen.height, \
             screen.availHeight, screen.orientation.type, screen.orientation.angle]",
            session,
        )
        .await
}

/// `(touch points, a touch event, coarse, fine, no hover, a coarse one
/// somewhere)`.
async fn touch(client: &mut Client, session: &str) -> Value {
    client
        .eval(
            "[navigator.maxTouchPoints, 'ontouchstart' in window, \
             matchMedia('(pointer: coarse)').matches, matchMedia('(pointer: fine)').matches, \
             matchMedia('(hover: none)').matches, matchMedia('(any-pointer: coarse)').matches]",
            session,
        )
        .await
}

/// Every call replaces the whole set, as in Chrome: a `media` left out is
/// back to `screen`, `features` left out is none.
#[tokio::test]
async fn emulated_media_replaces_the_whole_set_on_every_call() {
    let (_guard, mut client, session) = bridge("about:blank").await;
    // What the host answers for the colour scheme is the host's business.
    let own = media(&mut client, &session).await;
    assert_eq!(
        own.as_array().expect("a list")[0..2].to_vec(),
        vec![json!(true), json!(false)],
        "{own}"
    );

    client
        .run(
            "Emulation.setEmulatedMedia",
            json!({ "media": "print" }),
            Some(&session),
        )
        .await;
    assert_eq!(
        media(&mut client, &session).await,
        json!([false, true, own[2], own[3]])
    );

    client
        .run(
            "Emulation.setEmulatedMedia",
            json!({ "features": [{ "name": "prefers-color-scheme", "value": "dark" }] }),
            Some(&session),
        )
        .await;
    assert_eq!(
        media(&mut client, &session).await,
        json!([true, false, true, false])
    );

    // A feature with an empty value emulates nothing; the last one sent of
    // a feature is the one that counts.
    client
        .run(
            "Emulation.setEmulatedMedia",
            json!({ "media": "", "features": [] }),
            Some(&session),
        )
        .await;
    assert_eq!(media(&mut client, &session).await, own);

    // Of a feature sent twice the last one counts, as in Chrome.
    client
        .run(
            "Emulation.setEmulatedMedia",
            json!({ "features": [
                { "name": "prefers-color-scheme", "value": "light" },
                { "name": "prefers-color-scheme", "value": "dark" },
            ] }),
            Some(&session),
        )
        .await;
    assert_eq!(
        media(&mut client, &session).await,
        json!([true, false, true, false])
    );

    // A feature sent with an empty value emulates nothing.
    client
        .run(
            "Emulation.setEmulatedMedia",
            json!({ "features": [{ "name": "prefers-color-scheme", "value": "" }] }),
            Some(&session),
        )
        .await;
    assert_eq!(
        media(&mut client, &session).await,
        json!([true, false, own[2], own[3]])
    );
}

/// The media queries and the cascade are moved by one pin.
#[cfg(feature = "screenshot")]
#[tokio::test]
async fn emulated_media_moves_the_cascade_with_the_queries() {
    let (_guard, mut client, session) = bridge(PAGE).await;
    let colour = "getComputedStyle(document.getElementById('t')).color";
    let on = Some(session.as_str());
    let own = client.eval(colour, &session).await;
    assert_ne!(own, json!("rgb(1, 2, 3)"), "the page's own colour");

    client
        .run(
            "Emulation.setEmulatedMedia",
            json!({ "features": [{ "name": "prefers-color-scheme", "value": "dark" }] }),
            on,
        )
        .await;
    assert_eq!(
        media(&mut client, &session).await,
        json!([true, false, true, false])
    );
    assert_eq!(client.eval(colour, &session).await, json!("rgb(1, 2, 3)"));

    client
        .run(
            "Emulation.setEmulatedMedia",
            json!({ "features": [{ "name": "prefers-color-scheme", "value": "light" }] }),
            on,
        )
        .await;
    assert_eq!(
        media(&mut client, &session).await,
        json!([true, false, false, true])
    );
    assert_eq!(client.eval(colour, &session).await, own);
}

/// Touch emulation moves the touch points and what the pointer queries
/// answer — not whether the page has touch events, as in Chrome.
#[tokio::test]
async fn touch_emulation_moves_the_pointers_and_the_touch_points() {
    let (_guard, mut client, session) = bridge("about:blank").await;
    let on = Some(session.as_str());
    let own = touch(&mut client, &session).await;
    assert_eq!(own, json!([0, false, false, true, false, false]), "{own}");

    client
        .run(
            "Emulation.setTouchEmulationEnabled",
            json!({ "enabled": true, "maxTouchPoints": 5 }),
            on,
        )
        .await;
    assert_eq!(
        touch(&mut client, &session).await,
        json!([5, false, true, false, true, true])
    );
    // Off, nothing of it stays.
    client
        .run(
            "Emulation.setTouchEmulationEnabled",
            json!({ "enabled": false }),
            on,
        )
        .await;
    assert_eq!(touch(&mut client, &session).await, own);
}

/// The pins are the session's, not the document's: they are what the next
/// page reads. Each is put back the way Chrome puts it back — an empty id,
/// an empty `userAgent`, an `acceptLanguage` left out.
#[tokio::test]
async fn the_pins_are_the_sessions_and_outlive_the_page() {
    let (_guard, mut client, session) = bridge(PAGE).await;
    let on = Some(session.as_str());
    // The page's own zone, locale, language, UA and platform.
    let own = identity(&mut client, &session).await;

    client
        .run(
            "Emulation.setTimezoneOverride",
            json!({ "timezoneId": "Asia/Shanghai" }),
            on,
        )
        .await;
    client
        .run(
            "Emulation.setLocaleOverride",
            json!({ "locale": "de-DE" }),
            on,
        )
        .await;
    // The zone is where it was sent, Intl is moved by the locale — and the
    // language is neither's to move.
    let mut expected = own.clone();
    expected[0] = json!("Asia/Shanghai");
    expected[1] = json!("de-DE");
    expected[2] = json!("1.234,5");
    assert_eq!(identity(&mut client, &session).await, expected);

    client
        .run(
            "Emulation.setUserAgentOverride",
            json!({ "userAgent": "Probe/1.0", "acceptLanguage": "ja-JP", "platform": "Win32" }),
            on,
        )
        .await;
    expected.as_array_mut().expect("a list")[3..].clone_from_slice(&[
        json!("Probe/1.0"),
        json!("ja-JP"),
        json!("Win32"),
    ]);
    assert_eq!(identity(&mut client, &session).await, expected);

    // The UA sent empty is the page's own again; the rest is as sent.
    client
        .run(
            "Emulation.setUserAgentOverride",
            json!({ "userAgent": "", "acceptLanguage": "de-DE" }),
            on,
        )
        .await;
    expected.as_array_mut().expect("a list")[3..].clone_from_slice(&[
        own[3].clone(),
        json!("de-DE"),
        own[5].clone(),
    ]);
    assert_eq!(identity(&mut client, &session).await, expected);

    // A size of the session's own. Under it the page is touch-first here;
    // in Chrome the layout is the 980 of a page with no viewport `meta`.
    let before = size(&mut client, &session).await;
    client
        .run(
            "Emulation.setDeviceMetricsOverride",
            json!({
                "width": 390, "height": 844, "deviceScaleFactor": 2, "mobile": true,
                "screenWidth": 390, "screenHeight": 844,
                "screenOrientation": { "type": "portraitPrimary", "angle": 0 },
            }),
            on,
        )
        .await;
    let sized = json!([390, 844, 2, 390, 844, 844, "portrait-primary", 0]);
    assert_eq!(size(&mut client, &session).await, sized);
    assert_eq!(
        touch(&mut client, &session).await,
        json!([5, false, true, false, true, true])
    );
    #[cfg(feature = "screenshot")]
    assert_eq!(
        client
            .eval("document.body.getBoundingClientRect().width", &session)
            .await,
        json!(390)
    );
    let metrics = client.run("Page.getLayoutMetrics", json!({}), on).await;
    let viewport = json!({ "pageX": 0, "pageY": 0, "clientWidth": 390, "clientHeight": 844 });
    assert_eq!(metrics["layoutViewport"], viewport);
    assert_eq!(metrics["cssLayoutViewport"], viewport);
    assert_eq!(metrics["visualViewport"]["clientWidth"], 390);
    assert_eq!(metrics["cssVisualViewport"]["clientHeight"], 844);

    // The next document reads every pin.
    client
        .run("Page.navigate", json!({ "url": NEXT }), on)
        .await;
    for _ in 0..100 {
        if client
            .eval("document.getElementById('t').textContent", &session)
            .await
            == json!("next")
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(size(&mut client, &session).await, sized);
    assert_eq!(identity(&mut client, &session).await, expected);
    assert_eq!(
        touch(&mut client, &session).await,
        json!([5, false, true, false, true, true])
    );

    // And each is let go by what is empty.
    client
        .run("Emulation.clearDeviceMetricsOverride", json!({}), on)
        .await;
    assert_eq!(size(&mut client, &session).await, before);

    client
        .run(
            "Emulation.setTimezoneOverride",
            json!({ "timezoneId": "" }),
            on,
        )
        .await;
    // The zone is the language's here; in Chrome it is the host's.
    expected[0] = json!("Europe/Berlin");
    assert_eq!(identity(&mut client, &session).await, expected);

    client
        .run("Emulation.setLocaleOverride", json!({ "locale": "" }), on)
        .await;
    // Intl's locale is the language's again.
    expected[1] = json!("de-DE");
    assert_eq!(identity(&mut client, &session).await, expected);

    // The language left out is the page's own, and the zone with it.
    client
        .run(
            "Emulation.setUserAgentOverride",
            json!({ "userAgent": "" }),
            on,
        )
        .await;
    assert_eq!(identity(&mut client, &session).await, own);
}
