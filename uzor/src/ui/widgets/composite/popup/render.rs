//! Popup composite render — universal transient surface.
//!
//! # API
//!
//! - `register_input_coordinator_popup` — registers the composite + body
//!   overflow strips with an `InputCoordinator`. **No drawing.**
//! - `register_context_manager_popup`   — convenience wrapper: registers,
//!   draws the chrome, and (for `Custom`) hands off to the caller closure.
//! - `register_layout_manager_popup`    — public entry that resolves a
//!   layout overlay slot and drives the above.
//!
//! # Draw order (Plain)
//!
//! 1. Backdrop fill (when `BackdropKind::Dim`)
//! 2. Shadow rect
//! 3. Frame background + border (`BackgroundFill` dispatch)
//! 4. Body — caller draws inside `body_rect()` after the call returns
//! 5. Body scrollbar (when `OverflowMode::Scrollbar`)

use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::render::RenderContext;
use crate::types::{Rect, WidgetId, CompositeId};

use super::settings::PopupSettings;
use super::state::PopupState;
use super::style::BackgroundFill;
use super::types::{BackdropKind, PopupRenderKind, PopupView, PopupViewKind};

// ---------------------------------------------------------------------------
// Public API — registration only
// ---------------------------------------------------------------------------

/// Register the popup composite and body overflow hit-rects.  No drawing.
pub fn register_input_coordinator_popup(
    coord:    &mut InputCoordinator,
    id:       impl Into<WidgetId>,
    rect:     Rect,
    state:    &mut PopupState,
    view:     &PopupView<'_>,
    _settings: &PopupSettings,
    kind:     PopupRenderKind,
    layer:    &LayerId,
) -> CompositeId {
    let popup_id = coord.register_composite(id, WidgetKind::Popup, rect, Sense::CLICK, layer);

    if matches!(kind, PopupRenderKind::Custom) {
        return popup_id;
    }

    let frame = resolve_frame(rect, state, view);
    let body  = body_rect(frame, _settings);

    register_popup_body_overflow(coord, &popup_id, body, view, state);
    popup_id
}

/// Body overflow hit-rects (Scrollbar / Chevrons / fallback when Clip overflows).
fn register_popup_body_overflow(
    coord:    &mut InputCoordinator,
    popup_id: &CompositeId,
    body:     Rect,
    view:     &PopupView<'_>,
    state:    &mut PopupState,
) {
    if body.width <= 0.0 || body.height <= 0.0 { return; }
    state.body_viewport_h = body.height;

    let scroll = crate::ui::widgets::composite::overflow::BodyScrollState {
        offset_x:  0.0,
        offset_y:  state.scroll.offset,
        content_w: 0.0,
        content_h: state.body_content_h,
    };
    let layer = crate::input::core::coordinator::LayerId::main();
    let overflowing = scroll.overflows(body.width, body.height).any();
    let effective = match view.overflow {
        crate::types::OverflowMode::Scrollbar => crate::types::OverflowMode::Scrollbar,
        crate::types::OverflowMode::Chevrons  => crate::types::OverflowMode::Chevrons,
        _ if overflowing => crate::types::OverflowMode::Chevrons,
        other             => other,
    };
    match effective {
        crate::types::OverflowMode::Scrollbar => {
            if let Some(track) = crate::ui::widgets::composite::overflow::register_scrollbar_helper(
                coord, popup_id, body, &scroll,
                crate::ui::widgets::composite::overflow::ScrollAxis::Vertical,
                &layer,
            ) {
                state.body_scroll_track = Some(track);
            }
        }
        crate::types::OverflowMode::Chevrons => {
            crate::ui::widgets::composite::overflow::register_chevrons_helper(
                coord, popup_id, body, &scroll, &layer,
            );
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Public API — convenience wrapper (ContextManager)
// ---------------------------------------------------------------------------

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
) -> CompositeId {
    let coord = &mut ctx_mgr.input;
    let popup_id =
        register_input_coordinator_popup(coord, id, rect, state, view, settings, kind, layer);
    draw_popup(render, rect, state, view, settings, kind);
    popup_id
}

// ---------------------------------------------------------------------------
// Internal draw pipeline
// ---------------------------------------------------------------------------

fn draw_popup(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    state:    &mut PopupState,
    view:     &mut PopupView<'_>,
    settings: &PopupSettings,
    kind:     PopupRenderKind,
) {
    if let PopupViewKind::Custom { ref draw } = view.kind {
        // Custom — caller drives every paint call (incl. backdrop/frame).
        draw(ctx, rect, state, settings);
        return;
    }
    if !matches!(kind, PopupRenderKind::Plain) {
        return;
    }

    let theme = settings.theme.as_ref();
    let style = settings.style.as_ref();
    let frame = resolve_frame(rect, state, view);

    // 1. Backdrop
    if matches!(view.backdrop, BackdropKind::Dim) {
        ctx.set_fill_color(theme.backdrop_dim());
        ctx.fill_rect(0.0, 0.0, 99_999.0, 99_999.0);
    }

    // 2. Shadow
    let (sx, sy) = style.shadow_offset();
    ctx.set_fill_color(theme.shadow());
    ctx.fill_rounded_rect(
        frame.x + sx, frame.y + sy,
        frame.width, frame.height,
        style.radius(),
    );

    // 3. Frame background + border
    draw_frame(ctx, frame, settings);

    // 4. Body — caller draws after this returns inside `body_rect(frame)`.

    // 5. Body scrollbar (Scrollbar overflow only).
    if matches!(view.overflow, crate::types::OverflowMode::Scrollbar) {
        if let Some(track) = state.body_scroll_track {
            use crate::ui::widgets::atomic::scrollbar::{
                render::{draw_scrollbar, ScrollbarView, ScrollbarVisualState},
                style::StandardScrollbarStyle,
                theme::DefaultScrollbarTheme,
            };
            let sb_style = StandardScrollbarStyle::default();
            let sb_theme = DefaultScrollbarTheme::default();
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
                style:           &sb_style,
                theme:           &sb_theme,
            };
            let _ = draw_scrollbar(ctx, track, &sv);
        }
    }
}

// ---------------------------------------------------------------------------
// Frame helper
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
// Geometry
// ---------------------------------------------------------------------------

fn resolve_frame(rect: Rect, state: &PopupState, _view: &PopupView<'_>) -> Rect {
    if state.position.0 != 0.0 || state.position.1 != 0.0 {
        Rect::new(state.position.0, state.position.1, rect.width, rect.height)
    } else {
        rect
    }
}

/// Public helper: rect available for caller-drawn body content. Equals
/// `frame` shrunk by `style.padding()` on each side.
pub fn body_rect(frame: Rect, settings: &PopupSettings) -> Rect {
    let pad = settings.style.padding();
    Rect::new(
        frame.x + pad,
        frame.y + pad,
        (frame.width  - pad * 2.0).max(0.0),
        (frame.height - pad * 2.0).max(0.0),
    )
}
