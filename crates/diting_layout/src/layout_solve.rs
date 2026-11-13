//! Public layout entry: solve the box tree, then the `layout_dom*` faces.
//!
//! Split from the layout hub. The public names stay re-exported from
//! the crate root. `SolvedGeometry`'s fields stay crate-visible: the
//! collect walk and the fixup passes read them.

use super::*;

/// Lay a DOM tree out at a fixed viewport size and return each element's
/// absolute border-box rect. Elements are keyed by diting_dom NodeId; the
/// root's containing block is the viewport (a definite-sized root taffy
/// node, so bottom/percentage insets on fixed boxes resolve against it —
/// obscura#675 lineage fix).
pub fn layout_dom(
    tree: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    fonts: &FontBook,
    viewport_width: f32,
    viewport_height: f32,
) -> HashMap<NodeId, Rect> {
    layout_dom_with_paint(tree, styles, fonts, viewport_width, viewport_height).0
}

/// [`layout_dom`] plus the paint item list in document order (an element's
/// `Bg` precedes the items of its subtree, so a solid background lands
/// under its descendants' ink).
pub fn layout_dom_with_paint(
    tree: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    fonts: &FontBook,
    viewport_width: f32,
    viewport_height: f32,
) -> (HashMap<NodeId, Rect>, Vec<PaintItem>) {
    layout_dom_with_paint_and_images(
        tree,
        styles,
        fonts,
        viewport_width,
        viewport_height,
        None,
        None,
    )
}

/// [`layout_dom_with_paint`] with an injected table of fetched image bodies
/// (batch 6c): absolute `http(s)`/`file` URL → response body. `<img src>`
/// entries pointing at those URLs decode like data: URLs (PNG only); misses
/// keep the placeholder. The fetch itself is the caller's job — the screenshot
/// prefetch pass fills this from diting_net. `base_url` is the document URL
/// relative srcs resolve against to reach the table's absolute keys (same
/// join the fetchers used).
#[allow(clippy::too_many_arguments)]
pub fn layout_dom_with_paint_and_images(
    tree: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    fonts: &FontBook,
    viewport_width: f32,
    viewport_height: f32,
    network_bytes: Option<&HashMap<String, std::sync::Arc<Vec<u8>>>>,
    base_url: Option<&str>,
) -> (HashMap<NodeId, Rect>, Vec<PaintItem>) {
    let (rects, items, _order, _local_geom, _sticky_spans, _scroller_spans) =
        layout_dom_with_paint_order_and_images(
            tree,
            styles,
            fonts,
            viewport_width,
            viewport_height,
            network_bytes,
            base_url,
        );
    (rects, items)
}

/// Map a rect through a CSS-order affine array [a,b,c,d,e,f]: the diagonal
/// fast path keeps the historical two-corner normalize (negative scales
/// mirror); anything else maps all four corners and unions them — Chrome's
/// gBCR bounding-box behavior under rotation. Free-standing so
/// [`expand_wrapped_leaves`] can map through a recorded array — the walk's
/// `Xf` type is fn-local.
pub(super) fn map_rect_arr(m: [f32; 6], r: Rect) -> Rect {
    if m[1] == 0.0 && m[2] == 0.0 {
        let (x1, x2) = (r.x * m[0] + m[4], (r.x + r.width) * m[0] + m[4]);
        let (y1, y2) = (r.y * m[3] + m[5], (r.y + r.height) * m[3] + m[5]);
        Rect {
            x: x1.min(x2),
            y: y1.min(y2),
            width: (x2 - x1).abs(),
            height: (y2 - y1).abs(),
        }
    } else {
        let pt = |x: f32, y: f32| (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]);
        let (x0, y0) = pt(r.x, r.y);
        let (x1, y1) = pt(r.x + r.width, r.y);
        let (x2, y2) = pt(r.x, r.y + r.height);
        let (x3, y3) = pt(r.x + r.width, r.y + r.height);
        let xs = [x0, x1, x2, x3];
        let ys = [y0, y1, y2, y3];
        let min_x = xs.iter().cloned().fold(f32::MAX, f32::min);
        let max_x = xs.iter().cloned().fold(f32::MIN, f32::max);
        let min_y = ys.iter().cloned().fold(f32::MAX, f32::min);
        let max_y = ys.iter().cloned().fold(f32::MIN, f32::max);
        Rect {
            x: min_x,
            y: min_y,
            width: max_x - min_x,
            height: max_y - min_y,
        }
    }
}

