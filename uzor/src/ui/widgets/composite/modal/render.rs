//! Modal render entry point and per-kind layout pipelines.
//!
//! # API convention
//!
//! All composite widgets expose three functions:
//!
//! - `register_modal`  — registers the composite + all child hit-rects with the
//!   `InputCoordinator`.  **No drawing.**
//! - `draw_modal`      — pure rendering.  **No registration.**  The caller must
//!   have already called `register_modal` (or done equivalent registration).
//! - `modal`           — convenience wrapper that calls both in order.
//!
//! # Draw order for every non-Custom kind
//!
//! 1. Backdrop (if `view.backdrop` != `BackdropKind::None`)
//! 2. Shadow rect (offset by `style.shadow_offset()`)
//! 3. Frame background + border  (dispatches on `style.background_fill()`)
//! 4. Header strip + title + close-X + drag-zone (DragHandle)  (if kind has header)
//! 5. Tab strip — horizontal (`TopTabs`) or vertical sidebar (`SideTabs`)
//! 6. Dividers (header bottom, footer top, sidebar right)
//! 7. `(view.body)(ctx, body_rect, coord)`
//! 8. Footer buttons                                             (if kind has footer)
//! 9. Wizard nav (page dots + Back / Next)                       (Wizard only)

use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::{Rect, WidgetId, WidgetState};
use crate::ui::widgets::atomic::button::render::{
    draw_danger_button, draw_ghost_outline_button, draw_primary_button, DangerButtonView,
    DangerVariant, GhostOutlineButtonView, PrimaryButtonView,
};
use crate::ui::widgets::atomic::button::theme::DefaultButtonTheme;
use crate::ui::widgets::atomic::close_button::render::{draw_close_button, CloseButtonView};
use crate::ui::widgets::atomic::close_button::settings::CloseButtonSettings;
use crate::ui::widgets::atomic::close_button::style::DefaultCloseButtonStyle;
use crate::ui::widgets::atomic::close_button::theme::DefaultCloseButtonTheme;
use crate::ui::widgets::atomic::close_button::types::CloseButtonRenderKind;
use crate::ui::widgets::atomic::drag_handle::render::draw_drag_handle;
use crate::ui::widgets::atomic::drag_handle::settings::DragHandleSettings;
use crate::ui::widgets::atomic::drag_handle::types::{DragHandleRenderKind, DragHandleView};
use crate::ui::widgets::atomic::tab::render::{
    draw_modal_horizontal_tab, draw_modal_sidebar_tab, TabView,
};
use crate::ui::widgets::atomic::tab::style::{ModalHorizontalTabStyle, ModalSidebarTabStyle};
use crate::ui::widgets::atomic::tab::theme::DefaultTabTheme;
use crate::ui::widgets::atomic::tab::types::TabConfig;

use super::settings::ModalSettings;
use super::state::ModalState;
use super::style::BackgroundFill;
use super::types::{BackdropKind, FooterBtnStyle, ModalRenderKind, ModalView};

// ---------------------------------------------------------------------------
// Layout helper struct
// ---------------------------------------------------------------------------

/// Sub-rects produced by the per-kind layout pipeline.
struct ModalLayout {
    /// Header strip (zero height = no header).
    header: Rect,
    /// Close-button bounding box (zero = no close button).
    close_btn: Rect,
    /// Drag-handle hit rect (covers header minus close button area).
    drag_handle: Rect,
    /// Horizontal tab bar (zero height = no top tabs).
    tab_strip: Rect,
    /// Vertical sidebar (zero width = no sidebar).
    sidebar: Rect,
    /// Area available to the body closure.
    body: Rect,
    /// Footer strip (zero height = no footer).
    footer: Rect,
    /// Wizard nav area (zero = no wizard nav).
    wizard_nav: Rect,
}

// ---------------------------------------------------------------------------
// Public API — register
// ---------------------------------------------------------------------------

