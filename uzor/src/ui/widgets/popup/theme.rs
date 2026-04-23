//! Popup theme trait - Contract/Connector for popup colors
//!
//! # Architecture Role
//!
//! **PopupTheme is a CONTRACT/CONNECTOR trait** that connects:
//! - Factory rendering functions (`factory/mod.rs`)
//! - System theme managers (e.g., `AppTheme`, etc.)
//!
//! # How It Works
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │ 1. System Theme Manager (e.g., AppTheme)                │
//! │    - Stores actual color values                         │
//! │    - Implements PopupTheme trait (mapping)              │
//! └─────────────────────────────────────────────────────────┘
//!                           ↓
//! ┌─────────────────────────────────────────────────────────┐
//! │ 2. PopupTheme trait (THIS MODULE)                       │
//! │    - Defines contract (which methods/colors needed)     │
//! │    - Acts as connector interface                        │
//! └─────────────────────────────────────────────────────────┘
//!                           ↓
//! ┌─────────────────────────────────────────────────────────┐
//! │ 3. Factory render functions (factory/mod.rs)            │
//! │    - Accept &dyn PopupTheme                             │
//! │    - Call trait methods to get colors                   │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! # Notes
//!
//! - **Not used in production inline rendering** - Terminal uses direct color access
//! - **Used by factory for prototyping** - Enables factory to work with any theme
//! - **Each project implements its own mapping** - No hardcoded theme assumptions

/// Theme trait for popup colors - Contract between system themes and factory rendering
///
/// This trait defines the color contract that any system theme must implement
/// to work with factory rendering functions.
///
/// # Default Theme
///
/// If you don't have a system theme yet, use `DefaultPopupTheme`:
///
/// ```rust,ignore
/// use popup::factory::render_default;
/// use popup::theme::DefaultPopupTheme;
///
/// let theme = DefaultPopupTheme::new();
/// render_default(ctx, &popup, &theme, &state, &input, position);
/// ```
///
/// For production, implement PopupTheme for your system theme instead.
pub trait PopupTheme {
    // =========================================================================
    // Common popup properties
    // =========================================================================

    /// Padding inside the popup container
    /// Default: 8.0
    fn popup_padding(&self) -> f64;

    /// Background color of the popup
    /// Default: rgba(40, 40, 40, 255)
    fn background_color(&self) -> [u8; 4];

    /// Border color of the popup
    /// Default: rgba(80, 80, 80, 255)
    fn border_color(&self) -> [u8; 4];

    /// Shadow blur radius
    /// Default: 8.0
    fn shadow_blur(&self) -> f64;

    /// Shadow offset (x, y)
    /// Default: (0.0, 4.0)
    fn shadow_offset(&self) -> (f64, f64);

    /// Shadow color
    /// Default: rgba(0, 0, 0, 128)
    fn shadow_color(&self) -> [u8; 4];

    // =========================================================================
    // ContextMenu specific
    // =========================================================================

    /// Minimum width for context menu
    /// Default: 180.0
    fn menu_min_width(&self) -> f64;

    /// Height of each menu item
    /// Default: 32.0
    fn menu_item_height(&self) -> f64;

    /// Size of menu item icons
    /// Default: 16.0
    fn menu_icon_size(&self) -> f64;

    /// Height of separator line
    /// Default: 9.0
    fn menu_separator_height(&self) -> f64;

    /// Horizontal padding for menu items
    /// Default: 12.0
    fn menu_padding_horizontal(&self) -> f64;

    /// Vertical padding for menu container
    /// Default: 8.0
    fn menu_padding_vertical(&self) -> f64;

    /// Background color when hovering over menu item
    /// Default: rgba(60, 60, 60, 255)
    fn menu_hover_color(&self) -> [u8; 4];

    /// Text color for menu items
    /// Default: rgba(255, 255, 255, 255)
    fn menu_text_color(&self) -> [u8; 4];

    /// Icon color for menu items
    /// Default: rgba(180, 180, 180, 255)
    fn menu_icon_color(&self) -> [u8; 4];

    // =========================================================================
    // ColorPicker specific
    // =========================================================================

    /// Size of each color swatch
    /// Default: 24.0
    fn color_swatch_size(&self) -> f64;

    /// Spacing between color swatches in the grid
    /// Default: 4.0
    fn color_grid_spacing(&self) -> f64;

    /// Height of the hex input field
    /// Default: 30.0
    fn hex_input_height(&self) -> f64;

    /// Border color for selected color swatch
    /// Default: rgba(255, 255, 255, 255)
    fn color_selection_border(&self) -> [u8; 4];
}

// =============================================================================
// Default Theme Implementation
// =============================================================================

/// Default popup theme using prototype colors
///
/// This theme provides sensible default colors for quick prototyping
/// without requiring a system theme implementation.
///
/// Colors extracted from inline specs (layout/render_ui.rs, color_picker_popup.rs).
///
/// # Usage
///
/// ```rust,ignore
/// use popup::factory::render_default;
/// use popup::theme::DefaultPopupTheme;
///
/// let theme = DefaultPopupTheme::new();
/// render_default(ctx, &popup, &theme, &state, &input, position);
/// ```
///
/// For production, implement PopupTheme for your system theme instead.
pub struct DefaultPopupTheme;

impl DefaultPopupTheme {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DefaultPopupTheme {
    fn default() -> Self {
        Self::new()
    }
}

impl PopupTheme for DefaultPopupTheme {
    // Common popup properties
    fn popup_padding(&self) -> f64 { 8.0 }
    fn background_color(&self) -> [u8; 4] { [40, 40, 40, 255] }
    fn border_color(&self) -> [u8; 4] { [80, 80, 80, 255] }
    fn shadow_blur(&self) -> f64 { 8.0 }
    fn shadow_offset(&self) -> (f64, f64) { (0.0, 4.0) }
    fn shadow_color(&self) -> [u8; 4] { [0, 0, 0, 128] }

    // ContextMenu specific
    fn menu_min_width(&self) -> f64 { 180.0 }
    fn menu_item_height(&self) -> f64 { 32.0 }
    fn menu_icon_size(&self) -> f64 { 16.0 }
    fn menu_separator_height(&self) -> f64 { 9.0 }
    fn menu_padding_horizontal(&self) -> f64 { 12.0 }
    fn menu_padding_vertical(&self) -> f64 { 8.0 }
    fn menu_hover_color(&self) -> [u8; 4] { [60, 60, 60, 255] }
    fn menu_text_color(&self) -> [u8; 4] { [255, 255, 255, 255] }
    fn menu_icon_color(&self) -> [u8; 4] { [180, 180, 180, 255] }

    // ColorPicker specific
    fn color_swatch_size(&self) -> f64 { 24.0 }
    fn color_grid_spacing(&self) -> f64 { 4.0 }
    fn hex_input_height(&self) -> f64 { 30.0 }
    fn color_selection_border(&self) -> [u8; 4] { [255, 255, 255, 255] }
}
