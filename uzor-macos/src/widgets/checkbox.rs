//! macOS checkbox widget renderer

use uzor_render::{RenderContext, TextAlign, TextBaseline, draw_svg_icon};
use crate::colors::WidgetState;
use crate::themes::checkbox::{CheckboxTheme, CheckboxState};
use crate::icons::paths;
use crate::typography::{TypographyLevel, font_string};

/// Render a macOS checkbox with optional label. Returns (width, height).
pub fn render_checkbox(
    ctx: &mut dyn RenderContext,
    x: f64,
    y: f64,
    checked: CheckboxState,
    label: Option<&str>,
    theme: &CheckboxTheme,
    state: WidgetState,
) -> (f64, f64) {
    let size = theme.size();
    let border_radius = theme.border_radius();

    // Save context for opacity restoration
    ctx.save();

    // Apply disabled opacity if needed
    if state == WidgetState::Disabled {
        ctx.set_global_alpha(theme.disabled_opacity());
    }

    // 1. Draw focus ring if focused
    if state == WidgetState::Focused {
        let ring_width = theme.focus_ring_width();
        let ring_offset = ring_width / 2.0;

        ctx.set_stroke_color(theme.focus_ring_color());
        ctx.set_stroke_width(ring_width);

        let ring_x = x - ring_offset;
        let ring_y = y - ring_offset;
        let ring_size = size + ring_width;
        let ring_radius = border_radius + ring_offset;

        ctx.stroke_rounded_rect(ring_x, ring_y, ring_size, ring_size, ring_radius);
    }

    // 2. Draw checkbox background
    let is_checked = checked != CheckboxState::Unchecked;
    ctx.set_fill_color(theme.bg_color(is_checked, state));
    ctx.fill_rounded_rect(x, y, size, size, border_radius);

    // 3. Draw border for unchecked state
    if !is_checked {
        let border_width = theme.border_width();
        ctx.set_stroke_color(theme.border_color(false, state));
        ctx.set_stroke_width(border_width);
        ctx.stroke_rounded_rect(x, y, size, size, border_radius);
    }

    // 4. Draw checkmark or mixed icon
    if checked == CheckboxState::Checked {
        // Draw checkmark icon
        let checkmark_scale = theme.checkmark_scale();
        let icon_size = size * checkmark_scale;
        let icon_x = x + (size - icon_size) / 2.0;
        let icon_y = y + (size - icon_size) / 2.0;

        draw_svg_icon(
            ctx,
            paths::CHECKMARK,
            icon_x,
            icon_y,
            icon_size,
            icon_size,
            theme.checkmark_color(),
        );
    } else if checked == CheckboxState::Mixed {
        // Draw mixed (dash) icon
        let (dash_width_scale, dash_height_scale) = theme.mixed_dash_scale();
        let dash_width = size * dash_width_scale;
        let dash_height = size * dash_height_scale;
        let dash_x = x + (size - dash_width) / 2.0;
        let dash_y = y + (size - dash_height) / 2.0;

        draw_svg_icon(
            ctx,
            paths::CHECKMARK_MIXED,
            dash_x,
            dash_y,
            dash_width,
            dash_height,
            theme.checkmark_color(),
        );
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
        fn chart_width(&self) -> f64 { 800.0 }
        fn chart_height(&self) -> f64 { 600.0 }
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
    fn test_render_checkbox_unchecked() {
        let mut ctx = MockContext::new();
        let theme = CheckboxTheme::new(AppearanceMode::Light);

        let (width, height) = render_checkbox(
            &mut ctx,
            0.0,
            0.0,
            CheckboxState::Unchecked,
            None,
            &theme,
            WidgetState::Normal,
        );

        assert_eq!(width, theme.size());
        assert_eq!(height, theme.size());
    }

    #[test]
    fn test_render_checkbox_with_label() {
        let mut ctx = MockContext::new();
        let theme = CheckboxTheme::new(AppearanceMode::Light);

        let (width, _height) = render_checkbox(
            &mut ctx,
            0.0,
            0.0,
            CheckboxState::Checked,
            Some("Test Label"),
            &theme,
            WidgetState::Normal,
        );

        // Width should be checkbox size + spacing + text width
        let expected_width = theme.size() + theme.label_spacing() + 50.0;
        assert_eq!(width, expected_width);
    }

    #[test]
    fn test_render_checkbox_mixed_state() {
        let mut ctx = MockContext::new();
        let theme = CheckboxTheme::new(AppearanceMode::Dark);

        let (width, height) = render_checkbox(
            &mut ctx,
            0.0,
            0.0,
            CheckboxState::Mixed,
            None,
            &theme,
            WidgetState::Normal,
        );

        assert_eq!(width, theme.size());
        assert_eq!(height, theme.size());
    }
}
