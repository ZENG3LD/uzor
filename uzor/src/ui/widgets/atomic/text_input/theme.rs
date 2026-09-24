//! Text input colour palette.
//!
//! Geometry lives in `style.rs`; this trait is colour-only.

/// Colour trait — overridable by callers via custom `impl`.
pub trait TextInputTheme: Send + Sync {
    fn bg_normal(&self)        -> [u8; 4];
    fn bg_disabled(&self)      -> [u8; 4];
    fn border_normal(&self)    -> [u8; 4];
    fn border_hover(&self)     -> [u8; 4];
    fn border_focused(&self)   -> [u8; 4];
    fn text_normal(&self)      -> [u8; 4];
    fn text_disabled(&self)    -> [u8; 4];
    fn placeholder(&self)      -> [u8; 4];
    fn selection(&self)        -> [u8; 4];
    fn cursor(&self)           -> [u8; 4];
}

// =============================================================================
// Token-contract implementation
// =============================================================================
//
// `DefaultTextInputTheme` (a literal-colour prototype impl, Win32/generic
// defaults never matched to MLC's palette at all) was deleted in H1 Brief
// 7b — `crate::tokens::theme::TokenTheme` is now the one `TextInputTheme`
// implementation ships, backed by
// `crate::ui::widgets::atomic::text_input::tokens::TextInputTokens` (see
// `docs/uzor/plans/h1-token-contract-design-2026-09-24.md` §3). This is the
// one widget whose trait returns `[u8; 4]` instead of `&str` —
// `TextInputTokens` uses the byte-array sibling of the `component_tokens!`
// macro (`component_tokens_bytes!`, `crate::tokens::component_macro`).
