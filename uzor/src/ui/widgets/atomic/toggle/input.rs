//! InputCoordinator registration helpers for the toggle widget.

use crate::app_context::ContextManager;
use crate::layout::docking::DockPanel;
use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::layout::{LayoutManager, LayoutNodeId, WidgetNode};
use crate::render::RenderContext;
use crate::types::{IconId, Rect, WidgetId, WidgetState};

use super::render::draw_toggle;
use super::settings::ToggleSettings;
use super::state::ToggleState;
use super::types::{ToggleRenderKind, ToggleView};

/// Register a toggle widget with the coordinator for this frame.
pub fn register_toggle(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::Toggle, rect, Sense::CLICK, layer);
}

/// Level 1 — register a toggle with an explicit `InputCoordinator`.
pub fn register_input_coordinator_toggle(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    _state: &mut ToggleState,
) {
    coord.register_atomic(id, WidgetKind::Toggle, rect, Sense::CLICK, layer);
}

/// Level 2 — register a toggle via `ContextManager`, pulling state from the registry,
/// and draw it using the provided render context.
///
/// `widget_state` is supplied by the caller — the app owns the hover/press state machine.
/// `view` supplies per-frame toggled, label, and disabled state.
/// `settings` supplies visual style. `kind` selects the render variant.
pub fn register_context_manager_toggle(
    ctx: &mut ContextManager,
    render: &mut dyn RenderContext,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    widget_state: WidgetState,
    view: &ToggleView<'_>,
    settings: &ToggleSettings,
    kind: &ToggleRenderKind<'_>,
) {
    let id: WidgetId = id.into();
    let state = ctx.registry.get_or_insert_with(id.clone(), ToggleState::default);
    register_input_coordinator_toggle(&mut ctx.input, id, rect, layer, state);
    draw_toggle(render, rect, widget_state, view, settings, kind, |_, _: &IconId, _, _| {});
}

/// Level 3 — register a toggle via `LayoutManager`.
pub fn register_layout_manager_toggle<P: DockPanel>(
    layout: &mut LayoutManager<P>,
    render: &mut dyn RenderContext,
    parent: LayoutNodeId,
    id: impl Into<WidgetId>,
    rect: Rect,
    widget_state: WidgetState,
    view: &ToggleView<'_>,
    settings: &ToggleSettings,
    kind: &ToggleRenderKind<'_>,
) {
    let id: WidgetId = id.into();
    let layer = layout.compute_layer_for(parent);
    layout.tree_mut().add_widget(parent, WidgetNode { id: id.clone(), kind: WidgetKind::Toggle, rect, sense: Sense::CLICK, label: None });
    register_context_manager_toggle(
        layout.ctx_mut(), render, id, rect, &layer, widget_state, view, settings, kind,
    );
}
