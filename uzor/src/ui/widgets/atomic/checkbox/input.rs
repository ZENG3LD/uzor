//! InputCoordinator registration helpers for the checkbox widget.

use crate::app_context::ContextManager;
use crate::layout::docking::DockPanel;
use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::layout::{LayoutManager, LayoutNodeId, WidgetNode};
use crate::render::RenderContext;
use crate::types::{Rect, WidgetId, WidgetState};

use super::render::draw_checkbox;
use super::settings::CheckboxSettings;
use super::state::CheckboxState;
use super::types::{CheckboxRenderKind, CheckboxView};

/// Register a checkbox widget with the coordinator for this frame.
pub fn register_checkbox(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::Checkbox, rect, Sense::CLICK, layer);
}

/// Level 1 — register a checkbox with an explicit `InputCoordinator`.
pub fn register_input_coordinator_checkbox(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    _state: &mut CheckboxState,
) {
    coord.register_atomic(id, WidgetKind::Checkbox, rect, Sense::CLICK, layer);
}

/// Level 2 — register a checkbox via `ContextManager`, pulling state from the registry,
/// and draw it using the provided render context.
///
/// `widget_state` is supplied by the caller — the app owns the hover/press state machine.
/// `view` supplies per-frame data (checked, label). `settings` supplies visual style.
/// `kind` selects the visual variant. `font` is the label font string.
pub fn register_context_manager_checkbox(
    ctx: &mut ContextManager,
    render: &mut dyn RenderContext,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    widget_state: WidgetState,
    view: &CheckboxView<'_>,
    settings: &CheckboxSettings,
    kind: &CheckboxRenderKind<'_>,
    font: &str,
) {
    let id: WidgetId = id.into();
    let state = ctx.registry.get_or_insert_with(id.clone(), CheckboxState::default);
    register_input_coordinator_checkbox(&mut ctx.input, id, rect, layer, state);
    draw_checkbox(render, rect, widget_state, view, settings, kind, font);
}

/// Level 3 — register a checkbox via `LayoutManager`.
pub fn register_layout_manager_checkbox<P: DockPanel>(
    layout: &mut LayoutManager<P>,
    render: &mut dyn RenderContext,
    parent: LayoutNodeId,
    id: impl Into<WidgetId>,
    rect: Rect,
    widget_state: WidgetState,
    view: &CheckboxView<'_>,
    settings: &CheckboxSettings,
    kind: &CheckboxRenderKind<'_>,
    font: &str,
) {
    let id: WidgetId = id.into();
    let layer = layout.compute_layer_for(parent);
    layout.tree_mut().add_widget(parent, WidgetNode { id: id.clone(), kind: WidgetKind::Checkbox, rect, sense: Sense::CLICK });
    register_context_manager_checkbox(
        layout.ctx_mut(), render, id, rect, &layer, widget_state, view, settings, kind, font,
    );
}
