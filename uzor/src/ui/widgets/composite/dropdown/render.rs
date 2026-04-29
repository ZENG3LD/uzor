//! Dropdown render entry point and per-kind layout pipelines.
//!
//! # API
//!
//! - `register_input_coordinator_dropdown` — registers the composite + child
//!   hit-rects with an `InputCoordinator`.  No drawing.  Use when explicit
//!   z-order control is needed.
//! - `register_context_manager_dropdown` — convenience wrapper: registers and
//!   draws in one call using a `ContextManager`.
//!
//! # Draw order (non-Custom kinds)
//!
//! 1. Shadow rect
//! 2. Opaque frame background + border
//! 3. Per-kind content
//! 4. Submenu sibling panel (if `view.kind` carries submenu data and state has one open)

use crate::app_context::ContextManager;
use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::{Rect, WidgetId};

use super::settings::DropdownSettings;
use super::state::DropdownState;
use super::types::{
    CheckboxItem, DropdownGroup, DropdownItem, DropdownItemRight, DropdownRenderKind,
    DropdownView, DropdownViewKind, GridDropdownItem,
};

// ---------------------------------------------------------------------------
// Public API — registration only
// ---------------------------------------------------------------------------

/// Register the dropdown composite and all child hit-rects with the coordinator.
///
/// No drawing occurs.  Use when explicit z-order control is needed.
///
/// Returns the `WidgetId` assigned to the dropdown composite.
pub fn register_input_coordinator_dropdown(
    coord:    &mut InputCoordinator,
    id:       impl Into<WidgetId>,
    rect:     Rect,
    state:    &DropdownState,
    view:     &DropdownView<'_>,
    settings: &DropdownSettings,
    kind:     DropdownRenderKind,
    layer:    &LayerId,
) -> WidgetId {
    let dd_id = coord.register_composite(id, WidgetKind::Dropdown, rect, Sense::CLICK, layer);

    if !view.open {
        return dd_id;
    }

    let origin = view.position_override.unwrap_or(state.effective_origin());
    let frame  = compute_frame(origin, rect, view, settings);
    let layout = compute_layout(frame, settings);

    match kind {
        DropdownRenderKind::Flat => {
            if let DropdownViewKind::Flat { items, submenu_items, .. } = &view.kind {
                register_flat_hits(coord, &dd_id, layout.content, items, settings);
                if let Some((trigger_id, sub_items)) = submenu_items {
                    if state.submenu_open.as_deref() == Some(trigger_id) {
                        let sub_frame = compute_submenu_frame(frame, state, settings);
                        let sub_layout = compute_layout(sub_frame, settings);
                        register_flat_hits(coord, &dd_id, sub_layout.content, sub_items, settings);
                    }
                }
            }
        }
        DropdownRenderKind::Inline => {
            if let DropdownViewKind::Inline { options, .. } = &view.kind {
                register_inline_hits(coord, &dd_id, layout.content, options, settings);
            }
        }
        DropdownRenderKind::Grid => {
            if let DropdownViewKind::Grid { items, columns, .. } = &view.kind {
                register_grid_hits(coord, &dd_id, layout.content, items, *columns, settings);
            }
        }
        DropdownRenderKind::Grouped => {
            if let DropdownViewKind::Grouped { groups, list_items, .. } = &view.kind {
                register_grouped_hits(coord, &dd_id, layout.content, groups, list_items, settings);
            }
        }
        DropdownRenderKind::Custom => {}
    }

    dd_id
}

// ---------------------------------------------------------------------------
// Public API — convenience wrapper (ContextManager)
// ---------------------------------------------------------------------------

