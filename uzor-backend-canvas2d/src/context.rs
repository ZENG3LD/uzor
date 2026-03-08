use js_sys::Array;
use uzor_core::render::{RenderContext, RenderContextExt, TextAlign, TextBaseline};
use wasm_bindgen::JsValue;
use web_sys::CanvasRenderingContext2d;

/// Canvas 2D render context for WebAssembly targets.
///
/// Wraps `web_sys::CanvasRenderingContext2d` and implements the
/// `uzor_core::render::RenderContext` trait. This is the canonical backend for
/// browser-based rendering via the HTML Canvas API.
pub struct Canvas2dRenderContext {
    ctx: CanvasRenderingContext2d,
    dpr: f64,
}

impl Canvas2dRenderContext {
    /// Create a new context from an existing `CanvasRenderingContext2d`.
    ///
    /// `dpr` is the device pixel ratio (e.g. `window.devicePixelRatio`).
    pub fn new(ctx: CanvasRenderingContext2d, dpr: f64) -> Self {
        Self { ctx, dpr }
    }

    /// Access the underlying `CanvasRenderingContext2d`.
    pub fn canvas_context(&self) -> &CanvasRenderingContext2d {
        &self.ctx
    }
}

impl RenderContext for Canvas2dRenderContext {
    // -------------------------------------------------------------------------
    // Dimensions
    // -------------------------------------------------------------------------

    fn dpr(&self) -> f64 {
        self.dpr
    }

    // -------------------------------------------------------------------------
    // Stroke style
    // -------------------------------------------------------------------------

    fn set_stroke_color(&mut self, color: &str) {
        self.ctx.set_stroke_style_str(color);
    }

    fn set_stroke_width(&mut self, width: f64) {
        self.ctx.set_line_width(width);
    }

    fn set_line_dash(&mut self, pattern: &[f64]) {
        let arr = Array::new();
        for &v in pattern {
            arr.push(&JsValue::from_f64(v));
        }
        let _ = self.ctx.set_line_dash(&arr);
    }

    fn set_line_cap(&mut self, cap: &str) {
        self.ctx.set_line_cap(cap);
    }

    fn set_line_join(&mut self, join: &str) {
        self.ctx.set_line_join(join);
    }

    // -------------------------------------------------------------------------
    // Fill style
    // -------------------------------------------------------------------------

    fn set_fill_color(&mut self, color: &str) {
        self.ctx.set_fill_style_str(color);
    }

    fn set_global_alpha(&mut self, alpha: f64) {
        self.ctx.set_global_alpha(alpha);
    }

    // -------------------------------------------------------------------------
    // Path operations
    // -------------------------------------------------------------------------

    fn begin_path(&mut self) {
        self.ctx.begin_path();
    }

    fn move_to(&mut self, x: f64, y: f64) {
        self.ctx.move_to(x, y);
    }

    fn line_to(&mut self, x: f64, y: f64) {
        self.ctx.line_to(x, y);
    }

    fn close_path(&mut self) {
        self.ctx.close_path();
    }

    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.ctx.rect(x, y, w, h);
    }

    fn arc(&mut self, cx: f64, cy: f64, radius: f64, start_angle: f64, end_angle: f64) {
        let _ = self.ctx.arc(cx, cy, radius, start_angle, end_angle);
    }

    fn ellipse(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        rotation: f64,
        start: f64,
        end: f64,
    ) {
        let _ = self.ctx.ellipse(cx, cy, rx, ry, rotation, start, end);
    }

    fn quadratic_curve_to(&mut self, cpx: f64, cpy: f64, x: f64, y: f64) {
        self.ctx.quadratic_curve_to(cpx, cpy, x, y);
    }

    fn bezier_curve_to(
        &mut self,
        cp1x: f64,
        cp1y: f64,
        cp2x: f64,
        cp2y: f64,
        x: f64,
        y: f64,
    ) {
        self.ctx.bezier_curve_to(cp1x, cp1y, cp2x, cp2y, x, y);
    }

    // -------------------------------------------------------------------------
    // Stroke / fill operations
    // -------------------------------------------------------------------------

    fn stroke(&mut self) {
        self.ctx.stroke();
    }

    fn fill(&mut self) {
        self.ctx.fill();
    }

    fn clip(&mut self) {
        self.ctx.clip();
    }

    fn stroke_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.ctx.stroke_rect(x, y, w, h);
    }

    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.ctx.fill_rect(x, y, w, h);
    }

    // -------------------------------------------------------------------------
    // Text rendering
    // -------------------------------------------------------------------------

    fn set_font(&mut self, font: &str) {
        self.ctx.set_font(font);
    }

    fn set_text_align(&mut self, align: TextAlign) {
        let value = match align {
            TextAlign::Left => "left",
            TextAlign::Center => "center",
            TextAlign::Right => "right",
        };
        self.ctx.set_text_align(value);
    }

    fn set_text_baseline(&mut self, baseline: TextBaseline) {
        let value = match baseline {
            TextBaseline::Top => "top",
            TextBaseline::Middle => "middle",
            TextBaseline::Bottom => "bottom",
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

    fn measure_text(&self, text: &str) -> f64 {
        self.ctx
            .measure_text(text)
            .map(|m| m.width())
            .unwrap_or(0.0)
    }

    // -------------------------------------------------------------------------
    // Transform operations
    // -------------------------------------------------------------------------

    fn save(&mut self) {
        self.ctx.save();
    }

    fn restore(&mut self) {
        self.ctx.restore();
    }

    fn translate(&mut self, x: f64, y: f64) {
        let _ = self.ctx.translate(x, y);
    }

    fn rotate(&mut self, angle: f64) {
        let _ = self.ctx.rotate(angle);
    }

    fn scale(&mut self, x: f64, y: f64) {
        let _ = self.ctx.scale(x, y);
    }
}

impl RenderContextExt for Canvas2dRenderContext {
    type BlurImage = ();
}
