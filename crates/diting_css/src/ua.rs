//! User-agent defaults for the tags this engine styles.
//!
//! Split from the css hub. The public names stay re-exported from the
//! crate root. Pure move.

use super::*;

/// UA defaults per tag — the tiny corner of the upstream ua_style() that our
/// property subset can express: phrasing content is inline, everything else
/// block; b/strong bold.
pub fn ua_display(tag: &str) -> Display {
    match tag {
        "span" | "a" | "b" | "i" | "strong" | "em" | "code" | "small" | "sub" | "sup"
        | "label" | "time" | "abbr" | "q"
        // The phrasing-content rest (obscura#936): mark/ins/del/big/s/u/
        // strike/tt/samp/kbd/dfn/var/cite/bdi/bdo are inline in every
        // browser UA sheet — falling to Block made a <mark> paint a
        // full-width yellow band and put each <ins>/<del>/<big> on its own
        // line. (pre/xmp/listing/plaintext stay Block.)
        | "mark" | "ins" | "del" | "big" | "s" | "u" | "strike" | "tt" | "samp" | "kbd"
        | "dfn" | "var" | "cite" | "bdi" | "bdo" => Display::Inline,
        // Form controls are inline-level per every browser UA sheet, and the
        // inline-flavor matters twice: computed-style fidelity (CSSOM) and
        // the run dispatch — inline-block replaced elements join the text
        // run as atomic boxes, which is what keeps unstyled inputs on the
        // same line as their labels (obscura#807 class). button
        // shrink-to-fits its label content.
        "input" | "textarea" | "button" => Display::InlineBlock,
        // Table family: the layout engine's table branch dispatches on these
        // roles (rows become flex rows of cells inside a column-of-rows
        // table node); the CSSOM already reports the same values.
        "table" => Display::Table,
        "tr" | "thead" | "tbody" | "tfoot" => Display::TableRow,
        "td" | "th" => Display::TableCell,
        // Replaced media is inline-level in every browser UA sheet: an
        // unstyled <img> sits ON the text line (baseline = bottom margin
        // edge), not stacked as a block between the text lines. Same run
        // dispatch as form controls — the replaced leaf joins the run.
        "img" | "video" | "iframe" | "canvas" | "object" | "embed" => Display::Inline,
        // Non-visual metadata elements: every browser's UA stylesheet sets
        // display:none on these (blitz assets/default.css included). Without
        // it a page's <head>/<style> blocks lay out as empty boxes and push
        // the whole body down the page.
        "head" | "style" | "script" | "meta" | "link" | "title" | "noscript"
        | "template" | "base" => Display::None,
        _ => Display::Block,
    }
}

pub fn ua_font_weight(tag: &str) -> Option<u16> {
    match tag {
        "b" | "strong" | "th" => Some(700),
        // Every browser UA sheet (blitz default.css included) bolds the
        // heading levels; our ua_font_weight only fed b/strong before,
        // so unstyled <h1> painted regular weight.
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => Some(700),
        // Form controls reset to 400 (blitz#940): a <b>-wrapped control
        // paints a regular-weight label in every browser — UA beats the
        // inherited weight, author rules re-declare.
        "input" | "button" | "select" | "textarea" => Some(400),
        _ => None,
    }
}

/// UA text-align: th centers every browser UA sheet's header cells, caption
/// carries CSS2.1's `caption { text-align: center }`. The element's own UA
/// declaration beats an inherited value (cascadeElement merges with `.or`),
/// so a th stays centered inside a text-align:right ancestor, matching
/// Chrome.
pub fn ua_text_align(tag: &str) -> Option<TextAlign> {
    match tag {
        "th" | "caption" => Some(TextAlign::Center),
        _ => None,
    }
}

/// UA text decorations (every browser UA sheet's block): u/ins underline,
/// s/del/strike line-through, and links underline — the `a` entry stands for
/// `a:-webkit-any-link`, so the cascade site gates it on an href attribute.
/// Author declarations override (they apply later in cascade_element).
pub fn ua_text_decoration(tag: &str) -> Option<TextDecorations> {
    match tag {
        "u" | "ins" | "a" => Some(TextDecorations {
            underline: true,
            ..Default::default()
        }),
        "s" | "del" | "strike" => Some(TextDecorations {
            line_through: true,
            ..Default::default()
        }),
        _ => None,
    }
}

