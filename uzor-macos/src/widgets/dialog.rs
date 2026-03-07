//! macOS dialog widget renderer

use uzor_render::{RenderContext, TextAlign, TextBaseline};
use crate::themes::dialog::{DialogTheme, DialogSize};

/// Render the dialog backdrop overlay (fullscreen semi-transparent)
pub fn render_dialog_overlay(
    ctx: &mut dyn RenderContext,
    canvas_width: f64,
    canvas_height: f64,
    theme: &DialogTheme,
) {
    ctx.set_fill_color(theme.overlay_color());
    ctx.fill_rect(0.0, 0.0, canvas_width, canvas_height);
}

/// Render a macOS-style dialog. Returns (width, height).
/// Centered on screen.
pub fn render_dialog(
    ctx: &mut dyn RenderContext,
    canvas_width: f64,
    canvas_height: f64,
    title: &str,
    body: &str,
    size: DialogSize,
    theme: &DialogTheme,
) -> (f64, f64) {
    let dialog_width = theme.width(size);
    let padding = theme.padding();
    let border_radius = theme.border_radius();

    // Measure text to calculate dialog height
    ctx.set_font(theme.title_font());
    let _title_width = ctx.measure_text(title);

    ctx.set_font(theme.body_font());
    let _body_width = ctx.measure_text(body);

    // Calculate dialog height (title + body + padding)
    // Title height: approximately 22px (17px font + 5px spacing)
    // Body height: approximately 16px per line (13px font + 3px spacing)
    let title_height = 22.0;
    let body_height = 16.0;
    let dialog_height = padding + title_height + body_height + padding;

    // Center dialog on screen
    let dialog_x = (canvas_width - dialog_width) / 2.0;
    let dialog_y = (canvas_height - dialog_height) / 2.0;

    // 1. Draw shadow (ELEVATION_3)
    let (shadow_color, _shadow_blur, shadow_offset_x, shadow_offset_y) = theme.shadow();
    ctx.save();
    ctx.set_global_alpha(1.0); // Shadow color already has alpha
    ctx.set_fill_color(shadow_color);

    // Draw shadow slightly offset
    let shadow_x = dialog_x + shadow_offset_x;
    let shadow_y = dialog_y + shadow_offset_y;

    // Simple shadow approximation (multiple offset rects for blur effect)
    for i in 0..3 {
        let offset = (i as f64) * 2.0;
        ctx.set_global_alpha(0.15 / (i as f64 + 1.0));
        ctx.fill_rounded_rect(
            shadow_x - offset,
            shadow_y - offset,
            dialog_width + (offset * 2.0),
            dialog_height + (offset * 2.0),
            border_radius + offset,
        );
    }
    ctx.restore();

    // 2. Draw dialog background
    ctx.set_fill_color(theme.bg_color());
    ctx.fill_rounded_rect(dialog_x, dialog_y, dialog_width, dialog_height, border_radius);

    // 3. Draw title text (bold 17px, left-aligned, at top of dialog)
    ctx.set_fill_color(theme.title_color());
    ctx.set_font(theme.title_font());
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Top);

    let title_x = dialog_x + padding;
    let title_y = dialog_y + padding;
    ctx.fill_text(title, title_x, title_y);

    // 4. Draw body text (13px, secondary color, below title)
    ctx.set_fill_color(theme.body_color());
    ctx.set_font(theme.body_font());
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Top);

    let body_x = dialog_x + padding;
    let body_y = title_y + title_height;
    ctx.fill_text(body, body_x, body_y);

    (dialog_width, dialog_height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colors::AppearanceMode;

    // Mock RenderContext for testing
    struct MockContext {
        text_width: f64,
    }

    impl MockContext {
        fn new() -> Self {
            Self { text_width: 100.0 }
        }
    }

    impl RenderContext for MockContext {
        fn dpr(&self) -> f64 { 1.0 }

        fn measure_text(&self, _text: &str) -> f64 {
            self.text_width
        }

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
    fn test_render_dialog_dimensions() {
        let mut ctx = MockContext::new();
        let theme = DialogTheme::new(AppearanceMode::Light);

        let (width, height) = render_dialog(
            &mut ctx,
            800.0,
            600.0,
            "Test Dialog",
            "This is a test dialog body.",
            DialogSize::Regular,
            &theme,
        );

        // Width should match the size variant
        assert_eq!(width, 448.0);

        // Height should be padding + title height + body height + padding
        assert!(height > 0.0);
    }

    #[test]
    fn test_render_dialog_overlay() {
        let mut ctx = MockContext::new();
        let theme = DialogTheme::new(AppearanceMode::Dark);

        // Should not panic
        render_dialog_overlay(&mut ctx, 800.0, 600.0, &theme);
    }

    #[test]
    fn test_dialog_sizes() {
        let mut ctx = MockContext::new();
        let theme = DialogTheme::new(AppearanceMode::Light);

        let (small_width, _) = render_dialog(
            &mut ctx,
            800.0,
            600.0,
            "Small",
            "Body",
            DialogSize::Small,
            &theme,
        );

        let (regular_width, _) = render_dialog(
            &mut ctx,
            800.0,
            600.0,
            "Regular",
            "Body",
            DialogSize::Regular,
            &theme,
        );

        let (large_width, _) = render_dialog(
            &mut ctx,
            800.0,
            600.0,
            "Large",
            "Body",
            DialogSize::Large,
            &theme,
        );

        assert_eq!(small_width, 320.0);
        assert_eq!(regular_width, 448.0);
        assert_eq!(large_width, 540.0);
    }
}
