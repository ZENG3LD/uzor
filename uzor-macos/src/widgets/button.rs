//! macOS-style button renderer

use uzor_render::{RenderContext, TextAlign, TextBaseline};
use crate::colors::WidgetState;
use crate::themes::button::{ButtonTheme, ButtonVariant};
use crate::typography::{TypographyLevel, font_string};

/// Render a macOS-style button. Returns (width, height).
pub fn render_button(
    ctx: &mut dyn RenderContext,
    x: f64,
    y: f64,
    label: &str,
    theme: &ButtonTheme,
    state: WidgetState,
) -> (f64, f64) {
    let (padding_h, _padding_v) = theme.padding();
    let min_height = theme.min_height();
    let border_radius = theme.border_radius();

    // Measure text to calculate button width
    ctx.set_font(&font_string(TypographyLevel::Body));
    let text_width = ctx.measure_text(label);
    let button_width = text_width + (padding_h * 2.0);
    let button_height = min_height;

    // Save context for opacity restoration
    ctx.save();

    // Apply disabled opacity if needed
    if state == WidgetState::Disabled {
        ctx.set_global_alpha(theme.disabled_opacity());
    }

    // 1. Draw focus ring if focused
    if state == WidgetState::Focused {
        let offset = theme.focus_ring_offset();
        let ring_width = theme.focus_ring_width();

        ctx.set_stroke_color(theme.focus_ring_color());
        ctx.set_stroke_width(ring_width);

        let ring_x = x - offset;
        let ring_y = y - offset;
        let ring_w = button_width + (offset * 2.0);
        let ring_h = button_height + (offset * 2.0);
        let ring_radius = border_radius + offset;

        ctx.stroke_rounded_rect(ring_x, ring_y, ring_w, ring_h, ring_radius);
    }

    // 2. Draw button background
    ctx.set_fill_color(theme.bg_color(state));
    ctx.fill_rounded_rect(x, y, button_width, button_height, border_radius);

    // 3. Draw border for Default variant
    if theme.variant == ButtonVariant::Default {
        let border_width = theme.border_width();
        if border_width > 0.0 {
            ctx.set_stroke_color(theme.border_color(state));
            ctx.set_stroke_width(border_width);
            ctx.stroke_rounded_rect(x, y, button_width, button_height, border_radius);
        }
    }

    // 4. Draw button text (centered)
    ctx.set_fill_color(theme.text_color(state));
    ctx.set_font(&font_string(TypographyLevel::Body));
    ctx.set_text_align(TextAlign::Center);
    ctx.set_text_baseline(TextBaseline::Middle);

    let text_x = x + (button_width / 2.0);
    let text_y = y + (button_height / 2.0);

    ctx.fill_text(label, text_x, text_y);

    // Restore context
    ctx.restore();

    (button_width, button_height)
}