/// UA heading font-sizes (blitz default.css / HTML rendering spec). Expressed
/// as em so the cascade's font-size pre-pass folds them against the PARENT
/// font-size, where author declarations override. small/big/sub/sup mirror
/// every browser UA sheet's relative keywords as em factors (smaller = 13.33
/// at the 16px base, larger = 1.2 × parent).
pub fn ua_font_size(tag: &str) -> Option<CssLength> {
    match tag {
        "h1" => Some(CssLength::Em(2.0)),
        "h2" => Some(CssLength::Em(1.5)),
        "h3" => Some(CssLength::Em(1.17)),
        "h4" => Some(CssLength::Em(1.0)),
        "h5" => Some(CssLength::Em(0.83)),
        "h6" => Some(CssLength::Em(0.67)),
        "small" | "sub" | "sup" => Some(CssLength::Em(0.8333)),
        "big" => Some(CssLength::Em(1.2)),
        // Form controls carry an ABSOLUTE UA size (Chromium html.css's
        // `font: 400 13.3333px system-ui`), unlike the em rows above: a
        // body { font-size: 24px } page still shows 13.33px input text in
        // Chrome, so this must not fold against the parent (blitz#940 —
        // controls previously inherited the page font wholesale).
        "input" | "button" | "select" | "textarea" => Some(CssLength::Px(13.3333)),
        _ => None,
    }
}

/// UA monospace families: code/kbd/samp/tt and the preformatted group are
/// monospace in every browser UA sheet. Beats the inherited family (`.or`
/// at the merge site), author declarations override it.
pub fn ua_font_family(tag: &str) -> Option<&'static str> {
    match tag {
        "code" | "kbd" | "samp" | "tt" | "pre" | "xmp" | "listing" | "plaintext" => {
            Some("monospace")
        }
        // Form controls carry the UA's own family and do NOT inherit the
        // page font (why every reset ships `input { font: inherit }`).
        // textarea is monospace in every browser UA sheet; the rest take
        // the generic sans stack (blitz#940). Any non-mono name renders
        // through the primary pair; `sans-serif` says what it means.
        "input" | "button" | "select" => Some("sans-serif"),
        "textarea" => Some("monospace"),
        _ => None,
    }
}

/// Whether a computed font-family list selects a monospace face for the
/// run: any top-level list member named `monospace`/`ui-monospace` (case-
/// insensitive, unquoted — quoted names are literal families). The layout
/// stack renders that run's ASCII through the bundled mono face; chars it
/// lacks (CJK) fall through to the CJK pair per-character, like a browser
/// per-char cascade.
pub fn wants_monospace(family: &str) -> bool {
    family.split(',').any(|f| {
        let f = f.trim();
        let unquoted = f
            .strip_prefix('"')
            .and_then(|f| f.strip_suffix('"'))
            .or_else(|| f.strip_prefix('\'').and_then(|f| f.strip_suffix('\'')))
            .unwrap_or(f);
        unquoted.eq_ignore_ascii_case("monospace") || unquoted.eq_ignore_ascii_case("ui-monospace")
    })
}

/// UA margins in CSS order (top right bottom left), from the same blitz
/// default.css block every other browser UA sheet mirrors. Em folds against
/// the element's OWN font-size at cascade time (h1's .67em × its 2em size).
/// UA white-space defaults: the preformatted group keeps whitespace and
/// newlines in every browser UA sheet. Fills after inheritance so an author
/// declaration (`pre { white-space: normal }`) still wins.
pub fn ua_white_space(tag: &str) -> Option<WhiteSpace> {
    match tag {
        "pre" | "xmp" | "listing" | "plaintext" => Some(WhiteSpace::Pre),
        _ => None,
    }
}

