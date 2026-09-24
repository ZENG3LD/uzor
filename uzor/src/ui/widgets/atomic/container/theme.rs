//! Container colour palette.
//!
//! Two parallel mlc theme families are bridged here:
//! - `ToolbarTheme` slots: used by Card / Bordered / Panel(Toolbar|Sidebar)
//! - `PanelTheme` slots: used by Plain / Section / Panel(StatusBar)

/// Full container theme — covers all six `ContainerType` variants.
pub trait ContainerTheme {
    // -------------------------------------------------------------------------
    // Shared
    // -------------------------------------------------------------------------

    /// Primary background fill.
    fn bg(&self) -> &str;

    /// 1px border / separator color.
    fn border(&self) -> &str;

    /// Shadow fill color (used by Card; should be semi-transparent).
    fn shadow(&self) -> &str;

    // -------------------------------------------------------------------------
    // Card-specific
    // -------------------------------------------------------------------------

    /// Shadow color specifically for Card containers.
    ///
    /// Separated from generic `shadow` so Card and other future
    /// shadow-bearing types can differ. mlc default: `rgba(0,0,0,0.4)`.
    fn card_shadow_color(&self) -> &str {
        "rgba(0,0,0,0.4)"
    }

    // -------------------------------------------------------------------------
    // Section (header strip)
    // -------------------------------------------------------------------------

    /// Header strip background.
    ///
    /// mlc `PanelTheme::header_bg` default: `#161b22ff`.
    fn section_header_bg(&self) -> &str {
        "#161b22ff"
    }

    /// Header strip text color (for callers that render a label).
    ///
    /// mlc `ModalTheme::header_text` default: `#ffffff`.
    fn section_header_text(&self) -> &str {
        "#ffffff"
    }

    // -------------------------------------------------------------------------
    // Panel (PanelTheme bridge)
    // -------------------------------------------------------------------------

    /// Panel body background.
    ///
    /// mlc `PanelTheme::panel_bg` (bridged from `RuntimeTheme::toolbar_bg`): `#0d1117ff`.
    fn panel_bg(&self) -> &str {
        "#0d1117ff"
    }

    /// Panel border / separator.
    ///
    /// mlc `PanelTheme::separator`: `#30363dff`.
    fn panel_border(&self) -> &str {
        "#30363dff"
    }
}

// =============================================================================
// Token-contract implementation
// =============================================================================
//
// `DefaultContainerTheme` (a literal-colour prototype impl) was deleted in
// H1 Brief 6 — `crate::tokens::theme::TokenTheme` is now the one
// `ContainerTheme` implementation ships, backed by
// `crate::ui::widgets::atomic::container::tokens::ContainerTokens` (see
// `docs/uzor/plans/h1-token-contract-design-2026-09-24.md` §3). `TokenTheme`
// overrides all 8 methods explicitly, including the 5 with a default body
// above that returns its own literal (`card_shadow_color`,
// `section_header_bg`, `section_header_text`, `panel_bg`, `panel_border`) —
// each is reachable from render code like any other, per H1's scope note on
// trait-default methods.
