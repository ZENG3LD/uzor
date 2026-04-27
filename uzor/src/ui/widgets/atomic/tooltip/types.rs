//! Tooltip type definitions.

use crate::types::Rect;

/// Where the tooltip appears relative to its anchor rect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TooltipPosition {
    Above,
    Below,
    Left,
    Right,
}

/// Selects which mlc tooltip variant to use.
///
/// - `ChromeButton`   — single-line, 500 ms delay, 150 ms fade-in, shadow, auto-flip.
/// - `ToolbarButton`  — same renderer as Chrome but 700 ms delay.
/// - `Crosshair`      — multi-line, no delay, no fade, border, clamp-only positioning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TooltipKind {
    #[default]
    ChromeButton,
    ToolbarButton,
    Crosshair,
}

/// Configuration for a tooltip popup.
#[derive(Debug, Clone)]
pub struct TooltipConfig {
    /// Primary single-line text (or `\n`-separated for multi-line crosshair variant).
    pub text: String,
    /// Additional lines for the crosshair variant.
    /// When non-empty, `text` is used as the first line.
    pub lines: Option<Vec<String>>,
    /// Rect of the widget that triggered the tooltip (screen coords).
    pub anchor: Rect,
    /// Preferred placement relative to the anchor.
    pub position: TooltipPosition,
}

impl TooltipConfig {
    pub fn new(text: impl Into<String>, anchor: Rect, position: TooltipPosition) -> Self {
        Self { text: text.into(), lines: None, anchor, position }
    }

    pub fn above(text: impl Into<String>, anchor: Rect) -> Self {
        Self::new(text, anchor, TooltipPosition::Above)
    }

    pub fn below(text: impl Into<String>, anchor: Rect) -> Self {
        Self::new(text, anchor, TooltipPosition::Below)
    }

    /// Create a multi-line crosshair tooltip. `lines` must not be empty.
    pub fn multiline(lines: Vec<String>, anchor: Rect) -> Self {
        let text = lines.first().cloned().unwrap_or_default();
        Self { text, lines: Some(lines), anchor, position: TooltipPosition::Below }
    }

    /// Resolve the effective line list: explicit `lines` if set,
    /// otherwise split `text` on `'\n'`.
    pub fn resolved_lines(&self) -> Vec<&str> {
        if let Some(lines) = &self.lines {
            lines.iter().map(|s| s.as_str()).collect()
        } else {
            self.text.split('\n').collect()
        }
    }
}

/// Output of a tooltip — no interactive events, provided for completeness.
#[derive(Debug, Clone, Default)]
pub struct TooltipResponse {
    /// Whether the tooltip is currently visible (past the show delay).
    pub visible: bool,
}
