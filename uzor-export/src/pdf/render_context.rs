//! [`PdfRenderContext`] — a [`RenderContext`] backend that appends REAL PDF
//! content-stream operators to a page's own draw-op list, instead of
//! rasterizing (`uzor-render-tiny-skia`) or serializing SVG markup
//! (`uzor-render-svg`). Typography-gap WAVE 1: figures/tables/page chrome
//! render THROUGH this backend directly into the PDF page they belong to
//! — no intermediate whole-page raster.
//!
//! ## Design: bake transforms, defer only what PDF genuinely defers
//!
//! Same "bake transforms into coordinates, don't emit a nested operator per
//! `save`/`translate`/`scale`" discipline [`crate::pdf::render_context`]'s
//! sibling `uzor-render-svg::SvgRenderContext` already uses (see that
//! crate's own module doc for the "why"): every path/text/image position
//! this backend records is already the FINAL, absolute page-point
//! coordinate (in this crate's own top-left-origin, y-down convention —
//! same as [`super::PdfTextRun`]/[`super::PdfPageSpec`] already use), with
//! [`Transform`] (a private, self-contained 2D affine — the SAME matrix
//! shape `uzor-render-svg::transform::Transform` uses, reimplemented here
//! rather than reached across a crate boundary for an internal type) baked
//! in at `move_to`/`line_to`/`fill_text`/etc. call time.
//!
//! The ONE thing this backend genuinely CANNOT resolve at draw-call time:
//! [`super::pdf::mod`]'s font subsetting only knows a font's FULL used-glyph
//! set once every page in the whole document has been added ([`super::
//! subset::build_font_data`]) — so a [`PdfOp::Text`] carries its glyphs
//! already resolved to `(original gid, source char)` pairs (via
//! [`super::PdfBuilder::font_metrics`], the SAME per-font [`super::ttf::
//! TtfMetrics`] `pdf_adapter.rs`'s own paragraph-run resolution already
//! uses), and the gid -> CID remap happens later, at
//! [`super::PdfBuilder::finish`] time, exactly like every other text run
//! in this crate.
//!
//! ## PDF's bottom-up Y axis
//!
//! This backend's own accumulated ops stay in TOP-LEFT, y-down canvas
//! space (the SAME convention every other coordinate in this crate uses) —
//! the y-axis flip into PDF's native bottom-left, y-up space happens in
//! EXACTLY ONE place, [`emit_ops`] (called from [`super::write_page`]),
//! applying `y' = page_height_pt - y` to every emitted y-coordinate
//! uniformly (paths, text anchors, image rects, gradient axes) — the SAME
//! flip formula `super::write_page`'s own pre-existing `PdfTextRun` path
//! already applies, just now shared by every content kind instead of text
//! runs alone. This is a per-point translation-composed-with-reflection,
//! not a handedness-flipping transform on shape geometry: an existing,
//! visually-verified convention (see this crate's own Cyrillic/table/list
//! proof PDFs), not new territory.
//!
//! ## What's real vs. documented scope limits
//!
//! - Paths/rects/circles/lines/beziers, fills, strokes (dash/cap/join),
//!   clipping (`W n`, `q`/`Q`-scoped) — full, real PDF operators.
//! - Text — real Type0/CID `Tj` runs through the SAME font machinery
//!   `pdf_adapter.rs`'s paragraph runs use (see [`super::PdfFontCache`] for
//!   the shared registry both sides resolve through).
//! - Alpha (`Painter::set_global_alpha`) — a real `/ExtGState` (`ca`/`CA`)
//!   object per distinct alpha value, invoked via `gs` before EVERY
//!   paint op (never relying on state carried over from a previous op —
//!   see [`resolve_alpha_gstate_name`]'s own doc comment).
//! - Images (`ImagePainter::draw_image_rgba`) — a real Image XObject
//!   (`DeviceRGB`, `+/SMask` when the source data carries real alpha),
//!   placed via its own `cm`/`Do` pair. Embedded at the SOURCE data's own
//!   pixel resolution (no artificial upsampling) — there is no "whole-page
//!   raster" concept left to inherit a `2x` factor FROM; a real image
//!   block's own native pixels are what get embedded.
//! - Linear/radial gradients (`GradientPainter`) — a REAL PDF shading
//!   pattern (`/PatternType 2`, `/ShadingType 2` or `3`), built from a
//!   `StitchingFunction` over per-segment `ExponentialFunction`s — not the
//!   trait's own flat-fill fallback. **Scope limit, documented, not a
//!   silent drop**: PDF shading functions output color only, no alpha
//!   channel — a per-stop alpha difference (rare; no current figure
//!   supplies one) is NOT preserved per-stop; the whole gradient fill's
//!   own alpha comes from `global_alpha` alone (uniform `ExtGState`,
//!   same mechanism every other paint op uses), and each stop's OWN
//!   color-alpha component is dropped after being folded into `global_alpha`
//!   at most once (the first stop's). Every stop's RGB color is preserved
//!   exactly.
//! - Rotated/non-uniformly-scaled text — **documented scope limit, not
//!   exercised by any current caller** (verified: no `uzor-figures`/
//!   `uzor-typeset` call site calls `Painter::rotate` before `fill_text`,
//!   this backend's whole reason for existing this pass). `fill_text`
//!   bakes the transform's TRANSLATION into the anchor position and its
//!   own [`Transform::scale_factor`] into the font size, but always emits
//!   an axis-aligned (non-rotated) text matrix — a real gap for a
//!   hypothetical future rotated-label caller, inert today.
//! - Backdrop blur / shadow / blend modes ([`UiEffectHelpers`]/
//!   [`Effects`]) — no vector-PDF equivalent attempted, same documented
//!   absence `SvgRenderContext` already carries for the identical reason.
//! - Offscreen render targets — not supported (every
//!   [`RenderContext`] default applies) — no current figure/chrome caller
//!   needs one.

use std::collections::HashMap;

use pdf_writer::types::{ColorSpaceOperand, FunctionShadingType, LineCapStyle, LineJoinStyle};
use pdf_writer::{Content, Filter, Name, Pdf, Ref, Str};

use uzor::render::{
    BatchPainter, Effects, GradientPainter, ImagePainter, Masking, Painter, RenderContext,
    ShapeHelpers, TextAlign, TextBaseline, TextBounds, TextMetrics, TextRenderer,
    UiEffectHelpers,
};

use super::subset::FontData;
use super::{flate_compress, FontId, PdfBuilder, PdfFontCache, RefAllocator};

// ---------------------------------------------------------------------------
// Minimal 2D affine transform — same matrix shape as
// `uzor-render-svg::transform::Transform`, reimplemented locally (an
// internal type, not worth a cross-crate reach for ~40 lines).
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
struct Transform {
    sx: f64,
    ky: f64,
    kx: f64,
    sy: f64,
    tx: f64,
    ty: f64,
}

impl Transform {
    fn identity() -> Self {
        Self { sx: 1.0, ky: 0.0, kx: 0.0, sy: 1.0, tx: 0.0, ty: 0.0 }
    }

    fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (self.sx * x + self.kx * y + self.tx, self.ky * x + self.sy * y + self.ty)
    }

    fn pre_translate(&self, dx: f64, dy: f64) -> Self {
        Self { tx: self.sx * dx + self.kx * dy + self.tx, ty: self.ky * dx + self.sy * dy + self.ty, ..*self }
    }

    fn pre_scale(&self, sx: f64, sy: f64) -> Self {
        Self { sx: self.sx * sx, ky: self.ky * sx, kx: self.kx * sy, sy: self.sy * sy, ..*self }
    }

    fn pre_rotate(&self, angle_rad: f64) -> Self {
        let (s, c) = angle_rad.sin_cos();
        Self {
            sx: self.sx * c + self.kx * s,
            ky: self.ky * c + self.sy * s,
            kx: -self.sx * s + self.kx * c,
            sy: -self.ky * s + self.sy * c,
            tx: self.tx,
            ty: self.ty,
        }
    }

    /// Approximate uniform scale factor (average of the two basis-vector
    /// lengths) — used to scale stroke width / dash arrays / gradient
    /// radii / font size, since those aren't baked into path COORDINATES
    /// the way fill/stroke geometry is. Exact for uniform scale-only
    /// matrices (the only shape any current caller of this backend ever
    /// produces — see this module's own doc comment).
    fn scale_factor(&self) -> f64 {
        let x_len = (self.sx * self.sx + self.ky * self.ky).sqrt();
        let y_len = (self.kx * self.kx + self.sy * self.sy).sqrt();
        ((x_len + y_len) / 2.0).max(1e-9)
    }
}