/// Render a pill-shaped button (borderRadius = height/2). Returns (width, height).
pub fn render_pill_button(
    ctx: &mut dyn RenderContext,
    x: f64,
    y: f64,
    label: &str,
    theme: &ButtonTheme,
    state: WidgetState,
) -> (f64, f64) {
    let (padding_h, _padding_v) = theme.padding();
    let min_height = theme.min_height();

    // Measure text to calculate button width
    ctx.set_font(&font_string(TypographyLevel::Body));
    let text_width = ctx.measure_text(label);
    let button_width = text_width + (padding_h * 2.0);
    let button_height = min_height;

    // Pill radius is half the height
    let pill_radius = button_height / 2.0;

    // Save context for opacity restoration
    ctx.save();

    // Apply disabled opacity if needed
    if state == WidgetState::Disabled {
        ctx.set_global_alpha(theme.disabled_opacity());
    }

    // 1. Draw focus ring if focused
    if state == WidgetState::Focused {
        let offset = theme.focus_ring_offset();
        let ring_width = theme.focus_ring_width();

        ctx.set_stroke_color(theme.focus_ring_color());
        ctx.set_stroke_width(ring_width);

        let ring_x = x - offset;
        let ring_y = y - offset;
        let ring_w = button_width + (offset * 2.0);
        let ring_h = button_height + (offset * 2.0);
        let ring_radius = pill_radius + offset;

        ctx.stroke_rounded_rect(ring_x, ring_y, ring_w, ring_h, ring_radius);
    }

    // 2. Draw button background
    ctx.set_fill_color(theme.bg_color(state));
    ctx.fill_rounded_rect(x, y, button_width, button_height, pill_radius);

    // 3. Draw border for Default variant
    if theme.variant == ButtonVariant::Default {
        let border_width = theme.border_width();
        if border_width > 0.0 {
            ctx.set_stroke_color(theme.border_color(state));
            ctx.set_stroke_width(border_width);
            ctx.stroke_rounded_rect(x, y, button_width, button_height, pill_radius);
        }
    }

    // 4. Draw button text (centered)
    ctx.set_fill_color(theme.text_color(state));
    ctx.set_font(&font_string(TypographyLevel::Body));
    ctx.set_text_align(TextAlign::Center);
    ctx.set_text_baseline(TextBaseline::Middle);

    let text_x = x + (button_width / 2.0);
    let text_y = y + (button_height / 2.0);

    ctx.fill_text(label, text_x, text_y);

    // Restore context
    ctx.restore();

    (button_width, button_height)
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
            Self { text_width: 50.0 }
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
        fn fill_rect(&mut self, _x: f64, _y: f64, _width: f64, _height: f64) {}
        fn stroke_rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {}
        fn set_font(&mut self, _font: &str) {}
        fn fill_text(&mut self, _text: &str, _x: f64, _y: f64) {}
        fn stroke_text(&mut self, _text: &str, _x: f64, _y: f64) {}
        fn set_text_align(&mut self, _align: TextAlign) {}
        fn set_text_baseline(&mut self, _baseline: TextBaseline) {}
        fn begin_path(&mut self) {}
        fn move_to(&mut self, _x: f64, _y: f64) {}
        fn line_to(&mut self, _x: f64, _y: f64) {}
        fn close_path(&mut self) {}
        fn rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {}
        fn arc(&mut self, _cx: f64, _cy: f64, _radius: f64, _start_angle: f64, _end_angle: f64) {}
        fn ellipse(&mut self, _cx: f64, _cy: f64, _rx: f64, _ry: f64, _rotation: f64, _start: f64, _end: f64) {}
        fn quadratic_curve_to(&mut self, _cpx: f64, _cpy: f64, _x: f64, _y: f64) {}
        fn bezier_curve_to(&mut self, _cp1x: f64, _cp1y: f64, _cp2x: f64, _cp2y: f64, _x: f64, _y: f64) {}
        fn fill(&mut self) {}
        fn stroke(&mut self) {}
        fn clip(&mut self) {}
        fn save(&mut self) {}
        fn restore(&mut self) {}
        fn translate(&mut self, _x: f64, _y: f64) {}
        fn rotate(&mut self, _angle: f64) {}
        fn scale(&mut self, _x: f64, _y: f64) {}
        fn set_global_alpha(&mut self, _alpha: f64) {}
    }

    #[test]
    fn test_render_button_dimensions() {
        let mut ctx = MockContext::new();
        let theme = ButtonTheme::new(ButtonVariant::Default, AppearanceMode::Light);

        let (width, height) = render_button(
            &mut ctx,
            0.0,
            0.0,
            "Test",
            &theme,
            WidgetState::Normal,
        );

        // Width should be text width + 2 * horizontal padding
        let (padding_h, _) = theme.padding();
        assert_eq!(width, 50.0 + (padding_h * 2.0));

        // Height should be min_height
        assert_eq!(height, theme.min_height());
    }

    #[test]
    fn test_render_pill_button_radius() {
        let mut ctx = MockContext::new();
        let theme = ButtonTheme::new(ButtonVariant::Accent, AppearanceMode::Dark);

        let (width, height) = render_pill_button(
            &mut ctx,
            0.0,
            0.0,
            "Pill",
            &theme,
            WidgetState::Normal,
        );

        // Verify dimensions are calculated correctly
        assert!(width > 0.0);
        assert_eq!(height, theme.min_height());
    }
}
