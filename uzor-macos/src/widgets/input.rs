//! macOS-style text input renderer

use uzor_render::{RenderContext, TextAlign, TextBaseline};
use crate::colors::WidgetState;
use crate::themes::input::InputTheme;
use crate::typography::{TypographyLevel, font_string};

/// Render a macOS-style text input. Returns (width, height).
#[allow(clippy::too_many_arguments)]
pub fn render_input(
    ctx: &mut dyn RenderContext,
    x: f64,
    y: f64,
    width: f64,
    value: &str,
    placeholder: &str,
    theme: &InputTheme,
    state: WidgetState,
) -> (f64, f64) {
    let (padding_h, _padding_v) = theme.padding();
    let height = theme.height();
    let border_radius = theme.border_radius();

    // Save context for opacity restoration
    ctx.save();

    // Apply disabled opacity if needed
    if state == WidgetState::Disabled {
        ctx.set_global_alpha(theme.disabled_opacity());
    }

    // 1. Draw focus outline if focused
    if state == WidgetState::Focused {
        let offset = theme.focus_outline_offset();
        let outline_width = theme.focus_outline_width();

        ctx.set_stroke_color(theme.focus_outline_color());
        ctx.set_stroke_width(outline_width);

        let outline_x = x - offset;
        let outline_y = y - offset;
        let outline_w = width + (offset * 2.0);
        let outline_h = height + (offset * 2.0);
        let outline_radius = border_radius + offset;

        ctx.stroke_rounded_rect(outline_x, outline_y, outline_w, outline_h, outline_radius);
    }

    // 2. Draw input background
    ctx.set_fill_color(theme.bg_color(state));
    ctx.fill_rounded_rect(x, y, width, height, border_radius);

    // 3. Draw border
    let border_width = theme.border_width();
    ctx.set_stroke_color(theme.border_color(state));
    ctx.set_stroke_width(border_width);
    ctx.stroke_rounded_rect(x, y, width, height, border_radius);

    // 4. Draw text or placeholder
    ctx.set_font(&font_string(TypographyLevel::Body));
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);

    let text_x = x + padding_h;
    let text_y = y + (height / 2.0);

    if !value.is_empty() {
        // Draw actual value
        ctx.set_fill_color(theme.text_color(state));
        ctx.fill_text(value, text_x, text_y);
    } else if !placeholder.is_empty() {
        // Draw placeholder in lighter color
        ctx.set_fill_color(theme.placeholder_color());
        ctx.fill_text(placeholder, text_x, text_y);
    }

    // Restore context
    ctx.restore();

    (width, height)
}

/// Render a multi-line text input (textarea). Returns (width, height).
#[allow(clippy::too_many_arguments)]
pub fn render_textarea(
    ctx: &mut dyn RenderContext,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    value: &str,
    placeholder: &str,
    theme: &InputTheme,
    state: WidgetState,
) -> (f64, f64) {
    let (padding_h, padding_v) = theme.padding();
    let border_radius = theme.border_radius();

    // Save context for opacity restoration
    ctx.save();

    // Apply disabled opacity if needed
    if state == WidgetState::Disabled {
        ctx.set_global_alpha(theme.disabled_opacity());
    }

    // 1. Draw focus outline if focused
    if state == WidgetState::Focused {
        let offset = theme.focus_outline_offset();
        let outline_width = theme.focus_outline_width();

        ctx.set_stroke_color(theme.focus_outline_color());
        ctx.set_stroke_width(outline_width);

        let outline_x = x - offset;
        let outline_y = y - offset;
        let outline_w = width + (offset * 2.0);
        let outline_h = height + (offset * 2.0);
        let outline_radius = border_radius + offset;

        ctx.stroke_rounded_rect(outline_x, outline_y, outline_w, outline_h, outline_radius);
    }

    // 2. Draw textarea background
    ctx.set_fill_color(theme.bg_color(state));
    ctx.fill_rounded_rect(x, y, width, height, border_radius);

    // 3. Draw border
    let border_width = theme.border_width();
    ctx.set_stroke_color(theme.border_color(state));
    ctx.set_stroke_width(border_width);
    ctx.stroke_rounded_rect(x, y, width, height, border_radius);

    // 4. Draw text or placeholder
    ctx.set_font(&font_string(TypographyLevel::Body));
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Top);

    let text_x = x + padding_h;
    let text_y = y + padding_v;

    if !value.is_empty() {
        // Draw actual value
        ctx.set_fill_color(theme.text_color(state));
        ctx.fill_text(value, text_x, text_y);
    } else if !placeholder.is_empty() {
        // Draw placeholder in lighter color
        ctx.set_fill_color(theme.placeholder_color());
        ctx.fill_text(placeholder, text_x, text_y);
    }

    // Restore context
    ctx.restore();

    (width, height)
}