// ---------------------------------------------------------------------------
// Path segments + draw ops — the intermediate representation a page's
// content stream is built from. Kept `pub(crate)` (visible across the
// whole `pdf` module tree: `mod.rs`'s `PageRecord`/`write_page`,
// `subset.rs`'s glyph-usage walk) but never crosses the crate boundary
// itself — `PdfContentStream` (below) is the opaque public wrapper
// `uzor-typeset::export::pdf_adapter` actually holds.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub(crate) enum PathSeg {
    MoveTo(f64, f64),
    LineTo(f64, f64),
    CubicTo(f64, f64, f64, f64, f64, f64),
    Close,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PdfLineCap {
    Butt,
    Round,
    Square,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PdfLineJoin {
    Miter,
    Round,
    Bevel,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct GradientStop {
    pub offset: f32,
    pub rgb: (f32, f32, f32),
}

#[derive(Clone, Debug)]
pub(crate) enum PdfOp {
    Save,
    Restore,
    Clip {
        path: Vec<PathSeg>,
        even_odd: bool,
    },
    Fill {
        path: Vec<PathSeg>,
        rgb: (f32, f32, f32),
        alpha: f32,
        even_odd: bool,
    },
    Stroke {
        path: Vec<PathSeg>,
        rgb: (f32, f32, f32),
        alpha: f32,
        width_pt: f64,
        cap: PdfLineCap,
        join: PdfLineJoin,
        dash: Vec<f64>,
    },
    LinearGradientFill {
        path: Vec<PathSeg>,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        stops: Vec<GradientStop>,
        alpha: f32,
        even_odd: bool,
    },
    RadialGradientFill {
        path: Vec<PathSeg>,
        cx: f64,
        cy: f64,
        r: f64,
        stops: Vec<GradientStop>,
        alpha: f32,
        even_odd: bool,
    },
    Text {
        font: FontId,
        size_pt: f64,
        x_pt: f64,
        y_pt: f64,
        rgb: (f32, f32, f32),
        alpha: f32,
        /// `(original glyph id in the FULL, pre-subsetting font, source
        /// Unicode scalar)` pairs — same shape as `super::PageTextRun::
        /// glyphs`, resolved eagerly at `fill_text` call time via
        /// `PdfBuilder::font_metrics` (this backend has read access to
        /// the SAME `TtfMetrics` `add_page` uses for `PdfTextRun`).
        glyphs: Vec<(u16, char)>,
    },
    Image {
        x_pt: f64,
        y_pt: f64,
        w_pt: f64,
        h_pt: f64,
        /// Tightly packed RGB8, alpha dropped (present in `alpha_mask`
        /// instead when the source data carries real transparency).
        rgb: Vec<u8>,
        alpha_mask: Option<Vec<u8>>,
        px_w: u32,
        px_h: u32,
        alpha: f32,
    },
}

/// Opaque accumulator [`PdfRenderContext::finish`] hands back —
/// `uzor-typeset::export::pdf_adapter` plumbs this straight into
/// [`super::PdfPageSpec::content`] without ever naming a [`PdfOp`]
/// variant itself (this crate's own engine-agnostic boundary: the
/// content model's INTERNALS stay in `uzor-export`, only the composed
/// stream crosses into the adapter that knows `Page`/`ParagraphLayout`).
#[derive(Default)]
pub struct PdfContentStream(pub(crate) Vec<PdfOp>);

impl PdfContentStream {
    /// A page with no figure/table/chrome content at all (text-only, or
    /// before [`PdfRenderContext`] has drawn anything) — every
    /// pre-existing [`super::PdfPageSpec`] construction site across this
    /// crate's own tests uses this.
    pub fn empty() -> Self {
        Self(Vec::new())
    }
}

// ---------------------------------------------------------------------------
// Style/save-restore state
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
struct Rgba {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl Rgba {
    fn parse(s: &str) -> Self {
        let (r, g, b, a) = uzor::render::parse_color(s);
        Self { r, g, b, a }
    }

    fn with_alpha_frac(self, frac: f64) -> Self {
        let frac = frac.clamp(0.0, 1.0);
        let a = ((self.a as f64 / 255.0) * frac * 255.0).round().clamp(0.0, 255.0) as u8;
        Self { a, ..self }
    }

    fn alpha_frac(self) -> f64 {
        self.a as f64 / 255.0
    }

    fn rgb_f32(self) -> (f32, f32, f32) {
        (self.r as f32 / 255.0, self.g as f32 / 255.0, self.b as f32 / 255.0)
    }
}

const BLACK: Rgba = Rgba { r: 0, g: 0, b: 0, a: 255 };

#[derive(Clone)]
struct SavedState {
    fill_color: Rgba,
    stroke_color: Rgba,
    stroke_width: f64,
    line_cap: PdfLineCap,
    line_join: PdfLineJoin,
    line_dash: Vec<f64>,
    global_alpha: f64,
    font_css: String,
    text_align: TextAlign,
    text_baseline: TextBaseline,
    transform: Transform,
}

// ---------------------------------------------------------------------------
// PdfRenderContext
// ---------------------------------------------------------------------------

/// PDF-content-stream-emitting [`RenderContext`] — one instance per page,
/// borrowing the document-wide [`PdfBuilder`] (to register newly-seen
/// fonts and query already-registered ones' metrics) and [`PdfFontCache`]
/// (the shared font-resource registry, see that type's own doc comment)
/// for its whole lifetime.
pub struct PdfRenderContext<'a> {
    dpr: f64,

    builder: &'a mut PdfBuilder,
    fonts: &'a mut PdfFontCache,

    fill_color: Rgba,
    stroke_color: Rgba,
    stroke_width: f64,
    line_cap: PdfLineCap,
    line_join: PdfLineJoin,
    line_dash: Vec<f64>,
    global_alpha: f64,

    font_css: String,
    text_align: TextAlign,
    text_baseline: TextBaseline,

    transform: Transform,

    path: Vec<PathSeg>,
    path_has_point: bool,
    current_point: (f64, f64),

    state_stack: Vec<SavedState>,
    ops: Vec<PdfOp>,
}

impl<'a> PdfRenderContext<'a> {
    /// One page's worth of drawing. Deliberately does NOT take the page's
    /// own `width_pt`/`height_pt` — every op this backend records stays
    /// in TOP-LEFT, y-down canvas space (this module's own doc comment);
    /// the ONE place that needs the page's physical height (the
    /// bottom-up PDF y-flip) is [`emit_ops`]/`super::write_page`, which
    /// already reads it from [`super::PageRecord::height_pt`] — carrying
    /// a second copy here would be a field nothing reads.
    pub fn new(dpr: f64, builder: &'a mut PdfBuilder, fonts: &'a mut PdfFontCache) -> Self {
        Self {
            dpr,
            builder,
            fonts,
            fill_color: BLACK,
            stroke_color: BLACK,
            stroke_width: 1.0,
            line_cap: PdfLineCap::Butt,
            line_join: PdfLineJoin::Miter,
            line_dash: Vec::new(),
            global_alpha: 1.0,
            font_css: "12px sans-serif".to_string(),
            text_align: TextAlign::default(),
            text_baseline: TextBaseline::default(),
            transform: Transform::identity(),
            path: Vec::new(),
            path_has_point: false,
            current_point: (0.0, 0.0),
            state_stack: Vec::new(),
            ops: Vec::new(),
        }
    }

    /// Finish this page's drawing pass, handing back the accumulated
    /// op list for [`super::PdfPageSpec::content`].
    pub fn finish(self) -> PdfContentStream {
        PdfContentStream(self.ops)
    }

    // -----------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------

    fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        self.transform.apply(x, y)
    }

    fn take_path(&mut self) -> Option<Vec<PathSeg>> {
        self.path_has_point = false;
        if self.path.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.path))
        }
    }

    fn append_moveto(&mut self, x: f64, y: f64) {
        let p = self.apply(x, y);
        self.path.push(PathSeg::MoveTo(p.0, p.1));
        self.current_point = p;
    }

    fn append_lineto(&mut self, x: f64, y: f64) {
        let p = self.apply(x, y);
        self.path.push(PathSeg::LineTo(p.0, p.1));
        self.current_point = p;
    }

    fn append_quadto(&mut self, cx: f64, cy: f64, x: f64, y: f64) {
        // PDF has no native quadratic operator — convert to the
        // equivalent cubic (standard degree-elevation formula).
        let (tcx, tcy) = self.apply(cx, cy);
        let (tx, ty) = self.apply(x, y);
        let (px, py) = self.current_point;
        let c1 = (px + 2.0 / 3.0 * (tcx - px), py + 2.0 / 3.0 * (tcy - py));
        let c2 = (tx + 2.0 / 3.0 * (tcx - tx), ty + 2.0 / 3.0 * (tcy - ty));
        self.path.push(PathSeg::CubicTo(c1.0, c1.1, c2.0, c2.1, tx, ty));
        self.current_point = (tx, ty);
    }

    fn append_cubicto(&mut self, c1x: f64, c1y: f64, c2x: f64, c2y: f64, x: f64, y: f64) {
        let (t1x, t1y) = self.apply(c1x, c1y);
        let (t2x, t2y) = self.apply(c2x, c2y);
        let (tx, ty) = self.apply(x, y);
        self.path.push(PathSeg::CubicTo(t1x, t1y, t2x, t2y, tx, ty));
        self.current_point = (tx, ty);
    }

    fn append_close(&mut self) {
        self.path.push(PathSeg::Close);
    }

    /// Cubic-bezier approximation of a circular arc — ported from
    /// `uzor-render-svg::context::SvgRenderContext::arc_to_cubics`
    /// (identical math, emitting [`PathSeg`]s instead of SVG path
    /// command text).
    fn arc_to_cubics(&mut self, cx: f64, cy: f64, r: f64, start: f64, end: f64, has_current_point: bool) {
        let mut sweep = end - start;
        if sweep == 0.0 {
            return;
        }
        if sweep > std::f64::consts::TAU {
            sweep = std::f64::consts::TAU;
        }
        if sweep < -std::f64::consts::TAU {
            sweep = -std::f64::consts::TAU;
        }

        let n_segs = ((sweep.abs() / (std::f64::consts::PI / 2.0)).ceil() as u32).max(1);
        let seg_angle = sweep / n_segs as f64;
        let k = (4.0 / 3.0) * ((seg_angle / 4.0).abs().tan());

        let a0 = start;
        let start_x = cx + r * a0.cos();
        let start_y = cy + r * a0.sin();
        if has_current_point {
            self.append_lineto(start_x, start_y);
        } else {
            self.append_moveto(start_x, start_y);
        }

        let mut a = start;
        for _ in 0..n_segs {
            let a1 = a + seg_angle;
            let (sin_a, cos_a) = a.sin_cos();
            let (sin_a1, cos_a1) = a1.sin_cos();

            let p0x = cx + r * cos_a;
            let p0y = cy + r * sin_a;
            let p3x = cx + r * cos_a1;
            let p3y = cy + r * sin_a1;

            let cp1x = p0x - k * r * sin_a;
            let cp1y = p0y + k * r * cos_a;
            let cp2x = p3x + k * r * sin_a1;
            let cp2y = p3y - k * r * cos_a1;

            self.append_cubicto(cp1x, cp1y, cp2x, cp2y, p3x, p3y);
            a = a1;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn ellipse_to_cubics(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, start: f64, end: f64, has_current_point: bool) {
        let mut sweep = end - start;
        if sweep == 0.0 {
            return;
        }
        if sweep > std::f64::consts::TAU {
            sweep = std::f64::consts::TAU;
        }
        if sweep < -std::f64::consts::TAU {
            sweep = -std::f64::consts::TAU;
        }

        let n_segs = ((sweep.abs() / (std::f64::consts::PI / 2.0)).ceil() as u32).max(1);
        let seg_angle = sweep / n_segs as f64;
        let k = (4.0 / 3.0) * ((seg_angle / 4.0).abs().tan());

        let a0 = start;
        let start_x = cx + rx * a0.cos();
        let start_y = cy + ry * a0.sin();
        if has_current_point {
            self.append_lineto(start_x, start_y);
        } else {
            self.append_moveto(start_x, start_y);
        }

        let mut a = start;
        for _ in 0..n_segs {
            let a1 = a + seg_angle;
            let (sin_a, cos_a) = a.sin_cos();
            let (sin_a1, cos_a1) = a1.sin_cos();

            let p0x = cx + rx * cos_a;
            let p0y = cy + ry * sin_a;
            let p3x = cx + rx * cos_a1;
            let p3y = cy + ry * sin_a1;

            let cp1x = p0x - k * rx * sin_a;
            let cp1y = p0y + k * ry * cos_a;
            let cp2x = p3x + k * rx * sin_a1;
            let cp2y = p3y - k * ry * cos_a1;

            self.append_cubicto(cp1x, cp1y, cp2x, cp2y, p3x, p3y);
            a = a1;
        }
    }

    fn fill_paint(&self) -> ((f32, f32, f32), f32) {
        let alpha = ((self.fill_color.alpha_frac() * self.global_alpha).clamp(0.0, 1.0)) as f32;
        (self.fill_color.rgb_f32(), alpha)
    }

    fn stroke_paint(&self) -> ((f32, f32, f32), f32) {
        let alpha = ((self.stroke_color.alpha_frac() * self.global_alpha).clamp(0.0, 1.0)) as f32;
        (self.stroke_color.rgb_f32(), alpha)
    }
}

// ---------------------------------------------------------------------------
// Painter
// ---------------------------------------------------------------------------

impl Painter for PdfRenderContext<'_> {
    fn save(&mut self) {
        self.state_stack.push(SavedState {
            fill_color: self.fill_color,
            stroke_color: self.stroke_color,
            stroke_width: self.stroke_width,
            line_cap: self.line_cap,
            line_join: self.line_join,
            line_dash: self.line_dash.clone(),
            global_alpha: self.global_alpha,
            font_css: self.font_css.clone(),
            text_align: self.text_align,
            text_baseline: self.text_baseline,
            transform: self.transform,
        });
        // Always emit a real `q` — PDF clip regions ONLY reset at a
        // matching `Q`, so this backend's own `Masking::clip` (a plain
        // `W n` on whatever the CURRENT path is) relies on every
        // `save`/`restore` pair genuinely bracketing the content stream's
        // own graphics-state stack, not just this Rust-side style
        // bookkeeping.
        self.ops.push(PdfOp::Save);
    }

    fn restore(&mut self) {
        if let Some(s) = self.state_stack.pop() {
            self.fill_color = s.fill_color;
            self.stroke_color = s.stroke_color;
            self.stroke_width = s.stroke_width;
            self.line_cap = s.line_cap;
            self.line_join = s.line_join;
            self.line_dash = s.line_dash;
            self.global_alpha = s.global_alpha;
            self.font_css = s.font_css;
            self.text_align = s.text_align;
            self.text_baseline = s.text_baseline;
            self.transform = s.transform;
            // Only emit `Q` when a matching `Save` really happened — an
            // unbalanced extra `restore()` call must never corrupt the
            // content stream's own `q`/`Q` nesting.
            self.ops.push(PdfOp::Restore);
        }
    }

    fn translate(&mut self, x: f64, y: f64) {
        self.transform = self.transform.pre_translate(x, y);
    }

    fn rotate(&mut self, angle: f64) {
        self.transform = self.transform.pre_rotate(angle);
    }

    fn scale(&mut self, x: f64, y: f64) {
        self.transform = self.transform.pre_scale(x, y);
    }

    fn set_fill_color(&mut self, color: &str) {
        self.fill_color = Rgba::parse(color);
    }

    fn set_global_alpha(&mut self, alpha: f64) {
        self.global_alpha = alpha.clamp(0.0, 1.0);
    }

    fn set_fill_color_alpha(&mut self, color: &str, alpha: f64) {
        self.fill_color = Rgba::parse(color).with_alpha_frac(alpha);
    }

    fn set_stroke_color(&mut self, color: &str) {
        self.stroke_color = Rgba::parse(color);
    }

    fn set_stroke_width(&mut self, width: f64) {
        self.stroke_width = width;
    }

    fn set_line_dash(&mut self, pattern: &[f64]) {
        self.line_dash = pattern.to_vec();
    }

    fn set_line_cap(&mut self, cap: &str) {
        self.line_cap = match cap {
            "round" => PdfLineCap::Round,
            "square" => PdfLineCap::Square,
            _ => PdfLineCap::Butt,
        };
    }

    fn set_line_join(&mut self, join: &str) {
        self.line_join = match join {
            "round" => PdfLineJoin::Round,
            "bevel" => PdfLineJoin::Bevel,
            _ => PdfLineJoin::Miter,
        };
    }

    fn begin_path(&mut self) {
        self.path.clear();
        self.path_has_point = false;
    }

    fn move_to(&mut self, x: f64, y: f64) {
        self.append_moveto(x, y);
        self.path_has_point = true;
    }

    fn line_to(&mut self, x: f64, y: f64) {
        self.append_lineto(x, y);
        self.path_has_point = true;
    }

    fn close_path(&mut self) {
        self.append_close();
    }

    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.append_moveto(x, y);
        self.append_lineto(x + w, y);
        self.append_lineto(x + w, y + h);
        self.append_lineto(x, y + h);
        self.append_close();
        self.path_has_point = true;
    }

    fn arc(&mut self, cx: f64, cy: f64, radius: f64, start_angle: f64, end_angle: f64) {
        let has_point = self.path_has_point;
        self.arc_to_cubics(cx, cy, radius, start_angle, end_angle, has_point);
        self.path_has_point = true;
    }

    fn ellipse(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, _rotation: f64, start: f64, end: f64) {
        let has_point = self.path_has_point;
        self.ellipse_to_cubics(cx, cy, rx, ry, start, end, has_point);
        self.path_has_point = true;
    }

    fn quadratic_curve_to(&mut self, cpx: f64, cpy: f64, x: f64, y: f64) {
        self.append_quadto(cpx, cpy, x, y);
        self.path_has_point = true;
    }

    fn bezier_curve_to(&mut self, cp1x: f64, cp1y: f64, cp2x: f64, cp2y: f64, x: f64, y: f64) {
        self.append_cubicto(cp1x, cp1y, cp2x, cp2y, x, y);
        self.path_has_point = true;
    }

    fn stroke(&mut self) {
        let Some(path) = self.take_path() else { return };
        let (rgb, alpha) = self.stroke_paint();
        let width_pt = self.stroke_width * self.transform.scale_factor();
        let scale = self.transform.scale_factor();
        let dash = self.line_dash.iter().map(|v| v * scale).collect();
        self.ops.push(PdfOp::Stroke { path, rgb, alpha, width_pt, cap: self.line_cap, join: self.line_join, dash });
    }

    fn fill(&mut self) {
        let Some(path) = self.take_path() else { return };
        let (rgb, alpha) = self.fill_paint();
        self.ops.push(PdfOp::Fill { path, rgb, alpha, even_odd: false });
    }
}

