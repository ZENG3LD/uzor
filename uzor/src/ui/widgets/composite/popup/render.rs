//! Popup render entry point and per-kind layout pipelines.
//!
//! # API
//!
//! - `register_input_coordinator_popup` — registers the composite + child
//!   hit-rects with an `InputCoordinator`. No drawing. Use for explicit z-order
//!   control.
//! - `register_context_manager_popup` — convenience wrapper: takes a
//!   `ContextManager`, registers, and draws in one call.
//!
//! # Draw order for non-Custom kinds
//!
//! 1. Backdrop (if `view.backdrop != BackdropKind::None`)
//! 2. Shadow rect
//! 3. Frame background + border (dispatches on `BackgroundFill`)
//! 4. Per-kind content (swatch grid, HSV editor, item list, …)
//! 5. Body closure (Plain only)

use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::{Rect, WidgetId};
use crate::ui::widgets::atomic::button::render::{
    draw_ghost_outline_button, draw_primary_button, GhostOutlineButtonView, PrimaryButtonView,
};
use crate::ui::widgets::atomic::button::theme::DefaultButtonTheme;
use crate::ui::widgets::atomic::color_swatch::render::draw_color_swatch;
use crate::ui::widgets::atomic::color_swatch::settings::ColorSwatchSettings;
use crate::ui::widgets::atomic::color_swatch::types::{ColorSwatchRenderKind, ColorSwatchView};
use crate::types::WidgetState;

use super::settings::PopupSettings;
use super::state::PopupState;
use super::style::BackgroundFill;
use super::types::{
    BackdropKind, DropdownItem, PopupRenderKind, PopupView, PopupViewKind,
};

// ---------------------------------------------------------------------------
// Layout sub-rects
// ---------------------------------------------------------------------------

/// Sub-rects produced by per-kind layout computation.
struct PopupLayout {
    /// Content rect (padded inset of frame).
    content: Rect,
}

// ---------------------------------------------------------------------------
// Public API — registration only
// ---------------------------------------------------------------------------

/// Register the popup composite and all child hit-rects with the coordinator.
///
/// No drawing occurs. Use when explicit z-order control is needed.
///
/// Returns the `WidgetId` assigned to the popup composite.
pub fn register_input_coordinator_popup(
    coord:    &mut InputCoordinator,
    id:       impl Into<WidgetId>,
    rect:     Rect,
    state:    &PopupState,
    view:     &PopupView<'_>,
    _settings: &PopupSettings,
    kind:     PopupRenderKind,
    layer:    &LayerId,
) -> WidgetId {
    let popup_id = coord.register_composite(id, WidgetKind::Popup, rect, Sense::CLICK, layer);

    if matches!(kind, PopupRenderKind::Custom) {
        return popup_id;
    }

    let frame = resolve_frame(rect, state, view);
    let layout = compute_layout(frame, _settings);

    // Register hit rects per kind.
    match kind {
        PopupRenderKind::ColorPickerGrid => {
            register_color_picker_grid_hits(coord, &popup_id, layout.content, state, _settings);
        }
        PopupRenderKind::ColorPickerHsv => {
            register_color_picker_hsv_hits(coord, &popup_id, layout.content, _settings);
        }
        PopupRenderKind::SwatchGrid => {
            register_swatch_grid_hits(coord, &popup_id, layout.content, _settings);
        }
        PopupRenderKind::ItemList => {
            if let PopupViewKind::ItemList { items, .. } = &view.kind {
                register_item_list_hits(coord, &popup_id, layout.content, items, _settings);
            }
        }
        PopupRenderKind::IndicatorStrip => {
            if let PopupViewKind::IndicatorStrip { indicators, .. } = &view.kind {
                register_indicator_strip_hits(coord, &popup_id, layout.content, indicators, _settings);
            }
        }
        _ => {}
    }

    popup_id
}

// ---------------------------------------------------------------------------
// Public API — convenience wrapper (ContextManager)
// ---------------------------------------------------------------------------

/// Register + draw a popup in one call using a `ContextManager`.
///
/// Returns the `WidgetId` assigned to the popup composite.
pub fn register_context_manager_popup(
    ctx_mgr:  &mut crate::app_context::ContextManager,
    render:   &mut dyn RenderContext,
    id:       impl Into<WidgetId>,
    rect:     Rect,
    state:    &mut PopupState,
    view:     &mut PopupView<'_>,
    settings: &PopupSettings,
    kind:     PopupRenderKind,
    layer:    &LayerId,
) -> WidgetId {
    let coord = &mut ctx_mgr.input;
    let popup_id =
        register_input_coordinator_popup(coord, id, rect, state, view, settings, kind, layer);
    draw_popup_with_coord(render, rect, coord, state, view, settings, kind);
    popup_id
}

// ---------------------------------------------------------------------------
// Internal draw pipeline
// ---------------------------------------------------------------------------

