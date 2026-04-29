//! Sidebar render entry point and per-kind layout pipelines.
//!
//! # API convention
//!
//! - `register_input_coordinator_sidebar` — registers the composite + all child
//!   hit-rects with an `InputCoordinator`.  **No drawing.**
//! - `register_context_manager_sidebar`   — convenience wrapper: takes a
//!   `ContextManager`, registers, and draws in one call.
//!
//! # Draw order (non-Custom kinds)
//!
//! 1. Background + border
//! 2. Header strip (icon + title + action buttons)
//! 3. Header bottom divider
//! 4. Tab strip (WithTypeSelector only)
//! 5. Body closure `(view.body)(ctx, body_rect, coord)`
//! 6. Scrollbar (if `view.show_scrollbar`)
//! 7. Resize edge handle (Right/Left/WithTypeSelector — sidebar variant stroke)

use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::{Rect, WidgetId};
use crate::ui::widgets::atomic::scrollbar::render::{
    draw_scrollbar_standard, ScrollbarVisualState,
};
use crate::ui::widgets::atomic::separator::render::draw_sidebar_handle;
use crate::ui::widgets::atomic::separator::settings::SeparatorSettings;

use super::settings::SidebarSettings;
use super::state::SidebarState;
use super::style::BackgroundFill;
use super::types::{SidebarRenderKind, SidebarView};

// ---------------------------------------------------------------------------
// Internal layout struct
// ---------------------------------------------------------------------------

struct SidebarLayout {
    /// Full frame rect (same as outer `rect`).
    frame: Rect,
    /// Header strip (40 px fixed).
    header: Rect,
    /// Tab strip (WithTypeSelector only; zero height otherwise).
    tab_strip: Rect,
    /// Body area available to the body closure.
    body: Rect,
    /// Scrollbar column (zero width if scrollbar disabled).
    scrollbar: Rect,
    /// Resize edge hit zone (zero width for Embedded / Custom).
    resize_zone: Rect,
    /// 1 px border line rect.
    border_line: Rect,
}

// ---------------------------------------------------------------------------
// Public API — register (InputCoordinator)
// ---------------------------------------------------------------------------

/// Register the sidebar composite and all its child hit-rects.
///
/// **No drawing happens here.**  Use when you need explicit z-order control.
///
/// Returns the `WidgetId` assigned to the sidebar composite.
pub fn register_input_coordinator_sidebar(
    coord:    &mut InputCoordinator,
    id:       impl Into<WidgetId>,
    rect:     Rect,
    state:    &SidebarState,
    view:     &SidebarView<'_>,
    settings: &SidebarSettings,
    kind:     &SidebarRenderKind,
    layer:    &LayerId,
) -> WidgetId {
    let sidebar_id = coord.register_composite(id, WidgetKind::Sidebar, rect, Sense::CLICK, layer);

    if let SidebarRenderKind::Custom(_) = kind {
        return sidebar_id;
    }

    let layout = compute_layout(rect, state, view, settings, kind);

    // --- Header action buttons ---
    for (i, action) in view.header.actions.iter().enumerate() {
        let btn_size  = 24.0_f64;
        let btn_gap   = 8.0_f64;
        let pad_right = 12.0_f64;
        let btn_x = layout.header.x + layout.header.width
            - pad_right
            - btn_size * (i + 1) as f64
            - btn_gap * i as f64;
        let btn_y = layout.header.y + (layout.header.height - btn_size) / 2.0;
        coord.register_child(
            &sidebar_id,
            format!("{}:action:{}", sidebar_id.0, action.id),
            WidgetKind::Button,
            Rect::new(btn_x, btn_y, btn_size, btn_size),
            Sense::CLICK | Sense::HOVER,
        );
    }

    // --- Tab strip (WithTypeSelector) ---
    if let SidebarRenderKind::WithTypeSelector = kind {
        let tab_count = view.tabs.len();
        if tab_count > 0 && layout.tab_strip.height > 0.0 {
            let tab_w = layout.tab_strip.width / tab_count as f64;
            for i in 0..tab_count {
                let tab_rect = Rect::new(
                    layout.tab_strip.x + i as f64 * tab_w,
                    layout.tab_strip.y,
                    tab_w,
                    layout.tab_strip.height,
                );
                coord.register_child(
                    &sidebar_id,
                    format!("{}:tab:{}", sidebar_id.0, i),
                    WidgetKind::Button,
                    tab_rect,
                    Sense::CLICK | Sense::HOVER,
                );
            }
        }
    }

    // --- Resize zone ---
    if layout.resize_zone.width > 0.0 && layout.resize_zone.height > 0.0 {
        coord.register_child(
            &sidebar_id,
            format!("{}:resize", sidebar_id.0),
            WidgetKind::Separator,
            layout.resize_zone,
            Sense::DRAG,
        );
    }

    // --- Scrollbar handle + track ---
    if view.show_scrollbar && layout.scrollbar.width > 0.0 {
        // Inflated hit zone: ±5 px on x-axis (matches mlc scrollbar_handle inflation).
        let scroll = state
            .scroll_per_panel
            .get(view.active_tab.unwrap_or("default"));
        let scroll_offset = scroll.map(|s| s.offset).unwrap_or(0.0);
        let viewport_h = layout.body.height;
        let content_h  = view.content_height;

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
                &sidebar_id,
                format!("{}:scrollbar_handle", sidebar_id.0),
                WidgetKind::ScrollbarHandle,
                inflated,
                Sense::DRAG,
            );
            coord.register_child(
                &sidebar_id,
                format!("{}:scrollbar_track", sidebar_id.0),
                WidgetKind::ScrollbarTrack,
                layout.scrollbar,
                Sense::CLICK,
            );
        }
    }

    // --- Viewport scroll zone ---
    coord.register_child(
        &sidebar_id,
        format!("{}:viewport", sidebar_id.0),
        WidgetKind::Custom,
        layout.body,
        Sense::SCROLL,
    );

    sidebar_id
}