/// Register + draw a dropdown in one call using a `ContextManager`.
///
/// Returns the `WidgetId` assigned to the dropdown composite.
pub fn register_context_manager_dropdown(
    ctx_mgr:  &mut ContextManager,
    render:   &mut dyn RenderContext,
    id:       impl Into<WidgetId>,
    rect:     Rect,
    state:    &mut DropdownState,
    view:     &mut DropdownView<'_>,
    settings: &DropdownSettings,
    kind:     DropdownRenderKind,
    layer:    &LayerId,
) -> WidgetId {
    let coord = &mut ctx_mgr.input;
    let dd_id =
        register_input_coordinator_dropdown(coord, id, rect, state, view, settings, kind, layer);

    if view.open {
        draw_dropdown_panels(render, rect, state, view, settings, kind);
    }

    dd_id
}

// ---------------------------------------------------------------------------
// Internal draw pipeline
// ---------------------------------------------------------------------------

fn draw_dropdown_panels(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    state:    &DropdownState,
    view:     &DropdownView<'_>,
    settings: &DropdownSettings,
    kind:     DropdownRenderKind,
) {
    if let DropdownViewKind::Custom(ref draw) = view.kind {
        let origin = view.position_override.unwrap_or(state.effective_origin());
        let frame  = compute_frame(origin, rect, view, settings);
        draw(ctx, frame, state, settings);
        return;
    }

    let origin = view.position_override.unwrap_or(state.effective_origin());
    let frame  = compute_frame(origin, rect, view, settings);
    let layout = compute_layout(frame, settings);

    draw_frame(ctx, frame, settings);

    match kind {
        DropdownRenderKind::Flat => {
            if let DropdownViewKind::Flat {
                items,
                hovered_id,
                submenu_items,
                submenu_hovered_id,
            } = &view.kind
            {
                draw_flat_list(ctx, layout.content, items, *hovered_id, state, settings);

                // Submenu sibling panel
                if let Some((trigger_id, sub_items)) = submenu_items {
                    if state.submenu_open.as_deref() == Some(trigger_id) {
                        let sub_frame = compute_submenu_frame(frame, state, settings);
                        draw_frame(ctx, sub_frame, settings);
                        let sub_layout = compute_layout(sub_frame, settings);
                        draw_flat_list(ctx, sub_layout.content, sub_items, *submenu_hovered_id, state, settings);
                    }
                }
            }
        }
        DropdownRenderKind::Inline => {
            if let DropdownViewKind::Inline { options, hovered_id, .. } = &view.kind {
                draw_inline_list(ctx, layout.content, options, *hovered_id, settings);
            }
        }
        DropdownRenderKind::Grid => {
            if let DropdownViewKind::Grid { items, columns, hovered_id } = &view.kind {
                draw_grid(ctx, layout.content, items, *columns, *hovered_id, settings);
            }
        }
        DropdownRenderKind::Grouped => {
            if let DropdownViewKind::Grouped { groups, list_items, hovered_id } = &view.kind {
                draw_grouped(ctx, layout.content, groups, list_items, *hovered_id, settings);
            }
        }
        DropdownRenderKind::Custom => {}
    }
}

// ---------------------------------------------------------------------------
// Frame draw helper
// ---------------------------------------------------------------------------

fn draw_frame(ctx: &mut dyn RenderContext, frame: Rect, settings: &DropdownSettings) {
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

    // Opaque background (dropdowns are always solid)
    ctx.set_fill_color(theme.bg());
    ctx.fill_rounded_rect(frame.x, frame.y, frame.width, frame.height, style.radius());

    // Border
    ctx.set_stroke_color(theme.border());
    ctx.set_stroke_width(style.border_width());
    ctx.set_line_dash(&[]);
    ctx.stroke_rounded_rect(frame.x, frame.y, frame.width, frame.height, style.radius());
}

// ---------------------------------------------------------------------------
// Layout helpers
// ---------------------------------------------------------------------------

/// Compute the panel frame rect from the anchor / origin.
fn compute_frame(
    origin:   (f64, f64),
    rect:     Rect,
    _view:    &DropdownView<'_>,
    _settings: &DropdownSettings,
) -> Rect {
    // Use the caller-provided rect dimensions; override origin from state.
    Rect::new(origin.0, origin.1, rect.width, rect.height)
}

struct DropdownLayout {
    content: Rect,
}

