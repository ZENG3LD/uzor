//! Platform abstraction layer (compat shim)

pub mod types;
pub mod backends;

pub use types::*;
pub use backends::PlatformBackend;

// Re-export event types from their canonical location in input/
pub use crate::input::core::{PlatformEvent, ImeEvent, SystemTheme};

#[derive(Clone, Debug)]
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
    pub decorations: bool,
    pub transparent: bool,
    pub visible: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "uzor".to_string(),
            width: 800,
            height: 600,
            resizable: true,
            decorations: true,
            transparent: false,
            visible: true,
        }
    }
}

impl WindowConfig {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            ..Default::default()
        }
    }
}
