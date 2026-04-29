//! ContextMenu colour palette trait and default dark-theme implementation.
//!
//! Reuses the same token set as `DropdownTheme` — both menus share a common
//! dark-panel vocabulary.  ContextMenu does NOT use header, shortcut, toggle,
//! checkbox, or cell tokens; they are included here for completeness so the
//! same theme instance can back both widgets.

/// Colour tokens for the ContextMenu composite.
///
/// Token names and defaults match `DropdownTheme` exactly so that a shared
/// dark-theme implementation can serve both widgets.
pub trait ContextMenuTheme {
    // --- Panel frame ---

    /// Menu panel background.  Default: `#1e222d`.
    fn bg(&self) -> &str;

    /// Panel border (1 px stroke).  Default: `#363a45`.
    fn border(&self) -> &str;

    /// Shadow rect fill.  Default: `rgba(0,0,0,0.3)`.
    fn shadow(&self) -> &str;

    // --- Items ---

    /// Normal item background (transparent / same as bg).  Default: `#1e222d`.
    fn item_bg_normal(&self) -> &str;

    /// Hovered item background.  Default: `#2a2e39`.
    fn item_bg_hover(&self) -> &str;

    /// Danger item hover background.  Default: `rgba(242,54,69,0.15)`.
    fn item_bg_danger_hover(&self) -> &str;

    /// Normal item text colour.  Default: `#d1d4dc`.
    fn item_text(&self) -> &str;

    /// Hovered item text colour.  Default: `#ffffff`.
    fn item_text_hover(&self) -> &str;

    /// Disabled item text colour.  Default: `#6a6d78`.
    fn item_text_disabled(&self) -> &str;

    /// Danger item text colour (red).  Default: `#f23645`.
    fn item_text_danger(&self) -> &str;

    // --- Separator ---

    /// Separator line colour.  Default: `#363a45`.
    fn separator(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Default dark theme
// ---------------------------------------------------------------------------

/// Default dark-theme implementation.
///
/// Values match the mlc audit token table in `context-menu-deep.md` §5.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultContextMenuTheme;

impl ContextMenuTheme for DefaultContextMenuTheme {
    // Panel frame
    fn bg(&self)     -> &str { "#1e222d" }
    fn border(&self) -> &str { "#363a45" }
    fn shadow(&self) -> &str { "rgba(0,0,0,0.3)" }

    // Items
    fn item_bg_normal(&self)       -> &str { "#1e222d" }
    fn item_bg_hover(&self)        -> &str { "#2a2e39" }
    fn item_bg_danger_hover(&self) -> &str { "rgba(242,54,69,0.15)" }
    fn item_text(&self)            -> &str { "#d1d4dc" }
    fn item_text_hover(&self)      -> &str { "#ffffff" }
    fn item_text_disabled(&self)   -> &str { "#6a6d78" }
    fn item_text_danger(&self)     -> &str { "#f23645" }

    // Separator
    fn separator(&self) -> &str { "#363a45" }
}
