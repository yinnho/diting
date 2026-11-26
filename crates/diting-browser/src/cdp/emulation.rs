//! Page-level CDP commands that are neither script nor input: the
//! `Emulation` domain, document-start scripts, the two `Network` setters
//! and `Page.getLayoutMetrics`. Every arm moves engine state that was
//! built for it; a command this engine cannot apply is a protocol error
//! that says so, never an empty ok.
//!
//! The parameters are read the way Chrome reads them (`params`), and a
//! value out of range is `-32602` in the words of its check, in the order
//! Chrome makes them.
//!
//! Where this engine's answers differ from Chrome's:
//! - `setDeviceMetricsOverride`: `mobile` moves the pointer answers and
//!   touch points, but the viewport `<meta>` is not processed, so the
//!   layout viewport is the width sent. A width or height of 0 (Chrome:
//!   follow the window) and a `scale` other than 1 (screenshots are not
//!   rescaled) are refused. `dontSetVisibleSize`, `viewport`,
//!   `displayFeature` and `positionX/Y` are typed as Chrome types them and
//!   not applied (Chrome also requires fields inside the last two). A
//!   `deviceScaleFactor` of 0 is the persona's ratio, as in Chrome.
//! - `setEmulatedMedia`: a media type Chrome does not know is `screen`
//!   here; there it is neither `screen` nor `print`. Every call replaces
//!   both parts, as in Chrome. A feature value Chrome does not know
//!   emulates nothing there — the system's answer stands, so
//!   `prefers-reduced-motion: bogus` reads `no-preference` — and is taken
//!   as sent here: that value matches, and neither known one does.
//! - `setTimezoneOverride`: an id ICU does not know is refused with
//!   Chrome's words — the page would be pinned to a zone it cannot read.
//!   Chrome refuses it too while no override is on; with one on, it takes
//!   the call and leaves the zone as it was. The case of an id it knows is
//!   not checked here: Chrome refuses every spelling but the zone's own
//!   (`ASIA/SHANGHAI` is "Invalid timezone id" there) and this engine
//!   takes them, as its `Intl` does — telling them apart would mean
//!   shipping ICU's list of the spellings.
//! - `setLocaleOverride`: the locale is read as Chrome reads it — ICU's old
//!   spellings turned into BCP 47, then read with the ICU4X parser V8
//!   already links ([`locale_tag`]) — so what Chrome takes is taken and
//!   what it refuses is "Invalid locale name". What the page reads under it
//!   is what Chrome reads with that tag passed to `Intl` by name, not what
//!   it reads under its own override, which moves the isolate's default
//!   locale (the ICU default is process-wide here). Of 84 tags measured,
//!   73 format dates, numbers, sorting and units alike. The rest: a tag
//!   this V8 has no data for (`und`, `eu-ES`, `abc`) formats in the root
//!   there and in the persona's language here; a variant is dropped here
//!   (`en-abcde` → `en`); `sh` is `sr-Latn` here; `en-US-POSIX` keeps its
//!   group separators there and loses them here. And everywhere
//!   `Intl.Collator().resolvedOptions().locale` keeps no region here (`en`
//!   for `en-GB`). The override is the page's, not a session's: a second
//!   session's call replaces the first's where Chrome refuses it ("Another
//!   locale override is already in effect"), and a session that detaches
//!   leaves it on where Chrome drops it.
//! - `setUserAgentOverride`: an empty `userAgent` is the page's own UA
//!   again and an `acceptLanguage` left out or empty its own language, on
//!   the wire and in the page, as in Chrome (there `navigator.languages`
//!   keeps the old list on a page that has not navigated). `userAgentData`
//!   follows the UA string: `userAgentMetadata` is read as Chrome reads
//!   it and not applied, where Chrome takes `platform`, `mobile` and the
//!   rest from it — and answers an empty `brands` and `platform` for a UA
//!   sent without it, where this engine reads both off the UA. `platform`
//!   left out is cleared.
//! - `addScriptToEvaluateOnNewDocument`: `worldName` is accepted and not
//!   used — the engine has one realm, the script runs in the page's world —
//!   and `includeCommandLineAPI` is refused. The scripts run in every new
//!   document of the page, its main document only — not in its frames;
//!   Chrome runs a session's in every frame, once it called `Page.enable`.
//!   Identifiers are counted per process here, per session in Chrome.
use std::collections::HashMap;
use std::sync::atomic::Ordering;

use serde_json::{json, Value};

use crate::page::{Page, INTERACTION_EVAL_TIMEOUT};

use crate::session::{send_command, EmulateOp, PageOp, SessionCommand};

use super::params::{
    boolean, integer, invalid, invalid_params, kinds, number, object, optional, optional_boolean,
    optional_integer, optional_number, optional_string, string, texts, Kind,
};
use super::runtime::{no_such_flag, AWAIT_MS};
use super::Failure;

const METRICS: &str = "Emulation.setDeviceMetricsOverride";
/// Chrome's bound on the pixel sizes `setDeviceMetricsOverride` takes.
const MAX_METRIC: i64 = 10_000_000;

/// Read from the open document: `Page.getLayoutMetrics` answers with what
/// the page sees, not with what the bridge was last told. Playwright builds
/// every screenshot's clip from these numbers.
const LAYOUT_METRICS: &str = "(function(){\
    var vv = window.visualViewport || {};\
    var root = document.documentElement || document.body;\
    var iw = window.innerWidth || 0, ih = window.innerHeight || 0;\
    var sx = window.scrollX || 0, sy = window.scrollY || 0;\
    return {\
        iw: iw, ih: ih, sx: sx, sy: sy,\
        vw: vv.width || iw, vh: vv.height || ih,\
        ox: vv.offsetLeft || 0, oy: vv.offsetTop || 0, scale: vv.scale || 1,\
        cw: Math.max(iw, root ? root.scrollWidth || 0 : 0),\
        ch: Math.max(ih, root ? root.scrollHeight || 0 : 0)\
    };\
})()";

