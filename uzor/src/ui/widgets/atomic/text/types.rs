//! Text widget type definitions.

use crate::render::{TextAlign, TextBaseline};

/// What the Text widget should do when content overflows the rect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextOverflow {
    /// Hard clip at rect boundary — content cut off mid-glyph.
    Clip,
    /// Truncate and append "…" before the right edge.
    Ellipsis,
    /// Wrap to next line (multi-line).
    Wrap,
}

/// Per-frame text view.
pub struct TextView<'a> {
    pub text:     &'a str,
    pub align:    TextAlign,
    pub baseline: TextBaseline,
    /// Color override. If `None`, uses `theme.text_color()`.
    pub color:    Option<&'a str>,
    /// Optional font CSS-shorthand (e.g. `"13px Roboto"`). If `None`, uses `style.font()`.
    pub font:     Option<&'a str>,
    /// What to do on overflow. Default `Clip`.
    pub overflow: TextOverflow,
    /// Soft hint: hovered (caller-supplied).
    pub hovered:  bool,
}

impl<'a> Default for TextView<'a> {
    fn default() -> Self {
        Self {
            text:     "",
            align:    TextAlign::Left,
            baseline: TextBaseline::Middle,
            color:    None,
            font:     None,
            overflow: TextOverflow::Clip,
            hovered:  false,
        }
    }
}

/// Render variants. For now only `Plain`.
pub enum TextRenderKind {
    Plain,
}