/// Collect the canvas-space rects of everything a flattened inline hoisted:
/// descend through run wrappers (nested flattening wraps wrapper into
/// wrapper) and stop at leaves / boxed nodes, which carry their LOCAL rect
/// plus the accumulated map in `local_by_node` and a first-item index in
/// `node_first_item`. A wrapped run leaf is one taffy box covering every
/// line it broke into — split it with the same greedy-wrap truth the paint
/// path uses (in LOCAL space, where the wrap width still means what the
/// paint item's `wrap_at` means), then map each line band to canvas —
/// under rotation a horizontal line band is a vertical stripe, and the
/// mapped bbox of each band is the closest an axis-aligned Bg can get.
pub(super) fn expand_wrapped_leaves(
    node: diting_taffy::tree::NodeId,
    taffy_tree: &TaffyTree<TextLeaf>,
    wrapper_set: &std::collections::HashSet<diting_taffy::tree::NodeId>,
    local_by_node: &HashMap<diting_taffy::tree::NodeId, (Rect, [f32; 6])>,
    fonts: &FontBook,
    pieces: &mut Vec<Rect>,
    owners: &mut Vec<diting_taffy::tree::NodeId>,
) {
    if wrapper_set.contains(&node) {
        for c in taffy_tree.children(node).unwrap_or_default() {
            expand_wrapped_leaves(
                c,
                taffy_tree,
                wrapper_set,
                local_by_node,
                fonts,
                pieces,
                owners,
            );
        }
        return;
    }
    let Some((r, m)) = local_by_node.get(&node) else {
        return;
    };
    if let Some(TextLeaf::Run {
        text,
        font_size,
        bold,
        line_height,
        mono,
        word_spacing,
        ws,
        line_clamp,
        tokens,
        small_caps,
        han,
        ..
    }) = taffy_tree.get_node_context(node)
    {
        let tokens = run_tokens(
            text,
            *font_size,
            *bold,
            fonts,
            *mono,
            *word_spacing,
            *ws,
            tokens,
            *small_caps,
            *han,
        );
        // no-soft-wrap modes keep their single line here too: the affine
        // path expands a leaf into per-line tiles, and wrapping such a run
        // at the box width would split ink the rasterizer lays on one line.
        let mut lines = text::greedy_wrap(
            &tokens,
            if ws.no_soft_wrap() {
                None
            } else {
                Some(r.width.max(0.0))
            },
            *ws,
        );
        // line-clamp (#271): band pieces stop at the Nth line — the same
        // count-only rule the measure path applies, so an inline span's
        // background never bands over the clamped-out lines.
        if let Some(n) = line_clamp.filter(|n| lines.len() > *n as usize) {
            lines.truncate(n as usize);
        }
        for (i, line) in lines.iter().enumerate() {
            if line.width <= 0.0 {
                continue;
            }
            pieces.push(map_rect_arr(
                *m,
                Rect {
                    x: r.x,
                    y: r.y + i as f32 * line_height,
                    width: line.width,
                    height: *line_height,
                },
            ));
            owners.push(node);
        }
        return;
    }
    pieces.push(map_rect_arr(*m, *r));
    owners.push(node);
}

/// Baseline of a text line box below its top edge, from real font metrics
/// (the same quantized model the paint path rasterizes against).
fn text_baseline(fonts: &FontBook, font_size: f32, bold: bool, line_height: f32) -> f32 {
    let m = fonts
        .metrics(font_size, bold)
        .unwrap_or(text::ScaledMetrics {
            ascent: font_size,
            descent: font_size * 0.2,
            line_gap: 0.0,
        });
    text::baseline_offset(m.ascent, m.descent, line_height)
}