// ---------------------------------------------------------------------------
// ShapeHelpers
// ---------------------------------------------------------------------------

impl ShapeHelpers for PdfRenderContext<'_> {
    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.begin_path();
        self.rect(x, y, w, h);
        self.fill();
    }

    fn stroke_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.begin_path();
        self.rect(x, y, w, h);
        self.stroke();
    }
}

// ---------------------------------------------------------------------------
// Masking — a real `W n` (nonzero winding) clip, scoped by this backend's
// own real `q`/`Q` emission in `save`/`restore` (see those methods' own
// doc comments).
// ---------------------------------------------------------------------------

impl Masking for PdfRenderContext<'_> {
    fn clip(&mut self) {
        let Some(path) = self.take_path() else { return };
        self.ops.push(PdfOp::Clip { path, even_odd: false });
    }
}

// ---------------------------------------------------------------------------
// Effects / UiEffectHelpers / BatchPainter — no override needed. Effects
// (shadow/blend) has no vector-PDF equivalent attempted this pass (same
// documented absence `SvgRenderContext` carries); UiEffectHelpers
// (backdrop blur) has no raster backdrop to sample; BatchPainter's own
// default impls already loop over this backend's REAL `Painter`
// primitives, which is already correct.
// ---------------------------------------------------------------------------

