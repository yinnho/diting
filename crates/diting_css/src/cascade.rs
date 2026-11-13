//! Per-element cascade: UA defaults, author rules, inline style.
//!
//! Split from the css hub. `cascade_element` stays re-exported from the
//! crate root. Pure move.

use super::*;

// ---------------------------------------------------------------------------
// Cascade
// ---------------------------------------------------------------------------

/// One candidate: a compiled rule plus how strongly it applies.
struct CascadeCandidate<'a> {
    declarations: &'a str,
    specificity: u32,
    source_order: usize,
    layer_rank: u32,
}

/// Compute the style for one element: UA defaults ← author rules (specificity,
/// then source order) ← inline style. Inherited properties (color, font-*,
/// text-align) fall back to the parent's computed values.
///
/// Read-only entry point: callers pass the tree, the element, its matched
/// rules (already filtered by the caller's selector matching), and the parent
/// style. This keeps the module decoupled from any particular matching
/// strategy while still locking ordering semantics.
/// `dir="auto"` directionality (#188-3, HTML §3.2.6): the first character
/// with a strong bidi class decides — an R/AL script (Hebrew, Arabic)
/// means rtl, an L script (Latin/Greek/Cyrillic) means ltr. The candidate
/// text is the control's value for input/textarea (live mirror first, then
/// the parsed default), else the element's text content. Weak characters
/// (digits, punctuation, whitespace) never decide; no strong character at
/// all falls back to ltr.
fn first_strong_direction(
    tree: &diting_dom::tree::DomTree,
    node_id: diting_dom::tree::NodeId,
    tag: &str,
) -> Option<TextDirection> {
    let candidate = if matches!(tag, "input" | "textarea") {
        tree.with_node(node_id, |n| {
            n.live_value()
                .map(|v| v.to_string())
                .or_else(|| n.get_attribute("value").map(|v| v.to_string()))
        })
        .flatten()
        .unwrap_or_default()
    } else {
        tree.text_content(node_id)
    };
    candidate.chars().find_map(strong_dir_of_char)
}

/// The bidi-class coarse half first-strong resolution needs: strong L vs
/// strong R/AL by script block. Everything else (digits, punctuation,
/// CJK — script-dependent under full UAX #9, but never decisive under the
/// HTML rule) reads neutral here.
fn strong_dir_of_char(c: char) -> Option<TextDirection> {
    match c {
        // L: Latin (+extended), Greek, Cyrillic.
        'A'..='Z'
        | 'a'..='z'
        | '\u{00C0}'..='\u{024F}'
        | '\u{0370}'..='\u{03FF}'
        | '\u{0400}'..='\u{04FF}' => Some(TextDirection::Ltr),
        // R/AL: Hebrew, Arabic (+supplement/presentation forms), Syriac.
        '\u{0590}'..='\u{05FF}'
        | '\u{0600}'..='\u{06FF}'
        | '\u{0700}'..='\u{077F}'
        | '\u{FB1D}'..='\u{FB4F}'
        | '\u{FB50}'..='\u{FDFF}'
        | '\u{FE70}'..='\u{FEFF}' => Some(TextDirection::Rtl),
        _ => None,
    }
}

/// Each rule's cascade layer rank as `cascade_element` will read it. The
/// rank table is per-thread parse state, so a memo of cascade output keys
/// on these alongside the rules themselves.
pub fn rule_layer_ranks(rules: &[ParsedRule]) -> Vec<u32> {
    rules.iter().map(|r| layers::rank(r.layer)).collect()
}

