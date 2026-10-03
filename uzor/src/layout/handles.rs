//! Typed state handles for composite overlays.
//!
//! Each handle wraps a [`WidgetId`] and acts as an opaque key for the
//! owner of that composite's state (today: `uzor-framework`'s overlay
//! engine). App code receives handles from that owner and passes them
//! back into [`ClickDispatcher`](super::ClickDispatcher) patterns.

use crate::types::WidgetId;

macro_rules! state_handle {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name {
            pub(crate) id: WidgetId,
        }

        impl $name {
            /// A handle for the composite whose host widget id is `id`.
            pub fn new(id: WidgetId) -> Self {
                Self { id }
            }

            /// The composite's host widget id.
            pub fn id(&self) -> &WidgetId {
                &self.id
            }

            /// Read-only access to the inner widget id as a string slice.
            pub fn id_str(&self) -> &str {
                self.id.as_str()
            }
        }
    };
}

state_handle!(ModalHandle, "Opaque handle to a modal composite.");
state_handle!(PopupHandle, "Opaque handle to a popup composite.");
state_handle!(DropdownHandle, "Opaque handle to a dropdown composite.");
state_handle!(ToolbarHandle, "Opaque handle to a toolbar composite.");
state_handle!(SidebarHandle, "Opaque handle to a sidebar composite.");
state_handle!(ContextMenuHandle, "Opaque handle to a context menu composite.");