/// Register the modal composite and all its child hit-rects with the coordinator.
///
/// **No drawing happens here.**  Call this before `draw_modal` so the compositor
/// can correctly dispatch input events to child widgets (close button, drag zone,
/// tabs, footer buttons, etc.).
///
/// Returns the `WidgetId` assigned to the modal composite.
pub fn register_modal(
    coord:    &mut InputCoordinator,
    id:       impl Into<WidgetId>,
    rect:     Rect,
    state:    &ModalState,
    _view:    &ModalView<'_>,
    settings: &ModalSettings,
    kind:     &ModalRenderKind,
    layer:    &LayerId,
) -> WidgetId {
    // The composite catches clicks that land inside the modal frame but miss
    // every child widget, preventing click-through to the canvas behind.
    let modal_id = coord.register_composite(id, WidgetKind::Modal, rect, Sense::CLICK, layer);

    match kind {
        ModalRenderKind::Custom(_) => {
            // Custom — caller manages its own children.
            return modal_id;
        }
        _ => {}
    }

    // Resolve the actual frame rect (draggable modals may have been moved).
    let frame = resolve_frame(rect, state, kind);
    let layout = compute_layout(frame, state, _view, settings, kind);

    // Close button.
    if layout.close_btn.width > 0.0 {
        coord.register_child(
            &modal_id,
            format!("{}:close", modal_id.0),
            WidgetKind::CloseButton,
            layout.close_btn,
            Sense::CLICK,
        );
    }

    // Drag handle (header minus close button).
    if layout.drag_handle.width > 0.0 && layout.drag_handle.height > 0.0 {
        coord.register_child(
            &modal_id,
            format!("{}:drag", modal_id.0),
            WidgetKind::DragHandle,
            layout.drag_handle,
            Sense::DRAG,
        );
    }

    // Tab hit-rects.
    if !_view.tabs.is_empty() {
        let tab_count = _view.tabs.len();
        match kind {
            ModalRenderKind::TopTabs if layout.tab_strip.height > 0.0 => {
                let tab_w = layout.tab_strip.width / tab_count as f64;
                for i in 0..tab_count {
                    let tab_rect = Rect::new(
                        layout.tab_strip.x + i as f64 * tab_w,
                        layout.tab_strip.y,
                        tab_w,
                        layout.tab_strip.height,
                    );
                    coord.register_child(
                        &modal_id,
                        format!("{}:tab:{}", modal_id.0, i),
                        WidgetKind::Button,
                        tab_rect,
                        Sense::CLICK | Sense::HOVER,
                    );
                }
            }
            ModalRenderKind::SideTabs if layout.sidebar.width > 0.0 => {
                let tab_h = layout.sidebar.height / tab_count as f64;
                for i in 0..tab_count {
                    let tab_rect = Rect::new(
                        layout.sidebar.x,
                        layout.sidebar.y + i as f64 * tab_h,
                        layout.sidebar.width,
                        tab_h,
                    );
                    coord.register_child(
                        &modal_id,
                        format!("{}:tab:{}", modal_id.0, i),
                        WidgetKind::Button,
                        tab_rect,
                        Sense::CLICK | Sense::HOVER,
                    );
                }
            }
            _ => {}
        }
    }

    // Footer button hit-rects.
    if layout.footer.height > 0.0 && !_view.footer_buttons.is_empty() {
        let btn_w        = 80.0_f64;
        let btn_h        = 28.0_f64;
        let btn_gap      = 8.0_f64;
        let padding_right = 16.0_f64;
        let total_btns   = _view.footer_buttons.len();
        let total_w      = total_btns as f64 * btn_w
            + (total_btns.saturating_sub(1)) as f64 * btn_gap;
        let start_x      = layout.footer.x + layout.footer.width - total_w - padding_right;
        let btn_y        = layout.footer.y + (layout.footer.height - btn_h) / 2.0;

        for (i, _btn) in _view.footer_buttons.iter().enumerate() {
            let btn_x = start_x + i as f64 * (btn_w + btn_gap);
            coord.register_child(
                &modal_id,
                format!("{}:footer:{}", modal_id.0, i),
                WidgetKind::Button,
                Rect::new(btn_x, btn_y, btn_w, btn_h),
                Sense::CLICK | Sense::HOVER,
            );
        }
    }

    // Wizard nav buttons.
    if matches!(kind, ModalRenderKind::Wizard) && !_view.wizard_pages.is_empty() {
        let style     = settings.style.as_ref();
        let btn_w     = 72.0_f64;
        let btn_h     = 28.0_f64;
        let padding_x = style.padding();
        let nav       = layout.wizard_nav;
        let btn_y     = nav.y + (nav.height - btn_h) / 2.0;

        // Back button (only visible on pages > 0, but register it always for
        // consistent id stability).
        if state.current_page > 0 {
            coord.register_child(
                &modal_id,
                format!("{}:wizard:back", modal_id.0),
                WidgetKind::Button,
                Rect::new(nav.x + padding_x, btn_y, btn_w, btn_h),
                Sense::CLICK,
            );
        }

        // Next / Finish button.
        coord.register_child(
            &modal_id,
            format!("{}:wizard:next", modal_id.0),
            WidgetKind::Button,
            Rect::new(nav.x + nav.width - padding_x - btn_w, btn_y, btn_w, btn_h),
            Sense::CLICK,
        );
    }

    modal_id
}