impl Effects for PdfRenderContext<'_> {}
impl UiEffectHelpers for PdfRenderContext<'_> {}
impl BatchPainter for PdfRenderContext<'_> {}

// ---------------------------------------------------------------------------
// GradientPainter — a REAL PDF shading-pattern implementation (see this
// module's own doc comment for the documented per-stop-alpha scope
// limit).
// ---------------------------------------------------------------------------

impl GradientPainter for PdfRenderContext<'_> {
    fn fill_linear_gradient(&mut self, stops: &[(f32, &str)], x1: f64, y1: f64, x2: f64, y2: f64) {
        let Some(path) = self.take_path() else { return };
        if stops.is_empty() {
            return;
        }
        let (tx1, ty1) = self.apply(x1, y1);
        let (tx2, ty2) = self.apply(x2, y2);
        let (gstops, alpha) = self.resolve_gradient_stops(stops);
        self.ops.push(PdfOp::LinearGradientFill { path, x1: tx1, y1: ty1, x2: tx2, y2: ty2, stops: gstops, alpha, even_odd: false });
    }

    #[allow(clippy::too_many_arguments)]
    fn fill_radial_gradient(&mut self, cx: f64, cy: f64, r: f64, stops: &[(f32, &str)], _x: f64, _y: f64, _w: f64, _h: f64) {
        let Some(path) = self.take_path() else { return };
        if stops.is_empty() {
            return;
        }
        let (tcx, tcy) = self.apply(cx, cy);
        let tr = r * self.transform.scale_factor();
        let (gstops, alpha) = self.resolve_gradient_stops(stops);
        self.ops.push(PdfOp::RadialGradientFill { path, cx: tcx, cy: tcy, r: tr, stops: gstops, alpha, even_odd: false });
    }
}

impl PdfRenderContext<'_> {
    /// Parse `stops` into [`GradientStop`]s (color only — PDF shading
    /// functions have no alpha channel) and fold the FIRST stop's own
    /// color-alpha component (times `global_alpha`) into a single uniform
    /// alpha for the whole gradient fill — see this module's own doc
    /// comment for why per-stop alpha isn't preserved.
    fn resolve_gradient_stops(&self, stops: &[(f32, &str)]) -> (Vec<GradientStop>, f32) {
        let global_alpha = self.global_alpha;
        let gstops: Vec<GradientStop> = stops
            .iter()
            .map(|&(offset, color)| {
                let (r, g, b, _a) = uzor::render::parse_color(color);
                GradientStop { offset: offset.clamp(0.0, 1.0), rgb: (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0) }
            })
            .collect();
        let first_alpha = stops.first().map(|&(_, color)| uzor::render::parse_color(color).3).unwrap_or(255);
        let alpha = ((first_alpha as f64 / 255.0) * global_alpha).clamp(0.0, 1.0) as f32;
        (gstops, alpha)
    }
}

// ---------------------------------------------------------------------------
// ImagePainter
// ---------------------------------------------------------------------------

