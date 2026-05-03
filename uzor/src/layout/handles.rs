//! Typed node handles returned by composite L3 registration functions.
//!
//! Two families of handles live here:
//!
//! 1. **Node handles** (`ModalNode`, `PopupNode`, …) — wrap a `LayoutNodeId` and
//!    represent a specific composite widget kind in the layout tree.  Callers pass
//!    these as the `parent` parameter when registering child widgets.
//!
//! 2. **State handles** (`ModalHandle`, `PopupHandle`, …) — wrap a `WidgetId` and
//!    act as *opaque keys* into the `LayoutManager` composite state maps.  L3 app
//!    code obtains them from `LayoutManager::add_modal` / `add_popup` / … and
//!    passes them back to `layout.modal(h)` / `layout.modal_mut(h)` / etc.
//!    The inner `WidgetId` is `pub(crate)` so app code outside this crate cannot
//!    forge a handle from a raw string.

use super::tree::LayoutNodeId;
use crate::types::WidgetId;

// ---------------------------------------------------------------------------
// Node handles (layout tree position)
// ---------------------------------------------------------------------------

macro_rules! node_handle {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name(pub LayoutNodeId);

        impl From<$name> for LayoutNodeId {
            fn from(h: $name) -> Self {
                h.0
            }
        }

        impl AsRef<LayoutNodeId> for $name {
            fn as_ref(&self) -> &LayoutNodeId {
                &self.0
            }
        }
    };
}

node_handle!(ModalNode,         "Typed handle for a modal composite node.");
node_handle!(PopupNode,         "Typed handle for a popup composite node.");
node_handle!(DropdownNode,      "Typed handle for a dropdown composite node.");
node_handle!(ContextMenuNode,   "Typed handle for a context menu composite node.");
node_handle!(SidebarNode,       "Typed handle for a sidebar composite node.");
node_handle!(PanelNode,         "Typed handle for a panel composite node.");
node_handle!(ToolbarNode,       "Typed handle for a toolbar composite node.");
node_handle!(ChromeNode,        "Typed handle for a chrome composite node.");
node_handle!(BlackboxPanelNode, "Typed handle for a blackbox panel composite node.");

// ---------------------------------------------------------------------------
// State handles (opaque keys into LayoutManager state maps)
// ---------------------------------------------------------------------------

macro_rules! state_handle {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name {
            pub(crate) id: WidgetId,
        }

        impl $name {
            /// Read-only access to the inner widget id as a string slice.
            ///
            /// Used by framework-level builders to derive overlay slot ids.
            /// External crates cannot construct a handle from a string —
            /// they obtain it from `LayoutManager::add_*`.
            pub fn id_str(&self) -> &str {
                self.id.as_str()
            }
        }
    };
}

state_handle!(ModalHandle,       "Opaque handle to a modal composite owned by LayoutManager.");
state_handle!(PopupHandle,       "Opaque handle to a popup composite owned by LayoutManager.");
state_handle!(DropdownHandle,    "Opaque handle to a dropdown composite owned by LayoutManager.");
state_handle!(ToolbarHandle,     "Opaque handle to a toolbar composite owned by LayoutManager.");
state_handle!(SidebarHandle,     "Opaque handle to a sidebar composite owned by LayoutManager.");
state_handle!(ContextMenuHandle, "Opaque handle to a context menu composite owned by LayoutManager.");

// ---------------------------------------------------------------------------
// OverlayHandle — typed dismiss variant used in ClickOutcome
// ---------------------------------------------------------------------------

/// Typed overlay handle carried in [`super::manager::ClickOutcome::DismissOverlay`].
///
/// App code matches on this enum to identify *which kind* of overlay was clicked
/// outside of, without parsing `overlay_id.0.as_str()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlayHandle {
    Modal(ModalHandle),
    Popup(PopupHandle),
    Dropdown(DropdownHandle),
    ContextMenu(ContextMenuHandle),
    /// Other overlay kinds (chrome, tooltip, …) that have no typed handle yet.
    Other {
        /// The raw overlay slot id (e.g. `"modal-overlay"`).
        overlay_id: WidgetId,
        kind: Option<super::types::OverlayKind>,
    },
}
