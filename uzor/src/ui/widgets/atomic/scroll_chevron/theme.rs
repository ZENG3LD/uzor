//! Scroll chevron theme trait and default implementation.

/// Color slots for the scroll chevron glyph.
pub trait ScrollChevronTheme {
    /// Chevron color in idle state.
    /// Typical: `"#d1d4dc"`.
    fn scroll_chevron_color(&self) -> &str;

    /// Chevron color when hovered.
    /// Typical: `"#ffffff"`.
    fn scroll_chevron_color_hover(&self) -> &str;

    /// Chevron color when disabled (no items to scroll to).
    /// Typical: `"#4a4a4a"`.
    fn scroll_chevron_color_disabled(&self) -> &str;

    /// Hover background fill color.
    /// Typical: `"#2a2e39"`.
    fn scroll_chevron_bg_hover(&self) -> &str;
}

// =============================================================================
// Token-contract implementation
// =============================================================================
//
// `DefaultScrollChevronTheme` (a literal-colour prototype impl) was deleted
// in H1 Brief 7a — `crate::tokens::theme::TokenTheme` is now the one
// `ScrollChevronTheme` implementation ships, backed by
// `crate::ui::widgets::atomic::scroll_chevron::tokens::ScrollChevronTokens`
// (see `docs/uzor/plans/h1-token-contract-design-2026-09-24.md` §3).
