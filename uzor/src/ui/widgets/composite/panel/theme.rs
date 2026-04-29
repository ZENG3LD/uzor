//! Panel colour palette trait and default dark-theme implementation.
//!
//! Token values ported from mlc `panel_theme.rs` audit (`panel-deep.md` §6).

// ---------------------------------------------------------------------------
// PanelTheme trait
// ---------------------------------------------------------------------------

/// Colour tokens for the panel composite.
///
/// Implement on your app theme struct to plug in custom colours.
pub trait PanelTheme {
    // --- Frame ---

    /// Panel body background fill.  Default: `#0d1117`.
    fn bg(&self) -> &str;

    /// Border / outline colour.  Default: `#30363d`.
    fn border(&self) -> &str;

    // --- Header strip ---

    /// Header zone background.  Default: `#161b22`.
    fn header_bg(&self) -> &str;

    /// Header title text.  Default: `#8091a5`.
    fn header_text(&self) -> &str;

    // --- Column-header row ---

    /// Column-header zone background.  Default: `#161b22`.
    fn column_header_bg(&self) -> &str;

    /// Column-header label text.  Default: `#8b949e`.
    fn column_header_text(&self) -> &str;

    // --- Body rows ---

    /// Normal row background (even rows or ungrouped).  Default: `#0d1117`.
    fn row_bg_normal(&self) -> &str;

    /// Row background on hover.  Default: `#2a2f40`.
    fn row_bg_hover(&self) -> &str;

    /// Row background when selected.  Default: `#1e2538`.
    fn row_bg_selected(&self) -> &str;

    // --- Footer ---

    /// Footer zone background.  Default: `#161b22`.
    fn footer_bg(&self) -> &str;

    /// Footer text colour.  Default: `#8b949e`.
    fn footer_text(&self) -> &str;

    // --- Dividers ---

    /// 1 px separator line between zones.  Default: `#30363d`.
    fn divider(&self) -> &str;

    // --- Action buttons ---

    /// Icon colour for header action buttons in idle state.  Default: `#8b949e`.
    fn action_icon_normal(&self) -> &str;

    /// Icon colour for header action buttons on hover.  Default: `#e0e0e0`.
    fn action_icon_hover(&self) -> &str;

    // --- Sort arrow ---

    /// Sort arrow / indicator colour.  Default: `#58a6ff`.
    fn sort_arrow_color(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Default dark theme
// ---------------------------------------------------------------------------

/// Default dark-theme implementation matching mlc panel colours.
#[derive(Default)]
pub struct DefaultPanelTheme;

impl PanelTheme for DefaultPanelTheme {
    // Frame
    fn bg(&self)     -> &str { "#0d1117" }
    fn border(&self) -> &str { "#30363d" }

    // Header strip
    fn header_bg(&self)   -> &str { "#161b22" }
    fn header_text(&self) -> &str { "#8091a5" }

    // Column-header row
    fn column_header_bg(&self)   -> &str { "#161b22" }
    fn column_header_text(&self) -> &str { "#8b949e" }

    // Body rows
    fn row_bg_normal(&self)   -> &str { "#0d1117" }
    fn row_bg_hover(&self)    -> &str { "#2a2f40" }
    fn row_bg_selected(&self) -> &str { "#1e2538" }

    // Footer
    fn footer_bg(&self)   -> &str { "#161b22" }
    fn footer_text(&self) -> &str { "#8b949e" }

    // Dividers
    fn divider(&self) -> &str { "#30363d" }

    // Action buttons
    fn action_icon_normal(&self) -> &str { "#8b949e" }
    fn action_icon_hover(&self)  -> &str { "#e0e0e0" }

    // Sort arrow
    fn sort_arrow_color(&self) -> &str { "#58a6ff" }
}