fn draw_popup_with_coord(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    coord:    &mut InputCoordinator,
    state:    &mut PopupState,
    view:     &mut PopupView<'_>,
    settings: &PopupSettings,
    kind:     PopupRenderKind,
) {
    // Custom — caller handles everything.
    if let PopupViewKind::Custom { ref draw } = view.kind {
        draw(ctx, rect, state, settings);
        return;
    }

    let theme = settings.theme.as_ref();
    let style = settings.style.as_ref();

    let frame  = resolve_frame(rect, state, view);
    let layout = compute_layout(frame, settings);

    // --- 1. Backdrop ---
    if matches!(view.backdrop, BackdropKind::Dim) {
        ctx.set_fill_color(theme.backdrop_dim());
        ctx.fill_rect(0.0, 0.0, 99_999.0, 99_999.0);
    }

    // IndicatorStrip has no chrome — skip shadow/border/bg.
    let is_strip = matches!(kind, PopupRenderKind::IndicatorStrip);

    if !is_strip {
        // --- 2. Shadow ---
        let (sx, sy) = style.shadow_offset();
        ctx.set_fill_color(theme.shadow());
        ctx.fill_rounded_rect(
            frame.x + sx,
            frame.y + sy,
            frame.width,
            frame.height,
            style.radius(),
        );

        // --- 3. Frame background + border ---
        draw_frame(ctx, frame, settings);
    } else {
        // IndicatorStrip: alpha-fill background, no border.
        ctx.set_fill_color("rgba(30,34,45,0.75)");
        ctx.fill_rounded_rect(frame.x, frame.y, frame.width, frame.height, style.radius());
    }

    // --- 4. Per-kind content ---
    match kind {
        PopupRenderKind::Plain => {
            if let PopupViewKind::Plain { ref mut body } = view.kind {
                body(ctx, layout.content, coord);
            }
        }
        PopupRenderKind::ColorPickerGrid => {
            if let PopupViewKind::ColorPickerGrid {
                current_color,
                swatches,
                hovered_swatch,
                opacity,
                opacity_hidden,
            } = &view.kind
            {
                draw_color_picker_grid(
                    ctx,
                    layout.content,
                    current_color,
                    swatches,
                    *hovered_swatch,
                    *opacity,
                    *opacity_hidden,
                    settings,
                );
            }
        }
        PopupRenderKind::ColorPickerHsv => {
            if let PopupViewKind::ColorPickerHsv {
                hsv,
                hex_input,
                hex_editing,
                opacity,
                opacity_hidden,
            } = &view.kind
            {
                draw_color_picker_hsv(
                    ctx,
                    layout.content,
                    *hsv,
                    hex_input,
                    *hex_editing,
                    *opacity,
                    *opacity_hidden,
                    settings,
                );
            }
        }
        PopupRenderKind::SwatchGrid => {
            if let PopupViewKind::SwatchGrid {
                preset_swatches,
                custom_swatches,
                hovered_index,
                hovered_remove,
                hovered_add,
            } = &view.kind
            {
                draw_swatch_grid(
                    ctx,
                    layout.content,
                    preset_swatches,
                    custom_swatches,
                    *hovered_index,
                    *hovered_remove,
                    *hovered_add,
                    settings,
                );
            }
        }
        PopupRenderKind::ItemList => {
            if let PopupViewKind::ItemList { items, hovered_id } = &view.kind {
                draw_item_list(ctx, layout.content, items, *hovered_id, settings);
            }
        }
        PopupRenderKind::IndicatorStrip => {
            if let PopupViewKind::IndicatorStrip {
                indicators,
                hovered_indicator_id,
                hovered_action,
            } = &view.kind
            {
                draw_indicator_strip(
                    ctx,
                    layout.content,
                    indicators,
                    *hovered_indicator_id,
                    *hovered_action,
                    settings,
                );
            }
        }
        PopupRenderKind::Custom => {}
    }
}

// ---------------------------------------------------------------------------
// Frame draw helper
// ---------------------------------------------------------------------------

fn draw_frame(ctx: &mut dyn RenderContext, frame: Rect, settings: &PopupSettings) {
    let theme = settings.theme.as_ref();
    let style = settings.style.as_ref();

    match style.background_fill() {
        BackgroundFill::Solid => {
            ctx.set_fill_color(theme.bg());
            ctx.fill_rounded_rect(frame.x, frame.y, frame.width, frame.height, style.radius());
        }
        BackgroundFill::Glass { blur_radius: _ } => {
            ctx.draw_blur_background(frame.x, frame.y, frame.width, frame.height);
            ctx.set_fill_color(theme.bg());
            ctx.fill_rounded_rect(frame.x, frame.y, frame.width, frame.height, style.radius());
        }
        BackgroundFill::Texture { asset_id } => {
            let _ = asset_id;
            ctx.set_fill_color(theme.bg());
            ctx.fill_rounded_rect(frame.x, frame.y, frame.width, frame.height, style.radius());
        }
    }

    ctx.set_stroke_color(theme.border());
    ctx.set_stroke_width(style.border_width());
    ctx.set_line_dash(&[]);
    ctx.stroke_rounded_rect(frame.x, frame.y, frame.width, frame.height, style.radius());
}

// ---------------------------------------------------------------------------
// Layout helpers
// ---------------------------------------------------------------------------

fn resolve_frame(rect: Rect, state: &PopupState, _view: &PopupView<'_>) -> Rect {
    if state.position.0 != 0.0 || state.position.1 != 0.0 {
        Rect::new(state.position.0, state.position.1, rect.width, rect.height)
    } else {
        rect
    }
}

