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

/// Default theme with mlc dark-mode hex values.
#[derive(Default)]
pub struct DefaultContainerTheme;

impl ContainerTheme for DefaultContainerTheme {
    // bg: mlc ToolbarTheme::background = #1e1e2e; also dropdown_bg / button_bg = #1e222d.
    // We use the toolbar background as the universal default.
    fn bg(&self) -> &str {
        "#1e1e2e"
    }

    // border: mlc ToolbarTheme::separator = #313244; PopupTheme::border = #363a45.
    // Using the popup/modal value as the universal default.
    fn border(&self) -> &str {
        "#363a45"
    }

    // shadow: generic semi-transparent black.
    fn shadow(&self) -> &str {
        "rgba(0,0,0,0.4)"
    }
}
