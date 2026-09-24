//! Item widget theme trait and default implementation.

/// Color slots for item text and icon.
pub trait ItemTheme {
    /// Text / icon color for the item in its normal (non-interactive) state.
    /// Typical: `"#d1d4dc"`.
    fn item_text(&self) -> &str;

    /// Text / icon color for toolbar-style items.
    /// Typical: `"#d1d4dc"`.
    fn item_toolbar_text(&self) -> &str {
        self.item_text()
    }
}

// =============================================================================
// Token-contract implementation
// =============================================================================
//
// `DefaultItemTheme` (a literal-colour prototype impl) was deleted in H1
// Brief 6 — `crate::tokens::theme::TokenTheme` is now the one `ItemTheme`
// implementation ships, backed by
// `crate::ui::widgets::atomic::item::tokens::ItemTokens` (see
// `docs/uzor/plans/h1-token-contract-design-2026-09-24.md` §3).
//
// `ToolbarItemTheme` (a second literal-colour impl, `item_text` and
// `item_toolbar_text` both `"#d1d4dc"`) is also deleted — zero call sites
// anywhere in the tree (grepped `uzor-examples`, `uzor-desktop`, every other
// widget's re-exports). Its values were identical to `DefaultItemTheme`'s
// and to what the trait's own `item_toolbar_text` default body already
// delegates to (`self.item_text()`), so nothing is lost: `TokenTheme`
// reproduces the same look through the one required method plus the
// existing delegator.
