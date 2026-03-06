//! macOS-style menu renderer (NSMenu)
//!
//! This module provides a complete implementation of macOS-style dropdown menus
//! with support for:
//! - Action items (standard menu items)
//! - Submenu indicators (chevron arrows)
//! - Toggle items (checkmarks)
//! - Section headers
//! - Separators
//! - Keyboard shortcuts
//! - Hover and pressed states
//! - Disabled items
//! - Vibrancy/blur background support
//!
//! # Example
//!
//! ```rust,ignore
//! use uzor_macos::widgets::menu::{MenuItem, render_menu};
//! use uzor_macos::themes::menu::MenuTheme;
//! use uzor_macos::colors::AppearanceMode;
//!
//! let theme = MenuTheme::new(AppearanceMode::Dark);
//! let items = vec![
//!     MenuItem::action_with_shortcut("New File", "⌘N"),
//!     MenuItem::action_with_shortcut("Open...", "⌘O"),
//!     MenuItem::separator(),
//!     MenuItem::action_with_shortcut("Save", "⌘S"),
//!     MenuItem::action_with_shortcut("Save As...", "⌘⇧S"),
//!     MenuItem::separator(),
//!     MenuItem::submenu("Recent Files"),
//!     MenuItem::separator(),
//!     MenuItem::action_with_shortcut("Close", "⌘W"),
//! ];
//!
//! // Render at position (100, 100)
//! let (width, height) = render_menu(&mut ctx, 100.0, 100.0, &items, &theme);
//! ```

use uzor_render::{RenderContext, TextAlign, TextBaseline, draw_svg_icon};
use crate::colors::WidgetState;
use crate::themes::menu::{MenuTheme, MenuItemKind};
use crate::icons::paths;

/// A menu item to render
pub struct MenuItem<'a> {
    pub label: &'a str,
    pub kind: MenuItemKind,
    pub shortcut: Option<&'a str>,  // e.g. "⌘R", "⌘Q"
    pub enabled: bool,
    pub state: WidgetState,
}

impl<'a> MenuItem<'a> {
    /// Create a new action menu item
    pub fn action(label: &'a str) -> Self {
        Self {
            label,
            kind: MenuItemKind::Action,
            shortcut: None,
            enabled: true,
            state: WidgetState::Normal,
        }
    }

    /// Create a new action menu item with shortcut
    pub fn action_with_shortcut(label: &'a str, shortcut: &'a str) -> Self {
        Self {
            label,
            kind: MenuItemKind::Action,
            shortcut: Some(shortcut),
            enabled: true,
            state: WidgetState::Normal,
        }
    }

    /// Create a new submenu item
    pub fn submenu(label: &'a str) -> Self {
        Self {
            label,
            kind: MenuItemKind::Submenu,
            shortcut: None,
            enabled: true,
            state: WidgetState::Normal,
        }
    }

    /// Create a new toggle menu item
    pub fn toggle(label: &'a str, checked: bool) -> Self {
        Self {
            label,
            kind: MenuItemKind::Toggle { checked },
            shortcut: None,
            enabled: true,
            state: WidgetState::Normal,
        }
    }

    /// Create a new header menu item
    pub fn header(label: &'a str) -> Self {
        Self {
            label,
            kind: MenuItemKind::Header,
            shortcut: None,
            enabled: false,
            state: WidgetState::Normal,
        }
    }

    /// Create a separator
    pub fn separator() -> Self {
        Self {
            label: "",
            kind: MenuItemKind::Separator,
            shortcut: None,
            enabled: false,
            state: WidgetState::Normal,
        }
    }

    /// Set enabled state
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        if !enabled {
            self.state = WidgetState::Disabled;
        }
        self
    }

    /// Set widget state
    pub fn state(mut self, state: WidgetState) -> Self {
        self.state = state;
        self
    }
}

/// Compute height of a menu given its items
pub fn compute_menu_height(items: &[MenuItem], theme: &MenuTheme) -> f64 {
    let (vpad, _) = theme.container_padding();
    let mut height = vpad * 2.0; // Top and bottom padding

    for item in items {
        height += match item.kind {
            MenuItemKind::Separator => theme.separator_height() + 6.0, // +6 for spacing
            _ => theme.item_height(),
        };
    }

    height
}

