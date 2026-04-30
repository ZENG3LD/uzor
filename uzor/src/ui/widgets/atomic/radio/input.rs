//! InputCoordinator registration helpers for the radio widget.

use crate::app_context::ContextManager;
use crate::docking::panels::DockPanel;
use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::layout::{LayoutManager, LayoutNodeId, WidgetNode};
use crate::render::RenderContext;
use crate::types::{Rect, WidgetId, WidgetState};

use super::render::draw_radio;
use super::settings::RadioSettings;
use super::state::RadioState;
use super::types::RadioRenderKind;

/// Register a radio widget with the coordinator for this frame.
pub fn register_radio(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::Radio, rect, Sense::CLICK, layer);
}

/// Level 1 — register a radio widget with an explicit `InputCoordinator`.
pub fn register_input_coordinator_radio(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    _state: &mut RadioState,
) {
    coord.register_atomic(id, WidgetKind::Radio, rect, Sense::CLICK, layer);
}

/// Level 2 — register a radio widget via `ContextManager`, pulling state from the registry,
/// and draw it using the provided render context.
///
/// `widget_state` is supplied by the caller — the app owns the hover/press state machine.
/// `settings` supplies visual style. `kind` selects the render variant
/// (Group, Pair, or Custom).
pub fn register_context_manager_radio(
    ctx: &mut ContextManager,
    render: &mut dyn RenderContext,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    widget_state: WidgetState,
    settings: &RadioSettings,
    kind: &RadioRenderKind<'_>,
) {
    let id: WidgetId = id.into();
    let state = ctx.registry.get_or_insert_with(id.clone(), RadioState::default);
    register_input_coordinator_radio(&mut ctx.input, id, rect, layer, state);
    draw_radio(render, rect, widget_state, settings, kind);
}

/// Level 3 — register a radio widget via `LayoutManager`.
pub fn register_layout_manager_radio<P: DockPanel>(
    layout: &mut LayoutManager<P>,
    render: &mut dyn RenderContext,
    parent: LayoutNodeId,
    id: impl Into<WidgetId>,
    rect: Rect,
    widget_state: WidgetState,
    settings: &RadioSettings,
    kind: &RadioRenderKind<'_>,
) {
    let id: WidgetId = id.into();
    let layer = layout.compute_layer_for(parent);
    layout.tree_mut().add_widget(parent, WidgetNode { id: id.clone(), kind: WidgetKind::Radio, rect, sense: Sense::CLICK });
    register_context_manager_radio(
        layout.ctx_mut(), render, id, rect, &layer, widget_state, settings, kind,
    );
}