/// The LAST in-flow line baseline inside an atomic inline-level box (CSS2:
/// an inline-block baseline-aligns to the baseline of its last in-flow line
/// box), as a y offset from the box's border-box top. `rel_y` accumulates
/// taffy locations plus already-computed baseline shifts (inner runs are
/// processed before outer ones). Returns None when the subtree carries no
/// text — the caller then falls back to the bottom edge, which is the CSS
/// behavior for a box with no in-flow line boxes.
fn subtree_last_baseline(
    taffy_tree: &TaffyTree<TextLeaf>,
    node: diting_taffy::tree::NodeId,
    rel_y: f32,
    shifts: &HashMap<diting_taffy::tree::NodeId, f32>,
    fonts: &FontBook,
    best: &mut Option<f32>,
) {
    let Ok(layout) = taffy_tree.layout(node) else {
        return;
    };
    let rel_y = rel_y + shifts.get(&node).copied().unwrap_or(0.0);
    match taffy_tree.get_node_context(node) {
        Some(TextLeaf::Word {
            font_size,
            bold,
            line_height,
            baseline_shift,
            ..
        }) => {
            if layout.size.height > 0.0 {
                let b =
                    rel_y + text_baseline(fonts, *font_size, *bold, *line_height) - baseline_shift;
                *best = Some(best.map_or(b, |x: f32| x.max(b)));
            }
            return;
        }
        Some(TextLeaf::Run {
            font_size,
            bold,
            line_height,
            baseline_shift,
            ..
        }) => {
            // A wrapped run's last line sits one line box above its bottom;
            // the shift pad is empty reservation, not a text line.
            let pad = shift_pad(*baseline_shift, leaf_descent(fonts, *font_size, *bold));
            let b = rel_y + layout.size.height - pad - line_height
                + text_baseline(fonts, *font_size, *bold, *line_height)
                - baseline_shift;
            *best = Some(best.map_or(b, |x: f32| x.max(b)));
            return;
        }
        Some(TextLeaf::Replaced { strut_descent }) => {
            // Baseline = the element box's bottom edge (the pad below it is
            // strut reservation, not element box).
            let b = rel_y + layout.size.height - strut_descent;
            *best = Some(best.map_or(b, |x: f32| x.max(b)));
            return;
        }
        // A pseudo box is textless (#172): its children (none) hold no
        // baseline, the bottom-edge fallback applies.
        Some(TextLeaf::Pseudo { .. }) | None => {}
    }
    for child in taffy_tree.children(node).unwrap_or_default() {
        let Ok(cl) = taffy_tree.layout(child) else {
            continue;
        };
        subtree_last_baseline(
            taffy_tree,
            child,
            rel_y + cl.location.y,
            shifts,
            fonts,
            best,
        );
    }
}

/// Baseline alignment for inline runs (blitz#750 family): CSS aligns the
/// boxes of one line by their baselines, but taffy leaves cannot report
/// baselines (the measure closure returns Size only — still true at the
/// taffy rev blitz's own baseline PR pins), so run wrappers lay out
/// FLEX_START and we shift afterwards. Wrapping is width-driven, so the
/// flex lines taffy already formed ARE the CSS line boxes; per line every
/// item drops by (line max baseline − its own baseline). For uniform-font
/// runs every dy is zero, leaving existing geometry bit-identical — only
/// runs mixing font metrics or atomic boxes move.
fn compute_baseline_shifts(
    taffy_tree: &TaffyTree<TextLeaf>,
    run_wrappers: &[diting_taffy::tree::NodeId],
    fonts: &FontBook,
) -> HashMap<diting_taffy::tree::NodeId, f32> {
    let mut shifts: HashMap<diting_taffy::tree::NodeId, f32> = HashMap::new();
    // First-line baselines of processed wrappers: a nested wrapper (flattened
    // inline content) joins an outer run as ONE item and, as a flex
    // container, contributes its FIRST baseline (css-align, blitz#750's
    // Display::Flex arm).
    let mut first_baselines: HashMap<diting_taffy::tree::NodeId, f32> = HashMap::new();
    // Inner wrappers first: outer runs read their shifts and first baselines.
    let depth = |mut n: diting_taffy::tree::NodeId| {
        let mut d = 0usize;
        while let Some(p) = taffy_tree.parent(n) {
            d += 1;
            n = p;
        }
        d
    };
    let mut ordered = run_wrappers.to_vec();
    ordered.sort_by_key(|w| std::cmp::Reverse(depth(*w)));

    for wrapper in ordered {
        let children = taffy_tree.children(wrapper).unwrap_or_default().to_vec();
        if children.len() < 2 {
            // A lone item aligns to itself; nothing to shift. Still record
            // the first baseline for an enclosing run.
            if let Some(&only) = children.first() {
                if let Ok(l) = taffy_tree.layout(only) {
                    if l.size.height > 0.0 {
                        let b = item_baseline(
                            taffy_tree,
                            only,
                            &shifts,
                            &first_baselines,
                            fonts,
                            l.size.height,
                        );
                        first_baselines.insert(wrapper, b);
                    }
                }
            }
            continue;
        }
        // Group into flex lines by y (taffy rounds locations to the pixel
        // grid, so same-line items share a y).
        let mut lines: Vec<(f32, Vec<(diting_taffy::tree::NodeId, f32)>)> = Vec::new();
        for child in children {
            let Ok(l) = taffy_tree.layout(child) else {
                continue;
            };
            // Whitespace-only word leaves carry no height and no ink.
            if l.size.height <= 0.0 {
                continue;
            }
            let b = item_baseline(
                taffy_tree,
                child,
                &shifts,
                &first_baselines,
                fonts,
                l.size.height,
            );
            match lines
                .iter_mut()
                .find(|(y, _)| (*y - l.location.y).abs() < 0.5)
            {
                Some((_, items)) => items.push((child, b)),
                None => lines.push((l.location.y, vec![(child, b)])),
            }
        }
        lines.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        if let Some((_, items)) = lines.first() {
            let b = items.iter().fold(0.0f32, |m, (_, b)| m.max(*b));
            first_baselines.insert(wrapper, b);
        }
        for (_, items) in lines {
            let line_b = items.iter().fold(0.0f32, |m, (_, b)| m.max(*b));
            for (child, b) in items {
                let dy = line_b - b;
                if dy > 0.01 {
                    shifts.insert(child, dy);
                }
            }
        }
    }
    shifts
}

