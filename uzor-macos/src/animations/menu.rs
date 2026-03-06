//! macOS menu animation presets

/// Menu animation configuration
pub struct MenuAnimation {
    pub duration_ms: f64,
    pub opacity_from: f64,
    pub opacity_to: f64,
    pub scale_y_from: f64,
    pub scale_y_to: f64,
    pub bezier: (f64, f64, f64, f64),  // cubic-bezier control points
}

/// Menu opening animation
/// - 80ms duration (fast, snappy)
/// - Opacity: 0.0 → 1.0
/// - ScaleY: 0.95 → 1.0 (vertical expand from anchor)
/// - cubic-bezier(0.2, 0, 0.38, 0.9) — ease-out with slight anticipation
pub const OPEN: MenuAnimation = MenuAnimation {
    duration_ms: 80.0,
    opacity_from: 0.0,
    opacity_to: 1.0,
    scale_y_from: 0.95,
    scale_y_to: 1.0,
    bezier: (0.2, 0.0, 0.38, 0.9),
};

/// Menu closing animation
/// - 200ms duration (slower close feels more natural)
/// - Opacity: 1.0 → 0.0
/// - Scale is not animated on close (instant collapse feels cleaner)
pub const CLOSE: MenuAnimation = MenuAnimation {
    duration_ms: 200.0,
    opacity_from: 1.0,
    opacity_to: 0.0,
    scale_y_from: 1.0,
    scale_y_to: 1.0,  // No scale change on close
    bezier: (0.2, 0.0, 0.38, 0.9),
};
