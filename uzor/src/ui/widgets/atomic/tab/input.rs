//! Tab input-coordinator registration helpers.

use crate::input::core::coordinator::{InputCoordinator, LayerId};
use crate::input::core::sense::Sense;
use crate::input::core::widget_kind::WidgetKind;
use crate::types::{Rect, WidgetId};

// ---------------------------------------------------------------------------
// Core registration helpers (existing — kept)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Convenience wrappers — make call-sites self-documenting.
// All three wrappers use the same composite registration; the names clarify
// which visual variant the tab belongs to.
// ---------------------------------------------------------------------------

/// Register a Chrome (browser-style) tab on the main layer.
///
/// Pass `close_btn_rect = Some(...)` for closable tabs;
/// the close button is registered as a `Button` child with id `"{tab_id}:close"`.
pub fn register_chrome_tab(
    coord: &mut InputCoordinator,
    tab_id: impl Into<WidgetId>,
    rect: Rect,
    close_btn_rect: Option<Rect>,
) -> WidgetId {
    register_tab(coord, tab_id, rect, Sense::CLICK | Sense::HOVER, close_btn_rect)
}

/// Register a ModalSidebar (icon-only vertical) tab on the given layer.
///
/// Modal tabs always live on a layer (the modal's own layer); pass the layer
/// id created for that modal.
pub fn register_sidebar_tab(
    coord: &mut InputCoordinator,
    tab_id: impl Into<WidgetId>,
    rect: Rect,
    layer: &LayerId,
) -> WidgetId {
    // Sidebar tabs are never closable — no close child needed.
    register_tab_on_layer(coord, tab_id, rect, Sense::CLICK | Sense::HOVER, None, layer)
}

/// Register a ModalHorizontal (text + filled-bg active) tab on the given layer.
///
/// Pass `close_btn_rect = Some(...)` if the variant needs a close button
/// (not present in mlc horizontal tabs, but accepted for future use).
pub fn register_horizontal_tab(
    coord: &mut InputCoordinator,
    tab_id: impl Into<WidgetId>,
    rect: Rect,
    close_btn_rect: Option<Rect>,
    layer: &LayerId,
) -> WidgetId {
    register_tab_on_layer(coord, tab_id, rect, Sense::CLICK | Sense::HOVER, close_btn_rect, layer)
}
