//! Checkbox theme trait and default implementation.

/// Color contract for checkbox rendering.
pub trait CheckboxTheme {
    /// Checkbox fill when checked (active state).
    fn checkbox_bg_checked(&self) -> &str;
    /// Checkbox fill when unchecked.
    fn checkbox_bg_unchecked(&self) -> &str;
    /// Checkbox border color.
    fn checkbox_border(&self) -> &str;
    /// Checkmark stroke color (the ✓ path).
    fn checkbox_checkmark(&self) -> &str;
    /// Notification-checkbox inner fill color when enabled.
    fn checkbox_notification_inner(&self) -> &str;
    /// Label text color.
    fn checkbox_label_text(&self) -> &str;
}

// =============================================================================
// Token-contract implementation
// =============================================================================
//
// `DefaultCheckboxTheme` (a literal-colour prototype impl) was deleted in H1
// Brief 5 — `crate::tokens::theme::TokenTheme` is now the one `CheckboxTheme`
// implementation ships, backed by
// `crate::ui::widgets::atomic::checkbox::tokens::CheckboxTokens` (see
// `docs/uzor/plans/h1-token-contract-design-2026-09-24.md` §3).
