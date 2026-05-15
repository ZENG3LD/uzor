//! [`TextRenderer`] — stateful text rendering (current font applies).

use super::painter::Painter;
use super::types::{TextAlign, TextBaseline};

/// Text rendering — stateful (current font applies).
///
/// Has [`Painter`] as a supertrait so default impls can use
/// save/translate/rotate/restore without `where Self: Painter` clauses
/// (which would break dyn compatibility).
pub trait TextRenderer: Painter {
    /// Set the current font using a CSS-style font string.
    ///
    /// Examples: `"14px sans-serif"`, `"bold 16px monospace"`.
    fn set_font(&mut self, font: &str);

    /// Set text horizontal alignment.
    fn set_text_align(&mut self, align: TextAlign);

    /// Set text vertical baseline.
    fn set_text_baseline(&mut self, baseline: TextBaseline);

    /// Fill text at position `(x, y)` using the current font and fill style.
    fn fill_text(&mut self, text: &str, x: f64, y: f64);

    /// Stroke text outlines. Backends without native stroke-text provide a no-op.
    fn stroke_text(&mut self, text: &str, x: f64, y: f64) {
        let _ = (text, x, y);
    }

    /// Fill text with rotation around the anchor point.
    ///
    /// Default uses save/translate/rotate/fill_text/restore.
    /// Backends (vello-gpu) override for pre-rotation baseline correctness.
    fn fill_text_rotated(&mut self, text: &str, x: f64, y: f64, angle: f64) {
        if angle.abs() < 0.001 {
            self.fill_text(text, x, y);
        } else {
            self.save();
            self.translate(x, y);
            self.rotate(angle);
            self.fill_text(text, 0.0, 0.0);
            self.restore();
        }
    }

    /// Fill text centered at position.
    ///
    /// Default sets [`TextAlign::Center`] + [`TextBaseline::Middle`] then calls
    /// [`fill_text`](Self::fill_text).
    fn fill_text_centered(&mut self, text: &str, x: f64, y: f64) {
        self.set_text_align(TextAlign::Center);
        self.set_text_baseline(TextBaseline::Middle);
        self.fill_text(text, x, y);
    }
}