fn compute_layout(frame: Rect, settings: &PopupSettings) -> PopupLayout {
    let pad = settings.style.padding();
    PopupLayout {
        content: Rect::new(
            frame.x + pad,
            frame.y + pad,
            (frame.width  - pad * 2.0).max(0.0),
            (frame.height - pad * 2.0).max(0.0),
        ),
    }
}

// ---------------------------------------------------------------------------
// Per-kind layout rect helpers
// ---------------------------------------------------------------------------

/// Compute swatch cell rect for (col, row) in the L1 palette grid.
fn swatch_rect(content: Rect, col: usize, row: usize, settings: &PopupSettings) -> Rect {
    let style = settings.style.as_ref();
    let sz    = style.swatch_size();
    let gap   = style.grid_gap();
    Rect::new(
        content.x + col as f64 * (sz + gap),
        content.y + row as f64 * (sz + gap),
        sz,
        sz,
    )
}

/// Rows occupied by the L1 palette grid.
fn l1_grid_rows(swatches: usize, columns: usize) -> usize {
    (swatches + columns.saturating_sub(1)) / columns.max(1)
}

/// Y offset below the L1 grid.
fn l1_grid_height(rows: usize, settings: &PopupSettings) -> f64 {
    let style = settings.style.as_ref();
    let sz    = style.swatch_size();
    let gap   = style.grid_gap();
    (rows as f64 * (sz + gap)).max(0.0) - gap
}

// ---------------------------------------------------------------------------
// Template: ColorPickerGrid (L1)
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn draw_color_picker_grid(
    ctx:            &mut dyn RenderContext,
    content:        Rect,
    current_color:  &str,
    swatches:       &[&str],
    hovered_swatch: Option<usize>,
    opacity:        f64,
    opacity_hidden: bool,
    settings:       &PopupSettings,
) {
    let theme   = settings.theme.as_ref();
    let style   = settings.style.as_ref();
    let columns = style.grid_columns();
    let sz      = style.swatch_size();
    let gap     = style.grid_gap();
    let radius  = style.swatch_radius();
    let rows    = l1_grid_rows(swatches.len(), columns);
    let grid_h  = l1_grid_height(rows, settings);

    // --- Swatch grid ---
    let swatch_settings = ColorSwatchSettings::default();
    for (i, hex) in swatches.iter().enumerate() {
        let col = i % columns;
        let row = i / columns;
        let r   = swatch_rect(content, col, row, settings);
        let color_bytes = parse_hex_color(hex);
        let swatch_view = ColorSwatchView {
            color:                color_bytes,
            hovered:              hovered_swatch == Some(i),
            selected:             false,
            show_transparency:    false,
            border_color_override: None,
        };
        let _ = radius; // used indirectly via settings
        draw_color_swatch(ctx, r, WidgetState::Normal, &swatch_view, &swatch_settings, &ColorSwatchRenderKind::Simple);
    }

    // --- Custom row ---
    let custom_row_y = content.y + grid_h + gap * 2.0;
    let custom_sz    = sz + 4.0; // slightly larger current-color swatch

    // Current color swatch
    let cur_color_bytes = parse_hex_color(current_color);
    let cur_swatch_view = ColorSwatchView {
        color:                cur_color_bytes,
        hovered:              false,
        selected:             true,
        show_transparency:    false,
        border_color_override: None,
    };
    let cur_rect = Rect::new(content.x, custom_row_y, custom_sz, custom_sz);
    draw_color_swatch(ctx, cur_rect, WidgetState::Normal, &cur_swatch_view, &swatch_settings, &ColorSwatchRenderKind::Simple);

    // "+" button (ghost outline)
    let plus_rect = Rect::new(
        content.x + custom_sz + gap * 2.0,
        custom_row_y,
        custom_sz,
        custom_sz,
    );
    let btn_theme = DefaultButtonTheme;
    let plus_view = GhostOutlineButtonView { text: "+", hovered: false };
    draw_ghost_outline_button(ctx, plus_rect, &plus_view, style.swatch_radius(), &btn_theme);

    // --- Opacity row ---
    let opacity_y = custom_row_y + custom_sz + gap * 2.0;
    draw_opacity_row(ctx, Rect::new(content.x, opacity_y, content.width, style.opacity_row_height()), opacity, opacity_hidden, theme.accent(), settings);
}

