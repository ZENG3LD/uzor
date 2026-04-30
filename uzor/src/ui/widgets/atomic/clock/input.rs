//! InputCoordinator registration helpers for clock widgets.
//!
//! Clock uses HOVER sense — mlc has hover-only behavior (no click action).

use crate::app_context::ContextManager;
use crate::docking::panels::DockPanel;
use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::layout::LayoutManager;
use crate::render::RenderContext;
use crate::types::{Rect, WidgetId, WidgetState};

use super::render::{draw_clock, ClockView};
use super::settings::ClockSettings;
use super::state::ClockState;
use super::types::ClockRenderKind;

/// Register a clock widget with the coordinator for this frame.
/// Uses `Sense::HOVER` — the clock has hover-only behavior.
pub fn register(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::Clock, rect, Sense::HOVER, layer);
}

/// Level 1 — register a clock with an explicit `InputCoordinator`.
pub fn register_input_coordinator_clock(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    _state: &mut ClockState,
) {
    coord.register_atomic(id, WidgetKind::Clock, rect, Sense::HOVER, layer);
}

/// Level 2 — register a clock via `ContextManager`, pulling state from the registry,
/// and draw it using the provided render context.
///
/// `view` supplies the pre-formatted time string. `settings` supplies visual style.
/// `kind` selects the render variant.
pub fn register_context_manager_clock(
    ctx: &mut ContextManager,
    render: &mut dyn RenderContext,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    view: &ClockView<'_>,
    settings: &ClockSettings,
    kind: &ClockRenderKind,
) {
    let id: WidgetId = id.into();
    let state = ctx.registry.get_or_insert_with(id.clone(), ClockState::default);
    register_input_coordinator_clock(&mut ctx.input, id, rect, layer, state);
    draw_clock(render, rect, WidgetState::Normal, view, settings, kind);
}

/// Level 3 — register a clock via `LayoutManager`, forwarding to L2.
pub fn register_layout_manager_clock<P: DockPanel>(
    layout: &mut LayoutManager<P>,
    render: &mut dyn RenderContext,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    view: &ClockView<'_>,
    settings: &ClockSettings,
    kind: &ClockRenderKind,
) {
    register_context_manager_clock(
        layout.ctx_mut(), render, id, rect, layer, view, settings, kind,
    );
}
