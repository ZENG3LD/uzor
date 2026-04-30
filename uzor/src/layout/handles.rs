//! Typed node handles returned by composite L3 registration functions.
//!
//! Each handle wraps a `LayoutNodeId` and represents a specific composite
//! widget kind. Callers pass these as the `parent` parameter when registering
//! child widgets under a composite.

use super::tree::LayoutNodeId;

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

node_handle!(ModalNode,       "Typed handle for a modal composite node.");
node_handle!(PopupNode,       "Typed handle for a popup composite node.");
node_handle!(DropdownNode,    "Typed handle for a dropdown composite node.");
node_handle!(ContextMenuNode, "Typed handle for a context menu composite node.");
node_handle!(SidebarNode,     "Typed handle for a sidebar composite node.");
node_handle!(PanelNode,       "Typed handle for a panel composite node.");
node_handle!(ToolbarNode,     "Typed handle for a toolbar composite node.");
node_handle!(ChromeNode,      "Typed handle for a chrome composite node.");
node_handle!(BlackboxPanelNode, "Typed handle for a blackbox panel composite node.");
