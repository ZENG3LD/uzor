//! Backend-free [`RenderContext`] doubles for headless draw tests.
//!
//! - [`NullRenderContext`] discards every call; text measures zero wide.
//!   Use it when a test only needs a widget's draw pass to run (hit rects,
//!   state changes, "does not panic").
//! - [`RecordingRenderContext`] records every call as a [`DrawOp`], in
//!   order, so a test can assert which colours, fonts, texts and rects a
//!   draw pass produced without rasterising anything.
//!
//! Both report `dpr() == 1.0` and use the traits' default implementations
//! for every provided method (rounded rects, gradients, effects), so the
//! recorded stream is what those defaults lower to.

use crate::render::{
    BatchPainter, Effects, GradientPainter, Masking, Painter, RenderContext, ShapeHelpers,
    TextAlign, TextBaseline, TextBounds, TextMetrics, TextRenderer, UiEffectHelpers,
};

/// A [`RenderContext`] that draws nothing and measures every text as zero
/// wide.
#[derive(Clone, Copy, Debug, Default)]
pub struct NullRenderContext;

impl Painter for NullRenderContext {
    fn save(&mut self) {}
    fn restore(&mut self) {}
    fn translate(&mut self, _x: f64, _y: f64) {}
    fn rotate(&mut self, _angle: f64) {}
    fn scale(&mut self, _x: f64, _y: f64) {}
    fn set_fill_color(&mut self, _color: &str) {}
    fn set_global_alpha(&mut self, _alpha: f64) {}
    fn set_stroke_color(&mut self, _color: &str) {}
    fn set_stroke_width(&mut self, _width: f64) {}
    fn set_line_dash(&mut self, _pattern: &[f64]) {}
    fn set_line_cap(&mut self, _cap: &str) {}
    fn set_line_join(&mut self, _join: &str) {}
    fn begin_path(&mut self) {}
    fn move_to(&mut self, _x: f64, _y: f64) {}
    fn line_to(&mut self, _x: f64, _y: f64) {}
    fn close_path(&mut self) {}
    fn rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {}
    fn arc(&mut self, _cx: f64, _cy: f64, _radius: f64, _start: f64, _end: f64) {}
    fn ellipse(&mut self, _cx: f64, _cy: f64, _rx: f64, _ry: f64, _rotation: f64, _start: f64, _end: f64) {}
    fn quadratic_curve_to(&mut self, _cpx: f64, _cpy: f64, _x: f64, _y: f64) {}
    fn bezier_curve_to(&mut self, _cp1x: f64, _cp1y: f64, _cp2x: f64, _cp2y: f64, _x: f64, _y: f64) {}
    fn stroke(&mut self) {}
    fn fill(&mut self) {}
}

impl TextRenderer for NullRenderContext {
    fn set_font(&mut self, _font: &str) {}
    fn set_text_align(&mut self, _align: TextAlign) {}
    fn set_text_baseline(&mut self, _baseline: TextBaseline) {}
    fn fill_text(&mut self, _text: &str, _x: f64, _y: f64) {}
    fn stroke_text(&mut self, _text: &str, _x: f64, _y: f64) {}
}

impl TextMetrics for NullRenderContext {
    fn measure_text(&self, _text: &str) -> f64 {
        0.0
    }
    fn text_bounds(&self, _text: &str, _font: &str) -> TextBounds {
        TextBounds { x: 0.0, y: 0.0, w: 0.0, h: 0.0, ascent: 0.0, descent: 0.0 }
    }
}

impl Masking for NullRenderContext {
    fn clip(&mut self) {}
}
impl Effects for NullRenderContext {}
impl ShapeHelpers for NullRenderContext {
    fn stroke_rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {}
    fn fill_rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {}
}
impl GradientPainter for NullRenderContext {}
impl UiEffectHelpers for NullRenderContext {}
impl BatchPainter for NullRenderContext {}
impl RenderContext for NullRenderContext {
    fn dpr(&self) -> f64 {
        1.0
    }
}

