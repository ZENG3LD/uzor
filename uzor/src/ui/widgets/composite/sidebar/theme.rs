//! Sidebar colour palette trait and default dark-theme implementation.
//!
//! Token values ported from `ToolbarTheme` usage in the mlc sidebar audit
//! (`sidebar-deep.md` §6).

/// Colour tokens for the sidebar composite.
///
/// Implement on your app theme struct to plug in custom colours.
pub trait SidebarTheme {
    // --- Frame ---

    /// Sidebar body and header background fill.  Default: `#1e222d`.
    fn bg(&self) -> &str;

    /// Left/right border line (1 px separator between chart area and sidebar).
    /// Default: `#363a45`.
    fn border(&self) -> &str;

    // --- Header ---

    /// Header zone background (usually same as `bg`).  Default: `#1e222d`.
    fn header_bg(&self) -> &str;

    /// Header title text.  Default: `#ffffff`.
    fn header_text(&self) -> &str;

    /// Header icon colour (normal).  Default: `#9598a1`.
    fn header_icon(&self) -> &str;

    /// Header bottom divider line.  Default: `#363a45`.
    fn divider(&self) -> &str;

    // --- Action buttons ---

    /// Icon colour for header action buttons in idle state.  Default: `#9598a1`.
    fn action_icon_normal(&self) -> &str;

    /// Icon colour for header action buttons on hover.  Default: `#ffffff`.
    fn action_icon_hover(&self) -> &str;

    // --- Scrollbar (delegated) ---

    /// Scrollbar thumb colour in idle state.  Default: `#363a45`.
    fn scrollbar_thumb(&self) -> &str;

    /// Scrollbar thumb colour on hover / drag.  Default: `#787b86`.
    fn scrollbar_thumb_active(&self) -> &str;

    // --- Tab strip (WithTypeSelector) ---

    /// Active tab text colour.  Default: `#ffffff`.
    fn tab_text_active(&self) -> &str;

    /// Inactive tab text colour.  Default: `#787b86`.
    fn tab_text_inactive(&self) -> &str;

    /// Active tab underline accent.  Default: `#2962ff`.
    fn tab_accent(&self) -> &str;

    /// Active tab background highlight.  Default: `rgba(41,98,255,0.12)`.
    fn tab_bg_active(&self) -> &str;

    /// Hovered tab background.  Default: `rgba(255,255,255,0.06)`.
    fn tab_bg_hover(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Default dark theme
// ---------------------------------------------------------------------------

/// Default dark-theme implementation matching mlc sidebar colours.
#[derive(Default)]
pub struct DefaultSidebarTheme;

impl SidebarTheme for DefaultSidebarTheme {
    // Frame
    fn bg(&self)     -> &str { "#1e222d" }
    fn border(&self) -> &str { "#363a45" }

    // Header
    fn header_bg(&self)   -> &str { "#1e222d" }
    fn header_text(&self) -> &str { "#ffffff" }
    fn header_icon(&self) -> &str { "#9598a1" }
    fn divider(&self)     -> &str { "#363a45" }

    // Action buttons
    fn action_icon_normal(&self) -> &str { "#9598a1" }
    fn action_icon_hover(&self)  -> &str { "#ffffff" }

    // Scrollbar
    fn scrollbar_thumb(&self)        -> &str { "#363a45" }
    fn scrollbar_thumb_active(&self) -> &str { "#787b86" }

    // Tab strip
    fn tab_text_active(&self)   -> &str { "#ffffff" }
    fn tab_text_inactive(&self) -> &str { "#787b86" }
    fn tab_accent(&self)        -> &str { "#2962ff" }
    fn tab_bg_active(&self)     -> &str { "rgba(41,98,255,0.12)" }
    fn tab_bg_hover(&self)      -> &str { "rgba(255,255,255,0.06)" }
}
