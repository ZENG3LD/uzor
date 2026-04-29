//! Toolbar render entry point and per-kind layout pipelines.
//!
//! # API convention
//!
//! - `register_input_coordinator_toolbar` — registers the composite and all
//!   child hit-rects with an `InputCoordinator`.  **No drawing.**
//! - `register_context_manager_toolbar`   — convenience wrapper: registers
//!   and draws in one call via a `ContextManager`.
//!
//! # Draw order (all non-Custom kinds)
//!
//! 1. Toolbar background (BackgroundFill dispatch)
//! 2. Start section items (left / top)
//! 3. Center section items (if non-empty)
//! 4. End section items (right / bottom)
//! 5. Overflow chevrons (if items overflow the toolbar width)

use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::{Rect, WidgetId};

use super::settings::ToolbarSettings;
use super::state::ToolbarState;
use super::style::BackgroundFill;
use super::types::{ToolbarItem, ToolbarRenderKind, ToolbarSection, ToolbarView};

// ---------------------------------------------------------------------------
// Public API — InputCoordinator
// ---------------------------------------------------------------------------

/// Register the toolbar composite and all child item hit-rects with the
/// coordinator.  **No drawing happens here.**
///
/// Returns the `WidgetId` assigned to the toolbar composite.
pub fn register_input_coordinator_toolbar(
    coord:    &mut InputCoordinator,
    id:       impl Into<WidgetId>,
    rect:     Rect,
    state:    &ToolbarState,
    view:     &ToolbarView<'_>,
    settings: &ToolbarSettings,
    kind:     &ToolbarRenderKind,
    layer:    &LayerId,
) -> WidgetId {
    let toolbar_id = coord.register_composite(id, WidgetKind::Toolbar, rect, Sense::CLICK, layer);

    match kind {
        ToolbarRenderKind::Custom(_) => return toolbar_id,
        ToolbarRenderKind::ChromeStrip => {
            register_chrome_items(coord, &toolbar_id, rect, view, settings, state);
        }
        ToolbarRenderKind::Horizontal | ToolbarRenderKind::Inline => {
            register_horizontal_items(coord, &toolbar_id, rect, view, settings, state);
        }
        ToolbarRenderKind::Vertical => {
            register_vertical_items(coord, &toolbar_id, rect, view, settings, state);
        }
    }

    toolbar_id
}

// ---------------------------------------------------------------------------
// Public API — ContextManager convenience wrapper
// ---------------------------------------------------------------------------

/// Register + draw a toolbar in one call using a `ContextManager`.
///
/// This is the recommended entry point for typical use.
pub fn register_context_manager_toolbar(
    ctx_mgr:  &mut crate::app_context::ContextManager,
    render:   &mut dyn RenderContext,
    id:       impl Into<WidgetId>,
    rect:     Rect,
    state:    &mut ToolbarState,
    view:     &ToolbarView<'_>,
    settings: &ToolbarSettings,
    kind:     &ToolbarRenderKind,
    layer:    &LayerId,
) -> WidgetId {
    let coord = &mut ctx_mgr.input;
    let toolbar_id = register_input_coordinator_toolbar(coord, id, rect, state, view, settings, kind, layer);
    draw_toolbar_internal(render, rect, state, view, settings, kind);
    toolbar_id
}

// ---------------------------------------------------------------------------
// Internal draw dispatcher
// ---------------------------------------------------------------------------

fn draw_toolbar_internal(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    state:    &ToolbarState,
    view:     &ToolbarView<'_>,
    settings: &ToolbarSettings,
    kind:     &ToolbarRenderKind,
) {
    match kind {
        ToolbarRenderKind::Custom(f) => {
            f(ctx, rect, state, settings);
            return;
        }
        ToolbarRenderKind::ChromeStrip => {
            draw_background(ctx, rect, settings);
            draw_chrome_strip(ctx, rect, view, settings, state);
            return;
        }
        _ => {}
    }

    // 1. Background
    draw_background(ctx, rect, settings);

    // 2–4. Sections
    let is_vertical = matches!(kind, ToolbarRenderKind::Vertical);
    if is_vertical {
        draw_sections_vertical(ctx, rect, view, settings, state);
    } else {
        draw_sections_horizontal(ctx, rect, view, settings, state);
    }
}

// ---------------------------------------------------------------------------
// Background
// ---------------------------------------------------------------------------

