//! Panel render entry point and per-kind layout pipelines.
//!
//! # API convention
//!
//! - `register_input_coordinator_panel` — registers the composite + all child
//!   hit-rects with an `InputCoordinator`.  **No drawing.**
//! - `register_context_manager_panel`   — convenience wrapper: takes a
//!   `ContextManager`, registers, and draws in one call.
//!
//! # Draw order (non-Custom kinds)
//!
//! 1. Background fill
//! 2. Header strip (title + action buttons) — optional
//! 3. Header bottom divider
//! 4. Column-header row — optional (WithHeaderColumns / WithHeaderColumnsFooter)
//! 5. Column-header bottom divider
//! 6. Body closure
//! 7. Footer top divider + footer closure — optional
//! 8. Scrollbar — optional

use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::{Rect, WidgetId};
use crate::ui::widgets::atomic::scrollbar::render::{
    draw_scrollbar_standard, ScrollbarVisualState,
};

use super::settings::PanelSettings;
use super::state::PanelState;
use super::style::BackgroundFill;
use super::types::{PanelRenderKind, PanelView};

// ---------------------------------------------------------------------------
// Internal layout struct
// ---------------------------------------------------------------------------

struct PanelLayout {
    /// Full frame rect (same as outer `rect`).
    frame: Rect,
    /// Header strip; zero height when not rendered.
    header: Rect,
    /// Column-header row; zero height when not rendered.
    col_header: Rect,
    /// Body area available to the body closure.
    body: Rect,
    /// Footer strip; zero height when not rendered.
    footer: Rect,
    /// Scrollbar column; zero width when not rendered.
    scrollbar: Rect,
}

// ---------------------------------------------------------------------------
// Public API — register (InputCoordinator)
// ---------------------------------------------------------------------------

/// Register the panel composite and all its child hit-rects.
///
/// **No drawing happens here.**  Use when you need explicit z-order control.
///
/// Returns the `WidgetId` assigned to the panel composite.
pub fn register_input_coordinator_panel(
    coord:    &mut InputCoordinator,
    id:       impl Into<WidgetId>,
    rect:     Rect,
    state:    &PanelState,
    view:     &PanelView<'_>,
    settings: &PanelSettings,
    kind:     &PanelRenderKind,
    layer:    &LayerId,
) -> WidgetId {
    let panel_id = coord.register_composite(id, WidgetKind::Panel, rect, Sense::CLICK, layer);

    if let PanelRenderKind::Custom(_) = kind {
        return panel_id;
    }

    let layout = compute_layout(rect, view, settings, kind);

    // --- Header action buttons ---
    if let Some(hdr) = &view.header {
        let btn_size  = 20.0_f64;
        let btn_gap   = 4.0_f64;
        let pad_right = 6.0_f64;

        for (i, action) in hdr.actions.iter().enumerate() {
            let btn_x = layout.header.x + layout.header.width
                - pad_right
                - btn_size * (i + 1) as f64
                - btn_gap * i as f64;
            let btn_y = layout.header.y + (layout.header.height - btn_size) / 2.0;
            coord.register_child(
                &panel_id,
                format!("{}:action:{}", panel_id.0, action.id),
                WidgetKind::Button,
                Rect::new(btn_x, btn_y, btn_size, btn_size),
                Sense::CLICK | Sense::HOVER,
            );
        }
    }

    // --- Column-header cells (sortable) ---
    if layout.col_header.height > 0.0 && !view.columns.is_empty() {
        let available_w = layout.col_header.width;
        let mut cursor_x = layout.col_header.x;

        for col in view.columns {
            let col_w = available_w * col.width;
            let col_rect = Rect::new(cursor_x, layout.col_header.y, col_w, layout.col_header.height);
            if col.sortable {
                coord.register_child(
                    &panel_id,
                    format!("{}:col:{}", panel_id.0, col.id),
                    WidgetKind::Button,
                    col_rect,
                    Sense::CLICK | Sense::HOVER,
                );
            }
            cursor_x += col_w;
        }
    }

    // --- Viewport scroll zone ---
    coord.register_child(
        &panel_id,
        format!("{}:viewport", panel_id.0),
        WidgetKind::Custom,
        layout.body,
        Sense::SCROLL | Sense::HOVER,
    );

    // --- Scrollbar handle + track ---
    if view.show_scrollbar && layout.scrollbar.width > 0.0 {
        let scroll_offset  = state.scroll.offset;
        let viewport_h     = layout.body.height;
        let content_h      = view.content_height;

        if content_h > viewport_h {
            let visible_ratio = (viewport_h / content_h).clamp(0.0, 1.0);
            let max_scroll    = (content_h - viewport_h).max(0.0);
            let scroll_ratio  = if max_scroll > 0.0 { (scroll_offset / max_scroll).clamp(0.0, 1.0) } else { 0.0 };
            let thumb_len     = (layout.scrollbar.height * visible_ratio).max(24.0).min(layout.scrollbar.height);
            let available     = (layout.scrollbar.height - thumb_len).max(0.0);
            let thumb_y       = layout.scrollbar.y + scroll_ratio * available;
            let thumb_rect    = Rect::new(layout.scrollbar.x, thumb_y, layout.scrollbar.width, thumb_len);
            let inflated      = Rect::new(
                thumb_rect.x - 5.0,
                thumb_rect.y,
                thumb_rect.width + 10.0,
                thumb_rect.height,
            );

            coord.register_child(
                &panel_id,
                format!("{}:scrollbar_handle", panel_id.0),
                WidgetKind::ScrollbarHandle,
                inflated,
                Sense::DRAG,
            );
            coord.register_child(
                &panel_id,
                format!("{}:scrollbar_track", panel_id.0),
                WidgetKind::ScrollbarTrack,
                layout.scrollbar,
                Sense::CLICK,
            );
        }
    }

    panel_id
}

