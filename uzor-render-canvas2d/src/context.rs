use std::collections::HashMap;

use js_sys::Array;
use uzor::core::types::Rect as UzorRect;
use uzor::render::{
    BatchPainter, BlendMode as UzorBlendMode,
    Effects, GradientPainter, Masking, OffscreenTarget, OffscreenTargetDesc, OffscreenTargetId,
    Painter, RenderContext, RenderContextExt,
    ShapeHelpers, TextBounds, TextMetrics, TextRenderer,
    TextAlign, TextBaseline,
};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{
    CanvasGradient, CanvasRenderingContext2d, OffscreenCanvas, OffscreenCanvasRenderingContext2d,
};

/// The surface currently receiving draw calls.
///
/// Normally the caller-supplied root `CanvasRenderingContext2d` (attached
/// to a real `<canvas>`/`OffscreenCanvas` element the browser composites).
/// While recording an offscreen target (`push_offscreen_target` …
/// `pop_offscreen_target`), draw calls instead land on a fresh, detached
/// `OffscreenCanvas`'s own 2D context — the DOM canvas stays untouched
/// until the fragment is composited back in via `draw_cached_target`'s
/// `drawImage`. Both context types expose an identical method surface
/// (verified 1:1 against every method this file calls — `OffscreenCanvas`
/// is a standard Web API built for exactly this "render off the visible
/// tree, composite later" use case), so the inherent methods below are
/// thin, exhaustive 2-arm matches, never a partial/lossy bridge.
enum Canvas2dCtx {
    Root(CanvasRenderingContext2d),
    Recording(OffscreenCanvasRenderingContext2d),
}

