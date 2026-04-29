//! BlackboxPanel geometry parameters trait and default implementation.
//!
//! `BackgroundFill` is re-used from the sibling `panel` composite.

pub use crate::ui::widgets::composite::panel::style::BackgroundFill;

// ---------------------------------------------------------------------------
// BlackboxStyle trait
// ---------------------------------------------------------------------------

/// Geometry parameters for the blackbox panel composite.
pub trait BlackboxStyle {
    /// Header strip height in pixels.
    ///
    /// Default: `24.0`.
    fn header_height(&self) -> f64;

    /// Border line width in pixels (`WithBorder` / `WithHeaderBorder` kinds).
    ///
    /// Default: `1.0`.
    fn border_width(&self) -> f64;

    /// Inner horizontal padding in pixels (header title left-offset).
    ///
    /// Default: `8.0`.
    fn padding(&self) -> f64;

    /// Background fill strategy.
    ///
    /// Default: `BackgroundFill::Solid`.
    fn background_fill(&self) -> BackgroundFill {
        BackgroundFill::Solid
    }
}

// ---------------------------------------------------------------------------
// Default style
// ---------------------------------------------------------------------------

/// Default geometry preset for blackbox panel.
#[derive(Default)]
pub struct DefaultBlackboxStyle;

impl BlackboxStyle for DefaultBlackboxStyle {
    fn header_height(&self) -> f64 { 24.0 }
    fn border_width(&self)  -> f64 { 1.0  }
    fn padding(&self)       -> f64 { 8.0  }
}
