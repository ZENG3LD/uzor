//! InputCoordinator registration helpers for button widgets.

use crate::app_context::ContextManager;
use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::layout::LayoutManager;
use crate::render::RenderContext;
use crate::types::{Rect, WidgetId, WidgetState};
use crate::docking::panels::DockPanel;

use super::render::{draw_button, ButtonView};
use super::settings::ButtonSettings;
use super::state::ButtonState;

/// Register a button widget with the coordinator for this frame.
pub fn register(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::Button, rect, Sense::CLICK, layer);
}

/// Level 1 — register a button with an explicit `InputCoordinator`.
pub fn register_input_coordinator_button(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    _state: &mut ButtonState,
) {
    coord.register_atomic(id, WidgetKind::Button, rect, Sense::CLICK, layer);
}

/// Level 2 — register a button via `ContextManager`, pulling state from the registry,
/// and draw it using the provided render context.
///
/// `widget_state` is supplied by the caller — the app owns the hover/press/focus
/// state machine and passes the appropriate `WidgetState` each frame.
/// `view` supplies per-frame data (label, icon, active/disabled flags).
/// `settings` supplies the visual style and theme.
pub fn register_context_manager_button(
    ctx: &mut ContextManager,
    render: &mut dyn RenderContext,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    widget_state: WidgetState,
    view: &ButtonView<'_>,
    settings: &ButtonSettings,
) {
    let id: WidgetId = id.into();
    let state = ctx.registry.get_or_insert_with(id.clone(), ButtonState::default);
    register_input_coordinator_button(&mut ctx.input, id, rect, layer, state);
    draw_button(render, rect, widget_state, view, settings, |_, _, _, _| {});
}

/// Level 3 — register a button via `LayoutManager`, forwarding to L2.
///
/// `LayoutManager` owns the `ContextManager`; this is a thin wrapper that extracts it.
pub fn register_layout_manager_button<P: DockPanel>(
    layout: &mut LayoutManager<P>,
    render: &mut dyn RenderContext,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    widget_state: WidgetState,
    view: &ButtonView<'_>,
    settings: &ButtonSettings,
) {
    register_context_manager_button(layout.ctx_mut(), render, id, rect, layer, widget_state, view, settings);
}