/// One recorded call on a [`RecordingRenderContext`].
#[derive(Clone, Debug, PartialEq)]
pub enum DrawOp {
    Save,
    Restore,
    Translate { x: f64, y: f64 },
    Rotate { angle: f64 },
    Scale { x: f64, y: f64 },
    FillColor(String),
    GlobalAlpha(f64),
    StrokeColor(String),
    StrokeWidth(f64),
    LineDash(Vec<f64>),
    LineCap(String),
    LineJoin(String),
    BeginPath,
    MoveTo { x: f64, y: f64 },
    LineTo { x: f64, y: f64 },
    ClosePath,
    Rect { x: f64, y: f64, w: f64, h: f64 },
    Arc { cx: f64, cy: f64, radius: f64, start: f64, end: f64 },
    Ellipse { cx: f64, cy: f64, rx: f64, ry: f64, rotation: f64, start: f64, end: f64 },
    QuadraticCurveTo { cpx: f64, cpy: f64, x: f64, y: f64 },
    BezierCurveTo { cp1x: f64, cp1y: f64, cp2x: f64, cp2y: f64, x: f64, y: f64 },
    Stroke,
    Fill,
    Font(String),
    TextAlign(TextAlign),
    TextBaseline(TextBaseline),
    FillText { text: String, x: f64, y: f64 },
    StrokeText { text: String, x: f64, y: f64 },
    Clip,
    FillRect { x: f64, y: f64, w: f64, h: f64 },
    StrokeRect { x: f64, y: f64, w: f64, h: f64 },
}

/// A [`RenderContext`] that records every call as a [`DrawOp`].
///
/// Text measures `char_width` per `char` (default `0.0`), so layout code
/// that centres or truncates text can be exercised with a predictable
/// width.
#[derive(Clone, Debug, Default)]
pub struct RecordingRenderContext {
    /// Every call, in call order.
    pub ops: Vec<DrawOp>,
    /// Width reported per `char` by `measure_text` / `text_bounds`.
    pub char_width: f64,
}

