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
    /// shadow-bearing types can differ. Required (no default body, H1 §A6):
    /// a literal trait-default body is reachable from any third-party
    /// `ContainerTheme` impl that does not override it.
    fn card_shadow_color(&self) -> &str;

    // -------------------------------------------------------------------------
    // Section (header strip)
    // -------------------------------------------------------------------------

    /// Header strip background. Required for the same reason as
    /// `card_shadow_color`.
    fn section_header_bg(&self) -> &str;

    /// Header strip text color (for callers that render a label). Required
    /// for the same reason as `card_shadow_color`.
    fn section_header_text(&self) -> &str;

    // -------------------------------------------------------------------------
    // Panel (PanelTheme bridge)
    // -------------------------------------------------------------------------

    /// Panel body background. Required for the same reason as
    /// `card_shadow_color`.
    fn panel_bg(&self) -> &str;

    /// Panel border / separator. Required for the same reason as
    /// `card_shadow_color`.
    fn panel_border(&self) -> &str;
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
// implements all 8 methods explicitly; `card_shadow_color`,
// `section_header_bg`, `section_header_text`, `panel_bg`, `panel_border` were
// literal trait-default bodies until H1 §A6 made them required methods —
// each is reachable from render code like any other, so a literal default
// would defeat the literal-enforcement contract.