// ---------------------------------------------------------------------------
// Public API — draw
// ---------------------------------------------------------------------------

/// Draw a modal composite.
///
/// **Pure rendering — no `InputCoordinator` registration.**
/// The caller must have called `register_modal` (or equivalent) beforehand.
///
/// # Arguments
/// - `ctx`      — render context.
/// - `rect`     — bounding rect (ignored for draggable kinds when `state.position`
///                is non-zero — draggable modals use their stored position).
/// - `state`    — mutable modal state (hover flags are read this frame).
/// - `view`     — per-frame data (title, tabs, footer buttons, body closure, …).
/// - `settings` — theme + style configuration.
/// - `kind`     — selects the layout pipeline.
pub fn draw_modal(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    state:    &ModalState,
    view:     &mut ModalView<'_>,
    settings: &ModalSettings,
    kind:     &ModalRenderKind,
) {
    match kind {
        ModalRenderKind::Custom(f) => {
            f(ctx, rect, view, settings);
            return;
        }
        _ => {}
    }

    let theme = settings.theme.as_ref();
    let style = settings.style.as_ref();

    let frame  = resolve_frame(rect, state, kind);
    let layout = compute_layout(frame, state, view, settings, kind);

    // --- 1. Backdrop ----------------------------------------------------------
    match view.backdrop {
        BackdropKind::None => {}
        BackdropKind::Dim => {
            ctx.set_fill_color(theme.backdrop_dim());
            ctx.fill_rect(0.0, 0.0, 99_999.0, 99_999.0);
        }
        BackdropKind::FullBlock => {
            ctx.set_fill_color(theme.backdrop_full());
            ctx.fill_rect(0.0, 0.0, 99_999.0, 99_999.0);
        }
    }

    // --- 2. Shadow rect -------------------------------------------------------
    let offset = style.shadow_offset();
    ctx.set_fill_color(theme.shadow());
    ctx.fill_rounded_rect(
        frame.x + offset,
        frame.y + offset,
        frame.width,
        frame.height,
        style.radius(),
    );

    // --- 3. Frame background + border (BackgroundFill dispatch) ---------------
    match style.background_fill() {
        BackgroundFill::Solid => {
            ctx.set_fill_color(theme.bg());
            ctx.fill_rounded_rect(frame.x, frame.y, frame.width, frame.height, style.radius());
        }
        BackgroundFill::Glass { blur_radius: _ } => {
            // Blur backdrop first (no-op on backends without blur support).
            ctx.draw_blur_background(frame.x, frame.y, frame.width, frame.height);
            // Semi-transparent overlay.
            ctx.set_fill_color(theme.bg());
            ctx.fill_rounded_rect(frame.x, frame.y, frame.width, frame.height, style.radius());
        }
        BackgroundFill::Texture { asset_id } => {
            // Asset system not yet wired — fall back to solid fill.
            let _ = asset_id;
            ctx.set_fill_color(theme.bg());
            ctx.fill_rounded_rect(frame.x, frame.y, frame.width, frame.height, style.radius());
        }
    }

    ctx.set_stroke_color(theme.border());
    ctx.set_stroke_width(style.border_width());
    ctx.set_line_dash(&[]);
    ctx.stroke_rounded_rect(frame.x, frame.y, frame.width, frame.height, style.radius());

    // --- 4. Header ------------------------------------------------------------
    let has_header = layout.header.height > 0.0;
    if has_header {
        ctx.set_fill_color(theme.header_bg());
        ctx.fill_rect(
            layout.header.x,
            layout.header.y,
            layout.header.width,
            layout.header.height,
        );

        // Title text.
        let title = view.title.unwrap_or("");
        ctx.set_fill_color(theme.header_text());
        ctx.set_font("14px sans-serif");
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Middle);
        ctx.fill_text(
            title,
            layout.header.x + style.padding(),
            layout.header.y + layout.header.height / 2.0,
        );

        // Header bottom divider.
        ctx.set_fill_color(theme.divider());
        ctx.fill_rect(
            layout.header.x,
            layout.header.y + layout.header.height - 1.0,
            layout.header.width,
            1.0,
        );

        // Drag handle — invisible hit zone (pure registration, no visual).
        let dh_view     = DragHandleView { rect: layout.drag_handle };
        let dh_settings = DragHandleSettings::default();
        draw_drag_handle(ctx, layout.drag_handle, &dh_view, &dh_settings, &DragHandleRenderKind::Invisible);

        // Close-X button.
        let close_view = CloseButtonView {
            hovered: state.hovered_close,
        };
        let close_settings = CloseButtonSettings {
            theme: Box::new(DefaultCloseButtonTheme),
            style: Box::new(DefaultCloseButtonStyle),
        };
        draw_close_button(
            ctx,
            layout.close_btn,
            if state.hovered_close { WidgetState::Hovered } else { WidgetState::Normal },
            &close_view,
            &close_settings,
            &CloseButtonRenderKind::Default,
        );
    }

    // --- 5. Tab strip ---------------------------------------------------------
    if !view.tabs.is_empty() {
        match kind {
            ModalRenderKind::TopTabs => {
                draw_top_tabs(ctx, &layout, view, state, settings);
            }
            ModalRenderKind::SideTabs => {
                draw_side_tabs(ctx, &layout, view, state, settings);
            }
            _ => {}
        }
    }

    // --- 6. Footer divider ----------------------------------------------------
    if layout.footer.height > 0.0 {
        ctx.set_fill_color(theme.footer_border());
        ctx.fill_rect(
            layout.footer.x,
            layout.footer.y,
            layout.footer.width,
            1.0,
        );
        ctx.set_fill_color(theme.footer_bg());
        ctx.fill_rect(
            layout.footer.x,
            layout.footer.y + 1.0,
            layout.footer.width,
            layout.footer.height - 1.0,
        );
    }

    // --- 7. Body closure ------------------------------------------------------
    // The coordinator is not passed here — body registration is the caller's
    // responsibility (or handled via the `modal` convenience wrapper).
    // Pass a no-op coordinator stub by using the version with coord below.
    // NOTE: body still needs coord for its own child widgets — use the `modal`
    // convenience wrapper or call draw_modal_with_coord instead.
    (view.body)(ctx, layout.body, &mut crate::input::InputCoordinator::default());

    // --- 8. Footer buttons ----------------------------------------------------
    if layout.footer.height > 0.0 && !view.footer_buttons.is_empty() {
        draw_footer_buttons(ctx, &layout, view, state);
    }

    // --- 9. Wizard nav --------------------------------------------------------
    if matches!(kind, ModalRenderKind::Wizard) {
        draw_wizard_nav(ctx, &layout, view, state, settings);
    }
}

