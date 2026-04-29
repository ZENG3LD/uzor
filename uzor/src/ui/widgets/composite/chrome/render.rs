//! Chrome render entry point and 16-step draw pipeline.
//!
//! # API
//!
//! - `register_input_coordinator_chrome` — registers the composite + all child
//!   hit-rects with an `InputCoordinator`.  **No drawing.**
//! - `register_context_manager_chrome`   — convenience wrapper: registers and
//!   draws in one call via a `ContextManager`.
//!
//! # Draw order (non-Custom kinds)
//!
//! 1.  Background strip
//! 2.  Tab bodies (hover/active accent 2 px bottom line + label)
//! 3.  Tab close-X icons
//! 4.  New-tab "+" button
//! 5.  New-window icon button
//! 6.  Menu / gear icon button
//! 7.  Group divider (left of min/max/close)
//! 8.  Close-window icon button
//! 9.  Group divider (between close-window and minimize group)
//! 10. Minimize button
//! 11. Maximize / restore button
//! 12. Close-app button (red hover BG)
//! 13. Active tab bottom accent indicator
//! 14. Bottom separator line
//! 15. Context menu (if open)
//! 16. Tooltip overlay (if visible)

use crate::app_context::ContextManager;
use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::{Rect, WidgetId};

use super::settings::ChromeSettings;
use super::state::ChromeState;
use super::types::{ChromeRenderKind, ChromeTabConfig, ChromeView};

// ---------------------------------------------------------------------------
// Layout constants (mirrors chrome-deep.md §2)
// ---------------------------------------------------------------------------

const TAB_LEFT_MARGIN: f64 = 4.0;
const NEW_TAB_BTN_WIDTH: f64 = 28.0;
const BUTTON_WIDTH: f64 = 46.0;
const CLOSE_WINDOW_BTN_WIDTH: f64 = 36.0;
const MENU_BTN_WIDTH: f64 = 36.0;
const NEW_WINDOW_BTN_WIDTH: f64 = 36.0;

// ---------------------------------------------------------------------------
// ButtonPositions helper
// ---------------------------------------------------------------------------

struct ButtonPositions {
    close_x: f64,
    maximize_x: f64,
    minimize_x: f64,
    close_window_left: f64,
    menu_left: f64,
    new_window_left: f64,
}

impl ButtonPositions {
    fn compute(width: f64) -> Self {
        let close_x           = width - BUTTON_WIDTH;
        let maximize_x        = width - BUTTON_WIDTH * 2.0;
        let minimize_x        = width - BUTTON_WIDTH * 3.0;
        let close_window_left = minimize_x - CLOSE_WINDOW_BTN_WIDTH;
        let menu_left         = close_window_left - MENU_BTN_WIDTH;
        let new_window_left   = menu_left - NEW_WINDOW_BTN_WIDTH;
        Self {
            close_x,
            maximize_x,
            minimize_x,
            close_window_left,
            menu_left,
            new_window_left,
        }
    }
}

// ---------------------------------------------------------------------------
// Public API — InputCoordinator
// ---------------------------------------------------------------------------