fn draw_background(ctx: &mut dyn RenderContext, rect: Rect, settings: &ToolbarSettings) {
    let theme = settings.theme.as_ref();
    let style = settings.style.as_ref();

    match style.background_fill() {
        BackgroundFill::Transparent => {}
        BackgroundFill::Solid => {
            ctx.set_fill_color(theme.bg());
            ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);
        }
        BackgroundFill::Glass { blur_radius: _ } => {
            ctx.draw_blur_background(rect.x, rect.y, rect.width, rect.height);
            ctx.set_fill_color(theme.bg());
            ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);
        }
        BackgroundFill::Texture { asset_id } => {
            let _ = asset_id;
            ctx.set_fill_color(theme.bg());
            ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);
        }
    }
}

// ---------------------------------------------------------------------------
// Horizontal layout helpers
// ---------------------------------------------------------------------------

/// Returns per-item rects for a section laid out left→right starting at `start_x`.
fn layout_section_horizontal(
    section: &ToolbarSection<'_>,
    start_x: f64,
    bar_y:   f64,
    bar_h:   f64,
    settings: &ToolbarSettings,
) -> Vec<Rect> {
    let style    = settings.style.as_ref();
    let item_sz  = style.item_size();
    let spacing  = style.item_spacing();
    let sep_pad  = style.separator_padding();
    let sep_th   = style.separator_thickness();

    let mut rects = Vec::with_capacity(section.items.len());
    let mut x = start_x;
    let item_y = bar_y + (bar_h - item_sz) / 2.0;

    for item in section.items.iter() {
        match item {
            ToolbarItem::Separator => {
                // Separator consumes: sep_pad + sep_th + sep_pad
                rects.push(Rect::new(x, bar_y, sep_pad * 2.0 + sep_th, bar_h));
                x += sep_pad * 2.0 + sep_th + spacing;
            }
            ToolbarItem::Spacer { width } => {
                rects.push(Rect::new(x, item_y, *width, item_sz));
                x += width + spacing;
            }
            ToolbarItem::Clock { .. } | ToolbarItem::Label { .. } => {
                // Give labels/clock a wider slot
                let w = item_sz * 2.5;
                rects.push(Rect::new(x, item_y, w, item_sz));
                x += w + spacing;
            }
            ToolbarItem::SplitIconButton { .. } | ToolbarItem::SplitLineWidthButton { .. } => {
                // Main area + chevron
                let split_w = item_sz + style.split_chevron_width();
                rects.push(Rect::new(x, item_y, split_w, item_sz));
                x += split_w + spacing;
            }
            ToolbarItem::Dropdown { .. } => {
                let w = item_sz * 3.0;
                rects.push(Rect::new(x, item_y, w, item_sz));
                x += w + spacing;
            }
            ToolbarItem::IconTextButton { .. } | ToolbarItem::TextButton { .. } => {
                let w = item_sz * 2.5;
                rects.push(Rect::new(x, item_y, w, item_sz));
                x += w + spacing;
            }
            _ => {
                rects.push(Rect::new(x, item_y, item_sz, item_sz));
                x += item_sz + spacing;
            }
        }
    }
    rects
}

/// Total width consumed by a section (including inter-item spacing).
fn section_width_horizontal(section: &ToolbarSection<'_>, settings: &ToolbarSettings) -> f64 {
    let rects = layout_section_horizontal(section, 0.0, 0.0, 0.0, settings);
    rects.last().map(|r| r.x + r.width).unwrap_or(0.0)
}

fn draw_sections_horizontal(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    view:     &ToolbarView<'_>,
    settings: &ToolbarSettings,
    state:    &ToolbarState,
) {
    let style    = settings.style.as_ref();
    let padding  = style.padding();
    let sec_gap  = style.section_gap();

    let start_w  = section_width_horizontal(&view.start, settings);
    let center_w = section_width_horizontal(&view.center, settings);
    let end_w    = section_width_horizontal(&view.end, settings);

    // Start: left-aligned
    let start_x = rect.x + padding - state.scroll_offset;
    let start_rects = layout_section_horizontal(&view.start, start_x, rect.y, rect.height, settings);
    draw_section_items_horizontal(ctx, &view.start, &start_rects, settings, state);

    // Center: centered in the bar
    if !view.center.is_empty() {
        let center_x = rect.x + (rect.width - center_w) / 2.0;
        let center_rects = layout_section_horizontal(&view.center, center_x, rect.y, rect.height, settings);
        draw_section_items_horizontal(ctx, &view.center, &center_rects, settings, state);
    }

    // End: right-aligned
    if !view.end.is_empty() {
        let end_x = rect.x + rect.width - end_w - padding;
        let end_rects = layout_section_horizontal(&view.end, end_x, rect.y, rect.height, settings);
        draw_section_items_horizontal(ctx, &view.end, &end_rects, settings, state);
    }

    // Suppress unused warning for sec_gap (used in logical positioning, not yet active)
    let _ = (start_w, sec_gap);
}

