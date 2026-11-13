//! The RGBA paint target and the primitives that write into it.
//!
//! Split from `paint.rs`. `execute` still drives these methods. Pure move.

use super::*;

/// A straight-alpha RGBA8 image, row-major — our paint target.
#[derive(Debug)]
pub struct Canvas {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>,
    /// Active clip stack — exclusive right/bottom, in canvas px. A plain
    /// rect clips by bounds; a rounded entry (batch 7d) additionally cuts
    /// its corner zones with per-corner elliptical radii, so overflow
    /// content inside a `border-radius` + non-visible overflow box follows
    /// the curve like upstream's padding_box_path BezPath clip. An empty
    /// intersection is the degenerate (0,0,0,0): it clips everything.
    pub(super) clip: Vec<ClipShape>,
    /// Open transform-bracket stack (affine batch): entries are TOTAL
    /// local→canvas maps in the same CSS matrix order PaintItem::SetXf
    /// carries, composed at push time — an inner bracket multiplies onto
    /// the current top, an empty stack is the identity. Diagonal transforms
    /// never open a bracket (their geometry pre-bakes at collect time), so
    /// untouched pages keep the exact pre-batch path.
    pub(super) xf_stack: Vec<[f64; 6]>,
}

/// One clip stack entry: an axis-aligned rect, optionally with per-corner
/// radii (CSS order TL TR BR BL) resolved to px.
#[derive(Clone, Copy, Debug)]
pub(super) enum ClipShape {
    Rect(i64, i64, i64, i64),
    Rounded {
        x0: i64,
        y0: i64,
        x1: i64,
        y1: i64,
        radii: [(f32, f32); 4],
    },
    /// A clip whose LOCAL rect maps through an open transform bracket
    /// (affine batch): `inv` is the inverse of the canvas-space total map,
    /// `(x, y, w, h)` + `radii` the local rounded rect, and `(bx0..by1)`
    /// the canvas-space bounds (mapped bbox ∩ prior clips) that
    /// [`Canvas::allowed`] intersects on. A pixel passes when its inverse
    /// image falls inside the local rounded rect — so a rotated
    /// overflow:hidden window cuts child ink along the rotation.
    Affine {
        inv: [f64; 6],
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        radii: [(f32, f32); 4],
        bx0: i64,
        by0: i64,
        bx1: i64,
        by1: i64,
    },
}

impl ClipShape {
    fn bounds(&self) -> (i64, i64, i64, i64) {
        match *self {
            ClipShape::Rect(x0, y0, x1, y1) | ClipShape::Rounded { x0, y0, x1, y1, .. } => {
                (x0, y0, x1, y1)
            }
            ClipShape::Affine {
                bx0, by0, bx1, by1, ..
            } => (bx0, by0, bx1, by1),
        }
    }

    /// Whether pixel center `(cx, cy)` passes this clip shape.
    fn accepts(&self, cx: f64, cy: f64) -> bool {
        match *self {
            ClipShape::Rect(x0, y0, x1, y1) => {
                cx >= x0 as f64 && cx < x1 as f64 && cy >= y0 as f64 && cy < y1 as f64
            }
            ClipShape::Affine {
                inv,
                x,
                y,
                w,
                h,
                radii,
                ..
            } => {
                let (lx, ly) = mat_apply(inv, cx, cy);
                rounded_contains(x, y, w, h, radii, lx, ly)
            }
            ClipShape::Rounded {
                x0,
                y0,
                x1,
                y1,
                radii,
            } => {
                if !(cx >= x0 as f64 && cx < x1 as f64 && cy >= y0 as f64 && cy < y1 as f64) {
                    return false;
                }
                let clamp = |r: (f32, f32), w: i64, h: i64| {
                    (
                        r.0.clamp(0.0, w as f32 / 2.0),
                        r.1.clamp(0.0, h as f32 / 2.0),
                    )
                };
                let (rx0, ry0) = clamp(radii[0], x1 - x0, y1 - y0);
                let (rx1, ry1) = clamp(radii[1], x1 - x0, y1 - y0);
                let (rx2, ry2) = clamp(radii[2], x1 - x0, y1 - y0);
                let (rx3, ry3) = clamp(radii[3], x1 - x0, y1 - y0);
                let centers = [
                    (x0 as f64 + rx0 as f64, y0 as f64 + ry0 as f64),
                    (x1 as f64 - rx1 as f64, y0 as f64 + ry1 as f64),
                    (x1 as f64 - rx2 as f64, y1 as f64 - ry2 as f64),
                    (x0 as f64 + rx3 as f64, y1 as f64 - ry3 as f64),
                ];
                let zone = if cx < centers[0].0 && cy < centers[0].1 {
                    Some(0usize)
                } else if cx > centers[1].0 && cy < centers[1].1 {
                    Some(1)
                } else if cx > centers[2].0 && cy > centers[2].1 {
                    Some(2)
                } else if cx < centers[3].0 && cy > centers[3].1 {
                    Some(3)
                } else {
                    None
                };
                match zone {
                    None => true,
                    Some(i) => {
                        let rxs = [rx0, rx1, rx2, rx3];
                        let rys = [ry0, ry1, ry2, ry3];
                        if rxs[i] <= 0.0 || rys[i] <= 0.0 {
                            true
                        } else {
                            let dx = (cx - centers[i].0) / f64::from(rxs[i]);
                            let dy = (cy - centers[i].1) / f64::from(rys[i]);
                            dx * dx + dy * dy <= 1.0
                        }
                    }
                }
            }
        }
    }
}

impl Canvas {
    /// A canvas pre-filled with an opaque color (the page background stand-in
    /// — this slice models no html/body propagation yet).
    pub fn new_filled(width: usize, height: usize, color: [u8; 4]) -> Self {
        let mut data = vec![0u8; width * height * 4];
        for px in data.as_chunks_mut::<4>().0 {
            px.copy_from_slice(&color);
        }
        Self {
            width,
            height,
            data,
            clip: Vec::new(),
            xf_stack: Vec::new(),
        }
    }