/// Register the Chrome composite and all child hit-rects with the coordinator.
///
/// **No drawing happens here.**
///
/// Returns the `WidgetId` assigned to the composite.
pub fn register_input_coordinator_chrome(
    coord:    &mut InputCoordinator,
    id:       impl Into<WidgetId>,
    rect:     Rect,
    state:    &ChromeState,
    view:     &ChromeView<'_>,
    settings: &ChromeSettings,
    kind:     &ChromeRenderKind,
    layer:    &LayerId,
) -> WidgetId {
    let chrome_id = coord.register_composite(
        id,
        WidgetKind::Chrome,
        rect,
        Sense::NONE,
        layer,
    );

    if matches!(kind, ChromeRenderKind::Custom(_)) {
        return chrome_id;
    }

    let style = settings.style.as_ref();
    let bp    = ButtonPositions::compute(rect.width);
    let h     = style.chrome_height();

    // --- Tabs ---
    let show_tabs = !matches!(kind, ChromeRenderKind::WindowControlsOnly);
    if show_tabs {
        let mut x = rect.x + TAB_LEFT_MARGIN;
        for (i, tab) in view.tabs.iter().enumerate() {
            let tab_w = tab_width(tab, state, i, style.tab_padding_h(), style.tab_close_size());
            let tab_rect = Rect::new(x, rect.y, tab_w, h);

            // Tab body
            coord.register_child(
                &chrome_id,
                format!("{}:tab:{}", chrome_id.0, i),
                WidgetKind::Button,
                tab_rect,
                Sense::CLICK | Sense::HOVER,
            );

            // Tab close-X
            if tab.closable {
                let close_w = style.tab_close_size();
                let close_x = x + tab_w - close_w - style.tab_padding_h() / 2.0;
                let close_y = rect.y + (h - close_w) / 2.0;
                coord.register_child(
                    &chrome_id,
                    format!("{}:tab:{}:close", chrome_id.0, i),
                    WidgetKind::Button,
                    Rect::new(close_x, close_y, close_w, close_w),
                    Sense::CLICK | Sense::HOVER,
                );
            }

            x += tab_w + style.tab_gap();
        }

        // New-tab "+" button
        if view.show_new_tab_btn {
            coord.register_child(
                &chrome_id,
                format!("{}:new_tab", chrome_id.0),
                WidgetKind::Button,
                Rect::new(x, rect.y, NEW_TAB_BTN_WIDTH, h),
                Sense::CLICK | Sense::HOVER,
            );
            x += NEW_TAB_BTN_WIDTH;
        }

        // Caption / drag zone
        let drag_right = rect.x + bp.new_window_left;
        let drag_w = (drag_right - x).max(0.0);
        if drag_w > 0.0 {
            coord.register_child(
                &chrome_id,
                format!("{}:drag", chrome_id.0),
                WidgetKind::DragHandle,
                Rect::new(x, rect.y, drag_w, h),
                Sense::DRAG,
            );
        }
    } else {
        // WindowControlsOnly: drag zone fills the full width except button column
        let drag_end = rect.x + bp.minimize_x;
        let drag_w   = drag_end - rect.x;
        if drag_w > 0.0 {
            coord.register_child(
                &chrome_id,
                format!("{}:drag", chrome_id.0),
                WidgetKind::DragHandle,
                Rect::new(rect.x, rect.y, drag_w, h),
                Sense::DRAG,
            );
        }
    }

    // --- Action buttons (right side) ---
    let show_window_controls = !matches!(kind, ChromeRenderKind::Minimal);

    if show_window_controls {
        // New-window button
        if !matches!(kind, ChromeRenderKind::WindowControlsOnly) {
            coord.register_child(
                &chrome_id,
                format!("{}:new_win", chrome_id.0),
                WidgetKind::Button,
                Rect::new(rect.x + bp.new_window_left, rect.y, NEW_WINDOW_BTN_WIDTH, h),
                Sense::CLICK | Sense::HOVER,
            );

            // Menu button
            if view.show_menu_btn {
                coord.register_child(
                    &chrome_id,
                    format!("{}:menu", chrome_id.0),
                    WidgetKind::Button,
                    Rect::new(rect.x + bp.menu_left, rect.y, MENU_BTN_WIDTH, h),
                    Sense::CLICK | Sense::HOVER,
                );
            }

            // Close-window button
            coord.register_child(
                &chrome_id,
                format!("{}:close_win", chrome_id.0),
                WidgetKind::Button,
                Rect::new(rect.x + bp.close_window_left, rect.y, CLOSE_WINDOW_BTN_WIDTH, h),
                Sense::CLICK | Sense::HOVER,
            );
        }

        // Minimize
        coord.register_child(
            &chrome_id,
            format!("{}:min", chrome_id.0),
            WidgetKind::Button,
            Rect::new(rect.x + bp.minimize_x, rect.y, BUTTON_WIDTH, h),
            Sense::CLICK | Sense::HOVER,
        );

        // Maximize / restore
        coord.register_child(
            &chrome_id,
            format!("{}:max", chrome_id.0),
            WidgetKind::Button,
            Rect::new(rect.x + bp.maximize_x, rect.y, BUTTON_WIDTH, h),
            Sense::CLICK | Sense::HOVER,
        );

        // Close app
        coord.register_child(
            &chrome_id,
            format!("{}:close", chrome_id.0),
            WidgetKind::Button,
            Rect::new(rect.x + bp.close_x, rect.y, BUTTON_WIDTH, h),
            Sense::CLICK | Sense::HOVER,
        );
    }

    chrome_id
}

