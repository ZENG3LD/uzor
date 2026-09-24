//! Chevron colours.

pub trait ChevronTheme {
    fn color(&self)          -> &str;
    fn color_hover(&self)    -> &str;
    fn color_pressed(&self)  -> &str;
    fn color_disabled(&self) -> &str;
    fn color_active(&self)   -> &str;
    fn bg_hover(&self)       -> &str;
}

// =============================================================================
// Token-contract implementation
// =============================================================================
//
// `DefaultChevronTheme` (a literal-colour prototype impl) was deleted in H1
// Brief 5 — `crate::tokens::theme::TokenTheme` is now the one `ChevronTheme`
// implementation ships, backed by
// `crate::ui::widgets::atomic::chevron::tokens::ChevronTokens` (see
// `docs/uzor/plans/h1-token-contract-design-2026-09-24.md` §3).
