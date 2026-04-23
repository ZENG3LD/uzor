//! Window management types

/// Window position in screen coordinates
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WindowPosition {
    pub x: i32,
    pub y: i32,
}

/// Window size in logical pixels
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
}

/// Resize direction for drag-resize operations
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResizeDirection {
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

/// Window chrome configuration (custom title bar)
#[derive(Clone, Debug)]
pub struct ChromeConfig {
    /// Whether to use custom chrome (no OS decorations)
    pub custom: bool,
    /// Title bar height in logical pixels
    pub title_bar_height: f64,
    /// Whether window has shadow (even without decorations)
    pub shadow: bool,
}

impl Default for ChromeConfig {
    fn default() -> Self {
        Self {
            custom: false,
            title_bar_height: 28.0,
            shadow: true,
        }
    }
}

/// Window border configuration
#[derive(Clone, Debug)]
pub struct BorderConfig {
    /// Border color as hex string (e.g. "#FF0000")
    pub color: Option<String>,
    /// Corner radius (OS-level, not CSS)
    pub corner_radius: Option<f64>,
}

impl Default for BorderConfig {
    fn default() -> Self {
        Self {
            color: None,
            corner_radius: None,
        }
    }
}

/// Current window state
#[derive(Clone, Debug, Default)]
pub struct WindowState {
    pub position: WindowPosition,
    pub size: WindowSize,
    pub scale_factor: f64,
    pub maximized: bool,
    pub minimized: bool,
    pub fullscreen: bool,
    pub focused: bool,
    pub visible: bool,
}