/// Run one page-level command. `init_scripts` is the session's
/// `Page.addScriptToEvaluateOnNewDocument` registry, (identifier, source).
pub(crate) async fn handle(
    page: &mut Page,
    init_scripts: &mut Vec<(String, String)>,
    method: &str,
    params: &Value,
) -> Result<Value, Failure> {
    match method {
        op @ ("Emulation.setEmulatedMedia"
        | "Emulation.setTouchEmulationEnabled"
        | "Emulation.setTimezoneOverride"
        | "Emulation.setLocaleOverride"
        | "Emulation.setUserAgentOverride"
        | "Emulation.setDeviceMetricsOverride"
        | "Emulation.clearDeviceMetricsOverride") => {
            let op = emulate_op(op, params)?;
            apply_op(page, op).await;
            Ok(json!({}))
        }
        "Page.addScriptToEvaluateOnNewDocument" => {
            add_init_script(page, init_scripts, params).await
        }
        "Page.removeScriptToEvaluateOnNewDocument" => {
            remove_init_script(page, init_scripts, params)
        }
        "Page.getLayoutMetrics" => layout_metrics(page),
        "Network.setBlockedURLs" => {
            // Left out, the list is emptied — nothing blocked.
            let urls = texts(params, "urls")?
                .unwrap_or_default()
                .into_iter()
                .map(str::to_string)
                .collect();
            page.inner.set_blocked_urls(urls);
            Ok(json!({}))
        }
        "Network.setExtraHTTPHeaders" => extra_headers(page, params).await,
        // Unreachable from the bridge: `cdp::PAGE_CHANNEL` is the only way
        // here, and it lists every arm above.
        other => Err(Failure::from(format!(
            "'{other}' has no handler on the session thread"
        ))),
    }
}

/// `Emulation.setEmulatedMedia`: every call replaces the whole set, as in
/// Chrome — a `media` left out is back to `screen`, `features` left out is
/// none. An unknown media type or feature name is taken and changes
/// nothing; a feature with an empty value emulates nothing.
/// The `Emulation.*` params as the [`EmulateOp`] they pin — the face's
/// whole job for these commands now: read and check, never touch the
/// page.
fn emulate_op(method: &str, params: &Value) -> Result<EmulateOp, Failure> {
    match method {
        "Emulation.setEmulatedMedia" => emulated_media_op(params),
        "Emulation.setTouchEmulationEnabled" => {
            let enabled = boolean(params, "enabled")?;
            // Read whatever `enabled` says, as Chrome reads it.
            let points = match optional_integer(params, "maxTouchPoints")? {
                None => 1,
                Some(points) if (1..=16).contains(&points) => points,
                Some(_) => return Err(invalid("Touch points must be between 1 and 16")),
            };
            Ok(EmulateOp::TouchEmulation {
                enabled,
                max_points: points as u32,
            })
        }
        "Emulation.setTimezoneOverride" => {
            // An empty id clears the zone.
            let tz = string(params, "timezoneId")?;
            Ok(EmulateOp::Timezone {
                timezone: Some(tz.to_string()),
            })
        }
        "Emulation.setLocaleOverride" => locale_op(params),
        "Emulation.setUserAgentOverride" => user_agent_op(params),
        "Emulation.setDeviceMetricsOverride" => device_metrics_op(params),
        "Emulation.clearDeviceMetricsOverride" => Ok(EmulateOp::ClearDeviceMetrics),
        _ => Err(Failure::from(format!(
            "'{method}' is not an emulation command"
        ))),
    }
}

/// Apply an [`EmulateOp`] on the session thread — the `handle()` fallback
/// path's half of what `SessionActor::on_emulate` does for the IR path.
async fn apply_op(page: &mut Page, op: EmulateOp) {
    match op {
        EmulateOp::EmulatedMedia { features, media } => {
            page.inner.set_emulated_media(Some(features), Some(media));
        }
        EmulateOp::TouchEmulation {
            enabled,
            max_points,
        } => {
            page.inner.set_touch_emulation(enabled, max_points);
        }
        EmulateOp::Timezone { timezone } => {
            if let Some(tz) = &timezone {
                if !tz.is_empty() && !page.inner.timezone_id_supported(tz) {
                    // The fallback path cannot report through the IR
                    // reply — but `handle()` already validated via the op
                    // producer, so this is unreachable in practice.
                    return;
                }
                page.inner
                    .set_timezone_override((!tz.is_empty()).then(|| tz.clone()));
            } else {
                page.inner.set_timezone_override(None);
            }
        }
        EmulateOp::Locale { locale } => {
            page.inner.set_locale_override(locale);
        }
        EmulateOp::UserAgent {
            user_agent,
            accept_language,
            platform,
        } => {
            page.inner
                .set_user_agent_override(
                    &user_agent,
                    accept_language.as_deref(),
                    platform.as_deref(),
                )
                .await;
        }
        EmulateOp::DeviceMetrics {
            width,
            height,
            mobile,
            ratio,
            panel,
            orientation,
        } => {
            page.inner
                .set_viewport_override(width, height, mobile, Some(ratio));
            page.inner.set_screen_override(panel, orientation);
        }
        EmulateOp::ClearDeviceMetrics => {
            page.inner.clear_viewport_override();
        }
    }
}

/// The `Page.*`/`Network.*` params as the [`PageOp`] they run — the
/// face's whole job: read, check, mint the init-script identifier.
fn page_op(method: &str, params: &Value) -> Result<PageOp, Failure> {
    match method {
        "Page.addScriptToEvaluateOnNewDocument" => {
            const METHOD: &str = "Page.addScriptToEvaluateOnNewDocument";
            let source = string(params, "source")?;
            kinds(
                params,
                &[
                    ("runImmediately", Kind::Bool),
                    ("includeCommandLineAPI", Kind::Bool),
                    ("worldName", Kind::Text),
                ],
            )?;
            no_such_flag(
                METHOD,
                params,
                &[("includeCommandLineAPI", "there is no command line API")],
            )?;
            Ok(PageOp::AddInitScript {
                identifier: super::SCRIPT_IDS
                    .fetch_add(1, Ordering::Relaxed)
                    .to_string(),
                source: source.to_string(),
                run_immediately: optional_boolean(params, "runImmediately")? == Some(true),
            })
        }
        "Page.removeScriptToEvaluateOnNewDocument" => Ok(PageOp::RemoveInitScript {
            identifier: string(params, "identifier")?.to_string(),
        }),
        "Network.setBlockedURLs" => {
            // Left out, the list is emptied — nothing blocked.
            let urls = texts(params, "urls")?
                .unwrap_or_default()
                .into_iter()
                .map(str::to_string)
                .collect();
            Ok(PageOp::BlockedUrls { urls })
        }
        "Network.setExtraHTTPHeaders" => {
            let sent = optional(params, "headers", Value::as_object)?.ok_or_else(invalid_params)?;
            let headers: HashMap<String, String> = sent
                .iter()
                .map(|(name, value)| Ok((name.clone(), header_value(value)?)))
                .collect::<Result<_, Failure>>()?;
            for (name, value) in &headers {
                check_header(name, value)?;
            }
            Ok(PageOp::ExtraHeaders { headers })
        }
        _ => Err(Failure::from(format!("'{method}' is not a page command"))),
    }
}