impl RecordingRenderContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Same as [`Self::new`], with `char_width` per character for text
    /// measurement.
    pub fn with_char_width(char_width: f64) -> Self {
        Self { ops: Vec::new(), char_width }
    }

    /// Texts passed to `fill_text`, with their positions, in order.
    pub fn fill_texts(&self) -> Vec<(&str, f64, f64)> {
        self.ops
            .iter()
            .filter_map(|op| match op {
                DrawOp::FillText { text, x, y } => Some((text.as_str(), *x, *y)),
                _ => None,
            })
            .collect()
    }

    /// Colours passed to `set_fill_color`, in order.
    pub fn fill_colors(&self) -> Vec<&str> {
        self.ops
            .iter()
            .filter_map(|op| match op {
                DrawOp::FillColor(color) => Some(color.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Colours passed to `set_stroke_color`, in order.
    pub fn stroke_colors(&self) -> Vec<&str> {
        self.ops
            .iter()
            .filter_map(|op| match op {
                DrawOp::StrokeColor(color) => Some(color.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Fonts passed to `set_font`, in order.
    pub fn fonts(&self) -> Vec<&str> {
        self.ops
            .iter()
            .filter_map(|op| match op {
                DrawOp::Font(font) => Some(font.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Number of `save` calls minus `restore` calls; `0` after a balanced
    /// draw pass.
    pub fn save_depth(&self) -> i64 {
        self.ops.iter().fold(0, |depth, op| match op {
            DrawOp::Save => depth + 1,
            DrawOp::Restore => depth - 1,
            _ => depth,
        })
    }

    fn push(&mut self, op: DrawOp) {
        self.ops.push(op);
    }
}

impl Painter for RecordingRenderContext {
    fn save(&mut self) {
        self.push(DrawOp::Save);
    }
    fn restore(&mut self) {
        self.push(DrawOp::Restore);
    }
    fn translate(&mut self, x: f64, y: f64) {
        self.push(DrawOp::Translate { x, y });
    }
    fn rotate(&mut self, angle: f64) {
        self.push(DrawOp::Rotate { angle });
    }
    fn scale(&mut self, x: f64, y: f64) {
        self.push(DrawOp::Scale { x, y });
    }
    fn set_fill_color(&mut self, color: &str) {
        self.push(DrawOp::FillColor(color.to_owned()));
    }
    fn set_global_alpha(&mut self, alpha: f64) {
        self.push(DrawOp::GlobalAlpha(alpha));
    }
    fn set_stroke_color(&mut self, color: &str) {
        self.push(DrawOp::StrokeColor(color.to_owned()));
    }
    fn set_stroke_width(&mut self, width: f64) {
        self.push(DrawOp::StrokeWidth(width));
    }
    fn set_line_dash(&mut self, pattern: &[f64]) {
        self.push(DrawOp::LineDash(pattern.to_vec()));
    }
    fn set_line_cap(&mut self, cap: &str) {
        self.push(DrawOp::LineCap(cap.to_owned()));
    }
    fn set_line_join(&mut self, join: &str) {
        self.push(DrawOp::LineJoin(join.to_owned()));
    }
    fn begin_path(&mut self) {
        self.push(DrawOp::BeginPath);
    }
    fn move_to(&mut self, x: f64, y: f64) {
        self.push(DrawOp::MoveTo { x, y });
    }
    fn line_to(&mut self, x: f64, y: f64) {
        self.push(DrawOp::LineTo { x, y });
    }
    fn close_path(&mut self) {
        self.push(DrawOp::ClosePath);
    }
    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.push(DrawOp::Rect { x, y, w, h });
    }
    fn arc(&mut self, cx: f64, cy: f64, radius: f64, start: f64, end: f64) {
        self.push(DrawOp::Arc { cx, cy, radius, start, end });
    }
    fn ellipse(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, rotation: f64, start: f64, end: f64) {
        self.push(DrawOp::Ellipse { cx, cy, rx, ry, rotation, start, end });
    }
    fn quadratic_curve_to(&mut self, cpx: f64, cpy: f64, x: f64, y: f64) {
        self.push(DrawOp::QuadraticCurveTo { cpx, cpy, x, y });
    }
    fn bezier_curve_to(&mut self, cp1x: f64, cp1y: f64, cp2x: f64, cp2y: f64, x: f64, y: f64) {
        self.push(DrawOp::BezierCurveTo { cp1x, cp1y, cp2x, cp2y, x, y });
    }
    fn stroke(&mut self) {
        self.push(DrawOp::Stroke);
    }
    fn fill(&mut self) {
        self.push(DrawOp::Fill);
    }
}

impl TextRenderer for RecordingRenderContext {
    fn set_font(&mut self, font: &str) {
        self.push(DrawOp::Font(font.to_owned()));
    }
    fn set_text_align(&mut self, align: TextAlign) {
        self.push(DrawOp::TextAlign(align));
    }
    fn set_text_baseline(&mut self, baseline: TextBaseline) {
        self.push(DrawOp::TextBaseline(baseline));
    }
    fn fill_text(&mut self, text: &str, x: f64, y: f64) {
        self.push(DrawOp::FillText { text: text.to_owned(), x, y });
    }
    fn stroke_text(&mut self, text: &str, x: f64, y: f64) {
        self.push(DrawOp::StrokeText { text: text.to_owned(), x, y });
    }
}

impl TextMetrics for RecordingRenderContext {
    fn measure_text(&self, text: &str) -> f64 {
        text.chars().count() as f64 * self.char_width
    }
    fn text_bounds(&self, text: &str, _font: &str) -> TextBounds {
        TextBounds { x: 0.0, y: 0.0, w: self.measure_text(text), h: 0.0, ascent: 0.0, descent: 0.0 }
    }
}

impl Masking for RecordingRenderContext {
    fn clip(&mut self) {
        self.push(DrawOp::Clip);
    }
}
impl Effects for RecordingRenderContext {}
impl ShapeHelpers for RecordingRenderContext {
    fn stroke_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.push(DrawOp::StrokeRect { x, y, w, h });
    }
    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.push(DrawOp::FillRect { x, y, w, h });
    }
}
impl GradientPainter for RecordingRenderContext {}
impl UiEffectHelpers for RecordingRenderContext {}
impl BatchPainter for RecordingRenderContext {}
impl RenderContext for RecordingRenderContext {
    fn dpr(&self) -> f64 {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_context_keeps_call_order_and_filters_by_kind() {
        let mut ctx = RecordingRenderContext::with_char_width(7.0);
        ctx.save();
        ctx.set_fill_color("#112233");
        ctx.set_font("12px sans-serif");
        ctx.fill_text("abc", 4.0, 5.0);
        ctx.fill_rect(0.0, 0.0, 10.0, 20.0);
        ctx.restore();

        assert_eq!(ctx.ops.first(), Some(&DrawOp::Save));
        assert_eq!(ctx.ops.last(), Some(&DrawOp::Restore));
        assert_eq!(ctx.fill_colors(), vec!["#112233"]);
        assert_eq!(ctx.fonts(), vec!["12px sans-serif"]);
        assert_eq!(ctx.fill_texts(), vec![("abc", 4.0, 5.0)]);
        assert_eq!(ctx.save_depth(), 0);
        assert_eq!(ctx.measure_text("abcd"), 28.0);
    }

    #[test]
    fn provided_trait_methods_lower_onto_recorded_primitives() {
        let mut ctx = RecordingRenderContext::new();
        ctx.fill_rounded_rect(0.0, 0.0, 40.0, 20.0, 4.0);
        assert!(ctx.ops.contains(&DrawOp::Fill), "fill_rounded_rect must end in a fill: {:?}", ctx.ops);
        assert!(ctx.ops.contains(&DrawOp::BeginPath));
    }

    #[test]
    fn null_context_measures_zero_and_reports_unit_dpr() {
        let ctx = NullRenderContext;
        assert_eq!(ctx.measure_text("anything"), 0.0);
        assert_eq!(ctx.dpr(), 1.0);
    }
}
