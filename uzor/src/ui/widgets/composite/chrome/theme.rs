//! Chrome colour palette trait; token-contract default implementation lives
//! in `crate::tokens::theme::TokenTheme`, backed by `super::tokens`.
//!
//! All colour values are CSS hex strings (same convention as `ModalTheme`).
//! The renderer converts them via `RenderContext::set_fill_color`.

// ---------------------------------------------------------------------------
// ChromeTheme trait
// ---------------------------------------------------------------------------

/// Colour tokens for the Chrome composite.
///
/// Implement this on your app theme struct to plug in custom colours.
pub trait ChromeTheme: Send + Sync {
    // --- Background ---

    /// Titlebar strip background.  Default alias: `surface.app_chrome`.
    fn background(&self) -> &str;

    // --- Icons ---

    /// Icon colour in normal (idle) state.  Default alias: `text.secondary`.
    fn icon_normal(&self) -> &str;

    /// Icon colour on hover / active.  Default alias: `text.primary`.
    fn icon_hover(&self) -> &str;

    // --- Button hover backgrounds ---

    /// Hover background fill for most buttons (min, max, menu, …).
    /// Default alias: `surface.control.hover`.
    fn button_hover(&self) -> &str;

    /// Hover background fill for the close-app button (red).
    /// Default alias: `status.danger` (was the literal Windows close-red
    /// `"#e81123"` until H1 §A6 — that broke the high_contrast_mono set's
    /// grayscale contract, one meaning per role).
    fn close_hover(&self) -> &str;

    // --- Structural ---

    /// 1 px divider lines between button groups and the bottom edge.
    /// Default alias: `border.default`.
    fn separator(&self) -> &str;

    // --- Tab strip ---

    /// Normal (inactive) tab background.  Default: `"transparent"`.
    fn tab_bg_normal(&self) -> &str;

    /// Tab background on hover.  Default alias: `surface.control.hover`.
    fn tab_bg_hover(&self) -> &str;

    /// Active tab background.  Default alias: `surface.floating`.
    fn tab_bg_active(&self) -> &str;

    /// Tab label text — inactive / normal.  Default alias: `text.secondary`.
    fn tab_text_normal(&self) -> &str;

    /// Tab label text — hovered.  Default alias: `text.primary`.
    fn tab_text_hover(&self) -> &str;

    /// Tab label text — active.  Default alias: `text.on_accent`.
    fn tab_text_active(&self) -> &str;

    /// Active tab bottom accent line (2 px).  Default alias: `accent.default`.
    fn tab_accent(&self) -> &str;

    // --- Drag zone ---

    /// Caption / drag zone background (usually transparent).
    /// Default: `"transparent"`.
    fn drag_zone_bg(&self) -> &str;

    // --- Tooltip ---

    /// Tooltip background.  Default alias: `surface.control.hover`.
    fn tooltip_bg(&self) -> &str;

    /// Tooltip text colour.  Default alias: `text.on_accent`.
    fn tooltip_text(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Token-contract implementation
// ---------------------------------------------------------------------------
//
// `DefaultChromeTheme` (a literal-colour prototype impl) was deleted in H1
// Brief 8a — `crate::tokens::theme::TokenTheme` is now the one `ChromeTheme`
// implementation this crate ships, backed by
// `crate::ui::widgets::composite::chrome::tokens::ChromeTokens` (see
// `docs/uzor/plans/h1-token-contract-design-2026-09-24.md` §3).