// ---------------------------------------------------------------------------
// Public API — convenience wrapper (ContextManager)
// ---------------------------------------------------------------------------

/// Register + draw a panel in one call using a `ContextManager`.
///
/// Returns the `WidgetId` assigned to the panel composite.
pub fn register_context_manager_panel(
    ctx_mgr:  &mut crate::app_context::ContextManager,
    render:   &mut dyn RenderContext,
    id:       impl Into<WidgetId>,
    rect:     Rect,
    state:    &mut PanelState,
    view:     &mut PanelView<'_>,
    settings: &PanelSettings,
    kind:     &PanelRenderKind,
    layer:    &LayerId,
) -> WidgetId {
    let coord = &mut ctx_mgr.input;
    let panel_id =
        register_input_coordinator_panel(coord, id, rect, state, view, settings, kind, layer);
    draw_panel_with_coord(render, rect, coord, state, view, settings, kind);
    panel_id
}

// ---------------------------------------------------------------------------
// Internal draw pipeline
// ---------------------------------------------------------------------------

fn draw_panel_with_coord(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    coord:    &mut InputCoordinator,
    state:    &mut PanelState,
    view:     &mut PanelView<'_>,
    settings: &PanelSettings,
    kind:     &PanelRenderKind,
) {
    if let PanelRenderKind::Custom(f) = kind {
        f(ctx, rect, view, settings);
        return;
    }

    let theme  = settings.theme.as_ref();
    let style  = settings.style.as_ref();
    let layout = compute_layout(rect, view, settings, kind);

    // --- 1. Background -------------------------------------------------------
    match style.background_fill() {
        BackgroundFill::Solid => {
            ctx.set_fill_color(theme.bg());
            ctx.fill_rect(layout.frame.x, layout.frame.y, layout.frame.width, layout.frame.height);
        }
        BackgroundFill::Glass { blur_radius: _ } => {
            ctx.draw_blur_background(layout.frame.x, layout.frame.y, layout.frame.width, layout.frame.height);
            ctx.set_fill_color(theme.bg());
            ctx.fill_rect(layout.frame.x, layout.frame.y, layout.frame.width, layout.frame.height);
        }
    }

    // --- 2. Header strip -----------------------------------------------------
    let has_header = matches!(
        kind,
        PanelRenderKind::WithHeader
            | PanelRenderKind::WithHeaderColumns
            | PanelRenderKind::WithFooter
            | PanelRenderKind::WithHeaderColumnsFooter
    );

    if has_header && layout.header.height > 0.0 {
        ctx.set_fill_color(theme.header_bg());
        ctx.fill_rect(layout.header.x, layout.header.y, layout.header.width, layout.header.height);

        if let Some(hdr) = &view.header {
            let pad = style.padding();

            // Title
            ctx.set_fill_color(theme.header_text());
            ctx.set_font("11px sans-serif");
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text(
                hdr.title,
                layout.header.x + pad,
                layout.header.y + layout.header.height / 2.0,
            );

            // Action buttons (right-to-left)
            let btn_size  = 20.0_f64;
            let btn_gap   = 4.0_f64;
            let pad_right = 6.0_f64;
            for (i, action) in hdr.actions.iter().enumerate() {
                let btn_x = layout.header.x + layout.header.width
                    - pad_right
                    - btn_size * (i + 1) as f64
                    - btn_gap * i as f64;
                let btn_y = layout.header.y + (layout.header.height - btn_size) / 2.0;

                let is_hovered = state.hovered_action.as_deref() == Some(action.id)
                    || action.hovered;
                if is_hovered {
                    ctx.set_fill_color("rgba(255,255,255,0.08)");
                    ctx.fill_rounded_rect(btn_x, btn_y, btn_size, btn_size, 3.0);
                }

                let icon_color = if is_hovered {
                    theme.action_icon_hover()
                } else {
                    theme.action_icon_normal()
                };
                ctx.set_fill_color(icon_color);
                // Icon draw point reserved — actual SVG render by caller.
                let _ = (btn_x, btn_y);
            }
        }

        // Header bottom divider
        ctx.set_fill_color(theme.divider());
        ctx.fill_rect(
            layout.header.x,
            layout.header.y + layout.header.height - 1.0,
            layout.header.width,
            1.0,
        );
    }

    // --- 3. Column-header row ------------------------------------------------
    let has_col_header = matches!(
        kind,
        PanelRenderKind::WithHeaderColumns | PanelRenderKind::WithHeaderColumnsFooter
    );

    if has_col_header && layout.col_header.height > 0.0 {
        ctx.set_fill_color(theme.column_header_bg());
        ctx.fill_rect(
            layout.col_header.x,
            layout.col_header.y,
            layout.col_header.width,
            layout.col_header.height,
        );

        let available_w = layout.col_header.width;
        let mut cursor_x = layout.col_header.x;

        for col in view.columns {
            let col_w    = available_w * col.width;
            let label_x  = cursor_x + style.padding();
            let label_y  = layout.col_header.y + layout.col_header.height / 2.0;

            let is_sort_col = state.sort_column.as_deref() == Some(col.id);
            let is_hovered  = state.hovered_column.as_deref() == Some(col.id);

            let label_color = if is_hovered {
                theme.header_text()
            } else {
                theme.column_header_text()
            };

            ctx.set_fill_color(label_color);
            ctx.set_font("10px sans-serif");
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text(col.label, label_x, label_y);

            // Sort arrow (small caret after label)
            if is_sort_col {
                draw_sort_arrow(ctx, cursor_x + col_w - 12.0, label_y, state.sort_ascending, theme.sort_arrow_color());
            }

            cursor_x += col_w;
        }

        // Column-header bottom divider
        ctx.set_fill_color(theme.divider());
        ctx.fill_rect(
            layout.col_header.x,
            layout.col_header.y + layout.col_header.height - 1.0,
            layout.col_header.width,
            1.0,
        );
    }

    // --- 4. Body closure -----------------------------------------------------
    (view.body)(ctx, layout.body, coord);

    // --- 5. Footer -----------------------------------------------------------
    let has_footer = matches!(
        kind,
        PanelRenderKind::WithFooter | PanelRenderKind::WithHeaderColumnsFooter
    );

    if has_footer && layout.footer.height > 0.0 {
        if let Some(footer_fn) = &mut view.footer {
            // Footer top divider
            ctx.set_fill_color(theme.divider());
            ctx.fill_rect(
                layout.footer.x,
                layout.footer.y,
                layout.footer.width,
                1.0,
            );

            // Footer background
            ctx.set_fill_color(theme.footer_bg());
            ctx.fill_rect(
                layout.footer.x,
                layout.footer.y + 1.0,
                layout.footer.width,
                layout.footer.height - 1.0,
            );

            footer_fn(ctx, layout.footer, coord);
        }
    }

    // --- 6. Scrollbar --------------------------------------------------------
    if view.show_scrollbar && layout.scrollbar.width > 0.0 {
        let scb_state = if state.scroll.is_dragging {
            ScrollbarVisualState::Dragging
        } else {
            ScrollbarVisualState::Active
        };
        draw_scrollbar_standard(
            ctx,
            layout.scrollbar,
            view.content_height,
            layout.body.height,
            state.scroll.offset,
            scb_state,
            None,
        );
    }
}

