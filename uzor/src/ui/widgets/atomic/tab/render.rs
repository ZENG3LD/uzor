//! Tab rendering — 5 dedicated draw_* functions mirroring mlc variants,
//! plus a generic `draw_tab` dispatcher.

use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::Rect;

use super::settings::TabSettings;
use super::style::{ChromeTabStyle, ModalHorizontalTabStyle, ModalSidebarTabStyle, TagsTabsSidebarTabStyle};
use super::types::{TabConfig, TabKind};

// ---------------------------------------------------------------------------
// Shared view struct
// ---------------------------------------------------------------------------

pub struct TabView<'a> {
    pub tab: &'a TabConfig,
    pub hovered: bool,
    pub pressed: bool,
    pub close_btn_hovered: bool,
}

// ---------------------------------------------------------------------------
// Result
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone, Copy)]
pub struct TabResult {
    /// Rect of the close button (if `closable`); zero rect otherwise.
    pub close_rect: Rect,
}

// ---------------------------------------------------------------------------
// 1. Chrome tab
// ---------------------------------------------------------------------------

/// Draw a Chrome-style browser tab.
///
/// - No per-tab background fill — only the 2px bottom accent line marks the
///   active state (matching mlc: active tab has a colored line,
///   hovered-inactive tab has a muted line, inactive tab has nothing).
/// - Close × drawn inside the right edge of the tab.
/// - `rect` must already include the full cell width
///   (= `padding_h + text_w + close_size + padding_h`).
///
/// Returns the close-button rect so the caller can register its hit zone.
pub fn draw_chrome_tab(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    view: &TabView<'_>,
    style: &ChromeTabStyle,
    theme: &dyn super::theme::TabTheme,
) -> TabResult {
    // No per-tab background.

    // Bottom accent line — 2px, at y = rect.y + rect.height - accent_bar.
    let line_y = rect.y + rect.height - style.accent_bar_thickness;
    if view.tab.active {
        ctx.set_fill_color(theme.chrome_bottom_accent());
        ctx.fill_rect(rect.x, line_y, rect.width, style.accent_bar_thickness);
    } else if view.hovered {
        ctx.set_fill_color(theme.chrome_hover_line());
        ctx.fill_rect(rect.x, line_y, rect.width, style.accent_bar_thickness);
    }

    // Label — left-aligned, vertically centered.
    ctx.set_font(&format!("{}px sans-serif", style.font_size));
    ctx.set_fill_color(theme.text_normal()); // mlc: active text = same icon_normal color
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(
        &view.tab.label,
        rect.x + style.padding_h,
        rect.y + rect.height / 2.0,
    );

    // Close × (right-aligned in hit zone).
    let mut close_rect = Rect::default();
    if view.tab.closable {
        let icon_size = 14.0_f64; // mlc: 14×14 inside the 16px zone
        let zone_w = style.close_size;
        let cx = rect.x + rect.width - style.padding_h - zone_w;
        let cy = rect.y + (rect.height - zone_w) / 2.0;
        close_rect = Rect::new(cx, cy, zone_w, zone_w);

        let icon_x = cx + (zone_w - icon_size) / 2.0;
        let icon_y = cy + (zone_w - icon_size) / 2.0;
        let close_color = if view.close_btn_hovered {
            theme.close_hover()
        } else {
            theme.close_normal()
        };
        // Draw X as two crossed rectangles (1.5px stroke).
        ctx.set_fill_color(close_color);
        ctx.save();
        ctx.translate(icon_x + icon_size / 2.0, icon_y + icon_size / 2.0);
        ctx.rotate(std::f64::consts::FRAC_PI_4);
        ctx.fill_rect(-icon_size / 2.0, -0.75, icon_size, 1.5);
        ctx.fill_rect(-0.75, -icon_size / 2.0, 1.5, icon_size);
        ctx.restore();
    }

    TabResult { close_rect }
}

// ---------------------------------------------------------------------------
// 2. ModalSidebar tab
// ---------------------------------------------------------------------------