impl ImagePainter for PdfRenderContext<'_> {
    /// No URL/id-based image cache — matches every other backend in this
    /// workspace's identical documented `false` (this backend never loads
    /// images by id, only raw RGBA via `draw_image_rgba`).
    fn draw_image(&mut self, _image_id: &str, _x: f64, _y: f64, _width: f64, _height: f64) -> bool {
        false
    }

    fn draw_image_rgba(&mut self, data: &[u8], img_width: u32, img_height: u32, x: f64, y: f64, width: f64, height: f64) {
        if img_width == 0 || img_height == 0 || width <= 0.0 || height <= 0.0 {
            return;
        }
        let expected_len = img_width as usize * img_height as usize * 4;
        if data.len() != expected_len {
            return;
        }

        // Axis-aligned destination rect (see this module's own doc
        // comment: rotated image placement is a documented, inert scope
        // limit — no current caller rotates before drawing an image).
        let (p0x, p0y) = self.apply(x, y);
        let (p1x, p1y) = self.apply(x + width, y + height);
        let rx = p0x.min(p1x);
        let ry = p0y.min(p1y);
        let rw = (p1x - p0x).abs();
        let rh = (p1y - p0y).abs();

        let has_alpha = data.chunks_exact(4).any(|px| px[3] != 255);
        let pixel_count = img_width as usize * img_height as usize;
        let mut rgb = Vec::with_capacity(pixel_count * 3);
        let mut alpha_mask = has_alpha.then(|| Vec::with_capacity(pixel_count));
        for px in data.chunks_exact(4) {
            rgb.extend_from_slice(&px[0..3]);
            if let Some(mask) = alpha_mask.as_mut() {
                mask.push(px[3]);
            }
        }

        let alpha = self.global_alpha.clamp(0.0, 1.0) as f32;
        self.ops.push(PdfOp::Image { x_pt: rx, y_pt: ry, w_pt: rw, h_pt: rh, rgb, alpha_mask, px_w: img_width, px_h: img_height, alpha });
    }
}

// ---------------------------------------------------------------------------
// TextRenderer
// ---------------------------------------------------------------------------

impl TextRenderer for PdfRenderContext<'_> {
    fn set_font(&mut self, font: &str) {
        self.font_css = font.to_string();
    }

    fn set_text_align(&mut self, align: TextAlign) {
        self.text_align = align;
    }

    fn set_text_baseline(&mut self, baseline: TextBaseline) {
        self.text_baseline = baseline;
    }

    /// Real Type0/CID text — resolves `text`'s own characters to glyph
    /// ids via the SAME [`PdfFontCache`]/`TtfMetrics` machinery
    /// `pdf_adapter.rs`'s paragraph runs already use, so a figure's axis
    /// label and a paragraph's body text end up on the exact same `/Font`
    /// resource when they share a `(family, bold, italic)` combination.
    fn fill_text(&mut self, text: &str, x: f64, y: f64) {
        if text.is_empty() {
            return;
        }
        let font_info = uzor::fonts::parse_css_font(&self.font_css);
        let font_size = font_info.size as f64;

        let total_w = self.measure_text(text);
        let x_off = match self.text_align {
            TextAlign::Center => -(total_w / 2.0),
            TextAlign::Right => -total_w,
            TextAlign::Left => 0.0,
        };

        let lines = uzor::shaper::measure_glyphs_wrapped(text, &self.font_css, f64::MAX);
        let ascent = lines.first().map(|l| l.baseline_y).unwrap_or(font_size * 0.9);
        let y_off = match self.text_baseline {
            TextBaseline::Top => ascent,
            TextBaseline::Middle => ascent / 2.0,
            TextBaseline::Bottom => 0.0,
            TextBaseline::Alphabetic => 0.0,
        };

        let pen_x = x + x_off;
        let pen_y = y + y_off;
        let (abs_x, abs_y) = self.apply(pen_x, pen_y);
        let scaled_size = font_size * self.transform.scale_factor();

        let font_id = self.fonts.id_for(font_info.family, font_info.bold, font_info.italic, self.builder);
        let glyphs: Vec<(u16, char)> = {
            let metrics = self.builder.font_metrics(font_id);
            text.chars().map(|ch| (metrics.gid_for_char(ch).unwrap_or(0), ch)).collect()
        };

        let (rgb, alpha) = self.fill_paint();
        self.ops.push(PdfOp::Text { font: font_id, size_pt: scaled_size, x_pt: abs_x, y_pt: abs_y, rgb, alpha, glyphs });
    }

    /// Same "solid fill of the stroke color" approximation every other
    /// backend in this workspace uses (real stroked/outlined glyph
    /// rendering is out of scope everywhere, not just here).
    fn stroke_text(&mut self, text: &str, x: f64, y: f64) {
        let saved = self.fill_color;
        self.fill_color = self.stroke_color;
        self.fill_text(text, x, y);
        self.fill_color = saved;
    }
}

// ---------------------------------------------------------------------------
// TextMetrics — delegates to `uzor::shaper` (cosmic-text), same as every
// other `shaper`-feature backend in this workspace.
// ---------------------------------------------------------------------------

impl TextMetrics for PdfRenderContext<'_> {
    fn measure_text(&self, text: &str) -> f64 {
        uzor::shaper::measure_glyphs(text, &self.font_css).iter().map(|g| g.advance).sum()
    }

    fn text_bounds(&self, text: &str, font: &str) -> TextBounds {
        if text.is_empty() {
            return TextBounds { x: 0.0, y: 0.0, w: 0.0, h: 0.0, ascent: 0.0, descent: 0.0 };
        }
        let font_info = uzor::fonts::parse_css_font(font);
        let font_size = font_info.size as f64;
        let w: f64 = uzor::shaper::measure_glyphs(text, font).iter().map(|g| g.advance).sum();
        let lines = uzor::shaper::measure_glyphs_wrapped(text, font, f64::MAX);
        let ascent = lines.first().map(|l| l.baseline_y).unwrap_or(font_size * 0.9);
        let descent = (font_size * 1.2 - ascent).max(0.0);
        TextBounds { x: 0.0, y: -ascent, w, h: ascent + descent, ascent, descent }
    }

    fn measure_text_glyphs(&self, text: &str, font: &str) -> Vec<uzor::render::GlyphMetric> {
        uzor::shaper::measure_glyphs(text, font)
    }

    fn measure_text_wrapped(&self, text: &str, font: &str, max_width: f64) -> Vec<uzor::render::WrappedLine> {
        uzor::shaper::measure_glyphs_wrapped(text, font, max_width)
    }
}

// ---------------------------------------------------------------------------
// RenderContext
// ---------------------------------------------------------------------------

impl RenderContext for PdfRenderContext<'_> {
    fn dpr(&self) -> f64 {
        self.dpr
    }

    fn image_painter(&mut self) -> Option<&mut dyn ImagePainter> {
        Some(self)
    }

    // Offscreen render targets are not supported — every `RenderContext`
    // default applies (no current figure/chrome caller needs one).
}

// ---------------------------------------------------------------------------
// Serialization: PdfOp list -> real pdf-writer Content ops, called from
// `super::write_page`.
// ---------------------------------------------------------------------------

/// Resource-dictionary entries [`emit_ops`] discovers WHILE building the
/// content stream (patterns/gstates/inline images are allocated on
/// demand, only when a page actually uses one) — `super::write_page`
/// merges these into the page's own `/Resources` dict AFTER the content
/// stream is fully written (the dict itself is always written last, so
/// this ordering costs nothing).
#[derive(Default)]
pub(crate) struct PageOpResources {
    pub images: Vec<(String, Ref)>,
    pub patterns: Vec<(String, Ref)>,
    pub ext_gstates: Vec<(String, Ref)>,
}

