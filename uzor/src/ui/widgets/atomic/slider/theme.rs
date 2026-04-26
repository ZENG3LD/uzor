//! Slider colour palette.
//!
//! Tokens cover the centralised renderer (variants 1.1–1.4) plus the
//! toolbar-style line-width variant (1.5).

pub trait SliderTheme {
    // ── Track tokens ─────────────────────────────────────────────────────────

    /// Empty (right) portion of the track (mlc `border_normal = "#363a45"`).
    fn track_empty(&self) -> &str;

    /// Filled (left) portion of the track + hover halo (mlc `accent = "#2196F3"`).
    fn accent(&self) -> &str;

    // ── Handle tokens ────────────────────────────────────────────────────────

    /// Handle fill + label text (mlc `text_normal = "#d1d4dc"`).
    fn text_normal(&self) -> &str;

    /// Handle fill when disabled.
    fn text_disabled(&self) -> &str;

    /// Handle border colour (distinct from accent when overriding). Defaults to `accent`.
    fn handle_border(&self) -> &str {
        self.accent()
    }

    // ── Input box tokens (variants 1.1 / 1.2 / 1.4) ─────────────────────────

    /// Input box background (mlc `bg_normal = "#2a2e39"`).
    fn input_bg(&self) -> &str;

    /// Input box border when not editing.
    fn input_border_normal(&self) -> &str;

    /// Input box border when editing (mlc `border_focused = "#2196F3"`).
    fn input_border_focused(&self) -> &str;

    /// Input box text colour (same as `text_normal`).
    fn input_text(&self) -> &str {
        self.text_normal()
    }

    // ── Toolbar-style tokens (variant 1.5 line-width slider) ─────────────────

    /// Track empty colour when using toolbar style.
    fn toolbar_track_empty(&self) -> &str;

    /// Filled portion when using toolbar style.
    fn toolbar_track_filled(&self) -> &str;

    /// Handle + label colour when using toolbar style.
    fn toolbar_handle(&self) -> &str;
}

// ─── Dark default ─────────────────────────────────────────────────────────────

pub struct DefaultSliderTheme;

impl Default for DefaultSliderTheme {
    fn default() -> Self {
        Self
    }
}

impl SliderTheme for DefaultSliderTheme {
    fn track_empty(&self)          -> &str { "#363a45" }
    fn accent(&self)               -> &str { "#2196F3" }
    fn text_normal(&self)          -> &str { "#d1d4dc" }
    fn text_disabled(&self)        -> &str { "#787b86" }
    fn input_bg(&self)             -> &str { "#2a2e39" }
    fn input_border_normal(&self)  -> &str { "#363a45" }
    fn input_border_focused(&self) -> &str { "#2196F3" }
    // Toolbar-style (variant 1.5) — maps to toolbar_theme.separator /
    // item_bg_active / item_text from mlc compare_settings.
    fn toolbar_track_empty(&self)  -> &str { "#363a45" }
    fn toolbar_track_filled(&self) -> &str { "#2196F3" }
    fn toolbar_handle(&self)       -> &str { "#d1d4dc" }
}