    /// A fully transparent canvas — the LOCAL scratch surface an affine
    /// bracket rasterizes a subtree tile into before blitting it through
    /// the transform (text/svg/replaced ink keeps its own rasterizer, which
    /// only knows how to write axis-aligned integer pixels).
    pub fn new_transparent(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            data: vec![0u8; width * height * 4],
            clip: Vec::new(),
            xf_stack: Vec::new(),
        }
    }

    /// Rows [y0, y1) × cols [x0, x1) the next primitive may touch: the
    /// canvas bounds intersected with the BOUNDS of every active clip
    /// (rounded clips additionally cut pixels per-shape at draw time via
    /// [`Canvas::clip_accepts`]).
    fn allowed(&self) -> (i64, i64, i64, i64) {
        let mut r = (0, 0, self.width as i64, self.height as i64);
        for c in &self.clip {
            let b = c.bounds();
            r = (r.0.max(b.0), r.1.max(b.1), r.2.min(b.2), r.3.min(b.3));
        }
        if r.2 < r.0 || r.3 < r.1 {
            (0, 0, 0, 0)
        } else {
            r
        }
    }

    /// Whether pixel center `(cx, cy)` passes every active clip.
    fn clip_accepts(&self, cx: f64, cy: f64) -> bool {
        self.clip.iter().all(|c| c.accepts(cx, cy))
    }

    /// Pop the innermost clip.
    pub(crate) fn pop_clip(&mut self) {
        self.clip.pop();
    }

    /// Push a clip rect (intersected with the current one).
    pub(crate) fn push_clip(&mut self, x0: i64, y0: i64, x1: i64, y1: i64) {
        let (cx0, cy0, cx1, cy1) = self.allowed();
        let r = (cx0.max(x0), cy0.max(y0), cx1.min(x1), cy1.min(y1));
        self.clip.push(if r.2 < r.0 || r.3 < r.1 {
            ClipShape::Rect(0, 0, 0, 0)
        } else {
            ClipShape::Rect(r.0, r.1, r.2, r.3)
        });
    }

    /// Push a rounded clip: bounds intersect like a rect; the per-corner
    /// radii cut the corner zones at draw time (batch 7d).
    pub(super) fn push_rounded_clip(
        &mut self,
        x0: i64,
        y0: i64,
        x1: i64,
        y1: i64,
        radii: [(f32, f32); 4],
    ) {
        let (cx0, cy0, cx1, cy1) = self.allowed();
        let r = (cx0.max(x0), cy0.max(y0), cx1.min(x1), cy1.min(y1));
        self.clip.push(if r.2 < r.0 || r.3 < r.1 {
            ClipShape::Rect(0, 0, 0, 0)
        } else {
            ClipShape::Rounded {
                x0: r.0,
                y0: r.1,
                x1: r.2,
                y1: r.3,
                radii,
            }
        });
    }

    // --- affine bracket (rotate/skew/matrix paint) ---

    /// The current total local→canvas map, or None while no bracket is
    /// open (the identity — callers route to the axis-aligned primitives).
    pub(super) fn xf(&self) -> Option<[f64; 6]> {
        self.xf_stack.last().copied()
    }

    /// Open a transform bracket: the incoming map composes ONTO the current
    /// top (an empty stack is the identity), so nested brackets multiply —
    /// exactly the collect walk's child_xf chain.
    pub(crate) fn push_xf(&mut self, m: [f64; 6]) {
        let top = match self.xf() {
            Some(t) => mat_mul(t, m),
            None => m,
        };
        self.xf_stack.push(top);
    }

    /// Close the innermost transform bracket.
    pub(crate) fn pop_xf(&mut self) {
        self.xf_stack.pop();
    }

    /// Canvas-space bbox of a LOCAL rect through the current bracket
    /// (identity when none is open) — the affine items' band prefilter.
    pub(super) fn mapped_xf_bounds(&self, x: f64, y: f64, w: f64, h: f64) -> (i64, i64, i64, i64) {
        match self.xf() {
            Some(m) => mapped_bounds(m, x, y, w, h),
            None => (
                x.floor() as i64,
                y.floor() as i64,
                (x + w).ceil() as i64,
                (y + h).ceil() as i64,
            ),
        }
    }

    /// Push a clip whose LOCAL rect maps through the current bracket: the
    /// stack entry stores the inverse map plus the local rounded rect, and
    /// its bounds are the mapped bbox intersected with the prior clips. A
    /// singular map (degenerate scale) clips everything.
    pub(super) fn push_clip_affine(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        radii: [(f32, f32); 4],
    ) {
        let Some(m) = self.xf() else { return };
        let (bx0, by0, bx1, by1) = mapped_bounds(m, x, y, w, h);
        let (cx0, cy0, cx1, cy1) = self.allowed();
        let r = (bx0.max(cx0), by0.max(cy0), bx1.min(cx1), by1.min(cy1));
        match (r.2 > r.0 && r.3 > r.1, mat_inv(m)) {
            (true, Some(inv)) => self.clip.push(ClipShape::Affine {
                inv,
                x,
                y,
                w,
                h,
                radii,
                bx0: r.0,
                by0: r.1,
                bx1: r.2,
                by1: r.3,
            }),
            _ => self.clip.push(ClipShape::Rect(0, 0, 0, 0)),
        }
    }

    /// Source-over fill of a rounded LOCAL rect through the current
    /// bracket: each canvas pixel in the mapped bbox inverse-maps into
    /// local space and takes the same zone/ellipse corner test the
    /// axis-aligned fills use — hard edges, the batch-6b/7c posture
    /// (no antialiased arcs). Integer multiples of 90° are pixel-exact.
    pub(super) fn fill_shape_affine(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        radii: [(f32, f32); 4],
        color: [u8; 4],
    ) {
        let Some(m) = self.xf() else { return };
        let Some(inv) = mat_inv(m) else { return };
        let (bx0, by0, bx1, by1) = mapped_bounds(m, x, y, w, h);
        let (ax0, ay0, ax1, ay1) = self.allowed();
        for gy in by0.max(ay0).max(0)..by1.min(ay1).min(self.height as i64) {
            for gx in bx0.max(ax0).max(0)..bx1.min(ax1).min(self.width as i64) {
                let (cx, cy) = (gx as f64 + 0.5, gy as f64 + 0.5);
                let (lx, ly) = mat_apply(inv, cx, cy);
                if !rounded_contains(x, y, w, h, radii, lx, ly) {
                    continue;
                }
                if !self.clip_accepts(cx, cy) {
                    continue;
                }
                let i = (gy as usize * self.width + gx as usize) * 4;
                over(&mut self.data[i..i + 4], color);
            }
        }
    }

    /// Gradient fill through the current bracket: stop positions project in
    /// LOCAL space (the gradient rides the element's box through the
    /// rotation), colors sample per inverse-mapped pixel center, and the
    /// shape clips by the same local rounded test a solid fill uses.
    #[allow(clippy::too_many_arguments)] // paint plumbing — see run_tokens
    pub(super) fn fill_gradient_affine(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        stops: &[(f32, [u8; 4])],
        css_deg: f32,
        radii: [(f32, f32); 4],
    ) {
        if w <= 0.0 || h <= 0.0 || stops.len() < 2 {
            return;
        }
        let Some(m) = self.xf() else { return };
        let Some(inv) = mat_inv(m) else { return };
        let rad = css_deg.to_radians() as f64;
        let (dux, duy) = (rad.sin(), -rad.cos());
        let len = (w * dux.abs() + h * duy.abs()).max(1.0);
        let (ccx, ccy) = (x + w / 2.0, y + h / 2.0);
        let (bx0, by0, bx1, by1) = mapped_bounds(m, x, y, w, h);
        let (ax0, ay0, ax1, ay1) = self.allowed();
        for gy in by0.max(ay0).max(0)..by1.min(ay1).min(self.height as i64) {
            for gx in bx0.max(ax0).max(0)..bx1.min(ax1).min(self.width as i64) {
                let (cx, cy) = (gx as f64 + 0.5, gy as f64 + 0.5);
                let (lx, ly) = mat_apply(inv, cx, cy);
                if !rounded_contains(x, y, w, h, radii, lx, ly) {
                    continue;
                }
                if !self.clip_accepts(cx, cy) {
                    continue;
                }
                let t = (((lx - ccx) * dux + (ly - ccy) * duy) / len + 0.5).clamp(0.0, 1.0) as f32;
                let color = gradient_stop_color(stops, t);
                let i = (gy as usize * self.width + gx as usize) * 4;
                over(&mut self.data[i..i + 4], color);
            }
        }
    }

    /// Border through the current bracket: pixels inside the outer local
    /// rounded box and NOT inside the widths-inset inner rounded box — the
    /// rotated twin of the axis-aligned ring paint (affine residuals batch;
    /// radii ride the bracket like every other rounded shape).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn border_affine(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        widths: [f32; 4],
        radii: [(f32, f32); 4],
        color: [u8; 4],
    ) {
        let Some(m) = self.xf() else { return };
        let Some(inv) = mat_inv(m) else { return };
        let (bx0, by0, bx1, by1) = mapped_bounds(m, x, y, w, h);
        let (ax0, ay0, ax1, ay1) = self.allowed();
        for gy in by0.max(ay0).max(0)..by1.min(ay1).min(self.height as i64) {
            for gx in bx0.max(ax0).max(0)..bx1.min(ax1).min(self.width as i64) {
                let (cx, cy) = (gx as f64 + 0.5, gy as f64 + 0.5);
                let (lx, ly) = mat_apply(inv, cx, cy);
                if !border_ring_contains(x, y, w, h, widths, radii, lx, ly) {
                    continue;
                }
                if !self.clip_accepts(cx, cy) {
                    continue;
                }
                let i = (gy as usize * self.width + gx as usize) * 4;
                over(&mut self.data[i..i + 4], color);
            }
        }
    }

    /// Axis-aligned rounded border ring: the same outer-minus-inner test
    /// [`border_affine`](Self::border_affine) inverse-maps for, evaluated
    /// directly on canvas pixel centers. Only reached with nonzero radii —
    /// square borders keep the four-band fast path (bit-for-bit the
    /// historical paint).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn fill_border_ring(
        &mut self,
        x: i64,
        y: i64,
        w: i64,
        h: i64,
        widths: [f32; 4],
        radii: [(f32, f32); 4],
        color: [u8; 4],
    ) {
        let (ax0, ay0, ax1, ay1) = self.allowed();
        for gy in y.max(ay0).max(0)..(y + h).min(ay1).min(self.height as i64) {
            for gx in x.max(ax0).max(0)..(x + w).min(ax1).min(self.width as i64) {
                let (cx, cy) = (gx as f64 + 0.5, gy as f64 + 0.5);
                if !border_ring_contains(
                    x as f64, y as f64, w as f64, h as f64, widths, radii, cx, cy,
                ) {
                    continue;
                }
                if !self.clip_accepts(cx, cy) {
                    continue;
                }
                let i = (gy as usize * self.width + gx as usize) * 4;
                over(&mut self.data[i..i + 4], color);
            }
        }
    }

    /// Open a canvas-coordinate bracket ([`PaintItem::SetXfCanvas`]): the
    /// map REPLACES the current top instead of composing onto it, so items
    /// until the matching ClearXf paint in canvas space (the band shift in
    /// a viewport-band paint, the identity in full-page execute), cancelling
    /// any enclosing transform bracket.
    pub(crate) fn push_xf_canvas(&mut self, m: [f64; 6]) {
        self.xf_stack.push(m);
    }

    /// Nearest-neighbor image blit through the current bracket: the
    /// element's LOCAL paint rect maps through the map; each canvas pixel
    /// in the mapped bbox inverse-maps to a local point, which scales into
    /// source texel space exactly like the axis-aligned `blit_image`
    /// (destination-pixel-center sampling).
    pub(super) fn blit_image_affine(
        &mut self,
        image: &crate::image::DecodedImage,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        alpha: f32,
    ) {
        if w <= 0.0 || h <= 0.0 || image.width == 0 || image.height == 0 {
            return;
        }
        let Some(m) = self.xf() else { return };
        let Some(inv) = mat_inv(m) else { return };
        let src = &image.rgba;
        let (sw, sh) = (image.width as f64, image.height as f64);
        let (bx0, by0, bx1, by1) = mapped_bounds(m, x, y, w, h);
        let (ax0, ay0, ax1, ay1) = self.allowed();
        for gy in by0.max(ay0).max(0)..by1.min(ay1).min(self.height as i64) {
            for gx in bx0.max(ax0).max(0)..bx1.min(ax1).min(self.width as i64) {
                let (cx, cy) = (gx as f64 + 0.5, gy as f64 + 0.5);
                let (lx, ly) = mat_apply(inv, cx, cy);
                if !(lx >= x && lx < x + w && ly >= y && ly < y + h) {
                    continue;
                }
                if !self.clip_accepts(cx, cy) {
                    continue;
                }
                let sx = (((lx - x) * sw / w) as i64).clamp(0, image.width as i64 - 1);
                let sy = (((ly - y) * sh / h) as i64).clamp(0, image.height as i64 - 1);
                let i = ((sy as usize * image.width as usize) + sx as usize) * 4;
                let a = if alpha >= 1.0 {
                    src[i + 3]
                } else {
                    (src[i + 3] as f32 * alpha).round() as u8
                };
                let src_px = [src[i], src[i + 1], src[i + 2], a];
                let d = (gy as usize * self.width + gx as usize) * 4;
                over(&mut self.data[d..d + 4], src_px);
            }
        }
    }

    /// Blit a straight-alpha RGBA tile rasterized in LOCAL coordinates
    /// through the current bracket: tile pixel (0, 0) lands at local point
    /// `(ox, oy)`. Nearest sampling; transparent pixels skip — the affine
    /// twin of `blit_text`.
    pub(super) fn blit_rgba_affine(&mut self, src: &[u8], sw: usize, sh: usize, ox: f64, oy: f64) {
        if sw == 0 || sh == 0 {
            return;
        }
        let Some(m) = self.xf() else { return };
        let Some(inv) = mat_inv(m) else { return };
        let (bx0, by0, bx1, by1) = mapped_bounds(m, ox, oy, sw as f64, sh as f64);
        let (ax0, ay0, ax1, ay1) = self.allowed();
        for gy in by0.max(ay0).max(0)..by1.min(ay1).min(self.height as i64) {
            for gx in bx0.max(ax0).max(0)..bx1.min(ax1).min(self.width as i64) {
                let (cx, cy) = (gx as f64 + 0.5, gy as f64 + 0.5);
                let (lx, ly) = mat_apply(inv, cx, cy);
                let (tx, ty) = ((lx - ox) as i64, (ly - oy) as i64);
                if tx < 0 || ty < 0 || tx >= sw as i64 || ty >= sh as i64 {
                    continue;
                }
                if !self.clip_accepts(cx, cy) {
                    continue;
                }
                let i = (ty as usize * sw + tx as usize) * 4;
                let a = src[i + 3];
                if a == 0 {
                    continue;
                }
                let src_px = [src[i], src[i + 1], src[i + 2], a];
                let d = (gy as usize * self.width + gx as usize) * 4;
                over(&mut self.data[d..d + 4], src_px);
            }
        }
    }

    /// Source-over fill of an axis-aligned integer rect, clipped to the
    /// canvas and the clip stack. Opaque colors overwrite, matching how
    /// vello_cpu paints a solid blitz background over whatever is beneath.
    pub fn fill_rect(&mut self, x: i64, y: i64, w: i64, h: i64, color: [u8; 4]) {
        let (ax0, ay0, ax1, ay1) = self.allowed();
        for gy in (y.max(0)).max(ay0)..(y + h).min(self.height as i64).min(ay1) {
            for gx in (x.max(0)).max(ax0)..(x + w).min(self.width as i64).min(ax1) {
                if !self.clip_accepts(gx as f64 + 0.5, gy as f64 + 0.5) {
                    continue;
                }
                let i = (gy as usize * self.width + gx as usize) * 4;
                over(&mut self.data[i..i + 4], color);
            }
        }
    }

    /// Nearest-neighbor blit of an RGBA image scaled into the rect
    /// `(x, y, w, h)` (object-fit: fill), source-over per pixel, clipped to
    /// the canvas and the clip stack. 1:1 sizes are exact texel copies;
    /// scaled output is nearest-neighbor (vello samples bilinearly upstream,
    /// so scaled-image cross-checks compare bbox + sampled interior, not
    /// per-pixel). `alpha` (animation batch A) scales the source alpha —
    /// raster pixels can't fold a group opacity any other way.
    pub fn blit_image(
        &mut self,
        image: &crate::image::DecodedImage,
        x: i64,
        y: i64,
        w: i64,
        h: i64,
        alpha: f32,
    ) {
        if w <= 0 || h <= 0 || image.width == 0 || image.height == 0 {
            return;
        }
        let (ax0, ay0, ax1, ay1) = self.allowed();
        let src = &image.rgba;
        let (sw, sh) = (image.width as i64, image.height as i64);
        for gy in 0..h {
            let ty = y + gy;
            if ty < ay0 || ty >= ay1 || ty < 0 || ty >= self.height as i64 {
                continue;
            }
            // Sample the texel covering this destination row/column's center.
            let sy = (((gy as f64 + 0.5) * sh as f64 / h as f64) as i64).min(sh - 1);
            for gx in 0..w {
                let tx = x + gx;
                if tx < ax0 || tx >= ax1 || tx < 0 || tx >= self.width as i64 {
                    continue;
                }
                if !self.clip_accepts(tx as f64 + 0.5, ty as f64 + 0.5) {
                    continue;
                }
                let sx = (((gx as f64 + 0.5) * sw as f64 / w as f64) as i64).min(sw - 1);
                let i = ((sy * sw + sx) * 4) as usize;
                let a = if alpha >= 1.0 {
                    src[i + 3]
                } else {
                    (src[i + 3] as f32 * alpha).round() as u8
                };
                let src_px = [src[i], src[i + 1], src[i + 2], a];
                let d = (ty as usize * self.width + tx as usize) * 4;
                over(&mut self.data[d..d + 4], src_px);
            }
        }
    }

    /// Source-over fill of a rectangle with per-corner elliptical radii
    /// (batch 7c), clipped to the canvas and the clip stack. Radii are in
    /// CSS corner order (top-left, top-right, bottom-right, bottom-left),
    /// each `(rx, ry)` in px — rx already resolved against the box width
    /// and ry against its height. Each corner radius clamps to half its
    /// own box dimension (the CSS scale-down rule; upstream blitz skips it,
    /// so cross-check cases stay within the legal range). Hard-edged
    /// rasterization: vello antialiases the arc and we don't — cross-checks
    /// sample well inside/outside, never on the curve.
    pub fn fill_corner_rect(
        &mut self,
        x: i64,
        y: i64,
        w: i64,
        h: i64,
        radii: [(f32, f32); 4],
        color: [u8; 4],
    ) {
        if w <= 0 || h <= 0 {
            return;
        }
        let uniform = |r: (f32, f32)| r.0 == radii[0].0 && r.1 == radii[0].1;
        let degenerate = |r: (f32, f32)| r.0 <= 0.0 || r.1 <= 0.0;
        if radii.iter().all(|&r| degenerate(r)) {
            self.fill_rect(x, y, w, h, color);
            return;
        }
        // A fully uniform circular radius takes the batch-6b fast path.
        if radii.iter().all(|&r| uniform(r)) && !degenerate(radii[0]) {
            self.fill_rounded_rect(x, y, w, h, radii[0].0, color);
            return;
        }
        let (ax0, ay0, ax1, ay1) = self.allowed();
        let (fx, fy) = (x as f64, y as f64);
        let (fw, fh) = (w as f64, h as f64);
        // Clamp per-corner: rx ≤ half width, ry ≤ half height.
        let clamp_r = |r: (f32, f32)| {
            (
                r.0.clamp(0.0, w as f32 / 2.0) as f64,
                r.1.clamp(0.0, h as f32 / 2.0) as f64,
            )
        };
        // Corner circle/ellipse centers.
        let centers: [(f64, f64); 4] = [
            (fx + clamp_r(radii[0]).0, fy + clamp_r(radii[0]).1),
            (fx + fw - clamp_r(radii[1]).0, fy + clamp_r(radii[1]).1),
            (fx + fw - clamp_r(radii[2]).0, fy + fh - clamp_r(radii[2]).1),
            (fx + clamp_r(radii[3]).0, fy + fh - clamp_r(radii[3]).1),
        ];
        for gy in y.max(0).max(ay0)..(y + h).min(self.height as i64).min(ay1) {
            for gx in x.max(0).max(ax0)..(x + w).min(self.width as i64).min(ax1) {
                let (cx, cy) = (gx as f64 + 0.5, gy as f64 + 0.5);
                // Which corner zone does the pixel sit in (edge band × edge
                // band)? The cross-shaped middle is always inside; a corner
                // zone tests the (dx/rx)² + (dy/ry)² ≤ 1 ellipse equation.
                let zone = if cx < centers[0].0 && cy < centers[0].1 {
                    Some(0usize)
                } else if cx > centers[1].0 && cy < centers[1].1 {
                    Some(1)
                } else if cx > centers[2].0 && cy > centers[2].1 {
                    Some(2)
                } else if cx < centers[3].0 && cy > centers[3].1 {
                    Some(3)
                } else {
                    None
                };
                let inside = match zone {
                    None => true,
                    Some(i) => {
                        let (rx, ry) = clamp_r(radii[i]);
                        if rx <= 0.0 || ry <= 0.0 {
                            true
                        } else {
                            let (ccx, ccy) = centers[i];
                            let dx = (cx - ccx) / rx;
                            let dy = (cy - ccy) / ry;
                            dx * dx + dy * dy <= 1.0
                        }
                    }
                };
                if !inside {
                    continue;
                }
                let i = (gy as usize * self.width + gx as usize) * 4;
                over(&mut self.data[i..i + 4], color);
            }
        }
    }

    /// Source-over fill of a `linear-gradient(...)` box (gradient batch):
    /// each pixel's stop position is its projection onto the CSS gradient
    /// line — direction `(sin θ, −cos θ)` in screen space (y grows down,
    /// css_deg 0 = to top, clockwise), line length `|w·sinθ| + |h·cosθ|`,
    /// position `dot(p − center, dir)/len + 0.5` clamped to [0,1]. Stop
    /// colors interpolate in premultiplied space; the shape clips through
    /// the same per-corner elliptical test a solid `BgCorner` uses (the
    /// fill follows the rounded box).
    #[allow(clippy::too_many_arguments)] // paint plumbing — see run_tokens
    pub fn fill_gradient(
        &mut self,
        x: i64,
        y: i64,
        w: i64,
        h: i64,
        stops: &[(f32, [u8; 4])],
        css_deg: f32,
        radii: [(f32, f32); 4],
    ) {
        if w <= 0 || h <= 0 || stops.len() < 2 {
            return;
        }
        let (ax0, ay0, ax1, ay1) = self.allowed();
        let rad = css_deg.to_radians() as f64;
        let (dux, duy) = (rad.sin(), -rad.cos());
        let len = (w as f64 * dux.abs() + h as f64 * duy.abs()).max(1.0);
        let (ccx, ccy) = (x as f64 + w as f64 / 2.0, y as f64 + h as f64 / 2.0);
        // Row-invariant part of the position: hoisted so the inner loop is
        // one fused multiply-add per pixel.
        let col_a = dux / len;
        // Degenerate corners (either radius ≤ 0) don't round; only build the
        // clip shape when at least one corner is live.
        let rounded = radii.iter().any(|r| r.0 > 0.0 && r.1 > 0.0);
        let shape = rounded.then_some(ClipShape::Rounded {
            x0: x,
            y0: y,
            x1: x + w,
            y1: y + h,
            radii,
        });
        for gy in y.max(0).max(ay0)..(y + h).min(self.height as i64).min(ay1) {
            let row_base = (gy as f64 + 0.5 - ccy) * duy / len + 0.5;
            for gx in x.max(0).max(ax0)..(x + w).min(self.width as i64).min(ax1) {
                let (cx, cy) = (gx as f64 + 0.5, gy as f64 + 0.5);
                if let Some(s) = &shape {
                    if !s.accepts(cx, cy) {
                        continue;
                    }
                }
                let t = ((cx - ccx) * col_a + row_base).clamp(0.0, 1.0) as f32;
                let color = gradient_stop_color(stops, t);
                let i = (gy as usize * self.width + gx as usize) * 4;
                over(&mut self.data[i..i + 4], color);
            }
        }
    }

    /// Source-over fill of a rounded rectangle (batch 6b), clipped to the
    /// canvas and the clip stack. `radius` is the uniform circular corner
    /// radius in px (clamped to half the shorter side — the CSS scale-down
    /// rule; upstream blitz skips it and distorts past half, so cross-check
    /// cases stay within the legal range). Hard-edged rasterization: the
    /// corner boundary is a step function, while vello antialiases it —
    /// cross-checks sample well inside/outside, never on the arc.
    pub fn fill_rounded_rect(
        &mut self,
        x: i64,
        y: i64,
        w: i64,
        h: i64,
        radius: f32,
        color: [u8; 4],
    ) {
        if w <= 0 || h <= 0 {
            return;
        }
        let r = radius.clamp(0.0, (w.min(h) as f32) / 2.0);
        if r <= 0.0 {
            self.fill_rect(x, y, w, h, color);
            return;
        }
        let (ax0, ay0, ax1, ay1) = self.allowed();
        // Pixel centers (px+0.5) test against the quarter-circle arcs.
        let (fx, fy) = (x as f64, y as f64);
        let (fw, fh) = (w as f64, h as f64);
        let fr = r as f64;
        for gy in y.max(0).max(ay0)..(y + h).min(self.height as i64).min(ay1) {
            for gx in x.max(0).max(ax0)..(x + w).min(self.width as i64).min(ax1) {
                let (cx, cy) = (gx as f64 + 0.5, gy as f64 + 0.5);
                // A pixel is outside only when it sits in a corner zone
                // (edge band × edge band) beyond its quarter-circle arc;
                // the cross-shaped middle zone is always inside.
                let (x_left, x_right) = (cx < fx + fr, cx >= fx + fw - fr);
                let (y_top, y_bottom) = (cy < fy + fr, cy >= fy + fh - fr);
                let corner = if x_left && y_top {
                    Some((fx + fr, fy + fr))
                } else if x_right && y_top {
                    Some((fx + fw - fr, fy + fr))
                } else if x_right && y_bottom {
                    Some((fx + fw - fr, fy + fh - fr))
                } else if x_left && y_bottom {
                    Some((fx + fr, fy + fh - fr))
                } else {
                    None
                };
                let inside = match corner {
                    Some((ccx, ccy)) => {
                        let (dx, dy) = (cx - ccx, cy - ccy);
                        dx * dx + dy * dy <= fr * fr
                    }
                    None => true,
                };
                if !inside {
                    continue;
                }
                let i = (gy as usize * self.width + gx as usize) * 4;
                over(&mut self.data[i..i + 4], color);
            }
        }
    }

    /// backdrop-filter: blur() (blitz#901 family). Snapshot the canvas
    /// region under the box (padded by the blur's support so the kernel
    /// has input OUTSIDE the box), separable box-blur it in premultiplied
    /// RGBA, and write the result back ONLY inside the rounded border
    /// shape — the blur may sample outside the box, the output may not.
    pub(super) fn blur_backdrop(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radii: [(f32, f32); 4],
        blur: f32,
    ) {
        if w <= 0.0 || h <= 0.0 || self.width == 0 || self.height == 0 {
            return;
        }
        let pad = blur.ceil() as i64;
        let radius = ((blur * 0.5).round() as usize).max(1);
        let x0 = (x.floor() as i64 - pad).max(0);
        let y0 = (y.floor() as i64 - pad).max(0);
        let x1 = ((x + w).ceil() as i64 + pad).min(self.width as i64);
        let y1 = ((y + h).ceil() as i64 + pad).min(self.height as i64);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let rw = (x1 - x0) as usize;
        let rh = (y1 - y0) as usize;
        // Deinterleave the snapshot, then premultiply RGB by A so bright
        // pixels can't bleed across transparent ones.
        let mut planes: [Vec<u8>; 4] = [
            vec![0u8; rw * rh],
            vec![0u8; rw * rh],
            vec![0u8; rw * rh],
            vec![0u8; rw * rh],
        ];
        for (row, plane_rows) in planes.iter_mut().enumerate() {
            for gy in 0..rh {
                let src = ((y0 + gy as i64) as usize * self.width + x0 as usize) * 4;
                let dst = gy * rw;
                for gx in 0..rw {
                    plane_rows[dst + gx] = self.data[src + gx * 4 + row];
                }
            }
        }
        let alphas = planes[3].clone();
        for plane in planes.iter_mut().take(3) {
            for (v, a) in plane.iter_mut().zip(&alphas) {
                *v = ((*v as u32 * *a as u32 + 127) / 255) as u8;
            }
        }
        // One separable pass each way — the same two-round box kernel the
        // text-shadow feather uses (support ≈ blur).
        for plane in planes.iter_mut() {
            *plane = box_blur_alpha(plane, rw, rh, radius, true);
            *plane = box_blur_alpha(plane, rw, rh, radius, false);
        }
        // Composite: backdrop pixels inside the rounded shape are REPLACED
        // with their blur (blending blurred-over-sharp would double-count);
        // outside the shape the original stays crisp.
        let (bx, by) = (x as f64 + w as f64 / 2.0, y as f64 + h as f64 / 2.0);
        let (hw, hh) = (w as f64 / 2.0, h as f64 / 2.0);
        let rx0 = (x.floor() as i64).max(x0).max(0);
        let ry0 = (y.floor() as i64).max(y0).max(0);
        let ry1 = ((y + h).ceil() as i64).min(y1);
        let rx1 = ((x + w).ceil() as i64).min(x1);
        for gy in ry0..ry1 {
            for gx in rx0..rx1 {
                let (cx, cy) = (gx as f64 + 0.5, gy as f64 + 0.5);
                if !self.clip_accepts(cx, cy) {
                    continue;
                }
                let (px, py) = (cx - bx, cy - by);
                let r = shadow_corner_radius(radii, px, py).min(hw).min(hh);
                if sd_rounded_box(px, py, hw - r, hh - r, r) > 0.0 {
                    continue;
                }
                let (col, row) = ((gx - x0) as usize, (gy - y0) as usize);
                let av = planes[3][row * rw + col] as u32;
                let i = (gy as usize * self.width + gx as usize) * 4;
                for (c, plane) in planes.iter().take(3).enumerate() {
                    let q = (plane[row * rw + col] as u32) * 255 + av / 2;
                    self.data[i + c] = q.checked_div(av).map_or(0, |v| v.min(255) as u8);
                }
                self.data[i + 3] = av as u8;
            }
        }
    }

    /// Outer box-shadow (blitz#349 family, v1): per-pixel SDF around the
    /// offset/spread-inflated shadow box, the element's own border box
    /// knocked out (a transparent background must not show the shadow
    /// inside it — Chrome semantics). The feather is a linear falloff over
    /// [0, blur]: the same visible extent as Chrome's gaussian σ=blur/2,
    /// cheaper per pixel, no blur taps.
    ///
    /// The inset twin (`inset: true`, v2): the shadow box is the element
    /// box offset by (dx, dy) and SHRUNK by a positive spread; ink fills
    /// from its edge outward, fading to nothing `blur` px INTO the box
    /// (`alpha = 1 + d/blur` for the signed distance d), everything is
    /// hard-clipped to the element box, and a fully collapsed shadow box
    /// (spread ≥ half-dim) legally shadows the whole element.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn fill_box_shadow(
        &mut self,
        rect: &Rect,
        color: [u8; 4],
        radii: [(f32, f32); 4],
        dx: f32,
        dy: f32,
        blur: f32,
        spread: f32,
        inset: bool,
    ) {
        if rect.width <= 0.0 || rect.height <= 0.0 || color[3] == 0 {
            return;
        }
        let ehw = rect.width as f64 / 2.0;
        let ehh = rect.height as f64 / 2.0;
        let (ecx, ecy) = (rect.x as f64 + ehw, rect.y as f64 + ehh);
        let (scx, scy) = (ecx + dx as f64, ecy + dy as f64);
        let feather = blur.max(0.0) as f64;
        let (shw, shh) = if inset {
            // A collapsed shadow box is legal: clamp to a point and the
            // whole element ends up outside it (fully shadowed).
            (
                (ehw - spread as f64).max(0.0),
                (ehh - spread as f64).max(0.0),
            )
        } else {
            (ehw + spread as f64, ehh + spread as f64)
        };
        if !inset && (shw <= 0.0 || shh <= 0.0) {
            return;
        }
        // Outer bounds hug the feather-inflated shadow box; inset ink is
        // hard-clipped to the element box (its feather lives inside it),
        // so the loop never leaves the element.
        let (bx0, by0, bx1, by1) = if inset {
            (ecx - ehw, ecy - ehh, ecx + ehw, ecy + ehh)
        } else {
            let pad = feather + 1.0;
            (
                scx - shw - pad,
                scy - shh - pad,
                scx + shw + pad,
                scy + shh + pad,
            )
        };
        let (ax0, ay0, ax1, ay1) = self.allowed();
        let x0 = (bx0.floor() as i64).max(ax0).max(0);
        let y0 = (by0.floor() as i64).max(ay0).max(0);
        let x1 = (bx1.ceil() as i64).min(ax1).min(self.width as i64);
        let y1 = (by1.ceil() as i64).min(ay1).min(self.height as i64);
        for gy in y0..y1 {
            for gx in x0..x1 {
                let (cx, cy) = (gx as f64 + 0.5, gy as f64 + 0.5);
                let (qx, qy) = (cx - ecx, cy - ecy);
                let er = shadow_corner_radius(radii, qx, qy).min(ehw.min(ehh));
                let outside = sd_rounded_box(qx, qy, ehw - er, ehh - er, er) >= 0.0;
                // Outer shadows knock the element interior out; inset ink
                // never crosses the element edge — the two skip opposite
                // sides of the same test.
                if outside == inset {
                    continue;
                }
                let (px, py) = (cx - scx, cy - scy);
                let sr = (shadow_corner_radius(radii, px, py)
                    + if inset { -spread as f64 } else { spread as f64 })
                .clamp(0.0, shw.min(shh));
                let d = sd_rounded_box(px, py, shw - sr, shh - sr, sr);
                let a = if feather > 0.0 {
                    if inset {
                        (1.0 + d / feather).clamp(0.0, 1.0)
                    } else {
                        (1.0 - d / feather).clamp(0.0, 1.0)
                    }
                } else if inset == (d >= 0.0) {
                    1.0
                } else {
                    0.0
                };
                if a <= 0.0 {
                    continue;
                }
                let mut src = color;
                src[3] = ((color[3] as f64 * a).round() as i64).clamp(0, 255) as u8;
                if src[3] == 0 {
                    continue;
                }
                let i = (gy as usize * self.width + gx as usize) * 4;
                over(&mut self.data[i..i + 4], src);
            }
        }
    }

    /// The affine twin: the shadow box lives in LOCAL space (the offset
    /// rides the element's transform), the loop bounds are the
    /// feather-inflated local box mapped through the bracket, and every
    /// canvas pixel inverse-maps into local space for the same SDF test.
    /// Inset mirrors the canvas twin: shadow box shrunk by spread, ink
    /// hard-clipped to the local element box, feather falling inward.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn fill_shadow_affine(
        &mut self,
        rect: &Rect,
        color: [u8; 4],
        radii: [(f32, f32); 4],
        dx: f32,
        dy: f32,
        blur: f32,
        spread: f32,
        inset: bool,
    ) {
        if rect.width <= 0.0 || rect.height <= 0.0 || color[3] == 0 {
            return;
        }
        let Some(m) = self.xf() else { return };
        let Some(inv) = mat_inv(m) else { return };
        let ehw = rect.width as f64 / 2.0;
        let ehh = rect.height as f64 / 2.0;
        let (ecx, ecy) = (rect.x as f64 + ehw, rect.y as f64 + ehh);
        let (scx, scy) = (ecx + dx as f64, ecy + dy as f64);
        let feather = blur.max(0.0) as f64;
        let (shw, shh) = if inset {
            (
                (ehw - spread as f64).max(0.0),
                (ehh - spread as f64).max(0.0),
            )
        } else {
            (ehw + spread as f64, ehh + spread as f64)
        };
        if !inset && (shw <= 0.0 || shh <= 0.0) {
            return;
        }
        let (lx0, ly0, lx1, ly1) = if inset {
            (ecx - ehw, ecy - ehh, ecx + ehw, ecy + ehh)
        } else {
            let pad = feather + 1.0;
            (
                scx - shw - pad,
                scy - shh - pad,
                scx + shw + pad,
                scy + shh + pad,
            )
        };
        let (bx0, by0, bx1, by1) = mapped_bounds(m, lx0, ly0, lx1 - lx0, ly1 - ly0);
        let (ax0, ay0, ax1, ay1) = self.allowed();
        for gy in by0.max(ay0).max(0)..by1.min(ay1).min(self.height as i64) {
            for gx in bx0.max(ax0).max(0)..bx1.min(ax1).min(self.width as i64) {
                let (cx, cy) = (gx as f64 + 0.5, gy as f64 + 0.5);
                let (lx, ly) = mat_apply(inv, cx, cy);
                let (qx, qy) = (lx - ecx, ly - ecy);
                let er = shadow_corner_radius(radii, qx, qy).min(ehw.min(ehh));
                let outside = sd_rounded_box(qx, qy, ehw - er, ehh - er, er) >= 0.0;
                if outside == inset {
                    continue;
                }
                let (px, py) = (lx - scx, ly - scy);
                let sr = (shadow_corner_radius(radii, px, py)
                    + if inset { -spread as f64 } else { spread as f64 })
                .clamp(0.0, shw.min(shh));
                let d = sd_rounded_box(px, py, shw - sr, shh - sr, sr);
                let a = if feather > 0.0 {
                    if inset {
                        (1.0 + d / feather).clamp(0.0, 1.0)
                    } else {
                        (1.0 - d / feather).clamp(0.0, 1.0)
                    }
                } else if inset == (d >= 0.0) {
                    1.0
                } else {
                    0.0
                };
                if a <= 0.0 || !self.clip_accepts(cx, cy) {
                    continue;
                }
                let mut src = color;
                src[3] = ((color[3] as f64 * a).round() as i64).clamp(0, 255) as u8;
                if src[3] == 0 {
                    continue;
                }
                let i = (gy as usize * self.width + gx as usize) * 4;
                over(&mut self.data[i..i + 4], src);
            }
        }
    }

    /// Source-over composite of a text tile's straight-alpha pixels at
    /// integer offset `(x, y)`, clipped to the canvas and the clip stack.
    pub fn blit_text(&mut self, r: &TextRaster, x: i64, y: i64) {
        if r.width == 0 || r.height == 0 {
            return;
        }
        let (ax0, ay0, ax1, ay1) = self.allowed();
        for gy in 0..r.height as i64 {
            let ty = y + gy;
            if ty < ay0 || ty >= ay1 || ty < 0 || ty >= self.height as i64 {
                continue;
            }
            for gx in 0..r.width as i64 {
                let tx = x + gx;
                if tx < ax0 || tx >= ax1 || tx < 0 || tx >= self.width as i64 {
                    continue;
                }
                if !self.clip_accepts(tx as f64 + 0.5, ty as f64 + 0.5) {
                    continue;
                }
                let a = r.data[(gy as usize * r.width + gx as usize) * 4 + 3];
                if a == 0 {
                    continue;
                }
                let src = [
                    r.data[(gy as usize * r.width + gx as usize) * 4],
                    r.data[(gy as usize * r.width + gx as usize) * 4 + 1],
                    r.data[(gy as usize * r.width + gx as usize) * 4 + 2],
                    a,
                ];
                let i = (ty as usize * self.width + tx as usize) * 4;
                over(&mut self.data[i..i + 4], src);
            }
        }
    }
}