/// A `Page.*`/`Network.*` command on the IR — params become a [`PageOp`]
/// here, the session thread applies it.
pub(super) async fn page_run(engine: &str, method: &str, params: &Value) -> Result<Value, Failure> {
    let op = page_op(method, params)?;
    // The init-script identifier is the face's — minted in `page_op`,
    // so the answer names it without a reply payload.
    let identifier = match &op {
        PageOp::AddInitScript { identifier, .. } => Some(identifier.clone()),
        _ => None,
    };
    send_command(engine, |reply| SessionCommand::Page { op, reply })
        .await
        .map_err(|err| engine_failure_word(super::session_msg(err)))?;
    Ok(match identifier {
        Some(identifier) => json!({ "identifier": identifier }),
        None => json!({}),
    })
}

/// `Page.getLayoutMetrics` on the IR: the session thread reads the raw
/// numbers, the face shapes Chrome's triplet from them.
pub(super) async fn metrics_run(engine: &str) -> Result<Value, Failure> {
    let metrics = send_command(engine, |reply| SessionCommand::PageMetrics { reply })
        .await
        .map_err(|err| engine_failure_word(super::session_msg(err)))?;
    let f = |name: &str| metrics.get(name).and_then(Value::as_f64).unwrap_or(0.0);
    let (iw, ih) = (f("iw"), f("ih"));
    let (sx, sy) = (f("sx"), f("sy"));
    let (vw, vh) = (f("vw"), f("vh"));
    let (px, py) = (sx + f("ox"), sy + f("oy"));
    let layout = json!({
        "pageX": measure(sx), "pageY": measure(sy),
        "clientWidth": measure(iw), "clientHeight": measure(ih),
    });
    let visual = json!({
        "offsetX": measure(f("ox")), "offsetY": measure(f("oy")),
        "pageX": measure(px), "pageY": measure(py),
        "clientWidth": measure(vw), "clientHeight": measure(vh),
        "scale": measure(f("scale")), "zoom": 1,
    });
    let content = json!({ "x": 0, "y": 0, "width": measure(f("cw")), "height": measure(f("ch")) });
    Ok(json!({
        "layoutViewport": layout,
        "visualViewport": visual,
        "contentSize": content,
        "cssLayoutViewport": layout,
        "cssVisualViewport": visual,
        "cssContentSize": content,
    }))
}

/// Engine errors through the CDP face's words — `engine_failure` for
/// the generic case.
fn engine_failure_word(err: String) -> Failure {
    super::runtime::engine_failure(err)
}

/// An `Emulation.*` command on the IR: params become an [`EmulateOp`]
/// here, the session thread applies it, `{}` comes back. The one refusal
/// the page itself must answer — a timezone the runtime does not know —
/// comes back as its word and is mapped to `invalid` here.
pub(super) async fn emulate_run(
    engine: &str,
    method: &str,
    params: &Value,
) -> Result<Value, Failure> {
    let op = emulate_op(method, params)?;
    send_command(engine, |reply| SessionCommand::Emulate { op, reply })
        .await
        .map_err(|err| match super::session_msg(err).as_str() {
            "Invalid timezone id" => invalid("Invalid timezone id"),
            msg => super::runtime::engine_failure(msg.to_string()),
        })?;
    Ok(json!({}))
}

fn emulated_media_op(params: &Value) -> Result<EmulateOp, Failure> {
    // The page's own setter keeps a part it is not given; this one is sent
    // both every time, or a call naming only `features` would leave the
    // media type `print` for the rest of the session.
    let media = Some(
        optional_string(params, "media")?
            .filter(|media| !media.is_empty())
            .map(str::to_string),
    );
    let features = match optional(params, "features", Value::as_array)? {
        None => Vec::new(),
        Some(list) => name_values(list)?,
    };
    let features: Vec<(String, String)> = features
        .into_iter()
        .filter(|(_, value)| !value.is_empty())
        .collect();
    Ok(EmulateOp::EmulatedMedia {
        features,
        media: media.flatten(),
    })
}

/// Each feature is a `name` and a `value`, both strings; an empty name is
/// taken and matches nothing.
fn name_values(list: &[Value]) -> Result<Vec<(String, String)>, Failure> {
    list.iter()
        .map(|item| {
            Ok((
                string(item, "name")?.to_string(),
                string(item, "value")?.to_string(),
            ))
        })
        .collect()
}

/// `Emulation.setLocaleOverride`: no locale sent, or an empty one, clears
/// back to the persona's language. Anything else is read as Chrome reads
/// it ([`locale_tag`]); one it refuses is refused in its words.
fn locale_op(params: &Value) -> Result<EmulateOp, Failure> {
    let locale = optional_string(params, "locale")?.unwrap_or("");
    if locale.is_empty() {
        return Ok(EmulateOp::Locale { locale: None });
    }
    match locale_tag(locale) {
        Some(tag) => Ok(EmulateOp::Locale { locale: Some(tag) }),
        None => Err(Failure::from("Invalid locale name")),
    }
}

/// The tag `Emulation.setLocaleOverride` pins for `locale`; `None` is one
/// Chrome refuses. Chrome's inspector reads a locale in two steps, and
/// these are they. ICU's old spelling — `_` between subtags, a codeset
/// after `.`, keywords after `@` — becomes BCP 47 first
/// ([`legacy_to_bcp47`]); then the tag is read with the parser of the ICU
/// V8 already links, ICU4X, so the bridge borrows it rather than keeping
/// rules of its own. Back comes the tag as parsed: case and keyword order
/// normalized, subtags as written. What it does not do is what Chrome's
/// canonicalizer does next. Aliases are left to Intl, which runs them
/// where it knows the name (`cmn-Hans-CN` comes out `zh-Hans-CN` either
/// way) and not where Chrome keeps the old one on purpose (`sh`).
fn locale_tag(locale: &str) -> Option<String> {
    let tag = legacy_to_bcp47(locale).unwrap_or_else(|| locale.to_string());
    let parsed = icu_locale_core::Locale::try_from_str(&tag).ok()?;
    Some(parsed.to_string())
}

