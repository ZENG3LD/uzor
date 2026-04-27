//! Separator colour palette.

pub trait SeparatorTheme {
    // -------------------------------------------------------------------------
    // Generic divider / resize-handle slots (used by draw_separator)
    // -------------------------------------------------------------------------

    /// Idle line colour for plain dividers and resize handles.
    fn line(&self) -> &str;
    /// Resize-handle colour when hovered.
    fn handle_hover(&self) -> &str;
    /// Resize-handle colour while actively dragged.
    fn handle_active(&self) -> &str;

    // -------------------------------------------------------------------------
    // Pane-handle slots (split-panel / sub-pane separators, §8.1 / §8.2)
    //
    // Split-panel separator uses a different idle colour than the blue drag
    // accent. Sub-pane separator is always the idle colour (no hover change
    // on the visual line itself).
    // -------------------------------------------------------------------------

    /// Idle colour for split-panel and sub-pane separator lines.
    /// mlc: `frame_theme.toolbar_border` / `theme.scale_border` ≈ `"#363a45"`.
    fn pane_handle_idle(&self) -> &str;

    /// Hover / drag colour for split-panel separators.
    /// mlc: `theme.crosshair` ≈ `"#758696"` (or bright white in some themes).
    fn pane_handle_hover(&self) -> &str;

    // -------------------------------------------------------------------------
    // Sidebar separator slot (§8.3 / §1.3)
    //
    // No hover colour change on the 1 px line; cursor changes only.
    // -------------------------------------------------------------------------

    /// Sidebar separator line colour.
    /// mlc: `toolbar_theme.separator`.
    fn sidebar_separator(&self) -> &str;

    // -------------------------------------------------------------------------
    // Modal section divider slot (§1.9 / §8.x)
    //
    // Used for header/footer dividers inside modals (1 px stroke).
    // mlc: `toolbar_theme.separator` — same source as sidebar, but kept as a
    // separate slot so themes can diverge independently.
    // -------------------------------------------------------------------------

    /// Modal section divider colour (stroke, 1 px).
    fn modal_divider(&self) -> &str;
}

// =============================================================================
// Default theme
// =============================================================================

#[derive(Default)]
pub struct DefaultSeparatorTheme;

impl SeparatorTheme for DefaultSeparatorTheme {
    // Generic resize handle
    fn line(&self) -> &str {
        "#3a3a3a"
    }
    fn handle_hover(&self) -> &str {
        "#787b86"
    }
    fn handle_active(&self) -> &str {
        "#2962ff"
    }

    // Pane handles (split-panel + sub-pane)
    fn pane_handle_idle(&self) -> &str {
        "#363a45"
    }
    fn pane_handle_hover(&self) -> &str {
        "#758696"
    }

    // Sidebar separator
    fn sidebar_separator(&self) -> &str {
        "#363a45"
    }

    // Modal section divider
    fn modal_divider(&self) -> &str {
        "#363a45"
    }
}
