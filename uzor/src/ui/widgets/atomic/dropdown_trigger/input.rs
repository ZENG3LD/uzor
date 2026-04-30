//! InputCoordinator registration helpers for dropdown trigger widgets.

use crate::app_context::ContextManager;
use crate::docking::panels::DockPanel;
use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::layout::LayoutManager;
use crate::render::RenderContext;
use crate::types::{Rect, WidgetId, WidgetState};

use super::render::draw_dropdown_trigger;
use super::settings::DropdownTriggerSettings;
use super::state::DropdownTriggerState;
use super::types::DropdownTriggerRenderKind;

/// Register a dropdown trigger widget with the coordinator for this frame.
pub fn register_dropdown_trigger(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::DropdownTrigger, rect, Sense::CLICK, layer);
}

/// Level 1 — register a dropdown trigger with an explicit `InputCoordinator`.
pub fn register_input_coordinator_dropdown_trigger(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    _state: &mut DropdownTriggerState,
) {
    coord.register_atomic(id, WidgetKind::DropdownTrigger, rect, Sense::CLICK, layer);
}

/// Level 2 — register a dropdown trigger via `ContextManager`, pulling state from the registry,
/// and draw it using the provided render context.
///
/// `settings` supplies visual style. `kind` selects the render variant.
pub fn register_context_manager_dropdown_trigger(
    ctx: &mut ContextManager,
    render: &mut dyn RenderContext,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    settings: &DropdownTriggerSettings,
    kind: &DropdownTriggerRenderKind<'_>,
) {
    let id: WidgetId = id.into();
    let state = ctx.registry.get_or_insert_with(id.clone(), DropdownTriggerState::default);
    register_input_coordinator_dropdown_trigger(&mut ctx.input, id, rect, layer, state);
    draw_dropdown_trigger(render, rect, WidgetState::Normal, settings, kind);
}

/// Level 3 — register a dropdown trigger via `LayoutManager`, forwarding to L2.
pub fn register_layout_manager_dropdown_trigger<P: DockPanel>(
    layout: &mut LayoutManager<P>,
    render: &mut dyn RenderContext,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    settings: &DropdownTriggerSettings,
    kind: &DropdownTriggerRenderKind<'_>,
) {
    register_context_manager_dropdown_trigger(
        layout.ctx_mut(), render, id, rect, layer, settings, kind,
    );
}
