//! Per-frame composite & overlay-dismiss registries used by
//! [`super::LayoutManager::consume_event`].
//!
//! Both registries live on the per-window [`super::WindowBranch`] and are
//! cleared once per frame.  Composites push entries from inside their
//! `register_layout_manager_*` helpers.

use crate::core::types::Rect;
use crate::input::WidgetKind;
use crate::input::core::coordinator::LayerId;
use crate::types::WidgetId;

// ---------------------------------------------------------------------------
// Per-frame composite registry — used by consume_event
// ---------------------------------------------------------------------------

/// The kind of a registered composite, used for event routing in
/// [`super::LayoutManager::consume_event`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositeKind {
    Modal,
    Popup,
    Dropdown,
    Toolbar,
    Sidebar,
    ContextMenu,
    Chrome,
}

/// One entry in the per-frame composite registry.
///
/// Composites push one entry in their `register_layout_manager_*` helper.
/// [`super::LayoutManager::consume_event`] walks these in overlay-first
/// order (Modal → Popup → Dropdown → ContextMenu → Toolbar → Sidebar)
/// to route [`super::DispatchEvent`]s without the app spelling out the
/// chain manually.
#[derive(Debug, Clone)]
pub struct CompositeRegistration {
    pub kind:       CompositeKind,
    /// The stable overlay or edge slot id (e.g. `"modal-overlay"`,
    /// `"top-toolbar"`).
    pub slot_id:    String,
    /// The widget id used when registering the composite with the
    /// coordinator (e.g. `"modal-widget"`, `"top-toolbar-widget"`).
    pub widget_id:  WidgetId,
    /// Frame rect of the composite (overlay rect or edge slot rect).
    pub frame_rect: Rect,
}

// ---------------------------------------------------------------------------
// Overlay dismiss registry
// ---------------------------------------------------------------------------

/// A single entry in the per-frame overlay dismiss registry.
///
/// Composites push one entry when they call their
/// `register_layout_manager_*` helper.  The LayoutManager uses these
/// entries to implement [`super::LayoutManager::dismiss_topmost_at`].
#[derive(Clone)]
pub struct DismissFrame {
    /// Z-order priority.  Higher = on top.  The topmost open overlay is
    /// the one with the highest z value.  Ties are broken by insertion
    /// order: the most-recently-pushed entry wins.
    ///
    /// Recommended values:
    /// - 100 for context menu / popup
    /// - 50  for dropdown
    /// - 10  for modal
    pub z: u32,
    /// Screen-space rect of the overlay this frame.
    pub rect: Rect,
    /// Overlay slot id (e.g. `"modal-overlay"`, `"dd-file-overlay"`).
    pub overlay_id: WidgetId,
}

/// Map a composite [`WidgetKind`] to its coordinator [`LayerId`], or
/// `None` for atomics.
pub(crate) fn layer_for_widget_kind(kind: WidgetKind) -> Option<LayerId> {
    match kind {
        WidgetKind::Modal        => Some(LayerId::modal()),
        WidgetKind::ContextMenu  => Some(LayerId::new("context_menu")),
        WidgetKind::Popup        => Some(LayerId::popup()),
        WidgetKind::Dropdown     => Some(LayerId::new("dropdown")),
        WidgetKind::Tooltip      => Some(LayerId::tooltip()),
        WidgetKind::Chrome       => Some(LayerId::new("chrome")),
        _                        => None,
    }
}