/// Straight-alpha source-over onto one destination pixel.
fn over(dst: &mut [u8], src: [u8; 4]) {
    let a = src[3] as u32;
    if a == 255 {
        dst[..4].copy_from_slice(&src);
        return;
    }
    for c in 0..3 {
        dst[c] = ((src[c] as u32 * a + dst[c] as u32 * (255 - a)) / 255) as u8;
    }
    dst[3] = 255.min(dst[3] as u32 + a * (255 - dst[3] as u32) / 255) as u8;
}

// --- 2D affine math (affine batch) — CSS matrix order throughout:
// [a, b, c, d, e, f] maps x' = a·x + c·y + e, y' = b·x + d·y + f. ---

/// Matrix product m∘n (apply n first, then m) in the CSS column convention —
/// the same composition the collect walk's Xf::compose and the parse-time
/// function-list accumulation use, in f64 for raster-side exactness.
fn mat_mul(m: [f64; 6], n: [f64; 6]) -> [f64; 6] {
    [
        m[0] * n[0] + m[2] * n[1],
        m[1] * n[0] + m[3] * n[1],
        m[0] * n[2] + m[2] * n[3],
        m[1] * n[2] + m[3] * n[3],
        m[0] * n[4] + m[2] * n[5] + m[4],
        m[1] * n[4] + m[3] * n[5] + m[5],
    ]
}

