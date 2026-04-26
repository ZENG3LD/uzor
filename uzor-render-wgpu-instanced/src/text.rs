//! Text data accumulated during a frame.
//!
//! `TextAreaData` stores all parameters for a text draw call collected during
//! a frame.  Text rendering via a glyph rasterizer will be added back later
//! with a different approach (glyphon has been removed for now).

use uzor::fonts::FontFamily;
use uzor::render::{TextAlign, TextBaseline};

/// Raw data for a text draw call collected during a frame.
#[derive(Debug, Clone)]
pub struct TextAreaData {
    /// The text string to draw.
    pub text: String,
    /// Horizontal anchor in logical pixels.
    pub x: f32,
    /// Vertical anchor in logical pixels.
    pub y: f32,
    /// Font size in points / pixels.
    pub font_size: f32,
    /// Text color RGBA.
    pub color: [f32; 4],
    /// Font family (Roboto / PtRootUi / JetBrainsMono).
    pub family: FontFamily,
    /// Whether the font is bold.
    pub bold: bool,
    /// Whether the font is italic.
    pub italic: bool,
    /// Horizontal alignment relative to `x`.
    pub align: TextAlign,
    /// Vertical baseline relative to `y`.
    pub baseline: TextBaseline,
    /// Clip rectangle (x, y, w, h) — text outside is not drawn.
    pub clip: [f32; 4],
    /// Estimated text width in pixels (computed by the context before pushing).
    pub estimated_width: f32,
    /// Estimated text height in pixels.
    pub estimated_height: f32,
}

impl TextAreaData {
    /// Compute the top-left pixel position for this text area, taking alignment
    /// and baseline into account.
    ///
    /// Returns `(left, top)` ready for use as position of the text area.
    pub fn top_left(&self, ascent: f32, descent: f32) -> (f32, f32) {
        let left = match self.align {
            TextAlign::Left => self.x,
            TextAlign::Center => self.x - self.estimated_width * 0.5,
            TextAlign::Right => self.x - self.estimated_width,
        };

        let top = match self.baseline {
            TextBaseline::Top => self.y,
            TextBaseline::Middle => self.y - (ascent - descent) * 0.5,
            TextBaseline::Bottom => self.y - (ascent - descent),
            TextBaseline::Alphabetic => self.y - ascent,
        };

        (left, top)
    }

    /// Convert this text area's color to `[u8; 4]` RGBA.
    pub fn color_u8(&self) -> [u8; 4] {
        [
            (self.color[0] * 255.0).clamp(0.0, 255.0) as u8,
            (self.color[1] * 255.0).clamp(0.0, 255.0) as u8,
            (self.color[2] * 255.0).clamp(0.0, 255.0) as u8,
            (self.color[3] * 255.0).clamp(0.0, 255.0) as u8,
        ]
    }
}
