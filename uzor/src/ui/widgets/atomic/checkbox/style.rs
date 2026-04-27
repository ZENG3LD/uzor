//! Checkbox geometry traits and presets.
//!
//! `CheckboxStyle` controls box size, border radius, stroke widths, and label gap.
//! Each concrete preset matches one mlc usage site.

/// Geometry parameters for checkbox rendering.
pub trait CheckboxStyle {
    /// Outer square side length (mlc: 16.0 for standard / notification).
    fn size(&self) -> f64;
    /// Outer corner radius (mlc: 3.0 standard, 2.0 level/notification).
    fn radius(&self) -> f64;
    /// Outer border stroke width (mlc: 1.0).
    fn border_width(&self) -> f64;
    /// Checkmark stroke width (mlc: 2.0).
    fn checkmark_width(&self) -> f64;
    /// Gap between checkbox box right edge and label text (mlc: 12.0 standard / 8.0 others).
    fn label_gap(&self) -> f64;
    /// Pixel inset for checkmark bottom/top anchor points.
    /// mlc draw_checkbox (section 21): 4.0
    /// mlc indicator/primitive (sections 22-23): 3.0
    fn checkmark_inset(&self) -> f64 {
        3.0
    }
}

/// Standard checkbox (section 21 — `chart_settings.rs`).
/// Size 16×16, radius 3.0, label gap 12.
pub struct StandardCheckboxStyle;

impl CheckboxStyle for StandardCheckboxStyle {
    fn size(&self)            -> f64 { 16.0 }
    fn radius(&self)          -> f64 { 3.0 }
    fn border_width(&self)    -> f64 { 1.0 }
    fn checkmark_width(&self) -> f64 { 2.0 }
    fn label_gap(&self)       -> f64 { 12.0 }
    fn checkmark_inset(&self) -> f64 { 4.0 }
}

/// Visibility checkbox (section 22 — `indicator_settings.rs`).
/// Size 16×16, radius 3.0, label gap 8.
pub struct VisibilityCheckboxStyle;

impl CheckboxStyle for VisibilityCheckboxStyle {
    fn size(&self)            -> f64 { 16.0 }
    fn radius(&self)          -> f64 { 3.0 }
    fn border_width(&self)    -> f64 { 1.0 }
    fn checkmark_width(&self) -> f64 { 2.0 }
    fn label_gap(&self)       -> f64 { 8.0 }
}

/// Level-visibility checkbox (section 23 — `primitive_settings.rs`).
/// Size 16×16, radius 2.0 (sole distinction from Visibility).
pub struct LevelVisibilityCheckboxStyle;

impl CheckboxStyle for LevelVisibilityCheckboxStyle {
    fn size(&self)            -> f64 { 16.0 }
    fn radius(&self)          -> f64 { 2.0 }
    fn border_width(&self)    -> f64 { 1.0 }
    fn checkmark_width(&self) -> f64 { 2.0 }
    fn label_gap(&self)       -> f64 { 8.0 }
}

/// Notification checkbox (section 24 — `alert_settings.rs`).
/// Size 16×16, radius 2.0, no checkmark — uses filled inner rect.
pub struct NotificationCheckboxStyle;

impl CheckboxStyle for NotificationCheckboxStyle {
    fn size(&self)            -> f64 { 16.0 }
    fn radius(&self)          -> f64 { 2.0 }
    fn border_width(&self)    -> f64 { 1.0 }
    fn checkmark_width(&self) -> f64 { 2.0 }
    fn label_gap(&self)       -> f64 { 8.0 }
}
