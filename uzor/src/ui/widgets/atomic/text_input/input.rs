//! InputCoordinator registration helper for text input widgets.
//!
//! `register` is the one-call entry point a widget owner uses every frame
//! to:
//! - declare the atomic widget (`WidgetKind::Custom` for now — `TextInput`
//!   should be added to `WidgetKind` in a follow-up pass),
//! - tell the coordinator about the field's `TextFieldConfig` so char/key
//!   events get routed correctly while the field is focused.

use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense};
use crate::types::{Rect, WidgetId};

use super::settings::TextInputSettings;

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