// ---------------------------------------------------------------------------
// Public API — convenience wrapper
// ---------------------------------------------------------------------------

/// Register + draw a modal in one call.
///
/// This is the recommended entry point for typical use.  Under the hood it
/// calls `register_modal` then `draw_modal_inner` (which passes `coord` to
/// the body closure so inner widgets can register themselves).
///
/// # Arguments
/// - `coord`    — input coordinator.
/// - `ctx`      — render context.
/// - `id`       — stable widget id.
/// - `rect`     — bounding rect.
/// - `state`    — mutable modal state.
/// - `view`     — per-frame data.
/// - `settings` — theme + style configuration.
/// - `kind`     — selects the layout pipeline.
/// - `layer`    — coordinator layer.
pub fn modal(
    coord:    &mut InputCoordinator,
    ctx:      &mut dyn RenderContext,
    id:       impl Into<WidgetId>,
    rect:     Rect,
    state:    &mut ModalState,
    view:     &mut ModalView<'_>,
    settings: &ModalSettings,
    kind:     &ModalRenderKind,
    layer:    &LayerId,
) {
    let modal_id = register_modal(coord, id, rect, state, view, settings, kind, layer);
    draw_modal_with_coord(ctx, rect, coord, &modal_id, state, view, settings, kind);
}

