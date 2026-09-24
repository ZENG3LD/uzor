//! Toggle theme trait and default implementation.

/// Color contract for toggle rendering.
pub trait ToggleTheme {
    /// Track fill when OFF. mlc: `toolbar_item_bg_hover`.
    fn toggle_track_off(&self) -> &str;
    /// Track fill when ON. mlc: `toolbar_accent`.
    fn toggle_track_on(&self) -> &str;
    /// Thumb fill when OFF (white in mlc).
    fn toggle_thumb_off(&self) -> &str;
    /// Thumb fill when ON (white in mlc).
    fn toggle_thumb_on(&self) -> &str;
    /// Overlay applied over the whole toggle when disabled.
    fn toggle_disabled_overlay(&self) -> &str;
    /// Normal label text color.
    fn toggle_label_text(&self) -> &str;
    /// Disabled label text color.
    fn toggle_label_text_disabled(&self) -> &str;
    /// Icon color for the IconSwap variant in normal state.
    fn toggle_icon_normal(&self) -> &str;
    /// Icon color for the IconSwap variant when ON / active.
    fn toggle_icon_active(&self) -> &str;
}

// =============================================================================
// Token-contract implementation
// =============================================================================
//
// `DefaultToggleTheme` (a literal-colour prototype impl) was deleted in H1
// Brief 7b — `crate::tokens::theme::TokenTheme` is now the one `ToggleTheme`
// implementation ships, backed by
// `crate::ui::widgets::atomic::toggle::tokens::ToggleTokens` (see
// `docs/uzor/plans/h1-token-contract-design-2026-09-24.md` §3).
