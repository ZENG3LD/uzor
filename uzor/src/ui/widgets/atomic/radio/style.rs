//! Radio geometry traits and presets.
//!
//! `RadioStyle` controls the canonical group layout.
//! `RadioPairStyle` controls the inline pair / dot variants.

/// Geometry parameters for the `Group` render kind (section 35).
pub trait RadioStyle {
    /// Height of each radio row. mlc: 52.0.
    fn row_height(&self) -> f64;
    /// Vertical gap between rows. mlc: 8.0.
    fn gap(&self) -> f64;
    /// Outer ring radius. mlc: 7.0.
    fn outer_radius(&self) -> f64;
    /// Inner dot radius (drawn when selected). mlc: 4.0.
    fn inner_radius(&self) -> f64;
    /// Corner radius of the hover-highlight rect behind each row. mlc: 6.0.
    fn row_corner_radius(&self) -> f64 { 6.0 }
    /// Stroke width for the outer ring. mlc: 1.5.
    fn ring_stroke_width(&self) -> f64 { 1.5 }
    /// Horizontal offset from `x` to the circle center.
    fn circle_offset_x(&self) -> f64 { self.outer_radius() + 4.0 }
    /// Vertical offset from row top to circle center. mlc: 14.0.
    fn circle_offset_y(&self) -> f64 { 14.0 }
    /// Horizontal offset from `x` to the label start.
    fn label_offset_x(&self) -> f64 { self.outer_radius() * 2.0 + 16.0 }
    /// Vertical offset from row top to label baseline. mlc: 4.0.
    fn label_offset_y(&self) -> f64 { 4.0 }
    /// Vertical offset from row top to description baseline. mlc: 24.0.
    fn desc_offset_y(&self) -> f64 { 24.0 }
    /// Label font size. mlc: 13.0.
    fn label_font_size(&self) -> f64 { 13.0 }
    /// Description font size. mlc: 11.0.
    fn desc_font_size(&self) -> f64 { 11.0 }
}

/// Default radio group style — values from `mlc/radio_group.rs`.
pub struct DefaultRadioStyle;

impl RadioStyle for DefaultRadioStyle {
    fn row_height(&self)   -> f64 { 52.0 }
    fn gap(&self)          -> f64 { 8.0 }
    fn outer_radius(&self) -> f64 { 7.0 }
    fn inner_radius(&self) -> f64 { 4.0 }
}

/// Geometry parameters for the `Pair` / `Dot` render kinds (sections 36-37).
pub trait RadioPairStyle {
    /// Circle radius for inline radio dots. mlc: 5.0.
    fn radio_radius(&self) -> f64;
    /// Row height for inline radio rows. mlc: 22.0.
    fn row_height(&self) -> f64;
    /// Stroke width for the outer ring. mlc: 1.5.
    fn ring_stroke_width(&self) -> f64 { 1.5 }
    /// Inner dot radius when selected (`radio_radius - 2.5`). mlc: 2.5.
    fn inner_dot_radius(&self) -> f64 { self.radio_radius() - 2.5 }
    /// Horizontal gap between the radio circle and its inline label. mlc: 6.0.
    fn label_gap(&self) -> f64 { 6.0 }
    /// Font size for the inline label. mlc: 13.0.
    fn label_font_size(&self) -> f64 { 13.0 }
}

/// Default radio pair style — values from `mlc/profile_manager.rs` and
/// `mlc/user_settings.rs`.
pub struct DefaultRadioPairStyle;

impl RadioPairStyle for DefaultRadioPairStyle {
    fn radio_radius(&self) -> f64 { 5.0 }
    fn row_height(&self)   -> f64 { 22.0 }
}