/// Baseline of one run item below its border-box top.
fn item_baseline(
    taffy_tree: &TaffyTree<TextLeaf>,
    child: diting_taffy::tree::NodeId,
    shifts: &HashMap<diting_taffy::tree::NodeId, f32>,
    first_baselines: &HashMap<diting_taffy::tree::NodeId, f32>,
    fonts: &FontBook,
    height: f32,
) -> f32 {
    match taffy_tree.get_node_context(child) {
        Some(TextLeaf::Word {
            font_size,
            bold,
            line_height,
            baseline_shift,
            ..
        }) => text_baseline(fonts, *font_size, *bold, *line_height) - baseline_shift,
        Some(TextLeaf::Run {
            font_size,
            bold,
            line_height,
            baseline_shift,
            ..
        }) => {
            let pad = shift_pad(*baseline_shift, leaf_descent(fonts, *font_size, *bold));
            height - pad - line_height + text_baseline(fonts, *font_size, *bold, *line_height)
                - baseline_shift
        }
        Some(TextLeaf::Replaced { strut_descent }) => height - strut_descent,
        // A pseudo box is textless (#172): bottom-edge fallback, same as a
        // textless box.
        Some(TextLeaf::Pseudo { .. }) | None => {
            if let Some(b) = first_baselines.get(&child) {
                return *b;
            }
            if taffy_tree.children(child).map_or(0, |c| c.len()) > 0 {
                // Atomic inline-level box: last in-flow line baseline.
                let mut best = None;
                subtree_last_baseline(taffy_tree, child, 0.0, shifts, fonts, &mut best);
                if let Some(b) = best {
                    return b;
                }
            }
            // Textless box: bottom edge (CSS bottom margin-edge fallback —
            // run items carry no vertical margins in this model).
            height
        }
    }
}

/// [`layout_dom_with_paint_and_images`] plus the paint order: every boxed
/// element in the sequence the flat item list paints it, so the LAST entry
/// whose rect contains a point is the element a pixel there shows. This is
/// the ranking `elementFromPoint` needs — document order is not paint order
/// once positioned siblings with z-index hoist out of it (obscura #738).
/// Boxless flattened inline wrappers (span/a/label — obscura#722 lineage)
/// are absent; hit testing ranks them with their nearest boxed ancestor,
/// which paints their hoisted ink in-flow anyway.
pub fn layout_dom_with_paint_order_and_images(
    tree: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    fonts: &FontBook,
    viewport_width: f32,
    viewport_height: f32,
    network_bytes: Option<&HashMap<String, std::sync::Arc<Vec<u8>>>>,
    base_url: Option<&str>,
) -> LayoutCollect {
    let solved = layout_solve(
        tree,
        styles,
        fonts,
        viewport_width,
        viewport_height,
        network_bytes,
        base_url,
    );
    layout_collect(tree, styles, fonts, &solved, viewport_width)
}