/// Macro-free dispatch: every method this backend calls exists with an
/// identical name/signature on both `CanvasRenderingContext2d` and
/// `OffscreenCanvasRenderingContext2d` (2D Canvas spec parity) — one match
/// arm per call keeps every existing `Painter`/`TextRenderer`/etc. impl
/// body below completely unchanged (`self.ctx.foo(...)` still just works).
impl Canvas2dCtx {
    fn save(&self) { match self { Self::Root(c) => c.save(), Self::Recording(c) => c.save() } }
    fn restore(&self) { match self { Self::Root(c) => c.restore(), Self::Recording(c) => c.restore() } }
    fn translate(&self, x: f64, y: f64) -> Result<(), JsValue> {
        match self { Self::Root(c) => c.translate(x, y), Self::Recording(c) => c.translate(x, y) }
    }
    fn rotate(&self, angle: f64) -> Result<(), JsValue> {
        match self { Self::Root(c) => c.rotate(angle), Self::Recording(c) => c.rotate(angle) }
    }
    fn scale(&self, x: f64, y: f64) -> Result<(), JsValue> {
        match self { Self::Root(c) => c.scale(x, y), Self::Recording(c) => c.scale(x, y) }
    }
    fn set_fill_style_str(&self, color: &str) {
        match self { Self::Root(c) => c.set_fill_style_str(color), Self::Recording(c) => c.set_fill_style_str(color) }
    }
    fn set_global_alpha(&self, alpha: f64) {
        match self { Self::Root(c) => c.set_global_alpha(alpha), Self::Recording(c) => c.set_global_alpha(alpha) }
    }
    fn set_stroke_style_str(&self, color: &str) {
        match self { Self::Root(c) => c.set_stroke_style_str(color), Self::Recording(c) => c.set_stroke_style_str(color) }
    }
    fn set_line_width(&self, width: f64) {
        match self { Self::Root(c) => c.set_line_width(width), Self::Recording(c) => c.set_line_width(width) }
    }
    fn set_line_dash(&self, segments: &JsValue) -> Result<(), JsValue> {
        match self { Self::Root(c) => c.set_line_dash(segments), Self::Recording(c) => c.set_line_dash(segments) }
    }
    fn set_line_cap(&self, cap: &str) {
        match self { Self::Root(c) => c.set_line_cap(cap), Self::Recording(c) => c.set_line_cap(cap) }
    }
    fn set_line_join(&self, join: &str) {
        match self { Self::Root(c) => c.set_line_join(join), Self::Recording(c) => c.set_line_join(join) }
    }
    fn begin_path(&self) { match self { Self::Root(c) => c.begin_path(), Self::Recording(c) => c.begin_path() } }
    fn move_to(&self, x: f64, y: f64) {
        match self { Self::Root(c) => c.move_to(x, y), Self::Recording(c) => c.move_to(x, y) }
    }
    fn line_to(&self, x: f64, y: f64) {
        match self { Self::Root(c) => c.line_to(x, y), Self::Recording(c) => c.line_to(x, y) }
    }
    fn close_path(&self) { match self { Self::Root(c) => c.close_path(), Self::Recording(c) => c.close_path() } }
    fn rect(&self, x: f64, y: f64, w: f64, h: f64) {
        match self { Self::Root(c) => c.rect(x, y, w, h), Self::Recording(c) => c.rect(x, y, w, h) }
    }
    fn arc(&self, cx: f64, cy: f64, r: f64, start: f64, end: f64) -> Result<(), JsValue> {
        match self { Self::Root(c) => c.arc(cx, cy, r, start, end), Self::Recording(c) => c.arc(cx, cy, r, start, end) }
    }
    fn ellipse(&self, cx: f64, cy: f64, rx: f64, ry: f64, rot: f64, start: f64, end: f64) -> Result<(), JsValue> {
        match self {
            Self::Root(c) => c.ellipse(cx, cy, rx, ry, rot, start, end),
            Self::Recording(c) => c.ellipse(cx, cy, rx, ry, rot, start, end),
        }
    }
    fn quadratic_curve_to(&self, cpx: f64, cpy: f64, x: f64, y: f64) {
        match self {
            Self::Root(c) => c.quadratic_curve_to(cpx, cpy, x, y),
            Self::Recording(c) => c.quadratic_curve_to(cpx, cpy, x, y),
        }
    }
    fn bezier_curve_to(&self, cp1x: f64, cp1y: f64, cp2x: f64, cp2y: f64, x: f64, y: f64) {
        match self {
            Self::Root(c) => c.bezier_curve_to(cp1x, cp1y, cp2x, cp2y, x, y),
            Self::Recording(c) => c.bezier_curve_to(cp1x, cp1y, cp2x, cp2y, x, y),
        }
    }
    fn stroke(&self) { match self { Self::Root(c) => c.stroke(), Self::Recording(c) => c.stroke() } }
    fn fill(&self) { match self { Self::Root(c) => c.fill(), Self::Recording(c) => c.fill() } }
    fn set_font(&self, font: &str) {
        match self { Self::Root(c) => c.set_font(font), Self::Recording(c) => c.set_font(font) }
    }
    fn set_text_align(&self, value: &str) {
        match self { Self::Root(c) => c.set_text_align(value), Self::Recording(c) => c.set_text_align(value) }
    }
    fn set_text_baseline(&self, value: &str) {
        match self { Self::Root(c) => c.set_text_baseline(value), Self::Recording(c) => c.set_text_baseline(value) }
    }
    fn fill_text(&self, text: &str, x: f64, y: f64) -> Result<(), JsValue> {
        match self { Self::Root(c) => c.fill_text(text, x, y), Self::Recording(c) => c.fill_text(text, x, y) }
    }
    fn stroke_text(&self, text: &str, x: f64, y: f64) -> Result<(), JsValue> {
        match self { Self::Root(c) => c.stroke_text(text, x, y), Self::Recording(c) => c.stroke_text(text, x, y) }
    }
    fn measure_text(&self, text: &str) -> Result<web_sys::TextMetrics, JsValue> {
        match self { Self::Root(c) => c.measure_text(text), Self::Recording(c) => c.measure_text(text) }
    }
    fn clip(&self) { match self { Self::Root(c) => c.clip(), Self::Recording(c) => c.clip() } }
    fn set_shadow_offset_x(&self, v: f64) {
        match self { Self::Root(c) => c.set_shadow_offset_x(v), Self::Recording(c) => c.set_shadow_offset_x(v) }
    }
    fn set_shadow_offset_y(&self, v: f64) {
        match self { Self::Root(c) => c.set_shadow_offset_y(v), Self::Recording(c) => c.set_shadow_offset_y(v) }
    }
    fn set_shadow_blur(&self, v: f64) {
        match self { Self::Root(c) => c.set_shadow_blur(v), Self::Recording(c) => c.set_shadow_blur(v) }
    }
    fn set_shadow_color(&self, v: &str) {
        match self { Self::Root(c) => c.set_shadow_color(v), Self::Recording(c) => c.set_shadow_color(v) }
    }
    fn set_global_composite_operation(&self, op: &str) -> Result<(), JsValue> {
        match self {
            Self::Root(c) => c.set_global_composite_operation(op),
            Self::Recording(c) => c.set_global_composite_operation(op),
        }
    }
    fn fill_rect(&self, x: f64, y: f64, w: f64, h: f64) {
        match self { Self::Root(c) => c.fill_rect(x, y, w, h), Self::Recording(c) => c.fill_rect(x, y, w, h) }
    }
    fn stroke_rect(&self, x: f64, y: f64, w: f64, h: f64) {
        match self { Self::Root(c) => c.stroke_rect(x, y, w, h), Self::Recording(c) => c.stroke_rect(x, y, w, h) }
    }
    fn create_linear_gradient(&self, x1: f64, y1: f64, x2: f64, y2: f64) -> CanvasGradient {
        match self {
            Self::Root(c) => c.create_linear_gradient(x1, y1, x2, y2),
            Self::Recording(c) => c.create_linear_gradient(x1, y1, x2, y2),
        }
    }
    fn create_radial_gradient(
        &self, x1: f64, y1: f64, r1: f64, x2: f64, y2: f64, r2: f64,
    ) -> Result<CanvasGradient, JsValue> {
        match self {
            Self::Root(c) => c.create_radial_gradient(x1, y1, r1, x2, y2, r2),
            Self::Recording(c) => c.create_radial_gradient(x1, y1, r1, x2, y2, r2),
        }
    }
    fn set_fill_style_canvas_gradient(&self, gradient: &CanvasGradient) {
        match self {
            Self::Root(c) => c.set_fill_style_canvas_gradient(gradient),
            Self::Recording(c) => c.set_fill_style_canvas_gradient(gradient),
        }
    }
}

