//! InputCoordinator registration helpers for toast widgets.

use crate::app_context::ContextManager;
use crate::docking::panels::DockPanel;
use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::layout::{LayoutManager, LayoutNodeId, WidgetNode};
use crate::render::RenderContext;
use crate::types::{Rect, WidgetId};

use super::render::draw_toast;
use super::settings::ToastSettings;
use super::state::{ToastEntry, ToastStackState};

/// Register a toast widget with the coordinator for this frame.
/// Toast is hover-only (so user can pause auto-dismiss by hovering).
pub fn register(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::Tooltip, rect, Sense::HOVER, layer);
}

/// Level 1 — register a toast widget with an explicit `InputCoordinator`.
///
/// Each visible toast entry should be registered with its own rect and id.
/// `state` holds the entire stack; individual toast display is managed by the
/// render helpers (`draw_toast_stack`, `draw_toast_at`).
pub fn register_input_coordinator_toast(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    _state: &mut ToastStackState,
) {
    coord.register_atomic(id, WidgetKind::Tooltip, rect, Sense::HOVER, layer);
}

/// Level 2 — register a toast widget via `ContextManager`, pulling `ToastStackState`
/// from the registry, and draw it using the provided render context.
///
/// `entry` supplies the toast data to render. `settings` supplies visual style.
/// `now_ms` is the current timestamp in milliseconds for fade-out calculation.
pub fn register_context_manager_toast(
    ctx: &mut ContextManager,
    render: &mut dyn RenderContext,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    entry: &ToastEntry,
    settings: &ToastSettings,
    now_ms: u64,
) {
    let id: WidgetId = id.into();
    let state = ctx.registry.get_or_insert_with(id.clone(), ToastStackState::default);
    register_input_coordinator_toast(&mut ctx.input, id, rect, layer, state);
    draw_toast(render, rect, entry, settings, now_ms);
}

/// Level 3 — register a toast widget via `LayoutManager`.
pub fn register_layout_manager_toast<P: DockPanel>(
    layout: &mut LayoutManager<P>,
    render: &mut dyn RenderContext,
    parent: LayoutNodeId,
    id: impl Into<WidgetId>,
    rect: Rect,
    entry: &ToastEntry,
    settings: &ToastSettings,
    now_ms: u64,
) {
    let id: WidgetId = id.into();
    let layer = layout.compute_layer_for(parent);
    layout.tree_mut().add_widget(parent, WidgetNode { id: id.clone(), kind: WidgetKind::Tooltip, rect, sense: Sense::HOVER });
    register_context_manager_toast(
        layout.ctx_mut(), render, id, rect, &layer, entry, settings, now_ms,
    );
}
