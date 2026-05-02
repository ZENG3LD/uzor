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
use crate::types::{Rect, WidgetId, CompositeId};

use super::settings::DropdownSettings;
use super::state::DropdownState;
use super::types::{
    DropdownItem, DropdownItemRight, DropdownRenderKind, DropdownView, DropdownViewKind,
};

// ---------------------------------------------------------------------------
// Public API — measurement
// ---------------------------------------------------------------------------

/// Measure the natural size of a Flat-kind dropdown panel for the given items.
///
/// Width — widest item from the set, computed as
/// `item_padding_x + (icon? + gap) + label + middle_gap + right? + item_padding_x`,
/// clamped to at least `style.min_width()`. Right content (Shortcut / Subtitle /
/// Toggle / Checkmark / Chevron / Submenu marker) is included so dropdowns
/// auto-grow to fit secondary text.
///
/// Height — sum of per-row heights (Item / Submenu / Header / Separator) plus
/// 2× padding. If `style.max_visible_items() > 0`, the visible item count is
/// clamped to it.
pub fn measure_flat(
    items:    &[DropdownItem<'_>],
    settings: &DropdownSettings,
) -> (f64, f64) {
    let style    = settings.style.as_ref();
    let pad      = style.padding();
    let item_pad = style.item_padding_x();
    let item_h   = style.item_height();
    let header_h = style.header_height();
    let sep_h    = style.separator_height();
    let icon_sz  = style.icon_size();
    let icon_gap = style.icon_text_gap();
    let toggle_w = style.toggle_track_w();
    let max_vis  = style.max_visible_items();

    // Conservative monospace-ish text-width estimate (mlc convention).
    // Matches the same fallback used in chrome::tab_width / measure_button etc.
    const CHAR_PX: f64 = 7.0;
    // Minimum visual gap between left content (label) and right content
    // (shortcut / subtitle / toggle / chevron) so they don't touch.
    const MID_GAP: f64 = 24.0;

    let mut max_content_w = 0.0_f64;
    let mut item_count    = 0usize;
    let mut content_h     = 0.0_f64;

    for it in items {
        match it {
            DropdownItem::Item { label, icon, right, .. } => {
                if max_vis > 0 && item_count >= max_vis {
                    continue;
                }
                item_count += 1;
                content_h += item_h;

                let icon_part = if icon.is_some() { icon_sz + icon_gap } else { 0.0 };
                let label_w   = label.len() as f64 * CHAR_PX;
                let right_w   = right_content_width(right, toggle_w);
                let row_w = item_pad
                    + icon_part
                    + label_w
                    + if right_w > 0.0 { MID_GAP + right_w } else { 0.0 }
                    + item_pad;
                max_content_w = max_content_w.max(row_w);
            }
            DropdownItem::Submenu { label, .. } => {
                if max_vis > 0 && item_count >= max_vis {
                    continue;
                }
                item_count += 1;
                content_h += item_h;
                let label_w = label.len() as f64 * CHAR_PX;
                // Submenu row reserves space for the right-arrow chevron.
                let chevron_w = icon_sz; // arrow drawn at icon-size scale
                let row_w = item_pad + label_w + MID_GAP + chevron_w + item_pad;
                max_content_w = max_content_w.max(row_w);
            }
            DropdownItem::Header { label } => {
                content_h += header_h;
                let label_w = label.len() as f64 * CHAR_PX;
                max_content_w = max_content_w.max(item_pad + label_w + item_pad);
            }
            DropdownItem::Separator => content_h += sep_h,
        }
    }

    let w = max_content_w.max(style.min_width());
    let h = content_h + pad * 2.0;
    (w, h)
}

/// Width of the trailing right-side widget for an item row.
fn right_content_width(right: &DropdownItemRight<'_>, toggle_w: f64) -> f64 {
    const CHAR_PX: f64 = 7.0;
    match right {
        DropdownItemRight::None         => 0.0,
        DropdownItemRight::Shortcut(s)  => s.len() as f64 * CHAR_PX,
        DropdownItemRight::Subtitle(s)  => s.len() as f64 * CHAR_PX,
        DropdownItemRight::Toggle(_)    => toggle_w,
    }
}

// ---------------------------------------------------------------------------
// Public API — registration only
// ---------------------------------------------------------------------------

/// Register the dropdown composite and all child hit-rects with the coordinator.
///
/// No drawing occurs.  Use when explicit z-order control is needed.
///
/// Returns the [`CompositeId`] assigned to the dropdown composite.
pub fn register_input_coordinator_dropdown(
    coord:    &mut InputCoordinator,
    id:       impl Into<WidgetId>,
    rect:     Rect,
    state:    &DropdownState,
    view:     &DropdownView<'_>,
    settings: &DropdownSettings,
    kind:     DropdownRenderKind,
    layer:    &LayerId,
) -> CompositeId {
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
                register_flat_hits(coord, &dd_id, layout.content, items, settings, "item");
                if let Some((trigger_id, sub_items)) = submenu_items {
                    if state.submenu_open.as_deref() == Some(trigger_id) {
                        let sub_frame = compute_submenu_frame(frame, items, sub_items, trigger_id, settings, view.submenu_width);
                        let sub_layout = compute_layout(sub_frame, settings);
                        register_flat_hits(coord, &dd_id, sub_layout.content, sub_items, settings, "sub-item");
                    }
                }
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
/// Returns the [`CompositeId`] assigned to the dropdown composite.
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
) -> CompositeId {
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
                        let sub_frame = compute_submenu_frame(frame, items, sub_items, trigger_id, settings, view.submenu_width);
                        draw_frame(ctx, sub_frame, settings);
                        let sub_layout = compute_layout(sub_frame, settings);
                        // Prefer caller-supplied hover id if present, otherwise
                        // fall back to the auto-synced value on state.
                        let sub_hov = submenu_hovered_id
                            .or(state.submenu_hovered_id.as_deref());
                        draw_flat_list(ctx, sub_layout.content, sub_items, sub_hov, state, settings);
                    }
                }
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