/// Emit every op in `ops` (in RECORDED order — z-order matters: a figure's
/// axis line, then its bars, then its labels, must paint in that order)
/// into `content`, flipping every y-coordinate via `page_height_pt - y`
/// (this module's own doc comment: the ONE place this crate's top-left,
/// y-down convention converts to PDF's native bottom-left, y-up space —
/// shared by every content kind, not just text runs).
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_ops(
    pdf: &mut Pdf,
    refs: &mut RefAllocator,
    content: &mut Content,
    ops: &[PdfOp],
    page_height_pt: f64,
    font_data: &[FontData],
    font_names: &[String],
) -> PageOpResources {
    let mut resources = PageOpResources::default();
    let mut gstate_cache: Vec<(i32, String)> = Vec::new();
    let mut image_idx: u32 = 0;
    let mut pattern_idx: u32 = 0;

    for op in ops {
        match op {
            PdfOp::Save => {
                content.save_state();
            }
            PdfOp::Restore => {
                content.restore_state();
            }
            PdfOp::Clip { path, even_odd } => {
                emit_path(content, path, page_height_pt);
                if *even_odd {
                    content.clip_even_odd();
                } else {
                    content.clip_nonzero();
                }
                content.end_path();
            }
            PdfOp::Fill { path, rgb, alpha, even_odd } => {
                emit_path(content, path, page_height_pt);
                apply_alpha_gstate(pdf, refs, content, &mut resources, &mut gstate_cache, *alpha);
                content.set_fill_rgb(rgb.0, rgb.1, rgb.2);
                if *even_odd {
                    content.fill_even_odd();
                } else {
                    content.fill_nonzero();
                }
            }
            PdfOp::Stroke { path, rgb, alpha, width_pt, cap, join, dash } => {
                emit_path(content, path, page_height_pt);
                apply_alpha_gstate(pdf, refs, content, &mut resources, &mut gstate_cache, *alpha);
                content.set_stroke_rgb(rgb.0, rgb.1, rgb.2);
                content.set_line_width(*width_pt as f32);
                content.set_line_cap(pdf_line_cap(*cap));
                content.set_line_join(pdf_line_join(*join));
                content.set_dash_pattern(dash.iter().map(|v| *v as f32), 0.0);
                content.stroke();
            }
            PdfOp::LinearGradientFill { path, x1, y1, x2, y2, stops, alpha, even_odd } => {
                let name = format!("P{pattern_idx}");
                pattern_idx += 1;
                let coords = [*x1 as f32, (page_height_pt - y1) as f32, *x2 as f32, (page_height_pt - y2) as f32];
                let pat_ref = write_shading_pattern(pdf, refs, FunctionShadingType::Axial, &coords, stops);
                resources.patterns.push((name.clone(), pat_ref));

                emit_path(content, path, page_height_pt);
                apply_alpha_gstate(pdf, refs, content, &mut resources, &mut gstate_cache, *alpha);
                content.set_fill_color_space(ColorSpaceOperand::Pattern);
                content.set_fill_pattern(None, Name(name.as_bytes()));
                if *even_odd {
                    content.fill_even_odd();
                } else {
                    content.fill_nonzero();
                }
            }
            PdfOp::RadialGradientFill { path, cx, cy, r, stops, alpha, even_odd } => {
                let name = format!("P{pattern_idx}");
                pattern_idx += 1;
                let flipped_cy = (page_height_pt - cy) as f32;
                let coords = [*cx as f32, flipped_cy, 0.0, *cx as f32, flipped_cy, *r as f32];
                let pat_ref = write_shading_pattern(pdf, refs, FunctionShadingType::Radial, &coords, stops);
                resources.patterns.push((name.clone(), pat_ref));

                emit_path(content, path, page_height_pt);
                apply_alpha_gstate(pdf, refs, content, &mut resources, &mut gstate_cache, *alpha);
                content.set_fill_color_space(ColorSpaceOperand::Pattern);
                content.set_fill_pattern(None, Name(name.as_bytes()));
                if *even_odd {
                    content.fill_even_odd();
                } else {
                    content.fill_nonzero();
                }
            }
            PdfOp::Text { font, size_pt, x_pt, y_pt, rgb, alpha, glyphs } => {
                let fx = *x_pt as f32;
                let fy = (page_height_pt - y_pt) as f32;
                apply_alpha_gstate(pdf, refs, content, &mut resources, &mut gstate_cache, *alpha);
                content.begin_text();
                content.set_fill_rgb(rgb.0, rgb.1, rgb.2);
                content.set_font(Name(font_names[font.0 as usize].as_bytes()), *size_pt as f32);
                content.set_text_matrix([1.0, 0.0, 0.0, 1.0, fx, fy]);
                let orig_to_new = &font_data[font.0 as usize].orig_to_new;
                content.show(Str(&cid_bytes(glyphs, orig_to_new)));
                content.end_text();
            }
            PdfOp::Image { x_pt, y_pt, w_pt, h_pt, rgb, alpha_mask, px_w, px_h, alpha } => {
                let name = format!("CImg{image_idx}");
                image_idx += 1;
                let img_ref = write_image_xobject(pdf, refs, rgb, alpha_mask.as_deref(), *px_w, *px_h);
                resources.images.push((name.clone(), img_ref));

                apply_alpha_gstate(pdf, refs, content, &mut resources, &mut gstate_cache, *alpha);
                let flip_y = page_height_pt - y_pt - h_pt;
                content.save_state();
                content.transform([*w_pt as f32, 0.0, 0.0, *h_pt as f32, *x_pt as f32, flip_y as f32]);
                content.x_object(Name(name.as_bytes()));
                content.restore_state();
            }
        }
    }

    resources
}

fn emit_path(content: &mut Content, path: &[PathSeg], page_height_pt: f64) {
    for seg in path {
        match seg {
            PathSeg::MoveTo(x, y) => {
                content.move_to(*x as f32, (page_height_pt - y) as f32);
            }
            PathSeg::LineTo(x, y) => {
                content.line_to(*x as f32, (page_height_pt - y) as f32);
            }
            PathSeg::CubicTo(x1, y1, x2, y2, x3, y3) => {
                content.cubic_to(*x1 as f32, (page_height_pt - y1) as f32, *x2 as f32, (page_height_pt - y2) as f32, *x3 as f32, (page_height_pt - y3) as f32);
            }
            PathSeg::Close => {
                content.close_path();
            }
        }
    }
}

fn pdf_line_cap(cap: PdfLineCap) -> LineCapStyle {
    match cap {
        PdfLineCap::Butt => LineCapStyle::ButtCap,
        PdfLineCap::Round => LineCapStyle::RoundCap,
        PdfLineCap::Square => LineCapStyle::ProjectingSquareCap,
    }
}

fn pdf_line_join(join: PdfLineJoin) -> LineJoinStyle {
    match join {
        PdfLineJoin::Miter => LineJoinStyle::MiterJoin,
        PdfLineJoin::Round => LineJoinStyle::RoundJoin,
        PdfLineJoin::Bevel => LineJoinStyle::BevelJoin,
    }
}

/// Encode `glyphs` as a 2-byte-per-glyph, big-endian CID string — the
/// SAME encoding [`super::cid_bytes`] uses for `PdfTextRun`s; duplicated
/// here (not shared) only because the two call sites live in sibling
/// private-visibility contexts and this one-line duplication is cheaper
/// than threading a `pub(crate)` re-export back and forth for a single
/// `HashMap` lookup loop.
fn cid_bytes(glyphs: &[(u16, char)], orig_to_new: &HashMap<u16, u16>) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(glyphs.len() * 2);
    for &(gid, _) in glyphs {
        let cid = orig_to_new.get(&gid).copied().unwrap_or(0);
        bytes.extend_from_slice(&cid.to_be_bytes());
    }
    bytes
}

