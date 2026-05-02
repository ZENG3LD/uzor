//! Modal render entry point and per-kind layout pipelines.
//!
//! # API convention
//!
//! - `register_input_coordinator_modal` — registers the composite + all child
//!   hit-rects with an `InputCoordinator`.  **No drawing.**  Use when you need
//!   to separate registration from rendering (explicit z-order control).
//! - `register_context_manager_modal`   — convenience wrapper: takes a
//!   `ContextManager`, registers, and draws the chrome in one call.
//!   Body content is drawn by the caller after this call returns.
//!
//! # Draw order for every non-Custom kind
//!
//! 1. Backdrop (if `view.backdrop` != `BackdropKind::None`)
//! 2. Shadow rect (offset by `style.shadow_offset()`)
//! 3. Frame background + border  (dispatches on `style.background_fill()`)
//! 4. Header strip + title + close-X + drag-zone (DragHandle)  (if kind has header)
//! 5. Tab strip — horizontal (`TopTabs`) or vertical sidebar (`SideTabs`)
//! 6. Dividers (header bottom, footer top, sidebar right)
//! 7. (body drawn by caller after composite call)
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
use crate::ui::widgets::atomic::text::render::draw_text;
use crate::ui::widgets::atomic::text::settings::TextSettings;
use crate::ui::widgets::atomic::text::types::{TextOverflow, TextView};

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
/// **No drawing happens here.**  Use this when you need explicit z-order control
/// (register multiple composites, then draw them in order).
///
/// Returns the `WidgetId` assigned to the modal composite.
pub fn register_input_coordinator_modal(
    coord:    &mut InputCoordinator,
    id:       impl Into<WidgetId>,
    rect:     Rect,
    state:    &mut ModalState,
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

    // Body overflow strips (scrollbar / chevrons) and resize handles.
    register_body_overflow(coord, &modal_id, frame, _view, settings, kind, state);
    if _view.resizable {
        register_resize_handles(coord, &modal_id, frame);
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
// Public API — convenience wrapper (ContextManager)
// ---------------------------------------------------------------------------

/// Register + draw a modal in one call using a `ContextManager`.
///
/// This is the recommended entry point for typical use.  Under the hood it
/// calls `register_input_coordinator_modal` then the full draw pipeline,
/// passing `coord` to the body closure so inner widgets can self-register.
///
/// # Arguments
/// - `ctx_mgr`  — context manager (coord extracted as `&mut ctx_mgr.input`).
/// - `render`   — render context.
/// - `id`       — stable widget id.
/// - `rect`     — bounding rect.
/// - `state`    — mutable modal state.
/// - `view`     — per-frame data.
/// - `settings` — theme + style configuration.
/// - `kind`     — selects the layout pipeline.
/// - `layer`    — coordinator layer.
pub fn register_context_manager_modal(
    ctx_mgr:  &mut crate::app_context::ContextManager,
    render:   &mut dyn RenderContext,
    id:       impl Into<WidgetId>,
    rect:     Rect,
    state:    &mut ModalState,
    view:     &mut ModalView<'_>,
    settings: &ModalSettings,
    kind:     &ModalRenderKind,
    layer:    &LayerId,
) {
    let coord = &mut ctx_mgr.input;
    let modal_id = register_input_coordinator_modal(coord, id, rect, state, view, settings, kind, layer);
    draw_modal_with_coord(render, rect, coord, &modal_id, state, view, settings, kind);
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
    _coord:    &mut InputCoordinator,
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

        // Title — drawn via atomic Text widget so font / overflow / state-stack
        // hygiene is handled by the widget, not by composite-level fill_text calls.
        let title = view.title.unwrap_or("");
        let title_rect = Rect::new(
            layout.header.x + style.padding(),
            layout.header.y,
            (layout.header.width - layout.close_btn.width - style.padding() * 2.0).max(0.0),
            layout.header.height,
        );
        let title_color = theme.header_text();
        let title_view = TextView {
            text: title,
            align: TextAlign::Left,
            baseline: TextBaseline::Middle,
            color: Some(title_color),
            font: Some("14px sans-serif"),
            overflow: TextOverflow::Ellipsis,
            hovered: false,
        };
        draw_text(ctx, title_rect, &title_view, &TextSettings::default());

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

    // --- 7. (body drawn by caller after composite call) -----------------------

    // --- 8. Footer buttons ----------------------------------------------------
    if layout.footer.height > 0.0 && !view.footer_buttons.is_empty() {
        draw_footer_buttons(ctx, &layout, view, state);
    }

    // --- 9. Wizard nav --------------------------------------------------------
    if matches!(kind, ModalRenderKind::Wizard) {
        draw_wizard_nav(ctx, &layout, view, state, settings);
    }

    // --- 10. Body scrollbar (overflow Scrollbar) ----------------------------
    if matches!(view.overflow, crate::types::OverflowMode::Scrollbar) {
        if let Some(track) = state.body_scroll_track {
            use crate::ui::widgets::atomic::scrollbar::{
                render::{draw_scrollbar, ScrollbarView, ScrollbarVisualState},
                style::StandardScrollbarStyle,
                theme::DefaultScrollbarTheme,
            };
            let style = StandardScrollbarStyle::default();
            let theme = DefaultScrollbarTheme::default();
            let visual_state = if state.scroll.is_dragging {
                ScrollbarVisualState::Dragging
            } else {
                ScrollbarVisualState::Active
            };
            let sv = ScrollbarView {
                content_height:  state.body_content_h,
                viewport_height: state.body_viewport_h,
                scroll_offset:   state.scroll.offset,
                state:           visual_state,
                drag_pos_y:      None,
                style:           &style,
                theme:           &theme,
            };
            let _ = draw_scrollbar(ctx, track, &sv);
        }
    }

    // --- 11. Body scrollbar / chevron OVERLAYS ------------------------------
    // Chevrons must paint AFTER the caller-drawn body so they're the
    // top-most overlay over body content. The composite drew the
    // scrollbar at step 10 above, but the chevron overlays are exposed
    // as a public helper the host calls AFTER its body draw — see
    // `draw_body_overflow_chevrons`.
}

/// Paint the body chevron overlays (top / bottom / left / right strips
/// with a `▲▼◀▶` paging chevron). Call AFTER drawing body content so
/// the chevrons sit on top of every body widget.
///
/// No-op when `view.overflow != OverflowMode::Chevrons` or no overflow
/// on the corresponding axis.
pub fn draw_body_overflow_chevrons(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    state:    &ModalState,
    view:     &ModalView<'_>,
    settings: &ModalSettings,
    kind:     &ModalRenderKind,
) {
    if !matches!(view.overflow, crate::types::OverflowMode::Chevrons) { return; }
    let frame = resolve_frame(rect, state, kind);
    let body  = body_rect(frame, view, settings, kind);
    if body.width <= 0.0 || body.height <= 0.0 { return; }

    use crate::ui::widgets::atomic::chevron::{
        draw_chevron,
        settings::ChevronSettings,
        types::{ChevronDirection, ChevronUseCase, ChevronView, ChevronVisualKind,
                HitAreaPolicy, PlacementPolicy, VisibilityPolicy},
    };
    let strip = 26.0_f64;
    let theme = settings.theme.as_ref();
    let chev_settings = ChevronSettings::default();

    if state.body_content_h > body.height + 0.5 {
        let up = Rect::new(body.x, body.y, body.width, strip);
        let dn = Rect::new(body.x, body.y + body.height - strip, body.width, strip);
        let max_v = (state.body_content_h - body.height).max(0.0);
        let has_back = state.scroll.offset > 0.5;
        let has_fwd  = state.scroll.offset < max_v - 0.5;
        ctx.set_fill_color(theme.bg());
        ctx.fill_rect(up.x, up.y, up.width, up.height);
        ctx.fill_rect(dn.x, dn.y, dn.width, dn.height);
        let v_up = ChevronView {
            direction:   ChevronDirection::Up,
            use_case:    ChevronUseCase::PixelScrollStep,
            visibility:  VisibilityPolicy::WhenOverflow { has_more: has_back },
            placement:   PlacementPolicy::Overlay,
            hit_area:    HitAreaPolicy::Visual,
            visual_kind: ChevronVisualKind::Stroked,
            ..Default::default()
        };
        let v_dn = ChevronView {
            direction:   ChevronDirection::Down,
            use_case:    ChevronUseCase::PixelScrollStep,
            visibility:  VisibilityPolicy::WhenOverflow { has_more: has_fwd },
            placement:   PlacementPolicy::Overlay,
            hit_area:    HitAreaPolicy::Visual,
            visual_kind: ChevronVisualKind::Stroked,
            ..Default::default()
        };
        draw_chevron(ctx, up, &v_up, &chev_settings);
        draw_chevron(ctx, dn, &v_dn, &chev_settings);
    }
    if state.body_content_w > body.width + 0.5 {
        let lf = Rect::new(body.x, body.y, strip, body.height);
        let rt = Rect::new(body.x + body.width - strip, body.y, strip, body.height);
        let max_h = (state.body_content_w - body.width).max(0.0);
        let has_back = state.body_scroll_x > 0.5;
        let has_fwd  = state.body_scroll_x < max_h - 0.5;
        ctx.set_fill_color(theme.bg());
        ctx.fill_rect(lf.x, lf.y, lf.width, lf.height);
        ctx.fill_rect(rt.x, rt.y, rt.width, rt.height);
        let v_lf = ChevronView {
            direction:   ChevronDirection::Left,
            use_case:    ChevronUseCase::PixelScrollStep,
            visibility:  VisibilityPolicy::WhenOverflow { has_more: has_back },
            placement:   PlacementPolicy::Overlay,
            hit_area:    HitAreaPolicy::Visual,
            visual_kind: ChevronVisualKind::Stroked,
            ..Default::default()
        };
        let v_rt = ChevronView {
            direction:   ChevronDirection::Right,
            use_case:    ChevronUseCase::PixelScrollStep,
            visibility:  VisibilityPolicy::WhenOverflow { has_more: has_fwd },
            placement:   PlacementPolicy::Overlay,
            hit_area:    HitAreaPolicy::Visual,
            visual_kind: ChevronVisualKind::Stroked,
            ..Default::default()
        };
        draw_chevron(ctx, lf, &v_lf, &chev_settings);
        draw_chevron(ctx, rt, &v_rt, &chev_settings);
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

/// Effective sidebar width for a SideTabs modal — grows beyond
/// `style.sidebar_width()` if labels don't fit. For non-SideTabs kinds returns 0.
///
/// Heuristic: `max(style.sidebar_width(), longest_label.len() * 7.0 + padding * 2)`,
/// where 7.0 is the mlc per-character estimate also used by chrome / dropdown
/// measure helpers.
fn effective_sidebar_width(
    view: &ModalView<'_>,
    style: &dyn super::style::ModalStyle,
    kind: &ModalRenderKind,
) -> f64 {
    if !matches!(kind, ModalRenderKind::SideTabs) {
        return 0.0;
    }
    let base = style.sidebar_width();
    let pad  = 12.0_f64;
    let max_label_len = view.tabs.iter()
        .map(|t| t.len())
        .max()
        .unwrap_or(0) as f64;
    let label_w = max_label_len * 7.0 + pad * 2.0;
    base.max(label_w)
}

/// Compute the body rect (the area available for caller-drawn content) of a
/// modal whose total frame is `frame_rect`.
///
/// This is the inverse of `measure_chrome`: the modal composite eats
/// `header + (tabs|sidebar) + footer + wizard_nav` from the frame, and the
/// remaining inner rectangle is the body.
///
/// Use this in your render loop instead of subtracting header/footer
/// constants by hand:
/// ```ignore
/// let body = body_rect(modal_overlay_rect, &view, &settings, &kind);
/// // draw your content into `body`
/// ```
pub fn body_rect(
    frame_rect: Rect,
    view:       &ModalView<'_>,
    settings:   &ModalSettings,
    kind:       &ModalRenderKind,
) -> Rect {
    // Resolve dragged frame from raw rect (same as register_input_coordinator_modal).
    // We don't have ModalState here — caller already passes the post-drag rect
    // via overlay registration, so frame_rect IS the resolved frame.
    let style = settings.style.as_ref();

    let has_header   = matches!(
        kind,
        ModalRenderKind::WithHeader
            | ModalRenderKind::WithHeaderFooter
            | ModalRenderKind::TopTabs
            | ModalRenderKind::SideTabs
    );
    let has_top_tabs = matches!(kind, ModalRenderKind::TopTabs);
    let has_sidebar  = matches!(kind, ModalRenderKind::SideTabs);
    let has_footer   = matches!(
        kind,
        ModalRenderKind::WithHeaderFooter | ModalRenderKind::SideTabs
    ) || (matches!(kind, ModalRenderKind::TopTabs) && !view.footer_buttons.is_empty());
    let has_wizard   = matches!(kind, ModalRenderKind::Wizard);

    let header_h     = if has_header   { style.header_height()      } else { 0.0 };
    let tab_h        = if has_top_tabs { style.tab_height()         } else { 0.0 };
    let footer_h     = if has_footer   { style.footer_height()      } else { 0.0 };
    let wizard_nav_h = if has_wizard   { style.wizard_nav_height()  } else { 0.0 };
    let sidebar_w    = if has_sidebar  { effective_sidebar_width(view, style, kind) } else { 0.0 };

    Rect::new(
        frame_rect.x + sidebar_w,
        frame_rect.y + header_h + tab_h,
        (frame_rect.width  - sidebar_w).max(0.0),
        (frame_rect.height - header_h - tab_h - footer_h - wizard_nav_h).max(0.0),
    )
}

/// Measure the chrome overhead (header / tabs / footer / wizard nav / sidebar)
/// that a modal kind adds around its body.
///
/// Returns `(extra_w, extra_h)` — caller passes desired body `(bw, bh)` and uses
/// `(bw + extra_w, bh + extra_h)` as the overlay rect, instead of hardcoding
/// magic frame sizes.
pub fn measure_chrome(
    view:     &ModalView<'_>,
    settings: &ModalSettings,
    kind:     &ModalRenderKind,
) -> (f64, f64) {
    let style = settings.style.as_ref();

    let has_header   = matches!(
        kind,
        ModalRenderKind::WithHeader
            | ModalRenderKind::WithHeaderFooter
            | ModalRenderKind::TopTabs
            | ModalRenderKind::SideTabs
    );
    let has_top_tabs = matches!(kind, ModalRenderKind::TopTabs);
    let has_sidebar  = matches!(kind, ModalRenderKind::SideTabs);
    let has_footer   = matches!(
        kind,
        ModalRenderKind::WithHeaderFooter | ModalRenderKind::SideTabs
    ) || (matches!(kind, ModalRenderKind::TopTabs) && !view.footer_buttons.is_empty());
    let has_wizard   = matches!(kind, ModalRenderKind::Wizard);

    let header_h     = if has_header   { style.header_height()      } else { 0.0 };
    let tab_h        = if has_top_tabs { style.tab_height()         } else { 0.0 };
    let footer_h     = if has_footer   { style.footer_height()      } else { 0.0 };
    let wizard_nav_h = if has_wizard   { style.wizard_nav_height()  } else { 0.0 };
    let sidebar_w    = if has_sidebar  { effective_sidebar_width(view, style, kind) } else { 0.0 };

    let extra_w = sidebar_w;
    let extra_h = header_h + tab_h + footer_h + wizard_nav_h;
    (extra_w, extra_h)
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
    let sidebar_w    = if has_sidebar { effective_sidebar_width(view, style, kind) } else { 0.0 };
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

    ModalLayout {
        header,
        close_btn,
        drag_handle,
        tab_strip,
        sidebar,
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

// ---------------------------------------------------------------------------
// Body overflow strips & resize handles
// ---------------------------------------------------------------------------

/// Width / height of the resize-handle strips along each modal edge. Corners
/// inherit double-axis behaviour by overlapping two strips.
const RESIZE_HANDLE_THICKNESS: f64 = 6.0;

pub fn register_body_overflow(
    coord:    &mut InputCoordinator,
    modal_id: &WidgetId,
    frame:    Rect,
    view:     &ModalView<'_>,
    settings: &ModalSettings,
    kind:     &ModalRenderKind,
    state:    &mut ModalState,
) {
    let body = body_rect(frame, view, settings, kind);
    if body.width <= 0.0 || body.height <= 0.0 {
        return;
    }
    // Cache body geometry on state so input helpers can drive scroll math
    // without the host having to remember anything.
    state.body_viewport_h = body.height;
    state.body_viewport_w = body.width;

    match view.overflow {
        crate::types::OverflowMode::Scrollbar => {
            let track_w = 8.0_f64;
            let track = Rect::new(body.x + body.width - track_w, body.y, track_w, body.height);
            state.body_scroll_track = Some(track);
            coord.register_child(modal_id, format!("{}:scrollbar_track", modal_id.0),
                WidgetKind::ScrollbarTrack, track, Sense::CLICK);
            coord.register_child(modal_id, format!("{}:scrollbar_handle", modal_id.0),
                WidgetKind::ScrollbarHandle, track, Sense::DRAG | Sense::HOVER);
        }
        crate::types::OverflowMode::Chevrons => {
            let strip = 26.0_f64;
            // Vertical strips
            if state.body_content_h > body.height + 0.5 {
                let up = Rect::new(body.x, body.y, body.width, strip);
                let dn = Rect::new(body.x, body.y + body.height - strip, body.width, strip);
                coord.register_child(modal_id, format!("{}:chevron_up", modal_id.0),
                    WidgetKind::Button, up, Sense::CLICK | Sense::HOVER);
                coord.register_child(modal_id, format!("{}:chevron_down", modal_id.0),
                    WidgetKind::Button, dn, Sense::CLICK | Sense::HOVER);
            }
            // Horizontal strips
            if state.body_content_w > body.width + 0.5 {
                let lf = Rect::new(body.x, body.y, strip, body.height);
                let rt = Rect::new(body.x + body.width - strip, body.y, strip, body.height);
                coord.register_child(modal_id, format!("{}:chevron_left", modal_id.0),
                    WidgetKind::Button, lf, Sense::CLICK | Sense::HOVER);
                coord.register_child(modal_id, format!("{}:chevron_right", modal_id.0),
                    WidgetKind::Button, rt, Sense::CLICK | Sense::HOVER);
            }
        }
        _ => {}
    }
}

fn register_resize_handles(coord: &mut InputCoordinator, modal_id: &WidgetId, frame: Rect) {
    let t = RESIZE_HANDLE_THICKNESS;
    // Edges: N / S / W / E and four corners (NW NE SW SE).
    let edges = [
        ("resize_n",  Rect::new(frame.x, frame.y, frame.width, t)),
        ("resize_s",  Rect::new(frame.x, frame.y + frame.height - t, frame.width, t)),
        ("resize_w",  Rect::new(frame.x, frame.y, t, frame.height)),
        ("resize_e",  Rect::new(frame.x + frame.width - t, frame.y, t, frame.height)),
        ("resize_nw", Rect::new(frame.x, frame.y, t * 2.0, t * 2.0)),
        ("resize_ne", Rect::new(frame.x + frame.width - t * 2.0, frame.y, t * 2.0, t * 2.0)),
        ("resize_sw", Rect::new(frame.x, frame.y + frame.height - t * 2.0, t * 2.0, t * 2.0)),
        ("resize_se", Rect::new(frame.x + frame.width - t * 2.0, frame.y + frame.height - t * 2.0, t * 2.0, t * 2.0)),
    ];
    for (suffix, rect) in edges {
        coord.register_child(
            modal_id,
            format!("{}:{}", modal_id.0, suffix),
            WidgetKind::DragHandle,
            rect,
            Sense::DRAG | Sense::HOVER,
        );
    }
}
