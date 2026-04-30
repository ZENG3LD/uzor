//! InputCoordinator registration helpers for close button widgets.

use crate::app_context::ContextManager;
use crate::docking::panels::DockPanel;
use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::layout::LayoutManager;
use crate::render::RenderContext;
use crate::types::{Rect, WidgetId, WidgetState};

use super::render::{draw_close_button, CloseButtonView};
use super::settings::CloseButtonSettings;
use super::state::CloseButtonState;
use super::types::CloseButtonRenderKind;

/// Register a close button widget with the coordinator for this frame.
pub fn register(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::CloseButton, rect, Sense::CLICK, layer);
}

/// Level 1 — register a close button with an explicit `InputCoordinator`.
pub fn register_input_coordinator_close_button(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    _state: &mut CloseButtonState,
) {
    coord.register_atomic(id, WidgetKind::CloseButton, rect, Sense::CLICK, layer);
}

/// Level 2 — register a close button via `ContextManager`, pulling state from the registry,
/// and draw it using the provided render context.
///
/// `widget_state` is supplied by the caller — the app owns the hover/press state machine.
/// `view` supplies per-frame data. `settings` supplies visual style.
/// `kind` selects the render variant.
pub fn register_context_manager_close_button(
    ctx: &mut ContextManager,
    render: &mut dyn RenderContext,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    widget_state: WidgetState,
    view: &CloseButtonView,
    settings: &CloseButtonSettings,
    kind: &CloseButtonRenderKind,
) {
    let id: WidgetId = id.into();
    let state = ctx.registry.get_or_insert_with(id.clone(), CloseButtonState::default);
    register_input_coordinator_close_button(&mut ctx.input, id, rect, layer, state);
    draw_close_button(render, rect, widget_state, view, settings, kind);
}

/// Level 3 — register a close button via `LayoutManager`, forwarding to L2.
pub fn register_layout_manager_close_button<P: DockPanel>(
    layout: &mut LayoutManager<P>,
    render: &mut dyn RenderContext,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    widget_state: WidgetState,
    view: &CloseButtonView,
    settings: &CloseButtonSettings,
    kind: &CloseButtonRenderKind,
) {
    register_context_manager_close_button(
        layout.ctx_mut(), render, id, rect, layer, widget_state, view, settings, kind,
    );
}