// ---------------------------------------------------------------------------
// Public API — ContextManager convenience wrapper
// ---------------------------------------------------------------------------

/// Register + draw the Chrome composite in one call using a `ContextManager`.
///
/// Returns the `WidgetId` assigned to the composite.
pub fn register_context_manager_chrome(
    ctx_mgr:  &mut ContextManager,
    render:   &mut dyn RenderContext,
    id:       impl Into<WidgetId>,
    rect:     Rect,
    state:    &mut ChromeState,
    view:     &ChromeView<'_>,
    settings: &ChromeSettings,
    kind:     &ChromeRenderKind,
    layer:    &LayerId,
) -> WidgetId {
    let coord = &mut ctx_mgr.input;
    let chrome_id =
        register_input_coordinator_chrome(coord, id, rect, state, view, settings, kind, layer);
    draw_chrome_internal(render, rect, state, view, settings, kind);
    chrome_id
}

// ---------------------------------------------------------------------------
// Internal draw pipeline
// ---------------------------------------------------------------------------

fn draw_chrome_internal(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    state:    &ChromeState,
    view:     &ChromeView<'_>,
    settings: &ChromeSettings,
    kind:     &ChromeRenderKind,
) {
    match kind {
        ChromeRenderKind::Custom(f) => {
            f(ctx, rect, view, settings);
            return;
        }
        _ => {}
    }

    let theme = settings.theme.as_ref();
    let style = settings.style.as_ref();
    let h     = style.chrome_height();
    let w     = rect.width;
    let bp    = ButtonPositions::compute(w);

    // --- 1. Background strip ---
    ctx.set_fill_color(theme.background());
    ctx.fill_rect(rect.x, rect.y, w, h);

    let show_tabs     = !matches!(kind, ChromeRenderKind::WindowControlsOnly);
    let show_controls = !matches!(kind, ChromeRenderKind::Minimal);

    // --- 2–3. Tab bodies + close-X icons ---
    if show_tabs {
        let mut x = rect.x + TAB_LEFT_MARGIN;
        for (i, tab) in view.tabs.iter().enumerate() {
            let tw = tab_width(tab, state, i, style.tab_padding_h(), style.tab_close_size());
            let tab_rect = Rect::new(x, rect.y, tw, h);

            let ts = state.tabs_state.get(i);
            let hovered      = ts.map(|s| s.hovered).unwrap_or(false);
            let close_hovered = ts.map(|s| s.close_hovered).unwrap_or(false);
            let active = view.active_tab_id.map(|id| id == tab.id).unwrap_or(false);

            // Tab background
            let bg_color = if active {
                theme.tab_bg_active()
            } else if hovered || close_hovered {
                theme.tab_bg_hover()
            } else {
                theme.tab_bg_normal()
            };
            if bg_color != "transparent" {
                ctx.set_fill_color(bg_color);
                ctx.fill_rect(tab_rect.x, tab_rect.y, tab_rect.width, tab_rect.height);
            }

            // Label text
            let text_color = if active {
                theme.tab_text_active()
            } else if hovered {
                theme.tab_text_hover()
            } else {
                theme.tab_text_normal()
            };
            ctx.set_fill_color(text_color);
            ctx.set_font("12px sans-serif");
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text(
                tab.label,
                tab_rect.x + style.tab_padding_h(),
                tab_rect.y + h / 2.0,
            );

            // Tab close-X icon (step 3)
            if tab.closable {
                let icon_sz  = style.tab_close_icon_size();
                let close_bw = style.tab_close_size();
                let cx = x + tw - close_bw + (close_bw - icon_sz) / 2.0;
                let cy = rect.y + (h - icon_sz) / 2.0;
                let icon_color = if close_hovered {
                    theme.close_hover()
                } else {
                    theme.icon_normal()
                };
                draw_cross(ctx, cx, cy, icon_sz, icon_color, 1.0);
            }

            x += tw + style.tab_gap();
        }

        // --- 4. New-tab "+" button ---
        if view.show_new_tab_btn {
            let nt_x = rect.x
                + TAB_LEFT_MARGIN
                + tab_total_width(view.tabs, state, style.tab_padding_h(),
                                   style.tab_close_size(), style.tab_gap());
            // 1 px separator before "+"
            ctx.set_fill_color(theme.separator());
            ctx.fill_rect(nt_x, rect.y + 4.0, 1.0, h - 8.0);
            // Cross strokes
            let cx = nt_x + NEW_TAB_BTN_WIDTH / 2.0;
            let cy = rect.y + h / 2.0;
            let arm = 5.0_f64;
            ctx.set_stroke_color(theme.icon_normal());
            ctx.set_stroke_width(1.5);
            ctx.set_line_dash(&[]);
            // horizontal
            ctx.move_to(cx - arm, cy);
            ctx.line_to(cx + arm, cy);
            ctx.stroke();
            // vertical
            ctx.move_to(cx, cy - arm);
            ctx.line_to(cx, cy + arm);
            ctx.stroke();
        }
    }

    // --- Right-side action buttons ---
    if show_controls {
        let icon_sz = style.action_icon_size();

        // --- 5. New-window icon ---
        if show_tabs {
            let nw_cx = rect.x + bp.new_window_left + NEW_WINDOW_BTN_WIDTH / 2.0;
            let nw_cy = rect.y + h / 2.0;
            ctx.set_fill_color(theme.icon_normal());
            // Simple rectangle icon (placeholder for SVG)
            ctx.fill_rect(
                nw_cx - icon_sz / 2.0,
                nw_cy - icon_sz / 2.0,
                icon_sz,
                icon_sz,
            );
        }

        // --- 6. Menu icon ---
        if show_tabs && view.show_menu_btn {
            let menu_cx = rect.x + bp.menu_left + MENU_BTN_WIDTH / 2.0;
            let menu_cy = rect.y + h / 2.0;
            let line_w  = icon_sz;
            ctx.set_fill_color(theme.icon_normal());
            for i in 0_i32..3_i32 {
                let ly = menu_cy - 3.0 + i as f64 * 3.0;
                ctx.fill_rect(menu_cx - line_w / 2.0, ly - 0.75, line_w, 1.5);
            }
        }

        // --- 7. Divider left of min/max/close ---
        ctx.set_fill_color(theme.separator());
        ctx.fill_rect(rect.x + bp.minimize_x - 1.0, rect.y + 6.0, 1.0, h - 12.0);

        // --- 8. Close-window icon ---
        if show_tabs {
            let cw_cx = rect.x + bp.close_window_left + CLOSE_WINDOW_BTN_WIDTH / 2.0;
            let cw_cy = rect.y + h / 2.0;
            let arm = 3.5_f64;
            draw_cross(ctx, cw_cx - arm, cw_cy - arm, arm * 2.0, theme.icon_normal(), 1.0);
        }

        // --- 9. Divider between close-window and minimize group ---
        if show_tabs {
            ctx.set_fill_color(theme.separator());
            ctx.fill_rect(
                rect.x + bp.close_window_left - 1.0,
                rect.y + 6.0,
                1.0,
                h - 12.0,
            );
        }

        // --- 10. Minimize icon (10×1 filled rect) ---
        {
            let mid_x = rect.x + bp.minimize_x + BUTTON_WIDTH / 2.0;
            let mid_y = rect.y + h / 2.0;
            ctx.set_fill_color(theme.icon_normal());
            ctx.fill_rect(mid_x - 5.0, mid_y - 0.5, 10.0, 1.0);
        }

        // --- 11. Maximize / restore icon ---
        {
            let mid_x = rect.x + bp.maximize_x + BUTTON_WIDTH / 2.0;
            let mid_y = rect.y + h / 2.0;
            ctx.set_stroke_color(theme.icon_normal());
            ctx.set_stroke_width(1.0);
            ctx.set_line_dash(&[]);
            if view.is_maximized {
                // Restore: two overlapping rects
                ctx.stroke_rect(mid_x - 3.0, mid_y - 5.0, 8.0, 8.0);
                ctx.stroke_rect(mid_x - 5.0, mid_y - 3.0, 8.0, 8.0);
            } else {
                ctx.stroke_rect(mid_x - 5.0, mid_y - 5.0, 10.0, 10.0);
            }
        }

        // --- 12. Close-app button (hover = red BG) ---
        {
            let close_rect = Rect::new(rect.x + bp.close_x, rect.y, BUTTON_WIDTH, h);
            // Hover background
            if state.hovered == super::types::ChromeHit::CloseBtn {
                ctx.set_fill_color(theme.close_hover());
                ctx.fill_rect(close_rect.x, close_rect.y, close_rect.width, close_rect.height);
            }
            let cx  = rect.x + bp.close_x + BUTTON_WIDTH / 2.0;
            let cy  = rect.y + h / 2.0;
            let arm = 5.0_f64;
            draw_cross(ctx, cx - arm, cy - arm, arm * 2.0, theme.icon_normal(), 1.5);
        }
    }

    // --- 13. Active tab bottom accent indicator ---
    if show_tabs {
        let mut x = rect.x + TAB_LEFT_MARGIN;
        for (i, tab) in view.tabs.iter().enumerate() {
            let tw = tab_width(tab, state, i, style.tab_padding_h(), style.tab_close_size());
            let active = view.active_tab_id.map(|id| id == tab.id).unwrap_or(false);
            let ts = state.tabs_state.get(i);
            let hovered = ts.map(|s| s.hovered).unwrap_or(false);
            if active || hovered {
                let accent_color = if active {
                    theme.tab_accent()
                } else {
                    theme.button_hover()
                };
                ctx.set_fill_color(accent_color);
                ctx.fill_rect(
                    x,
                    rect.y + h - style.tab_accent_height(),
                    tw,
                    style.tab_accent_height(),
                );
            }
            x += tw + style.tab_gap();
        }
    }

    // --- 14. Bottom separator line ---
    ctx.set_fill_color(theme.separator());
    ctx.fill_rect(rect.x, rect.y + h - 1.0, w, 1.0);

    // --- 15. Context menu (if open) ---
    // Context menu rendering is delegated to the composite::context_menu widget.
    // The chrome state holds a `ContextMenuState`; callers drive rendering
    // via the context_menu composite after this call.

    // --- 16. Tooltip overlay ---
    // Tooltip rendering is also deferred to the caller via `state.tooltip`.
}

