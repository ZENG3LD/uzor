//! InputCoordinator registration helpers for item widgets.
//!
//! Item uses Sense::NONE — it is non-interactive. Registration still occurs
//! so hit-rect queries work for layout/tooltip purposes.

use crate::app_context::ContextManager;
use crate::docking::panels::DockPanel;
use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::layout::LayoutManager;
use crate::render::RenderContext;
use crate::types::{IconId, Rect, WidgetId, WidgetState};

use super::render::{draw_item, ItemView};
use super::settings::ItemSettings;
use super::state::ItemState;
use super::types::ItemRenderKind;

/// Register an item widget with the coordinator for this frame.
/// Uses `Sense::NONE` — items are non-interactive.
pub fn register(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::Item, rect, Sense::NONE, layer);
}

/// Level 1 — register an item with an explicit `InputCoordinator`.
pub fn register_input_coordinator_item(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    _state: &mut ItemState,
) {
    coord.register_atomic(id, WidgetKind::Item, rect, Sense::NONE, layer);
}

/// Level 2 — register an item via `ContextManager`, pulling state from the registry,
/// and draw it using the provided render context.
///
/// `widget_state` is supplied by the caller — the app owns the state machine.
/// `view` supplies per-frame label/icon/svg data. `settings` supplies visual style.
/// `kind` selects the render variant. Icon and SVG rendering use no-op closures by default.
pub fn register_context_manager_item<'a>(
    ctx: &mut ContextManager,
    render: &mut dyn RenderContext,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    widget_state: WidgetState,
    view: &ItemView<'a>,
    settings: &ItemSettings,
    kind: &ItemRenderKind<'a>,
) {
    let id: WidgetId = id.into();
    let state = ctx.registry.get_or_insert_with(id.clone(), ItemState::default);
    register_input_coordinator_item(&mut ctx.input, id, rect, layer, state);
    draw_item(
        render,
        rect,
        widget_state,
        view,
        settings,
        kind,
        |_, _: &IconId, _, _| {},
        |_, _, _, _| {},
    );
}

/// Level 3 — register an item via `LayoutManager`, forwarding to L2.
pub fn register_layout_manager_item<'a, P: DockPanel>(
    layout: &mut LayoutManager<P>,
    render: &mut dyn RenderContext,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    widget_state: WidgetState,
    view: &ItemView<'a>,
    settings: &ItemSettings,
    kind: &ItemRenderKind<'a>,
) {
    register_context_manager_item(
        layout.ctx_mut(), render, id, rect, layer, widget_state, view, settings, kind,
    );
}