/// Compute width of a menu given its items
pub fn compute_menu_width(ctx: &dyn RenderContext, items: &[MenuItem], theme: &MenuTheme) -> f64 {
    let (_, hpad) = theme.item_padding();
    let checkmark_space = 24.0; // Space reserved for checkmarks
    let chevron_space = 24.0;   // Space reserved for chevrons
    let shortcut_spacing = 32.0; // Gap between label and shortcut

    let mut max_width = theme.container_min_width();

    // Measure all items
    for item in items {
        let width = match item.kind {
            MenuItemKind::Separator | MenuItemKind::Header => {
                // Headers and separators don't affect width calculation
                continue;
            }
            MenuItemKind::Action | MenuItemKind::Toggle { .. } | MenuItemKind::Submenu => {
                let mut w = hpad * 2.0; // Horizontal padding

                // Add checkmark space if this item or any other is a toggle
                let has_toggles = items.iter().any(|i| matches!(i.kind, MenuItemKind::Toggle { .. }));
                if has_toggles {
                    w += checkmark_space;
                }

                // Measure label
                w += ctx.measure_text(item.label);

                // Add shortcut width if present
                if let Some(shortcut) = item.shortcut {
                    w += shortcut_spacing;
                    w += ctx.measure_text(shortcut);
                }

                // Add chevron space for submenus
                if matches!(item.kind, MenuItemKind::Submenu) {
                    w += chevron_space;
                }

                w
            }
        };

        if width > max_width {
            max_width = width;
        }
    }

    // Clamp to max width
    max_width.min(theme.container_max_width())
}

/// Render a complete macOS-style menu. Returns (width, height).
pub fn render_menu(
    ctx: &mut dyn RenderContext,
    x: f64,
    y: f64,
    items: &[MenuItem],
    theme: &MenuTheme,
) -> (f64, f64) {
    if items.is_empty() {
        return (0.0, 0.0);
    }

    // Compute dimensions
    let width = compute_menu_width(ctx, items, theme);
    let height = compute_menu_height(items, theme);

    // Draw shadow behind menu
    draw_menu_shadow(ctx, x, y, width, height, theme);

    // Draw menu container background
    draw_menu_container(ctx, x, y, width, height, theme);

    // Render each item
    let (vpad, _) = theme.container_padding();
    let mut current_y = y + vpad;

    for item in items {
        let item_height = render_menu_item(ctx, x, current_y, width, item, theme);
        current_y += item_height;
    }

    (width, height)
}

/// Draw shadow behind menu container
fn draw_menu_shadow(
    ctx: &mut dyn RenderContext,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    theme: &MenuTheme,
) {
    // macOS menu shadow: subtle, offset downward slightly
    let shadow_color = "#00000033"; // 20% black
    let _shadow_blur = 12.0; // Used for reference, actual blur simulated with layers
    let shadow_offset_x = 0.0;
    let shadow_offset_y = 4.0;

    // Draw shadow as a rounded rect with blur effect (simulated with multiple layers)
    let radius = theme.container_border_radius();

    // Draw multiple translucent layers to simulate blur
    for i in 0..3 {
        let layer_alpha = 0.05 + (0.05 * i as f64);
        let layer_spread = (i + 1) as f64 * 2.0;

        ctx.save();
        ctx.set_global_alpha(layer_alpha);
        ctx.set_fill_color(shadow_color);
        ctx.fill_rounded_rect(
            x + shadow_offset_x - layer_spread,
            y + shadow_offset_y + layer_spread,
            width + layer_spread * 2.0,
            height + layer_spread,
            radius,
        );
        ctx.restore();
    }
}

/// Draw menu container background
fn draw_menu_container(
    ctx: &mut dyn RenderContext,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    theme: &MenuTheme,
) {
    let radius = theme.container_border_radius();

    // Use vibrancy if available
    if ctx.has_blur_background() {
        ctx.draw_blur_background(x, y, width, height);
    }

    // Draw semi-transparent background
    ctx.set_fill_color(theme.container_bg());
    ctx.fill_rounded_rect(x, y, width, height, radius);
}

/// Render a single menu item. Returns item height.
pub fn render_menu_item(
    ctx: &mut dyn RenderContext,
    x: f64,
    y: f64,
    width: f64,
    item: &MenuItem,
    theme: &MenuTheme,
) -> f64 {
    match item.kind {
        MenuItemKind::Separator => render_menu_separator(ctx, x, y, width, theme),
        MenuItemKind::Header => render_menu_header(ctx, x, y, width, item.label, theme),
        _ => render_menu_action_item(ctx, x, y, width, item, theme),
    }
}

