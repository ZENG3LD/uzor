//! macOS modal animation presets

/// Modal animation configuration
pub struct ModalAnimation {
    pub duration_ms: f64,
    pub opacity_from: f64,
    pub opacity_to: f64,
    pub scale_from: f64,
    pub scale_to: f64,
    pub bezier: (f64, f64, f64, f64),  // cubic-bezier control points (x1, y1, x2, y2)
}

/// Modal opening animation
/// - 300ms duration
/// - Opacity: 0.0 → 1.0
/// - Scale: 1.08 → 1.0 (subtle zoom-in effect)
/// - cubic-bezier(0.22, 0.61, 0.36, 1) — smooth ease-out with bounce
pub const OPEN: ModalAnimation = ModalAnimation {
    duration_ms: 300.0,
    opacity_from: 0.0,
    opacity_to: 1.0,
    scale_from: 1.08,
    scale_to: 1.0,
    bezier: (0.22, 0.61, 0.36, 1.0),
};

/// Modal closing animation
/// - 200ms duration
/// - Opacity: 1.0 → 0.0
/// - Scale: 1.0 → 0.95 (subtle zoom-out effect)
/// - cubic-bezier(0.22, 0.61, 0.36, 1) — matching open curve
pub const CLOSE: ModalAnimation = ModalAnimation {
    duration_ms: 200.0,
    opacity_from: 1.0,
    opacity_to: 0.0,
    scale_from: 1.0,
    scale_to: 0.95,
    bezier: (0.22, 0.61, 0.36, 1.0),
};