/// Apply the map to a point.
fn mat_apply(m: [f64; 6], x: f64, y: f64) -> (f64, f64) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

/// Inverse of the map, or None when the linear part is singular (a
/// degenerate scale collapses the plane to a line — nothing to rasterize).
fn mat_inv(m: [f64; 6]) -> Option<[f64; 6]> {
    let det = m[0] * m[3] - m[1] * m[2];
    if det.abs() < 1e-9 {
        return None;
    }
    let ia = m[3] / det;
    let ib = -m[1] / det;
    let ic = -m[2] / det;
    let id = m[0] / det;
    Some([
        ia,
        ib,
        ic,
        id,
        -(ia * m[4] + ic * m[5]),
        -(ib * m[4] + id * m[5]),
    ])
}

/// Canvas-space integer bbox of a local rect mapped through `m`: the four
/// mapped corners' union, floored/ceiled outward — a conservative superset
/// of the true mapped area (fill loops re-test every pixel anyway).
fn mapped_bounds(m: [f64; 6], x: f64, y: f64, w: f64, h: f64) -> (i64, i64, i64, i64) {
    let (x0, y0) = mat_apply(m, x, y);
    let (x1, y1) = mat_apply(m, x + w, y);
    let (x2, y2) = mat_apply(m, x, y + h);
    let (x3, y3) = mat_apply(m, x + w, y + h);
    let xs = [x0, x1, x2, x3];
    let ys = [y0, y1, y2, y3];
    (
        xs.iter().copied().fold(f64::MAX, f64::min).floor() as i64,
        ys.iter().copied().fold(f64::MAX, f64::min).floor() as i64,
        xs.iter().copied().fold(f64::MIN, f64::max).ceil() as i64,
        ys.iter().copied().fold(f64::MIN, f64::max).ceil() as i64,
    )
}