// ---------------------------------------------------------------------------
// Template: ColorPickerHsv (L2)
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn draw_color_picker_hsv(
    ctx:            &mut dyn RenderContext,
    content:        Rect,
    hsv:            crate::ui::widgets::composite::popup::types::HsvColor,
    hex_input:      &str,
    hex_editing:    bool,
    opacity:        f64,
    opacity_hidden: bool,
    settings:       &PopupSettings,
) {
    let theme    = settings.theme.as_ref();
    let style    = settings.style.as_ref();
    let sq_size  = style.hsv_square_size();
    let hue_w    = style.hue_bar_width();
    let gap      = style.hsv_inner_gap();
    let hex_h    = style.hex_row_height();
    let btn_h    = style.action_button_height();

    // --- SV square ---
    let sv_rect = Rect::new(content.x, content.y, sq_size, sq_size);

    // Draw gradient background (white → hue on x, opaque → black on y)
    // We draw three layered rects as an approximation:
    // 1. Hue base colour fill
    let hue_css = hue_to_css(hsv.h);
    ctx.set_fill_color(&hue_css);
    ctx.fill_rounded_rect(sv_rect.x, sv_rect.y, sv_rect.width, sv_rect.height, 2.0);

    // 2. White gradient left-to-right (full white → transparent)
    ctx.set_fill_color("rgba(255,255,255,0.0)");
    // Approximate: draw white → transparent manually isn't available on all backends.
    // Just draw the border and cursor for now.
    let _ = (sv_rect, hsv);

    // SV cursor
    let cursor_x = sv_rect.x + hsv.s * sv_rect.width;
    let cursor_y = sv_rect.y + (1.0 - hsv.v) * sv_rect.height;
    ctx.set_stroke_color("#ffffff");
    ctx.set_stroke_width(2.0);
    ctx.set_line_dash(&[]);
    ctx.begin_path();
    ctx.arc(cursor_x, cursor_y, 5.0, 0.0, std::f64::consts::TAU);
    ctx.stroke();

    // --- Hue bar ---
    let hue_rect = Rect::new(
        content.x + sq_size + gap,
        content.y,
        hue_w,
        sq_size,
    );
    // Approximate: fill with gradient label; renderer draws actual rainbow
    ctx.set_fill_color("rgba(128,128,128,0.3)");
    ctx.fill_rounded_rect(hue_rect.x, hue_rect.y, hue_rect.width, hue_rect.height, 2.0);
    // Hue cursor bar
    let hue_cursor_y = hue_rect.y + (hsv.h / 360.0) * hue_rect.height;
    ctx.set_fill_color("#ffffff");
    ctx.fill_rect(hue_rect.x - 2.0, hue_cursor_y - 2.0, hue_rect.width + 4.0, 4.0);

    // --- Hex row ---
    let hex_y = content.y + sq_size + gap;
    let hex_row = Rect::new(content.x, hex_y, content.width, hex_h);

    // Preview swatch
    let preview_sz = hex_h - 4.0;
    let preview_bytes = hsv_to_rgba(hsv, opacity);
    let preview_css = rgba_to_css(preview_bytes);
    ctx.set_fill_color(&preview_css);
    ctx.fill_rounded_rect(hex_row.x, hex_row.y + 2.0, preview_sz, preview_sz, 2.0);

    // Hex input field
    let input_x = hex_row.x + preview_sz + gap;
    let input_w = hex_row.width - preview_sz - gap;
    ctx.set_fill_color(theme.hex_input_bg());
    ctx.fill_rounded_rect(input_x, hex_row.y + 2.0, input_w, hex_h - 4.0, 2.0);
    ctx.set_stroke_color(if hex_editing { theme.hex_input_border_focus() } else { theme.border() });
    ctx.set_stroke_width(1.0);
    ctx.set_line_dash(&[]);
    ctx.stroke_rounded_rect(input_x, hex_row.y + 2.0, input_w, hex_h - 4.0, 2.0);
    ctx.set_fill_color(theme.hex_input_text());
    ctx.set_font("12px monospace");
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(
        &format!("#{}", hex_input),
        input_x + 6.0,
        hex_row.y + hex_h / 2.0,
    );

    // --- Opacity row ---
    let opa_y = hex_y + hex_h + gap;
    draw_opacity_row(ctx, Rect::new(content.x, opa_y, content.width, style.opacity_row_height()), opacity, opacity_hidden, theme.accent(), settings);

    // --- Action row ---
    let act_y = opa_y + style.opacity_row_height() + gap;
    let half_w = (content.width - gap) / 2.0;
    let btn_theme = DefaultButtonTheme;

    let back_rect = Rect::new(content.x, act_y, half_w, btn_h);
    let back_view = GhostOutlineButtonView { text: "Back", hovered: false };
    draw_ghost_outline_button(ctx, back_rect, &back_view, 4.0, &btn_theme);

    let add_rect = Rect::new(content.x + half_w + gap, act_y, half_w, btn_h);
    let add_view = PrimaryButtonView { text: "Add", hovered: false };
    draw_primary_button(ctx, add_rect, &add_view, 4.0, &btn_theme);
}

