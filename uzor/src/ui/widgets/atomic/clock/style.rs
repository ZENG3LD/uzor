//! Clock widget geometry trait and presets.

/// Geometry parameters for `draw_clock`.
pub trait ClockStyle {
    /// Font string used for the time text. Default: `"13px monospace"`.
    fn font(&self) -> &str {
        "13px monospace"
    }

    /// Corner radius of the hover background rect. Default: 4.0.
    fn hover_bg_radius(&self) -> f64 {
        4.0
    }

    /// Vertical inset applied to the hover background rect on each side.
    /// The hover rect is drawn at `rect.y + inset`, height `rect.height - 2*inset`.
    /// Default: 2.0 (matches mlc toolbar_core.rs).
    fn hover_bg_vertical_inset(&self) -> f64 {
        2.0
    }

    /// Right padding between text and rect right edge. Default: 8.0.
    fn text_padding_right(&self) -> f64 {
        8.0
    }
}

/// Default clock style — toolbar variant (mlc default).
pub struct DefaultClockStyle;

impl ClockStyle for DefaultClockStyle {}
