//! Chrome input-coordinator registration helpers.

use crate::input::core::coordinator::{InputCoordinator, LayerId};
use crate::input::core::sense::Sense;
use crate::input::core::widget_kind::WidgetKind;
use crate::types::{Rect, WidgetId};

/// Register the Chrome composite widget and return its `WidgetId`.
///
/// Chrome uses `Sense::NONE` at the composite level; interaction happens
/// through the child buttons and tabs registered separately.
pub fn register_chrome(
    coord: &mut InputCoordinator,
    id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) -> WidgetId {
    coord.register_composite(id, WidgetKind::Chrome, rect, Sense::NONE, layer)
}

/// Register an atomic titlebar button as a child of a Chrome.
///
/// `btn_id` should be unique per button, e.g. `"{chrome_id}:close"`.
/// Uses `Sense::CLICK | Sense::HOVER` so both click and hover events flow
/// through the coordinator.
pub fn register_chrome_button(
    coord: &mut InputCoordinator,
    chrome_id: &WidgetId,
    btn_id: impl Into<WidgetId>,
    rect: Rect,
) {
    coord.register_child(
        chrome_id,
        btn_id,
        WidgetKind::Button,
        rect,
        Sense::CLICK | Sense::HOVER,
    );
}

/// Register the drag region as a child of a Chrome.
///
/// The drag region uses `Sense::DRAG` so the coordinator delivers drag-start
/// and drag-delta events for window repositioning.
pub fn register_chrome_drag_region(
    coord: &mut InputCoordinator,
    chrome_id: &WidgetId,
    drag_id: impl Into<WidgetId>,
    rect: Rect,
) {
    coord.register_child(
        chrome_id,
        drag_id,
        WidgetKind::Custom,
        rect,
        Sense::DRAG,
    );
}
