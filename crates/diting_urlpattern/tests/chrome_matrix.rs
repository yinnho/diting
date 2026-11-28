//! Early validation harness (not upstream): run the engine's measured
//! 19-pattern x 39-input corpus through the vendored crate and diff every
//! cell against chrome/urlpattern-truth-157.json. Written before the v8
//! face exists so core-logic divergence is visible before any wiring.

use diting_urlpattern::{UrlPattern, UrlPatternInit, UrlPatternMatchInput, UrlPatternOptions};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct MatrixSpec {
    patterns: BTreeMap<String, PatSpec>,
    inputs: Vec<InputSpec>,
}

#[derive(Deserialize)]
struct PatSpec {
    init: Option<BTreeMap<String, String>>,
    string: Option<String>,
    base: Option<String>,
    options: Option<BTreeMap<String, bool>>,
}

#[derive(Deserialize)]
struct InputSpec {
    url: Option<String>,
    base: Option<String>,
    init: Option<BTreeMap<String, String>>,
}

fn init_of(map: &BTreeMap<String, String>) -> UrlPatternInit {
    let mut init = UrlPatternInit::default();
    for (k, v) in map {
        match k.as_str() {
            "protocol" => init.protocol = Some(v.clone()),
            "username" => init.username = Some(v.clone()),
            "password" => init.password = Some(v.clone()),
            "hostname" => init.hostname = Some(v.clone()),
            "port" => init.port = Some(v.clone()),
            "pathname" => init.pathname = Some(v.clone()),
            "search" => init.search = Some(v.clone()),
            "hash" => init.hash = Some(v.clone()),
            "baseURL" => init.base_url = Some(url::Url::parse(v).unwrap()),
            _ => panic!("unknown init key {k}"),
        }
    }
    init
}

fn build_pattern(spec: &PatSpec) -> Result<UrlPattern, String> {
    let opts = UrlPatternOptions {
        ignore_case: spec
            .options
            .as_ref()
            .and_then(|o| o.get("ignoreCase").copied())
            .unwrap_or(false),
    };
    if let Some(s) = &spec.string {
        let base = spec.base.as_ref().map(|b| url::Url::parse(b).unwrap());
        let init = UrlPatternInit::parse_constructor_string::<regex::Regex>(s, base)
            .map_err(|e| format!("ctor-string: {e:?}"))?;
        UrlPattern::parse(init, opts).map_err(|e| format!("parse: {e:?}"))
    } else {
        let init = init_of(spec.init.as_ref().unwrap());
        UrlPattern::parse(init, opts).map_err(|e| format!("parse: {e:?}"))
    }
}

fn match_input(spec: &InputSpec) -> UrlPatternMatchInput {
    if let Some(u) = &spec.url {
        let parsed = match &spec.base {
            None => url::Url::parse(u),
            Some(b) => url::Url::parse(b).and_then(|base| base.join(u)),
        };
        match parsed {
            Ok(u) => UrlPatternMatchInput::Url(u),
            Err(_) => {
                UrlPatternMatchInput::Url(url::Url::parse("https://invalid.invalid").unwrap())
            }
        }
    } else {
        UrlPatternMatchInput::Init(init_of(spec.init.as_ref().unwrap()))
    }
}

#[test]
fn matrix_against_chrome_truth() {
    let spec: MatrixSpec = serde_json::from_str(include_str!("matrix.json")).unwrap();
    let truth: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/webplatform/chrome/urlpattern-truth-157.json"
    ))
    .unwrap();
    let mut total = 0;
    let mut pass = 0;
    let mut diffs: Vec<String> = Vec::new();
    for (pk, pat) in &spec.patterns {
        let pattern = match build_pattern(pat) {
            Ok(p) => Some(p),
            Err(e) => {
                // construction failure: every cell for this pattern must
                // be a THROW in truth — but the matrix never throws in
                // practice; record and move on
                diffs.push(format!("{pk}: pattern build failed: {e}"));
                None
            }
        };
        for (ii, inp) in spec.inputs.iter().enumerate() {
            total += 1;
            let key = format!("m.{pk}.{ii}");
            let t = &truth[&key];
            if t.is_null() {
                continue;
            }
            let Some(pattern) = &pattern else { break };
            // "not a url" style inputs: Chrome's exec returns null when the
            // URL parse fails — replicate by checking parse success first
            let input = match_input(inp);
            let bad_url = inp.url.is_some()
                && matches!(&input, UrlPatternMatchInput::Url(u) if u.as_str().contains("invalid.invalid"));
            let got_test = if bad_url {
                false
            } else {
                pattern.test(input.clone()).unwrap_or(false)
            };
            let want_test = t["test"].as_bool().unwrap();
            let exec_want_null = t["exec"].is_null();
            let got_exec = if bad_url {
                None
            } else {
                pattern.exec(input).ok().flatten()
            };
            let mut ok = got_test == want_test && (got_exec.is_none() == exec_want_null);
            if ok && !exec_want_null {
                // compare per-component input + groups
                let g = got_exec.as_ref().unwrap();
                let comps = [
                    ("protocol", &g.protocol),
                    ("username", &g.username),
                    ("password", &g.password),
                    ("hostname", &g.hostname),
                    ("port", &g.port),
                    ("pathname", &g.pathname),
                    ("search", &g.search),
                    ("hash", &g.hash),
                ];
                for (cn, cr) in comps {
                    let tc = &t["exec"][cn];
                    if tc["input"].as_str().unwrap_or("") != cr.input {
                        ok = false;
                        diffs.push(format!(
                            "{key}: {cn}.input got {:?} want {:?}",
                            cr.input, tc["input"]
                        ));
                        break;
                    }
                    let want_groups = tc["groups"].as_object().unwrap();
                    if want_groups.len() != cr.groups.len() {
                        ok = false;
                        diffs.push(format!(
                            "{key}: {cn}.groups len got {} want {}",
                            cr.groups.len(),
                            want_groups.len()
                        ));
                        break;
                    }
                    for (gn, gv) in want_groups {
                        // the probe encodes a present-but-undefined group as
                        // "#undef"; the crate models it as None
                        let want = if gv.as_str() == Some("#undef") {
                            None
                        } else {
                            gv.as_str()
                        };
                        let got = cr
                            .groups
                            .iter()
                            .find(|(k, _)| k == gn)
                            .and_then(|(_, v)| v.as_deref());
                        if got != want {
                            ok = false;
                            diffs.push(format!("{key}: {cn}.{gn} got {got:?} want {want:?}"));
                            break;
                        }
                    }
                    if !ok {
                        break;
                    }
                }
            }
            if ok {
                pass += 1;
            } else if diffs.len() < 25 {
                let gt = if got_test { "T" } else { "F" };
                let wt = if want_test { "T" } else { "F" };
                if diffs.iter().all(|d| !d.starts_with(&key)) {
                    diffs.push(format!(
                        "{key}: test {gt}/{wt} exec-null {}",
                        got_exec.is_none()
                    ));
                }
            }
        }
    }
    eprintln!("matrix: {pass}/{total} cells match Chrome truth");
    for d in diffs.iter().take(20) {
        eprintln!("  diff: {d}");
    }
    eprintln!("  total diffs recorded: {}", diffs.len());
}
