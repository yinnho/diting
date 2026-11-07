//! HTML → Markdown conversion on the htmlq / vendored html5ever 0.40 stack.
//!
//! Absorbs html2md's only used face (`parse_html`, four call sites in
//! render.rs / firecrawl_compat.rs) so the old html5ever 0.39 generation
//! leaves the dependency graph with it (#32). Output targets the same
//! construct shapes — ATX headings, `[text](url)` links, `![alt](src)`
//! images, `-`/`1.` lists with nesting, fenced code blocks with the
//! `language-*` class as the info string, `>` quotes, GFM pipe tables.
//!
//! Divergences from html2md, deliberate: headings are plain ATX (html2md
//! appended setext underlines and closing hashes), hard breaks are single
//! newlines (not two-space), and markdown metacharacters are NOT escaped —
//! this markdown is agent-facing output, and `\*` noise reads worse than a
//! literal asterisk. Whitespace collapses per HTML rules outside `pre`.

use ego_tree::NodeRef;
use crate::htmlq::Node;
use crate::htmlq::Html;

/// Convert an HTML document (or fragment) to markdown.
pub fn parse_html(html: &str) -> String {
    let doc = Html::parse_document(html);
    let mut conv = Converter::default();
    for node in doc.tree.root().children() {
        conv.node(node);
    }
    clean(&conv.out)
}

/// Tags whose entire subtree produces no markdown.
const SKIP: [&str; 6] = ["script", "style", "noscript", "template", "head", "svg"];

/// Block-level tags: force a paragraph boundary before and after.
const BLOCK: [&str; 17] = [
    "p", "div", "section", "article", "header", "footer", "nav", "main", "aside", "figure",
    "figcaption", "form", "fieldset", "hr", "blockquote", "details", "summary",
];

#[derive(Default)]
struct Converter {
    out: String,
    /// Ordered-list counters, one per open `ol` nesting level.
    counters: Vec<u64>,
}

impl Converter {
    fn node(&mut self, node: NodeRef<'_, Node>) {
        match node.value() {
            Node::Text(text) => self.text(text),
            Node::Element(el) => self.element(el, node),
            _ => {}
        }
    }

    fn text(&mut self, text: &str) {
        let at_line_start = self.out.is_empty() || self.out.ends_with('\n');
        if text.trim().is_empty() {
            // A whitespace-only node between inline content is a word
            // separator (HTML collapses runs to one space) — keep exactly
            // one unless we're at a line boundary.
            if !at_line_start && !self.out.ends_with(' ') {
                self.out.push(' ');
            }
            return;
        }
        let collapsed = collapse(text);
        if at_line_start {
            self.push_inline(collapsed.trim_start_matches(' '));
        } else {
            self.push_inline(&collapsed);
        }
    }

    fn push_inline(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        // Space-join at the inline boundary without doubling.
        if !self.out.is_empty() && !self.out.ends_with(['\n', ' ']) && s.starts_with(' ') {
            self.out.push(' ');
            self.out.push_str(s.trim_start_matches(' '));
        } else if self.out.ends_with(' ') && s.starts_with(' ') {
            self.out.push_str(s.trim_start_matches(' '));
        } else {
            self.out.push_str(s);
        }
    }

