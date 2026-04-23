//! Consolidated input handling module.
//!
//! Unifies widget input coordination, text field management, keyboard events,
//! and shortcut registry under one namespace.

pub mod coordinator;
pub mod sense;
pub mod response;
pub mod widget_state;
pub mod text_field;
pub mod keyboard;
pub mod shortcuts;

pub use coordinator::{InputCoordinator, LayerId, ScopedRegion};
pub use sense::Sense;
pub use response::WidgetResponse;
pub use widget_state::{WidgetInputState, FocusState, HoverState, DragState, WidgetInteraction};
pub use text_field::{TextFieldConfig, TextFieldState, TextAction, InputCapability};
pub use keyboard::KeyPress;
pub use shortcuts::{KeyboardShortcut, ShortcutRegistry};