/// Chromium's `ConvertLegacyCodeToBcp47IfNecessary` (base/i18n), ported
/// whole: `None` for a code with nothing old in it. What stands before the
/// first `.` or `@` is the tag, each `_` read as `-`; the codeset after `.`
/// is dropped. Keywords come after `@`, cut by `;` or `,`, and each becomes
/// a subtag of one `-u-` extension ([`legacy_keyword`]). Nothing after the
/// `@` is no extension at all; separators alone are an empty one, `en-u`,
/// which no parser takes.
fn legacy_to_bcp47(code: &str) -> Option<String> {
    let (dot, at) = (code.find('.'), code.find('@'));
    if dot.is_none() && at.is_none() && !code.contains('_') {
        return None;
    }
    // Whichever of the two comes first; the whole code when neither does.
    let end = dot.into_iter().chain(at).min().unwrap_or(code.len());
    let mut tag = code[..end].replace('_', "-");
    let keywords = at.map(|at| &code[at + 1..]).unwrap_or_default();
    if !keywords.is_empty() {
        tag.push_str("-u");
        for keyword in keywords.split([';', ',']).filter(|part| !part.is_empty()) {
            tag.push('-');
            tag.push_str(&legacy_keyword(keyword));
        }
    }
    Some(tag)
}

/// One keyword of ICU's old spelling as BCP 47. A bare `valencia` is the
/// variant it became; any other bare word is looked up as written, so
/// `calendar` is the key `ca` and `CALENDAR` is the attribute it spells.
/// In a `key=value` the key is looked up without regard to case and the
/// value is written as it came, under its old name unless it has a new
/// one ([`legacy_value`]).
fn legacy_keyword(keyword: &str) -> String {
    let Some((key, value)) = keyword.split_once('=') else {
        return match keyword {
            "valencia" => "va-valencia".to_string(),
            other => legacy_key(other).unwrap_or(other).to_string(),
        };
    };
    let name = key.to_ascii_lowercase();
    format!(
        "{}-{}",
        legacy_key(&name).unwrap_or(key),
        legacy_value(&name, value)
    )
}

/// The old values that are not BCP 47 types — Chromium's list, read by
/// the old key without regard to case and by the value as it came. A page
/// finds none of them: `en_US@calendar=ethiopic-amete-alem` dates by the
/// calendar only as `ca-ethioaa`, and `de_DE@collation=phonebook` sorts
/// by it only as `co-phonebk`. Any value not listed is its own type.
fn legacy_value<'a>(key: &str, value: &'a str) -> &'a str {
    LEGACY_VALUES
        .iter()
        .find(|((old_key, old_value), _)| *old_key == key && *old_value == value)
        .map_or(value, |(_, new)| *new)
}

/// `kLegacyIcuToBcp47ValueMap` in the file named at [`legacy_key`].
const LEGACY_VALUES: &[((&str, &str), &str)] = &[
    (("calendar", "ethiopic-amete-alem"), "ethioaa"),
    (("calendar", "gregorian"), "gregory"),
    (("colalternate", "non-ignorable"), "noignore"),
    (("colbackwards", "no"), "false"),
    (("colbackwards", "yes"), "true"),
    (("colcasefirst", "no"), "false"),
    (("colcaselevel", "no"), "false"),
    (("colcaselevel", "yes"), "true"),
    (("colhiraganaquaternary", "no"), "false"),
    (("colhiraganaquaternary", "yes"), "true"),
    (("collation", "dictionary"), "dict"),
    (("collation", "gb2312han"), "gb2312"),
    (("collation", "phonebook"), "phonebk"),
    (("collation", "traditional"), "trad"),
    (("colnormalization", "no"), "false"),
    (("colnormalization", "yes"), "true"),
    (("colnumeric", "no"), "false"),
    (("colnumeric", "yes"), "true"),
    (("colstrength", "identical"), "identic"),
    (("colstrength", "primary"), "level1"),
    (("colstrength", "quaternary"), "level4"),
    (("colstrength", "secondary"), "level2"),
    (("colstrength", "tertiary"), "level3"),
    (("measure", "imperial"), "uksystem"),
    (("numbers", "traditional"), "traditio"),
    (("timezone", "aqams"), "aqmcm"),
    (("timezone", "aukns"), "auhba"),
    (("timezone", "caffs"), "cawnp"),
    (("timezone", "camtr"), "cator"),
    (("timezone", "canpg"), "cator"),
    (("timezone", "capnt"), "caiql"),
    (("timezone", "cathu"), "cator"),
    (("timezone", "cayzf"), "caedm"),
    (("timezone", "cnckg"), "cnsha"),
    (("timezone", "cnhrb"), "cnsha"),
    (("timezone", "cnkhg"), "cnurc"),
    (("timezone", "cst6cdt"), "uschi"),
    (("timezone", "est5edt"), "usnyc"),
    (("timezone", "gaza"), "gazastrp"),
    (("timezone", "mncoq"), "mnuln"),
    (("timezone", "mst7mdt"), "usden"),
    (("timezone", "mxstis"), "mxtij"),
    (("timezone", "pst8pdt"), "uslax"),
    (("timezone", "uaozh"), "uaiev"),
    (("timezone", "uauzh"), "uaiev"),
    (("timezone", "umjon"), "ushnl"),
    (("timezone", "usnavajo"), "usden"),
];

/// The names ICU gave its keywords before BCP 47 keys, and the key each
/// became — Chromium's `kLegacyIcuToBcp47KeyMap`, in the same file as
/// [`LEGACY_VALUES`] (`base/i18n/internal/legacy_icu_converter.cc`).
fn legacy_key(name: &str) -> Option<&'static str> {
    match name {
        "calendar" => Some("ca"),
        "colalternate" => Some("ka"),
        "colbackwards" => Some("kb"),
        "colcasefirst" => Some("kf"),
        "colcaselevel" => Some("kc"),
        "colhiraganaquaternary" => Some("kh"),
        "collation" => Some("co"),
        "colnormalization" => Some("kk"),
        "colnumeric" => Some("kn"),
        "colreorder" => Some("kr"),
        "colstrength" => Some("ks"),
        "currency" => Some("cu"),
        "hours" => Some("hc"),
        "measure" => Some("ms"),
        "numbers" => Some("nu"),
        "timezone" => Some("tz"),
        "variabletop" => Some("vt"),
        _ => None,
    }
}