/// LOCAL-space rounded-rect containment (the affine twin of
/// [`ClipShape::Rounded::accepts`]): a pixel's inverse image lands in local
/// coordinates, where the same zone/ellipse corner test runs against the
/// rect's own radii (clamped to half the box per the CSS scale-down rule).
/// Zero radii degenerate to the plain rect test.
fn rounded_contains(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    radii: [(f32, f32); 4],
    lx: f64,
    ly: f64,
) -> bool {
    if !(lx >= x && lx < x + w && ly >= y && ly < y + h) {
        return false;
    }
    let clamp = |r: (f32, f32)| {
        (
            r.0.clamp(0.0, w as f32 / 2.0) as f64,
            r.1.clamp(0.0, h as f32 / 2.0) as f64,
        )
    };
    let (rx0, ry0) = clamp(radii[0]);
    let (rx1, ry1) = clamp(radii[1]);
    let (rx2, ry2) = clamp(radii[2]);
    let (rx3, ry3) = clamp(radii[3]);
    let centers = [
        (x + rx0, y + ry0),
        (x + w - rx1, y + ry1),
        (x + w - rx2, y + h - ry2),
        (x + rx3, y + h - ry3),
    ];
    let zone = if lx < centers[0].0 && ly < centers[0].1 {
        Some(0usize)
    } else if lx > centers[1].0 && ly < centers[1].1 {
        Some(1)
    } else if lx > centers[2].0 && ly > centers[2].1 {
        Some(2)
    } else if lx < centers[3].0 && ly > centers[3].1 {
        Some(3)
    } else {
        None
    };
    match zone {
        None => true,
        Some(i) => {
            let rxs = [rx0, rx1, rx2, rx3];
            let rys = [ry0, ry1, ry2, ry3];
            if rxs[i] <= 0.0 || rys[i] <= 0.0 {
                true
            } else {
                let dx = (lx - centers[i].0) / rxs[i];
                let dy = (ly - centers[i].1) / rys[i];
                dx * dx + dy * dy <= 1.0
            }
        }
    }
}

