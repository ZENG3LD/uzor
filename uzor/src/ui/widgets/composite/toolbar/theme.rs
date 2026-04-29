//! Toolbar colour palette trait and default dark-theme implementation.
//!
//! Token values ported from the mlc audit (`toolbar-deep.md` §6).

/// Colour tokens for the Toolbar composite.
///
/// Covers background, separators, item states, text, and icon colours.
pub trait ToolbarTheme {
    // --- Background ---

    /// Toolbar background fill.  Default: `#1e222d`.
    fn bg(&self) -> &str;

    // --- Separator ---

    /// Separator line colour.  Default: `#363a45`.
    fn separator(&self) -> &str;

    // --- Item backgrounds ---

    /// Normal item background (transparent over toolbar bg).  Default: `transparent`.
    fn item_bg_normal(&self) -> &str;

    /// Hovered item background.  Default: `#2a2e39`.
    fn item_bg_hover(&self) -> &str;

    /// Active / toggled item background.  Default: `rgba(41,98,255,0.15)`.
    fn item_bg_active(&self) -> &str;

    /// Pressed item background.  Default: `rgba(41,98,255,0.25)`.
    fn item_bg_pressed(&self) -> &str;

    // --- Item text ---

    /// Normal item text colour.  Default: `#d1d4dc`.
    fn item_text_normal(&self) -> &str;

    /// Hovered item text colour.  Default: `#ffffff`.
    fn item_text_hover(&self) -> &str;

    /// Active item text colour.  Default: `#2962ff`.
    fn item_text_active(&self) -> &str;

    /// Disabled item text colour.  Default: `#6a6d78`.
    fn item_text_disabled(&self) -> &str;

    // --- Item icons ---

    /// Normal icon colour.  Default: `#d1d4dc`.
    fn icon_normal(&self) -> &str;

    /// Hovered icon colour.  Default: `#ffffff`.
    fn icon_hover(&self) -> &str;

    /// Active icon colour.  Default: `#2962ff`.
    fn icon_active(&self) -> &str;

    /// Disabled icon colour.  Default: `#6a6d78`.
    fn icon_disabled(&self) -> &str;

    // --- Scroll chevron ---

    /// Overflow scroll chevron colour.  Default: `#6a6d78`.
    fn scroll_chevron_color(&self) -> &str;

    // --- Label / Clock ---

    /// Non-interactive label text colour.  Default: `#6a6d78`.
    fn label_text(&self) -> &str;

    /// Clock text colour.  Default: `#d1d4dc`.
    fn clock_text(&self) -> &str;

    // --- ChromeStrip tabs ---

    /// Active tab background.  Default: `#1e222d`.
    fn chrome_tab_bg_active(&self) -> &str;

    /// Inactive tab background.  Default: `transparent`.
    fn chrome_tab_bg_inactive(&self) -> &str;

    /// Hovered tab background.  Default: `#2a2e39`.
    fn chrome_tab_bg_hover(&self) -> &str;

    /// Active tab text colour.  Default: `#ffffff`.
    fn chrome_tab_text_active(&self) -> &str;

    /// Inactive tab text colour.  Default: `#6a6d78`.
    fn chrome_tab_text_inactive(&self) -> &str;

    /// Chrome window-control button hover background.  Default: `rgba(255,255,255,0.1)`.
    fn chrome_ctrl_hover(&self) -> &str;

    /// Chrome close-button hover background (red).  Default: `#e81123`.
    fn chrome_close_hover(&self) -> &str;

    /// Chrome close / minimize / maximize icon colour.  Default: `#d1d4dc`.
    fn chrome_ctrl_icon(&self) -> &str;

    // --- Color swatch border ---

    /// Border drawn around the color swatch.  Default: `#363a45`.
    fn color_swatch_border(&self) -> &str;

    // --- Split-button chevron ---

    /// Chevron icon colour in the split-button sub-zone.  Default: `#6a6d78`.
    fn split_chevron(&self) -> &str;

    /// Divider line between main and chevron zones.  Default: `#363a45`.
    fn split_divider(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Default dark theme
// ---------------------------------------------------------------------------

/// Default dark-theme implementation.
///
/// Values sourced from the mlc audit (`toolbar-deep.md` §6).
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultToolbarTheme;

impl ToolbarTheme for DefaultToolbarTheme {
    // Background
    fn bg(&self) -> &str { "#1e222d" }

    // Separator
    fn separator(&self) -> &str { "#363a45" }

    // Item backgrounds
    fn item_bg_normal(&self)   -> &str { "transparent" }
    fn item_bg_hover(&self)    -> &str { "#2a2e39" }
    fn item_bg_active(&self)   -> &str { "rgba(41,98,255,0.15)" }
    fn item_bg_pressed(&self)  -> &str { "rgba(41,98,255,0.25)" }

    // Item text
    fn item_text_normal(&self)   -> &str { "#d1d4dc" }
    fn item_text_hover(&self)    -> &str { "#ffffff" }
    fn item_text_active(&self)   -> &str { "#2962ff" }
    fn item_text_disabled(&self) -> &str { "#6a6d78" }

    // Icons
    fn icon_normal(&self)   -> &str { "#d1d4dc" }
    fn icon_hover(&self)    -> &str { "#ffffff" }
    fn icon_active(&self)   -> &str { "#2962ff" }
    fn icon_disabled(&self) -> &str { "#6a6d78" }

    // Scroll chevron
    fn scroll_chevron_color(&self) -> &str { "#6a6d78" }

    // Label / Clock
    fn label_text(&self) -> &str { "#6a6d78" }
    fn clock_text(&self) -> &str { "#d1d4dc" }

    // ChromeStrip tabs
    fn chrome_tab_bg_active(&self)    -> &str { "#1e222d" }
    fn chrome_tab_bg_inactive(&self)  -> &str { "transparent" }
    fn chrome_tab_bg_hover(&self)     -> &str { "#2a2e39" }
    fn chrome_tab_text_active(&self)  -> &str { "#ffffff" }
    fn chrome_tab_text_inactive(&self) -> &str { "#6a6d78" }
    fn chrome_ctrl_hover(&self)       -> &str { "rgba(255,255,255,0.1)" }
    fn chrome_close_hover(&self)      -> &str { "#e81123" }
    fn chrome_ctrl_icon(&self)        -> &str { "#d1d4dc" }

    // Color swatch
    fn color_swatch_border(&self) -> &str { "#363a45" }

    // Split-button
    fn split_chevron(&self) -> &str { "#6a6d78" }
    fn split_divider(&self) -> &str { "#363a45" }
}