// ---------------------------------------------------------------------------
// Public API — convenience wrapper (ContextManager)
// ---------------------------------------------------------------------------

/// Register + draw a sidebar in one call using a `ContextManager`.
///
/// # Arguments
/// - `ctx_mgr`  — context manager (`coord` extracted as `&mut ctx_mgr.input`).
/// - `render`   — render context.
/// - `id`       — stable widget id.
/// - `rect`     — bounding rect (full sidebar area).
/// - `state`    — mutable sidebar state.
/// - `view`     — per-frame data (header, body closure, etc.).
/// - `settings` — theme + style configuration.
/// - `kind`     — selects the layout pipeline.
/// - `layer`    — coordinator layer.
///
/// Returns the `WidgetId` assigned to the sidebar composite.
pub fn register_context_manager_sidebar(
    ctx_mgr:  &mut crate::app_context::ContextManager,
    render:   &mut dyn RenderContext,
    id:       impl Into<WidgetId>,
    rect:     Rect,
    state:    &mut SidebarState,
    view:     &mut SidebarView<'_>,
    settings: &SidebarSettings,
    kind:     &SidebarRenderKind,
    layer:    &LayerId,
) -> WidgetId {
    let coord = &mut ctx_mgr.input;
    let sidebar_id =
        register_input_coordinator_sidebar(coord, id, rect, state, view, settings, kind, layer);
    draw_sidebar_with_coord(render, rect, coord, state, view, settings, kind);
    sidebar_id
}

// ---------------------------------------------------------------------------
// Internal draw pipeline
// ---------------------------------------------------------------------------

