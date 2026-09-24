//! BlackboxPanel colour palette trait; token-contract default implementation
//! lives in `crate::tokens::theme::TokenTheme`, backed by `super::tokens`.

// ---------------------------------------------------------------------------
// BlackboxTheme trait
// ---------------------------------------------------------------------------

/// Colour tokens for the blackbox panel composite.
///
/// Implement on your app theme struct to plug in custom colours.
pub trait BlackboxTheme {
    /// Panel body background fill.
    ///
    /// Default alias: `surface.floating` (`#1e222d`).
    fn bg(&self) -> &str;

    /// 1 px border colour (`WithBorder` / `WithHeaderBorder` kinds).
    ///
    /// Default alias: `border.default` (`#363a45`).
    fn border(&self) -> &str;

    /// Header strip background.
    ///
    /// Default alias: `surface.header` (`#1e222d`).
    fn header_bg(&self) -> &str;

    /// Header title text colour.
    ///
    /// Default alias: `text.on_accent` (`#d1d4dc`).
    fn header_text(&self) -> &str;

    /// 1 px divider line between header and body.
    ///
    /// Default alias: `border.default` (`#363a45`).
    fn divider(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Token-contract implementation
// ---------------------------------------------------------------------------
//
// `DefaultBlackboxTheme` (a literal-colour prototype impl) was deleted in H1
// Brief 8a — `crate::tokens::theme::TokenTheme` is now the one `BlackboxTheme`
// implementation this crate ships, backed by
// `crate::ui::widgets::composite::blackbox_panel::tokens::BlackboxPanelTokens`
// (see `docs/uzor/plans/h1-token-contract-design-2026-09-24.md` §3).
