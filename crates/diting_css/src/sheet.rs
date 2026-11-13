//! Stylesheet parse: rules, at-rules, and `@container`.
//!
//! Split from the css hub. The public parse entry points and container
//! types stay re-exported from the crate root. Pure move.

use super::*;

/// `container-type` — marks an element as a queryable container ancestor
/// (css-conditional-5). v1 is a marker only: no size containment behavior
/// is modeled, the property just feeds `@container` target lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContainerType {
    #[default]
    Normal,
    Size,
    InlineSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerAxis {
    Width,
    Height,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ContainerCmp {
    Ge,
    Le,
    Gt,
    Lt,
    Eq,
}

/// One size feature of a `@container` condition, value pre-resolved to px
/// (same px-only limitation as `@media` features here).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContainerFeature {
    pub axis: ContainerAxis,
    pub cmp: ContainerCmp,
    pub px: f32,
}

/// AND-combination of size features. An absent condition (bare
/// `@container name {}`) matches any qualifying container.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ContainerCondition(pub Vec<ContainerFeature>);

impl ContainerCondition {
    pub fn matches(&self, width: f32, height: f32) -> bool {
        self.0.iter().all(|f| {
            let v = match f.axis {
                ContainerAxis::Width => width,
                ContainerAxis::Height => height,
            };
            match f.cmp {
                ContainerCmp::Ge => v >= f.px,
                ContainerCmp::Le => v <= f.px,
                ContainerCmp::Gt => v > f.px,
                ContainerCmp::Lt => v < f.px,
                ContainerCmp::Eq => (v - f.px).abs() < f32::EPSILON,
            }
        })
    }
}

/// One `@container` rule kept unflattened: the condition answers against the
/// element's nearest qualifying container ancestor, which only exists after
/// a layout pass — so unlike `@media` this cannot resolve at parse time
/// (moli#282: conditions that parse but never match).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ContainerRule {
    pub name: Option<String>,
    pub condition: Option<ContainerCondition>,
    pub rules: Vec<ParsedRule>,
}

/// Parse a stylesheet into flattened rules. Handles nested braces, comments,
/// and the at-rules whose bodies contain ordinary rules (`@media`,
/// `@supports`, `@layer`). Other at-rules (`@font-face`, `@import`, ...)
/// are dropped: they contribute no layout-relevant rule here. `@keyframes`
/// blocks are dropped as rules but their stops land in the animation table
/// when parsed via [`parse_stylesheet_timed`].
///
/// Error recovery mirrors browsers (and upstream): an unbalanced stray `}`
/// at top level resynchronizes instead of scrambling every following rule.
pub fn parse_stylesheet(css: &str) -> Vec<ParsedRule> {
    parse_stylesheet_for(css, (1280.0, 720.0), CssMediaType::Screen)
}

pub fn parse_stylesheet_for(
    css: &str,
    viewport: (f32, f32),
    media_type: CssMediaType,
) -> Vec<ParsedRule> {
    parse_stylesheet_timed(css, viewport, media_type).0
}

/// Full stylesheet parse: flattened rules PLUS the `@keyframes` table the
/// animation sampler samples from. Rules-only callers keep
/// [`parse_stylesheet_for`]; this is the entry the layout run uses so a
/// declarative SVG's `<style>` keyframes reach style resolution.
pub fn parse_stylesheet_timed(
    css: &str,
    viewport: (f32, f32),
    media_type: CssMediaType,
) -> (Vec<ParsedRule>, KeyframesMap) {
    parse_stylesheet_timed_with(css, viewport, media_type, &MediaOverrides::default())
}

/// [`parse_stylesheet_timed`] with emulated media in play: the layout run
/// passes the page's `Emulation.setEmulatedMedia` state so `@media
/// (prefers-*)` arms follow what `matchMedia` already answers (#29).
pub fn parse_stylesheet_timed_with(
    css: &str,
    viewport: (f32, f32),
    media_type: CssMediaType,
    overrides: &MediaOverrides,
) -> (Vec<ParsedRule>, KeyframesMap) {
    props::reset_for_parse();
    layers::reset();
    parse_stylesheet_with_containers(css, viewport, media_type, overrides, &mut Vec::new())
}

