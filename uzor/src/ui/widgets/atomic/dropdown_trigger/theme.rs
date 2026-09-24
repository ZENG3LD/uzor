//! DropdownTrigger theme trait — color contract for trigger rendering.

/// Theme trait for dropdown trigger colors.
pub trait DropdownTriggerTheme {
    /// Background fill for a trigger in idle state.
    /// mlc alert_settings: toolbar_theme.dropdown_bg (≈ toolbar_background).
    fn dropdown_field_bg(&self) -> &str;

    /// Background fill for a trigger on hover or when open.
    /// mlc alert_settings: toolbar_theme.item_bg_hover.
    fn dropdown_field_bg_hover(&self) -> &str;

    /// Border color for a trigger.
    /// mlc: toolbar_theme.separator.
    fn dropdown_field_border(&self) -> &str;

    /// Text color inside a trigger.
    fn dropdown_field_text(&self) -> &str;

    /// Chevron icon color used in triggers.
    /// mlc: toolbar_theme.item_text (or item_text_muted).
    fn dropdown_chevron_color(&self) -> &str;
}

// =============================================================================
// Token-contract implementation
// =============================================================================
//
// `DefaultDropdownTriggerTheme` (a literal-colour prototype impl) was
// deleted in H1 Brief 6 — `crate::tokens::theme::TokenTheme` is now the one
// `DropdownTriggerTheme` implementation ships, backed by
// `crate::ui::widgets::atomic::dropdown_trigger::tokens::DropdownTriggerTokens`
// (see `docs/uzor/plans/h1-token-contract-design-2026-09-24.md` §3).