/// Rounded-box signed distance (iq's formula, box-shadow v1): negative
/// inside, zero on the edge, the distance in px outside. `b` is the
/// half-extent MINUS the corner radius, `r` the radius.
pub(super) fn sd_rounded_box(px: f64, py: f64, bx: f64, by: f64, r: f64) -> f64 {
    let qx = px.abs() - bx;
    let qy = py.abs() - by;
    let (ox, oy) = (qx.max(0.0), qy.max(0.0));
    (ox * ox + oy * oy).sqrt() + qx.max(qy).min(0.0) - r
}

/// Per-quadrant corner radius (CSS order TL TR BR BL) as the mean of the
/// corner's (rx, ry): the SDF is circular, so an elliptical corner
/// approximates to its mean — uniform radii (the overwhelming case) are
/// exact. Quadrants read off the point's sign relative to the box center.
fn shadow_corner_radius(radii: [(f32, f32); 4], qx: f64, qy: f64) -> f64 {
    let i = if qx < 0.0 && qy < 0.0 {
        0
    } else if qx >= 0.0 && qy < 0.0 {
        1
    } else if qx >= 0.0 && qy >= 0.0 {
        2
    } else {
        3
    };
    (radii[i].0 + radii[i].1) as f64 / 2.0
}

/// LOCAL-space border-ring containment (affine residuals batch): inside the
/// outer rounded box AND outside the widths-inset inner rounded box — the
/// CSS border shape. Inner radii shrink by the adjacent border widths per
/// corner (TL: left/top, TR: right/top, …), floored at 0 — the corner-box
/// approximation every raster browser uses. A degenerate inner box (the
/// border thicker than the box on an axis) leaves a solid fill. Zero radii
/// degenerate to the plain square ring.
#[allow(clippy::too_many_arguments)]
fn border_ring_contains(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    widths: [f32; 4],
    radii: [(f32, f32); 4],
    lx: f64,
    ly: f64,
) -> bool {
    let [t, r, b, l] = widths;
    if !rounded_contains(x, y, w, h, radii, lx, ly) {
        return false;
    }
    let (ix, iy) = (x + l as f64, y + t as f64);
    let (iw, ih) = (
        (w - l as f64 - r as f64).max(0.0),
        (h - t as f64 - b as f64).max(0.0),
    );
    if iw <= 0.0 || ih <= 0.0 {
        return true;
    }
    let inner_radii = [
        ((radii[0].0 - l).max(0.0), (radii[0].1 - t).max(0.0)),
        ((radii[1].0 - r).max(0.0), (radii[1].1 - t).max(0.0)),
        ((radii[2].0 - r).max(0.0), (radii[2].1 - b).max(0.0)),
        ((radii[3].0 - l).max(0.0), (radii[3].1 - b).max(0.0)),
    ];
    !rounded_contains(ix, iy, iw, ih, inner_radii, lx, ly)
}