/// Resolve (allocating + writing a NEW `/ExtGState` object only the FIRST
/// time this exact alpha value — rounded to 3 decimal places, more than
/// enough precision for a visual alpha — appears on this page) the
/// resource name to invoke via `gs` for `alpha`. Called before EVERY
/// paint op (fill/stroke/text/gradient/image), even when `alpha == 1.0`
/// (a real, harmless `ca 1 CA 1` `ExtGState`) — deliberately never relies
/// on graphics state carried over from a PRIOR op, so a stale non-1.0
/// alpha can never silently leak onto a later op that forgot to reset it.
fn apply_alpha_gstate(pdf: &mut Pdf, refs: &mut RefAllocator, content: &mut Content, resources: &mut PageOpResources, cache: &mut Vec<(i32, String)>, alpha: f32) {
    let alpha = alpha.clamp(0.0, 1.0);
    let key = (alpha * 1000.0).round() as i32;
    let name = if let Some((_, existing)) = cache.iter().find(|(k, _)| *k == key) {
        existing.clone()
    } else {
        let name = format!("GS{}", cache.len());
        let gstate_ref = refs.next();
        {
            let mut gs = pdf.ext_graphics(gstate_ref);
            gs.non_stroking_alpha(alpha);
            gs.stroking_alpha(alpha);
        }
        resources.ext_gstates.push((name.clone(), gstate_ref));
        cache.push((key, name.clone()));
        name
    };
    content.set_parameters(Name(name.as_bytes()));
}

/// Build the `ExponentialFunction`(s) (+ a `StitchingFunction` wrapping
/// them, when there are more than 2 stops) driving one gradient's own
/// color ramp, and write the `/PatternType 2` shading pattern wrapping
/// it. Shared by [`PdfOp::LinearGradientFill`]/[`PdfOp::
/// RadialGradientFill`] — the only difference between the two is
/// `kind`/`coords` (`[x1,y1,x2,y2]` for axial, `[x0,y0,r0,x1,y1,r1]` for
/// radial).
fn write_shading_pattern(pdf: &mut Pdf, refs: &mut RefAllocator, kind: FunctionShadingType, coords: &[f32], stops: &[GradientStop]) -> Ref {
    let func_ref = write_stops_function(pdf, refs, stops);
    let pattern_ref = refs.next();
    {
        let mut pattern = pdf.shading_pattern(pattern_ref);
        let mut shading = pattern.function_shading();
        shading.shading_type(kind);
        shading.color_space().device_rgb();
        shading.coords(coords.iter().copied());
        shading.extend([true, true]);
        shading.function(func_ref);
    }
    pattern_ref
}

/// A single stop degrades to a flat (`c0 == c1`) exponential function — a
/// gradient always has `stops.len() >= 1` by the time this is called
/// (both `fill_linear_gradient`/`fill_radial_gradient` already bail out
/// on an empty slice before reaching here).
fn write_stops_function(pdf: &mut Pdf, refs: &mut RefAllocator, stops: &[GradientStop]) -> Ref {
    if stops.len() < 2 {
        let only = stops.first().map(|s| s.rgb).unwrap_or((0.0, 0.0, 0.0));
        let f_ref = refs.next();
        let mut func = pdf.exponential_function(f_ref);
        func.domain([0.0, 1.0]);
        func.range([0.0, 1.0, 0.0, 1.0, 0.0, 1.0]);
        func.c0([only.0, only.1, only.2]);
        func.c1([only.0, only.1, only.2]);
        func.n(1.0);
        return f_ref;
    }

    let mut sub_refs = Vec::with_capacity(stops.len() - 1);
    for w in stops.windows(2) {
        let f_ref = refs.next();
        {
            let mut func = pdf.exponential_function(f_ref);
            func.domain([0.0, 1.0]);
            func.range([0.0, 1.0, 0.0, 1.0, 0.0, 1.0]);
            func.c0([w[0].rgb.0, w[0].rgb.1, w[0].rgb.2]);
            func.c1([w[1].rgb.0, w[1].rgb.1, w[1].rgb.2]);
            func.n(1.0);
        }
        sub_refs.push(f_ref);
    }

    if sub_refs.len() == 1 {
        return sub_refs[0];
    }

    let stitch_ref = refs.next();
    {
        let mut stitch = pdf.stitching_function(stitch_ref);
        stitch.domain([0.0, 1.0]);
        stitch.range([0.0, 1.0, 0.0, 1.0, 0.0, 1.0]);
        stitch.functions(sub_refs.iter().copied());
        let bounds: Vec<f32> = stops[1..stops.len() - 1].iter().map(|s| s.offset).collect();
        stitch.bounds(bounds);
        let encode: Vec<f32> = (0..sub_refs.len()).flat_map(|_| [0.0, 1.0]).collect();
        stitch.encode(encode);
    }
    stitch_ref
}

