//! macOS radio widget renderer

use uzor_core::render::{RenderContext, TextAlign, TextBaseline};
use crate::colors::WidgetState;
use crate::themes::radio::RadioTheme;
use crate::typography::{TypographyLevel, font_string};
use std::f64::consts::PI;

/// Render a macOS radio button with optional label. Returns (width, height).
pub fn render_radio(
    ctx: &mut dyn RenderContext,
    x: f64,
    y: f64,
    selected: bool,
    label: Option<&str>,
    theme: &RadioTheme,
    state: WidgetState,
) -> (f64, f64) {
    let size = theme.size();
    let radius = size / 2.0;
    let center_x = x + radius;
    let center_y = y + radius;

    // Save context for opacity restoration
    ctx.save();

    // Apply disabled opacity if needed
    if state == WidgetState::Disabled {
        ctx.set_global_alpha(theme.disabled_opacity());
    }

    // 1. Draw focus ring if focused
    if state == WidgetState::Focused {
        let ring_width = theme.focus_ring_width();
        let ring_radius = radius + ring_width / 2.0;

        ctx.set_stroke_color(theme.focus_ring_color());
        ctx.set_stroke_width(ring_width);

        ctx.begin_path();
        ctx.arc(center_x, center_y, ring_radius, 0.0, 2.0 * PI);
        ctx.stroke();
    }

    // 2. Draw outer circle background
    ctx.set_fill_color(theme.bg_color(selected, state));
    ctx.begin_path();
    ctx.arc(center_x, center_y, radius, 0.0, 2.0 * PI);
    ctx.fill();

    // 3. Draw border for unselected state
    if !selected {
        let border_width = theme.border_width();
        ctx.set_stroke_color(theme.border_color(false, state));
        ctx.set_stroke_width(border_width);
        ctx.begin_path();
        ctx.arc(center_x, center_y, radius, 0.0, 2.0 * PI);
        ctx.stroke();
    }

    // 4. Draw inner dot if selected
    if selected {
        let dot_size = theme.inner_dot_size();
        let dot_radius = dot_size / 2.0;

        ctx.set_fill_color(theme.dot_color());
        ctx.begin_path();
        ctx.arc(center_x, center_y, dot_radius, 0.0, 2.0 * PI);
        ctx.fill();
    }

    // 5. Draw label if provided
    let total_width = if let Some(label_text) = label {
        let label_spacing = theme.label_spacing();
        let label_x = x + size + label_spacing;
        let label_y = y + size / 2.0 + theme.label_baseline_offset();

        ctx.set_fill_color(theme.bg_color(false, state)); // Use label color from palette
        ctx.set_font(&font_string(TypographyLevel::Body));
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Middle);

        ctx.fill_text(label_text, label_x, label_y);

        let text_width = ctx.measure_text(label_text);
        size + label_spacing + text_width
    } else {
        size
    };

    // Restore context
    ctx.restore();

    (total_width, size)
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
        fn stroke_rect(&mut self, _x: f64, _y: f64, _width: f64, _height: f64) {}
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
        fn arc(&mut self, _x: f64, _y: f64, _radius: f64, _start_angle: f64, _end_angle: f64) {}
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
    fn test_render_radio_unselected() {
        let mut ctx = MockContext::new();
        let theme = RadioTheme::new(AppearanceMode::Light);

        let (width, height) = render_radio(
            &mut ctx,
            0.0,
            0.0,
            false,
            None,
            &theme,
            WidgetState::Normal,
        );

        assert_eq!(width, theme.size());
        assert_eq!(height, theme.size());
    }

    #[test]
    fn test_render_radio_selected() {
        let mut ctx = MockContext::new();
        let theme = RadioTheme::new(AppearanceMode::Dark);

        let (width, height) = render_radio(
            &mut ctx,
            0.0,
            0.0,
            true,
            None,
            &theme,
            WidgetState::Normal,
        );

        assert_eq!(width, theme.size());
        assert_eq!(height, theme.size());
    }

    #[test]
    fn test_render_radio_with_label() {
        let mut ctx = MockContext::new();
        let theme = RadioTheme::new(AppearanceMode::Light);

        let (width, _height) = render_radio(
            &mut ctx,
            0.0,
            0.0,
            true,
            Some("Option 1"),
            &theme,
            WidgetState::Normal,
        );

        // Width should be radio size + spacing + text width
        let expected_width = theme.size() + theme.label_spacing() + 50.0;
        assert_eq!(width, expected_width);
    }
}
