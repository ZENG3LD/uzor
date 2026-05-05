//! Container input registration.
//!
//! **mlc finding:** plain containers have no input handling. The only mlc
//! container with any interaction surface is `TradingContainer`, whose
//! `handle_click` is a stub returning `false`. Popup / Modal containers
//! register hit-test helpers as stand-alone functions, not inside the
//! container primitive.
//!
//! `register` defaults to `Sense::NONE`. A `Sense::CLICK` overload is provided
//! for the rare case where a container acts as a clickable backdrop (e.g. a
//! dismissal surface behind a popup).

use crate::app_context::ContextManager;
use crate::layout::docking::DockPanel;
use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::layout::{LayoutManager, LayoutNodeId, WidgetNode};
use crate::render::RenderContext;
use crate::types::{Rect, WidgetId};

use super::render::{draw_container, ContainerView};
use super::settings::ContainerSettings;
use super::state::ContainerState;

/// Register a non-interactive container in the input coordinator.
///
/// Use this for all plain/bordered/section/panel containers that do not handle
/// clicks. The coordinator still tracks them for z-ordering purposes.
pub fn register(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::Custom, rect, Sense::NONE, layer);
}

/// Register a clickable container in the input coordinator.
///
/// Use when the container surface itself should absorb pointer events
/// (e.g. a backdrop that dismisses a floating panel on click).
pub fn register_clickable(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::Custom, rect, Sense::CLICK, layer);
}

/// Level 1 — register a non-interactive container with an explicit `InputCoordinator`.
pub fn register_input_coordinator_container(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    _state: &mut ContainerState,
) {
    coord.register_atomic(id, WidgetKind::Custom, rect, Sense::NONE, layer);
}

/// Level 2 — register a container via `ContextManager`, pulling state from the registry,
/// and draw it using the provided render context.
///
/// `view` selects the container variant and border flag. `settings` supplies theme and style.
pub fn register_context_manager_container(
    ctx: &mut ContextManager,
    render: &mut dyn RenderContext,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    view: &ContainerView,
    settings: &ContainerSettings,
) {
    let id: WidgetId = id.into();
    let state = ctx.registry.get_or_insert_with(id.clone(), ContainerState::default);
    register_input_coordinator_container(&mut ctx.input, id, rect, layer, state);
    draw_container(render, rect, view, settings.theme.as_ref(), settings.style.as_ref());
}

/// Level 3 — register a container via `LayoutManager`, forwarding to L2.
pub fn register_layout_manager_container<P: DockPanel>(
    layout: &mut LayoutManager<P>,
    render: &mut dyn RenderContext,
    parent: LayoutNodeId,
    id: impl Into<WidgetId>,
    rect: Rect,
    view: &ContainerView,
    settings: &ContainerSettings,
) {
    let id: WidgetId = id.into();
    let layer = layout.compute_layer_for(parent);
    layout.tree_mut().add_widget(parent, WidgetNode { id: id.clone(), kind: WidgetKind::Custom, rect, sense: Sense::NONE });
    register_context_manager_container(
        layout.ctx_mut(), render, id, rect, &layer, view, settings,
    );
}
