//! InputCoordinator registration helpers for the Text widget.
//!
//! Text is read-only. It registers with `Sense::HOVER` so callers can detect
//! cursor-over for tooltip or color-change purposes.

use crate::app_context::ContextManager;
use crate::layout::docking::DockPanel;
use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::layout::{LayoutManager, LayoutNodeId, WidgetNode};
use crate::render::RenderContext;
use crate::types::{Rect, WidgetId, WidgetState};

use super::render::draw_text;
use super::settings::TextSettings;
use super::types::TextView;

/// Level 1 — register a Text widget hit zone with an explicit `InputCoordinator`.
pub fn register_input_coordinator_text(
    coord: &mut InputCoordinator,
    id:    impl Into<WidgetId>,
    rect:  Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::Custom, rect, Sense::HOVER, layer);
}

/// Level 2 — register + draw via `ContextManager`.
///
/// `_state` is accepted for API uniformity with other atomic widgets but is
/// unused — Text has no persistent state beyond caller-supplied hover.
pub fn register_context_manager_text(
    ctx:      &mut ContextManager,
    render:   &mut dyn RenderContext,
    id:       impl Into<WidgetId>,
    rect:     Rect,
    layer:    &LayerId,
    _state:   WidgetState,
    view:     &TextView<'_>,
    settings: &TextSettings,
) {
    let id: WidgetId = id.into();
    register_input_coordinator_text(&mut ctx.input, id, rect, layer);
    draw_text(render, rect, view, settings);
}

/// Level 3 — register + draw via `LayoutManager`.
///
/// Inserts a `WidgetNode` into the `LayoutTree` under `parent`, then forwards
/// to L2. The coordinator layer is derived from the parent chain.
pub fn register_layout_manager_text<P: DockPanel>(
    layout:   &mut LayoutManager<P>,
    render:   &mut dyn RenderContext,
    parent:   LayoutNodeId,
    id:       impl Into<WidgetId>,
    rect:     Rect,
    state:    WidgetState,
    view:     &TextView<'_>,
    settings: &TextSettings,
) {
    let id: WidgetId = id.into();
    let layer = layout.compute_layer_for(parent);
    layout.tree_mut().add_widget(parent, WidgetNode {
        id: id.clone(),
        kind: WidgetKind::Custom,
        rect,
        sense: Sense::HOVER,
    });
    register_context_manager_text(layout.ctx_mut(), render, id, rect, &layer, state, view, settings);
}
