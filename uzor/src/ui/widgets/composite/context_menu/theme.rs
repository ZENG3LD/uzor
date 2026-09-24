//! ContextMenu colour palette trait; token-contract default implementation
//! lives in `crate::tokens::theme::TokenTheme`, backed by `super::tokens`.
//!
//! Shares the same default-alias vocabulary as `DropdownTheme` — both menus
//! alias the same semantic roles for the fields they hold in common (H1
//! companion doc, "context_menu"/"dropdown" sections: "that is the point"),
//! but each is its own registered `component.*` widget with its own
//! override surface.

/// Colour tokens for the ContextMenu composite.
///
/// Token names and defaults match `DropdownTheme` exactly so that a shared
/// dark-theme implementation can serve both widgets.
pub trait ContextMenuTheme {
    // --- Panel frame ---

    /// Menu panel background.  Default alias: `surface.floating`.
    fn bg(&self) -> &str;

    /// Panel border (1 px stroke).  Default alias: `border.default`.
    fn border(&self) -> &str;

    /// Shadow rect fill.  Default alias: `shadow.default`.
    fn shadow(&self) -> &str;

    // --- Items ---

    /// Normal item background (transparent / same as bg).  Default alias:
    /// `surface.floating`.
    fn item_bg_normal(&self) -> &str;

    /// Hovered item background.  Default alias: `surface.control.hover`.
    fn item_bg_hover(&self) -> &str;

    /// Danger item hover background.  Default alias: `status.danger_bg`.
    fn item_bg_danger_hover(&self) -> &str;

    /// Normal item text colour.  Default alias: `text.primary`.
    fn item_text(&self) -> &str;

    /// Hovered item text colour.  Default alias: `text.on_accent`.
    fn item_text_hover(&self) -> &str;

    /// Disabled item text colour.  Default alias: `text.disabled`.
    fn item_text_disabled(&self) -> &str;

    /// Danger item text colour (red).  Default alias: `status.danger`.
    fn item_text_danger(&self) -> &str;

    // --- Separator ---

    /// Separator line colour.  Default alias: `border.default`.
    fn separator(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Token-contract implementation
// ---------------------------------------------------------------------------
//
// `DefaultContextMenuTheme` (a literal-colour prototype impl) was deleted in
// H1 Brief 8a — `crate::tokens::theme::TokenTheme` is now the one
// `ContextMenuTheme` implementation this crate ships, backed by
// `crate::ui::widgets::composite::context_menu::tokens::ContextMenuTokens`
// (see `docs/uzor/plans/h1-token-contract-design-2026-09-24.md` §3).