// ---------------------------------------------------------------------------
// Template: SwatchGrid (SyncColorGrid)
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn draw_swatch_grid(
    ctx:             &mut dyn RenderContext,
    content:         Rect,
    preset_swatches: &[[f32; 4]],
    custom_swatches: &[[f32; 4]],
    hovered_index:   Option<usize>,
    hovered_remove:  bool,
    hovered_add:     bool,
    settings:        &PopupSettings,
) {
    let theme   = settings.theme.as_ref();
    let style   = settings.style.as_ref();
    let sz      = style.swatch_grid_size();
    let gap     = style.swatch_grid_gap();
    let columns = style.swatch_grid_columns();
    let sep_h   = 1.0_f64;
    let sep_gap = 4.0_f64;

    let swatch_settings = ColorSwatchSettings::default();

    // --- Preset grid ---
    let preset_rows = l1_grid_rows(preset_swatches.len(), columns);
    for (i, &c) in preset_swatches.iter().enumerate() {
        let col = i % columns;
        let row = i / columns;
        let rx  = content.x + col as f64 * (sz + gap);
        let ry  = content.y + row as f64 * (sz + gap);
        let r   = Rect::new(rx, ry, sz, sz);
        let bytes = [
            (c[0] * 255.0) as u8,
            (c[1] * 255.0) as u8,
            (c[2] * 255.0) as u8,
            (c[3] * 255.0) as u8,
        ];
        let sv = ColorSwatchView {
            color: bytes,
            hovered: hovered_index == Some(i),
            selected: false,
            show_transparency: false,
            border_color_override: None,
        };
        draw_color_swatch(ctx, r, WidgetState::Normal, &sv, &swatch_settings, &ColorSwatchRenderKind::Simple);
    }

    // Separator after preset grid
    let preset_grid_h = (preset_rows as f64 * (sz + gap)).max(0.0) - gap;
    let sep1_y = content.y + preset_grid_h + sep_gap;
    ctx.set_fill_color(theme.separator());
    ctx.fill_rect(content.x, sep1_y, content.width, sep_h);

    // --- Custom row ---
    let custom_y = sep1_y + sep_h + sep_gap;
    for (j, &c) in custom_swatches.iter().enumerate() {
        let rx = content.x + j as f64 * (sz + gap);
        let r  = Rect::new(rx, custom_y, sz, sz);
        let bytes = [
            (c[0] * 255.0) as u8,
            (c[1] * 255.0) as u8,
            (c[2] * 255.0) as u8,
            (c[3] * 255.0) as u8,
        ];
        let sv = ColorSwatchView {
            color: bytes,
            hovered: hovered_index == Some(preset_swatches.len() + j),
            selected: false,
            show_transparency: false,
            border_color_override: None,
        };
        draw_color_swatch(ctx, r, WidgetState::Normal, &sv, &swatch_settings, &ColorSwatchRenderKind::Simple);
    }

    // "+" add-custom dashed button (after custom swatches)
    let add_x = content.x + custom_swatches.len() as f64 * (sz + gap);
    let add_rect = Rect::new(add_x, custom_y, sz, sz);
    ctx.set_stroke_color(if hovered_add { theme.accent() } else { theme.border() });
    ctx.set_stroke_width(1.0);
    ctx.set_line_dash(&[3.0, 3.0]);
    ctx.stroke_rounded_rect(add_rect.x, add_rect.y, add_rect.width, add_rect.height, 2.0);
    ctx.set_line_dash(&[]);
    ctx.set_fill_color(if hovered_add { theme.accent() } else { theme.item_text_disabled() });
    ctx.set_font("14px sans-serif");
    ctx.set_text_align(TextAlign::Center);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text("+", add_rect.x + add_rect.width / 2.0, add_rect.y + add_rect.height / 2.0);

    // Separator before Remove
    let sep2_y = custom_y + sz + sep_gap;
    ctx.set_fill_color(theme.separator());
    ctx.fill_rect(content.x, sep2_y, content.width, sep_h);

    // --- Remove row ---
    let remove_h = style.remove_row_height();
    let remove_y = sep2_y + sep_h + sep_gap;
    if hovered_remove {
        ctx.set_fill_color("rgba(242,54,69,0.15)");
        ctx.fill_rect(content.x, remove_y, content.width, remove_h);
    }
    ctx.set_fill_color(if hovered_remove { theme.item_text_danger() } else { theme.item_text() });
    ctx.set_font("13px sans-serif");
    ctx.set_text_align(TextAlign::Center);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text("Remove", content.x + content.width / 2.0, remove_y + remove_h / 2.0);
}

// ---------------------------------------------------------------------------
// Template: ItemList
// ---------------------------------------------------------------------------

