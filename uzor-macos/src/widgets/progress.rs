//! macOS progress widget renderer

use uzor_core::render::RenderContext;
use crate::themes::progress::{ProgressTheme, ProgressSize};
use std::f64::consts::PI;

/// Render a progress bar. Returns (width, height).
pub fn render_progress_bar(
    ctx: &mut dyn RenderContext,
    x: f64,
    y: f64,
    width: f64,
    progress: f64,  // 0.0 to 1.0
    size: ProgressSize,
    theme: &ProgressTheme,
) -> (f64, f64) {
    let height = theme.bar_height(size);
    let radius = theme.bar_border_radius(size);
    let clamped_progress = progress.clamp(0.0, 1.0);

    // 1. Draw track (full width, pill radius)
    ctx.set_fill_color(theme.bar_track_color());
    ctx.fill_rounded_rect(x, y, width, height, radius);

    // 2. Draw fill (width * progress, accent color)
    if clamped_progress > 0.0 {
        let fill_width = width * clamped_progress;
        ctx.set_fill_color(theme.bar_fill_color());
        ctx.fill_rounded_rect(x, y, fill_width, height, radius);
    }

    (width, height)
}

/// Render a progress ring. Returns (size, size).
pub fn render_progress_ring(
    ctx: &mut dyn RenderContext,
    cx: f64,
    cy: f64,
    progress: f64,  // 0.0 to 1.0
    size: ProgressSize,
    theme: &ProgressTheme,
) -> (f64, f64) {
    let diameter = theme.ring_size(size);
    let radius = diameter / 2.0;
    let stroke_width = theme.ring_stroke_width(size);
    let clamped_progress = progress.clamp(0.0, 1.0);

    // Set line cap to round for rounded ends
    ctx.set_line_cap("round");

    // 1. Draw track circle (full circle, track color, stroke only)
    ctx.begin_path();
    ctx.arc(cx, cy, radius, 0.0, 2.0 * PI);
    ctx.set_stroke_color(theme.ring_track_color());
    ctx.set_stroke_width(stroke_width);
    ctx.stroke();

    // 2. Draw progress arc (from -PI/2, sweep = progress * 2*PI, accent color, stroke only)
    if clamped_progress > 0.0 {
        let start_angle = -PI / 2.0;  // Start at top (12 o'clock)
        let end_angle = start_angle + (clamped_progress * 2.0 * PI);

        ctx.begin_path();
        ctx.arc(cx, cy, radius, start_angle, end_angle);
        ctx.set_stroke_color(theme.ring_fill_color());
        ctx.set_stroke_width(stroke_width);
        ctx.stroke();
    }

    (diameter, diameter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colors::AppearanceMode;
    use uzor_core::render::{TextAlign, TextBaseline};

    // Mock RenderContext for testing
    struct MockContext;

    impl RenderContext for MockContext {
        fn dpr(&self) -> f64 { 1.0 }

        fn measure_text(&self, _text: &str) -> f64 { 50.0 }
        fn set_fill_color(&mut self, _color: &str) {}
        fn set_stroke_color(&mut self, _color: &str) {}
        fn set_stroke_width(&mut self, _width: f64) {}
        fn set_line_dash(&mut self, _pattern: &[f64]) {}
        fn set_line_cap(&mut self, _cap: &str) {}
        fn set_line_join(&mut self, _join: &str) {}
        fn set_global_alpha(&mut self, _alpha: f64) {}
        fn begin_path(&mut self) {}
        fn move_to(&mut self, _x: f64, _y: f64) {}
        fn line_to(&mut self, _x: f64, _y: f64) {}
        fn close_path(&mut self) {}
        fn rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {}
        fn arc(&mut self, _cx: f64, _cy: f64, _radius: f64, _start_angle: f64, _end_angle: f64) {}
        fn ellipse(&mut self, _cx: f64, _cy: f64, _rx: f64, _ry: f64, _rotation: f64, _start: f64, _end: f64) {}
        fn quadratic_curve_to(&mut self, _cpx: f64, _cpy: f64, _x: f64, _y: f64) {}
        fn bezier_curve_to(&mut self, _cp1x: f64, _cp1y: f64, _cp2x: f64, _cp2y: f64, _x: f64, _y: f64) {}
        fn stroke(&mut self) {}
        fn fill(&mut self) {}
        fn clip(&mut self) {}
        fn stroke_rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {}
        fn fill_rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {}
        fn set_font(&mut self, _font: &str) {}
        fn set_text_align(&mut self, _align: TextAlign) {}
        fn set_text_baseline(&mut self, _baseline: TextBaseline) {}
        fn fill_text(&mut self, _text: &str, _x: f64, _y: f64) {}
        fn stroke_text(&mut self, _text: &str, _x: f64, _y: f64) {}
        fn save(&mut self) {}
        fn restore(&mut self) {}
        fn translate(&mut self, _x: f64, _y: f64) {}
        fn rotate(&mut self, _angle: f64) {}
        fn scale(&mut self, _x: f64, _y: f64) {}
    }

    #[test]
    fn test_render_progress_bar_dimensions() {
        let mut ctx = MockContext;
        let theme = ProgressTheme::new(AppearanceMode::Light);

        let (width, height) = render_progress_bar(
            &mut ctx,
            0.0,
            0.0,
            200.0,
            0.5,
            ProgressSize::Regular,
            &theme,
        );

        assert_eq!(width, 200.0);
        assert_eq!(height, 4.0);
    }

    #[test]
    fn test_render_progress_bar_clamping() {
        let mut ctx = MockContext;
        let theme = ProgressTheme::new(AppearanceMode::Dark);

        // Should not panic with out-of-range progress values
        render_progress_bar(&mut ctx, 0.0, 0.0, 200.0, -0.5, ProgressSize::Small, &theme);
        render_progress_bar(&mut ctx, 0.0, 0.0, 200.0, 1.5, ProgressSize::Small, &theme);
    }

    #[test]
    fn test_render_progress_ring_dimensions() {
        let mut ctx = MockContext;
        let theme = ProgressTheme::new(AppearanceMode::Light);

        let (width, height) = render_progress_ring(
            &mut ctx,
            100.0,
            100.0,
            0.75,
            ProgressSize::Regular,
            &theme,
        );

        assert_eq!(width, 32.0);
        assert_eq!(height, 32.0);
    }

    #[test]
    fn test_progress_bar_sizes() {
        let mut ctx = MockContext;
        let theme = ProgressTheme::new(AppearanceMode::Light);

        let (_, small_height) = render_progress_bar(
            &mut ctx,
            0.0,
            0.0,
            200.0,
            0.5,
            ProgressSize::Small,
            &theme,
        );

        let (_, regular_height) = render_progress_bar(
            &mut ctx,
            0.0,
            0.0,
            200.0,
            0.5,
            ProgressSize::Regular,
            &theme,
        );

        let (_, large_height) = render_progress_bar(
            &mut ctx,
            0.0,
            0.0,
            200.0,
            0.5,
            ProgressSize::Large,
            &theme,
        );

        assert_eq!(small_height, 2.0);
        assert_eq!(regular_height, 4.0);
        assert_eq!(large_height, 6.0);
    }

    #[test]
    fn test_progress_ring_sizes() {
        let mut ctx = MockContext;
        let theme = ProgressTheme::new(AppearanceMode::Dark);

        let (small_size, _) = render_progress_ring(
            &mut ctx,
            100.0,
            100.0,
            0.5,
            ProgressSize::Small,
            &theme,
        );

        let (regular_size, _) = render_progress_ring(
            &mut ctx,
            100.0,
            100.0,
            0.5,
            ProgressSize::Regular,
            &theme,
        );

        let (large_size, _) = render_progress_ring(
            &mut ctx,
            100.0,
            100.0,
            0.5,
            ProgressSize::Large,
            &theme,
        );

        assert_eq!(small_size, 16.0);
        assert_eq!(regular_size, 32.0);
        assert_eq!(large_size, 64.0);
    }

    #[test]
    fn test_progress_ring_zero_progress() {
        let mut ctx = MockContext;
        let theme = ProgressTheme::new(AppearanceMode::Light);

        // Should render only the track circle
        let (size, _) = render_progress_ring(
            &mut ctx,
            100.0,
            100.0,
            0.0,
            ProgressSize::Regular,
            &theme,
        );

        assert_eq!(size, 32.0);
    }

    #[test]
    fn test_progress_ring_full_progress() {
        let mut ctx = MockContext;
        let theme = ProgressTheme::new(AppearanceMode::Light);

        // Should render full circle
        let (size, _) = render_progress_ring(
            &mut ctx,
            100.0,
            100.0,
            1.0,
            ProgressSize::Regular,
            &theme,
        );

        assert_eq!(size, 32.0);
    }
}