fn draw_section_items_horizontal(
    ctx:      &mut dyn RenderContext,
    section:  &ToolbarSection<'_>,
    rects:    &[Rect],
    settings: &ToolbarSettings,
    state:    &ToolbarState,
) {
    for (item, &item_rect) in section.items.iter().zip(rects.iter()) {
        draw_item(ctx, item, item_rect, settings, state);
    }
}

// ---------------------------------------------------------------------------
// Vertical layout helpers
// ---------------------------------------------------------------------------

fn layout_section_vertical(
    section: &ToolbarSection<'_>,
    bar_x:   f64,
    start_y: f64,
    bar_w:   f64,
    settings: &ToolbarSettings,
) -> Vec<Rect> {
    let style    = settings.style.as_ref();
    let item_sz  = style.item_size();
    let spacing  = style.item_spacing();
    let sep_pad  = style.separator_padding();
    let sep_th   = style.separator_thickness();

    let mut rects = Vec::with_capacity(section.items.len());
    let mut y     = start_y;
    let item_x    = bar_x + (bar_w - item_sz) / 2.0;

    for item in section.items.iter() {
        match item {
            ToolbarItem::Separator => {
                rects.push(Rect::new(bar_x, y, bar_w, sep_pad * 2.0 + sep_th));
                y += sep_pad * 2.0 + sep_th + spacing;
            }
            ToolbarItem::Spacer { width } => {
                rects.push(Rect::new(item_x, y, item_sz, *width));
                y += width + spacing;
            }
            _ => {
                rects.push(Rect::new(item_x, y, item_sz, item_sz));
                y += item_sz + spacing;
            }
        }
    }
    rects
}

fn section_height_vertical(section: &ToolbarSection<'_>, settings: &ToolbarSettings) -> f64 {
    let rects = layout_section_vertical(section, 0.0, 0.0, 0.0, settings);
    rects.last().map(|r| r.y + r.height).unwrap_or(0.0)
}

fn draw_sections_vertical(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    view:     &ToolbarView<'_>,
    settings: &ToolbarSettings,
    state:    &ToolbarState,
) {
    let style   = settings.style.as_ref();
    let padding = style.padding();

    let start_h  = section_height_vertical(&view.start, settings);
    let center_h = section_height_vertical(&view.center, settings);
    let end_h    = section_height_vertical(&view.end, settings);

    // Start: top-aligned
    let start_y = rect.y + padding - state.scroll_offset;
    let start_rects = layout_section_vertical(&view.start, rect.x, start_y, rect.width, settings);
    for (item, &r) in view.start.items.iter().zip(start_rects.iter()) {
        draw_item(ctx, item, r, settings, state);
    }

    // Center: centered vertically
    if !view.center.is_empty() {
        let center_y = rect.y + (rect.height - center_h) / 2.0;
        let center_rects = layout_section_vertical(&view.center, rect.x, center_y, rect.width, settings);
        for (item, &r) in view.center.items.iter().zip(center_rects.iter()) {
            draw_item(ctx, item, r, settings, state);
        }
    }

    // End: bottom-aligned
    if !view.end.is_empty() {
        let end_y = rect.y + rect.height - end_h - padding;
        let end_rects = layout_section_vertical(&view.end, rect.x, end_y, rect.width, settings);
        for (item, &r) in view.end.items.iter().zip(end_rects.iter()) {
            draw_item(ctx, item, r, settings, state);
        }
    }

    let _ = start_h;
}

// ---------------------------------------------------------------------------
// Per-item draw
// ---------------------------------------------------------------------------

