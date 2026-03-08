//! macOS tabs widget renderer

use uzor_core::render::{RenderContext, TextAlign, TextBaseline};
use crate::colors::WidgetState;
use crate::themes::tabs::TabTheme;

/// A tab to render
#[derive(Clone, Debug)]
pub struct Tab<'a> {
    pub label: &'a str,
    pub selected: bool,
    pub state: WidgetState,
}

impl<'a> Tab<'a> {
    /// Create a new tab
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            selected: false,
            state: WidgetState::Normal,
        }
    }

    /// Set the selected state
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Set the widget state
    pub fn state(mut self, state: WidgetState) -> Self {
        self.state = state;
        self
    }
}

/// Render a tab container with tabs. Returns (width, height).
pub fn render_tabs(
    ctx: &mut dyn RenderContext,
    x: f64,
    y: f64,
    tabs: &[Tab],
    theme: &TabTheme,
) -> (f64, f64) {
    if tabs.is_empty() {
        return (0.0, 0.0);
    }

    let container_height = theme.container_height();
    let container_padding = theme.container_padding();
    let container_radius = theme.container_border_radius();
    let tab_radius = theme.tab_border_radius();
    let (_tab_padding_v, tab_padding_h) = theme.tab_padding();

    // Set font for measuring
    ctx.set_font(theme.tab_font());

    // Calculate total width by measuring all tabs
    let mut total_tab_width = 0.0;
    let mut tab_widths = Vec::with_capacity(tabs.len());

    for tab in tabs {
        let text_width = ctx.measure_text(tab.label);
        let tab_width = text_width + (tab_padding_h * 2.0);
        tab_widths.push(tab_width);
        total_tab_width += tab_width;
    }

    // Add gaps between tabs (1px each)
    let gap = 1.0;
    let total_gaps = if tabs.len() > 1 {
        (tabs.len() - 1) as f64 * gap
    } else {
        0.0
    };

    let container_width = total_tab_width + total_gaps + (container_padding * 2.0);

    // 1. Draw container background
    ctx.save();
    ctx.set_fill_color(theme.container_bg());
    ctx.fill_rounded_rect(x, y, container_width, container_height, container_radius);
    ctx.restore();

    // 2. Draw each tab
    let mut current_x = x + container_padding;
    let tab_y = y + container_padding;
    let tab_height = container_height - (container_padding * 2.0);

    for (i, tab) in tabs.iter().enumerate() {
        let tab_width = tab_widths[i];
        let tab_bg = theme.tab_bg(tab.selected, tab.state);

        // Draw tab background if not transparent
        if tab_bg != "transparent" {
            ctx.save();
            ctx.set_fill_color(tab_bg);
            ctx.fill_rounded_rect(current_x, tab_y, tab_width, tab_height, tab_radius);
            ctx.restore();
        }

        // Draw tab label (centered)
        let text_color = theme.tab_text_color(tab.selected, tab.state);
        ctx.save();
        ctx.set_fill_color(text_color);
        ctx.set_font(theme.tab_font());
        ctx.set_text_align(TextAlign::Center);
        ctx.set_text_baseline(TextBaseline::Middle);

        let text_x = current_x + (tab_width / 2.0);
        let text_y = tab_y + (tab_height / 2.0);

        ctx.fill_text(tab.label, text_x, text_y);
        ctx.restore();

        // Move to next tab position
        current_x += tab_width + gap;
    }

    (container_width, container_height)
}

/// Hit test to determine which tab was clicked
pub fn hit_test(
    x: f64,
    y: f64,
    tabs: &[Tab],
    mouse_x: f64,
    mouse_y: f64,
    theme: &TabTheme,
) -> Option<usize> {
    if tabs.is_empty() {
        return None;
    }

    let container_height = theme.container_height();
    let container_padding = theme.container_padding();
    let (_, tab_padding_h) = theme.tab_padding();

    // Check if mouse is within vertical bounds
    let tab_y = y + container_padding;
    let tab_height = container_height - (container_padding * 2.0);

    if mouse_y < tab_y || mouse_y > tab_y + tab_height {
        return None;
    }

    // Measure tabs to find which one was clicked
    let mut ctx_temp = DummyMeasureContext::new();
    ctx_temp.set_font(theme.tab_font());

    let mut current_x = x + container_padding;
    let gap = 1.0;

    for (i, tab) in tabs.iter().enumerate() {
        let text_width = ctx_temp.measure_text(tab.label);
        let tab_width = text_width + (tab_padding_h * 2.0);

        if mouse_x >= current_x && mouse_x < current_x + tab_width {
            return Some(i);
        }

        current_x += tab_width + gap;
    }

    None
}

// Minimal context for text measurement in hit testing
struct DummyMeasureContext {
    font_size: f64,
}

impl DummyMeasureContext {
    fn new() -> Self {
        Self { font_size: 13.0 }
    }

    fn set_font(&mut self, font: &str) {
        // Parse font size from CSS-style string like "13px sans-serif"
        if let Some(px_pos) = font.find("px") {
            let start = font[..px_pos].rfind(|c: char| !c.is_numeric() && c != '.').map(|i| i + 1).unwrap_or(0);
            if let Ok(size) = font[start..px_pos].parse::<f64>() {
                self.font_size = size;
            }
        }
    }

    fn measure_text(&self, text: &str) -> f64 {
        // Rough approximation: 0.6 * font_size per character
        text.len() as f64 * self.font_size * 0.6
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colors::AppearanceMode;

    #[test]
    fn test_tab_creation() {
        let tab = Tab::new("Test");
        assert_eq!(tab.label, "Test");
        assert!(!tab.selected);
        assert_eq!(tab.state, WidgetState::Normal);
    }

    #[test]
    fn test_tab_builder() {
        let tab = Tab::new("Test")
            .selected(true)
            .state(WidgetState::Hovered);

        assert_eq!(tab.label, "Test");
        assert!(tab.selected);
        assert_eq!(tab.state, WidgetState::Hovered);
    }

    #[test]
    fn test_empty_tabs() {
        let theme = TabTheme::new(AppearanceMode::Dark);
        let tabs: Vec<Tab> = vec![];

        let result = hit_test(0.0, 0.0, &tabs, 10.0, 10.0, &theme);
        assert_eq!(result, None);
    }

    #[test]
    fn test_dummy_measure_context() {
        let mut ctx = DummyMeasureContext::new();
        assert_eq!(ctx.font_size, 13.0);

        ctx.set_font("16px sans-serif");
        assert_eq!(ctx.font_size, 16.0);

        let width = ctx.measure_text("Hello");
        assert!(width > 0.0);
    }
}
