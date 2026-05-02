//! Active-frame view + render kinds.

use crate::types::Rect;

/// Which visual preset to use for the active frame.
///
/// More variants (DashedFrame, GlowFrame, AccentBar) can be added later
/// without breaking callers — each new variant just renders differently.
#[derive(Clone, Copy, Debug, Default)]
pub enum ActiveFrameKind {
    /// Plain solid stroke around the rect.
    #[default]
    Stroke,
}

/// Per-frame paint config for [`crate::ui::widgets::atomic::active_frame::render::draw_active_frame`].
#[derive(Clone, Copy, Debug)]
pub struct ActiveFrameView<'a> {
    /// Bounds of the rect to highlight.
    pub rect:  Rect,
    /// Stroke color (CSS-style string — same convention as the rest of uzor).
    pub color: &'a str,
    /// Stroke width in logical pixels.
    pub width: f64,
}
