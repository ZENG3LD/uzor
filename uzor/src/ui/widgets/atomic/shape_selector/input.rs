//! InputCoordinator registration helpers for shape selector widgets.

use crate::app_context::ContextManager;
use crate::docking::panels::DockPanel;
use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::layout::LayoutManager;
use crate::render::RenderContext;
use crate::types::{Rect, WidgetId, WidgetState};

use super::render::draw_shape_selector;
use super::settings::ShapeSelectorSettings;
use super::state::ShapeSelectorState;
use super::types::ShapeSelectorRenderKind;

/// Register a shape selector widget with the coordinator for this frame.
pub fn register_shape_selector(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) {
    coord.register_atomic(id, WidgetKind::ShapeSelector, rect, Sense::CLICK, layer);
}

/// Level 1 — register a shape selector with an explicit `InputCoordinator`.
pub fn register_input_coordinator_shape_selector(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    _state: &mut ShapeSelectorState,
) {
    coord.register_atomic(id, WidgetKind::ShapeSelector, rect, Sense::CLICK, layer);
}

/// Level 2 — register a shape selector via `ContextManager`, pulling state from the registry,
/// and draw it using the provided render context.
///
/// `widget_state` is supplied by the caller — the app owns the hover/press state machine.
/// `settings` supplies visual style. `kind` selects the render variant.
/// For `Shape`, `ThemePreset`, and `UIStyle` kinds, use the dedicated
/// `draw_shape_selector_button`, `draw_theme_preset_button`, and `draw_ui_style_button`
/// functions directly for richer output.
pub fn register_context_manager_shape_selector(
    ctx: &mut ContextManager,
    render: &mut dyn RenderContext,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    widget_state: WidgetState,
    settings: &ShapeSelectorSettings,
    kind: &ShapeSelectorRenderKind<'_>,
) {
    let id: WidgetId = id.into();
    let state = ctx.registry.get_or_insert_with(id.clone(), ShapeSelectorState::default);
    register_input_coordinator_shape_selector(&mut ctx.input, id, rect, layer, state);
    draw_shape_selector(render, rect, widget_state, settings, kind);
}

/// Level 3 — register a shape selector via `LayoutManager`, forwarding to L2.
pub fn register_layout_manager_shape_selector<P: DockPanel>(
    layout: &mut LayoutManager<P>,
    render: &mut dyn RenderContext,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    widget_state: WidgetState,
    settings: &ShapeSelectorSettings,
    kind: &ShapeSelectorRenderKind<'_>,
) {
    register_context_manager_shape_selector(
        layout.ctx_mut(), render, id, rect, layer, widget_state, settings, kind,
    );
}