fn draw_item(
    ctx:      &mut dyn RenderContext,
    item:     &ToolbarItem<'_>,
    rect:     Rect,
    settings: &ToolbarSettings,
    state:    &ToolbarState,
) {
    let theme  = settings.theme.as_ref();
    let style  = settings.style.as_ref();

    match item {
        ToolbarItem::Separator => {
            let sep_pad = style.separator_padding();
            let sep_th  = style.separator_thickness();
            // Horizontal separator inside a horizontal toolbar
            let cx = rect.x + rect.width / 2.0;
            ctx.set_fill_color(theme.separator());
            ctx.fill_rect(
                cx - sep_th / 2.0,
                rect.y + sep_pad,
                sep_th,
                rect.height - sep_pad * 2.0,
            );
        }

        ToolbarItem::Spacer { .. } | ToolbarItem::Custom { .. } => {
            // Custom: caller-drawn; Spacer: no rendering
            if let ToolbarItem::Custom { draw, .. } = item {
                draw(ctx, rect);
            }
        }

        ToolbarItem::Label { text, .. } => {
            ctx.set_fill_color(theme.label_text());
            ctx.set_font(&format!("{}px sans-serif", style.font_size_small()));
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text(text, rect.x, rect.y + rect.height / 2.0);
        }

        ToolbarItem::Clock { time_text, .. } => {
            ctx.set_fill_color(theme.clock_text());
            ctx.set_font(&format!("{}px sans-serif", style.font_size_small()));
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text(time_text, rect.x, rect.y + rect.height / 2.0);
        }

        ToolbarItem::IconButton { id, icon, active, .. } => {
            let hovered = state.hovered_item_id.as_deref() == Some(id);
            let pressed = state.pressed_item_id.as_deref() == Some(id);
            draw_item_bg(ctx, rect, *active, hovered, pressed, settings);
            let icon_color = item_icon_color(*active, hovered, settings);
            draw_icon_centered(ctx, rect, icon.name(), icon_color, style.icon_size());
        }

        ToolbarItem::TextButton { id, text, active, .. } => {
            let hovered = state.hovered_item_id.as_deref() == Some(id);
            let pressed = state.pressed_item_id.as_deref() == Some(id);
            draw_item_bg(ctx, rect, *active, hovered, pressed, settings);
            let text_color = item_text_color(*active, hovered, settings);
            ctx.set_fill_color(text_color);
            ctx.set_font(&format!("{}px sans-serif", style.font_size()));
            ctx.set_text_align(TextAlign::Center);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text(text, rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
        }

        ToolbarItem::IconTextButton { id, icon, text, active, .. } => {
            let hovered = state.hovered_item_id.as_deref() == Some(id);
            let pressed = state.pressed_item_id.as_deref() == Some(id);
            draw_item_bg(ctx, rect, *active, hovered, pressed, settings);
            let icon_color = item_icon_color(*active, hovered, settings);
            let text_color = item_text_color(*active, hovered, settings);
            let icon_sz    = style.icon_size();
            let padding    = 4.0_f64;
            let icon_x     = rect.x + padding;
            let icon_y     = rect.y + (rect.height - icon_sz) / 2.0;
            draw_icon_at(ctx, icon_x, icon_y, icon.name(), icon_color, icon_sz);
            ctx.set_fill_color(text_color);
            ctx.set_font(&format!("{}px sans-serif", style.font_size()));
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text(text, icon_x + icon_sz + 4.0, rect.y + rect.height / 2.0);
        }

        ToolbarItem::Dropdown { id, label, icon, current, .. } => {
            let hovered  = state.hovered_item_id.as_deref() == Some(id);
            let pressed  = state.pressed_item_id.as_deref() == Some(id) || state.is_dropdown_open(id);
            draw_item_bg(ctx, rect, false, hovered, pressed, settings);
            let text_color = item_text_color(false, hovered, settings);
            let mut text_x = rect.x + 6.0;
            if let Some(ic) = icon {
                let icon_sz = style.icon_size();
                let icon_color = item_icon_color(false, hovered, settings);
                draw_icon_at(ctx, text_x, rect.y + (rect.height - icon_sz) / 2.0, ic.name(), icon_color, icon_sz);
                text_x += icon_sz + 4.0;
            }
            let display = current.unwrap_or(label);
            ctx.set_fill_color(text_color);
            ctx.set_font(&format!("{}px sans-serif", style.font_size()));
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text(display, text_x, rect.y + rect.height / 2.0);
            // Chevron
            draw_chevron_right(ctx, rect, theme.split_chevron());
        }

        ToolbarItem::ColorButton { id, color, .. } => {
            let hovered = state.hovered_item_id.as_deref() == Some(id);
            let pressed = state.pressed_item_id.as_deref() == Some(id);
            draw_item_bg(ctx, rect, false, hovered, pressed, settings);
            let swatch_sz    = style.color_swatch_size();
            let swatch_x     = rect.x + (rect.width - swatch_sz) / 2.0;
            let swatch_y     = rect.y + (rect.height - swatch_sz) / 2.0;
            let color_hex    = rgba_to_hex(*color);
            ctx.set_fill_color(&color_hex);
            ctx.fill_rect(swatch_x, swatch_y, swatch_sz, swatch_sz);
            ctx.set_stroke_color(theme.color_swatch_border());
            ctx.set_stroke_width(style.color_swatch_border_width());
            ctx.set_line_dash(&[]);
            ctx.stroke_rect(swatch_x, swatch_y, swatch_sz, swatch_sz);
        }

        ToolbarItem::LineWidthButton { id, width, .. } => {
            let hovered = state.hovered_item_id.as_deref() == Some(id);
            let pressed = state.pressed_item_id.as_deref() == Some(id);
            draw_item_bg(ctx, rect, false, hovered, pressed, settings);
            let line_w = *width as f64;
            let cx = rect.x + rect.width / 2.0;
            let cy = rect.y + rect.height / 2.0;
            let half = rect.width / 2.0 - 6.0;
            ctx.set_stroke_color(item_text_color(false, hovered, settings));
            ctx.set_stroke_width(line_w);
            ctx.set_line_dash(&[]);
            ctx.begin_path();
            ctx.move_to(cx - half, cy);
            ctx.line_to(cx + half, cy);
            ctx.stroke();
        }

        ToolbarItem::SplitIconButton { id, icon, .. } => {
            let hovered      = state.hovered_item_id.as_deref() == Some(id);
            let pressed      = state.pressed_item_id.as_deref() == Some(id);
            let split_w      = style.split_chevron_width();
            let main_rect    = Rect::new(rect.x, rect.y, rect.width - split_w, rect.height);
            let chevron_rect = Rect::new(rect.x + rect.width - split_w, rect.y, split_w, rect.height);
            draw_item_bg(ctx, main_rect, false, hovered, pressed, settings);
            let icon_color = item_icon_color(false, hovered, settings);
            draw_icon_centered(ctx, main_rect, icon.name(), icon_color, style.icon_size());
            // Divider + chevron zone
            ctx.set_fill_color(theme.split_divider());
            ctx.fill_rect(chevron_rect.x, chevron_rect.y + 4.0, 1.0, chevron_rect.height - 8.0);
            draw_small_chevron_down(ctx, chevron_rect, theme.split_chevron());
        }

        ToolbarItem::SplitLineWidthButton { id, width, .. } => {
            let hovered      = state.hovered_item_id.as_deref() == Some(id);
            let pressed      = state.pressed_item_id.as_deref() == Some(id);
            let split_w      = style.split_chevron_width();
            let main_rect    = Rect::new(rect.x, rect.y, rect.width - split_w, rect.height);
            let chevron_rect = Rect::new(rect.x + rect.width - split_w, rect.y, split_w, rect.height);
            draw_item_bg(ctx, main_rect, false, hovered, pressed, settings);
            let line_w = *width as f64;
            let cx  = main_rect.x + main_rect.width / 2.0;
            let cy  = main_rect.y + main_rect.height / 2.0;
            let half = main_rect.width / 2.0 - 4.0;
            ctx.set_stroke_color(item_text_color(false, hovered, settings));
            ctx.set_stroke_width(line_w);
            ctx.set_line_dash(&[]);
            ctx.begin_path();
            ctx.move_to(cx - half, cy);
            ctx.line_to(cx + half, cy);
            ctx.stroke();
            ctx.set_fill_color(theme.split_divider());
            ctx.fill_rect(chevron_rect.x, chevron_rect.y + 4.0, 1.0, chevron_rect.height - 8.0);
            draw_small_chevron_down(ctx, chevron_rect, theme.split_chevron());
        }
    }
}

// ---------------------------------------------------------------------------
// ChromeStrip draw
// ---------------------------------------------------------------------------

fn draw_chrome_strip(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    view:     &ToolbarView<'_>,
    settings: &ToolbarSettings,
    _state:   &ToolbarState,
) {
    let chrome = match &view.chrome {
        Some(c) => c,
        None => return,
    };

    let theme = settings.theme.as_ref();
    let style = settings.style.as_ref();
    let font_size = style.font_size();

    // --- Tabs ---
    let tab_padding_h = 12.0_f64;
    let close_sz      = 16.0_f64;
    let mut tab_x     = rect.x + style.padding();

    for (i, tab) in chrome.tabs.iter().enumerate() {
        // Estimate tab width from label + padding + optional close icon
        let label_w  = (tab.label.len() as f64) * (font_size * 0.6); // approx
        let tab_w    = tab_padding_h * 2.0 + label_w
            + if tab.closable { close_sz + 4.0 } else { 0.0 };
        let tab_rect = Rect::new(tab_x, rect.y, tab_w, rect.height);

        let bg = if tab.active {
            theme.chrome_tab_bg_active()
        } else if chrome.hovered_tab == Some(i) {
            theme.chrome_tab_bg_hover()
        } else {
            theme.chrome_tab_bg_inactive()
        };

        if bg != "transparent" {
            ctx.set_fill_color(bg);
            ctx.fill_rect(tab_rect.x, tab_rect.y, tab_rect.width, tab_rect.height);
        }

        let text_color = if tab.active {
            theme.chrome_tab_text_active()
        } else {
            theme.chrome_tab_text_inactive()
        };
        ctx.set_fill_color(text_color);
        ctx.set_font(&format!("{}px sans-serif", font_size));
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Middle);
        ctx.fill_text(
            tab.label,
            tab_rect.x + tab_padding_h,
            tab_rect.y + tab_rect.height / 2.0,
        );

        tab_x += tab_w + 2.0;
    }

    // Window controls (right side)
    if chrome.window_controls {
        let ctrl_sz    = 46.0_f64; // Windows-style ctrl button width
        let ctrl_h     = rect.height;
        let total_ctrl = ctrl_sz * 3.0;
        let mut cx     = rect.x + rect.width - total_ctrl;

        // Minimize
        let min_rect = Rect::new(cx, rect.y, ctrl_sz, ctrl_h);
        if _state.chrome_minimize_hovered {
            ctx.set_fill_color(theme.chrome_ctrl_hover());
            ctx.fill_rect(min_rect.x, min_rect.y, min_rect.width, min_rect.height);
        }
        draw_icon_centered(ctx, min_rect, "minimize", theme.chrome_ctrl_icon(), 10.0);
        cx += ctrl_sz;

        // Maximize
        let max_rect = Rect::new(cx, rect.y, ctrl_sz, ctrl_h);
        if _state.chrome_maximize_hovered {
            ctx.set_fill_color(theme.chrome_ctrl_hover());
            ctx.fill_rect(max_rect.x, max_rect.y, max_rect.width, max_rect.height);
        }
        draw_icon_centered(ctx, max_rect, "maximize", theme.chrome_ctrl_icon(), 10.0);
        cx += ctrl_sz;

        // Close
        let close_rect = Rect::new(cx, rect.y, ctrl_sz, ctrl_h);
        if _state.chrome_close_hovered {
            ctx.set_fill_color(theme.chrome_close_hover());
        } else {
            // transparent
            ctx.set_fill_color("transparent");
        }
        if _state.chrome_close_hovered {
            ctx.fill_rect(close_rect.x, close_rect.y, close_rect.width, close_rect.height);
        }
        draw_icon_centered(ctx, close_rect, "close", theme.chrome_ctrl_icon(), 10.0);
    }
}