    fn element(&mut self, el: &crate::htmlq::Element, node: NodeRef<'_, Node>) {
        let tag: &str = &el.name.local;
        if SKIP.contains(&tag) {
            return;
        }
        match tag {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                let level = tag[1..].parse::<usize>().unwrap_or(1);
                self.blank_line();
                self.out.push_str(&"#".repeat(level));
                self.out.push(' ');
                self.children(node);
                self.out.push_str("\n\n");
            }
            "br" => self.out.push('\n'),
            "hr" => {
                self.blank_line();
                self.out.push_str("---\n\n");
            }
            "img" => {
                let alt = el.attr("alt").unwrap_or_default();
                let src = el.attr("src").unwrap_or_default();
                let title = el.attr("title").map(|t| format!(" \"{t}\"")).unwrap_or_default();
                if !src.is_empty() {
                    self.push_inline(&format!("![{alt}]({src}{title})"));
                }
            }
            "a" => {
                let href = el.attr("href").unwrap_or_default().to_string();
                let start = self.out.len();
                self.children(node);
                if !href.is_empty() && self.out.len() > start {
                    let label = self.out[start..].trim().to_string();
                    self.out.truncate(start);
                    if label.is_empty() {
                        self.push_inline(&href);
                    } else {
                        self.push_inline(&format!("[{label}]({href})"));
                    }
                }
            }
            "strong" | "b" => self.wrap_inline(node, "**"),
            "em" | "i" => self.wrap_inline(node, "*"),
            "del" | "s" | "strike" => self.wrap_inline(node, "~~"),
            "code" | "samp" | "kbd" => {
                let start = self.out.len();
                self.children(node);
                let code = self.out[start..].trim().to_string();
                self.out.truncate(start);
                if !code.is_empty() {
                    self.push_inline(&format!("`{code}`"));
                }
            }
            "pre" => {
                let (lang, content) = pre_content(node);
                self.blank_line();
                self.out.push_str("```");
                self.out.push_str(&lang);
                self.out.push('\n');
                self.out.push_str(content.trim_matches('\n'));
                self.out.push('\n');
                self.out.push_str("```\n\n");
            }
            "blockquote" => {
                self.blank_line();
                let start = self.out.len();
                self.children(node);
                let quoted = self.out[start..].to_string();
                self.out.truncate(start);
                for line in quoted.trim().lines() {
                    if line.trim().is_empty() {
                        self.out.push_str(">\n");
                    } else {
                        self.out.push_str("> ");
                        self.out.push_str(line);
                        self.out.push('\n');
                    }
                }
                self.out.push('\n');
            }
            "ul" => {
                self.blank_line();
                self.list_items(node, false, 0);
                self.out.push('\n');
            }
            "ol" => {
                self.blank_line();
                self.counters.push(0);
                self.list_items(node, true, 0);
                self.counters.pop();
                self.out.push('\n');
            }
            "table" => {
                let rendered = table(node);
                if !rendered.is_empty() {
                    self.blank_line();
                    self.out.push_str(&rendered);
                    self.out.push('\n');
                }
            }
            _ => {
                if BLOCK.contains(&tag) {
                    self.blank_line();
                }
                self.children(node);
                if BLOCK.contains(&tag) {
                    self.out.push('\n');
                    self.blank_line();
                }
            }
        }
    }

    fn children(&mut self, node: NodeRef<'_, Node>) {
        for child in node.children() {
            self.node(child);
        }
    }

    fn wrap_inline(&mut self, node: NodeRef<'_, Node>, marker: &str) {
        let start = self.out.len();
        self.children(node);
        let inner = self.out[start..].trim().to_string();
        self.out.truncate(start);
        if !inner.is_empty() {
            self.push_inline(&format!("{marker}{inner}{marker}"));
        }
    }

    fn list_items(&mut self, node: NodeRef<'_, Node>, ordered: bool, depth: usize) {
        let indent = "  ".repeat(depth);
        for li in node.children().filter(|c| is_tag(*c, "li")) {
            // The item body is everything except a nested list; the nested
            // list continues on the next line, indented one level deeper.
            let nested = li.children().find(|c| is_tag(*c, "ul") || is_tag(*c, "ol"));
            let marker = if ordered {
                let n = self.counters.last_mut().map(|c| {
                    *c += 1;
                    *c
                }).unwrap_or(1);
                format!("{n}. ")
            } else {
                "- ".to_string()
            };
            let start = self.out.len();
            self.out.push_str(&indent);
            self.out.push_str(&marker);
            for child in li.children() {
                if !(is_tag(child, "ul") || is_tag(child, "ol")) {
                    self.node(child);
                }
            }
            // Trim the trailing blank lines the item body may have emitted.
            let body = self.out[start..].trim_end().to_string();
            self.out.truncate(start);
            self.out.push_str(&body);
            self.out.push('\n');
            if let Some(sub) = nested {
                let sub_ordered = is_tag(sub, "ol");
                if sub_ordered {
                    self.counters.push(0);
                }
                self.list_items(sub, sub_ordered, depth + 1);
                if sub_ordered {
                    self.counters.pop();
                }
            }
        }
    }

    fn blank_line(&mut self) {
        while self.out.ends_with('\n') {
            self.out.pop();
        }
        if !self.out.is_empty() {
            self.out.push_str("\n\n");
        }
    }
}

