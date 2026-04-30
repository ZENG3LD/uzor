//! Tooltip input-coordinator registration helpers.

use crate::app_context::ContextManager;
use crate::docking::panels::DockPanel;
use crate::input::core::coordinator::{InputCoordinator, LayerId};
use crate::input::core::sense::Sense;
use crate::input::core::widget_kind::WidgetKind;
use crate::layout::{LayoutManager, LayoutNodeId, WidgetNode};
use crate::render::RenderContext;
use crate::types::{Rect, WidgetId};

use super::render::draw_tooltip;
use super::settings::TooltipSettings;
use super::state::TooltipState;
use super::types::TooltipConfig;

/// Register the tooltip overlay rect with the input coordinator.
///
/// Tooltips are atomic and use `Sense::HOVER` so the coordinator tracks
/// "pointer over tooltip" for fade-out logic. The tooltip itself never
/// fires click events.
///
/// Place on `LayerId::tooltip()` so it sits above all other layers.
pub fn register_tooltip(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::Tooltip, rect, Sense::HOVER, layer);
}

/// Level 1 — register a tooltip with an explicit `InputCoordinator`.
///
/// `state` holds hover-timing and fade-in progress; it is read/written by the
/// caller between frames. Registration only declares the hit zone.
pub fn register_input_coordinator_tooltip(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    _state: &mut TooltipState,
) {
    coord.register_atomic(id, WidgetKind::Tooltip, rect, Sense::HOVER, layer);
}

/// Level 2 — register a tooltip via `ContextManager`, pulling `TooltipState`
/// from the registry, and draw it using the provided render context.
///
/// `config` supplies text, anchor, and position. `alpha` is the current
/// opacity (0.0–1.0, managed by the caller based on hover timing).
/// `settings` supplies visual style.
pub fn register_context_manager_tooltip(
    ctx: &mut ContextManager,
    render: &mut dyn RenderContext,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    config: &TooltipConfig,
    alpha: f64,
    settings: &TooltipSettings,
) {
    let id: WidgetId = id.into();
    let state = ctx.registry.get_or_insert_with(id.clone(), TooltipState::default);
    register_input_coordinator_tooltip(&mut ctx.input, id, rect, layer, state);
    draw_tooltip(render, rect, config, alpha, settings);
}

/// Level 3 — register a tooltip via `LayoutManager`.
pub fn register_layout_manager_tooltip<P: DockPanel>(
    layout: &mut LayoutManager<P>,
    render: &mut dyn RenderContext,
    parent: LayoutNodeId,
    id: impl Into<WidgetId>,
    rect: Rect,
    config: &TooltipConfig,
    alpha: f64,
    settings: &TooltipSettings,
) {
    let id: WidgetId = id.into();
    let layer = layout.compute_layer_for(parent);
    layout.tree_mut().add_widget(parent, WidgetNode { id: id.clone(), kind: WidgetKind::Tooltip, rect, sense: Sense::HOVER });
    register_context_manager_tooltip(
        layout.ctx_mut(), render, id, rect, &layer, config, alpha, settings,
    );
}