// ---------------------------------------------------------------------------
// Internal draw helpers (with coord — used by the convenience wrapper)
// ---------------------------------------------------------------------------

/// Full draw pipeline with `InputCoordinator` passed through to the body closure.
///
/// This is the version called by the `modal` convenience wrapper.
fn draw_modal_with_coord(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    coord:    &mut InputCoordinator,
    _modal_id: &WidgetId,
    state:    &ModalState,
    view:     &mut ModalView<'_>,
    settings: &ModalSettings,
    kind:     &ModalRenderKind,
) {
    match kind {
        ModalRenderKind::Custom(f) => {
            f(ctx, rect, view, settings);
            return;
        }
        _ => {}
    }

    let theme = settings.theme.as_ref();
    let style = settings.style.as_ref();

    let frame  = resolve_frame(rect, state, kind);
    let layout = compute_layout(frame, state, view, settings, kind);

    // --- 1. Backdrop ----------------------------------------------------------
    match view.backdrop {
        BackdropKind::None => {}
        BackdropKind::Dim => {
            ctx.set_fill_color(theme.backdrop_dim());
            ctx.fill_rect(0.0, 0.0, 99_999.0, 99_999.0);
        }
        BackdropKind::FullBlock => {
            ctx.set_fill_color(theme.backdrop_full());
            ctx.fill_rect(0.0, 0.0, 99_999.0, 99_999.0);
        }
    }

    // --- 2. Shadow rect -------------------------------------------------------
    let offset = style.shadow_offset();
    ctx.set_fill_color(theme.shadow());
    ctx.fill_rounded_rect(
        frame.x + offset,
        frame.y + offset,
        frame.width,
        frame.height,
        style.radius(),
    );

    // --- 3. Frame background + border (BackgroundFill dispatch) ---------------
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

    // --- 4. Header ------------------------------------------------------------
    let has_header = layout.header.height > 0.0;
    if has_header {
        ctx.set_fill_color(theme.header_bg());
        ctx.fill_rect(
            layout.header.x,
            layout.header.y,
            layout.header.width,
            layout.header.height,
        );

        let title = view.title.unwrap_or("");
        ctx.set_fill_color(theme.header_text());
        ctx.set_font("14px sans-serif");
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Middle);
        ctx.fill_text(
            title,
            layout.header.x + style.padding(),
            layout.header.y + layout.header.height / 2.0,
        );

        ctx.set_fill_color(theme.divider());
        ctx.fill_rect(
            layout.header.x,
            layout.header.y + layout.header.height - 1.0,
            layout.header.width,
            1.0,
        );

        // Drag handle — invisible (pure hit zone already registered).
        let dh_view     = DragHandleView { rect: layout.drag_handle };
        let dh_settings = DragHandleSettings::default();
        draw_drag_handle(ctx, layout.drag_handle, &dh_view, &dh_settings, &DragHandleRenderKind::Invisible);

        // Close-X button.
        let close_view = CloseButtonView { hovered: state.hovered_close };
        let close_settings = CloseButtonSettings {
            theme: Box::new(DefaultCloseButtonTheme),
            style: Box::new(DefaultCloseButtonStyle),
        };
        draw_close_button(
            ctx,
            layout.close_btn,
            if state.hovered_close { WidgetState::Hovered } else { WidgetState::Normal },
            &close_view,
            &close_settings,
            &CloseButtonRenderKind::Default,
        );
    }

    // --- 5. Tab strip ---------------------------------------------------------
    if !view.tabs.is_empty() {
        match kind {
            ModalRenderKind::TopTabs => {
                draw_top_tabs(ctx, &layout, view, state, settings);
            }
            ModalRenderKind::SideTabs => {
                draw_side_tabs(ctx, &layout, view, state, settings);
            }
            _ => {}
        }
    }

    // --- 6. Footer divider ----------------------------------------------------
    if layout.footer.height > 0.0 {
        ctx.set_fill_color(theme.footer_border());
        ctx.fill_rect(layout.footer.x, layout.footer.y, layout.footer.width, 1.0);
        ctx.set_fill_color(theme.footer_bg());
        ctx.fill_rect(
            layout.footer.x,
            layout.footer.y + 1.0,
            layout.footer.width,
            layout.footer.height - 1.0,
        );
    }

    // --- 7. Body closure (with coord) -----------------------------------------
    (view.body)(ctx, layout.body, coord);

    // --- 8. Footer buttons ----------------------------------------------------
    if layout.footer.height > 0.0 && !view.footer_buttons.is_empty() {
        draw_footer_buttons(ctx, &layout, view, state);
    }

    // --- 9. Wizard nav --------------------------------------------------------
    if matches!(kind, ModalRenderKind::Wizard) {
        draw_wizard_nav(ctx, &layout, view, state, settings);
    }
}