/// Canvas 2D render context for WebAssembly targets.
///
/// Wraps `web_sys::CanvasRenderingContext2d` (or, while recording an
/// offscreen target, an `OffscreenCanvasRenderingContext2d`) and
/// implements the `uzor::render::RenderContext` trait. This is the
/// canonical backend for browser-based rendering via the HTML Canvas API.
pub struct Canvas2dRenderContext {
    ctx: Canvas2dCtx,
    dpr: f64,

    // Offscreen-target support — `OffscreenCanvas`/`drawImage` composite
    // cache. See the `RenderContext` impl below for the full rationale.
    offscreen_targets: HashMap<OffscreenTargetId, CachedTarget>,
    next_offscreen_id: u64,
    offscreen_stack: Vec<SavedRecording>,
}

/// Recording state swapped out while painting into an offscreen target —
/// same "swap current surface, stash the previous one" discipline every
/// other backend's offscreen-target implementation uses.
struct SavedRecording {
    id:  OffscreenTargetId,
    ctx: Canvas2dCtx,
}

/// Cached offscreen-target content: the detached `OffscreenCanvas` a
/// recording rendered into, replayed via `drawImage` on every subsequent
/// `draw_cached_target` call — no re-rendering, matching the "cache"
/// contract (`OffscreenCanvas` retains its drawn pixels between frames,
/// same as `tiny_skia::Pixmap`).
struct CachedTarget {
    canvas: OffscreenCanvas,
    width:  u32,
    height: u32,
}