/// `Emulation.setUserAgentOverride`: the UA on every transport and in the
/// page, the language where it was sent, and `navigator.platform` — which
/// replaces, so left out or empty is back on what the UA says. An empty
/// `userAgent` is the page's own UA again and an `acceptLanguage` left out
/// or empty its own language, as in Chrome.
fn user_agent_op(params: &Value) -> Result<EmulateOp, Failure> {
    let ua = string(params, "userAgent")?;
    let language = optional_string(params, "acceptLanguage")?;
    let platform = optional_string(params, "platform")?;
    // Chrome reads every field before its handler looks at any value.
    let metadata = user_agent_metadata(params)?;
    if ua.is_empty() && metadata {
        return Err(invalid(
            "Empty userAgent invalid with userAgentMetadata provided",
        ));
    }
    Ok(EmulateOp::UserAgent {
        user_agent: ua.to_string(),
        accept_language: language.map(str::to_string),
        platform: platform.map(str::to_string),
    })
}

/// `userAgentMetadata` the way Chrome reads it: an object with
/// `architecture`, `model`, `platform` and `platformVersion` as strings and
/// `mobile` as a boolean; `brands` and `fullVersionList`, where sent, are
/// arrays of `{ brand, version }` strings and `wow64` a boolean. Answers
/// whether one was sent. Nothing is done with it.
fn user_agent_metadata(params: &Value) -> Result<bool, Failure> {
    let Some(metadata) = object(params, "userAgentMetadata")? else {
        return Ok(false);
    };
    for field in ["architecture", "model", "platform", "platformVersion"] {
        string(metadata, field)?;
    }
    boolean(metadata, "mobile")?;
    let versions = |value: &Value| {
        for entry in value.as_array()? {
            entry.get("brand")?.as_str()?;
            entry.get("version")?.as_str()?;
        }
        Some(())
    };
    optional(metadata, "brands", versions)?;
    optional(metadata, "fullVersionList", versions)?;
    optional_boolean(metadata, "wow64")?;
    Ok(true)
}

/// `Emulation.setDeviceMetricsOverride`: the viewport pin and the panel
/// the page sits on. Chrome's checks come first, in Chrome's order; what
/// this engine cannot apply is refused after them.
fn device_metrics_op(params: &Value) -> Result<EmulateOp, Failure> {
    // Every field is read before any value is looked at.
    let width = integer(params, "width")?;
    let height = integer(params, "height")?;
    let ratio = number(params, "deviceScaleFactor")?;
    let mobile = boolean(params, "mobile")?;
    let screen = (
        optional_integer(params, "screenWidth")?,
        optional_integer(params, "screenHeight")?,
    );
    let position = (
        optional_integer(params, "positionX")?,
        optional_integer(params, "positionY")?,
    );
    let scale = optional_number(params, "scale")?;
    let orientation = match object(params, "screenOrientation")? {
        None => None,
        // Both have to be sent.
        Some(orientation) => Some((string(orientation, "type")?, integer(orientation, "angle")?)),
    };
    optional_boolean(params, "dontSetVisibleSize")?;
    object(params, "viewport")?;
    object(params, "displayFeature")?;

    let too_big = |size: i64| size < 0 || size > MAX_METRIC;
    if too_big(width) || too_big(height) {
        return Err(invalid(
            "Width and height values must be positive, not greater than 10000000",
        ));
    }
    if screen.0.is_some_and(too_big) || screen.1.is_some_and(too_big) {
        return Err(invalid(
            "Screen width and height values must be positive, not greater than 10000000",
        ));
    }
    // Both sizes, both above zero, make a panel of their own. Without one
    // the page sits on the host's screen, where it cannot be moved — or,
    // under mobile emulation, on a panel the size of the viewport.
    let panel = match screen {
        (Some(width), Some(height)) if width > 0 && height > 0 => Some((width, height)),
        _ if mobile => Some((width, height)),
        _ => None,
    };
    let emulated = matches!(screen, (Some(w), Some(h)) if w > 0 && h > 0);
    if !emulated && (position.0.unwrap_or(0) != 0 || position.1.unwrap_or(0) != 0) {
        return Err(invalid("View position should be on the screen"));
    }
    if ratio < 0.0 {
        return Err(invalid("deviceScaleFactor must be non-negative"));
    }
    if let Some(scale) = scale {
        if scale <= 0.0 || scale > 10.0 {
            return Err(invalid("scale must be positive, not greater than 10"));
        }
    }
    if let Some((kind, _)) = orientation {
        if !matches!(
            kind,
            "portraitPrimary" | "portraitSecondary" | "landscapePrimary" | "landscapeSecondary"
        ) {
            return Err(invalid("Invalid screen orientation type value"));
        }
    }
    if let Some((_, angle)) = orientation {
        if !(0..360).contains(&angle) {
            return Err(invalid(
                "Screen orientation angle must be non-negative, less than 360",
            ));
        }
    }
    // Chrome reads a 0 side as "the window's"; this engine has no window.
    if width == 0 || height == 0 {
        return Err(Failure::from(format!(
            "{METRICS}: a width or height of 0 (follow the window) is not supported, send the size"
        )));
    }
    if let Some(scale) = scale {
        if scale != 1.0 {
            return Err(Failure::from(format!(
                "{METRICS}: scale {scale} is not supported, screenshots are not rescaled"
            )));
        }
    }

    let orientation = orientation.map(|(kind, angle)| match kind {
        "portraitPrimary" => ("portrait-primary", angle),
        "portraitSecondary" => ("portrait-secondary", angle),
        "landscapePrimary" => ("landscape-primary", angle),
        _ => ("landscape-secondary", angle),
    });
    Ok(EmulateOp::DeviceMetrics {
        width: width as f32,
        height: height as f32,
        mobile,
        ratio,
        panel: panel.map(|(width, height)| (width as f32, height as f32)),
        orientation,
    })
}

