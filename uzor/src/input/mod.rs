//! Input coordinator — centralized input handling for uzor
//!
//! Consolidates all input routing: clicks, scrolls, keyboard, text editing,
//! drag, touch, tooltips, and widget focus management.

pub mod core;
pub mod text;
pub mod keyboard;
pub mod pointer;
pub mod handlers;

pub use self::core::{PlatformEvent, ImeEvent, SystemTheme};

pub use self::core::{InputCoordinator, LayerId, ScopedRegion};
pub use self::core::response::*;
pub use self::core::sense::*;
pub use self::core::widget_state::*;

// Re-export text
pub use text::{InputCapability, TextAction, TextFieldConfig, TextFieldState, TextFieldStore};

// Re-export keyboard
pub use keyboard::events::*;
pub use keyboard::KeyPress;
pub use keyboard::shortcuts::*;

// Re-export pointer
pub use self::core::EventProcessor;
pub use pointer::ScrollManager;
pub use pointer::{InputState, MouseButton, ModifierKeys, PointerState, PointerDragState};
pub use pointer::touch::*;

// Compat shim — AnimatedValue moved to ui/animation/
pub use crate::ui::animation::{AnimatedValue, EasingFn};

// Re-export handlers (moved from _ongoing)
pub use handlers::*;

// Compat shims — tooltip and cursor moved to ui/, expose via input:: paths
pub use crate::ui::tooltip;
pub use crate::ui::cursor;
pub use crate::ui::tooltip::*;
pub use crate::ui::cursor::*;

// Flat module aliases (used by core/platform and ui/widgets via crate::input::X)
pub use keyboard::events;
pub use pointer::state;
pub use keyboard::shortcuts;
pub use core::widget_state;
