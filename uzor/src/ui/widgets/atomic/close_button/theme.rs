//! Close button theme trait and default implementation.

/// Color slots for the close button X glyph and hover background.
pub trait CloseButtonTheme {
    /// X glyph color in idle state.
    /// mlc watchlist idle: `item_text_muted`. Typical: `"#787b86"`.
    fn close_button_x_color(&self) -> &str;

    /// X glyph color when hovered.
    /// mlc watchlist hover: `item_text`. Typical: `"#ffffff"`.
    fn close_button_x_color_hover(&self) -> &str;

    /// Hover background fill color.
    /// Typical: `"#2a2e39"`.
    fn close_button_bg_hover(&self) -> &str;
}

// =============================================================================
// Token-contract implementation
// =============================================================================
//
// `DefaultCloseButtonTheme` (a literal-colour prototype impl) was deleted in
// H1 Brief 5 — `crate::tokens::theme::TokenTheme` is now the one
// `CloseButtonTheme` implementation ships, backed by
// `crate::ui::widgets::atomic::close_button::tokens::CloseButtonTokens` (see
// `docs/uzor/plans/h1-token-contract-design-2026-09-24.md` §3).
