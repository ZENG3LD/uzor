//! Button theme trait - Contract/Connector for button colors
//!
//! # Architecture Role
//!
//! **ButtonTheme is a CONTRACT/CONNECTOR trait** that connects:
//! - Factory rendering functions (`factory/render.rs`)
//! - System theme managers (e.g., `ToolbarTheme`, `AppTheme`, etc.)
//!
//! # How It Works
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │ 1. System Theme Manager (e.g., ToolbarTheme)           │
//! │    - Stores actual color values                         │
//! │    - Implements ButtonTheme trait (mapping)             │
//! └─────────────────────────────────────────────────────────┘
//!                           ↓
//! ┌─────────────────────────────────────────────────────────┐
//! │ 2. ButtonTheme trait (THIS MODULE)                      │
//! │    - Defines contract (which methods/colors needed)     │
//! │    - Acts as connector interface                        │
//! └─────────────────────────────────────────────────────────┘
//!                           ↓
//! ┌─────────────────────────────────────────────────────────┐
//! │ 3. Factory render functions (factory/render.rs)        │
//! │    - Accept &dyn ButtonTheme                            │
//! │    - Call trait methods to get colors                   │
//! └─────────────────────────────────────────────────────────┘
//! ```

/// Theme trait for button colors - Contract between system themes and factory rendering
pub trait ButtonTheme {
    // =========================================================================
    // Background colors
    // =========================================================================

    /// Normal state background color
    /// Typical: "transparent" or "#1e222d"
    fn button_bg_normal(&self) -> &str;

    /// Hover state background color
    /// Typical: "#2a2a2a"
    fn button_bg_hover(&self) -> &str;

    /// Pressed state background color
    /// Typical: "#1e3a5f"
    fn button_bg_pressed(&self) -> &str;

    /// Active/toggled state background color
    /// Typical: "#1e3a5f"
    fn button_bg_active(&self) -> &str;

    /// Disabled state background color
    /// Typical: "#3a3a3a"
    fn button_bg_disabled(&self) -> &str;

    // =========================================================================
    // Text colors
    // =========================================================================

    /// Normal state text color
    /// Typical: "#787b86"
    fn button_text_normal(&self) -> &str;

    /// Hover state text color
    /// Typical: "#ffffff"
    fn button_text_hover(&self) -> &str;

    /// Active state text color
    /// Typical: "#ffffff"
    fn button_text_active(&self) -> &str;

    /// Disabled state text color
    /// Typical: "#4a4a4a"
    fn button_text_disabled(&self) -> &str;

    // =========================================================================
    // Icon colors
    // =========================================================================

    /// Normal state icon color
    /// Typical: "#787b86"
    fn button_icon_normal(&self) -> &str;

    /// Hover state icon color
    /// Typical: "#ffffff"
    fn button_icon_hover(&self) -> &str;

    /// Active state icon color
    /// Typical: "#ffffff"
    fn button_icon_active(&self) -> &str;

    /// Disabled state icon color
    /// Typical: "#4a4a4a"
    fn button_icon_disabled(&self) -> &str;

    // =========================================================================
    // Border colors
    // =========================================================================

    /// Normal state border color
    /// Typical: "#3a3a3a"
    fn button_border_normal(&self) -> &str;

    /// Hover state border color
    /// Typical: "#2a2a2a"
    fn button_border_hover(&self) -> &str;

    /// Focused state border color
    /// Typical: "#1e3a5f"
    fn button_border_focused(&self) -> &str;

    // =========================================================================
    // Semantic colors
    // =========================================================================

    /// Primary/accent color for primary actions and active state
    /// Typical: "#2962ff"
    fn button_accent(&self) -> &str;

    /// Danger color for delete/remove actions
    /// Typical: "#ef4444"
    fn button_danger(&self) -> &str;

    /// Success color for confirm/success actions
    /// Typical: "#10b981"
    fn button_success(&self) -> &str;

    /// Warning color for warning actions
    /// Typical: "#f59e0b"
    fn button_warning(&self) -> &str;
}

// =============================================================================
// Default Theme Implementation
// =============================================================================

/// Default button theme using prototype colors
pub struct DefaultButtonTheme;

impl DefaultButtonTheme {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DefaultButtonTheme {
    fn default() -> Self {
        Self::new()
    }
}

impl ButtonTheme for DefaultButtonTheme {
    // Background colors
    fn button_bg_normal(&self) -> &str { "transparent" }
    fn button_bg_hover(&self) -> &str { "#2a2a2a" }
    fn button_bg_pressed(&self) -> &str { "#1e3a5f" }
    fn button_bg_active(&self) -> &str { "#1e3a5f" }
    fn button_bg_disabled(&self) -> &str { "#2a2a2a" }

    // Text colors
    fn button_text_normal(&self) -> &str { "#d1d5db" }
    fn button_text_hover(&self) -> &str { "#ffffff" }
    fn button_text_active(&self) -> &str { "#ffffff" }
    fn button_text_disabled(&self) -> &str { "#4a4a4a" }

    // Icon colors
    fn button_icon_normal(&self) -> &str { "#787b86" }
    fn button_icon_hover(&self) -> &str { "#e5e7eb" }
    fn button_icon_active(&self) -> &str { "#ffffff" }
    fn button_icon_disabled(&self) -> &str { "#4a4a4a" }

    // Border colors
    fn button_border_normal(&self) -> &str { "#3a3a3a" }
    fn button_border_hover(&self) -> &str { "#e5e7eb" }
    fn button_border_focused(&self) -> &str { "#2962ff" }

    // Semantic colors
    fn button_accent(&self) -> &str { "#2962ff" }
    fn button_danger(&self) -> &str { "#ef5350" }
    fn button_success(&self) -> &str { "#10b981" }
    fn button_warning(&self) -> &str { "#f59e0b" }
}