fn is_tag(node: NodeRef<'_, Node>, tag: &str) -> bool {
    node.value()
        .as_element()
        .map(|e| &*e.name.local == tag)
        .unwrap_or(false)
}

fn collapse(s: &str) -> String {
    // Collapse every whitespace run to one space, keeping one leading and
    // one trailing space as inline-boundary separators (the caller decides
    // whether a boundary is a line start, where they don't contribute).
    let mut out = String::with_capacity(s.len());
    let mut pending_ws = s.starts_with(|c: char| c.is_whitespace());
    for ch in s.chars() {
        if ch.is_whitespace() {
            pending_ws = true;
        } else {
            if pending_ws {
                out.push(' ');
            }
            pending_ws = false;
            out.push(ch);
        }
    }
    if s.ends_with(|c: char| c.is_whitespace()) {
        out.push(' ');
    }
    out
}

/// Extract `(language, raw-text)` from a `pre` element: the language comes
/// from the pre's own or its `code` child's `language-*`/`lang-*` class, the
/// text from text descendants with original whitespace.
fn pre_content(node: NodeRef<'_, Node>) -> (String, String) {
    let mut lang = String::new();
    let mut content = String::new();
    if let Some(el) = node.value().as_element() {
        if let Some(class) = el.attr("class") {
            lang = class_lang(class);
        }
    }
    for child in node.children() {
        match child.value() {
            Node::Text(t) => content.push_str(t),
            Node::Element(el) => {
                if lang.is_empty() {
                    if let Some(class) = el.attr("class") {
                        lang = class_lang(class);
                    }
                }
                collect_text(child, &mut content);
            }
            _ => {}
        }
    }
    (lang, content)
}

fn collect_text(node: NodeRef<'_, Node>, out: &mut String) {
    match node.value() {
        Node::Text(t) => out.push_str(t),
        Node::Element(el) if matches!(el.name.local.as_ref(), "br") => out.push('\n'),
        Node::Element(_) => {
            for child in node.children() {
                collect_text(child, out);
            }
        }
        _ => {}
    }
}

fn class_lang(classes: &str) -> String {
    classes
        .split_whitespace()
        .find_map(|c| c.strip_prefix("language-").or_else(|| c.strip_prefix("lang-")))
        .unwrap_or_default()
        .to_string()
}

/// Render a `table` as a GFM pipe table. A table without rows yields "".
fn table(node: NodeRef<'_, Node>) -> String {
    let rows = table_rows(node, 0);
    if rows.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for (i, row) in rows.iter().enumerate() {
        out.push('|');
        for cell in row {
            out.push(' ');
            out.push_str(cell);
            out.push_str(" |");
        }
        out.push('\n');
        if i == 0 {
            out.push('|');
            for _ in row {
                out.push_str(" --- |");
            }
            out.push('\n');
        }
    }
    out
}

fn table_rows(node: NodeRef<'_, Node>, depth: usize) -> Vec<Vec<String>> {
    if depth > 12 {
        return Vec::new();
    }
    let mut rows = Vec::new();
    for child in node.children() {
        if !matches!(child.value(), Node::Element(_)) {
            continue;
        }
        match tag_name(child) {
            Some("tr") => {
                let cells: Vec<String> = child
                    .children()
                    .filter(|c| is_tag(*c, "td") || is_tag(*c, "th"))
                    .map(cell_text)
                    .collect();
                if !cells.is_empty() {
                    rows.push(cells);
                }
            }
            Some("thead" | "tbody" | "tfoot") => rows.extend(table_rows(child, depth + 1)),
            _ => {}
        }
    }
    rows
}

fn tag_name(node: NodeRef<'_, Node>) -> Option<&str> {
    node.value().as_element().map(|e| &*e.name.local)
}

/// A cell is its inline content flattened to one line (`|` escaped so it
/// can't break the table geometry).
fn cell_text(node: NodeRef<'_, Node>) -> String {
    let mut conv = Converter::default();
    for child in node.children() {
        conv.node(child);
    }
    conv.out.trim().replace('|', "\\|")
}

