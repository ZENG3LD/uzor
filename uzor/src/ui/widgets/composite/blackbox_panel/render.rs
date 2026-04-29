//! BlackboxPanel render entry point and per-kind layout pipelines.
//!
//! # API convention
//!
//! - `register_input_coordinator_blackbox_panel` — registers ONE composite rect.
//!   **No drawing.**  No children (BlackboxPanel rejects children).
//! - `register_context_manager_blackbox_panel`   — registers + draws in one call.
//!
//! # Draw order (non-Custom kinds)
//!
//! **Default:**
//! 1. Background fill (`theme.bg()`)
//! 2. Body closure (`view.body(ctx, body_rect)`)
//!
//! **WithBorder:**
//! 1. Background fill
//! 2. 1 px border (`theme.border()`)
//! 3. Body closure
//!
//! **WithHeader:**
//! 1. Header rect fill (`theme.header_bg()`)
//! 2. Title text (left-aligned, vertically centred)
//! 3. Divider at header bottom
//! 4. Body fill (`theme.bg()`)
//! 5. Body closure
//!
//! **WithHeaderBorder:**
//! 1–5 same as WithHeader
//! 6. 1 px border around full rect

use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, WidgetKind};
use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::{Rect, WidgetId};

use super::settings::BlackboxPanelSettings;
use super::state::BlackboxState;
use super::style::BackgroundFill;
use super::types::{BlackboxRenderKind, BlackboxView};

// ---------------------------------------------------------------------------
// Internal layout struct
// ---------------------------------------------------------------------------

struct BlackboxLayout {
    /// Full panel rect.
    frame: Rect,
    /// Header strip rect; zero height when not rendered.
    header: Rect,
    /// Body area passed to the body closure.
    body: Rect,
}

// ---------------------------------------------------------------------------
// Public API — register (InputCoordinator only, no drawing)
// ---------------------------------------------------------------------------

/// Register the blackbox panel composite with the input coordinator.
///
/// Registers ONE rect — no children (blackbox panels reject children).
/// The `view.sense` flags control which events the coordinator forwards.
///
/// Returns the `WidgetId` assigned to the panel.
pub fn register_input_coordinator_blackbox_panel(
    coord:    &mut InputCoordinator,
    id:       impl Into<WidgetId>,
    rect:     Rect,
    _state:   &BlackboxState,
    view:     &BlackboxView<'_>,
    _settings: &BlackboxPanelSettings,
    _kind:    &BlackboxRenderKind,
    layer:    &LayerId,
) -> WidgetId {
    // BlackboxPanel is a composite kind; register_composite is the correct call.
    // No children are registered — BlackboxPanel rejects them at coordinator level.
    coord.register_composite(id, WidgetKind::BlackboxPanel, rect, view.sense, layer)
}

// ---------------------------------------------------------------------------
// Public API — convenience wrapper (ContextManager)
// ---------------------------------------------------------------------------

/// Register + draw the blackbox panel in one call.
///
/// Returns the `WidgetId` assigned to the panel.
pub fn register_context_manager_blackbox_panel(
    ctx_mgr:  &mut crate::app_context::ContextManager,
    render:   &mut dyn RenderContext,
    id:       impl Into<WidgetId>,
    rect:     Rect,
    state:    &mut BlackboxState,
    view:     &mut BlackboxView<'_>,
    settings: &BlackboxPanelSettings,
    kind:     &BlackboxRenderKind,
    layer:    &LayerId,
) -> WidgetId {
    let panel_id = register_input_coordinator_blackbox_panel(
        &mut ctx_mgr.input,
        id,
        rect,
        state,
        view,
        settings,
        kind,
        layer,
    );
    draw_blackbox(render, rect, view, settings, kind);
    panel_id
}

// ---------------------------------------------------------------------------
// Internal draw pipeline
// ---------------------------------------------------------------------------

fn draw_blackbox(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    view:     &mut BlackboxView<'_>,
    settings: &BlackboxPanelSettings,
    kind:     &BlackboxRenderKind,
) {
    if let BlackboxRenderKind::Custom(f) = kind {
        f(ctx, rect, view, settings);
        return;
    }

    let theme  = settings.theme.as_ref();
    let style  = settings.style.as_ref();
    let layout = compute_layout(rect, settings, kind);

    let has_header = matches!(
        kind,
        BlackboxRenderKind::WithHeader | BlackboxRenderKind::WithHeaderBorder
    );
    let has_border = matches!(
        kind,
        BlackboxRenderKind::WithBorder | BlackboxRenderKind::WithHeaderBorder
    );

    // --- 1. Background fill --------------------------------------------------
    match style.background_fill() {
        BackgroundFill::Solid => {
            ctx.set_fill_color(theme.bg());
            ctx.fill_rect(layout.frame.x, layout.frame.y, layout.frame.width, layout.frame.height);
        }
        BackgroundFill::Glass { blur_radius: _ } => {
            ctx.draw_blur_background(
                layout.frame.x,
                layout.frame.y,
                layout.frame.width,
                layout.frame.height,
            );
            ctx.set_fill_color(theme.bg());
            ctx.fill_rect(layout.frame.x, layout.frame.y, layout.frame.width, layout.frame.height);
        }
    }

    // --- 2. Header strip -----------------------------------------------------
    if has_header && layout.header.height > 0.0 {
        ctx.set_fill_color(theme.header_bg());
        ctx.fill_rect(
            layout.header.x,
            layout.header.y,
            layout.header.width,
            layout.header.height,
        );

        if let Some(title) = view.title {
            let pad = style.padding();
            ctx.set_fill_color(theme.header_text());
            ctx.set_font("12px sans-serif");
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text(
                title,
                layout.header.x + pad,
                layout.header.y + layout.header.height / 2.0,
            );
        }

        // Divider at header bottom
        ctx.set_fill_color(theme.divider());
        ctx.fill_rect(
            layout.header.x,
            layout.header.y + layout.header.height - 1.0,
            layout.header.width,
            1.0,
        );
    }

    // --- 3. Border -----------------------------------------------------------
    if has_border {
        let bw = style.border_width();
        ctx.set_stroke_color(theme.border());
        ctx.set_stroke_width(bw);
        ctx.stroke_rect(layout.frame.x, layout.frame.y, layout.frame.width, layout.frame.height);
    }

    // --- 4. Body closure -----------------------------------------------------
    (view.body)(ctx, layout.body);
}

// ---------------------------------------------------------------------------
// Layout computation
// ---------------------------------------------------------------------------

fn compute_layout(
    rect:     Rect,
    settings: &BlackboxPanelSettings,
    kind:     &BlackboxRenderKind,
) -> BlackboxLayout {
    let style = settings.style.as_ref();

    let has_header = matches!(
        kind,
        BlackboxRenderKind::WithHeader | BlackboxRenderKind::WithHeaderBorder
    );

    let header_h = if has_header { style.header_height() } else { 0.0 };

    let header = Rect::new(rect.x, rect.y, rect.width, header_h);

    let body_y = rect.y + header_h;
    let body_h = (rect.height - header_h).max(0.0);
    let body   = Rect::new(rect.x, body_y, rect.width, body_h);

    BlackboxLayout { frame: rect, header, body }
}
