//! ContextMenu render entry point and layout pipelines.
//!
//! # API
//!
//! - `register_input_coordinator_context_menu` — registers the composite +
//!   child hit-rects with an `InputCoordinator`.  No drawing.
//! - `register_context_manager_context_menu`   — convenience wrapper: registers
//!   and draws in one call using a `ContextManager`.
//!
//! # Draw order
//!
//! 1. Shadow rect
//! 2. Frame background (solid or glass depending on `style.background_fill()`)
//! 3. Frame border
//! 4. Per-item rows (icon, label, separator as needed)

use crate::app_context::ContextManager;
use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::{Rect, WidgetId};

use super::settings::ContextMenuSettings;
use super::state::ContextMenuState;
use super::style::BackgroundFill;
use super::types::{ContextMenuRenderKind, ContextMenuView};

// ---------------------------------------------------------------------------
// Public API — registration only
// ---------------------------------------------------------------------------

/// Register the context menu composite and all child hit-rects with the
/// coordinator.  No drawing occurs.
///
/// Returns the `WidgetId` assigned to the context menu composite.
pub fn register_input_coordinator_context_menu(
    coord:    &mut InputCoordinator,
    id:       impl Into<WidgetId>,
    state:    &ContextMenuState,
    view:     &ContextMenuView<'_>,
    settings: &ContextMenuSettings,
    kind:     &ContextMenuRenderKind<'_>,
    layer:    &LayerId,
) -> WidgetId {
    let style  = settings.style.as_ref();
    let menu_w = style.min_width();
    let menu_h = compute_menu_height(view, settings, kind);
    let frame  = Rect::new(state.x, state.y, menu_w, menu_h);

    let cm_id = coord.register_composite(
        id,
        WidgetKind::ContextMenu,
        frame,
        Sense::CLICK,
        layer,
    );

    if !state.is_open {
        return cm_id;
    }

    let content = content_rect(frame, settings);
    register_item_hits(coord, &cm_id, content, view, settings, kind);

    cm_id
}

// ---------------------------------------------------------------------------
// Public API — ContextManager convenience
// ---------------------------------------------------------------------------

/// Register + draw a context menu in one call using a `ContextManager`.
///
/// Returns the `WidgetId` assigned to the context menu composite.
pub fn register_context_manager_context_menu(
    ctx_mgr:  &mut ContextManager,
    render:   &mut dyn RenderContext,
    id:       impl Into<WidgetId>,
    state:    &mut ContextMenuState,
    view:     &mut ContextMenuView<'_>,
    settings: &ContextMenuSettings,
    kind:     &ContextMenuRenderKind<'_>,
    layer:    &LayerId,
) -> WidgetId {
    let coord = &mut ctx_mgr.input;
    let cm_id = register_input_coordinator_context_menu(
        coord, id, state, view, settings, kind, layer,
    );

    if state.is_open {
        draw_context_menu_panel(render, state, view, settings, kind);
    }

    cm_id
}

// ---------------------------------------------------------------------------
// Internal draw pipeline
// ---------------------------------------------------------------------------

fn draw_context_menu_panel(
    ctx:      &mut dyn RenderContext,
    state:    &ContextMenuState,
    view:     &ContextMenuView<'_>,
    settings: &ContextMenuSettings,
    kind:     &ContextMenuRenderKind<'_>,
) {
    if let ContextMenuRenderKind::Custom(draw) = kind {
        let style  = settings.style.as_ref();
        let menu_w = style.min_width();
        let menu_h = compute_menu_height(view, settings, kind);
        let frame  = Rect::new(state.x, state.y, menu_w, menu_h);
        draw(ctx, frame, state, view, settings);
        return;
    }

    let style  = settings.style.as_ref();
    let menu_w = style.min_width();
    let menu_h = compute_menu_height(view, settings, kind);
    let frame  = Rect::new(state.x, state.y, menu_w, menu_h);

    draw_frame(ctx, frame, settings);

    let content = content_rect(frame, settings);
    draw_items(ctx, content, state, view, settings, kind);
}

// ---------------------------------------------------------------------------
// Frame draw
// ---------------------------------------------------------------------------

fn draw_frame(ctx: &mut dyn RenderContext, frame: Rect, settings: &ContextMenuSettings) {
    let theme = settings.theme.as_ref();
    let style = settings.style.as_ref();

    // Shadow rect
    let (sx, sy) = style.shadow_offset();
    ctx.set_fill_color(theme.shadow());
    ctx.fill_rounded_rect(
        frame.x + sx,
        frame.y + sy,
        frame.width,
        frame.height,
        style.radius(),
    );

    // Background
    match style.background_fill() {
        BackgroundFill::Solid | BackgroundFill::Texture { .. } => {
            ctx.set_fill_color(theme.bg());
            ctx.fill_rounded_rect(frame.x, frame.y, frame.width, frame.height, style.radius());
        }
        BackgroundFill::Glass { .. } => {
            // Backends that support blur will apply it; others fall back to solid.
            ctx.draw_blur_background(frame.x, frame.y, frame.width, frame.height);
            ctx.set_fill_color(theme.bg());
            ctx.fill_rounded_rect(frame.x, frame.y, frame.width, frame.height, style.radius());
        }
    }

    // Border
    ctx.set_stroke_color(theme.border());
    ctx.set_stroke_width(style.border_width());
    ctx.set_line_dash(&[]);
    ctx.stroke_rounded_rect(frame.x, frame.y, frame.width, frame.height, style.radius());
}

// ---------------------------------------------------------------------------
// Item draw
// ---------------------------------------------------------------------------

