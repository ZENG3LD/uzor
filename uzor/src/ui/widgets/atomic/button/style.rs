//! Geometry parameters for button rendering.
//!
//! Numbers ported from mlc `chart/src/ui/widgets/button.rs` `ButtonConfig`
//! defaults plus the recurring values from cluster-A research:
//! `radius=4.0` dominates, `radius=3.0` for compact, `radius=0.0` for flat
//! footer buttons (Primary/Cancel).

/// Geometry trait — overridable by callers via custom `impl`.
pub trait ButtonStyle {
    /// Corner radius (mlc default 4.0).
    fn radius(&self)         -> f64;
    /// Horizontal padding (mlc 8.0).
    fn padding_x(&self)      -> f64;
    /// Vertical padding (mlc 4.0).
    fn padding_y(&self)      -> f64;
    /// Icon side length (mlc 16.0 — universal standard).
    fn icon_size(&self)      -> f64;
    /// Font size (mlc 13.0).
    fn font_size(&self)      -> f64;
    /// Gap between icon and text (mlc 6.0).
    fn gap(&self)            -> f64;
    /// Border thickness when active (mlc 1.0).
    fn border_width(&self)   -> f64;
    /// Whether the active state additionally strokes a border (mlc false).
    fn show_active_border(&self) -> bool;
}

/// Default style — values copied from mlc `ButtonConfig::default`.
pub struct DefaultButtonStyle;

impl Default for DefaultButtonStyle {
    fn default() -> Self {
        Self
    }
}

impl ButtonStyle for DefaultButtonStyle {
    fn radius(&self)             -> f64  { 4.0 }
    fn padding_x(&self)          -> f64  { 8.0 }
    fn padding_y(&self)          -> f64  { 4.0 }
    fn icon_size(&self)          -> f64  { 16.0 }
    fn font_size(&self)          -> f64  { 13.0 }
    fn gap(&self)                -> f64  { 6.0 }
    fn border_width(&self)       -> f64  { 1.0 }
    fn show_active_border(&self) -> bool { false }
}

/// Compact style for small inline buttons (mini delete, stepper).
/// Cluster-A research: `radius=3.0`, smaller icon/font.
pub struct CompactButtonStyle;

impl ButtonStyle for CompactButtonStyle {
    fn radius(&self)             -> f64  { 3.0 }
    fn padding_x(&self)          -> f64  { 4.0 }
    fn padding_y(&self)          -> f64  { 2.0 }
    fn icon_size(&self)          -> f64  { 12.0 }
    fn font_size(&self)          -> f64  { 11.0 }
    fn gap(&self)                -> f64  { 4.0 }
    fn border_width(&self)       -> f64  { 1.0 }
    fn show_active_border(&self) -> bool { false }
}

/// Flat (no rounding) style for modal footer Primary / Cancel buttons.
/// Cluster-A research: `radius=0.0`.
pub struct FlatButtonStyle;

impl ButtonStyle for FlatButtonStyle {
    fn radius(&self)             -> f64  { 0.0 }
    fn padding_x(&self)          -> f64  { 12.0 }
    fn padding_y(&self)          -> f64  { 6.0 }
    fn icon_size(&self)          -> f64  { 16.0 }
    fn font_size(&self)          -> f64  { 13.0 }
    fn gap(&self)                -> f64  { 6.0 }
    fn border_width(&self)       -> f64  { 1.0 }
    fn show_active_border(&self) -> bool { false }
}
