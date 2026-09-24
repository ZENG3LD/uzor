//! Scrollbar colour palette.
//!
//! Two colour slots are scrollbar-specific:
//! - `thumb_normal`  / `thumb_hover`  / `thumb_active` — handle colours for each state
//! - `track_bg`      — semi-transparent track background drawn only when
//!   `ScrollbarStyle::draw_track_bg()` returns `true` (signal-group variant).
//!
//! Colour strings are `#rrggbb` hex.  Alpha is applied at draw time.

pub trait ScrollbarTheme {
    /// Handle colour when neither hovered nor dragging (Active state, 0.5 opacity).
    fn thumb_normal(&self) -> &str;
    /// Handle colour when the cursor is over the thumb (0.8 opacity).
    fn thumb_hover(&self) -> &str;
    /// Handle colour while dragging (0.8 opacity).
    fn thumb_active(&self) -> &str;
    /// Track background colour — used only when `ScrollbarStyle::draw_track_bg()`.
    /// Signal-group applies this at ~12 % opacity (`hex + "20"`).
    fn track_bg(&self) -> &str;
}

// =============================================================================
// Token-contract implementation
// =============================================================================
//
// `DefaultScrollbarTheme` (a literal-colour prototype impl) was deleted in
// H1 Brief 6 — `crate::tokens::theme::TokenTheme` is now the one
// `ScrollbarTheme` implementation ships, backed by
// `crate::ui::widgets::atomic::scrollbar::tokens::ScrollbarTokens` (see
// `docs/uzor/plans/h1-token-contract-design-2026-09-24.md` §3).
//
// `LightScrollbarTheme` (a second literal-colour impl, `#9598a1`/`#131722`/
// `#131722`/`#c8cad0`) is also deleted, not carried forward as a token
// override set — its only caller was this crate's own
// `ScrollbarSettings::standard_light()`, itself uncalled anywhere in the
// tree (grepped `uzor-examples`, `uzor-desktop`, every other widget), so
// both are dropped together. The companion doc flags this palette as
// already diverging from what `light`'s alias table auto-derives from the
// dark set's `DefaultScrollbarTheme` roles — a real light-mode audit, not an
// H1 token-contract conversion.