impl Canvas2dRenderContext {
    /// Create a new context from an existing `CanvasRenderingContext2d`.
    ///
    /// `dpr` is the device pixel ratio (e.g. `window.devicePixelRatio`).
    pub fn new(ctx: CanvasRenderingContext2d, dpr: f64) -> Self {
        Self {
            ctx: Canvas2dCtx::Root(ctx),
            dpr,
            offscreen_targets: HashMap::new(),
            next_offscreen_id: 0,
            offscreen_stack:   Vec::new(),
        }
    }

    /// Access the underlying `CanvasRenderingContext2d` — `None` while an
    /// offscreen target is being recorded (the active surface is a
    /// detached `OffscreenCanvasRenderingContext2d` then, not the root).
    pub fn canvas_context(&self) -> Option<&CanvasRenderingContext2d> {
        match &self.ctx {
            Canvas2dCtx::Root(c) => Some(c),
            Canvas2dCtx::Recording(_) => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Painter
// ---------------------------------------------------------------------------

impl Painter for Canvas2dRenderContext {
    fn save(&mut self) { self.ctx.save(); }
    fn restore(&mut self) { self.ctx.restore(); }
    fn translate(&mut self, x: f64, y: f64) { let _ = self.ctx.translate(x, y); }
    fn rotate(&mut self, angle: f64) { let _ = self.ctx.rotate(angle); }
    fn scale(&mut self, x: f64, y: f64) { let _ = self.ctx.scale(x, y); }
    fn set_fill_color(&mut self, color: &str) { self.ctx.set_fill_style_str(color); }
    fn set_global_alpha(&mut self, alpha: f64) { self.ctx.set_global_alpha(alpha); }
    fn set_stroke_color(&mut self, color: &str) { self.ctx.set_stroke_style_str(color); }
    fn set_stroke_width(&mut self, width: f64) { self.ctx.set_line_width(width); }
    fn set_line_dash(&mut self, pattern: &[f64]) {
        let arr = Array::new();
        for &v in pattern { arr.push(&JsValue::from_f64(v)); }
        let _ = self.ctx.set_line_dash(&arr);
    }
    fn set_line_cap(&mut self, cap: &str) { self.ctx.set_line_cap(cap); }
    fn set_line_join(&mut self, join: &str) { self.ctx.set_line_join(join); }
    fn begin_path(&mut self) { self.ctx.begin_path(); }
    fn move_to(&mut self, x: f64, y: f64) { self.ctx.move_to(x, y); }
    fn line_to(&mut self, x: f64, y: f64) { self.ctx.line_to(x, y); }
    fn close_path(&mut self) { self.ctx.close_path(); }
    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64) { self.ctx.rect(x, y, w, h); }
    fn arc(&mut self, cx: f64, cy: f64, radius: f64, start_angle: f64, end_angle: f64) {
        let _ = self.ctx.arc(cx, cy, radius, start_angle, end_angle);
    }
    fn ellipse(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, rotation: f64, start: f64, end: f64) {
        let _ = self.ctx.ellipse(cx, cy, rx, ry, rotation, start, end);
    }
    fn quadratic_curve_to(&mut self, cpx: f64, cpy: f64, x: f64, y: f64) {
        self.ctx.quadratic_curve_to(cpx, cpy, x, y);
    }
    fn bezier_curve_to(&mut self, cp1x: f64, cp1y: f64, cp2x: f64, cp2y: f64, x: f64, y: f64) {
        self.ctx.bezier_curve_to(cp1x, cp1y, cp2x, cp2y, x, y);
    }
    fn stroke(&mut self) { self.ctx.stroke(); }
    fn fill(&mut self) { self.ctx.fill(); }
}

// ---------------------------------------------------------------------------
// TextRenderer
// ---------------------------------------------------------------------------

impl TextRenderer for Canvas2dRenderContext {
    fn set_font(&mut self, font: &str) { self.ctx.set_font(font); }
    fn set_text_align(&mut self, align: TextAlign) {
        let value = match align {
            TextAlign::Left   => "left",
            TextAlign::Center => "center",
            TextAlign::Right  => "right",
        };
        self.ctx.set_text_align(value);
    }
    fn set_text_baseline(&mut self, baseline: TextBaseline) {
        let value = match baseline {
            TextBaseline::Top        => "top",
            TextBaseline::Middle     => "middle",
            TextBaseline::Bottom     => "bottom",
            TextBaseline::Alphabetic => "alphabetic",
        };
        self.ctx.set_text_baseline(value);
    }
    fn fill_text(&mut self, text: &str, x: f64, y: f64) {
        let _ = self.ctx.fill_text(text, x, y);
    }
    fn stroke_text(&mut self, text: &str, x: f64, y: f64) {
        let _ = self.ctx.stroke_text(text, x, y);
    }
}

// ---------------------------------------------------------------------------
// TextMetrics
// ---------------------------------------------------------------------------

impl TextMetrics for Canvas2dRenderContext {
    fn measure_text(&self, text: &str) -> f64 {
        self.ctx.measure_text(text).map(|m| m.width()).unwrap_or(0.0)
    }

    // TODO(phase-4): wire real font parsing via uzor::fonts for canvas2d backend.
    // Canvas2D exposes TextMetrics.actualBoundingBoxAscent/Descent but only after
    // a measureText() call; for now we use approximation constants.
    fn text_bounds(&self, text: &str, font: &str) -> TextBounds {
        let font_size = uzor::fonts::parse_css_font(font).size as f64;
        let w = self.ctx.measure_text(text).map(|m| m.width()).unwrap_or_else(|_| {
            text.chars().count() as f64 * font_size * 0.6
        });
        let ascent  = font_size * 0.9;
        let descent = font_size * 0.3;
        TextBounds {
            x: 0.0,
            y: -ascent,
            w,
            h: ascent + descent,
            ascent,
            descent,
        }
    }
}

// ---------------------------------------------------------------------------
// Masking — Canvas2D clip(); default push_mask/pop_mask via save+clip work fine
// ---------------------------------------------------------------------------

impl Masking for Canvas2dRenderContext {
    fn clip(&mut self) { self.ctx.clip(); }
}

// ---------------------------------------------------------------------------
// Effects
// ---------------------------------------------------------------------------

impl Effects for Canvas2dRenderContext {
    fn set_shadow(&mut self, dx: f64, dy: f64, blur: f64, color: &str) {
        self.ctx.set_shadow_offset_x(dx);
        self.ctx.set_shadow_offset_y(dy);
        self.ctx.set_shadow_blur(blur);
        self.ctx.set_shadow_color(color);
    }

    fn clear_shadow(&mut self) {
        self.ctx.set_shadow_offset_x(0.0);
        self.ctx.set_shadow_offset_y(0.0);
        self.ctx.set_shadow_blur(0.0);
        self.ctx.set_shadow_color("transparent");
    }

    fn set_blend_mode(&mut self, mode: UzorBlendMode) {
        let op = match mode {
            UzorBlendMode::Normal     => "source-over",
            UzorBlendMode::Multiply   => "multiply",
            UzorBlendMode::Screen     => "screen",
            UzorBlendMode::Overlay    => "overlay",
            UzorBlendMode::Darken     => "darken",
            UzorBlendMode::Lighten    => "lighten",
            UzorBlendMode::ColorDodge => "color-dodge",
            UzorBlendMode::ColorBurn  => "color-burn",
            UzorBlendMode::HardLight  => "hard-light",
            UzorBlendMode::SoftLight  => "soft-light",
            UzorBlendMode::Difference => "difference",
            UzorBlendMode::Exclusion  => "exclusion",
            UzorBlendMode::Plus       => "lighter",
        };
        let _ = self.ctx.set_global_composite_operation(op);
    }
}

// ---------------------------------------------------------------------------
// ShapeHelpers — fill_rect and stroke_rect native; rounded_rect_corners override
// ---------------------------------------------------------------------------

impl ShapeHelpers for Canvas2dRenderContext {
    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64) { self.ctx.fill_rect(x, y, w, h); }
    fn stroke_rect(&mut self, x: f64, y: f64, w: f64, h: f64) { self.ctx.stroke_rect(x, y, w, h); }

    fn rounded_rect_corners(&mut self, x: f64, y: f64, w: f64, h: f64, tl: f64, tr: f64, br: f64, bl: f64) {
        let max_r = (w / 2.0).min(h / 2.0).max(0.0);
        let tl = tl.clamp(0.0, max_r);
        let tr = tr.clamp(0.0, max_r);
        let br = br.clamp(0.0, max_r);
        let bl = bl.clamp(0.0, max_r);
        self.begin_path();
        self.move_to(x + tl, y);
        self.line_to(x + w - tr, y);
        self.arc(x + w - tr, y + tr, tr, -std::f64::consts::FRAC_PI_2, 0.0);
        self.line_to(x + w, y + h - br);
        self.arc(x + w - br, y + h - br, br, 0.0, std::f64::consts::FRAC_PI_2);
        self.line_to(x + bl, y + h);
        self.arc(x + bl, y + h - bl, bl, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
        self.line_to(x, y + tl);
        self.arc(x + tl, y + tl, tl, std::f64::consts::PI, std::f64::consts::PI * 1.5);
        self.close_path();
    }
}

// ---------------------------------------------------------------------------
// GradientPainter
// ---------------------------------------------------------------------------

impl GradientPainter for Canvas2dRenderContext {
    fn fill_linear_gradient(&mut self, stops: &[(f32, &str)], x1: f64, y1: f64, x2: f64, y2: f64) {
        let gradient: CanvasGradient = self.ctx.create_linear_gradient(x1, y1, x2, y2);
        for &(offset, color) in stops {
            let _ = gradient.add_color_stop(offset, color);
        }
        self.ctx.set_fill_style_canvas_gradient(&gradient);
        self.ctx.fill();
    }

    fn fill_radial_gradient(&mut self, cx: f64, cy: f64, r: f64, stops: &[(f32, &str)], x: f64, y: f64, w: f64, h: f64) {
        let _ = (x, y, w, h);
        if let Ok(gradient) = self.ctx.create_radial_gradient(cx, cy, 0.0, cx, cy, r) {
            for &(offset, color) in stops {
                let _ = gradient.add_color_stop(offset, color);
            }
            self.ctx.set_fill_style_canvas_gradient(&gradient);
        }
        self.ctx.fill();
    }
}

// ---------------------------------------------------------------------------
// UiEffectHelpers — all defaults (no blur support)
// ---------------------------------------------------------------------------

impl uzor::render::UiEffectHelpers for Canvas2dRenderContext {}

// ---------------------------------------------------------------------------
// BatchPainter — default impl (JS bridge call cost dominates; no batching win)
// ---------------------------------------------------------------------------

impl BatchPainter for Canvas2dRenderContext {}

// ---------------------------------------------------------------------------
// RenderContext — offscreen targets via a detached `OffscreenCanvas`,
// composited back with `drawImage` (the standard Web API built for
// exactly this "render off the visible tree, replay later" use case).
// ---------------------------------------------------------------------------
//
// `push_offscreen_target` creates a fresh `OffscreenCanvas` sized to the
// requested target, gets its 2D context, and swaps `self.ctx` to point at
// it (same "swap current surface" discipline as tiny-skia's `Pixmap` swap
// — draw calls after this point land on the detached canvas, not the DOM
// one). `pop_offscreen_target` restores the outer surface and stashes the
// `OffscreenCanvas` itself as the cached target — its drawn pixels persist
// (an `OffscreenCanvas` is a real retained bitmap, exactly like
// `tiny_skia::Pixmap`), so `draw_cached_target` replays with a single
// `drawImage` call, no re-rendering.
impl RenderContext for Canvas2dRenderContext {
    fn dpr(&self) -> f64 { self.dpr }

    fn supports_offscreen_targets(&self) -> bool {
        true
    }

    fn push_offscreen_target(&mut self, desc: OffscreenTargetDesc) -> OffscreenTarget {
        let w = desc.width_px.max(1);
        let h = desc.height_px.max(1);

        let canvas = OffscreenCanvas::new(w, h).ok()?;
        let ctx2d = canvas
            .get_context("2d")
            .ok()
            .flatten()?
            .dyn_into::<OffscreenCanvasRenderingContext2d>()
            .ok()?;

        let id = OffscreenTargetId(self.next_offscreen_id);
        self.next_offscreen_id += 1;

        let previous = std::mem::replace(&mut self.ctx, Canvas2dCtx::Recording(ctx2d));
        self.offscreen_stack.push(SavedRecording { id, ctx: previous });

        Some(id)
    }

    fn pop_offscreen_target(&mut self) {
        let Some(saved) = self.offscreen_stack.pop() else {
            return;
        };
        let recorded = std::mem::replace(&mut self.ctx, saved.ctx);
        let Canvas2dCtx::Recording(ctx2d) = recorded else {
            // `push_offscreen_target` always swaps in `Recording` — a
            // `Root` value here would mean the stack was corrupted.
            // Unreachable in practice; stay total (no cache entry) rather
            // than panicking.
            return;
        };
        let canvas = ctx2d.canvas();
        self.offscreen_targets.insert(saved.id, CachedTarget {
            width:  canvas.width(),
            height: canvas.height(),
            canvas,
        });
    }

    fn draw_cached_target(&mut self, id: OffscreenTargetId, dst_rect: UzorRect) -> bool {
        if dst_rect.width <= 0.0 || dst_rect.height <= 0.0 {
            return false;
        }
        let Some(target) = self.offscreen_targets.get(&id) else {
            return false;
        };
        let draw = match &self.ctx {
            Canvas2dCtx::Root(c) => c.draw_image_with_offscreen_canvas_and_dw_and_dh(
                &target.canvas, dst_rect.x, dst_rect.y, dst_rect.width, dst_rect.height,
            ),
            Canvas2dCtx::Recording(c) => c.draw_image_with_offscreen_canvas_and_dw_and_dh(
                &target.canvas, dst_rect.x, dst_rect.y, dst_rect.width, dst_rect.height,
            ),
        };
        draw.is_ok()
    }

    fn resize_offscreen_target(&mut self, id: OffscreenTargetId, desc: OffscreenTargetDesc) -> bool {
        // `OffscreenCanvas` content is invalidated the instant its
        // `width`/`height` change (per spec, resizing clears the bitmap) —
        // report success only for a no-op resize, same "caller `free` +
        // `push` a new target otherwise" contract every other backend uses.
        self.offscreen_targets
            .get(&id)
            .is_some_and(|t| t.width == desc.width_px.max(1) && t.height == desc.height_px.max(1))
    }

    fn free_offscreen_target(&mut self, id: OffscreenTargetId) {
        self.offscreen_targets.remove(&id);
    }
}

impl RenderContextExt for Canvas2dRenderContext {
    type BlurImage = ();
}
