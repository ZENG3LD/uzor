//! Geometry parameters for text input rendering.
//!
//! Numbers come from mlc `draw_input` defaults (`InputConfig`) plus the
//! `TextInputTheme` legacy slots that were really geometry not colour.

/// Geometry trait — overridable by callers via custom `impl`.
pub trait TextInputStyle {
    /// Total input height in logical pixels (`mlc` default 30.0).
    fn height(&self) -> f64;
    /// Inner padding from the rect edges before the text starts (mlc 8.0).
    fn padding(&self) -> f64;
    /// Corner radius for both bg and border (mlc 4.0).
    fn radius(&self) -> f64;
    /// Border thickness when not focused (mlc 1.0).
    fn border_width_normal(&self) -> f64;
    /// Border thickness when focused (mlc 2.0).
    fn border_width_focused(&self) -> f64;
    /// Font size in pixels (mlc 13.0).
    fn font_size(&self) -> f64;
    /// Cursor stroke width (mlc 1.5).
    fn cursor_width(&self) -> f64;
    /// Right-side margin kept around the cursor when text is scrolled (mlc 4.0).
    fn cursor_margin(&self) -> f64;
    /// Cursor blink half-period in milliseconds (mlc 500 → 1000ms full cycle).
    fn cursor_blink_half_period_ms(&self) -> u64;
}

/// Default style — values copied from mlc `chart/src/ui/widgets/input.rs`.
pub struct DefaultTextInputStyle;

impl Default for DefaultTextInputStyle {
    fn default() -> Self {
        Self
    }
}

impl TextInputStyle for DefaultTextInputStyle {
    fn height(&self)                     -> f64 { 30.0 }
    fn padding(&self)                    -> f64 { 8.0 }
    fn radius(&self)                     -> f64 { 4.0 }
    fn border_width_normal(&self)        -> f64 { 1.0 }
    fn border_width_focused(&self)       -> f64 { 2.0 }
    fn font_size(&self)                  -> f64 { 13.0 }
    fn cursor_width(&self)               -> f64 { 1.5 }
    fn cursor_margin(&self)              -> f64 { 4.0 }
    fn cursor_blink_half_period_ms(&self) -> u64 { 500 }
}
