//! Text widget color theme.

/// Color palette for the Text widget.
pub trait TextTheme: Send + Sync {
    /// Default text color when `view.color` is `None` and not hovered.
    fn text_color(&self) -> &str;
    /// Text color when hovered.
    fn text_color_hover(&self) -> &str;
}

// =============================================================================
// Token-contract implementation
// =============================================================================
//
// `DefaultTextTheme` (a literal-colour prototype impl) was deleted in H1
// Brief 7a — `crate::tokens::theme::TokenTheme` is now the one `TextTheme`
// implementation ships, backed by
// `crate::ui::widgets::atomic::text::tokens::TextTokens` (see
// `docs/uzor/plans/h1-token-contract-design-2026-09-24.md` §3).
