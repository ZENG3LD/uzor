//! Tab input-coordinator registration helpers.

use crate::input::core::coordinator::{InputCoordinator, LayerId};
use crate::input::core::sense::Sense;
use crate::input::core::widget_kind::WidgetKind;
use crate::types::{Rect, WidgetId};

/// Register a tab as a top-level composite widget.
///
/// If `close_btn_rect` is `Some`, an atomic `Button` child is registered for
/// the close button. The close button id is `"{tab_id}:close"`.
///
/// Returns the `WidgetId` of the tab composite.
pub fn register_tab(
    coord: &mut InputCoordinator,
    tab_id: impl Into<WidgetId>,
    rect: Rect,
    sense: Sense,
    close_btn_rect: Option<Rect>,
) -> WidgetId {
    let id = coord.register_composite(tab_id, WidgetKind::Tab, rect, sense, &LayerId::main());
    if let Some(close_rect) = close_btn_rect {
        let close_id = format!("{}:close", id.0);
        coord.register_child(&id, close_id, WidgetKind::Button, close_rect, Sense::CLICK);
    }
    id
}

/// Register a tab composite on an explicit layer.
///
/// Use this variant when the tab strip lives on a non-main layer (e.g.
/// inside a Chrome that is on a popup layer).
pub fn register_tab_on_layer(
    coord: &mut InputCoordinator,
    tab_id: impl Into<WidgetId>,
    rect: Rect,
    sense: Sense,
    close_btn_rect: Option<Rect>,
    layer: &LayerId,
) -> WidgetId {
    let id = coord.register_composite(tab_id, WidgetKind::Tab, rect, sense, layer);
    if let Some(close_rect) = close_btn_rect {
        let close_id = format!("{}:close", id.0);
        coord.register_child(&id, close_id, WidgetKind::Button, close_rect, Sense::CLICK);
    }
    id
}
