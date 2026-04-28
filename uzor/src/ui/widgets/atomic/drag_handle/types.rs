//! Drag handle type definitions.

use crate::render::RenderContext;
use crate::types::Rect;

use super::settings::DragHandleSettings;

/// Per-instance view data for `draw_drag_handle`.
///
/// No visual state beyond the rect — the drag handle is purely a hit zone.
#[derive(Debug, Default, Clone)]
pub struct DragHandleView {
    /// Bounding rect of the drag handle (same rect passed to `draw_drag_handle`).
    pub rect: Rect,
}

/// Selects the visual rendering variant for the drag handle.
pub enum DragHandleRenderKind {
    /// No visual drawn — the drag handle is an invisible hit zone only.
    ///
    /// Use this when the composite already draws its own header background
    /// and the drag handle just provides the interaction region.
    Invisible,

    /// 6-dot 2×3 grip indicator centered in the rect.
    ///
    /// Typical usage: panel headers where a grip hint is desired.
    GripDots,

    /// Caller-supplied renderer — bypasses all built-in draw logic.
    Custom(
        Box<
            dyn Fn(
                &mut dyn RenderContext,
                Rect,
                &DragHandleView,
                &DragHandleSettings,
            ),
        >,
    ),
}