/// Register the source in the page's document-start group and answer the
/// identifier Chrome would. `runImmediately` also evaluates it in the
/// document that is open now; a throw there is the page's business, not a
/// protocol error.
async fn add_init_script(
    page: &mut Page,
    init_scripts: &mut Vec<(String, String)>,
    params: &Value,
) -> Result<Value, Failure> {
    const METHOD: &str = "Page.addScriptToEvaluateOnNewDocument";
    let source = string(params, "source")?;
    kinds(
        params,
        &[
            ("runImmediately", Kind::Bool),
            ("includeCommandLineAPI", Kind::Bool),
            ("worldName", Kind::Text),
        ],
    )?;
    no_such_flag(
        METHOD,
        params,
        &[("includeCommandLineAPI", "there is no command line API")],
    )?;
    let identifier = super::SCRIPT_IDS
        .fetch_add(1, Ordering::Relaxed)
        .to_string();
    page.inner.add_preload_script(source.to_string());
    init_scripts.push((identifier.clone(), source.to_string()));
    if optional_boolean(params, "runImmediately")? == Some(true) {
        page.inner
            .evaluate_remote_outcome(source, false, false, AWAIT_MS, None)
            .await;
    }
    Ok(json!({ "identifier": identifier }))
}

fn remove_init_script(
    page: &mut Page,
    init_scripts: &mut Vec<(String, String)>,
    params: &Value,
) -> Result<Value, Failure> {
    let identifier = string(params, "identifier")?;
    let at = init_scripts
        .iter()
        .position(|(id, _)| id == identifier)
        .ok_or_else(|| Failure::from("Script not found"))?;
    let (_, source) = init_scripts.remove(at);
    page.inner.remove_preload_script(&source);
    Ok(json!({}))
}

/// `Network.setExtraHTTPHeaders`: the whole set replaces the one before.
async fn extra_headers(page: &mut Page, params: &Value) -> Result<Value, Failure> {
    let sent = optional(params, "headers", Value::as_object)?.ok_or_else(invalid_params)?;
    let headers: HashMap<String, String> = sent
        .iter()
        .map(|(name, value)| Ok((name.clone(), header_value(value)?)))
        .collect::<Result<_, Failure>>()?;
    for (name, value) in &headers {
        check_header(name, value)?;
    }
    page.inner.set_extra_headers(headers).await;
    Ok(json!({}))
}

/// A header value is a string, whatever its name — and is read first.
fn header_value(value: &Value) -> Result<String, Failure> {
    value
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| invalid("Invalid header value, string expected"))
}

/// A header Chrome will carry: the name an ASCII token (RFC 9110) — any
/// other ASCII byte is a bad name, a byte past ASCII is a parameter it
/// will not read — and the value free of CR, LF and NUL.
fn check_header(name: &str, value: &str) -> Result<(), Failure> {
    if !name.is_ascii() {
        return Err(invalid("Invalid parameters"));
    }
    let token = |byte: u8| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte);
    if name.is_empty() || !name.bytes().all(token) {
        return Err(invalid("Invalid header name"));
    }
    if value.contains(['\r', '\n', '\0']) {
        return Err(invalid("Invalid header value"));
    }
    Ok(())
}

fn layout_metrics(page: &mut Page) -> Result<Value, Failure> {
    let metrics = page
        .evaluate_with_timeout_result(LAYOUT_METRICS, INTERACTION_EVAL_TIMEOUT)
        .map_err(|err| Failure::from(format!("Page.getLayoutMetrics: {err}")))?;
    let f = |name: &str| metrics.get(name).and_then(Value::as_f64).unwrap_or(0.0);
    let (iw, ih) = (f("iw"), f("ih"));
    let (sx, sy) = (f("sx"), f("sy"));
    let (vw, vh) = (f("vw"), f("vh"));
    let (px, py) = (sx + f("ox"), sy + f("oy"));
    let layout = json!({
        "pageX": measure(sx), "pageY": measure(sy),
        "clientWidth": measure(iw), "clientHeight": measure(ih),
    });
    let visual = json!({
        "offsetX": measure(f("ox")), "offsetY": measure(f("oy")),
        "pageX": measure(px), "pageY": measure(py),
        "clientWidth": measure(vw), "clientHeight": measure(vh),
        "scale": measure(f("scale")), "zoom": 1,
    });
    let content = json!({ "x": 0, "y": 0, "width": measure(f("cw")), "height": measure(f("ch")) });
    Ok(json!({
        "layoutViewport": layout,
        "visualViewport": visual,
        "contentSize": content,
        "cssLayoutViewport": layout,
        "cssVisualViewport": visual,
        "cssContentSize": content,
    }))
}