pub fn ua_margin(tag: &str) -> Option<[CssLength; 4]> {
    let em = |n: f32| CssLength::Em(n);
    let px = |n: f32| CssLength::Px(n);
    match tag {
        "body" => Some([px(8.0), px(8.0), px(8.0), px(8.0)]),
        "p" | "dl" | "multicol" | "search" => Some([em(1.0), px(0.0), em(1.0), px(0.0)]),
        "blockquote" | "figure" => Some([em(1.0), px(40.0), em(1.0), px(40.0)]),
        "dd" => Some([px(0.0), px(0.0), px(0.0), px(40.0)]),
        "ul" | "ol" | "menu" | "dir" => Some([em(1.0), px(0.0), em(1.0), px(0.0)]),
        "pre" | "xmp" | "listing" | "plaintext" => Some([em(1.0), px(0.0), em(1.0), px(0.0)]),
        "h1" => Some([em(0.67), px(0.0), em(0.67), px(0.0)]),
        "h2" => Some([em(0.83), px(0.0), em(0.83), px(0.0)]),
        "h3" => Some([em(1.0), px(0.0), em(1.0), px(0.0)]),
        "h4" => Some([em(1.33), px(0.0), em(1.33), px(0.0)]),
        "h5" => Some([em(1.67), px(0.0), em(1.67), px(0.0)]),
        "h6" => Some([em(2.33), px(0.0), em(2.33), px(0.0)]),
        _ => None,
    }
}

/// UA paddings: the classic 40px inline-start of nested list containers,
/// and Chrome html.css's `button { padding: 1px 6px }` — the horizontal
/// chrome auto-sized buttons were missing entirely (#159: an "OK" button
/// measured 19px, the bare label advance; Chrome ≈ 31).
pub fn ua_padding(tag: &str) -> Option<[CssLength; 4]> {
    match tag {
        "ul" | "ol" | "menu" | "dir" => Some([
            CssLength::Px(0.0),
            CssLength::Px(0.0),
            CssLength::Px(0.0),
            CssLength::Px(40.0),
        ]),
        "button" => Some([
            CssLength::Px(1.0),
            CssLength::Px(6.0),
            CssLength::Px(1.0),
            CssLength::Px(6.0),
        ]),
        _ => None,
    }
}

/// UA `border: 2px inset` on iframes (batch 7a) — the classic embedded
/// document frame, same rule blitz's assets/default.css carries (and every
/// real browser). The border lays out (600 attr width → 604 border box)
/// and paints as our uniform solid band; the `inset` style distinction is
/// a later batch. Buttons carry their 2px UA border from the same sheet
/// (#159) — the border box completes the shrink-to-fit chrome.
/// Parse one `list-style-type` value. Every numbering keyword maps onto
/// Decimal (v1 collapse — the marker shows and numbers correctly; roman/
/// alpha glyphs are a later batch); the geometric trio and none keep their
/// own slots. Unrecognized values return None (declaration drops, the
/// inherited slot survives — Chrome parse-time behavior).
pub(super) fn parse_list_style_type(v: &str) -> Option<ListStyleType> {
    match v {
        "none" => Some(ListStyleType::None),
        "disc" => Some(ListStyleType::Disc),
        "circle" => Some(ListStyleType::Circle),
        "square" => Some(ListStyleType::Square),
        "decimal"
        | "decimal-leading-zero"
        | "lower-roman"
        | "upper-roman"
        | "lower-alpha"
        | "lower-latin"
        | "upper-alpha"
        | "upper-latin" => Some(ListStyleType::Decimal),
        _ => None,
    }
}

pub fn ua_border(tag: &str) -> Option<(f32, BorderStyle)> {
    match tag {
        "iframe" => Some((2.0, BorderStyle::Solid)),
        "button" => Some((2.0, BorderStyle::Solid)),
        _ => None,
    }
}

/// UA `list-style-type` on the list containers (every browser UA sheet):
/// the disc family for ul/menu/dir, decimal for ol. `li` declares nothing
/// — it inherits from its container, and a bare li (no list ancestor)
/// keeps the CSS initial `disc`, which the marker use site fills in.
pub fn ua_list_style(tag: &str) -> Option<ListStyleType> {
    match tag {
        "ul" | "menu" | "dir" => Some(ListStyleType::Disc),
        "ol" => Some(ListStyleType::Decimal),
        _ => None,
    }
}
