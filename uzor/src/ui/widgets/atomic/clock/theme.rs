//! Clock widget theme trait and default implementation.

/// Color slots for the clock text and hover background.
pub trait ClockTheme {
    /// Time text color.
    /// Typical: `"#d1d4dc"`.
    fn clock_text(&self) -> &str;

    /// Hover background fill color (drawn with vertical inset).
    /// Typical: `"#2a2e39"`.
    fn clock_bg_hover(&self) -> &str;
}

// =============================================================================
// Token-contract implementation
// =============================================================================
//
// `DefaultClockTheme` (a literal-colour prototype impl) was deleted in H1
// Brief 5 — `crate::tokens::theme::TokenTheme` is now the one `ClockTheme`
// implementation ships, backed by
// `crate::ui::widgets::atomic::clock::tokens::ClockTokens` (see
// `docs/uzor/plans/h1-token-contract-design-2026-09-24.md` §3).