/// Write one content-embedded image XObject (`DeviceRGB`, Flate-
/// compressed) plus its own `/SMask` grayscale XObject when `alpha_mask`
/// carries real per-pixel transparency.
fn write_image_xobject(pdf: &mut Pdf, refs: &mut RefAllocator, rgb: &[u8], alpha_mask: Option<&[u8]>, px_w: u32, px_h: u32) -> Ref {
    let smask_ref = alpha_mask.map(|mask| {
        let sref = refs.next();
        let compressed = flate_compress(mask);
        let mut smask = pdf.image_xobject(sref, &compressed);
        smask.filter(Filter::FlateDecode);
        smask.width(px_w as i32);
        smask.height(px_h as i32);
        smask.color_space().device_gray();
        smask.bits_per_component(8);
        sref
    });

    let img_ref = refs.next();
    let compressed = flate_compress(rgb);
    let mut image = pdf.image_xobject(img_ref, &compressed);
    image.filter(Filter::FlateDecode);
    image.width(px_w as i32);
    image.height(px_h as i32);
    image.color_space().device_rgb();
    image.bits_per_component(8);
    if let Some(sref) = smask_ref {
        image.s_mask(sref);
    }
    img_ref
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::{PdfBuilder, PdfPageSpec};

    /// Build a single-page PDF from whatever `draw` issues against a
    /// fresh 200x150pt [`PdfRenderContext`], returning the finished byte
    /// buffer — the shared harness every test below drives.
    fn build_pdf(draw: impl FnOnce(&mut PdfRenderContext<'_>)) -> Vec<u8> {
        let mut builder = PdfBuilder::new();
        let mut fonts = PdfFontCache::new();
        let content = {
            let mut ctx = PdfRenderContext::new(1.0, &mut builder, &mut fonts);
            draw(&mut ctx);
            ctx.finish()
        };
        builder
            .add_page(PdfPageSpec { width_pt: 200.0, height_pt: 150.0, raster: None, raster_px: (0, 0), text_runs: Vec::new(), links: Vec::new(), content })
            .expect("add_page should succeed");
        builder.finish()
    }

    fn decompressed_content(bytes: &[u8]) -> String {
        let doc = lopdf::Document::load_mem(bytes).expect("lopdf must parse this backend's own PDF output");
        let pages = doc.get_pages();
        let page_id = *pages.values().next().expect("one page");
        let content = doc.get_page_content(page_id);
        String::from_utf8_lossy(&content).into_owned()
    }

    fn tokens(content: &str) -> Vec<&str> {
        content.split_whitespace().collect()
    }

    #[test]
    fn fill_rect_emits_real_path_and_fill_operators() {
        let bytes = build_pdf(|ctx| {
            ctx.set_fill_color("#ff0000");
            ctx.fill_rect(10.0, 20.0, 30.0, 40.0);
        });
        let content = decompressed_content(&bytes);
        let toks = tokens(&content);
        assert!(toks.contains(&"m"), "expected a moveto operator, got: {content}");
        assert!(toks.contains(&"l"), "expected a lineto operator, got: {content}");
        assert!(toks.contains(&"f"), "expected a nonzero fill operator, got: {content}");
        assert!(toks.contains(&"1"), "fill color 1 0 0 rg (red) must appear, got: {content}");
    }

    #[test]
    fn a_circle_drawn_via_arc_and_fill_emits_cubic_bezier_segments() {
        let bytes = build_pdf(|ctx| {
            ctx.set_fill_color("#00ff00");
            ctx.begin_path();
            ctx.arc(50.0, 50.0, 20.0, 0.0, std::f64::consts::TAU);
            ctx.fill();
        });
        let content = decompressed_content(&bytes);
        assert!(tokens(&content).contains(&"c"), "a bezier-approximated circle must contain cubic 'c' segments, got: {content}");
    }

    #[test]
    fn stroke_with_dash_emits_stroke_and_dash_pattern_operators() {
        let bytes = build_pdf(|ctx| {
            ctx.set_stroke_color("#0000ff");
            ctx.set_stroke_width(2.0);
            ctx.set_line_dash(&[4.0, 3.0]);
            ctx.begin_path();
            ctx.move_to(0.0, 0.0);
            ctx.line_to(100.0, 0.0);
            ctx.stroke();
        });
        let content = decompressed_content(&bytes);
        let toks = tokens(&content);
        assert!(toks.contains(&"S"), "expected a stroke operator, got: {content}");
        assert!(toks.contains(&"d"), "expected a dash-pattern operator, got: {content}");
    }

    #[test]
    fn global_alpha_emits_a_real_extgstate_with_ca_and_a_gs_invocation() {
        let bytes = build_pdf(|ctx| {
            ctx.set_fill_color("#ff0000");
            ctx.set_global_alpha(0.5);
            ctx.fill_rect(0.0, 0.0, 50.0, 50.0);
        });
        let content = decompressed_content(&bytes);
        assert!(tokens(&content).contains(&"gs"), "expected a 'gs' ExtGState invocation, got: {content}");

        let doc = lopdf::Document::load_mem(&bytes).expect("lopdf parse");
        let has_half_alpha_gstate = doc.objects.values().any(|obj| match obj {
            lopdf::Object::Dictionary(d) => d
                .get(b"ca")
                .ok()
                .and_then(|v| v.as_float().ok())
                .map(|v| (v - 0.5).abs() < 0.01)
                .unwrap_or(false),
            _ => false,
        });
        assert!(has_half_alpha_gstate, "expected a real /ExtGState object with /ca ~= 0.5");
    }

    #[test]
    fn clip_emits_a_real_w_n_sequence_scoped_by_q_and_q() {
        let bytes = build_pdf(|ctx| {
            ctx.save();
            ctx.begin_path();
            ctx.rect(0.0, 0.0, 50.0, 50.0);
            ctx.clip();
            ctx.set_fill_color("#123456");
            ctx.fill_rect(0.0, 0.0, 200.0, 150.0);
            ctx.restore();
        });
        let content = decompressed_content(&bytes);
        let toks = tokens(&content);
        assert!(toks.contains(&"q"), "expected save_state 'q', got: {content}");
        assert!(toks.contains(&"W"), "expected a clip 'W' operator, got: {content}");
        assert!(toks.contains(&"n"), "expected the no-op path-painting 'n' after W, got: {content}");
        assert!(toks.contains(&"Q"), "expected restore_state 'Q', got: {content}");
    }

    /// The money test: figure-style text (axis labels, titles) drawn
    /// through `fill_text` must be REAL, selectable/searchable Type0/CID
    /// PDF text — not baked into a raster background.
    #[test]
    fn fill_text_emits_real_cid_text_extractable_via_lopdf() {
        let mut builder = PdfBuilder::new();
        let mut fonts = PdfFontCache::new();
        // Pre-register Roboto/regular through the SHARED cache — mirrors
        // a paragraph run resolving this exact combination first (see
        // `pdf_adapter.rs`'s own `FontCache`/`PdfFontCache` usage).
        let font = fonts.id_for(uzor::fonts::FontFamily::Roboto, false, false, &mut builder);

        let content = {
            let mut ctx = PdfRenderContext::new(1.0, &mut builder, &mut fonts);
            ctx.set_font("14px sans-serif");
            ctx.set_fill_color("#000000");
            ctx.fill_text("Hop 1", 10.0, 20.0);
            ctx.finish()
        };
        // The SAME `PdfFontCache` must resolve `fill_text`'s own
        // "sans-serif" (-> Roboto/regular/regular) request to the
        // IDENTICAL id the pre-registration above already produced —
        // proving the shared registry actually dedupes across both
        // consumers, not just within one.
        let via_cache = fonts.id_for(uzor::fonts::FontFamily::Roboto, false, false, &mut builder);
        assert_eq!(font, via_cache, "PdfFontCache must resolve the SAME FontId across both call sites");

        builder
            .add_page(PdfPageSpec { width_pt: 200.0, height_pt: 150.0, raster: None, raster_px: (0, 0), text_runs: Vec::new(), links: Vec::new(), content })
            .expect("add_page should succeed");
        let bytes = builder.finish();

        let doc = lopdf::Document::load_mem(&bytes).expect("lopdf must parse this backend's own PDF output");
        let pages = doc.get_pages();
        let page_numbers: Vec<u32> = pages.keys().copied().collect();
        let extracted = doc.extract_text(&page_numbers).expect("lopdf text extraction must succeed");
        assert!(extracted.contains("Hop 1"), "extracted PDF text must contain the fill_text label verbatim, got: {extracted:?}");

        let page_id = *pages.values().next().expect("one page");
        let content_bytes = doc.get_page_content(page_id);
        let content_str = String::from_utf8_lossy(&content_bytes);
        assert!(content_str.contains("Tj"), "content stream must contain a Tj text-showing operator, got: {content_str}");
    }

    #[test]
    fn draw_image_rgba_embeds_a_real_xobject_with_a_do_invocation() {
        #[rustfmt::skip]
        let rgba: [u8; 16] = [
            255, 0, 0, 255,   0, 255, 0, 255,
              0, 0, 255, 255, 255, 255, 0, 255,
        ];
        let bytes = build_pdf(|ctx| {
            ctx.draw_image_rgba(&rgba, 2, 2, 10.0, 10.0, 40.0, 40.0);
        });
        let content = decompressed_content(&bytes);
        assert!(tokens(&content).contains(&"Do"), "expected an XObject 'Do' invocation, got: {content}");

        let doc = lopdf::Document::load_mem(&bytes).expect("lopdf parse");
        let pages = doc.get_pages();
        let page_id = *pages.values().next().expect("one page");
        let (resources, _) = doc.get_page_resources(page_id).expect("get_page_resources");
        let resources = resources.expect("Resources dict is inline");
        let x_objects = resources.get(b"XObject").and_then(lopdf::Object::as_dict).expect("XObject dict must be present");
        assert!(!x_objects.is_empty(), "the content-embedded image must appear in the page's own XObject dict");
    }

    #[test]
    fn linear_gradient_fill_emits_a_real_shading_pattern_not_a_flat_fallback() {
        let bytes = build_pdf(|ctx| {
            ctx.begin_path();
            ctx.rect(0.0, 0.0, 100.0, 20.0);
            ctx.fill_linear_gradient(&[(0.0, "#ff0000"), (1.0, "#0000ff")], 0.0, 0.0, 100.0, 0.0);
        });
        let content = decompressed_content(&bytes);
        assert!(tokens(&content).contains(&"scn"), "a pattern fill must use the 'scn' operator, got: {content}");

        let doc = lopdf::Document::load_mem(&bytes).expect("lopdf parse");
        let has_shading_pattern = doc.objects.values().any(|obj| match obj {
            lopdf::Object::Dictionary(d) => d.get(b"PatternType").and_then(|v| v.as_i64()).map(|v| v == 2).unwrap_or(false),
            _ => false,
        });
        assert!(has_shading_pattern, "expected a real /PatternType 2 (shading pattern) object, not a flat-fill fallback");
    }

    #[test]
    fn translate_and_scale_bake_into_the_emitted_fill_path() {
        let bytes = build_pdf(|ctx| {
            ctx.set_fill_color("#ffffff");
            ctx.translate(10.0, 20.0);
            ctx.scale(2.0, 3.0);
            ctx.fill_rect(0.0, 0.0, 5.0, 5.0);
        });
        let content = decompressed_content(&bytes);
        // (0,0) -> (10,20); page height 150 -> PDF y = 150-20 = 130.
        assert!(content.contains("10 130"), "expected the baked+flipped moveto at (10,130), got: {content}");
    }
}