// ---------------------------------------------------------------------------
// InputCoordinator child registration helpers
// ---------------------------------------------------------------------------

fn register_horizontal_items(
    coord:    &mut InputCoordinator,
    parent:   &WidgetId,
    rect:     Rect,
    view:     &ToolbarView<'_>,
    settings: &ToolbarSettings,
    state:    &ToolbarState,
) {
    let style   = settings.style.as_ref();
    let padding = style.padding();

    let start_w = section_width_horizontal(&view.start, settings);
    let end_w   = section_width_horizontal(&view.end, settings);
    let center_w = section_width_horizontal(&view.center, settings);

    let start_x = rect.x + padding - state.scroll_offset;
    register_section_horizontal(coord, parent, &view.start, start_x, rect, settings);

    if !view.center.is_empty() {
        let center_x = rect.x + (rect.width - center_w) / 2.0;
        register_section_horizontal(coord, parent, &view.center, center_x, rect, settings);
    }

    if !view.end.is_empty() {
        let end_x = rect.x + rect.width - end_w - padding;
        register_section_horizontal(coord, parent, &view.end, end_x, rect, settings);
    }

    let _ = start_w;
}

fn register_section_horizontal(
    coord:    &mut InputCoordinator,
    parent:   &WidgetId,
    section:  &ToolbarSection<'_>,
    start_x:  f64,
    bar:      Rect,
    settings: &ToolbarSettings,
) {
    let rects = layout_section_horizontal(section, start_x, bar.y, bar.height, settings);
    for (item, &item_rect) in section.items.iter().zip(rects.iter()) {
        register_item(coord, parent, item, item_rect, settings);
    }
}

