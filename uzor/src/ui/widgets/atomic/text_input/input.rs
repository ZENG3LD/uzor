//! InputCoordinator registration helpers for text input widgets.
//!
//! `register` is the one-call entry point a widget owner uses every frame
//! to:
//! - declare the atomic widget (`WidgetKind::Custom` for now — `TextInput`
//!   should be added to `WidgetKind` in a follow-up pass),
//! - tell the coordinator about the field's `TextFieldConfig` so char/key
//!   events get routed correctly while the field is focused.

use crate::app_context::ContextManager;
use crate::docking::panels::DockPanel;
use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::layout::{LayoutManager, LayoutNodeId, WidgetNode};
use crate::render::RenderContext;
use crate::types::{Rect, WidgetId, WidgetState};

use super::render::{draw_input, draw_input_cursor, InputResult, InputView};
use super::settings::TextInputSettings;
use super::state::TextFieldStore;

/// Register a text input widget with the coordinator for this frame.
///
/// `id` is the widget's stable identifier (used both for hit-testing and
/// for keying its row in `TextFieldStore`).
pub fn register(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    _settings: &TextInputSettings,
) {
    let id = id.into();
    let sense = Sense::CLICK.with_focus().with_text();
    coord.register_atomic(id, crate::input::WidgetKind::Custom, rect, sense, layer);
    // Note: coord.register_text_field still expects the canonical
    // `input::text::store::TextFieldConfig`. Until that type is consolidated
    // with `state::TextFieldConfig` (which is a copy), the caller is expected
    // to invoke `register_text_field` directly with the canonical type.
}

/// Level 1 — register a text input with an explicit `InputCoordinator`.
///
/// `state` holds the per-field text/cursor/selection store. Text field state
/// is managed by the caller's `TextFieldStore`; this call registers the hit zone.
pub fn register_input_coordinator_text_input(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    settings: &TextInputSettings,
    _state: &mut TextFieldStore,
) {
    register(coord, id, rect, layer, settings);
}

/// Level 2 — register a text input via `ContextManager`, pulling `TextFieldStore`
/// from the registry, and draw it using the provided render context.
///
/// `widget_state` is supplied by the caller — the app owns the focus/hover state machine.
/// `view` supplies per-frame text, cursor, selection, and focus state.
/// `settings` supplies visual style and theme.
pub fn register_context_manager_text_input(
    ctx: &mut ContextManager,
    render: &mut dyn RenderContext,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
    widget_state: WidgetState,
    view: &InputView<'_>,
    settings: &TextInputSettings,
) -> InputResult {
    let id: WidgetId = id.into();
    let state = ctx.registry.get_or_insert_with(id.clone(), TextFieldStore::default);
    register_input_coordinator_text_input(&mut ctx.input, id.clone(), rect, layer, settings, state);
    let result = draw_input(render, rect, widget_state, view, settings);
    // Push the screen rect + char-x boundaries into the canonical
    // TextFieldStore so on_drag_start can hit-test and focus this field
    // when the user clicks inside it.
    ctx.input.text_fields_mut().update_field(
        &id,
        (rect.x, rect.y, rect.width, rect.height),
        result.char_x_positions.clone(),
    );
    // Mirror the text-field focus into the coordinator's widget-focus so
    // bridge-driven keyboard input reaches the right field.  Without this
    // step `coord.focused_widget()` stays `None` and KeyboardInput is
    // never routed to TextFieldStore::on_char.
    if ctx.input.text_fields().is_focused(&id)
        && ctx.input.focused_widget() != Some(&id)
    {
        ctx.input.set_focus(id.clone());
    }
    result
}

/// Level 3 — register a text input via `LayoutManager`, forwarding to L2.
pub fn register_layout_manager_text_input<P: DockPanel>(
    layout: &mut LayoutManager<P>,
    render: &mut dyn RenderContext,
    parent: LayoutNodeId,
    id: impl Into<WidgetId>,
    rect: Rect,
    widget_state: WidgetState,
    view: &InputView<'_>,
    settings: &TextInputSettings,
) -> InputResult {
    let id: WidgetId = id.into();
    let layer = layout.compute_layer_for(parent);
    let sense = Sense::CLICK.with_focus().with_text();
    layout.tree_mut().add_widget(parent, WidgetNode { id: id.clone(), kind: WidgetKind::Custom, rect, sense });
    let now_ms = layout.frame_time_ms() as u64;
    let result = register_context_manager_text_input(
        layout.ctx_mut(), render, id.clone(), rect, &layer, widget_state, view, settings,
    );
    // Draw blink caret automatically when the field is focused — caller no
    // longer needs to read `text_fields().cursor_visible()` and call
    // `draw_input_cursor` by hand.
    if view.focused
        && layout.ctx().input.text_fields().cursor_visible(now_ms)
    {
        draw_input_cursor(
            render,
            result.cursor_x,
            result.cursor_y,
            result.cursor_height,
            1.5,
            [220, 220, 220, 255],
        );
    }
    result
}
