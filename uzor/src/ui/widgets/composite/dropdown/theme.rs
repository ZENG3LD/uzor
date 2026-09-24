//! Dropdown colour palette trait; token-contract default implementation
//! lives in `crate::tokens::theme::TokenTheme`, backed by `super::tokens`.
//!
//! Shares the same default-alias vocabulary as `ContextMenuTheme` — both
//! menus alias the same semantic roles for the fields they hold in common
//! (H1 companion doc: "that is the point"), but each is its own registered
//! `component.*` widget with its own override surface.

/// Colour tokens for the Dropdown composite.
///
/// Covers both the trigger button (closed state) and the open menu panel.
pub trait DropdownTheme {
    // --- Panel frame ---

    /// Menu panel background (fully opaque).  Default alias: `surface.floating`.
    fn bg(&self) -> &str;

    /// Menu panel border (1 px stroke).  Default alias: `border.default`.
    fn border(&self) -> &str;

    /// Shadow rect fill.  Default alias: `shadow.default`.
    fn shadow(&self) -> &str;

    // --- Items ---

    /// Normal item background (transparent / same as bg).  Default alias:
    /// `surface.floating`.
    fn item_bg_normal(&self) -> &str;

    /// Hovered item background.  Default alias: `surface.control.hover`.
    fn item_bg_hover(&self) -> &str;

    /// Selected / active item background.  Default alias: `accent.default`
    /// + alpha(0.15).
    fn item_bg_selected(&self) -> &str;

    /// Danger item hover background.  Default alias: `status.danger_bg`.
    fn item_bg_danger_hover(&self) -> &str;

    /// Normal item text colour.  Default alias: `text.primary`.
    fn item_text(&self) -> &str;

    /// Hovered item text colour.  Default alias: `text.on_accent`.
    fn item_text_hover(&self) -> &str;

    /// Disabled item text colour.  Default alias: `text.disabled`.
    fn item_text_disabled(&self) -> &str;

    /// Danger item text colour.  Default alias: `status.danger`.
    fn item_text_danger(&self) -> &str;

    // --- Headers ---

    /// Section header text colour.  Default alias: `text.on_accent`.
    fn header_text(&self) -> &str;

    /// Header bottom separator line colour.  Default alias: `border.default`.
    fn header_border(&self) -> &str;

    // --- Separators ---

    /// Separator line colour.  Default alias: `border.default`.
    fn separator(&self) -> &str;

    // --- Right-side content ---

    /// Shortcut / subtitle text colour (right-aligned).  Default alias:
    /// `text.disabled`.
    fn shortcut_text(&self) -> &str;

    // --- Submenu caret ---

    /// Submenu arrow / caret colour.  Default alias: `text.disabled`.
    fn caret_color(&self) -> &str;

    // --- Toggle ---

    /// Toggle track colour when on.  Default alias: `accent.default`.
    fn toggle_on(&self) -> &str;

    /// Toggle track colour when off.  Default alias: `text.disabled`.
    fn toggle_off(&self) -> &str;

    /// Toggle thumb colour.  Default alias: `text.on_accent`.
    fn toggle_thumb(&self) -> &str;

    // --- Trigger button (closed state) ---

    /// Trigger button background.  Default alias: `surface.control.hover`.
    fn trigger_bg(&self) -> &str;

    /// Trigger button background on hover.  Default alias: `border.default`
    /// (value match, odd role-name reuse — companion doc).
    fn trigger_bg_hover(&self) -> &str;

    /// Trigger button border.  Default alias: `border.default`.
    fn trigger_border(&self) -> &str;

    /// Trigger button text.  Default alias: `text.primary`.
    fn trigger_text(&self) -> &str;

    /// Trigger button chevron / arrow.  Default alias: `text.disabled`.
    fn trigger_arrow(&self) -> &str;

    // --- Checkbox (Grouped template) ---

    /// Checkbox stroke colour (unchecked).  Default alias: `text.disabled`
    /// (value match, odd role-name reuse — companion doc).
    fn checkbox_border(&self) -> &str;

    /// Checkbox fill colour (checked).  Default alias: `accent.default`.
    fn checkbox_checked(&self) -> &str;

    // --- Grid cell (Grid / Grouped templates) ---

    /// Grid cell hover background.  Default alias: `surface.control.hover`.
    fn cell_bg_hover(&self) -> &str;

    /// Grid cell border colour.  Default alias: `border.default`.
    fn cell_border(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Token-contract implementation
// ---------------------------------------------------------------------------
//
// `DefaultDropdownTheme` (a literal-colour prototype impl) was deleted in H1
// Brief 8a — `crate::tokens::theme::TokenTheme` is now the one
// `DropdownTheme` implementation this crate ships, backed by
// `crate::ui::widgets::composite::dropdown::tokens::DropdownTokens` (see
// `docs/uzor/plans/h1-token-contract-design-2026-09-24.md` §3).