/// Draw an icon-only vertical modal sidebar tab.
///
/// - Active: 3px left accent bar + `draw_sidebar_active_item` background.
/// - Hover: only icon color change (no background — matches mlc).
/// - Icon centered in the cell; label is NOT rendered (sidebar is icon-only).
///
/// `rect` = full cell rect (width × button_height).
pub fn draw_modal_sidebar_tab(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    view: &TabView<'_>,
    style: &ModalSidebarTabStyle,
    theme: &dyn super::theme::TabTheme,
) -> TabResult {
    if view.tab.active {
        ctx.draw_sidebar_active_item(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            theme.sidebar_left_accent(),
            theme.sidebar_bg_active(),
            style.accent_bar_width,
        );
    }
    // No hover background for sidebar tabs (mlc: only icon color changes).

    // Icon centered in cell.
    let icon_x = rect.x + (rect.width - style.icon_size) / 2.0;
    let icon_y = rect.y + (rect.height - style.icon_size) / 2.0;
    let icon_color = if view.tab.active {
        theme.text_active()
    } else {
        theme.text_normal()
    };

    if let Some(icon_name) = view.tab.icon.as_ref() {
        // Caller renders the icon via its own icon system; we reserve the space
        // and provide a fallback single-char label so the region is visible.
        let _ = icon_name;
        ctx.set_font(&format!("{}px sans-serif", style.icon_size * 0.6));
        ctx.set_fill_color(icon_color);
        ctx.fill_text_centered(&view.tab.label, icon_x + style.icon_size / 2.0, icon_y + style.icon_size / 2.0);
    } else {
        // No icon — draw label as fallback, centered.
        ctx.set_font(&format!("bold {}px sans-serif", style.font_size));
        ctx.set_fill_color(icon_color);
        ctx.fill_text_centered(&view.tab.label, rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
    }

    TabResult::default()
}

// ---------------------------------------------------------------------------
// 3. ModalHorizontal tab
// ---------------------------------------------------------------------------

/// Draw a text-label horizontal tab (primitive settings, alert settings, etc.).
///
/// - Active: `draw_active_rect` (theme-aware glass/solid), white text.
/// - Inactive: text only, no background.
/// - No accent bar.
/// - `rect` must be pre-computed by caller (intrinsic width = `text_w + padding_h * 2`).
pub fn draw_modal_horizontal_tab(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    view: &TabView<'_>,
    style: &ModalHorizontalTabStyle,
    theme: &dyn super::theme::TabTheme,
) -> TabResult {
    if view.tab.active {
        ctx.draw_active_rect(rect.x, rect.y, rect.width, rect.height, theme.bg_active());
        ctx.set_fill_color("#ffffff"); // mlc: hardcoded white for active text
    } else {
        ctx.set_fill_color(theme.text_normal());
    }

    ctx.set_font(&format!("{}px sans-serif", style.font_size));
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(
        &view.tab.label,
        rect.x + style.padding_h,
        rect.y + rect.height / 2.0,
    );

    TabResult::default()
}

// ---------------------------------------------------------------------------
// 4. TagsTabsSidebar tab
// ---------------------------------------------------------------------------

/// Draw a text-only pill tab for the TagsTabsSidebar (Tabs / Tags / Map).
///
/// - Active: `fill_rounded_rect` at `accent` color × 0.20 alpha; text = accent.
/// - Hover: same rect at `item_text` × 0.08 alpha; text = item_text.
/// - Inactive: text only, no background.
/// - Pill inset: x+4, y+2, w-8, h-4 (matches mlc geometry).
///
/// `rect` = full item cell rect (`width × item_height`).
pub fn draw_tags_tabs_sidebar_tab(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    view: &TabView<'_>,
    style: &TagsTabsSidebarTabStyle,
    theme: &dyn super::theme::TabTheme,
) -> TabResult {
    let pill_x = rect.x + style.pill_inset_x;
    let pill_y = rect.y + style.pill_inset_y;
    let pill_w = rect.width - style.pill_inset_x * 2.0;
    let pill_h = rect.height - style.pill_inset_y * 2.0;

    if view.tab.active {
        ctx.set_fill_color_alpha(theme.tags_pill_bg_active(), style.active_pill_alpha);
        ctx.fill_rounded_rect(pill_x, pill_y, pill_w, pill_h, style.pill_radius);
        ctx.reset_alpha();
        ctx.set_fill_color(theme.accent());
    } else if view.hovered {
        ctx.set_fill_color_alpha(theme.tags_pill_bg_hover(), style.hover_pill_alpha);
        ctx.fill_rounded_rect(pill_x, pill_y, pill_w, pill_h, style.pill_radius);
        ctx.reset_alpha();
        ctx.set_fill_color(theme.text_normal());
    } else {
        ctx.set_fill_color(theme.text_normal());
    }

    ctx.set_font(&format!("bold {}px sans-serif", style.font_size));
    ctx.fill_text_centered(&view.tab.label, rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);

    TabResult::default()
}

// ---------------------------------------------------------------------------
// 5. Generic / fallback dispatcher
// ---------------------------------------------------------------------------

/// Generic tab renderer.
///
/// Uses the style/theme stored in `settings` and renders via the `TabStyle`
/// trait — suitable for custom variants or generic tab strips where the caller
/// does not need a specific mlc variant.
///
/// For mlc-parity callers, prefer the dedicated `draw_chrome_tab`,
/// `draw_modal_sidebar_tab`, `draw_modal_horizontal_tab`,
/// `draw_tags_tabs_sidebar_tab` functions with their typed style structs.
pub fn draw_tab(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    view: &TabView<'_>,
    settings: &TabSettings,
) -> TabResult {
    let style = settings.style.as_ref();
    let theme = settings.theme.as_ref();

    // Background (active wins over hover).
    let bg = if view.tab.active {
        theme.bg_active()
    } else if view.hovered || view.pressed {
        theme.bg_hover()
    } else {
        theme.bg_normal()
    };
    ctx.set_fill_color(bg);
    ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, style.radius());

    // Active accent bar (left edge).
    if view.tab.active {
        ctx.set_fill_color(theme.accent());
        ctx.fill_rect(rect.x, rect.y, style.accent_bar(), rect.height);
    }

    // Content layout: icon (optional) + label.
    let pad_x = style.padding_x();
    let mut text_x = rect.x + pad_x;

    if view.tab.icon.is_some() {
        // Icon rendering deferred to caller (no IconId in TabConfig — uses string name).
        // Reserve the space.
        text_x += style.icon_size() + style.gap();
    }

    // Label.
    ctx.set_font(&format!("{}px sans-serif", style.font_size()));
    ctx.set_fill_color(if view.tab.active { theme.text_active() } else { theme.text_normal() });
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(&view.tab.label, text_x, rect.y + rect.height / 2.0);

    // Close button (right-aligned).
    let mut close_rect = Rect::default();
    if view.tab.closable {
        let s = style.close_btn_size();
        let cx = rect.x + rect.width - pad_x - s;
        let cy = rect.y + (rect.height - s) / 2.0;
        close_rect = Rect::new(cx, cy, s, s);
        let close_color = if view.close_btn_hovered { theme.close_hover() } else { theme.close_normal() };
        ctx.set_fill_color(close_color);
        ctx.fill_rect(cx + s * 0.45, cy + s * 0.15, 1.5, s * 0.7);
        ctx.fill_rect(cx + s * 0.15, cy + s * 0.45, s * 0.7, 1.5);
    }

    TabResult { close_rect }
}

// ---------------------------------------------------------------------------
// 6. TabKind dispatcher
// ---------------------------------------------------------------------------

/// Dispatch to the appropriate variant renderer based on `kind`.
///
/// Uses the typed preset styles from `settings.variant_styles`.  The caller
/// must supply `settings` with the correct variant styles populated.
pub fn draw_tab_variant(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    view: &TabView<'_>,
    kind: TabKind,
    settings: &TabSettings,
) -> TabResult {
    match kind {
        TabKind::Chrome => draw_chrome_tab(
            ctx, rect, view,
            &settings.chrome,
            settings.theme.as_ref(),
        ),
        TabKind::ModalSidebar => draw_modal_sidebar_tab(
            ctx, rect, view,
            &settings.modal_sidebar,
            settings.theme.as_ref(),
        ),
        TabKind::ModalHorizontal => draw_modal_horizontal_tab(
            ctx, rect, view,
            &settings.modal_horizontal,
            settings.theme.as_ref(),
        ),
        TabKind::TagsTabsSidebar => draw_tags_tabs_sidebar_tab(
            ctx, rect, view,
            &settings.tags_sidebar,
            settings.theme.as_ref(),
        ),
    }
}