fn register_vertical_items(
    coord:    &mut InputCoordinator,
    parent:   &WidgetId,
    rect:     Rect,
    view:     &ToolbarView<'_>,
    settings: &ToolbarSettings,
    state:    &ToolbarState,
) {
    let style   = settings.style.as_ref();
    let padding = style.padding();

    let start_h  = section_height_vertical(&view.start, settings);
    let center_h = section_height_vertical(&view.center, settings);
    let end_h    = section_height_vertical(&view.end, settings);

    let start_y = rect.y + padding - state.scroll_offset;
    let start_rects = layout_section_vertical(&view.start, rect.x, start_y, rect.width, settings);
    for (item, &r) in view.start.items.iter().zip(start_rects.iter()) {
        register_item(coord, parent, item, r, settings);
    }

    if !view.center.is_empty() {
        let center_y = rect.y + (rect.height - center_h) / 2.0;
        let center_rects = layout_section_vertical(&view.center, rect.x, center_y, rect.width, settings);
        for (item, &r) in view.center.items.iter().zip(center_rects.iter()) {
            register_item(coord, parent, item, r, settings);
        }
    }

    if !view.end.is_empty() {
        let end_y = rect.y + rect.height - end_h - padding;
        let end_rects = layout_section_vertical(&view.end, rect.x, end_y, rect.width, settings);
        for (item, &r) in view.end.items.iter().zip(end_rects.iter()) {
            register_item(coord, parent, item, r, settings);
        }
    }

    let _ = start_h;
}

