//! InputCoordinator registration helpers for scroll chevron widgets.

use crate::app_context::ContextManager;
use crate::layout::docking::DockPanel;
use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::layout::{LayoutManager, LayoutNodeId, WidgetNode};
use crate::render::RenderContext;
use crate::types::{Rect, WidgetId, WidgetState};

use super::render::{draw_scroll_chevron, ScrollChevronView};
use super::settings::ScrollChevronSettings;
use super::state::ScrollChevronState;
use super::types::ScrollChevronRenderKind;

/// Register a scroll chevron widget with the coordinator for this frame.
pub fn register(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::ScrollChevron, rect, Sense::CLICK, layer);
}

/// Level 1 — register a scroll chevron with an explicit `InputCoordinator`.
pub fn register_input_coordinator_scroll_chevron(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    _state: &mut ScrollChevronState,
) {
    coord.register_atomic(id, WidgetKind::ScrollChevron, rect, Sense::CLICK, layer);
}

/// Level 2 — register a scroll chevron via `ContextManager`, pulling state from the registry,
/// and draw it using the provided render context.
///
/// `widget_state` is supplied by the caller — the app owns the hover/press state machine.
/// `view` supplies per-frame direction and disabled state.
/// `settings` supplies visual style. `kind` selects the render variant.
pub fn register_context_manager_scroll_chevron(
    ctx: &mut ContextManager,
    render: &mut dyn RenderContext,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    widget_state: WidgetState,
    view: &ScrollChevronView,
    settings: &ScrollChevronSettings,
    kind: &ScrollChevronRenderKind,
) {
    let id: WidgetId = id.into();
    let state = ctx.registry.get_or_insert_with(id.clone(), ScrollChevronState::default);
    register_input_coordinator_scroll_chevron(&mut ctx.input, id, rect, layer, state);
    draw_scroll_chevron(render, rect, widget_state, view, settings, kind);
}

/// Level 3 — register a scroll chevron via `LayoutManager`, forwarding to L2.
pub fn register_layout_manager_scroll_chevron<P: DockPanel>(
    layout: &mut LayoutManager<P>,
    render: &mut dyn RenderContext,
    parent: LayoutNodeId,
    id: impl Into<WidgetId>,
    rect: Rect,
    widget_state: WidgetState,
    view: &ScrollChevronView,
    settings: &ScrollChevronSettings,
    kind: &ScrollChevronRenderKind,
) {
    let id: WidgetId = id.into();
    let layer = layout.compute_layer_for(parent);
    layout.tree_mut().add_widget(parent, WidgetNode { id: id.clone(), kind: WidgetKind::ScrollChevron, rect, sense: Sense::CLICK, label: None });
    register_context_manager_scroll_chevron(
        layout.ctx_mut(), render, id, rect, &layer, widget_state, view, settings, kind,
    );
}
