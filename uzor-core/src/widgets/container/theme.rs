//! Container theme trait - Contract/Connector for scrollbar colors and dimensions
//!
//! # Architecture Role
//!
//! **ContainerTheme is a CONTRACT/CONNECTOR trait** that connects:
//! - Factory rendering functions (`factory/render.rs`)
//! - System theme managers (e.g., `AppTheme`, etc.)
//!
//! # How It Works
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │ 1. System Theme Manager (e.g., AppTheme)                   │
//! │    - Stores actual color values                             │
//! │    - Implements ContainerTheme trait (mapping)              │
//! └─────────────────────────────────────────────────────────────┘
//!                           ↓
//! ┌─────────────────────────────────────────────────────────────┐
//! │ 2. ContainerTheme trait (THIS MODULE)                       │
//! │    - Defines contract (which methods/colors needed)         │
//! │    - Acts as connector interface                            │
//! └─────────────────────────────────────────────────────────────┘
//!                           ↓
//! ┌─────────────────────────────────────────────────────────────┐
//! │ 3. Factory render functions (factory/render.rs)            │
//! │    - Accept &dyn ContainerTheme                             │
//! │    - Call trait methods to get colors/dimensions            │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Implementation Example
//!
//! Each system theme manager implements ContainerTheme with proper color mapping:
//!
//! ```rust,ignore
//! // In ui/render/theme.rs (or your theme module)
//! impl ContainerTheme for AppTheme {
//!     fn scrollbar_width(&self) -> f64 {
//!         12.0
//!     }
//!
//!     fn scrollbar_track_color(&self) -> [u8; 4] {
//!         [30, 30, 30, 255]  // ← Map to your theme's scrollbar track color
//!     }
//!
//!     fn scrollbar_thumb_color(&self) -> [u8; 4] {
//!         [80, 80, 80, 255]  // ← Map to your theme's scrollbar thumb color
//!     }
//!     // ... rest of methods
//! }
//! ```
//!
//! # Usage in Factory
//!
//! ```rust,ignore
//! use container::factory::render_default;
//!
//! let app_theme = AppTheme::default();
//!
//! // AppTheme automatically converts to &dyn ContainerTheme
//! render_default(ctx, &container, &app_theme, &state, &input_handler, rect);
//! //                               ↑ Any theme implementing ContainerTheme
//! ```
//!
//! # Notes
//!
//! - **Not used in production inline rendering** - Terminal uses direct color access
//! - **Used by factory for prototyping** - Enables factory to work with any theme
//! - **Each project implements its own mapping** - No hardcoded theme assumptions

/// Theme trait for container scrollbar colors and dimensions
///
/// This trait defines the color and dimension contract that any system theme
/// must implement to work with factory rendering functions.
///
/// # Responsibility
///
/// **System theme managers** (e.g., AppTheme) implement this trait by:
/// - Mapping their internal color fields to ContainerTheme methods
/// - Providing appropriate colors for scrollbar states
/// - Providing dimensions for scrollbar sizing
///
/// **Factory rendering** uses this trait by:
/// - Accepting `&dyn ContainerTheme` parameter
/// - Calling trait methods to get colors and dimensions
/// - Working with ANY theme that implements this contract
///
/// # Default Theme
///
/// If you don't have a system theme yet, use `DefaultContainerTheme`:
///
/// ```rust,ignore
/// use container::factory::render_default;
/// use container::theme::DefaultContainerTheme;
///
/// let theme = DefaultContainerTheme::new();
/// render_default(ctx, &container, &theme, &state, &input_handler, rect);
/// ```
///
/// # Implementation Location
///
/// Implement this trait in the **same module as your system theme**:
///
/// ```rust,ignore
/// // In ui/render/theme.rs (example for Terminal)
/// pub struct AppTheme {
///     pub scrollbar_track: [u8; 4],
///     pub scrollbar_thumb: [u8; 4],
///     pub scrollbar_hover: [u8; 4],
///     // ... other theme fields
/// }
///
/// impl ContainerTheme for AppTheme {
///     fn scrollbar_width(&self) -> f64 { 12.0 }
///     fn scrollbar_track_color(&self) -> [u8; 4] { self.scrollbar_track }
///     fn scrollbar_thumb_color(&self) -> [u8; 4] { self.scrollbar_thumb }
///     // ... map all methods to your theme fields
/// }
/// ```
pub trait ContainerTheme {
    // =========================================================================
    // Scrollbar Dimensions
    // =========================================================================

    /// Scrollbar width in pixels
    /// Typical: 12.0
    fn scrollbar_width(&self) -> f64;

    /// Minimum scrollbar thumb height in pixels
    /// Typical: 20.0
    fn min_thumb_height(&self) -> f64;

    /// Container padding in pixels
    /// Typical: 0.0
    fn container_padding(&self) -> f64;

    // =========================================================================
    // Scrollbar Colors (RGBA)
    // =========================================================================

    /// Scrollbar track background color
    /// Typical: rgba(30, 30, 30, 255) - dark grey
    fn scrollbar_track_color(&self) -> [u8; 4];

    /// Scrollbar thumb normal color
    /// Typical: rgba(80, 80, 80, 255) - medium grey
    fn scrollbar_thumb_color(&self) -> [u8; 4];

    /// Scrollbar thumb hover color
    /// Typical: rgba(100, 100, 100, 255) - lighter grey
    fn scrollbar_hover_color(&self) -> [u8; 4];
}

// =============================================================================
// Default Theme Implementation
// =============================================================================

/// Default container theme using inline spec colors
///
/// This theme provides the exact colors and dimensions from the inline specs
/// (layout/render_ui.rs:246) for quick prototyping.
///
/// # Colors
///
/// - Track: rgba(30, 30, 30, 255)
/// - Thumb: rgba(80, 80, 80, 255)
/// - Hover: rgba(100, 100, 100, 255)
///
/// # Dimensions
///
/// - Scrollbar width: 12.0px
/// - Min thumb height: 20.0px
/// - Container padding: 0.0px
///
/// # Usage
///
/// ```rust,ignore
/// use container::factory::render_default;
/// use container::theme::DefaultContainerTheme;
///
/// let theme = DefaultContainerTheme::new();
/// render_default(ctx, &container, &theme, &state, &input_handler, rect);
/// ```
///
/// For production, implement ContainerTheme for your system theme instead.
pub struct DefaultContainerTheme;

impl DefaultContainerTheme {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DefaultContainerTheme {
    fn default() -> Self {
        Self::new()
    }
}

impl ContainerTheme for DefaultContainerTheme {
    // Dimensions (from inline specs)
    fn scrollbar_width(&self) -> f64 {
        12.0
    }

    fn min_thumb_height(&self) -> f64 {
        20.0
    }

    fn container_padding(&self) -> f64 {
        0.0
    }

    // Colors (from inline specs: layout/render_ui.rs:246)
    fn scrollbar_track_color(&self) -> [u8; 4] {
        [30, 30, 30, 255] // rgba(30, 30, 30, 255)
    }

    fn scrollbar_thumb_color(&self) -> [u8; 4] {
        [80, 80, 80, 255] // rgba(80, 80, 80, 255)
    }

    fn scrollbar_hover_color(&self) -> [u8; 4] {
        [100, 100, 100, 255] // rgba(100, 100, 100, 255)
    }
}