// ---------------------------------------------------------------------------
// Internal draw helpers
// ---------------------------------------------------------------------------

/// Draw an × cross centred at `(x, y)` (top-left origin) with `size` and `stroke_w`.
fn draw_cross(
    ctx:      &mut dyn RenderContext,
    x:        f64,
    y:        f64,
    size:     f64,
    color:    &str,
    stroke_w: f64,
) {
    ctx.set_stroke_color(color);
    ctx.set_stroke_width(stroke_w);
    ctx.set_line_dash(&[]);
    ctx.move_to(x, y);
    ctx.line_to(x + size, y + size);
    ctx.stroke();
    ctx.move_to(x + size, y);
    ctx.line_to(x, y + size);
    ctx.stroke();
}

// ---------------------------------------------------------------------------
// Tab width helpers
// ---------------------------------------------------------------------------

/// Pixel width for tab `i`, using cached `tab_widths` if available.
fn tab_width(
    tab:          &ChromeTabConfig<'_>,
    state:        &ChromeState,
    i:            usize,
    padding_h:    f64,
    close_size:   f64,
) -> f64 {
    if let Some(&w) = state.tab_widths.get(i) {
        return w;
    }
    // Fallback: estimate from label length (12 px per char).
    let text_w = tab.label.len() as f64 * 7.0;
    padding_h + text_w + close_size + padding_h
}

/// Total pixel width consumed by all tabs + gaps.
fn tab_total_width(
    tabs:       &[ChromeTabConfig<'_>],
    state:      &ChromeState,
    padding_h:  f64,
    close_size: f64,
    gap:        f64,
) -> f64 {
    let mut total = 0.0_f64;
    for (i, tab) in tabs.iter().enumerate() {
        total += tab_width(tab, state, i, padding_h, close_size);
        if i + 1 < tabs.len() {
            total += gap;
        }
    }
    total
}