fn draw_sidebar_with_coord(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    coord:    &mut InputCoordinator,
    state:    &mut SidebarState,
    view:     &mut SidebarView<'_>,
    settings: &SidebarSettings,
    kind:     &SidebarRenderKind,
) {
    if let SidebarRenderKind::Custom(f) = kind {
        f(ctx, rect, view, settings);
        return;
    }

    let theme  = settings.theme.as_ref();
    let style  = settings.style.as_ref();
    let layout = compute_layout(rect, state, view, settings, kind);

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

    // --- 2. Border line (sidebar separator) ----------------------------------
    if layout.border_line.width > 0.0 {
        let sep_settings = SeparatorSettings::default();
        match kind {
            SidebarRenderKind::Left => {
                // Right edge: stroke at border_line.x + border_line.width - 0.5
                draw_sidebar_handle(
                    ctx,
                    layout.border_line.x + layout.border_line.width,
                    layout.border_line.y,
                    layout.border_line.height,
                    &sep_settings,
                );
            }
            _ => {
                // Right / WithTypeSelector: left edge
                draw_sidebar_handle(
                    ctx,
                    layout.border_line.x,
                    layout.border_line.y,
                    layout.border_line.height,
                    &sep_settings,
                );
            }
        }
    }

    // --- 3. Header background + icon + title ---------------------------------
    ctx.set_fill_color(theme.header_bg());
    ctx.fill_rect(layout.header.x, layout.header.y, layout.header.width, layout.header.height);

    let pad       = style.padding();
    let icon_size = 18.0_f64;
    let icon_pad  = 12.0_f64;
    let title_off = 36.0_f64; // icon_pad + icon_size + gap (≈6 px)

    // Icon
    if view.header.icon.is_some() {
        // Icon rendering delegated to caller via draw_icon callback.
        // Here we just reserve the rect; the caller's body closure can draw it.
        // Rendering inline as a colored rect placeholder at icon position:
        let icon_rect = Rect::new(
            layout.header.x + icon_pad,
            layout.header.y + (layout.header.height - icon_size) / 2.0,
            icon_size,
            icon_size,
        );
        let _ = icon_rect; // icon_rect registered visually; actual draw is caller's job
    }

    // Title
    let title_x = if view.header.icon.is_some() {
        layout.header.x + title_off
    } else {
        layout.header.x + pad
    };
    ctx.set_fill_color(theme.header_text());
    ctx.set_font("13px sans-serif");
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(
        view.header.title,
        title_x,
        layout.header.y + layout.header.height / 2.0,
    );

    // Action buttons (icon-only, right-to-left)
    let btn_size  = 24.0_f64;
    let btn_gap   = 8.0_f64;
    let pad_right = 12.0_f64;
    for (i, action) in view.header.actions.iter().enumerate() {
        let btn_x = layout.header.x + layout.header.width
            - pad_right
            - btn_size * (i + 1) as f64
            - btn_gap * i as f64;
        let btn_y = layout.header.y + (layout.header.height - btn_size) / 2.0;
        let hovered = state.header_action_hovered.as_deref() == Some(action.id);

        if hovered {
            ctx.set_fill_color("rgba(255,255,255,0.08)");
            ctx.fill_rounded_rect(btn_x, btn_y, btn_size, btn_size, 4.0);
        }

        let icon_color = if hovered {
            theme.action_icon_hover()
        } else {
            theme.action_icon_normal()
        };
        // Icon drawn as a placeholder colored square for now;
        // actual SVG drawing uses caller-provided callback.
        ctx.set_fill_color(icon_color);
        let icon_inner = 16.0_f64;
        let ix = btn_x + (btn_size - icon_inner) / 2.0;
        let iy = btn_y + (btn_size - icon_inner) / 2.0;
        let _ = (ix, iy); // icon draw point; real SVG render by caller
    }

    // --- 4. Header bottom divider --------------------------------------------
    ctx.set_fill_color(theme.divider());
    ctx.fill_rect(
        layout.header.x,
        layout.header.y + layout.header.height - 1.0,
        layout.header.width,
        1.0,
    );

    // --- 5. Tab strip (WithTypeSelector) -------------------------------------
    if let SidebarRenderKind::WithTypeSelector = kind {
        draw_tab_strip(ctx, &layout, view, state, settings);
    }

    // --- 6. Body closure -----------------------------------------------------
    (view.body)(ctx, layout.body, coord);

    // --- 7. Scrollbar --------------------------------------------------------
    if view.show_scrollbar && layout.scrollbar.width > 0.0 {
        let panel_key = view.active_tab.unwrap_or("default");
        let scroll_offset = state
            .scroll_per_panel
            .get(panel_key)
            .map(|s| s.offset)
            .unwrap_or(0.0);
        let scb_state = if state.scroll_per_panel
            .get(panel_key)
            .map(|s| s.is_dragging)
            .unwrap_or(false)
        {
            ScrollbarVisualState::Dragging
        } else {
            ScrollbarVisualState::Active
        };
        draw_scrollbar_standard(
            ctx,
            layout.scrollbar,
            view.content_height,
            layout.body.height,
            scroll_offset,
            scb_state,
            None,
        );
    }

    // --- 8. Resize edge visual (Right / Left / WithTypeSelector) -------------
    let shows_resize = matches!(
        kind,
        SidebarRenderKind::Right
            | SidebarRenderKind::Left
            | SidebarRenderKind::WithTypeSelector
    );
    if shows_resize && layout.resize_zone.width > 0.0 {
        // The visual is handled by draw_sidebar_handle (already drawn in step 2).
        // Resize zone only provides the interactive hit rect; no extra draw needed.
    }
}

// ---------------------------------------------------------------------------
// Tab strip drawing helper
// ---------------------------------------------------------------------------