/// The solved half of a layout run, split out for #395: the taffy tree after
/// every pass (build, static-position harvest, reparent, main solve, float
/// continuation, calc repair, table span placement) plus the auxiliary maps
/// the collect walk consumes. Paint-only property writes — transform and
/// opacity, everything a GSAP-style timeline seek lands — are invisible to
/// all of it: `to_taffy_style` maps only geometry families, the solve passes
/// consume only those, and baseline shifts read pure layout. A fresh
/// [`layout_collect`] over a cached [`SolvedGeometry`] is therefore
/// bit-identical to re-running the whole pipeline — which is what lets the
/// video pump repaint per-seek frames without the full-page relayout it used
/// to pay (43 solves × 114ms on the probe page).
pub struct SolvedGeometry {
    pub(crate) taffy_tree: TaffyTree<TextLeaf>,
    pub(crate) node_map: HashMap<diting_taffy::tree::NodeId, NodeId>,
    pub(crate) images: HashMap<NodeId, DecodedImage>,
    pub(crate) static_pos: HashMap<NodeId, (f32, f32)>,
    pub(crate) baseline_shifts: HashMap<diting_taffy::tree::NodeId, f32>,
    pub(crate) run_wrappers: Vec<diting_taffy::tree::NodeId>,
    pub(crate) collapsed_edges: HashMap<NodeId, [bool; 4]>,
    pub(crate) flattened: HashMap<NodeId, Vec<diting_taffy::tree::NodeId>>,
    /// The synthetic ICB the walk roots at; `None` marks an aborted solve
    /// (no element root, or a taffy error) — collect returns the same empty
    /// outputs the undivided pipeline returned on those paths.
    pub(crate) icb_node: Option<diting_taffy::tree::NodeId>,
    /// Which DOM text node(s) each text leaf carries (range / client-rect queries).
    pub(crate) text_src: text_src::SrcMap,
}

impl SolvedGeometry {
    /// DOM-side solved border boxes (DOM id → (w, h)): the container-query
    /// pass reads candidate container widths from here without a full
    /// collect. Display:none nodes have no taffy box and are absent — a
    /// missing box means "not queryable", matching the containment contract.
    pub fn dom_boxes(&self) -> HashMap<NodeId, (f32, f32)> {
        self.node_map
            .iter()
            .filter_map(|(t, d)| {
                self.taffy_tree
                    .layout(*t)
                    .ok()
                    .map(|l| (*d, (l.size.width, l.size.height)))
            })
            .collect()
    }

    /// The empty solve the early-return paths hand back: nothing walked,
    /// nothing collected.
    fn aborted() -> Self {
        SolvedGeometry {
            taffy_tree: TaffyTree::new(),
            node_map: HashMap::new(),
            images: HashMap::new(),
            static_pos: HashMap::new(),
            baseline_shifts: HashMap::new(),
            run_wrappers: Vec::new(),
            collapsed_edges: HashMap::new(),
            flattened: HashMap::new(),
            icb_node: None,
            text_src: Default::default(),
        }
    }
}

/// Solve the taffy tree for the live DOM (the first half of what
/// [`layout_dom_with_paint_order_and_images`] used to do in one body): build
/// the node tree, harvest static positions, reparent out-of-flow boxes onto
/// their containing blocks, run the block/flex/grid solve, then the
/// float-continuation, calc-repair and table-span fixups that re-solve when
/// they adjust geometry. See [`SolvedGeometry`] for why the result is worth
/// caching separately from the collected items.
pub fn layout_solve(
    tree: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    fonts: &FontBook,
    viewport_width: f32,
    viewport_height: f32,
    network_bytes: Option<&HashMap<String, std::sync::Arc<Vec<u8>>>>,
    base_url: Option<&str>,
) -> SolvedGeometry {
    text_src::reset();
    float_lines::reset();
    // The document node is not an element; lay out from the first element
    // descendant (the <html> root), like upstream.
    let root = tree
        .children(tree.document())
        .into_iter()
        .find(|id| tree.with_node(*id, |n| n.is_element()).unwrap_or(false));
    layout_solve_rooted(
        tree,
        styles,
        fonts,
        viewport_width,
        viewport_height,
        network_bytes,
        base_url,
        root,
    )
}

