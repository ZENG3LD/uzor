//! Radio theme trait and default implementation.

/// Color contract for radio rendering.
pub trait RadioTheme {
    /// Stroke color for the outer ring of an unselected radio button.
    fn radio_outer_border(&self) -> &str;
    /// Stroke color for the outer ring of a selected radio button.
    fn radio_outer_border_selected(&self) -> &str;
    /// Fill color for the inner dot of a selected radio button.
    fn radio_inner_dot(&self) -> &str;
    /// Overlay applied over a disabled radio.
    fn radio_disabled_overlay(&self) -> &str;
    /// Hover-row background for `Group` variant.
    fn radio_row_bg_hover(&self) -> &str;
    /// Normal label text color.
    fn radio_label_text(&self) -> &str;
    /// Label text color when the row is selected.
    fn radio_label_text_selected(&self) -> &str;
    /// Muted description text color.
    fn radio_description_text(&self) -> &str;
}

// =============================================================================
// Token-contract implementation
// =============================================================================
//
// `DefaultRadioTheme` (a literal-colour prototype impl) was deleted in H1
// Brief 6 — `crate::tokens::theme::TokenTheme` is now the one `RadioTheme`
// implementation ships, backed by
// `crate::ui::widgets::atomic::radio::tokens::RadioTokens` (see
// `docs/uzor/plans/h1-token-contract-design-2026-09-24.md` §3).