fn draw_items(
    ctx:      &mut dyn RenderContext,
    content:  Rect,
    state:    &ContextMenuState,
    view:     &ContextMenuView<'_>,
    settings: &ContextMenuSettings,
    kind:     &ContextMenuRenderKind<'_>,
) {
    let theme  = settings.theme.as_ref();
    let style  = settings.style.as_ref();
    let show_icons = matches!(kind, ContextMenuRenderKind::Default);

    // Optional title row
    if let Some(title) = view.title {
        let h = style.item_height();
        ctx.set_fill_color(theme.item_text_disabled());
        ctx.set_font(&format!("bold {}px sans-serif", style.font_size()));
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Middle);
        ctx.fill_text(title, content.x + style.item_padding_x(), content.y + h / 2.0);
        // separator under title
        ctx.set_fill_color(theme.separator());
        ctx.fill_rect(content.x, content.y + h - 1.0, content.width, 1.0);
    }

    let title_h = view.title.map(|_| style.item_height()).unwrap_or(0.0);
    let mut cursor_y = content.y + title_h;

    for (idx, item) in view.items.iter().enumerate() {
        let h       = style.item_height();
        let hovered = state.hovered_index == Some(idx);

        // --- Item background ---
        if item.enabled {
            let bg = if hovered {
                if item.danger {
                    theme.item_bg_danger_hover()
                } else {
                    theme.item_bg_hover()
                }
            } else {
                theme.item_bg_normal()
            };
            ctx.set_fill_color(bg);
            ctx.fill_rounded_rect(
                content.x + 2.0,
                cursor_y,
                content.width - 4.0,
                h,
                style.item_hover_radius(),
            );
        }

        // --- Icon (Default only) ---
        let text_x = if show_icons {
            let icon_x = content.x + style.item_padding_x();
            if let Some(icon_str) = item.icon {
                let icon_y = cursor_y + (h - style.icon_size()) / 2.0;
                let icon_color = if !item.enabled {
                    theme.item_text_disabled()
                } else {
                    theme.item_text()
                };
                ctx.set_fill_color(icon_color);
                ctx.set_font(&format!("{}px sans-serif", style.icon_size()));
                ctx.set_text_align(TextAlign::Left);
                ctx.set_text_baseline(TextBaseline::Top);
                ctx.fill_text(icon_str, icon_x, icon_y);
            }
            icon_x + style.icon_size() + style.icon_text_gap()
        } else {
            content.x + style.item_padding_x()
        };

        // --- Label ---
        let text_color = if !item.enabled {
            theme.item_text_disabled()
        } else if item.danger {
            theme.item_text_danger()
        } else if hovered {
            theme.item_text_hover()
        } else {
            theme.item_text()
        };
        ctx.set_fill_color(text_color);
        ctx.set_font(&format!("{}px sans-serif", style.font_size()));
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Middle);
        ctx.fill_text(item.label, text_x, cursor_y + h / 2.0);

        cursor_y += h;

        // --- Separator after (Default only) ---
        if item.separator_after && matches!(kind, ContextMenuRenderKind::Default) {
            let sep_h = style.separator_height();
            ctx.set_fill_color(theme.separator());
            ctx.fill_rect(
                content.x + style.item_padding_x(),
                cursor_y + sep_h / 2.0 - 0.5,
                content.width - style.item_padding_x() * 2.0,
                1.0,
            );
            cursor_y += sep_h;
        }
    }
}

// ---------------------------------------------------------------------------
// Hit-rect registration
// ---------------------------------------------------------------------------

fn register_item_hits(
    coord:    &mut InputCoordinator,
    parent:   &WidgetId,
    content:  Rect,
    view:     &ContextMenuView<'_>,
    settings: &ContextMenuSettings,
    kind:     &ContextMenuRenderKind<'_>,
) {
    if matches!(kind, ContextMenuRenderKind::Custom(_)) {
        return;
    }

    let style   = settings.style.as_ref();
    let title_h = view.title.map(|_| style.item_height()).unwrap_or(0.0);
    let mut cursor_y = content.y + title_h;

    for (idx, item) in view.items.iter().enumerate() {
        let h = style.item_height();

        if item.enabled {
            coord.register_child(
                parent,
                format!("{}:item:{}", parent.0, idx),
                WidgetKind::Button,
                Rect::new(content.x, cursor_y, content.width, h),
                Sense::CLICK | Sense::HOVER,
            );
        }
        cursor_y += h;

        if item.separator_after && matches!(kind, ContextMenuRenderKind::Default) {
            cursor_y += style.separator_height();
        }
    }
}

// ---------------------------------------------------------------------------
// Layout helpers
// ---------------------------------------------------------------------------

fn content_rect(frame: Rect, settings: &ContextMenuSettings) -> Rect {
    let pad = settings.style.padding();
    Rect::new(
        frame.x + pad,
        frame.y + pad,
        (frame.width - pad * 2.0).max(0.0),
        (frame.height - pad * 2.0).max(0.0),
    )
}

/// Compute the total menu panel height from items + optional title.
fn compute_menu_height(
    view:     &ContextMenuView<'_>,
    settings: &ContextMenuSettings,
    kind:     &ContextMenuRenderKind<'_>,
) -> f64 {
    let style  = settings.style.as_ref();
    let pad    = style.padding();
    let item_h = style.item_height();
    let sep_h  = style.separator_height();

    let title_h = view.title.map(|_| item_h).unwrap_or(0.0);

    let items_h: f64 = view.items.iter().map(|item| {
        let row = item_h;
        let sep = if item.separator_after && matches!(kind, ContextMenuRenderKind::Default) {
            sep_h
        } else {
            0.0
        };
        row + sep
    }).sum();

    pad * 2.0 + title_h + items_h
}