// ---------------------------------------------------------------------------
// Sort arrow helper
// ---------------------------------------------------------------------------

fn draw_sort_arrow(ctx: &mut dyn RenderContext, x: f64, y: f64, ascending: bool, color: &str) {
    // Draw a small upward (ascending) or downward (descending) caret (4×4 px).
    ctx.set_fill_color(color);
    if ascending {
        // Upward triangle: top point at (x+2, y-3), base at (x, y+1) and (x+4, y+1).
        ctx.fill_rect(x + 1.0, y - 2.0, 2.0, 1.0);
        ctx.fill_rect(x,       y - 1.0, 4.0, 1.0);
        ctx.fill_rect(x,       y,       4.0, 1.0);
    } else {
        // Downward triangle.
        ctx.fill_rect(x,       y - 1.0, 4.0, 1.0);
        ctx.fill_rect(x,       y,       4.0, 1.0);
        ctx.fill_rect(x + 1.0, y + 1.0, 2.0, 1.0);
    }
}

// ---------------------------------------------------------------------------
// Layout computation
// ---------------------------------------------------------------------------

fn compute_layout(
    rect:     Rect,
    view:     &PanelView<'_>,
    settings: &PanelSettings,
    kind:     &PanelRenderKind,
) -> PanelLayout {
    let style = settings.style.as_ref();

    let has_header = matches!(
        kind,
        PanelRenderKind::WithHeader
            | PanelRenderKind::WithHeaderColumns
            | PanelRenderKind::WithFooter
            | PanelRenderKind::WithHeaderColumnsFooter
    );
    let has_col_header = matches!(
        kind,
        PanelRenderKind::WithHeaderColumns | PanelRenderKind::WithHeaderColumnsFooter
    );
    let has_footer = matches!(
        kind,
        PanelRenderKind::WithFooter | PanelRenderKind::WithHeaderColumnsFooter
    );

    let header_h     = if has_header     { style.header_height()        } else { 0.0 };
    let col_header_h = if has_col_header { style.column_header_height() } else { 0.0 };
    let footer_h     = if has_footer && view.footer.is_some() { style.footer_height() } else { 0.0 };
    let scrollbar_w  = if view.show_scrollbar { style.scrollbar_width() } else { 0.0 };

    let frame = rect;

    let header = Rect::new(frame.x, frame.y, frame.width, header_h);

    let col_header = if col_header_h > 0.0 {
        Rect::new(frame.x, frame.y + header_h, frame.width, col_header_h)
    } else {
        Rect::default()
    };

    let body_y = frame.y + header_h + col_header_h;
    let body_h = (frame.height - header_h - col_header_h - footer_h).max(0.0);
    let body_w = frame.width - scrollbar_w;
    let body   = Rect::new(frame.x, body_y, body_w, body_h);

    let scrollbar = if scrollbar_w > 0.0 {
        Rect::new(
            frame.x + frame.width - scrollbar_w,
            body_y,
            scrollbar_w,
            body_h,
        )
    } else {
        Rect::default()
    };

    let footer = if footer_h > 0.0 {
        Rect::new(
            frame.x,
            frame.y + frame.height - footer_h,
            frame.width,
            footer_h,
        )
    } else {
        Rect::default()
    };

    PanelLayout {
        frame,
        header,
        col_header,
        body,
        footer,
        scrollbar,
    }
}