/// [`parse_stylesheet_timed_with`] plus the `@container` table. Container
/// rules never flatten into the ordinary rule list — their conditions need
/// per-element layout answers — so callers that can evaluate them (the
/// layout run) take this entry; everyone else keeps dropping them.
pub fn parse_stylesheet_full(
    css: &str,
    viewport: (f32, f32),
    media_type: CssMediaType,
    overrides: &MediaOverrides,
) -> (Vec<ParsedRule>, KeyframesMap, Vec<ContainerRule>) {
    let mut containers = Vec::new();
    props::reset_for_parse();
    layers::reset();
    let (rules, keyframes) =
        parse_stylesheet_with_containers(css, viewport, media_type, overrides, &mut containers);
    (rules, keyframes, containers)
}

fn parse_stylesheet_with_containers(
    css: &str,
    viewport: (f32, f32),
    media_type: CssMediaType,
    overrides: &MediaOverrides,
    containers: &mut Vec<ContainerRule>,
) -> (Vec<ParsedRule>, KeyframesMap) {
    let mut rules = Vec::new();
    let mut keyframes = KeyframesMap::new();
    let mut current_selector = String::new();
    let mut current_decls = String::new();
    let mut block_depth = 0usize;
    let mut in_comment = false;
    let mut chars = css.chars().peekable();

    while let Some(c) = chars.next() {
        if in_comment {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_comment = false;
            }
            continue;
        }
        if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            in_comment = true;
            continue;
        }

        if c == '{' {
            if block_depth != 0 {
                current_decls.push(c);
            }
            block_depth += 1;
        } else if c == '}' && block_depth == 0 {
            // Stray top-level close brace: error-recover and keep parsing.
            current_selector.clear();
        } else if c == '}' {
            block_depth -= 1;
            if block_depth == 0 {
                let sel = current_selector.trim();
                let decls = current_decls.trim();
                if let Some(at) = sel.strip_prefix('@') {
                    flush_at_rule(
                        at,
                        decls,
                        &mut rules,
                        &mut keyframes,
                        viewport,
                        media_type,
                        overrides,
                        containers,
                    );
                } else if let Some(flat) = decls
                    .contains('{')
                    .then(|| nesting::flatten(sel, decls))
                    .flatten()
                {
                    let (r, k) = parse_stylesheet_with_containers(
                        &flat, viewport, media_type, overrides, containers,
                    );
                    rules.extend(r);
                    keyframes.extend(k);
                } else {
                    rules.push(ParsedRule {
                        selector: sel.to_string(),
                        declarations: decls.to_string(),
                        layer: layers::current(),
                    });
                }
                current_selector.clear();
                current_decls.clear();
            } else {
                current_decls.push(c);
            }
        } else if c == ';' && block_depth == 0 {
            // Statement at-rules (`@layer a;`) establish ordering slots but
            // emit no rule; drop them so the prelude cannot bleed into the
            // next selector.
            if let Some(p) = current_selector.trim().strip_prefix("@layer") {
                layers::declare_list(p);
            }
            current_selector.clear();
        } else if block_depth > 0 {
            current_decls.push(c);
        } else {
            current_selector.push(c);
        }
    }
    (rules, keyframes)
}