fn draw_tab_strip(
    ctx:      &mut dyn RenderContext,
    layout:   &SidebarLayout,
    view:     &SidebarView<'_>,
    state:    &SidebarState,
    settings: &SidebarSettings,
) {
    let theme     = settings.theme.as_ref();
    let tab_count = view.tabs.len();
    if tab_count == 0 || layout.tab_strip.height <= 0.0 {
        return;
    }

    let tab_w = layout.tab_strip.width / tab_count as f64;

    // Tab strip background (same as sidebar bg — no separate fill needed)
    // Draw each tab
    for (i, tab) in view.tabs.iter().enumerate() {
        let tab_rect = Rect::new(
            layout.tab_strip.x + i as f64 * tab_w,
            layout.tab_strip.y,
            tab_w,
            layout.tab_strip.height,
        );

        let is_active = view.active_tab == Some(tab.id)
            || state.active_tab.as_deref() == Some(tab.id);

        // Background
        if is_active {
            ctx.set_fill_color(theme.tab_bg_active());
            ctx.fill_rect(tab_rect.x, tab_rect.y, tab_rect.width, tab_rect.height);
        }

        // Label
        let text_color = if is_active {
            theme.tab_text_active()
        } else {
            theme.tab_text_inactive()
        };
        ctx.set_fill_color(text_color);
        ctx.set_font("12px sans-serif");
        ctx.set_text_align(TextAlign::Center);
        ctx.set_text_baseline(TextBaseline::Middle);
        ctx.fill_text(
            tab.label,
            tab_rect.x + tab_rect.width / 2.0,
            tab_rect.y + tab_rect.height / 2.0,
        );

        // Active underline (2 px bottom accent)
        if is_active {
            ctx.set_fill_color(theme.tab_accent());
            ctx.fill_rect(
                tab_rect.x,
                tab_rect.y + tab_rect.height - 2.0,
                tab_rect.width,
                2.0,
            );
        }
    }

    // Tab strip bottom border
    ctx.set_fill_color(settings.theme.divider());
    ctx.fill_rect(
        layout.tab_strip.x,
        layout.tab_strip.y + layout.tab_strip.height - 1.0,
        layout.tab_strip.width,
        1.0,
    );
}

// ---------------------------------------------------------------------------
// Layout computation
// ---------------------------------------------------------------------------

fn compute_layout(
    rect:     Rect,
    _state:   &SidebarState,
    view:     &SidebarView<'_>,
    settings: &SidebarSettings,
    kind:     &SidebarRenderKind,
) -> SidebarLayout {
    let style = settings.style.as_ref();

    let header_h     = style.header_height();
    let tab_strip_h  = match kind {
        SidebarRenderKind::WithTypeSelector => style.tab_strip_height(),
        _ => 0.0,
    };
    let scrollbar_w  = if view.show_scrollbar { style.scrollbar_width() } else { 0.0 };
    let resize_w     = match kind {
        SidebarRenderKind::Embedded | SidebarRenderKind::Custom(_) => 0.0,
        _ => style.resize_zone_width(),
    };
    let border_w     = style.border_width();

    let frame = rect;

    let header = Rect::new(frame.x, frame.y, frame.width, header_h);

    let tab_strip = if tab_strip_h > 0.0 {
        Rect::new(frame.x, frame.y + header_h, frame.width, tab_strip_h)
    } else {
        Rect::default()
    };

    // Body starts after header + tab strip; right edge reserved for scrollbar
    let body_y = frame.y + header_h + tab_strip_h;
    let body_h = (frame.height - header_h - tab_strip_h).max(0.0);
    let body_w = frame.width - scrollbar_w;
    let body   = Rect::new(frame.x, body_y, body_w, body_h);

    let scrollbar = if scrollbar_w > 0.0 {
        Rect::new(frame.x + frame.width - scrollbar_w, body_y, scrollbar_w, body_h)
    } else {
        Rect::default()
    };

    // Resize zone: centered on the relevant border edge
    let resize_zone = if resize_w > 0.0 {
        match kind {
            SidebarRenderKind::Left => {
                // Right edge of a left sidebar
                Rect::new(
                    frame.x + frame.width - resize_w / 2.0,
                    frame.y,
                    resize_w,
                    frame.height,
                )
            }
            _ => {
                // Left edge of a right sidebar
                Rect::new(
                    frame.x - resize_w / 2.0,
                    frame.y,
                    resize_w,
                    frame.height,
                )
            }
        }
    } else {
        Rect::default()
    };

    // Border line (1 px visual)
    let border_line = if border_w > 0.0 {
        match kind {
            SidebarRenderKind::Left => {
                Rect::new(frame.x + frame.width - border_w, frame.y, border_w, frame.height)
            }
            _ => {
                Rect::new(frame.x, frame.y, border_w, frame.height)
            }
        }
    } else {
        Rect::default()
    };

    SidebarLayout {
        frame,
        header,
        tab_strip,
        body,
        scrollbar,
        resize_zone,
        border_line,
    }
}