#[allow(clippy::too_many_arguments)]
pub fn cascade_element(
    tag: &str,
    tree: &diting_dom::tree::DomTree,
    node_id: diting_dom::tree::NodeId,
    matched_rules: &[(&ParsedRule, u32)],
    parent: Option<&ComputedStyle>,
    inline_css: Option<&str>,
    root_font_size: f32,
    viewport: (f32, f32),
) -> ComputedStyle {
    let mut style = ComputedStyle {
        display: Some(ua_display(tag)),
        display_from_ua: true,
        font_weight: ua_font_weight(tag),
        text_align: ua_text_align(tag),
        list_style_type: ua_list_style(tag),
        ..Default::default()
    };
    if let Some((px, line)) = ua_border(tag) {
        style.border_width.top = Some(Length::Px(px));
        style.border_width.right = Some(Length::Px(px));
        style.border_width.bottom = Some(Length::Px(px));
        style.border_width.left = Some(Length::Px(px));
        style.border_style = Some(line);
        // The UA button border is the gray `buttonborder`, not currentColor
        // (which would paint black); the same value the FormRun shell uses,
        // so a <button> and an <input type=button> read as one control.
        if tag == "button" {
            style.border_color = Some(Color(118, 118, 118, 255));
        }
    }

    // Inherited defaults from parent BEFORE author rules (author overrides).
    if let Some(parent) = parent {
        style.color = parent.color;
        style.color_modern = parent.color_modern;
        // The ancestor-resolved used scheme (#18): a root `color-scheme:dark`
        // flips descendants' light-dark() branches.
        style.scheme_dark_hint = scheme_dark_of(parent);
        style.font_size = parent.font_size;
        // font-family inherits like the spec: the closest ancestor that
        // names a stack wins. `or` is enough because a child without its
        // own declaration carries None here.
        style.font_family = parent.font_family.clone();
        style.font_weight = style.font_weight.or(parent.font_weight);
        // `.or` (not overwrite): the element's own UA declaration beats an
        // inherited value — a th stays centered inside a text-align:right
        // ancestor, matching every browser UA sheet.
        style.text_align = style.text_align.or(parent.text_align);
        // direction inherits the same way (#188-3): an ancestor's rtl
        // flows down until an element declares its own.
        style.direction = style.direction.or(parent.direction);
        // text-align-last inherits (css-text-3); no UA sheet declares it,
        // so a plain `.or` is the whole rule (#214).
        style.text_align_last = style.text_align_last.or(parent.text_align_last);
        // list-style-type inherits, with the element's own UA declaration
        // beating the inherited value — a nested <ol> stays decimal inside
        // a disc <ul>.
        style.list_style_type = style.list_style_type.or(parent.list_style_type);
        // Number keeps its multiplier for descendants (spec computed value);
        // Px inherits as absolute px — both copy straight through.
        style.line_height = parent.line_height;
        // Same inherited posture as text_align above: the element's own UA
        // declaration beats an inherited value, author rules below re-declare.
        style.word_spacing = style.word_spacing.or(parent.word_spacing);
        // white-space inherits; the element's own declaration wins.
        style.white_space = style.white_space.or(parent.white_space);
        // font-variant-caps inherits (small-caps synthesis flows down).
        style.font_variant_caps = style.font_variant_caps.or(parent.font_variant_caps);
        // `quotes` inherits so a pseudo's open-quote picks up an ancestor's
        // declared pairs.
        style.quotes = style.quotes.or(parent.quotes.clone());
        // text-shadow inherits the whole layer list (blitz#271 family) —
        // like color, author rules below re-declare per element.
        style.text_shadow = parent.text_shadow.clone();
        // zoom inherits computed (Chrome semantics, #120): a wrapper's
        // zoom covers every descendant until one declares its own.
        style.zoom = parent.zoom;
        // Custom properties inherit computed (already-substituted-where-
        // possible) values; author rules below may re-declare per element.
        style.custom = props::inherit_custom(&parent.custom);
        // lang inherits computed (the nearest ancestor's attribute, #139);
        // the element's own attribute read below overrides.
        style.lang = parent.lang.clone();
    }
    // #139 (Han unification): the element's own lang/xml:lang attribute
    // wins over the inherited one; neither present keeps the parent's
    // language. Attribute-driven, never declared by author rules.
    if let Some(own) = tree
        .with_node(node_id, |n| {
            n.get_attribute("lang")
                .or_else(|| n.get_attribute("xml:lang"))
                .map(str::to_string)
        })
        .flatten()
    {
        style.lang = Some(own);
    }
    // UA per-tag family/colors AFTER inherited defaults (an element's own
    // UA declaration beats an inherited value — same posture as text_align
    // above) and BEFORE author rules (author declarations override). mark is
    // yellow-on-black in every browser UA sheet; code/kbd/samp/tt are
    // monospace (obscura#936 table).
    if tag == "mark" {
        style.background_color = Some(Color(255, 255, 0, 255));
        style.color = Some(Color(0, 0, 0, 255));
    }
    if let Some(family) = ua_font_family(tag) {
        style.font_family = Some(family.to_string());
    }
    // UA line-height for form controls: `normal`, not inherited (Chromium
    // html.css; blitz#940 family). The form paint centers a control's run
    // as (h − lh)/2 inside its ~22px box, so an inherited body
    // { line-height: 3 } would push the label out of the field. Author
    // declarations below re-declare like every other slot here.
    if matches!(tag, "input" | "button" | "select" | "textarea") {
        style.line_height = Some(LineHeightSpec::Normal);
    }
    // UA decorations ride the same slot. The link rule is `a:-webkit-any-link`
    // upstream — an <a> without href is a named anchor and stays plain.
    let linked_anchor = tag == "a"
        && tree
            .with_node(node_id, |n| n.get_attribute("href").is_some())
            .unwrap_or(false);
    if let Some(d) = ua_text_decoration(tag) {
        if !(tag == "a" && !linked_anchor) {
            style.text_decoration_line = Some(d);
        }
    }
    // UA link color (Chromium html.css `a:-webkit-any-link { color:
    // -webkit-link }` = rgb(0,0,238)). Same href gate as the underline;
    // author declarations below override. :visited purple stays untracked —
    // we keep no per-link visit history, and upstream limits :visited
    // styling to this same property anyway.
    if linked_anchor {
        style.color = Some(Color(0, 0, 238, 255));
    }
    // UA baseline shifts (Chromium html.css: `sub { vertical-align: sub }`,
    // `sup { vertical-align: super }`); font-size: smaller already rides the
    // ua_font_size table. Author declarations below override.
    match tag {
        "sub" => style.vertical_align = Some(VerticalAlign::Sub),
        "sup" => style.vertical_align = Some(VerticalAlign::Super),
        _ => {}
    }
    // UA white-space (the preformatted group). Runs after inheritance, so a
    // plain `<pre>` inherits normal, then the UA default fills None — author
    // declarations still win.
    if let Some(ws) = ua_white_space(tag) {
        style.white_space = style.white_space.or(Some(ws));
    }

    // Author rules: sort by (specificity, source order) ascending, apply in
    // order so later/higher-specificity wins per property.
    let mut candidates: Vec<CascadeCandidate> = matched_rules
        .iter()
        .enumerate()
        .map(|(order, (rule, spec))| CascadeCandidate {
            declarations: &rule.declarations,
            specificity: *spec,
            source_order: order,
            layer_rank: layers::rank(rule.layer),
        })
        .collect();
    candidates.sort_by_key(|c| (c.layer_rank, c.specificity, c.source_order));
    // !important inverts layer order: the earlier layer wins, unlayered loses.
    let mut important_order: Vec<&CascadeCandidate> = candidates.iter().collect();
    important_order.sort_by_key(|c| {
        (
            std::cmp::Reverse(c.layer_rank),
            c.specificity,
            c.source_order,
        )
    });

    // Font-size pre-pass: CSS computes font-size before every other
    // property (regardless of declaration order within a block), because
    // em lengths elsewhere resolve against it. Walk the same winning order
    // (sorted candidates, then inline) and keep the last parseable
    // declaration, then fold em/% against the PARENT size and rem against
    // the root size. The UA heading size (h1 2em …) is the lowest-priority
    // candidate — author declarations win over it.
    let parent_fs = parent
        .and_then(|p| p.font_size)
        .unwrap_or(DEFAULT_ROOT_FONT_SIZE);
    let mut fs_decl: Option<CssLength> = None;
    let mut fs_inherit = false;
    for candidate in &candidates {
        let (d, inherit) = last_font_size_decl(candidate.declarations);
        if d.is_some() || inherit {
            fs_decl = d;
            fs_inherit = inherit;
        }
    }
    if let Some(inline) = inline_css {
        let (d, inherit) = last_font_size_decl(inline);
        if d.is_some() || inherit {
            fs_decl = d;
            fs_inherit = inherit;
        }
    }
    // An author's explicit `font-size: inherit` beats the UA default
    // (blitz#940: controls carry one); it rides a flag because the keyword
    // is not a CssLength the scan can return.
    let own_fs = if fs_inherit {
        parent_fs
    } else {
        fs_decl
            .or_else(|| ua_font_size(tag))
            .map(|d| font_size_px(d, parent_fs, root_font_size, viewport))
            .unwrap_or(parent_fs)
    };
    style.font_size = Some(own_fs);
    let fonts = FontCtx {
        own: own_fs,
        root: root_font_size,
        viewport_w: viewport.0,
        viewport_h: viewport.1,
    };

    // UA box defaults AFTER the fs pre-pass (em margins resolve against the
    // element's OWN font-size — h1's .67em × its 2em size) and BEFORE author
    // rules, so authored margin/padding overrides per side.
    if let Some(m) = ua_margin(tag) {
        let [t, r, b, l] = m.map(|side| resolve_len(side, &fonts));
        style.margin.top = style.margin.top.or(Some(t));
        style.margin.right = style.margin.right.or(Some(r));
        style.margin.bottom = style.margin.bottom.or(Some(b));
        style.margin.left = style.margin.left.or(Some(l));
    }
    if let Some(p) = ua_padding(tag) {
        let [t, r, b, l] = p.map(|side| resolve_len(side, &fonts));
        style.padding.top = style.padding.top.or(Some(t));
        style.padding.right = style.padding.right.or(Some(r));
        style.padding.bottom = style.padding.bottom.or(Some(b));
        style.padding.left = style.padding.left.or(Some(l));
    }

    // Presentational attribute hint (blitz#507): the `height` attribute on
    // td/th/tr is a px height per the HTML spec (bare number). It slots
    // below every author declaration — the author candidates below apply
    // after this and win unconditionally — but fills the slot an author
    // rule never touched. HTML-email bar charts are bare
    // `<td height="55">` columns; without this they collapse to 0.
    if matches!(tag, "td" | "th" | "tr") {
        let attr_h = tree
            .with_node(node_id, |n| {
                n.get_attribute("height").and_then(|v| v.trim().css_f32())
            })
            .flatten();
        if let Some(h) = attr_h {
            style.height = Some(Length::Px(h));
        }
    }
    // The `width` attribute, td/th arm of the same hint family: a px width
    // (bare number) slotted below every author declaration. Fixed table
    // layout and the auto column maxima both read it off the computed
    // style, so a bare `<td width="80">` pins its column with no CSS.
    if matches!(tag, "td" | "th") {
        let attr_w = tree
            .with_node(node_id, |n| {
                n.get_attribute("width").and_then(|v| v.trim().css_f32())
            })
            .flatten();
        if let Some(w) = attr_w {
            style.width = Some(Length::Px(w));
        }
    }
    // valign attribute (blitz#508), same hint slot: fills vertical_align
    // below every author declaration, so `td { vertical-align: top }`
    // outranks valign="middle" like in a real browser. middle (and unknown
    // values) stay None — the alignment site's UA default is middle anyway.
    if matches!(tag, "td" | "th") {
        let attr_va = tree
            .with_node(node_id, |n| {
                n.get_attribute("valign")
                    .map(|v| v.trim().to_ascii_lowercase())
            })
            .flatten()
            .and_then(|v| match v.as_str() {
                "top" => Some(VerticalAlign::Top),
                "bottom" => Some(VerticalAlign::Bottom),
                _ => None,
            });
        if attr_va.is_some() {
            style.vertical_align = attr_va;
        }
    }
    // nowrap attribute (blitz#931), same hint slot: td[nowrap]/th[nowrap]
    // reads as white-space: nowrap — presence-based, matching the UA rule
    // `td[nowrap] { white-space: nowrap }`. Author CSS outranks it like
    // every hint here: the declarations below apply after and win.
    if matches!(tag, "td" | "th") {
        let attr_nw = tree
            .with_node(node_id, |n| n.get_attribute("nowrap").is_some())
            .unwrap_or(false);
        if attr_nw {
            style.white_space = Some(WhiteSpace::Nowrap);
        }
    }
    // dir attribute (#188-3, blitz#981), global-attribute arm of the hint
    // family: the content attribute maps to the direction property (HTML
    // "the dir attribute"), slotting below every author declaration like
    // the hints above — `dir="rtl"` loses to an author `direction: ltr`.
    // `auto` resolves by the HTML first-strong rule: the control's value
    // for input/textarea, else the element's text content.
    if let Some(dir) = tree
        .with_node(node_id, |n| {
            n.get_attribute("dir")
                .map(|v| v.trim().to_ascii_lowercase())
        })
        .flatten()
    {
        let resolved = match dir.as_str() {
            "ltr" => Some(TextDirection::Ltr),
            "rtl" => Some(TextDirection::Rtl),
            "auto" => first_strong_direction(tree, node_id, tag),
            _ => None,
        };
        if resolved.is_some() {
            style.direction = resolved;
        }
    }
    // Author declarations in two passes (CSS 2.1 §6.4.1). Normal pass:
    // stylesheet rules in (specificity, source order) sequence — inline
    // style last, the top of normal. Important pass on top of everything:
    // author !important beats every author-normal declaration INCLUDING
    // inline style, and inline !important beats stylesheet !important
    // (Chrome order). Without the second pass a print override like
    // `transform: none !important` would lose to the inline
    // translateX(...) a carousel script parks on every slide.
    for candidate in &candidates {
        apply_declarations_importance(
            &mut style,
            candidate.declarations,
            &fonts,
            Importance::Normal,
            parent,
        );
    }
    // Inline style (same font context: inline em resolves against the
    // element's own font-size too).
    if let Some(inline) = inline_css {
        apply_declarations_importance(&mut style, inline, &fonts, Importance::Normal, parent);
    }
    for candidate in &important_order {
        apply_declarations_importance(
            &mut style,
            candidate.declarations,
            &fonts,
            Importance::Important,
            parent,
        );
    }
    if let Some(inline) = inline_css {
        apply_declarations_importance(&mut style, inline, &fonts, Importance::Important, parent);
    }

    style
}

/// The winning font-size declaration in one declaration block (last
/// parseable wins, matching apply order). Caller folds candidate+inline in
/// order to find the overall winner.
/// Last font-size declaration wins. Returns `(length, inherit)` — the
/// keyword `inherit` is not a CssLength, so it rides its own flag: an
/// author's explicit inherit must beat the UA default the blitz#940 rows
/// install for form controls.
fn last_font_size_decl(declarations: &str) -> (Option<CssLength>, bool) {
    let mut found = None;
    let mut inherit = false;
    for (name, value) in split_declarations(declarations) {
        if name == "font-size" {
            if let Some(d) = parse_font_size_len(&value) {
                found = Some(d);
                inherit = false;
            } else if value.trim().eq_ignore_ascii_case("inherit") {
                found = None;
                inherit = true;
            }
        }
    }
    (found, inherit)
}

/// Inline styles use the same declaration grammar.
pub fn apply_inline_declarations(style: &mut ComputedStyle, css: &str) -> bool {
    apply_declarations(style, css)
}
