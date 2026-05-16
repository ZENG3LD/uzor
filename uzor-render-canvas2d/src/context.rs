use js_sys::Array;
use uzor::render::{
    BatchPainter, BlendMode as UzorBlendMode,
    Effects, GradientPainter, Masking, Painter, RenderContext, RenderContextExt,
    ShapeHelpers, TextBounds, TextMetrics, TextRenderer,
    TextAlign, TextBaseline,
};
use wasm_bindgen::JsValue;
use web_sys::{CanvasGradient, CanvasRenderingContext2d};

/// Canvas 2D render context for WebAssembly targets.
///
/// Wraps `web_sys::CanvasRenderingContext2d` and implements the
/// `uzor::render::RenderContext` trait. This is the canonical backend for
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
// RenderContext (dpr only)
// ---------------------------------------------------------------------------

impl RenderContext for Canvas2dRenderContext {
    fn dpr(&self) -> f64 { self.dpr }
}

impl RenderContextExt for Canvas2dRenderContext {
    type BlurImage = ();
}