// ---------------------------------------------------------------------------
// Layout computation
// ---------------------------------------------------------------------------

/// Resolve the actual on-screen frame rect for draggable kinds.
fn resolve_frame(rect: Rect, state: &ModalState, kind: &ModalRenderKind) -> Rect {
    if matches!(
        kind,
        ModalRenderKind::WithHeader
            | ModalRenderKind::WithHeaderFooter
            | ModalRenderKind::TopTabs
            | ModalRenderKind::SideTabs
    ) && (state.position.0 != 0.0 || state.position.1 != 0.0)
    {
        Rect::new(state.position.0, state.position.1, rect.width, rect.height)
    } else {
        rect
    }
}

fn compute_layout(
    frame:    Rect,
    _state:   &ModalState,
    view:     &ModalView<'_>,
    settings: &ModalSettings,
    kind:     &ModalRenderKind,
) -> ModalLayout {
    let style = settings.style.as_ref();

    let has_header   = matches!(
        kind,
        ModalRenderKind::WithHeader
            | ModalRenderKind::WithHeaderFooter
            | ModalRenderKind::TopTabs
            | ModalRenderKind::SideTabs
    );
    let has_close    = has_header;
    let has_top_tabs = matches!(kind, ModalRenderKind::TopTabs);
    let has_sidebar  = matches!(kind, ModalRenderKind::SideTabs);
    let has_footer   = matches!(
        kind,
        ModalRenderKind::WithHeaderFooter | ModalRenderKind::SideTabs
    ) || (matches!(kind, ModalRenderKind::TopTabs) && !view.footer_buttons.is_empty());
    let has_wizard   = matches!(kind, ModalRenderKind::Wizard);

    let header_h     = if has_header { style.header_height() } else { 0.0 };
    let tab_h        = if has_top_tabs { style.tab_height() } else { 0.0 };
    let sidebar_w    = if has_sidebar { style.sidebar_width() } else { 0.0 };
    let footer_h     = if has_footer { style.footer_height() } else { 0.0 };
    let wizard_nav_h = if has_wizard { style.wizard_nav_height() } else { 0.0 };
    let btn_size     = style.close_btn_size();

    let header = Rect::new(frame.x, frame.y, frame.width, header_h);

    let close_btn = if has_close && header_h > 0.0 {
        let padding = 10.0_f64;
        Rect::new(
            frame.x + frame.width - btn_size - padding,
            frame.y + (header_h - btn_size) / 2.0,
            btn_size,
            btn_size,
        )
    } else {
        Rect::default()
    };

    // Drag handle covers header minus the close-button column.
    let drag_handle = if has_header && header_h > 0.0 {
        Rect::new(
            frame.x,
            frame.y,
            frame.width - close_btn.width,
            header_h,
        )
    } else {
        Rect::default()
    };

    let tab_strip = if has_top_tabs {
        Rect::new(frame.x, frame.y + header_h, frame.width, tab_h)
    } else {
        Rect::default()
    };

    let sidebar = if has_sidebar {
        Rect::new(
            frame.x,
            frame.y + header_h,
            sidebar_w,
            frame.height - header_h - footer_h,
        )
    } else {
        Rect::default()
    };

    let footer = if has_footer || has_wizard {
        Rect::new(
            frame.x,
            frame.y + frame.height - footer_h - wizard_nav_h,
            frame.width,
            footer_h,
        )
    } else {
        Rect::default()
    };

    let wizard_nav = if has_wizard {
        Rect::new(
            frame.x,
            frame.y + frame.height - wizard_nav_h,
            frame.width,
            wizard_nav_h,
        )
    } else {
        Rect::default()
    };

    let body_x = frame.x + sidebar_w;
    let body_y = frame.y + header_h + tab_h;
    let body_w = frame.width - sidebar_w;
    let body_h = frame.height - header_h - tab_h - footer_h - wizard_nav_h;
    let body   = Rect::new(body_x, body_y, body_w, body_h.max(0.0));

    ModalLayout {
        header,
        close_btn,
        drag_handle,
        tab_strip,
        sidebar,
        body,
        footer,
        wizard_nav,
    }
}