fn compute_layout(frame: Rect, settings: &DropdownSettings) -> DropdownLayout {
    let pad = settings.style.padding();
    DropdownLayout {
        content: Rect::new(
            frame.x + pad,
            frame.y + pad,
            (frame.width  - pad * 2.0).max(0.0),
            (frame.height - pad * 2.0).max(0.0),
        ),
    }
}

/// Compute the submenu panel frame (sibling to the right of parent).
fn compute_submenu_frame(
    parent:   Rect,
    state:    &DropdownState,
    settings: &DropdownSettings,
) -> Rect {
    let gap = settings.style.submenu_gap();
    let (_, sy) = state.submenu_origin;
    // X: parent.right() + gap; Y: from submenu_origin (trigger item y)
    Rect::new(
        parent.x + parent.width + gap,
        sy,
        parent.width,
        parent.height,
    )
}

// ---------------------------------------------------------------------------
// Template: Flat list
// ---------------------------------------------------------------------------

fn draw_flat_list(
    ctx:        &mut dyn RenderContext,
    content:    Rect,
    items:      &[DropdownItem<'_>],
    hovered_id: Option<&str>,
    state:      &DropdownState,
    settings:   &DropdownSettings,
) {
    let theme = settings.theme.as_ref();
    let style = settings.style.as_ref();

    let mut cursor_y = content.y;

    for item in items {
        match item {
            DropdownItem::Header { label } => {
                let h = style.header_height();
                // Header text
                ctx.set_fill_color(theme.header_text());
                ctx.set_font(&format!("bold {}px sans-serif", style.font_size()));
                ctx.set_text_align(TextAlign::Left);
                ctx.set_text_baseline(TextBaseline::Middle);
                ctx.fill_text(label, content.x + style.item_padding_x(), cursor_y + h / 2.0);
                // Bottom border
                ctx.set_fill_color(theme.header_border());
                ctx.fill_rect(content.x, cursor_y + h - 1.0, content.width, 1.0);
                cursor_y += h;
            }

            DropdownItem::Item { id, label, icon, right, disabled, danger, accent_color } => {
                let h       = style.item_height();
                let hovered = hovered_id == Some(id);
                let selected = state.selected_id.as_deref() == Some(id);

                // Background
                let bg = if hovered {
                    if *danger { theme.item_bg_danger_hover() } else { theme.item_bg_hover() }
                } else if selected {
                    theme.item_bg_selected()
                } else {
                    theme.item_bg_normal()
                };
                ctx.set_fill_color(bg);
                ctx.fill_rounded_rect(
                    content.x,
                    cursor_y,
                    content.width,
                    h,
                    style.item_hover_radius(),
                );

                // Left accent bar
                if let Some(color) = accent_color {
                    let inset = style.accent_bar_inset_y();
                    ctx.set_fill_color(color);
                    ctx.fill_rect(content.x, cursor_y + inset, style.accent_bar_w(), h - inset * 2.0);
                }

                // Icon (if present)
                let text_x = if icon.is_some() {
                    let icon_x = content.x + style.item_padding_x();
                    let icon_y = cursor_y + (h - style.icon_size()) / 2.0;
                    let icon_color = if *disabled {
                        theme.item_text_disabled()
                    } else {
                        theme.item_text()
                    };
                    ctx.set_fill_color(icon_color);
                    ctx.set_font(&format!("{}px sans-serif", style.icon_size()));
                    ctx.set_text_align(TextAlign::Left);
                    ctx.set_text_baseline(TextBaseline::Top);
                    ctx.fill_text(icon.unwrap_or(""), icon_x, icon_y);
                    icon_x + style.icon_size() + style.icon_text_gap()
                } else {
                    content.x + style.item_padding_x()
                };

                // Label
                let text_color = if *disabled {
                    theme.item_text_disabled()
                } else if *danger {
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
                ctx.fill_text(label, text_x, cursor_y + h / 2.0);

                // Right content
                match right {
                    DropdownItemRight::Shortcut(s) => {
                        ctx.set_fill_color(theme.shortcut_text());
                        ctx.set_font(&format!("{}px sans-serif", style.font_size_subtitle()));
                        ctx.set_text_align(TextAlign::Right);
                        ctx.fill_text(
                            s,
                            content.x + content.width - style.item_padding_x(),
                            cursor_y + h / 2.0,
                        );
                    }
                    DropdownItemRight::Subtitle(s) => {
                        ctx.set_fill_color(theme.item_text_disabled());
                        ctx.set_font(&format!("{}px sans-serif", style.font_size_subtitle()));
                        ctx.set_text_align(TextAlign::Right);
                        ctx.fill_text(
                            s,
                            content.x + content.width - style.item_padding_x(),
                            cursor_y + h / 2.0,
                        );
                    }
                    DropdownItemRight::Toggle(on) => {
                        draw_toggle_pill(ctx, content, cursor_y, h, *on, settings);
                    }
                    DropdownItemRight::None => {}
                }

                cursor_y += h;
            }

            DropdownItem::Separator => {
                let h = style.separator_height();
                ctx.set_fill_color(theme.separator());
                ctx.fill_rect(content.x, cursor_y + h / 2.0 - 0.5, content.width, 1.0);
                cursor_y += h;
            }

            DropdownItem::Submenu { id, label, icon } => {
                let h       = style.item_height();
                let hovered = hovered_id == Some(id);

                let bg = if hovered { theme.item_bg_hover() } else { theme.item_bg_normal() };
                ctx.set_fill_color(bg);
                ctx.fill_rounded_rect(
                    content.x,
                    cursor_y,
                    content.width,
                    h,
                    style.item_hover_radius(),
                );

                // Icon
                let text_x = if icon.is_some() {
                    let ix = content.x + style.item_padding_x();
                    let iy = cursor_y + (h - style.icon_size()) / 2.0;
                    ctx.set_fill_color(theme.item_text());
                    ctx.set_font(&format!("{}px sans-serif", style.icon_size()));
                    ctx.set_text_align(TextAlign::Left);
                    ctx.set_text_baseline(TextBaseline::Top);
                    ctx.fill_text(icon.unwrap_or(""), ix, iy);
                    ix + style.icon_size() + style.icon_text_gap()
                } else {
                    content.x + style.item_padding_x()
                };

                // Label
                let text_color = if hovered { theme.item_text_hover() } else { theme.item_text() };
                ctx.set_fill_color(text_color);
                ctx.set_font(&format!("{}px sans-serif", style.font_size()));
                ctx.set_text_align(TextAlign::Left);
                ctx.set_text_baseline(TextBaseline::Middle);
                ctx.fill_text(label, text_x, cursor_y + h / 2.0);

                // Caret arrow
                ctx.set_fill_color(theme.caret_color());
                ctx.set_text_align(TextAlign::Right);
                ctx.fill_text("▶", content.x + content.width - style.item_padding_x(), cursor_y + h / 2.0);

                cursor_y += h;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Template: Inline (split button)
// ---------------------------------------------------------------------------

fn draw_inline_list(
    ctx:        &mut dyn RenderContext,
    content:    Rect,
    options:    &[(&str, &str)],
    hovered_id: Option<&str>,
    settings:   &DropdownSettings,
) {
    let theme = settings.theme.as_ref();
    let style = settings.style.as_ref();
    let mut cursor_y = content.y;

    for (id, label) in options {
        let h       = style.item_height();
        let hovered = hovered_id == Some(id);

        let bg = if hovered { theme.item_bg_hover() } else { theme.item_bg_normal() };
        ctx.set_fill_color(bg);
        ctx.fill_rounded_rect(content.x, cursor_y, content.width, h, style.item_hover_radius());

        let text_color = if hovered { theme.item_text_hover() } else { theme.item_text() };
        ctx.set_fill_color(text_color);
        ctx.set_font(&format!("{}px sans-serif", style.font_size()));
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Middle);
        ctx.fill_text(label, content.x + style.item_padding_x(), cursor_y + h / 2.0);

        cursor_y += h;
    }
}

// ---------------------------------------------------------------------------
// Template: Grid
// ---------------------------------------------------------------------------

fn draw_grid(
    ctx:        &mut dyn RenderContext,
    content:    Rect,
    items:      &[GridDropdownItem<'_>],
    columns:    usize,
    hovered_id: Option<&str>,
    settings:   &DropdownSettings,
) {
    let theme    = settings.theme.as_ref();
    let style    = settings.style.as_ref();
    let cell_sz  = style.cell_size();
    let gap      = style.cell_gap();
    let icon_sz  = style.icon_size();
    let columns  = columns.max(1);

    for (i, cell) in items.iter().enumerate() {
        let col = i % columns;
        let row = i / columns;
        let cx  = content.x + col as f64 * (cell_sz + gap);
        let cy  = content.y + row as f64 * (cell_sz + gap);
        let cr  = Rect::new(cx, cy, cell_sz, cell_sz);

        let hovered = hovered_id == Some(cell.id);

        // Cell background
        let bg = if hovered { theme.cell_bg_hover() } else { theme.item_bg_normal() };
        ctx.set_fill_color(bg);
        ctx.fill_rounded_rect(cr.x, cr.y, cr.width, cr.height, style.item_hover_radius());

        // Cell border
        ctx.set_stroke_color(theme.cell_border());
        ctx.set_stroke_width(1.0);
        ctx.set_line_dash(&[]);
        ctx.stroke_rounded_rect(cr.x, cr.y, cr.width, cr.height, style.item_hover_radius());

        // Icon centered in cell
        let icon_color = if cell.disabled {
            theme.item_text_disabled()
        } else {
            theme.item_text()
        };
        ctx.set_fill_color(icon_color);
        ctx.set_font(&format!("{}px sans-serif", icon_sz));
        ctx.set_text_align(TextAlign::Center);
        ctx.set_text_baseline(TextBaseline::Middle);
        ctx.fill_text(cell.icon, cx + cell_sz / 2.0, cy + cell_sz / 2.0);
    }
}

// ---------------------------------------------------------------------------
// Template: Grouped
// ---------------------------------------------------------------------------

fn draw_grouped(
    ctx:        &mut dyn RenderContext,
    content:    Rect,
    groups:     &[DropdownGroup<'_>],
    list_items: &[CheckboxItem<'_>],
    hovered_id: Option<&str>,
    settings:   &DropdownSettings,
) {
    let theme       = settings.theme.as_ref();
    let style       = settings.style.as_ref();
    let cell_sz     = style.cell_size();
    let gap         = style.cell_gap();
    let icon_sz     = style.icon_size();
    let label_w     = style.row_label_width();
    let item_h      = style.item_height();
    let sep_h       = style.separator_height();
    let checkbox_sz = style.checkbox_size();

    let mut cursor_y = content.y;

    // --- Grid section ---
    for group in groups {
        // Row label
        ctx.set_fill_color(theme.item_text_disabled());
        ctx.set_font(&format!("{}px sans-serif", style.font_size()));
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Middle);
        ctx.fill_text(group.label, content.x, cursor_y + cell_sz / 2.0);

        // Cells in this row
        let cells_x = content.x + label_w + gap;
        for (j, cell) in group.items.iter().enumerate() {
            let cx = cells_x + j as f64 * (cell_sz + gap);
            let cy = cursor_y;
            let cr = Rect::new(cx, cy, cell_sz, cell_sz);

            let hovered = hovered_id == Some(cell.id);
            let bg = if hovered { theme.cell_bg_hover() } else { theme.item_bg_normal() };
            ctx.set_fill_color(bg);
            ctx.fill_rounded_rect(cr.x, cr.y, cr.width, cr.height, style.item_hover_radius());

            ctx.set_stroke_color(theme.cell_border());
            ctx.set_stroke_width(1.0);
            ctx.set_line_dash(&[]);
            ctx.stroke_rounded_rect(cr.x, cr.y, cr.width, cr.height, style.item_hover_radius());

            let icon_color = if cell.disabled { theme.item_text_disabled() } else { theme.item_text() };
            ctx.set_fill_color(icon_color);
            ctx.set_font(&format!("{}px sans-serif", icon_sz));
            ctx.set_text_align(TextAlign::Center);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text(cell.icon, cx + cell_sz / 2.0, cy + cell_sz / 2.0);
        }

        cursor_y += cell_sz + gap;
    }

    if !list_items.is_empty() {
        // Separator between grid and list sections
        ctx.set_fill_color(theme.separator());
        ctx.fill_rect(content.x, cursor_y + sep_h / 2.0 - 0.5, content.width, 1.0);
        cursor_y += sep_h;

        // --- Checkbox list section ---
        for cb in list_items {
            let hovered = hovered_id == Some(cb.id);

            let bg = if hovered { theme.item_bg_hover() } else { theme.item_bg_normal() };
            ctx.set_fill_color(bg);
            ctx.fill_rounded_rect(content.x, cursor_y, content.width, item_h, style.item_hover_radius());

            // Stroke-only checkbox square
            let cb_x = content.x + style.item_padding_x();
            let cb_y = cursor_y + (item_h - checkbox_sz) / 2.0;
            if cb.checked {
                ctx.set_fill_color(theme.checkbox_checked());
                ctx.fill_rect(cb_x, cb_y, checkbox_sz, checkbox_sz);
            }
            ctx.set_stroke_color(if cb.checked { theme.checkbox_checked() } else { theme.checkbox_border() });
            ctx.set_stroke_width(1.0);
            ctx.set_line_dash(&[]);
            ctx.stroke_rect(cb_x, cb_y, checkbox_sz, checkbox_sz);

            // Label
            let text_color = if cb.disabled { theme.item_text_disabled() } else { theme.item_text() };
            ctx.set_fill_color(text_color);
            ctx.set_font(&format!("{}px sans-serif", style.font_size()));
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text(cb.label, cb_x + checkbox_sz + style.icon_text_gap(), cursor_y + item_h / 2.0);

            cursor_y += item_h;
        }
    }
}

// ---------------------------------------------------------------------------
// Toggle pill helper
// ---------------------------------------------------------------------------

fn draw_toggle_pill(
    ctx:      &mut dyn RenderContext,
    content:  Rect,
    cursor_y: f64,
    row_h:    f64,
    on:       bool,
    settings: &DropdownSettings,
) {
    let theme     = settings.theme.as_ref();
    let style     = settings.style.as_ref();
    let track_w   = style.toggle_track_w();
    let track_h   = style.toggle_track_h();
    let thumb_d   = style.toggle_thumb_d();
    let pad       = style.item_padding_x();

    let track_x = content.x + content.width - pad - track_w;
    let track_y = cursor_y + (row_h - track_h) / 2.0;

    // Track
    let track_color = if on { theme.toggle_on() } else { theme.toggle_off() };
    ctx.set_fill_color(track_color);
    ctx.fill_rounded_rect(track_x, track_y, track_w, track_h, track_h / 2.0);

    // Thumb
    let thumb_margin = (track_h - thumb_d) / 2.0;
    let thumb_x = if on {
        track_x + track_w - thumb_d - thumb_margin
    } else {
        track_x + thumb_margin
    };
    let thumb_y = track_y + thumb_margin;

    ctx.set_fill_color(theme.toggle_thumb());
    ctx.fill_rounded_rect(thumb_x, thumb_y, thumb_d, thumb_d, thumb_d / 2.0);
}

// ---------------------------------------------------------------------------
// Hit-rect registration helpers
// ---------------------------------------------------------------------------

fn register_flat_hits(
    coord:    &mut InputCoordinator,
    parent:   &WidgetId,
    content:  Rect,
    items:    &[DropdownItem<'_>],
    settings: &DropdownSettings,
) {
    let style     = settings.style.as_ref();
    let mut cursor_y = content.y;

    for item in items {
        match item {
            DropdownItem::Header { .. } => {
                cursor_y += style.header_height();
            }
            DropdownItem::Item { id, disabled, .. } => {
                let h = style.item_height();
                if !disabled {
                    coord.register_child(
                        parent,
                        format!("{}:item:{}", parent.0, id),
                        WidgetKind::Button,
                        Rect::new(content.x, cursor_y, content.width, h),
                        Sense::CLICK | Sense::HOVER,
                    );
                }
                cursor_y += h;
            }
            DropdownItem::Separator => {
                cursor_y += style.separator_height();
            }
            DropdownItem::Submenu { id, .. } => {
                let h = style.item_height();
                coord.register_child(
                    parent,
                    format!("{}:submenu:{}", parent.0, id),
                    WidgetKind::Button,
                    Rect::new(content.x, cursor_y, content.width, h),
                    Sense::CLICK | Sense::HOVER,
                );
                cursor_y += h;
            }
        }
    }
}

fn register_inline_hits(
    coord:    &mut InputCoordinator,
    parent:   &WidgetId,
    content:  Rect,
    options:  &[(&str, &str)],
    settings: &DropdownSettings,
) {
    let style     = settings.style.as_ref();
    let mut cursor_y = content.y;

    for (id, _label) in options {
        let h = style.item_height();
        coord.register_child(
            parent,
            format!("{}:option:{}", parent.0, id),
            WidgetKind::Button,
            Rect::new(content.x, cursor_y, content.width, h),
            Sense::CLICK | Sense::HOVER,
        );
        cursor_y += h;
    }
}

fn register_grid_hits(
    coord:    &mut InputCoordinator,
    parent:   &WidgetId,
    content:  Rect,
    items:    &[GridDropdownItem<'_>],
    columns:  usize,
    settings: &DropdownSettings,
) {
    let style   = settings.style.as_ref();
    let cell_sz = style.cell_size();
    let gap     = style.cell_gap();
    let columns = columns.max(1);

    for (i, cell) in items.iter().enumerate() {
        if cell.disabled {
            continue;
        }
        let col = i % columns;
        let row = i / columns;
        let cx  = content.x + col as f64 * (cell_sz + gap);
        let cy  = content.y + row as f64 * (cell_sz + gap);
        coord.register_child(
            parent,
            format!("{}:cell:{}", parent.0, cell.id),
            WidgetKind::Button,
            Rect::new(cx, cy, cell_sz, cell_sz),
            Sense::CLICK | Sense::HOVER,
        );
    }
}

fn register_grouped_hits(
    coord:      &mut InputCoordinator,
    parent:     &WidgetId,
    content:    Rect,
    groups:     &[DropdownGroup<'_>],
    list_items: &[CheckboxItem<'_>],
    settings:   &DropdownSettings,
) {
    let style       = settings.style.as_ref();
    let cell_sz     = style.cell_size();
    let gap         = style.cell_gap();
    let label_w     = style.row_label_width();
    let item_h      = style.item_height();
    let sep_h       = style.separator_height();

    let mut cursor_y = content.y;

    for group in groups {
        let cells_x = content.x + label_w + gap;
        for (j, cell) in group.items.iter().enumerate() {
            if cell.disabled {
                continue;
            }
            let cx = cells_x + j as f64 * (cell_sz + gap);
            coord.register_child(
                parent,
                format!("{}:cell:{}", parent.0, cell.id),
                WidgetKind::Button,
                Rect::new(cx, cursor_y, cell_sz, cell_sz),
                Sense::CLICK | Sense::HOVER,
            );
        }
        cursor_y += cell_sz + gap;
    }

    if !list_items.is_empty() {
        cursor_y += sep_h;
        for cb in list_items {
            if !cb.disabled {
                coord.register_child(
                    parent,
                    format!("{}:checkbox:{}", parent.0, cb.id),
                    WidgetKind::Button,
                    Rect::new(content.x, cursor_y, content.width, item_h),
                    Sense::CLICK | Sense::HOVER,
                );
            }
            cursor_y += item_h;
        }
    }
}
