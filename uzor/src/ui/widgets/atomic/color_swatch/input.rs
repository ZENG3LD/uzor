//! InputCoordinator registration helpers for color swatch widgets.

use crate::app_context::ContextManager;
use crate::docking::panels::DockPanel;
use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::layout::LayoutManager;
use crate::render::RenderContext;
use crate::types::{Rect, WidgetId, WidgetState};

use super::render::draw_color_swatch;
use super::settings::ColorSwatchSettings;
use super::state::ColorSwatchState;
use super::types::{ColorSwatchRenderKind, ColorSwatchView};

/// Register a color swatch widget with the coordinator for this frame.
pub fn register_color_swatch(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::ColorSwatch, rect, Sense::CLICK, layer);
}

/// Level 1 — register a color swatch with an explicit `InputCoordinator`.
pub fn register_input_coordinator_color_swatch(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    _state: &mut ColorSwatchState,
) {
    coord.register_atomic(id, WidgetKind::ColorSwatch, rect, Sense::CLICK, layer);
}

/// Level 2 — register a color swatch via `ContextManager`, pulling state from the registry,
/// and draw it using the provided render context.
///
/// `widget_state` is supplied by the caller — the app owns the hover/press state machine.
/// `view` supplies per-frame color data. `settings` supplies visual style.
/// `kind` selects the render variant.
pub fn register_context_manager_color_swatch(
    ctx: &mut ContextManager,
    render: &mut dyn RenderContext,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    widget_state: WidgetState,
    view: &ColorSwatchView<'_>,
    settings: &ColorSwatchSettings,
    kind: &ColorSwatchRenderKind<'_>,
) {
    let id: WidgetId = id.into();
    let state = ctx.registry.get_or_insert_with(id.clone(), ColorSwatchState::default);
    register_input_coordinator_color_swatch(&mut ctx.input, id, rect, layer, state);
    draw_color_swatch(render, rect, widget_state, view, settings, kind);
}

/// Level 3 — register a color swatch via `LayoutManager`, forwarding to L2.
pub fn register_layout_manager_color_swatch<P: DockPanel>(
    layout: &mut LayoutManager<P>,
    render: &mut dyn RenderContext,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    widget_state: WidgetState,
    view: &ColorSwatchView<'_>,
    settings: &ColorSwatchSettings,
    kind: &ColorSwatchRenderKind<'_>,
) {
    register_context_manager_color_swatch(
        layout.ctx_mut(), render, id, rect, layer, widget_state, view, settings, kind,
    );
}