// ---------------------------------------------------------------------------
// Tab rendering helpers
// ---------------------------------------------------------------------------

fn draw_top_tabs(
    ctx:      &mut dyn RenderContext,
    layout:   &ModalLayout,
    view:     &ModalView<'_>,
    state:    &ModalState,
    _settings: &ModalSettings,
) {
    let tab_theme = DefaultTabTheme;
    let tab_style = ModalHorizontalTabStyle::default();
    let tab_count = view.tabs.len();
    if tab_count == 0 {
        return;
    }

    let tab_w = layout.tab_strip.width / tab_count as f64;

    for (i, label) in view.tabs.iter().enumerate() {
        let tab_rect = Rect::new(
            layout.tab_strip.x + i as f64 * tab_w,
            layout.tab_strip.y,
            tab_w,
            layout.tab_strip.height,
        );
        let is_active  = i == state.active_tab;
        let is_hovered = state.hovered_tab == Some(i);
        let cfg        = TabConfig {
            id:              format!("tab_{}", i),
            label:           label.to_string(),
            active:          is_active,
            closable:        false,
            icon:            None,
            intrinsic_width: false,
        };
        let tab_view = TabView {
            tab:               &cfg,
            hovered:           is_hovered,
            pressed:           false,
            close_btn_hovered: false,
        };
        draw_modal_horizontal_tab(ctx, tab_rect, &tab_view, &tab_style, &tab_theme);
    }
}

fn draw_side_tabs(
    ctx:      &mut dyn RenderContext,
    layout:   &ModalLayout,
    view:     &ModalView<'_>,
    state:    &ModalState,
    _settings: &ModalSettings,
) {
    let tab_theme = DefaultTabTheme;
    let tab_style = ModalSidebarTabStyle::default();
    let tab_count = view.tabs.len();
    if tab_count == 0 {
        return;
    }

    let tab_h = layout.sidebar.height / tab_count as f64;

    for (i, label) in view.tabs.iter().enumerate() {
        let tab_rect = Rect::new(
            layout.sidebar.x,
            layout.sidebar.y + i as f64 * tab_h,
            layout.sidebar.width,
            tab_h,
        );
        let is_active  = i == state.active_tab;
        let is_hovered = state.hovered_tab == Some(i);
        let cfg        = TabConfig {
            id:              format!("tab_{}", i),
            label:           label.to_string(),
            active:          is_active,
            closable:        false,
            icon:            None,
            intrinsic_width: false,
        };
        let tab_view = TabView {
            tab:               &cfg,
            hovered:           is_hovered,
            pressed:           false,
            close_btn_hovered: false,
        };
        draw_modal_sidebar_tab(ctx, tab_rect, &tab_view, &tab_style, &tab_theme);
    }
}

// ---------------------------------------------------------------------------
// Footer buttons
// ---------------------------------------------------------------------------