fn clean(data: &str) -> String {
    let mut out = String::with_capacity(data.len());
    let mut blank = 0;
    for line in data.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            blank += 1;
        } else {
            blank = 0;
        }
        if blank <= 1 {
            out.push_str(line);
            out.push('\n');
        }
    }
    out.trim().to_string() + "\n"
}

#[cfg(test)]
mod tests {
    use super::parse_html;

    #[test]
    fn headings_are_atx() {
        let md = parse_html("<h1>Title</h1><h2>Sub</h2><h3>Deep</h3>");
        assert_eq!(md, "# Title\n\n## Sub\n\n### Deep\n");
    }

    #[test]
    fn links_and_images_keep_their_targets() {
        let md = parse_html(
            r#"<p><a href="https://a.test/x">go <b>there</b></a> and <a href="/rel">plain</a></p>
               <img src="pic.png" alt="a pic" title="the pic">"#,
        );
        assert_eq!(
            md,
            "[go **there**](https://a.test/x) and [plain](/rel)\n\n![a pic](pic.png \"the pic\")\n"
        );
    }

    #[test]
    fn nested_lists_keep_order_and_indent() {
        let md = parse_html(
            "<ul><li>one</li><li>two<ol><li>two.a</li><li>two.b</li></ol></li><li>three</li></ul>",
        );
        assert_eq!(md, "- one\n- two\n  1. two.a\n  2. two.b\n- three\n");
    }

    #[test]
    fn code_blocks_fence_with_language_class() {
        let md = parse_html(
            "<pre><code class=\"language-rust\">fn main() {\n    println!(\"hi\");\n}\n</code></pre>",
        );
        assert_eq!(md, "```rust\nfn main() {\n    println!(\"hi\");\n}\n```\n");
    }

    #[test]
    fn inline_code_stays_on_one_line() {
        let md = parse_html("<p>use <code>x &amp; y</code> here</p>");
        assert_eq!(md, "use `x & y` here\n");
    }

    #[test]
    fn quotes_prefix_each_line() {
        let md = parse_html("<blockquote><p>line one</p><p>line two</p></blockquote>");
        assert_eq!(md, "> line one\n>\n> line two\n");
    }

    #[test]
    fn tables_become_gfm_pipes() {
        let md = parse_html(
            "<table><tr><th>k</th><th>v</th></tr><tr><td>a|b</td><td>1</td></tr></table>",
        );
        assert_eq!(md, "| k | v |\n| --- | --- |\n| a\\|b | 1 |\n");
    }

    #[test]
    fn whitespace_collapses_but_pre_keeps_it() {
        let md = parse_html("<p>hello\n\n   world\t!</p><pre>  keep\n   me  </pre>");
        assert_eq!(md, "hello world !\n\n```\n  keep\n   me\n```\n");
    }

    #[test]
    fn skipped_tags_vanish() {
        let md = parse_html("<div><script>evil()</script><style>a{}</style><p>kept</p></div>");
        assert_eq!(md, "kept\n");
    }

    #[test]
    fn hr_and_br_shapes() {
        let md = parse_html("<p>a<br>b</p><hr><p>c</p>");
        assert_eq!(md, "a\nb\n\n---\n\nc\n");
    }

    #[test]
    fn real_page_shape_end_to_end() {
        // Representative stripped-body shape: header + nav chaff around the
        // article is the common firecrawl/render_markdown input.
        let md = parse_html(
            r#"<html><body>
               <nav><a href="/">home</a> <a href="/about">about</a></nav>
               <article><h1>Engines</h1>
               <p>Diting renders <em>fast</em>. See <a href="https://diting.test">docs</a>.</p>
               <h2>Stack</h2><ul><li>taffy</li><li>swash</li></ul>
               <pre><code class="language-js">let x = 1;</code></pre>
               </article></body></html>"#,
        );
        let want = "\
[home](/) [about](/about)

# Engines

Diting renders *fast*. See [docs](https://diting.test).

## Stack

- taffy
- swash

```js
let x = 1;
```
";
        assert_eq!(md, want);
    }
}
