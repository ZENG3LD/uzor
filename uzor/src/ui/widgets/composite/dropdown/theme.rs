//! Dropdown colour palette trait and default dark-theme implementation.
//!
//! Token values ported from the mlc audit (`dropdown-deep.md` §6).

/// Colour tokens for the Dropdown composite.
///
/// Covers both the trigger button (closed state) and the open menu panel.
pub trait DropdownTheme {
    // --- Panel frame ---

    /// Menu panel background (fully opaque).  Default: `#1e222d`.
    fn bg(&self) -> &str;

    /// Menu panel border (1 px stroke).  Default: `#363a45`.
    fn border(&self) -> &str;

    /// Shadow rect fill.  Default: `rgba(0,0,0,0.5)`.
    fn shadow(&self) -> &str;

    // --- Items ---

    /// Normal item background (transparent / same as bg).  Default: `#1e222d`.
    fn item_bg_normal(&self) -> &str;

    /// Hovered item background.  Default: `#2a2e39`.
    fn item_bg_hover(&self) -> &str;

    /// Selected / active item background.  Default: `rgba(41,98,255,0.15)`.
    fn item_bg_selected(&self) -> &str;

    /// Danger item hover background.  Default: `rgba(242,54,69,0.15)`.
    fn item_bg_danger_hover(&self) -> &str;

    /// Normal item text colour.  Default: `#d1d4dc`.
    fn item_text(&self) -> &str;

    /// Hovered item text colour.  Default: `#ffffff`.
    fn item_text_hover(&self) -> &str;

    /// Disabled item text colour.  Default: `#6a6d78`.
    fn item_text_disabled(&self) -> &str;

    /// Danger item text colour.  Default: `#f23645`.
    fn item_text_danger(&self) -> &str;

    // --- Headers ---

    /// Section header text colour.  Default: `#ffffff`.
    fn header_text(&self) -> &str;

    /// Header bottom separator line colour.  Default: `#363a45`.
    fn header_border(&self) -> &str;

    // --- Separators ---

    /// Separator line colour.  Default: `#363a45`.
    fn separator(&self) -> &str;

    // --- Right-side content ---

    /// Shortcut / subtitle text colour (right-aligned).  Default: `#6a6d78`.
    fn shortcut_text(&self) -> &str;

    // --- Submenu caret ---

    /// Submenu arrow / caret colour.  Default: `#6a6d78`.
    fn caret_color(&self) -> &str;

    // --- Toggle ---

    /// Toggle track colour when on.  Default: `#2962ff`.
    fn toggle_on(&self) -> &str;

    /// Toggle track colour when off.  Default: `#6a6d78`.
    fn toggle_off(&self) -> &str;

    /// Toggle thumb colour.  Default: `#ffffff`.
    fn toggle_thumb(&self) -> &str;

    // --- Trigger button (closed state) ---

    /// Trigger button background.  Default: `#2a2e39`.
    fn trigger_bg(&self) -> &str;

    /// Trigger button background on hover.  Default: `#363a45`.
    fn trigger_bg_hover(&self) -> &str;

    /// Trigger button border.  Default: `#363a45`.
    fn trigger_border(&self) -> &str;

    /// Trigger button text.  Default: `#d1d4dc`.
    fn trigger_text(&self) -> &str;

    /// Trigger button chevron / arrow.  Default: `#6a6d78`.
    fn trigger_arrow(&self) -> &str;

    // --- Checkbox (Grouped template) ---

    /// Checkbox stroke colour (unchecked).  Default: `#6a6d78`.
    fn checkbox_border(&self) -> &str;

    /// Checkbox fill colour (checked).  Default: `#2962ff`.
    fn checkbox_checked(&self) -> &str;

    // --- Grid cell (Grid / Grouped templates) ---

    /// Grid cell hover background.  Default: `#2a2e39`.
    fn cell_bg_hover(&self) -> &str;

    /// Grid cell border colour.  Default: `#363a45`.
    fn cell_border(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Default dark theme
// ---------------------------------------------------------------------------

/// Default dark-theme implementation.
///
/// Values sourced from the mlc audit (`dropdown-deep.md` §6).
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultDropdownTheme;

impl DropdownTheme for DefaultDropdownTheme {
    // Panel frame
    fn bg(&self)     -> &str { "#1e222d" }
    fn border(&self) -> &str { "#363a45" }
    fn shadow(&self) -> &str { "rgba(0,0,0,0.5)" }

    // Items
    fn item_bg_normal(&self)       -> &str { "#1e222d" }
    fn item_bg_hover(&self)        -> &str { "#2a2e39" }
    fn item_bg_selected(&self)     -> &str { "rgba(41,98,255,0.15)" }
    fn item_bg_danger_hover(&self) -> &str { "rgba(242,54,69,0.15)" }
    fn item_text(&self)            -> &str { "#d1d4dc" }
    fn item_text_hover(&self)      -> &str { "#ffffff" }
    fn item_text_disabled(&self)   -> &str { "#6a6d78" }
    fn item_text_danger(&self)     -> &str { "#f23645" }

    // Headers
    fn header_text(&self)   -> &str { "#ffffff" }
    fn header_border(&self) -> &str { "#363a45" }

    // Separators
    fn separator(&self) -> &str { "#363a45" }

    // Right-side content
    fn shortcut_text(&self) -> &str { "#6a6d78" }

    // Submenu caret
    fn caret_color(&self) -> &str { "#6a6d78" }

    // Toggle
    fn toggle_on(&self)    -> &str { "#2962ff" }
    fn toggle_off(&self)   -> &str { "#6a6d78" }
    fn toggle_thumb(&self) -> &str { "#ffffff" }

    // Trigger button
    fn trigger_bg(&self)       -> &str { "#2a2e39" }
    fn trigger_bg_hover(&self) -> &str { "#363a45" }
    fn trigger_border(&self)   -> &str { "#363a45" }
    fn trigger_text(&self)     -> &str { "#d1d4dc" }
    fn trigger_arrow(&self)    -> &str { "#6a6d78" }

    // Checkbox
    fn checkbox_border(&self)  -> &str { "#6a6d78" }
    fn checkbox_checked(&self) -> &str { "#2962ff" }

    // Grid cell
    fn cell_bg_hover(&self) -> &str { "#2a2e39" }
    fn cell_border(&self)   -> &str { "#363a45" }
}