/// Render a regular menu action item (Action, Toggle, Submenu)
fn render_menu_action_item(
    ctx: &mut dyn RenderContext,
    x: f64,
    y: f64,
    width: f64,
    item: &MenuItem,
    theme: &MenuTheme,
) -> f64 {
    let item_height = theme.item_height();
    let (_, hpad) = theme.item_padding();

    // Determine state for rendering
    let state = if !item.enabled {
        WidgetState::Disabled
    } else {
        item.state
    };

    // Draw background for hovered/pressed state
    if matches!(state, WidgetState::Hovered | WidgetState::Pressed) {
        ctx.set_fill_color(theme.item_bg(state));
        ctx.fill_rounded_rect(x + 4.0, y, width - 8.0, item_height, 4.0);
    }

    // Set text color based on state
    let text_color = theme.item_text_color(state);
    ctx.set_fill_color(text_color);
    ctx.set_font(theme.item_font());
    ctx.set_text_baseline(TextBaseline::Middle);

    let mut current_x = x + hpad;

    // Draw checkmark for toggle items
    if let MenuItemKind::Toggle { checked } = item.kind {
        if checked {
            let checkmark_x = current_x;
            let checkmark_y = y + item_height / 2.0 - 6.0; // Center vertically (12x12 icon)
            draw_svg_icon(ctx, paths::CHECKMARK, checkmark_x, checkmark_y, 12.0, 12.0, text_color);
        }
        current_x += 24.0; // Space for checkmark
    }

    // Draw label
    ctx.set_text_align(TextAlign::Left);
    ctx.fill_text(item.label, current_x, y + item_height / 2.0);

    // Draw shortcut (right-aligned, before chevron)
    if let Some(shortcut) = item.shortcut {
        let shortcut_color = if matches!(state, WidgetState::Hovered | WidgetState::Pressed) {
            text_color // Same as label when hovered
        } else {
            theme.item_shortcut_color()
        };

        ctx.set_fill_color(shortcut_color);
        ctx.set_text_align(TextAlign::Right);

        let shortcut_x = if matches!(item.kind, MenuItemKind::Submenu) {
            x + width - hpad - 24.0 // Leave space for chevron
        } else {
            x + width - hpad
        };

        ctx.fill_text(shortcut, shortcut_x, y + item_height / 2.0);
    }

    // Draw chevron for submenu items
    if matches!(item.kind, MenuItemKind::Submenu) {
        let chevron_color = if matches!(state, WidgetState::Hovered | WidgetState::Pressed) {
            text_color
        } else {
            theme.item_chevron_color()
        };

        let chevron_x = x + width - hpad - 12.0; // Right edge, accounting for icon width
        let chevron_y = y + item_height / 2.0 - 6.0; // Center vertically (12x12 icon)

        draw_svg_icon(ctx, paths::CHEVRON_RIGHT, chevron_x, chevron_y, 8.0, 12.0, chevron_color);
    }

    item_height
}

/// Render a separator line. Returns height.
pub fn render_menu_separator(
    ctx: &mut dyn RenderContext,
    x: f64,
    y: f64,
    width: f64,
    theme: &MenuTheme,
) -> f64 {
    let sep_height = theme.separator_height();
    let inset = theme.separator_inset();
    let spacing = 3.0; // Vertical spacing around separator

    // Draw separator line in the middle of the allocated space
    let line_y = y + spacing;

    ctx.set_fill_color(theme.separator_color());
    ctx.fill_rect(x + inset, line_y, width - inset * 2.0, sep_height);

    sep_height + spacing * 2.0
}

/// Render a section header. Returns height.
pub fn render_menu_header(
    ctx: &mut dyn RenderContext,
    x: f64,
    y: f64,
    _width: f64,
    label: &str,
    theme: &MenuTheme,
) -> f64 {
    let item_height = theme.item_height();
    let (_, hpad) = theme.item_padding();

    ctx.set_fill_color(theme.header_text_color());
    ctx.set_font(theme.header_font());
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);

    ctx.fill_text(label, x + hpad, y + item_height / 2.0);

    item_height
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colors::AppearanceMode;

    #[test]
    fn test_menu_item_builder() {
        let item = MenuItem::action("Open");
        assert_eq!(item.label, "Open");
        assert!(item.enabled);
        assert_eq!(item.kind, MenuItemKind::Action);

        let item = MenuItem::action_with_shortcut("Save", "⌘S");
        assert_eq!(item.shortcut, Some("⌘S"));

        let item = MenuItem::toggle("Show Sidebar", true);
        assert!(matches!(item.kind, MenuItemKind::Toggle { checked: true }));

        let item = MenuItem::separator();
        assert_eq!(item.kind, MenuItemKind::Separator);

        let item = MenuItem::header("File");
        assert_eq!(item.kind, MenuItemKind::Header);
    }

    #[test]
    fn test_menu_item_enabled() {
        let item = MenuItem::action("Delete").enabled(false);
        assert!(!item.enabled);
        assert_eq!(item.state, WidgetState::Disabled);
    }

    #[test]
    fn test_compute_menu_height() {
        let theme = MenuTheme::new(AppearanceMode::Dark);
        let items = vec![
            MenuItem::action("Open"),
            MenuItem::action("Save"),
            MenuItem::separator(),
            MenuItem::action("Quit"),
        ];

        let height = compute_menu_height(&items, &theme);

        // Expected: vpad*2 + 3*item_height + separator_height + 6
        let expected = theme.container_padding().0 * 2.0
            + theme.item_height() * 3.0
            + theme.separator_height() + 6.0;

        assert_eq!(height, expected);
    }
}
