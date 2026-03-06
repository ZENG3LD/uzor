//! macOS button animation presets

/// Button hover transition config
pub struct ButtonHoverAnimation {
    pub duration_ms: f64,
    pub easing: &'static str,  // CSS-like easing name for reference
}

pub const HOVER_IN: ButtonHoverAnimation = ButtonHoverAnimation {
    duration_ms: 100.0,
    easing: "ease-in",
};

pub const HOVER_OUT: ButtonHoverAnimation = ButtonHoverAnimation {
    duration_ms: 200.0,
    easing: "ease-out",
};

pub const PRESS: ButtonHoverAnimation = ButtonHoverAnimation {
    duration_ms: 50.0,
    easing: "ease-in",
};

/// Scale factor for button press (macOS uses subtle scale)
pub const PRESS_SCALE: f64 = 0.97;