fn draw_footer_buttons(
    ctx:    &mut dyn RenderContext,
    layout: &ModalLayout,
    view:   &ModalView<'_>,
    state:  &ModalState,
) {
    let btn_theme    = DefaultButtonTheme;
    let btn_w        = 80.0_f64;
    let btn_h        = 28.0_f64;
    let btn_gap      = 8.0_f64;
    let padding_right = 16.0_f64;

    let total_btns = view.footer_buttons.len();
    let total_w    = total_btns as f64 * btn_w
        + (total_btns.saturating_sub(1)) as f64 * btn_gap;
    let start_x    = layout.footer.x + layout.footer.width - total_w - padding_right;
    let btn_y      = layout.footer.y + (layout.footer.height - btn_h) / 2.0;

    for (i, btn) in view.footer_buttons.iter().enumerate() {
        let btn_x    = start_x + i as f64 * (btn_w + btn_gap);
        let btn_rect = Rect::new(btn_x, btn_y, btn_w, btn_h);
        let hovered  = state.footer_hovered == Some(i);

        match btn.style {
            FooterBtnStyle::Primary => {
                let btn_view = PrimaryButtonView { text: btn.label, hovered };
                draw_primary_button(ctx, btn_rect, &btn_view, 4.0, &btn_theme);
            }
            FooterBtnStyle::Ghost => {
                let btn_view = GhostOutlineButtonView { text: btn.label, hovered };
                draw_ghost_outline_button(ctx, btn_rect, &btn_view, 4.0, &btn_theme);
            }
            FooterBtnStyle::Danger => {
                let btn_view = DangerButtonView {
                    text:    btn.label,
                    hovered,
                    variant: DangerVariant::Delete,
                    icon:    None,
                };
                draw_danger_button(
                    ctx,
                    btn_rect,
                    &btn_view,
                    4.0,
                    &btn_theme,
                    |_ctx, _icon, _rect, _color| {},
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Wizard nav (page dots + Back / Next)
// ---------------------------------------------------------------------------

fn draw_wizard_nav(
    ctx:      &mut dyn RenderContext,
    layout:   &ModalLayout,
    view:     &ModalView<'_>,
    state:    &ModalState,
    settings: &ModalSettings,
) {
    let theme      = settings.theme.as_ref();
    let style      = settings.style.as_ref();
    let page_count = view.wizard_pages.len();
    if page_count == 0 {
        return;
    }

    let nav = layout.wizard_nav;

    // --- Page dots -----------------------------------------------------------
    let dot_size    = 8.0_f64;
    let dot_gap     = 6.0_f64;
    let dots_total_w = page_count as f64 * dot_size
        + (page_count.saturating_sub(1)) as f64 * dot_gap;
    let dots_x = nav.x + (nav.width - dots_total_w) / 2.0;
    let dots_y = nav.y + nav.height / 2.0 - dot_size / 2.0;

    for i in 0..page_count {
        let cx = dots_x + i as f64 * (dot_size + dot_gap);
        let color = if i == state.current_page {
            theme.wizard_dot_active()
        } else {
            theme.wizard_dot_inactive()
        };
        ctx.set_fill_color(color);
        ctx.fill_rounded_rect(cx, dots_y, dot_size, dot_size, dot_size / 2.0);
    }

    // --- Back / Next buttons -------------------------------------------------
    let btn_w     = 72.0_f64;
    let btn_h     = 28.0_f64;
    let btn_theme = DefaultButtonTheme;
    let btn_y     = nav.y + (nav.height - btn_h) / 2.0;
    let padding_x = style.padding();

    if state.current_page > 0 {
        let back_rect = Rect::new(nav.x + padding_x, btn_y, btn_w, btn_h);
        let hovered   = state.footer_hovered == Some(0);
        let back_view = GhostOutlineButtonView { text: "Back", hovered };
        draw_ghost_outline_button(ctx, back_rect, &back_view, 4.0, &btn_theme);
    }

    let is_last    = state.current_page + 1 >= page_count;
    let next_label = if is_last { "Finish" } else { "Next" };
    let next_rect  = Rect::new(nav.x + nav.width - padding_x - btn_w, btn_y, btn_w, btn_h);
    let hovered    = state.footer_hovered == Some(1);
    let next_view  = PrimaryButtonView { text: next_label, hovered };
    draw_primary_button(ctx, next_rect, &next_view, 4.0, &btn_theme);
}
