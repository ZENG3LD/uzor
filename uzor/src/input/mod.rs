//! Input handling for uzor
//!
//! This module provides platform-agnostic input state types that capture
//! user interactions (mouse, keyboard, touch) and can be passed to widgets
//! and rendering code for interaction detection.

pub mod animation;
pub mod coordinator;
pub mod cursor;
pub mod event_processor;
pub mod events;
pub mod handlers;
pub mod response;
pub mod sense;
pub mod shortcuts;
pub mod state;
pub mod tooltip;
pub mod touch;
pub mod scroll_manager;
pub mod widget_state;

// Re-export all input types at the module level
pub use animation::*;
pub use coordinator::{InputCoordinator, LayerId, ScopedRegion};
pub use cursor::*;
pub use event_processor::EventProcessor;
pub use crate::platform::PlatformEvent;
pub use events::*;
pub use handlers::*;
pub use response::*;
pub use sense::*;
pub use shortcuts::*;
// Explicitly re-export to avoid conflict with widget_state
pub use state::{InputState, MouseButton, ModifierKeys, PointerState};
pub use state::DragState as PointerDragState;
pub use tooltip::*;
pub use touch::*;
pub use scroll_manager::ScrollManager;
pub use widget_state::*;