/// A measurement off the page, spelled as Chrome's inspector spells a
/// number: a whole one without a fraction (`"clientWidth":390`), the rest
/// with it.
fn measure(value: f64) -> Value {
    match value {
        whole if whole.fract() == 0.0 && whole.abs() < 9_007_199_254_740_992.0 => {
            json!(whole as i64)
        }
        other => json!(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The locales Chrome 158 took — every row of three tables asked of it.
    const TAKEN: &[&str] = &[
        "abc",
        "eng",
        "yue",
        "en-abcde",
        "en-POSIX",
        "en-Posix",
        "en-abcdf",
        "en-1234",
        "en-1abc",
        "en-12ab",
        "en-abcdefgh",
        "en-12345678",
        "en-1234567",
        "en-a-foo",
        "en-x-foo",
        "en-u-ca-gregory",
        "en-abcde-u-ca-gregory",
        "en-a-ca-gregory",
        "en-123",
        "en-ABCD",
        "en-latn",
        "en-LATN",
        "zh-Hans-CN",
        "zh-Hant",
        "art-lojban",
        "sl-rozaj-biske",
        "cmn-Hans-CN",
        "en-GB-oxendict",
        "cel-gaulish",
        "en_US",
        "en_US.UTF-8",
        "en_US@currency=EUR",
        "de_DE@currency=EUR",
        "ja_JP@calendar=japanese",
        "zh_Hans_CN",
        "de_CH_1996",
        "en.",
        "en.utf8",
        "en.utf-8",
        "en-US.",
        "en-US.UTF-8",
        "en@",
        "en_US_POSIX",
        "EN-us",
        "ZH-hans",
        "en-uS",
        "EN-US",
        "en-LATN-us",
        "en@foo=bar",
        "en@ca=japanese",
        "en@calendar=japanese",
        "en@calendar=bogus",
        "en@calendar",
        "en@calendar=japanese;collation=phonebook",
        "de_DE@collation=phonebook",
        "de_DE@collation=bogus",
        "de_DE@numbers=arab",
        "en_US.UTF-8@currency=EUR",
        "en_US.UTF-8@calendar=japanese",
        "en_US@calendar=japanese",
        "en@calendar=japanese;x=y",
        "en@CALNDAR=JAPANESE",
        "de_DE@colLation=phonebook",
        "en@colstrength=identic",
        "en@collation=phonebook;colstrength=identic",
        "de_CH.1901",
        "zh_CN",
        "en_419",
        "es_419",
        "en_latn_us",
        "en_US.UTF8",
        "en_US.utf8",
        "en_US.UTF-8.UTF-8",
        "en_US.1996",
        "en_US.1234",
        "en_US.abcde",
        "en_US.oed",
        "en-x-foo-bar",
        "en-x-a",
        "en-u-ca",
        "en-1abc-12ab",
        "en-a-foo-u-ca-gregory",
        "en-u-ca-gregory-a-foo",
        "en-x-foo-u-ca-gregory",
        "en-US-x-foo-Latn",
        "en-a-foo-abcde",
        "en-US-a-foo-abcde",
        "en-US-x-a-b-c",
        "en-US-u-ca-gregory-nu-latn",
        "en-US-u-nu-latn-ca-gregory",
        "en-US-u-foo",
        "en-US-u-ca",
        "en-US-a-ca",
        "en-US-a-abc",
        "en-US-a-12345678",
        "en-US-12345678",
        "en-abcd-abcde",
        "en-1abc-POSIX",
        "EN-us-x-PRIVATE",
        "en-us-x-Abc-DEF",
        "en-u-ca-gregory-x-foo",
        "en-x-foo-a-foo",
        "en-a-foo-b-bar",
        "en@CALENDAR=JAPANESE",
        "en@CALENDAR",
        "en@Calendar",
        "en@valencia",
        "en@calendar=japanese,collation=phonebook",
        "en@calendar=japanese,",
        "en@,calendar=japanese",
        "en_US@calendar=gregorian",
        "en_US@calendar=ethiopic-amete-alem",
        "en_US@calendar=iso8601",
        "en_US@calendar=buddhist",
        "de_DE@collation=dictionary",
        "de_DE@collation=traditional",
        "de_DE@collation=gb2312han",
        "de_DE@collation=phonebk",
        "de_DE@collation=dict",
        "de_DE@collation=trad",
        "de_DE@colnumeric=yes",
        "de_DE@colnumeric=true",
        "de_DE@colnumeric=no",
        "de_DE@colbackwards=yes",
        "de_DE@colnormalization=yes",
        "en_US@numbers=traditional",
        "en_US@measure=imperial",
        "de_DE@colstrength=primary",
        "de_DE@colalternate=non-ignorable",
        "de_DE@colcasefirst=no",
        "de_DE@colcasefirst=yes",
        "de_DE@colcaselevel=no",
        "de_DE@colhiraganaquaternary=yes",
        "de_DE@colreorder=currency",
        "de_DE@variabletop=at",
        "en_US@timezone=cst6cdt",
        "en_US@timezone=usnyc",
        "en_US@timezone=EST5EDT",
        "en_US@hours=h23",
        "en_US@hours=H23",
        "en_US@hours=h11",
        "sh",
        "SH",
        "sh-RS",
        "en-x-foo@calendar=japanese",
        "und",
        "UND",
        "zh",
        "zh-CN",
        "zh-Hans",
        "zh-TW",
        "zh-Hant-TW",
        "fr",
        "fr-CA",
        "de",
        "ja",
        "ko",
        "ru",
        "ar",
        "th",
        "vi",
        "tr",
        "pl",
        "pt-BR",
        "hi",
        "bn",
        "ta",
        "es-MX",
        "it",
        "nl",
        "sv",
        "da",
        "fi",
        "nb",
        "he",
        "iw",
        "in",
        "id",
        "tl",
        "fil",
    ];

    /// The ones it refused with "Invalid locale name".
    const REFUSED: &[&str] = &[
        "abcde",
        "ABCDE",
        "Abcde",
        "abcdf",
        "posix",
        "Posix",
        "abcde-Latn",
        "abcde-US",
        "abcdef",
        "abcdefg",
        "abcdefgh",
        "abcdefgi",
        "ABCDEF",
        "abcde1",
        "abcd1",
        "ab1de",
        "a1234",
        "abcd",
        "Latn",
        "abcd-US",
        "en-abcde-abcde",
        "en-abc1",
        "en-a",
        "en-x",
        "en-a-b",
        "en-12",
        "en-1A",
        "en-A1",
        "en-Latn-Latn",
        "en-Latn-US-latn",
        "zh-cmn-Hans-CN",
        "en-GB-oed",
        "sgn-BE-FR",
        "zh-min",
        "zh-cmn",
        "i-default",
        "i-navajo",
        "no-bok",
        "no-nyn",
        "en_US.UTF-8@x=y",
        "en_.",
        "en__US",
        "_en",
        ".",
        "en_US_.",
        "e.",
        "a_US",
        "root.en",
        "C.UTF-8",
        "POSIX.utf8",
        "en@.",
        "en@@x",
        "en-US ",
        "en US",
        "en‑US",
        "ｅｎ",
        "en-US\u{0}",
        "en@x=y",
        "en_US@x=y",
        "en@calendar=",
        "en@=japanese",
        "de_DE@hours=H",
        "de_DE@hourcycle=h23",
        "de_DE@timezone=Asia/Tokyo",
        "en@x=y;calendar=japanese",
        "en@;",
        "en@;;",
        "en-US-u-nu-latn@calendar=japanese",
        "en-US-UTF8",
        "en-utf8",
        "en-UTF-8",
        "en-POSIX-utf8",
        "en-a-b-foo",
        "en-u",
        "en-US-POSIX-POSIX",
        "en-1996-1996",
        "en-abcde-Latn",
        "en-US-x-123456789",
        "en-US-x-",
        "en-US-123456789",
        "en-US-12345678-x",
        "en-a-b-bar",
        "en-t-foo-t-bar",
        "en-a-foo-a-bar",
        "en_US@timezone=America/New_York",
        "x-private",
        "x-foo",
        "X-foo",
    ];

    #[test]
    fn a_locale_is_taken_as_chrome_takes_one() {
        for locale in TAKEN {
            assert!(locale_tag(locale).is_some(), "Chrome takes {locale:?}");
        }
        for locale in REFUSED {
            assert_eq!(locale_tag(locale), None, "Chrome refuses {locale:?}");
        }
    }

    /// The old spellings turned into BCP 47 by hand — what goes in, what
    /// Chromium's converter writes out, before any parser reads it. Every
    /// input is a row Chrome was asked.
    #[test]
    fn an_old_spelling_becomes_bcp47() {
        for (sent, written) in [
            ("en_US", "en-US"),
            ("zh_Hans_CN", "zh-Hans-CN"),
            ("de_CH_1996", "de-CH-1996"),
            ("en_US_POSIX", "en-US-POSIX"),
            ("en.", "en"),
            ("en.utf8", "en"),
            ("en-US.UTF-8", "en-US"),
            ("en_US.UTF-8.UTF-8", "en-US"),
            ("en@", "en"),
            ("en@;", "en-u"),
            ("en@;;", "en-u"),
            ("en@.", "en-u-."),
            ("en@@x", "en-u-@x"),
            ("en@calendar", "en-u-ca"),
            ("en@CALENDAR", "en-u-CALENDAR"),
            ("en@ca=japanese", "en-u-ca-japanese"),
            ("en@CALENDAR=JAPANESE", "en-u-ca-JAPANESE"),
            ("en@CALNDAR=JAPANESE", "en-u-CALNDAR-JAPANESE"),
            ("en@calendar=", "en-u-ca-"),
            ("en@=japanese", "en-u--japanese"),
            ("en@valencia", "en-u-va-valencia"),
            ("en@x=y", "en-u-x-y"),
            ("en@foo=bar", "en-u-foo-bar"),
            ("en_US@currency=EUR", "en-US-u-cu-EUR"),
            ("en_US.UTF-8@x=y", "en-US-u-x-y"),
            ("ja_JP@calendar=japanese", "ja-JP-u-ca-japanese"),
            (
                "en@calendar=japanese;collation=phonebook",
                "en-u-ca-japanese-co-phonebk",
            ),
            ("en@calendar=japanese;x=y", "en-u-ca-japanese-x-y"),
            (
                "en@calendar=japanese,collation=phonebook",
                "en-u-ca-japanese-co-phonebk",
            ),
            ("en@calendar=japanese,", "en-u-ca-japanese"),
            ("en@,calendar=japanese", "en-u-ca-japanese"),
            ("en@x=y;calendar=japanese", "en-u-x-y-ca-japanese"),
            ("de_DE@colLation=phonebook", "de-DE-u-co-phonebk"),
            (
                "en-US-u-nu-latn@calendar=japanese",
                "en-US-u-nu-latn-u-ca-japanese",
            ),
            ("en-x-foo@calendar=japanese", "en-x-foo-u-ca-japanese"),
            // values that changed name on the way
            ("en_US@calendar=gregorian", "en-US-u-ca-gregory"),
            ("en_US@calendar=ethiopic-amete-alem", "en-US-u-ca-ethioaa"),
            ("de_DE@collation=phonebook", "de-DE-u-co-phonebk"),
            ("de_DE@colnumeric=yes", "de-DE-u-kn-true"),
            ("de_DE@colnumeric=true", "de-DE-u-kn-true"),
            ("de_DE@colnumeric=no", "de-DE-u-kn-false"),
            ("de_DE@colcasefirst=no", "de-DE-u-kf-false"),
            ("de_DE@colstrength=primary", "de-DE-u-ks-level1"),
            ("en_US@measure=imperial", "en-US-u-ms-uksystem"),
            ("en_US@numbers=traditional", "en-US-u-nu-traditio"),
            ("en_US@timezone=cst6cdt", "en-US-u-tz-uschi"),
            (
                "en_US@timezone=America/New_York",
                "en-US-u-tz-America/New_York",
            ),
            ("en_US@hours=H23", "en-US-u-hc-H23"),
        ] {
            assert_eq!(legacy_to_bcp47(sent).as_deref(), Some(written), "{sent:?}");
            assert!(
                TAKEN.contains(&sent) || REFUSED.contains(&sent),
                "{sent:?} was never asked of Chrome"
            );
        }
        for plain in [
            "",
            "en",
            "en-US",
            "EN-us",
            "en-Latn-US",
            "en-u-ca-gregory",
            "e",
            "en-US ",
        ] {
            assert_eq!(
                legacy_to_bcp47(plain),
                None,
                "{plain:?} has nothing old in it"
            );
        }
    }

    /// What is pinned: the tag as the parser writes it back — case and
    /// keyword order normalized, a `true` dropped as its type's default,
    /// the subtags as written (`cmn` stays where Chrome's canonicalizer,
    /// which this bridge does not run, would make it `zh`).
    #[test]
    fn the_pin_is_the_tag_as_parsed() {
        for (sent, tag) in [
            ("en", "en"),
            ("EN-us", "en-US"),
            ("en_LATN_us", "en-Latn-US"),
            ("en-1abc-12ab", "en-12ab-1abc"),
            ("en-US-u-nu-latn-ca-gregory", "en-US-u-ca-gregory-nu-latn"),
            ("cmn-Hans-CN", "cmn-Hans-CN"),
            ("en@", "en"),
            ("en@CALENDAR=JAPANESE", "en-u-ca-japanese"),
            ("en@CALENDAR", "en-u-calendar"),
            ("en_US@currency=EUR", "en-US-u-cu-eur"),
            ("ja_JP@calendar=japanese", "ja-JP-u-ca-japanese"),
            ("en_US@calendar=ethiopic-amete-alem", "en-US-u-ca-ethioaa"),
            ("de_DE@collation=phonebook", "de-DE-u-co-phonebk"),
            ("de_DE@colnumeric=yes", "de-DE-u-kn"),
            ("de_DE@colnumeric=no", "de-DE-u-kn-false"),
        ] {
            assert_eq!(locale_tag(sent).as_deref(), Some(tag), "{sent:?}");
        }
        assert_eq!(locale_tag(""), None);
    }
}
