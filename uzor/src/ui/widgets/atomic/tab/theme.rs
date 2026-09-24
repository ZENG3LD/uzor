//! Tab colour palette.

pub trait TabTheme {
    fn bg_normal(&self)   -> &str;
    fn bg_hover(&self)    -> &str;
    fn bg_active(&self)   -> &str;
    fn text_normal(&self) -> &str;
    fn text_active(&self) -> &str;
    fn accent(&self)      -> &str;
    fn close_normal(&self) -> &str;
    fn close_hover(&self)  -> &str;

    // -------------------------------------------------------------------------
    // Chrome variant — 2px bottom accent line on active tab.
    // Default: same as `accent`.
    // -------------------------------------------------------------------------

    /// Color of the 2px bottom accent line drawn on the active Chrome tab.
    fn chrome_bottom_accent(&self) -> &str { self.accent() }

    /// Color of the 2px bottom line drawn on a *hovered but inactive* Chrome tab.
    fn chrome_hover_line(&self) -> &str { self.bg_hover() }

    // -------------------------------------------------------------------------
    // Sidebar variant — 3px left accent bar.
    // Default: same as `accent` / `bg_active`.
    // -------------------------------------------------------------------------

    /// Color of the 3px left accent bar on the active ModalSidebar tab.
    fn sidebar_left_accent(&self) -> &str { self.accent() }

    /// Background color for the active ModalSidebar tab (used with `draw_active_rect`).
    fn sidebar_bg_active(&self) -> &str { self.bg_active() }

    // -------------------------------------------------------------------------
    // TagsTabsSidebar variant — pill with rounded rect background.
    // -------------------------------------------------------------------------

    /// Background of the pill on an *active* TagsTabsSidebar tab
    /// (should be `accent` at 0.20 opacity — callers apply opacity themselves).
    fn tags_pill_bg_active(&self) -> &str { self.accent() }

    /// Background of the pill on a *hovered* TagsTabsSidebar tab
    /// (should be `text_normal` at 0.08 opacity — callers apply opacity).
    fn tags_pill_bg_hover(&self) -> &str { self.text_normal() }
}

// =============================================================================
// Token-contract implementation
// =============================================================================
//
// `DefaultTabTheme` (a literal-colour prototype impl that overrode every
// default-body method with its own literal) was deleted in H1 Brief 7a —
// `crate::tokens::theme::TokenTheme` is now the one `TabTheme`
// implementation ships, backed by
// `crate::ui::widgets::atomic::tab::tokens::TabTokens` (see
// `docs/uzor/plans/h1-token-contract-design-2026-09-24.md` §3 and this
// trait's own `tokens.rs` module doc for which default-body methods
// `TokenTheme` overrides vs. leaves as pure delegators).