fn register_item(
    coord:    &mut InputCoordinator,
    parent:   &WidgetId,
    item:     &ToolbarItem<'_>,
    rect:     Rect,
    settings: &ToolbarSettings,
) {
    let style = settings.style.as_ref();
    match item {
        ToolbarItem::Separator | ToolbarItem::Spacer { .. } | ToolbarItem::Label { .. } => {}

        ToolbarItem::Clock { id, .. } => {
            coord.register_child(
                parent,
                format!("{}:{}", parent.0, id),
                WidgetKind::Clock,
                rect,
                Sense::HOVER,
            );
        }

        ToolbarItem::Custom { id, .. } => {
            coord.register_child(
                parent,
                format!("{}:{}", parent.0, id),
                WidgetKind::Custom,
                rect,
                Sense::CLICK | Sense::HOVER,
            );
        }

        ToolbarItem::IconButton { id, .. }
        | ToolbarItem::TextButton { id, .. }
        | ToolbarItem::IconTextButton { id, .. }
        | ToolbarItem::ColorButton { id, .. }
        | ToolbarItem::LineWidthButton { id, .. } => {
            coord.register_child(
                parent,
                format!("{}:{}", parent.0, id),
                WidgetKind::Button,
                rect,
                Sense::CLICK | Sense::HOVER,
            );
        }

        ToolbarItem::Dropdown { id, .. } => {
            coord.register_child(
                parent,
                format!("{}:{}", parent.0, id),
                WidgetKind::DropdownTrigger,
                rect,
                Sense::CLICK | Sense::HOVER,
            );
        }

        ToolbarItem::SplitIconButton { id, .. } | ToolbarItem::SplitLineWidthButton { id, .. } => {
            let split_w = style.split_chevron_width();
            let main_rect = Rect::new(rect.x, rect.y, rect.width - split_w, rect.height);
            let chevron_rect = Rect::new(rect.x + rect.width - split_w, rect.y, split_w, rect.height);
            coord.register_child(
                parent,
                format!("{}:{}", parent.0, id),
                WidgetKind::Button,
                main_rect,
                Sense::CLICK | Sense::HOVER,
            );
            coord.register_child(
                parent,
                format!("{}:{}:chevron", parent.0, id),
                WidgetKind::Button,
                chevron_rect,
                Sense::CLICK | Sense::HOVER,
            );
        }
    }
}