/// Gradient stop color at position `t` (0..1). Stops are ascending; `t`
/// outside the list clamps to the end stops. The ramp scan uses a strict
/// upper bound, so a shared position (hard line, `blue 50%, green 50%`)
/// falls through to the next ramp and the LATER stop's color owns the
/// line itself — Chrome's behavior at the discontinuity.
pub(super) fn gradient_stop_color(stops: &[(f32, [u8; 4])], t: f32) -> [u8; 4] {
    if t <= stops[0].0 {
        return stops[0].1;
    }
    let last = stops.len() - 1;
    if t >= stops[last].0 {
        return stops[last].1;
    }
    for w in stops.windows(2) {
        let (p0, c0) = w[0];
        let (p1, c1) = w[1];
        if t < p1 {
            // Windows tile [p0, last) and the outer clamps pin t >= p0,
            // so the selected window always has span > 0.
            let k = (t - p0) / (p1 - p0);
            return lerp_premultiplied(c0, c1, k);
        }
    }
    stops[last].1
}

/// Interpolate two straight-alpha colors at `k` in premultiplied space:
/// premultiply both endpoints, lerp, un-premultiply by the lerped alpha.
/// A straight lerp of rgba() midpoints drags transparent colors toward
/// gray (rgba(255,0,0,0)→rgba(0,0,255,255) at 0.5 would come out purple).
pub(super) fn lerp_premultiplied(a: [u8; 4], b: [u8; 4], k: f32) -> [u8; 4] {
    let out_a = a[3] as f32 + (b[3] as f32 - a[3] as f32) * k;
    if out_a <= 0.0 {
        return [0, 0, 0, 0];
    }
    let ch = |x: u8, y: u8| {
        let px = x as f32 * a[3] as f32 / 255.0;
        let py = y as f32 * b[3] as f32 / 255.0;
        ((px + (py - px) * k) * 255.0 / out_a)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    [
        ch(a[0], b[0]),
        ch(a[1], b[1]),
        ch(a[2], b[2]),
        out_a.round().clamp(0.0, 255.0) as u8,
    ]
}