/// Handle the at-rules whose bodies contain ordinary rules. `@media` applies
/// inner rules when the query holds; `@supports` when the condition evaluates
/// true; `@layer` recurses with the named layer tracked (ordering only — we
/// have no layer-priority cascade yet, so layers flatten). `@keyframes`
/// parses its stops into the animation table.
#[allow(clippy::too_many_arguments)]
fn flush_at_rule(
    at: &str,
    inner: &str,
    rules: &mut Vec<ParsedRule>,
    keyframes: &mut KeyframesMap,
    viewport: (f32, f32),
    media_type: CssMediaType,
    overrides: &MediaOverrides,
    containers: &mut Vec<ContainerRule>,
) {
    let recurse = |rules: &mut Vec<ParsedRule>,
                   keyframes: &mut KeyframesMap,
                   containers: &mut Vec<ContainerRule>| {
        // Nested @container inside @media/@supports/@layer must surface in
        // the caller's container table, not a throwaway vec (the failing
        // branch simply never recurses — drop semantics hold there).
        let (inner_rules, inner_kf) =
            parse_stylesheet_with_containers(inner, viewport, media_type, overrides, containers);
        rules.extend(inner_rules);
        for (k, v) in inner_kf {
            keyframes.entry(k).or_insert(v);
        }
    };
    if let Some(prelude) = at_rule_prelude(at, "media") {
        if media_query_applies_with(prelude, viewport, media_type, overrides) {
            recurse(rules, keyframes, containers);
        }
    } else if let Some(prelude) = at_rule_prelude(at, "supports") {
        if supports_condition_applies(prelude) {
            recurse(rules, keyframes, containers);
        }
    } else if let Some(prelude) = at_rule_prelude(at, "layer") {
        layers::enter(prelude);
        recurse(rules, keyframes, containers);
        layers::leave();
    } else if let Some(prelude) = at_rule_prelude(at, "scope") {
        if let Some((root, limit)) = scope::parse_prelude(prelude) {
            let (mut inner_rules, inner_kf) = parse_stylesheet_with_containers(
                inner, viewport, media_type, overrides, containers,
            );
            for r in &mut inner_rules {
                r.selector = scope::rewrite(&r.selector, &root, limit.as_deref());
            }
            rules.extend(inner_rules);
            for (k, v) in inner_kf {
                keyframes.entry(k).or_insert(v);
            }
        }
    } else if let Some(prelude) = at_rule_prelude(at, "keyframes") {
        // `from`/`to`/`NN%` stops; a stop that fails to parse is skipped,
        // not fatal — the sampler works with however many stops survive.
        if let Some(kf) = parse_keyframes_body(inner) {
            let name = prelude.trim();
            if !name.is_empty() {
                keyframes.insert(name.to_string(), kf);
            }
        }
    } else if let Some(prelude) = at_rule_prelude(at, "property") {
        props::define_from_rule(prelude, inner);
    } else if let Some(prelude) = at_rule_prelude(at, "container") {
        parse_container_rule(prelude, inner, viewport, media_type, overrides, containers);
    }
    // Other at-rules carry no layout-relevant rules for us; drop them.
}

/// Parse one `@container` prelude + body into the container table. The body
/// recurses through the ordinary parser: plain rules land on this rule,
/// nested `@container` rules AND-compose their conditions with ours (v1
/// approximation of nearest-container semantics for the nested spelling),
/// and `@media` inside the body resolves at parse time as usual. A prelude
/// whose condition fails to parse (style queries, range triples) drops the
/// whole rule — the conservative "never matches", not "always matches".
fn parse_container_rule(
    prelude: &str,
    inner: &str,
    viewport: (f32, f32),
    media_type: CssMediaType,
    overrides: &MediaOverrides,
    containers: &mut Vec<ContainerRule>,
) {
    let prelude = prelude.trim();
    let (name, cond_text) = split_container_prelude(prelude);
    if let Some(text) = cond_text {
        if parse_container_condition(text).is_none() {
            return;
        }
    }
    let mut nested = Vec::new();
    // v1: @keyframes inside a container body is dropped (rare in practice;
    // the plain rules are what the gated cascade needs).
    let (plain, _) =
        parse_stylesheet_with_containers(inner, viewport, media_type, overrides, &mut nested);
    let own = ContainerRule {
        name: name.clone(),
        condition: cond_text.and_then(parse_container_condition),
        rules: plain,
    };
    for mut n in nested {
        n.name = n.name.take().or_else(|| name.clone());
        n.condition = merge_conditions(own.condition.as_ref(), n.condition.take());
        containers.push(n);
    }
    // A container whose body holds ONLY nested @container rules contributes
    // nothing of its own — pushing the empty shell would sit in the plan as
    // a no-op entry.
    if !own.rules.is_empty() {
        containers.push(own);
    }
}

fn merge_conditions(
    outer: Option<&ContainerCondition>,
    inner: Option<ContainerCondition>,
) -> Option<ContainerCondition> {
    match (outer, inner) {
        (None, i) => i,
        (Some(o), None) => Some(o.clone()),
        (Some(o), Some(mut i)) => {
            i.0.extend_from_slice(&o.0);
            Some(i)
        }
    }
}

/// `@container` prelude = `[<name>]? <condition>?`. A leading `(` means no
/// name; otherwise the first identifier is the name and the rest (if any)
/// is the condition.
fn split_container_prelude(prelude: &str) -> (Option<String>, Option<&str>) {
    let p = prelude.trim();
    if p.is_empty() {
        return (None, None);
    }
    if p.starts_with('(') {
        return (None, Some(p));
    }
    match p.find(char::is_whitespace) {
        Some(i) => {
            let name = &p[..i];
            let rest = p[i..].trim();
            let ident = !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || c == '_');
            if ident && (rest.is_empty() || rest.starts_with('(')) {
                (
                    Some(name.to_string()),
                    if rest.is_empty() { None } else { Some(rest) },
                )
            } else {
                (None, Some(p))
            }
        }
        None => (Some(p.to_string()), None),
    }
}

