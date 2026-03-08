//! Core types for panel API

use serde::{Serialize, Deserialize};

/// Rectangle for panel layout (same layout as uzor-core Rect, but self-contained)
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PanelRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl PanelRect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self { x, y, width, height }
    }

    pub fn right(&self) -> f64 { self.x + self.width }
    pub fn bottom(&self) -> f64 { self.y + self.height }
    pub fn center_x(&self) -> f64 { self.x + self.width / 2.0 }
    pub fn center_y(&self) -> f64 { self.y + self.height / 2.0 }

    pub fn contains(&self, px: f64, py: f64) -> bool {
        px >= self.x && px <= self.right() && py >= self.y && py <= self.bottom()
    }

    /// Shrink rect by padding on all sides
    pub fn inset(&self, padding: f64) -> Self {
        Self {
            x: self.x + padding,
            y: self.y + padding,
            width: (self.width - padding * 2.0).max(0.0),
            height: (self.height - padding * 2.0).max(0.0),
        }
    }
}

/// Hit zone returned by panel toolbar rendering
#[derive(Clone, Debug)]
pub struct HitZone {
    pub id: String,
    pub rect: PanelRect,
}

/// Mouse button state
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MouseButton {
    #[default]
    Left,
    Right,
    Middle,
}

/// Input state passed to panel each frame
#[derive(Clone, Debug, Default)]
pub struct PanelInput {
    /// Mouse position relative to panel rect (0,0 = top-left of panel)
    pub mouse_x: f64,
    pub mouse_y: f64,
    /// Mouse position in screen coordinates
    pub mouse_screen_x: f64,
    pub mouse_screen_y: f64,
    /// Whether mouse is inside this panel's rect
    pub mouse_in_panel: bool,
    /// Whether this panel currently has focus
    pub focused: bool,
    /// Whether the terminal wants this panel to show its toolbar
    pub toolbar_visible: bool,
    /// Current time in milliseconds (for animations)
    pub time_ms: u64,
    /// Device pixel ratio
    pub dpr: f64,
    /// Left mouse button clicked this frame (screen coords where click happened)
    pub clicked: Option<(f64, f64)>,
    /// Mouse button currently held down
    pub button_down: bool,
    /// Mouse button released this frame
    pub button_released: bool,
    /// Scroll delta (horizontal, vertical)
    pub scroll_delta: (f64, f64),
    /// Whether input was consumed by a higher-priority layer
    pub input_consumed: bool,
}

/// Output returned by panel after rendering
#[derive(Clone, Debug, Default)]
pub struct PanelOutput {
    /// Hit zones for toolbar items (terminal uses these for click dispatch)
    pub toolbar_hit_zones: Vec<HitZone>,
    /// Hit zones for panel content (optional, for context menus etc.)
    pub content_hit_zones: Vec<HitZone>,
    /// Requested cursor style (e.g., "pointer", "crosshair", "default")
    pub cursor: Option<String>,
    /// Height consumed by the panel's local toolbar (0 if no toolbar)
    pub toolbar_height: f64,
}

/// Theme colors passed from terminal to panel for consistent styling
#[derive(Clone, Debug)]
pub struct PanelTheme {
    pub toolbar_bg: String,
    pub toolbar_separator: String,
    pub item_bg_hover: String,
    pub item_bg_active: String,
    pub item_text: String,
    pub item_text_muted: String,
    pub item_text_hover: String,
    pub item_text_active: String,
    pub accent: String,
    /// Whether to use sidebar-style rendering (vertical accent bars)
    pub sidebar_style: bool,
}

impl Default for PanelTheme {
    fn default() -> Self {
        Self {
            toolbar_bg: "#1e1e2e".to_string(),
            toolbar_separator: "#333346".to_string(),
            item_bg_hover: "#2a2a3e".to_string(),
            item_bg_active: "#3b82f6".to_string(),
            item_text: "#cdd6f4".to_string(),
            item_text_muted: "#6c7086".to_string(),
            item_text_hover: "#ffffff".to_string(),
            item_text_active: "#ffffff".to_string(),
            accent: "#3b82f6".to_string(),
            sidebar_style: false,
        }
    }
}