/// Explicit-root variant (fabricated iframe documents): `root` is the
/// orphan subtree's topmost element — the whole downstream build
/// (build_element walk, layout_collect) runs from it, and the ICB is that
/// subtree's own viewport.
#[allow(clippy::too_many_arguments)]
pub fn layout_solve_rooted(
    tree: &DomTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    fonts: &FontBook,
    viewport_width: f32,
    viewport_height: f32,
    network_bytes: Option<&HashMap<String, std::sync::Arc<Vec<u8>>>>,
    base_url: Option<&str>,
    root: Option<NodeId>,
) -> SolvedGeometry {
    let mut taffy_tree = TaffyTree::new();
    let mut node_map: HashMap<diting_taffy::tree::NodeId, NodeId> = HashMap::new();

    // Pre-pass (batches 5b + 6c, parallelized by #112): resolve every img
    // src through the ImageCache — data: URLs decode inline, http(s)/file
    // URLs consult the fetched-byte table; results are cached so repeated
    // srcs and layout re-runs decode once. Unresolvable imgs keep the
    // batch-5a placeholder. #112: the decodes used to run inline in
    // scan_images — serial, one image at a time on the layout thread, and
    // invisible to the layout trace (which times styles/taffy, not this
    // pass) — so an image-heavy page's screenshot wall clock was ~all
    // decode. Collect the page's unique srcs first, decode them across the
    // cores (ImageCache::prefetch), and the walk below only clones cached
    // Arcs.
    let empty: HashMap<String, std::sync::Arc<Vec<u8>>> = HashMap::new();
    let cache = image::ImageCache::with_network(network_bytes.unwrap_or(&empty));
    let mut images: HashMap<NodeId, DecodedImage> = HashMap::new();
    fn collect_img_srcs(
        tree: &DomTree,
        id: NodeId,
        srcs: &mut HashSet<String>,
        viewport_width: f32,
        base_url: Option<&str>,
    ) {
        let is_img = tree
            .with_node(id, |n| n.as_element().map(|e| e.local.to_string() == "img"))
            .flatten()
            .unwrap_or(false);
        if is_img {
            if let Some(src) = resolve_img_source(tree, id, viewport_width, base_url) {
                srcs.insert(src);
            }
        }
        for child in render_children(tree, id) {
            collect_img_srcs(tree, child, srcs, viewport_width, base_url);
        }
    }
    fn scan_images(
        tree: &DomTree,
        id: NodeId,
        cache: &image::ImageCache,
        images: &mut HashMap<NodeId, DecodedImage>,
        viewport_width: f32,
        base_url: Option<&str>,
    ) {
        let is_img = tree
            .with_node(id, |n| n.as_element().map(|e| e.local.to_string() == "img"))
            .flatten()
            .unwrap_or(false);
        if is_img {
            let src = resolve_img_source(tree, id, viewport_width, base_url);
            if let Some(img) = src.as_deref().and_then(|s| cache.resolve(s)) {
                images.insert(id, (*img).clone());
            }
        }
        for child in render_children(tree, id) {
            scan_images(tree, child, cache, images, viewport_width, base_url);
        }
    }
    if let Some(root_id) = &root {
        let mut srcs: HashSet<String> = HashSet::new();
        collect_img_srcs(tree, *root_id, &mut srcs, viewport_width, base_url);
        cache.prefetch(&srcs.into_iter().collect::<Vec<_>>());
        scan_images(
            tree,
            *root_id,
            &cache,
            &mut images,
            viewport_width,
            base_url,
        );
    }

    // Flattened inline wrappers (see build_element) recorded as
    // dom id → hoisted taffy children, for the union pass after the walk.
    let mut flattened: HashMap<NodeId, Vec<diting_taffy::tree::NodeId>> = HashMap::new();
    // Run wrappers (the IFC stand-in flex rows) recorded at assembly for the
    // post-layout baseline-alignment pass (blitz#750 family) — they can't be
    // re-identified structurally (inline-block boxes share the same taffy
    // style), so the builders log them as they create them.
    let mut run_wrappers: Vec<diting_taffy::tree::NodeId> = Vec::new();
    // Table span bookkeeping (rowspan/colspan): build_table logs each spanned
    // table here; the post-layout fixup reads the placeholder geometry and
    // places the lifted absolute cells over their spanned band.
    let mut table_meta = TableBuildMeta::default();
    let Some(root_id) = root else {
        return SolvedGeometry::aborted();
    };
    let Some(root_node) = build_element(
        tree,
        root_id,
        styles,
        &images,
        fonts,
        &mut taffy_tree,
        &mut node_map,
        &mut flattened,
        &mut run_wrappers,
        &mut table_meta,
    ) else {
        return SolvedGeometry::aborted();
    };

    // The initial containing block (obscura#675 lineage fix): CSS anchors a
    // fixed box's insets — and every percentage that bottoms out at the root
    // — to the VIEWPORT, a notional box DISTINCT from the root element's own
    // content-driven box (blitz keeps html at content height too; conflating
    // the two made the cross-check read html=600 vs blitz=140). Wrap the
    // root element in a synthetic definite viewport-sized taffy node; the
    // reparent pass below lands fixed boxes — and ancestor-less absolutes,
    // whose containing block is also the ICB — on it, so taffy's inset math
    // has a real viewport to resolve against. The wrapper has no DOM node,
    // so collect/paint walks keyed by node_map skip it. Children taller
    // than the viewport overflow visibly; taffy doesn't clip.
    let icb_node = taffy_tree
        .new_leaf(Style {
            // Block, not taffy's Flex default: the root element must stay a
            // stretching BLOCK child (a flex item would shrink html to
            // max-content — probe: 500px child → html=500 instead of VW).
            display: Display::Block,
            size: Size {
                width: diting_taffy::style::Dimension::length(viewport_width),
                height: diting_taffy::style::Dimension::length(viewport_height),
            },
            ..Default::default()
        })
        .expect("synthetic ICB node");
    let _ = taffy_tree.add_child(icb_node, root_node);

    let available = Size {
        width: AvailableSpace::Definite(viewport_width),
        height: AvailableSpace::MaxContent,
    };

    let static_pos = harvest_static_positions(
        tree,
        styles,
        fonts,
        &mut taffy_tree,
        &node_map,
        icb_node,
        available,
    );
    reparent_out_of_flow(tree, styles, &mut taffy_tree, &node_map, icb_node, root);
    // Measured-leaf dispatch (batch 3a): pure-text runs carry a TextLeaf
    // context and measure through the FontBook; every other leaf must fall
    // through to taffy's own style-based sizing — the closure fires for ALL
    // childless nodes, so returning HIDDEN here would zero plain leaves
    // (that's exactly what stock compute_layout does below via the same fn).
    let measured = taffy_tree.compute_layout_with_block_measure(
        icb_node,
        available,
        |inputs, id, ctx, style, block| {
            float_lines::measure_node(fonts, inputs, id, ctx, style, block)
        },
    );
    if measured.is_err() {
        return SolvedGeometry::aborted();
    }

    if continue_floats(
        tree,
        styles,
        fonts,
        &mut taffy_tree,
        &node_map,
        icb_node,
        available,
    )
    .is_err()
    {
        return SolvedGeometry::aborted();
    }
    let Ok(mut repaired) = repair_calc_widths(
        styles,
        fonts,
        &mut taffy_tree,
        &node_map,
        icb_node,
        available,
    ) else {
        return SolvedGeometry::aborted();
    };
    if repair_box_sizes(
        styles,
        fonts,
        &mut taffy_tree,
        &node_map,
        icb_node,
        available,
        &mut repaired,
    )
    .is_err()
    {
        return SolvedGeometry::aborted();
    }
    if repair_wrap_host_intrinsics(
        tree,
        styles,
        fonts,
        &mut taffy_tree,
        &node_map,
        icb_node,
        available,
    )
    .is_err()
    {
        return SolvedGeometry::aborted();
    }
    if place_table_spans(fonts, &mut taffy_tree, icb_node, available, &table_meta).is_err() {
        return SolvedGeometry::aborted();
    }
    // Both trees round to the pixel grid inside compute_layout (taffy's
    // use_rounding defaults on; blitz rounds via the same path), so the
    // rect comparisons assume integer edges on both sides.

    // Baseline alignment of inline runs (blitz#750 family): per-line shifts
    // computed against the FINAL layout, applied during the collect walk.
    let baseline_shifts = compute_baseline_shifts(&taffy_tree, &run_wrappers, fonts);
    // Layout is final: the result caches only cost memory from here on.
    taffy_tree.drop_caches();

    SolvedGeometry {
        taffy_tree,
        node_map,
        images,
        static_pos,
        baseline_shifts,
        run_wrappers,
        collapsed_edges: table_meta.collapsed_edges,
        flattened,
        icb_node: Some(icb_node),
        text_src: text_src::take(),
    }
}