/// AND-split a condition on top-level `and` (paren-depth aware), then parse
/// each feature. OR-spelling and style queries parse-fail → the rule drops.
pub(super) fn parse_container_condition(text: &str) -> Option<ContainerCondition> {
    let mut feats = Vec::new();
    for part in split_top_level_and(text) {
        feats.push(parse_container_feature(part)?);
    }
    if feats.is_empty() {
        return None;
    }
    Some(ContainerCondition(feats))
}

fn split_top_level_and(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => {
                let rest = &text[i..];
                if rest.len() >= 3 {
                    let (word, tail) = rest.split_at(3);
                    if word.eq_ignore_ascii_case("and")
                        && tail.starts_with(|c: char| c.is_ascii_whitespace())
                        && text[..i].ends_with(|c: char| c.is_ascii_whitespace())
                    {
                        parts.push(&text[start..i]);
                        i += 3;
                        start = i;
                        continue;
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    parts.push(text[start..].trim());
    parts.into_iter().filter(|p| !p.trim().is_empty()).collect()
}

fn parse_container_feature(f: &str) -> Option<ContainerFeature> {
    let f = f.trim();
    let f = if f.starts_with('(') && f.ends_with(')') && f.len() > 1 {
        f[1..f.len() - 1].trim()
    } else {
        f
    };
    if let Some((name, value)) = f.split_once(':') {
        let px = parse_px(value.trim())?;
        let feat = |axis, cmp| Some(ContainerFeature { axis, cmp, px });
        return match name.trim() {
            "min-width" | "min-inline-size" => feat(ContainerAxis::Width, ContainerCmp::Ge),
            "max-width" | "max-inline-size" => feat(ContainerAxis::Width, ContainerCmp::Le),
            "width" | "inline-size" => feat(ContainerAxis::Width, ContainerCmp::Eq),
            "min-height" | "min-block-size" => feat(ContainerAxis::Height, ContainerCmp::Ge),
            "max-height" | "max-block-size" => feat(ContainerAxis::Height, ContainerCmp::Le),
            "height" | "block-size" => feat(ContainerAxis::Height, ContainerCmp::Eq),
            _ => None,
        };
    }
    // Range syntax: (width >= 300px) or (300px <= width). Multi-value
    // triples (`300px <= width <= 500px`) fail the single-split parse.
    for (sym, cmp) in [
        (">=", ContainerCmp::Ge),
        ("<=", ContainerCmp::Le),
        (">", ContainerCmp::Gt),
        ("<", ContainerCmp::Lt),
        ("=", ContainerCmp::Eq),
    ] {
        if let Some((l, r)) = f.split_once(sym) {
            let (l, r) = (l.trim(), r.trim());
            if let Some(axis) = container_axis_of(l) {
                return Some(ContainerFeature {
                    axis,
                    cmp,
                    px: parse_px(r)?,
                });
            }
            if let Some(axis) = container_axis_of(r) {
                let flipped = match cmp {
                    ContainerCmp::Ge => ContainerCmp::Le,
                    ContainerCmp::Le => ContainerCmp::Ge,
                    ContainerCmp::Gt => ContainerCmp::Lt,
                    ContainerCmp::Lt => ContainerCmp::Gt,
                    ContainerCmp::Eq => ContainerCmp::Eq,
                };
                return Some(ContainerFeature {
                    axis,
                    cmp: flipped,
                    px: parse_px(l)?,
                });
            }
        }
    }
    None
}

fn container_axis_of(s: &str) -> Option<ContainerAxis> {
    match s.trim() {
        "width" | "inline-size" => Some(ContainerAxis::Width),
        "height" | "block-size" => Some(ContainerAxis::Height),
        _ => None,
    }
}

fn at_rule_prelude<'a>(at: &'a str, name: &str) -> Option<&'a str> {
    let rest = at.strip_prefix(name)?;
    if !rest.is_empty() && !rest.starts_with(|c: char| c.is_ascii_whitespace() || c == '(') {
        return None;
    }
    let prelude = rest.trim();
    Some(prelude.strip_suffix('{').unwrap_or(prelude).trim())
}