/// Compute the submenu panel frame (sibling to the right of parent),
/// row-to-head: the submenu's top edge aligns with the trigger row's top.
/// Width / height come from `measure_flat(sub_items)`.
fn compute_submenu_frame(
    parent:        Rect,
    main_items:    &[DropdownItem<'_>],
    sub_items:     &[DropdownItem<'_>],
    trigger_id:    &str,
    settings:      &DropdownSettings,
    submenu_width: super::types::SubmenuWidth,
) -> Rect {
    let style    = settings.style.as_ref();
    let pad      = style.padding();
    let gap      = style.submenu_gap();

    // Walk main_items computing each row's Y until we hit the trigger.
    let mut cursor_y = parent.y + pad;
    for item in main_items {
        let h = match item {
            DropdownItem::Header { .. }     => style.header_height(),
            DropdownItem::Separator         => style.separator_height(),
            DropdownItem::Item { .. }       => style.item_height(),
            DropdownItem::Submenu { id, .. } => {
                if *id == trigger_id { break; }
                style.item_height()
            }
        };
        cursor_y += h;
    }

    let (sw, sh) = measure_flat(sub_items, settings);
    let width = match submenu_width {
        super::types::SubmenuWidth::Auto          => sw,
        super::types::SubmenuWidth::InheritParent => parent.width,
    };
    // Natural sizing — frame keeps its own padding so its first row
    // sits inside, just like the parent panel's first row sits below
    // the parent's top padding.
    Rect::new(
        parent.x + parent.width + gap,
        cursor_y - pad,
        width,
        sh,
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

            DropdownItem::Submenu { id, label, icon, .. } => {
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

                // Caret — atomic chevron, Right direction.
                use crate::ui::widgets::atomic::chevron::{
                    draw_chevron,
                    settings::ChevronSettings,
                    types::{ChevronDirection, ChevronUseCase, ChevronView, ChevronVisualKind,
                            HitAreaPolicy, PlacementPolicy, VisibilityPolicy},
                };
                let chev_size = (h * 0.6).clamp(10.0, 18.0);
                let chev_x = content.x + content.width - style.item_padding_x() - chev_size;
                let chev_y = cursor_y + (h - chev_size) / 2.0;
                let cv = ChevronView {
                    direction:   ChevronDirection::Right,
                    use_case:    ChevronUseCase::SubmenuTrigger,
                    visibility:  VisibilityPolicy::Always,
                    placement:   PlacementPolicy::InlineCorner { trailing: true },
                    hit_area:    HitAreaPolicy::None,
                    visual_kind: ChevronVisualKind::Stroked,
                    hovered, ..Default::default()
                };
                draw_chevron(ctx,
                    Rect::new(chev_x, chev_y, chev_size, chev_size),
                    &cv,
                    &ChevronSettings::default());

                cursor_y += h;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Template: Inline (split button)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------

fn register_flat_hits(
    coord:       &mut InputCoordinator,
    parent:      &CompositeId,
    content:     Rect,
    items:       &[DropdownItem<'_>],
    settings:    &DropdownSettings,
    item_prefix: &str,
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
                        format!("{}:{}:{}", parent.0.0, item_prefix, id),
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
            DropdownItem::Submenu { id, trigger, .. } => {
                let h = style.item_height();
                let row = Rect::new(content.x, cursor_y, content.width, h);
                match trigger {
                    super::types::SubmenuTrigger::Hover => {
                        // The whole row toggles submenu on hover.
                        coord.register_child(
                            parent,
                            format!("{}:submenu:{}", parent.0.0, id),
                            WidgetKind::Button,
                            row,
                            Sense::CLICK | Sense::HOVER,
                        );
                    }
                    super::types::SubmenuTrigger::ChevronClick => {
                        // Body of the row is inert; the trailing chevron
                        // is its own hit zone that opens on click.
                        let chev_w = h; // square at the end
                        let chev_rect = Rect::new(
                            content.x + content.width - chev_w,
                            cursor_y,
                            chev_w,
                            h,
                        );
                        coord.register_child(
                            parent,
                            format!("{}:submenu-chevron:{}", parent.0.0, id),
                            WidgetKind::Button,
                            chev_rect,
                            Sense::CLICK | Sense::HOVER,
                        );
                    }
                }
                cursor_y += h;
            }
        }
    }
}


// ---------------------------------------------------------------------------
// Toggle pill helper (used by Flat rows with DropdownItemRight::Toggle)
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

    let track_color = if on { theme.toggle_on() } else { theme.toggle_off() };
    ctx.set_fill_color(track_color);
    ctx.fill_rounded_rect(track_x, track_y, track_w, track_h, track_h / 2.0);

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
