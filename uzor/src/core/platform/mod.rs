//! Platform abstraction layer
//!
//! Defines traits that platform backends (desktop, web, mobile) must implement.

pub mod types;
pub mod backends;

use crate::input::events::KeyCode;
use crate::input::state::{ModifierKeys, MouseButton};

// Re-export common types
pub use types::*;
pub use backends::PlatformBackend;

// =============================================================================
// Window Configuration
// =============================================================================

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

// =============================================================================
// Platform Events
// =============================================================================

#[derive(Clone, Debug)]
pub enum PlatformEvent {
    WindowCreated,
    WindowResized { width: u32, height: u32 },
    WindowMoved { x: i32, y: i32 },
    WindowFocused(bool),
    WindowCloseRequested,
    WindowDestroyed,
    RedrawRequested,
    PointerEntered,
    PointerLeft,
    PointerMoved { x: f64, y: f64 },
    PointerDown { x: f64, y: f64, button: MouseButton },
    PointerUp { x: f64, y: f64, button: MouseButton },
    TouchStart { id: u64, x: f64, y: f64 },
    TouchMove { id: u64, x: f64, y: f64 },
    TouchEnd { id: u64, x: f64, y: f64 },
    TouchCancel { id: u64 },
    Scroll { dx: f64, dy: f64 },
    KeyDown { key: KeyCode, modifiers: ModifierKeys },
    KeyUp { key: KeyCode, modifiers: ModifierKeys },
    TextInput { text: String },
    ModifiersChanged { modifiers: ModifierKeys },
    ClipboardPaste { text: String },
    FileDropped { path: std::path::PathBuf },
    FileHovered { path: std::path::PathBuf },
    FileCancelled,
    Ime(ImeEvent),
    ThemeChanged { dark_mode: bool },
    ScaleFactorChanged { scale: f64 },
}

#[derive(Clone, Debug, PartialEq)]
pub enum ImeEvent {
    Enabled,
    Preedit(String, Option<(usize, usize)>),
    Commit(String),
    Disabled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemTheme {
    Light,
    Dark,
}