/// Render a search field (rounded pill shape with optional icon). Returns (width, height).
#[allow(clippy::too_many_arguments)]
pub fn render_search_field(
    ctx: &mut dyn RenderContext,
    x: f64,
    y: f64,
    width: f64,
    value: &str,
    placeholder: &str,
    theme: &InputTheme,
    state: WidgetState,
) -> (f64, f64) {
    let (padding_h, _padding_v) = theme.padding();
    let height = theme.height();

    // Search field uses pill shape (height/2 radius)
    let pill_radius = height / 2.0;

    // Save context for opacity restoration
    ctx.save();

    // Apply disabled opacity if needed
    if state == WidgetState::Disabled {
        ctx.set_global_alpha(theme.disabled_opacity());
    }

    // 1. Draw focus outline if focused
    if state == WidgetState::Focused {
        let offset = theme.focus_outline_offset();
        let outline_width = theme.focus_outline_width();

        ctx.set_stroke_color(theme.focus_outline_color());
        ctx.set_stroke_width(outline_width);

        let outline_x = x - offset;
        let outline_y = y - offset;
        let outline_w = width + (offset * 2.0);
        let outline_h = height + (offset * 2.0);
        let outline_radius = pill_radius + offset;

        ctx.stroke_rounded_rect(outline_x, outline_y, outline_w, outline_h, outline_radius);
    }

    // 2. Draw search field background
    ctx.set_fill_color(theme.bg_color(state));
    ctx.fill_rounded_rect(x, y, width, height, pill_radius);

    // 3. Draw border
    let border_width = theme.border_width();
    ctx.set_stroke_color(theme.border_color(state));
    ctx.set_stroke_width(border_width);
    ctx.stroke_rounded_rect(x, y, width, height, pill_radius);

    // 4. Draw text or placeholder
    ctx.set_font(&font_string(TypographyLevel::Body));
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);

    // Add extra padding for search icon space
    let text_x = x + padding_h + 20.0; // Reserve 20px for icon
    let text_y = y + (height / 2.0);

    if !value.is_empty() {
        // Draw actual value
        ctx.set_fill_color(theme.text_color(state));
        ctx.fill_text(value, text_x, text_y);
    } else if !placeholder.is_empty() {
        // Draw placeholder in lighter color
        ctx.set_fill_color(theme.placeholder_color());
        ctx.fill_text(placeholder, text_x, text_y);
    }

    // TODO: Add search icon rendering when icon system is available

    // Restore context
    ctx.restore();

    (width, height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colors::AppearanceMode;

    // Mock RenderContext for testing
    struct MockContext;

    impl RenderContext for MockContext {
        fn chart_width(&self) -> f64 { 800.0 }
        fn chart_height(&self) -> f64 { 600.0 }
        fn dpr(&self) -> f64 { 1.0 }

        fn measure_text(&self, _text: &str) -> f64 {
            50.0
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
    fn test_render_input_dimensions() {
        let mut ctx = MockContext;
        let theme = InputTheme::new(AppearanceMode::Light);

        let width = 200.0;
        let (result_width, result_height) = render_input(
            &mut ctx,
            0.0,
            0.0,
            width,
            "test",
            "placeholder",
            &theme,
            WidgetState::Normal,
        );

        assert_eq!(result_width, width);
        assert_eq!(result_height, theme.height());
    }

    #[test]
    fn test_render_textarea_dimensions() {
        let mut ctx = MockContext;
        let theme = InputTheme::new(AppearanceMode::Dark);

        let width = 300.0;
        let height = 100.0;
        let (result_width, result_height) = render_textarea(
            &mut ctx,
            0.0,
            0.0,
            width,
            height,
            "multi\nline\ntext",
            "Enter text...",
            &theme,
            WidgetState::Normal,
        );

        assert_eq!(result_width, width);
        assert_eq!(result_height, height);
    }

    #[test]
    fn test_render_search_field_dimensions() {
        let mut ctx = MockContext;
        let theme = InputTheme::new(AppearanceMode::Light);

        let width = 250.0;
        let (result_width, result_height) = render_search_field(
            &mut ctx,
            0.0,
            0.0,
            width,
            "",
            "Search...",
            &theme,
            WidgetState::Normal,
        );

        assert_eq!(result_width, width);
        assert_eq!(result_height, theme.height());
    }
}
