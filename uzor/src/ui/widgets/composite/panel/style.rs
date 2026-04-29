//! Panel geometry parameters trait, presets, and `BackgroundFill`.
//!
//! Values ported from mlc panel audit (`panel-deep.md` §5).

// ---------------------------------------------------------------------------
// BackgroundFill
// ---------------------------------------------------------------------------

/// How the panel frame background is filled.
#[derive(Debug, Clone)]
pub enum BackgroundFill {
    /// Flat colour — uses `theme.bg()`.
    Solid,

    /// GPU blur of what is behind the panel, then `theme.bg()` at reduced alpha.
    ///
    /// Falls back to `Solid` on backends without blur support.
    Glass {
        /// Blur kernel radius in pixels.
        blur_radius: f64,
    },
}

// ---------------------------------------------------------------------------
// PanelStyle trait
// ---------------------------------------------------------------------------

/// Geometry parameters for the panel composite.
///
/// Implement this trait to customise sizes without touching colours.
pub trait PanelStyle {
    /// Header strip height in pixels.  Default: `22.0`.
    fn header_height(&self) -> f64;

    /// Column-header row height in pixels.  Default: `18.0`.
    fn column_header_height(&self) -> f64;

    /// Body row height hint (informational — used by callers, not the composite).
    /// Default: `20.0`.
    fn row_height(&self) -> f64;

    /// Footer strip height in pixels.  Default: `20.0`.
    fn footer_height(&self) -> f64;

    /// Inner horizontal padding in pixels.  Default: `6.0`.
    fn padding(&self) -> f64;

    /// Border line thickness in pixels.  Default: `1.0`.
    fn border_width(&self) -> f64;

    /// Scrollbar column width in pixels.  Default: `8.0`.
    fn scrollbar_width(&self) -> f64;

    /// Background fill strategy.  Default: `BackgroundFill::Solid`.
    fn background_fill(&self) -> BackgroundFill {
        BackgroundFill::Solid
    }
}

// ---------------------------------------------------------------------------
// Default style
// ---------------------------------------------------------------------------

/// Default style preset matching mlc panel geometry.
#[derive(Default)]
pub struct DefaultPanelStyle;

impl PanelStyle for DefaultPanelStyle {
    fn header_height(&self)        -> f64 { 22.0 }
    fn column_header_height(&self) -> f64 { 18.0 }
    fn row_height(&self)           -> f64 { 20.0 }
    fn footer_height(&self)        -> f64 { 20.0 }
    fn padding(&self)              -> f64 { 6.0  }
    fn border_width(&self)         -> f64 { 1.0  }
    fn scrollbar_width(&self)      -> f64 { 8.0  }
}