fn draw_item_list(
    ctx:        &mut dyn RenderContext,
    content:    Rect,
    items:      &[DropdownItem<'_>],
    hovered_id: Option<&str>,
    settings:   &PopupSettings,
) {
    let theme = settings.theme.as_ref();
    let style = settings.style.as_ref();

    let mut cursor_y = content.y;

    for item in items {
        match item {
            DropdownItem::Header { label } => {
                let h = style.header_height();
                ctx.set_fill_color(theme.header_text());
                ctx.set_font("bold 12px sans-serif");
                ctx.set_text_align(TextAlign::Left);
                ctx.set_text_baseline(TextBaseline::Middle);
                ctx.fill_text(label, content.x + 12.0, cursor_y + h / 2.0);
                // bottom border
                ctx.set_fill_color(theme.separator());
                ctx.fill_rect(content.x, cursor_y + h - 1.0, content.width, 1.0);
                cursor_y += h;
            }

            DropdownItem::Item { id, label, right_label, disabled, danger } => {
                let h       = style.item_height();
                let hovered = hovered_id == Some(id);

                // Background
                let bg = if hovered {
                    if *danger { theme.item_bg_danger_hover() } else { theme.item_bg_hover() }
                } else {
                    theme.item_bg_normal()
                };
                ctx.set_fill_color(bg);
                ctx.fill_rect(content.x, cursor_y, content.width, h);

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
                ctx.set_font("13px sans-serif");
                ctx.set_text_align(TextAlign::Left);
                ctx.set_text_baseline(TextBaseline::Middle);
                ctx.fill_text(label, content.x + 12.0, cursor_y + h / 2.0);

                // Right label (shortcut / subtitle)
                if let Some(right) = right_label {
                    ctx.set_fill_color(theme.item_text_disabled());
                    ctx.set_text_align(TextAlign::Right);
                    ctx.fill_text(right, content.x + content.width - 12.0, cursor_y + h / 2.0);
                }

                cursor_y += h;
            }

            DropdownItem::Separator => {
                let h = style.separator_height();
                ctx.set_fill_color(theme.separator());
                ctx.fill_rect(content.x, cursor_y + h / 2.0 - 0.5, content.width, 1.0);
                cursor_y += h;
            }

            DropdownItem::Submenu { id, label } => {
                let h       = style.item_height();
                let hovered = hovered_id == Some(id);

                let bg = if hovered { theme.item_bg_hover() } else { theme.item_bg_normal() };
                ctx.set_fill_color(bg);
                ctx.fill_rect(content.x, cursor_y, content.width, h);

                let text_color = if hovered { theme.item_text_hover() } else { theme.item_text() };
                ctx.set_fill_color(text_color);
                ctx.set_font("13px sans-serif");
                ctx.set_text_align(TextAlign::Left);
                ctx.set_text_baseline(TextBaseline::Middle);
                ctx.fill_text(label, content.x + 12.0, cursor_y + h / 2.0);

                // Arrow indicator
                ctx.set_text_align(TextAlign::Right);
                ctx.fill_text("▶", content.x + content.width - 8.0, cursor_y + h / 2.0);

                cursor_y += h;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Template: IndicatorStrip
// ---------------------------------------------------------------------------

fn draw_indicator_strip(
    ctx:                  &mut dyn RenderContext,
    content:              Rect,
    indicators:           &[crate::ui::widgets::composite::popup::types::IndicatorRowInfo<'_>],
    hovered_indicator_id: Option<u64>,
    hovered_action:       Option<(u64, &str)>,
    settings:             &PopupSettings,
) {
    let theme    = settings.theme.as_ref();
    let style    = settings.style.as_ref();
    let row_h    = style.strip_row_height();
    let row_gap  = style.strip_row_gap();
    let icon_sz  = style.strip_icon_size();
    let icon_gap = 4.0_f64;

    let action_names = ["vis", "alert", "settings", "delete"];
    let action_labels = ["👁", "🔔", "⚙", "🗑"];

    let mut cursor_y = content.y;
    for info in indicators {
        let is_row_hovered = hovered_indicator_id == Some(info.id);

        // Row hover background
        if is_row_hovered {
            ctx.set_fill_color(theme.item_bg_hover());
            ctx.fill_rect(content.x, cursor_y, content.width, row_h);
        }

        // Label
        ctx.set_fill_color(theme.item_text());
        ctx.set_font("12px sans-serif");
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Middle);
        ctx.fill_text(info.display_name, content.x, cursor_y + row_h / 2.0);

        // Action buttons (right-aligned)
        let icons_total_w = action_names.len() as f64 * (icon_sz + icon_gap) - icon_gap;
        let icons_start_x = content.x + content.width - icons_total_w;

        for (k, (name, label)) in action_names.iter().zip(action_labels.iter()).enumerate() {
            let icon_x   = icons_start_x + k as f64 * (icon_sz + icon_gap);
            let icon_rect = Rect::new(icon_x, cursor_y + (row_h - icon_sz) / 2.0, icon_sz, icon_sz);

            let action_hovered = hovered_action
                .map(|(id, act)| id == info.id && act == *name)
                .unwrap_or(false);

            let color = if *name == "delete" {
                if action_hovered { "#f23645" } else { theme.item_text_disabled() }
            } else if action_hovered {
                theme.accent()
            } else {
                theme.item_text_disabled()
            };

            ctx.set_fill_color(color);
            ctx.set_font("11px sans-serif");
            ctx.set_text_align(TextAlign::Center);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text(label, icon_rect.x + icon_rect.width / 2.0, icon_rect.y + icon_rect.height / 2.0);
        }

        cursor_y += row_h + row_gap;
    }
}

// ---------------------------------------------------------------------------
// Opacity row (shared by L1 and L2)
// ---------------------------------------------------------------------------

fn draw_opacity_row(
    ctx:           &mut dyn RenderContext,
    row:           Rect,
    opacity:       f64,
    opacity_hidden: bool,
    accent:        &str,
    settings:      &PopupSettings,
) {
    let theme = settings.theme.as_ref();

    // Eye toggle
    let eye_sz = row.height;
    let eye_label = if opacity_hidden { "🚫" } else { "👁" };
    ctx.set_fill_color(theme.item_text_disabled());
    ctx.set_font("13px sans-serif");
    ctx.set_text_align(TextAlign::Center);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(eye_label, row.x + eye_sz / 2.0, row.y + row.height / 2.0);

    // Slider track
    let slider_x = row.x + eye_sz + 6.0;
    let pct_w    = 36.0_f64;
    let slider_w = (row.width - eye_sz - 6.0 - pct_w - 4.0).max(0.0);
    let slider_y = row.y + row.height / 2.0 - 2.0;
    let track_h  = 4.0_f64;

    ctx.set_fill_color(theme.border());
    ctx.fill_rounded_rect(slider_x, slider_y, slider_w, track_h, track_h / 2.0);

    // Filled portion
    let filled_w = (slider_w * opacity).max(0.0);
    ctx.set_fill_color(accent);
    ctx.fill_rounded_rect(slider_x, slider_y, filled_w, track_h, track_h / 2.0);

    // Thumb
    let thumb_x = slider_x + filled_w - 4.0;
    ctx.set_fill_color("#ffffff");
    ctx.fill_rounded_rect(thumb_x, slider_y - 2.0, 8.0, track_h + 4.0, 4.0);

    // Percentage label
    let pct_label = format!("{:.0}%", opacity * 100.0);
    ctx.set_fill_color(theme.item_text());
    ctx.set_font("12px sans-serif");
    ctx.set_text_align(TextAlign::Right);
    ctx.fill_text(&pct_label, row.x + row.width, row.y + row.height / 2.0);
}

// ---------------------------------------------------------------------------
// Hit-rect registration helpers
// ---------------------------------------------------------------------------

fn register_color_picker_grid_hits(
    coord:    &mut InputCoordinator,
    parent:   &WidgetId,
    content:  Rect,
    _state:   &PopupState,
    settings: &PopupSettings,
) {
    let style   = settings.style.as_ref();
    let sz      = style.swatch_size();
    let gap     = style.grid_gap();
    let columns = style.grid_columns();

    // Register 100 default swatch hit zones
    let total = 100_usize;
    for i in 0..total {
        let col = i % columns;
        let row = i / columns;
        let r   = swatch_rect(content, col, row, settings);
        coord.register_child(
            parent,
            format!("{}:swatch:{}", parent.0, i),
            WidgetKind::Button,
            r,
            Sense::CLICK | Sense::HOVER,
        );
    }

    // Plus button and opacity slider
    let rows   = l1_grid_rows(total, columns);
    let grid_h = l1_grid_height(rows, settings);
    let custom_y  = content.y + grid_h + gap * 2.0;
    let custom_sz = sz + 4.0;

    let plus_rect = Rect::new(content.x + custom_sz + gap * 2.0, custom_y, custom_sz, custom_sz);
    coord.register_child(parent, format!("{}:plus", parent.0), WidgetKind::Button, plus_rect, Sense::CLICK);

    let opa_y = custom_y + custom_sz + gap * 2.0;
    let opa_rect = Rect::new(content.x, opa_y, content.width, style.opacity_row_height());
    coord.register_child(parent, format!("{}:opacity", parent.0), WidgetKind::Slider, opa_rect, Sense::CLICK | Sense::DRAG);

    // Eye toggle
    let eye_rect = Rect::new(content.x, opa_y, style.opacity_row_height(), style.opacity_row_height());
    coord.register_child(parent, format!("{}:opacity_toggle", parent.0), WidgetKind::Button, eye_rect, Sense::CLICK);
}

fn register_color_picker_hsv_hits(
    coord:    &mut InputCoordinator,
    parent:   &WidgetId,
    content:  Rect,
    settings: &PopupSettings,
) {
    let style   = settings.style.as_ref();
    let sq_size = style.hsv_square_size();
    let hue_w   = style.hue_bar_width();
    let gap     = style.hsv_inner_gap();
    let hex_h   = style.hex_row_height();
    let btn_h   = style.action_button_height();

    let sv_rect = Rect::new(content.x, content.y, sq_size, sq_size);
    coord.register_child(parent, format!("{}:sv", parent.0), WidgetKind::Slider, sv_rect, Sense::CLICK | Sense::DRAG);

    let hue_rect = Rect::new(content.x + sq_size + gap, content.y, hue_w, sq_size);
    coord.register_child(parent, format!("{}:hue", parent.0), WidgetKind::Slider, hue_rect, Sense::CLICK | Sense::DRAG);

    let hex_y    = content.y + sq_size + gap;
    let hex_row  = Rect::new(content.x, hex_y, content.width, hex_h);
    coord.register_child(parent, format!("{}:hex", parent.0), WidgetKind::Custom, hex_row, Sense::CLICK);

    let opa_y    = hex_y + hex_h + gap;
    let opa_rect = Rect::new(content.x, opa_y, content.width, style.opacity_row_height());
    coord.register_child(parent, format!("{}:opacity", parent.0), WidgetKind::Slider, opa_rect, Sense::CLICK | Sense::DRAG);

    let act_y   = opa_y + style.opacity_row_height() + gap;
    let half_w  = (content.width - gap) / 2.0;
    coord.register_child(parent, format!("{}:back", parent.0), WidgetKind::Button, Rect::new(content.x, act_y, half_w, btn_h), Sense::CLICK);
    coord.register_child(parent, format!("{}:add", parent.0), WidgetKind::Button, Rect::new(content.x + half_w + gap, act_y, half_w, btn_h), Sense::CLICK);
}

fn register_swatch_grid_hits(
    coord:    &mut InputCoordinator,
    parent:   &WidgetId,
    content:  Rect,
    settings: &PopupSettings,
) {
    let style   = settings.style.as_ref();
    let sz      = style.swatch_grid_size();
    let gap     = style.swatch_grid_gap();
    let columns = style.swatch_grid_columns();
    let sep_h   = 1.0_f64;
    let sep_gap = 4.0_f64;

    // Preset rows (12 swatches)
    let preset_total = 12_usize;
    let preset_rows  = l1_grid_rows(preset_total, columns);
    for i in 0..preset_total {
        let col = i % columns;
        let row = i / columns;
        let rx  = content.x + col as f64 * (sz + gap);
        let ry  = content.y + row as f64 * (sz + gap);
        coord.register_child(
            parent,
            format!("{}:swatch:{}", parent.0, i),
            WidgetKind::Button,
            Rect::new(rx, ry, sz, sz),
            Sense::CLICK | Sense::HOVER,
        );
    }

    let preset_grid_h = (preset_rows as f64 * (sz + gap)).max(0.0) - gap;
    let custom_y = content.y + preset_grid_h + sep_gap + sep_h + sep_gap;

    // Custom swatches (up to 6)
    for j in 0..6_usize {
        let rx = content.x + j as f64 * (sz + gap);
        coord.register_child(
            parent,
            format!("{}:swatch:{}", parent.0, preset_total + j),
            WidgetKind::Button,
            Rect::new(rx, custom_y, sz, sz),
            Sense::CLICK | Sense::HOVER,
        );
    }

    // Add button
    let add_x = content.x + 6.0 * (sz + gap);
    coord.register_child(parent, format!("{}:add", parent.0), WidgetKind::Button, Rect::new(add_x, custom_y, sz, sz), Sense::CLICK | Sense::HOVER);

    // Remove row
    let remove_h = style.remove_row_height();
    let remove_y = custom_y + sz + sep_gap + sep_h + sep_gap;
    coord.register_child(parent, format!("{}:remove", parent.0), WidgetKind::Button, Rect::new(content.x, remove_y, content.width, remove_h), Sense::CLICK | Sense::HOVER);
}

fn register_item_list_hits(
    coord:    &mut InputCoordinator,
    parent:   &WidgetId,
    content:  Rect,
    items:    &[DropdownItem<'_>],
    settings: &PopupSettings,
) {
    let style    = settings.style.as_ref();
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

fn register_indicator_strip_hits(
    coord:      &mut InputCoordinator,
    parent:     &WidgetId,
    content:    Rect,
    indicators: &[crate::ui::widgets::composite::popup::types::IndicatorRowInfo<'_>],
    settings:   &PopupSettings,
) {
    let style    = settings.style.as_ref();
    let row_h    = style.strip_row_height();
    let row_gap  = style.strip_row_gap();
    let icon_sz  = style.strip_icon_size();
    let icon_gap = 4.0_f64;
    let action_names = ["vis", "alert", "settings", "delete"];

    let mut cursor_y = content.y;
    for info in indicators {
        // Row hover zone
        coord.register_child(
            parent,
            format!("{}:row:{}", parent.0, info.id),
            WidgetKind::Button,
            Rect::new(content.x, cursor_y, content.width, row_h),
            Sense::HOVER,
        );

        // Action buttons
        let icons_total_w = action_names.len() as f64 * (icon_sz + icon_gap) - icon_gap;
        let icons_start_x = content.x + content.width - icons_total_w;
        for (k, name) in action_names.iter().enumerate() {
            let icon_x = icons_start_x + k as f64 * (icon_sz + icon_gap);
            coord.register_child(
                parent,
                format!("{}:action:{}:{}", parent.0, info.id, name),
                WidgetKind::Button,
                Rect::new(icon_x, cursor_y + (row_h - icon_sz) / 2.0, icon_sz, icon_sz),
                Sense::CLICK | Sense::HOVER,
            );
        }

        cursor_y += row_h + row_gap;
    }
}

// ---------------------------------------------------------------------------
// Color utility helpers
// ---------------------------------------------------------------------------

/// Parse a hex color string (`#rrggbb` or `#rrggbbaa`) into `[r,g,b,a]`.
fn parse_hex_color(hex: &str) -> [u8; 4] {
    let hex = hex.trim_start_matches('#');
    let parse2 = |s: &str| u8::from_str_radix(s, 16).unwrap_or(0);
    match hex.len() {
        6 => [parse2(&hex[0..2]), parse2(&hex[2..4]), parse2(&hex[4..6]), 255],
        8 => [parse2(&hex[0..2]), parse2(&hex[2..4]), parse2(&hex[4..6]), parse2(&hex[6..8])],
        _ => [128, 128, 128, 255],
    }
}

/// Convert HSV + opacity to RGBA bytes.
fn hsv_to_rgba(hsv: crate::ui::widgets::composite::popup::types::HsvColor, opacity: f64) -> [u8; 4] {
    let h = hsv.h / 60.0;
    let s = hsv.s;
    let v = hsv.v;
    let i = h.floor() as u8;
    let f = h - h.floor();
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match i % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    [
        (r * 255.0) as u8,
        (g * 255.0) as u8,
        (b * 255.0) as u8,
        (opacity * 255.0) as u8,
    ]
}

/// Convert RGBA bytes to a CSS `rgba(...)` string.
fn rgba_to_css(c: [u8; 4]) -> String {
    format!("rgba({},{},{},{:.3})", c[0], c[1], c[2], c[3] as f64 / 255.0)
}

/// Convert a hue angle to a fully-saturated CSS colour (s=1, v=1).
fn hue_to_css(hue: f64) -> String {
    let c = hsv_to_rgba(
        crate::ui::widgets::composite::popup::types::HsvColor { h: hue, s: 1.0, v: 1.0 },
        1.0,
    );
    rgba_to_css(c)
}