fn register_chrome_items(
    coord:    &mut InputCoordinator,
    parent:   &WidgetId,
    rect:     Rect,
    view:     &ToolbarView<'_>,
    settings: &ToolbarSettings,
    _state:   &ToolbarState,
) {
    let chrome = match &view.chrome {
        Some(c) => c,
        None => return,
    };

    let style         = settings.style.as_ref();
    let tab_padding_h = 12.0_f64;
    let font_size     = style.font_size();
    let close_sz      = 16.0_f64;
    let mut tab_x     = rect.x + style.padding();

    for (i, tab) in chrome.tabs.iter().enumerate() {
        let label_w = (tab.label.len() as f64) * (font_size * 0.6);
        let tab_w   = tab_padding_h * 2.0 + label_w
            + if tab.closable { close_sz + 4.0 } else { 0.0 };
        let tab_rect = Rect::new(tab_x, rect.y, tab_w, rect.height);
        coord.register_child(
            parent,
            format!("{}:chrome_tab:{}", parent.0, i),
            WidgetKind::Button,
            tab_rect,
            Sense::CLICK | Sense::HOVER,
        );
        tab_x += tab_w + 2.0;
    }

    if chrome.window_controls {
        let ctrl_sz    = 46.0_f64;
        let total_ctrl = ctrl_sz * 3.0;
        let base_x     = rect.x + rect.width - total_ctrl;

        coord.register_child(
            parent,
            format!("{}:chrome_minimize", parent.0),
            WidgetKind::Button,
            Rect::new(base_x, rect.y, ctrl_sz, rect.height),
            Sense::CLICK | Sense::HOVER,
        );
        coord.register_child(
            parent,
            format!("{}:chrome_maximize", parent.0),
            WidgetKind::Button,
            Rect::new(base_x + ctrl_sz, rect.y, ctrl_sz, rect.height),
            Sense::CLICK | Sense::HOVER,
        );
        coord.register_child(
            parent,
            format!("{}:chrome_close", parent.0),
            WidgetKind::Button,
            Rect::new(base_x + ctrl_sz * 2.0, rect.y, ctrl_sz, rect.height),
            Sense::CLICK | Sense::HOVER,
        );
    }

    if chrome.drag_zone {
        // Drag zone = entire bar minus tabs minus controls
        let ctrl_w = if chrome.window_controls { 46.0 * 3.0 } else { 0.0 };
        let drag_x = tab_x;
        let drag_w = (rect.x + rect.width - ctrl_w - drag_x).max(0.0);
        if drag_w > 0.0 {
            coord.register_child(
                parent,
                format!("{}:chrome_drag", parent.0),
                WidgetKind::DragHandle,
                Rect::new(drag_x, rect.y, drag_w, rect.height),
                Sense::DRAG,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Primitive draw helpers
// ---------------------------------------------------------------------------

fn draw_item_bg(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    active:   bool,
    hovered:  bool,
    pressed:  bool,
    settings: &ToolbarSettings,
) {
    let theme  = settings.theme.as_ref();
    let style  = settings.style.as_ref();
    let radius = style.item_radius();

    let color = if pressed {
        theme.item_bg_pressed()
    } else if active {
        theme.item_bg_active()
    } else if hovered {
        theme.item_bg_hover()
    } else {
        theme.item_bg_normal()
    };

    if color == "transparent" {
        return;
    }

    ctx.set_fill_color(color);
    ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, radius);
}

fn item_icon_color<'a>(active: bool, hovered: bool, settings: &'a ToolbarSettings) -> &'a str {
    let theme = settings.theme.as_ref();
    if active { theme.icon_active() } else if hovered { theme.icon_hover() } else { theme.icon_normal() }
}

fn item_text_color<'a>(active: bool, hovered: bool, settings: &'a ToolbarSettings) -> &'a str {
    let theme = settings.theme.as_ref();
    if active { theme.item_text_active() } else if hovered { theme.item_text_hover() } else { theme.item_text_normal() }
}

/// Draw an icon centered within `rect`.
fn draw_icon_centered(
    ctx:    &mut dyn RenderContext,
    rect:   Rect,
    name:   &str,
    color:  &str,
    size:   f64,
) {
    let ix = rect.x + (rect.width - size) / 2.0;
    let iy = rect.y + (rect.height - size) / 2.0;
    draw_icon_at(ctx, ix, iy, name, color, size);
}

/// Draw icon at an explicit top-left position.
/// Falls back to a filled square placeholder until asset system is wired.
fn draw_icon_at(
    ctx:   &mut dyn RenderContext,
    x:     f64,
    y:     f64,
    _name: &str,
    color: &str,
    size:  f64,
) {
    ctx.set_fill_color(color);
    ctx.fill_rounded_rect(x + 1.0, y + 1.0, size - 2.0, size - 2.0, 1.0);
}

/// Draw a right-pointing chevron on the right edge of `rect`.
fn draw_chevron_right(ctx: &mut dyn RenderContext, rect: Rect, color: &str) {
    let sz   = 6.0_f64;
    let cx   = rect.x + rect.width - 8.0;
    let cy   = rect.y + rect.height / 2.0;
    let half = sz / 2.0;
    ctx.set_fill_color(color);
    ctx.begin_path();
    ctx.move_to(cx - half, cy - half);
    ctx.line_to(cx + half / 2.0, cy);
    ctx.line_to(cx - half, cy + half);
    ctx.close_path();
    ctx.fill();
}

/// Draw a small downward chevron centered in `rect`.
fn draw_small_chevron_down(ctx: &mut dyn RenderContext, rect: Rect, color: &str) {
    let sz   = 5.0_f64;
    let cx   = rect.x + rect.width / 2.0;
    let cy   = rect.y + rect.height / 2.0;
    let half = sz / 2.0;
    ctx.set_fill_color(color);
    ctx.begin_path();
    ctx.move_to(cx - half, cy - half / 2.0);
    ctx.line_to(cx + half, cy - half / 2.0);
    ctx.line_to(cx, cy + half);
    ctx.close_path();
    ctx.fill();
}

/// Convert `[r, g, b, a]` (0–255 each) to a CSS hex string.
fn rgba_to_hex(color: [u8; 4]) -> String {
    if color[3] == 255 {
        format!("#{:02x}{:02x}{:02x}", color[0], color[1], color[2])
    } else {
        format!("#{:02x}{:02x}{:02x}{:02x}", color[0], color[1], color[2], color[3])
    }
}
