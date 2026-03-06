//! macOS switch toggle widget renderer

use uzor_render::RenderContext;
use crate::colors::WidgetState;
use crate::themes::switch_toggle::SwitchTheme;
use std::f64::consts::PI;

/// Helper function to interpolate between two hex colors
fn interpolate_color(color_off: &str, color_on: &str, position: f64) -> String {
    // Simple interpolation - parse hex colors and blend
    let parse_hex = |s: &str| -> (u8, u8, u8, u8) {
        let s = s.trim_start_matches('#');
        if s.len() >= 6 {
            let r = u8::from_str_radix(&s[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&s[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&s[4..6], 16).unwrap_or(0);
            let a = if s.len() >= 8 {
                u8::from_str_radix(&s[6..8], 16).unwrap_or(255)
            } else {
                255
            };
            (r, g, b, a)
        } else {
            (0, 0, 0, 255)
        }
    };

    let (r1, g1, b1, a1) = parse_hex(color_off);
    let (r2, g2, b2, a2) = parse_hex(color_on);

    let r = (r1 as f64 + (r2 as f64 - r1 as f64) * position) as u8;
    let g = (g1 as f64 + (g2 as f64 - g1 as f64) * position) as u8;
    let b = (b1 as f64 + (b2 as f64 - b1 as f64) * position) as u8;
    let a = (a1 as f64 + (a2 as f64 - a1 as f64) * position) as u8;

    format!("#{:02X}{:02X}{:02X}{:02X}", r, g, b, a)
}

/// Render a macOS toggle switch. Returns (width, height).
/// `position` is 0.0 (off) to 1.0 (on) for animation.
pub fn render_switch(
    ctx: &mut dyn RenderContext,
    x: f64,
    y: f64,
    on: bool,
    position: f64,  // 0.0=off, 1.0=on (animated)
    theme: &SwitchTheme,
    state: WidgetState,
) -> (f64, f64) {
    let width = theme.width();
    let height = theme.height();
    let border_radius = theme.border_radius();
    let thumb_size = theme.thumb_size();
    let thumb_margin = theme.thumb_margin();

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
        let ring_w = width + ring_width;
        let ring_h = height + ring_width;
        let ring_radius = border_radius + ring_offset;

        ctx.stroke_rounded_rect(ring_x, ring_y, ring_w, ring_h, ring_radius);
    }

    // 2. Draw track with interpolated color
    let track_color_off = theme.track_bg(false, state);
    let track_color_on = theme.track_bg(true, state);
    let track_color = interpolate_color(track_color_off, track_color_on, position);

    ctx.set_fill_color(&track_color);
    ctx.fill_rounded_rect(x, y, width, height, border_radius);

    // 3. Draw track border (optional, subtle)
    let border_width = theme.track_border_width();
    if border_width > 0.0 {
        ctx.set_stroke_color(theme.track_border_color(on));
        ctx.set_stroke_width(border_width);
        ctx.stroke_rounded_rect(x, y, width, height, border_radius);
    }

    // 4. Calculate thumb position based on animation position
    let thumb_off_x = x + thumb_margin;
    let thumb_on_x = x + width - thumb_size - thumb_margin;
    let thumb_x = thumb_off_x + (thumb_on_x - thumb_off_x) * position;
    let thumb_y = y + theme.thumb_y_offset();

    // 5. Draw thumb shadow (simple approximation)
    let shadow = theme.thumb_shadow();
    if shadow.blur_radius > 0.0 {
        ctx.save();
        ctx.set_global_alpha(0.3); // Shadow opacity
        ctx.set_fill_color(shadow.color);

        let shadow_x = thumb_x + shadow.offset_x;
        let shadow_y = thumb_y + shadow.offset_y;
        let shadow_radius = thumb_size / 2.0;

        ctx.begin_path();
        ctx.arc(shadow_x + shadow_radius, shadow_y + shadow_radius, shadow_radius, 0.0, 2.0 * PI);
        ctx.fill();
        ctx.restore();
    }

    // 6. Draw thumb (white circle)
    ctx.set_fill_color(theme.thumb_bg(state));
    let thumb_radius = thumb_size / 2.0;
    let thumb_center_x = thumb_x + thumb_radius;
    let thumb_center_y = thumb_y + thumb_radius;

    ctx.begin_path();
    ctx.arc(thumb_center_x, thumb_center_y, thumb_radius, 0.0, 2.0 * PI);
    ctx.fill();

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

    impl MockContext {
        fn new() -> Self {
            Self
        }
    }

    impl RenderContext for MockContext {
        fn chart_width(&self) -> f64 { 800.0 }
        fn chart_height(&self) -> f64 { 600.0 }
        fn dpr(&self) -> f64 { 1.0 }

        fn measure_text(&self, _text: &str) -> f64 {
            0.0
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
        fn set_text_align(&mut self, _align: uzor_render::TextAlign) {}
        fn set_text_baseline(&mut self, _baseline: uzor_render::TextBaseline) {}
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
    fn test_render_switch_off() {
        let mut ctx = MockContext::new();
        let theme = SwitchTheme::new(AppearanceMode::Light);

        let (width, height) = render_switch(
            &mut ctx,
            0.0,
            0.0,
            false,
            0.0,
            &theme,
            WidgetState::Normal,
        );

        assert_eq!(width, theme.width());
        assert_eq!(height, theme.height());
    }

    #[test]
    fn test_render_switch_on() {
        let mut ctx = MockContext::new();
        let theme = SwitchTheme::new(AppearanceMode::Dark);

        let (width, height) = render_switch(
            &mut ctx,
            0.0,
            0.0,
            true,
            1.0,
            &theme,
            WidgetState::Normal,
        );

        assert_eq!(width, theme.width());
        assert_eq!(height, theme.height());
    }

    #[test]
    fn test_render_switch_animated() {
        let mut ctx = MockContext::new();
        let theme = SwitchTheme::new(AppearanceMode::Light);

        // Test mid-animation position
        let (width, height) = render_switch(
            &mut ctx,
            0.0,
            0.0,
            true,
            0.5,
            &theme,
            WidgetState::Normal,
        );

        assert_eq!(width, theme.width());
        assert_eq!(height, theme.height());
    }

    #[test]
    fn test_interpolate_color() {
        let result = interpolate_color("#FF0000FF", "#00FF00FF", 0.5);
        // Should be roughly middle between red and green
        assert!(result.starts_with('#'));
        assert_eq!(result.len(), 9); // #RRGGBBAA
    }
}
